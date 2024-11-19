use super::{Context, Result};
use crate::emoji::named::*;
use poise::serenity_prelude::*;
use std::process::ExitStatus;
use tokio::sync::broadcast::{channel, error::TryRecvError as ChannelE};
use tokio::sync::oneshot::{channel as oneshot, error::TryRecvError as OneE};

pub async fn sysadmin(c: Context<'_>) -> Result<bool> {
    let c = c.author_member().await.ok_or(anyhow::anyhow!("dang"))?;
    Ok(sys_ck(&c))
}

pub fn sys_ck(c: &Member) -> bool {
    c.user.name == "bendn" || c.roles.contains(&RoleId::new(1113997024220696606))
}

#[poise::command(
    slash_command,
    check = "sysadmin",
    category = "System Administration",
    default_member_permissions = "ADMINISTRATOR",
    guild_only
)]
/// Executes any command. Please no `rm -rf /*`.
/// Executes in a shell, so you can >|& as you desire.
pub async fn exec(
    c: Context<'_>,
    #[description = "command to run in bash"] command: String,
) -> Result<()> {
    #[derive(Debug, Clone)]
    enum Payload {
        Stdout(String),
        Stderr(String),
        Exit(ExitStatus),
    }
    let (otx, mut orx) = channel::<Payload>(16);
    let (ttx, mut trx) = oneshot::<()>();
    let cc = command.clone();
    tokio::task::spawn(async move {
        let mut c = tokio::process::Command::new("bash")
            .arg("-c")
            .arg(cc)
            .env("FORCE_COLOR", "1")
            .current_dir("/root")
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            .spawn()
            .unwrap();
        let mut stdout = c.stdout.take().unwrap();
        let mut o = Box::new([0; 1 << 20]);
        let mut stderr = c.stderr.take().unwrap();
        loop {
            match trx.try_recv() {
                Err(e) => match e {
                    OneE::Closed => _ = c.kill().await,
                    OneE::Empty => {}
                },
                Ok(()) => c.kill().await.unwrap(),
            }
            tokio::select! {
                n = tokio::io::AsyncReadExt::read(&mut stdout, &mut *o) => {
                    let n = n.unwrap();
                    if n != 0 {
                        let string = String::from_utf8_lossy(&o[..n]).into_owned();
                        otx.send(Payload::Stdout(string)).unwrap();
                    }
                },
                () = tokio::time::sleep(tokio::time::Duration::from_millis(50)) => (),
            };
            tokio::select! {
                n = tokio::io::AsyncReadExt::read(&mut stderr, &mut *o) => {
                    let n = n.unwrap();
                    if n != 0 {
                        let string = String::from_utf8_lossy(&o[..n]).into_owned();
                        otx.send(Payload::Stderr(string)).unwrap();
                    }
                },
                () = tokio::time::sleep(tokio::time::Duration::from_millis(50)) => (),
            };
            if let Ok(Some(n)) = c.try_wait() {
                otx.send(Payload::Exit(n)).unwrap();
                break;
            }
            tokio::time::sleep(tokio::time::Duration::from_millis(25)).await;
        }
    });
    let h = poise::send_reply(
        c,
        poise::CreateReply::default()
            .content("<a:lod:1198459385790337164>\u{200b}")
            .components(vec![CreateActionRow::Buttons(vec![CreateButton::new(
                format!("{}_ctrl_c", c.id()),
            )
            .label("C^c")
            .style(ButtonStyle::Danger)
            .emoji(CANCEL.parse::<ReactionType>().unwrap())])]),
    )
    .await?;
    let mut dat = String::new();
    let mut ttx = Some(ttx);
    let cc = c.serenity_context().clone();
    let cid = c.id();
    let k = tokio::spawn(async move {
        while let Some(x) = tokio::select! {
            x =async { ComponentInteractionCollector::new(&cc)
            .filter(move |press| press.data.custom_id.starts_with(&cid.to_string()))
            .timeout(std::time::Duration::from_secs(60 * 60)).await }=> x,
            () = tokio::time::sleep(tokio::time::Duration::from_millis(100)) => None,
        } {
            if sys_ck(x.member.as_ref().unwrap()) {
                _ = ttx.take().unwrap().send(());
            } else {
                x.create_followup(
                    &cc,
                    CreateInteractionResponseFollowup::default()
                        .ephemeral(true)
                        .content("not admin"),
                )
                .await
                .unwrap();
            }
        }
    });

    loop {
        match orx.try_recv() {
            Ok(Payload::Stdout(x) | Payload::Stderr(x)) => {
                dat.push_str(&x);
                if dat.len() > 1900 {
                    let mut i = dat.len() - 1900;
                    while !dat.is_char_boundary(i) {
                        i -= 1;
                    }
                    dat.drain(0..i);
                }
                h.edit(
                    c,
                    poise::CreateReply::default().content(format!("```ansi\n{dat}\n```")),
                )
                .await?;
            }
            Ok(Payload::Exit(x)) => {
                let e = if x.success() { OK } else { CANCEL };
                h.edit(
                    c,
                    poise::CreateReply::default()
                        .content(format!("{e} ```ansi\n{dat}\n```"))
                        .components(vec![]),
                )
                .await?;
                k.abort();
                break;
            }
            Err(ChannelE::Empty | ChannelE::Lagged(_)) => {}
            Err(ChannelE::Closed) => panic!(),
        }
        tokio::time::sleep(std::time::Duration::from_millis(200)).await;
    }

    Ok(())
}
