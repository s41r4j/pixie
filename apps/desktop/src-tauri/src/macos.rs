//! Thin AppKit / CoreGraphics bindings. Every NSWindow call must run on the main thread.
//!
//! Geometry convention: [`Rect`] is in CoreGraphics global points (origin top-left of the
//! primary display), the same space the helper reports screenshots, OCR and AX frames in.

use objc2::msg_send;
use objc2::runtime::{AnyClass, AnyObject};
use objc2_foundation::{NSEdgeInsets, NSPoint, NSRect, NSSize};
use serde::Serialize;
use tauri::{AppHandle, Runtime, WebviewWindow};

/// Nothing to set up on the Mac (the Windows twin keeps an app handle).
pub fn init(_app: &AppHandle) {}

#[repr(C)]
#[derive(Clone, Copy)]
struct CGPoint {
    x: f64,
    y: f64,
}

#[link(name = "CoreGraphics", kind = "framework")]
extern "C" {
    fn CGEventSourceFlagsState(state_id: i32) -> u64;
    fn CGEventSourceKeyState(state_id: i32, key: u16) -> bool;
    fn CGEventCreate(source: *const std::ffi::c_void) -> *mut std::ffi::c_void;
    fn CGEventGetLocation(event: *mut std::ffi::c_void) -> CGPoint;
}

#[link(name = "CoreFoundation", kind = "framework")]
extern "C" {
    fn CFRelease(cf: *mut std::ffi::c_void);
}

/// kCGEventSourceStateCombinedSessionState
const COMBINED_SESSION: i32 = 0;

/// Current modifier flags (CGEventFlags). Needs no permission.
pub fn modifier_flags() -> u64 {
    unsafe { CGEventSourceFlagsState(COMBINED_SESSION) }
}

/// Whether a virtual key is down right now.
pub fn key_down(keycode: u16) -> bool {
    unsafe { CGEventSourceKeyState(COMBINED_SESSION, keycode) }
}

/// Mouse location in CG global points. Safe from any thread.
pub fn cursor() -> (f64, f64) {
    unsafe {
        let ev = CGEventCreate(std::ptr::null());
        if ev.is_null() {
            return (0.0, 0.0);
        }
        let p = CGEventGetLocation(ev);
        CFRelease(ev);
        (p.x, p.y)
    }
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
    /// Height of the menu bar / notch strip at the top.
    pub top_inset: f64,
    /// Width of the camera housing, when the screen has one.
    pub notch_width: Option<f64>,
    pub scale: f64,
}

pub const NS_STATUS_WINDOW_LEVEL: isize = 25;
pub const OVERLAY_LEVEL: isize = 26;
pub const PANEL_LEVEL: isize = 27;
const CAN_JOIN_ALL_SPACES: usize = 1 << 0;
const STATIONARY: usize = 1 << 4;
const IGNORES_CYCLE: usize = 1 << 6;
const FULL_SCREEN_AUXILIARY: usize = 1 << 8;
const MOVE_TO_ACTIVE_SPACE: usize = 1 << 1;
const NS_FLOATING_WINDOW_LEVEL: isize = 3;
const NS_WINDOW_SHARING_NONE: usize = 0;

fn ns_window<R: Runtime>(win: &WebviewWindow<R>) -> Option<*mut AnyObject> {
    win.ns_window().ok().map(|p| p as *mut AnyObject).filter(|p| !p.is_null())
}

fn ns_screens() -> Vec<*mut AnyObject> {
    unsafe {
        let Some(cls) = AnyClass::get(c"NSScreen") else { return vec![] };
        let arr: *mut AnyObject = msg_send![cls, screens];
        if arr.is_null() {
            return vec![];
        }
        let n: usize = msg_send![arr, count];
        (0..n).map(|i| msg_send![arr, objectAtIndex: i]).collect()
    }
}

/// All screens, in CG coordinates. Main thread.
pub fn screens() -> Vec<ScreenInfo> {
    let list = ns_screens();
    let Some(primary) = list.first() else { return vec![] };
    let primary_h = unsafe {
        let f: NSRect = msg_send![*primary, frame];
        f.size.height
    };
    list.iter()
        .map(|s| unsafe {
            let f: NSRect = msg_send![*s, frame];
            let vis: NSRect = msg_send![*s, visibleFrame];
            let insets: NSEdgeInsets = msg_send![*s, safeAreaInsets];
            let left: NSRect = msg_send![*s, auxiliaryTopLeftArea];
            let right: NSRect = msg_send![*s, auxiliaryTopRightArea];
            let scale: f64 = msg_send![*s, backingScaleFactor];
            let menubar = (f.origin.y + f.size.height) - (vis.origin.y + vis.size.height);
            let notch = (insets.top > 0.0 && left.size.width > 0.0 && right.size.width > 0.0)
                .then_some(f.size.width - left.size.width - right.size.width)
                .filter(|w| *w > 40.0);
            ScreenInfo {
                frame: Rect {
                    x: f.origin.x,
                    y: primary_h - (f.origin.y + f.size.height),
                    w: f.size.width,
                    h: f.size.height,
                },
                top_inset: insets.top.max(menubar).max(24.0),
                notch_width: notch,
                scale,
            }
        })
        .collect()
}

/// The screen under the mouse (falls back to the primary). Main thread.
pub fn screen_at_cursor() -> ScreenInfo {
    let (x, y) = cursor();
    let all = screens();
    all.iter().copied().find(|s| s.frame.contains(x, y)).or(all.first().copied()).unwrap_or_default()
}

pub fn screen_containing(x: f64, y: f64) -> ScreenInfo {
    let all = screens();
    all.iter().copied().find(|s| s.frame.contains(x, y)).or(all.first().copied()).unwrap_or_default()
}

fn primary_height() -> f64 {
    ns_screens()
        .first()
        .map(|s| unsafe {
            let f: NSRect = msg_send![*s, frame];
            f.size.height
        })
        .unwrap_or(0.0)
}

/// Place a window at a CG-space rectangle (may cover the menu bar). Main thread.
pub fn set_frame<R: Runtime>(win: &WebviewWindow<R>, r: Rect) {
    let Some(ns) = ns_window(win) else { return };
    let ph = primary_height();
    let frame = NSRect::new(NSPoint::new(r.x, ph - (r.y + r.h)), NSSize::new(r.w, r.h));
    unsafe {
        let _: () = msg_send![ns, setFrame: frame, display: true];
    }
}

/// A window's actual frame in CG space (after any adjustment AppKit made). Main thread.
pub fn frame<R: Runtime>(win: &WebviewWindow<R>) -> Option<Rect> {
    let ns = ns_window(win)?;
    let f: NSRect = unsafe { msg_send![ns, frame] };
    let ph = primary_height();
    Some(Rect { x: f.origin.x, y: ph - (f.origin.y + f.size.height), w: f.size.width, h: f.size.height })
}

pub fn set_ignores_mouse<R: Runtime>(win: &WebviewWindow<R>, ignore: bool) {
    let Some(ns) = ns_window(win) else { return };
    unsafe {
        let _: () = msg_send![ns, setIgnoresMouseEvents: ignore];
    }
}

/// Floating on every Space, above full-screen apps, invisible to screen capture.
pub fn configure_overlay<R: Runtime>(win: &WebviewWindow<R>, level: isize) {
    let Some(ns) = ns_window(win) else { return };
    unsafe {
        let _: () = msg_send![ns, setLevel: level];
        let _: () = msg_send![
            ns,
            setCollectionBehavior: CAN_JOIN_ALL_SPACES | STATIONARY | IGNORES_CYCLE | FULL_SCREEN_AUXILIARY
        ];
        // All overlays are excluded from screen capture (BUILD-SPEC §1).
        let _: () = msg_send![ns, setSharingType: NS_WINDOW_SHARING_NONE];
        let _: () = msg_send![ns, setHidesOnDeactivate: false];
        let _: () = msg_send![ns, setHasShadow: false];
        // Borderless windows may sit over the menu bar.
        let _: () = msg_send![ns, setMovable: false];
    }
}

/// Show without activating the app or taking key focus from the user's app.
pub fn order_front_without_focus<R: Runtime>(win: &WebviewWindow<R>) {
    let Some(ns) = ns_window(win) else { return };
    unsafe {
        let _: () = msg_send![ns, orderFrontRegardless];
    }
}

pub fn order_out<R: Runtime>(win: &WebviewWindow<R>) {
    let Some(ns) = ns_window(win) else { return };
    unsafe {
        let _: () = msg_send![ns, orderOut: std::ptr::null::<AnyObject>()];
    }
}

/// Make the panel key so it can take typing (activates openclicky).
pub fn make_key<R: Runtime>(win: &WebviewWindow<R>) {
    let Some(ns) = ns_window(win) else { return };
    unsafe {
        if let Some(app_cls) = AnyClass::get(c"NSApplication") {
            let app: *mut AnyObject = msg_send![app_cls, sharedApplication];
            let _: () = msg_send![app, activateIgnoringOtherApps: true];
        }
        let _: () = msg_send![ns, makeKeyAndOrderFront: std::ptr::null::<AnyObject>()];
    }
}

/// Bring a normal window up where they are: on the current Space (over a full-screen app too),
/// in front, with keyboard focus.
pub fn present<R: Runtime>(win: &WebviewWindow<R>) {
    let Some(ns) = ns_window(win) else { return };
    unsafe {
        let _: () = msg_send![ns, setCollectionBehavior: MOVE_TO_ACTIVE_SPACE | FULL_SCREEN_AUXILIARY];
        let _: () = msg_send![ns, setLevel: NS_FLOATING_WINDOW_LEVEL];
        let _: () = msg_send![ns, setHidesOnDeactivate: false];
    }
    make_key(win);
}

/// Pixie is the active app (false once they click another app or the desktop).
pub fn app_active_in<R: Runtime>(_app: &tauri::AppHandle<R>) -> bool {
    unsafe {
        let Some(app_cls) = AnyClass::get(c"NSApplication") else { return true };
        let app: *mut AnyObject = msg_send![app_cls, sharedApplication];
        let active: bool = msg_send![app, isActive];
        active
    }
}

/// Give focus back to whatever app the user was in.
pub fn deactivate() {
    unsafe {
        if let Some(app_cls) = AnyClass::get(c"NSApplication") {
            let app: *mut AnyObject = msg_send![app_cls, sharedApplication];
            let _: () = msg_send![app, deactivate];
        }
    }
}
