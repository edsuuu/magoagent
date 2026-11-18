// The open island's pages: Início (feed.rs), Conversas and Claude Code (home.rs), the conversation (prompt.rs,
// session.rs), the new chat (forms.rs), the island's settings (paint.rs) and the card of a question or a permission
// (ask.rs). What they do goes to Rust as `ui.island.*` or as the app's own actions (`set_setting`, `chat_send`…).

import AppKit
import SwiftUI

extension View {
    /// The page's card: `CARD`, 20 round, the hairline.
    func islandCard(_ color: Color = IslandInk.card) -> some View {
        background(RoundedRectangle(cornerRadius: 20, style: .continuous).fill(color))
            .overlay(RoundedRectangle(cornerRadius: 20, style: .continuous).strokeBorder(IslandInk.hairline))
            .clipShape(RoundedRectangle(cornerRadius: 20, style: .continuous))
    }
}

/// A small round button (`forms::chip`): `strong` is the one that does it.
struct IslandChip: View {
    let text: String
    var strong = false
    let action: () -> Void
    @State private var hovered = false

    var body: some View {
        Button(action: action) {
            Text(text).font(.system(size: 11.5)).foregroundStyle(strong ? Color.black : IslandInk.ink)
                .padding(.horizontal, 11).frame(height: 26)
                .background(Capsule().fill(strong ? IslandInk.ink.opacity(hovered ? 1 : 0.92) : Color.white.opacity((hovered ? 34 : 18) / 255)))
                .contentShape(Capsule())
        }
        .buttonStyle(.plain)
        .onHover { hovered = $0 }
    }
}

/// A word that is a button, dim and lit under the pointer (`feed::link`).
struct IslandLink: View {
    let text: String
    var size: CGFloat = 10.5
    let action: () -> Void
    @State private var hovered = false

    var body: some View {
        Button(action: action) {
            Text(text).font(.system(size: size)).foregroundStyle(hovered ? IslandInk.ink : IslandInk.dim).underline(hovered)
        }
        .buttonStyle(.plain)
        .onHover { hovered = $0 }
    }
}

/// Luna on a page, where it puts her; a click pokes her (annoyed, then dizzy).
struct IslandLuna: View {
    let model: IslandModel
    let center: CGPoint
    let size: CGFloat
    let state: LunaState

    var body: some View {
        ZStack(alignment: .topLeading) {
            IslandCanvas(model: model, sprites: [.luna(center: center, size: size, state: state)])
            Circle().fill(Color.clear).contentShape(Circle())
                .frame(width: size * 0.9, height: size * 0.9)
                .position(center)
                .onTapGesture {
                    let motion = Luna.motion("island-main"), time = Date().timeIntervalSinceReferenceDate
                    motion.poke(time)
                    model.act("chime", ["name": motion.dizzy(time) ? "dizzy" : "poke"])
                }
        }
    }
}

/// How long ago, in words (`feed::ago`).
func islandAgo(_ at: Int64, now: Int64 = Int64(Date().timeIntervalSince1970 * 1000)) -> String {
    guard at > 0 else { return "" }
    let minutes = max(now - at, 0) / 60_000
    switch minutes {
    case 0: return "agora"
    case 1...59: return "há \(minutes) min"
    case 60...1439: return "há \(minutes / 60) h"
    case 1440...2879: return "ontem"
    default: return "há \(minutes / 1440) dias"
    }
}

/// The page under the header: a question, the settings, or the tab's (`island_page`).
struct IslandPage: View {
    let model: IslandModel
    let size: CGSize

    var body: some View {
        let snapshot = model.snapshot
        let key = snapshot.ask.map { "ask-\($0.chat)" } ?? (snapshot.settings ? "settings" : snapshot.tab)
        ZStack(alignment: .topLeading) {
            Group {
                if let ask = snapshot.ask {
                    IslandAskCard(model: model, ask: ask, size: size)
                } else if snapshot.settings {
                    IslandSettingsPage(model: model, size: size)
                } else {
                    switch snapshot.tab {
                    case "home": IslandFeed(model: model, size: size)
                    case "github": IslandGithubPage(model: model, size: size)
                    case "music": IslandMusicPage(model: model, size: size)
                    case "chats", "claude": IslandHome(model: model, size: size)
                    case "chat": IslandChatPage(model: model, size: size)
                    default: IslandNewPage(model: model, size: size)
                    }
                }
            }
            .frame(width: size.width, height: size.height, alignment: .topLeading)
            // the reference's `.view.on`: its opacity, and from 97% of its size
            .id(key)
            .transition(.asymmetric(
                insertion: .opacity.combined(with: .scale(scale: 0.97)).animation(.timingCurve(0.3, 1.2, 0.4, 1, duration: 0.3).delay(0.16)),
                removal: .opacity.animation(.easeIn(duration: 0.16))))
        }
    }
}

// MARK: Início

struct IslandFeed: View {
    let model: IslandModel
    let size: CGSize

    var body: some View {
        let snapshot = model.snapshot
        let rows = snapshot.feed.compactMap { id in snapshot.lines.first { $0.id == id } }
        HStack(spacing: 10) {
            summary(snapshot).frame(width: 236).islandCard()
            list(snapshot, rows).frame(maxWidth: .infinity, maxHeight: .infinity).islandCard()
        }
    }

    /// The summary's lines: (colour, the count in bold or none, the words).
    static func summary(_ snapshot: IslandSnapshot) -> [(Color, Int?, String)] {
        let needs = snapshot.lines.filter { $0.rank == 0 }.count
        let working = snapshot.lines.filter { $0.state == "working" }.count
        let failing = snapshot.github?.prs.filter { $0.mine && $0.ci == "failure" }.count ?? 0
        var rows: [(Color, Int?, String)] = []
        if needs > 0 { rows.append((IslandInk.amber, needs, needs == 1 ? "precisa de você" : "precisam de você")) }
        if working > 0 { rows.append((IslandInk.blue, working, working == 1 ? "agente trabalhando" : "agentes trabalhando")) }
        if failing > 0 { rows.append((IslandInk.red, failing, failing == 1 ? "PR com CI falhando" : "PRs com CI falhando")) }
        if snapshot.song?.playing == true { rows.append((IslandInk.green, nil, "tocando no Spotify")) }
        if rows.isEmpty { rows.append((IslandInk.dim3, nil, "Tudo calmo por aqui")) }
        return rows
    }

    private func summary(_ snapshot: IslandSnapshot) -> some View {
        let rows = Self.summary(snapshot)
        let top = 14 + 26 + CGFloat(rows.count) * 20
        let luna = min(max(size.height - 12 - top, 30), 64)
        return ZStack(alignment: .topLeading) {
            VStack(alignment: .leading, spacing: 0) {
                Text("Resumo").font(.system(size: 13, weight: .semibold)).foregroundStyle(IslandInk.ink).frame(height: 26, alignment: .top)
                ForEach(Array(rows.enumerated()), id: \.offset) { _, row in
                    HStack(spacing: 0) {
                        Circle().fill(row.0).frame(width: 7, height: 7).padding(.trailing, 7)
                        if let count = row.1 { Text("\(count)").font(.system(size: 12, weight: .semibold)).foregroundStyle(IslandInk.ink).padding(.trailing, 4) }
                        Text(row.2).font(.system(size: 12)).foregroundStyle(IslandInk.dim)
                    }
                    .frame(height: 20)
                }
            }
            .padding(.leading, 18).padding(.top, 14)
            IslandLuna(model: model, center: CGPoint(x: 118, y: size.height - 10 - luna / 2), size: luna, state: LunaState(rawValue: snapshot.state) ?? .idle)
        }
    }

    private func list(_ snapshot: IslandSnapshot, _ rows: [IslandLine]) -> some View {
        VStack(spacing: 0) {
            if rows.isEmpty {
                Text(snapshot.hidden > 0 ? "Tudo o que havia está oculto" : "Nada acontecendo agora")
                    .font(.system(size: 12)).foregroundStyle(IslandInk.dim3).frame(maxWidth: .infinity, maxHeight: .infinity)
            } else {
                ScrollView {
                    LazyVStack(spacing: 1) { ForEach(rows) { IslandFeedRow(model: model, line: $0) } }
                }
                .scrollIndicators(.never)
            }
            HStack(spacing: 0) {
                if snapshot.hidden > 0 {
                    Text("\(snapshot.hidden) \(snapshot.hidden == 1 ? "oculto" : "ocultos") · ").font(.system(size: 10.5)).foregroundStyle(IslandInk.dim3)
                    IslandLink(text: "Mostrar ocultos") { model.setting("island_feed", "show_hidden") }
                }
                Spacer()
                if !rows.isEmpty { IslandLink(text: "Ocultar tudo") { model.setting("island_feed", "hide_all") } }
            }
            .padding(.horizontal, 6).frame(height: 16)
        }
        .padding(.horizontal, 8).padding(.vertical, 7)
    }
}

/// One line: the mini, the name over what it does, how long ago (the × in its place under the pointer).
struct IslandFeedRow: View {
    let model: IslandModel
    let line: IslandLine
    @State private var hovered = false

    var body: some View {
        let when: String = {
            if line.state == "working" { return "agora" }
            let ago = islandAgo(line.at)
            if ago.isEmpty, line.id == "integration:spotify" { return model.snapshot.song?.playing == true ? "tocando" : "pausado" }
            return ago
        }()
        let tone = line.rank == 0 ? IslandInk.amber : line.rank == 1 ? IslandInk.red : IslandInk.dim3
        HStack(spacing: 10) {
            IslandCanvas(model: model, sprites: [.mini(line: line, center: CGPoint(x: 11, y: 20), body: 20, mark: line.outside)]).frame(width: 22, height: 38)
            VStack(alignment: .leading, spacing: 2) {
                Text(line.name).font(.system(size: 12, weight: .semibold)).foregroundStyle(IslandInk.ink).lineLimit(1)
                Text(line.detail).font(.system(size: 11)).foregroundStyle(tone).lineLimit(1)
            }
            Spacer(minLength: 10)
            if hovered {
                Button { model.act("hide", ["id": line.id]) } label: {
                    Image(systemName: "xmark").font(.system(size: 9, weight: .bold)).foregroundStyle(IslandInk.ink)
                        .frame(width: 20, height: 20).background(Circle().fill(IslandInk.rgb(0x2b2e33)))
                }
                .buttonStyle(.plain)
                .help("Ocultar")
            } else if !when.isEmpty {
                Text(when).font(.system(size: 10.5)).foregroundStyle(IslandInk.dim3)
            }
        }
        .padding(.horizontal, 6)
        .frame(height: 38)
        .background(RoundedRectangle(cornerRadius: 10).fill(hovered ? IslandInk.hover : .clear))
        .contentShape(Rectangle())
        .onHover { hovered = $0 }
        .onTapGesture {
            // its tab, with it in the card; the integrations with no tab of their own do their first button
            if line.tab == "home" { return model.act("press", ["id": line.id]) }
            model.open(tab: line.tab, focus: line.id)
            if line.chat || line.outside { model.act("open", ["id": line.id]) }
        }
    }
}

// MARK: Conversas and Claude Code

struct IslandHome: View {
    let model: IslandModel
    let size: CGSize

    /// What an empty page says (`home::empty_words`).
    static func emptyWords(_ tab: String, hooks: Bool) -> (String, String) {
        switch tab {
        case "claude" where hooks: ("Nenhuma sessão do Claude Code", "Os hooks estão instalados: as sessões aparecem aqui quando começarem")
        case "claude": ("Nenhuma sessão do Claude Code", "Instale os hooks em Ajustes › Integrações")
        case "github": ("O GitHub está desligado", "Ligue em Ajustes › Integrações")
        case "music": ("Nada tocando agora", "Ligue o Spotify em Ajustes › Integrações")
        default: ("Nenhum agente ainda", "Crie um chat em  +")
        }
    }

    var body: some View {
        let snapshot = model.snapshot
        let tab = snapshot.tab
        let lines = snapshot.lines.filter { tab == "claude" ? $0.tab == "claude" || $0.claude : $0.tab == "chats" }
        let focus = snapshot.focus.flatMap { id in snapshot.lines.first { $0.id == id } }
        let activity = focus == nil ? nil : snapshot.activity
        let editing = activity?.phase == "editing"
        let alone = focus == nil && lines.isEmpty
        let state = focus.flatMap { LunaState(rawValue: $0.mago) } ?? LunaState(rawValue: snapshot.state) ?? .idle
        let leftWidth = alone ? size.width : editing ? 150 : 322
        HStack(spacing: 10) {
            ZStack(alignment: .topLeading) {
                IslandWash(color: IslandHome.wash(state), strength: 0.3)
                if let focus, let activity, editing {
                    IslandEditSide(line: focus, activity: activity)
                } else if let focus, let activity {
                    IslandActivityCard(line: focus, activity: activity)
                } else if let focus {
                    IslandIdleCard(model: model, line: focus)
                } else {
                    let (title, hint) = Self.emptyWords(tab, hooks: snapshot.claudeHooks)
                    VStack(alignment: .leading, spacing: 6) {
                        Text(title).font(.system(size: 13)).foregroundStyle(IslandInk.ink)
                        Text(hint).font(.system(size: 11.5)).foregroundStyle(IslandInk.dim)
                    }
                    .padding(.leading, 106).frame(maxHeight: .infinity)
                }
                IslandLuna(model: model, center: editing ? CGPoint(x: 36, y: 40) : CGPoint(x: 58, y: size.height / 2), size: editing ? 46 : 62, state: state)
            }
            .frame(width: leftWidth, height: size.height, alignment: .topLeading)
            .islandCard()
            if !alone {
                Group {
                    if let activity, editing { IslandEditCard(activity: activity) } else { grid(lines, tab: tab) }
                }
                .frame(maxWidth: .infinity, maxHeight: .infinity)
                .islandCard()
            }
        }
        .overlay { IslandDizzy() }
    }

    /// The colour under the card by the state (`wash`).
    static func wash(_ state: LunaState) -> Color {
        switch state {
        case .needsYou, .asking: IslandInk.amber
        case .error: IslandInk.red
        case .working: IslandInk.blue
        case .done: IslandInk.mint
        default: .clear
        }
    }

    /// Two columns of pills, a mini wizard and the name, in the middle when they fit.
    private func grid(_ lines: [IslandLine], tab: String) -> some View {
        ScrollView {
            LazyVGrid(columns: [GridItem(.flexible(), spacing: 4), GridItem(.flexible(), spacing: 4)], spacing: 4) {
                ForEach(lines) { line in
                    let tool = line.id.hasPrefix("integration:")
                    let opens = (tab == "claude" && !line.outside || model.snapshot.focus == line.id) && !tool
                    IslandPillButton(model: model, line: line, opens: opens)
                }
            }
            .padding(.horizontal, 8)
            .frame(minHeight: size.height)
        }
        .scrollIndicators(.never)
    }
}

/// The warm light under a card, from its bottom (`wash_with`).
struct IslandWash: View {
    let color: Color
    let strength: Double

    var body: some View {
        GeometryReader { proxy in
            RadialGradient(colors: [color.opacity(strength), .clear], center: UnitPoint(x: 0.5, y: 1.15), startRadius: 0, endRadius: max(proxy.size.width, proxy.size.height))
        }
        .allowsHitTesting(false)
    }
}

/// One pill of the grid (`paint::pill`): its mini and its name; the pointer lights it in its colour.
struct IslandPillButton: View {
    let model: IslandModel
    let line: IslandLine
    let opens: Bool
    @State private var hovered = false

    var body: some View {
        let color = IslandInk.hex(line.color)
        Button {
            model.act("focus", ["id": line.id])
            if opens { model.act("open", ["id": line.id]) }
        } label: {
            ZStack(alignment: .leading) {
                RoundedRectangle(cornerRadius: 14).fill(IslandInk.cardFlat)
                RoundedRectangle(cornerRadius: 14).fill(hovered ? color.opacity(0x2e / 255) : .clear)
                RoundedRectangle(cornerRadius: 14).strokeBorder(color.opacity((hovered ? 0x8c : line.asleep ? 0x1e : 0x24) / 255))
                IslandCanvas(model: model, sprites: [.mini(line: line, center: CGPoint(x: 19, y: 14), body: 24, mark: true)])
                Text(line.name).font(.system(size: 10, weight: .semibold)).lineLimit(1)
                    .foregroundStyle(hovered ? IslandInk.lighten(line.color, 0.3) : IslandInk.dim3)
                    .frame(maxWidth: .infinity).padding(.leading, 34).padding(.trailing, 8)
            }
            .frame(height: 28)
            .shadow(color: hovered ? color.opacity(0x59 / 255) : .clear, radius: 5, y: 2)
            .contentShape(Rectangle())
        }
        .buttonStyle(.plain)
        .onHover { hovered = $0 }
        .help(line.name)
    }
}

/// "● name  Claude Code" and, at the right, the step's place in the turn.
struct IslandNameRow: View {
    let line: IslandLine
    var place: [Int] = []

    var body: some View {
        HStack(spacing: 6) {
            Circle().fill(IslandInk.hex(line.color)).frame(width: 8, height: 8)
            Text(line.name).font(.system(size: 12, weight: .semibold)).foregroundStyle(IslandInk.ink).lineLimit(1)
            Text(line.kind).font(.system(size: 11)).foregroundStyle(IslandInk.dim).lineLimit(1)
            Spacer(minLength: 4)
            if place.count == 2, place[1] > 0 { Text("\(place[0])/\(place[1])").font(.system(size: 11)).foregroundStyle(IslandInk.dim3) }
        }
        .frame(height: 18)
    }
}

/// An agent with nothing to say of its turn: its name, its state and the buttons; a tool's card has its own buttons.
struct IslandIdleCard: View {
    let model: IslandModel
    let line: IslandLine

    var body: some View {
        VStack(alignment: .leading, spacing: 8) {
            IslandNameRow(line: line)
            Text(line.detail).font(.system(size: 11.5)).foregroundStyle(line.state == "waiting" ? IslandInk.amber : IslandInk.dim).lineLimit(2)
            Spacer()
            HStack(spacing: 8) {
                if line.tool {
                    ForEach(line.chips, id: \.self) { chip in
                        IslandChip(text: chip.first ?? "", strong: chip == line.chips.first) { model.act("press", ["action": chip.last ?? ""]) }
                    }
                } else {
                    IslandChip(text: "Abrir", strong: true) { model.act("open", ["id": line.id]) }
                    // a chat's Abrir is its conversation here; on the canvas the second goes there instead
                    if line.chat, line.canvas {
                        IslandChip(text: "No canvas") { model.act("canvas", ["id": line.id]) }.help("Abrir o cartão no canvas do app")
                    } else if line.chat {
                        IslandChip(text: "Conversar") { model.act("talk", ["id": line.id]) }
                    }
                }
            }
        }
        .padding(.leading, 106).padding(.trailing, 14).padding(.top, 10).padding(.bottom, 12)
    }
}

/// The tool in progress, or how the turn ended, with a glimpse under it.
struct IslandActivityCard: View {
    let line: IslandLine
    let activity: IslandActivity

    var body: some View {
        VStack(alignment: .leading, spacing: 6) {
            IslandNameRow(line: line, place: activity.place)
            HStack(spacing: 5) {
                Text(activity.verb).font(.system(size: 12, weight: .semibold)).foregroundStyle(activity.phase == "failed" ? IslandInk.red : IslandInk.ink)
                Text(activity.headline ?? activity.target).font(.system(size: 12)).foregroundStyle(IslandInk.dim).lineLimit(1)
            }
            ForEach(Array(activity.moves.suffix(3).enumerated()), id: \.offset) { _, step in
                HStack(spacing: 6) {
                    Circle().fill(step.failed ? IslandInk.red : step.running ? IslandInk.blue : IslandInk.mint).frame(width: 5, height: 5)
                    Text(step.verb).font(.system(size: 11)).foregroundStyle(IslandInk.dim).lineLimit(1)
                }
            }
            ForEach(Array(activity.output.suffix(3).enumerated()), id: \.offset) { _, text in
                Text(text).font(.system(size: 11, design: .monospaced)).foregroundStyle(IslandInk.dim3).lineLimit(1)
            }
            Spacer(minLength: 0)
        }
        .padding(.leading, 106).padding(.trailing, 14).padding(.top, 10)
    }
}

/// Editing a file: the name and the file at the narrow left card.
struct IslandEditSide: View {
    let line: IslandLine
    let activity: IslandActivity

    var body: some View {
        VStack(alignment: .leading, spacing: 4) {
            Spacer().frame(height: 70)
            Text(line.name).font(.system(size: 12, weight: .semibold)).foregroundStyle(IslandInk.ink).lineLimit(1)
            Text(activity.verb).font(.system(size: 11)).foregroundStyle(IslandInk.dim).lineLimit(1)
            if let diff = activity.diff, diff.count == 2 {
                HStack(spacing: 6) {
                    Text("+\(diff[0])").foregroundStyle(IslandInk.mint)
                    Text("−\(diff[1])").foregroundStyle(IslandInk.red)
                }
                .font(.system(size: 11, weight: .semibold, design: .monospaced))
            }
        }
        .padding(.horizontal, 14)
    }
}

/// The change being written, at the right: the file and its lines, added green and removed red.
struct IslandEditCard: View {
    let activity: IslandActivity

    var body: some View {
        VStack(alignment: .leading, spacing: 6) {
            Text(activity.file ?? activity.target).font(.system(size: 11.5, weight: .semibold)).foregroundStyle(IslandInk.ink).lineLimit(1)
            ScrollView {
                VStack(alignment: .leading, spacing: 1) {
                    ForEach(Array(activity.patch.enumerated()), id: \.offset) { index, text in
                        HStack(spacing: 8) {
                            Text("\(activity.patchLine + index)").foregroundStyle(IslandInk.dim3).frame(width: 30, alignment: .trailing)
                            Text(text).foregroundStyle(text.hasPrefix("+") ? IslandInk.mint : text.hasPrefix("-") ? IslandInk.red : IslandInk.rowInk).lineLimit(1)
                        }
                        .font(.system(size: 11, design: .monospaced))
                    }
                }
            }
            .scrollIndicators(.never)
        }
        .padding(14)
    }
}

/// Poked too much: Luna is dizzy and the card says so for a moment.
struct IslandDizzy: View {
    var body: some View {
        TimelineView(.periodic(from: .now, by: 0.25)) { timeline in
            if Luna.motion("island-main").dizzy(timeline.date.timeIntervalSinceReferenceDate) {
                VStack(spacing: 4) {
                    Text("Muitos toques de uma vez.").font(.system(size: 13)).foregroundStyle(IslandInk.ink)
                    Text("Me dá um segundo…").font(.system(size: 11.5)).foregroundStyle(IslandInk.dim)
                }
                .frame(maxWidth: .infinity, maxHeight: .infinity)
                .islandCard()
                .allowsHitTesting(false)
            }
        }
    }
}

// MARK: Ajustes da ilha

struct IslandSettingsPage: View {
    let model: IslandModel
    let size: CGSize
    @State private var volume: Double?

    var body: some View {
        let snapshot = model.snapshot
        ZStack(alignment: .topLeading) {
            IslandLuna(model: model, center: CGPoint(x: 44, y: size.height / 2), size: 46, state: LunaState(rawValue: snapshot.state) ?? .idle)
            VStack(alignment: .leading, spacing: 10) {
                // 1: the sound, its volume, and the colours
                HStack(spacing: 10) {
                    Toggle("", isOn: Binding(get: { snapshot.sound }, set: { model.setting("island_sound", $0 ? "on" : "off") }))
                        .toggleStyle(.switch).tint(IslandInk.green).labelsHidden().controlSize(.small)
                    Text("Som").font(.system(size: 12.5)).foregroundStyle(IslandInk.rowInk)
                    Slider(value: Binding(get: { volume ?? snapshot.volume }, set: { volume = ($0 / 0.005).rounded() * 0.005 }), in: 0...0.2) { editing in
                        if !editing, let volume { model.setting("island_volume", String(format: "%.3f", volume)) }
                    }
                    .controlSize(.mini).frame(width: 72).opacity(snapshot.sound ? 1 : 0.4)
                    Spacer()
                    ForEach(IslandInk.colors, id: \.self) { hex in
                        let chosen = snapshot.color.lowercased() == hex
                        Button { model.setting("island_color", hex) } label: {
                            Circle().fill(IslandInk.hex(hex)).frame(width: 11, height: 11)
                                .overlay(Circle().strokeBorder(Color.white.opacity(40 / 255)))
                                .padding(2.5).overlay(Circle().strokeBorder(chosen ? IslandInk.ink : .clear, lineWidth: 1.2))
                        }
                        .buttonStyle(.plain).help("Cor da ilha")
                    }
                }
                .frame(height: 20)
                // 2: how long the open island stays after the pointer left
                HStack(spacing: 6) {
                    Image(systemName: "timer").font(.system(size: 11)).foregroundStyle(IslandInk.rowInk)
                    Text("Fechar auto · \(snapshot.closeAfter)s").font(.system(size: 12.5)).foregroundStyle(IslandInk.rowInk)
                    Spacer()
                    ForEach([5, 10, 15, 30], id: \.self) { seconds in
                        let chosen = seconds == snapshot.closeAfter
                        Button { model.setting("island_close_after", "\(seconds)") } label: {
                            Text("\(seconds)s").font(.system(size: 11)).foregroundStyle(chosen ? IslandInk.ink : IslandInk.dim3)
                                .padding(.horizontal, 7).frame(height: 19)
                                .background(Capsule().fill(chosen ? IslandInk.rgb(0x252830) : .clear))
                        }
                        .buttonStyle(.plain)
                    }
                }
                .frame(height: 24)
                // 3: the hooks of Claude Code, the API, and the app's settings
                HStack(spacing: 14) {
                    ForEach([("Claude Code", snapshot.agentHooks), ("API", false)], id: \.0) { name, on in
                        HStack(spacing: 4) {
                            Circle().fill(on ? IslandInk.green : IslandInk.red).frame(width: 6, height: 6)
                            Text(name).font(.system(size: 11)).foregroundStyle(IslandInk.dim)
                        }
                    }
                    Spacer()
                    IslandLink(text: "Ajustes…", size: 11.5) { model.act("app_settings") }
                }
                .frame(height: 18)
            }
            .padding(.leading, 84).padding(.trailing, 16)
            .frame(width: size.width, height: size.height)
        }
        .frame(width: size.width, height: size.height, alignment: .topLeading)
        .islandCard()
    }
}

// MARK: the conversation

struct IslandChatPage: View {
    let model: IslandModel
    let size: CGSize
    @State private var draft = ""
    @FocusState private var focused: Bool

    var body: some View {
        Group {
            if let chat = model.snapshot.chat, let session = chat.session {
                // a session outside the app: it is answered in its own terminal
                VStack(spacing: 10) {
                    Text(chat.name).font(.system(size: 13, weight: .semibold)).foregroundStyle(IslandInk.ink)
                    IslandChip(text: "Ir ao terminal", strong: true) { model.act("terminal", ["id": session]) }
                }
                .frame(maxWidth: .infinity, maxHeight: .infinity).islandCard()
            } else if let chat = model.snapshot.chat {
                if chat.full, let view = MagoViews.chat?(chat.id) {
                    view.clipShape(RoundedRectangle(cornerRadius: 20, style: .continuous))
                } else {
                    talk(chat)
                }
            } else {
                Text("Escolha um agente em Conversas.").font(.system(size: 12)).foregroundStyle(IslandInk.dim)
                    .frame(maxWidth: .infinity, maxHeight: .infinity).islandCard()
            }
        }
        .onChange(of: focused) { model.typing = focused }
        .onDisappear { model.typing = false }
    }

    /// The short conversation (`prompt.rs`): the last things said and the box to say more.
    private func talk(_ chat: IslandChat) -> some View {
        VStack(spacing: 8) {
            HStack {
                Text(chat.name).font(.system(size: 12, weight: .semibold)).foregroundStyle(IslandInk.ink).lineLimit(1)
                if chat.working { ProgressView().controlSize(.mini) }
                Spacer()
                IslandLink(text: "Abrir em miniatura", size: 11) { model.act("full") }
            }
            ScrollViewReader { reader in
                ScrollView {
                    VStack(alignment: .leading, spacing: 6) {
                        ForEach(Array(chat.said.enumerated()), id: \.offset) { index, said in
                            Text(said.text).font(.system(size: 12)).foregroundStyle(said.person ? IslandInk.ink : IslandInk.rowInk)
                                .textSelection(.enabled)
                                .padding(.horizontal, said.person ? 10 : 0).padding(.vertical, said.person ? 6 : 0)
                                .background(RoundedRectangle(cornerRadius: 12).fill(said.person ? Color.white.opacity(18 / 255) : .clear))
                                .frame(maxWidth: .infinity, alignment: said.person ? .trailing : .leading)
                                .id(index)
                        }
                    }
                }
                .scrollIndicators(.never)
                .onAppear { reader.scrollTo(chat.said.count - 1, anchor: .bottom) }
                .onChange(of: chat.said.count) { reader.scrollTo(chat.said.count - 1, anchor: .bottom) }
            }
            IslandPromptBar(text: $draft, placeholder: "Responder…", focused: $focused) {
                let words = draft.trimmingCharacters(in: .whitespacesAndNewlines)
                guard !words.isEmpty else { return }
                model.app("chat_send", ["chat": chat.id, "text": words])
                draft = ""
            }
        }
        .padding(12)
        .islandCard()
    }
}

/// The reference's `.chat-bar`: the words and the round button that sends them.
struct IslandPromptBar: View {
    @Binding var text: String
    let placeholder: String
    var focused: FocusState<Bool>.Binding
    var ready = true
    let send: () -> Void

    var body: some View {
        HStack(alignment: .bottom, spacing: 8) {
            TextField(placeholder, text: $text, axis: .vertical)
                .textFieldStyle(.plain).font(.system(size: 12.5)).foregroundStyle(IslandInk.ink)
                .lineLimit(1...5).focused(focused)
                .onSubmit(send)
                .padding(.vertical, 6)
            Button(action: send) {
                Image(systemName: "arrow.up").font(.system(size: 12, weight: .bold)).foregroundStyle(IslandInk.black)
                    .frame(width: 28, height: 28).background(Circle().fill(IslandInk.ink.opacity(ready ? 1 : 0.4)))
            }
            .buttonStyle(.plain)
        }
        .padding(.leading, 10).padding(.trailing, 10).padding(.vertical, 6)
        .background(RoundedRectangle(cornerRadius: 12).fill(Color.white.opacity(18 / 255)))
    }
}
