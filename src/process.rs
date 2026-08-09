use std::io::Read;
use std::process::{Command, ExitStatus, Stdio};
use std::sync::mpsc::{self, Receiver};
use std::thread;
use std::time::{Duration, Instant};

use crate::limits::MAX_DIAGNOSTIC_BYTES;

#[derive(Debug)]
pub struct ProcessOutput {
    pub status: Option<ExitStatus>,
    pub timed_out: bool,
    pub stdout: Capture,
    pub stderr: Capture,
}

#[derive(Debug, Default)]
pub struct Capture {
    pub bytes: Vec<u8>,
    pub truncated: bool,
}

pub fn run(mut command: Command, timeout: Duration) -> Result<ProcessOutput, ()> {
    let mut child = command
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|_| ())?;
    let stdout = child.stdout.take().map(spawn_capture);
    let stderr = child.stderr.take().map(spawn_capture);
    let started = Instant::now();
    let (status, timed_out) = loop {
        match child.try_wait() {
            Ok(Some(status)) => break (Some(status), false),
            Ok(None) if started.elapsed() < timeout => {
                thread::sleep(Duration::from_millis(10));
            }
            Ok(None) => {
                let _ = child.kill();
                break (child.wait().ok(), true);
            }
            Err(_) => {
                let _ = child.kill();
                let _ = child.wait();
                break (None, false);
            }
        }
    };

    Ok(ProcessOutput {
        status,
        timed_out,
        stdout: join_capture(stdout),
        stderr: join_capture(stderr),
    })
}

fn spawn_capture(mut reader: impl Read + Send + 'static) -> Receiver<Capture> {
    let (sender, receiver) = mpsc::sync_channel(1);
    thread::spawn(move || {
        let mut retained = Vec::new();
        let mut buffer = [0u8; 8 * 1024];
        let mut truncated = false;
        loop {
            match reader.read(&mut buffer) {
                Ok(0) | Err(_) => break,
                Ok(length) => {
                    let available = MAX_DIAGNOSTIC_BYTES.saturating_sub(retained.len());
                    let to_keep = available.min(length);
                    retained.extend_from_slice(&buffer[..to_keep]);
                    truncated |= to_keep != length;
                }
            }
        }
        let _ = sender.send(Capture {
            bytes: retained,
            truncated,
        });
    });
    receiver
}

fn join_capture(receiver: Option<Receiver<Capture>>) -> Capture {
    receiver
        .and_then(|receiver| receiver.recv_timeout(Duration::from_secs(1)).ok())
        .unwrap_or(Capture {
            bytes: Vec::new(),
            truncated: true,
        })
}

#[cfg(test)]
mod tests {
    use std::process::Command;

    use super::*;

    #[test]
    fn bounded_capture_drains_and_marks_truncation() {
        let bytes = vec![b'x'; MAX_DIAGNOSTIC_BYTES + 10];
        let captured = spawn_capture(std::io::Cursor::new(bytes))
            .recv_timeout(Duration::from_secs(1))
            .unwrap();
        assert_eq!(captured.bytes.len(), MAX_DIAGNOSTIC_BYTES);
        assert!(captured.truncated);
    }

    #[test]
    fn captures_a_successful_process() {
        let command = if cfg!(windows) {
            let mut command = Command::new("cmd");
            command.args(["/C", "echo output"]);
            command
        } else {
            let mut command = Command::new("sh");
            command.args(["-c", "printf output"]);
            command
        };
        let output = run(command, Duration::from_secs(2)).unwrap();
        assert!(output.status.is_some_and(|status| status.success()));
        assert_eq!(output.stdout.bytes.trim_ascii_end(), b"output");
    }

    #[test]
    fn terminates_a_process_at_the_deadline() {
        let mut command = if cfg!(windows) {
            let mut command = Command::new("cmd");
            command.args(["/C", "ping -n 6 127.0.0.1 >nul"]);
            command
        } else {
            let mut command = Command::new("sh");
            command.args(["-c", "sleep 5"]);
            command
        };
        #[cfg(unix)]
        command.env_clear().env("PATH", "/usr/bin:/bin");
        let started = Instant::now();
        let output = run(command, Duration::from_millis(10)).unwrap();
        assert!(output.timed_out);
        assert!(started.elapsed() < Duration::from_secs(3));
    }
}
