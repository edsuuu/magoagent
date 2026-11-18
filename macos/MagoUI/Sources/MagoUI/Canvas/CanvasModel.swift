import AppKit
import CoreText
import SwiftUI

// The snapshots Rust publishes (src/app/canvas/native.rs), in world coordinates. Every key is one word: they decode
// the same with or without .convertFromSnakeCase.

struct CanvasItem: Decodable, Equatable {
    let id: String
    let rev: UInt64
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

struct CanvasNote: Decodable {
    let text: String?
    let ink: [UInt8]
    let size: CGFloat
    let area: [CGFloat]
    let raw: Bool
    let blurred: Bool
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
    let note: Bool
    let center: Bool
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

/// An element ready to paint: its paths built once per revision.
struct CanvasEntry {
    enum Painted {
        case fill(Path, Color)
        case stroke(Path, Color, StrokeStyle)
        case text(CanvasOp)
    }

    let id: String
    let drawing: CanvasDrawing
    let painted: [Painted]
    let noteText: AttributedString?

    init(id: String, drawing: CanvasDrawing) {
        self.id = id
        self.drawing = drawing
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
        noteText = drawing.note.flatMap { note in
            guard let text = note.text, !note.blurred else { return nil }
            if note.raw { return AttributedString(text) }
            let options = AttributedString.MarkdownParsingOptions(interpretedSyntax: .inlineOnlyPreservingWhitespace)
            return (try? AttributedString(markdown: text, options: options)) ?? AttributedString(text)
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
                entries[item.id] = CanvasEntry(id: item.id, drawing: drawing)
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
