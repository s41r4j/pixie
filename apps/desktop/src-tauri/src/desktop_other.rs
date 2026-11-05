//! The same API as `macos.rs` for Windows (and Linux builds), on top of Tauri's own window
//! calls. Geometry is in logical points with the origin at the top-left of the primary display,
//! like on the Mac. There is no notch: the panel sits at the top centre of the screen.

use serde::Serialize;
use std::sync::OnceLock;
use tauri::{AppHandle, LogicalPosition, LogicalSize, Runtime, WebviewWindow};

static APP: OnceLock<AppHandle> = OnceLock::new();

/// Remember the app so screen queries work without a handle.
pub fn init(app: &AppHandle) {
    let _ = APP.set(app.clone());
}

#[derive(Debug, Clone, Copy, Default, Serialize, PartialEq)]
pub struct Rect {
    pub x: f64,
    pub y: f64,
    pub w: f64,
    pub h: f64,
}

impl Rect {
    pub fn contains(&self, x: f64, y: f64) -> bool {
        x >= self.x && x <= self.x + self.w && y >= self.y && y <= self.y + self.h
    }
}

#[derive(Debug, Clone, Copy, Default, Serialize)]
pub struct ScreenInfo {
    pub frame: Rect,
    pub top_inset: f64,
    pub notch_width: Option<f64>,
    pub scale: f64,
}

pub const NS_STATUS_WINDOW_LEVEL: isize = 25;
pub const OVERLAY_LEVEL: isize = 26;
pub const PANEL_LEVEL: isize = 27;

// The hotkey code speaks CGEventFlags; Windows modifiers map onto the same bits
// (Ctrl → control, Alt → option, Win → command).
const FLAG_SHIFT: u64 = 0x0002_0000;
const FLAG_CONTROL: u64 = 0x0004_0000;
const FLAG_OPTION: u64 = 0x0008_0000;
const FLAG_COMMAND: u64 = 0x0010_0000;

#[cfg(windows)]
mod win {
    use windows_sys::Win32::Foundation::POINT;
    use windows_sys::Win32::UI::Input::KeyboardAndMouse::GetAsyncKeyState;
    use windows_sys::Win32::UI::WindowsAndMessaging::GetCursorPos;

    pub fn down(vk: i32) -> bool {
        unsafe { (GetAsyncKeyState(vk) as u16 & 0x8000) != 0 }
    }

    pub fn cursor() -> (f64, f64) {
        let mut p = POINT { x: 0, y: 0 };
        unsafe { GetCursorPos(&mut p) };
        (p.x as f64, p.y as f64)
    }
}

pub fn modifier_flags() -> u64 {
    #[cfg(windows)]
    {
        let mut f = 0;
        if win::down(0x10) {
            f |= FLAG_SHIFT;
        }
        if win::down(0x11) {
            f |= FLAG_CONTROL;
        }
        if win::down(0x12) {
            f |= FLAG_OPTION;
        }
        if win::down(0x5B) || win::down(0x5C) {
            f |= FLAG_COMMAND;
        }
        f
    }
    #[cfg(not(windows))]
    {
        let _ = (FLAG_SHIFT, FLAG_CONTROL, FLAG_OPTION, FLAG_COMMAND);
        0
    }
}

/// Mac virtual key codes in, so callers stay the same. Only Escape is used.
pub fn key_down(keycode: u16) -> bool {
    #[cfg(windows)]
    {
        match keycode {
            53 => win::down(0x1B),
            _ => false,
        }
    }
    #[cfg(not(windows))]
    {
        let _ = keycode;
        false
    }
}

pub fn mouse_down() -> bool {
    #[cfg(windows)]
    {
        win::down(0x01)
    }
    #[cfg(not(windows))]
    {
        false
    }
}

/// Cursor in logical points.
pub fn cursor() -> (f64, f64) {
    #[cfg(windows)]
    {
        let (px, py) = win::cursor();
        // Physical → logical with the scale of the monitor under the cursor.
        let scale = monitors()
            .into_iter()
            .find(|(pos, size, _)| px >= pos.0 && px < pos.0 + size.0 && py >= pos.1 && py < pos.1 + size.1)
            .map(|m| m.2)
            .unwrap_or(1.0);
        (px / scale, py / scale)
    }
    #[cfg(not(windows))]
    {
        APP.get()
            .and_then(|a| {
                let scale = a.primary_monitor().ok().flatten().map(|m| m.scale_factor()).unwrap_or(1.0);
                a.cursor_position().ok().map(|p| (p.x / scale, p.y / scale))
            })
            .unwrap_or((0.0, 0.0))
    }
}

/// (physical position, physical size, scale) for each monitor, primary first.
fn monitors() -> Vec<((f64, f64), (f64, f64), f64)> {
    let Some(app) = APP.get() else { return vec![] };
    let primary = app.primary_monitor().ok().flatten().map(|m| *m.position());
    let mut all: Vec<_> = app
        .available_monitors()
        .unwrap_or_default()
        .into_iter()
        .map(|m| {
            let p = m.position();
            let s = m.size();
            ((p.x as f64, p.y as f64), (s.width as f64, s.height as f64), m.scale_factor())
        })
        .collect();
    if let Some(pp) = primary {
        all.sort_by_key(|m| (m.0 .0 as i32 != pp.x || m.0 .1 as i32 != pp.y) as u8);
    }
    all
}

pub fn screens() -> Vec<ScreenInfo> {
    monitors()
        .into_iter()
        .map(|(pos, size, scale)| ScreenInfo {
            frame: Rect { x: pos.0 / scale, y: pos.1 / scale, w: size.0 / scale, h: size.1 / scale },
            top_inset: 8.0,
            notch_width: None,
            scale,
        })
        .collect()
}

pub fn screen_at_cursor() -> ScreenInfo {
    let (x, y) = cursor();
    screen_containing(x, y)
}

pub fn screen_containing(x: f64, y: f64) -> ScreenInfo {
    let all = screens();
    all.iter().copied().find(|s| s.frame.contains(x, y)).or(all.first().copied()).unwrap_or(ScreenInfo {
        frame: Rect { x: 0.0, y: 0.0, w: 1440.0, h: 900.0 },
        top_inset: 8.0,
        notch_width: None,
        scale: 1.0,
    })
}

pub fn set_frame<R: Runtime>(win: &WebviewWindow<R>, r: Rect) {
    let _ = win.set_size(LogicalSize::new(r.w, r.h));
    let _ = win.set_position(LogicalPosition::new(r.x, r.y));
}

pub fn frame<R: Runtime>(win: &WebviewWindow<R>) -> Option<Rect> {
    let scale = win.scale_factor().ok()?;
    let p = win.outer_position().ok()?.to_logical::<f64>(scale);
    let s = win.outer_size().ok()?.to_logical::<f64>(scale);
    Some(Rect { x: p.x, y: p.y, w: s.width, h: s.height })
}

pub fn set_ignores_mouse<R: Runtime>(win: &WebviewWindow<R>, ignore: bool) {
    let _ = win.set_ignore_cursor_events(ignore);
}

/// Always on top, out of the taskbar, and hidden from screen capture.
pub fn configure_overlay<R: Runtime>(win: &WebviewWindow<R>, _level: isize) {
    let _ = win.set_always_on_top(true);
    let _ = win.set_skip_taskbar(true);
    let _ = win.set_content_protected(true);
    let _ = win.set_shadow(false);
}

pub fn order_front_without_focus<R: Runtime>(win: &WebviewWindow<R>) {
    let _ = win.show();
}

pub fn order_out<R: Runtime>(win: &WebviewWindow<R>) {
    let _ = win.hide();
}

pub fn make_key<R: Runtime>(win: &WebviewWindow<R>) {
    let _ = win.show();
    let _ = win.set_focus();
}

pub fn deactivate() {}
