// 系統 WKWebView（與 Tauri macOS 同一套）重現 ⋯ 選單鍵盤行為：事件以 NSEvent 直送本視窗，不碰全域輸入
import AppKit
import WebKit

final class KeyWindow: NSWindow {
  override var canBecomeKey: Bool { true }
  override var canBecomeMain: Bool { true }
}

let url = URL(string: CommandLine.arguments.count > 1 ? CommandLine.arguments[1] : "http://localhost:5199/")!
let keysArg = CommandLine.arguments.count > 2 ? CommandLine.arguments[2] : "Down,Down,Up,Right,Up,Right,Escape"

let app = NSApplication.shared
app.setActivationPolicy(.accessory)
let W: CGFloat = 800, H: CGFloat = 600
let win = KeyWindow(contentRect: NSRect(x: -3000, y: -3000, width: W, height: H), styleMask: [.borderless], backing: .buffered, defer: false)
let web = WKWebView(frame: NSRect(x: 0, y: 0, width: W, height: H))
win.contentView = web
let previous = NSWorkspace.shared.frontmostApplication
NSApp.activate(ignoringOtherApps: true)
win.makeKeyAndOrderFront(nil)
win.makeFirstResponder(web)

let state = """
(() => { const a = document.activeElement; const fv = document.querySelector(':focus-visible');
const log = (window.__log || []).splice(0);
return JSON.stringify({ hasFocus: document.hasFocus(), active: a === document.body ? 'BODY' : (a && (a.textContent || a.tagName)),
focusVisible: fv ? (fv.textContent || fv.tagName) : null, activeMatchesFocus: !!(a && a.matches(':focus')), bg: a && a.getAttribute && a.getAttribute('role')==='menuitem' ? getComputedStyle(a).backgroundColor : null, menuOpen: !!document.querySelector('[role=menu]'), log }); })()
"""

var shotN = 0
func dump(_ tag: String, _ next: @escaping () -> Void) {
  web.evaluateJavaScript(state) { r, e in
    print("[\(tag)]", r ?? e ?? "nil")
    web.takeSnapshot(with: nil) { img, _ in
      if let img, let tiff = img.tiffRepresentation, let rep = NSBitmapImageRep(data: tiff) {
        try? rep.representation(using: .png, properties: [:])?.write(to: URL(fileURLWithPath: "shot-\(shotN)-\(tag.replacingOccurrences(of: " ", with: "_")).png"))
      }
      shotN += 1
      next()
    }
  }
}

func after(_ s: Double, _ f: @escaping () -> Void) { DispatchQueue.main.asyncAfter(deadline: .now() + s, execute: f) }

func mouse(_ type: NSEvent.EventType, _ p: NSPoint) {
  let ev = NSEvent.mouseEvent(with: type, location: p, modifierFlags: [], timestamp: ProcessInfo.processInfo.systemUptime,
                              windowNumber: win.windowNumber, context: nil, eventNumber: 0, clickCount: 1, pressure: type == .leftMouseDown ? 1 : 0)!
  win.sendEvent(ev)
}

let keyTable: [String: (UInt16, Int)] = [
  "Up": (126, NSUpArrowFunctionKey), "Down": (125, NSDownArrowFunctionKey),
  "Left": (123, NSLeftArrowFunctionKey), "Right": (124, NSRightArrowFunctionKey),
  "Escape": (53, 0x1b), "Tab": (48, 0x09),
]

func key(_ name: String) {
  let (code, ch) = keyTable[name]!
  let s = String(Character(UnicodeScalar(ch)!))
  let flags: NSEvent.ModifierFlags = ch >= 0xF700 ? [.numericPad, .function] : []
  for t in [NSEvent.EventType.keyDown, .keyUp] {
    let ev = NSEvent.keyEvent(with: t, location: .zero, modifierFlags: flags, timestamp: ProcessInfo.processInfo.systemUptime,
                              windowNumber: win.windowNumber, context: nil, characters: s, charactersIgnoringModifiers: s,
                              isARepeat: false, keyCode: code)!
    win.sendEvent(ev)
  }
}

var started = false
final class Nav: NSObject, WKNavigationDelegate {
  func webView(_ webView: WKWebView, didFinish navigation: WKNavigation!) {
    if started { return }
    started = true
    after(1.5) { start() }
  }
}
let nav = Nav()
web.navigationDelegate = nav
web.load(URLRequest(url: url))

func clickAt(_ sel: String, _ next: @escaping () -> Void) {
  let js = "(() => { const el = document.querySelector(\(String(reflecting: sel))); if (!el) return null; const r = el.getBoundingClientRect(); return [r.x + r.width/2, r.y + r.height/2]; })()"
  web.evaluateJavaScript(js) { r, e in
    guard let xy = r as? [Double] else { print("no element", sel, e ?? ""); exit(1) }
    let p = NSPoint(x: xy[0], y: Double(H) - xy[1])
    mouse(.leftMouseDown, p)
    mouse(.leftMouseUp, p)
    next()
  }
}

func start() {
  let steps = keysArg.split(separator: ",").map(String.init)
  dump("start") { runKeys(steps.contains { $0.hasPrefix("click:") } ? steps : ["click:[aria-haspopup=menu]"] + steps) }
}

func runKeys(_ rest: [String]) {
  guard let k = rest.first else { previous?.activate(); after(0.2) { exit(0) }; return }
  let go = { after(0.4) { dump(k) { runKeys(Array(rest.dropFirst())) } } }
  if k.hasPrefix("click:") { clickAt(String(k.dropFirst(6))) { go() } } else { key(k); go() }
}

after(60) { print("timeout"); exit(2) }
app.run()
