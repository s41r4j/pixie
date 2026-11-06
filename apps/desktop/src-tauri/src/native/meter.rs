//! Mic level for the waveform while listening (the recognizer has its own capture).

use super::emit;
use serde_json::json;
use std::sync::mpsc;
use std::time::{Duration, Instant};

/// Meter the default input until the returned sender is dropped.
pub fn start() -> Option<mpsc::Sender<()>> {
    let (tx, rx) = mpsc::channel::<()>();
    std::thread::Builder::new()
        .name("pixie-meter".into())
        .spawn(move || {
            if let Err(e) = run(rx) {
                log::debug!("meter: {e}");
            }
        })
        .ok()?;
    Some(tx)
}

fn level(samples: impl Iterator<Item = f32>, last: &mut Instant) {
    let (mut sum, mut n) = (0f32, 0usize);
    for x in samples {
        sum += x * x;
        n += 1;
    }
    if n == 0 || last.elapsed() < Duration::from_millis(33) {
        return;
    }
    *last = Instant::now();
    let rms = (sum / n as f32).sqrt();
    // -60..-10 dBFS onto 0..1, like the Mac.
    let db = 20.0 * rms.max(1e-7).log10();
    emit("level", json!({ "level": ((db + 60.0) / 50.0).clamp(0.0, 1.0) as f64 }));
}

fn run(stop: mpsc::Receiver<()>) -> Result<(), String> {
    use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
    let host = cpal::default_host();
    let dev = host.default_input_device().ok_or("no microphone")?;
    let cfg = dev.default_input_config().map_err(|e| e.to_string())?;
    let err = |e| log::debug!("meter stream: {e}");
    let stream = match cfg.sample_format() {
        cpal::SampleFormat::F32 => {
            let mut last = Instant::now();
            dev.build_input_stream(&cfg.into(), move |d: &[f32], _: &_| level(d.iter().copied(), &mut last), err, None)
        }
        cpal::SampleFormat::I16 => {
            let mut last = Instant::now();
            dev.build_input_stream(&cfg.into(), move |d: &[i16], _: &_| level(d.iter().map(|x| *x as f32 / 32768.0), &mut last), err, None)
        }
        f => return Err(format!("sample format {f:?}")),
    }
    .map_err(|e| e.to_string())?;
    stream.play().map_err(|e| e.to_string())?;
    // Dropped sender → Err(Disconnected) → stop.
    let _ = stop.recv();
    emit("level", json!({ "level": 0.0 }));
    Ok(())
}
