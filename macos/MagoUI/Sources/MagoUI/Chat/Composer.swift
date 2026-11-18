// The box (chat/composer.rs): the words, Enter sends and Shift+Enter breaks the line, the slash menu over it, and under
// them the chat's chips (chat/footer.rs) and send. The band of what the agent runs and the card of what it asks sit over it.
import AppKit
import SwiftUI

struct Composer: View {
    let snapshot: ChatSnapshot
    private let store = MagoStore.shared
    @State private var text = ""
    @State private var seq: UInt64 = 0
    @State private var selected = 0
    @FocusState private var focused: Bool

    var body: some View {
        VStack(alignment: .leading, spacing: 6) {
            if !snapshot.slash.isEmpty { slashMenu }
            VStack(alignment: .leading, spacing: 10) {
                ZStack(alignment: .topLeading) {
                    if text.isEmpty {
                        Text(snapshot.hint).foregroundStyle(.tertiary).padding(.leading, 5).allowsHitTesting(false)
                    }
                    TextEditor(text: $text)
                        .font(.system(size: 15))
                        .scrollContentBackground(.hidden)
                        .frame(minHeight: 40, maxHeight: 200)
                        .fixedSize(horizontal: false, vertical: true)
                        .focused($focused)
                        .onKeyPress(phases: .down) { press in key(press) }
                }
                HStack(spacing: 8) {
                    if let chips = snapshot.chips { ChipsRow(chat: snapshot.id, chips: chips) }
                    Spacer()
                    Button(action: send) {
                        Image(systemName: "arrow.up")
                            .font(.system(size: 14, weight: .bold))
                            .frame(width: 32, height: 32)
                            .foregroundStyle(ready ? Color(nsColor: .windowBackgroundColor) : .secondary)
                            .background(Circle().fill(ready ? Color.primary : Color.secondary.opacity(0.2)))
                    }
                    .buttonStyle(.plain)
                    .disabled(!ready)
                }
            }
            .padding(EdgeInsets(top: 12, leading: 16, bottom: 10, trailing: 12))
            .background(RoundedRectangle(cornerRadius: 18).fill(Color(nsColor: .controlBackgroundColor)))
            .overlay(RoundedRectangle(cornerRadius: 18).stroke(Color.secondary.opacity(0.25)))
        }
        .onAppear {
            text = snapshot.draft
            seq = snapshot.draftSeq
            focused = true
        }
        // Rust changed the words (Editar, Citar, a refused send): the box takes them
        .onChange(of: snapshot.draftSeq) { _, value in
            if value != seq {
                seq = value
                text = snapshot.draft
            }
        }
        .onChange(of: text) { _, value in
            if value != snapshot.draft { store.send("ui.chat.draft", ["chat": snapshot.id, "text": value]) }
            selected = 0
        }
    }

    private var ready: Bool { !text.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty }

    private func send() {
        guard ready else { return }
        store.send("ui.chat.send", ["chat": snapshot.id, "text": text])
        withAnimation(.smooth) { text = "" }
    }

    /// Enter sends, Shift+Enter breaks the line; with the slash menu open the arrows, Tab and Enter are its; Esc stops the
    /// agent while it works, else goes back to the canvas.
    private func key(_ press: KeyPress) -> KeyPress.Result {
        let menu = snapshot.slash
        switch press.key {
        case .downArrow where !menu.isEmpty:
            selected = (selected + 1) % menu.count
        case .upArrow where !menu.isEmpty:
            selected = (selected + menu.count - 1) % menu.count
        case .tab where !menu.isEmpty:
            complete(menu[min(selected, menu.count - 1)])
        case .return where press.modifiers.contains(.shift):
            return .ignored
        case .return where !menu.isEmpty:
            complete(menu[min(selected, menu.count - 1)])
        case .return:
            send()
        case .escape where snapshot.state == "working":
            store.send("ui.chat.stop", ["chat": snapshot.id])
        case .escape:
            store.send("chats_view", ["state": "close"])
        default:
            return .ignored
        }
        return .handled
    }

    private func complete(_ entry: SlashEntry) {
        text = "/\(entry.name)" + (entry.args.isEmpty ? "" : " ")
    }

    private var slashMenu: some View {
        VStack(alignment: .leading, spacing: 0) {
            ForEach(Array(snapshot.slash.enumerated()), id: \.element.id) { index, entry in
                Button { complete(entry) } label: {
                    HStack {
                        Text("/\(entry.name)").font(.system(size: 13, design: .monospaced))
                        if !entry.args.isEmpty { Text(entry.args).font(.caption.monospaced()).foregroundStyle(.secondary) }
                        Spacer()
                        Text(entry.desc).font(.caption).foregroundStyle(.secondary).lineLimit(1)
                    }
                    .padding(.horizontal, 10)
                    .padding(.vertical, 5)
                    .background(RoundedRectangle(cornerRadius: 6).fill(index == selected ? Color.accentColor.opacity(0.2) : .clear))
                    .contentShape(Rectangle())
                }
                .buttonStyle(.plain)
            }
        }
        .padding(4)
        .background(RoundedRectangle(cornerRadius: 10).fill(Color(nsColor: .controlBackgroundColor)).shadow(radius: 6))
        .transition(.opacity.combined(with: .move(edge: .bottom)))
    }
}

/// The chips: folder (locked once the conversation started), permission, and the model with its effort.
struct ChipsRow: View {
    let chat: String
    let chips: ChatChips
    private let store = MagoStore.shared

    var body: some View {
        HStack(spacing: 6) {
            Menu {
                Button(checked("Nenhuma pasta", chips.folderPath == chips.noFolder)) { footer(["folder": chips.noFolder]) }
                if !chips.recent.isEmpty {
                    Section("Recente") {
                        ForEach(chips.recent, id: \.self) { pair in
                            Button(checked(pair.first ?? "", pair.last == chips.folderPath)) { footer(["folder": pair.last ?? ""]) }
                                .help(pair.last ?? "")
                        }
                    }
                }
                Divider()
                Button("Abrir pasta…") { pickFolder() }
            } label: {
                Label(chips.folder, systemImage: "folder")
            }
            .disabled(chips.folderLocked)
            .help(chips.folderLocked ? "Esta conversa já começou na pasta dela: para usar outra pasta, abra uma conversa nova." : chips.folderPath)
            Menu {
                ForEach(Array(chips.modes.enumerated()), id: \.offset) { index, mode in
                    Button(checked(mode, index == chips.mode)) { footer(["mode": index]) }
                }
            } label: {
                Label(chips.modes[min(chips.mode, chips.modes.count - 1)], systemImage: "checkmark.shield")
            }
            Menu {
                Section("Modelo") {
                    ForEach(chips.models) { model in
                        Button(checked(model.name, model.id == chips.model)) { footer(["model": model.id]) }
                    }
                }
                if !chips.more.isEmpty {
                    Menu("Mais modelos") {
                        ForEach(chips.more) { model in
                            Button(checked(model.name, model.id == chips.model)) { footer(["model": model.id]) }
                        }
                    }
                }
                if !chips.efforts.isEmpty {
                    Section("Esforço") {
                        ForEach(chips.efforts) { effort in
                            Button(checked(effort.name, effort.id == chips.effort)) { footer(["effort": effort.id]) }
                        }
                    }
                }
            } label: {
                Text(chips.tuning)
            }
        }
        .menuStyle(.borderlessButton)
        .fixedSize()
        .font(.system(size: 13))
    }

    private func checked(_ name: String, _ on: Bool) -> String { on ? "✓ \(name)" : name }

    private func footer(_ args: [String: Any]) {
        var all = args
        all["chat"] = chat
        store.send("ui.chat.footer", all)
    }

    /// Abrir pasta…: the system's panel, as rfd's in the egui chat.
    private func pickFolder() {
        let panel = NSOpenPanel()
        panel.title = "Pasta de trabalho"
        panel.canChooseFiles = false
        panel.canChooseDirectories = true
        if panel.runModal() == .OK, let url = panel.url { footer(["folder": url.path]) }
    }
}

/// While the agent works: the tool running and its command, a bar when its output counts, the time and Interromper.
struct WorkBand: View {
    let id: String
    let band: ChatBand
    private let store = MagoStore.shared

    var body: some View {
        HStack(spacing: 10) {
            ProgressView().controlSize(.small)
            if band.tool.isEmpty {
                Text(band.thinking)
                    .foregroundStyle(.orange)
                    .phaseAnimator([0.55, 1.0]) { line, phase in line.opacity(phase) } animation: { _ in .easeInOut(duration: 1.1) }
                    .contentTransition(.opacity)
                    .animation(.smooth, value: band.thinking)
            } else {
                Text(band.tool)
                    .font(.system(size: 12, design: .monospaced))
                    .padding(.horizontal, 8)
                    .padding(.vertical, 3)
                    .background(RoundedRectangle(cornerRadius: 6).fill(Color.secondary.opacity(0.15)))
                Text(band.what).font(.system(size: 12, design: .monospaced)).foregroundStyle(.secondary).lineLimit(1).truncationMode(.middle)
            }
            Spacer()
            if let counter = band.counter, counter.count == 2, counter[1] > 0 {
                ProgressView(value: Double(counter[0]), total: Double(counter[1])).frame(width: 120)
                Text("\(counter[0])/\(counter[1])").font(.caption).foregroundStyle(.secondary)
            }
            Text(String(format: "%d:%02d", band.seconds / 60, band.seconds % 60)).font(.caption.monospaced()).foregroundStyle(.secondary)
            Button {
                store.send("ui.chat.stop", ["chat": id])
            } label: {
                Label("Interromper", systemImage: "stop.fill")
            }
            .help("Interromper (Esc)")
        }
        .padding(.horizontal, 14)
        .padding(.vertical, 8)
        .background(RoundedRectangle(cornerRadius: 12).fill(Color.secondary.opacity(0.08)))
        .overlay(RoundedRectangle(cornerRadius: 12).stroke(Color.secondary.opacity(0.2)))
        .transition(.opacity.combined(with: .move(edge: .bottom)))
    }
}

/// What the chat's agent asks the person (chat/decide.rs): its questions with options, or a permission.
struct AskCard: View {
    let id: String
    let ask: ChatAsk
    private let store = MagoStore.shared
    @State private var answers: [String: String] = [:]

    var body: some View {
        VStack(alignment: .leading, spacing: 10) {
            if ask.questions.isEmpty {
                Text("\(ask.tool) pede permissão").font(.headline)
                HStack {
                    Button("Permitir") { store.send("chat_decide", ["chat": id, "allow": true]) }.keyboardShortcut(.defaultAction)
                    Button("Negar") { store.send("chat_decide", ["chat": id, "allow": false]) }
                }
            } else {
                ForEach(ask.questions) { question in
                    VStack(alignment: .leading, spacing: 6) {
                        if !question.header.isEmpty { Text(question.header).font(.caption).foregroundStyle(.secondary) }
                        Text(question.text).font(.headline)
                        ForEach(question.options, id: \.self) { option in
                            let label = option.first ?? ""
                            Button {
                                answers[question.text] = label
                            } label: {
                                HStack {
                                    Image(systemName: answers[question.text] == label ? "largecircle.fill.circle" : "circle")
                                    VStack(alignment: .leading) {
                                        Text(label)
                                        if let note = option.last, option.count > 1, !note.isEmpty { Text(note).font(.caption).foregroundStyle(.secondary) }
                                    }
                                }
                            }
                            .buttonStyle(.plain)
                        }
                    }
                }
                HStack {
                    Button("Responder") { store.send("chat_decide", ["chat": id, "answers": answers]) }
                        .disabled(ask.questions.contains { answers[$0.text] == nil })
                        .keyboardShortcut(.defaultAction)
                    Button("Recusar") { store.send("chat_decide", ["chat": id, "allow": false]) }
                }
            }
        }
        .padding(14)
        .frame(maxWidth: .infinity, alignment: .leading)
        .background(RoundedRectangle(cornerRadius: 12).fill(Color.orange.opacity(0.1)))
        .overlay(RoundedRectangle(cornerRadius: 12).stroke(Color.orange.opacity(0.4)))
        .transition(.opacity.combined(with: .move(edge: .bottom)))
    }
}
