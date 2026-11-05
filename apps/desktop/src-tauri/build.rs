//! Builds the Swift helper (`../helper/main.swift`) into `binaries/openclicky-helper-<triple>`,
//! which Tauri bundles as a sidecar next to the main binary.

use std::path::{Path, PathBuf};
use std::process::Command;

fn newer(src: &[&Path], out: &Path) -> bool {
    let Ok(out_m) = out.metadata().and_then(|m| m.modified()) else { return true };
    src.iter().any(|s| s.metadata().and_then(|m| m.modified()).map(|m| m > out_m).unwrap_or(true))
}

fn build_helper() {
    let manifest = PathBuf::from(std::env::var("CARGO_MANIFEST_DIR").unwrap());
    let src = manifest.join("../helper/main.swift");
    let plist = manifest.join("../helper/Info.plist");
    println!("cargo:rerun-if-changed={}", src.display());
    println!("cargo:rerun-if-changed={}", plist.display());
    let target = std::env::var("TARGET").unwrap();
    let bin_dir = manifest.join("binaries");
    std::fs::create_dir_all(&bin_dir).unwrap();
    let exe = if target.contains("windows") { ".exe" } else { "" };
    let out = bin_dir.join(format!("openclicky-helper-{target}{exe}"));

    if !target.contains("apple-darwin") {
        // Non-mac builds get an empty placeholder so bundling config stays valid.
        if !out.exists() {
            std::fs::write(&out, b"").unwrap();
        }
        return;
    }
    let arch = if target.starts_with("aarch64") { "arm64" } else { "x86_64" };
    if newer(&[&src, &plist], &out) {
        let status = Command::new("xcrun")
            .args(["swiftc", "-O", "-swift-version", "5", "-target"])
            .arg(format!("{arch}-apple-macos14"))
            .arg(&src)
            .arg("-o")
            .arg(&out)
            .args(["-Xlinker", "-sectcreate", "-Xlinker", "__TEXT", "-Xlinker", "__info_plist", "-Xlinker"])
            .arg(&plist)
            .status()
            .expect("run xcrun swiftc (install Xcode command line tools)");
        assert!(status.success(), "building the Swift helper failed");
    }
    // Universal bundles look for `-universal-apple-darwin`; make it once both arches exist.
    let arm = bin_dir.join("openclicky-helper-aarch64-apple-darwin");
    let x86 = bin_dir.join("openclicky-helper-x86_64-apple-darwin");
    let uni = bin_dir.join("openclicky-helper-universal-apple-darwin");
    if arm.exists() && x86.exists() && (newer(&[&arm, &x86], &uni)) {
        let _ = Command::new("lipo").arg("-create").arg(&arm).arg(&x86).arg("-output").arg(&uni).status();
    }
}

/// The local neural voice server (`../voice/openclicky-voice.c`), loads sherpa-onnx at runtime.
fn build_voice() {
    let manifest = PathBuf::from(std::env::var("CARGO_MANIFEST_DIR").unwrap());
    let src = manifest.join("../voice/openclicky-voice.c");
    let header = manifest.join("../voice/vendor/sherpa-onnx-c-api.h");
    println!("cargo:rerun-if-changed={}", src.display());
    let target = std::env::var("TARGET").unwrap();
    let bin_dir = manifest.join("binaries");
    let exe = if target.contains("windows") { ".exe" } else { "" };
    let out = bin_dir.join(format!("openclicky-voice-{target}{exe}"));
    if !target.contains("apple-darwin") {
        std::fs::create_dir_all(&bin_dir).unwrap();
        if !out.exists() {
            std::fs::write(&out, b"").unwrap();
        }
        return;
    }
    let arch = if target.starts_with("aarch64") { "arm64" } else { "x86_64" };
    if newer(&[&src, &header], &out) {
        let status = Command::new("xcrun")
            .args(["clang", "-O2", "-arch", arch, "-mmacosx-version-min=14.0", "-o"])
            .arg(&out)
            .arg(&src)
            .status()
            .expect("run xcrun clang");
        assert!(status.success(), "building openclicky-voice failed");
    }
    let arm = bin_dir.join("openclicky-voice-aarch64-apple-darwin");
    let x86 = bin_dir.join("openclicky-voice-x86_64-apple-darwin");
    let uni = bin_dir.join("openclicky-voice-universal-apple-darwin");
    if arm.exists() && x86.exists() && newer(&[&arm, &x86], &uni) {
        let _ = Command::new("lipo").arg("-create").arg(&arm).arg(&x86).arg("-output").arg(&uni).status();
    }
}

fn main() {
    build_helper();
    build_voice();
    tauri_build::build()
}
