use super::{Context, Result};
use crate::bot::player::{self, Players};
use crate::{return_next, send_ctx};

#[poise::command(
    slash_command,
    category = "Control",
    rename = "ban",
    required_permissions = "ADMINISTRATOR",
    default_member_permissions = "ADMINISTRATOR"
)]
/// ban a ingame player by uuid and ip
pub async fn add(
    ctx: Context<'_>,
    #[description = "player to ban"]
    #[autocomplete = "player::autocomplete"]
    player: String,
) -> Result<()> {
    let _ = ctx.defer().await;
    let player = Players::find(&ctx.data().stdin, player)
        .await
        .unwrap()
        .unwrap();
    send_ctx!(ctx, "ban ip {}", player.ip)?;
    send_ctx!(ctx, "ban id {}", player.uuid)?;
    return_next!(ctx)
}

#[poise::command(
    slash_command,
    category = "Control",
    required_permissions = "ADMINISTRATOR",
    default_member_permissions = "ADMINISTRATOR"
)]
/// kick somebody off the server
pub async fn kick(
    ctx: Context<'_>,
    #[description = "player to ban"]
    #[autocomplete = "player::autocomplete"]
    player: String,
) -> Result<()> {
    let _ = ctx.defer().await;
    let player = Players::find(&ctx.data().stdin, player)
        .await
        .unwrap()
        .unwrap();
    send_ctx!(ctx, "kick {}", player.uuid)?; // FIXME
    return_next!(ctx)
}

#[poise::command(
    slash_command,
    category = "Control",
    rename = "ban_raw",
    required_permissions = "ADMINISTRATOR",
    default_member_permissions = "ADMINISTRATOR"
)]
/// ban a player by uuid and/or ip
pub async fn add_raw(
    ctx: Context<'_>,
    #[description = "uuid of player to ban"] uuid: Option<String>,
    #[description = "ip address of player to ban"] ip: Option<String>,
) -> Result<()> {
    let _ = ctx.defer().await;
    if uuid.is_none() && ip.is_none() {
        anyhow::bail!("what are you banning? yourself?")
    }
    if let Some(uuid) = uuid {
        send_ctx!(ctx, "ban id {}", uuid)?;
    }
    if let Some(ip) = ip {
        send_ctx!(ctx, "ban ip {}", ip)?;
    }
    return_next!(ctx)
}

#[poise::command(
    slash_command,
    category = "Control",
    rename = "unban",
    default_member_permissions = "ADMINISTRATOR",
    required_permissions = "ADMINISTRATOR"
)]
/// unban a player by uuid or ip
pub async fn remove(
    ctx: Context<'_>,
    #[description = "Player id/ip"]
    #[rename = "ip_or_id"]
    player: String,
) -> Result<()> {
    let _ = ctx.defer().await;
    send_ctx!(ctx, "unban {}", player)?;
    return_next!(ctx)
}

// TODO: listbans
