// One conversation (chat/mod.rs `chat_screen`): the header, the messages as they stream, the band of what the agent
// runs, the card of what it asks, the queue and the box.
import AppKit
import SwiftUI

struct ChatView: View {
    let id: String
    private let store = MagoStore.shared

    var body: some View {
        let snapshot = store.decode(ChatSnapshot.self, topic: "chat:\(id)")
        VStack(spacing: 0) {
            if let snapshot {
                ChatHeader(snapshot: snapshot)
                Divider()
                Conversation(snapshot: snapshot)
                VStack(spacing: 8) {
                    if let band = snapshot.band { WorkBand(id: id, band: band) }
                    if let ask = snapshot.ask { AskCard(id: id, ask: ask) }
                    Composer(snapshot: snapshot)
                }
                .padding(.horizontal, 16)
                .padding(.bottom, 12)
                .frame(maxWidth: 1100)
                .animation(.smooth, value: snapshot.band)
                .animation(.smooth, value: snapshot.ask)
            } else {
                ProgressView().frame(maxWidth: .infinity, maxHeight: .infinity)
            }
        }
        .background(Color(nsColor: .windowBackgroundColor))
        .task(id: id) {
            store.send("ui.chat.watch", ["chat": id])
        }
        .onDisappear { store.send("ui.chat.unwatch", ["chat": id]) }
    }
}

/// The bar: name and folder, the state's dot and words, and its buttons (Ajustes and Voltar ao canvas on the Mac).
struct ChatHeader: View {
    let snapshot: ChatSnapshot
    private let store = MagoStore.shared

    var body: some View {
        HStack(spacing: 10) {
            VStack(alignment: .leading, spacing: 2) {
                HStack(spacing: 6) {
                    Text(snapshot.title).font(.headline).lineLimit(1)
                    Text(snapshot.folder).font(.callout.monospaced()).foregroundStyle(.secondary).lineLimit(1).truncationMode(.middle)
                }
                HStack(spacing: 6) {
                    Circle()
                        .fill(stateColor(snapshot.state))
                        .frame(width: 6, height: 6)
                        .phaseAnimator([1.0, 0.4]) { dot, phase in dot.opacity(snapshot.state == "ready" ? 1 : phase) } animation: { _ in .easeInOut(duration: 0.8) }
                    Text(snapshot.status).font(.caption).foregroundStyle(.secondary).lineLimit(1)
                }
            }
            Spacer()
            if snapshot.isChat {
                Button { store.send("mini_chat", ["chat": snapshot.id, "state": "open"]) } label: { Image(systemName: "pip") }
                    .help("Abrir em miniatura")
            }
            Button("Ver no canvas") { store.send("ui.chat.on_canvas", ["chat": snapshot.id]) }
            Button { store.send("ui.chat.settings") } label: { Image(systemName: "gearshape") }
                .help("Ajustes")
            Button { store.send("chats_view", ["state": "close"]) } label: { Image(systemName: "square.on.square") }
                .help("Voltar ao canvas")
        }
        .buttonStyle(.borderless)
        .padding(.horizontal, 16)
        .frame(height: 56)
    }
}

/// The messages, kept at the bottom as new words come (the egui chat's `stick_to_bottom`).
struct Conversation: View {
    let snapshot: ChatSnapshot
    private let store = MagoStore.shared

    var body: some View {
        ScrollViewReader { reader in
            ScrollView {
                LazyVStack(alignment: .leading, spacing: 12) {
                    if snapshot.earlier > 0 {
                        Button("Mostrar mensagens anteriores (\(snapshot.earlier) acima)") { store.send("ui.chat.earlier", ["chat": snapshot.id]) }
                            .buttonStyle(.borderless)
                            .foregroundStyle(.secondary)
                    }
                    if snapshot.items.isEmpty && snapshot.fresh { Welcome(snapshot: snapshot) }
                    ForEach(snapshot.items) { item in
                        MessageRow(chat: snapshot.id, item: item)
                            .transition(.opacity.combined(with: .move(edge: .bottom)))
                    }
                    ForEach(Array(snapshot.queue.enumerated()), id: \.offset) { _, text in
                        QueuedBubble(chat: snapshot.id, text: text, isChat: snapshot.isChat)
                    }
                    Color.clear.frame(height: 1).id("end")
                }
                .frame(maxWidth: 1100)
                .padding(.horizontal, 32)
                .padding(.vertical, 16)
                .frame(maxWidth: .infinity)
                .animation(.smooth(duration: 0.25), value: snapshot.items)
            }
            .defaultScrollAnchor(.bottom)
            // ponytail: always follows the end as words stream, even when scrolled up; keep the place once macOS 15's
            // onScrollGeometryChange can tell the bottom is off screen
            .onChange(of: snapshot.items.last) { _, _ in
                withAnimation(.smooth) { reader.scrollTo("end", anchor: .bottom) }
            }
            .onChange(of: snapshot.queue) { _, _ in
                withAnimation(.smooth) { reader.scrollTo("end", anchor: .bottom) }
            }
        }
    }
}

/// A fresh chat: which agent answers, chosen before the first message.
struct Welcome: View {
    let snapshot: ChatSnapshot
    private let store = MagoStore.shared

    var body: some View {
        VStack(spacing: 16) {
            Image(systemName: "sparkles").font(.system(size: 48)).foregroundStyle(.secondary)
            HStack(spacing: 6) {
                ForEach(snapshot.providers) { provider in
                    Button(provider.name) {
                        if !provider.on { store.send("chat_provider", ["chat": snapshot.id, "agent": provider.program]) }
                    }
                    .buttonStyle(.bordered)
                    .tint(provider.on ? .accentColor : .secondary)
                }
            }
        }
        .frame(maxWidth: .infinity)
        .padding(.top, 40)
    }
}

struct MessageRow: View {
    let chat: String
    let item: ChatItem
    private let store = MagoStore.shared

    var body: some View {
        switch item.kind {
        case "user":
            VStack(alignment: .trailing, spacing: 4) {
                if let who = item.from, !who.isEmpty { Text("Pergunta de \(who)").font(.caption).foregroundStyle(.secondary) }
                Text(item.text ?? "")
                    .textSelection(.enabled)
                    .padding(.horizontal, 12)
                    .padding(.vertical, 7)
                    .foregroundStyle(item.from == nil ? .white : .primary)
                    .background(RoundedRectangle(cornerRadius: 16).fill(item.from == nil ? Color.accentColor : Color.secondary.opacity(0.15)))
                    .contextMenu { citeMenu(item.text ?? "") }
            }
            .frame(maxWidth: .infinity, alignment: .trailing)
            .padding(.leading, 48)
        case "agent":
            AgentWords(parts: item.parts ?? [])
                .contextMenu { citeMenu((item.parts ?? []).map(\.text).joined(separator: "\n\n")) }
        case "tools":
            ToolRun(item: item)
        case "divider":
            HStack {
                VStack { Divider() }
                Text(item.text ?? "").font(.caption).foregroundStyle(.secondary)
                VStack { Divider() }
            }
        case "system":
            let text = item.text ?? ""
            if text.contains("\n") {
                Text(text.trimmingCharacters(in: .newlines))
                    .font(.system(size: 13, design: .monospaced))
                    .textSelection(.enabled)
                    .padding(10)
                    .frame(maxWidth: .infinity, alignment: .leading)
                    .background(RoundedRectangle(cornerRadius: 8).fill(Color.secondary.opacity(0.12)))
            } else {
                Text(text).font(.caption).italic().foregroundStyle(.secondary)
            }
        case "error":
            VStack(alignment: .leading, spacing: 4) {
                Label(item.text ?? "", systemImage: "exclamationmark.triangle").foregroundStyle(.red).textSelection(.enabled)
                if let hint = item.hint {
                    Text(hint).font(.callout).foregroundStyle(.secondary)
                    Button("Entrar") { store.send("chat_command", ["chat": chat, "command": "login"]) }
                }
            }
        case "image":
            Label("Imagem", systemImage: "photo")
                .foregroundStyle(.secondary)
                .frame(maxWidth: .infinity, alignment: item.from == nil ? .leading : .trailing)
        default:
            EmptyView()
        }
    }

    /// Copiar and Citar (the quote goes into the box, as the egui chat's right-click menu).
    @ViewBuilder private func citeMenu(_ text: String) -> some View {
        Button("Copiar") {
            NSPasteboard.general.clearContents()
            NSPasteboard.general.setString(text, forType: .string)
        }
        Button("Citar") { store.send("ui.chat.cite", ["chat": chat, "text": text]) }
    }
}

/// The agent's words: markdown, code blocks with Copiar, pictures and files by name.
struct AgentWords: View {
    let parts: [ChatPart]

    var body: some View {
        VStack(alignment: .leading, spacing: 8) {
            ForEach(Array(parts.enumerated()), id: \.offset) { _, part in
                switch part.kind {
                case "code":
                    CodeBlock(info: part.info ?? "", code: part.text)
                case "image", "file":
                    Label(part.info?.isEmpty == false ? part.info! : part.text, systemImage: part.kind == "image" ? "photo" : "doc")
                        .foregroundStyle(.secondary)
                        .help(part.text)
                default:
                    Text(markdown(part.text)).textSelection(.enabled).frame(maxWidth: .infinity, alignment: .leading)
                }
            }
        }
    }

    private func markdown(_ text: String) -> AttributedString {
        // ponytail: inline markdown only (bold, italics, code, links); headings and lists read as their plain lines
        (try? AttributedString(markdown: text, options: .init(interpretedSyntax: .inlineOnlyPreservingWhitespace))) ?? AttributedString(text)
    }
}

struct CodeBlock: View {
    let info: String
    let code: String

    var body: some View {
        VStack(alignment: .leading, spacing: 0) {
            HStack {
                Text(info).font(.caption).foregroundStyle(.secondary)
                Spacer()
                Button("Copiar") {
                    NSPasteboard.general.clearContents()
                    NSPasteboard.general.setString(code, forType: .string)
                }
                .buttonStyle(.borderless)
                .font(.caption)
            }
            .padding(.horizontal, 10)
            .padding(.vertical, 4)
            ScrollView(.horizontal) {
                Text(code).font(.system(size: 13, design: .monospaced)).textSelection(.enabled).padding(10)
            }
        }
        .background(RoundedRectangle(cornerRadius: 8).fill(Color.secondary.opacity(0.12)))
    }
}

/// A run of tool steps: folded into its summary when it is done and words came after it, each step opening its card.
struct ToolRun: View {
    let item: ChatItem
    @State private var open: Bool?

    var body: some View {
        let steps = item.steps ?? []
        if item.folded || open == false {
            DisclosureGroup(isExpanded: Binding(get: { open ?? !item.folded }, set: { open = $0 })) {
                stepList(steps)
            } label: {
                Text(item.summary ?? "").font(.system(size: 12, design: .monospaced)).foregroundStyle(.secondary)
            }
        } else {
            stepList(steps)
        }
    }

    private func stepList(_ steps: [ChatStep]) -> some View {
        VStack(alignment: .leading, spacing: 6) {
            ForEach(steps) { step in StepRow(step: step) }
        }
    }
}

struct StepRow: View {
    let step: ChatStep
    @State private var open = false

    var body: some View {
        VStack(alignment: .leading, spacing: 4) {
            Button { withAnimation(.smooth) { open.toggle() } } label: {
                HStack(spacing: 6) {
                    if step.running {
                        ProgressView().controlSize(.mini)
                    } else {
                        Image(systemName: step.failed ? "xmark.circle" : "checkmark.circle").foregroundStyle(step.failed ? .red : .green)
                    }
                    Text(step.name).font(.system(size: 12, weight: .semibold, design: .monospaced))
                    Text(step.what).font(.system(size: 12, design: .monospaced)).foregroundStyle(.secondary).lineLimit(1).truncationMode(.middle)
                    if let diff = step.diff, diff.count == 2 {
                        Text("+\(diff[0])").foregroundStyle(.green).font(.caption.monospaced())
                        Text("-\(diff[1])").foregroundStyle(.red).font(.caption.monospaced())
                    } else if !step.note.isEmpty {
                        Text(step.note).font(.caption).foregroundStyle(.secondary)
                    }
                }
            }
            .buttonStyle(.plain)
            if open {
                VStack(alignment: .leading, spacing: 6) {
                    Text(step.input).font(.system(size: 12, design: .monospaced)).textSelection(.enabled)
                    if !step.output.isEmpty { Divider(); Text(step.output).font(.system(size: 12, design: .monospaced)).foregroundStyle(.secondary).textSelection(.enabled) }
                    if !step.error.isEmpty { Text(step.error).font(.caption).foregroundStyle(.red).textSelection(.enabled) }
                }
                .padding(10)
                .frame(maxWidth: .infinity, alignment: .leading)
                .background(RoundedRectangle(cornerRadius: 8).fill(Color.secondary.opacity(0.1)))
                .transition(.opacity)
            }
        }
    }
}

/// A message written while the agent was busy, at the conversation's end: Enviar agora or Editar.
struct QueuedBubble: View {
    let chat: String
    let text: String
    let isChat: Bool
    private let store = MagoStore.shared

    var body: some View {
        VStack(alignment: .trailing, spacing: 4) {
            HStack(spacing: 6) {
                Text(isChat ? "Na fila · vai quando ele terminar ·" : "Na fila · vai quando você responder no terminal ·")
                    .foregroundStyle(.secondary)
                Button("Enviar agora") { store.send("ui.chat.queue", ["chat": chat, "action": "send_now"]) }
                    .help(isChat ? "Interrompe a resposta e manda esta mensagem já" : "Responde Esc no terminal (recusa o pedido) e manda esta mensagem já")
                Text("·").foregroundStyle(.secondary)
                Button("Editar") { store.send("ui.chat.queue", ["chat": chat, "action": "edit"]) }
            }
            .font(.caption)
            .buttonStyle(.borderless)
            Text(text)
                .padding(.horizontal, 14)
                .padding(.vertical, 7)
                .foregroundStyle(.white)
                .background(RoundedRectangle(cornerRadius: 18).fill(Color.accentColor.opacity(0.55)))
        }
        .frame(maxWidth: .infinity, alignment: .trailing)
    }
}
