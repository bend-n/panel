use super::{Context, Result};
use lemu::Executor;
use poise::{serenity_prelude::AttachmentType, KeyValueArgs};
use regex::Regex;
use std::{borrow::Cow, sync::LazyLock};

#[poise::command(prefix_command, category = "Misc", track_edits, rename = "eval")]
pub async fn run(
    ctx: Context<'_>,
    #[description = "number of iterations"] kv: KeyValueArgs,
    #[rest]
    #[description = "Script"]
    from: String,
) -> Result<()> {
    let _ = ctx.channel_id().start_typing(&ctx.serenity_context().http);

    static RE: LazyLock<Regex> =
        LazyLock::new(|| Regex::new(r#"```(arm|x86asm)?([^`]+)```"#).unwrap());

    let lemu::Output {
        output: Some(output),
        displays,
        ..
    } = ({
        let mat = RE
            .captures(&from)
            .ok_or(anyhow::anyhow!(r#"no code found (use \`\`\`arm...\`\`\`."#))?;
        let script = mat.get(2).unwrap().as_str();
        match Executor::with_output(vec![])
            .display()
            .limit_iterations(
                kv.get("iters")
                    .map_or(1, |v| v.parse::<usize>().unwrap_or(1).clamp(1, 50)),
            )
            .limit_instructions(30000)
            .program(&script)
        {
            Ok(mut v) => {
                v.run();
                v.output()
            }
            Err(e) => {
                let s = format!("{}", e.diagnose(script)).replace("`", "\u{200b}`");
                ctx.send(|c| {
                    c.allowed_mentions(|a| a.empty_parse())
                        .content(format!("```ansi\n{s}\n```"))
                })
                .await?;
                return Ok(());
            }
        }
    })
    else {
        unreachable!()
    };
    let displays: Box<[_; 1]> = displays.try_into().unwrap();
    let [display] = *displays;

    ctx.send(|c| {
        let mut empty = true;
        if !output.is_empty() {
            c.content(format!(
                "```\n{}\n```",
                String::from_utf8_lossy(&output).replace('`', "\u{200b}`")
            ));
            empty = false;
        }
        if display.buffer().iter().any(|&n| n != 0) {
            let p = oxipng::RawImage::new(
                display.width(),
                display.height(),
                oxipng::ColorType::RGBA,
                oxipng::BitDepth::Eight,
                display.take_buffer(),
            )
            .unwrap();
            let p = p
                .create_optimized_png(&oxipng::Options {
                    filter: oxipng::indexset! { oxipng::RowFilter::None },
                    bit_depth_reduction: false,
                    color_type_reduction: false,
                    palette_reduction: false,
                    grayscale_reduction: false,
                    ..oxipng::Options::from_preset(0)
                })
                .unwrap();
            c.attachment(AttachmentType::Bytes {
                data: Cow::from(p),
                filename: "display1.png".to_string(),
            });
            c.embed(|e| e.attachment("display1.png"));
            empty = false;
        }
        if empty {
            c.content("no output");
        }
        c.allowed_mentions(|a| a.empty_parse())
    })
    .await?;

    Ok(())
}
