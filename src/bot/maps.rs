use super::{get_nextblock, strip_colors, Context, Result, SUCCESS};
use crate::send;
use futures_util::StreamExt;
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
        .map(|name| name.to_string())
}

#[poise::command(
    slash_command,
    prefix_command,
    required_permissions = "USE_SLASH_COMMANDS",
    category = "Info",
    rename = "maps"
)]
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
