use crate::server::State;
use axum::extract::ws::{Message, WebSocket as RealWebSocket};
use futures_util::SinkExt;
use std::{
    sync::Arc,
    time::{Duration, Instant},
};
use tokio::{sync::broadcast::error::TryRecvError, task::JoinHandle};
use tokio_stream::StreamExt;
pub struct WebSocket(JoinHandle<()>);
impl WebSocket {
    pub async fn spawn(stream: RealWebSocket, state: Arc<State>) {
        let (mut sender, mut reciever) = futures::stream::StreamExt::split(stream);
        let mut stdout = state.stdout_html.subscribe();
        dummy_print!("websocket");
        let mut last: Option<Instant> = None;
        let mut waiting: usize = 0;
        loop {
            let out = stdout.try_recv();
            let now = Instant::now();
            match out {
                Err(e) => match e {
                    TryRecvError::Closed => fail!("closed"),
                    TryRecvError::Lagged(_) => continue, // no delay
                    _ => {
                        if let Some(earlier) = last {
                            let since = now.duration_since(earlier).as_millis();
                            if since > 200 || waiting > 15 {
                                last.take();
                                sender.flush().await.unwrap();
                                waiting = 0;
                                flush!();
                            }
                        }
                    }
                },
                Ok(m) => {
                    #[allow(unused_variables)]
                    for line in m.lines() {
                        input!("{line}");
                        if let Err(e) = sender.feed(Message::Text(line.to_owned())).await {
                            fail!("{e}");
                        };
                        waiting += 1;
                    }
                    last = Some(now);
                }
            }
            match tokio::select! {
                next = reciever.next() => next,
                _ = async_std::task::sleep(Duration::from_millis(20)) => continue,
            } {
                Some(r) => match r {
                    Ok(m) => {
                        if let Message::Text(m) = m {
                            output!("{m}");
                            state.stdin.send(m).unwrap();
                        }
                    }
                    #[allow(unused_variables)]
                    Err(e) => {
                        fail!("{e}");
                    }
                },
                None => {
                    nooutput!();
                }
            }
            async_std::task::sleep(Duration::from_millis(20)).await;
            continue;
        }
    }
}
