mod admin;
mod bans;
mod config;
mod js;
pub mod maps;
mod player;
mod schematic;
mod status;
mod voting;

use crate::webhook::Webhook;
use anyhow::Result;
use maps::Maps;

use serenity::http::Http;
use serenity::prelude::*;
use std::fmt::Write;
use std::fs::read_to_string;
use std::sync::{
    atomic::{AtomicU8, Ordering},
    Arc, OnceLock,
};
use tokio::sync::broadcast;

#[derive(Debug)]
pub struct Data {
    stdin: broadcast::Sender<String>,
    vote_data: voting::Votes,
}

static SKIPPING: OnceLock<(Arc<AtomicU8>, broadcast::Sender<String>)> = OnceLock::new();

#[macro_export]
macro_rules! send {
    ($e:expr, $fmt:literal $(, $args:expr)* $(,)?) => {
        $e.send(format!($fmt $(, $args)*))
    };
}

#[macro_export]
macro_rules! send_ctx {
    ($e:expr,$fmt:literal $(, $args:expr)* $(,)?) => {
        $e.data().stdin.send(format!($fmt $(, $args)*))
    };
}

#[cfg(not(debug_assertions))]
const PFX: &'static str = ">";
#[cfg(debug_assertions)]
const PFX: &str = "-";

const SUCCESS: (u8, u8, u8) = (34, 139, 34);
const FAIL: (u8, u8, u8) = (255, 69, 0);
const DISABLED: (u8, u8, u8) = (112, 128, 144);

pub struct Bot;
impl Bot {
    pub async fn spawn(stdout: broadcast::Receiver<String>, stdin: broadcast::Sender<String>) {
        let tok = std::env::var("TOKEN").unwrap_or(read_to_string("token").expect("wher token"));
        let f: poise::FrameworkBuilder<Data, anyhow::Error> = poise::Framework::builder()
            .options(poise::FrameworkOptions {
                commands: vec![
                    raw(),
                    say(),
                    bans::add(),
                    bans::remove(),
                    bans::add_raw(),
                    bans::kick(),
                    admin::add(),
                    admin::remove(),
                    js::run(),
                    maps::list(),
                    maps::view(),
                    player::list(),
                    status::command(),
                    config::set(),
                    voting::create(),
                    voting::fixall(),
                    voting::list(),
                    schematic::draw(),
                    schematic::context_draw(),
                    start(),
                    end(),
                    help(),
                ],
                on_error: |e| Box::pin(on_error(e)),
                prefix_options: poise::PrefixFrameworkOptions {
                    edit_tracker: Some(poise::EditTracker::for_timespan(
                        std::time::Duration::from_secs(5 * 60),
                    )),
                    prefix: Some(PFX.to_string()),
                    ..Default::default()
                },
                ..Default::default()
            })
            .token(tok)
            .intents(GatewayIntents::GUILD_MESSAGES | GatewayIntents::MESSAGE_CONTENT)
            .setup(|ctx, _ready, framework| {
                Box::pin(async move {
                    poise::builtins::register_globally(ctx, &framework.options().commands).await?;
                    println!("registered");
                    Ok(Data {
                        stdin,
                        vote_data: voting::Votes::new(vec![]),
                    })
                    // todo: voting::fixall() auto
                })
            });
        tokio::spawn(async move {
            let http = Http::new("");
            let wh = std::env::var("WEBHOOK")
                .unwrap_or(read_to_string("webhook").expect("wher webhook"));
            let mut wh = Webhook::new(&http, &wh).await;
            SKIPPING.get_or_init(|| (wh.skip.clone(), wh.skipped.clone()));
            wh.link(stdout).await;
        });
        f.run().await.unwrap();
    }
}

type Context<'a> = poise::Context<'a, Data, anyhow::Error>;

async fn on_error(error: poise::FrameworkError<'_, Data, anyhow::Error>) {
    use poise::FrameworkError::Command;
    match error {
        Command { error, ctx } => {
            let mut msg;
            {
                let mut chain = error.chain();
                msg = format!("e: `{}`", chain.next().unwrap());
                for mut source in chain {
                    write!(msg, "from: `{source}`").unwrap();
                    loop {
                        let Some(next) = source.source() else { break };
                        write!(msg, "from: `{next}`").unwrap();
                        source = next;
                    }
                }
            }
            let bt = error.backtrace();
            if bt.status() == std::backtrace::BacktraceStatus::Captured {
                let parsed = btparse::deserialize(dbg!(error.backtrace())).unwrap();
                let mut s = vec![];
                for frame in parsed.frames {
                    if let Some(line) = frame.line
                        && (frame.function.contains("panel")
                        || frame.function.contains("poise")
                        || frame.function.contains("serenity")
                        || frame.function.contains("mindus")
                        || frame.function.contains("image"))
                        {
                            s.push(format!("l{}@{}", line, frame.function));
                        }
                }
                s.truncate(15);
                write!(msg, "trace: ```rs\n{}\n```", s.join("\n")).unwrap();
            }
            ctx.say(msg).await.unwrap();
        }
        err => poise::builtins::on_error(err).await.unwrap(),
    }
}

#[poise::command(
    prefix_command,
    required_permissions = "ADMINISTRATOR",
    default_member_permissions = "ADMINISTRATOR",
    category = "Control",
    track_edits
)]
/// send a raw command to the server
async fn raw(
    ctx: Context<'_>,
    #[description = "Command"]
    #[rest]
    cmd: String,
) -> Result<()> {
    send_ctx!(ctx, "{cmd}")?;
    Ok(())
}

#[macro_export]
macro_rules! return_next {
    ($ctx:expr) => {{
        let line = $crate::bot::get_nextblock().await;
        $ctx.send(|m| m.content(line)).await?;
        return Ok(());
    }};
}

async fn get_nextblock() -> String {
    let (skip_count, skip_send) = SKIPPING.get().unwrap();

    skip_count.fetch_add(1, Ordering::Relaxed);

    skip_send
        .subscribe()
        .recv()
        .await
        .unwrap_or("._?".to_string())
}

#[poise::command(
    slash_command,
    category = "Control",
    default_member_permissions = "ADMINISTRATOR",
    required_permissions = "ADMINISTRATOR"
)]
/// say something as the server
async fn say(ctx: Context<'_>, #[description = "Message"] message: String) -> Result<()> {
    ctx.data().stdin.send(format!("say {message}"))?;
    return_next!(ctx)
}

pub fn strip_colors(from: &str) -> String {
    let mut result = String::new();
    result.reserve(from.len());
    let mut level: u8 = 0;
    for c in from.chars() {
        if c == '[' {
            level += 1;
        } else if c == ']' {
            level -= 1;
        } else if level == 0 {
            result.push(c);
        }
    }
    result
}

#[poise::command(
    slash_command,
    default_member_permissions = "ADMINISTRATOR",
    required_permissions = "ADMINISTRATOR",
    category = "Control"
)]
/// start the game.
pub async fn start(
    ctx: Context<'_>,
    #[description = "the map"]
    #[autocomplete = "maps::autocomplete"]
    map: String,
) -> Result<()> {
    send_ctx!(ctx, "host {}", Maps::find(&map, &ctx.data().stdin).await)?;
    return_next!(ctx)
}

#[poise::command(
    slash_command,
    category = "Control",
    default_member_permissions = "ADMINISTRATOR",
    required_permissions = "ADMINISTRATOR"
)]
/// end the game.
pub async fn end(
    ctx: Context<'_>,
    #[description = "the map to go to"]
    #[autocomplete = "maps::autocomplete"]
    map: String,
) -> Result<()> {
    send_ctx!(
        ctx,
        "gameover {}",
        Maps::find(&map, &ctx.data().stdin).await
    )?;
    return_next!(ctx)
}

#[poise::command(prefix_command, slash_command, track_edits, category = "Info")]
/// show help and stuff
pub async fn help(
    ctx: Context<'_>,
    #[description = "command to show help about"]
    #[autocomplete = "poise::builtins::autocomplete_command"]
    command: Option<String>,
) -> Result<()> {
    poise::builtins::help(
        ctx,
        command.as_deref(),
        poise::builtins::HelpConfiguration {
            extra_text_at_bottom: "Mindustry server management bot",
            ..Default::default()
        },
    )
    .await?;
    Ok(())
}
