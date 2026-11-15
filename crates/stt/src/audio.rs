//! Microphone audio to what the model wants: 16 kHz, one channel, f32.

use crate::SAMPLE_RATE;

/// Mixes channels down and resamples a stream, chunk by chunk (linear interpolation: speech
/// recognition features are far coarser than what this loses).
pub struct Resampler {
    channels: usize,
    step: f64,
    /// Position of the next output sample, in input frames, relative to the next chunk's start.
    pos: f64,
    /// The last frame of the previous chunk.
    prev: f32,
}

impl Resampler {
    pub fn new(rate: u32, channels: u16) -> Self {
        Self { channels: channels.max(1) as usize, step: rate.max(1) as f64 / SAMPLE_RATE as f64, pos: 0.0, prev: 0.0 }
    }

    /// Interleaved samples in, 16 kHz mono appended to `out`.
    pub fn push(&mut self, input: &[f32], out: &mut Vec<f32>) {
        let frames = input.len() / self.channels;
        if frames == 0 {
            return;
        }
        let mono = |i: usize| -> f32 {
            let f = &input[i * self.channels..(i + 1) * self.channels];
            f.iter().sum::<f32>() / self.channels as f32
        };
        // Frame -1 is the previous chunk's last frame.
        while self.pos < frames as f64 - 1.0 + 1e-9 {
            let i = self.pos.floor();
            let frac = (self.pos - i) as f32;
            let a = if i < 0.0 { self.prev } else { mono(i as usize) };
            let b = mono(((i + 1.0) as usize).min(frames - 1));
            out.push(a + (b - a) * frac);
            self.pos += self.step;
        }
        self.prev = mono(frames - 1);
        self.pos -= frames as f64;
    }
}

/// Loudness of a chunk for the waveform in the notch: about -60…-10 dBFS onto 0…1.
pub fn level(samples: &[f32]) -> f32 {
    if samples.is_empty() {
        return 0.0;
    }
    let rms = (samples.iter().map(|s| s * s).sum::<f32>() / samples.len() as f32).sqrt();
    let db = 20.0 * rms.max(1e-7).log10();
    ((db + 60.0) / 50.0).clamp(0.0, 1.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stereo_48k_becomes_mono_16k_of_the_same_length() {
        let mut r = Resampler::new(48_000, 2);
        let mut out = Vec::new();
        // One second in uneven chunks, a 440 Hz tone on both channels.
        let tone: Vec<f32> = (0..48_000).flat_map(|i| { let s = (i as f32 * 440.0 * std::f32::consts::TAU / 48_000.0).sin(); [s, s] }).collect();
        for chunk in tone.chunks(2 * 1023) {
            r.push(chunk, &mut out);
        }
        assert!((out.len() as i64 - 16_000).abs() <= 2, "{}", out.len());
        // Still the same tone: it crosses zero 880 times a second.
        let crossings = out.windows(2).filter(|w| (w[0] < 0.0) != (w[1] < 0.0)).count();
        assert!((crossings as i64 - 880).abs() <= 4, "{crossings}");
    }

    #[test]
    fn sixteen_k_mono_passes_through() {
        let mut r = Resampler::new(16_000, 1);
        let mut out = Vec::new();
        let input: Vec<f32> = (0..1600).map(|i| i as f32 / 1600.0).collect();
        r.push(&input[..700], &mut out);
        r.push(&input[700..], &mut out);
        assert!((out.len() as i64 - 1600).abs() <= 1);
        assert!(out.iter().zip(&input).all(|(a, b)| (a - b).abs() < 1e-3));
    }

    #[test]
    fn silence_is_zero_and_speech_is_not() {
        assert_eq!(level(&[0.0; 320]), 0.0);
        assert!(level(&[0.2; 320]) > 0.8);
    }
}
