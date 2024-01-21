use super::{return_next, send_ctx, Context, Result};
use regex::Regex;
use std::sync::LazyLock;

fn parse_js(from: &str) -> Result<String> {
    static RE: LazyLock<Regex> =
        LazyLock::new(|| Regex::new(r"```(js|javascript)?([^`]+)```").unwrap());
    let mat = RE
        .captures(from)
        .ok_or(anyhow::anyhow!(r#"no code found (use \`\`\`js...\`\`\`."#))?;
    Ok(mat.get(2).unwrap().as_str().replace('\n', ";"))
}

#[poise::command(
    prefix_command,
    required_permissions = "ADMINISTRATOR",
    category = "Control",
    track_edits,
    rename = "js",
    check = "crate::bot::in_guild"
)]
/// run arbitrary javascript
pub async fn run(
    ctx: Context<'_>,
    #[description = "Script"]
    #[rest]
    script: String,
) -> Result<()> {
    let _ = ctx.channel_id().start_typing(&ctx.serenity_context().http);
    let script = parse_js(&script)?;
    send_ctx!(ctx, "js {script}")?;
    return_next!(ctx)
}
