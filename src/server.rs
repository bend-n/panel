use crate::bot::Bot;
use crate::process::Process;
use axum::{
    http::header::CONTENT_TYPE,
    response::{AppendHeaders, Html, IntoResponse},
    routing::get,
    Router, Server as AxumServer,
};

use std::{net::SocketAddr, sync::Arc};
use tokio::{sync::broadcast, task::JoinHandle, time::sleep, time::Duration};

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
        Self { stdout, stdin }
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
            (
                AppendHeaders([(CONTENT_TYPE, "image/png")]),
                include_bytes!(concat!("../media/", stringify!($file), ".png")),
            )
        })
    };
}

async fn map_view(
    axum::extract::State(state): axum::extract::State<Arc<State>>,
) -> impl IntoResponse {
    (
        AppendHeaders([(CONTENT_TYPE, "image/png")]),
        crate::bot::maps::MAP_IMAGE
            .get(&state.stdin)
            .await
            .unwrap()
            .0
            .clone(),
    )
}

pub struct Server;
impl Server {
    pub async fn spawn(addr: SocketAddr) {
        let (stdin_tx, stdin) = broadcast::channel(2);
        let state = Arc::new(State::new(stdin_tx));
        let router = Router::new()
            .route("/", html!(index))
            .route("/plaguess.png", png!(plaguess))
            .route("/favicon.ico", png!(logo32))
            .route("/view", get(map_view))
            .with_state(state.clone());
        tokio::spawn(async move {
            AxumServer::bind(&addr)
                .serve(router.into_make_service())
                .await
                .unwrap();
        });
        let stdout = state.stdout.clone();
        tokio::spawn(async move {
            let mut process_handle: Option<JoinHandle<()>> = None;
            let mut backoff = 1u64;
            macro_rules! backoff {
                () => {
                    backoff <<= 1;
                    sleep(Duration::from_secs(backoff)).await;
                    continue;
                };
            }
            loop {
                if let Some(h) = process_handle {
                    let _ = h.await;
                    process_handle = None;
                    println!("process died; waiting {}s", backoff << 2);
                }
                let Ok(spawn) = Process::spawn().await else {
                    backoff!();
                };
                process_handle = Some(
                    spawn
                        .input(stdin.resubscribe())
                        .output(stdout.clone())
                        .link(),
                );
                if backoff == 1 {
                    continue;
                }
                backoff!();
            }
        });
        Bot::spawn(state.stdout.subscribe(), state.stdin.clone()).await;
    }
}
