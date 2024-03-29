use super::{return_next, send_ctx, Context, Result};
use crate::bot::player::{self, Players};

#[poise::command(
    slash_command,
    category = "Configuration",
    rename = "add_admin",
    required_permissions = "ADMINISTRATOR",
    default_member_permissions = "ADMINISTRATOR"
)]
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
    return_next!(ctx)
}

#[poise::command(
    slash_command,
    category = "Configuration",
    rename = "remove_admin",
    required_permissions = "ADMINISTRATOR",
    default_member_permissions = "ADMINISTRATOR"
)]
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
    return_next!(ctx)
}
