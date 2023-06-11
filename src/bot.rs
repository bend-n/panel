use crate::webhook::Webhook;
use anyhow::Result;
use futures_util::StreamExt;
use minify_js::TopLevelMode;
use regex::Regex;
use serenity::http::Http;
use serenity::prelude::*;
use std::fs::read_to_string;
use std::sync::{Arc, Mutex, OnceLock};
use strum::{AsRefStr, EnumString, EnumVariantNames, VariantNames};
use tokio::sync::broadcast;
use tokio::sync::OnceCell as TokLock;
pub struct Data {
    stdin: broadcast::Sender<String>,
}

static SKIPPING: OnceLock<(Arc<Mutex<u8>>, broadcast::Sender<String>)> = OnceLock::new();

pub struct Bot;
impl Bot {
    pub async fn new(stdout: broadcast::Receiver<String>, stdin: broadcast::Sender<String>) {
        let tok = std::env::var("TOKEN").unwrap_or(read_to_string("token").expect("wher token"));
        let f: poise::FrameworkBuilder<Data, anyhow::Error> = poise::Framework::builder()
            .options(poise::FrameworkOptions {
                commands: vec![raw(), say(), ban(), js(), maps(), start(), end(), help()],
                prefix_options: poise::PrefixFrameworkOptions {
                    prefix: Some(">".to_string()),
                    ..Default::default()
                },
                ..Default::default()
            })
            .token(&tok)
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
    category = "ADMINISTRATION"
)]
/// send a raw command to the server
async fn raw(
    ctx: Context<'_>,
    #[description = "Command"]
    #[rest]
    cmd: String,
) -> Result<()> {
    ctx.data().stdin.send(cmd)?;
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

#[poise::command(slash_command, category = "ADMINISTRATION")]
/// say something as the server
async fn say(ctx: Context<'_>, #[description = "Message"] message: String) -> Result<()> {
    let _ = ctx.defer();
    ctx.data().stdin.send(format!("say {message}"))?;
    return_next!(ctx)
}

#[derive(EnumString, EnumVariantNames, AsRefStr)]
#[strum(serialize_all = "snake_case")]
enum BanType {
    Id,
    Ip,
    Name,
}

async fn autocomplete_ban<'a>(
    _ctx: Context<'_>,
    partial: &'a str,
) -> impl futures::Stream<Item = String> + 'a {
    futures::stream::iter(BanType::VARIANTS)
        .filter(move |name| futures::future::ready(name.starts_with(partial)))
        .map(|name| name.to_string())
}

#[poise::command(slash_command, category = "ADMINISTRATION")]
/// ban a player
async fn ban(
    ctx: Context<'_>,
    #[autocomplete = "autocomplete_ban"] ban_type: BanType,
    #[description = "Player (id/ip/name)"] player: String,
) -> Result<()> {
    let _ = ctx.defer();
    ctx.data()
        .stdin
        .send(format!("ban {} {player}", ban_type.as_ref()))?;
    return_next!(ctx)
}

#[poise::command(
    prefix_command,
    required_permissions = "USE_SLASH_COMMANDS",
    category = "ADMINISTRATION",
    track_edits
)]
/// run arbitrary javascript
async fn js(
    ctx: Context<'_>,
    #[description = "Script"]
    #[rest]
    script: String,
) -> Result<()> {
    static RE: OnceLock<Regex> = OnceLock::new();
    let _ = ctx.channel_id().start_typing(&ctx.serenity_context().http);
    let re = RE.get_or_init(|| Regex::new(r#"```(js|javascript)?([^`]+)```"#).unwrap());
    let mat = re
        .captures(&script)
        .ok_or(anyhow::anyhow!(r#"no code found (use \`\`\`js...\`\`\`."#))?;
    let script = mat.get(2).unwrap().as_str();
    let mut out = vec![];
    let script = if let Err(_) = minify_js::minify(TopLevelMode::Global, script.into(), &mut out) {
        std::borrow::Cow::from(script.replace('\n', ";")) // xd
    } else {
        String::from_utf8_lossy(&out)
    };
    ctx.data().stdin.send(format!("js {script}"))?;
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

static MAPS: TokLock<Vec<String>> = TokLock::const_new();
async fn get_maps(stdin: &broadcast::Sender<String>) -> &Vec<String> {
    MAPS.get_or_init(|| async move {
        stdin.send(format!("listmaps")).unwrap();
        let res = get_nextblock().await;
        let mut vec = vec![];
        for line in res.lines() {
            if let Some((_, name)) = line.split_once(':') {
                vec.push(strip_colors(name));
            }
        }
        vec
    })
    .await
}

#[poise::command(slash_command, required_permissions = "USE_SLASH_COMMANDS")]
/// lists the maps.
pub async fn maps(ctx: Context<'_>) -> Result<()> {
    let _ = ctx.defer();
    let maps = get_maps(&ctx.data().stdin).await;
    poise::send_reply(ctx, |m| {
        m.embed(|e| {
            for (k, v) in maps.iter().enumerate() {
                e.field((k + 1).to_string(), v, true);
            }
            e.description("map list.")
        })
    })
    .await?;
    Ok(())
}

async fn autocomplete_map<'a>(
    ctx: Context<'a>,
    partial: &'a str,
) -> impl futures::Stream<Item = String> + 'a {
    futures::stream::iter(get_maps(&ctx.data().stdin).await)
        .filter(move |name| futures::future::ready(name.starts_with(partial)))
        .map(|name| name.to_string())
}

async fn mapi(map: &str, stdin: &broadcast::Sender<String>) -> usize {
    get_maps(stdin).await.iter().position(|r| r == map).unwrap()
}

#[poise::command(slash_command, required_permissions = "USE_SLASH_COMMANDS")]
/// start the game.
pub async fn start(
    ctx: Context<'_>,
    #[description = "the map"]
    #[autocomplete = "autocomplete_map"]
    map: String,
) -> Result<()> {
    let _ = ctx.defer();
    ctx.data()
        .stdin
        .send(format!("plague {}", mapi(&map, &ctx.data().stdin).await))
        .unwrap();
    return_next!(ctx)
}

#[poise::command(slash_command, required_permissions = "USE_SLASH_COMMANDS")]
/// end the game.
pub async fn end(
    ctx: Context<'_>,
    #[description = "the map to go to"]
    #[autocomplete = "autocomplete_map"]
    map: String,
) -> Result<()> {
    let _ = ctx.defer();
    ctx.data()
        .stdin
        .send(format!("endplague {}", mapi(&map, &ctx.data().stdin).await))
        .unwrap();
    return_next!(ctx)
}

#[poise::command(
    prefix_command,
    slash_command,
    required_permissions = "USE_SLASH_COMMANDS",
    track_edits
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
