import AppKit
import SwiftUI

/// The main window in SwiftUI: full-size content under a see-through title bar, the window's buttons over the sidebar's top.
/// Everything it shows is topic "shell" (src/app/ui/native/); everything it does goes back as `ui.shell.*` or the app's own actions.
@MainActor public enum ShellArea {
    /// Opens the window and installs the menu bar. The areas that register views (`MagoViews`) start before this.
    public static func start() {
        ShellController.shared.open()
    }
}

/// The window, its delegate and what follows the snapshot into AppKit (the title, the appearance, the menus, the tools' keys).
@MainActor final class ShellController: NSObject, NSWindowDelegate {
    static let shared = ShellController()
    private var window: NSWindow?
    /// The middle of the window's buttons, from the top (the toolbar's row: TOP_ROW 12 + half of its 40).
    private let buttonsCenter: CGFloat = 32

    func open() {
        guard window == nil else { return }
        let frame = NSRect(x: 0, y: 0, width: 1440, height: 920)
        let window = MagoWindow.make(title: "MagoAgent", frame: frame, borderless: false, transparent: false, level: .normal, content: ShellRoot())
        window.styleMask.formUnion([.titled, .closable, .miniaturizable, .resizable, .fullSizeContentView])
        window.titlebarAppearsTransparent = true
        window.titleVisibility = .hidden
        window.isMovableByWindowBackground = false
        window.minSize = NSSize(width: 920, height: 640)
        window.collectionBehavior.insert(.fullScreenPrimary)
        window.delegate = self
        window.center()
        self.window = window
        ToolKeys.shared.start(window: window)
        let center = NotificationCenter.default
        // AppKit lays the buttons out again on a resize, a full screen and a new key window
        for name in [NSWindow.didResizeNotification, NSWindow.didEndLiveResizeNotification, NSWindow.didExitFullScreenNotification, NSWindow.didBecomeKeyNotification] {
            center.addObserver(forName: name, object: window, queue: .main) { [weak self] _ in MainActor.assumeIsolated { self?.placeButtons() } }
        }
        // a Dock click or the tray's Show brings back a window hidden by Fechar mantém em segundo plano
        center.addObserver(forName: NSApplication.didBecomeActiveNotification, object: nil, queue: .main) { [weak self] _ in
            MainActor.assumeIsolated { self?.reopenIfHidden() }
        }
        window.makeKeyAndOrderFront(nil)
        placeButtons()
    }

    /// What the snapshot says that AppKit holds: the title, the theme, the menu bar and the tools' keys.
    func apply(_ state: ShellState) {
        window?.title = state.title
        window?.appearance = state.theme == "system" ? nil : NSAppearance(named: state.dark ? .darkAqua : .aqua)
        MenuBar.shared.update(state.shortcuts)
        let badges = (state.tools.main + state.tools.drawing + state.tools.extra).map { $0.badge.lowercased() }.filter { $0.count == 1 }
        // the numeric keys pick the same tools; Shift+X is Desenhar para criar forma
        ToolKeys.shared.keys = Set(badges + ["1", "2", "3", "4", "5", "6", "7", "8", "0", "x"])
        placeButtons()
    }

    /// Centers the close / minimize / zoom buttons `buttonsCenter` below the window's top (Electron's trafficLightPosition): the title
    /// bar's container grows to hold them.
    private func placeButtons() {
        guard let window else { return }
        let kinds: [NSWindow.ButtonType] = [.closeButton, .miniaturizeButton, .zoomButton]
        let buttons = kinds.compactMap { window.standardWindowButton($0) }
        guard let container = buttons.first?.superview?.superview else { return }
        var bounds = container.frame
        bounds.size.height = buttonsCenter * 2
        bounds.origin.y = window.frame.height - bounds.size.height
        container.frame = bounds
        for (index, button) in buttons.enumerated() {
            var origin = button.frame.origin
            origin.x = 18 + CGFloat(index) * 20
            origin.y = (bounds.size.height - button.frame.height) / 2
            button.setFrameOrigin(origin)
        }
    }

    private func reopenIfHidden() {
        guard let window, !window.isVisible, !window.isMiniaturized else { return }
        window.makeKeyAndOrderFront(nil)
    }

    // The close goes to Rust: Fechar mantém em segundo plano hides the window, else the app ends.
    func windowShouldClose(_ sender: NSWindow) -> Bool {
        Task { @MainActor in
            let reply = await MagoStore.shared.act("ui.shell.window", ["action": "close"])
            if reply.ok, reply.text == "hidden" { sender.orderOut(nil) }
        }
        return false
    }
}
