import AppKit
import CoreText
import SwiftUI

// The snapshots Rust publishes (src/app/canvas/native/), in world coordinates. Every key is one word: they decode
// the same with or without .convertFromSnakeCase.

struct CanvasItem: Decodable, Equatable {
    let id: String
    let rev: UInt64
    /// A window or a note: it stacks over the drawings, each over the cards before it.
    let card: Bool
}

/// Topic "scene": the elements in paint order, each with its revision.
struct CanvasSceneList: Decodable, Equatable {
    let version: UInt64
    let ground: [UInt8]
    let grid: [UInt8]?
    let items: [CanvasItem]
}

/// One painted step: "fill", "stroke" or "text".
struct CanvasOp: Decodable {
    let op: String
    let points: [CGFloat]?
    let color: [UInt8]
    let width: CGFloat?
    let dash: [CGFloat]?
    let closed: Bool?
    let value: String?
    let x: CGFloat?
    let y: CGFloat?
    let size: CGFloat?
    let hand: Bool?
    let center: CGFloat?
    let halo: [UInt8]?
}

struct CanvasNode: Decodable {
    let kind: String
    let name: String
    let header: CGFloat
}

/// A run of a note's text as egui's note editor lays it out (`markdown::live_runs`): characters `from..<to` (Unicode
/// scalars), its size and its line's, the share of the ink (0: out of sight) and of the ink under it.
struct CanvasRun: Decodable, Equatable {
    let from: Int
    let to: Int
    let size: CGFloat
    let line: CGFloat
    let ink: CGFloat
    let bold: Bool
    let italic: Bool
    let strike: Bool
    let underline: Bool
    let back: CGFloat
}

/// What goes over a hidden mark: "bullet", "task", "quote" or "rule", at character `at`.
struct CanvasMark: Decodable, Equatable {
    let kind: String
    let at: Int
    let done: Bool
}

struct CanvasNoteButton: Decodable {
    let label: String
    let tip: String
    let rect: [CGFloat]
}

/// A page's tab on a pile's rail: its outline (world), the paper or the shade of a back tab, its name down it.
struct CanvasTab: Decodable {
    let id: String
    let rect: [CGFloat]
    let outline: [CGFloat]
    let paper: [UInt8]
    let shade: [[UInt8]]?
    let ink: [UInt8]
    let name: String
    let room: CGFloat
    let front: Bool
}

struct CanvasPage: Decodable {
    let id: String
    let name: String
}

/// "Mais N páginas": the last tab, the pages past the rail in its menu.
struct CanvasMore: Decodable {
    let rect: [CGFloat]
    let outline: [CGFloat]
    let count: Int
    let label: String
    let pages: [CanvasPage]
}

struct CanvasNote: Decodable {
    let text: String?
    let ink: [UInt8]
    let size: CGFloat
    let area: [CGFloat]
    let raw: Bool
    let blurred: Bool
    let runs: [CanvasRun]
    let marks: [CanvasMark]
    let buttons: [CanvasNoteButton]
    let tabs: [CanvasTab]
    let more: CanvasMore?
}

/// Topic "scene.<id>": one element.
struct CanvasDrawing: Decodable {
    let rev: UInt64
    let rect: [CGFloat]
    let reach: [CGFloat]
    let angle: CGFloat
    let ops: [CanvasOp]
    let node: CanvasNode?
    let note: CanvasNote?
    let image: Bool
}

struct CanvasCamera: Decodable, Equatable {
    let serial: UInt64
    let x: CGFloat
    let y: CGFloat
    let zoom: CGFloat
}

struct CanvasSelection: Decodable {
    let corners: [[CGFloat]]
    let handles: Bool
    let rotation: [CGFloat]?
}

struct CanvasEditing: Decodable {
    let id: String
    let value: String
    let rect: [CGFloat]
    let size: CGFloat
    let color: [UInt8]
    /// The note it is the text of (its id); nil for a text or a label.
    let note: String?
    let center: Bool
    let raw: Bool
    let runs: [CanvasRun]
    let marks: [CanvasMark]
}

/// The window's place on the canvas while it is enlarged (world), its title strip's height and the grey bars' widths.
struct CanvasSkeleton: Decodable {
    let rect: [CGFloat]
    let head: CGFloat
    let name: String
    let words: String
    let lines: [CGFloat]
}

/// The window enlarged, or on its way back: where it shows in the view and at what scale, the blur behind it, its
/// skeleton, the grip on its title, the docked edge and where it would dock (screen).
struct CanvasLift: Decodable {
    let id: String
    let frame: [CGFloat]
    let zoom: CGFloat
    let blur: CGFloat
    let skeleton: CanvasSkeleton?
    let grip: [CGFloat]?
    let edge: [CGFloat]?
    let zone: [CGFloat]?
}

struct CanvasMapNode: Decodable {
    let rect: [CGFloat]
    let fill: [UInt8]?
    let stroke: [UInt8]?
}

/// The minimap: the panel, the surface's inset, the distance from the canvas's bottom right corner, the dots and the
/// view in the surface's points.
struct CanvasMinimap: Decodable {
    let size: [CGFloat]
    let inset: CGFloat
    let right: CGFloat
    let bottom: CGFloat
    let nodes: [CanvasMapNode]
    let view: [CGFloat]
    let accent: [UInt8]
}

struct CanvasLinkMark: Decodable {
    let id: String
    let link: String
    let bounds: [CGFloat]
}

struct CanvasChain: Decodable {
    let fill: [UInt8]
    let hover: [UInt8]
    let ink: [UInt8]
}

/// Topic "canvas": the camera, the tool and what goes over the scene.
struct CanvasOverlay: Decodable {
    let camera: CanvasCamera
    let tool: String
    let dark: Bool
    let selection: CanvasSelection?
    let windows: [[CGFloat]]
    let points: [[CGFloat]]
    let midpoints: [[CGFloat]]
    let editor: Bool
    let picked: Int?
    let marquee: [CGFloat]?
    let lasso: [CGFloat]
    let guides: [[CGFloat]]
    let binding: [CGFloat]
    let draft: CanvasDrawing?
    let placing: [CGFloat]?
    let editing: CanvasEditing?
    let lift: CanvasLift?
    let minimap: CanvasMinimap?
    let links: [CanvasLinkMark]
    let chain: CanvasChain
    let back: String?
}

func canvasColor(_ rgba: [UInt8]) -> Color {
    guard rgba.count == 4 else { return .clear }
    return Color(.sRGB, red: Double(rgba[0]) / 255, green: Double(rgba[1]) / 255, blue: Double(rgba[2]) / 255, opacity: Double(rgba[3]) / 255)
}

func canvasRect(_ values: [CGFloat]) -> CGRect {
    values.count == 4 ? CGRect(x: values[0], y: values[1], width: values[2], height: values[3]) : .zero
}

func canvasPath(_ flat: [CGFloat], closed: Bool) -> Path {
    var path = Path()
    var index = 0
    while index + 1 < flat.count {
        let point = CGPoint(x: flat[index], y: flat[index + 1])
        if index == 0 { path.move(to: point) } else { path.addLine(to: point) }
        index += 2
    }
    if closed { path.closeSubpath() }
    return path
}


/// An element ready to paint: its paths built once per revision, a note's text laid out once per revision.
@MainActor final class CanvasEntry {
    enum Painted {
        case fill(Path, Color)
        case stroke(Path, Color, StrokeStyle)
        case text(CanvasOp)
    }

    let id: String
    let drawing: CanvasDrawing
    let card: Bool
    let painted: [Painted]
    /// The note's text at rest, laid out in its own points (CanvasNoteText.swift).
    private(set) lazy var noteLayout: NoteLayout? = drawing.note.flatMap { note in
        guard let text = note.text, !note.blurred, !text.isEmpty else { return nil }
        return NoteLayout(text: text, runs: note.runs, marks: note.marks, raw: note.raw, size: note.size, ink: note.ink, scale: 1,
                          width: canvasRect(note.area).width)
    }

    init(id: String, drawing: CanvasDrawing, card: Bool = false) {
        self.id = id
        self.drawing = drawing
        self.card = card
        painted = drawing.ops.map { op in
            switch op.op {
            case "fill":
                return .fill(canvasPath(op.points ?? [], closed: true), canvasColor(op.color))
            case "stroke":
                let style = StrokeStyle(lineWidth: op.width ?? 1, lineCap: .round, lineJoin: .round, dash: op.dash ?? [])
                return .stroke(canvasPath(op.points ?? [], closed: op.closed ?? false), canvasColor(op.color), style)
            default:
                return .text(op)
            }
        }
    }
}

/// The drawings Swift has, by element: decoded again only when an element's revision moves.
@MainActor final class CanvasCache {
    private(set) var entries: [String: CanvasEntry] = [:]
    var images: [String: (rev: UInt64, image: NSImage)] = [:]
    private var version: UInt64 = .max
    private var ordered: [CanvasEntry] = []

    /// The entries in paint order for `list`; an element whose topic is behind its revision keeps its last drawing.
    func sync(_ list: CanvasSceneList, store: MagoStore) -> [CanvasEntry] {
        let stale = list.items.contains { entries[$0.id]?.drawing.rev != $0.rev }
        if list.version == version && !stale { return ordered }
        for item in list.items where entries[item.id]?.drawing.rev != item.rev {
            if let drawing = store.decode(CanvasDrawing.self, topic: "scene.\(item.id)") {
                entries[item.id] = CanvasEntry(id: item.id, drawing: drawing, card: item.card)
            }
        }
        let alive = Set(list.items.map(\.id))
        entries = entries.filter { alive.contains($0.key) }
        images = images.filter { alive.contains($0.key) }
        version = list.version
        ordered = list.items.compactMap { entries[$0.id] }
        return ordered
    }
}

/// The hand font (Excalifont), handed over by Rust once and registered for this process.
@MainActor enum CanvasFonts {
    static var hand: String?

    static func register(store: MagoStore) async {
        guard hand == nil else { return }
        let reply = await store.act("ui.canvas.font")
        guard reply.ok, let data = Data(base64Encoded: reply.text), let provider = CGDataProvider(data: data as CFData),
              let font = CGFont(provider) else { return }
        CTFontManagerRegisterGraphicsFont(font, nil)
        hand = font.postScriptName as String?
    }
}
