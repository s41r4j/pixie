//! Speech playback: Kokoro WAVs (`play`) and system-voice sentences (`speak`), one after
//! another, with `speak_start` / `speak_word` / `speak_end` events like the Mac helper.

use super::emit;
use serde_json::{json, Value};
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::mpsc;
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, Instant};

enum Item {
    Wav { id: String, path: PathBuf, text: String },
    Say { id: String, text: String, voice: String, rate: f64 },
}

static QUEUE: OnceLock<Mutex<mpsc::Sender<(u64, Item)>>> = OnceLock::new();
/// Bumped by stop(): everything queued before it is dropped.
static GEN: AtomicU64 = AtomicU64::new(0);
static VOICE: Mutex<(String, f64)> = Mutex::new((String::new(), 0.52));

pub fn start() {
    let (tx, rx) = mpsc::channel::<(u64, Item)>();
    let _ = QUEUE.set(Mutex::new(tx));
    std::thread::Builder::new()
        .name("pixie-speech".into())
        .spawn(move || {
            #[cfg(windows)]
            super::win::init_thread();
            let out = rodio::OutputStream::try_default();
            let handle = match &out {
                Ok((_, h)) => Some(h.clone()),
                Err(e) => {
                    log::warn!("speech output: {e}");
                    None
                }
            };
            let mut n = 0u64;
            for (gen, item) in rx {
                let (id, path, text, temp) = match item {
                    Item::Wav { id, path, text } => (id, path, text, true),
                    Item::Say { id, .. } if GEN.load(Ordering::SeqCst) != gen => {
                        emit("speak_end", json!({ "id": id, "cancelled": true }));
                        continue;
                    }
                    Item::Say { id, text, voice, rate } => {
                        n += 1;
                        let path = std::env::temp_dir().join(format!("pixie-say-{}-{n}.wav", std::process::id()));
                        if let Err(e) = super::os::synth(&text, &voice, rate, &path) {
                            log::warn!("speak: {e}");
                            emit("speak_end", json!({ "id": id, "cancelled": true }));
                            continue;
                        }
                        (id, path, text, true)
                    }
                };
                if GEN.load(Ordering::SeqCst) != gen {
                    emit("speak_end", json!({ "id": id, "cancelled": true }));
                    let _ = std::fs::remove_file(&path);
                    continue;
                }
                let done = handle.as_ref().map(|h| play_one(h, &id, &path, &text, gen)).unwrap_or(false);
                emit("speak_end", json!({ "id": id, "cancelled": !done }));
                if temp {
                    let _ = std::fs::remove_file(&path);
                }
            }
        })
        .expect("speech thread");
}

/// Play to the end (true) or until stop() (false).
fn play_one(h: &rodio::OutputStreamHandle, id: &str, path: &PathBuf, text: &str, gen: u64) -> bool {
    use rodio::Source;
    let Ok(file) = std::fs::File::open(path) else { return false };
    let Ok(dec) = rodio::Decoder::new(std::io::BufReader::new(file)) else { return false };
    let dur = dec.total_duration().map(|d| d.as_secs_f64()).unwrap_or(text.len() as f64 * 0.06);
    let Ok(sink) = rodio::Sink::try_new(h) else { return false };
    sink.append(dec);
    emit("speak_start", json!({ "id": id }));
    let words = timings(text, dur);
    let t0 = Instant::now();
    let mut next = 0;
    while !sink.empty() {
        if GEN.load(Ordering::SeqCst) != gen {
            sink.stop();
            return false;
        }
        let now = t0.elapsed().as_secs_f64();
        while next < words.len() && words[next].0 <= now {
            let (_, at, len) = words[next];
            emit("speak_word", json!({ "id": id, "at": at, "len": len }));
            next += 1;
        }
        std::thread::sleep(Duration::from_millis(15));
    }
    true
}

/// (time, utf-16 offset, utf-16 length) per word; time shared out by word length.
fn timings(text: &str, dur: f64) -> Vec<(f64, usize, usize)> {
    let mut words = vec![];
    let mut at = 0usize;
    let mut start: Option<usize> = None;
    for ch in text.chars() {
        let w = ch.len_utf16();
        if ch.is_alphanumeric() || ch == '\'' {
            start.get_or_insert(at);
        } else if let Some(s) = start.take() {
            words.push((s, at - s));
        }
        at += w;
    }
    if let Some(s) = start {
        words.push((s, at - s));
    }
    let total: usize = words.iter().map(|w| w.1 + 1).sum::<usize>().max(1);
    let usable = (dur - 0.15).max(0.2);
    let mut t = 0.06;
    words
        .into_iter()
        .map(|(s, l)| {
            let here = t;
            t += usable * (l + 1) as f64 / total as f64;
            (here, s, l)
        })
        .collect()
}

fn push(item: Item) {
    if let Some(q) = QUEUE.get() {
        let _ = q.lock().unwrap().send((GEN.load(Ordering::SeqCst), item));
    }
}

pub fn speak(args: &Value) -> Value {
    let mut v = VOICE.lock().unwrap();
    if let Some(id) = args.get("voice").and_then(Value::as_str) {
        v.0 = id.to_string();
    }
    if let Some(r) = args.get("rate").and_then(Value::as_f64) {
        v.1 = r;
    }
    let s = |k: &str| args.get(k).and_then(Value::as_str).unwrap_or("").to_string();
    push(Item::Say { id: s("utterance"), text: s("text"), voice: v.0.clone(), rate: v.1 });
    json!({})
}

pub fn play(args: &Value) -> Value {
    let s = |k: &str| args.get(k).and_then(Value::as_str).unwrap_or("").to_string();
    push(Item::Wav { id: s("utterance"), path: PathBuf::from(s("path")), text: s("text") });
    json!({})
}

pub fn stop() {
    GEN.fetch_add(1, Ordering::SeqCst);
}

#[cfg(test)]
mod tests {
    #[test]
    fn word_timings_follow_the_text() {
        let w = super::timings("Hi there, Pixie.", 2.0);
        assert_eq!(w.iter().map(|x| (x.1, x.2)).collect::<Vec<_>>(), vec![(0, 2), (3, 5), (10, 5)]);
        assert!(w[0].0 < w[1].0 && w[1].0 < w[2].0);
    }
}
