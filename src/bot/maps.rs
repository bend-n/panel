use super::{get_nextblock, strip_colors, Context, Result, SUCCESS};
use crate::send;
use futures_util::StreamExt;
use mindus::*;
use oxipng::*;
use poise::serenity_prelude::*;
use std::borrow::Cow;
use std::sync::atomic::{AtomicU64, Ordering::Relaxed};
use std::time::{Duration, Instant, SystemTime};
use tokio::sync::broadcast::{self, Sender};
use tokio::sync::{MutexGuard, OnceCell};
pub struct Maps;
impl Maps {
    pub async fn find(map: &str, stdin: &broadcast::Sender<String>) -> usize {
        Self::get_all(stdin)
            .await
            .iter()
            .position(|r| r == map)
            .unwrap()
    }

    pub async fn get_all(stdin: &broadcast::Sender<String>) -> &Vec<String> {
        static MAPS: OnceCell<Vec<String>> = OnceCell::const_new();
        MAPS.get_or_init(|| async move {
            send!(stdin, "maps").unwrap();
            let res = get_nextblock().await;
            let mut vec = vec![];
            for line in res.lines() {
                if let Some((_, name)) = line.split_once(':') {
                    vec.push(strip_colors(name));
                }
            }
            vec
        })
        .await
    }
}

pub async fn autocomplete<'a>(
    ctx: Context<'a>,
    partial: &'a str,
) -> impl futures::Stream<Item = String> + 'a {
    futures::stream::iter(Maps::get_all(&ctx.data().stdin).await)
        .filter(move |name| futures::future::ready(name.starts_with(partial)))
        .map(ToString::to_string)
}

#[poise::command(slash_command, category = "Info", rename = "maps")]
/// lists the maps.
pub async fn list(ctx: Context<'_>) -> Result<()> {
    let _ = ctx.defer_or_broadcast().await;
    let maps = Maps::get_all(&ctx.data().stdin).await;
    poise::send_reply(ctx, |m| {
        m.embed(|e| {
            for (k, v) in maps.iter().enumerate() {
                e.field((k + 1).to_string(), v, true);
            }
            e.description("map list.").color(SUCCESS)
        })
    })
    .await?;
    Ok(())
}

pub struct RenderInfo {
    deserialization: Duration,
    render: Duration,
    compression: Duration,
    total: Duration,
    name: String,
}
pub static MAP_IMAGE: MapImage = MapImage(Mutex::const_new(vec![]), AtomicU64::new(0));
pub struct MapImage(Mutex<Vec<u8>>, AtomicU64);
impl MapImage {
    /// procure the map image.
    pub async fn get(
        &self,
        stdin: &Sender<String>,
        // returning a guard is questionable
    ) -> Result<(MutexGuard<Vec<u8>>, Option<RenderInfo>)> {
        let mut lock = self.0.lock().await;
        // me in a million years when its 1901 and we never get a new render
        let now = SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .unwrap()
            .as_secs();
        Ok(
            if self
                .1
                .fetch_update(Relaxed, Relaxed, |then| (now > then + 70).then_some(now))
                .is_err()
            {
                (lock, None)
            } else {
                send!(stdin, "save 0")?;
                let _ = get_nextblock().await;

                // parsing the thing doesnt negate the need for a env var sooo
                let o = std::fs::read(std::env::var("SAVE_PATH").unwrap())?;
                let then = Instant::now();
                let m = Map::deserialize(&mut mindus::data::DataRead::new(&o))?;
                let deser_took = then.elapsed();
                let name = m.tags.get("mapname").unwrap().to_owned();
                let render_took = Instant::now();
                let i = m.render();
                let render_took = render_took.elapsed();
                let compression_took = Instant::now();
                let i = RawImage::new(
                    i.width(),
                    i.height(),
                    ColorType::RGB {
                        transparent_color: None,
                    },
                    BitDepth::Eight,
                    i.take_buffer(),
                )
                .unwrap();
                *lock = i
                    .create_optimized_png(&oxipng::Options {
                        filter: indexset! { RowFilter::None },
                        bit_depth_reduction: false,
                        color_type_reduction: false,
                        palette_reduction: false,
                        grayscale_reduction: false,
                        ..oxipng::Options::from_preset(0)
                    })
                    .unwrap();
                let compression_took = compression_took.elapsed();
                let total = then.elapsed();
                (
                    lock,
                    Some(RenderInfo {
                        deserialization: deser_took,
                        render: render_took,
                        compression: compression_took,
                        name,
                        total,
                    }),
                )
            },
        )
    }
}

#[poise::command(slash_command, category = "Info")]
/// look at the current game.
pub async fn view(ctx: Context<'_>) -> Result<()> {
    let _ = ctx.defer_or_broadcast().await;
    let (i, info) = MAP_IMAGE.get(&ctx.data().stdin).await?;
    poise::send_reply(ctx, |m| {
        m.attachment(AttachmentType::Bytes {
            data: Cow::Borrowed(&i),
            filename: "0.png".to_string(),
        })
        .embed(|e| {
            if let Some(RenderInfo { deserialization, render, compression, total, name }) = info {
                e.footer(|f| f.text(format!("render of {name} took: {:.3}s (deser: {}ms, render: {:.3}s, compression: {:.3}s)", total.as_secs_f32(), deserialization.as_millis(), render.as_secs_f32(), compression.as_secs_f32())));
            }
            e.attachment("0.png").color(SUCCESS)
        })
    })
    .await?;
    Ok(())
}
