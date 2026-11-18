import AppKit
import SwiftUI

/// The app's blue (PRIMARY).
let canvasPrimary = Color(.sRGB, red: 0x0a / 255, green: 0x84 / 255, blue: 0xff / 255)

/// Where the world is in the view: screen = world × zoom + (x, y).
struct CanvasViewport: Equatable {
    var x: CGFloat = 0
    var y: CGFloat = 0
    var zoom: CGFloat = 1

    func screen(_ x: CGFloat, _ y: CGFloat) -> CGPoint {
        CGPoint(x: x * zoom + self.x, y: y * zoom + self.y)
    }

    func screen(_ rect: CGRect) -> CGRect {
        CGRect(origin: screen(rect.minX, rect.minY), size: CGSize(width: rect.width * zoom, height: rect.height * zoom))
    }

    /// The part of the world in a view of `size`.
    func world(_ size: CGSize) -> CGRect {
        CGRect(x: -x / zoom, y: -y / zoom, width: size.width / zoom, height: size.height / zoom)
    }

    /// Zooms by `factor` keeping the world point under `anchor` where it is (zoom_at: 10% to 3000%).
    mutating func zoom(by factor: CGFloat, at anchor: CGPoint) {
        let next = min(max(zoom * factor, 0.1), 30)
        let world = CGPoint(x: (anchor.x - x) / zoom, y: (anchor.y - y) / zoom)
        zoom = next
        x = anchor.x - world.x * next
        y = anchor.y - world.y * next
    }
}

/// The graph paper: GRID (20) apart, every fifth line when a cell would be under 6 points.
func paintGrid(_ context: inout GraphicsContext, size: CGSize, camera: CanvasViewport, color: Color) {
    var step = 20 * camera.zoom
    if step < 6 { step *= 5 }
    var path = Path()
    var x = camera.x.truncatingRemainder(dividingBy: step)
    while x <= size.width {
        path.move(to: CGPoint(x: x, y: 0))
        path.addLine(to: CGPoint(x: x, y: size.height))
        x += step
    }
    var y = camera.y.truncatingRemainder(dividingBy: step)
    while y <= size.height {
        path.move(to: CGPoint(x: 0, y: y))
        path.addLine(to: CGPoint(x: size.width, y: y))
        y += step
    }
    context.stroke(path, with: .color(color), lineWidth: 0.5)
}

/// One element in world coordinates (the context carries the camera).
func paintEntry(_ context: inout GraphicsContext, _ entry: CanvasEntry, hand: String?, image: NSImage?, dark: Bool) {
    let drawing = entry.drawing
    let rect = canvasRect(drawing.rect)
    var context = context
    if drawing.angle != 0 {
        context.translateBy(x: rect.midX, y: rect.midY)
        context.rotate(by: .radians(drawing.angle))
        context.translateBy(x: -rect.midX, y: -rect.midY)
    }
    if let node = drawing.node {
        paintNodeFrame(&context, rect: rect, node: node, dark: dark)
        return
    }
    if drawing.image {
        if let image { context.draw(Image(nsImage: image), in: rect) } else { context.stroke(Path(rect), with: .color(.gray), lineWidth: 1) }
        return
    }
    for painted in entry.painted {
        switch painted {
        case let .fill(path, color): context.fill(path, with: .color(color))
        case let .stroke(path, color, style): context.stroke(path, with: .color(color), style: style)
        case let .text(op): paintText(&context, op, hand: hand)
        }
    }
    if let note = drawing.note, let text = entry.noteText {
        let area = canvasRect(note.area)
        var clipped = context
        clipped.clip(to: Path(area))
        let font: Font = note.raw ? .system(size: note.size, design: .monospaced) : .system(size: note.size)
        clipped.draw(Text(text).font(font).foregroundStyle(canvasColor(note.ink)), in: area)
    }
}

/// `value`'s lines from (x, y), LINE_HEIGHT (1.25) apart; centered in `center` when it is above zero; a halo in the
/// ground's color first when it crosses lines.
func paintText(_ context: inout GraphicsContext, _ op: CanvasOp, hand: String?) {
    guard let value = op.value, let x = op.x, let y = op.y, let size = op.size else { return }
    let font: Font = op.hand == true ? .custom(hand ?? "Excalifont", size: size) : .system(size: size)
    for (index, line) in value.split(separator: "\n", omittingEmptySubsequences: false).enumerated() {
        let text = context.resolve(Text(String(line)).font(font).foregroundStyle(canvasColor(op.color)))
        var at = CGPoint(x: x, y: y + CGFloat(index) * size * 1.25)
        if let center = op.center, center > 0 {
            at.x += max(0, center - text.measure(in: CGSize(width: CGFloat.infinity, height: .infinity)).width) / 2
        }
        if let halo = op.halo {
            let ring = context.resolve(Text(String(line)).font(font).foregroundStyle(canvasColor(halo)))
            let radius = min(max(size * 0.08, 1), 3)
            for step in 0..<12 {
                let angle = Double(step) / 12 * 2 * .pi
                context.draw(ring, at: CGPoint(x: at.x + cos(angle) * radius, y: at.y + sin(angle) * radius), anchor: .topLeading)
            }
        }
        context.draw(text, at: at, anchor: .topLeading)
    }
}

/// A window's card: the body, its title strip with the name; the hosted view (MagoViews) covers the body when there is one.
// ponytail: a plain card for every kind; each window's own header (icons, buttons, status) belongs to its area's view
func paintNodeFrame(_ context: inout GraphicsContext, rect: CGRect, node: CanvasNode, dark: Bool) {
    let body = Path(roundedRect: rect, cornerRadius: 8)
    context.fill(body, with: .color(dark ? Color(white: 0.12) : .white))
    let header = CGRect(x: rect.minX, y: rect.minY, width: rect.width, height: min(node.header, rect.height))
    var strip = context
    strip.clip(to: body)
    strip.fill(Path(header), with: .color(dark ? Color(white: 0.17) : Color(white: 0.95)))
    context.stroke(body, with: .color(Color.gray.opacity(0.4)), lineWidth: 1)
    let name = context.resolve(Text(node.name).font(.system(size: 11, weight: .medium)).foregroundStyle(dark ? Color(white: 0.85) : Color(white: 0.25)))
    context.draw(name, at: CGPoint(x: rect.minX + 10, y: header.midY), anchor: .leading)
}

/// What goes over the scene, in screen coordinates (sizes as egui's paint_scene: handles 8, rotation 4.5, points 5 or 10).
func paintOverlay(_ context: inout GraphicsContext, _ overlay: CanvasOverlay, camera: CanvasViewport, hand: String?) {
    let screen = { (point: [CGFloat]) -> CGPoint in point.count >= 2 ? camera.screen(point[0], point[1]) : .zero }
    let handleFill: Color = overlay.dark ? Color(white: 0.07) : .white
    for window in overlay.windows {
        let rect = camera.screen(canvasRect(window)).insetBy(dx: -4, dy: -4)
        context.stroke(Path(roundedRect: rect, cornerRadius: 8), with: .color(canvasPrimary), style: StrokeStyle(lineWidth: 1, dash: [6, 5]))
    }
    if let selection = overlay.selection {
        let corners = selection.corners.map(screen)
        var box = Path()
        box.addLines(corners)
        box.closeSubpath()
        context.stroke(box, with: .color(canvasPrimary), lineWidth: 1)
        if selection.handles {
            for corner in corners {
                let handle = Path(roundedRect: CGRect(x: corner.x - 4, y: corner.y - 4, width: 8, height: 8), cornerRadius: 2)
                context.fill(handle, with: .color(handleFill))
                context.stroke(handle, with: .color(canvasPrimary), lineWidth: 1)
            }
            if let rotation = selection.rotation {
                let point = screen(rotation)
                let circle = Path(ellipseIn: CGRect(x: point.x - 4.5, y: point.y - 4.5, width: 9, height: 9))
                context.fill(circle, with: .color(handleFill))
                context.stroke(circle, with: .color(canvasPrimary), lineWidth: 1)
            }
        }
    }
    for midpoint in overlay.midpoints {
        let point = screen(midpoint)
        context.fill(Path(ellipseIn: CGRect(x: point.x - 5, y: point.y - 5, width: 10, height: 10)), with: .color(canvasPrimary.opacity(0.6)))
    }
    let radius: CGFloat = overlay.editor ? 10 : 5
    for (index, point) in overlay.points.map(screen).enumerated() {
        let circle = Path(ellipseIn: CGRect(x: point.x - radius, y: point.y - radius, width: radius * 2, height: radius * 2))
        context.fill(circle, with: .color(overlay.picked == index ? canvasPrimary.opacity(0.9) : Color.white.opacity(0.9)))
        context.stroke(circle, with: .color(canvasPrimary), lineWidth: 1)
    }
    for guide in overlay.guides where guide.count == 4 {
        var line = Path()
        line.addLines([camera.screen(guide[0], guide[1]), camera.screen(guide[2], guide[3])])
        context.stroke(line, with: .color(Color(.sRGB, red: 1, green: 0x6b / 255, blue: 0x6b / 255)), lineWidth: 1)
    }
    if overlay.binding.count >= 4 {
        let path = canvasPath(overlay.binding, closed: true).applying(CGAffineTransform(a: camera.zoom, b: 0, c: 0, d: camera.zoom, tx: camera.x, ty: camera.y))
        let blue = overlay.dark ? Color(.sRGB, red: 104 / 255, green: 182 / 255, blue: 240 / 255) : Color(.sRGB, red: 106 / 255, green: 189 / 255, blue: 252 / 255)
        context.stroke(path, with: .color(blue), lineWidth: 2)
    }
    if let marquee = overlay.marquee {
        let rect = camera.screen(canvasRect(marquee).standardized)
        context.fill(Path(rect), with: .color(canvasPrimary.opacity(0.08)))
        context.stroke(Path(rect), with: .color(canvasPrimary), lineWidth: 1)
    }
    if overlay.lasso.count >= 4 {
        let path = canvasPath(overlay.lasso, closed: false).applying(CGAffineTransform(a: camera.zoom, b: 0, c: 0, d: camera.zoom, tx: camera.x, ty: camera.y))
        context.stroke(path, with: .color(canvasPrimary), style: StrokeStyle(lineWidth: 1, dash: [4, 4]))
    }
    if let placing = overlay.placing {
        let rect = Path(roundedRect: camera.screen(canvasRect(placing).standardized), cornerRadius: 8)
        context.fill(rect, with: .color(canvasPrimary.opacity(0.08)))
        context.stroke(rect, with: .color(canvasPrimary), lineWidth: 1)
    }
    if let draft = overlay.draft {
        var world = context
        world.translateBy(x: camera.x, y: camera.y)
        world.scaleBy(x: camera.zoom, y: camera.zoom)
        paintEntry(&world, CanvasEntry(id: "draft", drawing: draft), hand: hand, image: nil, dark: overlay.dark)
    }
}
