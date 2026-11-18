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

struct IslandAskCard: View {
    let model: IslandModel
    let ask: IslandAsk
    let size: CGSize
    @State private var other = ""
    @FocusState private var focused: Bool

    var body: some View {
        let wash = ask.approval ? IslandInk.amber : IslandInk.cyan
        let color = ask.color.map(IslandInk.hex) ?? wash
        ZStack(alignment: .topLeading) {
            IslandWash(color: wash, strength: ask.approval ? 0.42 : 0.38)
            IslandLuna(model: model, center: CGPoint(x: 52, y: size.height / 2), size: 56, state: ask.approval ? .needsYou : .asking)
            // "Depois": folded, it keeps waiting
            HStack(spacing: 8) {
                if ask.queued > 0 { Text("+\(ask.queued) na fila").font(.system(size: 11)).foregroundStyle(IslandInk.dim) }
                Button { model.fold() } label: {
                    Image(systemName: "chevron.up").font(.system(size: 8, weight: .bold)).foregroundStyle(IslandInk.rgb(0x5f646d))
                        .frame(width: 16, height: 16).background(Circle().fill(Color.white.opacity(18 / 255)))
                }
                .buttonStyle(.plain).help("Depois — deixar esperando")
            }
            .frame(maxWidth: .infinity, alignment: .trailing).padding(.trailing, 10).padding(.top, 8)
            VStack(alignment: .leading, spacing: 5) {
                HStack(spacing: 7) {
                    if let name = ask.name {
                        Circle().fill(color).frame(width: 8, height: 8)
                        Text(name).font(.system(size: 12, weight: .semibold)).foregroundStyle(IslandInk.ink).lineLimit(1)
                    }
                    Text(ask.doing).font(.system(size: 12)).foregroundStyle(IslandInk.dim).lineLimit(1)
                }
                if ask.approval { approval } else { question }
            }
            .padding(.leading, 106).padding(.trailing, 40).padding(.vertical, 14)
            .frame(width: size.width, height: size.height, alignment: ask.approval && !ask.open ? .leading : .topLeading)
        }
        .frame(width: size.width, height: size.height)
        .islandCard()
        .onAppear { report() }
        .onChange(of: ask) { report() }
        .onChange(of: focused) { model.typing = focused }
        .onDisappear { model.typing = false }
    }

    /// The card's height for the island (`ask.rs`: 160 a permission, 380 opened, 200 a question growing with its options).
    private func report() {
        let options = ask.questions.isEmpty ? 0 : ask.questions[min(ask.step, ask.questions.count - 1)].options.count
        let height = ask.approval ? (ask.open ? IslandSize.approvalOpenH : IslandSize.approvalH)
            : min(IslandSize.questionH + CGFloat(max(options - 2, 0)) * 34, IslandSize.questionMost)
        if height != model.askHeight { model.askHeight = height }
    }

    private func press(_ args: [String: Any]) {
        // what arrives under the pointer is not clicked by accident
        guard model.now - model.askSince > 0.6 else { return }
        model.act("ask", args.merging(["chat": ask.chat]) { first, _ in first })
    }

    private var approval: some View {
        VStack(alignment: .leading, spacing: 5) {
            HStack(alignment: .top) {
                if ask.open {
                    ScrollView { Text(ask.details).textSelection(.enabled).frame(maxWidth: .infinity, alignment: .leading) }.scrollIndicators(.never)
                } else {
                    Text(ask.summary).lineLimit(1)
                }
                IslandLink(text: ask.open ? "recolher" : "ver tudo", size: 11) { press(["open": true]) }
            }
            .font(.system(size: 12, design: .monospaced)).foregroundStyle(IslandInk.rgb(0xe8e9ec))
            .padding(.horizontal, 10).padding(.vertical, 5)
            .background(RoundedRectangle(cornerRadius: 10).fill(Color.white.opacity(18 / 255)))
            .overlay(RoundedRectangle(cornerRadius: 10).strokeBorder(Color.white.opacity(15 / 255)))
            HStack(spacing: 8) {
                IslandAskButton(label: "Negar", key: "N") { press(["allow": false]) }
                IslandAskButton(label: "Permitir", key: "Y", primary: true) { press(["allow": true]) }
                if ask.outside { IslandLink(text: "Responder no terminal", size: 11.5) { press(["terminal": true]) } }
            }
        }
    }

    private var question: some View {
        let step = min(ask.step, max(ask.questions.count - 1, 0))
        let current = ask.questions.isEmpty ? nil : ask.questions[step]
        let picked = step < ask.picked.count ? ask.picked[step] : []
        return ScrollView {
            VStack(alignment: .leading, spacing: 6) {
                if let current {
                    Text(current.text).font(.system(size: 13)).foregroundStyle(IslandInk.ink).fixedSize(horizontal: false, vertical: true)
                    if current.multiple { Text("Escolha uma ou mais").font(.system(size: 11)).foregroundStyle(IslandInk.dim) }
                    ForEach(Array(current.options.enumerated()), id: \.offset) { index, option in
                        Button { press(["pick": index]) } label: {
                            HStack(alignment: .top, spacing: 8) {
                                Text("\(index + 1)").font(.system(size: 11, weight: .semibold, design: .monospaced)).foregroundStyle(IslandInk.dim3)
                                VStack(alignment: .leading, spacing: 2) {
                                    Text(option.first ?? "").font(.system(size: 12, weight: .semibold)).foregroundStyle(IslandInk.ink)
                                    if option.count > 1, !option[1].isEmpty {
                                        Text(option[1]).font(.system(size: 11)).foregroundStyle(IslandInk.dim).fixedSize(horizontal: false, vertical: true)
                                    }
                                }
                                Spacer(minLength: 0)
                            }
                            .padding(.horizontal, 10).padding(.vertical, 6)
                            .background(RoundedRectangle(cornerRadius: 10).fill(Color.white.opacity((picked.contains(index) ? 40 : 14) / 255)))
                            .contentShape(Rectangle())
                        }
                        .buttonStyle(.plain)
                    }
                    HStack(spacing: 8) {
                        TextField("Outro: escreva a sua resposta…", text: $other).textFieldStyle(.plain).font(.system(size: 12)).focused($focused)
                            .onSubmit { press(["other": other]); other = "" }
                        if current.multiple { IslandAskButton(label: "OK", primary: true) { press(["next": true]) } }
                    }
                    .padding(.horizontal, 10).frame(height: 30)
                    .background(RoundedRectangle(cornerRadius: 10).fill(Color.white.opacity(14 / 255)))
                    if ask.outside { IslandLink(text: "Responder no terminal", size: 11.5) { press(["terminal": true]) } }
                }
            }
        }
        .scrollIndicators(.never)
    }
}

/// A button of the ask's row: `.btn.primary` (ink on white) or `.btn.secondary` (9% white), with its key.
struct IslandAskButton: View {
    let label: String
    var key: String?
    var primary = false
    let action: () -> Void
    @State private var hovered = false

    var body: some View {
        Button(action: action) {
            HStack(spacing: 6) {
                Text(label).font(.system(size: 12, weight: .semibold))
                if let key {
                    Text(key).font(.system(size: 10, weight: .semibold, design: .monospaced)).opacity(0.55)
                        .padding(.horizontal, 4).background(RoundedRectangle(cornerRadius: 4).strokeBorder((primary ? Color.black : IslandInk.ink).opacity(0.3)))
                }
            }
            .foregroundStyle(primary ? Color.black : IslandInk.ink)
            .padding(.horizontal, 14).frame(height: 28)
            .background(Capsule().fill(primary ? IslandInk.ink.opacity(hovered ? 1 : 0.92) : Color.white.opacity((hovered ? 38 : 23) / 255)))
            .contentShape(Capsule())
        }
        .buttonStyle(.plain)
        .onHover { hovered = $0 }
    }
}
