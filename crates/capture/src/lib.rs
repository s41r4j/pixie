//! Screen capture + annotation model.
//!
//! Capture itself runs in the Swift helper (`apps/desktop/helper/main.swift`: ScreenCaptureKit
//! screenshot of the display under the pointer, our windows excluded, Vision OCR, and the
//! frontmost window's Accessibility controls). The desktop crate reads it as
//! `screen::Capture`. Annotations (`highlight`, `draw`) are rendered by the overlay window.
//! This crate is kept for a future Rust-side capture model.
