use poise::serenity_prelude::Webhook as RealHook;
use regex::Regex;
use serenity::{builder::ExecuteWebhook, http::Http, json};
use std::convert::AsRef;
use std::sync::{
    atomic::{AtomicU8, Ordering},
    Arc, LazyLock,
};
use tokio::sync::broadcast::{self, error::TryRecvError};
use tokio::time::{sleep, Duration};

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
        execute_webhook.allowed_mentions(|m| m.empty_parse().roles(vec![1169521140998357002, 1133416252791074877]).users(vec![696196765564534825, 600014432298598400]));
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
        define_print!("webhook");
        output!("{username}: {content}");
        self.send(|m| m.username(username).content(content)).await;
    }

    pub async fn link(&mut self, mut stdout: broadcast::Receiver<String>) {
        define_print!("webhook");
        loop {
            let out = stdout.try_recv();
            match out {
                Err(e) => match e {
                    TryRecvError::Closed => fail!("closed"),
                    _ => sleep(Duration::from_millis(100)).await,
                },
                Ok(m) => {
                    if self
                        .skip
                        .fetch_update(Ordering::Relaxed, Ordering::Relaxed, |n| n.checked_sub(1))
                        .is_ok()
                    {
                        input!("{m} < skipped");
                        if let Err(e) = self.skipped.send(m) {
                            eprintln!("err skipping: {e}");
                        }
                        continue;
                    }
                    for line in m.lines() {
                        self.push(line).await;
                    }
                }
            }
            sleep(Duration::from_millis(100)).await;
        }
    }

    pub async fn push(&self, msg: &str) {
        match get(msg) {
            Some(Message::Chat { player, content }) => {
                self.send_message(&player, &content).await;
            }
            Some(Message::Join { player }) => {
                self.send_message(&player, "<has joined the game>").await;
            }
            Some(Message::Left { player }) => {
                self.send_message(&player, "<has left the game>").await;
            }
            Some(Message::Load { map }) => {
                self.send_message("server", &format!("loading map {map}"))
                    .await;
            }
            _ => (),
        }
    }
}

#[derive(PartialEq, Debug, Clone)]
pub enum Message {
    Join { player: String },
    Left { player: String },
    Chat { player: String, content: String },
    Load { map: String },
}

fn get(line: &str) -> Option<Message> {
    macro_rules! s {
        ($line: expr, $($e:expr),+ $(,)?) => {
            $(
                $line.starts_with($e) ||
            )+ false
        };
    }
    if s!(line, [' ', '\t'], "at", "Lost command socket connection", "Kicking connection") {
        return None;
    }

    static HAS_UUID: LazyLock<Regex> =
        LazyLock::new(|| Regex::new(r"[a-zA-Z0-9+/]{22}==").unwrap());

    if let Some((u, c)) = line.split_once(": ") {
        let u = u.trim_start_matches('<');
        let c = c.trim_end_matches('>');
        if !(u.is_empty() || c.is_empty() || HAS_UUID.is_match(c) || HAS_UUID.is_match(u)) {
            return Some(Message::Chat {
                player: unify(u),
                content: unify(&crate::conv::replace(c)),
            });
        }
    }

    static JOINAGE: LazyLock<Regex> = LazyLock::new(|| {
        Regex::new(r"(.+) has (dis)?connected. \[([a-zA-Z0-9+/]{22}==)\]").unwrap()
    });
    if let Some(captures) = JOINAGE.captures(line) {
        let player = unify(captures.get(1).unwrap().as_str());
        return Some(if captures.get(2).is_some() {
            Message::Left { player }
        } else {
            Message::Join { player }
        });
    }

    static MAP_LOAD: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"Loading map (.+)").unwrap());
    if let Some(captures) = MAP_LOAD.captures(line) {
        return Some(Message::Load {
            map: captures.get(1).unwrap().as_str().to_string(),
        });
    }
    None
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
        ($line:literal) => {
            let got = get($line);
            assert_eq!(got, None);
        };
        ($line:literal, $what:expr) => {
            let got = get($line);
            assert_eq!(got, Some($what));
        };
    }
    //unnamed
    test_line!("undefined");
    test_line!("Lost command socket connection: localhost/127.0.0.1:6859");
    //named
    test_line!(
        "abc: hi",
        Message::Chat {
            player: "abc".into(),
            content: "hi".into()
        }
    );
    test_line!(
        "<a: /help>",
        Message::Chat {
            player: "a".into(),
            content: "/help".into()
        }
    );
    test_line!(
        "a has connected. [+41521zhHB8321xAbXYedw==]",
        Message::Join { player: "a".into() }
    );
    test_line!(
        "a has disconnected. [+41521zhHB8321xAbXYedw==] (closed)",
        Message::Left { player: "a".into() }
    );
    test_line!(
        "a: :o",
        Message::Chat {
            player: "a".into(),
            content: ":o".into()
        }
    );
    test_line!(
        "a:b: :o",
        Message::Chat {
            player: "a:b".into(),
            content: ":o".into()
        }
    );
}

#[test]
fn test_unify() {
    assert!(unify("grassྱྊၔ") == "grass");
    assert!(unify("иди к черту") == "иди к черту")
}
