import AppKit
import SwiftUI

/// What the canvas's NSView hears: the pointer for Rust, the pan and zoom it does itself, the keys for Rust.
enum CanvasInputEvent {
    case pointer(phase: String, at: CGPoint, flags: NSEvent.ModifierFlags, clicks: Int, middle: Bool)
    case pan(dx: CGFloat, dy: CGFloat)
    case zoom(factor: CGFloat, at: CGPoint)
    case key(name: String, flags: NSEvent.ModifierFlags)
}

/// SwiftUI's gestures have no hover, click count, wheel or modifiers on press: the canvas listens as an NSView. The one
/// under everything (`base`) hears the hover; a card's own piece (its title strip, a note) sends its presses the same way,
/// in the canvas's points, so a card in front takes them from the ones behind it.
struct CanvasInput: NSViewRepresentable {
    let handTool: Bool
    var base = false
    /// The wheel over a note's text: true when the note scrolled (the canvas leaves it alone).
    var onWheel: ((CGFloat) -> Bool)? = nil
    let onEvent: (CanvasInputEvent) -> Void

    func makeNSView(context: Context) -> CanvasInputView {
        let view = CanvasInputView()
        if base { CanvasInputView.base = view }
        return view
    }

    func updateNSView(_ view: CanvasInputView, context: Context) {
        view.handTool = handTool
        view.onEvent = onEvent
        view.onWheel = onWheel
    }
}

final class CanvasInputView: NSView {
    /// The canvas's own: every piece says where the pointer is in its points.
    // ponytail: one canvas per process
    static weak var base: CanvasInputView?
    /// Where the last press on the canvas was (its points): a note opened by it puts the caret there.
    static var pressed: CGPoint?
    /// The enlarged tree's frame (the canvas's points): the pinch and Cmd + wheel over it zoom it, not the canvas, even
    /// over its own view (canvas/wheel.rs).
    static var liftedTree: CGRect?
    private var monitor: Any?
    var onEvent: ((CanvasInputEvent) -> Void)?
    var onWheel: ((CGFloat) -> Bool)?
    var handTool = false
    private var spaceDown = false
    private var panning = false
    private var last = CGPoint.zero

    override var isFlipped: Bool { true }
    override var acceptsFirstResponder: Bool { true }

    override func updateTrackingAreas() {
        super.updateTrackingAreas()
        trackingAreas.forEach(removeTrackingArea)
        guard Self.base === self else { return }
        addTrackingArea(NSTrackingArea(rect: .zero, options: [.mouseMoved, .activeInKeyWindow, .inVisibleRect], owner: self))
    }

    private func at(_ event: NSEvent) -> CGPoint {
        (Self.base ?? self).convert(event.locationInWindow, from: nil)
    }

    override func viewDidMoveToWindow() {
        super.viewDidMoveToWindow()
        if let monitor { NSEvent.removeMonitor(monitor) }
        monitor = nil
        guard Self.base === self, window != nil else { return }
        monitor = NSEvent.addLocalMonitorForEvents(matching: [.scrollWheel, .magnify]) { [weak self] event in
            let taken = MainActor.assumeIsolated { self?.zoomTree(event) ?? false }
            return taken ? nil : event
        }
    }

    /// The pinch or Cmd + wheel over the enlarged tree: its own zoom (`ui.canvas.lifted_zoom`), true when taken.
    private func zoomTree(_ event: NSEvent) -> Bool {
        guard event.window === window, let tree = Self.liftedTree, tree.contains(convert(event.locationInWindow, from: nil)) else { return false }
        let factor: CGFloat
        if event.type == .magnify {
            factor = 1 + event.magnification
        } else if event.modifierFlags.contains(.command) {
            factor = exp(event.scrollingDeltaY * (event.hasPreciseScrollingDeltas ? 1 : 10) * 0.01)
        } else {
            return false
        }
        MagoStore.shared.send("ui.canvas.lifted_zoom", ["factor": Double(factor)])
        return true
    }

    private func pointer(_ phase: String, _ event: NSEvent, middle: Bool = false) {
        onEvent?(.pointer(phase: phase, at: at(event), flags: event.modifierFlags, clicks: event.clickCount, middle: middle))
    }

    private func startPan(_ event: NSEvent) {
        panning = true
        last = at(event)
        NSCursor.closedHand.push()
    }

    private func pan(_ event: NSEvent) {
        let point = at(event)
        onEvent?(.pan(dx: point.x - last.x, dy: point.y - last.y))
        last = point
    }

    private func endPan() {
        panning = false
        NSCursor.pop()
    }

    override func mouseDown(with event: NSEvent) {
        window?.makeFirstResponder(self)
        Self.pressed = at(event)
        if spaceDown || handTool { startPan(event) } else { pointer("down", event) }
    }

    override func mouseDragged(with event: NSEvent) {
        if panning { pan(event) } else { pointer("move", event) }
    }

    override func mouseUp(with event: NSEvent) {
        if panning { endPan() } else { pointer("up", event) }
    }

    override func mouseMoved(with event: NSEvent) {
        if !panning { pointer("move", event) }
    }

    // the middle button pans, as egui's canvas
    override func otherMouseDown(with event: NSEvent) { startPan(event) }
    override func otherMouseDragged(with event: NSEvent) { if panning { pan(event) } }
    override func otherMouseUp(with event: NSEvent) { if panning { endPan() } }

    override func scrollWheel(with event: NSEvent) {
        // a mouse wheel's notch is a line: about 10 points
        let scale: CGFloat = event.hasPreciseScrollingDeltas ? 1 : 10
        if !event.modifierFlags.contains(.command), let onWheel, onWheel(event.scrollingDeltaY * scale) { return }
        if event.modifierFlags.contains(.command) {
            // ponytail: an exponential step per point of scroll; egui's canvas adds Prefs.zoom_step per notch
            onEvent?(.zoom(factor: exp(event.scrollingDeltaY * scale * 0.01), at: at(event)))
        } else {
            onEvent?(.pan(dx: event.scrollingDeltaX * scale, dy: event.scrollingDeltaY * scale))
        }
    }

    override func magnify(with event: NSEvent) {
        onEvent?(.zoom(factor: 1 + event.magnification, at: at(event)))
    }

    /// egui's name for the key (`Key::from_name`), or nil.
    private func keyName(_ event: NSEvent) -> String? {
        switch event.keyCode {
        case 51: return "Backspace"
        case 117: return "Delete"
        case 53: return "Escape"
        case 36, 76: return "Enter"
        case 123: return "ArrowLeft"
        case 124: return "ArrowRight"
        case 125: return "ArrowDown"
        case 126: return "ArrowUp"
        default:
            guard let characters = event.charactersIgnoringModifiers?.uppercased(), characters.count == 1,
                  characters.first?.isLetter == true || characters.first?.isNumber == true else { return nil }
            return characters
        }
    }

    override func keyDown(with event: NSEvent) {
        if event.keyCode == 49 {
            spaceDown = true
            return
        }
        if let name = keyName(event) { onEvent?(.key(name: name, flags: event.modifierFlags)) }
    }

    override func keyUp(with event: NSEvent) {
        if event.keyCode == 49 { spaceDown = false }
    }

    // Cmd+Z, Cmd+A, Cmd+D and Cmd+Y belong to the canvas while it has the keys, not to the menu
    override func performKeyEquivalent(with event: NSEvent) -> Bool {
        guard window?.firstResponder === self, event.modifierFlags.contains(.command),
              let name = keyName(event), ["Z", "A", "D", "Y"].contains(name) else { return super.performKeyEquivalent(with: event) }
        onEvent?(.key(name: name, flags: event.modifierFlags))
        return true
    }
}
