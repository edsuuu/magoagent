// As áreas ligadas por MAGOAGENT_SWIFTUI: o Rust publica a lista no tópico "areas" e cada uma começa aqui.
import SwiftUI

@MainActor
enum Areas {
    /// A ordem de começar: as que registram vistas em `MagoViews` antes da moldura (shell), que as embute.
    static let order = ["chat", "terminal", "filetree", "settings", "canvas", "island", "shell", "probe"]

    static func start(_ names: [String]) {
        for name in order where names.contains(name) {
            switch name {
            // ÁREAS: um caso por área, ex.: case "island": IslandArea.start()
            case "chat": ChatArea.start()
            case "canvas": CanvasArea.start()
            case "island": IslandArea.start()
            case "shell": ShellArea.start()
            case "filetree": FileTreeArea.start()
            case "terminal": TerminalArea.start()
            case "probe": ProbeArea.start()
            default: break
            }
        }
    }
}

/// As vistas que as áreas registram no `start()` para outras embutirem (a moldura, o canvas) sem depender delas ao
/// compilar. nil enquanto a área não está ligada.
@MainActor
public enum MagoViews {
    public static var canvas: (() -> AnyView)?
    /// o id do chat
    public static var chat: ((String) -> AnyView)?
    public static var chats: (() -> AnyView)?
    /// o id do elemento terminal
    public static var terminal: ((String) -> AnyView)?
    /// o id do elemento árvore de arquivos
    public static var filetree: ((String) -> AnyView)?
    public static var settings: (() -> AnyView)?
}
