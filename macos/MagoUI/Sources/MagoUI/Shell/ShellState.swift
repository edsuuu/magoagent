import Foundation
import Observation
import SwiftUI

// What Rust publishes as topic "shell" (src/app/ui/native/model.rs). Keys are camelCase, read as they come.

struct ShellState: Decodable, Equatable {
    var title: String
    var dev: Bool
    var dark: Bool
    var theme: String
    var screen: String
    var keysFree: Bool
    var space: SpaceRef?
    var floor: String
    var chats: ChatCounts
    var sidebar: SidebarState
    var tools: ToolGroups
    var tool: String
    var drawing: Bool
    var drawOpen: Bool
    var hint: String?
    var properties: StyleProperties?
    var canUndo: Bool
    var canRedo: Bool
    var zoom: Int
    var minimap: Bool
    var grid: Bool
    var floors: [FloorRow]
    var floorsOverview: Bool
    var pane: PaneState
    var dialogs: Dialogs
    var toast: Toast?
    var shortcuts: [MenuKey]
    var settingsOpen: Bool
    var strokePicks: [[Int]]
    var backgroundPicks: [[Int]]
    var spaceColors: [[Int]]
}

struct SpaceRef: Decodable, Equatable {
    var id: String
    var name: String
    var color: [Int]
}

struct ChatCounts: Decodable, Equatable {
    var running: Int
    var waiting: Int
}

struct SidebarState: Decodable, Equatable {
    var hidden: Bool
    var waiting: Int
    var agentsClosed: Bool
    var nextKeys: String
    var agents: [AgentRow]
    var spaces: [SpaceRow]
    var groups: [GroupRow]
}

struct AgentRow: Decodable, Equatable, Identifiable {
    var id: String
    var name: String
    var state: String
    var label: String
    var note: String?
    var mission: String?
    var chat: Bool
    var needsYou: Bool
    var program: String
}

struct SpaceRow: Decodable, Equatable, Identifiable {
    var id: String
    var name: String
    var color: [Int]
    var root: String
    var pinned: Bool
    var group: String
    var running: Int
    var needs: Int
    var current: Bool
    var floors: Int
}

struct GroupRow: Decodable, Equatable, Identifiable {
    var id: String
    var name: String
    var open: Bool
}

struct ToolRow: Decodable, Equatable, Identifiable {
    var id: String
    var label: String
    var badge: String
}

struct ToolGroups: Decodable, Equatable {
    var main: [ToolRow]
    var drawing: [ToolRow]
    var extra: [ToolRow]
}

struct StyleProperties: Decodable, Equatable {
    var showStroke: Bool
    var showFill: Bool
    var showFillStyle: Bool
    var showStrokeOptions: Bool
    var showArrow: Bool
    var showEdges: Bool
    var showFont: Bool
    var fillTool: Bool
    var frameName: String?
    var stroke: [Int]
    var fill: [Int]?
    var fillStyle: String
    var strokeWidth: Double
    var strokeStyle: String
    var roughness: Int
    var round: Bool
    var opacity: Int
    var fontSize: Double
    var startHead: String
    var endHead: String
    var selection: Int
    var actions: [PropertyAction]
}

struct PropertyAction: Decodable, Equatable, Identifiable {
    var id: String
    var label: String
    var keys: String
}

struct FloorRow: Decodable, Equatable, Identifiable {
    var id: String
    var name: String
    var color: [Int]
    var current: Bool
}

struct PaneState: Decodable, Equatable {
    var open: String?
    var buttons: [PaneButton]
}

struct PaneButton: Decodable, Equatable, Identifiable {
    var id: String
    var name: String
}

struct Dialogs: Decodable, Equatable {
    var terminal: TerminalDialog?
    var export: ExportDialog?
    var wipe: WipeDialog?
    var palette: PaletteState?
    var welcome: WelcomeDialog?
    var tour: TourDialog?
    var changelog: ChangelogDialog?
}

struct TerminalDialog: Decodable, Equatable {
    var editing: Bool
    var chat: Bool
    var name: String
    var command: String
    var cwd: String
    var maestro: Bool
    var startInChat: Bool
    var presets: [PresetRow]
}

struct PresetRow: Decodable, Equatable {
    var name: String
    var command: String
    var icon: String
    var color: [Int]?
    var selected: Bool
}

struct ExportDialog: Decodable, Equatable {
    var svg: Bool
    var scale: Double
    var background: Bool
}

struct WipeDialog: Decodable, Equatable {
    var keychain: Bool
    var folders: [String]
}

struct PaletteState: Decodable, Equatable {
    var query: String
    var index: Int
    var nodes: Bool
    var rows: [PaletteRow]
}

struct PaletteRow: Decodable, Equatable {
    var group: String
    var label: String
    var keys: String
}

struct WelcomeDialog: Decodable, Equatable {
    var noSpace: Bool
    var error: String
}

struct TourDialog: Decodable, Equatable {
    var step: Int
    var steps: Int
}

struct ChangelogDialog: Decodable, Equatable {
    var version: String
    var notes: String
    var installed: Bool
    var confirming: Bool
}

struct Toast: Decodable, Equatable {
    var id: Double
    var text: String
    var seconds: Double
}

struct MenuKey: Decodable, Equatable {
    var id: String
    var key: String
    var command: Bool
    var option: Bool
    var shift: Bool
    var text: String
}

/// The snapshot, kept up to date: the store tells when the topic changed (it is observable), and the next read is tracked again.
@MainActor @Observable final class ShellModel {
    static let shared = ShellModel()
    private(set) var state: ShellState?
    /// A message of Swift's own (what a failed verb answered), shown as a toast.
    var notice: Toast?

    private init() { track() }

    private func track() {
        withObservationTracking {
            let next = MagoStore.shared.decode(ShellState.self, topic: "shell")
            if next != state { state = next }
        } onChange: { [weak self] in
            Task { @MainActor in self?.track() }
        }
    }

    func say(_ text: String) {
        notice = Toast(id: Date().timeIntervalSince1970, text: text, seconds: 4)
    }
}

/// A `ui.shell.<verb>`: Rust calls what the egui chrome called.
@MainActor func shellVerb(_ verb: String, _ args: [String: Any] = [:]) {
    perform("ui.shell.\(verb)", args)
}

/// One of the app's own actions (the MCP's `act`: workspaces, floors, panel, sidebar...).
@MainActor func appAction(_ action: String, _ args: [String: Any] = [:]) {
    perform(action, args)
}

@MainActor private func perform(_ action: String, _ args: [String: Any]) {
    Task { @MainActor in
        let reply = await MagoStore.shared.act(action, args)
        // what went wrong comes back as the text of the answer (a name taken, the last space): said on screen
        if !reply.ok, !reply.text.isEmpty { ShellModel.shared.say(reply.text) }
    }
}

extension Color {
    init(rgba: [Int]) {
        let channel = { (i: Int) in i < rgba.count ? Double(rgba[i]) / 255 : 0 }
        self.init(.sRGB, red: channel(0), green: channel(1), blue: channel(2), opacity: rgba.count > 3 ? channel(3) : 1)
    }

    /// The reference app's accent.
    static let accent007aff = Color(red: 0, green: 122.0 / 255, blue: 1)
    /// An agent that needs the person.
    static let amber = Color(red: 0xf0 / 255.0, green: 0xa0 / 255.0, blue: 0x20 / 255.0)
}
