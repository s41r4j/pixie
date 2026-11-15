//! The recogniser as a child process.
//!
//! Pixie starts itself again as `pixie --stt-engine <runtime> <model>` ([`run`]) and talks to it
//! over pipes ([`Engine`]). A library that crashes or a model that won't load ends that child,
//! not Pixie, and the model stays loaded between dictations so the first word isn't late.
//!
//! In (stdin), frames of `tag: u8, length: u32 LE, payload`:
//!   `A` audio: 16 kHz mono f32 LE     `E` they stopped: finish and send the final text
//!   `R` forget what was heard (a cancelled dictation)
//! Out (stdout), one JSON object per line:
//!   `{"ready":true}`   `{"text":"…"}` whenever the words so far change
//!   `{"text":"…","final":true}` after `E`   `{"fatal":"…"}` when it can't start

use crate::ffi::Recognizer;
use crate::text::Transcript;
use std::io::{BufRead, BufReader, Read, Write};
use std::path::Path;
use std::process::{Child, ChildStdin, Command, Stdio};
use std::sync::mpsc::{channel, Receiver, RecvTimeoutError};
use std::sync::{Arc, Mutex};
use std::time::Duration;

/// The first argument that makes the program an engine instead of the app.
pub const FLAG: &str = "--stt-engine";

/// `Some((runtime, model))` when this launch is the engine child.
pub fn is_engine_launch(args: &[String]) -> Option<(&str, &str)> {
    match args {
        [_, flag, runtime, model, ..] if flag == FLAG => Some((runtime, model)),
        _ => None,
    }
}

fn say(out: &mut impl Write, v: serde_json::Value) {
    let _ = writeln!(out, "{v}");
    let _ = out.flush();
}

fn frame(input: &mut impl Read) -> Option<(u8, Vec<u8>)> {
    let mut head = [0u8; 5];
    input.read_exact(&mut head).ok()?;
    let len = u32::from_le_bytes([head[1], head[2], head[3], head[4]]) as usize;
    // A second of audio is 64 KB; anything huge is a broken stream.
    if len > 8 << 20 {
        return None;
    }
    let mut body = vec![0u8; len];
    input.read_exact(&mut body).ok()?;
    Some((head[0], body))
}

/// The child's whole life. Returns the exit code.
pub fn run(runtime: &str, model: &str) -> i32 {
    let mut out = std::io::stdout().lock();
    let threads = std::thread::available_parallelism().map(|n| n.get()).unwrap_or(2).clamp(1, 2) as i32;
    let mut rec = match Recognizer::open(Path::new(runtime), Path::new(model), threads) {
        Ok(r) => r,
        Err(e) => {
            say(&mut out, serde_json::json!({ "fatal": e }));
            return 1;
        }
    };
    say(&mut out, serde_json::json!({ "ready": true }));
    let mut input = std::io::stdin().lock();
    let mut heard = Transcript::default();
    let mut last = String::new();
    while let Some((tag, body)) = frame(&mut input) {
        match tag {
            b'A' => {
                let samples: Vec<f32> = body.as_chunks::<4>().0.iter().map(|b| f32::from_le_bytes(*b)).collect();
                rec.accept(&samples);
                heard.hear(&rec.text());
                if rec.at_pause() {
                    heard.pause();
                    rec.reset();
                }
                let now = heard.text();
                if now != last {
                    say(&mut out, serde_json::json!({ "text": now }));
                    last = now;
                }
            }
            b'E' => {
                heard.hear(&rec.finish());
                heard.pause();
                say(&mut out, serde_json::json!({ "text": heard.text(), "final": true }));
                heard = Transcript::default();
                last.clear();
            }
            b'R' => {
                rec.reset();
                heard = Transcript::default();
                last.clear();
            }
            _ => {}
        }
    }
    0
}

/// Pixie's side of the pipe.
pub struct Engine {
    child: Child,
    stdin: ChildStdin,
    finals: Receiver<String>,
    last: Arc<Mutex<String>>,
}

impl Engine {
    /// Start the child (`command` is the program with [`FLAG`], the runtime and the model) and
    /// wait until the model is loaded. `on_text` gets the words so far each time they change,
    /// on the reader thread.
    pub fn start(mut command: Command, on_text: impl Fn(&str) + Send + 'static) -> Result<Self, String> {
        let mut child = command.stdin(Stdio::piped()).stdout(Stdio::piped()).stderr(Stdio::null()).spawn().map_err(|e| format!("couldn't start the speech engine: {e}"))?;
        let stdin = child.stdin.take().ok_or("no pipe to the speech engine")?;
        let stdout = child.stdout.take().ok_or("no pipe from the speech engine")?;
        let (ready_tx, ready_rx) = channel::<Result<(), String>>();
        let (final_tx, finals) = channel::<String>();
        let last = Arc::new(Mutex::new(String::new()));
        let last2 = last.clone();
        std::thread::Builder::new()
            .name("pixie-stt-reader".into())
            .spawn(move || {
                for line in BufReader::new(stdout).lines().map_while(Result::ok) {
                    let Ok(v) = serde_json::from_str::<serde_json::Value>(&line) else { continue };
                    if v["ready"].as_bool() == Some(true) {
                        let _ = ready_tx.send(Ok(()));
                    } else if let Some(e) = v["fatal"].as_str() {
                        let _ = ready_tx.send(Err(e.to_string()));
                    } else if let Some(t) = v["text"].as_str() {
                        if v["final"].as_bool() == Some(true) {
                            last2.lock().unwrap().clear();
                            let _ = final_tx.send(t.to_string());
                        } else {
                            *last2.lock().unwrap() = t.to_string();
                            on_text(t);
                        }
                    }
                }
            })
            .map_err(|e| e.to_string())?;
        match ready_rx.recv_timeout(Duration::from_secs(20)) {
            Ok(Ok(())) => Ok(Self { child, stdin, finals, last }),
            Ok(Err(e)) => {
                let _ = child.wait();
                Err(e)
            }
            Err(_) => {
                let _ = child.kill();
                let _ = child.wait();
                Err("the speech engine didn't start".into())
            }
        }
    }

    fn send(&mut self, tag: u8, body: &[u8]) -> bool {
        let mut head = [tag, 0, 0, 0, 0];
        head[1..].copy_from_slice(&(body.len() as u32).to_le_bytes());
        self.stdin.write_all(&head).and_then(|_| self.stdin.write_all(body)).and_then(|_| self.stdin.flush()).is_ok()
    }

    /// More 16 kHz mono audio. False when the engine has gone.
    pub fn audio(&mut self, samples: &[f32]) -> bool {
        let mut body = Vec::with_capacity(samples.len() * 4);
        for s in samples {
            body.extend_from_slice(&s.to_le_bytes());
        }
        self.send(b'A', &body)
    }

    /// They stopped talking: everything heard, with the last word finished. If the engine
    /// doesn't answer in time, the words it had sent so far.
    pub fn finish(&mut self) -> String {
        // An answer left over from an earlier, abandoned finish isn't this one's.
        while self.finals.try_recv().is_ok() {}
        if self.send(b'E', &[]) {
            match self.finals.recv_timeout(Duration::from_secs(4)) {
                Ok(t) => return t,
                Err(RecvTimeoutError::Timeout) | Err(RecvTimeoutError::Disconnected) => {}
            }
        }
        std::mem::take(&mut *self.last.lock().unwrap())
    }

    /// Throw away what was heard.
    pub fn forget(&mut self) {
        self.send(b'R', &[]);
        self.last.lock().unwrap().clear();
    }

    pub fn alive(&mut self) -> bool {
        matches!(self.child.try_wait(), Ok(None))
    }
}

impl Drop for Engine {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_the_engine_flag_makes_an_engine() {
        let a = |v: &[&str]| v.iter().map(|s| s.to_string()).collect::<Vec<_>>();
        assert_eq!(is_engine_launch(&a(&["pixie", FLAG, "/rt", "/model"])), Some(("/rt", "/model")));
        assert_eq!(is_engine_launch(&a(&["pixie"])), None);
        assert_eq!(is_engine_launch(&a(&["pixie", FLAG, "/rt"])), None);
        assert_eq!(is_engine_launch(&a(&["pixie", "--ask", FLAG, "/rt", "/model"])), None);
    }

    #[test]
    fn frames_are_read_whole_and_a_cut_stream_ends() {
        let mut bytes = vec![b'A', 4, 0, 0, 0, 1, 2, 3, 4, b'E', 0, 0, 0, 0, b'A', 9, 0, 0, 0, 1];
        let mut r = std::io::Cursor::new(&mut bytes);
        assert_eq!(frame(&mut r), Some((b'A', vec![1, 2, 3, 4])));
        assert_eq!(frame(&mut r), Some((b'E', vec![])));
        assert_eq!(frame(&mut r), None);
    }
}
