//! Pixie's own speech to text, the same on macOS, Windows and Linux.
//!
//! A streaming recogniser (a Zipformer transducer through sherpa-onnx's C API) hears audio as it
//! arrives and gives the words so far, a few times a second. The library and the model are
//! downloaded on request (Settings → Voice), never shipped, and loaded at runtime.
//!
//! - [`ffi`]: the library, loaded with dlopen / LoadLibrary.
//! - [`engine`]: the recogniser as a child process (`pixie --stt-engine …`), so a bad library or
//!   model can't take Pixie down with it. Audio in on stdin, words out on stdout.
//! - [`text`]: turning what the model says (capitals, no punctuation, in pieces) into running
//!   text, and working out the fewest keystrokes that make a text field match it.
//! - [`audio`]: any microphone format to 16 kHz mono.

pub mod audio;
pub mod engine;
pub mod ffi;
pub mod text;

/// What the model hears at.
pub const SAMPLE_RATE: u32 = 16_000;
