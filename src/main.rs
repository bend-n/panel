#![feature(utf8_chunks)]

use std::str::FromStr;

#[macro_use]
mod logging;
mod process;
mod server;
mod webhook;
mod websocket;

use process::*;
use server::*;
use std::net::SocketAddr;

#[tokio::main]
async fn main() {
    let process = Process::spawn(
        std::env::var("SERVER_DIR")
            .unwrap_or("~/mserv".replace('~', &std::env::var("HOME").unwrap_or("/root".into())))
            .into(),
    );
    Server::spawn(
        SocketAddr::from((
            [0, 0, 0, 0],
            std::env::var("PORT").map_or(4001, |x| u16::from_str(&x).unwrap()),
        )),
        process,
    )
    .await;
}
