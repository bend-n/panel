use super::{get_nextblock, Context, SUCCESS};
use anyhow::Result;
use emoji::named::*;
use poise::serenity_prelude::*;
use std::net::Ipv4Addr;

#[derive(serde_derive::Deserialize)]
struct PlayerInfo {
    #[serde(rename = "i")]
    id: String,
    #[serde(rename = "ln")]
    last_name: String,
    #[serde(rename = "lp")]
    last_ip: Ipv4Addr,
    #[serde(rename = "is")]
    ips: Vec<Ipv4Addr>,
    #[serde(rename = "ns")]
    names: Vec<String>,
    #[serde(rename = "t")]
    times_joined: usize,
    #[serde(rename = "a")]
    admin: bool,
}

#[poise::command(slash_command, category = "Info")]
/// trace a player
/// find out all about them
pub async fn trace(
    ctx: Context<'_>,
    #[autocomplete = "super::player::autocomplete"] player: String,
) -> Result<()> {
    super::send_ctx!(ctx, "trace {player}").unwrap();
    let res = get_nextblock().await;
    let info = res
        .lines()
        .filter(|x| !x.is_empty())
        .map(serde_json::from_str::<PlayerInfo>)
        .map(Result::unwrap);
    let authorized = match ctx {
        poise::Context::Application(x) => x
            .author_member()
            .await
            .map(|x| x.roles.clone())
            .unwrap_or(vec![])
            .iter()
            .any(|&x| x == 1133416252791074877),
        _ => unreachable!(),
    };
    let mut r = poise::CreateReply::default().ephemeral(authorized);
    for found in info {
        let mut e = CreateEmbed::new()
            .field(
                "name",
                if found.admin {
                    format!("{} <{ADMIN}>", found.last_name)
                } else {
                    found.last_name
                },
                true,
            )
            .field(
                "all names used",
                found
                    .names
                    .into_iter()
                    .intersperse("|".to_string())
                    .fold(String::new(), |acc, x| acc + &x),
                true,
            )
            .field("has joined", found.times_joined.to_string(), true)
            .color(SUCCESS);
        if authorized {
            e = e
                .field("uuid", found.id, true)
                .field("last used ip", found.last_ip.to_string(), true)
                .field(
                    "all ips used",
                    found
                        .ips
                        .into_iter()
                        .map(|x| x.to_string())
                        .intersperse("|".to_string())
                        .fold(String::new(), |acc, x| acc + &x),
                    true,
                );
        }
        r = r.embed(e);
    }
    poise::send_reply(ctx, r).await?;
    Ok(())
}
