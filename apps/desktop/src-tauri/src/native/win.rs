//! Windows: GDI capture, Windows OCR, UI Automation, SendInput, WinRT speech.
//! Every COM call runs on the native worker thread (COM initialised there once).

use super::{emit, rect, Bgra, Shot};
use crate::macos::{to_logical, to_physical};
use serde_json::{json, Value};
use std::cell::RefCell;
use std::collections::HashMap;
use std::sync::Mutex;
use std::time::{Duration, Instant};
use windows::core::{Interface, HSTRING};
use windows::Win32::Foundation::{HWND, RECT};
use windows::Win32::System::Com::{CoCreateInstance, CoInitializeEx, CLSCTX_INPROC_SERVER, COINIT_MULTITHREADED};
use windows::Win32::UI::Accessibility::*;
use windows_sys::Win32::UI::Input::KeyboardAndMouse as km;
use windows_sys::Win32::UI::WindowsAndMessaging as wm;

pub fn init_thread() {
    unsafe {
        let _ = CoInitializeEx(None, COINIT_MULTITHREADED);
    }
}

// ---------- screen ----------

/// The monitor under the cursor, physical pixels (BGRA, top-down).
pub fn capture() -> Result<Shot, String> {
    use windows_sys::Win32::Graphics::Gdi::*;
    let (cx, cy) = phys_cursor();
    let (pos, size, scale) = crate::macos::phys_monitor_at(cx, cy);
    let (x, y, w, h) = (pos.0 as i32, pos.1 as i32, size.0 as i32, size.1 as i32);
    if w <= 0 || h <= 0 {
        return Err("No display.".into());
    }
    let mut px = vec![0u8; (w * h * 4) as usize];
    unsafe {
        let screen = GetDC(std::ptr::null_mut());
        let mem = CreateCompatibleDC(screen);
        let bmp = CreateCompatibleBitmap(screen, w, h);
        let old = SelectObject(mem, bmp);
        // Overlays are WDA_EXCLUDEFROMCAPTURE, so Pixie itself never shows up.
        let ok = BitBlt(mem, 0, 0, w, h, screen, x, y, SRCCOPY | CAPTUREBLT);
        let mut info: BITMAPINFO = std::mem::zeroed();
        info.bmiHeader.biSize = std::mem::size_of::<BITMAPINFOHEADER>() as u32;
        info.bmiHeader.biWidth = w;
        info.bmiHeader.biHeight = -h;
        info.bmiHeader.biPlanes = 1;
        info.bmiHeader.biBitCount = 32;
        info.bmiHeader.biCompression = BI_RGB;
        let lines = GetDIBits(mem, bmp, 0, h as u32, px.as_mut_ptr().cast(), &mut info, DIB_RGB_COLORS);
        SelectObject(mem, old);
        DeleteObject(bmp);
        DeleteDC(mem);
        ReleaseDC(std::ptr::null_mut(), screen);
        if ok == 0 || lines == 0 {
            return Err("Screen capture failed.".into());
        }
    }
    Ok(Shot {
        img: Bgra { w: w as u32, h: h as u32, px },
        display: crate::macos::Rect { x: pos.0 / scale, y: pos.1 / scale, w: size.0 / scale, h: size.1 / scale },
        scale,
    })
}

/// Windows OCR (the languages installed for the user). Lines in global logical points.
pub fn ocr(shot: &Shot) -> Vec<Value> {
    use windows::Graphics::Imaging::{BitmapPixelFormat, SoftwareBitmap};
    use windows::Media::Ocr::OcrEngine;
    use windows::Security::Cryptography::CryptographicBuffer;
    let run = || -> windows::core::Result<Vec<Value>> {
        let engine = OcrEngine::TryCreateFromUserProfileLanguages()?;
        // OCR is limited to 4096 px a side (OcrEngine::MaxImageDimension).
        let (img, k) = shot.img.fit(4096);
        let buf = CryptographicBuffer::CreateFromByteArray(&img.px)?;
        let bmp = SoftwareBitmap::CreateCopyFromBuffer(&buf, BitmapPixelFormat::Bgra8, img.w as i32, img.h as i32)?;
        let result = engine.RecognizeAsync(&bmp)?.join()?;
        let mut out = vec![];
        for line in result.Lines()? {
            let (mut x0, mut y0, mut x1, mut y1) = (f32::MAX, f32::MAX, f32::MIN, f32::MIN);
            for word in line.Words()? {
                let r = word.BoundingRect()?;
                x0 = x0.min(r.X);
                y0 = y0.min(r.Y);
                x1 = x1.max(r.X + r.Width);
                y1 = y1.max(r.Y + r.Height);
            }
            if x0 > x1 {
                continue;
            }
            let f = |v: f32| v as f64 / k / shot.scale;
            out.push(json!({
                "text": line.Text()?.to_string(),
                "frame": rect(shot.display.x + f(x0), shot.display.y + f(y0), f(x1 - x0), f(y1 - y0)),
            }));
        }
        Ok(out)
    };
    run().unwrap_or_else(|e| {
        log::warn!("ocr: {e}");
        vec![]
    })
}

// ---------- UI Automation ----------

struct Item {
    el: IUIAutomationElement,
    role: String,
    label: String,
    frame: Value,
}

thread_local! {
    static UIA: RefCell<Option<IUIAutomation>> = const { RefCell::new(None) };
    static CACHE: RefCell<HashMap<String, Item>> = RefCell::new(HashMap::new());
}

fn uia() -> Result<IUIAutomation, String> {
    UIA.with(|u| {
        if let Some(a) = u.borrow().as_ref() {
            return Ok(a.clone());
        }
        let a: IUIAutomation = unsafe { CoCreateInstance(&CUIAutomation, None, CLSCTX_INPROC_SERVER) }.map_err(|e| format!("UI Automation: {e}"))?;
        *u.borrow_mut() = Some(a.clone());
        Ok(a)
    })
}

#[allow(non_upper_case_globals)]
fn role_of(t: UIA_CONTROLTYPE_ID) -> Option<&'static str> {
    Some(match t {
        UIA_ButtonControlTypeId => "button",
        UIA_HyperlinkControlTypeId => "link",
        UIA_CheckBoxControlTypeId => "checkbox",
        UIA_RadioButtonControlTypeId => "radiobutton",
        UIA_ComboBoxControlTypeId => "combobox",
        UIA_EditControlTypeId => "textfield",
        UIA_DocumentControlTypeId => "textarea",
        UIA_TabItemControlTypeId => "tab",
        UIA_MenuItemControlTypeId => "menuitem",
        UIA_ListItemControlTypeId => "listitem",
        UIA_SliderControlTypeId => "slider",
        UIA_SplitButtonControlTypeId => "menubutton",
        UIA_TreeItemControlTypeId => "treeitem",
        _ => return None,
    })
}

fn bstr(r: windows::core::Result<windows::core::BSTR>) -> String {
    r.map(|b| b.to_string().trim().to_string()).unwrap_or_default()
}

fn logical_rect(r: RECT) -> Value {
    let (x, y) = to_logical(r.left as f64, r.top as f64);
    let (x2, y2) = to_logical(r.right as f64, r.bottom as f64);
    rect(x, y, x2 - x, y2 - y)
}

/// The foreground window's controls, numbered e1, e2… like the Mac's Accessibility snapshot.
pub fn snapshot(limit: usize) -> Value {
    let Ok(a) = uia() else { return json!({ "trusted": false, "elements": [] }) };
    CACHE.with(|c| c.borrow_mut().clear());
    let hwnd = unsafe { HWND(wm::GetForegroundWindow()) };
    let Ok(root) = (unsafe { a.ElementFromHandle(hwnd) }) else { return json!({ "trusted": true, "elements": [] }) };
    let Ok(walker) = (unsafe { a.ControlViewWalker() }) else { return json!({ "trusted": true, "elements": [] }) };
    let title = bstr(unsafe { root.CurrentName() });
    let deadline = Instant::now() + Duration::from_millis(900);
    let mut out = vec![];
    let mut visited = 0usize;
    let mut stack = vec![(root, 0usize)];
    while let Some((el, depth)) = stack.pop() {
        if out.len() >= limit || visited > 5000 || Instant::now() > deadline {
            break;
        }
        visited += 1;
        let ty = unsafe { el.CurrentControlType() }.unwrap_or_default();
        if let Some(role) = role_of(ty) {
            let off = unsafe { el.CurrentIsOffscreen() }.map(|b| b.as_bool()).unwrap_or(false);
            let r = unsafe { el.CurrentBoundingRectangle() }.unwrap_or_default();
            let mut label = bstr(unsafe { el.CurrentName() });
            if label.is_empty() {
                label = bstr(unsafe { el.CurrentHelpText() });
            }
            if !off && r.right - r.left > 1 && r.bottom - r.top > 1 && (!label.is_empty() || role == "textfield") {
                let id = format!("e{}", out.len() + 1);
                let label: String = label.chars().take(80).collect();
                let frame = logical_rect(r);
                out.push(json!({ "id": id, "role": role, "label": label, "frame": frame }));
                CACHE.with(|c| c.borrow_mut().insert(id, Item { el: el.clone(), role: role.into(), label, frame }));
            }
        }
        if depth < 45 {
            // Children pushed in reverse so the walk stays in reading order.
            let mut kids = vec![];
            let mut next = unsafe { walker.GetFirstChildElement(&el) }.ok();
            while let Some(k) = next {
                next = unsafe { walker.GetNextSiblingElement(&k) }.ok();
                kids.push(k);
                if kids.len() > 400 {
                    break;
                }
            }
            for k in kids.into_iter().rev() {
                stack.push((k, depth + 1));
            }
        }
    }
    json!({ "trusted": true, "window": title, "elements": out })
}

fn find(target: &str) -> Option<(IUIAutomationElement, String, String, Value)> {
    let get = |t: &str| {
        CACHE.with(|c| {
            let c = c.borrow();
            if let Some(i) = c.get(t) {
                return Some((i.el.clone(), i.role.clone(), i.label.clone(), i.frame.clone()));
            }
            let t = t.to_lowercase();
            let pick = |f: &dyn Fn(&str) -> bool| c.values().find(|i| f(&i.label.to_lowercase())).map(|i| (i.el.clone(), i.role.clone(), i.label.clone(), i.frame.clone()));
            pick(&|l| l == t).or_else(|| pick(&|l| l.starts_with(&t))).or_else(|| pick(&|l| l.contains(&t)))
        })
    };
    get(target).or_else(|| {
        snapshot(200);
        get(target)
    })
}

pub fn find_json(target: &str) -> Result<Value, String> {
    let (_, role, label, frame) = find(target).ok_or_else(|| format!("I couldn't find \"{target}\"."))?;
    Ok(json!({ "label": label, "role": role, "frame": frame }))
}

pub fn press(target: &str) -> Result<Value, String> {
    let (el, role, label, frame) = find(target).ok_or_else(|| format!("I couldn't find \"{target}\" on screen."))?;
    let done = unsafe {
        el.GetCurrentPatternAs::<IUIAutomationInvokePattern>(UIA_InvokePatternId).and_then(|p| p.Invoke()).is_ok()
            || el.GetCurrentPatternAs::<IUIAutomationTogglePattern>(UIA_TogglePatternId).and_then(|p| p.Toggle()).is_ok()
            || el.GetCurrentPatternAs::<IUIAutomationSelectionItemPattern>(UIA_SelectionItemPatternId).and_then(|p| p.Select()).is_ok()
            || el.GetCurrentPatternAs::<IUIAutomationExpandCollapsePattern>(UIA_ExpandCollapsePatternId).and_then(|p| p.Expand()).is_ok()
    };
    if !done {
        // No pattern: click its middle.
        let f = |k: &str| frame[k].as_f64().unwrap_or(0.0);
        click(f("x") + f("w") / 2.0, f("y") + f("h") / 2.0, "left", 1)?;
    }
    Ok(json!({ "pressed": label, "role": role, "frame": frame }))
}

pub fn focus(target: &str) -> Result<Value, String> {
    let (el, _, label, frame) = find(target).ok_or_else(|| format!("I couldn't find \"{target}\"."))?;
    unsafe { el.SetFocus() }.map_err(|e| e.to_string())?;
    Ok(json!({ "focused": label, "frame": frame }))
}

/// "File > Save as": press each menu item in turn.
pub fn menu(path: &str) -> Result<Value, String> {
    for part in path.split('>').map(str::trim).filter(|p| !p.is_empty()) {
        snapshot(300);
        press(part)?;
        std::thread::sleep(Duration::from_millis(250));
    }
    Ok(json!({ "menu": path }))
}

pub fn focused() -> String {
    let Ok(a) = uia() else { return String::new() };
    unsafe { a.GetFocusedElement() }
        .ok()
        .map(|el| {
            let role = role_of(unsafe { el.CurrentControlType() }.unwrap_or_default()).unwrap_or("element");
            format!("{role} {}", bstr(unsafe { el.CurrentName() }))
        })
        .unwrap_or_default()
}

// ---------- input ----------

fn phys_cursor() -> (f64, f64) {
    let mut p = windows_sys::Win32::Foundation::POINT { x: 0, y: 0 };
    unsafe { wm::GetCursorPos(&mut p) };
    (p.x as f64, p.y as f64)
}

fn send(inputs: &[km::INPUT]) {
    unsafe { km::SendInput(inputs.len() as u32, inputs.as_ptr(), std::mem::size_of::<km::INPUT>() as i32) };
}

fn mouse(flags: u32, data: i32) -> km::INPUT {
    let mut i: km::INPUT = unsafe { std::mem::zeroed() };
    i.r#type = km::INPUT_MOUSE;
    i.Anonymous.mi = km::MOUSEINPUT { dx: 0, dy: 0, mouseData: data as _, dwFlags: flags, time: 0, dwExtraInfo: 0 };
    i
}

fn key(vk: u16, up: bool) -> km::INPUT {
    let mut i: km::INPUT = unsafe { std::mem::zeroed() };
    i.r#type = km::INPUT_KEYBOARD;
    let extended = matches!(vk, 0x21..=0x28 | 0x2D | 0x2E | 0x5B | 0x5C);
    let mut flags = if up { km::KEYEVENTF_KEYUP } else { 0 };
    if extended {
        flags |= km::KEYEVENTF_EXTENDEDKEY;
    }
    i.Anonymous.ki = km::KEYBDINPUT { wVk: vk, wScan: 0, dwFlags: flags, time: 0, dwExtraInfo: 0 };
    i
}

/// Click at a logical point, then put the user's cursor back.
pub fn click(x: f64, y: f64, button: &str, count: u32) -> Result<Value, String> {
    let original = phys_cursor();
    let (px, py) = to_physical(x, y);
    unsafe { wm::SetCursorPos(px.round() as i32, py.round() as i32) };
    std::thread::sleep(Duration::from_millis(30));
    let (down, up) = if button == "right" { (km::MOUSEEVENTF_RIGHTDOWN, km::MOUSEEVENTF_RIGHTUP) } else { (km::MOUSEEVENTF_LEFTDOWN, km::MOUSEEVENTF_LEFTUP) };
    for _ in 0..count.clamp(1, 3) {
        send(&[mouse(down, 0)]);
        std::thread::sleep(Duration::from_millis(18));
        send(&[mouse(up, 0)]);
        std::thread::sleep(Duration::from_millis(40));
    }
    std::thread::sleep(Duration::from_millis(60));
    unsafe { wm::SetCursorPos(original.0 as i32, original.1 as i32) };
    Ok(json!({ "clicked": { "x": x as i64, "y": y as i64 }, "button": button, "count": count }))
}

pub fn drag(x: f64, y: f64, tx: f64, ty: f64) -> Result<Value, String> {
    let original = phys_cursor();
    let (ax, ay) = to_physical(x, y);
    let (bx, by) = to_physical(tx, ty);
    unsafe { wm::SetCursorPos(ax as i32, ay as i32) };
    send(&[mouse(km::MOUSEEVENTF_LEFTDOWN, 0)]);
    for i in 1..=24 {
        let t = i as f64 / 24.0;
        unsafe { wm::SetCursorPos((ax + (bx - ax) * t) as i32, (ay + (by - ay) * t) as i32) };
        send(&[mouse(km::MOUSEEVENTF_MOVE, 0)]);
        std::thread::sleep(Duration::from_millis(12));
    }
    send(&[mouse(km::MOUSEEVENTF_LEFTUP, 0)]);
    std::thread::sleep(Duration::from_millis(60));
    unsafe { wm::SetCursorPos(original.0 as i32, original.1 as i32) };
    Ok(json!({ "dragged": { "from": { "x": x as i64, "y": y as i64 }, "to": { "x": tx as i64, "y": ty as i64 } } }))
}

pub fn scroll(up: bool) {
    send(&[mouse(km::MOUSEEVENTF_WHEEL, if up { 360 } else { -360 })]);
}

fn vk(name: &str) -> Option<u16> {
    let n = name.trim();
    if n.chars().count() == 1 {
        let c = n.chars().next()?.to_ascii_uppercase();
        if c.is_ascii_alphanumeric() {
            return Some(c as u16);
        }
        return Some(match c {
            '=' => 0xBB,
            '-' => 0xBD,
            ',' => 0xBC,
            '.' => 0xBE,
            '/' => 0xBF,
            ';' => 0xBA,
            '\'' => 0xDE,
            '[' => 0xDB,
            ']' => 0xDD,
            '\\' => 0xDC,
            '`' => 0xC0,
            _ => return None,
        });
    }
    if let Some(f) = n.strip_prefix('f').and_then(|d| d.parse::<u16>().ok()).filter(|d| (1..=24).contains(d)) {
        return Some(0x6F + f);
    }
    Some(match n {
        "return" | "enter" => 0x0D,
        "tab" => 0x09,
        "space" => 0x20,
        "delete" | "backspace" => 0x08,
        "forwarddelete" | "del" => 0x2E,
        "escape" | "esc" => 0x1B,
        "left" => 0x25,
        "up" => 0x26,
        "right" => 0x27,
        "down" => 0x28,
        "home" => 0x24,
        "end" => 0x23,
        "pageup" => 0x21,
        "pagedown" => 0x22,
        "insert" => 0x2D,
        _ => return None,
    })
}

/// "cmd+shift+t": ⌘ means Ctrl here, ⌥ Alt; "win" is the Windows key.
pub fn press_keys(combo: &str) -> Result<(), String> {
    let mut mods = vec![];
    let mut main = None;
    for p in combo.to_lowercase().split('+').map(str::trim) {
        match p {
            "cmd" | "command" | "⌘" | "ctrl" | "control" | "⌃" => mods.push(0x11),
            "shift" | "⇧" => mods.push(0x10),
            "opt" | "option" | "alt" | "⌥" => mods.push(0x12),
            "win" | "windows" | "super" | "meta" => mods.push(0x5B),
            other => main = vk(other),
        }
    }
    let k = main.ok_or_else(|| format!("Unknown key in \"{combo}\"."))?;
    let mut seq: Vec<km::INPUT> = mods.iter().map(|m| key(*m, false)).collect();
    seq.push(key(k, false));
    seq.push(key(k, true));
    seq.extend(mods.iter().rev().map(|m| key(*m, true)));
    send(&seq);
    Ok(())
}

pub fn frontmost() -> Value {
    use windows_sys::Win32::System::Threading::*;
    unsafe {
        let hwnd = wm::GetForegroundWindow();
        let mut title = [0u16; 512];
        let n = wm::GetWindowTextW(hwnd, title.as_mut_ptr(), title.len() as i32);
        let mut pid = 0u32;
        wm::GetWindowThreadProcessId(hwnd, &mut pid);
        let mut name = String::new();
        let h = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, 0, pid);
        if !h.is_null() {
            let mut buf = [0u16; 1024];
            let mut len = buf.len() as u32;
            if QueryFullProcessImageNameW(h, 0, buf.as_mut_ptr(), &mut len) != 0 {
                let path = String::from_utf16_lossy(&buf[..len as usize]);
                name = std::path::Path::new(&path).file_stem().map(|s| s.to_string_lossy().to_string()).unwrap_or_default();
            }
            windows_sys::Win32::Foundation::CloseHandle(h);
        }
        let window = String::from_utf16_lossy(&title[..n.max(0) as usize]);
        json!({ "app": pretty_app(&name, &window), "bundle": name, "pid": pid, "secure": false, "window": window })
    }
}

/// "chrome" → "Google Chrome" etc.; otherwise the window title's app part.
fn pretty_app(exe: &str, title: &str) -> String {
    let known = [("chrome", "Google Chrome"), ("msedge", "Microsoft Edge"), ("firefox", "Firefox"), ("code", "Visual Studio Code"), ("explorer", "File Explorer"), ("winword", "Word"), ("excel", "Excel"), ("powerpnt", "PowerPoint"), ("outlook", "Outlook"), ("olk", "Outlook"), ("ms-teams", "Teams"), ("slack", "Slack"), ("notepad", "Notepad"), ("brave", "Brave")];
    let e = exe.to_lowercase();
    known
        .iter()
        .find(|(k, _)| *k == e)
        .map(|(_, v)| v.to_string())
        .or_else(|| title.rsplit(" - ").next().filter(|s| !s.is_empty() && s.len() < 40).map(str::to_string))
        .unwrap_or_else(|| exe.to_string())
}

/// URL, file, folder, or an app by name (anything ShellExecute knows: notepad, chrome, excel…).
pub fn open(target: &str) -> Result<(), String> {
    use windows_sys::Win32::UI::Shell::ShellExecuteW;
    let wide = |s: &str| s.encode_utf16().chain(Some(0)).collect::<Vec<u16>>();
    let t = target.trim();
    if t.is_empty() {
        return Err("Nothing to open.".into());
    }
    let home = std::env::var_os("USERPROFILE").map(std::path::PathBuf::from).unwrap_or_default();
    let expanded = t.strip_prefix("~/").or_else(|| t.strip_prefix("~\\")).map(|r| home.join(r).display().to_string()).unwrap_or_else(|| t.to_string());
    let r = unsafe { ShellExecuteW(std::ptr::null_mut(), wide("open").as_ptr(), wide(&expanded).as_ptr(), std::ptr::null(), std::ptr::null(), wm::SW_SHOWNORMAL) };
    if r as isize <= 32 {
        return Err(format!("I couldn't find an app or file called \"{t}\"."));
    }
    Ok(())
}

// ---------- speech ----------

pub fn voices() -> Value {
    use windows::Media::SpeechSynthesis::SpeechSynthesizer;
    let list = || -> windows::core::Result<(Vec<Value>, String)> {
        let mut out = vec![];
        for v in SpeechSynthesizer::AllVoices()? {
            out.push(json!({ "id": v.Id()?.to_string(), "name": v.DisplayName()?.to_string(), "language": v.Language()?.to_string(), "quality": "default" }));
        }
        let def = SpeechSynthesizer::DefaultVoice().and_then(|v| v.Id()).map(|s| s.to_string()).unwrap_or_default();
        Ok((out, def))
    };
    let (voices, default) = list().unwrap_or_default();
    json!({ "voices": voices, "default": default })
}

/// Text → a WAV file with the chosen Windows voice. `rate` is the Mac's 0–1 scale (0.52 normal).
pub fn synth(text: &str, voice: &str, rate: f64, out: &std::path::Path) -> Result<(), String> {
    use windows::Media::SpeechSynthesis::SpeechSynthesizer;
    use windows::Storage::Streams::DataReader;
    use windows_future::IAsyncOperation;
    let run = || -> windows::core::Result<Vec<u8>> {
        let s = SpeechSynthesizer::new()?;
        if !voice.is_empty() {
            if let Some(v) = SpeechSynthesizer::AllVoices()?.into_iter().find(|v| v.Id().map(|i| i == voice).unwrap_or(false)) {
                s.SetVoice(&v)?;
            }
        }
        let _ = s.Options()?.SetSpeakingRate((rate / 0.52).clamp(0.5, 3.0));
        let stream = s.SynthesizeTextToStreamAsync(&HSTRING::from(text))?.join()?;
        let size = stream.Size()? as u32;
        let reader = DataReader::CreateDataReader(&stream.GetInputStreamAt(0)?)?;
        let op: IAsyncOperation<u32> = reader.LoadAsync(size)?.cast()?;
        op.join()?;
        let mut bytes = vec![0u8; size as usize];
        reader.ReadBytes(&mut bytes)?;
        Ok(bytes)
    };
    let bytes = run().map_err(|e| format!("Windows voice: {e}"))?;
    std::fs::write(out, bytes).map_err(|e| e.to_string())
}

struct Listening {
    rec: windows::Media::SpeechRecognition::SpeechRecognizer,
    text: std::sync::Arc<Mutex<(String, String)>>,
    meter: Option<std::sync::mpsc::Sender<()>>,
}
// WinRT speech objects are agile (free-threaded).
unsafe impl Send for Listening {}

static LISTEN: Mutex<Option<Listening>> = Mutex::new(None);

const PRIVACY: &str = "Windows speech recognition is off. Turn on Settings → Privacy & security → Speech → Online speech recognition, then try again.";

pub fn listen_start(locale: &str) -> Result<Value, String> {
    use windows::Foundation::TypedEventHandler;
    use windows::Globalization::Language;
    use windows::Media::SpeechRecognition::*;
    listen_cancel();
    let text = std::sync::Arc::new(Mutex::new((String::new(), String::new())));
    let start = || -> windows::core::Result<SpeechRecognizer> {
        let rec = match (!locale.is_empty()).then(|| Language::CreateLanguage(&HSTRING::from(locale.replace('_', "-")))) {
            Some(Ok(l)) => SpeechRecognizer::Create(&l).or_else(|_| SpeechRecognizer::new())?,
            _ => SpeechRecognizer::new()?,
        };
        let compiled = rec.CompileConstraintsAsync()?.join()?;
        if compiled.Status()? != SpeechRecognitionResultStatus::Success {
            return Err(windows::core::Error::new(windows::core::HRESULT(0x80045509u32 as i32), "compile"));
        }
        let t = text.clone();
        rec.HypothesisGenerated(&TypedEventHandler::new(move |_, args: windows::core::Ref<SpeechRecognitionHypothesisGeneratedEventArgs>| {
            if let Ok(a) = args.ok() {
                let h = a.Hypothesis()?.Text()?.to_string();
                let mut g = t.lock().unwrap();
                g.1 = h;
                emit("partial", json!({ "text": join(&g.0, &g.1) }));
            }
            Ok(())
        }))?;
        let session = rec.ContinuousRecognitionSession()?;
        let t = text.clone();
        session.ResultGenerated(&TypedEventHandler::new(move |_, args: windows::core::Ref<SpeechContinuousRecognitionResultGeneratedEventArgs>| {
            if let Ok(a) = args.ok() {
                let r = a.Result()?;
                if r.Status()? == SpeechRecognitionResultStatus::Success {
                    let mut g = t.lock().unwrap();
                    let said = r.Text()?.to_string();
                    g.0 = join(&g.0, &said);
                    g.1.clear();
                    emit("partial", json!({ "text": g.0.clone() }));
                }
            }
            Ok(())
        }))?;
        session.StartAsync()?.join()?;
        Ok(rec)
    };
    let rec = start().map_err(|e| {
        if e.code().0 as u32 == 0x80045509 {
            PRIVACY.to_string()
        } else if e.code().0 as u32 == 0x80070005 {
            "Pixie can't use the microphone. Allow it in Settings → Privacy & security → Microphone.".to_string()
        } else {
            format!("Windows speech recognition couldn't start: {e}")
        }
    })?;
    let meter = super::meter::start();
    *LISTEN.lock().unwrap() = Some(Listening { rec, text, meter });
    Ok(json!({ "on_device": false }))
}

fn join(a: &str, b: &str) -> String {
    match (a.is_empty(), b.is_empty()) {
        (true, _) => b.to_string(),
        (_, true) => a.to_string(),
        _ => format!("{a} {b}"),
    }
}

pub fn listen_stop() -> Value {
    let Some(l) = LISTEN.lock().unwrap().take() else { return json!({ "text": "" }) };
    drop(l.meter);
    if let Ok(s) = l.rec.ContinuousRecognitionSession() {
        // StopAsync waits for the last phrase to be finalised.
        if let Ok(op) = s.StopAsync() {
            let _ = op.join();
        }
    }
    let g = l.text.lock().unwrap();
    json!({ "text": join(&g.0, &g.1) })
}

pub fn listen_cancel() {
    if let Some(l) = LISTEN.lock().unwrap().take() {
        if let Ok(s) = l.rec.ContinuousRecognitionSession() {
            if let Ok(op) = s.CancelAsync() {
                let _ = op.join();
            }
        }
    }
}

/// For the snapshot result shape shared with Linux.
pub fn controls_after() -> Value {
    let ax = snapshot(60);
    let controls: Vec<String> = ax["elements"].as_array().map(|a| a.iter().take(25).map(|e| format!("{} {} {}", e["id"].as_str().unwrap_or(""), e["role"].as_str().unwrap_or(""), e["label"].as_str().unwrap_or(""))).collect()).unwrap_or_default();
    let f = frontmost();
    json!({ "app": f["app"], "window": ax["window"], "focused": focused(), "controls": controls })
}
