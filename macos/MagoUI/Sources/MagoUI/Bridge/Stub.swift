// STUB: replaced by the foundation
// The contract the areas are built against: the store of the snapshots Rust publishes, the actions sent back, the
// windows and the views other areas register.

import AppKit
import Observation
import SwiftUI

@MainActor @Observable public final class MagoStore {
    public static let shared = MagoStore()
    /// The last JSON of each topic.
    var topics: [String: Data] = [:]

    public func decode<T: Decodable>(_ type: T.Type, topic: String) -> T? {
        guard let data = topics[topic] else { return nil }
        let decoder = JSONDecoder()
        decoder.keyDecodingStrategy = .convertFromSnakeCase
        return try? decoder.decode(T.self, from: data)
    }

    public func act(_ action: String, _ args: [String: Any]? = nil) async -> MagoReply {
        MagoReply(id: 0, ok: false, text: "stub")
    }

    public func send(_ action: String, _ args: [String: Any]? = nil) {}
}

public struct MagoReply: Decodable, Sendable {
    public let id: UInt64
    public let ok: Bool
    public let text: String
}

@MainActor public enum MagoWindow {
    public static func make<Content: View>(
        title: String, frame: NSRect, borderless: Bool, transparent: Bool, level: NSWindow.Level, content: Content
    ) -> NSWindow {
        let window = NSPanel(
            contentRect: frame, styleMask: borderless ? [.borderless, .nonactivatingPanel] : [.titled, .closable, .resizable],
            backing: .buffered, defer: false)
        window.title = title
        window.level = level
        if transparent {
            window.isOpaque = false
            window.backgroundColor = .clear
        }
        window.contentView = NSHostingView(rootView: content)
        return window
    }
}

@MainActor public enum MagoViews {
    public static var canvas: (() -> AnyView)?
    public static var chat: ((String) -> AnyView)?
    public static var chats: (() -> AnyView)?
    public static var terminal: ((String) -> AnyView)?
    public static var filetree: ((String) -> AnyView)?
    public static var settings: (() -> AnyView)?
}
