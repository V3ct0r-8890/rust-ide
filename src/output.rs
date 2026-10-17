//! Runs a program with its input and output connected to the IDE's
//! Output window (Turbo Pascal's "User screen").

use std::io::{Read, Write};
use std::process::Child;
use std::sync::{Arc, Mutex, mpsc as std_mpsc};
use std::time::Duration;

use iced::futures::channel::mpsc;
use iced::futures::{SinkExt, Stream, StreamExt};

use crate::runner::{self, RunTarget};

/// Controls a running program: send input, stop it.
#[derive(Clone)]
pub struct Handle {
    stdin: std_mpsc::Sender<Vec<u8>>,
    child: Arc<Mutex<Child>>,
}

impl std::fmt::Debug for Handle {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("output::Handle")
    }
}

impl Handle {
    pub fn send(&self, text: &str) {
        let _ = self.stdin.send(text.as_bytes().to_vec());
    }

    pub fn kill(&self) {
        if let Ok(mut child) = self.child.lock() {
            let _ = child.kill();
        }
    }
}

#[derive(Debug, Clone)]
pub enum Event {
    Started(Handle),
    Output(String),
    Exited(Option<i32>),
    Failed(String),
}

/// Starts `target` and streams its output until it exits.
pub fn run(target: RunTarget) -> impl Stream<Item = Event> {
    iced::stream::channel(100, async move |mut output| {
        let mut child = match runner::spawn_captured(&target) {
            Ok(child) => child,
            Err(e) => {
                let _ = output
                    .send(Event::Failed(format!("Cannot run {}: {e}", target.program.display())))
                    .await;
                return;
            }
        };

        let (events_tx, mut events_rx) = mpsc::unbounded();
        let mut readers = Vec::new();
        if let Some(stdout) = child.stdout.take() {
            readers.push(spawn_reader(stdout, events_tx.clone()));
        }
        if let Some(stderr) = child.stderr.take() {
            readers.push(spawn_reader(stderr, events_tx.clone()));
        }

        let (stdin_tx, stdin_rx) = std_mpsc::channel::<Vec<u8>>();
        if let Some(mut stdin) = child.stdin.take() {
            std::thread::spawn(move || {
                for data in stdin_rx {
                    if stdin.write_all(&data).and_then(|()| stdin.flush()).is_err() {
                        break;
                    }
                }
            });
        }

        let child = Arc::new(Mutex::new(child));
        let waiter_child = child.clone();
        std::thread::spawn(move || {
            let code = loop {
                let status = waiter_child.lock().ok().and_then(|mut c| c.try_wait().ok());
                match status {
                    Some(Some(status)) => break status.code(),
                    Some(None) => std::thread::sleep(Duration::from_millis(50)),
                    None => break None,
                }
            };
            // Let the readers drain the last output before reporting the exit.
            for reader in readers {
                let _ = reader.join();
            }
            let _ = events_tx.unbounded_send(Event::Exited(code));
        });

        let _ = output.send(Event::Started(Handle { stdin: stdin_tx, child })).await;
        while let Some(event) = events_rx.next().await {
            let exited = matches!(event, Event::Exited(_));
            let _ = output.send(event).await;
            if exited {
                break;
            }
        }
    })
}

/// Forwards raw output chunks (not lines, so prompts without a newline show up).
fn spawn_reader(
    mut source: impl Read + Send + 'static,
    events: mpsc::UnboundedSender<Event>,
) -> std::thread::JoinHandle<()> {
    std::thread::spawn(move || {
        let mut buffer = [0u8; 4096];
        let mut pending = Vec::new();
        loop {
            let n = match source.read(&mut buffer) {
                Ok(0) | Err(_) => break,
                Ok(n) => n,
            };
            pending.extend_from_slice(&buffer[..n]);
            let text = take_utf8(&mut pending);
            if !text.is_empty() && events.unbounded_send(Event::Output(text)).is_err() {
                return;
            }
        }
        if !pending.is_empty() {
            let _ = events.unbounded_send(Event::Output(String::from_utf8_lossy(&pending).into()));
        }
    })
}

/// Decodes as much UTF-8 as possible, keeping an incomplete trailing sequence.
fn take_utf8(pending: &mut Vec<u8>) -> String {
    match std::str::from_utf8(pending) {
        Ok(text) => {
            let text = text.to_string();
            pending.clear();
            text
        }
        Err(e) if e.error_len().is_none() => {
            let valid = e.valid_up_to();
            let text = String::from_utf8_lossy(&pending[..valid]).into_owned();
            pending.drain(..valid);
            text
        }
        Err(_) => {
            let text = String::from_utf8_lossy(pending).into_owned();
            pending.clear();
            text
        }
    }
}

/// Appends program output to the screen buffer: strips ANSI escape codes,
/// treats a lone `\r` as "back to start of line" (progress bars), and keeps
/// the buffer bounded.
pub fn append(screen: &mut String, chunk: &str) {
    const MAX: usize = 200_000;
    let mut chars = chunk.chars().peekable();
    while let Some(c) = chars.next() {
        match c {
            '\u{1b}' => match chars.next() {
                Some('[') => {
                    // CSI: parameters until a final byte in '@'..='~'
                    while let Some(c) = chars.next() {
                        if ('@'..='~').contains(&c) {
                            break;
                        }
                    }
                }
                Some(']') => {
                    // OSC: until BEL or ESC \
                    while let Some(c) = chars.next() {
                        if c == '\u{7}' || (c == '\u{1b}' && chars.next_if_eq(&'\\').is_some()) {
                            break;
                        }
                    }
                }
                _ => {}
            },
            '\r' if chars.peek() == Some(&'\n') => {}
            '\r' => {
                let line_start = screen.rfind('\n').map_or(0, |i| i + 1);
                screen.truncate(line_start);
            }
            '\u{8}' => {
                screen.pop();
            }
            c => screen.push(c),
        }
    }
    if screen.len() > MAX {
        let cut = screen.len() - MAX;
        let cut = screen[cut..].find('\n').map_or(cut, |i| cut + i + 1);
        screen.drain(..cut);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn strips_ansi_and_handles_carriage_return() {
        let mut screen = String::new();
        append(&mut screen, "\u{1b}[32mgreen\u{1b}[0m\r\n");
        append(&mut screen, "50%\r100%\n");
        assert_eq!(screen, "green\n100%\n");
    }

    #[test]
    fn keeps_split_utf8_sequences() {
        let bytes = "ไทย".as_bytes();
        let mut pending = bytes[..4].to_vec();
        assert_eq!(take_utf8(&mut pending), "ไ");
        pending.extend_from_slice(&bytes[4..]);
        assert_eq!(take_utf8(&mut pending), "ทย");
    }

    #[test]
    fn captures_program_output_and_input() {
        let dir = std::env::temp_dir().join("rust-ide-output-test");
        std::fs::create_dir_all(&dir).unwrap();
        let script = dir.join("echo.py");
        std::fs::write(&script, "name = input('Name? ')\nprint('Hello', name)\n").unwrap();
        let target = RunTarget {
            program: crate::platform::find_program(crate::platform::python()).unwrap(),
            args: vec![script.to_string_lossy().into_owned()],
            cwd: dir,
            gui: false,
            entry: None,
        };

        let events = iced::futures::executor::block_on_stream(Box::pin(run(target)));
        let mut screen = String::new();
        let mut code = None;
        for event in events {
            match event {
                Event::Started(handle) => handle.send("Turbo\n"),
                Event::Output(text) => append(&mut screen, &text),
                Event::Exited(c) => code = c,
                Event::Failed(e) => panic!("{e}"),
            }
        }
        assert_eq!(code, Some(0));
        assert!(screen.contains("Name? Hello Turbo"), "{screen:?}");
    }
}
