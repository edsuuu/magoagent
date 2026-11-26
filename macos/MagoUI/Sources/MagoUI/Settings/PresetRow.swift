import SwiftUI

/// A terminal's mark as the other SwiftUI areas draw it (the egui ones are the agents' own logos): by its key, else by
/// the command's first word.
private func markSymbol(_ key: String) -> String {
    switch key {
    case "claude": "sparkle"
    case "codex": "chevron.left.forwardslash.chevron.right"
    case "opencode": "rectangle.portrait"
    case "agents": "person.3"
    case "monitor": "gauge"
    case "search": "magnifyingglass"
    case "composer": "text.bubble"
    case "palette": "bolt"
    case "grid": "square.grid.3x3"
    case "eye": "eye"
    case "map": "map"
    default: "terminal"
    }
}

/// Claude's mark wears its own orange whatever the tint (`terminal_icon` paints its logo as it is).
private func markColor(_ key: String, _ tint: Color) -> Color { key == "claude" ? rgb(0xd97757) : tint }

/// The marks offered, in `TERMINAL_ICONS`' order (the first is "by the command").
private let markKeys = ["", "terminal", "claude", "codex", "opencode", "agents", "monitor", "search", "composer", "palette", "grid", "eye", "map"]

/// A preset of the new-terminal dialog (`terminal.rs`): its check, its mark, the name over the command (fields while it
/// is edited), and ▲ ▼ edit ⊖ at the right; while edited, the marks and tints its terminals wear under it.
struct SettingsPresetRow: View {
    let item: SettingsItem
    @Environment(\.colorScheme) private var scheme

    private var editing: Bool { item.value["editing"].bool }
    private var name: String { item.label.trimmingCharacters(in: .whitespaces).isEmpty ? "sem nome" : item.label }
    private var tint: Color {
        let colors = item.value["colors"].array
        let index = item.value["color"].int
        return colors.indices.contains(index) && colors[index] != .null ? Color(hex: colors[index].string) : SK.of(scheme).fg
    }

    var body: some View {
        let k = SK.of(scheme)
        VStack(alignment: .leading, spacing: 0) {
            HStack(spacing: 9.75) {
                PresetCheck(on: item.value["on"].bool, name: name) { setSetting(item.id + ".on", !item.value["on"].bool) }
                Image(systemName: markSymbol(item.value["mark"].string)).font(.system(size: 12))
                    .foregroundStyle(markColor(item.value["mark"].string, tint)).frame(width: 16.25)
                if editing {
                    PresetField(id: item.id + ".name", value: item.value["name"].string, placeholder: "Nome", label: "Nome do preset", width: 150, first: true)
                    PresetField(id: item.id + ".command", value: item.value["command"].string, placeholder: "comando (vazio: shell)", label: "Comando do preset", width: 210)
                    Spacer(minLength: 0)
                } else {
                    VStack(alignment: .leading, spacing: 0) {
                        Text(name).kitLine(SK.body, SK.bodyLine).foregroundStyle(item.label.trimmingCharacters(in: .whitespaces).isEmpty ? k.mutedFg : k.fg)
                        Text(item.hint.isEmpty ? "shell" : item.hint).kitLine(SK.small, SK.smallLine).foregroundStyle(k.mutedFg)
                    }
                    .lineLimit(1)
                    .frame(maxWidth: .infinity, alignment: .leading)
                }
                HStack(spacing: 6.5) {
                    // the two chevrons side by side, as one control
                    HStack(spacing: 0) {
                        GhostButton(icon: "chevronUp", name: "Mover \(name) para cima", enabled: !item.value["first"].bool) { op("up") }
                        GhostButton(icon: "chevronDown", name: "Mover \(name) para baixo", enabled: !item.value["last"].bool) { op("down") }
                    }
                    GhostButton(icon: editing ? "check" : "edit", name: "\(editing ? "Pronto" : "Editar") \(name)") { op("edit") }
                    GhostButton(icon: "trash", name: "Remover \(name)", danger: true) { op("remove") }
                }
            }
            .padding(.horizontal, 9.75)
            .frame(height: 48.25)
            if editing { PresetLook(item: item, tint: tint).padding(.leading, 9.75 + 18 + 9.75) }
        }
    }

    private func op(_ name: String) {
        settingsSend("press", ["id": "\(item.id).\(name)"])
    }
}

/// The check at a preset's left: a filled circle when it shows in the dialog.
private struct PresetCheck: View {
    let on: Bool
    let name: String
    let action: () -> Void
    @Environment(\.colorScheme) private var scheme

    var body: some View {
        let k = SK.of(scheme)
        Button(action: action) {
            ZStack {
                if on {
                    Circle().fill(k.primary).frame(width: 14, height: 14)
                    SettingsIcon(name: "check", color: .white, scale: 0.62)
                } else {
                    Circle().stroke(k.mutedFg, lineWidth: 1.2).frame(width: 13, height: 13)
                }
            }
            .frame(width: 18, height: 18)
            .contentShape(Rectangle())
        }
        .buttonStyle(.plain)
        .handCursor()
        .accessibilityLabel("Mostrar \(name) no diálogo")
        .accessibilityAddTraits(on ? .isSelected : [])
    }
}

/// The name or the command of a preset being edited: each key goes to it.
private struct PresetField: View {
    let id: String
    let value: String
    let placeholder: String
    let label: String
    let width: CGFloat
    var first = false
    @State private var draft = ""
    @FocusState private var focused: Bool
    @Environment(\.colorScheme) private var scheme

    var body: some View {
        let k = SK.of(scheme)
        TextField(label, text: $draft, prompt: Text(placeholder).foregroundStyle(k.mutedFg))
            .focused($focused)
            .kitWell(k, width: width, focused: focused)
            .onAppear {
                draft = value
                // straight into the name, ready to type
                if first { focused = true }
            }
            .onChange(of: draft) { _, now in if now != value { setSetting(id, now) } }
    }
}

/// Under a preset being edited: a row of the marks ("auto" for the one its command picks) and a row of the tints.
private struct PresetLook: View {
    let item: SettingsItem
    let tint: Color
    @Environment(\.colorScheme) private var scheme

    var body: some View {
        let k = SK.of(scheme)
        let colors = item.value["colors"].array
        VStack(alignment: .leading, spacing: 8) {
            HStack(spacing: 4) {
                ForEach(Array(markKeys.enumerated()), id: \.offset) { index, key in
                    LookCell(chosen: item.value["icon"].int == index, name: item.options.indices.contains(index) ? item.options[index] : key) {
                        setSetting(item.id + ".icon", index)
                    } content: {
                        if key.isEmpty {
                            Text("auto").font(.kit(9)).foregroundStyle(k.mutedFg)
                        } else {
                            Image(systemName: markSymbol(key)).font(.system(size: 11)).foregroundStyle(markColor(key, tint))
                        }
                    }
                }
            }
            HStack(spacing: 4) {
                ForEach(Array(colors.enumerated()), id: \.offset) { index, color in
                    Swatch(color: color == .null ? nil : Color(hex: color.string), chosen: item.value["color"].int == index,
                           name: index == 0 ? "Cor padrão" : "Cor \(index)") {
                        setSetting(item.id + ".color", index)
                    }
                }
            }
            .padding(.leading, 3)
        }
        .frame(height: 50, alignment: .top)
        .padding(.bottom, 12)
    }
}

private struct LookCell<Content: View>: View {
    let chosen: Bool
    let name: String
    let action: () -> Void
    @ViewBuilder let content: () -> Content
    @Environment(\.colorScheme) private var scheme
    @State private var hover = false

    var body: some View {
        let k = SK.of(scheme)
        Button(action: action) {
            content()
                .frame(width: 24, height: 24)
                .background(hover ? k.fg.opacity(0.08) : .clear, in: .rect(cornerRadius: 6.5))
                .overlay { if chosen { RoundedRectangle(cornerRadius: 6.5).strokeBorder(k.primary, lineWidth: 1.5) } }
                .contentShape(Rectangle())
        }
        .buttonStyle(.plain)
        .onHover { hover = $0 }
        .handCursor()
        .help(name)
        .accessibilityLabel(name)
        .accessibilityAddTraits(chosen ? .isSelected : [])
    }
}

private struct Swatch: View {
    let color: Color?
    let chosen: Bool
    let name: String
    let action: () -> Void
    @Environment(\.colorScheme) private var scheme

    var body: some View {
        let k = SK.of(scheme)
        Button(action: action) {
            ZStack {
                if let color { Circle().fill(color).frame(width: 14, height: 14) } else { Circle().stroke(k.mutedFg, lineWidth: 1).frame(width: 13, height: 13) }
                if chosen { Circle().stroke(k.primary, lineWidth: 1.5).frame(width: 19, height: 19) }
            }
            .frame(width: 18, height: 18)
            .contentShape(Rectangle())
        }
        .buttonStyle(.plain)
        .handCursor()
        .accessibilityLabel(name)
        .accessibilityAddTraits(chosen ? .isSelected : [])
    }
}
