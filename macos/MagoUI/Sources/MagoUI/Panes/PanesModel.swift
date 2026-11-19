import Foundation
import Observation
import SwiftUI

// What Rust publishes as topic "panes" (src/app/ui/native/panes.rs): the panels on the window's right, Alterações and Nota rápida.

struct PanesState: Decodable {
    var open: String?
    var width: Double
    var count: Int
    var listRows: Int
    var look: PaneLook
    var note: String
    var changes: ChangesState?
}

/// The egui panel's colors.
struct PaneLook: Decodable {
    var bg: [Int]
    var text: [Int]
    var muted: [Int]
    var danger: [Int]
    var added: [Int]
    var selected: [Int]
    var field: [Int]
    var raised: [Int]
    var ink: [Int]
    var accent: [Int]
    var addedRow: [Int]
    var removedRow: [Int]
}

struct ChangesState: Decodable {
    var worktree: String?
    var worktrees: [WorktreeChoice]
    var search: String
    var found: String?
    var empty: String?
    var groups: [ChangeGroup]
    var selected: String?
    var diff: [DiffRow]
    var picked: [Int]?
    var comment: String?
    var review: ReviewState?
}

struct WorktreeChoice: Decodable, Identifiable {
    var path: String
    var label: String
    var on: Bool
    var tip: String
    var id: String { path }
}

struct ChangeGroup: Decodable {
    var title: String
    var files: [ChangedFile]
}

struct ChangedFile: Decodable, Identifiable {
    var tree: String
    var path: String
    var name: String
    var color: [Int]
    var icon: FTIconRef
    /// Added and removed, once counted.
    var lines: [Int]?
    var code: String?
    var id: String { path }
}

struct DiffRow: Decodable {
    var mark: String
    var old: Int?
    var new: Int?
    /// [text, [r, g, b, a]] stretches.
    var runs: [DiffRun]
}

struct DiffRun: Decodable {
    var text: String
    var color: [Int]

    init(from decoder: Decoder) throws {
        var pair = try decoder.unkeyedContainer()
        text = try pair.decode(String.self)
        color = try pair.decode([Int].self)
    }
}

struct ReviewState: Decodable {
    var heading: String
    var comments: [ReviewNote]
    var note: String
    var agents: [ReviewAgent]
    var agent: String?
    var canSend: Bool
    var canAsk: Bool
}

struct ReviewNote: Decodable {
    var place: String
    var first: String
    var note: String
}

struct ReviewAgent: Decodable, Identifiable {
    var id: String
    var name: String
    var label: String
    var cwd: String
}

/// The topic, decoded again only when its JSON changed (a diff can be long, and every topic wakes the store).
@MainActor @Observable final class PanesModel {
    static let shared = PanesModel()
    private(set) var state: PanesState?
    @ObservationIgnored private var data: Data?

    private init() { track() }

    private func track() {
        withObservationTracking {
            let next = MagoStore.shared.topics["panes"]
            if next != data {
                data = next
                state = MagoStore.shared.decode(PanesState.self, topic: "panes")
            }
        } onChange: { [weak self] in
            Task { @MainActor in self?.track() }
        }
    }

    /// How much of the canvas's right the open panel takes (the egui `pane_width`).
    var inset: CGFloat { state?.open == nil ? 0 : CGFloat(state?.width ?? 0) }
}

/// A `ui.panes.<verb>`: what the egui panel's click does; a refusal is said on screen.
@MainActor func paneVerb(_ verb: String, _ args: [String: Any] = [:]) {
    appAction("ui.panes.\(verb)", args)
}

/// The app's interface text (the Assistant) and its code text (egui's monospace, Hack).
extension Font {
    static func code(_ size: CGFloat) -> Font { .custom("Hack", size: size) }
}

/// A field whose text lives in Rust: what is typed goes at once, and the topic's text comes in only when it is not the echo
/// of what went (a frame later), so typing never jumps back.
struct SyncedText: ViewModifier {
    let remote: String
    @Binding var text: String
    let send: (String) -> Void
    @State private var sent: String?

    func body(content: Content) -> some View {
        content
            .onAppear {
                text = remote
                sent = remote
            }
            .onChange(of: text) { _, now in
                if now != sent {
                    sent = now
                    send(now)
                }
            }
            .onChange(of: remote) { _, now in
                if now != sent {
                    sent = now
                    text = now
                }
            }
    }
}

extension View {
    func synced(_ remote: String, _ text: Binding<String>, send: @escaping (String) -> Void) -> some View {
        modifier(SyncedText(remote: remote, text: text, send: send))
    }
}
