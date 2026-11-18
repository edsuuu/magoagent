// Uma janela com uma vista SwiftUI dentro, para as áreas.
import AppKit
import SwiftUI

@MainActor
public enum MagoWindow {
    /// Faz a janela de `content` e a põe na frente sem ativar o app (um roteiro de teste nunca rouba o foco).
    /// `borderless`: um painel sem barra que não ativa o app nem some quando ele perde o foco, mas aceita o teclado
    /// (a ilha, popups). `transparent`: sem fundo nem sombra (a vista desenha a forma). `frame` em pontos da tela, origem
    /// embaixo à esquerda. Fecha sozinha no `mago_ui_stop`.
    public static func make<Content: View>(
        title: String,
        frame: NSRect,
        borderless: Bool,
        transparent: Bool,
        level: NSWindow.Level,
        content: Content
    ) -> NSWindow {
        let window: NSWindow
        if borderless {
            let panel = KeyPanel(contentRect: frame, styleMask: [.borderless, .nonactivatingPanel], backing: .buffered, defer: false)
            panel.hidesOnDeactivate = false
            panel.becomesKeyOnlyIfNeeded = true
            window = panel
        } else {
            window = NSWindow(contentRect: frame, styleMask: [.titled, .closable, .miniaturizable, .resizable], backing: .buffered, defer: false)
        }
        window.isReleasedWhenClosed = false
        window.title = title
        window.level = level
        if transparent {
            window.isOpaque = false
            window.backgroundColor = .clear
            window.hasShadow = false
        }
        window.contentView = NSHostingView(rootView: content)
        // num roteiro de teste (MAGOAGENT_SCRIPT) a janela fica invisível e deixa o clique passar, para não aparecer por
        // cima do trabalho da pessoa; o `shot` a salva assim mesmo (MagoStore.shoot)
        if ProcessInfo.processInfo.environment["MAGOAGENT_SCRIPT"] != nil {
            window.alphaValue = 0
            window.ignoresMouseEvents = true
        }
        MagoStore.shared.windows.append(window)
        window.orderFrontRegardless()
        return window
    }
}

/// Um painel sem barra recusa o teclado por padrão; este aceita (um campo de texto na ilha).
private final class KeyPanel: NSPanel {
    override var canBecomeKey: Bool { true }
}
