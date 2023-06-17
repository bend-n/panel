use super::{Context, Result};
use crate::{return_next, send_ctx};
use minify_js::TopLevelMode;
use regex::Regex;
use std::sync::LazyLock;

fn parse_js(from: &str) -> Result<String> {
    static RE: LazyLock<Regex> =
        LazyLock::new(|| Regex::new(r#"```(js|javascript)?([^`]+)```"#).unwrap());
    let mat = RE
        .captures(&from)
        .ok_or(anyhow::anyhow!(r#"no code found (use \`\`\`js...\`\`\`."#))?;
    let script = mat.get(2).unwrap().as_str();
    let mut out = vec![];
    Ok(
        if minify_js::minify(TopLevelMode::Global, script.into(), &mut out).is_ok() {
            String::from_utf8_lossy(&out).to_string()
        } else {
            script.replace('\n', ";")
        },
    )
}

#[poise::command(
    prefix_command,
    required_permissions = "USE_SLASH_COMMANDS",
    category = "Control",
    track_edits,
    rename = "js"
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

#[test]
fn test_parse_js() {
    assert!(
        parse_js("```js\nLog.info(4)\nLog.info(4+2)\n```").unwrap() == "Log.info(4);Log.info(4+ 2)" // hmm
    )
}
