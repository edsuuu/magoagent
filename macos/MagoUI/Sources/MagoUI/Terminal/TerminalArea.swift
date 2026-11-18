import SwiftUI

/// The terminal's area of the SwiftUI app: `MagoViews.terminal` draws a terminal element by its id.
@MainActor
public enum TerminalArea {
    public static func start() {
        MagoViews.terminal = { id in AnyView(TerminalView(id: id)) }
    }
}
