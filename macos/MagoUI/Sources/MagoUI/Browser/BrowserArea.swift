import SwiftUI

/// The canvas's browsers, started with the canvas. Rust's side is src/app/browser/native.rs: the pages are WebKit's own
/// views, made and placed by Rust inside `PageHostView` (`mago_ui_page_host`); SwiftUI draws each card's body and bar
/// (topic "portals", verbs `ui.browser.*`) and Design's Enviar.
@MainActor public enum BrowserArea {
    public static func start() {
        MagoViews.portal = { id in AnyView(PortalBody(id: id)) }
        MagoViews.pages = { AnyView(PagesLayer()) }
    }
}

/// Topic "portals" (`PortalsTopic` in src/app/browser/native.rs).
struct PortalsTopic: Decodable {
    let cards: [String: PortalCard]
    let confirm: DesignAsk?

    @MainActor static var current: PortalsTopic? { MagoStore.shared.decode(PortalsTopic.self, topic: "portals") }
}

struct PortalCard: Decodable, Equatable {
    /// The address whole (where an edit starts) and as the bar shows it.
    let url: String
    let address: String
    let loading: Bool
    /// The live bar shows (not hidden, big enough on screen, nothing over the canvas).
    let bar: Bool
    let hidden: Bool
    let unloaded: Bool
    let design: Bool
}

struct DesignAsk: Decodable, Equatable {
    let name: String
    let prompt: String
    let chat: Bool
}

@MainActor func browserVerb(_ verb: String, _ args: [String: Any] = [:]) {
    MagoStore.shared.send("ui.browser.\(verb)", args)
}
