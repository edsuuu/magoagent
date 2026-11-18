import AppKit
import SwiftUI

/// The code of a text tab on an NSTextView. The text is Rust's: it is loaded once per `loadRev`, each edit goes back as a range
/// (`ui.filetree.edit`, UTF-16 units as the text view counts them), and the colors, the change markers and the problems come back
/// from Rust (`filetree:<id>:live`). Line numbers and the markers are the ruler's.
struct FTCodeEditor: NSViewRepresentable {
    let treeId: String
    let doc: FTDoc
    let live: FTLive?
    let wrap: Bool
    let theme: FTTheme

    func makeCoordinator() -> FTCodeCoordinator { FTCodeCoordinator(treeId: treeId) }

    func makeNSView(context: Context) -> NSScrollView {
        let coordinator = context.coordinator
        let storage = NSTextStorage()
        let layout = NSLayoutManager()
        let container = NSTextContainer(size: NSSize(width: 1e7, height: 1e7))
        storage.addLayoutManager(layout)
        layout.addTextContainer(container)
        let textView = FTTextView(frame: .zero, textContainer: container)
        textView.isRichText = false
        textView.allowsUndo = true
        textView.isAutomaticQuoteSubstitutionEnabled = false
        textView.isAutomaticDashSubstitutionEnabled = false
        textView.isAutomaticTextReplacementEnabled = false
        textView.isAutomaticSpellingCorrectionEnabled = false
        textView.isContinuousSpellCheckingEnabled = false
        textView.isAutomaticLinkDetectionEnabled = false
        textView.usesFindBar = true
        textView.isIncrementalSearchingEnabled = true
        textView.textContainerInset = NSSize(width: 4, height: 0)
        textView.minSize = NSSize(width: 0, height: 0)
        textView.maxSize = NSSize(width: 1e7, height: 1e7)
        textView.isVerticallyResizable = true
        storage.delegate = coordinator

        let scroll = NSScrollView()
        scroll.documentView = textView
        scroll.hasVerticalScroller = true
        scroll.hasHorizontalScroller = true
        scroll.autohidesScrollers = true
        scroll.borderType = .noBorder
        let ruler = FTRuler(scrollView: scroll, orientation: .verticalRuler)
        ruler.clientView = textView
        scroll.verticalRulerView = ruler
        scroll.hasVerticalRuler = true
        scroll.rulersVisible = true
        scroll.contentView.postsBoundsChangedNotifications = true
        NotificationCenter.default.addObserver(coordinator, selector: #selector(FTCodeCoordinator.scrolled), name: NSView.boundsDidChangeNotification, object: scroll.contentView)
        NotificationCenter.default.addObserver(coordinator, selector: #selector(FTCodeCoordinator.selectionMoved), name: NSTextView.didChangeSelectionNotification, object: textView)

        coordinator.textView = textView
        coordinator.scroll = scroll
        coordinator.ruler = ruler
        ruler.coordinator = coordinator
        textView.coordinator = coordinator
        return scroll
    }

    func updateNSView(_ scroll: NSScrollView, context: Context) {
        context.coordinator.update(doc: doc, live: live, wrap: wrap, theme: theme)
    }

    static func dismantleNSView(_ scroll: NSScrollView, coordinator: FTCodeCoordinator) {
        NotificationCenter.default.removeObserver(coordinator)
    }
}

/// What each tree's editor already did with a jump and a focus request: a view made again (a tab came back) does not do them twice.
@MainActor
private var handledJumps: [String: UInt64] = [:]
@MainActor
private var handledFocus: [String: UInt64] = [:]

@MainActor
final class FTCodeCoordinator: NSObject, @preconcurrency NSTextStorageDelegate, NSViewToolTipOwner {
    let treeId: String
    weak var textView: FTTextView?
    weak var scroll: NSScrollView?
    weak var ruler: FTRuler?
    var path = ""
    var loadedRev: UInt64?
    /// Edits sent so far; Rust's `appliedRev` catches up with it.
    var rev: UInt64 = 0
    var suppress = false
    var lineStarts: [Int] = [0]
    var theme: FTTheme?
    var hunks: [FTHunk] = []
    var diags: [FTDiag] = []
    var hasHead = false
    private var wrap: Bool?
    private var lastLive = 0
    private var viewport = (first: -1, last: -1)
    private var popover: NSPopover?
    private(set) var font = FTCodeCoordinator.codeFont()

    init(treeId: String) { self.treeId = treeId }

    static func codeFont() -> NSFont {
        NSFont(name: "JetBrainsMono-Regular", size: 13) ?? NSFont.monospacedSystemFont(ofSize: 13, weight: .regular)
    }

    private var baseAttributes: [NSAttributedString.Key: Any] {
        let style = NSMutableParagraphStyle()
        style.lineHeightMultiple = 1.2
        return [.font: font, .foregroundColor: NSColor(hex: theme?.text ?? "#BCBEC4"), .paragraphStyle: style]
    }

    // MARK: props in

    func update(doc: FTDoc, live: FTLive?, wrap: Bool, theme: FTTheme) {
        guard let textView, let scroll else { return }
        self.theme = theme
        textView.theme = theme
        textView.backgroundColor = NSColor(hex: theme.background)
        textView.insertionPointColor = NSColor(hex: theme.caret)
        textView.selectedTextAttributes = [.backgroundColor: NSColor(hex: theme.selection)]
        scroll.backgroundColor = NSColor(hex: theme.background)
        ruler?.theme = theme
        if self.wrap != wrap { applyWrap(wrap) }
        if loadedRev != doc.loadRev || path != doc.path { load(doc) }
        if let live, live.path == doc.path, live.loadRev == loadedRev {
            // a stale answer (more edits were typed since) waits for the next one
            if live.appliedRev == rev { apply(live) }
            focusAndJump(live)
        }
    }

    private func applyWrap(_ on: Bool) {
        guard let textView, let scroll else { return }
        wrap = on
        let container = textView.textContainer
        container?.widthTracksTextView = on
        textView.isHorizontallyResizable = !on
        textView.autoresizingMask = on ? [.width] : []
        if on {
            container?.containerSize = NSSize(width: max(scroll.contentSize.width - (scroll.verticalRulerView?.ruleThickness ?? 0), 50), height: 1e7)
        } else {
            container?.containerSize = NSSize(width: 1e7, height: 1e7)
        }
    }

    private func load(_ doc: FTDoc) {
        guard let textView, let storage = textView.textStorage, let scroll else { return }
        let sameFile = path == doc.path
        let selection = textView.selectedRange()
        let top = scroll.contentView.bounds.origin
        suppress = true
        textView.string = doc.text ?? ""
        storage.setAttributes(baseAttributes, range: NSRange(location: 0, length: storage.length))
        textView.typingAttributes = baseAttributes
        suppress = false
        (path, loadedRev, rev, lastLive) = (doc.path, doc.loadRev, 0, 0)
        hunks = []
        diags = []
        textView.undoManager?.removeAllActions()
        rebuildLines()
        if sameFile {
            // Rust changed the text (the disk's, a rollback): the caret and the scroll stay where they were
            textView.setSelectedRange(NSRange(location: min(selection.location, storage.length), length: 0))
            scroll.contentView.scroll(to: top)
        } else {
            textView.setSelectedRange(NSRange(location: 0, length: 0))
            scroll.contentView.scroll(to: .zero)
        }
        scroll.reflectScrolledClipView(scroll.contentView)
        viewport = (-1, -1)
        scrolled()
        ruler?.needsDisplay = true
    }

    // MARK: live in

    private func apply(_ live: FTLive) {
        guard let textView, let storage = textView.textStorage, let theme else { return }
        var hasher = Hasher()
        hasher.combine(live.spans)
        hasher.combine(live.spanRange)
        hasher.combine(live.appliedRev)
        hasher.combine(live.hunks)
        hasher.combine(live.diags)
        let key = hasher.finalize()
        guard key != lastLive else { return }
        lastLive = key
        hunks = live.hunks
        diags = live.diags
        hasHead = live.hasHead
        let length = storage.length
        if live.spanRange.count == 2 {
            let from = min(live.spanRange[0], length)
            let range = NSRange(location: from, length: max(0, min(live.spanRange[1], length) - from))
            let colors = theme.kinds.map { NSColor(hex: $0.color) }
            let italic = NSFontManager.shared.convert(font, toHaveTrait: .italicFontMask)
            suppress = true
            storage.beginEditing()
            storage.setAttributes(baseAttributes, range: range)
            var at = 0
            while at + 2 < live.spans.count {
                let start = Int(live.spans[at]), end = Int(live.spans[at + 1]), kind = Int(live.spans[at + 2])
                at += 3
                guard end <= length, start < end, kind < colors.count else { continue }
                let span = NSRange(location: start, length: end - start)
                storage.addAttribute(.foregroundColor, value: colors[kind], range: span)
                if theme.kinds[kind].italic { storage.addAttribute(.font, value: italic, range: span) }
            }
            storage.endEditing()
            suppress = false
        }
        markDiagnostics()
        ruler?.needsDisplay = true
        textView.needsDisplay = true
    }

    /// The problems: a colored underline under their range, and their message in a tooltip.
    private func markDiagnostics() {
        guard let textView, let layout = textView.layoutManager, let storage = textView.textStorage, let theme else { return }
        let full = NSRange(location: 0, length: storage.length)
        layout.removeTemporaryAttribute(.underlineStyle, forCharacterRange: full)
        layout.removeTemporaryAttribute(.underlineColor, forCharacterRange: full)
        textView.removeAllToolTips()
        for (index, diag) in diags.enumerated() {
            let start = offset(line: diag.line, col: diag.col)
            var end = offset(line: diag.endLine, col: diag.endCol)
            if end <= start { end = min(start + 1, storage.length) }
            guard start < storage.length, end > start else { continue }
            let range = NSRange(location: start, length: end - start)
            let color = NSColor(hex: diag.severity == 1 ? theme.error : theme.warning)
            layout.addTemporaryAttribute(.underlineStyle, value: NSUnderlineStyle.single.rawValue | NSUnderlineStyle.patternDash.rawValue, forCharacterRange: range)
            layout.addTemporaryAttribute(.underlineColor, value: color, forCharacterRange: range)
            if index < 300 {
                let glyphs = layout.glyphRange(forCharacterRange: range, actualCharacterRange: nil)
                if let container = textView.textContainer {
                    var rect = layout.boundingRect(forGlyphRange: glyphs, in: container)
                    rect.origin.x += textView.textContainerOrigin.x
                    rect.origin.y += textView.textContainerOrigin.y
                    textView.addToolTip(rect.insetBy(dx: 0, dy: -1), owner: self, userData: UnsafeMutableRawPointer(bitPattern: index + 1))
                }
            }
        }
    }

    nonisolated func view(_ view: NSView, stringForToolTip tag: NSView.ToolTipTag, point: NSPoint, userData data: UnsafeMutableRawPointer?) -> String {
        let index = Int(bitPattern: data) - 1
        return MainActor.assumeIsolated { diags.indices.contains(index) ? diags[index].message : "" }
    }

    private func focusAndJump(_ live: FTLive) {
        guard let textView, let scroll else { return }
        if live.focusSeq > (handledFocus[treeId] ?? 0) {
            handledFocus[treeId] = live.focusSeq
            textView.window?.makeFirstResponder(textView)
        }
        guard let jump = live.jump, jump.seq > (handledJumps[treeId] ?? 0) else { return }
        handledJumps[treeId] = jump.seq
        let location = offset(line: jump.line, col: jump.col)
        textView.setSelectedRange(NSRange(location: location, length: 0))
        guard let layout = textView.layoutManager, let storage = textView.textStorage, storage.length > 0 else { return }
        let char = min(location, storage.length - 1)
        layout.ensureLayout(forCharacterRange: NSRange(location: 0, length: min(storage.length, char + 1)))
        let glyph = layout.glyphIndexForCharacter(at: char)
        let rect = layout.lineFragmentRect(forGlyphAt: glyph, effectiveRange: nil)
        let height = scroll.contentView.bounds.height
        let y = max(0, rect.midY + textView.textContainerOrigin.y - height / 2)
        scroll.contentView.scroll(to: NSPoint(x: 0, y: y))
        scroll.reflectScrolledClipView(scroll.contentView)
        if jump.focus { textView.window?.makeFirstResponder(textView) }
    }

    // MARK: lines

    func rebuildLines() {
        guard let storage = textView?.textStorage else { return }
        var starts = [0]
        var index = 0
        for unit in storage.string.utf16 {
            index += 1
            if unit == 10 { starts.append(index) }
        }
        lineStarts = starts
    }

    /// The line a character is in (binary search over the line starts).
    func line(ofChar char: Int) -> Int {
        var low = 0, high = lineStarts.count - 1
        while low < high {
            let mid = (low + high + 1) / 2
            if lineStarts[mid] <= char { low = mid } else { high = mid - 1 }
        }
        return low
    }

    /// The character offset of a line and column, kept inside the text.
    func offset(line: Int, col: Int) -> Int {
        let length = textView?.textStorage?.length ?? 0
        let row = min(max(line, 0), lineStarts.count - 1)
        let end = row + 1 < lineStarts.count ? lineStarts[row + 1] - 1 : length
        return min(lineStarts[row] + max(col, 0), max(end, lineStarts[row]))
    }

    // MARK: events

    func textStorage(_ textStorage: NSTextStorage, didProcessEditing editedMask: NSTextStorageEditActions, range editedRange: NSRange, changeInLength delta: Int) {
        guard editedMask.contains(.editedCharacters), !suppress else { return }
        let replacement = textStorage.mutableString.substring(with: editedRange)
        rev += 1
        FT.send("edit", treeId, ["path": path, "from": editedRange.location, "to": editedRange.location + editedRange.length - delta, "text": replacement, "rev": rev])
        rebuildLines()
        ruler?.needsDisplay = true
    }

    @objc func selectionMoved() {
        textView?.needsDisplay = true
        ruler?.needsDisplay = true
    }

    /// The lines in view, told to Rust (a big file is colored around them).
    @objc func scrolled() {
        ruler?.needsDisplay = true
        guard let textView, let scroll, let layout = textView.layoutManager, let container = textView.textContainer, (textView.textStorage?.length ?? 0) > 120_000 else { return }
        let glyphs = layout.glyphRange(forBoundingRect: scroll.contentView.bounds, in: container)
        let chars = layout.characterRange(forGlyphRange: glyphs, actualGlyphRange: nil)
        let (first, last) = (line(ofChar: chars.location), line(ofChar: chars.location + chars.length))
        if abs(first - viewport.first) < 20 && abs(last - viewport.last) < 20 { return }
        viewport = (first, last)
        FT.send("viewport", treeId, ["first": first, "last": last])
    }

    // MARK: asks of the editor

    func save() { FT.send("save", treeId) }
    func compare() { FT.send("diff", treeId, ["path": path]) }

    func find() {
        guard let textView else { return }
        let range = textView.selectedRange()
        var query = ""
        if range.length > 0 { query = (textView.string as NSString).substring(with: range) }
        FT.send("find_open", treeId, ["query": query.contains("\n") ? "" : query])
    }

    /// Ctrl-click on a name: its declaration.
    func declaration(at point: NSPoint) {
        guard let textView else { return }
        let index = textView.characterIndexForInsertion(at: point)
        FT.send("declaration", treeId, ["path": path, "offset": index])
    }

    /// "Comentar para a Luna": the selected lines with a note go to the Maestro's chat.
    func comment() {
        guard let textView else { return }
        let range = textView.selectedRange()
        guard range.length > 0 else { return }
        let from = line(ofChar: range.location) + 1
        let to = line(ofChar: max(range.location, range.location + range.length - 1)) + 1
        let alert = NSAlert()
        alert.messageText = "Comentar para a Luna"
        alert.informativeText = from == to ? "Linha \(from)" : "Linhas \(from) a \(to)"
        let field = NSTextField(frame: NSRect(x: 0, y: 0, width: 320, height: 24))
        alert.accessoryView = field
        alert.addButton(withTitle: "Enviar")
        alert.addButton(withTitle: "Cancelar")
        alert.window.initialFirstResponder = field
        guard alert.runModal() == .alertFirstButtonReturn, !field.stringValue.trimmingCharacters(in: .whitespaces).isEmpty else { return }
        let args: [String: Any] = ["tree": treeId, "path": path, "from_line": from, "to_line": to, "comment": field.stringValue]
        Task {
            let reply = await MagoStore.shared.act("maestro_comment", args)
            if !reply.ok {
                let failure = NSAlert()
                failure.messageText = reply.text
                failure.runModal()
            }
        }
    }

    /// A marker of the gutter pressed: the popup with the old lines and what can be done with the change.
    func showHunk(_ index: Int, from rect: NSRect) {
        guard let ruler, hunks.indices.contains(index), let theme else { return }
        popover?.close()
        let hunk = hunks[index]
        let view = FTHunkPopover(
            hunk: hunk, index: index, count: hunks.count, theme: theme,
            go: { [weak self] to in self?.goToHunk(to) },
            revert: { [weak self] in
                guard let self else { return }
                popover?.close()
                FT.send("revert_change", treeId, ["path": path, "index": index])
            },
            copy: {
                NSPasteboard.general.clearContents()
                NSPasteboard.general.setString(hunk.old, forType: .string)
            },
            diff: { [weak self] in
                guard let self else { return }
                popover?.close()
                compare()
            })
        let pop = NSPopover()
        pop.behavior = .transient
        pop.contentViewController = NSHostingController(rootView: view)
        pop.show(relativeTo: rect, of: ruler, preferredEdge: .maxX)
        popover = pop
    }

    private func goToHunk(_ index: Int) {
        guard hunks.indices.contains(index), let textView, let scroll, let layout = textView.layoutManager, let ruler else { return }
        let location = offset(line: hunks[index].start, col: 0)
        textView.setSelectedRange(NSRange(location: location, length: 0))
        let glyph = layout.glyphIndexForCharacter(at: min(location, max((textView.textStorage?.length ?? 1) - 1, 0)))
        let rect = layout.lineFragmentRect(forGlyphAt: glyph, effectiveRange: nil)
        let y = max(0, rect.midY - scroll.contentView.bounds.height / 2)
        scroll.contentView.scroll(to: NSPoint(x: 0, y: y))
        scroll.reflectScrolledClipView(scroll.contentView)
        ruler.layoutSubtreeIfNeeded()
        ruler.displayIfNeeded()
        if let markerRect = ruler.markerRect(index) { showHunk(index, from: markerRect) }
    }
}

/// The text view: the caret's row painted under the text, and the editor's keys.
final class FTTextView: NSTextView {
    weak var coordinator: FTCodeCoordinator?
    var theme: FTTheme?

    override func drawBackground(in rect: NSRect) {
        super.drawBackground(in: rect)
        guard let theme, let layout = layoutManager, selectedRange().length == 0 else { return }
        let location = selectedRange().location
        var line = NSRect.zero
        if location >= string.utf16.count && layout.extraLineFragmentRect != .zero {
            line = layout.extraLineFragmentRect
        } else if string.utf16.count > 0 {
            let glyph = layout.glyphIndexForCharacter(at: min(location, string.utf16.count - 1))
            line = layout.lineFragmentRect(forGlyphAt: glyph, effectiveRange: nil)
        }
        guard line != .zero else { return }
        NSColor(hex: theme.caretRow).setFill()
        NSRect(x: rect.minX, y: line.minY + textContainerOrigin.y, width: rect.width, height: line.height).fill()
    }

    override func performKeyEquivalent(with event: NSEvent) -> Bool {
        guard window?.firstResponder === self else { return super.performKeyEquivalent(with: event) }
        let flags = event.modifierFlags.intersection(.deviceIndependentFlagsMask)
        switch (event.charactersIgnoringModifiers?.lowercased(), flags) {
        case ("s", .command): coordinator?.save()
        case ("d", .command): coordinator?.compare()
        case ("f", let f) where f == [.command, .shift]: coordinator?.find()
        case ("m", let f) where f == [.command, .option]: coordinator?.comment()
        default: return super.performKeyEquivalent(with: event)
        }
        return true
    }

    override func mouseDown(with event: NSEvent) {
        if event.modifierFlags.contains(.command) {
            coordinator?.declaration(at: convert(event.locationInWindow, from: nil))
            return
        }
        super.mouseDown(with: event)
    }

    override func menu(for event: NSEvent) -> NSMenu? {
        let menu = super.menu(for: event) ?? NSMenu()
        let item = NSMenuItem(title: "Comentar para a Luna", action: #selector(commentSelection), keyEquivalent: "")
        item.target = self
        item.isEnabled = selectedRange().length > 0
        menu.insertItem(item, at: 0)
        menu.insertItem(.separator(), at: 1)
        return menu
    }

    @objc private func commentSelection() { coordinator?.comment() }
}

/// The gutter: the line numbers (the caret's lighter) and, at its right edge, a bar for each range changed against the last commit.
final class FTRuler: NSRulerView {
    weak var coordinator: FTCodeCoordinator?
    var theme: FTTheme? { didSet { needsDisplay = true } }

    override init(scrollView: NSScrollView?, orientation: NSRulerView.Orientation) {
        super.init(scrollView: scrollView, orientation: orientation)
        ruleThickness = 52
    }

    required init(coder: NSCoder) { fatalError("init(coder:) has not been implemented") }

    override var isFlipped: Bool { true }

    private var textView: NSTextView? { clientView as? NSTextView }

    private func updateThickness() {
        guard let coordinator else { return }
        let digits = max(3, String(coordinator.lineStarts.count).count)
        let width = ("0" as NSString).size(withAttributes: [.font: coordinator.font]).width
        let wanted = ceil(CGFloat(digits) * width + 26)
        if abs(ruleThickness - wanted) > 0.5 { ruleThickness = wanted }
    }

    /// The top and bottom of line `line` in the ruler's coordinates.
    private func span(of line: Int) -> (CGFloat, CGFloat)? {
        guard let textView, let layout = textView.layoutManager, let coordinator, line >= 0, line < coordinator.lineStarts.count else { return nil }
        let length = textView.string.utf16.count
        let start = coordinator.lineStarts[line]
        let end = line + 1 < coordinator.lineStarts.count ? coordinator.lineStarts[line + 1] - 1 : length
        if start >= length {
            let extra = layout.extraLineFragmentRect
            guard extra != .zero else { return nil }
            return (convert(NSPoint(x: 0, y: extra.minY + textView.textContainerOrigin.y), from: textView).y, convert(NSPoint(x: 0, y: extra.maxY + textView.textContainerOrigin.y), from: textView).y)
        }
        let first = layout.lineFragmentRect(forGlyphAt: layout.glyphIndexForCharacter(at: start), effectiveRange: nil)
        let last = layout.lineFragmentRect(forGlyphAt: layout.glyphIndexForCharacter(at: max(min(end, length - 1), start)), effectiveRange: nil)
        let top = convert(NSPoint(x: 0, y: first.minY + textView.textContainerOrigin.y), from: textView).y
        let bottom = convert(NSPoint(x: 0, y: last.maxY + textView.textContainerOrigin.y), from: textView).y
        return (top, bottom)
    }

    /// Where the bar of hunk `index` is.
    func markerRect(_ index: Int) -> NSRect? {
        guard let coordinator, coordinator.hunks.indices.contains(index) else { return nil }
        let hunk = coordinator.hunks[index]
        let x = bounds.width - 11
        if hunk.kind == "deleted" {
            guard let (top, _) = span(of: hunk.start) else { return nil }
            return NSRect(x: x - 2, y: top - 4, width: 8, height: 8)
        }
        guard let (top, _) = span(of: hunk.start), let (_, bottom) = span(of: max(hunk.end - 1, hunk.start)) else { return nil }
        return NSRect(x: x - 2, y: top + 1, width: 8, height: max(bottom - top - 2, 4))
    }

    override func drawHashMarksAndLabels(in rect: NSRect) {
        guard let textView, let layout = textView.layoutManager, let container = textView.textContainer, let coordinator, let theme, let scroll = scrollView else { return }
        updateThickness()
        NSColor(hex: theme.background).setFill()
        bounds.fill()
        let length = textView.string.utf16.count
        let caretLine = coordinator.line(ofChar: min(textView.selectedRange().location, length))
        // the caret's row, across the numbers too
        if textView.selectedRange().length == 0, let (top, bottom) = span(of: caretLine) {
            NSColor(hex: theme.caretRow).setFill()
            NSRect(x: 0, y: top, width: bounds.width, height: bottom - top).fill()
        }
        let numberFont = coordinator.font
        func draw(_ line: Int, _ rect: NSRect) {
            let color = NSColor(hex: line == caretLine ? theme.lineNumberCaret : theme.lineNumber)
            let text = NSAttributedString(string: String(line + 1), attributes: [.font: numberFont, .foregroundColor: color])
            let size = text.size()
            let y = convert(NSPoint(x: 0, y: rect.minY + textView.textContainerOrigin.y), from: textView).y + (rect.height - size.height) / 2
            text.draw(at: NSPoint(x: bounds.width - 16 - size.width, y: y))
        }
        let glyphs = layout.glyphRange(forBoundingRect: scroll.contentView.bounds, in: container)
        layout.enumerateLineFragments(forGlyphRange: glyphs) { fragment, _, _, glyphRange, _ in
            let chars = layout.characterRange(forGlyphRange: glyphRange, actualGlyphRange: nil)
            let line = coordinator.line(ofChar: chars.location)
            if coordinator.lineStarts[line] == chars.location { draw(line, fragment) }
        }
        if length == 0 || textView.string.hasSuffix("\n"), layout.extraLineFragmentRect != .zero { draw(coordinator.lineStarts.count - 1, layout.extraLineFragmentRect) }
        for (index, hunk) in coordinator.hunks.enumerated() {
            guard let marker = markerRect(index), marker.maxY > 0, marker.minY < bounds.height else { continue }
            switch hunk.kind {
            case "deleted":
                let path = NSBezierPath()
                path.move(to: NSPoint(x: marker.minX + 1, y: marker.minY))
                path.line(to: NSPoint(x: marker.maxX + 1, y: marker.midY))
                path.line(to: NSPoint(x: marker.minX + 1, y: marker.maxY))
                NSColor(hex: theme.deleted).setFill()
                path.fill()
            default:
                NSColor(hex: hunk.kind == "added" ? theme.added : theme.modified).setFill()
                NSBezierPath(roundedRect: NSRect(x: marker.minX + 2, y: marker.minY, width: 4, height: marker.height), xRadius: 1, yRadius: 1).fill()
            }
        }
    }

    override func mouseDown(with event: NSEvent) {
        let point = convert(event.locationInWindow, from: nil)
        guard let coordinator else { return }
        for index in coordinator.hunks.indices {
            if let marker = markerRect(index), marker.insetBy(dx: -3, dy: -2).contains(point) {
                coordinator.showHunk(index, from: marker)
                return
            }
        }
    }
}

/// The popup of a change marker: the lines that were there and what can be done with the change.
struct FTHunkPopover: View {
    let hunk: FTHunk
    let index: Int
    let count: Int
    let theme: FTTheme
    let go: (Int) -> Void
    let revert: () -> Void
    let copy: () -> Void
    let diff: () -> Void

    var body: some View {
        VStack(alignment: .leading, spacing: 6) {
            HStack(spacing: 6) {
                Button { go(index - 1) } label: { Image(systemName: "chevron.up") }.disabled(index == 0).help("Anterior")
                Button { go(index + 1) } label: { Image(systemName: "chevron.down") }.disabled(index + 1 >= count).help("Próxima")
                Text("\(index + 1) de \(count)").font(.system(size: 11)).foregroundStyle(.secondary)
                Spacer()
                Button("Reverter", action: revert)
                Button("Copiar", action: copy)
                Button("Mostrar Diff", action: diff)
            }
            .controlSize(.small)
            if hunk.kind != "added" && !hunk.old.isEmpty {
                ScrollView([.vertical, .horizontal]) {
                    Text(hunk.old.hasSuffix("\n") ? String(hunk.old.dropLast()) : hunk.old)
                        .font(.system(size: 12, design: .monospaced))
                        .foregroundStyle(Color(hex: theme.text))
                        .padding(6)
                        .frame(maxWidth: .infinity, alignment: .leading)
                }
                .frame(maxHeight: 180)
                .background(Color(hex: theme.removedLine))
                .clipShape(RoundedRectangle(cornerRadius: 4))
            }
        }
        .padding(8)
        .frame(minWidth: 320, maxWidth: 520)
    }
}
