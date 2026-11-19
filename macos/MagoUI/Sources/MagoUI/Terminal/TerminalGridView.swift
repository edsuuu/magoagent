import AppKit
import CoreText
import SwiftUI

/// The terminal's grid: rows of text runs Rust resolved (colors, selection, search marks), drawn with Core Text,
/// repainting only the rows whose revision changed. Also the keyboard (text, IME and dead keys through
/// NSTextInputClient), the mouse (selection by click, double and triple click), the wheel and the size.
final class TerminalGridView: NSView, @preconcurrency NSTextInputClient {
    /// The grid's padding inside the view, as the egui terminal's (8 at the sides, 4 over and under).
    static let padX: CGFloat = 8
    static let padY: CGFloat = 4

    let id: String
    var onFind: (() -> Void)?
    private(set) var state: TerminalState?

    private var fontSize: Double = 14
    private var fonts = TerminalFonts(size: 14)
    private var look = TerminalLook(bg: 0x181818, fg: 0xd8d8d8, cursor: 0xaaaaaa)
    private var rows: [TerminalRow] = []
    private var cursor: TerminalCursor?
    private var offset = 0
    private var history = 0
    private var marked = ""
    private var requestedFocus = false
    private var reportedGrid: (Int, Int)?
    private var dragging = false
    private var dragCell: GridPoint?
    private var scrollRemainder: CGFloat = 0
    private var dragTimer: Timer?

    /// A cell and which half of it the pointer is on.
    private struct GridPoint: Equatable {
        var row: Int
        var col: Int
        var right: Bool
    }

    init(id: String) {
        self.id = id
        super.init(frame: .zero)
        wantsLayer = true
        layerContentsRedrawPolicy = .onSetNeedsDisplay
    }

    required init?(coder: NSCoder) { fatalError("not from a nib") }

    override var isFlipped: Bool { true }
    override var isOpaque: Bool { true }
    override var acceptsFirstResponder: Bool { true }
    override func acceptsFirstMouse(for event: NSEvent?) -> Bool { true }

    private var isFocused: Bool { window?.firstResponder === self }
    private var cellSize: CGSize { fonts.cell }

    // MARK: state

    func apply(_ new: TerminalState) {
        state = new
        if new.font != fontSize {
            fontSize = new.font
            fonts = TerminalFonts(size: CGFloat(new.font))
            needsDisplay = true
            reportSize(force: true)
        }
        if new.look != look {
            look = new.look
            needsDisplay = true
        }
        let newRows = new.grid?.rows ?? []
        if newRows.count != rows.count {
            needsDisplay = true
        } else {
            for (index, row) in newRows.enumerated() where row.rev != rows[index].rev {
                setNeedsDisplay(rowRect(index))
            }
        }
        rows = newRows
        let newCursor = new.grid?.cursor
        if newCursor != cursor {
            invalidateCursor()
            cursor = newCursor
            invalidateCursor()
        }
        if new.grid?.offset != offset || new.grid?.history != history {
            offset = new.grid?.offset ?? 0
            history = new.grid?.history ?? 0
            needsDisplay = true
        }
        // the app moved the keys here (an agent, a menu)
        if new.focused != requestedFocus {
            requestedFocus = new.focused
            if new.focused, !isFocused { window?.makeFirstResponder(self) }
        }
    }

    private func rowRect(_ index: Int) -> NSRect {
        NSRect(x: 0, y: Self.padY + CGFloat(index) * cellSize.height, width: bounds.width, height: cellSize.height)
    }

    private func invalidateCursor() {
        if let cursor { setNeedsDisplay(rowRect(cursor.row)) }
    }

    // MARK: size

    override func setFrameSize(_ newSize: NSSize) {
        super.setFrameSize(newSize)
        reportSize(force: false)
    }

    override func viewDidMoveToWindow() {
        super.viewDidMoveToWindow()
        reportSize(force: true)
    }

    /// Tells Rust how many columns and rows fit: the shell takes the view's size, not the card's in the scene.
    private func reportSize(force: Bool) {
        guard window != nil, bounds.width > 0 else { return }
        let cols = max(2, Int(((bounds.width - 2 * Self.padX) / cellSize.width).rounded(.down)))
        let lines = max(1, Int(((bounds.height - 2 * Self.padY) / cellSize.height).rounded(.down)))
        if !force, let reportedGrid, reportedGrid == (cols, lines) { return }
        reportedGrid = (cols, lines)
        MagoStore.shared.send("ui.terminal.resize", ["id": id, "cols": cols, "rows": lines])
    }

    // MARK: drawing

    override func draw(_ dirtyRect: NSRect) {
        guard let context = NSGraphicsContext.current?.cgContext else { return }
        context.setFillColor(NSColor(rgb: look.bg).cgColor)
        context.fill(dirtyRect)
        let height = cellSize.height
        let first = max(0, Int(((dirtyRect.minY - Self.padY) / height).rounded(.down)))
        let last = min(rows.count - 1, Int(((dirtyRect.maxY - Self.padY) / height).rounded(.down)))
        if first <= last {
            for index in first...last { drawRow(index, in: context) }
        }
        drawCursor(in: context)
        drawMarked(in: context)
    }

    private func drawRow(_ index: Int, in context: CGContext) {
        let top = Self.padY + CGFloat(index) * cellSize.height
        let runs = rows[index].runs
        for run in runs where run.bg >= 0 {
            context.setFillColor(NSColor(rgb: UInt32(run.bg)).cgColor)
            context.fill(CGRect(x: Self.padX + CGFloat(run.col) * cellSize.width, y: top, width: CGFloat(run.cells) * cellSize.width, height: cellSize.height))
        }
        for run in runs {
            drawText(run.text, col: run.col, cells: run.cells, style: run.style, color: NSColor(rgb: run.fg), top: top, in: context)
        }
    }

    /// `text` from column `col`: ASCII glyph by glyph on the cell grid, any other character (one per run) in its own cell.
    private func drawText(_ text: String, col: Int, cells: Int, style: UInt8, color: NSColor, top: CGFloat, in context: CGContext) {
        let width = cellSize.width
        let x = Self.padX + CGFloat(col) * width
        let font = fonts.font(bold: style & 1 != 0, italic: style & 2 != 0)
        context.saveGState()
        context.setFillColor(color.cgColor)
        if style & 4 != 0 { context.fill(CGRect(x: x, y: top + cellSize.height - 1, width: CGFloat(cells) * width, height: 1)) }
        if style & 8 != 0 { context.fill(CGRect(x: x, y: top + cellSize.height / 2, width: CGFloat(cells) * width, height: 1)) }
        if let scalar = text.unicodeScalars.first, text.unicodeScalars.count == 1, let arms = terminalBoxArms(scalar.value) {
            // a one-point line from the centre to each side it reaches; a round corner bends through the centre
            let cell = CGRect(x: x, y: top, width: CGFloat(cells) * width, height: cellSize.height)
            let centre = CGPoint(x: cell.midX, y: cell.midY)
            let ends = [(arms.left, CGPoint(x: cell.minX, y: centre.y)), (arms.up, CGPoint(x: centre.x, y: cell.minY)),
                        (arms.right, CGPoint(x: cell.maxX, y: centre.y)), (arms.down, CGPoint(x: centre.x, y: cell.maxY))]
                .filter(\.0).map(\.1)
            context.setStrokeColor(color.cgColor)
            context.setLineWidth(1)
            // square ends: the two arms of a corner cover its centre
            context.setLineCap(.square)
            if arms.round, ends.count == 2 {
                context.move(to: ends[0])
                context.addArc(tangent1End: centre, tangent2End: ends[1], radius: min(cell.width, cell.height) / 2)
                context.addLine(to: ends[1])
            } else {
                for end in ends { context.move(to: centre); context.addLine(to: end) }
            }
            context.strokePath()
            context.restoreGState()
            return
        }
        if let scalar = text.unicodeScalars.first, text.unicodeScalars.count == 1, let block = terminalBlockParts(scalar.value) {
            // block elements fill their cell exactly
            context.setFillColor(color.withAlphaComponent(block.alpha).cgColor)
            let cell = CGRect(x: x, y: top, width: CGFloat(cells) * width, height: cellSize.height)
            for part in block.parts {
                context.fill(CGRect(x: cell.minX + part[0] * cell.width, y: cell.minY + part[1] * cell.height,
                                    width: (part[2] - part[0]) * cell.width, height: (part[3] - part[1]) * cell.height))
            }
            context.restoreGState()
            return
        }
        // the text's origin is the baseline, y up
        let baseline = top + (cellSize.height - (fonts.ascent + fonts.descent)) / 2 + fonts.ascent
        context.translateBy(x: x, y: baseline)
        context.scaleBy(x: 1, y: -1)
        // an italic Hack is slanted, as the egui terminal has none
        if style & 2 != 0, !CTFontGetSymbolicTraits(font).contains(.traitItalic) {
            context.concatenate(CGAffineTransform(a: 1, b: 0, c: 0.2, d: 1, tx: 0, ty: 0))
        }
        if text.utf8.allSatisfy({ $0 < 128 }) {
            var characters = Array(text.utf16)
            var glyphs = [CGGlyph](repeating: 0, count: characters.count)
            CTFontGetGlyphsForCharacters(font, &characters, &glyphs, characters.count)
            let positions = (0..<glyphs.count).map { CGPoint(x: CGFloat($0) * width, y: 0) }
            CTFontDrawGlyphs(font, glyphs, positions, glyphs.count, context)
            if style & 1 != 0, fonts.syntheticBold {
                CTFontDrawGlyphs(font, glyphs, positions.map { CGPoint(x: $0.x + 0.6, y: 0) }, glyphs.count, context)
            }
        } else {
            let attributes: [NSAttributedString.Key: Any] = [
                NSAttributedString.Key(kCTFontAttributeName as String): font,
                NSAttributedString.Key(kCTForegroundColorFromContextAttributeName as String): true,
            ]
            let line = CTLineCreateWithAttributedString(NSAttributedString(string: text, attributes: attributes))
            context.textPosition = .zero
            CTLineDraw(line, context)
        }
        context.restoreGState()
    }

    private func cursorRect(_ cursor: TerminalCursor) -> CGRect {
        CGRect(x: Self.padX + CGFloat(cursor.col) * cellSize.width, y: Self.padY + CGFloat(cursor.row) * cellSize.height, width: cellSize.width, height: cellSize.height)
    }

    /// As the egui terminal: a block over the cell (the letter under it is covered), hollow while the terminal has no keys;
    /// whatever shape or blink the program asks for.
    private func drawCursor(in context: CGContext) {
        guard let cursor, cursor.row < rows.count else { return }
        let rect = cursorRect(cursor)
        let color = NSColor(rgb: look.cursor)
        if isFocused {
            context.setFillColor(color.cgColor)
            context.fill(rect)
        } else {
            context.setStrokeColor(color.cgColor)
            context.setLineWidth(1)
            context.stroke(rect.insetBy(dx: 0.5, dy: 0.5))
        }
    }

    /// What an input method is composing, at the cursor, underlined.
    private func drawMarked(in context: CGContext) {
        guard !marked.isEmpty, let cursor else { return }
        let rect = cursorRect(cursor)
        let cells = marked.unicodeScalars.reduce(0) { $0 + ($1.value > 0x2E80 ? 2 : 1) }
        context.setFillColor(NSColor(rgb: look.bg).cgColor)
        context.fill(CGRect(x: rect.minX, y: rect.minY, width: CGFloat(cells) * cellSize.width, height: rect.height))
        drawText(marked, col: cursor.col, cells: cells, style: 4, color: NSColor(rgb: look.fg), top: rect.minY, in: context)
    }

    // MARK: focus

    override func becomeFirstResponder() -> Bool {
        MagoStore.shared.send("ui.terminal.focus", ["id": id, "focused": true])
        invalidateCursor()
        return true
    }

    override func resignFirstResponder() -> Bool {
        MagoStore.shared.send("ui.terminal.focus", ["id": id, "focused": false])
        invalidateCursor()
        return true
    }

    // MARK: keyboard

    /// ⌘C, ⌘V, ⌘F, ⌘K and ⌘A, as the egui terminal has them; every other ⌘ key is the app's.
    override func performKeyEquivalent(with event: NSEvent) -> Bool {
        guard isFocused, event.modifierFlags.intersection(.deviceIndependentFlagsMask) == .command else { return false }
        switch event.charactersIgnoringModifiers?.lowercased() {
        case "c": TerminalActions.copy(id: id)
        case "v": TerminalActions.paste(id: id)
        case "k": MagoStore.shared.send("ui.terminal.clear", ["id": id])
        case "f": onFind?()
        case "a": MagoStore.shared.send("ui.terminal.select", ["id": id, "phase": "all"])
        default: return false
        }
        return true
    }

    override func keyDown(with event: NSEvent) {
        let flags = event.modifierFlags.intersection(.deviceIndependentFlagsMask)
        // an input method composing takes every key, Enter and arrows included
        if hasMarkedText() {
            interpretKeyEvents([event])
            return
        }
        if let key = TerminalKeys.special(event) ?? (flags.contains(.control) ? TerminalKeys.control(event) : nil) {
            MagoStore.shared.send("ui.terminal.key", ["id": id, "key": key, "mods": TerminalKeys.modifiers(flags)])
            return
        }
        // the other ⌘ keys are not the terminal's
        if flags.contains(.command) { return }
        interpretKeyEvents([event])
    }

    // MARK: NSTextInputClient

    func insertText(_ string: Any, replacementRange: NSRange) {
        marked = ""
        invalidateCursor()
        let text = (string as? NSAttributedString)?.string ?? (string as? String) ?? ""
        guard !text.isEmpty else { return }
        MagoStore.shared.send("ui.terminal.input", ["id": id, "text": text])
    }

    func setMarkedText(_ string: Any, selectedRange: NSRange, replacementRange: NSRange) {
        marked = (string as? NSAttributedString)?.string ?? (string as? String) ?? ""
        invalidateCursor()
    }

    func unmarkText() {
        marked = ""
        invalidateCursor()
    }

    func selectedRange() -> NSRange { NSRange(location: NSNotFound, length: 0) }

    func markedRange() -> NSRange { marked.isEmpty ? NSRange(location: NSNotFound, length: 0) : NSRange(location: 0, length: (marked as NSString).length) }

    func hasMarkedText() -> Bool { !marked.isEmpty }

    func attributedSubstring(forProposedRange range: NSRange, actualRange: NSRangePointer?) -> NSAttributedString? { nil }

    func validAttributesForMarkedText() -> [NSAttributedString.Key] { [] }

    func characterIndex(for point: NSPoint) -> Int { 0 }

    /// Where the input method's window goes: under the cursor.
    func firstRect(forCharacterRange range: NSRange, actualRange: NSRangePointer?) -> NSRect {
        let rect = cursor.map(cursorRect) ?? .zero
        return window?.convertToScreen(convert(rect, to: nil)) ?? rect
    }

    /// Keys the input method turns into commands (arrows, Enter…) were sent before it as keys.
    override func doCommand(by selector: Selector) {}

    // MARK: mouse

    private func gridPoint(_ event: NSEvent) -> GridPoint {
        let point = convert(event.locationInWindow, from: nil)
        let x = (point.x - Self.padX) / cellSize.width
        let y = (point.y - Self.padY) / cellSize.height
        return GridPoint(row: Int(y.rounded(.down)), col: max(0, Int(x.rounded(.down))), right: x - x.rounded(.down) >= 0.5)
    }

    private func select(_ phase: String, _ point: GridPoint, pick: String = "chars", copies: Bool = false) {
        let args: [String: Any] = ["id": id, "phase": phase, "pick": pick, "row": point.row, "col": point.col, "right": point.right]
        guard copies else {
            MagoStore.shared.send("ui.terminal.select", args)
            return
        }
        // "copiar ao selecionar" answers with the text to put on the clipboard
        Task { @MainActor in
            let reply = await MagoStore.shared.act("ui.terminal.select", args)
            TerminalActions.toPasteboard(reply.text)
        }
    }

    override func mouseDown(with event: NSEvent) {
        window?.makeFirstResponder(self)
        let point = gridPoint(event)
        dragging = true
        dragCell = point
        let pick = event.clickCount >= 3 ? "line" : event.clickCount == 2 ? "word" : "chars"
        select("begin", point, pick: pick, copies: pick != "chars")
    }

    override func mouseDragged(with event: NSEvent) {
        guard dragging else { return }
        let point = gridPoint(event)
        guard point != dragCell else { return }
        dragCell = point
        select("extend", point)
        // held above or below the grid it keeps scrolling the history
        let outside = point.row < 0 || point.row >= rows.count
        if outside, dragTimer == nil {
            dragTimer = Timer.scheduledTimer(withTimeInterval: 0.05, repeats: true) { [weak self] _ in
                MainActor.assumeIsolated {
                    if let self, let cell = self.dragCell { self.select("extend", cell) }
                }
            }
        } else if !outside {
            dragTimer?.invalidate()
            dragTimer = nil
        }
    }

    override func mouseUp(with event: NSEvent) {
        guard dragging else { return }
        dragging = false
        dragTimer?.invalidate()
        dragTimer = nil
        select("end", gridPoint(event), copies: event.clickCount == 1)
    }

    override func scrollWheel(with event: NSEvent) {
        // a trackpad's points are lines by the cell's height; a wheel's notch is three lines
        let lines = event.hasPreciseScrollingDeltas ? event.scrollingDeltaY / cellSize.height : event.scrollingDeltaY * 3
        scrollRemainder += lines
        let whole = Int(scrollRemainder.rounded(.towardZero))
        guard whole != 0 else { return }
        scrollRemainder -= CGFloat(whole)
        let point = gridPoint(event)
        MagoStore.shared.send("ui.terminal.scroll", [
            "id": id, "lines": whole, "col": point.col, "row": max(0, point.row), "shift": event.modifierFlags.contains(.shift),
        ])
    }

    // MARK: menu

    override func menu(for event: NSEvent) -> NSMenu? {
        guard let state else { return nil }
        return makeMenu(state.entries)
    }

    private func makeMenu(_ entries: [TerminalEntry]) -> NSMenu {
        let menu = NSMenu()
        menu.autoenablesItems = false
        for entry in entries {
            if entry.sep {
                menu.addItem(.separator())
            } else if !entry.items.isEmpty {
                let item = NSMenuItem(title: entry.label, action: nil, keyEquivalent: "")
                item.submenu = makeMenu(entry.items)
                menu.addItem(item)
            } else {
                let item = NSMenuItem(title: entry.label, action: #selector(pickEntry(_:)), keyEquivalent: "")
                item.target = self
                item.representedObject = entry.id
                item.isEnabled = entry.enabled
                if entry.check == true { item.state = .on }
                menu.addItem(item)
            }
        }
        return menu
    }

    @objc private func pickEntry(_ item: NSMenuItem) {
        if let entry = item.representedObject as? String, let state { TerminalActions.pick(entry, state: state) }
    }

    isolated deinit {
        dragTimer?.invalidate()
    }
}

/// The grid in SwiftUI.
struct TerminalSurface: NSViewRepresentable {
    let id: String
    let state: TerminalState
    let handle: TerminalGridHandle
    let onFind: () -> Void

    func makeNSView(context: Context) -> TerminalGridView {
        let view = TerminalGridView(id: id)
        handle.view = view
        view.onFind = onFind
        view.apply(state)
        return view
    }

    func updateNSView(_ view: TerminalGridView, context: Context) {
        view.onFind = onFind
        view.apply(state)
    }
}
