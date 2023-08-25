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

use poise::serenity_prelude::GuildId;
use serenity::http::Http;
use serenity::model::channel::Message;
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
pub const SOURCE_GUILD: u64 = 1003092764919091282;
pub mod emojis {
    use super::SOURCE_GUILD;
    use mindus::item::Type as Item;
    use poise::serenity_prelude::Emoji;
    use serenity::http::client::Http;
    use std::sync::OnceLock;

    macro_rules! create {
        ($($i: ident),+ $(,)?) => { paste::paste! {
            $(pub static $i: OnceLock<Emoji> = OnceLock::new();)+

            pub async fn load(c: &Http) {
                let all = c.get_emojis(SOURCE_GUILD).await.unwrap();
                for e in all {
                    match e.name.as_str() {
                        $(stringify!([< $i:lower >])=>{let _=$i.get_or_init(||e);},)+
                        _ => { /*println!("{n} unused");*/ }
                    }
                }
                $(
                    $i.get().expect(&format!("{} should be loaded", stringify!($i)));
                )+
            }
        } };
    }
    create![
        ARROW,
        COPPER,
        GRAPHITE,
        LEAD,
        SILICON,
        TITANIUM,
        SAND,
        COAL,
        PYRATITE,
        PLASTANIUM,
        SCRAP,
        SPOREPOD,
        THORIUM,
        SURGEALLOY,
        TUNGSTEN,
        PHASEFABRIC,
        OXIDE,
        METAGLASS,
        FISSILEMATTER,
        DORMANTCYST,
        CARBIDE,
        BLASTCOMPOUND,
        BERYLLIUM
    ];

    macro_rules! get {
        ($e: ident) => {
            crate::bot::emojis::$e.get().unwrap().clone()
        };
    }
    pub(crate) use get;

    pub fn item(i: Item) -> Emoji {
        use Item::*;
        // exec() when
        match i {
            Copper => get!(COPPER),
            Lead => get!(LEAD),
            Metaglass => get!(METAGLASS),
            Graphite => get!(GRAPHITE),
            Sand => get!(SAND),
            Coal => get!(COAL),
            Titanium => get!(TITANIUM),
            Thorium => get!(THORIUM),
            Scrap => get!(SCRAP),
            Silicon => get!(SILICON),
            Plastanium => get!(PLASTANIUM),
            PhaseFabric => get!(PHASEFABRIC),
            SurgeAlloy => get!(SURGEALLOY),
            SporePod => get!(SPOREPOD),
            BlastCompound => get!(BLASTCOMPOUND),
            Pyratite => get!(PYRATITE),
            Beryllium => get!(BERYLLIUM),
            Tungsten => get!(TUNGSTEN),
            Oxide => get!(OXIDE),
            Carbide => get!(CARBIDE),
            FissileMatter => get!(FISSILEMATTER),
            DormantCyst => get!(DORMANTCYST),
        }
    }
}
const PFX: &str = ">";
#[cfg(debug_assertions)]
const GUILD: u64 = SOURCE_GUILD;
#[cfg(debug_assertions)]
const CHANNEL: u64 = 1003092765581787279;
#[cfg(not(debug_assertions))]
const GUILD: u64 = 1110086242177142854;
#[cfg(not(debug_assertions))]
const CHANNEL: u64 = 1142100900442296441;

const SUCCESS: (u8, u8, u8) = (34, 139, 34);
const FAIL: (u8, u8, u8) = (255, 69, 0);
const DISABLED: (u8, u8, u8) = (112, 128, 144);

pub async fn in_guild(ctx: Context<'_>) -> Result<bool> {
    Ok(ctx.guild_id().map_or(false, |i| i.0 == GUILD))
}

pub async fn safe(m: &Message, c: &serenity::client::Context) -> String {
    let mut result = m.content.clone();

    for u in &m.mentions {
        let mut at_distinct = String::with_capacity(33);
        at_distinct.push('@');
        at_distinct.push_str(&u.nick_in(c, GuildId(GUILD)).await.unwrap_or(u.name.clone()));

        let mut m = u.mention().to_string();
        if !result.contains(&m) {
            m.insert(2, '!');
        }
        result = result.replace(&m, &at_distinct);
    }

    for id in &m.mention_roles {
        let mention = id.mention().to_string();

        if let Some(role) = id.to_role_cached(&c) {
            result = result.replace(&mention, &["@", &role.name].concat());
        } else {
            result = result.replace(&mention, "@deleted-role");
        }
    }
    result
}

pub async fn say(c: &serenity::client::Context, m: &Message, d: &Data) -> Result<()> {
    let n = m
        .author_nick(&c.http)
        .await
        .unwrap_or_else(|| m.author.name.replace("ggfenguin", "eris"));
    for l in safe(m, c).await.lines() {
        if send!(
            d.stdin,
            "say [royal] [coral][[[scarlet]{n}[coral]]:[white] {l}"
        )
        .is_err()
        {
            return Ok(());
        };
    }
    m.react(&c.http, emojis::get!(ARROW)).await?;
    Ok(())
}

pub struct Bot;
impl Bot {
    pub async fn spawn(stdout: broadcast::Receiver<String>, stdin: broadcast::Sender<String>) {
        println!("bot startup");
        let tok = std::env::var("TOKEN").unwrap_or(read_to_string("token").expect("wher token"));
        let f: poise::FrameworkBuilder<Data, anyhow::Error> = poise::Framework::builder()
            .options(poise::FrameworkOptions {
                commands: vec![
                    raw(),
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
                    start(),
                    end(),
                    help(),
                ],
                event_handler: |c, e, _, d| {
                    Box::pin(async move {
                        match e {
                            poise::Event::Ready { .. } => {
                                println!("bot ready");
                                emojis::load(&c.http).await;
                            }
                            poise::Event::Message { new_message } => {
                                if new_message.content.starts_with('!')
                                    || new_message.content.starts_with(PFX)
                                    || new_message.author.bot
                                {
                                    return Ok(());
                                }
                                if schematic::with(new_message, c).await?.is_break() {
                                    return Ok(());
                                }
                                if CHANNEL == new_message.channel_id.0 {
                                    say(c, new_message, d).await?;
                                }
                            }
                            _ => {}
                        };
                        Ok(())
                    })
                },
                on_error: |e| Box::pin(on_error(e)),
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
            .intents(GatewayIntents::all())
            .setup(|ctx, _ready, framework| {
                Box::pin(async move {
                    poise::builtins::register_in_guild(
                        ctx,
                        &framework.options().commands[..18],
                        GuildId(GUILD),
                    )
                    .await?;
                    poise::builtins::register_globally(ctx, &framework.options().commands[18..])
                        .await?;
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
                    while let Some(next) = source.source() {
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
    check = "crate::bot::in_guild",
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
    if in_guild(ctx).await.unwrap() {
        poise::builtins::help(
            ctx,
            command.as_deref(),
            poise::builtins::HelpConfiguration {
                extra_text_at_bottom: "Mindustry server management bot",
                ..Default::default()
            },
        )
        .await?;
    } else {
        ctx.say(include_str!("usage.md")).await?;
    }
    Ok(())
}
