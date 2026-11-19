// The island's GitHub and Spotify tabs (github/, music.rs): the pull requests by filter with what one can do to them,
// and the song playing with its controls. What they write goes by the app's `integrations` action and Spotify's words
// (`ui.island.spotify`).

import AppKit
import SwiftUI

// MARK: GitHub

struct IslandGithubPage: View {
    let model: IslandModel
    let size: CGSize
    @State private var filter = "all"
    @State private var chosen: String?

    static let filters = [("all", "Todas"), ("open", "Abertas"), ("needs_you", "Precisa de você"), ("closed", "Fechadas")]

    static func shows(_ pull: IslandPull, _ filter: String) -> Bool {
        switch filter {
        case "open": pull.state == "open"
        case "needs_you": pull.needsYou
        case "closed": pull.state != "open"
        default: true
        }
    }

    var body: some View {
        Group {
            if let github = model.snapshot.github {
                if let pull = github.prs.first(where: { $0.id == chosen }) {
                    IslandPullDetail(model: model, pull: pull) { chosen = nil }
                } else {
                    list(github)
                }
            } else {
                let (title, hint) = IslandHome.emptyWords("github", hooks: false)
                VStack(spacing: 6) {
                    Text(title).font(.system(size: 13)).foregroundStyle(IslandInk.ink)
                    Text(hint).font(.system(size: 11.5)).foregroundStyle(IslandInk.dim)
                }
                .frame(maxWidth: .infinity, maxHeight: .infinity)
            }
        }
        .islandCard()
    }

    private func list(_ github: IslandGithub) -> some View {
        let pulls = github.prs.filter { Self.shows($0, filter) }
        return VStack(alignment: .leading, spacing: 6) {
            HStack(spacing: 4) {
                ForEach(Self.filters, id: \.0) { key, label in
                    Button { filter = key } label: {
                        Text(label).font(.system(size: 11)).foregroundStyle(filter == key ? IslandInk.ink : IslandInk.dim3)
                            .padding(.horizontal, 8).frame(height: 22)
                            .background(Capsule().fill(filter == key ? IslandInk.rgb(0x252830) : .clear))
                    }
                    .buttonStyle(.plain)
                }
                Spacer()
                if !github.login.isEmpty { Text(github.login).font(.system(size: 11)).foregroundStyle(IslandInk.dim3) }
            }
            if let error = github.error {
                Text(error).font(.system(size: 11.5)).foregroundStyle(IslandInk.red).lineLimit(2)
            }
            if pulls.isEmpty {
                Text(github.prs.isEmpty ? "Nenhum pull request aqui" : "Nenhum pull request neste filtro")
                    .font(.system(size: 12)).foregroundStyle(IslandInk.dim3).frame(maxWidth: .infinity, maxHeight: .infinity)
            } else {
                ScrollView {
                    LazyVStack(spacing: 1) {
                        ForEach(pulls) { pull in IslandPullRow(pull: pull) { chosen = pull.id } }
                    }
                }
                .scrollIndicators(.never)
            }
        }
        .padding(10)
    }
}

/// The CI's colour: green passed, red failed, amber running, dim none.
func islandCi(_ state: String) -> Color {
    switch state {
    case "success": IslandInk.mint
    case "failure": IslandInk.red
    case "pending": IslandInk.amber
    default: IslandInk.dim3
    }
}

struct IslandPullRow: View {
    let pull: IslandPull
    let action: () -> Void
    @State private var hovered = false

    var body: some View {
        Button(action: action) {
            HStack(spacing: 10) {
                Circle().fill(islandCi(pull.ci)).frame(width: 8, height: 8)
                VStack(alignment: .leading, spacing: 2) {
                    Text(pull.title).font(.system(size: 12, weight: .semibold)).foregroundStyle(IslandInk.ink).lineLimit(1)
                    HStack(spacing: 6) {
                        Text(pull.id).foregroundStyle(IslandInk.dim3)
                        if pull.draft { Text("rascunho").foregroundStyle(IslandInk.dim3) }
                        if pull.conflict { Text("Conflito").foregroundStyle(IslandInk.red) }
                        if pull.needsYou { Text("precisa de você").foregroundStyle(IslandInk.amber) }
                    }
                    .font(.system(size: 11)).lineLimit(1)
                }
                Spacer(minLength: 6)
                Text(islandAgo(pull.updatedAt)).font(.system(size: 10.5)).foregroundStyle(IslandInk.dim3)
            }
            .padding(.horizontal, 8).frame(height: 38)
            .background(RoundedRectangle(cornerRadius: 10).fill(hovered ? IslandInk.hover : .clear))
            .contentShape(Rectangle())
        }
        .buttonStyle(.plain)
        .onHover { hovered = $0 }
    }
}

/// A pull request opened: its branches and checks, and Aprovar, Mesclar and Comentar, each asked again before it
/// writes on GitHub in the person's name.
struct IslandPullDetail: View {
    let model: IslandModel
    let pull: IslandPull
    let back: () -> Void
    @State private var asking: (String, String)?
    @State private var comment = ""
    @State private var reply: String?
    @FocusState private var focused: Bool

    var body: some View {
        VStack(alignment: .leading, spacing: 8) {
            HStack(spacing: 8) {
                IslandLink(text: "Voltar", size: 11.5, action: back)
                Text(pull.id).font(.system(size: 11)).foregroundStyle(IslandInk.dim3)
                Spacer()
                IslandLink(text: "Abrir no navegador", size: 11.5) { if let url = URL(string: pull.url) { NSWorkspace.shared.open(url) } }
            }
            Text(pull.title).font(.system(size: 14, weight: .semibold)).foregroundStyle(IslandInk.ink).lineLimit(2)
            HStack(spacing: 6) {
                Text("\(pull.headRef) → \(pull.baseRef)").font(.system(size: 11, design: .monospaced)).foregroundStyle(IslandInk.dim)
                Circle().fill(islandCi(pull.ci)).frame(width: 6, height: 6)
                Text(pull.conflict ? "Conflito com a \(pull.baseRef)" : pull.ci == "pending" ? "Checks rodando" : pull.failing.isEmpty ? "Nada travando" : pull.failing.joined(separator: ", "))
                    .font(.system(size: 11)).foregroundStyle(pull.conflict || !pull.failing.isEmpty ? IslandInk.red : IslandInk.dim).lineLimit(1)
            }
            Spacer(minLength: 0)
            if let (action, question) = asking {
                HStack(spacing: 8) {
                    Text(question).font(.system(size: 12)).foregroundStyle(IslandInk.ink).lineLimit(1)
                    Spacer()
                    IslandChip(text: "Cancelar") { asking = nil }
                    IslandChip(text: "Confirmar", strong: true) { write(action) }
                }
            } else {
                HStack(spacing: 8) {
                    TextField("Escreva um comentário…", text: $comment).textFieldStyle(.plain).font(.system(size: 12)).focused($focused)
                        .padding(.horizontal, 10).frame(height: 28)
                        .background(RoundedRectangle(cornerRadius: 10).fill(Color.white.opacity(18 / 255)))
                    if !comment.trimmingCharacters(in: .whitespaces).isEmpty {
                        IslandChip(text: "Comentar") { asking = ("pr_comment", "Publicar o comentário em \(pull.id)?") }
                    }
                    if !pull.mine, pull.state == "open" { IslandChip(text: "Aprovar") { asking = ("pr_approve", "Aprovar \(pull.id)?") } }
                    if pull.mine, pull.state == "open", !pull.draft, !pull.conflict {
                        IslandChip(text: "Mesclar", strong: true) { asking = ("pr_merge", "Mesclar \(pull.id)?") }
                    }
                }
            }
            if let reply { Text(reply).font(.system(size: 11)).foregroundStyle(IslandInk.dim).lineLimit(2) }
        }
        .padding(14)
        .onChange(of: focused) { model.typing = focused }
        .onDisappear { model.typing = false }
    }

    private func write(_ action: String) {
        asking = nil
        reply = "Enviando ao GitHub…"
        let text = comment
        Task {
            let answer = await MagoStore.shared.act("integrations", ["integration": "github", "action": action, "pr": pull.id, "text": text])
            reply = answer.text
            if answer.ok, action == "pr_comment" { comment = "" }
        }
    }
}

// MARK: Spotify

struct IslandMusicPage: View {
    let model: IslandModel
    let size: CGSize
    @State private var volume: Double?

    var body: some View {
        Group {
            if let song = model.snapshot.song {
                playing(song)
            } else {
                VStack(spacing: 6) {
                    Text("Nada tocando agora").font(.system(size: 13)).foregroundStyle(IslandInk.ink)
                    Text("Toque algo no Spotify (ou escolha o Soloist no celular)").font(.system(size: 11.5)).foregroundStyle(IslandInk.dim)
                }
                .frame(maxWidth: .infinity, maxHeight: .infinity)
            }
        }
        .islandCard()
    }

    private func press(_ words: String) { model.act("spotify", ["words": words]) }

    // ponytail: the song playing and its controls only; the queue, the playlists, the liked songs and the search of
    // music.rs need their lists in the snapshot first
    private func playing(_ song: IslandSong) -> some View {
        let tint = IslandInk.hex(song.tint ?? IslandInk.spotify)
        let cover = min(size.height - 28, 220)
        return HStack(spacing: 18) {
            IslandCover(song: song, side: cover, corner: 14).shadow(color: tint.opacity(0.35), radius: 14, y: 4)
            VStack(alignment: .leading, spacing: 4) {
                Text(song.title).font(.system(size: 16, weight: .semibold)).foregroundStyle(IslandInk.ink).lineLimit(1)
                Text(song.artist).font(.system(size: 12.5)).foregroundStyle(IslandInk.rowInk).lineLimit(1)
                Text(song.album).font(.system(size: 11.5)).foregroundStyle(IslandInk.dim3).lineLimit(1)
                Spacer(minLength: 8)
                if let duration = song.durationMs, duration > 0 {
                    TimelineView(.periodic(from: .now, by: 1)) { timeline in
                        let at = song.position(now: timeline.date) ?? 0
                        VStack(spacing: 4) {
                            GeometryReader { proxy in
                                ZStack(alignment: .leading) {
                                    Capsule().fill(Color.white.opacity(0.14))
                                    Capsule().fill(tint).frame(width: proxy.size.width * min(at / Double(duration), 1))
                                }
                                .contentShape(Rectangle())
                                .onTapGesture { point in press("seek \(Int(point.x / proxy.size.width * Double(duration)))") }
                            }
                            .frame(height: 4)
                            HStack {
                                Text(Self.clock(at)); Spacer(); Text(Self.clock(Double(duration)))
                            }
                            .font(.system(size: 10.5, design: .monospaced)).foregroundStyle(IslandInk.dim3)
                        }
                    }
                }
                HStack(spacing: 18) {
                    control("shuffle", tip: "Aleatório", on: song.shuffle == true) { press("shuffle \(song.shuffle == true ? "off" : "on")") }
                    control("backward.fill", tip: "Voltar") { press("previous") }
                    control(song.playing ? "pause.fill" : "play.fill", tip: song.playing ? "Pausar" : "Tocar", size: 22) { press("playpause") }
                    control("forward.fill", tip: "Pular") { press("next") }
                    control(song.repeat == "track" ? "repeat.1" : "repeat", tip: "Repetir", on: (song.repeat ?? "off") != "off") {
                        press("repeat \(["off": "context", "context": "track"][song.repeat ?? "off"] ?? "off")")
                    }
                    if let liked = song.liked { control(liked ? "heart.fill" : "heart", tip: "Curtir", on: liked) { press(liked ? "unlike" : "like") } }
                    Spacer()
                    if let current = song.volume {
                        Image(systemName: "speaker.wave.1").font(.system(size: 11)).foregroundStyle(IslandInk.dim)
                        Slider(value: Binding(get: { volume ?? current }, set: { volume = $0 }), in: 0...100) { editing in
                            if !editing, let volume { press("volume \(Int(volume))") }
                        }
                        .controlSize(.mini).frame(width: 80).tint(tint)
                    }
                }
                if !song.device.isEmpty { Text("Tocando em \(song.device)").font(.system(size: 10.5)).foregroundStyle(IslandInk.dim3) }
            }
        }
        .padding(14)
    }

    private func control(_ symbol: String, tip: String, on: Bool = false, size: CGFloat = 15, action: @escaping () -> Void) -> some View {
        IslandHeaderButton(symbol: symbol, tip: tip, on: on, size: size, action: action)
    }

    static func clock(_ milliseconds: Double) -> String {
        let seconds = Int(milliseconds / 1000)
        return String(format: "%d:%02d", seconds / 60, seconds % 60)
    }
}
