use itertools::Itertools;
use poise::serenity_prelude::Webhook as RealHook;
use regex::Regex;
use serenity::{builder::ExecuteWebhook, http::Http, json};
use std::convert::AsRef;
use std::sync::{
    atomic::{AtomicU8, Ordering},
    Arc, LazyLock,
};
use tokio::sync::broadcast::{self, error::TryRecvError};
use tokio::time::{sleep, Duration, Instant};

pub struct Webhook<'a> {
    pub skipped: broadcast::Sender<String>,
    pub skip: Arc<AtomicU8>,
    inner: RealHook,
    http: &'a Http,
}

impl<'a> Webhook<'a> {
    pub async fn new(http: &'a impl AsRef<Http>, url: &str) -> Webhook<'a> {
        Self {
            inner: RealHook::from_url(http, url).await.unwrap(),
            http: http.as_ref(),
            skip: Arc::new(AtomicU8::new(0)),
            skipped: broadcast::channel(16).0,
        }
    }

    async fn send<F>(&self, block: F)
    where
        for<'b> F: FnOnce(&'b mut ExecuteWebhook<'a>) -> &'b mut ExecuteWebhook<'a>,
    {
        let mut execute_webhook = ExecuteWebhook::default();
        execute_webhook.allowed_mentions(|m| m.empty_parse());
        block(&mut execute_webhook);

        let map = json::hashmap_to_json_map(execute_webhook.0);
        if let Err(e) = self
            .http
            .as_ref()
            .execute_webhook(
                self.inner.id.0,
                self.inner.token.as_ref().unwrap(),
                false,
                &map,
            )
            .await
        {
            println!("sending {map:#?} got error {e}.");
        };
    }

    async fn send_message(&self, username: &str, content: &str) {
        self.send(|m| m.username(username).content(content)).await;
    }

    pub async fn link(&mut self, mut stdout: broadcast::Receiver<String>) {
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
                            if since > 1 || feed.len() > 15 {
                                last.take();
                                self.flush::<MindustryStyle>(feed).await;
                                feed = vec![];
                                flush!();
                            }
                        }
                        sleep(Duration::from_millis(20)).await;
                        continue;
                    }
                },
                Ok(m) => {
                    if self
                        .skip
                        .fetch_update(Ordering::Relaxed, Ordering::Relaxed, |n| n.checked_sub(1))
                        .is_ok()
                    {
                        input!("{m} < skipped");
                        match self.skipped.send(m) {
                            Err(e) => eprintln!("err skipping: {e}"),
                            Ok(_) => {}
                        };
                        continue;
                    }
                    for line in m.lines() {
                        let line = line.to_string();
                        input!("{line}");
                        feed.push(line);
                    }
                    last = Some(now);
                }
            }
            sleep(Duration::from_millis(20)).await;
        }
    }

    pub async fn flush<Style: OutputStyle>(&self, feed: Vec<String>) {
        let mut current: Option<String> = None;
        let mut message: Option<String> = None;
        let mut unnamed: Option<String> = None;
        for line in feed {
            let line: String = Style::fix(line);
            if let Some((name, msg)) = Style::split(&line) {
                if let Some(n) = current.as_ref() {
                    if n == &name {
                        message.madd_panic(&msg);
                        continue;
                    }
                    let message = message.take().unwrap();
                    self.send_message(n, &message).await;
                    current.take();
                }
                current = Some(name.to_owned());
                message = Some(msg.to_owned());
                // interrupt
                if let Some(msg) = unnamed.take() {
                    self.send_message("server", &msg).await;
                }
                continue;
            }
            unnamed.madd(unify(&line));
        }
        // finish
        if let Some(n) = current.as_ref() {
            let message = message.take().unwrap();
            self.send_message(n, &message).await;
        }
        if let Some(msg) = unnamed.as_ref() {
            self.send_message("server", msg).await;
        }
    }
}

/// functions ordered by call order
pub trait OutputStyle {
    /// first step
    fn fix(raw_line: String) -> String {
        raw_line
    }
    /// get the user and the content (none for no user)
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

macro_rules! tern {
    ($predicate:expr, $true: expr, $false: expr) => {{
        if $predicate {
            $true
        } else {
            $false
        }
    }};
}
pub struct MindustryStyle;
impl OutputStyle for MindustryStyle {
    fn split(line: &str) -> Option<(String, String)> {
        if s!(line, [' ', '\t']) || s!(line, "at") || s!(line, "Lost command socket connection") {
            return None;
        }

        if let Some((u, c)) = line.split(": ").map(unify).collect_tuple() {
            let u = u.trim_start_matches('<');
            let c = c.trim_end_matches('>');
            if !(u.is_empty() || c.is_empty()) {
                return Some((u.to_owned(), c.to_owned()));
            }
        }

        static REGEX: LazyLock<Regex> = LazyLock::new(|| {
            Regex::new(r"(.+) has (dis)?connected. \[([a-zA-Z0-9+/]+==)\]").unwrap()
        });
        if let Some(captures) = REGEX.captures(line) {
            let player = unify(captures.get(1).unwrap().as_str());
            let prefix = tern!(captures.get(2).is_some(), "left", "joined");
            let uuid = captures.get(3).unwrap().as_str();
            return Some((player, format!("{prefix} ({uuid})")));
        }
        None
    }
}

pub fn unify(s: &str) -> String {
    s.chars().filter(|&c| c < 'џ').collect()
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

#[test]
fn style() {
    macro_rules! test_line {
        ($line:expr) => {
            let line = $line.to_string(); // no fixing done!
            let got = MindustryStyle::split(&line);
            assert!(got == None, "got {got:?}, expected None");
        };
        ($line:expr, $name: expr, $content: expr) => {
            let line = $line.to_string();
            let got = MindustryStyle::split(&line);
            assert!(
                got == Some(($name.into(), $content.into())),
                "got {got:?}, expected ({}, {})",
                $name,
                $content
            );
        };
    }
    //unnamed
    test_line!("undefined");
    test_line!("Lost command socket connection: localhost/127.0.0.1:6859");
    //named
    test_line!("abc: hi", "abc", "hi");
    test_line!("<a: /help>", "a", "/help");
    test_line!("a has connected. [abc==]", "a", "joined (abc==)");
    test_line!("a has disconnected. [abc==] (closed)", "a", "left (abc==)");
    test_line!("a: :o", "a", ":o");
    test_line!("a:b: :o", "a:b", ":o");
}

#[test]
fn test_unify() {
    assert!(unify("grassྱྊၔ") == "grass");
    assert!(unify("иди к черту") == "иди к черту")
}
