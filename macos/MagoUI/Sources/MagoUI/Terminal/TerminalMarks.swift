import AppKit
import SwiftUI

/// The marks of a card's strip, drawn as the egui terminal draws them (src/app/ui/icons.rs `paint_icon`, 16-point grid at
/// 1.25, corners cut by 1.4) instead of the system's symbols.
enum TerminalMarks {
    /// One stroke: the points (a closed one joins its last to its first) in the icon's grid around (0, 0).
    struct Line {
        var points: [CGPoint]
        var closed = false
    }

    private static func line(_ points: [(CGFloat, CGFloat)], closed: Bool = false) -> Line {
        Line(points: points.map { CGPoint(x: $0.0, y: $0.1) }, closed: closed)
    }

    private static func box(_ x: CGFloat, _ y: CGFloat, _ width: CGFloat, _ height: CGFloat) -> Line {
        line([(x, y), (x + width, y), (x + width, y + height), (x, y + height)], closed: true)
    }

    /// The mark's strokes by the topic's `icon` key (the claude, codex and opencode marks are not lines).
    static func lines(_ icon: String) -> [Line] {
        switch icon {
        case "agents": return [-4.5, 0, 4.5].map { line([(-2, $0), (6, $0)]) }
        case "monitor": return [box(-7, -5.5, 14, 9.5), line([(-3, 6.5), (3, 6.5)]), line([(0, 4), (0, 6.5)])]
        case "search": return [line([(2.5, 2.5), (6.5, 6.5)])]
        case "composer": return [box(-7, -6, 14, 9), line([(-4, -2), (3, -2)])]
        case "palette": return [line([(1.5, -7), (-5, 1), (0, 1), (-1.5, 7), (5, -1), (0, -1)], closed: true)]
        case "grid":
            return [box(-6.5, -6.5, 13, 13), line([(-2.2, -6.5), (-2.2, 6.5)]), line([(2.2, -6.5), (2.2, 6.5)]),
                    line([(-6.5, -2.2), (6.5, -2.2)]), line([(-6.5, 2.2), (6.5, 2.2)])]
        case "eye": return [line([(-7, 0), (-4, -3.5), (0, -5), (4, -3.5), (7, 0), (4, 3.5), (0, 5), (-4, 3.5)], closed: true)]
        case "map":
            return [line([(-7, -5), (-2.5, -7), (2.5, -5), (7, -7), (7, 5), (2.5, 7), (-2.5, 5), (-7, 7)], closed: true),
                    line([(-2.5, -7), (-2.5, 5)]), line([(2.5, -5), (2.5, 7)])]
        case "expand":
            return [line([(1.5, -6.5), (6.5, -6.5), (6.5, -1.5)]), line([(6.5, -6.5), (1.5, -1.5)]),
                    line([(-1.5, 6.5), (-6.5, 6.5), (-6.5, 1.5)]), line([(-6.5, 6.5), (-1.5, 1.5)])]
        default: // the terminal
            return [box(-7, -6, 14, 12), line([(-4, -2.5), (-1.5, 0), (-4, 2.5)]), line([(0.5, 2.5), (4, 2.5)])]
        }
    }

    /// egui's `round_corners`: each inner corner cut into a short curve (`CORNER_ROUND` along both sides, or half the side).
    static func path(_ line: Line, at center: CGPoint) -> Path {
        let points = line.points.map { CGPoint(x: center.x + $0.x, y: center.y + $0.y) }
        var path = Path()
        guard points.count >= 3 else {
            if let first = points.first { path.move(to: first) }
            for point in points.dropFirst() { path.addLine(to: point) }
            return path
        }
        func toward(_ from: CGPoint, _ to: CGPoint, _ reach: CGFloat) -> CGPoint {
            let length = hypot(to.x - from.x, to.y - from.y)
            return CGPoint(x: from.x + (to.x - from.x) / length * reach, y: from.y + (to.y - from.y) / length * reach)
        }
        let count = points.count
        for index in 0..<count {
            let corner = points[index]
            if !line.closed && (index == 0 || index == count - 1) {
                if index == 0 { path.move(to: corner) } else { path.addLine(to: corner) }
                continue
            }
            let before = points[(index + count - 1) % count], after = points[(index + 1) % count]
            let reach = min(1.4, hypot(before.x - corner.x, before.y - corner.y) / 2, hypot(after.x - corner.x, after.y - corner.y) / 2)
            let start = toward(corner, before, reach), end = toward(corner, after, reach)
            if index == 0 { path.move(to: start) } else { path.addLine(to: start) }
            path.addQuadCurve(to: end, control: corner)
        }
        if line.closed { path.closeSubpath() }
        return path
    }
}

/// A mark of the strip in its 11-point slot: the lines of `TerminalMarks` (the egui icon's 16-point grid overflows the slot,
/// centered on it), Claude's orange mark and OpenAI's in the text color, and OpenCode's frame.
struct TerminalMark: View {
    let icon: String
    let color: Color

    var body: some View {
        switch icon {
        case "claude": Self.image(Self.claude, template: false).frame(width: 11, height: 11)
        case "codex": Self.image(Self.openai, template: true).foregroundStyle(color).frame(width: 11, height: 11)
        case "opencode":
            // a tall frame with a smaller one in its lower half
            ZStack {
                RoundedRectangle(cornerRadius: 1.5).strokeBorder(color, lineWidth: 1.4).frame(width: 11, height: 14)
                RoundedRectangle(cornerRadius: 0.5).fill(color).frame(width: 5, height: 4).offset(y: 2.5)
            }
            .frame(width: 11, height: 11)
        default:
            Canvas { context, size in
                let center = CGPoint(x: size.width / 2, y: size.height / 2)
                for line in TerminalMarks.lines(icon) {
                    context.stroke(TerminalMarks.path(line, at: center), with: .color(color), style: StrokeStyle(lineWidth: 1.25, lineCap: .round, lineJoin: .round))
                }
                if icon == "search" { context.stroke(Path(ellipseIn: CGRect(x: center.x - 6, y: center.y - 6, width: 10, height: 10)), with: .color(color), lineWidth: 1.25) }
                if icon == "agents" {
                    for y in [-4.5, 0, 4.5] { context.fill(Path(ellipseIn: CGRect(x: center.x - 6.4, y: center.y + y - 1.4, width: 2.8, height: 2.8)), with: .color(color)) }
                }
                if icon == "composer" {
                    for x in [-5.0, 0, 5] { context.fill(Path(ellipseIn: CGRect(x: center.x + x - 1.2, y: center.y + 4.8, width: 2.4, height: 2.4)), with: .color(color)) }
                }
                if icon == "eye" { context.stroke(Path(ellipseIn: CGRect(x: center.x - 2.2, y: center.y - 2.2, width: 4.4, height: 4.4)), with: .color(color), lineWidth: 1.25) }
            }
            .frame(width: 16, height: 16)
            .frame(width: 11, height: 11)
        }
    }

    private static func image(_ svg: String, template: Bool) -> some View {
        let image = NSImage(data: Data(svg.utf8)) ?? NSImage()
        image.isTemplate = template
        return Image(nsImage: image).resizable().renderingMode(template ? .template : .original).aspectRatio(contentMode: .fit)
    }

    // assets/editor-icons/claude.svg and openai.svg (Apache 2.0, with the NOTICE of that folder)
    private static let claude = ##"<svg fill="#D97757" role="img" viewBox="0 0 24 24" xmlns="http://www.w3.org/2000/svg"><title>Claude</title><path d="m4.7144 15.9555 4.7174-2.6471.079-.2307-.079-.1275h-.2307l-.7893-.0486-2.6956-.0729-2.3375-.0971-2.2646-.1214-.5707-.1215-.5343-.7042.0546-.3522.4797-.3218.686.0608 1.5179.1032 2.2767.1578 1.6514.0972 2.4468.255h.3886l.0546-.1579-.1336-.0971-.1032-.0972L6.973 9.8356l-2.55-1.6879-1.3356-.9714-.7225-.4918-.3643-.4614-.1578-1.0078.6557-.7225.8803.0607.2246.0607.8925.686 1.9064 1.4754 2.4893 1.8336.3643.3035.1457-.1032.0182-.0728-.164-.2733-1.3539-2.4467-1.445-2.4893-.6435-1.032-.17-.6194c-.0607-.255-.1032-.4674-.1032-.7285L6.287.1335 6.6997 0l.9957.1336.419.3642.6192 1.4147 1.0018 2.2282 1.5543 3.0296.4553.8985.2429.8318.091.255h.1579v-.1457l.1275-1.706.2368-2.0947.2307-2.6957.0789-.7589.3764-.9107.7468-.4918.5828.2793.4797.686-.0668.4433-.2853 1.8517-.5586 2.9021-.3643 1.9429h.2125l.2429-.2429.9835-1.3053 1.6514-2.0643.7286-.8196.85-.9046.5464-.4311h1.0321l.759 1.1293-.34 1.1657-1.0625 1.3478-.8804 1.1414-1.2628 1.7-.7893 1.36.0729.1093.1882-.0183 2.8535-.607 1.5421-.2794 1.8396-.3157.8318.3886.091.3946-.3278.8075-1.967.4857-2.3072.4614-3.4364.8136-.0425.0304.0486.0607 1.5482.1457.6618.0364h1.621l3.0175.2247.7892.522.4736.6376-.079.4857-1.2142.6193-1.6393-.3886-3.825-.9107-1.3113-.3279h-.1822v.1093l1.0929 1.0686 2.0035 1.8092 2.5075 2.3314.1275.5768-.3218.4554-.34-.0486-2.2039-1.6575-.85-.7468-1.9246-1.621h-.1275v.17l.4432.6496 2.3436 3.5214.1214 1.0807-.17.3521-.6071.2125-.6679-.1214-1.3721-1.9246L14.38 17.959l-1.1414-1.9428-.1397.079-.674 7.2552-.3156.3703-.7286.2793-.6071-.4614-.3218-.7468.3218-1.4753.3886-1.9246.3157-1.53.2853-1.9004.17-.6314-.0121-.0425-.1397.0182-1.4328 1.9672-2.1796 2.9446-1.7243 1.8456-.4128.164-.7164-.3704.0667-.6618.4008-.5889 2.386-3.0357 1.4389-1.882.929-1.0868-.0062-.1579h-.0546l-6.3385 4.1164-1.1293.1457-.4857-.4554.0608-.7467.2307-.2429 1.9064-1.3114Z"/></svg>"##
    private static let openai = ##"<svg role="img" fill="#000000" viewBox="0 0 24 24" xmlns="http://www.w3.org/2000/svg"><title>OpenAI</title><path d="M22.2819 9.8211a5.9847 5.9847 0 0 0-.5157-4.9108 6.0462 6.0462 0 0 0-6.5098-2.9A6.0651 6.0651 0 0 0 4.9807 4.1818a5.9847 5.9847 0 0 0-3.9977 2.9 6.0462 6.0462 0 0 0 .7427 7.0966 5.98 5.98 0 0 0 .511 4.9107 6.051 6.051 0 0 0 6.5146 2.9001A5.9847 5.9847 0 0 0 13.2599 24a6.0557 6.0557 0 0 0 5.7718-4.2058 5.9894 5.9894 0 0 0 3.9977-2.9001 6.0557 6.0557 0 0 0-.7475-7.0729zm-9.022 12.6081a4.4755 4.4755 0 0 1-2.8764-1.0408l.1419-.0804 4.7783-2.7582a.7948.7948 0 0 0 .3927-.6813v-6.7369l2.02 1.1686a.071.071 0 0 1 .038.052v5.5826a4.504 4.504 0 0 1-4.4945 4.4944zm-9.6607-4.1254a4.4708 4.4708 0 0 1-.5346-3.0137l.142.0852 4.783 2.7582a.7712.7712 0 0 0 .7806 0l5.8428-3.3685v2.3324a.0804.0804 0 0 1-.0332.0615L9.74 19.9502a4.4992 4.4992 0 0 1-6.1408-1.6464zM2.3408 7.8956a4.485 4.485 0 0 1 2.3655-1.9728V11.6a.7664.7664 0 0 0 .3879.6765l5.8144 3.3543-2.0201 1.1685a.0757.0757 0 0 1-.071 0l-4.8303-2.7865A4.504 4.504 0 0 1 2.3408 7.872zm16.5963 3.8558L13.1038 8.364 15.1192 7.2a.0757.0757 0 0 1 .071 0l4.8303 2.7913a4.4944 4.4944 0 0 1-.6765 8.1042v-5.6772a.79.79 0 0 0-.407-.667zm2.0107-3.0231l-.142-.0852-4.7735-2.7818a.7759.7759 0 0 0-.7854 0L9.409 9.2297V6.8974a.0662.0662 0 0 1 .0284-.0615l4.8303-2.7866a4.4992 4.4992 0 0 1 6.6802 4.66zM8.3065 12.863l-2.02-1.1638a.0804.0804 0 0 1-.038-.0567V6.0742a4.4992 4.4992 0 0 1 7.3757-3.4537l-.142.0805L8.704 5.459a.7948.7948 0 0 0-.3927.6813zm1.0976-2.3654l2.602-1.4998 2.6069 1.4998v2.9994l-2.5974 1.4997-2.6067-1.4997Z"/></svg>"##
}

/// The Maestro's crown (src/app/ui/mod.rs `maestro_crown`): a band with three spikes, in amber.
struct TerminalCrown: View {
    var body: some View {
        Canvas { context, size in
            let origin = CGPoint(x: size.width / 2, y: size.height / 2 + 1)
            func point(_ x: CGFloat, _ y: CGFloat) -> CGPoint { CGPoint(x: origin.x + x, y: origin.y + y) }
            let amber = GraphicsContext.Shading.color(Color(rgb: 0xf0a020))
            context.fill(Path(CGRect(x: origin.x - 6, y: origin.y + 1, width: 12, height: 4)), with: amber)
            for spike in [[(-6.5, 1.0), (-6.0, -4.0), (-3.0, 1.0)], [(-3.5, 1.0), (0.0, -6.0), (3.5, 1.0)], [(3.0, 1.0), (6.0, -4.0), (6.5, 1.0)]] {
                var path = Path()
                path.move(to: point(spike[0].0, spike[0].1))
                for corner in spike.dropFirst() { path.addLine(to: point(corner.0, corner.1)) }
                path.closeSubpath()
                context.fill(path, with: amber)
            }
        }
    }
}
