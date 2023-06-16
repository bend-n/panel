use crate::bot::Bot;
use crate::process::Process;
use crate::websocket::WebSocket;
use axum::{
    extract::{
        ws::{Message, WebSocketUpgrade},
        Path, State as StateW,
    },
    http::header::CONTENT_TYPE,
    response::{AppendHeaders, Html, IntoResponse},
    routing::get,
    Router, Server as AxumServer,
};
use futures::sink::SinkExt;
use std::{net::SocketAddr, sync::Arc};
use tokio::sync::broadcast;

pub struct State {
    // sent from the process to the websockets
    pub stdout_html: broadcast::Sender<String>,
    pub stdout_plain: broadcast::Sender<String>,
    // sent to the process
    pub stdin: broadcast::Sender<String>,
}

impl State {
    fn new(stdin: broadcast::Sender<String>) -> Self {
        let (stdout_html, _) = broadcast::channel(16);
        let (stdout_plain, _) = broadcast::channel(2);
        Self {
            stdin,
            stdout_html,
            stdout_plain,
        }
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
            .route("/panel", html!(panel))
            .route("/plaguess.png", png!(plaguess))
            .route("/favicon.ico", png!(logo32))
            .route("/connect/:id", get(connect_ws))
            .with_state(state.clone());
        let mut server_handle = tokio::spawn(async move {
            AxumServer::bind(&addr)
                .serve(router.into_make_service())
                .await
                .unwrap()
        });
        let mut process_handle = proc.input(stdin).with_state(&state).link();
        Bot::spawn(state.stdout_plain.subscribe(), state.stdin.clone()).await;
        tokio::select! {
            _ = (&mut server_handle) => process_handle.abort(),
            _ = (&mut process_handle) => server_handle.abort(),
        }
        panic!("oh no");
    }
}

fn matches(id: &str) -> bool {
    std::env::var("ID").as_deref().unwrap_or("4") == id
}

async fn connect_ws(
    ws: WebSocketUpgrade,
    StateW(state): StateW<Arc<State>>,
    Path(id): Path<String>,
) -> impl IntoResponse {
    ws.on_upgrade(|socket| async move {
        if !matches(&id) {
            let mut s = futures::stream::StreamExt::split(socket).0;
            let _ = s.send(Message::Text("correct id".to_string())).await;
            return;
        }
        tokio::spawn(WebSocket::spawn(socket, state));
    })
}
