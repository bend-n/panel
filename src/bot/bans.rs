use super::{Context, Result};
use crate::bot::player::{self, Players};
use crate::{return_next, send_ctx};

#[poise::command(slash_command, category = "Control", rename = "ban")]
/// ban a player by uuid and ip
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

#[poise::command(slash_command, category = "Control", rename = "unban")]
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
