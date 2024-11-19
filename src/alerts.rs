use crate::emoji::named::*;
use regex::Regex;
use std::{
    sync::{
        atomic::{
            AtomicU64,
            Ordering::{Acquire, Relaxed},
        },
        LazyLock,
    },
    time::Duration,
};
use tokio::{
    io::{AsyncBufReadExt, AsyncReadExt},
    sync::broadcast::error::TryRecvError,
};
pub async fn run() {
    let (tx, mut rx) = tokio::sync::broadcast::channel::<u64>(10);
    static COUNT: AtomicU64 = AtomicU64::new(0);
    let mut f =
        tokio::io::BufReader::new(tokio::fs::File::open("/var/log/kern.log").await.unwrap());
    let mut buf = [0; 1 << 20];
    while f.read(&mut buf).await.unwrap() != 0 {}
    static RE: LazyLock<Regex> = LazyLock::new(|| {
        Regex::new("[^ ]+ [0-9]+ [0-9]+:[0-9]+:[0-9]+ .+ kernel: [[0-9]+.[0-9]+] IN=.+ OUT= MAC=.+ SRC=([0-9]+.[0-9]+.[0-9]+.[0-9]+) DST=[0-9]+.[0-9]+.[0-9]+.[0-9]+ LEN=[0-9]+ TOS=0x[0-9a-f]+ PREC=0x[0-9a-f]+ TTL=[0-9]+ ID=[0-9]+(?: DF)? PROTO=TCP SPT=[0-9]+ DPT=[0-9]+ WINDOW=[0-9]+ RES=0x[a-f0-9]+ SYN URGP=[0-9]").unwrap()
    });
    tokio::spawn(async move {
        loop {
            let mut s = String::new();
            f.read_line(&mut s).await.unwrap();
            if RE.is_match(&s) {
                COUNT.fetch_add(1, Relaxed);
            }
            tokio::time::sleep(tokio::time::Duration::from_millis(5)).await;
        }
    });
    tokio::spawn(async move {
        let mut alarmed = tokio::time::Instant::now() - Duration::from_secs(5 * 60);
        loop {
            let before = COUNT.load(Acquire);
            tokio::time::sleep(tokio::time::Duration::from_secs(5)).await;
            let after = COUNT.load(Acquire);
            let δ = after - before;
            let now = tokio::time::Instant::now();
            if δ > 10 && alarmed < now - Duration::from_secs(5 * 60) {
                alarmed = now;
                tx.send((δ + 1) / 5).unwrap();
            }
        }
    });
    let http = serenity::http::Http::new("");
    let wh =
        std::env::var("AOOK").unwrap_or(std::fs::read_to_string("aook").expect("wher webhook"));
    let wh = crate::webhook::Webhook::new(&http, &wh).await;
    loop {
        match rx.try_recv() {
            Ok(x) => {
                wh.send(|m| {
                    m.content(&format!(
                        "{WARNING} <@&1202414272030974033> attacked by {x} bots/s"
                    ))
                })
                .await;
            }
            Err(TryRecvError::Closed) => panic!(),
            Err(TryRecvError::Lagged(_)) => continue,
            Err(TryRecvError::Empty) => {
                tokio::time::sleep(Duration::from_secs(4)).await;
                continue;
            }
        }
    }
}
