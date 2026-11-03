//! Follows the journal of the `wg-quick` services in a background thread and
//! hands the lines over to the UI, like `utility/log_thread.py` did with a
//! `QThread`.

use std::io::{BufRead, BufReader};
use std::process::{Command, Stdio};
use std::sync::mpsc::{self, Receiver};
use std::thread;

/// Receives the lines printed by `journalctl`.
pub struct LogWatcher {
    receiver: Receiver<String>,
}

impl LogWatcher {
    /// Starts following the journal of every `wg-quick@` unit.
    ///
    /// The last 200 lines are read first so the log tab is not empty when the
    /// application starts.
    pub fn spawn() -> Self {
        let (sender, receiver) = mpsc::channel();

        thread::Builder::new()
            .name("journalctl".to_owned())
            .spawn(move || {
                let mut child = match Command::new("journalctl")
                    .arg("--unit")
                    .arg("wg-quick@*")
                    .arg("--follow")
                    .arg("--lines")
                    .arg("200")
                    .arg("--no-pager")
                    .arg("--output")
                    .arg("short-iso")
                    .stdin(Stdio::null())
                    .stdout(Stdio::piped())
                    .stderr(Stdio::null())
                    .spawn()
                {
                    Ok(child) => child,
                    Err(_) => return,
                };

                if let Some(stdout) = child.stdout.take() {
                    for line in BufReader::new(stdout).lines().map_while(Result::ok) {
                        if sender.send(line).is_err() {
                            break;
                        }
                    }
                }

                let _ = child.kill();
                let _ = child.wait();
            })
            .ok();

        Self { receiver }
    }

    /// Takes every line received since the last call.
    pub fn drain(&self) -> Vec<String> {
        let mut lines = Vec::new();

        while let Ok(line) = self.receiver.try_recv() {
            lines.push(line);
        }

        lines
    }
}
