use super::{get_nextblock, strip_colors, Context, FAIL, SUCCESS};
use crate::send;
use anyhow::Result;
use futures_util::StreamExt;
use itertools::Itertools;
use std::net::Ipv4Addr;
use std::str::FromStr;
use std::time::Instant;
use tokio::sync::{broadcast, MappedMutexGuard, Mutex, MutexGuard};

#[derive(Clone, Debug)]
pub struct Player {
    pub admin: bool,
    pub name: String,
    pub uuid: String,
    pub ip: Ipv4Addr,
}

static PLAYERS: Mutex<(Vec<Player>, Option<Instant>)> = Mutex::const_new((vec![], None));

async fn update(
    stdin: &broadcast::Sender<String>,
) -> Result<MutexGuard<(Vec<Player>, Option<Instant>)>> {
    let mut lock = PLAYERS.lock().await;
    if lock.1.is_none() || lock.1.unwrap().elapsed().as_millis() > 500 {
        lock.0 = get_players(stdin).await?;
        lock.1 = Some(Instant::now());
    }
    Ok(lock)
}
pub struct Players {}
impl Players {
    pub async fn get_all(
        stdin: &broadcast::Sender<String>,
    ) -> Result<MappedMutexGuard<Vec<Player>>> {
        {
            Ok(MutexGuard::map(update(stdin).await?, |(p, _)| p))
        }
    }

    pub async fn find(
        stdin: &broadcast::Sender<String>,
        name: String,
    ) -> Result<Option<MappedMutexGuard<Player>>> {
        Ok(MutexGuard::try_map(update(stdin).await?, |(p, _)| {
            p.iter_mut().find(|x| x.name == name)
        })
        .ok())
    }
}

async fn get_players(stdin: &broadcast::Sender<String>) -> Result<Vec<Player>> {
    let mut players = vec![];
    send!(stdin, "players")?;
    let recv = get_nextblock().await;
    for line in recv.lines() {
        if line.starts_with("No") {
            break;
        } else if line.is_empty() {
            continue;
        }
        if let Some((first, uuid, ip)) = line.split('|').collect_tuple() {
            if let Some((admin, name)) = first.split_once(' ') {
                players.push(Player {
                    admin: admin == "[A]",
                    name: strip_colors(name),
                    uuid: uuid.to_owned(),
                    ip: Ipv4Addr::from_str(ip).unwrap(),
                });
            }
        }
    }
    Ok(players)
}

pub async fn autocomplete<'a>(
    ctx: Context<'a>,
    partial: &'a str,
) -> impl futures::Stream<Item = String> + 'a {
    let x = Players::get_all(&ctx.data().stdin).await.unwrap().clone();
    futures::stream::iter(x)
        .filter(move |p| futures::future::ready(p.name.starts_with(partial)))
        .map(|p| p.name)
}

#[poise::command(slash_command, category = "Info", rename = "players")]
/// lists the currently online players.
pub async fn list(ctx: Context<'_>) -> Result<()> {
    let _ = ctx.defer().await;
    let players = Players::get_all(&ctx.data().stdin).await.unwrap().clone();
    poise::send_reply(ctx, |m| {
        m.embed(|e| {
            if players.is_empty() {
                return e.title("no players online.").color(FAIL);
            }
            e.fields(players.into_iter().map(|p| {
                let admins = if p.admin {
                    "<:admin:1182128872435749005>"
                } else {
                    ""
                };
                (p.name, admins, true)
            }))
            .description("currently online players.")
            .color(SUCCESS)
        })
    })
    .await?;
    Ok(())
}
