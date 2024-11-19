#![feature(let_chains, iter_intersperse)]
#![allow(mixed_script_confusables)]
use std::str::FromStr;
#[macro_use]
mod logging;
mod alerts;
mod bot;
mod process;
mod server;
mod webhook;

use server::*;
use std::net::SocketAddr;
emojib::the_crate! {}
#[tokio::main(flavor = "current_thread")]
async fn main() {
    tokio::spawn(alerts::run());
    Server::spawn(SocketAddr::from((
        [0, 0, 0, 0],
        std::env::var("PORT").map_or(4001, |x| u16::from_str(&x).unwrap()),
    )))
    .await;
}
