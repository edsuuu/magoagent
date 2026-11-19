import AppKit

/// The menus egui draws over the canvas (a right click's, a connection's) as the system's own: `state.menu` opens one at the
/// pointer, and the row picked (or none, "") goes back as `ui.shell.menu`, which runs what egui runs.
@MainActor final class NativeMenu: NSObject {
    static let shared = NativeMenu()
    private var open = false
    /// The menu just closed: the snapshot can still carry it until Rust hears of the close.
    private var dismissed: MenuModel?

    func update(_ model: MenuModel?) {
        guard let model else {
            dismissed = nil
            return
        }
        guard !open, model != dismissed else { return }
        open = true
        let menu = build(model.entries)
        DispatchQueue.main.async { [self] in
            let picked = menu.popUp(positioning: nil, at: NSEvent.mouseLocation, in: nil)
            open = false
            dismissed = model
            if !picked { shellVerb("menu", ["id": ""]) }
        }
    }

    private func build(_ entries: [MenuEntry]) -> NSMenu {
        let menu = NSMenu()
        menu.autoenablesItems = false
        for entry in entries {
            switch entry.kind {
            case "sep": menu.addItem(.separator())
            case "title": menu.addItem(.sectionHeader(title: entry.label))
            default:
                let item = NSMenuItem(title: entry.label, action: #selector(pick), keyEquivalent: "")
                item.target = self
                item.representedObject = entry.id
                item.isEnabled = entry.enabled
                item.state = entry.check == true ? .on : .off
                if entry.danger { item.attributedTitle = NSAttributedString(string: entry.label, attributes: [.foregroundColor: NSColor.systemRed]) }
                if entry.kind == "sub" { item.submenu = build(entry.items) }
                shortcut(entry.keys, item)
                menu.addItem(item)
            }
        }
        return menu
    }

    @objc private func pick(_ item: NSMenuItem) {
        shellVerb("menu", ["id": item.representedObject as? String ?? ""])
    }

    /// The keys egui shows at the row's right ("⌘K", "Cmd+Shift+L", "Delete") as the item's key equivalent.
    private func shortcut(_ keys: String, _ item: NSMenuItem) {
        let spelled = keys.replacingOccurrences(of: "⌘", with: "Cmd+").replacingOccurrences(of: "⇧", with: "Shift+")
            .replacingOccurrences(of: "⌥", with: "Option+").replacingOccurrences(of: "⌃", with: "Ctrl+")
        let parts = spelled.split(separator: "+").map { $0.trimmingCharacters(in: .whitespaces) }
        let named = ["Delete": "\u{8}", "Backspace": "\u{8}", "Enter": "\r", "Esc": "\u{1b}", "Tab": "\t"]
        guard let last = parts.last, let key = last.count == 1 ? last.lowercased() : named[last] else { return }
        var flags: NSEvent.ModifierFlags = []
        for part in parts.dropLast() {
            switch part {
            case "Cmd": flags.insert(.command)
            case "Shift": flags.insert(.shift)
            case "Option", "Alt": flags.insert(.option)
            case "Ctrl": flags.insert(.control)
            default: break
            }
        }
        item.keyEquivalent = key
        item.keyEquivalentModifierMask = flags
    }
}
