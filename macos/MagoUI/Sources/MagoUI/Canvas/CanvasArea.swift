import SwiftUI

/// The canvas area: registers `MagoViews.canvas`. Rust's side is src/app/canvas/native.rs (topics "scene", "scene.<id>",
/// "canvas"; verbs `ui.canvas.view|pointer|tool|key|text|text_done|image|font`).
@MainActor public enum CanvasArea {
    public static func start() {
        MagoViews.canvas = { AnyView(CanvasView()) }
    }

    /// The live view of a window on the canvas, when its area registered one (a browser's page is native, in `MagoViews.pages`).
    static func view(kind: String, id: String) -> AnyView? {
        switch kind {
        case "terminal": MagoViews.terminal?(id)
        case "chat": MagoViews.chat?(id)
        case "filetree": MagoViews.filetree?(id)
        case "portal": MagoViews.portal?(id)
        default: nil
        }
    }
}
