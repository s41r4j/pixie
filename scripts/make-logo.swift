// Renders the openclicky mark — a literal `:)` — into the icon slots.
//   xcrun swift scripts/make-logo.swift
// Writes resources/icons/{app-icon-1024,tray-template,ext-16,ext-32,ext-48,ext-128}.png
import AppKit
import CoreText

let root = URL(fileURLWithPath: FileManager.default.currentDirectoryPath)
let fonts = root.appendingPathComponent("resources/fonts")
CTFontManagerRegisterFontsForURL(fonts.appendingPathComponent("Barlow-SemiBold.ttf") as CFURL, .process, nil)
let icons = root.appendingPathComponent("resources/icons")

func font(_ size: CGFloat) -> NSFont {
    NSFont(name: "Barlow-SemiBold", size: size) ?? NSFont.systemFont(ofSize: size, weight: .bold)
}

func png(_ size: Int, _ draw: (CGContext, CGFloat) -> Void) -> Data {
    let rep = NSBitmapImageRep(bitmapDataPlanes: nil, pixelsWide: size, pixelsHigh: size, bitsPerSample: 8,
                               samplesPerPixel: 4, hasAlpha: true, isPlanar: false, colorSpaceName: .deviceRGB,
                               bytesPerRow: 0, bitsPerPixel: 0)!
    NSGraphicsContext.saveGraphicsState()
    NSGraphicsContext.current = NSGraphicsContext(bitmapImageRep: rep)
    let ctx = NSGraphicsContext.current!.cgContext
    draw(ctx, CGFloat(size))
    NSGraphicsContext.restoreGraphicsState()
    return rep.representation(using: .png, properties: [:])!
}

/// Outline of `:)` as a path, centred in `rect`.
func smilePath(in rect: CGRect, size: CGFloat) -> CGPath {
    let attr = NSAttributedString(string: ":)", attributes: [.font: font(size), .kern: -size * 0.02])
    let line = CTLineCreateWithAttributedString(attr)
    let path = CGMutablePath()
    for run in CTLineGetGlyphRuns(line) as! [CTRun] {
        let attrs = CTRunGetAttributes(run) as NSDictionary
        let f = attrs[kCTFontAttributeName as String] as! CTFont
        let n = CTRunGetGlyphCount(run)
        var glyphs = [CGGlyph](repeating: 0, count: n)
        var pos = [CGPoint](repeating: .zero, count: n)
        CTRunGetGlyphs(run, CFRange(location: 0, length: n), &glyphs)
        CTRunGetPositions(run, CFRange(location: 0, length: n), &pos)
        for i in 0..<n {
            if let g = CTFontCreatePathForGlyph(f, glyphs[i], nil) {
                path.addPath(g, transform: CGAffineTransform(translationX: pos[i].x, y: pos[i].y))
            }
        }
    }
    let b = path.boundingBoxOfPath
    var t = CGAffineTransform(translationX: rect.midX - b.midX, y: rect.midY - b.midY)
    return path.copy(using: &t)!
}

/// `:)` centred in `rect`, filled with a gradient (or a flat colour), optional glow.
func smile(_ ctx: CGContext, in rect: CGRect, size: CGFloat, colors: [NSColor], glow: NSColor?) {
    let path = smilePath(in: rect, size: size)
    if let glow {
        ctx.saveGState()
        ctx.setShadow(offset: .zero, blur: size * 0.16, color: glow.withAlphaComponent(0.85).cgColor)
        ctx.addPath(path)
        ctx.setFillColor(colors.last!.cgColor)
        ctx.fillPath()
        ctx.restoreGState()
    }
    ctx.saveGState()
    ctx.addPath(path)
    ctx.clip()
    if colors.count == 1 {
        ctx.setFillColor(colors[0].cgColor)
        ctx.fill(rect)
    } else {
        let b = path.boundingBoxOfPath
        let g = CGGradient(colorsSpace: CGColorSpaceCreateDeviceRGB(), colors: colors.map(\.cgColor) as CFArray, locations: nil)!
        ctx.drawLinearGradient(g, start: CGPoint(x: b.minX, y: b.maxY), end: CGPoint(x: b.maxX, y: b.minY), options: [])
    }
    ctx.restoreGState()
}

let sky = NSColor(srgbRed: 0x5A / 255, green: 0xA8 / 255, blue: 0xF5 / 255, alpha: 1)
let focus = NSColor(srgbRed: 0x8E / 255, green: 0xC5 / 255, blue: 0xFF / 255, alpha: 1)
let white = NSColor(srgbRed: 0xEC / 255, green: 0xEC / 255, blue: 0xEF / 255, alpha: 1)
let rim = NSColor(srgbRed: 0x22 / 255, green: 0x18 / 255, blue: 0x98 / 255, alpha: 1)

func appIcon(_ n: Int) -> Data {
    png(n) { ctx, s in
        // macOS icon grid: 824/1024 rounded square, ~22.5% corner radius.
        let inset = s * 100 / 1024
        let r = CGRect(x: inset, y: inset, width: s - 2 * inset, height: s - 2 * inset)
        let path = CGPath(roundedRect: r, cornerWidth: r.width * 0.225, cornerHeight: r.width * 0.225, transform: nil)
        ctx.saveGState()
        ctx.setShadow(offset: CGSize(width: 0, height: -s * 0.01), blur: s * 0.03, color: NSColor(white: 0, alpha: 0.45).cgColor)
        ctx.addPath(path)
        ctx.setFillColor(NSColor(white: 0.06, alpha: 1).cgColor)
        ctx.fillPath()
        ctx.restoreGState()
        ctx.saveGState()
        ctx.addPath(path)
        ctx.clip()
        let bg = CGGradient(colorsSpace: CGColorSpaceCreateDeviceRGB(),
                            colors: [NSColor(srgbRed: 0.10, green: 0.09, blue: 0.20, alpha: 1).cgColor,
                                     NSColor(srgbRed: 0.04, green: 0.04, blue: 0.05, alpha: 1).cgColor] as CFArray,
                            locations: [0, 1])!
        ctx.drawRadialGradient(bg, startCenter: CGPoint(x: s / 2, y: s * 0.78), startRadius: 0,
                               endCenter: CGPoint(x: s / 2, y: s * 0.5), endRadius: s * 0.62, options: [.drawsAfterEndLocation])
        // Hairline rim.
        ctx.addPath(path)
        ctx.setStrokeColor(NSColor(white: 1, alpha: 0.08).cgColor)
        ctx.setLineWidth(s * 0.004)
        ctx.strokePath()
        ctx.restoreGState()
        smile(ctx, in: r, size: s * 0.46, colors: [NSColor.white, focus, sky], glow: rim.blended(withFraction: 0.4, of: sky))
    }
}

try! appIcon(1024).write(to: icons.appendingPathComponent("app-icon-1024.png"))
// Menu bar: template image (black on transparent); macOS tints it.
try! png(44) { ctx, s in
    smile(ctx, in: CGRect(x: 0, y: 0, width: s, height: s), size: s * 0.78, colors: [NSColor.black], glow: nil)
}.write(to: icons.appendingPathComponent("tray-template.png"))
for n in [16, 32, 48, 128] {
    try! png(n) { ctx, s in
        let r = CGRect(x: 0, y: 0, width: s, height: s)
        let path = CGPath(roundedRect: r.insetBy(dx: s * 0.04, dy: s * 0.04), cornerWidth: s * 0.24, cornerHeight: s * 0.24, transform: nil)
        ctx.addPath(path)
        ctx.setFillColor(NSColor(white: 0.07, alpha: 1).cgColor)
        ctx.fillPath()
        smile(ctx, in: r, size: s * 0.58, colors: n < 32 ? [focus] : [white, focus, sky], glow: nil)
    }.write(to: icons.appendingPathComponent("ext-\(n).png"))
}
print("icons written to \(icons.path)")
