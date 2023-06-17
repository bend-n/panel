use super::{Context, Result};
use crate::bot::player::{self, Players};
use crate::send_ctx;

#[poise::command(slash_command, category = "Configuration", rename = "add_admin")]
/// make somebody a admin
pub async fn add(
    ctx: Context<'_>,
    #[description = "The player to make admin"]
    #[autocomplete = "player::autocomplete"]
    player: String,
) -> Result<()> {
    let player = Players::find(&ctx.data().stdin, player)
        .await
        .unwrap()
        .unwrap();
    send_ctx!(ctx, "admin add {}", player.uuid)?;
    Ok(())
}

#[poise::command(slash_command, category = "Configuration", rename = "remove_admin")]
/// remove the admin status
pub async fn remove(
    ctx: Context<'_>,
    #[description = "The player to remove admin status from"]
    #[autocomplete = "player::autocomplete"]
    player: String,
) -> Result<()> {
    let player = Players::find(&ctx.data().stdin, player)
        .await
        .unwrap()
        .unwrap();
    send_ctx!(ctx, "admin remove {}", player.uuid)?;
    Ok(())
}
