import AppKit
import SwiftUI

/// A card (a window or a note) in its own layer, over the drawings and the cards before it (paint_scene's order): its
/// paper or frame, the live view of a window under its title strip, a note's buttons and its pile's tabs. Its presses go
/// to Rust's canvas as the canvas's own, so the card in front takes them.
struct CanvasCard: View {
    let entry: CanvasEntry
    /// Where it shows (the canvas's points) and at what scale: its box on the canvas, or the enlarged window's frame.
    let place: CGRect
    let scale: CGFloat
    let view: CGSize
    let hand: String?
    let dark: Bool
    let handTool: Bool
    /// How far down a note's text is scrolled (its points).
    let scroll: CGFloat
    let onEvent: (CanvasInputEvent) -> Void
    /// The wheel over a note's text, by how much: true when it scrolled.
    let onWheel: (CGFloat) -> Bool

    var body: some View {
        let drawing = entry.drawing
        let rect = canvasRect(drawing.rect)
        // its own points: a window enlarged keeps its origin and takes the frame's size at its scale
        let local = CGRect(origin: rect.origin, size: CGSize(width: place.width / scale, height: place.height / scale))
        let viewport = CanvasViewport(x: place.minX - rect.minX * scale, y: place.minY - rect.minY * scale, zoom: scale)
        let shown = viewport.screen(canvasRect(drawing.reach).union(local)).intersection(CGRect(origin: .zero, size: view))
        if !shown.isNull && !shown.isEmpty {
            ZStack(alignment: .topLeading) {
                // ponytail: the card's picture covers what of it is in view, drawn again as the camera moves
                Canvas { context, _ in
                    var world = context
                    world.translateBy(x: viewport.x - shown.minX, y: viewport.y - shown.minY)
                    world.scaleBy(x: scale, y: scale)
                    paintEntry(&world, entry, hand: hand, image: nil, dark: dark, frame: local, scroll: scroll)
                }
                .frame(width: shown.width, height: shown.height)
                .offset(x: shown.minX, y: shown.minY)
                .allowsHitTesting(false)
                if let node = drawing.node {
                    window(node, local: local)
                } else if let note = drawing.note {
                    let card = viewport.screen(rect)
                    CanvasInput(handTool: handTool, onWheel: { wheel($0, note: note, card: card) }, onEvent: onEvent)
                        .frame(width: card.width, height: card.height)
                        .offset(x: card.minX, y: card.minY)
                    noteParts(note, viewport: viewport)
                }
            }
        }
    }

    /// A window: its live view below the title strip when its area has one (the strip is Rust's: it moves the window),
    /// otherwise the whole card is the canvas's.
    @ViewBuilder
    private func window(_ node: CanvasNode, local: CGRect) -> some View {
        if let live = CanvasArea.view(kind: node.kind, id: entry.id) {
            // a terminal's view has its own strip, so it covers the whole card
            let strip = node.kind == "terminal" ? 0 : min(node.header, local.height)
            let (header, top) = (strip * scale, strip == 0 ? 8 * scale : 0)
            if header > 0 {
                CanvasInput(handTool: handTool, onEvent: onEvent)
                    .frame(width: place.width, height: header)
                    .offset(x: place.minX, y: place.minY)
            }
            live.frame(width: local.width, height: max(local.height - strip, 0))
                .scaleEffect(scale, anchor: .topLeading)
                .frame(width: place.width, height: max(place.height - header, 0), alignment: .topLeading)
                .clipShape(UnevenRoundedRectangle(topLeadingRadius: top, bottomLeadingRadius: 8 * scale, bottomTrailingRadius: 8 * scale,
                                                  topTrailingRadius: top))
                .offset(x: place.minX, y: place.minY + header)
        } else {
            CanvasInput(handTool: handTool, onEvent: onEvent)
                .frame(width: place.width, height: place.height)
                .offset(x: place.minX, y: place.minY)
        }
    }

    /// notes/scroll.rs `note_wheel`: over the text (under the header) of a note the card cuts, the text scrolls.
    private func wheel(_ delta: CGFloat, note: CanvasNote, card: CGRect) -> Bool {
        guard let layout = entry.noteLayout, let base = CanvasInputView.base, let window = base.window else { return false }
        let (area, rect) = (canvasRect(note.area), canvasRect(entry.drawing.rect))
        // the header ends a padding above the text
        let header = (area.minY - (area.minX - rect.minX) - rect.minY) * scale
        let pointer = base.convert(window.mouseLocationOutsideOfEventStream, from: nil)
        guard layout.height - area.height > 0.5, pointer.y - card.minY >= header else { return false }
        return onWheel(delta / scale)
    }

    /// The buttons at a note's foot and its pile's tabs.
    @ViewBuilder
    private func noteParts(_ note: CanvasNote, viewport: CanvasViewport) -> some View {
        let ink = canvasColor(note.ink)
        ForEach(Array(note.buttons.enumerated()), id: \.offset) { index, button in
            let frame = viewport.screen(canvasRect(button.rect))
            NoteButtonView(label: button.label, ink: ink, scale: scale) {
                MagoStore.shared.send("ui.canvas.note_button", ["id": entry.id, "index": index])
            } remove: {
                MagoStore.shared.send("ui.canvas.note_button", ["id": entry.id, "index": index, "remove": true])
            }
            .help(button.tip)
            .frame(width: frame.width, height: frame.height)
            .offset(x: frame.minX, y: frame.minY)
        }
        ForEach(note.tabs, id: \.id) { tab in
            let frame = viewport.screen(canvasRect(tab.rect))
            NoteTabView(tab: tab, scale: scale)
                .frame(width: frame.width, height: frame.height)
                .offset(x: frame.minX, y: frame.minY)
        }
        if let more = note.more {
            let frame = viewport.screen(canvasRect(more.rect))
            Menu {
                ForEach(more.pages, id: \.id) { page in
                    Button(page.name) { showPage(page.id) }
                }
            } label: {
                NoteTabShape(outline: more.outline, origin: CGPoint(x: more.rect[0], y: more.rect[1]))
                    .fill(Color(nsColor: .windowBackgroundColor))
                    .overlay {
                        Text("+\(more.count)").font(Font(noteFont(10 * scale) as CTFont)).foregroundStyle(.secondary).fixedSize().rotationEffect(.degrees(90))
                    }
                    .frame(width: frame.width, height: frame.height)
            }
            .menuStyle(.button)
            .buttonStyle(.plain)
            .menuIndicator(.hidden)
            .help(more.label)
            .frame(width: frame.width, height: frame.height)
            .offset(x: frame.minX, y: frame.minY)
        }
    }
}

/// The pointing hand while over something to click (macOS 14 has no `pointerStyle`); `inside` back.
@MainActor
func linkHover(_ inside: Bool) -> Bool {
    if inside { NSCursor.pointingHand.push() } else { NSCursor.pop() }
    return inside
}

/// Shows a page of a pile (the MCP's `note_stack` show).
@MainActor
func showPage(_ id: String) {
    MagoStore.shared.send("note_stack", ["action": "show", "notes": [id]])
}

/// A tab's outline (`tab_edge`, world) in its own box.
struct NoteTabShape: Shape {
    let outline: [CGFloat]
    let origin: CGPoint

    func path(in rect: CGRect) -> Path {
        guard outline.count >= 4 else { return Path() }
        // the outline is at the tab's size in the world; the box is it at the zoom
        let width = (stride(from: 0, to: outline.count, by: 2).map { outline[$0] }.max() ?? origin.x) - origin.x
        let height = (stride(from: 1, to: outline.count, by: 2).map { outline[$0] }.max() ?? origin.y) - origin.y
        let scale = CGSize(width: width > 0 ? rect.width / width : 1, height: height > 0 ? rect.height / height : 1)
        var path = Path()
        // the page's side straight, the curve out and back (as tab_shape's mesh fills it)
        path.move(to: CGPoint(x: rect.minX, y: rect.minY))
        for index in stride(from: 0, to: outline.count - 1, by: 2) {
            path.addLine(to: CGPoint(x: rect.minX + (outline[index] - origin.x) * scale.width, y: rect.minY + (outline[index + 1] - origin.y) * scale.height))
        }
        path.addLine(to: CGPoint(x: rect.minX, y: rect.maxY))
        path.closeSubpath()
        return path
    }
}

/// A page's tab on a pile's rail (stack.rs `stack_tabs_ui`): the paper, or a back page's shade; its name down it; a click
/// shows the page.
struct NoteTabView: View {
    let tab: CanvasTab
    let scale: CGFloat
    @State private var hovering = false

    var body: some View {
        let shape = NoteTabShape(outline: tab.outline, origin: CGPoint(x: tab.rect[0], y: tab.rect[1]))
        let shade = tab.shade ?? []
        let room = max(tab.room * scale, 1)
        ZStack {
            if shade.count == 2 {
                shape.fill(LinearGradient(stops: [.init(color: canvasColor(shade[0]), location: 0), .init(color: canvasColor(shade[1]), location: 0.45)],
                                          startPoint: .leading, endPoint: .trailing))
            } else {
                shape.fill(canvasColor(tab.paper))
            }
            if hovering && !tab.front { shape.fill(Color.white.opacity(0.15)) }
            // turned a quarter clockwise: it reads from the top down, centered on the tab
            Text(tab.name)
                .font(Font(noteFont(10 * scale) as CTFont))
                .foregroundStyle(canvasColor(tab.ink))
                .lineLimit(1)
                .truncationMode(.tail)
                .frame(width: room)
                .rotationEffect(.degrees(90))
        }
        .contentShape(shape)
        .onHover { hovering = linkHover($0) }
        .onTapGesture { showPage(tab.id) }
    }
}

/// A note's button (notes/buttons.rs): runs its action; its menu removes it.
struct NoteButtonView: View {
    let label: String
    let ink: Color
    let scale: CGFloat
    let run: () -> Void
    let remove: () -> Void
    @State private var hovering = false

    var body: some View {
        let shape = RoundedRectangle(cornerRadius: 10 * scale)
        Button(action: run) {
            Text(label)
                .font(.app(11 * scale))
                .foregroundStyle(ink)
                .lineLimit(1)
                .frame(maxWidth: .infinity, maxHeight: .infinity)
                .background(shape.fill(ink.opacity(hovering ? 0.22 : 0.12)))
                .overlay(shape.strokeBorder(ink.opacity(0.35), lineWidth: scale))
                .contentShape(shape)
        }
        .buttonStyle(.plain)
        .onHover { hovering = linkHover($0) }
        .contextMenu {
            Button("Remover botão", systemImage: "trash", role: .destructive, action: remove)
        }
    }
}
