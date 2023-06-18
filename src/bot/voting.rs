use super::{Context, DISABLED, SUCCESS};
use ::serenity::builder::CreateActionRow;
use ::serenity::builder::CreateEmbed;
use anyhow::Result;
use itertools::Itertools;
use poise::serenity_prelude::CollectComponentInteraction as Interaction;
use poise::serenity_prelude::*;
use std::collections::HashMap;
use std::str::FromStr;
use std::sync::Mutex;
use std::time::SystemTime;

pub type Vote = usize;
pub enum VoteData {
    Running(HashMap<UserId, Vote>),
    Finished(Vec<usize>),
}

impl VoteData {
    pub fn get(&mut self) -> &mut HashMap<UserId, Vote> {
        match self {
            Self::Running(x) => x,
            Self::Finished(_) => unreachable!(),
        }
    }

    pub fn summarize_running(&self, optcount: usize) -> Vec<usize> {
        match self {
            Self::Running(s) => {
                let mut ret = vec![];
                ret.resize(optcount, 0);
                for (_, v) in s {
                    ret[*v] += 1
                }
                ret
            }
            Self::Finished(_) => unreachable!(),
        }
    }

    pub fn get_summarized(&self) -> &Vec<usize> {
        match self {
            Self::Finished(ret) => ret,
            Self::Running(_) => unreachable!(),
        }
    }

    pub fn finish(self, optcount: usize) -> Self {
        Self::Finished(self.summarize_running(optcount))
    }
}

pub type Votes = Mutex<Vec<VoteData>>;

trait Imgor {
    fn imageor<S: ToString>(&mut self, img: Option<S>) -> &mut Self;
}

impl Imgor for CreateEmbed {
    fn imageor<S: ToString>(&mut self, img: Option<S>) -> &mut Self {
        if let Some(iuri) = img {
            self.image(iuri)
        } else {
            self
        }
    }
}

#[poise::command(slash_command, category = "Discord", rename = "create_vote")]
/// make a vote
pub async fn create(
    ctx: Context<'_>,
    #[description = "picture url"] image: Option<String>,
    #[description = "pressables (psv)"] options: String,
    #[description = "option styles (psv)"] styles: String,
    #[description = "how long the vote will be up"] length: String,
    title: String,
) -> Result<()> {
    let ctx_id = ctx.id();
    let options = options.split('|').map(|s| s.trim()).collect_vec();
    let styles = styles
        .split('|')
        .map(|s| s.trim().to_lowercase())
        .map(|s| {
            use ButtonStyle::*;
            match s.as_str() {
                "blue" => Primary,
                "gray" => Secondary,
                "green" => Success,
                "red" => Danger,
                _ => Primary,
            }
        })
        .collect_vec();
    let dur = if let Ok(t) = parse_duration::parse(&length) {
        t
    } else {
        ctx.say(format!("`{length}` is not time")).await?;
        return Ok(());
    };
    let end = format!(
        "<t:{}:R>",
        (SystemTime::now() + dur)
            .duration_since(SystemTime::UNIX_EPOCH)?
            .as_secs()
    );
    macro_rules! update_msg {
        // give me unhygenic macros
        ($m:expr, $ctx:expr, $image:expr, $title:expr, $options:expr, $ctx_id:expr, $n:expr) => {
            $m.embed(|e| {
                for (option, votes) in $ctx.data().vote_data.lock().unwrap()[$n]
                    .summarize_running($options.len())
                    .iter()
                    .enumerate()
                {
                    e.field(&$options[option], votes, true);
                }
                e.imageor($image.as_ref())
                    .color(SUCCESS)
                    .title(&$title)
                    .description(format!("vote ends {end}"))
            })
        };
    }
    let n = {
        let mut data = ctx.data().vote_data.lock().unwrap();
        let n = data.len();
        data.push(VoteData::Running(HashMap::new()));
        n
    };
    let handle = poise::send_reply(ctx, |m| {
        update_msg!(m, ctx, image, title, options, ctx_id, n).components(|c| {
            c.create_action_row(|r| {
                for (n, option) in options.iter().enumerate() {
                    r.create_button(|b| {
                        b.custom_id(format!("{}{n}", ctx_id))
                            .label(option)
                            .style(styles[n])
                    });
                }
                r
            })
        })
    })
    .await?;
    let ctx_id_len = ctx_id.to_string().len();
    while let Some(press) = Interaction::new(ctx)
        .filter(move |press| press.data.custom_id.starts_with(&ctx_id.to_string()))
        .timeout(dur)
        .await
    {
        let s = {
            if ctx.data().vote_data.lock().unwrap()[n]
                .get()
                .insert(
                    press.user.id,
                    Vote::from_str(&press.data.custom_id[ctx_id_len..]).unwrap(),
                )
                .is_some()
            {
                "updated"
            } else {
                "voted"
            }
        };
        println!("got vote!");
        tokio::join!(
            press.create_followup_message(ctx, |m| {
                m.ephemeral(true).embed(|e| e.title(s).color(SUCCESS))
            }),
            press.create_interaction_response(ctx, |c| {
                c.kind(InteractionResponseType::UpdateMessage)
                    .interaction_response_data(|m| {
                        update_msg!(m, ctx, image, title, options, ctx_id, n)
                    })
            })
        )
        .0?;
    }
    println!("vote ended!");
    handle
        .edit(ctx, |m| {
            m.embed(|e| {
                for (option, votes) in ctx
                    .data()
                    .vote_data
                    .lock()
                    .unwrap()
                    .remove(n)
                    .finish(options.len())
                    .get_summarized()
                    .into_iter()
                    .enumerate()
                {
                    e.field(&options[option], votes, true);
                }
                e.color(DISABLED)
                    .title(&title)
                    .imageor(image.as_ref())
                    .description(format!("vote ended!"))
            })
            .components(|c| {
                c.set_action_row({
                    let mut r = CreateActionRow::default();
                    for (n, option) in options.iter().enumerate() {
                        r.create_button(|b| {
                            b.custom_id(format!("{}{n}", ctx_id))
                                .label(option)
                                .disabled(true)
                                .style(styles[n])
                        });
                    }
                    r
                })
            })
        })
        .await?;
    Ok(())
}
