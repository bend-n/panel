use super::{strip_colors, Context, SUCCESS};
use anyhow::{anyhow, Result};
use image::{codecs::png::PngEncoder, ImageEncoder};
use mindus::data::{DataRead, DataWrite, Serializer};
use mindus::*;
use poise::serenity_prelude::*;
use regex::Regex;
use std::borrow::Cow;
use std::path::Path;
use std::sync::LazyLock;

static RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r#"(```)?(\n)?([^`]+)(\n)?(```)?"#).unwrap());
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
            let s = ss.deserialize(&mut s).map_err(|e| anyhow!("invalid schematic: {e} with file {}", a.filename))?;
            return send(&ctx, &s, false).await;
    }
    draw_impl(ctx, &msg.content, false).await
}

#[poise::command(
    prefix_command,
    slash_command,
    category = "Info",
    rename = "draw_schematic"
)]
/// draw schematic.
pub async fn draw(ctx: Context<'_>, schematic: String) -> Result<()> {
    let _ = ctx.defer_or_broadcast().await;
    draw_impl(ctx, &schematic, true).await
}

async fn send(ctx: &Context<'_>, s: &Schematic<'_>, send_schematic: bool) -> Result<()> {
    let mut b = vec![];
    let p = s.render();
    PngEncoder::new(&mut b).write_image(&p, p.width(), p.height(), image::ColorType::Rgba8)?;
    let n = strip_colors(s.tags.get("name").unwrap());
    poise::send_reply(*ctx, |m| {
        if send_schematic {
            let mut out = DataWrite::default();
            SchematicSerializer(&REG).serialize(&mut out, s).unwrap();
            m.attachment(AttachmentType::Bytes {
                data: Cow::Owned(out.consume()),
                filename: "schem.msch".to_string(),
            });
        }
        m.attachment(AttachmentType::Bytes {
            data: Cow::Owned(b),
            filename: "image.png".to_string(),
        })
        .embed(|e| {
            if let Some(d) = s.tags.get("description") {
                e.description(d);
            }
            if send_schematic {
                e.attachment("schem.msch");
            }
            e.title(n).attachment("image.png").color(SUCCESS)
        })
    })
    .await?;
    Ok(())
}

async fn draw_impl(ctx: Context<'_>, msg: &str, send_schematic: bool) -> Result<()> {
    let mut ss = SchematicSerializer(&REG);
    let schem_text = RE
        .captures(msg)
        .ok_or(anyhow!("couldnt find schematic"))?
        .get(3)
        .unwrap()
        .as_str();
    let s = ss
        .deserialize_base64(schem_text)
        .map_err(|e| anyhow!("schematic deserializatiion failed: {e}"))?;
    send(&ctx, &s, send_schematic).await
}
