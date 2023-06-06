use std::{
    sync::Arc,
    time::{Duration, Instant},
};

use tokio::{
    sync::broadcast::{self, error::TryRecvError},
    task::JoinHandle,
};
use webhook::{client::WebhookClient, models::Message};

macro_rules! send {
    ($client:ident, $block:expr) => {{
        let client = $client.clone();
        let mut m = Message::new();
        let mut bloc: Box<dyn FnMut(&mut Message) -> &mut Message> = Box::new($block);
        bloc(&mut m);
        m.allow_mentions(None, None, None, false);
        tokio::spawn(async move {
            client.send_message(&m).await.unwrap();
        });
        // handle goes out of scope, detatched
    }};
}

pub struct Webhook(JoinHandle<()>);
impl Webhook {
    pub async fn new(mut stdout: broadcast::Receiver<String>, url: &str) -> Self {
        let client = Arc::new(WebhookClient::new(url));
        Self(tokio::spawn(async move {
            define_print!("webhook");
            let mut last: Option<Instant> = None;
            let mut feed: Vec<String> = vec![];
            loop {
                let out = stdout.try_recv();
                let now = Instant::now();
                match out {
                    Err(e) => match e {
                        TryRecvError::Closed => fail!("closed"),
                        TryRecvError::Lagged(_) => continue,
                        TryRecvError::Empty => {
                            if let Some(earlier) = last {
                                let since = now.duration_since(earlier).as_secs();
                                if since > 2 || feed.len() > 15 {
                                    last.take();
                                    flush::<MindustryStyle>(feed, &client).await;
                                    feed = vec![];
                                    flush!();
                                }
                            }
                            async_std::task::sleep(Duration::from_millis(20)).await;
                            continue;
                        }
                    },
                    Ok(m) => {
                        for line in m.lines() {
                            feed.push(line.to_string());
                            input!("{line}");
                        }
                        last = Some(now);
                    }
                }
                async_std::task::sleep(Duration::from_millis(20)).await;
            }
        }))
    }

    pub fn running(&self) -> bool {
        !self.0.is_finished()
    }
}

/// functions ordered by call order
pub trait OutputStyle {
    /// first step
    fn fix(raw_line: String) -> String;
    /// ignore completely?
    fn ignore(line: &str) -> bool;
    /// is there no user
    fn unnamed(line: &str) -> bool;
    /// get the user and the content
    fn split(line: &str) -> Option<(String, String)>;
}

macro_rules! s {
    ($line:expr, $e:ident) => {
        $line.starts_with(stringify!($e))
    };
    ($line:expr, $e:expr) => {
        $line.starts_with($e)
    };
}
pub struct MindustryStyle;
impl OutputStyle for MindustryStyle {
    fn fix(raw_line: String) -> String {
        raw_line.split(' ').skip(3).collect::<Vec<&str>>().join(" ")
    }
    fn ignore(line: &str) -> bool {
        line.trim().is_empty()
            || line.contains("requested trace info")
            || s!(line, Server)
            || s!(line, Connected)
            || s!(line, PLAGUE)
            || s!(line, "1 mods loaded")
    }

    fn unnamed(line: &str) -> bool {
        s!(line, [' ', '\t'])
    }

    fn split(line: &str) -> Option<(String, String)> {
        if let Some(n) = line.find(':') {
            if n < 12 {
                if let Some((u, c)) = line.split_once(':') {
                    return Some((
                        unify(u).trim_start_matches('<').to_owned(),
                        unify(c).trim_end_matches('>').to_owned(),
                    ));
                }
            }
        }
        if let Some(index) = line.find("has") {
            if line.contains("connected") {
                let player = &line[..index];
                return Some((unify(player).trim().to_owned(), "joined".to_owned()));
            }
        }
        None
    }
}

async fn flush<Style: OutputStyle>(feed: Vec<String>, client: &Arc<WebhookClient>) {
    let mut current: Option<String> = None;
    let mut message: Option<String> = None;
    let mut unnamed: Option<String> = None;

    // this code is very game dependent
    for line in feed.into_iter() {
        // [0-0-0-0 0-0-0-0] [i] ... -> ...
        let line: String = Style::fix(line);
        if Style::ignore(&line) {
            continue;
        }
        if Style::unnamed(&line) {
            unnamed.madd(line);
            continue;
        }

        if let Some((name, msg)) = Style::split(&line) {
            if !(msg.is_empty() || name.is_empty()) {
                if let Some(n) = current.as_ref() {
                    if n == &name {
                        message.madd_panic(&msg);
                        continue;
                    } else {
                        let message = message.take().unwrap();
                        send!(client, |m| m.content(&message).username(n));
                        current.take();
                    }
                }
                current = Some(name.to_owned());
                message = Some(msg.to_owned());
                // interrupt
                if let Some(msg) = unnamed.take() {
                    send!(client, |m| m.username("server").content(&msg));
                }
                continue;
            }
        }
        unnamed.madd(line);
    }
    // leftovers
    if let Some(n) = current.as_ref() {
        let message = message.take().unwrap();
        send!(client, |m| m.content(&message).username(n));
    }
    if let Some(msg) = unnamed.as_ref() {
        send!(client, |m| m.username("server").content(msg));
    }
}
/// latin > extended a > kill
fn unify(s: &str) -> String {
    s.chars()
        .map(|c| if (c as u32) < 384 { c } else { ' ' })
        .collect()
}

trait Madd {
    fn madd(&mut self, line: String);
    fn madd_panic(&mut self, line: &str);
}

// cant impl addassign because no impl for other types because
impl Madd for Option<String> {
    fn madd(&mut self, line: String) {
        match self.take() {
            Some(x) => *self = Some(x + "\n" + &line),
            None => *self = Some(line),
        }
    }
    fn madd_panic(&mut self, line: &str) {
        match self.take() {
            Some(x) => *self = Some(x + "\n" + line),
            None => unreachable!(),
        }
    }
}
