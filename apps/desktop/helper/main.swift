// openclicky-helper: the native half of openclicky.
//
// A small Swift process the Rust shell spawns and talks to over JSON lines on stdin/stdout:
//   → {"id": 1, "cmd": "capture", "args": {...}}
//   ← {"id": 1, "ok": true, "result": {...}}      or {"id": 1, "ok": false, "error": "..."}
//   ← {"event": "partial", "data": {"text": "..."}}  (unsolicited)
//
// Swift owns the Apple frameworks that are awkward from Rust: Speech (streaming dictation),
// AVAudioEngine (mic + level meter), ScreenCaptureKit + Vision (screenshot + OCR),
// AVSpeechSynthesizer (voice), Accessibility (read UI, press, type) and WebKit (HTML → PDF).
// macOS attributes privacy permissions to the parent app, so prompts say "openclicky".
//
// Also a CLI for the job agent's bash tool:
//   openclicky-helper pdf in.html out.pdf [letter|a4]   paginated, real paper sizes
//   openclicky-helper text file.pdf                     text of PDF/Word/RTF/HTML

import AVFoundation
import AppKit
import ApplicationServices
import Carbon
import Contacts
import EventKit
import Foundation
import ScreenCaptureKit
import Speech
import Vision
import PDFKit
import WebKit

// MARK: - wire

let outLock = NSLock()

func send(_ obj: [String: Any]) {
    guard let data = try? JSONSerialization.data(withJSONObject: obj, options: []) else { return }
    outLock.lock()
    FileHandle.standardOutput.write(data)
    FileHandle.standardOutput.write(Data([0x0A]))
    outLock.unlock()
}

func emit(_ name: String, _ data: [String: Any] = [:]) { send(["event": name, "data": data]) }
func reply(_ id: Any, _ result: Any = [String: Any]()) { send(["id": id, "ok": true, "result": result]) }
func fail(_ id: Any, _ message: String) { send(["id": id, "ok": false, "error": message]) }

struct HelperError: Error, CustomStringConvertible {
    let description: String
    init(_ d: String) { description = d }
}

func rectJSON(_ r: CGRect) -> [String: Int] {
    ["x": Int(r.minX.rounded()), "y": Int(r.minY.rounded()), "w": Int(r.width.rounded()), "h": Int(r.height.rounded())]
}

// MARK: - listening (mic → level meter + streaming speech recognition)

final class Listener {
    private let engine = AVAudioEngine()
    private var request: SFSpeechAudioBufferRecognitionRequest?
    private var task: SFSpeechRecognitionTask?
    private var latest = ""
    private var pendingStop: Any?
    private var running = false
    private var session = 0
    private var lastLevel = Date.distantPast

    func start(locale: String?, onDevice: Bool, id: Any) {
        stopEngine()
        cancelTask()
        session += 1
        let mySession = session
        latest = ""
        pendingStop = nil

        let begin = { [self] in
            let loc = locale.flatMap { $0.isEmpty ? nil : Locale(identifier: $0) } ?? Locale.current
            guard let rec = SFSpeechRecognizer(locale: loc) ?? SFSpeechRecognizer(), rec.isAvailable else {
                fail(id, "Speech recognition isn't available right now.")
                return
            }
            let req = SFSpeechAudioBufferRecognitionRequest()
            req.shouldReportPartialResults = true
            req.addsPunctuation = true
            if onDevice && rec.supportsOnDeviceRecognition { req.requiresOnDeviceRecognition = true }
            request = req

            let input = engine.inputNode
            let format = input.outputFormat(forBus: 0)
            guard format.sampleRate > 0 else {
                fail(id, "No microphone input is available.")
                return
            }
            input.removeTap(onBus: 0)
            input.installTap(onBus: 0, bufferSize: 1024, format: format) { [weak self] buffer, _ in
                req.append(buffer)
                self?.meter(buffer)
            }
            engine.prepare()
            do { try engine.start() } catch {
                fail(id, "Couldn't start the microphone: \(error.localizedDescription)")
                return
            }
            running = true
            task = rec.recognitionTask(with: req) { [weak self] result, error in
                DispatchQueue.main.async {
                    guard let self, self.session == mySession else { return }
                    if let result {
                        self.latest = result.bestTranscription.formattedString
                        emit("partial", ["text": self.latest])
                        if result.isFinal { self.finish() }
                    }
                    if error != nil { self.finish() }
                }
            }
            reply(id, ["on_device": req.requiresOnDeviceRecognition])
        }

        authorize { ok in
            DispatchQueue.main.async {
                guard self.session == mySession else { return }
                if ok { begin() } else { fail(id, "openclicky needs Microphone and Speech Recognition access.") }
            }
        }
    }

    private func authorize(_ done: @escaping (Bool) -> Void) {
        let speechOK = { (cont: @escaping (Bool) -> Void) in
            switch SFSpeechRecognizer.authorizationStatus() {
            case .authorized: cont(true)
            case .notDetermined: SFSpeechRecognizer.requestAuthorization { cont($0 == .authorized) }
            default: cont(false)
            }
        }
        switch AVCaptureDevice.authorizationStatus(for: .audio) {
        case .authorized: speechOK(done)
        case .notDetermined:
            AVCaptureDevice.requestAccess(for: .audio) { granted in granted ? speechOK(done) : done(false) }
        default: done(false)
        }
    }

    private func meter(_ buffer: AVAudioPCMBuffer) {
        guard let ch = buffer.floatChannelData?[0] else { return }
        let n = Int(buffer.frameLength)
        guard n > 0 else { return }
        var sum: Float = 0
        for i in 0..<n { sum += ch[i] * ch[i] }
        let rms = sqrt(sum / Float(n))
        let now = Date()
        if now.timeIntervalSince(lastLevel) < 0.033 { return }
        lastLevel = now
        // Map roughly -60..-10 dBFS onto 0..1 for the waveform.
        let db = 20 * log10(max(rms, 1e-7))
        let level = min(1, max(0, (db + 60) / 50))
        emit("level", ["level": Double(level)])
    }

    /// End of speech: flush audio and reply with the final transcript (or the latest partial).
    func stop(id: Any) {
        guard running || task != nil else {
            reply(id, ["text": latest])
            return
        }
        pendingStop = id
        stopEngine()
        request?.endAudio()
        let mySession = session
        // Final results usually land within ~300 ms; don't wait forever.
        DispatchQueue.main.asyncAfter(deadline: .now() + 1.2) { [weak self] in
            guard let self, self.session == mySession else { return }
            self.finish()
        }
    }

    func cancel() {
        session += 1
        stopEngine()
        cancelTask()
        if let id = pendingStop { reply(id, ["text": ""]) }
        pendingStop = nil
    }

    private func finish() {
        if let id = pendingStop {
            reply(id, ["text": latest])
            pendingStop = nil
            cancelTask()
        }
    }

    private func stopEngine() {
        if running {
            engine.inputNode.removeTap(onBus: 0)
            engine.stop()
            running = false
        }
    }

    private func cancelTask() {
        task?.cancel()
        task = nil
        request = nil
    }
}

// MARK: - speaking

final class Speaker: NSObject, AVSpeechSynthesizerDelegate {
    private let synth = AVSpeechSynthesizer()
    private var ids: [ObjectIdentifier: String] = [:]
    var voiceId: String?
    var rate: Float = 0.52

    override init() {
        super.init()
        synth.delegate = self
    }

    static func bestVoice(language: String) -> AVSpeechSynthesisVoice? {
        let lang = language.lowercased()
        let voices = AVSpeechSynthesisVoice.speechVoices().filter {
            $0.language.lowercased().hasPrefix(String(lang.prefix(2)))
        }
        func score(_ v: AVSpeechSynthesisVoice) -> Int {
            var s = 0
            switch v.quality {
            case .premium: s += 300
            case .enhanced: s += 200
            default: s += 0
            }
            if v.language.lowercased() == lang { s += 50 }
            // Novelty voices (Bells, Zarvox…) are never a good default.
            if v.identifier.contains("speech.synthesis.voice") { s -= 500 }
            if v.identifier.contains("eloquence") { s -= 100 }
            if ["Zoe", "Ava", "Evan", "Samantha", "Allison", "Nathan", "Joelle"].contains(where: { v.name.hasPrefix($0) }) { s += 20 }
            return s
        }
        return voices.max { score($0) < score($1) } ?? AVSpeechSynthesisVoice(language: language)
    }

    func speak(_ text: String, utterance uid: String) {
        let u = AVSpeechUtterance(string: text)
        let lang = Locale.preferredLanguages.first ?? "en-US"
        u.voice = voiceId.flatMap { AVSpeechSynthesisVoice(identifier: $0) } ?? Speaker.bestVoice(language: lang)
        u.rate = rate
        u.pitchMultiplier = 1.02
        u.postUtteranceDelay = 0.04
        ids[ObjectIdentifier(u)] = uid
        synth.speak(u)
    }

    func stop() { synth.stopSpeaking(at: .immediate) }
    var speaking: Bool { synth.isSpeaking }

    func speechSynthesizer(_ s: AVSpeechSynthesizer, didStart u: AVSpeechUtterance) {
        emit("speak_start", ["id": ids[ObjectIdentifier(u)] ?? ""])
    }

    func speechSynthesizer(_ s: AVSpeechSynthesizer, willSpeakRangeOfSpeechString r: NSRange, utterance u: AVSpeechUtterance) {
        emit("speak_word", ["id": ids[ObjectIdentifier(u)] ?? "", "at": r.location, "len": r.length])
    }

    func speechSynthesizer(_ s: AVSpeechSynthesizer, didFinish u: AVSpeechUtterance) {
        emit("speak_end", ["id": ids.removeValue(forKey: ObjectIdentifier(u)) ?? "", "cancelled": false])
    }

    func speechSynthesizer(_ s: AVSpeechSynthesizer, didCancel u: AVSpeechUtterance) {
        emit("speak_end", ["id": ids.removeValue(forKey: ObjectIdentifier(u)) ?? "", "cancelled": true])
    }

    static func list() -> [[String: Any]] {
        AVSpeechSynthesisVoice.speechVoices()
            .filter { !$0.identifier.contains("speech.synthesis.voice") }
            .map {
                ["id": $0.identifier, "name": $0.name, "language": $0.language,
                 "quality": $0.quality == .premium ? "premium" : $0.quality == .enhanced ? "enhanced" : "default"]
            }
    }
}

// MARK: - accessibility

struct AXItem {
    let id: String
    let role: String
    let label: String
    let frame: CGRect
    let element: AXUIElement
}

enum AX {
    static var cache: [String: AXItem] = [:]

    static func attr(_ el: AXUIElement, _ name: String) -> CFTypeRef? {
        var v: CFTypeRef?
        return AXUIElementCopyAttributeValue(el, name as CFString, &v) == .success ? v : nil
    }

    static func string(_ el: AXUIElement, _ name: String) -> String? {
        guard let v = attr(el, name) else { return nil }
        if let s = v as? String { return s.trimmingCharacters(in: .whitespacesAndNewlines) }
        return nil
    }

    static func frame(_ el: AXUIElement) -> CGRect? {
        guard let p = attr(el, kAXPositionAttribute), let s = attr(el, kAXSizeAttribute),
              CFGetTypeID(p) == AXValueGetTypeID(), CFGetTypeID(s) == AXValueGetTypeID() else { return nil }
        var point = CGPoint.zero
        var size = CGSize.zero
        AXValueGetValue(p as! AXValue, .cgPoint, &point)
        AXValueGetValue(s as! AXValue, .cgSize, &size)
        return CGRect(origin: point, size: size)
    }

    static func children(_ el: AXUIElement) -> [AXUIElement] {
        (attr(el, kAXChildrenAttribute) as? [AXUIElement]) ?? []
    }

    static func label(_ el: AXUIElement, role: String) -> String {
        for key in [kAXTitleAttribute, kAXDescriptionAttribute, "AXPlaceholderValue", kAXHelpAttribute] {
            if let s = string(el, key), !s.isEmpty { return String(s.prefix(80)) }
        }
        if role != "AXTextArea", let s = string(el, kAXValueAttribute), !s.isEmpty { return String(s.prefix(80)) }
        return ""
    }

    static let interesting: Set<String> = [
        "AXButton", "AXLink", "AXCheckBox", "AXRadioButton", "AXPopUpButton", "AXMenuButton",
        "AXTextField", "AXTextArea", "AXComboBox", "AXDisclosureTriangle", "AXSlider", "AXTab",
        "AXMenuBarItem", "AXSegmentedControl", "AXIncrementor", "AXColorWell",
    ]

    static func frontApp() -> NSRunningApplication? { NSWorkspace.shared.frontmostApplication }

    /// Walk the frontmost app's focused window + menu bar and number the controls (`e1`, `e2`…).
    static func snapshot(limit: Int = 160, on screenRect: CGRect? = nil) -> [String: Any] {
        cache.removeAll()
        guard AXIsProcessTrusted(), let app = frontApp() else {
            return ["trusted": AXIsProcessTrusted(), "elements": [] as [Any]]
        }
        let root = AXUIElementCreateApplication(app.processIdentifier)
        AXUIElementSetMessagingTimeout(root, 0.4)
        var out: [[String: Any]] = []
        var visited = 0
        let deadline = Date().addingTimeInterval(0.6)
        var counter = 0

        func walk(_ el: AXUIElement, depth: Int) {
            if out.count >= limit || visited > 5000 || depth > 45 || Date() > deadline { return }
            visited += 1
            let role = string(el, kAXRoleAttribute) ?? ""
            if interesting.contains(role), let f = frame(el), f.width > 1, f.height > 1,
               screenRect.map({ $0.intersects(f) }) ?? true {
                let l = label(el, role: role)
                if !l.isEmpty || role == "AXTextField" || role == "AXTextArea" {
                    counter += 1
                    let id = "e\(counter)"
                    cache[id] = AXItem(id: id, role: role, label: l, frame: f, element: el)
                    out.append(["id": id, "role": role.replacingOccurrences(of: "AX", with: "").lowercased(),
                                "label": l, "frame": rectJSON(f)])
                }
            }
            if role == "AXMenuBar" {
                // Top-level menu titles only; open menus are not walked.
                for c in children(el) { walk(c, depth: 44) }
                return
            }
            for c in children(el) { walk(c, depth: depth + 1) }
        }

        let window = attr(root, kAXFocusedWindowAttribute).map { $0 as! AXUIElement }
            ?? (attr(root, kAXMainWindowAttribute).map { $0 as! AXUIElement })
        var title = ""
        if let w = window {
            title = string(w, kAXTitleAttribute) ?? ""
            walk(w, depth: 0)
        }
        if let bar = attr(root, kAXMenuBarAttribute) { walk(bar as! AXUIElement, depth: 0) }
        return ["trusted": true, "window": title, "elements": out]
    }

    static func find(_ target: String) -> AXItem? {
        if let hit = cache[target] { return hit }
        if cache.isEmpty { _ = snapshot() }
        let t = target.lowercased()
        let items = Array(cache.values)
        return items.first { $0.label.lowercased() == t }
            ?? items.first { $0.label.lowercased().hasPrefix(t) }
            ?? items.first { $0.label.lowercased().contains(t) }
            ?? { _ = snapshot(); return cache.values.first { $0.label.lowercased().contains(t) } }()
    }

    static func press(_ target: String) throws -> [String: Any] {
        guard AXIsProcessTrusted() else { throw HelperError("Accessibility access is off.") }
        guard let item = find(target) else { throw HelperError("I couldn't find \"\(target)\" on screen.") }
        let r = AXUIElementPerformAction(item.element, kAXPressAction as CFString)
        if r != .success {
            // Some controls only take focus + confirm.
            AXUIElementSetAttributeValue(item.element, kAXFocusedAttribute as CFString, kCFBooleanTrue)
            if AXUIElementPerformAction(item.element, kAXConfirmAction as CFString) != .success {
                throw HelperError("\"\(item.label)\" didn't respond to a press (AX error \(r.rawValue)).")
            }
        }
        return ["pressed": item.label, "role": item.role, "frame": rectJSON(item.frame)]
    }

    static func focus(_ target: String) throws -> [String: Any] {
        guard let item = find(target) else { throw HelperError("I couldn't find \"\(target)\".") }
        AXUIElementSetAttributeValue(item.element, kAXFocusedAttribute as CFString, kCFBooleanTrue)
        return ["focused": item.label, "frame": rectJSON(item.frame)]
    }

    /// Open a menu path like "File > Export…" in the frontmost app.
    static func menu(_ path: String) throws -> [String: Any] {
        guard AXIsProcessTrusted(), let app = frontApp() else { throw HelperError("Accessibility access is off.") }
        let root = AXUIElementCreateApplication(app.processIdentifier)
        guard let barRef = attr(root, kAXMenuBarAttribute) else { throw HelperError("No menu bar.") }
        var current = barRef as! AXUIElement
        let parts = path.split(separator: ">").map { $0.trimmingCharacters(in: .whitespaces).lowercased() }
        for (i, part) in parts.enumerated() {
            var candidates = children(current)
            // Menu bar item → its AXMenu child holds the items.
            if candidates.count == 1, string(candidates[0], kAXRoleAttribute) == "AXMenu" {
                candidates = children(candidates[0])
            }
            let clean = { (s: String) in s.lowercased().replacingOccurrences(of: "…", with: "").replacingOccurrences(of: "...", with: "") }
            guard let next = candidates.first(where: { clean(string($0, kAXTitleAttribute) ?? "") == clean(part) })
                ?? candidates.first(where: { clean(string($0, kAXTitleAttribute) ?? "").hasPrefix(clean(part)) }) else {
                throw HelperError("No menu item \"\(part)\".")
            }
            AXUIElementPerformAction(next, kAXPressAction as CFString)
            if i < parts.count - 1 {
                Thread.sleep(forTimeInterval: 0.12)
                current = next
            }
        }
        return ["menu": path]
    }
}

// MARK: - keyboard & text

enum Keys {
    static let codes: [String: CGKeyCode] = [
        "a": 0, "s": 1, "d": 2, "f": 3, "h": 4, "g": 5, "z": 6, "x": 7, "c": 8, "v": 9, "b": 11,
        "q": 12, "w": 13, "e": 14, "r": 15, "y": 16, "t": 17, "1": 18, "2": 19, "3": 20, "4": 21,
        "6": 22, "5": 23, "=": 24, "9": 25, "7": 26, "-": 27, "8": 28, "0": 29, "]": 30, "o": 31,
        "u": 32, "[": 33, "i": 34, "p": 35, "l": 37, "j": 38, "'": 39, "k": 40, ";": 41, "\\": 42,
        ",": 43, "/": 44, "n": 45, "m": 46, ".": 47, "`": 50,
        "return": 36, "enter": 36, "tab": 48, "space": 49, "delete": 51, "backspace": 51,
        "escape": 53, "esc": 53, "left": 123, "right": 124, "down": 125, "up": 126,
        "home": 115, "end": 119, "pageup": 116, "pagedown": 121, "forwarddelete": 117,
    ]

    static func press(_ combo: String) throws {
        guard AXIsProcessTrusted() else { throw HelperError("Accessibility access is off.") }
        var flags: CGEventFlags = []
        var key: CGKeyCode?
        for raw in combo.lowercased().split(separator: "+") {
            let p = raw.trimmingCharacters(in: .whitespaces)
            switch p {
            case "cmd", "command", "⌘": flags.insert(.maskCommand)
            case "shift", "⇧": flags.insert(.maskShift)
            case "opt", "option", "alt", "⌥": flags.insert(.maskAlternate)
            case "ctrl", "control", "⌃": flags.insert(.maskControl)
            default: key = codes[p]
            }
        }
        guard let code = key else { throw HelperError("Unknown key in \"\(combo)\".") }
        let src = CGEventSource(stateID: .hidSystemState)
        let down = CGEvent(keyboardEventSource: src, virtualKey: code, keyDown: true)
        let up = CGEvent(keyboardEventSource: src, virtualKey: code, keyDown: false)
        down?.flags = flags
        up?.flags = flags
        down?.post(tap: .cghidEventTap)
        up?.post(tap: .cghidEventTap)
    }

    /// Put text into the focused field: paste via the clipboard, then restore the clipboard.
    static func insert(_ text: String) throws -> [String: Any] {
        guard AXIsProcessTrusted() else { throw HelperError("Accessibility access is off.") }
        let pb = NSPasteboard.general
        let saved: [NSPasteboardItem] = (pb.pasteboardItems ?? []).map { item in
            let copy = NSPasteboardItem()
            for t in item.types { if let d = item.data(forType: t) { copy.setData(d, forType: t) } }
            return copy
        }
        pb.clearContents()
        pb.setString(text, forType: .string)
        try press("cmd+v")
        DispatchQueue.main.asyncAfter(deadline: .now() + 0.6) {
            pb.clearContents()
            if !saved.isEmpty { pb.writeObjects(saved) }
        }
        let app = NSWorkspace.shared.frontmostApplication?.localizedName ?? ""
        return ["inserted": text.count, "app": app]
    }
}

// MARK: - pointer actions (the buddy's hands)

enum Pointer {
    /// Click at a global point, then put the user's cursor back where it was.
    static func click(x: Double, y: Double, button: String, count: Int) throws -> [String: Any] {
        guard AXIsProcessTrusted() else { throw HelperError("Accessibility access is off.") }
        let src = CGEventSource(stateID: .hidSystemState)
        let original = CGEvent(source: nil)?.location ?? .zero
        let p = CGPoint(x: x, y: y)
        let (downT, upT, btn): (CGEventType, CGEventType, CGMouseButton) =
            button == "right" ? (.rightMouseDown, .rightMouseUp, .right) : (.leftMouseDown, .leftMouseUp, .left)
        CGEvent(mouseEventSource: src, mouseType: .mouseMoved, mouseCursorPosition: p, mouseButton: .left)?.post(tap: .cghidEventTap)
        usleep(30_000)
        for i in 1...max(1, min(count, 3)) {
            let down = CGEvent(mouseEventSource: src, mouseType: downT, mouseCursorPosition: p, mouseButton: btn)
            let up = CGEvent(mouseEventSource: src, mouseType: upT, mouseCursorPosition: p, mouseButton: btn)
            down?.setIntegerValueField(.mouseEventClickState, value: Int64(i))
            up?.setIntegerValueField(.mouseEventClickState, value: Int64(i))
            down?.post(tap: .cghidEventTap)
            usleep(18_000)
            up?.post(tap: .cghidEventTap)
            usleep(40_000)
        }
        usleep(60_000)
        CGWarpMouseCursorPosition(original)
        CGAssociateMouseAndMouseCursorPosition(1)
        return ["clicked": ["x": Int(x), "y": Int(y)], "button": button, "count": count]
    }

    static func drag(from a: CGPoint, to b: CGPoint) throws -> [String: Any] {
        guard AXIsProcessTrusted() else { throw HelperError("Accessibility access is off.") }
        let src = CGEventSource(stateID: .hidSystemState)
        let original = CGEvent(source: nil)?.location ?? .zero
        CGEvent(mouseEventSource: src, mouseType: .leftMouseDown, mouseCursorPosition: a, mouseButton: .left)?.post(tap: .cghidEventTap)
        let steps = 24
        for i in 1...steps {
            let t = Double(i) / Double(steps)
            let p = CGPoint(x: a.x + (b.x - a.x) * t, y: a.y + (b.y - a.y) * t)
            CGEvent(mouseEventSource: src, mouseType: .leftMouseDragged, mouseCursorPosition: p, mouseButton: .left)?.post(tap: .cghidEventTap)
            usleep(12_000)
        }
        CGEvent(mouseEventSource: src, mouseType: .leftMouseUp, mouseCursorPosition: b, mouseButton: .left)?.post(tap: .cghidEventTap)
        usleep(60_000)
        CGWarpMouseCursorPosition(original)
        CGAssociateMouseAndMouseCursorPosition(1)
        return ["dragged": ["from": ["x": Int(a.x), "y": Int(a.y)], "to": ["x": Int(b.x), "y": Int(b.y)]]]
    }

    /// What changed, for checking an action worked.
    static func afterState() -> [String: Any] {
        let app = NSWorkspace.shared.frontmostApplication
        let ax = AX.snapshot(limit: 60)
        var focused = ""
        let sys = AXUIElementCreateSystemWide()
        if let f = AX.attr(sys, kAXFocusedUIElementAttribute) {
            let el = f as! AXUIElement
            let role = AX.string(el, kAXRoleAttribute) ?? ""
            focused = "\(role.replacingOccurrences(of: "AX", with: "").lowercased()) \(AX.label(el, role: role))"
        }
        return ["app": app?.localizedName ?? "", "window": ax["window"] ?? "", "focused": focused,
                "controls": (ax["elements"] as? [[String: Any]] ?? []).prefix(25).map { "\($0["id"]!) \($0["role"]!) \($0["label"]!)" }]
    }
}

// MARK: - screen capture + OCR

enum Screen {
    static func cursor() -> CGPoint { CGEvent(source: nil)?.location ?? .zero }

    static func capture(path: String?, maxWidth: Int, ocr: Bool, done: @escaping (Result<[String: Any], Error>) -> Void) {
        if IsSecureEventInputEnabled() {
            done(.success(["secure": true]))
            return
        }
        // Never trigger the system prompt from a voice turn: only capture once it's allowed.
        guard CGPreflightScreenCaptureAccess() else {
            done(.failure(HelperError("Screen Recording is off for openclicky (Settings → Permissions).")))
            return
        }
        SCShareableContent.getExcludingDesktopWindows(false, onScreenWindowsOnly: true) { content, error in
            guard let content else {
                done(.failure(HelperError(error.map { "Screen capture unavailable: \($0.localizedDescription)" }
                    ?? "Screen Recording access is off.")))
                return
            }
            let mouse = cursor()
            guard let display = content.displays.first(where: { $0.frame.contains(mouse) }) ?? content.displays.first else {
                done(.failure(HelperError("No display.")))
                return
            }
            let mine: Set<pid_t> = [getpid(), getppid()]
            let excluded = content.applications.filter {
                mine.contains($0.processID) || $0.bundleIdentifier == "com.openclicky.app"
            }
            let filter = SCContentFilter(display: display, excludingApplications: excluded, exceptingWindows: [])
            let scale = CGFloat(filter.pointPixelScale)
            let cfg = SCStreamConfiguration()
            cfg.width = Int(CGFloat(display.width) * scale)
            cfg.height = Int(CGFloat(display.height) * scale)
            cfg.showsCursor = true
            cfg.captureResolution = .best
            SCScreenshotManager.captureImage(contentFilter: filter, configuration: cfg) { image, error in
                guard let image else {
                    done(.failure(HelperError("Capture failed: \(error?.localizedDescription ?? "unknown")")))
                    return
                }
                DispatchQueue.global(qos: .userInitiated).async {
                    var result: [String: Any] = [
                        "secure": false,
                        "display": rectJSON(display.frame),
                        "scale": Double(scale),
                        "cursor": ["x": Int(mouse.x), "y": Int(mouse.y)],
                    ]
                    if let path {
                        // 1 image pixel = 1 screen point, so positions read off the image are exact.
                        let (w, h) = write(image, to: path, maxWidth: min(maxWidth, Int(display.frame.width)))
                        result["path"] = path
                        result["image"] = ["w": w, "h": h]
                    }
                    if ocr { result["text"] = recognize(image, in: display.frame) }
                    let app = NSWorkspace.shared.frontmostApplication
                    result["app"] = app?.localizedName ?? ""
                    result["bundle"] = app?.bundleIdentifier ?? ""
                    DispatchQueue.main.async {
                        let ax = AX.snapshot(on: display.frame)
                        result["window"] = ax["window"] ?? ""
                        result["elements"] = ax["elements"] ?? []
                        result["ax_trusted"] = ax["trusted"] ?? false
                        done(.success(result))
                    }
                }
            }
        }
    }

    /// Downscale to `maxWidth` and write a JPEG; returns the written pixel size.
    static func write(_ image: CGImage, to path: String, maxWidth: Int) -> (Int, Int) {
        let ratio = min(1, CGFloat(maxWidth) / CGFloat(image.width))
        let w = Int(CGFloat(image.width) * ratio)
        let h = Int(CGFloat(image.height) * ratio)
        guard let ctx = CGContext(data: nil, width: w, height: h, bitsPerComponent: 8, bytesPerRow: 0,
                                  space: CGColorSpaceCreateDeviceRGB(),
                                  bitmapInfo: CGImageAlphaInfo.noneSkipLast.rawValue) else { return (0, 0) }
        ctx.interpolationQuality = .high
        ctx.draw(image, in: CGRect(x: 0, y: 0, width: w, height: h))
        guard let scaled = ctx.makeImage() else { return (0, 0) }
        let rep = NSBitmapImageRep(cgImage: scaled)
        if let data = rep.representation(using: .jpeg, properties: [.compressionFactor: 0.72]) {
            try? data.write(to: URL(fileURLWithPath: path))
        }
        return (w, h)
    }

    /// OCR in global screen points (top-left origin), reading order.
    static func recognize(_ image: CGImage, in frame: CGRect) -> [[String: Any]] {
        let req = VNRecognizeTextRequest()
        req.recognitionLevel = .accurate
        req.usesLanguageCorrection = true
        let handler = VNImageRequestHandler(cgImage: image, options: [:])
        try? handler.perform([req])
        var lines: [[String: Any]] = []
        for obs in req.results ?? [] {
            guard let top = obs.topCandidates(1).first, top.confidence > 0.3 else { continue }
            let b = obs.boundingBox
            let r = CGRect(x: frame.minX + b.minX * frame.width,
                           y: frame.minY + (1 - b.maxY) * frame.height,
                           width: b.width * frame.width, height: b.height * frame.height)
            lines.append(["text": top.string, "frame": rectJSON(r)])
        }
        lines.sort {
            let a = $0["frame"] as! [String: Int], b = $1["frame"] as! [String: Int]
            return abs(a["y"]! - b["y"]!) > 6 ? a["y"]! < b["y"]! : a["x"]! < b["x"]!
        }
        return Array(lines.prefix(400))
    }
}

// MARK: - HTML → PDF (CLI)

final class PDFMaker: NSObject, WKNavigationDelegate {
    let web: WKWebView
    let out: URL
    let page: CGSize
    let margin: CGFloat = 36

    /// Paper sizes in points; CSS pixels map 1:1 to points in WebKit's PDF output.
    init(input: URL, output: URL, paper: String) {
        page = paper.lowercased() == "a4" ? CGSize(width: 595, height: 842) : CGSize(width: 612, height: 792)
        web = WKWebView(frame: CGRect(x: 0, y: 0, width: page.width, height: page.height))
        out = output
        super.init()
        web.navigationDelegate = self
        web.loadFileURL(input, allowingReadAccessTo: input.deletingLastPathComponent())
    }

    func fail(_ msg: String) -> Never {
        FileHandle.standardError.write("\(msg)\n".data(using: .utf8)!)
        exit(1)
    }

    func webView(_ webView: WKWebView, didFinish navigation: WKNavigation!) {
        // Let fonts and images settle, then cut the page into real paper-sized sheets.
        DispatchQueue.main.asyncAfter(deadline: .now() + 0.5) {
            webView.evaluateJavaScript("Math.max(document.body.scrollHeight, document.documentElement.scrollHeight)") { h, _ in
                let height = max((h as? CGFloat) ?? self.page.height, 1)
                let sheets = Int(ceil(height / self.page.height - 0.02))
                let doc = PDFDocument()
                func render(_ i: Int) {
                    if i >= max(sheets, 1) {
                        guard doc.write(to: self.out) else { self.fail("write failed") }
                        print(self.out.path)
                        print("pages: \(doc.pageCount), size: \(Int(self.page.width))x\(Int(self.page.height)) pt")
                        exit(0)
                    }
                    let cfg = WKPDFConfiguration()
                    cfg.rect = CGRect(x: 0, y: CGFloat(i) * self.page.height, width: self.page.width, height: self.page.height)
                    webView.createPDF(configuration: cfg) { result in
                        switch result {
                        case .success(let data):
                            if let part = PDFDocument(data: data), let p = part.page(at: 0) {
                                doc.insert(p, at: doc.pageCount)
                            }
                            render(i + 1)
                        case .failure(let e):
                            self.fail("pdf failed: \(e)")
                        }
                    }
                }
                render(0)
            }
        }
    }

    func webView(_ webView: WKWebView, didFail navigation: WKNavigation!, withError error: Error) {
        fail("load failed: \(error)")
    }
}

// MARK: - playing synthesized speech (Kokoro wavs)

final class Player: NSObject, AVAudioPlayerDelegate {
    private struct Item { let id: String; let url: URL; let text: String }
    private var queue: [Item] = []
    private var current: AVAudioPlayer?
    private var currentItem: Item?
    private var words: [DispatchWorkItem] = []

    func enqueue(id: String, path: String, text: String) {
        queue.append(Item(id: id, url: URL(fileURLWithPath: path), text: text))
        if current == nil { next() }
    }

    private func next() {
        guard !queue.isEmpty else { current = nil; currentItem = nil; return }
        let item = queue.removeFirst()
        guard let p = try? AVAudioPlayer(contentsOf: item.url) else {
            emit("speak_end", ["id": item.id, "cancelled": true])
            next()
            return
        }
        p.delegate = self
        p.prepareToPlay()
        current = p
        currentItem = item
        p.play()
        emit("speak_start", ["id": item.id])
        scheduleWords(item, duration: p.duration)
    }

    /// Light up words in step with the audio: time is shared out by word length.
    private func scheduleWords(_ item: Item, duration: TimeInterval) {
        let ns = item.text as NSString
        var ranges: [NSRange] = []
        ns.enumerateSubstrings(in: NSRange(location: 0, length: ns.length), options: .byWords) { _, r, _, _ in ranges.append(r) }
        let total = max(1, ranges.reduce(0) { $0 + $1.length + 1 })
        var t = 0.06
        let usable = max(0.2, duration - 0.15)
        for r in ranges {
            let w = DispatchWorkItem { emit("speak_word", ["id": item.id, "at": r.location, "len": r.length]) }
            words.append(w)
            DispatchQueue.main.asyncAfter(deadline: .now() + t, execute: w)
            t += usable * Double(r.length + 1) / Double(total)
        }
    }

    func audioPlayerDidFinishPlaying(_ player: AVAudioPlayer, successfully flag: Bool) {
        if let item = currentItem {
            emit("speak_end", ["id": item.id, "cancelled": false])
            try? FileManager.default.removeItem(at: item.url)
        }
        words.removeAll()
        next()
    }

    func stop() {
        words.forEach { $0.cancel() }
        words.removeAll()
        current?.stop()
        if let item = currentItem { emit("speak_end", ["id": item.id, "cancelled": true]) }
        for item in queue { emit("speak_end", ["id": item.id, "cancelled": true]) }
        queue.removeAll()
        current = nil
        currentItem = nil
    }

    var playing: Bool { current != nil }
}

// MARK: - Calendar, Reminders, Contacts (EventKit / Contacts; permission is asked once, by Pixie)

enum PIM {
    static let store = EKEventStore()
    static let people = CNContactStore()

    static func ek(_ s: EKAuthorizationStatus) -> String {
        switch s {
        case .fullAccess: return "granted"
        case .writeOnly: return "write_only"
        case .denied: return "denied"
        case .restricted: return "restricted"
        case .notDetermined: return "not_determined"
        default: return "granted"
        }
    }

    static func status(_ kind: String) -> String {
        switch kind {
        case "calendar": return ek(EKEventStore.authorizationStatus(for: .event))
        case "reminders": return ek(EKEventStore.authorizationStatus(for: .reminder))
        case "contacts":
            switch CNContactStore.authorizationStatus(for: .contacts) {
            case .denied: return "denied"
            case .restricted: return "restricted"
            case .notDetermined: return "not_determined"
            default: return "granted"
            }
        default: return "unknown"
        }
    }

    static func request(_ kind: String, _ done: @escaping (String) -> Void) {
        let back: (Bool, Error?) -> Void = { _, _ in DispatchQueue.main.async { store.refreshSourcesIfNecessary(); done(status(kind)) } }
        switch kind {
        case "calendar": store.requestFullAccessToEvents(completion: back)
        case "reminders": store.requestFullAccessToReminders(completion: back)
        case "contacts": people.requestAccess(for: .contacts, completionHandler: back)
        default: done("unknown")
        }
    }

    static func need(_ kind: String) throws {
        let s = status(kind)
        if s != "granted" {
            let what = kind == "calendar" ? "Calendars" : kind == "reminders" ? "Reminders" : "Contacts"
            throw HelperError("Pixie can't use \(what) yet (\(s)). Turn it on in Settings → Apps, or System Settings → Privacy & Security → \(what).")
        }
    }

    static let isoOut: ISO8601DateFormatter = {
        let f = ISO8601DateFormatter()
        f.formatOptions = [.withInternetDateTime]
        f.timeZone = .current
        return f
    }()

    static func iso(_ d: Date?) -> String { d.map { isoOut.string(from: $0) } ?? "" }

    /// ISO 8601 with or without a zone ("2026-11-05T15:00", "2026-11-05"), local time when none.
    static func date(_ s: String?) -> Date? {
        guard let s = s?.trimmingCharacters(in: .whitespaces), !s.isEmpty else { return nil }
        for opts: ISO8601DateFormatter.Options in [[.withInternetDateTime], [.withInternetDateTime, .withFractionalSeconds]] {
            let f = ISO8601DateFormatter()
            f.formatOptions = opts
            if let d = f.date(from: s) { return d }
        }
        let df = DateFormatter()
        df.locale = Locale(identifier: "en_US_POSIX")
        df.timeZone = .current
        for fmt in ["yyyy-MM-dd'T'HH:mm:ss", "yyyy-MM-dd'T'HH:mm", "yyyy-MM-dd HH:mm", "yyyy-MM-dd"] {
            df.dateFormat = fmt
            if let d = df.date(from: s) { return d }
        }
        return nil
    }

    static func events(_ a: [String: Any]) throws -> [String: Any] {
        try need("calendar")
        let from = date(str(a, "from")) ?? Calendar.current.startOfDay(for: Date())
        let to = date(str(a, "to")) ?? from.addingTimeInterval(7 * 86400)
        let q = (str(a, "query") ?? "").lowercased()
        let limit = (a["limit"] as? Int) ?? 40
        let pred = store.predicateForEvents(withStart: from, end: to, calendars: nil)
        var evs = store.events(matching: pred).sorted { $0.startDate < $1.startDate }
        if !q.isEmpty {
            evs = evs.filter { e in [e.title, e.location, e.notes].contains { ($0 ?? "").lowercased().contains(q) } }
        }
        let list: [[String: Any]] = evs.prefix(limit).map { e in
            ["id": e.calendarItemIdentifier, "title": e.title ?? "", "start": iso(e.startDate), "end": iso(e.endDate),
             "all_day": e.isAllDay, "location": e.location ?? "", "calendar": e.calendar?.title ?? "",
             "notes": String((e.notes ?? "").prefix(300)), "attendees": (e.attendees ?? []).compactMap { $0.name }]
        }
        return ["from": iso(from), "to": iso(to), "count": evs.count, "events": list]
    }

    static func addEvent(_ a: [String: Any]) throws -> [String: Any] {
        try need("calendar")
        guard let title = str(a, "title"), !title.isEmpty else { throw HelperError("An event needs a title.") }
        guard let start = date(str(a, "start")) else { throw HelperError("start must be a date like 2026-11-05T15:00.") }
        let e = EKEvent(eventStore: store)
        e.title = title
        e.startDate = start
        e.isAllDay = (a["all_day"] as? Bool) ?? false
        e.endDate = date(str(a, "end")) ?? start.addingTimeInterval(e.isAllDay ? 86400 : 3600)
        if let l = str(a, "location"), !l.isEmpty { e.location = l }
        if let n = str(a, "notes"), !n.isEmpty { e.notes = n }
        let name = (str(a, "calendar") ?? "").lowercased()
        e.calendar = store.calendars(for: .event).first { $0.title.lowercased() == name && $0.allowsContentModifications }
            ?? store.defaultCalendarForNewEvents
        try store.save(e, span: .thisEvent, commit: true)
        return ["id": e.calendarItemIdentifier, "title": title, "start": iso(e.startDate), "end": iso(e.endDate), "calendar": e.calendar?.title ?? ""]
    }

    static func reminders(_ id: Any, _ a: [String: Any]) throws {
        try need("reminders")
        let name = (str(a, "list") ?? "").lowercased()
        let lists = store.calendars(for: .reminder)
        let cals = name.isEmpty ? nil : lists.filter { $0.title.lowercased() == name }
        let done = (a["done"] as? Bool) ?? false
        let limit = (a["limit"] as? Int) ?? 50
        let pred = done
            ? store.predicateForCompletedReminders(withCompletionDateStarting: Date().addingTimeInterval(-30 * 86400), ending: nil, calendars: cals)
            : store.predicateForIncompleteReminders(withDueDateStarting: nil, ending: nil, calendars: cals)
        store.fetchReminders(matching: pred) { rs in
            let items: [[String: Any]] = (rs ?? []).prefix(limit).map { r in
                ["id": r.calendarItemIdentifier, "title": r.title ?? "", "list": r.calendar?.title ?? "",
                 "due": iso(r.dueDateComponents?.date ?? r.dueDateComponents.flatMap { Calendar.current.date(from: $0) }),
                 "done": r.isCompleted, "notes": String((r.notes ?? "").prefix(200)), "priority": r.priority]
            }
            DispatchQueue.main.async { reply(id, ["lists": lists.map { $0.title }, "reminders": items]) }
        }
    }

    static func addReminder(_ a: [String: Any]) throws -> [String: Any] {
        try need("reminders")
        guard let title = str(a, "title"), !title.isEmpty else { throw HelperError("A reminder needs a title.") }
        let r = EKReminder(eventStore: store)
        r.title = title
        let name = (str(a, "list") ?? "").lowercased()
        r.calendar = store.calendars(for: .reminder).first { $0.title.lowercased() == name } ?? store.defaultCalendarForNewReminders()
        if let due = date(str(a, "due")) {
            r.dueDateComponents = Calendar.current.dateComponents([.year, .month, .day, .hour, .minute], from: due)
            r.addAlarm(EKAlarm(absoluteDate: due))
        }
        if let n = str(a, "notes"), !n.isEmpty { r.notes = n }
        try store.save(r, commit: true)
        return ["id": r.calendarItemIdentifier, "title": title, "list": r.calendar?.title ?? ""]
    }

    static func completeReminder(_ a: [String: Any]) throws -> [String: Any] {
        try need("reminders")
        guard let r = store.calendarItem(withIdentifier: str(a, "id") ?? "") as? EKReminder else {
            throw HelperError("No reminder with that id; list them first.")
        }
        r.isCompleted = true
        try store.save(r, commit: true)
        return ["id": r.calendarItemIdentifier, "title": r.title ?? "", "done": true]
    }

    static func findContacts(_ a: [String: Any]) throws -> [String: Any] {
        try need("contacts")
        let q = (str(a, "query") ?? "").trimmingCharacters(in: .whitespaces)
        guard !q.isEmpty else { throw HelperError("Say who to look for.") }
        let keys = [CNContactGivenNameKey, CNContactFamilyNameKey, CNContactOrganizationNameKey, CNContactJobTitleKey,
                    CNContactEmailAddressesKey, CNContactPhoneNumbersKey, CNContactBirthdayKey, CNContactPostalAddressesKey] as [CNKeyDescriptor]
        var found = try people.unifiedContacts(matching: CNContact.predicateForContacts(matchingName: q), keysToFetch: keys)
        if found.isEmpty && q.contains("@") {
            found = try people.unifiedContacts(matching: CNContact.predicateForContacts(matchingEmailAddress: q), keysToFetch: keys)
        }
        let list: [[String: Any]] = found.prefix((a["limit"] as? Int) ?? 10).map { c in
            var d: [String: Any] = [
                "name": "\(c.givenName) \(c.familyName)".trimmingCharacters(in: .whitespaces),
                "organization": c.organizationName, "job": c.jobTitle,
                "emails": c.emailAddresses.map { "\(CNLabeledValue<NSString>.localizedString(forLabel: $0.label ?? "")): \($0.value)" },
                "phones": c.phoneNumbers.map { "\(CNLabeledValue<CNPhoneNumber>.localizedString(forLabel: $0.label ?? "")): \($0.value.stringValue)" },
                "addresses": c.postalAddresses.map { CNPostalAddressFormatter.string(from: $0.value, style: .mailingAddress).replacingOccurrences(of: "\n", with: ", ") },
            ]
            if let b = c.birthday, let m = b.month, let day = b.day { d["birthday"] = b.year.map { "\($0)-\(m)-\(day)" } ?? "\(m)-\(day)" }
            return d
        }
        return ["count": found.count, "contacts": list]
    }
}

// MARK: - dispatcher

let listener = Listener()
let speaker = Speaker()
let player = Player()
var keep: [AnyObject] = []

func str(_ a: [String: Any], _ k: String) -> String? { a[k] as? String }

func handle(_ line: String) {
    guard let data = line.data(using: .utf8),
          let msg = try? JSONSerialization.jsonObject(with: data) as? [String: Any],
          let id = msg["id"], let cmd = msg["cmd"] as? String else { return }
    let args = msg["args"] as? [String: Any] ?? [:]
    do {
        switch cmd {
        case "ping":
            reply(id, ["pid": getpid(), "ax_trusted": AXIsProcessTrusted()])
        case "listen_start":
            listener.start(locale: str(args, "locale"), onDevice: (args["on_device"] as? Bool) ?? true, id: id)
        case "listen_stop":
            listener.stop(id: id)
        case "listen_cancel":
            listener.cancel()
            reply(id)
        case "speak":
            if let v = str(args, "voice") { speaker.voiceId = v.isEmpty ? nil : v }
            if let r = args["rate"] as? Double { speaker.rate = Float(r) }
            speaker.speak(str(args, "text") ?? "", utterance: str(args, "utterance") ?? "")
            reply(id)
        case "speak_stop":
            speaker.stop()
            player.stop()
            reply(id)
        case "play":
            player.enqueue(id: str(args, "utterance") ?? "", path: str(args, "path") ?? "", text: str(args, "text") ?? "")
            reply(id)
        case "voices":
            let lang = Locale.preferredLanguages.first ?? "en-US"
            reply(id, ["voices": Speaker.list(), "default": Speaker.bestVoice(language: lang)?.identifier ?? ""])
        case "capture":
            Screen.capture(path: str(args, "path"), maxWidth: (args["max_width"] as? Int) ?? 1440,
                           ocr: (args["ocr"] as? Bool) ?? true) { result in
                switch result {
                case .success(let r): reply(id, r)
                case .failure(let e): fail(id, "\(e)")
                }
            }
        case "cursor":
            let p = Screen.cursor()
            reply(id, ["x": Int(p.x), "y": Int(p.y)])
        case "ax_snapshot":
            reply(id, AX.snapshot())
        case "click_at":
            var r = try Pointer.click(x: args["x"] as? Double ?? 0, y: args["y"] as? Double ?? 0,
                                      button: str(args, "button") ?? "left", count: args["count"] as? Int ?? 1)
            let id2 = id
            DispatchQueue.main.asyncAfter(deadline: .now() + 0.45) {
                r["after"] = Pointer.afterState()
                reply(id2, r)
            }
        case "drag":
            var r = try Pointer.drag(from: CGPoint(x: args["x"] as? Double ?? 0, y: args["y"] as? Double ?? 0),
                                     to: CGPoint(x: args["to_x"] as? Double ?? 0, y: args["to_y"] as? Double ?? 0))
            r["after"] = Pointer.afterState()
            reply(id, r)
        case "insert_text":
            reply(id, try Keys.insert(str(args, "text") ?? ""))
        case "mac_step":
            let action = str(args, "action") ?? ""
            let target = str(args, "target") ?? ""
            switch action {
            case "press", "click": reply(id, try AX.press(target))
            case "focus": reply(id, try AX.focus(target))
            case "menu": reply(id, try AX.menu(target))
            case "type": reply(id, try Keys.insert(target))
            case "key":
                try Keys.press(target)
                reply(id, ["key": target])
            case "find":
                guard let item = AX.find(target) else { throw HelperError("I couldn't find \"\(target)\".") }
                reply(id, ["label": item.label, "role": item.role, "frame": rectJSON(item.frame)])
            case "open":
                try openTarget(target)
                reply(id, ["opened": target])
            case "scroll":
                let dy: Int32 = target.lowercased().contains("up") ? 8 : -8
                CGEvent(scrollWheelEvent2Source: nil, units: .line, wheelCount: 1, wheel1: dy, wheel2: 0, wheel3: 0)?
                    .post(tap: .cghidEventTap)
                reply(id, ["scrolled": target])
            default:
                throw HelperError("Unknown mac_step action \"\(action)\".")
            }
        case "open":
            try openTarget(str(args, "target") ?? "")
            reply(id)
        case "pim_status":
            reply(id, ["calendar": PIM.status("calendar"), "reminders": PIM.status("reminders"), "contacts": PIM.status("contacts")])
        case "pim_request":
            PIM.request(str(args, "kind") ?? "") { s in reply(id, ["status": s]) }
        case "calendar_events":
            reply(id, try PIM.events(args))
        case "calendar_add":
            reply(id, try PIM.addEvent(args))
        case "reminders_list":
            try PIM.reminders(id, args)
        case "reminder_add":
            reply(id, try PIM.addReminder(args))
        case "reminder_done":
            reply(id, try PIM.completeReminder(args))
        case "contacts_find":
            reply(id, try PIM.findContacts(args))
        case "frontmost":
            let app = NSWorkspace.shared.frontmostApplication
            reply(id, ["app": app?.localizedName ?? "", "bundle": app?.bundleIdentifier ?? "",
                       "pid": app?.processIdentifier ?? 0, "secure": IsSecureEventInputEnabled()])
        default:
            fail(id, "unknown command \(cmd)")
        }
    } catch {
        fail(id, "\(error)")
    }
}

func openTarget(_ target: String) throws {
    let t = target.trimmingCharacters(in: .whitespaces)
    guard !t.isEmpty else { throw HelperError("Nothing to open.") }
    if let url = URL(string: t), let scheme = url.scheme, scheme.count > 1 {
        NSWorkspace.shared.open(url)
        return
    }
    let path = (t as NSString).expandingTildeInPath
    if FileManager.default.fileExists(atPath: path) {
        NSWorkspace.shared.open(URL(fileURLWithPath: path))
        return
    }
    let p = Process()
    p.executableURL = URL(fileURLWithPath: "/usr/bin/open")
    p.arguments = ["-a", t]
    try p.run()
    p.waitUntilExit()
    if p.terminationStatus != 0 { throw HelperError("I couldn't find an app called \"\(t)\".") }
}

// MARK: - main

let app = NSApplication.shared
app.setActivationPolicy(.prohibited)
let argv = CommandLine.arguments

if argv.count >= 4 && argv[1] == "pdf" {
    let input = URL(fileURLWithPath: (argv[2] as NSString).expandingTildeInPath)
    let output = URL(fileURLWithPath: (argv[3] as NSString).expandingTildeInPath)
    let paper = argv.count >= 5 ? argv[4] : "letter"
    keep.append(PDFMaker(input: input, output: output, paper: paper))
    DispatchQueue.main.asyncAfter(deadline: .now() + 30) {
        FileHandle.standardError.write("pdf timed out\n".data(using: .utf8)!)
        exit(1)
    }
    app.run()
} else if argv.count >= 3 && argv[1] == "text" {
    // Plain text of a PDF, Word, RTF, HTML or Pages file.
    let url = URL(fileURLWithPath: (argv[2] as NSString).expandingTildeInPath)
    if url.pathExtension.lowercased() == "pdf" {
        guard let doc = PDFDocument(url: url) else {
            FileHandle.standardError.write("can't open pdf\n".data(using: .utf8)!)
            exit(1)
        }
        for i in 0..<doc.pageCount {
            if let t = doc.page(at: i)?.string { print("--- page \(i + 1) ---\n\(t)") }
        }
        exit(0)
    }
    if let attr = try? NSAttributedString(url: url, options: [:], documentAttributes: nil) {
        print(attr.string)
        exit(0)
    }
    FileHandle.standardError.write("unsupported file\n".data(using: .utf8)!)
    exit(1)
} else if argv.count >= 2 && argv[1] == "screen-check" {
    // A fresh process sees the current Screen Recording grant (the app's own check is cached).
    // Preflight never shows a prompt; calling ScreenCaptureKit here would.
    exit(CGPreflightScreenCaptureAccess() ? 0 : 1)
} else if argv.count >= 2 && argv[1] == "voices" {
    for v in Speaker.list() { print("\(v["quality"]!)\t\(v["language"]!)\t\(v["name"]!)\t\(v["id"]!)") }
    exit(0)
} else {
    setvbuf(stdout, nil, _IONBF, 0)
    signal(SIGPIPE, SIG_IGN)
    Thread.detachNewThread {
        while let line = readLine(strippingNewline: true) {
            DispatchQueue.main.async { handle(line) }
        }
        exit(0) // parent went away
    }
    emit("ready", ["pid": getpid()])
    app.run()
}
