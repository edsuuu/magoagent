import AppKit
import SwiftUI

/// The names egui gives the keys that have no character of their own, by the Mac's key codes.
private let namedKeys: [UInt16: String] = [
    49: "Space", 36: "Enter", 76: "Enter", 53: "Escape", 51: "Backspace", 117: "Delete", 48: "Tab",
    123: "ArrowLeft", 124: "ArrowRight", 125: "ArrowDown", 126: "ArrowUp",
    115: "Home", 119: "End", 116: "PageUp", 121: "PageDown",
    122: "F1", 120: "F2", 99: "F3", 118: "F4", 96: "F5", 97: "F6", 98: "F7", 100: "F8", 101: "F9", 109: "F10", 103: "F11", 111: "F12",
]

/// A shortcut: its keys (a click records others), Limpar and Padrão, a global one's switch. While it changes: what was
/// pressed, Salvar (or Enter) and Cancelar (or Esc).
struct HotkeyRow: View {
    let item: SettingsItem
    @State private var held = ""
    @State private var monitor: Any?

    private var keys: SettingsKeys { item.keys! }

    var body: some View {
        Shell(item: item) {
            HStack(spacing: 6) {
                if keys.editing {
                    Button("Cancelar") { act("cancel") }
                    if keys.pending {
                        Button("Salvar") { act("save") }.buttonStyle(.borderedProminent).disabled(keys.problem)
                    }
                    keysButton(edge: keys.problem ? .red : .accentColor)
                } else {
                    if keys.global {
                        Toggle(item.label, isOn: Binding(get: { keys.enabled }, set: { settingsSend("hotkey", ["op": "switch", "id": item.id, "on": $0]) }))
                            .labelsHidden().toggleStyle(.switch)
                    }
                    iconButton("arrow.counterclockwise", "Voltar ao padrão", enabled: keys.changed) { act("default") }
                    iconButton("trash", "Limpar: deixar sem tecla", enabled: keys.canClear) { act("clear") }
                    keysButton(edge: nil)
                }
            }
        }
        .onChange(of: keys.editing, initial: true) { _, editing in editing ? listen() : stop() }
        .onDisappear { stop() }
    }

    private func act(_ op: String) { settingsSend("hotkey", ["op": op, "id": item.id]) }

    private func iconButton(_ symbol: String, _ tip: String, enabled: Bool, _ action: @escaping () -> Void) -> some View {
        Button(action: action) { Image(systemName: symbol) }
            .buttonStyle(.borderless).disabled(!enabled).help(tip).accessibilityLabel("\(tip): \(item.label)")
    }

    private func keysButton(edge: Color?) -> some View {
        let waiting = keys.editing && !keys.pending
        let text = waiting ? (held.isEmpty ? "Pressione as teclas…" : held) : keys.text
        return Button(text) { if !keys.editing { act("record") } }
            .buttonStyle(.bordered)
            .foregroundStyle(waiting || keys.text == "Sem tecla" ? .secondary : .primary)
            .overlay(RoundedRectangle(cornerRadius: 6).stroke(edge ?? .clear, lineWidth: 1.5))
            .frame(minWidth: 96)
            .help("Clique para gravar outras teclas")
    }

    /// While recording, the keys pressed go to the shortcut and nowhere else.
    private func listen() {
        stop()
        let id = item.id
        monitor = NSEvent.addLocalMonitorForEvents(matching: [.keyDown, .flagsChanged]) { event in
            // plain values out of the event first: it is not Sendable
            let (isFlags, flags, code, chars, again) = (event.type == .flagsChanged, event.modifierFlags, event.keyCode, event.charactersIgnoringModifiers ?? "", event.isARepeat)
            let swallow: Bool = MainActor.assumeIsolated {
                if isFlags {
                    var parts: [String] = []
                    if flags.contains(.command) { parts.append("Cmd") }
                    if flags.contains(.option) { parts.append("Alt") }
                    if flags.contains(.shift) { parts.append("Shift") }
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
