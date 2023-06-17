#![feature(lazy_cell)]

use std::str::FromStr;

#[macro_use]
mod logging;
#[macro_use]
mod bot;
mod process;
mod server;
mod webhook;

use process::*;
use server::*;
use std::net::SocketAddr;

#[tokio::main]
async fn main() {
    let process = Process::spawn().await;
    Server::spawn(
        SocketAddr::from((
            [0, 0, 0, 0],
            std::env::var("PORT").map_or(4001, |x| u16::from_str(&x).unwrap()),
        )),
        process,
    )
    .await;
}
