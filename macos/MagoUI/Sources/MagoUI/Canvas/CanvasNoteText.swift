import AppKit
import SwiftUI

// A note's text as egui writes it (render/markdown.rs, notes/editor.rs): Hack, each run as `markdown::live_runs` says
// (size, line, faded, out of sight, bold, italic, struck, linked, a code's ground), and over the hidden marks the bullet,
// the task's box, the quote's bar or the rule. The resting note and the one being typed share it: at rest Rust sends the
// runs without a caret (every mark hidden), typed with the caret (the marks of what it is on show faint).

/// The notes' face: Hack, egui's monospace.
func noteFont(_ size: CGFloat) -> NSFont {
    NSFont(name: "Hack-Regular", size: size) ?? NSFont(name: "Hack", size: size) ?? .monospacedSystemFont(ofSize: size, weight: .regular)
}

/// `rgba` with its alpha times `share` (egui's gamma_multiply on a premultiplied color).
func noteColor(_ rgba: [UInt8], _ share: CGFloat = 1) -> NSColor {
    guard rgba.count == 4 else { return .clear }
    return NSColor(srgbRed: CGFloat(rgba[0]) / 255, green: CGFloat(rgba[1]) / 255, blue: CGFloat(rgba[2]) / 255, alpha: CGFloat(rgba[3]) / 255 * share)
}

/// Each Unicode scalar's UTF-16 offset in `text`, and its end: Rust counts characters in scalars, NSString in UTF-16.
func utf16Offsets(_ text: String) -> [Int] {
    var offsets: [Int] = []
    offsets.reserveCapacity(text.unicodeScalars.count + 1)
    var at = 0
    for scalar in text.unicodeScalars {
        offsets.append(at)
        at += scalar.utf16.count
    }
    offsets.append(at)
    return offsets
}

/// The scalar at UTF-16 offset `utf16` (the first one at or past it).
func scalarIndex(_ offsets: [Int], _ utf16: Int) -> Int {
    offsets.firstIndex { $0 >= utf16 } ?? max(offsets.count - 1, 0)
}

/// A run's attributes at `scale` (1 in the note's points; the zoom in the editor): its line 1.5 times its size
/// (NOTE_LINE), the letters in the middle of it; bold as egui's (painted again a hair wider), italic slanted.
func noteAttributes(size: CGFloat, line: CGFloat, ink: [UInt8], share: CGFloat, bold: Bool = false, italic: Bool = false, strike: Bool = false,
                    underline: Bool = false, back: CGFloat = 0, scale: CGFloat) -> [NSAttributedString.Key: Any] {
    let font = noteFont(max(size * scale, 0.01))
    let height = line * 1.5 * scale
    let paragraph = NSMutableParagraphStyle()
    paragraph.minimumLineHeight = height
    paragraph.maximumLineHeight = height
    let color = noteColor(ink, share)
    var attributes: [NSAttributedString.Key: Any] = [
        .font: font, .foregroundColor: color, .paragraphStyle: paragraph,
        .baselineOffset: max(0, (height - (font.ascender - font.descender)) / 2),
    ]
    if bold { attributes[.strokeWidth] = -3.0; attributes[.strokeColor] = color }
    if italic { attributes[.obliqueness] = 0.2 }
    if strike { attributes[.strikethroughStyle] = NSUnderlineStyle.single.rawValue; attributes[.strikethroughColor] = color }
    if underline { attributes[.underlineStyle] = NSUnderlineStyle.single.rawValue; attributes[.underlineColor] = noteColor(ink, 0.4) }
    if back > 0 { attributes[.backgroundColor] = noteColor(ink, back) }
    return attributes
}

/// Sets `runs` on `storage` (whose string is the runs' text).
// ponytail: a raw note is all at its size; egui's raw view sizes each heading's line (`edit_job`)
func applyRuns(_ storage: NSTextStorage, runs: [CanvasRun], raw: Bool, size: CGFloat, ink: [UInt8], scale: CGFloat) {
    let offsets = utf16Offsets(storage.string)
    storage.beginEditing()
    storage.setAttributes(noteAttributes(size: size, line: size, ink: ink, share: 1, scale: scale), range: NSRange(location: 0, length: storage.length))
    if !raw {
        for run in runs where run.from < run.to && run.to < offsets.count {
            let range = NSRange(location: offsets[run.from], length: offsets[run.to] - offsets[run.from])
            storage.setAttributes(noteAttributes(size: run.size, line: run.line, ink: ink, share: run.ink, bold: run.bold, italic: run.italic,
                                                 strike: run.strike, underline: run.underline, back: run.back, scale: scale), range: range)
        }
    }
    storage.endEditing()
}

/// The bullet, the task's box, the quote's bar and the rule over their hidden marks (`paint_live`), in the current
/// AppKit context, the text's top left at `origin`.
func drawMarks(_ marks: [CanvasMark], manager: NSLayoutManager, container: NSTextContainer, offsets: [Int], origin: CGPoint, ink: [UInt8],
               scale: CGFloat, width: CGFloat) {
    for mark in marks where mark.at < offsets.count - 1 {
        let glyph = manager.glyphIndexForCharacter(at: offsets[mark.at])
        guard glyph < manager.numberOfGlyphs else { continue }
        let letter = manager.boundingRect(forGlyphRange: NSRange(location: glyph, length: 1), in: container)
        let line = manager.lineFragmentRect(forGlyphAt: glyph, effectiveRange: nil)
        let cell = CGRect(x: letter.minX + origin.x, y: line.minY + origin.y, width: letter.width, height: line.height)
        switch mark.kind {
        case "bullet":
            noteColor(ink, 0.4).setFill()
            NSBezierPath(ovalIn: CGRect(x: cell.minX + 4 * scale - 2 * scale, y: cell.midY - 2 * scale, width: 4 * scale, height: 4 * scale)).fill()
        case "task":
            let box = CGRect(x: cell.minX + 6 * scale - 6 * scale, y: cell.midY - 6 * scale, width: 12 * scale, height: 12 * scale)
            if mark.done {
                noteColor(ink, 0.5).setFill()
                NSBezierPath(roundedRect: box, xRadius: 2 * scale, yRadius: 2 * scale).fill()
            } else {
                noteColor(ink, 0.5).setStroke()
                let path = NSBezierPath(roundedRect: box.insetBy(dx: scale / 2, dy: scale / 2), xRadius: 2 * scale, yRadius: 2 * scale)
                path.lineWidth = scale
                path.stroke()
            }
        case "quote":
            noteColor(ink, 0.2).setFill()
            NSBezierPath(rect: CGRect(x: cell.minX, y: cell.minY, width: 2.5 * scale, height: cell.height)).fill()
        case "rule":
            noteColor(ink, 0.2).setStroke()
            let path = NSBezierPath()
            path.move(to: CGPoint(x: origin.x, y: cell.midY))
            path.line(to: CGPoint(x: origin.x + width, y: cell.midY))
            path.lineWidth = scale
            path.stroke()
        default: break
        }
    }
}

/// A resting note's text laid out once (per revision) in its own points, drawn at any zoom by the canvas's transform.
@MainActor final class NoteLayout {
    private let storage: NSTextStorage
    private let manager = NSLayoutManager()
    private let container: NSTextContainer
    private let marks: [CanvasMark]
    private let offsets: [Int]
    private let ink: [UInt8]
    private let width: CGFloat
    /// How tall the text is, in the note's points.
    let height: CGFloat

    init(text: String, runs: [CanvasRun], marks: [CanvasMark], raw: Bool, size: CGFloat, ink: [UInt8], scale: CGFloat, width: CGFloat) {
        storage = NSTextStorage(string: text)
        container = NSTextContainer(size: CGSize(width: max(width, 1), height: .greatestFiniteMagnitude))
        container.lineFragmentPadding = 0
        manager.addTextContainer(container)
        storage.addLayoutManager(manager)
        applyRuns(storage, runs: runs, raw: raw, size: size, ink: ink, scale: scale)
        manager.ensureLayout(for: container)
        self.marks = raw ? [] : marks
        self.ink = ink
        self.width = width
        offsets = utf16Offsets(text)
        height = manager.usedRect(for: container).height
    }

    /// Draws it with its top left at `origin` (the context in the note's points).
    func draw(_ context: inout GraphicsContext, at origin: CGPoint) {
        context.withCGContext { cg in
            NSGraphicsContext.saveGraphicsState()
            NSGraphicsContext.current = NSGraphicsContext(cgContext: cg, flipped: true)
            let glyphs = manager.glyphRange(for: container)
            manager.drawBackground(forGlyphRange: glyphs, at: origin)
            manager.drawGlyphs(forGlyphRange: glyphs, at: origin)
            drawMarks(marks, manager: manager, container: container, offsets: offsets, origin: origin, ink: ink, scale: 1, width: width)
            NSGraphicsContext.restoreGraphicsState()
        }
    }
}


/// A note's text being typed over its card (notes/editor.rs): the same look, scrolling in place; every change and every
/// move of the caret goes to Rust (`ui.canvas.text`), which answers with the runs for where the caret is. Esc or
/// Cmd+Enter ends it.
struct CanvasNoteEditor: NSViewRepresentable {
    let editing: CanvasEditing
    let zoom: CGFloat
    /// Where the resting note was scrolled to (its points): the typing starts there.
    let scroll: CGFloat
    /// Where the press that opened it was, in the editor's points: the caret goes there (or after the text).
    let press: CGPoint?

    func makeCoordinator() -> Coordinator { Coordinator() }

    func makeNSView(context: Context) -> NSScrollView {
        let view = NSScrollView()
        view.drawsBackground = false
        view.hasVerticalScroller = true
        view.autohidesScrollers = true
        view.scrollerStyle = .overlay
        let text = NoteTextView(frame: .zero)
        text.drawsBackground = false
        text.isRichText = false
        text.allowsUndo = true
        text.isAutomaticQuoteSubstitutionEnabled = false
        text.isAutomaticDashSubstitutionEnabled = false
        text.isAutomaticTextReplacementEnabled = false
        text.textContainerInset = .zero
        text.textContainer?.lineFragmentPadding = 0
        text.textContainer?.widthTracksTextView = true
        text.minSize = .zero
        text.maxSize = NSSize(width: CGFloat.greatestFiniteMagnitude, height: .greatestFiniteMagnitude)
        text.isVerticallyResizable = true
        text.isHorizontallyResizable = false
        text.autoresizingMask = [.width]
        text.string = editing.value
        text.delegate = context.coordinator
        text.start = (scroll * zoom, press.map { CGPoint(x: $0.x, y: $0.y + scroll * zoom) })
        view.documentView = text
        context.coordinator.text = text
        context.coordinator.update(editing, zoom: zoom)
        return view
    }

    func updateNSView(_ view: NSScrollView, context: Context) {
        context.coordinator.update(editing, zoom: zoom)
    }

    @MainActor final class Coordinator: NSObject, NSTextViewDelegate {
        weak var text: NoteTextView?
        /// The texts sent to Rust not yet answered: its echo of any of them is not a change of its own.
        private var sent: [String] = []
        /// What the text's look was last set from.
        private var applied: (value: String, runs: [CanvasRun], raw: Bool, zoom: CGFloat)?

        func update(_ editing: CanvasEditing, zoom: CGFloat) {
            guard let text, let storage = text.textStorage, !text.hasMarkedText() else { return }
            // the text changed in Rust (a bar's mark): it takes it
            if editing.value != text.string && !sent.contains(editing.value) {
                text.string = editing.value
            }
            if let index = sent.firstIndex(of: editing.value) { sent.removeFirst(index + 1) }
            text.typingAttributes = noteAttributes(size: editing.size, line: editing.size, ink: editing.color, share: 1, scale: zoom)
            text.ink = editing.color
            text.size = editing.size
            text.scale = zoom
            // the runs are for the text Rust has: while it is behind the typing, the last look stays
            guard editing.value == text.string else { return }
            if let applied, applied.value == editing.value, applied.runs == editing.runs, applied.raw == editing.raw, applied.zoom == zoom { return }
            applied = (editing.value, editing.runs, editing.raw, zoom)
            let selection = text.selectedRanges
            applyRuns(storage, runs: editing.runs, raw: editing.raw, size: editing.size, ink: editing.color, scale: zoom)
            if text.selectedRanges != selection { text.selectedRanges = selection }
            text.marks = editing.raw ? [] : editing.marks
            text.needsDisplay = true
        }

        private func send() {
            guard let text else { return }
            let offsets = utf16Offsets(text.string)
            let range = text.selectedRange()
            // ponytail: the last 16 texts sent; a Rust change equal to one of them while it is behind is taken as an echo
            sent = Array((sent + [text.string]).suffix(16))
            MagoStore.shared.send("ui.canvas.text", ["value": text.string, "anchor": scalarIndex(offsets, range.location),
                                                     "head": scalarIndex(offsets, range.location + range.length)])
        }

        func textDidChange(_ notification: Notification) { send() }
        func textViewDidChangeSelection(_ notification: Notification) { send() }

        func textView(_ textView: NSTextView, doCommandBy selector: Selector) -> Bool {
            let command = NSApp.currentEvent?.modifierFlags.contains(.command) ?? false
            if selector == #selector(NSResponder.cancelOperation(_:)) || (selector == #selector(NSResponder.insertNewline(_:)) && command) {
                MagoStore.shared.send("ui.canvas.text_done")
                return true
            }
            return false
        }
    }
}

/// The editor's text view: the marks over the hidden marks, and the resting note's placeholder while it is empty.
final class NoteTextView: NSTextView {
    var marks: [CanvasMark] = []
    var ink: [UInt8] = [0, 0, 0, 255]
    var size: CGFloat = 14
    var scale: CGFloat = 1
    /// Where it opens scrolled to and where the press was (its points), taken once it is in the window.
    var start: (top: CGFloat, press: CGPoint?)?

    override func viewDidMoveToWindow() {
        super.viewDidMoveToWindow()
        guard let window, let start, let manager = layoutManager, let container = textContainer else { return }
        self.start = nil
        CanvasInputView.pressed = nil
        window.makeFirstResponder(self)
        manager.ensureLayout(for: container)
        var caret = (string as NSString).length
        if let press = start.press, press.y < manager.usedRect(for: container).maxY {
            var fraction: CGFloat = 0
            let index = manager.characterIndex(for: press, in: container, fractionOfDistanceBetweenInsertionPoints: &fraction)
            caret = min(index + (fraction > 0.5 ? 1 : 0), caret)
        }
        setSelectedRange(NSRange(location: caret, length: 0))
        scroll(CGPoint(x: 0, y: start.top))
    }

    override func draw(_ dirtyRect: NSRect) {
        super.draw(dirtyRect)
        guard let manager = layoutManager, let container = textContainer else { return }
        if string.isEmpty {
            // egui's placeholder: the note's face, faint and slanted
            let attributes: [NSAttributedString.Key: Any] = [.font: noteFont(size * scale), .foregroundColor: noteColor(ink, 0.4), .obliqueness: 0.2]
            ("Clique para editar…" as NSString).draw(at: textContainerOrigin, withAttributes: attributes)
            return
        }
        drawMarks(marks, manager: manager, container: container, offsets: utf16Offsets(string), origin: textContainerOrigin, ink: ink, scale: scale,
                  width: container.containerSize.width)
    }
}
