import AppKit
import SwiftUI

/// A change of the drawing's style: Rust applies it to the selection (one undo step) and to what is drawn next.
@MainActor private func style(_ args: [String: Any], final: Bool = true) {
    var args = args
    args["final"] = final
    shellVerb("style", args)
}

/// The properties row, under the toolbars: as buttons while it fits, else one button per group that opens its choices.
struct PropertiesRow: View {
    let props: StyleProperties
    let state: ShellState

    var body: some View {
        ViewThatFits(in: .horizontal) {
            PropertiesContent(props: props, state: state, compact: false)
            PropertiesContent(props: props, state: state, compact: true)
        }
    }
}

private struct PropertiesContent: View {
    let props: StyleProperties
    let state: ShellState
    let compact: Bool

    var body: some View {
        GlassRow {
            if props.showStroke {
                ColorDotButton(title: "Contorno", color: props.stroke, picks: state.strokePicks, allowsClear: false) { style(["stroke": $0]) }
            }
            if props.showFill {
                ColorDotButton(title: "Fundo", color: props.fill, picks: state.backgroundPicks.filter { !props.fillTool || $0[3] > 0 }, allowsClear: !props.fillTool) {
                    style(["fill": $0])
                }
            }
            if props.showStroke || props.showFill { GlassDivider() }
            if props.showFillStyle {
                PickGroup(label: "Preenchimento", compact: compact, options: [
                    PickOption(tip: "Hachura", on: props.fillStyle == "hachure", glyph: .fill("hachure"), args: ["fillStyle": "hachure"]),
                    PickOption(tip: "Hachura cruzada", on: props.fillStyle == "cross_hatch", glyph: .fill("cross"), args: ["fillStyle": "cross_hatch"]),
                    PickOption(tip: "Sólido", on: props.fillStyle == "solid", glyph: .fill("solid"), args: ["fillStyle": "solid"]),
                ])
                GlassDivider()
            }
            if props.showStrokeOptions {
                PickGroup(label: "Espessura do traço", compact: compact, options: [
                    PickOption(tip: "Fino", on: props.strokeWidth == 1, glyph: .width(1), args: ["strokeWidth": 1]),
                    PickOption(tip: "Espesso", on: props.strokeWidth == 2, glyph: .width(2), args: ["strokeWidth": 2]),
                    PickOption(tip: "Muito espesso", on: props.strokeWidth == 4, glyph: .width(4), args: ["strokeWidth": 4]),
                ])
                PickGroup(label: "Estilo do traço", compact: compact, options: [
                    PickOption(tip: "Sólido", on: props.strokeStyle == "solid", glyph: .dash("solid"), args: ["strokeStyle": "solid"]),
                    PickOption(tip: "Tracejado", on: props.strokeStyle == "dashed", glyph: .dash("dashed"), args: ["strokeStyle": "dashed"]),
                    PickOption(tip: "Pontilhado", on: props.strokeStyle == "dotted", glyph: .dash("dotted"), args: ["strokeStyle": "dotted"]),
                ])
                PickGroup(label: "Precisão do traço", compact: compact, options: [
                    PickOption(tip: "Arquiteto", on: props.roughness == 0, glyph: .rough(0), args: ["roughness": 0]),
                    PickOption(tip: "Artista", on: props.roughness == 1, glyph: .rough(1), args: ["roughness": 1]),
                    PickOption(tip: "Cartunista", on: props.roughness == 2, glyph: .rough(2), args: ["roughness": 2]),
                ])
                GlassDivider()
            }
            if props.showArrow {
                PickGroup(label: "Tipo de seta", compact: compact, options: [
                    PickOption(tip: "Seta reta", on: !props.round, glyph: .arrow(false), args: ["round": false]),
                    PickOption(tip: "Seta curva", on: props.round, glyph: .arrow(true), args: ["round": true]),
                ])
                HeadMenu(title: "Ponta inicial", head: props.startHead, end: false)
                HeadMenu(title: "Ponta final", head: props.endHead, end: true)
                GlassDivider()
            }
            if props.showEdges {
                PickGroup(label: "Arestas", compact: compact, options: [
                    PickOption(tip: "Pontudo", on: !props.round, glyph: .edge(false), args: ["round": false]),
                    PickOption(tip: "Arredondado", on: props.round, glyph: .edge(true), args: ["round": true]),
                ])
                GlassDivider()
            }
            if props.showFont {
                PickGroup(label: "Tamanho da fonte", compact: compact, options: [
                    PickOption(tip: "Pequeno", on: props.fontSize == 16, glyph: .label("S"), args: ["fontSize": 16]),
                    PickOption(tip: "Médio", on: props.fontSize == 20, glyph: .label("M"), args: ["fontSize": 20]),
                    PickOption(tip: "Grande", on: props.fontSize == 28, glyph: .label("L"), args: ["fontSize": 28]),
                    PickOption(tip: "Muito grande", on: props.fontSize == 36, glyph: .label("XL"), args: ["fontSize": 36]),
                ])
                GlassDivider()
            }
            if let name = props.frameName { FrameNameButton(name: name) }
            OpacityButton(opacity: props.opacity)
            if !props.actions.isEmpty {
                GlassDivider()
                ForEach(props.actions) { action in
                    GlassButton(symbol: actionSymbol(action.id), tip: action.keys.isEmpty ? action.label : "\(action.label) — \(action.keys)") {
                        shellVerb("command", ["name": action.id])
                    }
                }
            }
        }
    }

    private func actionSymbol(_ id: String) -> String {
        switch id {
        case "duplicate": "plus.square.on.square"
        case "delete": "trash"
        case "group": "rectangle.3.group"
        case "ungroup": "rectangle.split.3x1"
        default: "lock"
        }
    }
}

// MARK: choices

struct PickOption: Identifiable {
    let tip: String
    let on: Bool
    let glyph: Glyph
    let args: [String: Any]
    var id: String { tip }
}

/// A group of choices: a button each, or one button holding them in a menu when the row would not fit.
private struct PickGroup: View {
    let label: String
    let compact: Bool
    let options: [PickOption]

    var body: some View {
        if compact {
            let shown = options.first(where: \.on) ?? options[0]
            Menu {
                ForEach(options) { option in
                    Button {
                        style(option.args)
                    } label: {
                        if option.on { Label(option.tip, systemImage: "checkmark") } else { Text(option.tip) }
                    }
                }
            } label: {
                shown.glyph.frame(width: 30, height: 30)
            }
            .menuStyle(.borderlessButton)
            .menuIndicator(.hidden)
            .frame(width: 30, height: 30)
            .help(label)
        } else {
            ForEach(options) { option in
                Button { style(option.args) } label: { option.glyph }
                    .buttonStyle(GlassButtonStyle(on: option.on))
                    .help(option.tip)
            }
        }
    }
}

/// The little drawings the choices wear.
enum Glyph: View {
    case width(Double), dash(String), rough(Int), fill(String), arrow(Bool), edge(Bool), label(String)

    var body: some View {
        switch self {
        case .label(let text):
            Text(text).font(.system(size: 12, weight: .medium))
        default:
            Canvas { context, size in draw(context, size) }.frame(width: 18, height: 18)
        }
    }

    private func draw(_ context: GraphicsContext, _ size: CGSize) {
        let ink = GraphicsContext.Shading.foreground
        let box = CGRect(origin: .zero, size: size).insetBy(dx: 1, dy: 1)
        switch self {
        case .width(let value):
            context.stroke(Path { $0.move(to: CGPoint(x: 1, y: size.height / 2)); $0.addLine(to: CGPoint(x: size.width - 1, y: size.height / 2)) }, with: ink, lineWidth: value)
        case .dash(let kind):
            let pattern: [CGFloat] = kind == "dashed" ? [4, 3] : kind == "dotted" ? [1, 3] : []
            let style = StrokeStyle(lineWidth: 2, lineCap: .round, dash: pattern)
            context.stroke(Path { $0.move(to: CGPoint(x: 1, y: size.height / 2)); $0.addLine(to: CGPoint(x: size.width - 1, y: size.height / 2)) }, with: ink, style: style)
        case .rough(let level):
            let wave = CGFloat(level) * 1.6
            let path = Path { path in
                path.move(to: CGPoint(x: 1, y: size.height / 2))
                path.addCurve(to: CGPoint(x: size.width - 1, y: size.height / 2), control1: CGPoint(x: size.width * 0.3, y: size.height / 2 - wave * 2), control2: CGPoint(x: size.width * 0.7, y: size.height / 2 + wave * 2))
            }
            context.stroke(path, with: ink, lineWidth: 1.6)
        case .fill(let kind):
            context.stroke(Path(roundedRect: box, cornerRadius: 2), with: ink, lineWidth: 1)
            if kind == "solid" {
                context.fill(Path(roundedRect: box.insetBy(dx: 2, dy: 2), cornerRadius: 1), with: ink)
                return
            }
            var clipped = context
            clipped.clip(to: Path(box))
            for offset in stride(from: -size.height, to: size.width, by: 4) {
                clipped.stroke(Path { $0.move(to: CGPoint(x: offset, y: size.height)); $0.addLine(to: CGPoint(x: offset + size.height, y: 0)) }, with: ink, lineWidth: 1)
                if kind == "cross" {
                    clipped.stroke(Path { $0.move(to: CGPoint(x: offset, y: 0)); $0.addLine(to: CGPoint(x: offset + size.height, y: size.height)) }, with: ink, lineWidth: 1)
                }
            }
        case .arrow(let curved):
            let path = Path { path in
                path.move(to: CGPoint(x: 2, y: size.height - 3))
                if curved {
                    path.addQuadCurve(to: CGPoint(x: size.width - 3, y: 3), control: CGPoint(x: 2, y: 3))
                } else {
                    path.addLine(to: CGPoint(x: size.width - 3, y: 3))
                }
            }
            context.stroke(path, with: ink, style: StrokeStyle(lineWidth: 1.6, lineCap: .round))
            context.stroke(Path { $0.move(to: CGPoint(x: size.width - 9, y: 3)); $0.addLine(to: CGPoint(x: size.width - 3, y: 3)); $0.addLine(to: CGPoint(x: size.width - 3, y: 9)) }, with: ink, style: StrokeStyle(lineWidth: 1.6, lineCap: .round, lineJoin: .round))
        case .edge(let round):
            let path = Path { path in
                let radius: CGFloat = round ? 5 : 0
                path.move(to: CGPoint(x: 1, y: size.height - 1))
                path.addLine(to: CGPoint(x: 1, y: 1 + radius))
                if round { path.addQuadCurve(to: CGPoint(x: 1 + radius, y: 1), control: CGPoint(x: 1, y: 1)) } else { path.addLine(to: CGPoint(x: 1, y: 1)) }
                path.addLine(to: CGPoint(x: size.width - 1, y: 1))
            }
            context.stroke(path, with: ink, style: StrokeStyle(lineWidth: 1.6, lineCap: .round, lineJoin: .round))
        case .label:
            break
        }
    }
}

// MARK: color, opacity, frame name

/// A round button showing a color (a slash through it when there is none); its popover holds the picks and the system's picker.
private struct ColorDotButton: View {
    let title: String
    let color: [Int]?
    let picks: [[Int]]
    let allowsClear: Bool
    let choose: ([Int]) -> Void
    @State private var open = false

    var body: some View {
        Button { open.toggle() } label: { dot }
            .buttonStyle(GlassButtonStyle(on: open))
            .help(title)
            .popover(isPresented: $open, arrowEdge: .bottom) {
                HStack(spacing: 8) {
                    ForEach(picks, id: \.self) { pick in
                        Button { choose(pick); open = false } label: { swatch(pick) }.buttonStyle(.plain)
                    }
                    ColorPicker("", selection: Binding(get: { Color(rgba: color ?? [0, 0, 0, 255]) }, set: { choose(Self.components($0)) }), supportsOpacity: false)
                        .labelsHidden().help("Mais cores")
                }
                .padding(12)
            }
    }

    @ViewBuilder private var dot: some View {
        if let color {
            Circle().fill(Color(rgba: color)).overlay(Circle().stroke(Color.primary.opacity(0.3), lineWidth: 1)).frame(width: 16, height: 16)
        } else {
            ZStack {
                Circle().stroke(Color.primary.opacity(0.3), lineWidth: 1)
                Path { $0.move(to: CGPoint(x: 2, y: 14)); $0.addLine(to: CGPoint(x: 14, y: 2)) }.stroke(Color.primary.opacity(0.3), lineWidth: 1)
            }.frame(width: 16, height: 16)
        }
    }

    @ViewBuilder private func swatch(_ pick: [Int]) -> some View {
        let selected = pick == color || (pick[3] == 0 && color == nil)
        ZStack {
            if pick[3] == 0 {
                // transparent: a slash through an empty swatch
                RoundedRectangle(cornerRadius: 5).fill(Color.white)
                Path { $0.move(to: CGPoint(x: 3, y: 19)); $0.addLine(to: CGPoint(x: 19, y: 3)) }.stroke(Color.red.opacity(0.7), lineWidth: 1)
            } else {
                RoundedRectangle(cornerRadius: 5).fill(Color(rgba: pick))
            }
        }
        .frame(width: 22, height: 22)
        .overlay(RoundedRectangle(cornerRadius: 5).stroke(Color.black.opacity(0.1), lineWidth: 1))
        .overlay(RoundedRectangle(cornerRadius: 6.5).stroke(Color.accent007aff, lineWidth: selected ? 1.5 : 0).padding(-2))
    }

    private static func components(_ color: Color) -> [Int] {
        let ns = NSColor(color).usingColorSpace(.sRGB) ?? .black
        return [Int(ns.redComponent * 255), Int(ns.greenComponent * 255), Int(ns.blueComponent * 255), 255]
    }
}

/// The opacity: its percentage on the button, a slider in the popover (10 at a time).
private struct OpacityButton: View {
    let opacity: Int
    @State private var open = false
    @State private var value = 100.0

    var body: some View {
        Button { open.toggle() } label: { Text("\(opacity)%").font(.system(size: 12)) }
            .buttonStyle(GlassButtonStyle(on: open, size: 36))
            .help("Opacidade")
            .popover(isPresented: $open, arrowEdge: .bottom) {
                Slider(value: $value, in: 0...100, step: 10) { editing in
                    if !editing { style(["opacity": Int(value)]) }
                }
                .frame(width: 160)
                .padding(12)
                .onChange(of: value) { _, next in style(["opacity": Int(next)], final: false) }
                .onAppear { value = Double(opacity) }
            }
    }
}

private struct FrameNameButton: View {
    let name: String
    @State private var open = false
    @State private var text = ""

    var body: some View {
        GlassButton(symbol: "pencil", tip: "Nome do quadro", on: open) { open.toggle() }
            .popover(isPresented: $open, arrowEdge: .bottom) {
                TextField("Nome do quadro", text: $text)
                    .textFieldStyle(.roundedBorder).frame(width: 170).padding(12)
                    .onChange(of: text) { _, next in shellVerb("frame_name", ["name": next]) }
                    .onAppear { text = name }
            }
    }
}

// MARK: arrowheads

private let heads: [(id: String, name: String)] = [
    ("none", "Nenhuma"), ("arrow", "Flecha"), ("triangle", "Triângulo"), ("triangle_outline", "Triângulo (contorno)"),
    ("circle", "Círculo"), ("circle_outline", "Círculo (contorno)"), ("diamond", "Diamante"), ("diamond_outline", "Diamante (contorno)"), ("bar", "Barra"),
]

/// An arrowhead of the arrows' row: a menu of the heads, the current one drawn on the button.
private struct HeadMenu: View {
    let title: String
    let head: String
    let end: Bool

    var body: some View {
        Menu {
            ForEach(heads, id: \.id) { choice in
                Button {
                    shellVerb("head", ["end": end, "head": choice.id])
                } label: {
                    if choice.id == head { Label(choice.name, systemImage: "checkmark") } else { Text(choice.name) }
                }
            }
        } label: {
            HeadGlyph(head: head, flip: !end).frame(width: 30, height: 30)
        }
        .menuStyle(.borderlessButton)
        .menuIndicator(.hidden)
        .frame(width: 30, height: 30)
        .help(title)
    }
}

/// A line with its head at one end (the start's drawn mirrored).
private struct HeadGlyph: View {
    let head: String
    let flip: Bool

    var body: some View {
        Canvas { context, size in
            let y = size.height / 2
            let tip = CGPoint(x: size.width - 3, y: y)
            let ink = GraphicsContext.Shading.foreground
            context.stroke(Path { $0.move(to: CGPoint(x: 3, y: y)); $0.addLine(to: tip) }, with: ink, lineWidth: 1.5)
            let open = head.hasSuffix("_outline")
            let shape: Path
            switch head.replacingOccurrences(of: "_outline", with: "") {
            case "arrow":
                shape = Path { $0.move(to: CGPoint(x: tip.x - 6, y: y - 5)); $0.addLine(to: tip); $0.addLine(to: CGPoint(x: tip.x - 6, y: y + 5)) }
                context.stroke(shape, with: ink, style: StrokeStyle(lineWidth: 1.5, lineCap: .round, lineJoin: .round))
                return
            case "triangle":
                shape = Path { $0.move(to: CGPoint(x: tip.x - 7, y: y - 5)); $0.addLine(to: tip); $0.addLine(to: CGPoint(x: tip.x - 7, y: y + 5)); $0.closeSubpath() }
            case "circle":
                shape = Path(ellipseIn: CGRect(x: tip.x - 8, y: y - 4, width: 8, height: 8))
            case "diamond":
                shape = Path { $0.move(to: CGPoint(x: tip.x - 9, y: y)); $0.addLine(to: CGPoint(x: tip.x - 4.5, y: y - 5)); $0.addLine(to: tip); $0.addLine(to: CGPoint(x: tip.x - 4.5, y: y + 5)); $0.closeSubpath() }
            case "bar":
                shape = Path { $0.move(to: CGPoint(x: tip.x, y: y - 5)); $0.addLine(to: CGPoint(x: tip.x, y: y + 5)) }
                context.stroke(shape, with: ink, lineWidth: 1.5)
                return
            default:
                return
            }
            if open { context.stroke(shape, with: ink, lineWidth: 1.5) } else { context.fill(shape, with: ink) }
        }
        .scaleEffect(x: flip ? -1 : 1)
    }
}
