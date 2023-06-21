use super::{Context, DISABLED, SUCCESS};
use ::serenity::builder::CreateActionRow;
use ::serenity::builder::CreateEmbed;
use anyhow::anyhow;
use anyhow::Result;
use itertools::Itertools;
use poise::serenity_prelude::*;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs::{read_dir, File};
use std::io::BufReader;
use std::path::Path;
use std::str::FromStr;
use std::sync::Mutex;
use std::time::SystemTime;
use tokio::time::*;

pub type Vote = usize;

#[derive(Default, Clone, Serialize, Deserialize, Debug)]
struct VoteOptions {
    options: Vec<String>,
    styles: Vec<ButtonStyle>,
    title: String,
    fields: HashMap<String, String>,
    image: Option<String>,
}

#[derive(Serialize, Deserialize, Debug, Clone, Default)]
pub struct BeforePushVoteData {
    votes: HashMap<UserId, Vote>,
    // you can parse this from the message but its easier to do this
    deadline: Duration,
    #[serde(skip)]
    deadline_changed: Duration,
    options: VoteOptions,
    #[serde(skip)]
    reply: Option<Box<Message>>,
    id: u64,
    mid: MessageId,
    cid: ChannelId,
    endat: u64,
}

#[derive(Clone, Debug)]
pub struct AfterPushVoteData {
    index: usize,
    deadline_changed: Duration,
    options: VoteOptions,
    id: u64,
}

macro_rules! read {
    ($self:expr,$ctx:expr,$v:expr) => {{
        $v = $ctx.data().vote_data.lock().unwrap();
        match $v.get_mut($self.index).unwrap() {
            VoteData::Before(x) => x,
            VoteData::After(_) => unreachable!(),
        }
    }};
}

#[derive(Debug)]
pub enum VoteData {
    Before(BeforePushVoteData),
    After(AfterPushVoteData),
}

pub type Votes = Mutex<Vec<VoteData>>;

trait EmbedUtil {
    fn imageor<S: ToString>(&mut self, img: Option<S>) -> &mut Self;
    fn set_fields<M: IntoIterator<Item = (S, S)>, S: ToString>(&mut self, fields: M) -> &mut Self;
}

impl EmbedUtil for CreateEmbed {
    fn imageor<S: ToString>(&mut self, img: Option<S>) -> &mut Self {
        if let Some(iuri) = img {
            self.image(iuri)
        } else {
            self
        }
    }

    fn set_fields<M: IntoIterator<Item = (S, S)>, S: ToString>(&mut self, fields: M) -> &mut Self {
        for (k, v) in fields {
            self.field(k, v, false);
        }
        self
    }
}

macro_rules! votes {
    ($self:expr, $ctx:expr, $v:expr) => {
        match &mut $self {
            VoteData::After(a) => &mut read!(a, $ctx, $v).votes,
            VoteData::Before(b) => &mut b.votes,
        }
    };
}

impl VoteData {
    pub fn summarize(&mut self, ctx: &Context<'_>, optcount: usize) -> Vec<usize> {
        let mut ret = vec![];
        ret.resize(optcount, 0);
        let mut v;
        for v in votes!(*self, ctx, v).values() {
            ret[*v] += 1;
        }
        ret
    }

    fn deadline(&self) -> Duration {
        match self {
            Self::After(a) => a.deadline_changed,
            Self::Before(b) => b.deadline_changed,
        }
    }

    fn id(&self) -> u64 {
        match self {
            Self::After(a) => a.id,
            Self::Before(b) => b.id,
        }
    }

    fn options(&self) -> &VoteOptions {
        match self {
            Self::After(a) => &a.options,
            Self::Before(b) => &b.options,
        }
    }

    fn set_reply(&mut self, ctx: &Context<'_>, reply: Message) {
        let mut v;
        let mid = reply.id;
        let cid = reply.channel_id;
        let reply = Some(Box::new(reply));
        match self {
            Self::After(a) => {
                let read = read!(a, ctx, v);
                read.reply = reply;
                read.mid = mid;
                read.cid = cid;
            }
            Self::Before(b) => {
                b.reply = reply;
                b.mid = mid;
                b.cid = cid;
            }
        }
    }

    fn get_reply(&mut self, ctx: &Context<'_>) -> Box<Message> {
        let mut v;
        match self {
            Self::After(a) => read!(a, ctx, v).reply.take().unwrap(),
            Self::Before(b) => b.reply.take().unwrap(),
        }
    }

    fn set_end(&mut self) {
        let end = self.dead_secs();
        match self {
            Self::Before(x) => x.endat = end,
            _ => unreachable!(),
        }
    }

    fn dead_secs(&self) -> u64 {
        (SystemTime::now() + self.deadline())
            .duration_since(SystemTime::UNIX_EPOCH)
            .unwrap()
            .as_secs()
    }

    fn end_stamp(&self) -> String {
        format!("<t:{}:R>", self.dead_secs())
    }

    pub async fn begin(mut self, ctx: Context<'_>) -> Result<Self> {
        self.set_end();
        let o = self.options();
        let handle = poise::send_reply(ctx, |m| {
            m.embed(|e| {
                e.imageor(o.image.as_ref())
                    .color(SUCCESS)
                    .title(&o.title)
                    .description(format!("vote ends {}", self.end_stamp()))
                    .set_fields(&o.fields)
            })
            .components(|c| {
                c.create_action_row(|r| {
                    for (n, option) in o.options.iter().enumerate() {
                        r.create_button(|b| {
                            b.custom_id(format!("{}{n}", self.id()))
                                .label(option)
                                .style(o.styles[n])
                        });
                    }
                    r
                })
            })
        })
        .await?;
        let msg = handle.into_message().await?;
        self.set_reply(&ctx, msg);
        self.push(&ctx).save(&ctx)
    }

    fn save_ref(&self, ctx: &Context<'_>) -> Result<()> {
        let t = self.options().title.clone() + ".vd";
        let mut re;
        let thing = match &self {
            Self::Before(x) => x,
            Self::After(y) => {
                re = ctx.data().vote_data.lock().unwrap();
                match re.get_mut(y.index).unwrap() {
                    VoteData::Before(x) => x,
                    VoteData::After(_) => unreachable!(),
                }
            }
        };
        std::fs::write(t, serde_json::to_string(thing)?)?;
        Ok(())
    }

    pub fn save(self, ctx: &Context<'_>) -> Result<Self> {
        self.save_ref(ctx)?;
        Ok(self)
    }

    pub fn push(self, ctx: &Context<'_>) -> Self {
        let mut data = ctx.data().vote_data.lock().unwrap();
        let n = data.len();
        let v = Self::After(AfterPushVoteData {
            index: n,
            id: self.id(),
            deadline_changed: self.deadline(),
            options: self.options().clone(),
        });
        data.push(self);
        v
    }

    fn remove(self, ctx: &Context<'_>) -> Self {
        match self {
            Self::After(x) => ctx.data().vote_data.lock().unwrap().remove(x.index),
            Self::Before(_) => unreachable!(),
        }
    }

    pub async fn input(mut self, ctx: &Context<'_>) -> Result<Self> {
        let dead = self.deadline();
        let ctx_id = self.id();
        let ctx_id_len = ctx_id.to_string().len();
        let o = self.options().clone();
        let timestamp = self.end_stamp();
        while let Some(press) = CollectComponentInteraction::new(ctx)
            .filter(move |press| press.data.custom_id.starts_with(&ctx_id.to_string()))
            .timeout(dead)
            .await
        {
            let s = {
                let mut v;
                if votes!(self, ctx, v)
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
            self.save_ref(ctx)?;
            let (_m, _) = tokio::join!(
                press.create_followup_message(ctx, |m| { m.ephemeral(true).content(s) }),
                press.create_interaction_response(ctx, |c| {
                    c.kind(InteractionResponseType::UpdateMessage)
                        .interaction_response_data(|m| {
                            m.embed(|e| {
                                for (option, votes) in
                                    self.summarize(ctx, o.options.len()).iter().enumerate()
                                {
                                    e.field(&o.options[option], votes, true);
                                }
                                e.imageor(o.image.as_ref())
                                    .color(SUCCESS)
                                    .title(&o.title)
                                    .set_fields(&o.fields)
                                    .description(format!("vote ends {timestamp}"))
                            })
                        })
                })
            );
            // let m = m?;
            // let http = ctx.serenity_context().http.clone();
            // tokio::spawn(async move {
            //     sleep(Duration::from_secs(10)).await;
            //     m.delete(http).await.unwrap();
            // });
        }
        Ok(self)
    }

    pub async fn finish(mut self, ctx: &Context<'_>) -> Result<()> {
        let o = self.options().clone();
        let filename = format!("{}.vd", o.title);
        let p = std::path::Path::new(&filename);
        if p.exists() {
            let _ = std::fs::remove_file(p);
        }
        self.get_reply(ctx)
            .edit(ctx, |m| {
                m.embed(|e| {
                    for (option, votes) in self
                        .remove(ctx)
                        .summarize(ctx, o.options.len())
                        .iter()
                        .enumerate()
                    {
                        e.field(&o.options[option], votes, true);
                    }
                    e.color(DISABLED)
                        .title(&o.title)
                        .imageor(o.image.as_ref())
                        .set_fields(o.fields)
                        .description(format!("vote ended!"))
                })
                .components(|c| {
                    c.set_action_row({
                        let mut r = CreateActionRow::default();
                        for (n, option) in o.options.iter().enumerate() {
                            r.create_button(|b| {
                                b.label(option)
                                    .disabled(true)
                                    .style(o.styles[n])
                                    .custom_id("_")
                            });
                        }
                        r
                    })
                })
            })
            .await?;
        Ok(())
    }
}

trait Parsing {
    fn parse_pscsp(self) -> HashMap<String, String>;
    fn parse_psv(self) -> Vec<String>;
    fn parse_psv_to_styles(self) -> Vec<ButtonStyle>;
}

impl<S> Parsing for S
where
    S: AsRef<str>,
{
    fn parse_pscsp(self) -> HashMap<String, String> {
        let pair = self.as_ref().split('|');
        let pairs = pair.map(|s| {
            s.split(':')
                .map(|s| s.trim().to_owned())
                .collect_tuple()
                .unwrap()
        });
        let mut map = HashMap::new();
        for (k, v) in pairs {
            map.insert(k, v);
        }
        map
    }
    fn parse_psv(self) -> Vec<String> {
        self.as_ref()
            .split('|')
            .map(|s| s.trim().to_owned())
            .collect()
    }
    fn parse_psv_to_styles(self) -> Vec<ButtonStyle> {
        self.as_ref()
            .split('|')
            .map(|s| {
                use ButtonStyle::*;
                match s.trim().to_lowercase().as_str() {
                    // "blue" => Primary,
                    "gray" => Secondary,
                    "green" => Success,
                    "red" => Danger,
                    _ => Primary,
                }
            })
            .collect()
    }
}

#[poise::command(slash_command, category = "Discord", rename = "create_vote")]
/// make a vote
pub async fn create(
    ctx: Context<'_>,
    #[description = "picture url"] image: Option<String>,
    #[description = "pressables (psv)"] options: String,
    #[description = "option styles (psv) {blue|gray|green|red}"] styles: Option<String>,
    #[description = "how long the vote will be up"] length: String,
    #[description = "fields (pipe separated colon seperated pairs) (a:b|c:d)"] fields: Option<
        String,
    >,
    title: String,
) -> Result<()> {
    let options = options.parse_psv();
    let styles = styles.map_or(
        vec![ButtonStyle::Primary; options.len()],
        Parsing::parse_psv_to_styles,
    );
    let fields = fields.as_ref().map_or(HashMap::new(), Parsing::parse_pscsp);
    let Ok(dur) = parse_duration::parse(&length) else {
        ctx.say(format!("`{length}` is not time")).await?;
        return Ok(());
    };

    VoteData::Before({
        BeforePushVoteData {
            votes: HashMap::new(),
            deadline: dur,
            deadline_changed: dur,
            options: VoteOptions {
                options,
                styles,
                title,
                fields,
                image,
            },
            reply: None,
            id: ctx.id(),
            ..Default::default()
        }
    })
    .begin(ctx)
    .await?
    .input(&ctx)
    .await?
    .finish(&ctx)
    .await
}

async fn fix(ctx: &Context<'_>, data: BufReader<std::fs::File>) -> Result<()> {
    let mut v: BeforePushVoteData = serde_json::from_reader(data)?;
    let m = ctx.http().get_message(v.cid.0, v.mid.0).await?;
    let end = dbg!(m.timestamp.unix_timestamp()) as u64;
    v.reply = Some(Box::new(m));
    let now = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .unwrap()
        .as_secs();
    let end = end + dbg!(v.deadline.as_secs());
    println!("@{now} | :{end}");
    // cant use abs() because unsigned
    v.deadline_changed = if now < end {
        Duration::from_secs(end - now)
    } else {
        Duration::from_secs(now - end)
    };
    let v = VoteData::Before(v);
    if end < now {
        v.push(&ctx).finish(&ctx).await
    } else {
        v.push(&ctx).input(&ctx).await?.finish(&ctx).await
    }
}

#[poise::command(
    slash_command,
    category = "Discord",
    required_permissions = "ADMINISTRATOR"
)]
pub async fn fixall(ctx: Context<'_>) -> Result<()> {
    use futures::future;
    let mut futs = vec![];
    for e in read_dir(".")? {
        let e = e?;
        let fname = e.file_name();
        let p = Path::new(&fname);
        if let Some(ext) = p.extension() {
            if ext == "vd" {
                futs.push(fix(&ctx, BufReader::new(File::open(p).unwrap())));
            }
        }
    }
    let msg = format!("fixed {}", futs.len());
    poise::send_reply(ctx, |m| m.content(msg).ephemeral(true)).await?;
    future::join_all(futs).await;
    Ok(())
}

// #[poise::command(
//     context_menu_command = "fix vote",
//     slash_command,
//     category = "Discord",
//     rename = "fix_vote",
//     required_permissions = "ADMINISTRATOR"
// )]
// /// restart vote (use ctx menu)
// pub async fn reinstate(
//     ctx: Context<'_>,
//     #[description = "previous vote, id or link"] m: Message,
// ) -> Result<()> {
//     static RE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r#"<t:([0-9]+):R>"#).unwrap());
//     let e = m.embeds.get(0).ok_or(anyhow!("no embed?"))?;
//     let end = u64::from_str(
//         RE.captures(
//             m.embeds
//                 .get(0)
//                 .ok_or(anyhow!("no embed?"))?
//                 .description
//                 .as_ref()
//                 .ok_or(anyhow!("no desc?"))?,
//         )
//         .ok_or(anyhow!("no timestamp?"))?
//         .get(1)
//         .unwrap()
//         .as_str(),
//     )
//     .unwrap();
//     let now = SystemTime::now()
//         .duration_since(SystemTime::UNIX_EPOCH)
//         .unwrap()
//         .as_secs();
//     let f = BufReader::new(std::fs::File::open(
//         e.title.to_owned().ok_or(anyhow!("no title?"))? + ".vd",
//     )?);
//     poise::send_reply(ctx, |m| m.content("yes").ephemeral(true)).await?;
//     let mut v: BeforePushVoteData = serde_json::from_reader(f)?;
//     v.reply = Some(Box::new(m));
//     // cant use abs() because unsigned
//     v.deadline = if now < end {
//         Duration::from_secs(end - now)
//     } else {
//         Duration::from_secs(now - end)
//     };
//     let v = VoteData::Before(v);
//     if end < now {
//         v.push(&ctx).finish(&ctx).await
//     } else {
//         v.push(&ctx).input(&ctx).await?.finish(&ctx).await
//     }
// }

// voters

#[poise::command(prefix_command, slash_command, category = "Discord", rename = "votes")]
pub async fn list(ctx: Context<'_>, #[description = "the vote title"] vote: String) -> Result<()> {
    let vd = {
        let buf = ctx.data().vote_data.lock().unwrap();
        match &buf[buf
            .iter()
            .position(|x| x.options().title == vote)
            .ok_or(anyhow!("vote doesnt exist"))?]
        {
            VoteData::Before(x) => x.clone(),
            VoteData::After(_) => unreachable!(),
        }
    };
    poise::send_reply(ctx, |m| {
        m.allowed_mentions(|x| x.empty_parse()).embed(|e| {
            let mut votes: HashMap<usize, Vec<u64>> = HashMap::new();
            for (user, vote) in vd.votes {
                votes.entry(vote).or_default().push(user.0);
            }
            for (vote, voters) in votes {
                let mut s = vec![];
                s.reserve(voters.len());
                for person in voters {
                    s.push(format!("<@{person}>"));
                }
                e.field(&vd.options.options[vote], s.join("\n"), false);
            }
            e.color(SUCCESS)
                .title(format!("voter list for {vote}"))
                .footer(|f| f.text("privacy is a illusion"))
        })
    })
    .await?;
    Ok(())
}
