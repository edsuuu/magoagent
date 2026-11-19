import SwiftUI

/// The sidebar's tones (`ui/sidebar.rs::tones`).
enum Tones {
    static let ground = Color.themed(dark: 0x2c2c2e, light: 0xe9e9eb)
    static let hover = Color.themed(dark: 0xffffff, light: 0x000000, darkAlpha: 0x0f / 255, lightAlpha: 0x0d / 255)
    static let active = Color.themed(dark: 0xffffff, light: 0x000000, darkAlpha: 0x1c / 255, lightAlpha: 0x14 / 255)
    static let line = Color.themed(dark: 0xffffff, light: 0x000000, darkAlpha: 0x1a / 255, lightAlpha: 0x1a / 255)
    static let text = Color.themed(dark: 0xf5f5f7, light: 0x1d1d1f)
    static let muted = Color.themed(dark: 0xebebf5, light: 0x3c3c43, darkAlpha: 0x99 / 255, lightAlpha: 0x99 / 255)
}

/// The chevron of a foldable heading: down while open, right while folded (`attention.rs::chevron`).
struct Chevron: View {
    let open: Bool
    var body: some View {
        Canvas { context, size in
            let (x, y) = (size.width / 2, size.height / 2)
            let points = open
                ? [CGPoint(x: x - 3.5, y: y - 1.5), CGPoint(x: x, y: y + 2), CGPoint(x: x + 3.5, y: y - 1.5)]
                : [CGPoint(x: x - 1.5, y: y - 3.5), CGPoint(x: x + 2, y: y), CGPoint(x: x - 1.5, y: y + 3.5)]
            context.stroke(Path { $0.addLines(points) }, with: .foreground, style: StrokeStyle(lineWidth: 1.3, lineCap: .round, lineJoin: .round))
        }
        .frame(width: 10, height: 10).foregroundStyle(Tones.muted)
    }
}

/// A sidebar row: the hover's wash, or the current one's.
struct RowStyle: ButtonStyle {
    var current = false
    var radius: CGFloat = 6
    func makeBody(configuration: Configuration) -> some View {
        RowLabel(configuration: configuration, current: current, radius: radius)
    }
}

private struct RowLabel: View {
    let configuration: ButtonStyleConfiguration
    let current: Bool
    let radius: CGFloat
    @State private var hovering = false
    var body: some View {
        configuration.label
            .background(RoundedRectangle(cornerRadius: radius).fill(current ? Tones.active : hovering || configuration.isPressed ? Tones.hover : .clear))
            .contentShape(Rectangle())
            .onHover { hovering = $0 }
    }
}

/// "N agentes precisam de você" with Ir para o próximo, or the green notice that nothing waits (`attention.rs`).
struct AttentionCard: View {
    let state: ShellState
    @Environment(\.colorScheme) private var scheme

    var body: some View {
        let waiting = state.sidebar.waiting
        let dark = scheme == .dark
        let (wash, ink): (Color, Color) = dark
            ? (Color(red: 0xf0 / 255, green: 0xa0 / 255, blue: 0x20 / 255).opacity(38 / 255), .amber)
            : (Color(red: 0xf0 / 255, green: 0xa0 / 255, blue: 0x20 / 255).opacity(41 / 255), Color(red: 0xb8 / 255, green: 0x73 / 255, blue: 0x0a / 255))
        // nothing waiting: no notice (the main's sidebar has no green one)
        if waiting > 0 {
            VStack(alignment: .leading, spacing: 0) {
                HStack(spacing: 0) {
                    Circle().fill(ink).frame(width: 8, height: 8).frame(width: 12)
                    Text(waitingText(waiting)).font(.app(13)).padding(.leading, 6)
                    Spacer(minLength: 0)
                }
                .foregroundStyle(ink).frame(height: 38)
                NextButton(keys: state.sidebar.nextKeys).padding(.bottom, 10)
            }
            .padding(.horizontal, 10)
            .background(RoundedRectangle(cornerRadius: 10).fill(wash))
            .padding(.bottom, 8)
        }
    }

    private func waitingText(_ count: Int) -> String {
        switch count {
        case ...1: "1 agente precisa de você"
        default: "\(count) agentes precisam de você"
        }
    }
}

/// Ir para o próximo, with its keys as Ajustes › Atalhos has them ("Cmd ⇧ A").
private struct NextButton: View {
    let keys: String
    @State private var hovering = false
    var body: some View {
        let ink = Color(white: 0x1d / 255)
        Button { appAction("sidebar", ["action": "next"]) } label: {
            HStack {
                Text("Ir para o próximo").font(.app(13))
                Spacer()
                if !keys.isEmpty {
                    Text(keys.replacingOccurrences(of: "Shift", with: "⇧").replacingOccurrences(of: "+", with: " "))
                        .font(.system(size: 10, design: .monospaced)).foregroundStyle(ink.opacity(0.75))
                        .padding(.horizontal, 4).frame(height: 16)
                        .overlay(RoundedRectangle(cornerRadius: 4).stroke(ink.opacity(0.5), lineWidth: 1))
                }
            }
            .foregroundStyle(ink).padding(.horizontal, 10).frame(height: 28)
            .background(RoundedRectangle(cornerRadius: 7).fill(Color.amber.opacity(hovering ? 0.85 : 1)))
        }
        .buttonStyle(.plain).onHover { hovering = $0 }.help("Próximo agente que precisa de você")
    }
}

/// "Agentes deste espaço" and its count, foldable; a row per agent with its mascot, a Maestro's team folded under it.
struct AgentsSection: View {
    let state: ShellState

    var body: some View {
        let side = state.sidebar
        Button {
            appAction("sidebar", ["action": side.agentsClosed ? "agents_open" : "agents_close"])
        } label: {
            HStack(spacing: 0) {
                Chevron(open: !side.agentsClosed).frame(width: 29 - 8, alignment: .center).padding(.leading, 4)
                Text("AGENTES DESTE ESPAÇO").font(.app(12)).foregroundStyle(Tones.muted).padding(.leading, 2)
                Spacer()
                Text("\(side.agents.count)").font(.app(11)).foregroundStyle(Tones.text)
                    .padding(.horizontal, 6).frame(minWidth: 16, minHeight: 16).background(Capsule().fill(Tones.active)).padding(.trailing, 8)
            }
            .frame(height: 24)
        }
        .buttonStyle(RowStyle())
        .help(side.agentsClosed ? "Mostrar agentes deste espaço" : "Recolher agentes deste espaço")
        if !side.agentsClosed {
            if side.agents.isEmpty {
                Text("Nenhum agente aqui ainda").font(.app(12)).foregroundStyle(Tones.muted).padding(.leading, 12).frame(height: 26)
            }
            ForEach(side.agents) { agent in
                Button { shellVerb("agent", ["id": agent.id]) } label: { AgentRowView(agent: agent) }
                    .buttonStyle(RowStyle(radius: 7))
                    .padding(.leading, agent.member ? 14 : 0)
                    .help(agent.mission ?? agent.name)
                if let team = agent.team {
                    Button { appAction("sidebar", ["action": team.open ? "team_close" : "team_open", "team": agent.name]) } label: {
                        HStack(spacing: 0) {
                            Chevron(open: team.open).frame(width: 10).padding(.leading, 14 + 19 - 5)
                            Text(team.summary).font(.app(12)).foregroundStyle(Tones.muted).lineLimit(1).padding(.leading, 36 - 19 - 5)
                            Spacer(minLength: 6)
                        }
                        .frame(height: 24)
                    }
                    .buttonStyle(RowStyle()).help(team.open ? "Recolher os agentes" : "Mostrar os agentes")
                }
            }
        }
        // egui's 8 after the section, less the stack's own 2 that egui's add_space does not take
        Spacer().frame(height: 6)
    }
}

/// An agent: its mini wizard in its colour, its name, and its state with what it is doing.
private struct AgentRowView: View {
    let agent: AgentRow

    var body: some View {
        HStack(spacing: 0) {
            AgentMini(agent: agent).frame(width: 38)
            VStack(alignment: .leading, spacing: 0) {
                Text(agent.name).font(.app(14)).foregroundStyle(Tones.text).lineLimit(1).frame(height: 16)
                Text([agent.label, agent.note].compactMap { $0 }.joined(separator: " · ")).font(.app(12)).foregroundStyle(Tones.muted).lineLimit(1).frame(height: 16)
            }
            .padding(.leading, -2)
            Spacer(minLength: 6)
        }
        .frame(height: 40)
    }
}

/// The mini wizard (`mascot/mago::paint_mini`): still at rest, so a quiet sidebar asks for no frames; alive otherwise.
private struct AgentMini: View {
    let agent: AgentRow

    var body: some View {
        if ["idle", "sleeping"].contains(agent.mago) {
            mini(time: 0, still: true)
        } else {
            TimelineView(.animation) { timeline in mini(time: timeline.date.timeIntervalSinceReferenceDate, still: false) }
        }
    }

    private func mini(time: Double, still: Bool) -> some View {
        let (id, color, state) = (agent.id, Color(rgba: agent.color), LunaState(rawValue: agent.mago) ?? .idle)
        return Canvas { context, size in
            let motion = still ? LunaMotion(seed: UInt32(truncatingIfNeeded: id.hashValue)) : Luna.motion("sidebar-\(id)")
            motion.advance(state, time, gaze: still ? .zero : motion.glance(time))
            Luna.paintMini(&context, center: CGPoint(x: size.width / 2, y: size.height / 2), body: 20, color: color, motion: motion, time: time)
        }
        .frame(width: 30, height: 36)
    }
}
