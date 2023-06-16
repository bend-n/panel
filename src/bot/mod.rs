mod maps;
mod player;

use crate::webhook::Webhook;
use anyhow::{anyhow, Result};
use itertools::Itertools;
use maps::Maps;
use minify_js::TopLevelMode;
use player::Players;
use regex::Regex;
use serenity::http::{CacheHttp, Http};
use serenity::prelude::*;
use std::fs::read_to_string;
use std::str::FromStr;
use std::sync::{Arc, LazyLock, Mutex, OnceLock};
use tokio::sync::broadcast;

pub struct Data {
    stdin: broadcast::Sender<String>,
}

static SKIPPING: OnceLock<(Arc<Mutex<u8>>, broadcast::Sender<String>)> = OnceLock::new();

#[macro_export]
macro_rules! send {
    ($e:expr, $fmt:literal $(, $args:expr)* $(,)?) => {
        $e.send(format!($fmt $(, $args)*))
    };
}

macro_rules! send_ctx {
    ($e:expr,$fmt:literal $(, $args:expr)* $(,)?) => {
        $e.data().stdin.send(format!($fmt $(, $args)*))
    };
}

#[cfg(not(debug_assertions))]
const PFX: &'static str = ">";
#[cfg(debug_assertions)]
const PFX: &'static str = "-";

const SUCCESS: (u8, u8, u8) = (34, 139, 34);
const FAIL: (u8, u8, u8) = (255, 69, 0);

pub struct Bot;
impl Bot {
    pub async fn spawn(stdout: broadcast::Receiver<String>, stdin: broadcast::Sender<String>) {
        let tok = std::env::var("TOKEN").unwrap_or(read_to_string("token").expect("wher token"));
        let f: poise::FrameworkBuilder<Data, anyhow::Error> = poise::Framework::builder()
            .options(poise::FrameworkOptions {
                commands: vec![
                    raw(),
                    say(),
                    ban(),
                    unban(),
                    add_admin(),
                    remove_admin(),
                    js(),
                    maps(),
                    players(),
                    status(),
                    start(),
                    end(),
                    help(),
                ],
                prefix_options: poise::PrefixFrameworkOptions {
                    edit_tracker: Some(poise::EditTracker::for_timespan(
                        std::time::Duration::from_secs(2 * 60),
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
                    Ok(Data { stdin })
                })
            });

        tokio::spawn(async move {
            let http = Http::new("");
            let mut wh = Webhook::new(&http, &std::env::var("WEBHOOK").expect("no webhook!")).await;
            SKIPPING.get_or_init(|| (wh.skip.clone(), wh.skipped.clone()));
            wh.link(stdout).await;
        });
        tokio::spawn(async move {
            f.run().await.unwrap();
        });
    }
}

type Context<'a> = poise::Context<'a, Data, anyhow::Error>;

#[poise::command(
    prefix_command,
    required_permissions = "USE_SLASH_COMMANDS",
    category = "Control"
)]
/// send a raw command to the server
async fn raw(
    ctx: Context<'_>,
    #[description = "Command"]
    #[rest]
    cmd: String,
) -> Result<()> {
    send_ctx!(ctx, "{cmd}")?;
    println!("sent");
    Ok(())
}

macro_rules! return_next {
    ($ctx:expr) => {{
        let line = get_nextblock().await;
        $ctx.send(|m| m.content(line)).await?;
        Ok(())
    }};
}

async fn get_nextblock() -> String {
    let (skip_count, skip_send) = SKIPPING.get().unwrap();
    {
        *skip_count.lock().unwrap() += 1;
    }
    skip_send
        .subscribe()
        .recv()
        .await
        .unwrap_or("._?".to_string())
}

#[poise::command(slash_command, category = "Control")]
/// say something as the server
async fn say(ctx: Context<'_>, #[description = "Message"] message: String) -> Result<()> {
    let _ = ctx.defer().await;
    ctx.data().stdin.send(format!("say {message}"))?;
    return_next!(ctx)
}

#[poise::command(slash_command, category = "Control")]
/// ban a player by uuid and ip
async fn ban(
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

#[poise::command(slash_command, category = "Control")]
/// unban a player by uuid or ip
async fn unban(
    ctx: Context<'_>,
    #[description = "Player id/ip"]
    #[rename = "ip_or_id"]
    player: String,
) -> Result<()> {
    let _ = ctx.defer().await;
    send_ctx!(ctx, "unban {}", player)?;
    return_next!(ctx)
}

#[poise::command(
    prefix_command,
    required_permissions = "USE_SLASH_COMMANDS",
    category = "Control",
    track_edits
)]
/// run arbitrary javascript
async fn js(
    ctx: Context<'_>,
    #[description = "Script"]
    #[rest]
    script: String,
) -> Result<()> {
    static RE: LazyLock<Regex> =
        LazyLock::new(|| Regex::new(r#"```(js|javascript)?([^`]+)```"#).unwrap());
    let _ = ctx.channel_id().start_typing(&ctx.serenity_context().http);
    let mat = RE
        .captures(&script)
        .ok_or(anyhow::anyhow!(r#"no code found (use \`\`\`js...\`\`\`."#))?;
    let script = mat.get(2).unwrap().as_str();
    let mut out = vec![];
    let script = if minify_js::minify(TopLevelMode::Global, script.into(), &mut out).is_err() {
        std::borrow::Cow::from(script.replace('\n', ";")) // xd
    } else {
        String::from_utf8_lossy(&out)
    };
    send_ctx!(ctx, "js {script}")?;
    let line = get_nextblock().await;
    ctx.send(|m| m.content(line)).await?;
    Ok(())
}

fn strip_colors(from: &str) -> String {
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
    required_permissions = "USE_SLASH_COMMANDS",
    category = "Info"
)]
/// lists the maps.
pub async fn maps(ctx: Context<'_>) -> Result<()> {
    let _ = ctx.defer().await;
    let maps = Maps::get_all(&ctx.data().stdin).await;
    poise::send_reply(ctx, |m| {
        m.embed(|e| {
            for (k, v) in maps.iter().enumerate() {
                e.field((k + 1).to_string(), v, true);
            }
            e.description("map list.").color(SUCCESS)
        })
    })
    .await?;
    Ok(())
}

#[poise::command(prefix_command, slash_command, category = "Info")]
/// server status.
pub async fn status(ctx: Context<'_>) -> Result<()> {
    let _ = ctx.defer_or_broadcast().await;
    send_ctx!(ctx, "status")?;
    let block = tokio::select! {
        block = get_nextblock() => block,
        _ = async_std::task::sleep(std::time::Duration::from_secs(5)) =>
            { poise::send_reply(ctx, |m| m.embed(|e| e.title("server down").color(FAIL))).await?; return Ok(()) },
    };
    let (tps, mem, pcount) = block
        .split('/')
        .map(|s| u32::from_str(s.trim().split_once(' ').unwrap().0).unwrap())
        .collect_tuple()
        .ok_or(anyhow!("couldnt split block"))?;
    poise::send_reply(ctx, |m| {
        m.embed(|e| {
            if pcount > 0 {
                e.footer(|f| f.text("see /players for player list"));
            }
            e.title("server online")
                .field("tps", tps, true)
                .field("memory use", mem, true)
                .field("players", pcount, true)
                .color(SUCCESS)
        })
    })
    .await?;
    Ok(())
}

#[poise::command(slash_command, prefix_command, category = "Info")]
/// lists the currently online players.
pub async fn players(ctx: Context<'_>) -> Result<()> {
    let _ = ctx.defer().await;
    let players = Players::get_all(&ctx.data().stdin).await.unwrap().clone();
    let perms = ctx
        .partial_guild()
        .await
        .unwrap()
        .member_permissions(ctx.http(), ctx.author().id)
        .await?;
    // let perms = ctx
    //     .author_member()
    //     .await
    //     .ok_or(anyhow!("couldnt get perms"))?
    //     .permissions(ctx.cache().unwrap())?;
    poise::send_reply(ctx, |m| {
        m.embed(|e| {
            if players.is_empty() {
                return e.title("no players online.").color(FAIL);
            }
            e.fields(players.into_iter().map(|p| {
                let admins = if p.admin { " [A]" } else { "" };
                (
                    p.name,
                    if perms.use_slash_commands() {
                        format!("{id}, {ip}", id = p.uuid, ip = p.ip) + admins
                    } else {
                        admins.to_string()
                    },
                    true,
                )
            }));
            e.description("currently online players.").color(SUCCESS)
        })
    })
    .await?;
    Ok(())
}

#[poise::command(
    slash_command,
    required_permissions = "USE_SLASH_COMMANDS",
    category = "Control"
)]
/// start the game.
pub async fn start(
    ctx: Context<'_>,
    #[description = "the map"]
    #[autocomplete = "maps::autocomplete"]
    map: String,
) -> Result<()> {
    let _ = ctx.defer().await;
    send_ctx!(ctx, "host {}", Maps::find(&map, &ctx.data().stdin).await)?;
    return_next!(ctx)
}

#[poise::command(
    slash_command,
    category = "Control",
    required_permissions = "USE_SLASH_COMMANDS"
)]
/// end the game.
pub async fn end(
    ctx: Context<'_>,
    #[description = "the map to go to"]
    #[autocomplete = "maps::autocomplete"]
    map: String,
) -> Result<()> {
    let _ = ctx.defer().await;
    send_ctx!(
        ctx,
        "gameover {}",
        Maps::find(&map, &ctx.data().stdin).await
    )?;
    return_next!(ctx)
}

#[poise::command(slash_command, category = "Configuration")]
/// make somebody a admin
pub async fn add_admin(
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

#[poise::command(slash_command, category = "Configuration")]
/// remove the admin status
pub async fn remove_admin(
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

#[poise::command(
    prefix_command,
    slash_command,
    required_permissions = "USE_SLASH_COMMANDS",
    track_edits,
    category = "Info"
)]
/// show help and stuff
pub async fn help(
    ctx: Context<'_>,
    #[description = "Specific command to show help about"]
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
