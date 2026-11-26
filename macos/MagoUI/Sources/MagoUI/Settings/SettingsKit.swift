import AppKit
import SwiftUI

/// Ajustes' tokens, the egui page's (`src/app/ui/settings/kit.rs`, `K`): drawn dark, the light ones the same steps the
/// other way.
struct SK {
    let bg, fg, mutedFg, card, primary, destructive, well, wellLine, hairline, button, buttonHover, buttonLine: Color
    let chosen, segment, softFg, muted, border, switchOff: Color

    static func of(_ scheme: ColorScheme) -> SK { scheme == .dark ? night : day }

    static let night = SK(
        bg: rgb(0x17181b), fg: rgb(0xf5f5f7), mutedFg: rgb(0x8e8e93), card: rgb(0x1e1f23), primary: rgb(0x0a84ff),
        destructive: rgb(0xff453a), well: rgb(0x111215), wellLine: .white.opacity(20 / 255), hairline: .white.opacity(15 / 255),
        button: rgb(0x26282d), buttonHover: rgb(0x323439), buttonLine: .white.opacity(26 / 255), chosen: rgb(0x24262b),
        segment: rgb(0x2c2e34), softFg: rgb(0xa1a1aa), muted: rgb(0x24262b), border: rgb(0x545458, 0x80 / 255),
        switchOff: .white.opacity(46 / 255))
    static let day = SK(
        bg: rgb(0xf5f5f7), fg: rgb(0x1d1d1f), mutedFg: rgb(0x6e6e73), card: .white, primary: rgb(0x007aff),
        destructive: rgb(0xff3b30), well: rgb(0xececf0), wellLine: .black.opacity(20 / 255), hairline: .black.opacity(15 / 255),
        button: .white, buttonHover: rgb(0xf1f1f1), buttonLine: .black.opacity(31 / 255), chosen: rgb(0xdcdce2),
        segment: .white, softFg: rgb(0x5a5a62), muted: rgb(0xe8e8ec), border: .black.opacity(0x1f / 255),
        switchOff: .black.opacity(46 / 255))

    /// The integrations' green and amber (`integrations.rs`).
    static let green = rgb(0x34c759)
    static let amber = rgb(0xf5a524)

    /// The sizes of the egui page's text: a row's name, its line, a control's and a group's title.
    static let body: CGFloat = 13.5
    static let small: CGFloat = 12
    static let control: CGFloat = 13
    /// Their line heights (`BODY_LINE`, `SMALL_LINE`).
    static let bodyLine: CGFloat = 18
    static let smallLine: CGFloat = 16
}

extension Font {
    /// The Assistant at exactly `size` (`Font.app` follows Dynamic Type, which rounds the half points up: 12,5 came out 13).
    static func kit(_ size: CGFloat) -> Font { .custom("Assistant", fixedSize: size) }
}

func rgb(_ value: UInt32, _ opacity: Double = 1) -> Color {
    Color(.sRGB, red: Double(value >> 16 & 0xff) / 255, green: Double(value >> 8 & 0xff) / 255, blue: Double(value & 0xff) / 255, opacity: opacity)
}

extension View {
    /// The hand over what answers to a click, as the egui page's.
    func handCursor() -> some View {
        onContinuousHover { phase in
            if case .active = phase { NSCursor.pointingHand.set() } else { NSCursor.arrow.set() }
        }
    }

    /// The Assistant at `size` with egui's line height (`galley`'s `line`) in place of its own (1.308 of the size): the
    /// rest of each line goes half over and half under it, as egui centers the glyphs in their line.
    func kitLine(_ size: CGFloat, _ line: CGFloat) -> some View {
        let extra = max(0, line - size * 1.308)
        return font(.kit(size)).lineSpacing(extra).padding(.vertical, extra / 2)
    }

    /// A field's well: 30 high, the line going to the accent while it has the focus.
    func kitWell(_ k: SK, width: CGFloat, focused: Bool) -> some View {
        textFieldStyle(.plain)
            .font(.kit(SK.control))
            .foregroundStyle(k.fg)
            .padding(.horizontal, 10)
            .frame(width: width, height: 30)
            .background(k.well, in: .rect(cornerRadius: 8))
            .overlay { RoundedRectangle(cornerRadius: 8).strokeBorder(focused ? k.primary : k.wellLine) }
    }
}

/// The app's 16 px icons (`src/app/ui/icons.rs`), drawn at 1.25 px with round joins, by name.
struct SettingsIcon: View {
    let name: String
    let color: Color
    var scale: CGFloat = 1

    var body: some View {
        Canvas { context, size in
            context.translateBy(x: size.width / 2, y: size.height / 2)
            context.scaleBy(x: scale, y: scale)
            draw(in: context)
        }
        .frame(width: 16, height: 16)
        .accessibilityHidden(true)
    }

    private func draw(in context: GraphicsContext) {
        let style = StrokeStyle(lineWidth: 1.25, lineCap: .round, lineJoin: .round)
        func line(_ points: [(CGFloat, CGFloat)], closed: Bool = false) {
            var path = Path()
            path.addLines(points.map { CGPoint(x: $0.0, y: $0.1) })
            if closed { path.closeSubpath() }
            context.stroke(path, with: .color(color), style: style)
        }
        func circle(_ x: CGFloat, _ y: CGFloat, _ radius: CGFloat, filled: Bool) {
            let path = Path(ellipseIn: CGRect(x: x - radius, y: y - radius, width: 2 * radius, height: 2 * radius))
            if filled { context.fill(path, with: .color(color)) } else { context.stroke(path, with: .color(color), style: style) }
        }
        func rect(_ x: CGFloat, _ y: CGFloat, _ width: CGFloat, _ height: CGFloat, _ radius: CGFloat) {
            context.stroke(Path(roundedRect: CGRect(x: x, y: y, width: width, height: height), cornerRadius: radius), with: .color(color), style: style)
        }
        switch name {
        case "gear":
            // eight teeth around a hub: two points out, two in, per tooth
            let step = 2 * CGFloat.pi / 32
            line((0..<32).map { index in
                let (angle, radius) = ((CGFloat(index) - 0.5) * step, index % 4 < 2 ? 7.0 : 5.2)
                return (radius * cos(angle), radius * sin(angle))
            }, closed: true)
            circle(0, 0, 2.3, filled: false)
        case "monitor":
            rect(-7, -5.5, 14, 9.5, 2)
            line([(-3, 6.5), (3, 6.5)])
            line([(0, 4), (0, 6.5)])
        case "grid":
            rect(-6.5, -6.5, 13, 13, 2)
            for at: CGFloat in [-2.2, 2.2] {
                line([(at, -6.5), (at, 6.5)])
                line([(-6.5, at), (6.5, at)])
            }
        case "code":
            line([(-2.5, -4.5), (-6, 0), (-2.5, 4.5)])
            line([(2.5, -4.5), (6, 0), (2.5, 4.5)])
            line([(1.5, -5.5), (-1.5, 5.5)])
        case "markdown":
            rect(-6.5, -6.5, 13, 13, 2)
            line([(-3.5, 4), (-3.5, -3.5), (0, 1), (3.5, -3.5), (3.5, 4)])
        case "codeBlock":
            line([(-2, -6), (-4, -5), (-4, -1), (-6, 0), (-4, 1), (-4, 5), (-2, 6)])
            line([(2, -6), (4, -5), (4, -1), (6, 0), (4, 1), (4, 5), (2, 6)])
        case "agents":
            for y: CGFloat in [-4.5, 0, 4.5] {
                circle(-5, y, 1.4, filled: true)
                line([(-2, y), (6, y)])
            }
        case "chat": line([(-7, -6), (7, -6), (7, 3), (-0.5, 3), (-4, 6.5), (-4, 3), (-7, 3)], closed: true)
        case "connect":
            line([(0, 7), (0, 0)])
            line([(-5.5, -6), (0, 0), (5.5, -6)])
            for (x, y): (CGFloat, CGFloat) in [(-5.5, -6), (5.5, -6), (0, 7)] { circle(x, y, 1.4, filled: true) }
        case "menu": for y: CGFloat in [-5, 0, 5] { line([(-7, y), (7, y)]) }
        case "open": line([(-7, -5), (-2, -5), (0, -3), (7, -3), (7, 5.5), (-7, 5.5)], closed: true)
        case "eye":
            line([(-7, 0), (-4, -3.5), (0, -5), (4, -3.5), (7, 0), (4, 3.5), (0, 5), (-4, 3.5)], closed: true)
            circle(0, 0, 2.2, filled: false)
        case "check": line([(-5.5, 0.5), (-1.5, 4.5), (6, -4.5)])
        case "trash":
            line([(-6.5, -4), (6.5, -4)])
            line([(-2, -4), (-2, -6.5), (2, -6.5), (2, -4)])
            line([(-5, -4), (-4, 6.5), (4, 6.5), (5, -4)])
            line([(-1.5, -1), (-1.5, 4)])
            line([(1.5, -1), (1.5, 4)])
        case "restart":
            // the circle turning right, its arrow at the top right
            var points: [(CGFloat, CGFloat)] = (0...18).map { index in
                let angle = CGFloat(index) * 15 * .pi / 180
                return (6.5 * cos(angle), 6.5 * sin(angle))
            }
            points += [(1.8, -6.5), (4.9, -4.5), (6.5, -2.9)]
            line(points)
            line([(6.5, -6.5), (6.5, -2.9), (2.9, -2.9)])
        case "edit":
            rect(-6.5, -5, 11.5, 11.5, 2)
            line([(-1.5, 2), (6.5, -6)])
            line([(-1.5, 2), (-2, 3)])
        case "plus":
            line([(-6, 0), (6, 0)])
            line([(0, -6), (0, 6)])
        case "minus": line([(-6, 0), (6, 0)])
        case "chevronUp", "chevronDown":
            let y: CGFloat = name == "chevronUp" ? 2 : -2
            var path = Path()
            path.addLines([CGPoint(x: -4.5, y: y), CGPoint(x: 0, y: -y), CGPoint(x: 4.5, y: y)])
            context.stroke(path, with: .color(color), style: StrokeStyle(lineWidth: 1.4, lineCap: .round, lineJoin: .round))
        default: break
        }
    }
}

/// The page's icon by its name.
func tabIcon(_ tab: String) -> String {
    switch tab {
    case "Geral": "gear"
    case "Ilha": "monitor"
    case "Canvas": "grid"
    case "Navegação e editor": "code"
    case "Terminal": "codeBlock"
    case "Agentes": "agents"
    case "Chat": "chat"
    case "Skills": "markdown"
    case "Integrações": "connect"
    case "Atalhos": "menu"
    case "Dados": "open"
    default: "eye"
    }
}

/// A button 30 high (#26282d, a line of white 10%, radius 8), the text at 12.5.
struct KitButtonStyle: ButtonStyle {
    func makeBody(configuration: Configuration) -> some View { KitButtonBody(label: configuration.label) }
}

private struct KitButtonBody<Label: View>: View {
    let label: Label
    @Environment(\.colorScheme) private var scheme
    @Environment(\.isEnabled) private var enabled
    @State private var hover = false

    var body: some View {
        let k = SK.of(scheme)
        label
            .font(.kit(12.5))
            .foregroundStyle(k.fg)
            .lineLimit(1)
            .fixedSize()
            .padding(.horizontal, 12)
            .frame(height: 30)
            .background(hover && enabled ? k.buttonHover : k.button, in: .rect(cornerRadius: 8))
            .overlay { RoundedRectangle(cornerRadius: 8).strokeBorder(k.buttonLine) }
            .contentShape(.rect(cornerRadius: 8))
            .onHover { hover = $0 }
            .handCursor()
    }
}

/// `switch`: 38 × 22, the knob sliding, the accent when on.
struct SwitchStyle: ToggleStyle {
    func makeBody(configuration: Configuration) -> some View { SwitchBody(configuration: configuration) }
}

private struct SwitchBody: View {
    let configuration: ToggleStyleConfiguration
    @Environment(\.colorScheme) private var scheme

    var body: some View {
        let k = SK.of(scheme)
        Capsule()
            .fill(configuration.isOn ? k.primary : k.switchOff)
            .frame(width: 38, height: 22)
            .overlay(alignment: configuration.isOn ? .trailing : .leading) {
                Circle().fill(.white).frame(width: 18, height: 18).padding(.horizontal, 2)
            }
            .animation(.easeOut(duration: 0.12), value: configuration.isOn)
            .contentShape(Capsule())
            .onTapGesture { configuration.isOn.toggle() }
            .handCursor()
            .accessibilityAddTraits(.isToggle)
    }
}

/// The 24 px ghost button with an icon; red under the pointer when `danger`.
struct GhostButton: View {
    let icon: String
    let name: String
    var enabled = true
    var danger = false
    let action: () -> Void
    @Environment(\.colorScheme) private var scheme
    @State private var hover = false

    var body: some View {
        let k = SK.of(scheme)
        let hot = enabled && hover
        Button(action: action) {
            SettingsIcon(name: icon, color: !enabled ? k.mutedFg.opacity(0.4) : hot && danger ? k.destructive : hot ? k.fg : k.mutedFg)
                .frame(width: 24, height: 24)
                .background(hot ? k.chosen : .clear, in: .rect(cornerRadius: 6))
                .contentShape(Rectangle())
        }
        .buttonStyle(.plain)
        .disabled(!enabled)
        .onHover { hover = $0 }
        .handCursor()
        .help(name)
        .accessibilityLabel(name)
    }
}

/// A choice of a few short options side by side, 30 high: the track (#111215), the one chosen raised (#2c2e34).
struct Segments: View {
    let label: String
    let options: [String]
    let chosen: Int
    let pick: (Int) -> Void
    @Environment(\.colorScheme) private var scheme

    var body: some View {
        HStack(spacing: 2) {
            ForEach(Array(options.enumerated()), id: \.offset) { index, name in
                Segment(name: name, chosen: index == chosen) { if index != chosen { pick(index) } }
            }
        }
        .padding(2)
        .frame(height: 30)
        .background(SK.of(scheme).well, in: .rect(cornerRadius: 8))
        .accessibilityElement(children: .contain)
        .accessibilityLabel(label)
    }
}

private struct Segment: View {
    let name: String
    let chosen: Bool
    let action: () -> Void
    @Environment(\.colorScheme) private var scheme
    @State private var hover = false

    var body: some View {
        let k = SK.of(scheme)
        Button(action: action) {
            Text(name)
                .font(.kit(12.5))
                .foregroundStyle(chosen || hover ? k.fg : k.softFg)
                .padding(.horizontal, 12)
                .frame(maxHeight: .infinity)
                .background(chosen ? k.segment : .clear, in: .rect(cornerRadius: 6))
                .contentShape(Rectangle())
        }
        .buttonStyle(.plain)
        .onHover { hover = $0 }
        .handCursor()
        .accessibilityAddTraits(chosen ? .isSelected : [])
    }
}

/// `SelectField`: 30 high, at least 120 wide, the chevron at 14 from the right; the options in the system's menu, the
/// chosen one checked.
struct SelectField: View {
    let label: String
    let options: [String]
    let chosen: Int
    let pick: (Int) -> Void
    @Environment(\.colorScheme) private var scheme
    @State private var hover = false

    var body: some View {
        let k = SK.of(scheme)
        Menu {
            Picker(label, selection: Binding(get: { chosen }, set: { if $0 != chosen { pick($0) } })) {
                ForEach(Array(options.enumerated()), id: \.offset) { index, name in Text(name).tag(index) }
            }
            .pickerStyle(.inline)
            .labelsHidden()
        } label: {
            HStack(spacing: 0) {
                // as wide as the widest option, as the egui field
                ZStack(alignment: .leading) {
                    ForEach(options, id: \.self) { Text($0).hidden() }
                    Text(options.indices.contains(chosen) ? options[chosen] : "")
                }
                .font(.kit(SK.control))
                .foregroundStyle(k.fg)
                .lineLimit(1)
                Spacer(minLength: 10)
                SelectChevron().stroke(k.mutedFg, style: StrokeStyle(lineWidth: 1.4, lineCap: .round, lineJoin: .round)).frame(width: 7, height: 3.5)
            }
            .padding(.leading, 10)
            .padding(.trailing, 10.5)
            .frame(minWidth: 120)
            .frame(height: 30)
            .background(k.well, in: .rect(cornerRadius: 8))
            .overlay { RoundedRectangle(cornerRadius: 8).strokeBorder(hover ? k.buttonLine : k.wellLine) }
            .contentShape(.rect(cornerRadius: 8))
        }
        .menuStyle(.button)
        .buttonStyle(.plain)
        .menuIndicator(.hidden)
        .fixedSize()
        .onHover { hover = $0 }
        .handCursor()
        .accessibilityLabel("\(label): \(options.indices.contains(chosen) ? options[chosen] : "")")
    }
}

/// The select's chevron: a V 7 wide.
private struct SelectChevron: Shape {
    func path(in rect: CGRect) -> Path {
        Path { path in
            path.move(to: CGPoint(x: rect.minX, y: rect.minY))
            path.addLine(to: CGPoint(x: rect.midX, y: rect.maxY))
            path.addLine(to: CGPoint(x: rect.maxX, y: rect.minY))
        }
    }
}

/// `NumberStepper`: − the value + in a well 30 high, the buttons 30 wide.
struct StepperField: View {
    let label: String
    let value: Int
    let range: ClosedRange<Int>
    let shown: String
    let set: (Int) -> Void
    @Environment(\.colorScheme) private var scheme

    var body: some View {
        let k = SK.of(scheme)
        HStack(spacing: 0) {
            StepButton(icon: "minus", name: "\(label) −", live: value - 1 >= range.lowerBound) { set(value - 1) }
            Text(shown).font(.kit(SK.control)).foregroundStyle(k.fg).monospacedDigit().frame(width: 48)
            StepButton(icon: "plus", name: "\(label) +", live: value + 1 <= range.upperBound) { set(value + 1) }
        }
        .frame(height: 30)
        .background(k.well, in: .rect(cornerRadius: 8))
        .overlay { RoundedRectangle(cornerRadius: 8).strokeBorder(k.wellLine) }
    }
}

private struct StepButton: View {
    let icon: String
    let name: String
    let live: Bool
    let action: () -> Void
    @Environment(\.colorScheme) private var scheme
    @State private var hover = false

    var body: some View {
        let k = SK.of(scheme)
        Button(action: action) {
            SettingsIcon(name: icon, color: !live ? k.mutedFg.opacity(0.4) : hover ? k.fg : k.softFg)
                .frame(width: 30, height: 30)
                .contentShape(Rectangle())
        }
        .buttonStyle(.plain)
        .disabled(!live)
        .onHover { hover = $0 }
        .handCursor()
        .accessibilityLabel(name)
    }
}
