use ansi_to_html::convert_escaped;
use std::process::Stdio;
use std::{ffi::OsString, time::Duration};
use tokio::io::{AsyncReadExt, AsyncWriteExt, BufReader};
use tokio::process::{Child, ChildStdin, ChildStdout, Command};
use tokio::sync::broadcast;
use tokio::sync::broadcast::error::TryRecvError;
use tokio::task::JoinHandle;
pub struct Process {
    _inner: Child,
    input: Option<broadcast::Receiver<String>>,
    output: Option<broadcast::Sender<String>>,
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
            output: None,
        }
    }

    pub fn input(mut self, input: broadcast::Receiver<String>) -> Self {
        self.input = Some(input);
        return self;
    }

    pub fn output(mut self, output: broadcast::Sender<String>) -> Self {
        self.output = Some(output);
        return self;
    }

    pub fn link(mut self) -> JoinHandle<()> {
        define_print!("process");
        let mut input = self.input.unwrap();
        let output = self.output.unwrap();
        tokio::spawn(async move {
            let mut stdout = [0; 4096];
            loop {
                nextiter!();
                if output.receiver_count() == 0 {
                    async_std::task::sleep(Duration::from_millis(500)).await;
                    cont!();
                }
                match input.try_recv() {
                    Err(e) => match e {
                        TryRecvError::Empty => noinput!(),
                        TryRecvError::Closed => fail!("closed"),
                        TryRecvError::Lagged(_) => noinput!("lagged"),
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
                        _ = async_std::task::sleep(Duration::from_millis(500)) => cont!()
                    };
                    String::from_utf8_lossy(&stdout[..n]).into_owned()
                };
                for line in string.lines() {
                    output!("{line}");
                    output.send(ansi2html(&line)).unwrap();
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
