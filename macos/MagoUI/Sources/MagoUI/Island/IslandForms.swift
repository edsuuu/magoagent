// The open island's forms: the new chat (forms.rs) and the card of a question or a permission (ask.rs).

import AppKit
import SwiftUI

// MARK: Novo

struct IslandNewPage: View {
    let model: IslandModel
    let size: CGSize
    @State private var folder = ""
    @State private var worktree = false
    @State private var prompt = ""
    @State private var failed: String?
    @FocusState private var focused: Bool

    var body: some View {
        VStack(alignment: .leading, spacing: 8) {
            HStack(spacing: 8) {
                Text("Nova sessão de agente").font(.system(size: 14)).foregroundStyle(IslandInk.ink)
                Spacer()
                Menu {
                    ForEach(model.snapshot.projects, id: \.self) { project in Button(project) { folder = project } }
                    if !model.snapshot.projects.isEmpty { Divider() }
                    Button("Escolher uma pasta…", action: pick)
                } label: {
                    Text(folder.isEmpty ? "Escolha um projeto" : (folder as NSString).lastPathComponent).font(.system(size: 11.5))
                }
                .menuStyle(.borderlessButton).fixedSize()
                Button { worktree.toggle() } label: {
                    Text("worktree").font(.system(size: 11)).foregroundStyle(worktree ? IslandInk.picked : IslandInk.dim)
                        .padding(.horizontal, 8).frame(height: 22)
                        .background(RoundedRectangle(cornerRadius: 6).fill(worktree ? IslandInk.picked.opacity(0.22) : .clear))
                }
                .buttonStyle(.plain)
            }
            IslandPromptBar(text: $prompt, placeholder: "O que o agente deve fazer?", focused: $focused, ready: !folder.isEmpty, send: create)
            Text(failed ?? "Abre um chat novo no projeto, num espaço próprio, com as suas palavras.")
                .font(.system(size: 11)).foregroundStyle(failed == nil ? IslandInk.dim : IslandInk.red).lineLimit(1)
            Spacer(minLength: 0)
        }
        .padding(.horizontal, 16).padding(.vertical, 14)
        .islandCard()
        .onAppear { folder = model.snapshot.projects.first ?? "" }
        .onChange(of: focused) { model.typing = focused }
        .onDisappear { model.typing = false }
    }

    private func pick() {
        let panel = NSOpenPanel()
        (panel.canChooseDirectories, panel.canChooseFiles, panel.allowsMultipleSelection) = (true, false, false)
        panel.prompt = "Escolher"
        if panel.runModal() == .OK, let url = panel.url { folder = url.path }
    }

    private func create() {
        guard !folder.isEmpty else { return pick() }
        Task {
            let reply = await MagoStore.shared.act("ui.island.new", ["folder": folder, "worktree": worktree, "prompt": prompt, "name": ""])
            if reply.ok { prompt = "" } else { failed = reply.text }
        }
    }
}

// MARK: a question or a permission

/// The card of the first ask waiting (`ask.rs`): its wash (amber a permission, cyan a question), Luna at the left and,
/// from 116, who asks, then the permission's command with Negar and Permitir, or the question whole with its options as
/// rows and "Outro"; the card grows with them up to the window's room, and past it they scroll.
struct IslandAskCard: View {
    let model: IslandModel
    let ask: IslandAsk
    let size: CGSize
    @State private var other = ""
    /// The question's list as tall as it is (`content` in `island_question`), for the island's height.
    @State private var content: CGFloat = 0
    @FocusState private var focused: Bool

    static let left: CGFloat = 116

    var body: some View {
        let wash = ask.approval ? IslandInk.amber : IslandInk.cyan
        ZStack(alignment: .topLeading) {
            IslandWash(color: wash, strength: ask.approval ? 0.42 : 0.38)
            // the middle of an 84 tall card under the header (`botPosition`), 52 in
            IslandLuna(model: model, center: CGPoint(x: 52, y: size.height / 2 + 5), size: 56, state: ask.approval ? .needsYou : .asking)
            fold
            if ask.approval { approval } else { question }
        }
        .frame(width: size.width, height: size.height, alignment: .topLeading)
        .islandCard()
        .onAppear { report() }
        .onChange(of: ask) { report() }
        .onChange(of: content) { report() }
        .onChange(of: focused) { model.typing = focused }
        .onDisappear { model.typing = false }
    }

    /// The card's height for the island: 160 a permission, 380 opened; a question as tall as what it holds, from 200 to
    /// 420 (`ask.rs`).
    private func report() {
        let height: CGFloat
        if ask.approval {
            height = ask.open ? IslandSize.approvalOpenH : IslandSize.approvalH
        } else {
            let needed = 14 + 15 + 10 + content + (footer ? 38 : 0) + 14
            height = min(max(needed + IslandSize.pageTop + 10, IslandSize.questionH), IslandSize.questionMost)
        }
        if height != model.askHeight { model.askHeight = height }
    }

    private func press(_ args: [String: Any]) {
        // what arrives under the pointer is not clicked by accident
        guard model.now - model.askSince > 0.6 else { return }
        model.act("ask", args.merging(["chat": ask.chat]) { first, _ in first })
    }

    private var step: Int { min(ask.step, max(ask.questions.count - 1, 0)) }
    private var current: IslandQuestion? { ask.questions.isEmpty ? nil : ask.questions[step] }
    /// OK for a multiple choice, the way to the terminal for a session outside the app.
    private var footer: Bool { current?.multiple == true || ask.outside }

    /// "Depois — deixar esperando": the island folds and the ask keeps waiting; the asks behind it at its left.
    private var fold: some View {
        HStack(spacing: 8) {
            if ask.queued > 0 { Text("+\(ask.queued) na fila").font(.system(size: 11)).foregroundStyle(IslandInk.dim) }
            Button { model.fold() } label: {
                Image(systemName: "chevron.up").font(.system(size: 8, weight: .bold)).foregroundStyle(IslandInk.rgb(0x5f646d))
                    .frame(width: 16, height: 16).background(Circle().fill(Color.white.opacity(18 / 255)))
            }
            .buttonStyle(.plain).help("Depois — deixar esperando")
        }
        .frame(width: size.width - 10, height: 16, alignment: .trailing)
        .offset(y: 8)
    }

    /// `agentWho`: its dot, its name, what it does (cut before the fold).
    private var who: some View {
        HStack(spacing: 7) {
            if let name = ask.name {
                Circle().fill(ask.color.map(IslandInk.hex) ?? (ask.approval ? IslandInk.amber : IslandInk.cyan)).frame(width: 8, height: 8)
                Text(name).font(.system(size: 12, weight: .semibold)).foregroundStyle(IslandInk.ink).lineLimit(1).fixedSize()
            }
            Text(ask.doing).font(.system(size: 12)).foregroundStyle(IslandInk.dim).lineLimit(1)
        }
        .frame(height: 15)
        .padding(.trailing, 24)
    }

    /// Who asks, the command and the row of buttons, centred down the card, 5 apart.
    private var approval: some View {
        VStack(alignment: .leading, spacing: 5) {
            who
            well
            HStack(spacing: 8) {
                IslandAskButton(label: "Negar", key: "N") { press(["allow": false]) }
                IslandAskButton(label: "Permitir", key: "Y", primary: true) { press(["allow": true]) }
                if ask.outside { IslandAskLink(text: "Responder no terminal") { press(["terminal": true]) } }
            }
        }
        .frame(width: size.width - Self.left - 16, alignment: .leading)
        .frame(maxHeight: .infinity)
        .padding(.vertical, ask.open ? 14 : 0)
        .padding(.leading, Self.left)
    }

    /// The command on one line with "ver tudo" at its end; opened, the whole request in a well that scrolls.
    private var well: some View {
        Group {
            if ask.open {
                VStack(alignment: .trailing, spacing: 6) {
                    IslandAskLink(text: "recolher", inset: 0) { press(["open": true]) }.padding(.trailing, 6)
                    ScrollView { Text(ask.details).textSelection(.enabled).frame(maxWidth: .infinity, alignment: .leading) }.scrollIndicators(.never)
                }
                .padding(.leading, 10).padding(.trailing, 4).padding(.top, 5).padding(.bottom, 6)
                .frame(maxWidth: .infinity, maxHeight: .infinity, alignment: .topLeading)
            } else {
                HStack(spacing: 10) {
                    Text(ask.summary).lineLimit(1)
                    IslandAskLink(text: "ver tudo", inset: 0) { press(["open": true]) }
                }
                .padding(.horizontal, 10).padding(.vertical, 5)
            }
        }
        .font(.system(size: 12, design: .monospaced)).foregroundStyle(IslandInk.rgb(0xe8e9ec))
        .background(RoundedRectangle(cornerRadius: 10).fill(Color.white.opacity(18 / 255)))
        .overlay(RoundedRectangle(cornerRadius: 10).strokeBorder(Color.white.opacity(15 / 255)))
        .contentShape(Rectangle())
        // closed, the whole well opens it
        .onTapGesture { if !ask.open { press(["open": true]) } }
    }

    /// Who asks; under it the question, its options and "Outro", scrolling past the room; OK and the terminal at the bottom.
    private var question: some View {
        let picked = step < ask.picked.count ? ask.picked[step] : []
        return VStack(alignment: .leading, spacing: 0) {
            who
            ScrollView {
                VStack(alignment: .leading, spacing: 0) {
                    if let current {
                        Text(current.text).font(.system(size: 15, weight: .semibold)).foregroundStyle(IslandInk.ink).fixedSize(horizontal: false, vertical: true)
                        if current.multiple { Text("Escolha uma ou mais").font(.system(size: 11)).foregroundStyle(IslandInk.dim).padding(.top, 4) }
                        VStack(spacing: 6) {
                            ForEach(Array(current.options.enumerated()), id: \.offset) { index, option in
                                IslandAskOption(number: index, option: option, multiple: current.multiple, on: picked.contains(index)) { press(["pick": index]) }
                            }
                            otherBox
                        }
                        .padding(.top, 10)
                    }
                }
                .padding(.trailing, 4)
                .onGeometryChange(for: CGFloat.self) { $0.size.height } action: { content = $0 }
            }
            .scrollIndicators(.never)
            .padding(.top, 10)
            if footer {
                HStack(spacing: 8) {
                    if current?.multiple == true {
                        // with nothing picked nor written it waits; written, OK sends the words too (`answer_other`)
                        let words = other.trimmingCharacters(in: .whitespacesAndNewlines)
                        IslandAskButton(label: "OK", primary: true, off: picked.isEmpty && words.isEmpty) { words.isEmpty ? press(["next": true]) : sendOther() }
                    }
                    if ask.outside { IslandAskLink(text: "Responder no terminal") { press(["terminal": true]) } }
                }
                .frame(height: 30)
                .padding(.top, 8)
            }
        }
        .padding(.vertical, 14)
        .frame(width: size.width - Self.left - 12, height: size.height, alignment: .topLeading)
        .padding(.leading, Self.left)
    }

    /// "Outro": the answer in one's own words, sent by Enter or its round button.
    private var otherBox: some View {
        let empty = other.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty
        return HStack(alignment: .bottom, spacing: 8) {
            TextField("", text: $other, prompt: Text("Outro: escreva a sua resposta…").foregroundStyle(IslandInk.dim3), axis: .vertical)
                .textFieldStyle(.plain).font(.system(size: 13)).foregroundStyle(IslandInk.ink)
                .lineLimit(1...5).focused($focused)
                .onSubmit(sendOther)
                .padding(.vertical, 3)
            Button(action: sendOther) {
                Image(systemName: "arrow.up").font(.system(size: 11, weight: .bold)).foregroundStyle(IslandInk.black)
                    .frame(width: 24, height: 24).background(Circle().fill(IslandInk.ink.opacity(empty ? 0.5 : 1)))
            }
            .buttonStyle(.plain).help("Enviar")
        }
        .padding(.leading, 12).padding(.trailing, 8).padding(.vertical, 8)
        .frame(minHeight: 40)
        .background(RoundedRectangle(cornerRadius: 12).fill(Color.white.opacity(18 / 255)))
        .overlay(RoundedRectangle(cornerRadius: 12).strokeBorder(Color.white.opacity(14 / 255)))
    }

    private func sendOther() {
        guard !other.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty else { return }
        press(["other": other])
        other = ""
    }
}

/// An option as a row (`island_question`): the key that picks it (✓ once picked in a multiple choice), its label and
/// what it means, wrapped.
struct IslandAskOption: View {
    let number: Int
    let option: [String]
    let multiple: Bool
    let on: Bool
    let action: () -> Void
    @State private var hovered = false

    var body: some View {
        Button(action: action) {
            HStack(alignment: .top, spacing: 8) {
                Text(multiple && on ? "✓" : "\(number + 1)").font(.system(size: 10.5)).foregroundStyle(IslandInk.ink.opacity(on ? 1 : 0.7))
                    .frame(width: 18, height: 18)
                    .overlay(RoundedRectangle(cornerRadius: 5).strokeBorder(Color.white.opacity((on ? 140 : 70) / 255)))
                VStack(alignment: .leading, spacing: 2) {
                    Text(option.first ?? "").font(.system(size: 13, weight: .semibold)).foregroundStyle(IslandInk.ink)
                    if option.count > 1, !option[1].isEmpty { Text(option[1]).font(.system(size: 11.5)).foregroundStyle(IslandInk.dim) }
                }
                .fixedSize(horizontal: false, vertical: true)
                Spacer(minLength: 0)
            }
            .padding(.horizontal, 12).padding(.vertical, 9)
            .frame(minHeight: 34)
            .background(RoundedRectangle(cornerRadius: 10).fill(on ? IslandInk.cyan.opacity(0.2) : Color.white.opacity((hovered ? 34 : 18) / 255)))
            .overlay(RoundedRectangle(cornerRadius: 10).strokeBorder(on ? IslandInk.cyan.opacity(0.8) : Color.white.opacity(14 / 255)))
            .contentShape(Rectangle())
        }
        .buttonStyle(.plain)
        .onHover { hovered = $0 }
    }
}

/// A button of the ask's row (`paint_button`): `.btn.primary` (black on the ink) or `.btn.secondary` (9% white), 12.5,
/// padding 7 13, with its key 7 after the label; `off`, faded and dead.
struct IslandAskButton: View {
    let label: String
    var key: String?
    var primary = false
    var off = false
    let action: () -> Void
    @State private var hovered = false

    var body: some View {
        Button(action: action) {
            HStack(spacing: 7) {
                Text(label).font(.system(size: 12.5))
                if let key {
                    Text(key).font(.system(size: 10.5)).opacity(0.55)
                        .padding(.horizontal, 5)
                        .overlay(RoundedRectangle(cornerRadius: 4).strokeBorder((primary ? Color.black : Color.white).opacity(102 / 255 * 0.55)))
                }
            }
            .foregroundStyle(primary ? IslandInk.black : IslandInk.rgb(0xf1f2f4))
            .padding(.horizontal, 13).padding(.vertical, 7)
            .background(Capsule().fill(primary ? IslandInk.ink : Color.white.opacity((hovered ? 38 : 23) / 255)))
            .opacity(off ? 0.4 : 1)
            .contentShape(Capsule())
        }
        .buttonStyle(.plain)
        .disabled(off)
        .onHover { hovered = $0 }
    }
}

/// `.link-btn`: 11, light enough on the wash, the ink and a line under it under the pointer.
struct IslandAskLink: View {
    let text: String
    var inset: CGFloat = 6
    let action: () -> Void
    @State private var hovered = false

    var body: some View {
        Button(action: action) {
            Text(text).font(.system(size: 11)).foregroundStyle(hovered ? IslandInk.ink : IslandInk.link).underline(hovered)
                .padding(.horizontal, inset)
        }
        .buttonStyle(.plain)
        .onHover { hovered = $0 }
    }
}
