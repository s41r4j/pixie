//! Chrome native messaging host `com.openclicky.browser` (M7).
//!
//! Chrome starts this with the extension origin as its first argument and talks over stdio
//! using 4-byte little-endian length-prefixed JSON. We relay every message, as one JSON line,
//! to the running app over a unix socket (`<app_data>/browser.sock`), and back.
//!
//! The app's own binary doubles as the host: `main` calls [`run`] when argv[1] starts with
//! `chrome-extension://`, so there is nothing extra to install.

#![cfg_attr(not(unix), allow(unused))]

use std::io::{self, BufRead, BufReader, Read, Write};
#[cfg(unix)]
use std::os::unix::net::UnixStream;
use std::path::PathBuf;
use std::time::Duration;

pub const HOST_NAME: &str = "com.openclicky.browser";
pub const EXTENSION_ID: &str = "hdbcbdoddehijplnbajmbhlpjammchmf";
/// Chrome's limit for host → extension messages.
const MAX_OUT: usize = 1024 * 1024;

pub fn is_native_host_launch(args: &[String]) -> bool {
    args.get(1).map(|a| a.starts_with("chrome-extension://")).unwrap_or(false)
}

pub fn default_socket() -> PathBuf {
    let home = std::env::var_os("HOME").map(PathBuf::from).unwrap_or_default();
    home.join("Library/Application Support/com.openclicky.app/browser.sock")
}

/// Read one native message (None on EOF).
pub fn read_message(r: &mut impl Read) -> io::Result<Option<Vec<u8>>> {
    let mut len = [0u8; 4];
    match r.read_exact(&mut len) {
        Ok(()) => {}
        Err(e) if e.kind() == io::ErrorKind::UnexpectedEof => return Ok(None),
        Err(e) => return Err(e),
    }
    let n = u32::from_le_bytes(len) as usize;
    if n > 64 * 1024 * 1024 {
        return Err(io::Error::new(io::ErrorKind::InvalidData, "message too large"));
    }
    let mut buf = vec![0u8; n];
    r.read_exact(&mut buf)?;
    Ok(Some(buf))
}

pub fn write_message(w: &mut impl Write, json: &[u8]) -> io::Result<()> {
    w.write_all(&(json.len() as u32).to_le_bytes())?;
    w.write_all(json)?;
    w.flush()
}

#[cfg(unix)]
fn connect(socket: &PathBuf) -> io::Result<UnixStream> {
    let mut last = None;
    for i in 0..40 {
        match UnixStream::connect(socket) {
            Ok(s) => return Ok(s),
            Err(e) => {
                if i == 0 {
                    // Start the app if it isn't running.
                    let _ = std::process::Command::new("/usr/bin/open").args(["-g", "-b", "com.openclicky.app"]).status();
                }
                last = Some(e);
                std::thread::sleep(Duration::from_millis(250));
            }
        }
    }
    Err(last.unwrap_or_else(|| io::Error::other("no socket")))
}

/// The browser bridge is macOS-only for now (it relays over a unix socket).
#[cfg(not(unix))]
pub fn run(_socket: PathBuf) -> i32 {
    eprintln!("openclicky native host: the browser bridge isn't available on this system yet");
    1
}

/// Relay until either side closes. Returns the process exit code.
#[cfg(unix)]
pub fn run(socket: PathBuf) -> i32 {
    let stream = match connect(&socket) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("openclicky native host: can't reach the app at {}: {e}", socket.display());
            return 1;
        }
    };
    let mut to_app = stream.try_clone().expect("clone socket");
    // Chrome → app.
    let up = std::thread::spawn(move || {
        let mut stdin = io::stdin().lock();
        while let Ok(Some(msg)) = read_message(&mut stdin) {
            if to_app.write_all(&msg).and_then(|_| to_app.write_all(b"\n")).is_err() {
                break;
            }
        }
        let _ = to_app.shutdown(std::net::Shutdown::Both);
    });
    // App → Chrome.
    let mut stdout = io::stdout().lock();
    for line in BufReader::new(stream).lines() {
        let Ok(line) = line else { break };
        if line.trim().is_empty() {
            continue;
        }
        if line.len() > MAX_OUT {
            let err = serde_json::json!({ "type": "error", "error": "message too large for Chrome" }).to_string();
            let _ = write_message(&mut stdout, err.as_bytes());
            continue;
        }
        if write_message(&mut stdout, line.as_bytes()).is_err() {
            break;
        }
    }
    let _ = up.join();
    0
}

/// The manifest Chrome reads to find this host.
pub fn host_manifest(binary: &std::path::Path) -> String {
    serde_json::json!({
        "name": HOST_NAME,
        "description": "openclicky browser bridge",
        "path": binary,
        "type": "stdio",
        "allowed_origins": [format!("chrome-extension://{EXTENSION_ID}/")],
    })
    .to_string()
}

/// NativeMessagingHosts folders for Chromium browsers on macOS.
pub fn manifest_dirs() -> Vec<PathBuf> {
    let home = std::env::var_os("HOME").map(PathBuf::from).unwrap_or_default();
    let base = home.join("Library/Application Support");
    [
        "Google/Chrome",
        "Google/Chrome Beta",
        "Google/Chrome Canary",
        "Chromium",
        "BraveSoftware/Brave-Browser",
        "Microsoft Edge",
        "Arc/User Data",
        "Vivaldi",
    ]
    .iter()
    .map(|d| base.join(d))
    .filter(|d| d.exists())
    .map(|d| d.join("NativeMessagingHosts"))
    .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;

    #[test]
    fn framing_roundtrip() {
        let mut buf = vec![];
        write_message(&mut buf, br#"{"type":"hello"}"#).unwrap();
        write_message(&mut buf, b"{}").unwrap();
        assert_eq!(&buf[..4], &16u32.to_le_bytes());
        let mut r = Cursor::new(buf);
        assert_eq!(read_message(&mut r).unwrap().unwrap(), br#"{"type":"hello"}"#);
        assert_eq!(read_message(&mut r).unwrap().unwrap(), b"{}");
        assert!(read_message(&mut r).unwrap().is_none());
    }

    #[test]
    fn launch_detection_and_manifest() {
        assert!(is_native_host_launch(&["x".into(), "chrome-extension://abc/".into()]));
        assert!(!is_native_host_launch(&["x".into(), "--show-panel".into()]));
        let m: serde_json::Value = serde_json::from_str(&host_manifest(std::path::Path::new("/A/openclicky"))).unwrap();
        assert_eq!(m["allowed_origins"][0], format!("chrome-extension://{EXTENSION_ID}/"));
        assert_eq!(m["path"], "/A/openclicky");
    }
}
