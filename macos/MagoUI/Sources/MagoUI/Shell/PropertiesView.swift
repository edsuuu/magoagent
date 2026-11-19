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
                ColorDotButton(title: "Contorno", color: props.stroke, picks: zip(state.strokePicks, ["Preto", "Vermelho", "Verde", "Azul", "Laranja"]).map { ($0, $1) }, allowsClear: false) { style(["stroke": $0]) }
            }
            if props.showFill {
                ColorDotButton(title: "Fundo", color: props.fill, picks: zip(state.backgroundPicks, fillNames).map { ($0, $1) }.filter { !props.fillTool || $0.0[3] > 0 }, allowsClear: !props.fillTool) {
                    style(["fill": $0])
                }
            }
            if props.showStroke || props.showFill { GlassDivider() }
            if props.showFillStyle {
                PickGroup(label: "Preenchimento", compact: compact, options: [
                    PickOption(tip: "Hachura", on: props.fillStyle == "hachure", icon: .hachure, args: ["fillStyle": "hachure"]),
                    PickOption(tip: "Hachura cruzada", on: props.fillStyle == "cross_hatch", icon: .crossHatch, args: ["fillStyle": "cross_hatch"]),
                    PickOption(tip: "Sólido", on: props.fillStyle == "solid", icon: .solidFill, args: ["fillStyle": "solid"]),
                ])
                GlassDivider()
            }
            if props.showStrokeOptions {
                PickGroup(label: "Espessura do traço", compact: compact, options: [
                    PickOption(tip: "Fino", on: props.strokeWidth == 1, icon: .width(1), args: ["strokeWidth": 1]),
                    PickOption(tip: "Espesso", on: props.strokeWidth == 2, icon: .width(2), args: ["strokeWidth": 2]),
                    PickOption(tip: "Muito espesso", on: props.strokeWidth == 4, icon: .width(4), args: ["strokeWidth": 4]),
                ])
                GlassDivider()
                PickGroup(label: "Estilo do traço", compact: compact, options: [
                    PickOption(tip: "Sólido", on: props.strokeStyle == "solid", icon: .dash("solid"), args: ["strokeStyle": "solid"]),
                    PickOption(tip: "Tracejado", on: props.strokeStyle == "dashed", icon: .dash("dashed"), args: ["strokeStyle": "dashed"]),
                    PickOption(tip: "Pontilhado", on: props.strokeStyle == "dotted", icon: .dash("dotted"), args: ["strokeStyle": "dotted"]),
                ])
                GlassDivider()
                PickGroup(label: "Precisão do traço", compact: compact, options: [
                    PickOption(tip: "Arquiteto", on: props.roughness == 0, icon: .sloppiness(0), args: ["roughness": 0]),
                    PickOption(tip: "Artista", on: props.roughness == 1, icon: .sloppiness(1), args: ["roughness": 1]),
                    PickOption(tip: "Cartunista", on: props.roughness == 2, icon: .sloppiness(2), args: ["roughness": 2]),
                ])
                GlassDivider()
            }
            if props.showArrow {
                PickGroup(label: "Tipo de seta", compact: compact, options: [
                    PickOption(tip: "Seta reta", on: !props.round, icon: .arrowType(false), args: ["round": false]),
                    PickOption(tip: "Seta curva", on: props.round, icon: .arrowType(true), args: ["round": true]),
                ])
                GlassDivider()
                HeadMenu(title: "Ponta inicial", head: props.startHead, end: false)
                HeadMenu(title: "Ponta final", head: props.endHead, end: true)
                GlassDivider()
            }
            if props.showEdges {
                PickGroup(label: "Arestas", compact: compact, options: [
                    PickOption(tip: "Pontudo", on: !props.round, icon: .edge(false), args: ["round": false]),
                    PickOption(tip: "Arredondado", on: props.round, icon: .edge(true), args: ["round": true]),
                ])
                GlassDivider()
            }
            if props.showFont {
                PickGroup(label: "Tamanho da fonte", compact: compact, options: [
                    PickOption(tip: "Pequeno", on: props.fontSize == 16, icon: .label("S"), args: ["fontSize": 16]),
                    PickOption(tip: "Médio", on: props.fontSize == 20, icon: .label("M"), args: ["fontSize": 20]),
                    PickOption(tip: "Grande", on: props.fontSize == 28, icon: .label("L"), args: ["fontSize": 28]),
                    PickOption(tip: "Muito grande", on: props.fontSize == 36, icon: .label("XL"), args: ["fontSize": 36]),
                ])
                GlassDivider()
            }
            if let name = props.frameName { FrameNameButton(name: name) }
            OpacityButton(opacity: props.opacity)
            if !props.actions.isEmpty {
                GlassDivider()
                ForEach(props.actions) { action in
                    GlassButton(icon: actionIcon(action.id), tip: action.keys.isEmpty ? action.label : "\(action.label) — \(action.keys)") {
                        shellVerb("command", ["name": action.id])
                    }
                }
            }
        }
    }

    private func actionIcon(_ id: String) -> MagoIcon {
        switch id {
        case "duplicate": .duplicate
        case "delete": .trash
        case "group": .group
        case "ungroup": .ungroup
        default: .lock(true)
        }
    }
}

// MARK: choices

struct PickOption: Identifiable {
    let tip: String
    let on: Bool
    let icon: MagoIcon
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
                IconView(icon: shown.icon, scale: 1.05).foregroundStyle(Color.glassInk.opacity(0.85)).frame(width: 30, height: 30)
            }
            .menuStyle(.borderlessButton)
            .menuIndicator(.hidden)
            .frame(width: 30, height: 30)
            .help(label)
        } else {
            ForEach(options) { option in
                GlassButton(icon: option.icon, tip: option.tip, on: option.on) { style(option.args) }
            }
        }
    }
}

// MARK: color, opacity, frame name

private extension Color {
    /// A drawing's color as the canvas paints it (`render::themed`): in the dark, inverted to 93% and turned back to its hue.
    init(canvas rgba: [Int], dark: Bool) {
        guard dark else {
            self.init(rgba: rgba)
            return
        }
        let flipped = rgba.prefix(3).map { (Double($0) * 0.07 + (255 - Double($0)) * 0.93).rounded() / 255 }
        let (red, green, blue) = (flipped[0], flipped[1], flipped[2])
        let out = [-0.574 * red + 1.43 * green + 0.144 * blue, 0.426 * red + 0.43 * green + 0.144 * blue, 0.426 * red + 1.43 * green - 0.856 * blue]
            .map { (min(max($0, 0), 1) * 255).rounded() / 255 }
        self.init(.sRGB, red: out[0], green: out[1], blue: out[2], opacity: Double(rgba[3]) / 255)
    }
}

/// A round button showing a color (a slash through it when there is none); its popover holds the picks and the system's picker.
/// The fills' names, in `BACKGROUND_PICKS`' order.
private let fillNames = ["Transparente", "Vermelho claro", "Verde claro", "Azul claro", "Amarelo claro"]

private struct ColorDotButton: View {
    let title: String
    let color: [Int]?
    /// Each pick and its name (the tip).
    let picks: [([Int], String)]
    let allowsClear: Bool
    let choose: ([Int]) -> Void
    @State private var open = false
    @Environment(\.colorScheme) private var scheme

    var body: some View {
        Button { open.toggle() } label: { dot }
            .buttonStyle(GlassButtonStyle(on: open))
            .help(title)
            .popover(isPresented: $open, arrowEdge: .bottom) {
                HStack(spacing: 8) {
                    ForEach(picks, id: \.1) { pick, name in
                        Button { choose(pick); open = false } label: { swatch(pick) }.buttonStyle(.plain).help(name)
                    }
                    ColorPicker("", selection: Binding(get: { Color(rgba: color ?? [0, 0, 0, 255]) }, set: { choose(Self.components($0)) }), supportsOpacity: false)
                        .labelsHidden().help("Mais cores")
                }
                .padding(12)
            }
    }

    @ViewBuilder private var dot: some View {
        if let color {
            Circle().fill(Color(canvas: color, dark: scheme == .dark)).overlay(Circle().stroke(open ? Color.glassAccent : Color.glassInk.opacity(0.3), lineWidth: 1)).frame(width: 16, height: 16)
        } else {
            ZStack {
                Circle().stroke(open ? Color.glassAccent : Color.glassInk.opacity(0.3), lineWidth: 1)
                Path { $0.move(to: CGPoint(x: 2, y: 14)); $0.addLine(to: CGPoint(x: 14, y: 2)) }.stroke(open ? Color.glassAccent : Color.glassInk.opacity(0.3), lineWidth: 1)
            }.frame(width: 16, height: 16)
        }
    }

    @ViewBuilder private func swatch(_ pick: [Int]) -> some View {
        let selected = pick == color || (pick[3] == 0 && color == nil)
        ZStack {
            if pick[3] == 0 {
                // transparent: a checkerboard (`ui::swatch`)
                Canvas { context, size in
                    context.fill(Path(CGRect(origin: .zero, size: size)), with: .color(.white))
                    let cell = size.width / 4
                    for row in 0..<4 {
                        for column in 0..<4 where (row + column) % 2 == 0 {
                            context.fill(Path(CGRect(x: CGFloat(column) * cell, y: CGFloat(row) * cell, width: cell, height: cell)), with: .color(Color(white: 225 / 255)))
                        }
                    }
                }
                .clipShape(RoundedRectangle(cornerRadius: 4))
            } else {
                RoundedRectangle(cornerRadius: 4).fill(Color(canvas: pick, dark: scheme == .dark))
            }
        }
        .frame(width: 20, height: 20)
        .overlay(RoundedRectangle(cornerRadius: 4).strokeBorder(Color.black.opacity(25 / 255), lineWidth: 1))
        .padding(1)
        .overlay(RoundedRectangle(cornerRadius: 5).stroke(Color.accent007aff, lineWidth: selected ? 1 : 0).padding(-2))
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
        Button { open.toggle() } label: { Text("\(opacity)%").font(.app(12)).frame(width: 36, height: 30) }
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
        GlassButton(icon: .edit, tip: "Nome do quadro", on: open) { open.toggle() }
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
            IconView(icon: .head(head, flip: !end), scale: 1.05).foregroundStyle(Color.glassInk.opacity(0.85)).frame(width: 30, height: 30)
        }
        .menuStyle(.borderlessButton)
        .menuIndicator(.hidden)
        .frame(width: 30, height: 30)
        .help(title)
    }
}

