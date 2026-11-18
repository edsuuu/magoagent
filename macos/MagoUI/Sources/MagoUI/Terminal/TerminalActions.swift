import AppKit

/// What the card's menu, its keys and the pasteboard do. The rows that already have an action in the app go
/// through it (`terminal_menu`, `terminal_find`, `terminal_restart`, `terminal_processes`); `ui.terminal.menu`
/// runs the right-click menu's own rows (duplicate, lock, delete, disconnect…).
@MainActor
enum TerminalActions {
    static func toPasteboard(_ text: String) {
        guard !text.isEmpty else { return }
        let pasteboard = NSPasteboard.general
        pasteboard.clearContents()
        pasteboard.setString(text, forType: .string)
    }

    /// ⌘C: what is selected goes to the pasteboard (nothing selected: nothing).
    static func copy(id: String) {
        Task {
            let reply = await MagoStore.shared.act("ui.terminal.copy", ["id": id])
            toPasteboard(reply.text)
        }
    }

    /// ⌘V: the pasteboard's text, or a picture as the path of a png (what the egui terminal sends).
    static func paste(id: String) {
        let pasteboard = NSPasteboard.general
        if let text = pasteboard.string(forType: .string), !text.isEmpty {
            MagoStore.shared.send("ui.terminal.paste", ["id": id, "text": text])
        } else if let image = NSImage(pasteboard: pasteboard), let tiff = image.tiffRepresentation,
                  let png = NSBitmapImageRep(data: tiff)?.representation(using: .png, properties: [:]) {
            let url = FileManager.default.temporaryDirectory.appendingPathComponent("magoagent-paste-\(UUID().uuidString).png")
            if (try? png.write(to: url)) != nil {
                MagoStore.shared.send("ui.terminal.paste", ["id": id, "path": url.path])
            }
        }
    }

    /// A row of the terminal's menu (`entries` of the topic).
    static func pick(_ entry: String, state: TerminalState) {
        let store = MagoStore.shared
        switch entry {
        case "copy_sel": copy(id: state.id)
        case "paste_text": paste(id: state.id)
        case "rename": rename(state)
        // the egui rename dialog is not the view's: the name goes through the MCP's action
        default: store.send("ui.terminal.menu", ["id": state.id, "entry": entry])
        }
    }

    static func rename(_ state: TerminalState) {
        let alert = NSAlert()
        alert.messageText = "Renomear"
        alert.addButton(withTitle: "Renomear")
        alert.addButton(withTitle: "Cancelar")
        let field = NSTextField(string: state.name)
        field.frame = NSRect(x: 0, y: 0, width: 240, height: 24)
        alert.accessoryView = field
        alert.window.initialFirstResponder = field
        guard alert.runModal() == .alertFirstButtonReturn else { return }
        let name = field.stringValue.trimmingCharacters(in: .whitespaces)
        guard !name.isEmpty else { return }
        MagoStore.shared.send("terminal_menu", ["name": state.id, "action": "rename", "value": name])
    }
}
