import SwiftUI

/// Alterações (app/panes/changes/): the worktree picker, the search, the changed files of each folder in a box ten rows tall,
/// the diff of the one picked with Abrir na Árvore, and the review that goes to an agent. All of it is the egui panel's state.
struct ChangesBody: View {
    let changes: ChangesState
    let look: PaneLook
    let rows: Int
    @State private var search = ""

    private var muted: Color { Color(rgba: look.muted) }

    /// The list's box: as tall as its rows (a folder's header 20, a file 26, 40 with its line of code, 2 apart), `rows` files at most.
    private var listHeight: CGFloat {
        var heights: [CGFloat] = []
        for group in changes.groups {
            heights.append(20)
            for file in group.files { heights.append(file.code == nil ? 26 : 40) }
        }
        let total = heights.reduce(0, +) + CGFloat(max(heights.count - 1, 0)) * 2
        return min(total, CGFloat(rows) * 28 + 20)
    }

    var body: some View {
        if let worktree = changes.worktree {
            Menu {
                ForEach(changes.worktrees) { choice in
                    Button { paneVerb("worktree", ["path": choice.path]) } label: {
                        if choice.on { Label(choice.label, systemImage: "checkmark") } else { Text(choice.label) }
                    }
                    .help(choice.tip)
                }
            } label: {
                ComboLabel(text: worktree, look: look)
            }
            // the button style draws the label as it is (the borderless one keeps only its text and icon)
            .menuStyle(.button).buttonStyle(.plain).menuIndicator(.hidden)
        }
        if let empty = changes.empty {
            Text(empty).font(.app(14)).foregroundStyle(muted)
        } else {
            TextField("", text: $search, prompt: Text("Buscar arquivo ou trecho de código").foregroundStyle(muted))
                .paneField(look)
                .synced(changes.search, $search) { paneVerb("search", ["text": $0]) }
            if let found = changes.found { Text(found).font(.app(11)).foregroundStyle(muted) }
            ScrollView(.vertical) {
                // lazy: a repo with its build folders untracked lists hundreds of thousands of files
                LazyVStack(alignment: .leading, spacing: 2) {
                    ForEach(Array(changes.groups.enumerated()), id: \.offset) { _, group in
                        Text(group.title).font(.app(12)).foregroundStyle(muted).lineLimit(1)
                            .padding(.horizontal, 2).frame(height: 20)
                        ForEach(group.files) { file in
                            ChangeRow(file: file, selected: file.path == changes.selected, look: look)
                        }
                    }
                }
            }
            .frame(height: listHeight)
        }
        if let selected = changes.selected {
            DiffBox(rows: changes.diff, picked: changes.picked, look: look).id(selected)
            let tree = changes.groups.flatMap(\.files).first { $0.path == selected }?.tree ?? ""
            Button("Abrir na Árvore") { paneVerb("open", ["tree": tree, "path": selected]) }.buttonStyle(PaneButtonStyle(look: look))
        }
        if let review = changes.review { ReviewSection(review: review, look: look) }
    }
}

/// A combo box's face, as egui's: the field color, the text, a small triangle at the right.
struct ComboLabel: View {
    let text: String
    let look: PaneLook

    var body: some View {
        HStack {
            Text(text).font(.app(14)).lineLimit(1).truncationMode(.tail)
            Spacer(minLength: 6)
            Image(systemName: "triangle.fill").font(.system(size: 7)).rotationEffect(.degrees(180))
        }
        .foregroundStyle(Color(rgba: look.text))
        .padding(.horizontal, 12)
        .frame(height: 28)
        .frame(maxWidth: .infinity)
        .background(Color(rgba: look.field), in: RoundedRectangle(cornerRadius: 6))
        .contentShape(Rectangle())
    }
}

/// A file's row: its icon, its path in the folder in its git color, its lines added and removed; under it, the changed
/// line the search found the text in. A press shows its diff (again: hides it).
private struct ChangeRow: View {
    let file: ChangedFile
    let selected: Bool
    let look: PaneLook
    @State private var hovering = false
    @Environment(\.colorScheme) private var scheme

    var body: some View {
        VStack(alignment: .leading, spacing: 0) {
            HStack(spacing: 0) {
                FTIcon(icon: file.icon, dark: scheme == .dark).frame(width: 28)
                Text(file.name).font(.code(12)).foregroundStyle(Color(rgba: file.color)).lineLimit(1).truncationMode(.tail)
                Spacer(minLength: 8)
                if let lines = file.lines, lines.count == 2 {
                    Text("+\(lines[0])").font(.code(12)).foregroundStyle(Color(rgba: look.added))
                    Text("−\(lines[1])").font(.code(12)).foregroundStyle(Color(rgba: look.danger)).padding(.leading, 6)
                }
            }
            .padding(.trailing, 8)
            .frame(height: 26)
            if let code = file.code {
                Text("no código: \(code)").font(.code(10.5)).foregroundStyle(Color(rgba: look.muted)).lineLimit(1)
                    .padding(.leading, 28).padding(.trailing, 8).frame(height: 14, alignment: .top)
            }
        }
        .background {
            RoundedRectangle(cornerRadius: 7)
                .fill(selected ? Color(rgba: look.selected) : Color(rgba: look.ink).opacity(hovering ? 0.06 : 0))
        }
        .contentShape(Rectangle())
        .onHover { hovering = $0 }
        .onTapGesture { paneVerb("select", ["tree": file.tree, "path": file.path]) }
        .accessibilityElement(children: .combine)
        .accessibilityAddTraits(.isButton)
        .accessibilityLabel(file.name)
    }
}

/// The review: its comments (each with its ×), the general note, the agent it goes to and the two ways to send.
private struct ReviewSection: View {
    let review: ReviewState
    let look: PaneLook
    @State private var note = ""

    private var muted: Color { Color(rgba: look.muted) }

    var body: some View {
        Text(review.heading).font(.app(12)).foregroundStyle(muted).padding(.top, 4)
        ForEach(Array(review.comments.enumerated()), id: \.offset) { index, comment in
            HStack(alignment: .top) {
                VStack(alignment: .leading, spacing: 2) {
                    Text(comment.place).font(.code(12)).lineLimit(1)
                    Text(comment.first).font(.code(11)).foregroundStyle(muted).lineLimit(1)
                    Text(comment.note).font(.app(14)).fixedSize(horizontal: false, vertical: true)
                }
                Spacer(minLength: 4)
                Button("×") { paneVerb("remove", ["index": index]) }.buttonStyle(.plain).font(.app(14)).help("Tirar o comentário")
            }
            .paneCard(look)
        }
        if review.comments.isEmpty {
            Text("Clique nas linhas de um diff para comentar, como numa revisão de PR.").font(.app(11)).foregroundStyle(muted)
        }
        TextEditor(text: $note)
            .font(.app(14))
            .scrollContentBackground(.hidden)
            .frame(height: 40)
            .overlay(alignment: .topLeading) {
                if note.isEmpty { Text("Observação geral (opcional)").font(.app(14)).foregroundStyle(muted).padding(.leading, 5).allowsHitTesting(false) }
            }
            .padding(.horizontal, 7).padding(.vertical, 4)
            .background(Color(rgba: look.field), in: RoundedRectangle(cornerRadius: 6))
            .synced(review.note, $note) { paneVerb("note", ["text": $0]) }
        HStack(spacing: 8) {
            Text("Para").font(.app(14)).foregroundStyle(muted)
            Menu {
                ForEach(review.agents) { agent in
                    Button { paneVerb("agent", ["id": agent.id]) } label: {
                        if agent.id == review.agent { Label(agent.label, systemImage: "checkmark") } else { Text(agent.label) }
                    }
                    .help(agent.cwd)
                }
            } label: {
                ComboLabel(text: review.agents.first { $0.id == review.agent }?.name ?? "Nenhum agente", look: look)
            }
            .menuStyle(.button).buttonStyle(.plain).menuIndicator(.hidden)
        }
        HStack(spacing: 8) {
            Button("Enviar revisão") { paneVerb("send") }
                .buttonStyle(PaneButtonStyle(look: look)).disabled(!review.canSend)
                .help("Os comentários e a observação, numa mensagem ao agente")
            Button("Pedir análise de PR") { paneVerb("pr") }
                .buttonStyle(PaneButtonStyle(look: look)).disabled(!review.canAsk)
                .help("O agente revisa todo o diff destas pastas, como num PR")
        }
    }
}

/// The comment's bar at the panel's bottom while lines of the diff are picked: where, the field (it takes the keys), Comentar
/// (⌘↩) and Cancelar (Esc).
struct CommentBar: View {
    let title: String
    let look: PaneLook
    @State private var draft = ""
    @FocusState private var focused: Bool

    private var written: Bool { !draft.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty }

    var body: some View {
        VStack(alignment: .leading, spacing: 6) {
            Text(title).font(.app(12)).foregroundStyle(Color(rgba: look.muted)).lineLimit(1)
            TextEditor(text: $draft)
                .font(.app(14))
                .scrollContentBackground(.hidden)
                .focused($focused)
                .overlay(alignment: .topLeading) {
                    if draft.isEmpty {
                        Text("O que está errado aqui?").font(.app(14)).foregroundStyle(Color(rgba: look.muted)).padding(.leading, 5).allowsHitTesting(false)
                    }
                }
                .padding(.horizontal, 7).padding(.vertical, 4)
                .frame(height: 48)
                .background(Color(rgba: look.field), in: RoundedRectangle(cornerRadius: 6))
            HStack(spacing: 8) {
                Button("Comentar") {
                    paneVerb("comment", ["note": draft])
                    draft = ""
                }
                .buttonStyle(PaneButtonStyle(look: look)).disabled(!written)
                .keyboardShortcut(.return, modifiers: .command).help("Ctrl+Enter")
                Button("Cancelar") { paneVerb("pick") }
                    .buttonStyle(PaneButtonStyle(look: look))
                    .keyboardShortcut(.cancelAction).help("Esc")
            }
        }
        .padding(.horizontal, 12).padding(.vertical, 8)
        .frame(height: 136, alignment: .top)
        .onAppear { focused = true }
    }
}

extension View {
    /// egui's `card` in the panels: a faint fill of the ink, a hairline, rounded 8, 8 of room.
    func paneCard(_ look: PaneLook) -> some View {
        self
            .padding(8)
            .frame(maxWidth: .infinity, alignment: .leading)
            .background {
                RoundedRectangle(cornerRadius: 8).fill(Color(rgba: look.ink).opacity(0.04)).stroke(Color(rgba: look.ink).opacity(0.12), lineWidth: 1)
            }
    }
}
