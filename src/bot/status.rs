use super::{get_nextblock, Context, FAIL, SUCCESS};
use crate::send_ctx;
use anyhow::Result;
use itertools::Itertools;
use std::str::FromStr;

fn parse(line: &str) -> Option<(u32, u32, u32)> {
    line.split('/')
        .map(|s| u32::from_str(s.trim().split_once(' ').unwrap().0).unwrap())
        .collect_tuple()
}

#[allow(dead_code)]
pub enum Size {
    Gb(f64),
    Mb(f64),
    Kb(f64),
    B(f64),
}
const UNIT: f64 = 1024.0;

impl Size {
    pub fn bytes(self) -> f64 {
        match self {
            Self::B(x) => x,
            Self::Kb(x) => x * UNIT,
            Self::Mb(x) => x * UNIT * UNIT,
            Self::Gb(x) => x * UNIT * UNIT * UNIT,
        }
    }
}
// https://git.sr.ht/~f9/human_bytes
pub fn humanize_bytes<T: Into<Size>>(bytes: T) -> String {
    const SUFFIX: [&str; 4] = ["B", "KB", "MB", "GB"];
    let size = dbg!(bytes.into().bytes());

    if size <= 0.0 {
        return "0 B".to_string();
    }

    let base = size.log10() / UNIT.log10();

    let result = format!("{:.1}", UNIT.powf(base - base.floor()),)
        .trim_end_matches(".0")
        .to_owned();

    // Add suffix
    [&result, SUFFIX[base.floor() as usize]].join(" ")
}

#[poise::command(prefix_command, slash_command, category = "Info", rename = "status")]
/// server status.
pub async fn command(ctx: Context<'_>) -> Result<()> {
    let _ = ctx.defer_or_broadcast().await;
    send_ctx!(ctx, "status")?;
    let block = tokio::select! {
        block = get_nextblock() => block,
        _ = async_std::task::sleep(std::time::Duration::from_secs(5)) =>
            { poise::send_reply(ctx, |m| m.embed(|e| e.title("server down").color(FAIL))).await?; return Ok(()) },
    };
    let (tps, mem, pcount) =
        parse(&block).ok_or(anyhow::anyhow!("couldnt split block {block}."))?;
    poise::send_reply(ctx, |m| {
        m.embed(|e| {
            if pcount > 0 {
                e.footer(|f| f.text("see /players for player list"));
            }
            e.title("server online")
                .field("tps", tps, true)
                .field("memory use", humanize_bytes(Size::Mb(mem as f64)), true)
                .field("players", pcount, true)
                .color(SUCCESS)
        })
    })
    .await?;
    Ok(())
}

#[test]
fn test_parse() {
    assert!(parse("57 TPS / 274 MB / 7 PLAYERS") == Some((57, 274, 7)));
}

#[test]
fn test_bytes() {
    assert!(humanize_bytes(Size::B(0.0)) == "0 B");
    assert!(humanize_bytes(Size::B(550.0)) == "550 B");
    assert!(humanize_bytes(Size::Kb(550.0)) == "550 KB");
    assert!(humanize_bytes(Size::Mb(650.0)) == "650 MB");
    assert!(humanize_bytes(Size::Gb(15.3)) == "15.3 GB");
}
