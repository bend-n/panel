use super::{strip_colors, Context, SUCCESS};
use anyhow::{anyhow, Result};
use image::{codecs::png::PngEncoder, ImageEncoder};
use mindus::data::schematic::R64Error;
use mindus::data::{DataRead, Serializer};
use mindus::*;
use poise::serenity_prelude::*;
use regex::Regex;
use std::borrow::Cow;
use std::path::Path;
use std::sync::LazyLock;

static RE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r#"(```)?([^`]+)(```)?"#).unwrap());
static REG: LazyLock<mindus::block::BlockRegistry> = LazyLock::new(build_registry);

#[poise::command(context_menu_command = "Render schematic", category = "Info")]
/// draw schematic.
pub async fn context_draw(ctx: Context<'_>, msg: Message) -> Result<()> {
    let _ = ctx.defer_or_broadcast().await;

    if let Some(a) = msg.attachments.get(0)
        && let Some(e) = Path::new(&a.filename).extension()
        && e == "msch" {
            let mut ss = SchematicSerializer(&REG);
            let s = a.download().await?;
            let mut s = DataRead::new(&s);
            let Ok(s) = ss.deserialize(&mut s) else {
                ctx.say(format!("invalid schematic ({})", a.filename)).await?;
                return Ok(());
            };
            return send(&ctx, &s).await;
    }
    draw_impl(ctx, &msg.content).await
}

#[poise::command(
    prefix_command,
    slash_command,
    category = "Info",
    rename = "draw_schematic"
)]
/// server status.
pub async fn draw(ctx: Context<'_>, schematic: String) -> Result<()> {
    let _ = ctx.defer_or_broadcast().await;
    draw_impl(ctx, &schematic).await
}

async fn send(ctx: &Context<'_>, s: &Schematic<'_>) -> Result<()> {
    let mut b = vec![];
    let p = s.render();
    PngEncoder::new(&mut b).write_image(&p, p.width(), p.height(), image::ColorType::Rgba8)?;
    let n = strip_colors(s.tags.get("name").unwrap());
    let filename = "image.png";
    poise::send_reply(*ctx, |m| {
        m.attachment(AttachmentType::Bytes {
            data: Cow::Owned(b),
            filename: filename.to_string(),
        })
        .embed(|e| {
            if let Some(d) = s.tags.get("description") {
                e.description(d);
            }
            e.title(n).attachment(filename).color(SUCCESS)
        })
    })
    .await?;
    Ok(())
}

async fn draw_impl(ctx: Context<'_>, msg: &str) -> Result<()> {
    let mut ss = SchematicSerializer(&REG);
    let schem_text = RE
        .captures(msg)
        .ok_or(anyhow!("couldnt find schematic"))?
        .get(2)
        .unwrap()
        .as_str();
    let s = match ss.deserialize_base64(schem_text) {
        Err(e) => {
            ctx.say(match e {
                R64Error::Base64(_) => "invalid base64",
                R64Error::Content(_) => "invalid schematic",
            })
            .await?;
            return Ok(());
        }
        Ok(x) => x,
    };
    send(&ctx, &s).await
}
