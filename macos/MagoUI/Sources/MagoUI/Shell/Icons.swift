import SwiftUI

/// The app's own icons (`src/app/ui/icons.rs`): the same marks egui paints, on its 16-point grid around the middle, in
/// 1.25-point strokes with round corners.
enum MagoIcon: Equatable {
    /// A tool, by its id on the wire ("select", "filetree").
    case tool(String)
    case shapes, composer, menu, gear, plus, minus, undo, redo, open, search, fit, map, check, more, diff
    case trash, duplicate, group, ungroup, edit, code, export, download, moon, agents
    case layers(Bool), lock(Bool)
    // the properties row's choices
    case width(Double), dash(String), sloppiness(Int), edge(Bool), hachure, crossHatch, solidFill, arrowType(Bool), label(String)
    /// An arrowhead ("none", "triangle_outline"...) at the end, mirrored for the start.
    case head(String, flip: Bool)
}

/// An icon `scale` times its 16 points, in the foreground style.
struct IconView: View {
    let icon: MagoIcon
    var scale: CGFloat = 1

    var body: some View {
        Canvas { context, size in
            var context = context
            context.translateBy(x: size.width / 2, y: size.height / 2)
            context.scaleBy(x: scale, y: scale)
            IconPen(context: context).draw(icon)
        }
        .frame(width: 20 * scale, height: 20 * scale)
        .allowsHitTesting(false)
        .accessibilityHidden(true)
    }
}

private struct IconPen {
    let context: GraphicsContext
    private let ink = GraphicsContext.Shading.foreground
    private let stroke = StrokeStyle(lineWidth: 1.25, lineCap: .round, lineJoin: .round)

    init(context: GraphicsContext) { self.context = context }

    private func points(_ list: [(CGFloat, CGFloat)], closed: Bool) -> Path {
        Path { path in
            path.addLines(list.map { CGPoint(x: $0.0, y: $0.1) })
            if closed { path.closeSubpath() }
        }
    }

    private func line(_ list: [(CGFloat, CGFloat)]) { context.stroke(points(list, closed: false), with: ink, style: stroke) }
    private func closed(_ list: [(CGFloat, CGFloat)]) { context.stroke(points(list, closed: true), with: ink, style: stroke) }
    private func rect(_ x: CGFloat, _ y: CGFloat, _ width: CGFloat, _ height: CGFloat, _ radius: CGFloat) {
        context.stroke(Path(roundedRect: CGRect(x: x, y: y, width: width, height: height), cornerRadius: radius), with: ink, style: stroke)
    }
    private func circle(_ x: CGFloat, _ y: CGFloat, _ radius: CGFloat) {
        context.stroke(Path(ellipseIn: CGRect(x: x - radius, y: y - radius, width: 2 * radius, height: 2 * radius)), with: ink, style: stroke)
    }
    private func dot(_ x: CGFloat, _ y: CGFloat, _ radius: CGFloat) {
        context.fill(Path(ellipseIn: CGRect(x: x - radius, y: y - radius, width: 2 * radius, height: 2 * radius)), with: ink)
    }

    func draw(_ icon: MagoIcon) {
        switch icon {
        case .tool(let id): tool(id)
        case .shapes:
            rect(-7, -1, 8, 8, 1)
            circle(2.5, -2.5, 4.5)
        case .composer:
            rect(-7, -6, 14, 9, 2)
            line([(-4, -2), (3, -2)])
            for x: CGFloat in [-5, 0, 5] { dot(x, 6, 1.2) }
        case .menu:
            for y: CGFloat in [-5, 0, 5] { line([(-7, y), (7, y)]) }
        case .gear:
            // eight teeth around a hub: two points out, two in, per tooth
            let step = 2 * CGFloat.pi / 32
            closed((0..<32).map { k in
                let (angle, radius) = ((CGFloat(k) - 0.5) * step, k % 4 < 2 ? 7.0 : 5.2)
                return (radius * cos(angle), radius * sin(angle))
            })
            circle(0, 0, 2.3)
        case .plus:
            line([(-6, 0), (6, 0)])
            line([(0, -6), (0, 6)])
        case .minus: line([(-6, 0), (6, 0)])
        case .undo, .redo:
            let m: CGFloat = icon == .undo ? 1 : -1
            line([(-3 * m, -5), (-6 * m, -2), (-3 * m, 1)])
            line([(-6 * m, -2), (2 * m, -2), (5 * m, 0), (6 * m, 3), (5 * m, 5), (2 * m, 6), (-m, 6)])
        case .open: closed([(-7, -5), (-2, -5), (0, -3), (7, -3), (7, 5.5), (-7, 5.5)])
        case .search:
            circle(-1, -1, 5)
            line([(2.5, 2.5), (6.5, 6.5)])
        case .fit:
            for (x, y): (CGFloat, CGFloat) in [(-1, -1), (1, -1), (-1, 1), (1, 1)] { line([(2.5 * x, 6.5 * y), (6.5 * x, 6.5 * y), (6.5 * x, 2.5 * y)]) }
        case .map:
            closed([(-7, -5), (-2.5, -7), (2.5, -5), (7, -7), (7, 5), (2.5, 7), (-2.5, 5), (-7, 7)])
            line([(-2.5, -7), (-2.5, 5)])
            line([(2.5, -5), (2.5, 7)])
        case .check: line([(-5.5, 0.5), (-1.5, 4.5), (6, -4.5)])
        case .more:
            for y: CGFloat in [-5, 0, 5] { dot(0, y, 1.3) }
        case .diff:
            line([(-4, -6.5), (-4, -0.5)])
            line([(-7, -3.5), (-1, -3.5)])
            line([(1, 4), (7, 4)])
            line([(-6, 6.5), (6, -6.5)])
        case .trash:
            line([(-6.5, -4), (6.5, -4)])
            line([(-2, -4), (-2, -6.5), (2, -6.5), (2, -4)])
            line([(-5, -4), (-4, 6.5), (4, 6.5), (5, -4)])
            line([(-1.5, -1), (-1.5, 4)])
            line([(1.5, -1), (1.5, 4)])
        case .duplicate:
            rect(-6, -6, 9, 9, 2)
            rect(-3, -3, 9, 9, 2)
        case .group, .ungroup:
            rect(-6.5, -6.5, 8, 8, 1)
            rect(-1.5, -1.5, 8, 8, 1)
            if icon == .group {
                context.stroke(Path(CGRect(x: -7.5, y: -7.5, width: 15, height: 15)), with: ink, style: StrokeStyle(lineWidth: 1, dash: [1.5, 1.5]))
            }
        case .edit:
            rect(-6.5, -5, 11.5, 11.5, 2)
            line([(-1.5, 2), (6.5, -6)])
            line([(-1.5, 2), (-2, 3)])
        case .code:
            line([(-2.5, -4.5), (-6, 0), (-2.5, 4.5)])
            line([(2.5, -4.5), (6, 0), (2.5, 4.5)])
            line([(1.5, -5.5), (-1.5, 5.5)])
        case .export:
            rect(-6.5, -5.5, 13, 11, 2)
            line([(-6.5, 3), (-2.5, -1), (1, 2.5), (3, 0.5), (6.5, 4)])
            circle(2.5, -2.5, 1.2)
        case .download:
            line([(-6, 3), (-6, 6), (6, 6), (6, 3)])
            line([(0, -6), (0, 2.5)])
            line([(-3.5, -1), (0, 2.5), (3.5, -1)])
        case .moon:
            let outer = Path(ellipseIn: CGRect(x: -6.5, y: -6.5, width: 13, height: 13))
            let bite = Path(ellipseIn: CGRect(x: 3.6 - 6.3, y: -3 - 6.3, width: 12.6, height: 12.6))
            context.stroke(outer.subtracting(bite), with: ink, style: stroke)
        case .agents:
            for y: CGFloat in [-4.5, 0, 4.5] {
                dot(-5, y, 1.4)
                line([(-2, y), (6, y)])
            }
        case .layers(let three):
            closed([(0, -7), (7, -3.5), (0, 0), (-7, -3.5)])
            line([(-7, 2), (0, 5.5), (7, 2)])
            if three { line([(-7, 5.5), (0, 9), (7, 5.5)]) }
        case .lock(let locked):
            rect(-5, -1, 10, 8, 2)
            line([(-3, -1), (-3, -4), (-2, -6), (0, -6.8), (2, -6), (3, -4)] + (locked ? [(3, -1)] : []))
            dot(0, 3, 1)
        case .width(let width):
            context.stroke(points([(-6.5, 0), (6.5, 0)], closed: false), with: ink, lineWidth: max(width, 1.25) * 0.9 + 0.4)
        case .dash(let style):
            let dash: [CGFloat] = style == "dashed" ? [3.5, 2.5] : style == "dotted" ? [1.5, 2.5] : []
            context.stroke(points([(-7, 0), (7, 0)], closed: false), with: ink, style: StrokeStyle(lineWidth: 2, dash: dash))
        case .sloppiness(0): line([(-7, 3), (-3, -1.5), (3, -1.5), (7, 3)])
        case .sloppiness(1):
            line([(-7, 3), (-3.5, -1), (0, 1.5), (3.5, -2), (7, 2.5)])
            line([(-6.5, 1.5), (-3, -2.5), (0.5, 0), (4, -3), (7, 1)])
        case .sloppiness: line([(-7, 4), (-4, -3), (-1, 3), (2, -3.5), (5, 2.5), (7, -1)])
        case .edge(let round):
            line(round ? [(-6, 6), (-6, -1), (-5, -4.5), (-1, -6), (6, -6)] : [(-6, 6), (-6, -6), (6, -6)])
            context.stroke(points([(-6, 6), (6, 6), (6, -6)], closed: false), with: ink, style: StrokeStyle(lineWidth: 1, dash: [1.5, 2]))
        case .solidFill: context.fill(Path(roundedRect: CGRect(x: -6.5, y: -6.5, width: 13, height: 13), cornerRadius: 2), with: ink)
        case .hachure, .crossHatch:
            rect(-6.5, -6.5, 13, 13, 2)
            // each stroke's ends kept inside the square, as egui clamps them
            let clamp = { (x: CGFloat, y: CGFloat) in (min(max(x, -6.5), 6.5), min(max(y, -6.5), 6.5)) }
            for offset in stride(from: -7.0, through: 7.0, by: 3.5) {
                line([clamp(offset - 4, 5), clamp(offset + 4, -5)])
                if icon == .crossHatch { line([clamp(offset - 4, -5), clamp(offset + 4, 5)]) }
            }
        case .arrowType(let curved):
            line(curved ? [(-6, 6), (-5, 0), (-2, -4), (3, -6), (6.5, -6)] : [(-6, 6), (6, -6)])
            line([(2, -6.5), (6.5, -6), (6, -1.5)])
        case .label(let text):
            var resolved = context.resolve(Text(text).font(.app(14)))
            resolved.shading = ink
            context.draw(resolved, at: .zero)
        case .head(let head, let flip):
            let m: CGFloat = flip ? -1 : 1
            let back = { (dx: CGFloat, dy: CGFloat) in ((6.5 - dx) * m, dy) }
            line([(-6.5 * m, 0), (6.5 * m, 0)])
            switch head {
            case "arrow": line([(2 * m, -3.5), (6.5 * m, 0), (2 * m, 3.5)])
            case "bar": line([(6.5 * m, -4), (6.5 * m, 4)])
            case "triangle", "triangle_outline", "diamond", "diamond_outline":
                let shape = head.hasPrefix("triangle")
                    ? points([(6.5 * m, 0), back(5, -3), back(5, 3)], closed: true)
                    : points([(6.5 * m, 0), back(3, -2.5), back(6, 0), back(3, 2.5)], closed: true)
                if !head.hasSuffix("_outline") { context.fill(shape, with: ink) }
                context.stroke(shape, with: ink, style: stroke)
            case "circle", "circle_outline":
                let (x, y) = back(2.5, 0)
                if head == "circle" { dot(x, y, 2.5) }
                circle(x, y, 2.5)
            default: break
            }
        }
    }

    private func tool(_ id: String) {
        switch id {
        case "hand":
            line([(-3, 1), (-3, -5)])
            line([(0, 0), (0, -6.5)])
            line([(3, 0), (3, -5)])
            line([(5.5, 1), (5.5, -2.5)])
            line([(-3, 0), (-5.5, -2), (-6.5, 0), (-3.5, 5), (0, 7), (3, 7), (5.5, 4), (5.5, 1)])
        case "select":
            closed([(-4.5, -6.5), (5.5, 1), (1, 2), (-1.5, 6.5)])
            line([(1, 2), (4, 6.5)])
        case "rectangle": rect(-6, -6, 12, 12, 2)
        case "diamond": closed([(0, -7), (7, 0), (0, 7), (-7, 0)])
        case "ellipse": circle(0, 0, 6.5)
        case "arrow":
            line([(-7, 0), (7, 0)])
            line([(3, -4), (7, 0), (3, 4)])
        case "line": line([(-7, 0), (7, 0)])
        case "draw":
            closed([(-6, 6), (-6, 3), (3.5, -6.5), (6.5, -3.5), (-3, 6)])
            line([(1.5, -4.5), (4.5, -1.5)])
        case "text":
            line([(-6, 6.5), (0, -6.5), (6, 6.5)])
            line([(-3.5, 2), (3.5, 2)])
            line([(-2, -6.5), (2, -6.5)])
        case "sticky":
            closed([(-6.5, -6.5), (6.5, -6.5), (6.5, 2), (2, 6.5), (-6.5, 6.5)])
            line([(6.5, 2), (2, 2), (2, 6.5)])
            line([(-3.5, -2.5), (3, -2.5)])
            line([(-3.5, 0.5), (0, 0.5)])
        case "autoshape":
            circle(-1.5, -1.5, 4.5)
            rect(-0.5, -0.5, 7, 7, 2)
        case "laser":
            line([(-6.5, 6.5), (2, -2)])
            line([(-5, 7), (3.5, -1.5)])
            for (x, y): (CGFloat, CGFloat) in [(4.5, -6.5), (6.5, -4.5), (6, -7), (2.5, -7), (7, -2)] { dot(x, y, 0.8) }
        case "fill":
            closed([(-4, -3), (1, -7), (6, -2), (1, 3)])
            line([(-4, -3), (-5.5, 1)])
            dot(-5.5, 4.5, 1.6)
        case "lasso":
            line([(-1, 3), (-5, 1), (-6.5, -2.5), (-4, -6), (1.5, -6.5), (6, -4), (6.5, -0.5), (3, 2.5), (-1, 3)])
            line([(-1, 3), (-2.5, 6.5)])
        case "portal":
            circle(0, 0, 7)
            line([(-7, 0), (7, 0)])
            line([(-6, -3.5), (6, -3.5)])
            line([(-6, 3.5), (6, 3.5)])
            let meridian: [(CGFloat, CGFloat)] = (0...12).map { k in
                let angle = -CGFloat.pi / 2 + CGFloat(k) * .pi / 12
                return (3.2 * cos(angle), 7 * sin(angle))
            }
            line(meridian)
            line(meridian.map { (-$0.0, $0.1) })
        case "filetree": closed([(-7, -5), (-2.5, -5), (-1, -3.5), (7, -3.5), (7, 5.5), (-7, 5.5)])
        // a speech balloon, its tail down at the left
        case "chat": closed([(-7, -6), (7, -6), (7, 3), (-0.5, 3), (-4, 6.5), (-4, 3), (-7, 3)])
        case "terminal":
            closed([(-7, -6), (7, -6), (7, 6), (-7, 6)])
            line([(-4, -2.5), (-1.5, 0), (-4, 2.5)])
            line([(0.5, 2.5), (4, 2.5)])
        case "frame":
            line([(-3, -7), (-3, 7)])
            line([(3, -7), (3, 7)])
            line([(-7, -3), (7, -3)])
            line([(-7, 3), (7, 3)])
        case "eraser":
            closed([(-6.5, 2.5), (1.5, -5.5), (6.5, -0.5), (0.5, 5.5), (-3.5, 5.5)])
            line([(-2.5, -1.5), (2.5, 3.5)])
            line([(0.5, 5.5), (6.5, 5.5)])
        default: break
        }
    }
}
