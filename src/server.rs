use crate::bot::Bot;
use crate::process::Process;
use axum::{
    http::header::CONTENT_TYPE,
    response::{AppendHeaders, Html},
    routing::get,
    Router, Server as AxumServer,
};

use std::{net::SocketAddr, sync::Arc};
use tokio::sync::broadcast;

// its a arced arcs
pub struct State {
    // sent from the process to the websockets
    pub stdout: broadcast::Sender<String>,
    // sent to the process
    pub stdin: broadcast::Sender<String>,
}

impl State {
    fn new(stdin: broadcast::Sender<String>) -> Self {
        let (stdout, _) = broadcast::channel(2);
        Self { stdin, stdout }
    }
}

macro_rules! html {
    ($file:expr) => {
        get(|| async {
            let ret: Html<&'static [u8]> = Html(include_bytes!(concat!(
                "../html/",
                stringify!($file),
                ".html"
            )));
            ret
        })
    };
}

macro_rules! png {
    ($file:expr) => {
        get(|| async {
            {
                (
                    AppendHeaders([(CONTENT_TYPE, "image/png")]),
                    include_bytes!(concat!("../media/", stringify!($file), ".png")),
                )
            }
        })
    };
}

pub struct Server;
impl Server {
    pub async fn spawn(addr: SocketAddr, proc: Process) {
        let (stdin_tx, stdin) = broadcast::channel(2);
        let state = Arc::new(State::new(stdin_tx));
        let router = Router::new()
            .route("/", html!(index))
            .route("/plaguess.png", png!(plaguess))
            .route("/favicon.ico", png!(logo32))
            .with_state(state.clone());
        let mut server_handle = tokio::spawn(async move {
            AxumServer::bind(&addr)
                .serve(router.into_make_service())
                .await
                .unwrap()
        });
        let mut process_handle = proc.input(stdin).output(state.stdout.clone()).link();
        Bot::spawn(state.stdout.subscribe(), state.stdin.clone()).await;
        tokio::select! {
            _ = (&mut server_handle) => process_handle.abort(),
            _ = (&mut process_handle) => server_handle.abort(),
        }
        panic!("oh no");
    }
}
