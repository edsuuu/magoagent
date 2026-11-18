// STUB: replaced by the foundation
import AppKit
import SwiftUI

public struct MagoReply: Decodable, Sendable {
    public let id: UInt64
    public let ok: Bool
    public let text: String
}

@MainActor @Observable public final class MagoStore {
    public static let shared = MagoStore()
    var topics: [String: Data] = [:]

    public func decode<T: Decodable>(_ type: T.Type, topic: String) -> T? {
        topics[topic].flatMap { try? JSONDecoder().decode(type, from: $0) }
    }

    public func act(_ action: String, _ args: [String: Any]? = nil) async -> MagoReply {
        MagoReply(id: 0, ok: false, text: "stub")
    }

    public func send(_ action: String, _ args: [String: Any]? = nil) {}
}

@MainActor public enum MagoWindow {
    public static func make<Content: View>(title: String, frame: NSRect, borderless: Bool, transparent: Bool, level: NSWindow.Level, content: Content) -> NSWindow {
        let window = NSWindow(contentRect: frame, styleMask: borderless ? [.borderless] : [.titled, .closable, .resizable], backing: .buffered, defer: false)
        window.title = title
        window.level = level
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
