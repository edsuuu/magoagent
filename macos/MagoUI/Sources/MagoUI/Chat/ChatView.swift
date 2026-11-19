// One conversation as the egui chat lays it out (chat/mod.rs `chat_screen`): the header, the messages in a column up to
// 1100 wide, and under them the band of what the agent runs, the amber card of what it asks and the box.
import AppKit
import SwiftUI

struct ChatView: View {
    let id: String
    private let store = MagoStore.shared
    @Environment(\.chatLook) private var look

    var body: some View {
        let snapshot = store.decode(ChatSnapshot.self, topic: "chat:\(id)")
        GeometryReader { geometry in
            // the column: 32 each side, at least 360 (or all there is), at most 1100
            let column = min(max(geometry.size.width - 64, min(360, geometry.size.width)), 1100)
            VStack(spacing: 0) {
                if let snapshot {
                    ChatHeader(snapshot: snapshot)
                    Conversation(snapshot: snapshot, width: column)
                    VStack(spacing: 8) {
                        if let band = snapshot.band { WorkBand(id: id, band: band, claude: snapshot.claude) }
                        if let ask = snapshot.ask { AskCard(id: id, ask: ask) }
                        if let waiting = snapshot.waiting { WaitingCard(id: id, waiting: waiting) }
                        Composer(snapshot: snapshot)
                    }
                    .frame(width: column)
                    .padding(.bottom, 12)
                    .animation(.smooth, value: snapshot.band)
                    .animation(.smooth, value: snapshot.ask)
                } else {
                    ProgressView().frame(maxWidth: .infinity, maxHeight: .infinity)
                }
            }
            .frame(maxWidth: .infinity)
        }
        .background(look.bg)
        .task(id: id) { store.send("ui.chat.watch", ["chat": id]) }
        .onDisappear { store.send("ui.chat.unwatch", ["chat": id]) }
    }
}

/// The bar (chat/header.rs): the agent's mark in its square, the name in bold · the folder in monospace, the state's dot
/// and words under them; at the right Buscar na conversa and Ver no canvas. A hairline under it.
struct ChatHeader: View {
    let snapshot: ChatSnapshot
    private let store = MagoStore.shared
    @Environment(\.chatLook) private var look
    @State private var searching = false
    @State private var query = ""

    var body: some View {
        HStack(spacing: 12) {
            RoundedRectangle(cornerRadius: 8)
                .fill(snapshot.claude ? look.claudeSquare : look.hover)
                .frame(width: 28, height: 28)
                .overlay {
                    if snapshot.claude {
                        Image(systemName: "asterisk").font(.system(size: 15, weight: .bold)).foregroundStyle(ChatLook.claude)
                    } else {
                        Image(systemName: "terminal").font(.system(size: 13)).foregroundStyle(look.text)
                    }
                }
            VStack(alignment: .leading, spacing: 3) {
                HStack(spacing: 8) {
                    Thick(text: snapshot.title, size: 15, color: look.strong).lineLimit(1).layoutPriority(1)
                    Text("·").font(.app(14)).foregroundStyle(look.muted)
                    Text(snapshot.folder).font(.mono(13)).foregroundStyle(look.soft).lineLimit(1).truncationMode(.middle).help(snapshot.folder)
                }
                HStack(spacing: 6) {
                    Circle()
                        .fill(look.dot(snapshot.state))
                        .frame(width: 6, height: 6)
                        .phaseAnimator([1.0, 0.35]) { dot, phase in dot.opacity(snapshot.state == "ready" ? 1 : phase) } animation: { _ in .easeInOut(duration: 0.8) }
                    Text(snapshot.status).font(.app(13)).foregroundStyle(look.muted).lineLimit(1)
                }
            }
            Spacer(minLength: 8)
            if searching {
                HStack(spacing: 6) {
                    TextField("", text: $query, prompt: Text("Buscar na conversa").foregroundStyle(look.muted))
                        .textFieldStyle(.plain)
                        .font(.app(14))
                        .foregroundStyle(look.text)
                        .onChange(of: query) { _, value in store.send("ui.chat.search", ["chat": snapshot.id, "query": value]) }
                        .onExitCommand { close() }
                    Button(action: close) { Image(systemName: "xmark").font(.system(size: 11, weight: .semibold)).foregroundStyle(look.soft) }
                        .buttonStyle(.plain)
                }
                .padding(.horizontal, 10)
                .frame(width: 240, height: 32)
                .background(RoundedRectangle(cornerRadius: 8).fill(look.surface))
            } else {
                Button { searching = true } label: {
                    Image(systemName: "magnifyingglass").font(.system(size: 15)).foregroundStyle(look.soft).frame(width: 32, height: 32).contentShape(Rectangle())
                }
                .buttonStyle(HoverGround(radius: 8, color: look.hover))
                .help("Buscar na conversa")
            }
            Button { store.send("ui.chat.on_canvas", ["chat": snapshot.id]) } label: {
                HStack(spacing: 7) {
                    Image(systemName: "map").font(.system(size: 13))
                    Text("Ver no canvas").font(.app(13))
                }
                .foregroundStyle(look.soft)
                .padding(.horizontal, 10)
                .frame(height: 32)
                .overlay(RoundedRectangle(cornerRadius: 8).strokeBorder(look.line))
                .contentShape(Rectangle())
            }
            .buttonStyle(HoverGround(radius: 8, color: look.hover))
            .help("Ver no canvas")
        }
        .padding(.leading, 8)
        .frame(height: 56)
        .overlay(alignment: .bottom) { Rectangle().fill(look.hover).frame(height: 1) }
        .onAppear { query = snapshot.query; searching = !snapshot.query.isEmpty }
    }

    private func close() {
        query = ""
        searching = false
        store.send("ui.chat.search", ["chat": snapshot.id, "query": ""])
    }
}

/// The messages, kept at the bottom as new words come (the egui chat's `stick_to_bottom`).
struct Conversation: View {
    let snapshot: ChatSnapshot
    let width: CGFloat
    private let store = MagoStore.shared
    @Environment(\.chatLook) private var look

    var body: some View {
        ScrollViewReader { reader in
            ScrollView {
                LazyVStack(alignment: .leading, spacing: 12) {
                    if !snapshot.query.isEmpty && snapshot.items.isEmpty {
                        Text("Nenhuma mensagem com essas palavras.").font(.app(13)).foregroundStyle(look.muted)
                    }
                    if snapshot.earlier > 0 {
                        Button { store.send("ui.chat.earlier", ["chat": snapshot.id]) } label: {
                            Text("Mostrar mensagens anteriores (\(snapshot.earlier) acima)").font(.app(13)).foregroundStyle(look.muted)
                        }
                        .buttonStyle(.plain)
                    }
                    if snapshot.items.isEmpty && snapshot.fresh { Welcome(snapshot: snapshot) }
                    ForEach(snapshot.items) { item in
                        MessageRow(chat: snapshot.id, item: item, width: width)
                    }
                    if loginExpired { LoginButton(chat: snapshot.id) }
                    ForEach(Array(snapshot.queue.enumerated()), id: \.offset) { _, text in
                        QueuedBubble(chat: snapshot.id, text: text, isChat: snapshot.isChat, width: width)
                    }
                    ForEach(snapshot.team, id: \.self) { line in TeamLine(line: line) }
                    Color.clear.frame(height: 1).id("end")
                }
                .padding(.top, 16)
                .padding(.trailing, 14)
                .frame(width: width)
                .frame(maxWidth: .infinity)
            }
            .defaultScrollAnchor(.bottom)
            // ponytail: always follows the end as words stream, even when scrolled up; keep the place once macOS 15's
            // onScrollGeometryChange can tell the bottom is off screen
            .onChange(of: snapshot.items.last) { _, _ in reader.scrollTo("end", anchor: .bottom) }
            .onChange(of: snapshot.queue) { _, _ in withAnimation(.smooth) { reader.scrollTo("end", anchor: .bottom) } }
        }
    }

    /// The last message (past the system lines) is an expired login: Entrar (commands/mod.rs `login_buttons`).
    private var loginExpired: Bool {
        snapshot.isChat && snapshot.items.last(where: { $0.kind != "system" }).map { $0.kind == "error" && $0.hint != nil } == true
    }
}

private struct LoginButton: View {
    let chat: String
    @Environment(\.chatLook) private var look

    var body: some View {
        Button { MagoStore.shared.send("ui.chat.send", ["chat": chat, "text": "/login"]) } label: {
            Thick(text: "Entrar", size: 14, color: look.dark ? .black : .white)
                .frame(minWidth: 120, minHeight: 34)
                .background(Capsule().fill(look.dark ? Color.white : Color.black))
        }
        .buttonStyle(.plain)
    }
}

/// A fresh chat: which agent answers, chosen before the first message (conversation.rs `providers`).
struct Welcome: View {
    let snapshot: ChatSnapshot
    private let store = MagoStore.shared
    @Environment(\.chatLook) private var look

    var body: some View {
        VStack(spacing: 16) {
            if snapshot.claude {
                Image(systemName: "asterisk").font(.system(size: 64, weight: .bold)).foregroundStyle(ChatLook.claude).frame(height: 130)
            }
            HStack(spacing: 6) {
                ForEach(snapshot.providers) { provider in
                    Button { if !provider.on { store.send("chat_provider", ["chat": snapshot.id, "agent": provider.program]) } } label: {
                        Text(provider.name).font(.app(14)).foregroundStyle(provider.on ? look.text : look.soft)
                            .padding(.horizontal, 14)
                            .frame(height: 32)
                            .background(Capsule().fill(provider.on ? look.raised : .clear))
                            .overlay(Capsule().strokeBorder(look.line))
                            .contentShape(Capsule())
                    }
                    .buttonStyle(.plain)
                }
            }
        }
        .frame(maxWidth: .infinity)
    }
}

struct MessageRow: View {
    let chat: String
    let item: ChatItem
    let width: CGFloat
    private let store = MagoStore.shared
    @Environment(\.chatLook) private var look

    var body: some View {
        switch item.kind {
        case "user":
            // another agent's question, its answer brought back or a task it gave: said whose, in a grey bubble
            let other = item.from != nil
            VStack(alignment: .trailing, spacing: 4) {
                if let who = item.from { Text(who).font(.app(12)).foregroundStyle(look.muted) }
                Text(item.text ?? "")
                    .font(.app(prose))
                    .foregroundStyle(other ? look.words : .white)
                    .textSelection(.enabled)
                    .padding(.horizontal, 12)
                    .padding(.vertical, 7)
                    .background(RoundedRectangle(cornerRadius: 16).fill(other ? look.surfaceHigh : look.accent))
                    .frame(maxWidth: width - 48, alignment: .trailing)
                    .contextMenu { citeMenu(item.text ?? "") }
            }
            .frame(maxWidth: .infinity, alignment: .trailing)
        case "agent":
            AgentWords(chat: chat, parts: item.parts ?? [], width: width)
                .contextMenu { citeMenu(plain(item.parts ?? [])) }
        case "tools":
            ToolRun(chat: chat, item: item, width: width)
        case "divider":
            HStack(spacing: 10) {
                Rectangle().fill(look.tint(30)).frame(height: 1)
                Text(item.text ?? "").font(.app(12)).foregroundStyle(look.muted).fixedSize()
                Rectangle().fill(look.tint(30)).frame(height: 1)
            }
            .frame(height: 20)
        case "system":
            let text = item.text ?? ""
            if text.contains("\n") {
                Text(text.trimmingCharacters(in: .whitespacesAndNewlines))
                    .font(.mono(13))
                    .foregroundStyle(look.words)
                    .textSelection(.enabled)
                    .padding(10)
                    .background(RoundedRectangle(cornerRadius: 8).fill(look.surfaceHigh))
                    .frame(maxWidth: width - 20, alignment: .leading)
            } else {
                Text(text).font(.app(12)).italic().foregroundStyle(look.muted)
            }
        case "error":
            VStack(alignment: .leading, spacing: 4) {
                Text(item.text ?? "").font(.app(13)).foregroundStyle(look.danger).textSelection(.enabled)
                if let hint = item.hint { Text(hint).font(.app(13)).foregroundStyle(look.muted) }
            }
        case "image":
            Label("Imagem", systemImage: "photo")
                .font(.app(13))
                .foregroundStyle(look.muted)
                .frame(maxWidth: .infinity, alignment: item.from == nil ? .leading : .trailing)
        default:
            EmptyView()
        }
    }

    /// Copiar and Citar on a right click (list/mod.rs `message_menu`): the quote goes into the box.
    @ViewBuilder private func citeMenu(_ text: String) -> some View {
        Button("Copiar") {
            NSPasteboard.general.clearContents()
            NSPasteboard.general.setString(text, forType: .string)
        }
        Button("Citar") { store.send("ui.chat.cite", ["chat": chat, "text": text]) }
    }

    /// The agent's words without their marks, for Copiar and Citar.
    private func plain(_ parts: [ChatPart]) -> String {
        parts.map { part in
            if let paras = part.paras {
                return paras.map { para in
                    if let rows = para.rows { return rows.map { $0.map { $0.map(\.text).joined() }.joined(separator: " | ") }.joined(separator: "\n") }
                    return (para.lines ?? []).map { ($0.mark.map { "\($0) " } ?? "") + $0.runs.map(\.text).joined() }.joined(separator: "\n")
                }
                .joined(separator: "\n\n")
            }
            return part.text ?? ""
        }
        .joined(separator: "\n\n")
    }
}

/// A run of tool steps (steps.rs `tool_run`): two or more with words after them fold into their summary ("› 4
/// ferramentas · Edit, Bash ×2 · 46 s"); each step opens its card.
struct ToolRun: View {
    let chat: String
    let item: ChatItem
    let width: CGFloat
    @Environment(\.chatLook) private var look
    @State private var open: Bool?
    @State private var hovered = false

    var body: some View {
        let steps = item.steps ?? []
        let shown = open ?? !item.folded
        VStack(alignment: .leading, spacing: 6) {
            if steps.count >= 2 && item.folded {
                Button { withAnimation(.smooth(duration: 0.2)) { open = !shown } } label: {
                    HStack(spacing: 6) {
                        Image(systemName: "chevron.right").font(.system(size: 9, weight: .bold)).foregroundStyle(look.muted).rotationEffect(.degrees(shown ? 90 : 0))
                        Text(item.summary ?? "").font(.mono(12)).foregroundStyle(look.muted).lineLimit(1)
                    }
                    .padding(.leading, 5)
                    .padding(.trailing, 8)
                    .frame(height: 24)
                    .background(RoundedRectangle(cornerRadius: 6).fill(hovered ? look.surface.opacity(0.6) : .clear))
                    .contentShape(Rectangle())
                }
                .buttonStyle(.plain)
                .onHover { hovered = $0 }
            }
            if shown || steps.count < 2 || !item.folded {
                ForEach(steps) { step in StepRow(chat: chat, step: step) }
            }
            ForEach(Array((item.media ?? []).enumerated()), id: \.offset) { _, part in
                if part.kind == "image" { Picture(path: part.text ?? "") } else { FileCard(chat: chat, part: part) }
            }
        }
    }
}

/// One step (steps.rs `step_row`): ✓ (✗ failed) or a pulsing dot while it runs, its name in a 6ch column, what it was
/// given, what its answer says, the chevron at the end; a running command's last line under it. A click opens its card.
struct StepRow: View {
    let chat: String
    let step: ChatStep
    @Environment(\.chatLook) private var look
    @State private var open = false
    @State private var hovered = false

    var body: some View {
        let live = step.live != "done"
        VStack(alignment: .leading, spacing: 6) {
            Button { withAnimation(.smooth(duration: 0.2)) { open.toggle() } } label: {
                VStack(alignment: .leading, spacing: 0) {
                    HStack(spacing: 8) {
                        mark.frame(width: 13)
                        Text(step.name).font(.mono(12.5)).foregroundStyle(look.text).frame(minWidth: 46, alignment: .leading).fixedSize()
                        Text(step.what).font(.mono(12.5)).foregroundStyle(live ? look.text.opacity(0.9) : look.soft).lineLimit(1).truncationMode(.tail)
                        tail
                        Spacer(minLength: 0)
                        Image(systemName: "chevron.right").font(.system(size: 9, weight: .bold)).foregroundStyle(look.muted).rotationEffect(.degrees(open ? 90 : 0))
                    }
                    .frame(height: 18)
                    if live && !step.output.isEmpty {
                        Text(step.output).font(.mono(12)).foregroundStyle(look.muted).lineLimit(1).padding(.leading, 21).frame(height: 17)
                    }
                }
                .padding(.horizontal, 6)
                .background(RoundedRectangle(cornerRadius: 4).fill(hovered ? look.surface.opacity(0.6) : .clear).padding(.vertical, -2))
                .contentShape(Rectangle())
            }
            .buttonStyle(.plain)
            .onHover { hovered = $0 }
            if open { StepCard(chat: chat, step: step).transition(.opacity) }
        }
    }

    @ViewBuilder private var mark: some View {
        switch step.live {
        case "running": PulseDot(color: ChatLook.green, size: 7)
        case "waiting": PulseDot(color: ChatLook.amber, size: 7)
        default:
            Image(systemName: step.failed ? "xmark" : "checkmark")
                .font(.system(size: 10, weight: .heavy))
                .foregroundStyle(step.failed ? ChatLook.red : step.ended ? ChatLook.green : look.muted)
        }
    }

    @ViewBuilder private var tail: some View {
        switch step.live {
        case "running": Text("· rodando").font(.mono(12.5)).foregroundStyle(ChatLook.green).fixedSize()
        case "waiting": Text("· aguardando você").font(.mono(12.5)).foregroundStyle(ChatLook.amber).fixedSize()
        default:
            if !step.note.isEmpty {
                Text("· \(step.note)").font(.mono(12.5)).foregroundStyle(step.failed ? ChatLook.red : look.muted).lineLimit(1)
            } else if let diff = step.diff, diff.count == 2 {
                HStack(spacing: 8) {
                    Text("·").foregroundStyle(look.muted)
                    Text("+\(diff[0])").foregroundStyle(ChatLook.green)
                    Text("−\(diff[1])").foregroundStyle(ChatLook.red)
                }
                .font(.mono(12.5))
                .fixedSize()
            }
        }
    }
}

/// A step's card (steps.rs `step_card`): what it is and how it went, its folder and files (links to them in the Árvore),
/// its input and the end of what it printed in code cards (Copiar takes all of it), and why it failed.
private struct StepCard: View {
    let chat: String
    let step: ChatStep
    @Environment(\.chatLook) private var look

    var body: some View {
        VStack(alignment: .leading, spacing: 6) {
            Text(step.head).font(.mono(12)).foregroundStyle(look.muted).textSelection(.enabled)
            if !step.files.isEmpty {
                (Text("Arquivos: ").foregroundStyle(look.muted) + Text(files))
                    .font(.mono(12))
                    .tint(look.code)
                    .environment(\.openURL, OpenURLAction { url in
                        guard url.scheme == "mago-link", let index = Int(url.absoluteString.dropFirst("mago-link:".count)), let file = step.files[safe: index] else { return .systemAction }
                        MagoStore.shared.send("file_link", ["path": file, "asker": chat])
                        return .handled
                    })
            }
            if !step.input.isEmpty { CodeCard(info: step.inputLabel, code: step.input) }
            if !step.log.isEmpty {
                CodeCard(info: "saída", code: step.log, full: step.logCut ? { await full() } : nil)
            }
            if !step.error.isEmpty { Text(step.error).font(.mono(13)).foregroundStyle(ChatLook.red).textSelection(.enabled) }
        }
        .padding(.leading, 27)
        .padding(.bottom, 4)
    }

    private var files: AttributedString {
        var all = AttributedString()
        for (index, file) in step.files.enumerated() {
            if index > 0 { all += AttributedString(", ") }
            var piece = AttributedString(file)
            piece.link = URL(string: "mago-link:\(index)")
            all += piece
        }
        return all
    }

    /// The whole of what it printed (the card shows its end).
    private func full() async -> String {
        let reply = await MagoStore.shared.act("ui.chat.step_log", ["chat": chat, "step": step.id])
        return reply.ok ? reply.text : step.log
    }
}

/// A dot that breathes (list/mod.rs `pulse_dot`).
struct PulseDot: View {
    let color: Color
    let size: CGFloat

    var body: some View {
        Circle().fill(color).frame(width: size, height: size)
            .phaseAnimator([1.0, 0.35]) { dot, phase in dot.opacity(phase) } animation: { _ in .easeInOut(duration: 0.8) }
    }
}

/// A message written while the agent was busy (chat/queue.rs): "Na fila · …" with Enviar agora · Editar over its bubble
/// in the faded accent.
struct QueuedBubble: View {
    let chat: String
    let text: String
    let isChat: Bool
    let width: CGFloat
    private let store = MagoStore.shared
    @Environment(\.chatLook) private var look

    var body: some View {
        VStack(alignment: .trailing, spacing: 4) {
            HStack(spacing: 4) {
                Text(isChat ? "Na fila · vai quando ele terminar ·" : "Na fila · vai quando você responder no terminal ·").foregroundStyle(look.muted)
                Button("Enviar agora") { store.send("ui.chat.queue", ["chat": chat, "action": "send_now"]) }
                    .help(isChat ? "Interrompe a resposta e manda esta mensagem já" : "Responde Esc no terminal (recusa o pedido) e manda esta mensagem já")
                Text("·").foregroundStyle(look.muted)
                Button("Editar") { store.send("ui.chat.queue", ["chat": chat, "action": "edit"]) }
            }
            .font(.app(12))
            .buttonStyle(.plain)
            .foregroundStyle(look.soft)
            Text(text)
                .font(.app(14))
                .foregroundStyle(.white)
                .padding(.horizontal, 14)
                .padding(.vertical, 7)
                .background(RoundedRectangle(cornerRadius: 18).fill(ChatLook.blue.opacity(0.55)))
                .frame(maxWidth: width * 0.7, alignment: .trailing)
        }
        .frame(maxWidth: .infinity, alignment: .trailing)
    }
}

/// An agent this one recruited, at work now (conversation.rs `team_rows`): what it finishes comes in as its answer.
struct TeamLine: View {
    let line: String
    @Environment(\.chatLook) private var look

    var body: some View {
        HStack(spacing: 10) {
            PulseDot(color: ChatLook.green, size: 6)
            Text(line).font(.app(12.5)).foregroundStyle(look.muted).lineLimit(1)
        }
        .padding(.leading, 3)
        .frame(height: 20)
    }
}
