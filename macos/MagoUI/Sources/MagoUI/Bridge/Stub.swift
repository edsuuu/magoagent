// STUB: replaced by the foundation
import AppKit
import Observation
import SwiftUI

public struct MagoReply: Decodable, Sendable {
    public let id: UInt64
    public let ok: Bool
    public let text: String
}

@MainActor @Observable
public final class MagoStore {
    public static let shared = MagoStore()
    private var topics: [String: Data] = [:]
    public func decode<T: Decodable>(_ type: T.Type, topic: String) -> T? {
        guard let data = topics[topic] else { return nil }
        return try? JSONDecoder().decode(type, from: data)
    }
    public func act(_ action: String, _ args: [String: Any]? = nil) async -> MagoReply {
        MagoReply(id: 0, ok: false, text: "stub")
    }
    public func send(_ action: String, _ args: [String: Any]? = nil) {}
}

@MainActor
public enum MagoWindow {
    public static func make<Content: View>(title: String, frame: NSRect, borderless: Bool, transparent: Bool, level: NSWindow.Level, content: Content) -> NSWindow {
        NSWindow(contentRect: frame, styleMask: [.titled], backing: .buffered, defer: false)
    }
}

@MainActor
public enum MagoViews {
    public static var canvas: (() -> AnyView)?
    public static var chat: ((String) -> AnyView)?
    public static var chats: (() -> AnyView)?
    public static var terminal: ((String) -> AnyView)?
    public static var filetree: ((String) -> AnyView)?
    public static var settings: (() -> AnyView)?
}
