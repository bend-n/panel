use poise::serenity_prelude::Webhook as RealHook;
use serenity::{builder::ExecuteWebhook, http::Http, json};
use std::convert::AsRef;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use tokio::sync::broadcast::{self, error::TryRecvError};

pub struct Webhook<'a> {
    pub skipped: broadcast::Sender<String>,
    pub skip: Arc<Mutex<u8>>,
    inner: RealHook,
    http: &'a Http,
}

impl<'a> Webhook<'a> {
    pub async fn new(http: &'a impl AsRef<Http>, url: &str) -> Webhook<'a> {
        Self {
            inner: RealHook::from_url(http, url).await.unwrap(),
            http: http.as_ref(),
            skip: Arc::new(Mutex::new(0)),
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
                        async_std::task::sleep(Duration::from_millis(20)).await;
                        continue;
                    }
                },
                Ok(m) => {
                    let mut lock = self.skip.lock().unwrap();
                    if *lock > 0 {
                        *lock -= 1;
                        input!("{m} < skipped");
                        self.skipped.send(m).unwrap();
                        continue;
                    }
                    drop(lock);
                    for line in m.lines() {
                        let line = line.to_string();
                        input!("{line}");
                        feed.push(line);
                    }
                    last = Some(now);
                }
            }
            async_std::task::sleep(Duration::from_millis(20)).await;
        }
    }

    pub async fn flush<Style: OutputStyle>(&self, feed: Vec<String>) {
        let mut current: Option<String> = None;
        let mut message: Option<String> = None;
        let mut unnamed: Option<String> = None;

        // this code is very game dependent
        for line in feed.into_iter() {
            let line: String = Style::fix(line);
            if let Some((name, msg)) = Style::split(&line) {
                if let Some(n) = current.as_ref() {
                    if n == &name {
                        message.madd_panic(&msg);
                        continue;
                    } else {
                        let message = message.take().unwrap();
                        self.send_message(n, &message).await;
                        current.take();
                    }
                }
                current = Some(name.to_owned());
                message = Some(msg.to_owned());
                // interrupt
                if let Some(msg) = unnamed.take() {
                    self.send_message("server", &msg).await;
                }
                continue;
            }
            unnamed.madd(line);
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
pub struct MindustryStyle;
impl OutputStyle for MindustryStyle {
    fn split(line: &str) -> Option<(String, String)> {
        if s!(line, [' ', '\t']) || s!(line, "at") {
            return None;
        }
        if line.chars().filter(|x| x == &':').count() == 1 {
            if let Some((u, c)) = line.split_once(':') {
                let u = unify(u).trim_start_matches('<').trim().to_owned();
                let c = unify(c).trim_end_matches('>').trim().to_owned();
                if !(u.is_empty() || c.is_empty()) {
                    return Some((u, c));
                }
            }
        }
        if let Some(index) = line.find("has") {
            if line.contains("connected") {
                let player = &line[..index];
                let prefix = if line.contains("disconnected") {
                    "left"
                } else {
                    "joined"
                };
                return Some((unify(player).trim().to_owned(), prefix.to_owned()));
            }
        }
        None
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
    test_line!("a has connected. [abc==]", "a", "joined");
    test_line!("a has disconnected. [abc==] (closed)", "a", "left");
}
