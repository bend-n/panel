#![feature(lazy_cell, let_chains)]
use std::str::FromStr;
#[macro_use]
mod logging;
#[macro_use]
mod bot;
mod process;
mod server;
mod webhook;

use server::*;
use std::net::SocketAddr;
// loads all the images into memory, ~300mb
use mindus::data::renderer::warmup;

#[tokio::main(flavor = "current_thread")]
async fn main() {
    warmup();
    Server::spawn(SocketAddr::from((
        [0, 0, 0, 0],
        std::env::var("PORT").map_or(4001, |x| u16::from_str(&x).unwrap()),
    )))
    .await;
}
