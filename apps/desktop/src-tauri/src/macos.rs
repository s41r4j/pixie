//! Thin AppKit / CoreGraphics bindings. Every NSWindow call must run on the main thread.

use objc2::msg_send;
use objc2::runtime::AnyObject;
use tauri::{Runtime, WebviewWindow};

#[link(name = "CoreGraphics", kind = "framework")]
extern "C" {
    fn CGEventSourceFlagsState(state_id: i32) -> u64;
    fn CGEventSourceKeyState(state_id: i32, key: u16) -> bool;
}

/// kCGEventSourceStateCombinedSessionState
const COMBINED_SESSION: i32 = 0;

/// Current modifier flags (CGEventFlags). Needs no permission.
pub fn modifier_flags() -> u64 {
    unsafe { CGEventSourceFlagsState(COMBINED_SESSION) }
}

/// Whether a virtual key is down right now.
/// TODO(M2): verify on a clean machine; may need Input Monitoring for non-modifier keys.
pub fn key_down(keycode: u16) -> bool {
    unsafe { CGEventSourceKeyState(COMBINED_SESSION, keycode) }
}

const NS_STATUS_WINDOW_LEVEL: isize = 25;
const CAN_JOIN_ALL_SPACES: usize = 1 << 0;
const STATIONARY: usize = 1 << 4;
const IGNORES_CYCLE: usize = 1 << 6;
const FULL_SCREEN_AUXILIARY: usize = 1 << 8;
const NS_WINDOW_SHARING_NONE: usize = 0;

fn ns_window<R: Runtime>(win: &WebviewWindow<R>) -> Option<*mut AnyObject> {
    win.ns_window().ok().map(|p| p as *mut AnyObject).filter(|p| !p.is_null())
}

/// Floating overlay on every Space, above full-screen apps, invisible to screen capture.
pub fn configure_overlay<R: Runtime>(win: &WebviewWindow<R>) {
    let Some(ns) = ns_window(win) else { return };
    unsafe {
        let _: () = msg_send![ns, setLevel: NS_STATUS_WINDOW_LEVEL];
        let _: () = msg_send![
            ns,
            setCollectionBehavior: CAN_JOIN_ALL_SPACES | STATIONARY | IGNORES_CYCLE | FULL_SCREEN_AUXILIARY
        ];
        // All overlays are excluded from screen capture (BUILD-SPEC §1).
        let _: () = msg_send![ns, setSharingType: NS_WINDOW_SHARING_NONE];
        let _: () = msg_send![ns, setHidesOnDeactivate: false];
    }
}

/// Show without activating the app or taking key focus from the user's app.
/// TODO(M3): convert to a non-activating NSPanel so clicks inside it also keep focus.
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
