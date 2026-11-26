// The Contexto panel beside the open chat (chat/context_view.rs), from the topic "chat_context" (null while closed): the
// header with Atualizar and Fechar, the share in use, the stacked bar with its legend, the band with Compactar, "Compactar
// automaticamente em", the MCP tools, the connectors and, at the foot, the memory files. Esc with the pointer over it
// closes it, as in egui.
import AppKit
import SwiftUI

struct ChatPanel: Decodable, Equatable {
    let chat: String
    let loading: Bool
    let note: String
    let report: PanelReport?
    let connected: String
    let connectors: [PanelConnector]
    let connectorsNote: String
    let more: String

    enum CodingKeys: String, CodingKey {
        case chat, loading, note, report, connected, connectors, more
        case connectorsNote = "connectors_note"
    }

    /// The panel open now, if any.
    @MainActor static var current: ChatPanel? { MagoStore.shared.decode(ChatPanel?.self, topic: "chat_context") ?? nil }

    /// The ring and ×: the MCP's `chat_context`, as egui's header button and × set `prefs.chat_panel`.
    @MainActor static func send(_ chat: String, _ state: String) { MagoStore.shared.send("chat_context", ["chat": chat, "state": state]) }
}

struct PanelReport: Decodable, Equatable {
    let percent, used, until, autocompact, memory, memoryTokens, memoryTip: String
    /// [share of the window, [r, g, b, a]]
    let bar: [PanelSegment]
    let legend: [PanelLegend]
    let choices: [String]
    /// [server · name, tokens]
    let tools: [[String]]

    enum CodingKeys: String, CodingKey {
        case percent, used, until, autocompact, memory, bar, legend, choices, tools
        case memoryTokens = "memory_tokens"
        case memoryTip = "memory_tip"
    }
}

struct PanelSegment: Decodable, Equatable {
    let share: Double
    let color: [Int]

    init(from decoder: Decoder) throws {
        var pair = try decoder.unkeyedContainer()
        share = try pair.decode(Double.self)
        color = try pair.decode([Int].self)
    }
}

struct PanelLegend: Decodable, Equatable {
    let name, tokens, tip: String
    let color: [Int]
    let counts: Bool
}

struct PanelConnector: Decodable, Equatable, Identifiable {
    let name, letter, note, status: String
    let ink: [Int]
    let tools: [String]
    var id: String { name }
}

struct ContextPanel: View {
    let panel: ChatPanel
    @Environment(\.chatLook) private var look
    @State private var esc = EscOver()

    var body: some View {
        VStack(spacing: 0) {
            header
            ScrollView {
                VStack(alignment: .leading, spacing: 0) {
                    Summary(panel: panel)
                        .padding(.horizontal, 18)
                        .padding(.vertical, 16)
                    Rectangle().fill(look.hover).frame(height: 1)
                    Connectors(panel: panel)
                }
            }
            if let report = panel.report, !report.memory.isEmpty { footer(report) }
        }
        // the ground: the chat's 40% of the way to the panel's grey, a hairline at the left
        .background { ZStack { look.bg; look.panel.opacity(0.4) } }
        .overlay(alignment: .leading) { Rectangle().fill(look.hover).frame(width: 1) }
        .onHover { esc.over = $0 }
        .onAppear { esc.start { ChatPanel.send(panel.chat, "close") } }
        .onDisappear { esc.stop() }
    }

    private var header: some View {
        HStack(spacing: 4) {
            Thick(text: "Contexto", size: 14, color: look.strong)
            Spacer(minLength: 8)
            Button { ChatPanel.send(panel.chat, "refresh") } label: {
                TimelineView(.animation(paused: !panel.loading)) { timeline in
                    Image(systemName: "arrow.clockwise")
                        .font(.system(size: 13, weight: .semibold))
                        .foregroundStyle(panel.loading ? look.text : look.soft)
                        .rotationEffect(.radians(panel.loading ? timeline.date.timeIntervalSinceReferenceDate * 4 : 0))
                }
                .frame(width: 28, height: 28)
                .contentShape(Rectangle())
            }
            .buttonStyle(HoverGround(radius: 7, color: look.hover))
            .help(panel.loading ? "Atualizando…" : "Atualizar")
            Button { ChatPanel.send(panel.chat, "close") } label: {
                Image(systemName: "xmark").font(.system(size: 12, weight: .semibold)).foregroundStyle(look.soft).frame(width: 28, height: 28).contentShape(Rectangle())
            }
            .buttonStyle(HoverGround(radius: 7, color: look.hover))
            .help("Fechar (Esc)")
        }
        .padding(.leading, 18)
        .padding(.trailing, 12)
        .frame(height: 56)
        .overlay(alignment: .bottom) { Rectangle().fill(look.hover).frame(height: 1) }
    }

    /// The memory files: the first one (and how many more) and their tokens; each one in the tip.
    private func footer(_ report: PanelReport) -> some View {
        HStack(spacing: 0) {
            Image(systemName: "doc").font(.system(size: 12)).foregroundStyle(look.muted).frame(width: 13)
            Text(report.memory).font(.app(13)).foregroundStyle(look.muted).lineLimit(1).truncationMode(.tail).padding(.leading, 5).help(report.memoryTip)
            Spacer(minLength: 10)
            Text(report.memoryTokens).font(.app(13)).foregroundStyle(look.muted)
        }
        .padding(.leading, 18)
        .padding(.trailing, 18)
        .frame(height: 44)
        .overlay(alignment: .top) { Rectangle().fill(look.hover).frame(height: 1) }
    }
}

/// Esc with the pointer over the panel closes it before the chat sees it (context_ui.rs `context_esc`); a menu open
/// tracks its own keys, so its Esc never comes here.
@MainActor private final class EscOver {
    var over = false
    private var monitor: Any?

    func start(_ close: @escaping @MainActor () -> Void) {
        guard monitor == nil else { return }
        // events come on the main thread; the monitor's closure is not typed as such
        monitor = NSEvent.addLocalMonitorForEvents(matching: .keyDown) { [weak self] event in
            let taken = MainActor.assumeIsolated {
                guard event.keyCode == 53, event.modifierFlags.intersection([.command, .control, .option, .shift]).isEmpty, self?.over == true else { return false }
                close()
                return true
            }
            return taken ? nil : event
        }
    }

    func stop() {
        if let monitor { NSEvent.removeMonitor(monitor) }
        monitor = nil
    }
}

/// The share in use, the bar and its legend, the band of when it compacts, the value picked and the MCP tools.
private struct Summary: View {
    let panel: ChatPanel
    private let store = MagoStore.shared
    @Environment(\.chatLook) private var look
    @State private var toolsOpen = false
    @State private var compactHovered = false

    var body: some View {
        if let report = panel.report {
            VStack(alignment: .leading, spacing: 12) {
                // 75%  301,8k de 400k tokens, on one baseline
                HStack(alignment: .lastTextBaseline, spacing: 8) {
                    Thick(text: report.percent, size: 26, color: look.strong)
                    Text(report.used).font(.app(13)).foregroundStyle(look.muted)
                }
                .frame(maxWidth: .infinity, minHeight: 32, alignment: .bottomLeading)
                Canvas { context, size in
                    var x: CGFloat = 0
                    for segment in report.bar {
                        let width = size.width * segment.share
                        guard width >= 0.5 else { continue }
                        context.fill(Path(CGRect(x: x, y: 0, width: width, height: size.height)), with: .color(Color(rgba: segment.color)))
                        x += width
                    }
                }
                .frame(height: 8)
                .background(look.surface)
                .clipShape(RoundedRectangle(cornerRadius: 4))
                VStack(spacing: 7) {
                    ForEach(report.legend, id: \.name) { row in legend(row) }
                }
                band(report)
                autocompact(report)
                if !report.tools.isEmpty { tools(report) }
            }
        } else {
            Text(panel.note).font(.app(13)).foregroundStyle(look.muted).frame(maxWidth: .infinity, alignment: .leading)
        }
    }

    private func legend(_ row: PanelLegend) -> some View {
        HStack(spacing: 0) {
            Group {
                if row.counts {
                    RoundedRectangle(cornerRadius: 2).fill(Color(rgba: row.color))
                } else {
                    RoundedRectangle(cornerRadius: 2).strokeBorder(look.raised)
                }
            }
            .frame(width: 8, height: 8)
            Text(row.name).font(.app(13)).foregroundStyle(look.soft).lineLimit(1).padding(.leading, 10)
            Spacer(minLength: 8)
            Text(row.tokens).font(.app(13)).foregroundStyle(look.muted)
        }
        .frame(height: 14)
        .contentShape(Rectangle())
        .help(row.tip)
    }

    /// When it compacts by itself, and Compactar now (/compact).
    private func band(_ report: PanelReport) -> some View {
        HStack(spacing: 0) {
            Clock().stroke(ChatLook.amber, style: StrokeStyle(lineWidth: 1.6, lineCap: .round)).frame(width: 14, height: 14)
            Text("Compacta sozinho em ").font(.app(13)).foregroundStyle(look.soft).padding(.leading, 8)
            Thick(text: report.until, size: 12, color: look.text)
            Spacer(minLength: 8)
            Button { store.send("ui.chat.context", ["chat": panel.chat, "action": "compact"]) } label: {
                Thick(text: "Compactar", size: 12, color: look.bg)
                    .padding(.horizontal, 10)
                    .frame(height: 26)
                    .background(RoundedRectangle(cornerRadius: 7).fill(compactHovered ? look.strong : look.text))
                    .contentShape(Rectangle())
            }
            .buttonStyle(.plain)
            .onHover { compactHovered = $0 }
            .help("Resume a conversa para liberar contexto (/compact)")
        }
        .frame(height: 26)
        .padding(.horizontal, 10)
        .padding(.vertical, 9)
        .background(RoundedRectangle(cornerRadius: 9).fill(look.hover))
    }

    /// Compactar automaticamente em [Automático ▾]
    private func autocompact(_ report: PanelReport) -> some View {
        HStack(spacing: 0) {
            Text("Compactar automaticamente em").font(.app(13)).foregroundStyle(look.muted).lineLimit(1)
            Spacer(minLength: 8)
            Menu {
                Button(checked("Automático", report.autocompact.isEmpty)) { pick("") }
                ForEach(report.choices, id: \.self) { value in
                    Button(checked(value, report.autocompact == value)) { pick(value) }
                }
            } label: {
                HStack(spacing: 0) {
                    Text(report.autocompact.isEmpty ? "Automático" : report.autocompact).font(.app(13)).foregroundStyle(look.text)
                    Image(systemName: "chevron.down").font(.system(size: 9, weight: .bold)).foregroundStyle(look.text).frame(width: 22)
                }
                .padding(.leading, 8)
                .frame(height: 26)
                .overlay(RoundedRectangle(cornerRadius: 7).strokeBorder(look.surface))
                .contentShape(Rectangle())
            }
            .menuStyle(.button)
            .buttonStyle(HoverGround(radius: 7, color: look.hover))
            .menuIndicator(.hidden)
            .fixedSize()
            .help("Um valor menor consome menos do seu limite. O Claude resume as mensagens mais antigas mais cedo.")
        }
        .frame(height: 26)
    }

    /// The MCP tools one by one with their tokens, folded under "Ferramentas MCP  N".
    private func tools(_ report: PanelReport) -> some View {
        VStack(alignment: .leading, spacing: 4) {
            Button { withAnimation(.smooth(duration: 0.2)) { toolsOpen.toggle() } } label: {
                HStack(spacing: 6) {
                    Image(systemName: "chevron.right").font(.system(size: 9, weight: .bold)).foregroundStyle(look.soft).rotationEffect(.degrees(toolsOpen ? 90 : 0))
                    Text("Ferramentas MCP  \(report.tools.count)").font(.app(13)).foregroundStyle(look.soft)
                }
                .frame(height: 18)
                .contentShape(Rectangle())
            }
            .buttonStyle(.plain)
            if toolsOpen {
                ForEach(Array(report.tools.enumerated()), id: \.offset) { _, tool in
                    HStack(spacing: 8) {
                        Text(tool.first ?? "").font(.app(12)).foregroundStyle(look.soft).lineLimit(1)
                        Spacer(minLength: 0)
                        Text(tool.last ?? "").font(.app(12)).foregroundStyle(look.muted)
                    }
                    .frame(height: 16)
                }
            }
        }
    }

    private func pick(_ value: String) { store.send("ui.chat.context", ["chat": panel.chat, "action": "autocompact", "value": value]) }

    private func checked(_ name: String, _ on: Bool) -> String { on ? "✓ \(name)" : name }
}

/// The amber clock of the band: a ring with its two hands.
private struct Clock: Shape {
    func path(in rect: CGRect) -> Path {
        let center = CGPoint(x: rect.midX, y: rect.midY)
        var path = Path(ellipseIn: CGRect(x: center.x - 6, y: center.y - 6, width: 12, height: 12))
        path.move(to: CGPoint(x: center.x, y: center.y - 3.5))
        path.addLine(to: center)
        path.addLine(to: CGPoint(x: center.x + 2.5, y: center.y + 1.7))
        return path
    }
}

/// CONECTORES with how many are on; the ones that need attention first, past 5 of more than 6 folded in "+ N".
private struct Connectors: View {
    let panel: ChatPanel
    @Environment(\.chatLook) private var look
    @State private var all = false
    @State private var moreHovered = false

    var body: some View {
        VStack(alignment: .leading, spacing: 0) {
            HStack {
                Text("CONECTORES").font(.app(12)).tracking(0.66).foregroundStyle(look.faint)
                Spacer()
                Text(panel.connected).font(.app(12)).foregroundStyle(look.faint)
            }
            .padding(.horizontal, 18)
            .frame(height: 24)
            .padding(.top, 14)
            VStack(alignment: .leading, spacing: 2) {
                if panel.connectors.isEmpty {
                    Text(panel.connectorsNote).font(.app(13)).foregroundStyle(look.muted).padding(.horizontal, 10).padding(.vertical, 4)
                } else {
                    let shown = all || panel.more.isEmpty ? panel.connectors : Array(panel.connectors.prefix(5))
                    ForEach(shown) { connector in ConnectorRow(chat: panel.chat, connector: connector) }
                    if !panel.more.isEmpty {
                        Button { all.toggle() } label: {
                            Text(all ? "Mostrar menos" : panel.more).font(.app(13)).foregroundStyle(moreHovered ? look.text : look.muted)
                                .padding(.leading, 10)
                                .frame(maxWidth: .infinity, minHeight: 26, alignment: .leading)
                                .contentShape(Rectangle())
                        }
                        .buttonStyle(.plain)
                        .onHover { moreHovered = $0 }
                    }
                }
            }
            .padding(.horizontal, 8)
            .padding(.bottom, 12)
        }
    }
}

/// A connector: its letter in its colour, the name, its tools or its state and the green dot; Tentar on one that
/// failed. A click shows its tools.
private struct ConnectorRow: View {
    let chat: String
    let connector: PanelConnector
    @Environment(\.chatLook) private var look
    @State private var open = false
    @State private var hovered = false

    var body: some View {
        let ink = Color(rgba: connector.ink)
        VStack(alignment: .leading, spacing: 0) {
            HStack(spacing: 0) {
                RoundedRectangle(cornerRadius: 7)
                    .fill(look.bg)
                    .overlay(RoundedRectangle(cornerRadius: 7).fill(ink.opacity(look.dark ? 0.14 : 0.18)))
                    .overlay { Thick(text: connector.letter, size: 12, color: ink) }
                    .frame(width: 26, height: 26)
                    .padding(.leading, 10)
                VStack(alignment: .leading, spacing: 0) {
                    Text(connector.name).font(.app(14)).foregroundStyle(look.text).lineLimit(1).frame(height: 17)
                    Text(connector.note).font(.app(12)).foregroundStyle(noteColor).lineLimit(1).frame(height: 15)
                }
                .padding(.leading, 10)
                Spacer(minLength: 8)
                switch connector.status {
                case "failed":
                    TryButton(chat: chat).padding(.trailing, 10)
                case "connected":
                    Circle().fill(ChatLook.ready).frame(width: 7, height: 7).padding(.trailing, 10.5)
                default:
                    EmptyView()
                }
            }
            .frame(height: 40)
            .background(RoundedRectangle(cornerRadius: 9).fill(hovered || open ? look.hover : .clear))
            .contentShape(Rectangle())
            .onHover { hovered = $0 }
            .onTapGesture { if !connector.tools.isEmpty { withAnimation(.smooth(duration: 0.2)) { open.toggle() } } }
            .help(connector.status == "needs_auth" ? "Entre pelo /mcp numa sessão do Claude Code" : "")
            if open {
                VStack(alignment: .leading, spacing: 3) {
                    ForEach(connector.tools, id: \.self) { tool in Text(tool).font(.app(12)).foregroundStyle(look.soft) }
                }
                .padding(.leading, 46)
                .padding(.trailing, 10)
                .padding(.top, 2)
                .padding(.bottom, 6)
            }
        }
    }

    private var noteColor: Color {
        switch connector.status {
        case "needs_auth": ChatLook.amber
        case "failed": ChatLook.red
        default: look.muted
        }
    }
}

/// Tentar: the `claude mcp list` tests the connectors again.
private struct TryButton: View {
    let chat: String
    @Environment(\.chatLook) private var look
    @State private var hovered = false

    var body: some View {
        Button { MagoStore.shared.send("ui.chat.context", ["chat": chat, "action": "retry"]) } label: {
            Text("Tentar").font(.app(12)).foregroundStyle(look.text)
                .padding(.horizontal, 9)
                .frame(height: 24)
                .background(RoundedRectangle(cornerRadius: 6).fill(hovered ? look.raised : .clear))
                .overlay(RoundedRectangle(cornerRadius: 6).strokeBorder(look.raised))
                .contentShape(Rectangle())
        }
        .buttonStyle(.plain)
        .onHover { hovered = $0 }
        .help("Testa a conexão de novo")
    }
}
