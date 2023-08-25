use anyhow::{anyhow, Result};
use mindus::data::DataRead;
use mindus::*;
use oxipng::*;
use poise::serenity_prelude::*;
use regex::Regex;
use std::sync::LazyLock;
use std::{borrow::Cow, ops::ControlFlow};

use super::{emojis, strip_colors, SUCCESS};

static RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(```)?(\n)?([^`]+)(\n)?(```)?").unwrap());

async fn from_attachments(attchments: &[Attachment]) -> Result<Option<Schematic<'_>>> {
    for a in attchments {
        if a.filename.ends_with("msch") {
            let s = a.download().await?;
            let mut s = DataRead::new(&s);
            let Ok(s) = Schematic::deserialize(&mut s) else {
                continue;
            };
            return Ok(Some(s));
        // discord uploads base64 as a file when its too long
        } else if a.filename == "message.txt" {
            let Ok(s) = String::from_utf8(a.download().await?) else {
                continue;
            };
            let Ok(s) = Schematic::deserialize_base64(&s) else {
                continue;
            };
            return Ok(Some(s));
        }
    }
    Ok(None)
}

pub async fn with(m: &Message, c: &serenity::client::Context) -> Result<ControlFlow<(), ()>> {
    let send = |v| async move {
        let p = to_png(&v);
        let author = m.author_nick(c).await.unwrap_or(m.author.name.clone());
        m.channel_id
            .send_message(c, |m| {
                m.add_file(AttachmentType::Bytes {
                    data: Cow::Owned(p),
                    filename: "image.png".to_string(),
                })
                .embed(|e| {
                    e.attachment("image.png");
                    if let Some(d) = v.tags.get("description") {
                        e.description(d);
                    }
                    let mut s = String::new();
                    for (i, n) in v.compute_total_cost().0.iter() {
                        if n == 0 {
                            continue;
                        }
                        use std::fmt::Write;
                        write!(s, "{} {n} ", emojis::item(i)).unwrap();
                    }
                    e.field("", s, true);
                    e.title(strip_colors(v.tags.get("name").unwrap()))
                        .footer(|f| f.text(format!("requested by {author}",)))
                        .color(SUCCESS)
                })
            })
            .await?;
        anyhow::Ok(())
    };

    if let Ok(Some(v)) = from_attachments(&m.attachments).await {
        send(v).await?;
        return Ok(ControlFlow::Break(()));
    }
    if let Ok(v) = from_msg(&m.content) {
        send(v).await?;
        return Ok(ControlFlow::Break(()));
    }
    Ok(ControlFlow::Continue(()))
}

pub fn to_png(s: &Schematic<'_>) -> Vec<u8> {
    let p = s.render();
    let p = RawImage::new(
        p.width(),
        p.height(),
        ColorType::RGB {
            transparent_color: None,
        },
        BitDepth::Eight,
        p.buffer,
    )
    .unwrap();
    p.create_optimized_png(&oxipng::Options {
        filter: indexset! { RowFilter::None },
        bit_depth_reduction: false,
        color_type_reduction: false,
        palette_reduction: false,
        grayscale_reduction: false,
        ..oxipng::Options::from_preset(0)
    })
    .unwrap()
}

fn from_msg<'l>(msg: &str) -> Result<Schematic<'l>> {
    let schem_text = RE
        .captures(msg)
        .ok_or(anyhow!("couldnt find schematic"))?
        .get(3)
        .unwrap()
        .as_str();
    Ok(Schematic::deserialize_base64(schem_text)?)
}
