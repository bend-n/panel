use super::{get_nextblock, strip_colors, Context, Result, SUCCESS};
use crate::send;
use futures_util::StreamExt;
use image::{codecs::png::PngEncoder, ImageEncoder};
use mindus::*;
use oxipng::{optimize_from_memory as compress, Options};
use poise::serenity_prelude::*;
use std::borrow::Cow;
use std::sync::LazyLock;
use std::time::Instant;
use tokio::sync::broadcast;
use tokio::sync::OnceCell;
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

#[poise::command(slash_command, prefix_command, category = "Info", rename = "maps")]
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

static REG: LazyLock<mindus::block::BlockRegistry> = LazyLock::new(build_registry);

#[poise::command(slash_command, prefix_command, category = "Info")]
/// look at the current game.
pub async fn view(ctx: Context<'_>) -> Result<()> {
    let _ = ctx.defer_or_broadcast().await;
    send!(ctx.data().stdin, "save 0")?;
    let _ = get_nextblock().await;

    // parsing the thing doesnt negate the need for a env var sooo
    let o = std::fs::read(std::env::var("SAVE_PATH").unwrap())?;
    let then = Instant::now();
    let m = MapSerializer(&REG).deserialize(&mut mindus::data::DataRead::new(&o))?;
    let deser_took = then.elapsed();
    let name = m.tags.get("mapname").map_or("<unknown>", |v| v);
    let render_took = Instant::now();
    let i = m.render();
    let render_took = render_took.elapsed();
    let encoding_took = Instant::now();
    let mut b = vec![];
    PngEncoder::new(&mut b).write_image(&i, i.width(), i.height(), image::ColorType::Rgba8)?;
    let encoding_took = encoding_took.elapsed();
    let from = b.len();
    let compression_took = Instant::now();
    b = compress(&b, &Options::from_preset(0)).unwrap();
    use super::status::{humanize_bytes as human, Size};
    let size_from = human(Size::B(from as f64));
    let size_to = human(Size::B(b.len() as f64));
    let compression_took = compression_took.elapsed();
    let took = then.elapsed();
    poise::send_reply(ctx, |m| {
        m.attachment(AttachmentType::Bytes {
            data: Cow::Owned(b),
            filename: "0.png".to_string(),
        })
        .embed(|e| e.attachment("0.png").color(SUCCESS).footer(|f| f.text(format!("render of {name} took: {:.2}s (deser: {}ms, render: {:.2}s, encoding: {}ms, compression: {:.2}s ({size_from} -> {size_to}))", took.as_secs_f32(), deser_took.as_millis(), render_took.as_secs_f32(), encoding_took.as_millis(), compression_took.as_secs_f32()))))
    })
    .await?;
    Ok(())
}
