// The box (chat/composer.rs): a card of radius 18 with the words (15 pt, Enter sends, Shift+Enter breaks the line), the
// pictures that go with them, the slash menu over it, and under the words + (Anexar imagem), the chat's chips
// (chat/footer.rs) and send. No microphone and no call: the chat has no voice (voice::DICTATION_ON and CALL_ON).
import AppKit
import SwiftUI
import UniformTypeIdentifiers

struct Composer: View {
    let snapshot: ChatSnapshot
    private let store = MagoStore.shared
    @Environment(\.chatLook) private var look
    @State private var text = ""
    @State private var seq: UInt64 = 0
    @State private var selected = 0
    @FocusState private var focused: Bool

    var body: some View {
        VStack(alignment: .leading, spacing: 6) {
            if !snapshot.slash.isEmpty { slashMenu }
            VStack(alignment: .leading, spacing: 10) {
                if !snapshot.attached.isEmpty { attachedRow }
                ZStack(alignment: .topLeading) {
                    if text.isEmpty {
                        Text(snapshot.hint).font(.app(15)).foregroundStyle(look.muted).padding(.leading, 5).allowsHitTesting(false)
                    }
                    TextEditor(text: $text)
                        .font(.app(15))
                        .foregroundStyle(look.text)
                        .scrollContentBackground(.hidden)
                        .frame(minHeight: 40, maxHeight: 200)
                        .fixedSize(horizontal: false, vertical: true)
                        .focused($focused)
                        .onKeyPress(phases: .down) { press in key(press) }
                }
                HStack(spacing: 6) {
                    Button(action: attach) {
                        Image(systemName: "plus").font(.system(size: 15, weight: .medium)).foregroundStyle(look.soft).frame(width: 32, height: 32).contentShape(Circle())
                    }
                    .buttonStyle(HoverGround(radius: 16, color: look.hover))
                    .help("Anexar imagem")
                    if let chips = snapshot.chips { ChipsRow(chat: snapshot.id, chips: chips) } else { Spacer() }
                    Button(action: send) {
                        Image(systemName: "arrow.up")
                            .font(.system(size: 15, weight: .bold))
                            .foregroundStyle(ready ? look.bg : look.muted)
                            .frame(width: 32, height: 32)
                            .background(Circle().fill(ready ? look.text : look.raised))
                    }
                    .buttonStyle(.plain)
                    .disabled(!ready)
                }
                .frame(height: 32)
            }
            .padding(EdgeInsets(top: 12, leading: 16, bottom: 10, trailing: 12))
            .background(RoundedRectangle(cornerRadius: 18).fill(look.panel))
            .overlay(RoundedRectangle(cornerRadius: 18).strokeBorder(look.line))
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

    private var ready: Bool { !text.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty || !snapshot.attached.isEmpty }

    private func send() {
        guard ready else { return }
        store.send("ui.chat.send", ["chat": snapshot.id, "text": text])
        text = ""
    }

    /// + : the system's panel for pictures, as rfd's in the egui chat (images.rs `chat_attach_pick`).
    private func attach() {
        let panel = NSOpenPanel()
        panel.title = "Anexar imagem"
        panel.allowsMultipleSelection = true
        panel.canChooseDirectories = false
        panel.allowedContentTypes = [.image]
        if panel.runModal() == .OK { store.send("ui.chat.attach", ["paths": panel.urls.map(\.path)]) }
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

    /// The pictures that go with the next message (images.rs `attached_row`): each with where it came from and a × to take
    /// it out; their count and weight at the right.
    private var attachedRow: some View {
        HStack(alignment: .bottom, spacing: 12) {
            ForEach(Array(snapshot.attached.enumerated()), id: \.offset) { index, pair in
                VStack(alignment: .leading, spacing: 5) {
                    Group {
                        if let image = Picture.load(pair.last ?? "") { Image(nsImage: image).resizable().scaledToFill() } else { look.surface }
                    }
                    .frame(width: 96, height: 72)
                    .clipShape(RoundedRectangle(cornerRadius: 9))
                    .overlay(RoundedRectangle(cornerRadius: 9).strokeBorder(look.edge))
                    .overlay(alignment: .topTrailing) {
                        Button { store.send("ui.chat.unattach", ["index": index]) } label: {
                            Image(systemName: "xmark").font(.system(size: 8, weight: .bold)).foregroundStyle(Color(rgb: 0xececec))
                                .frame(width: 20, height: 20)
                                .background(Circle().fill(Color(rgb: 0x121212)).overlay(Circle().strokeBorder(look.surfaceLow, lineWidth: 2)))
                        }
                        .buttonStyle(.plain)
                        .offset(x: 6, y: -6)
                        .help("Remover")
                    }
                    Text(pair.first ?? "").font(.app(11.5)).foregroundStyle(look.words).lineLimit(1).frame(width: 96, alignment: .leading)
                }
            }
            Spacer()
            Text(snapshot.attachedTotal).font(.app(13)).foregroundStyle(look.muted)
        }
        .padding(.top, 8)
    }

    private var slashMenu: some View {
        VStack(alignment: .leading, spacing: 0) {
            ForEach(Array(snapshot.slash.enumerated()), id: \.element.id) { index, entry in
                Button { complete(entry) } label: {
                    HStack(spacing: 8) {
                        Text("/\(entry.name)").font(.mono(13)).foregroundStyle(look.text)
                        if !entry.args.isEmpty { Text(entry.args).font(.mono(12)).foregroundStyle(look.muted) }
                        Spacer()
                        Text(entry.desc).font(.app(12.5)).foregroundStyle(look.muted).lineLimit(1)
                    }
                    .padding(.horizontal, 10)
                    .frame(height: 28)
                    .background(RoundedRectangle(cornerRadius: 6).fill(index == selected ? look.hover : .clear))
                    .contentShape(Rectangle())
                }
                .buttonStyle(.plain)
            }
        }
        .padding(4)
        .background(RoundedRectangle(cornerRadius: 10).fill(look.panel))
        .overlay(RoundedRectangle(cornerRadius: 10).strokeBorder(look.line))
        .transition(.opacity.combined(with: .move(edge: .bottom)))
    }
}

/// The chips (footer.rs `chat_footer`): the folder (locked once the conversation started) and the permission outlined at
/// the left, the model with its effort at the right; each opens its menu.
struct ChipsRow: View {
    let chat: String
    let chips: ChatChips
    private let store = MagoStore.shared
    @Environment(\.chatLook) private var look

    var body: some View {
        HStack(spacing: 6) {
            Menu {
                Button(checked("Nenhuma pasta", chips.folderPath == chips.noFolder)) { footer(["folder": chips.noFolder]) }
                if !chips.recent.isEmpty {
                    Section("Recente") {
                        ForEach(chips.recent, id: \.self) { pair in
                            Button(checked(pair.first ?? "", pair.last == chips.folderPath)) { footer(["folder": pair.last ?? ""]) }
                        }
                    }
                }
                Divider()
                Button("Abrir pasta…") { pickFolder() }
            } label: {
                chip(icon: "folder", text: chips.folder, enabled: !chips.folderLocked)
            }
            .disabled(chips.folderLocked)
            .help(chips.folderLocked ? "Esta conversa já começou na pasta dela: para usar outra pasta, abra uma conversa nova." : chips.folderPath)
            Menu {
                ForEach(Array(chips.modes.enumerated()), id: \.offset) { index, mode in
                    Button(checked(mode, index == chips.mode)) { footer(["mode": index]) }
                }
            } label: {
                chip(icon: "checkmark", text: chips.modes[safe: chips.mode] ?? "", enabled: true)
            }
            Spacer(minLength: 6)
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
                HStack(spacing: 4) {
                    Text(chips.tuning).font(.app(13))
                    Image(systemName: "chevron.down").font(.system(size: 9, weight: .bold))
                }
                .foregroundStyle(look.soft)
                .padding(.leading, 10)
                .padding(.trailing, 8)
                .frame(height: 28)
                .contentShape(Capsule())
            }
        }
        .menuStyle(.button)
        .buttonStyle(HoverGround(radius: 14, color: look.hover))
        .menuIndicator(.hidden)
        .fixedSize(horizontal: false, vertical: true)
    }

    private func chip(icon: String, text: String, enabled: Bool) -> some View {
        HStack(spacing: 6) {
            Image(systemName: icon).font(.system(size: 11, weight: .semibold))
            Text(text).font(.app(13)).lineLimit(1)
        }
        .foregroundStyle(enabled ? look.soft : look.muted)
        .padding(.leading, 10)
        .padding(.trailing, 10)
        .frame(height: 28)
        .overlay(Capsule().strokeBorder(look.line))
        .contentShape(Capsule())
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

/// While the agent works (steps.rs `work_band`): the busy dots, the tool's chip and its command (or the thinking line),
/// a bar when its output counts, how long it runs and Interromper (Esc).
struct WorkBand: View {
    let id: String
    let band: ChatBand
    let claude: Bool
    private let store = MagoStore.shared
    @Environment(\.chatLook) private var look

    var body: some View {
        HStack(spacing: 12) {
            BusyDots()
            if band.tool.isEmpty {
                Text(band.thinking)
                    .font(.app(14))
                    .foregroundStyle(claude ? ChatLook.claude : look.text)
                    .lineLimit(1)
                    .phaseAnimator([0.55, 1.0]) { line, phase in line.opacity(phase) } animation: { _ in .easeInOut(duration: 1.1) }
            } else {
                HStack(spacing: 6) {
                    Image(systemName: toolIcon(band.tool)).font(.system(size: 10, weight: .semibold))
                    Text(band.tool).font(.mono(12))
                }
                .foregroundStyle(look.text)
                .padding(.horizontal, 8)
                .frame(height: 22)
                .background(RoundedRectangle(cornerRadius: 6).fill(look.raised))
                Text(band.what).font(.mono(12)).foregroundStyle(look.soft).lineLimit(1).truncationMode(.tail)
            }
            Spacer(minLength: 0)
            if let counter = band.counter, counter.count == 2, counter[1] > 0 {
                ZStack(alignment: .leading) {
                    Capsule().fill(look.line)
                    Capsule().fill(ChatLook.blue).frame(width: 120 * CGFloat(counter[0]) / CGFloat(counter[1]))
                }
                .frame(width: 120, height: 4)
                Text("\(counter[0])/\(counter[1])").font(.app(12.5)).foregroundStyle(look.soft)
            }
            Text(String(format: "%d:%02d", band.seconds / 60, band.seconds % 60)).font(.mono(12)).foregroundStyle(look.muted)
            Button { store.send("ui.chat.stop", ["chat": id]) } label: {
                HStack(spacing: 7) {
                    RoundedRectangle(cornerRadius: 1.5).fill(look.text).frame(width: 7, height: 7).padding(.leading, 3)
                    Text("Interromper").font(.app(14)).foregroundStyle(look.text)
                    Text("Esc").font(.mono(11)).foregroundStyle(look.soft).padding(.horizontal, 5).frame(height: 16)
                        .overlay(RoundedRectangle(cornerRadius: 4).strokeBorder(look.line))
                }
                .padding(.horizontal, 10)
                .frame(height: 28)
                .background(RoundedRectangle(cornerRadius: 8).fill(look.raised))
                .contentShape(Rectangle())
            }
            .buttonStyle(HoverGround(radius: 8, color: look.selected))
            .help("Interromper (Esc)")
        }
        .frame(height: 28)
        .padding(EdgeInsets(top: 6, leading: 14, bottom: 6, trailing: 6))
        .background(RoundedRectangle(cornerRadius: 12).fill(look.band))
        .overlay(RoundedRectangle(cornerRadius: 12).strokeBorder(look.line))
        .transition(.opacity.combined(with: .move(edge: .bottom)))
    }

    private func toolIcon(_ tool: String) -> String {
        switch tool {
        case "Bash": "terminal"
        case "Read": "doc.text"
        case "Edit", "Write", "MultiEdit", "NotebookEdit": "pencil"
        case "Grep", "Glob", "ToolSearch": "magnifyingglass"
        case "WebFetch", "WebSearch": "globe"
        case "Task", "Agent": "person.2"
        default: "wrench.and.screwdriver"
        }
    }
}

/// Three green dots taking turns (steps.rs `busy_dots`).
struct BusyDots: View {
    var body: some View {
        TimelineView(.animation) { context in
            let time = context.date.timeIntervalSinceReferenceDate
            HStack(spacing: 3) {
                ForEach(0..<3) { index in
                    Circle().fill(ChatLook.green).frame(width: 5, height: 5).opacity(0.35 + 0.65 * max(0, sin((time * 4 - Double(index) * 0.8))))
                }
            }
        }
        .frame(width: 23)
    }
}

/// The amber card over the box (chat/decide.rs `decide_card`): a permission with its summary, Permitir and Negar; or the
/// agent's questions, their tabs, the options, another answer typed, Voltar / Próxima / Enviar and Recusar. Every click
/// goes to Rust (`ui.chat.ask`), which keeps the picks.
struct AskCard: View {
    let id: String
    let ask: ChatAsk
    private let store = MagoStore.shared
    @Environment(\.chatLook) private var look
    @State private var other = ""

    var body: some View {
        AmberFrame(head: ask.head) {
            if ask.questions.isEmpty {
                Text(ask.summary)
                    .font(.mono(13))
                    .foregroundStyle(look.text)
                    .textSelection(.enabled)
                    .padding(.horizontal, 12)
                    .padding(.vertical, 8)
                    .frame(maxWidth: .infinity, alignment: .leading)
                    .background(RoundedRectangle(cornerRadius: 8).fill(look.well))
                    .overlay(RoundedRectangle(cornerRadius: 8).strokeBorder(look.wellEdge))
                HStack(spacing: 8) {
                    Pill(text: "Permitir", strong: true) { act("allow") }
                    Pill(text: "Negar", strong: false) { act("deny") }
                    if ask.outside { Pill(text: "Responder no terminal", strong: false) { act("terminal") } }
                }
            } else {
                questions
            }
        }
        .onAppear { other = ask.other[safe: ask.step] ?? "" }
        .onChange(of: ask.step) { _, step in other = ask.other[safe: step] ?? "" }
    }

    @ViewBuilder private var questions: some View {
        let step = min(ask.step, ask.questions.count - 1)
        let question = ask.questions[step]
        if ask.questions.count > 1 {
            HStack(spacing: 8) {
                ForEach(Array(ask.questions.enumerated()), id: \.offset) { index, item in
                    let answered = !(ask.picked[safe: index] ?? []).isEmpty || !(ask.other[safe: index] ?? "").trimmingCharacters(in: .whitespaces).isEmpty
                    Pill(text: (answered ? "✓ " : "") + (item.header.isEmpty ? "\(index + 1)" : item.header), strong: index == step) { act("tab", index: index) }
                }
            }
        }
        Text(question.text).font(.app(14.5)).foregroundStyle(look.text).fixedSize(horizontal: false, vertical: true)
        if question.multiple { Text("Escolha uma ou mais").font(.app(12)).foregroundStyle(look.muted) }
        ForEach(Array(question.options.enumerated()), id: \.offset) { index, option in
            OptionRow(number: index + 1, option: option, multiple: question.multiple, on: (ask.picked[safe: step] ?? []).contains(index)) { act("pick", index: index) }
        }
        TextField("", text: $other, prompt: Text("Outra resposta…").foregroundStyle(look.muted))
            .textFieldStyle(.plain)
            .font(.app(14))
            .foregroundStyle(look.text)
            .padding(.horizontal, 10)
            .padding(.vertical, 6)
            .background(RoundedRectangle(cornerRadius: 8).fill(look.well))
            .overlay(RoundedRectangle(cornerRadius: 8).strokeBorder(look.wellEdge))
            .onChange(of: other) { _, value in
                if value != (ask.other[safe: step] ?? "") { store.send("ui.chat.ask", ["chat": id, "action": "other", "text": value]) }
            }
        HStack(spacing: 8) {
            let last = step + 1 == ask.questions.count
            if step > 0 { Pill(text: "Voltar", strong: false) { act("tab", index: step - 1) } }
            if !last { Pill(text: "Próxima", strong: true) { act("tab", index: step + 1) } }
            if last && ask.complete { Pill(text: "Enviar", strong: true) { act("allow") } }
            Pill(text: "Recusar", strong: false) { act("deny") }
            if ask.outside { Pill(text: "Responder no terminal", strong: false) { act("terminal") } }
        }
    }

    private func act(_ action: String, index: Int? = nil) {
        var args: [String: Any] = ["chat": id, "action": action]
        if let index { args["index"] = index }
        store.send("ui.chat.ask", args)
    }
}

/// An option of a question: "n. label" and what it means, in the well; amber edge when picked.
private struct OptionRow: View {
    let number: Int?
    let option: [String]
    let multiple: Bool
    let on: Bool
    let action: () -> Void
    @Environment(\.chatLook) private var look
    @State private var hovered = false

    var body: some View {
        Button(action: action) {
            VStack(alignment: .leading, spacing: 3) {
                Text((multiple ? (on ? "☑ " : "☐ ") : "") + (number.map { "\($0). " } ?? "") + (option.first ?? "")).font(.app(14)).foregroundStyle(look.text)
                if let meaning = option[safe: 1], !meaning.isEmpty { Text(meaning).font(.app(12.5)).foregroundStyle(look.muted) }
            }
            .fixedSize(horizontal: false, vertical: true)
            .padding(.horizontal, 12)
            .padding(.vertical, 8)
            .frame(maxWidth: .infinity, minHeight: 34, alignment: .leading)
            .background(RoundedRectangle(cornerRadius: 8).fill(hovered ? ChatLook.amber.opacity(0.22) : look.well))
            .overlay(RoundedRectangle(cornerRadius: 8).strokeBorder(on ? ChatLook.amber : look.wellEdge))
            .contentShape(Rectangle())
        }
        .buttonStyle(.plain)
        .onHover { hovered = $0 }
    }
}

/// A terminal stopped on a question only its screen answers (steps.rs `waiting_card`): why it waits, and the menu on its
/// screen as buttons (the row's own answers, `ui.chat.answer`), else its last lines.
struct WaitingCard: View {
    let id: String
    let waiting: ChatWaiting
    @Environment(\.chatLook) private var look

    var body: some View {
        AmberFrame(head: waiting.head) {
            Text(waiting.why).font(.app(13)).foregroundStyle(look.amberNote).lineLimit(1).padding(.leading, 20)
            if !waiting.question.isEmpty { Text(waiting.question).font(.app(14)).foregroundStyle(look.text) }
            ForEach(Array(waiting.choices.enumerated()), id: \.offset) { index, choice in
                OptionRow(number: nil, option: [choice], multiple: false, on: index == waiting.marked) {
                    MagoStore.shared.send("ui.chat.answer", ["chat": id, "index": index])
                }
            }
            if !waiting.screen.isEmpty {
                // no menu to read: its last lines
                Text(waiting.screen)
                    .font(.mono(13))
                    .foregroundStyle(look.text)
                    .padding(.horizontal, 12)
                    .padding(.vertical, 8)
                    .frame(maxWidth: .infinity, alignment: .leading)
                    .background(RoundedRectangle(cornerRadius: 8).fill(look.well))
                    .overlay(RoundedRectangle(cornerRadius: 8).strokeBorder(look.wellEdge))
            }
        }
        // Abrir terminal: the terminal a click away (on the canvas, where SwiftUI shows it)
        .overlay(alignment: .topTrailing) {
            Button { MagoStore.shared.send("ui.chat.on_canvas", ["chat": id]) } label: {
                HStack(spacing: 6) {
                    Image(systemName: "chevron.right").font(.system(size: 11, weight: .bold))
                    Text("Abrir terminal").font(.app(14))
                }
                .foregroundStyle(look.amberTitle)
                .padding(.horizontal, 12)
                .frame(height: 30)
                .overlay(RoundedRectangle(cornerRadius: 8).strokeBorder(look.amberEdge))
                .contentShape(Rectangle())
            }
            .buttonStyle(HoverGround(radius: 8, color: ChatLook.amber.opacity(0.18)))
            .padding(12)
        }
    }
}

/// The amber card itself (AmberCard): radius 12, 12 around, the pulsing dot and its bold head.
struct AmberFrame<Content: View>: View {
    let head: String
    @ViewBuilder let content: Content
    @Environment(\.chatLook) private var look

    var body: some View {
        VStack(alignment: .leading, spacing: 6) {
            HStack(spacing: 11) {
                PulseDot(color: ChatLook.amber, size: 9)
                Thick(text: head, size: 14, color: look.amberTitle)
            }
            .frame(height: 20)
            content
        }
        .padding(12)
        .frame(maxWidth: .infinity, alignment: .leading)
        .background(RoundedRectangle(cornerRadius: 12).fill(look.amberFill))
        .overlay(RoundedRectangle(cornerRadius: 12).strokeBorder(look.amberEdge))
        .transition(.opacity.combined(with: .move(edge: .bottom)))
    }
}

/// A round button of the amber card (decide.rs `pill`): 30 tall, 13.5 pt; the one that answers in amber.
struct Pill: View {
    let text: String
    let strong: Bool
    let action: () -> Void
    @Environment(\.chatLook) private var look
    @State private var hovered = false

    var body: some View {
        Button(action: action) {
            Text(text).font(.app(13.5)).foregroundStyle(look.text)
                .padding(.horizontal, 12)
                .frame(height: 30)
                .background(Capsule().fill(strong ? ChatLook.amber.opacity(hovered ? 0.4 : 0.28) : (hovered ? ChatLook.amber.opacity(0.18) : .clear)))
                .overlay(Capsule().strokeBorder(look.amberEdge))
                .contentShape(Capsule())
        }
        .buttonStyle(.plain)
        .onHover { hovered = $0 }
    }
}
