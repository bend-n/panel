use crate::process::Process;
use axum::{
    extract::{
        ws::{Message, WebSocket, WebSocketUpgrade},
        Path, State as WsState,
    },
    http::header::CONTENT_TYPE,
    response::{AppendHeaders, Html, IntoResponse},
    routing::get,
    Router, Server as AxumServer,
};
use futures::sink::SinkExt;
use minify_html::{minify, Cfg};
use paste::paste;
use std::{
    net::SocketAddr,
    sync::{Arc, OnceLock},
    time::{Duration, Instant},
};
use tokio::sync::broadcast::{self, error::TryRecvError};
use tokio_stream::StreamExt;

struct State {
    // sent from the process to the websockets
    stdout: broadcast::Sender<String>,
    // sent by websockets to the process
    stdin: broadcast::Sender<String>,
}

impl State {
    fn new(stdin: broadcast::Sender<String>) -> Self {
        let (stdout, _rx) = broadcast::channel(5);
        Self { stdin, stdout }
    }
}

macro_rules! html {
    ($file:expr) => {
        get(paste!(
            || async {
                static [<$file:upper>]: OnceLock<Vec<u8>> = OnceLock::new();
                Html(from_utf8([<$file:upper>].get_or_init(|| {
                    minify(
                        include_bytes!(concat!("../html/", stringify!($file), ".html")),
                        &Cfg {
                            minify_js: true,
                            minify_css: true,
                            ..Default::default()
                        },
                    )
                })).replace("ws://localhost:4001/connect/", &format!("{}", std::env::var("URL").unwrap_or("ws://localhost:4001/connect/".to_string()))))
            }
        ))
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
            .route("/connect/:id", get(connect))
            .with_state(state.clone());
        let mut server_handle = tokio::spawn(async move {
            AxumServer::bind(&addr)
                .serve(router.into_make_service())
                .await
                .unwrap()
        });
        let mut process_handle = proc.input(stdin).output(state.stdout.clone()).link();
        tokio::select! {
            _ = (&mut server_handle) => process_handle.abort(),
            _ = (&mut process_handle) => server_handle.abort(),
        }
        panic!("oh no");
    }
}

/// like [String::from_utf8_lossy] but instead of being lossy it panics
pub fn from_utf8(v: &[u8]) -> &str {
    let mut iter = std::str::Utf8Chunks::new(v);
    if let Some(chunk) = iter.next() {
        let valid = chunk.valid();
        if chunk.invalid().is_empty() {
            debug_assert_eq!(valid.len(), v.len());
            return valid;
        }
    } else {
        return "";
    };
    unreachable!("invalid utf8")
}

async fn connect(
    ws: WebSocketUpgrade,
    WsState(state): WsState<Arc<State>>,
    Path(id): Path<String>,
) -> impl IntoResponse {
    ws.on_upgrade(|socket| async move {
        if std::env::var("ID").unwrap_or_else(|_| "4".to_string()) != id {
            let mut s = futures::stream::StreamExt::split(socket).0;
            let _ = s.send(Message::Text("correct id".to_string())).await;
            return;
        }
        setup(socket, state).await;
    })
}

async fn setup(stream: WebSocket, state: Arc<State>) {
    let (mut sender, mut reciever) = futures::stream::StreamExt::split(stream);
    let mut stdout = state.stdout.subscribe();
    let ws_task = tokio::spawn(async move {
        define_print!("websocket");
        let mut last: Option<Instant> = None;
        let mut waiting: usize = 0;
        loop {
            nextiter!();
            let out = stdout.try_recv();
            let now = Instant::now();
            match out {
                Err(e) => match e {
                    TryRecvError::Closed => fail!("closed"),
                    _ => {
                        if let Some(earlier) = last {
                            let since = now.duration_since(earlier).as_millis();
                            if since > 600 || waiting > 10 {
                                last.take();
                                sender.flush().await.unwrap();
                                waiting = 0;
                                flush!();
                            }
                        }
                        noinput!();
                        // async_std::task::sleep(Duration::from_millis(500)).await;
                        // cont!();
                    }
                },
                Ok(m) => {
                    input!("{m}");
                    if let Err(e) = sender.feed(Message::Text(m)).await {
                        fail!("{e}");
                    };
                    last = Some(now);
                    waiting += 1;
                }
            }
            match tokio::select! {
                next = reciever.next() => next,
                _ = async_std::task::sleep(Duration::from_millis(100)) => cont!(),
            } {
                Some(r) => match r {
                    Ok(m) => {
                        if let Message::Text(m) = m {
                            output!("{m}");
                            state.stdin.send(m).unwrap();
                        }
                    }
                    Err(e) => {
                        fail!("{e}");
                    }
                },
                None => {
                    nooutput!()
                }
            }
            async_std::task::sleep(Duration::from_millis(100)).await;
            cont!();
        }
    });
    // let mut recv_task = tokio::spawn(async move {
    //     while let Some(Ok(Message::Text(m))) = reciever.try_next().await {
    //         println!("ws sent {m}");
    //         state.stdin.send(m).unwrap();
    //     }
    // });

    ws_task.await.unwrap();
    println!("websocket !! finish");
}
