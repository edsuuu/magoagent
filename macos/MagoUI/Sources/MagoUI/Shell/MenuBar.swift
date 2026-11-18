import AppKit

/// The menu bar: File, Edit, View... each item runs the app's own command (`ui.shell.command`, the ids of Ajustes › Atalhos), with the
/// keys the person set there (the snapshot's `shortcuts`).
@MainActor final class MenuBar: NSObject {
    static let shared = MenuBar()
    private var installedKeys: [MenuKey]?

    /// Installs the menus, and again whenever the shortcuts change.
    func update(_ keys: [MenuKey]) {
        guard keys != installedKeys else { return }
        installedKeys = keys
        NSApp.mainMenu = build(keys)
    }

    @objc private func fire(_ item: NSMenuItem) {
        guard let name = item.representedObject as? String else { return }
        switch name {
        // a field being edited keeps its own undo and selection
        case "undo", "redo":
            if let text = editedText() { (name == "undo" ? text.undoManager?.undo() : text.undoManager?.redo()) } else { shellVerb("command", ["name": name]) }
        case "selectAll":
            if let text = editedText() { text.selectAll(nil) } else { shellVerb("command", ["name": name]) }
        // the egui window's Cmd+Q: it saves and ends the shells before the app goes
        case "quit": shellVerb("window", ["action": "quit"])
        default: shellVerb("command", ["name": name])
        }
    }

    @objc private func about() { NSApp.orderFrontStandardAboutPanel(nil) }

    private func editedText() -> NSTextView? {
        (NSApp.keyWindow?.firstResponder as? NSTextView).flatMap { $0.isEditable ? $0 : nil }
    }

    private func build(_ keys: [MenuKey]) -> NSMenu {
        let bar = NSMenu()
        func key(_ id: String) -> MenuKey? { keys.first { $0.id == id } }
        func item(_ title: String, _ id: String, in menu: NSMenu) {
            let entry = NSMenuItem(title: title, action: #selector(fire(_:)), keyEquivalent: "")
            entry.target = self
            entry.representedObject = id
            if let shortcut = key(id), !shortcut.key.isEmpty {
                entry.keyEquivalent = shortcut.key
                var mask: NSEvent.ModifierFlags = []
                if shortcut.command { mask.insert(.command) }
                if shortcut.option { mask.insert(.option) }
                if shortcut.shift { mask.insert(.shift) }
                entry.keyEquivalentModifierMask = mask
            } else if let shortcut = key(id) {
                // a key AppKit has no character for (F5...): shown beside the title
                entry.title = "\(title)    \(shortcut.text)"
            }
            menu.addItem(entry)
        }
        func submenu(_ title: String, _ build: (NSMenu) -> Void) {
            let menu = NSMenu(title: title)
            build(menu)
            let host = NSMenuItem(title: title, action: nil, keyEquivalent: "")
            host.submenu = menu
            bar.addItem(host)
        }

        submenu("MagoAgent") { menu in
            let about = NSMenuItem(title: "Sobre o MagoAgent", action: #selector(about), keyEquivalent: "")
            about.target = self
            menu.addItem(about)
            menu.addItem(.separator())
            item("Ajustes…", "settings", in: menu)
            menu.addItem(.separator())
            menu.addItem(NSMenuItem(title: "Ocultar o MagoAgent", action: #selector(NSApplication.hide(_:)), keyEquivalent: "h"))
            let others = NSMenuItem(title: "Ocultar os outros", action: #selector(NSApplication.hideOtherApplications(_:)), keyEquivalent: "h")
            others.keyEquivalentModifierMask = [.command, .option]
            menu.addItem(others)
            menu.addItem(NSMenuItem(title: "Mostrar tudo", action: #selector(NSApplication.unhideAllApplications(_:)), keyEquivalent: ""))
            menu.addItem(.separator())
            let quit = NSMenuItem(title: "Encerrar o MagoAgent", action: #selector(fire(_:)), keyEquivalent: "q")
            quit.target = self
            quit.representedObject = "quit"
            menu.addItem(quit)
        }
        submenu("Arquivo") { menu in
            item("Novo chat", "newChat", in: menu)
            menu.addItem(.separator())
            item("Abrir…", "open", in: menu)
            item("Salvar", "save", in: menu)
            item("Salvar como…", "saveAs", in: menu)
            menu.addItem(.separator())
            item("Exportar imagem…", "export", in: menu)
            menu.addItem(.separator())
            menu.addItem(NSMenuItem(title: "Fechar a janela", action: #selector(NSWindow.performClose(_:)), keyEquivalent: "w"))
        }
        submenu("Editar") { menu in
            item("Desfazer", "undo", in: menu)
            item("Refazer", "redo", in: menu)
            menu.addItem(.separator())
            menu.addItem(NSMenuItem(title: "Recortar", action: #selector(NSText.cut(_:)), keyEquivalent: "x"))
            menu.addItem(NSMenuItem(title: "Copiar", action: #selector(NSText.copy(_:)), keyEquivalent: "c"))
            menu.addItem(NSMenuItem(title: "Colar", action: #selector(NSText.paste(_:)), keyEquivalent: "v"))
            item("Selecionar tudo", "selectAll", in: menu)
            menu.addItem(.separator())
            item("Duplicar", "duplicate", in: menu)
            item("Agrupar", "group", in: menu)
            item("Desagrupar", "ungroup", in: menu)
            item("Travar ou destravar", "lock", in: menu)
            item("Arrumar a seleção", "tidy", in: menu)
            let layers = NSMenu(title: "Camadas")
            item("Uma camada à frente", "layerForward", in: layers)
            item("Uma camada atrás", "layerBackward", in: layers)
            item("Trazer para a frente de tudo", "layerFront", in: layers)
            item("Enviar para trás de tudo", "layerBack", in: layers)
            let host = NSMenuItem(title: "Camadas", action: nil, keyEquivalent: "")
            host.submenu = layers
            menu.addItem(host)
            menu.addItem(.separator())
            item("Limpar o canvas", "clearCanvas", in: menu)
        }
        submenu("Exibir") { menu in
            item("Paleta de comandos", "palette", in: menu)
            menu.addItem(.separator())
            item("Tela do chat", "showChat", in: menu)
            item("Voltar ao canvas", "showCanvas", in: menu)
            item("Abrir a Árvore de Arquivos", "openTree", in: menu)
            item("Visão dos andares", "floors", in: menu)
            item("Próximo agente que precisa de você", "nextNeedingYou", in: menu)
            menu.addItem(.separator())
            item("Barra lateral", "sidebar", in: menu)
            item("Mostrar ou esconder a grade", "grid", in: menu)
            item("Minimapa", "minimap", in: menu)
            item("Tema claro ou escuro", "theme", in: menu)
            menu.addItem(.separator())
            item("Aproximar", "zoomIn", in: menu)
            item("Afastar", "zoomOut", in: menu)
            item("Voltar a 100%", "zoomReset", in: menu)
            item("Ver tudo", "fitAll", in: menu)
            item("Ver a seleção", "fitSelection", in: menu)
            menu.addItem(.separator())
            let full = NSMenuItem(title: "Tela cheia", action: #selector(NSWindow.toggleFullScreen(_:)), keyEquivalent: "f")
            full.keyEquivalentModifierMask = [.command, .control]
            menu.addItem(full)
        }
        submenu("Janela") { menu in
            menu.addItem(NSMenuItem(title: "Minimizar", action: #selector(NSWindow.performMiniaturize(_:)), keyEquivalent: "m"))
            menu.addItem(NSMenuItem(title: "Ampliar", action: #selector(NSWindow.performZoom(_:)), keyEquivalent: ""))
            NSApp.windowsMenu = menu
        }
        submenu("Ajuda") { menu in
            item("Ver a apresentação", "tour", in: menu)
        }
        return bar
    }
}

/// A tool's letter picks the tool while the keys are free (no field, terminal or dialog holds them), as egui's `handle_shortcuts` does:
/// Rust says which tools answer now. Commands with Command held are the menus'.
@MainActor final class ToolKeys {
    static let shared = ToolKeys()
    private var monitor: Any?
    /// The keys some tool wears (V, H, 1...), from the snapshot's badges.
    var keys: Set<String> = []

    func start(window: NSWindow) {
        guard monitor == nil else { return }
        // events come on the main thread; the monitor's closure is not typed as such
        monitor = NSEvent.addLocalMonitorForEvents(matching: .keyDown) { [weak self, weak window] event in
            let taken = MainActor.assumeIsolated { self?.take(event, in: window) ?? false }
            return taken ? nil : event
        }
    }

    private func take(_ event: NSEvent, in window: NSWindow?) -> Bool {
        guard event.window === window, window?.firstResponder is NSText == false else { return false }
        guard event.modifierFlags.intersection([.command, .control, .option]).isEmpty,
              let letter = event.charactersIgnoringModifiers?.lowercased(), letter.count == 1,
              ShellModel.shared.state?.keysFree == true, keys.contains(letter) else { return false }
        shellVerb("tool_key", ["key": letter, "shift": event.modifierFlags.contains(.shift)])
        return true
    }
}
