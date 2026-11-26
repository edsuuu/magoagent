import AppKit
import SwiftUI

/// The names egui gives the keys that have no character of their own, by the Mac's key codes.
private let namedKeys: [UInt16: String] = [
    49: "Space", 36: "Enter", 76: "Enter", 53: "Escape", 51: "Backspace", 117: "Delete", 48: "Tab",
    123: "ArrowLeft", 124: "ArrowRight", 125: "ArrowDown", 126: "ArrowUp",
    115: "Home", 119: "End", 116: "PageUp", 121: "PageDown",
    122: "F1", 120: "F2", 99: "F3", 118: "F4", 96: "F5", 97: "F6", 98: "F7", 100: "F8", 101: "F9", 109: "F10", 103: "F11", 111: "F12",
]

/// A shortcut (`hotkeys.rs`): at the right, a global one's switch, Padrão, Limpar and its keys (a click records others).
/// While it changes: what was pressed, Salvar (or Enter) and Cancelar (or Esc).
struct HotkeyRow: View {
    let item: SettingsItem
    @State private var held = ""
    @State private var monitor: Any?

    /// `ItemRow` only makes the row when the shortcut came with its keys.
    private var keys: SettingsKeys { item.keys! }

    var body: some View {
        RowShell(label: item.label, hint: item.hint) {
            if keys.editing {
                KeysButton(text: shown, none: waiting, edge: keys.problem ? .destructive : .primary) {}
                if keys.pending { SaveButton(enabled: !keys.problem) { act("save") } }
                Button("Cancelar") { act("cancel") }.buttonStyle(KitButtonStyle())
            } else {
                KeysButton(text: keys.text, none: keys.text == "Sem tecla", edge: nil) { act("record") }
                    .help("Clique para gravar outras teclas")
                GhostButton(icon: "trash", name: "Limpar: \(item.label)", enabled: keys.canClear, danger: true) { act("clear") }
                GhostButton(icon: "restart", name: "Voltar ao padrão: \(item.label)", enabled: keys.changed) { act("default") }
                if keys.global {
                    Toggle(item.label, isOn: Binding(get: { keys.enabled }, set: { settingsSend("hotkey", ["op": "switch", "id": item.id, "on": $0]) }))
                        .toggleStyle(SwitchStyle())
                }
            }
        }
        .onChange(of: keys.editing, initial: true) { _, editing in editing ? listen() : stop() }
        .onDisappear { stop() }
    }

    /// Waiting for keys: the modifiers held, as egui names them (Cmd is Ctrl there), or the invitation.
    private var waiting: Bool { !keys.pending }
    private var shown: String { waiting ? (held.isEmpty ? "Pressione as teclas…" : held) : keys.text }

    private func act(_ op: String) { settingsSend("hotkey", ["op": op, "id": item.id]) }

    /// While recording, the keys pressed go to the shortcut and nowhere else.
    private func listen() {
        stop()
        let id = item.id
        monitor = NSEvent.addLocalMonitorForEvents(matching: [.keyDown, .flagsChanged]) { event in
            // plain values out of the event first: it is not Sendable
            let (isFlags, flags, code, chars, again) = (event.type == .flagsChanged, event.modifierFlags, event.keyCode, event.charactersIgnoringModifiers ?? "", event.isARepeat)
            let swallow: Bool = MainActor.assumeIsolated {
                if isFlags {
                    // `keys::held_label`: the command key reads Ctrl, as the egui page shows it
                    let parts = [(flags.contains(.command), "Ctrl"), (flags.contains(.option), "Alt"), (flags.contains(.shift), "Shift")].filter(\.0).map(\.1)
                    held = parts.isEmpty ? "" : parts.joined(separator: "+") + "+…"
                    return false
                }
                let name = namedKeys[code] ?? chars.uppercased()
                if !again && !name.isEmpty {
                    settingsSend("hotkey", [
                        "op": "key", "id": id, "key": name,
                        "cmd": flags.contains(.command), "alt": flags.contains(.option), "shift": flags.contains(.shift),
                    ])
                }
                return true
            }
            return swallow ? nil : event
        }
    }

    private func stop() {
        if let monitor { NSEvent.removeMonitor(monitor) }
        monitor = nil
        held = ""
    }
}

/// A shortcut's keys as a button 30 high, at least 96 wide; its line the accent while it changes (red while what was
/// pressed can't be saved).
private struct KeysButton: View {
    enum Edge { case primary, destructive }
    let text: String
    let none: Bool
    let edge: Edge?
    let action: () -> Void
    @Environment(\.colorScheme) private var scheme
    @State private var hover = false

    var body: some View {
        let k = SK.of(scheme)
        let line = edge.map { $0 == .primary ? k.primary : k.destructive } ?? k.buttonLine
        Button(action: action) {
            Text(text).font(.kit(12.5)).foregroundStyle(none || text.hasSuffix("…") ? k.mutedFg : k.fg).lineLimit(1).fixedSize()
                .padding(.horizontal, 12)
                .frame(minWidth: 96, minHeight: 30, maxHeight: 30)
                .background(hover ? k.buttonHover : k.button, in: .rect(cornerRadius: 8))
                .overlay { RoundedRectangle(cornerRadius: 8).strokeBorder(line, lineWidth: edge == nil ? 1 : 1.5) }
                .contentShape(.rect(cornerRadius: 8))
        }
        .buttonStyle(.plain)
        .onHover { hover = $0 }
        .handCursor()
        .accessibilityLabel(text)
    }
}

/// Salvar, in the accent; dimmed and deaf while what was pressed can't be saved.
private struct SaveButton: View {
    let enabled: Bool
    let action: () -> Void
    @Environment(\.colorScheme) private var scheme
    @State private var hover = false

    var body: some View {
        let k = SK.of(scheme)
        Button(action: action) {
            Text("Salvar").font(.kit(12.5)).foregroundStyle(.white.opacity(enabled ? 1 : 0.6))
                .padding(.horizontal, 12)
                .frame(height: 30)
                .background {
                    RoundedRectangle(cornerRadius: 8).fill(k.primary.opacity(enabled ? 1 : 0.35))
                    if enabled && hover { RoundedRectangle(cornerRadius: 8).fill(.white.opacity(0.12)) }
                }
                .contentShape(.rect(cornerRadius: 8))
        }
        .buttonStyle(.plain)
        .disabled(!enabled)
        .onHover { hover = $0 }
        .handCursor()
    }
}
