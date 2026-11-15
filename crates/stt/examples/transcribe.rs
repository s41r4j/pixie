//! Feed a 16-bit WAV to the engine at speaking pace and print the words as they arrive:
//!   cargo run -p pixie-stt --example transcribe -- <runtime> <model> file.wav
//! (`<runtime>` and `<model>` as Pixie downloads them into `<app data>/voices/`.)
use pixie_stt::audio::Resampler;
use pixie_stt::engine::{self, Engine};
use std::time::{Duration, Instant};

fn main() {
    let args: Vec<String> = std::env::args().collect();
    if let Some((runtime, model)) = engine::is_engine_launch(&args) {
        std::process::exit(engine::run(runtime, model));
    }
    let [_, runtime, model, wav] = &args[..] else {
        eprintln!("usage: transcribe <runtime> <model> file.wav");
        std::process::exit(2);
    };
    let bytes = std::fs::read(wav).expect("read wav");
    let channels = u16::from_le_bytes([bytes[22], bytes[23]]);
    let rate = u32::from_le_bytes([bytes[24], bytes[25], bytes[26], bytes[27]]);
    let data = bytes.windows(4).position(|w| w == b"data").expect("data chunk") + 8;
    let samples: Vec<f32> = bytes[data..].as_chunks::<2>().0.iter().map(|b| i16::from_le_bytes(*b) as f32 / 32768.0).collect();
    let seconds = samples.len() as f32 / (rate as f32 * channels as f32);
    let t0 = Instant::now();
    let mut cmd = std::process::Command::new(std::env::current_exe().unwrap());
    cmd.args([engine::FLAG, runtime, model]);
    let begun = std::sync::Arc::new(std::sync::Mutex::new(Instant::now()));
    let b2 = begun.clone();
    let mut eng = Engine::start(cmd, move |t| println!("{:6.2}s  {t}", b2.lock().unwrap().elapsed().as_secs_f32())).expect("engine");
    println!("model loaded in {} ms; audio is {seconds:.1} s", t0.elapsed().as_millis());
    *begun.lock().unwrap() = Instant::now();
    let mut rs = Resampler::new(rate, channels);
    // 100 ms at a time, as a microphone would deliver it.
    let chunk = (rate as usize / 10) * channels as usize;
    let start = Instant::now();
    for (i, c) in samples.chunks(chunk).enumerate() {
        let mut out = Vec::new();
        rs.push(c, &mut out);
        eng.audio(&out);
        let due = Duration::from_millis(100 * (i as u64 + 1));
        if let Some(wait) = due.checked_sub(start.elapsed()) {
            std::thread::sleep(wait);
        }
    }
    let t = Instant::now();
    let text = eng.finish();
    println!("final after {} ms: {text}", t.elapsed().as_millis());
}
