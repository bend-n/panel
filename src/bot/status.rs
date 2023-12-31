use super::{get_nextblock, Context, FAIL, SUCCESS};
use crate::send_ctx;
use anyhow::Result;
use itertools::Itertools;
use poise::serenity_prelude::*;
use std::str::FromStr;
use tokio::time::{sleep, Duration};

fn parse(line: &str) -> Option<(u32, u32, u32)> {
    let mut v = vec![];
    for piece in line.split('/') {
        v.push(u32::from_str(piece.trim().split_once(' ')?.0).ok()?);
    }
    v.into_iter().collect_tuple()
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
    let size = bytes.into().bytes();

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

#[poise::command(slash_command, category = "Info", rename = "status")]
/// server status.
pub async fn command(ctx: Context<'_>) -> Result<()> {
    let _ = ctx.defer_or_broadcast().await;
    send_ctx!(ctx, "status")?;
    macro_rules! fail {
        ($ctx:expr,$fail:expr) => {{
            poise::send_reply(
                ctx,
                poise::CreateReply::default()
                    .embed(CreateEmbed::new().title("server down").color($fail)),
            )
            .await?;
            return Ok(());
        }};
    }
    let block = tokio::select! {
        block = get_nextblock() => block,
        _ = sleep(Duration::from_secs(5)) => fail!(ctx, FAIL),
    };
    let Some((tps, mem, pcount)) = parse(&block) else {
        fail!(ctx, FAIL);
    };
    poise::send_reply(
        ctx,
        poise::CreateReply::default().embed(if pcount > 0 {
            CreateEmbed::new()
                .title("server online")
                .field("tps", format!("{tps}"), true)
                .field("memory use", humanize_bytes(Size::Mb(f64::from(mem))), true)
                .field("players", format!("{pcount}"), true)
                .color(SUCCESS)
                .footer(CreateEmbedFooter::new("see /players for player list"))
        } else {
            CreateEmbed::new().title("no players online").color(FAIL)
        }),
    )
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
    assert!(humanize_bytes(Size::Mb(2000.0)) == "2 GB");
    assert!(humanize_bytes(Size::Gb(15.3)) == "15.3 GB");
}
