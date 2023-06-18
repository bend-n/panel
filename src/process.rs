use std::time::Duration;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;
use tokio::sync::broadcast;
use tokio::sync::broadcast::error::TryRecvError;
use tokio::task::JoinHandle;

pub struct Process {
    inner: TcpStream,
    input: Option<broadcast::Receiver<String>>,
    output: Option<broadcast::Sender<String>>,
}

impl Process {
    /// spawns the server
    #[must_use]
    pub async fn spawn() -> anyhow::Result<Self> {
        let stream = TcpStream::connect("localhost:6859").await?;
        Ok(Self {
            inner: stream,
            input: None,
            output: None,
        })
    }

    pub fn input(mut self, input: broadcast::Receiver<String>) -> Self {
        self.input = Some(input);
        self
    }

    pub fn output(mut self, output: broadcast::Sender<String>) -> Self {
        self.output = Some(output);
        self
    }

    pub fn link(mut self) -> JoinHandle<()> {
        define_print!("process");
        let mut input = self.input.unwrap();
        let output = self.output.unwrap();
        tokio::spawn(async move {
            let mut stdout = [0; 4096];
            loop {
                if output.receiver_count() == 0 {
                    async_std::task::sleep(Duration::from_millis(500)).await;
                    continue;
                }
                match input.try_recv() {
                    Err(e) => match e {
                        TryRecvError::Closed => fail!("closed"),
                        TryRecvError::Lagged(_) => continue,
                        TryRecvError::Empty => {}
                    },
                    Ok(mut s) => {
                        input!("{s}");
                        s += "\n";
                        self.inner.write_all(s.as_bytes()).await.unwrap();
                        // let mut last = 250;
                        // while let Err(e) = self.inner.write_all(s.as_bytes()).await {
                        //     last *= last;
                        //     if e.kind() == std::io::ErrorKind::BrokenPipe {
                        //         println!("failed write, waiting {last}ms to retry.");
                        //         async_std::task::sleep(Duration::from_millis(last)).await;
                        //         continue;
                        //     }
                        //     panic!("{e:?}");
                        // }
                        self.inner.flush().await.unwrap();
                    }
                }

                let string = {
                    let n = tokio::select! {
                        n = {self.inner.read(&mut stdout)} => n.unwrap(),
                        _ = async_std::task::sleep(Duration::from_millis(500)) => continue,
                    };
                    String::from_utf8_lossy(&stdout[..n]).into_owned()
                };
                for line in string.lines() {
                    output!("{line}");
                }
                let stripped =
                    String::from_utf8_lossy(&strip_ansi_escapes::strip(&string).unwrap())
                        .into_owned();
                output.send(stripped).unwrap();
                async_std::task::sleep(Duration::from_millis(500)).await;
            }
        })
    }
}
