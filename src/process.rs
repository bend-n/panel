use ansi_to_html::convert_escaped;
use std::process::Stdio;
use std::sync::Arc;
use std::{ffi::OsString, time::Duration};
use tokio::io::{AsyncReadExt, AsyncWriteExt, BufReader};
use tokio::process::{Child, ChildStdin, ChildStdout, Command};
use tokio::sync::broadcast;
use tokio::sync::broadcast::error::TryRecvError;
use tokio::task::JoinHandle;

use crate::server::State;
pub struct Process {
    _inner: Child,
    input: Option<broadcast::Receiver<String>>,
    html_output: Option<broadcast::Sender<String>>,
    plain_output: Option<broadcast::Sender<String>>,
    stdout: BufReader<ChildStdout>,
    stdin: ChildStdin,
}

impl Process {
    /// spawns the server
    #[must_use]
    pub fn spawn(server_dir: OsString) -> Self {
        let mut p = Command::new("bash")
            .arg("run.sh")
            .current_dir(server_dir)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .spawn()
            .expect("failed to spawn");

        Self {
            // mindus doesnt output stderr
            stdout: BufReader::new(p.stdout.take().unwrap()),
            stdin: p.stdin.take().unwrap(),
            _inner: p,
            input: None,
            html_output: None,
            plain_output: None,
        }
    }

    pub fn input(mut self, input: broadcast::Receiver<String>) -> Self {
        self.input = Some(input);
        self
    }

    pub fn html_output(mut self, output: broadcast::Sender<String>) -> Self {
        self.html_output = Some(output);
        self
    }

    pub fn plain_output(mut self, output: broadcast::Sender<String>) -> Self {
        self.plain_output = Some(output);
        self
    }

    pub fn with_state(self, state: &Arc<State>) -> Self {
        self.html_output(state.stdout_html.clone())
            .plain_output(state.stdout_plain.clone())
    }

    pub fn link(mut self) -> JoinHandle<()> {
        define_print!("process");
        let mut input = self.input.unwrap();
        let html_output = self.html_output.unwrap();
        let plain_output = self.plain_output.unwrap();
        tokio::spawn(async move {
            let mut stdout = [0; 4096];
            loop {
                if html_output.receiver_count() + plain_output.receiver_count() == 0 {
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
                        self.stdin.write_all(s.as_bytes()).await.unwrap();
                        self.stdin.flush().await.unwrap();
                    }
                }

                let string = {
                    let n = tokio::select! {
                        n = {self.stdout.read(&mut stdout)} => n.unwrap(),
                        _ = async_std::task::sleep(Duration::from_millis(500)) => continue,
                    };
                    String::from_utf8_lossy(&stdout[..n]).into_owned()
                };
                for line in string.lines() {
                    output!("{line}");
                }
                if plain_output.receiver_count() > 0 {
                    let stripped =
                        String::from_utf8_lossy(&strip_ansi_escapes::strip(&string).unwrap())
                            .into_owned();
                    plain_output.send(stripped).unwrap();
                }
                if html_output.receiver_count() > 0 {
                    html_output.send(ansi2html(&string)).unwrap();
                }
                nooutput!();
                async_std::task::sleep(Duration::from_millis(500)).await;
            }
        })
    }
}

/// for dark theme
fn ansi2html(ansi: &str) -> String {
    convert_escaped(ansi)
        .unwrap()
        .replace("#555", "#a4a4a0")
        .replace("#55f", "#7486fd")
        .replace("#fff", "wheat")
        .replace("#a00", "#d05047")
}
