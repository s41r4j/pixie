//! sherpa-onnx's streaming recogniser, loaded at runtime from the downloaded library.
//!
//! The structs mirror `apps/desktop/voice/vendor/sherpa-onnx-c-api.h` (v2.0.1, Apache-2.0). They
//! hold only pointers, 32-bit ints and floats, so the layout is the same on every 64-bit system.
//! Fields Pixie doesn't set stay zero, as the header asks.

use std::ffi::{c_char, c_void, CStr, CString};
use std::path::{Path, PathBuf};

#[repr(C)]
#[derive(Default)]
struct Transducer {
    encoder: *const c_char,
    decoder: *const c_char,
    joiner: *const c_char,
}

#[repr(C)]
struct ModelConfig {
    transducer: Transducer,
    paraformer: [*const c_char; 2],
    zipformer2_ctc: *const c_char,
    tokens: *const c_char,
    num_threads: i32,
    provider: *const c_char,
    debug: i32,
    model_type: *const c_char,
    modeling_unit: *const c_char,
    bpe_vocab: *const c_char,
    tokens_buf: *const c_char,
    tokens_buf_size: i32,
    nemo_ctc: *const c_char,
    t_one_ctc: *const c_char,
}

#[repr(C)]
struct RecognizerConfig {
    sample_rate: i32,
    feature_dim: i32,
    model: ModelConfig,
    decoding_method: *const c_char,
    max_active_paths: i32,
    enable_endpoint: i32,
    rule1_min_trailing_silence: f32,
    rule2_min_trailing_silence: f32,
    rule3_min_utterance_length: f32,
    hotwords_file: *const c_char,
    hotwords_score: f32,
    ctc_graph: *const c_char,
    ctc_max_active: i32,
    rule_fsts: *const c_char,
    rule_fars: *const c_char,
    blank_penalty: f32,
    hotwords_buf: *const c_char,
    hotwords_buf_size: i32,
    hr: [*const c_char; 3],
}

#[repr(C)]
struct RecognizerResult {
    text: *const c_char,
    tokens: *const c_char,
    tokens_arr: *const *const c_char,
    timestamps: *mut f32,
    count: i32,
    json: *const c_char,
}

type Rec = *const c_void;
type Stream = *const c_void;

/// The library file inside a runtime folder (`<runtime>/lib/…`).
pub fn library(runtime: &Path) -> PathBuf {
    let name = if cfg!(target_os = "macos") {
        "libsherpa-onnx-c-api.dylib"
    } else if cfg!(windows) {
        "sherpa-onnx-c-api.dll"
    } else {
        "libsherpa-onnx-c-api.so"
    };
    // The Windows package keeps its DLLs in bin.
    let lib = runtime.join("lib").join(name);
    if cfg!(windows) && !lib.exists() {
        return runtime.join("bin").join(name);
    }
    lib
}

/// The model files Pixie downloads into a model folder.
pub const MODEL_FILES: [&str; 4] = ["encoder.int8.onnx", "decoder.int8.onnx", "joiner.int8.onnx", "tokens.txt"];

/// Are the library and every model file there?
pub fn installed(runtime: &Path, model: &Path) -> bool {
    library(runtime).exists() && MODEL_FILES.iter().all(|f| model.join(f).metadata().map(|m| m.len() > 0).unwrap_or(false))
}

/// A loaded recogniser with one stream. Not Send: it lives on the thread that made it.
pub struct Recognizer {
    lib: libloading::Library,
    rec: Rec,
    stream: Stream,
}

#[cfg(windows)]
fn load(path: &Path) -> Result<libloading::Library, String> {
    // The library needs onnxruntime from the same folder.
    use libloading::os::windows::{Library, LOAD_WITH_ALTERED_SEARCH_PATH};
    // SAFETY: loading a library runs its initialisers; this is the one Pixie downloaded.
    unsafe { Library::load_with_flags(path, LOAD_WITH_ALTERED_SEARCH_PATH) }.map(Into::into).map_err(|e| format!("{}: {e}", path.display()))
}

#[cfg(not(windows))]
fn load(path: &Path) -> Result<libloading::Library, String> {
    // SAFETY: loading a library runs its initialisers; this is the one Pixie downloaded.
    unsafe { libloading::Library::new(path) }.map_err(|e| format!("{}: {e}", path.display()))
}

fn c(path: PathBuf) -> Result<CString, String> {
    CString::new(path.to_string_lossy().as_bytes()).map_err(|_| "a path has a zero byte in it".to_string())
}

impl Recognizer {
    /// Load the library and the model (about a second). `threads` for the model: 2 keeps up
    /// with speech on anything recent without taking the machine over.
    pub fn open(runtime: &Path, model: &Path, threads: i32) -> Result<Self, String> {
        if !installed(runtime, model) {
            return Err("Pixie's speech model isn't downloaded yet (Settings → Voice).".into());
        }
        let lib = load(&library(runtime))?;
        let (encoder, decoder, joiner, tokens) = (c(model.join(MODEL_FILES[0]))?, c(model.join(MODEL_FILES[1]))?, c(model.join(MODEL_FILES[2]))?, c(model.join(MODEL_FILES[3]))?);
        let (cpu, greedy) = (CString::new("cpu").unwrap(), CString::new("greedy_search").unwrap());
        // SAFETY: all-zero is the documented starting state of this plain C struct.
        let mut cfg: RecognizerConfig = unsafe { std::mem::zeroed() };
        cfg.sample_rate = crate::SAMPLE_RATE as i32;
        cfg.feature_dim = 80;
        cfg.model.transducer = Transducer { encoder: encoder.as_ptr(), decoder: decoder.as_ptr(), joiner: joiner.as_ptr() };
        cfg.model.tokens = tokens.as_ptr();
        cfg.model.num_threads = threads.max(1);
        cfg.model.provider = cpu.as_ptr();
        cfg.decoding_method = greedy.as_ptr();
        // A pause ends a piece: 1.2 s of quiet after words, 2.4 s with none, or 30 s of talking.
        cfg.enable_endpoint = 1;
        cfg.rule1_min_trailing_silence = 2.4;
        cfg.rule2_min_trailing_silence = 1.2;
        cfg.rule3_min_utterance_length = 30.0;
        // SAFETY: the symbols and their signatures are those of the vendored header; the config
        // and its strings outlive the call.
        unsafe {
            let create: libloading::Symbol<unsafe extern "C" fn(*const RecognizerConfig) -> Rec> = lib.get(b"SherpaOnnxCreateOnlineRecognizer").map_err(|e| e.to_string())?;
            let rec = create(&cfg);
            if rec.is_null() {
                return Err(format!("couldn't load the speech model in {}", model.display()));
            }
            let new_stream: libloading::Symbol<unsafe extern "C" fn(Rec) -> Stream> = lib.get(b"SherpaOnnxCreateOnlineStream").map_err(|e| e.to_string())?;
            let stream = new_stream(rec);
            if stream.is_null() {
                return Err("couldn't start a speech stream".into());
            }
            // Fail now, not mid-sentence, if the library is missing anything used later.
            for name in ["SherpaOnnxOnlineStreamAcceptWaveform", "SherpaOnnxIsOnlineStreamReady", "SherpaOnnxDecodeOnlineStream", "SherpaOnnxGetOnlineStreamResult", "SherpaOnnxDestroyOnlineRecognizerResult", "SherpaOnnxOnlineStreamReset", "SherpaOnnxOnlineStreamInputFinished", "SherpaOnnxOnlineStreamIsEndpoint", "SherpaOnnxDestroyOnlineStream", "SherpaOnnxDestroyOnlineRecognizer"] {
                lib.get::<unsafe extern "C" fn()>(name.as_bytes()).map_err(|e| format!("{name}: {e}"))?;
            }
            Ok(Self { lib, rec, stream })
        }
    }

    fn sym<T>(&self, name: &[u8]) -> libloading::Symbol<'_, T> {
        // SAFETY: `open` checked that every name used here exists.
        unsafe { self.lib.get(name).expect("checked in open") }
    }

    /// Hear 16 kHz mono audio and decode what's ready.
    pub fn accept(&mut self, samples: &[f32]) {
        if samples.is_empty() {
            return;
        }
        let accept: libloading::Symbol<unsafe extern "C" fn(Stream, i32, *const f32, i32)> = self.sym(b"SherpaOnnxOnlineStreamAcceptWaveform");
        // SAFETY: the pointers are live for the call; the library copies the samples.
        unsafe { accept(self.stream, crate::SAMPLE_RATE as i32, samples.as_ptr(), samples.len() as i32) };
        self.decode();
    }

    fn decode(&mut self) {
        let ready: libloading::Symbol<unsafe extern "C" fn(Rec, Stream) -> i32> = self.sym(b"SherpaOnnxIsOnlineStreamReady");
        let decode: libloading::Symbol<unsafe extern "C" fn(Rec, Stream)> = self.sym(b"SherpaOnnxDecodeOnlineStream");
        // SAFETY: both handles are live until drop.
        unsafe {
            while ready(self.rec, self.stream) != 0 {
                decode(self.rec, self.stream);
            }
        }
    }

    /// The words of the piece being spoken, as the model writes them.
    pub fn text(&self) -> String {
        let get: libloading::Symbol<unsafe extern "C" fn(Rec, Stream) -> *const RecognizerResult> = self.sym(b"SherpaOnnxGetOnlineStreamResult");
        let free: libloading::Symbol<unsafe extern "C" fn(*const RecognizerResult)> = self.sym(b"SherpaOnnxDestroyOnlineRecognizerResult");
        // SAFETY: the result is owned by us until `free`; its text is a C string or null.
        unsafe {
            let r = get(self.rec, self.stream);
            if r.is_null() {
                return String::new();
            }
            let text = if (*r).text.is_null() { String::new() } else { CStr::from_ptr((*r).text).to_string_lossy().into_owned() };
            free(r);
            text
        }
    }

    /// Did they pause long enough to end this piece?
    pub fn at_pause(&self) -> bool {
        let f: libloading::Symbol<unsafe extern "C" fn(Rec, Stream) -> i32> = self.sym(b"SherpaOnnxOnlineStreamIsEndpoint");
        // SAFETY: both handles are live until drop.
        unsafe { f(self.rec, self.stream) != 0 }
    }

    /// Start the next piece (after a pause, or for a new dictation).
    pub fn reset(&mut self) {
        let f: libloading::Symbol<unsafe extern "C" fn(Rec, Stream)> = self.sym(b"SherpaOnnxOnlineStreamReset");
        // SAFETY: both handles are live until drop.
        unsafe { f(self.rec, self.stream) };
    }

    /// They stopped: push the last words out. Half a second of quiet lets the model finish the
    /// final word (it looks ahead), then the stream starts over for the next dictation.
    pub fn finish(&mut self) -> String {
        self.accept(&vec![0.0; crate::SAMPLE_RATE as usize / 2]);
        let text = self.text();
        self.reset();
        text
    }
}

impl Drop for Recognizer {
    fn drop(&mut self) {
        let drop_stream: libloading::Symbol<unsafe extern "C" fn(Stream)> = self.sym(b"SherpaOnnxDestroyOnlineStream");
        let drop_rec: libloading::Symbol<unsafe extern "C" fn(Rec)> = self.sym(b"SherpaOnnxDestroyOnlineRecognizer");
        // SAFETY: made in `open`, freed once here.
        unsafe {
            drop_stream(self.stream);
            drop_rec(self.rec);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The header's struct sizes on 64-bit, as the C compiler reports them for the vendored
    /// header (sizeof: 136, 272, 48). A field added or dropped here would shift what follows.
    #[test]
    fn the_structs_are_the_size_the_library_expects() {
        assert_eq!(std::mem::size_of::<ModelConfig>(), 136);
        assert_eq!(std::mem::size_of::<RecognizerConfig>(), 272);
        assert_eq!(std::mem::size_of::<RecognizerResult>(), 48);
        assert_eq!(std::mem::offset_of!(ModelConfig, provider), 64);
        assert_eq!(std::mem::offset_of!(RecognizerConfig, decoding_method), 144);
        assert_eq!(std::mem::offset_of!(RecognizerConfig, rule_fsts), 208);
        assert_eq!(std::mem::offset_of!(RecognizerConfig, hr), 248);
    }

    #[test]
    fn nothing_is_installed_in_an_empty_folder() {
        let d = tempfile::tempdir().unwrap();
        assert!(!installed(d.path(), d.path()));
        assert!(Recognizer::open(d.path(), d.path(), 1).is_err());
    }
}
