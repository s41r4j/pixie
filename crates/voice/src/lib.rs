//! Voice: the UI sound bus now (M1); speech traits in M2.
//!
//! TODO(M2): `trait SpeechToText` (streaming), `trait TextToSpeech` (streaming), VAD, AEC —
//! vendors (Deepgram / AssemblyAI / Whisper; ElevenLabs / Cartesia / `say`) are adapters
//! behind these traits so the UI never couples to one vendor.

pub mod bus;

pub use bus::{AudioBus, AudioOutput, Gains, PlaybackId};
#[cfg(feature = "rodio")]
pub use bus::RodioOutput;
