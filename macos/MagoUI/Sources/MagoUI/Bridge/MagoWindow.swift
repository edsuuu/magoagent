// Uma janela (ou painel) com uma vista SwiftUI dentro, para as áreas.
import AppKit
import SwiftUI

@MainActor
public enum MagoWindow {
    /// Abre `content` numa janela nova, na frente sem ativar o app (um roteiro de teste nunca rouba o foco).
    /// `panel`: um NSPanel que não ativa o app nem some quando ele perde o foco. `borderless`: sem barra de título.
    /// `transparent`: sem fundo nem sombra (a vista desenha a forma). `frame` em pontos da tela, origem embaixo à esquerda.
    @discardableResult
    public static func open<Content: View>(
        _ content: Content,
        frame: NSRect,
        title: String = "",
        panel: Bool = false,
        borderless: Bool = false,
        transparent: Bool = false,
        level: NSWindow.Level = .normal
    ) -> NSWindow {
        var style: NSWindow.StyleMask = borderless ? [.borderless] : [.titled, .closable, .miniaturizable, .resizable]
        let window: NSWindow
        if panel {
            style.insert(.nonactivatingPanel)
            let made = NSPanel(contentRect: frame, styleMask: style, backing: .buffered, defer: false)
            made.hidesOnDeactivate = false
            made.becomesKeyOnlyIfNeeded = true
            window = made
        } else {
            window = NSWindow(contentRect: frame, styleMask: style, backing: .buffered, defer: false)
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
        MagoStore.shared.windows.append(window)
        window.orderFrontRegardless()
        return window
    }
}
