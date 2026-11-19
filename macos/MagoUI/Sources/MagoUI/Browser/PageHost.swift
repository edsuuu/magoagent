import AppKit
import SwiftUI

/// What lies over the whole canvas for the browsers: the pages' view, and Design's Enviar while it waits.
struct PagesLayer: View {
    var body: some View {
        ZStack {
            PageHost()
            if let ask = PortalsTopic.current?.confirm {
                DesignConfirm(ask: ask)
            }
        }
    }
}

/// The view Rust's pages go in, as big as the canvas.
struct PageHost: NSViewRepresentable {
    func makeNSView(context: Context) -> PageHostView {
        PageHostView()
    }

    func updateNSView(_ view: PageHostView, context: Context) {}
}

/// Where the pages live (`mago_ui_page_host`): its origin top left like the canvas's, so Rust's points land as they do over
/// egui's window, and see-through to the pointer wherever no page is. A new one (the canvas back from the chats) takes the
/// pages of the last, which stays alive until then: a page always has a view above it.
final class PageHostView: NSView {
    /// The one the pages are in.
    static var current: PageHostView?

    /// The one Rust makes and places pages in: the current one, only while it is in a window.
    static var live: PageHostView? {
        current?.window == nil ? nil : current
    }

    override var isFlipped: Bool { true }

    override func hitTest(_ point: NSPoint) -> NSView? {
        let hit = super.hitTest(point)
        return hit === self ? nil : hit
    }

    override func viewDidMoveToWindow() {
        super.viewDidMoveToWindow()
        guard window != nil, Self.current !== self else { return }
        for page in Self.current?.subviews ?? [] {
            addSubview(page)
        }
        Self.current = self
    }

    /// A page has the keyboard in `window`: the keys are the page's, not the tools'.
    static func hasKeyboard(_ window: NSWindow?) -> Bool {
        guard let host = live, let view = window?.firstResponder as? NSView else { return false }
        return view.isDescendant(of: host)
    }
}
