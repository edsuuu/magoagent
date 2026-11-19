import AppKit
import SwiftUI

/// A file's or a folder's icon: the JetBrains SVG Rust sent, or the lookalike badge for the languages that have none.
struct FTIcon: View {
    let icon: FTIconRef
    let dark: Bool
    var size: CGFloat = 16

    var body: some View {
        if let key = icon.key {
            if let image = FT.icon(key, dark: dark) {
                Image(nsImage: image).resizable().interpolation(.high).frame(width: size, height: size)
            } else {
                Color.clear.frame(width: size, height: size)
            }
        } else if let label = icon.label, let hex = icon.color {
            let tint = Color(hex: hex)
            Text(label)
                .font(.system(size: label.count > 1 ? 7 : 9, weight: .semibold))
                .foregroundStyle(tint)
                .frame(width: size * 0.82, height: size * 0.82)
                .background(RoundedRectangle(cornerRadius: 3).fill(tint.opacity(0.18)))
                .overlay(RoundedRectangle(cornerRadius: 3).stroke(tint, lineWidth: 1))
                .frame(width: size, height: size)
        } else {
            Color.clear.frame(width: size, height: size)
        }
    }
}

/// A name asked in a small sheet: a new file or folder in `at`, or the new name of `at`.
struct FTAsk: Identifiable {
    enum Kind { case newFile, newFolder, rename }
    let id = UUID()
    let kind: Kind
    let at: String
    var text: String
}

/// The tree column: the header (back, forward, the folder's name, the view menu, the branch), the rows, the search bar and,
/// over the rows, the Alterações panel.
struct FTTreeColumn: View {
    let state: FTState
    @State private var ask: FTAsk?
    @State private var trash: [String]?
    @State private var clicked: String?
    @FocusState private var focused: Bool

    private var pal: FTPalette { FTPalette(dark: state.dark) }

    var body: some View {
        VStack(spacing: 0) {
            FTHeader(state: state)
            FTRule(color: pal.border).padding(.horizontal, 1).allowsHitTesting(false)
            ZStack {
                rows
                if state.changesOpen { FTChangesPanel(state: state) }
            }
            FTRule(color: pal.border).padding(.horizontal, 1)
            FTSearchBar(state: state)
            if state.dropped > 0 {
                FTRule(color: pal.border).padding(.horizontal, 1)
                Text("Exibindo um número limitado de itens. Mais \(state.dropped) não estão listados.")
                    .font(.app(11)).foregroundStyle(Color(pal.muted)).lineLimit(1)
                    .frame(maxWidth: .infinity, alignment: .leading).padding(.horizontal, 8.125).frame(height: 23)
            }
        }
        .sheet(item: $ask) { item in FTNameSheet(treeId: state.id, ask: item) }
        .confirmationDialog(
            "Mover para a Lixeira",
            isPresented: Binding(get: { trash != nil }, set: { if !$0 { trash = nil } }),
            titleVisibility: .visible
        ) {
            Button("Mover para a Lixeira", role: .destructive) {
                if let paths = trash { Task { await FT.act("trash", state.id, ["paths": paths]) } }
                trash = nil
            }
            Button("Cancelar", role: .cancel) { trash = nil }
        } message: {
            Text(trash.map { $0.count == 1 ? "\((($0[0]) as NSString).lastPathComponent) vai para a lixeira." : "\($0.count) itens vão para a lixeira." } ?? "")
        }
    }

    private var rows: some View {
        ScrollViewReader { proxy in
            ScrollView {
                LazyVStack(spacing: 0) {
                    if state.searching && !state.rows.isEmpty {
                        Text(state.rows.count == 1 ? "1 ocorrência" : "\(state.rows.count) ocorrências")
                            .font(.app(11)).foregroundStyle(Color(pal.muted))
                            .frame(maxWidth: .infinity, minHeight: 23, alignment: .leading).padding(.horizontal, 8.125)
                        FTRule(color: pal.border)
                    }
                    ForEach(state.rows) { row in
                        FTRowView(state: state, row: row, searching: state.searching, selected: state.selected == row.path, ring: focused && clicked == nil) { action in
                            handle(action, row)
                        }
                        .id(row.path)
                    }
                    if state.searching && state.hitsCut {
                        FTRule(color: pal.border)
                        Text("Parou em \(state.rows.count) ocorrências. Existem mais.")
                            .font(.app(11)).foregroundStyle(Color(pal.muted))
                            .frame(maxWidth: .infinity, minHeight: 23, alignment: .leading).padding(.horizontal, 8.125)
                    }
                    if let note = state.emptyNote {
                        Text(note)
                            .font(.app(state.searching ? 13 : 12)).foregroundStyle(Color(pal.muted))
                            .multilineTextAlignment(state.searching ? .center : .leading)
                            .frame(maxWidth: .infinity, alignment: state.searching ? .center : .leading)
                            .padding(.horizontal, 9.75).padding(.top, state.searching ? 40 : 6.5)
                    }
                    // empty space: the menu of the folder shown
                    Color.clear.frame(height: 40).contentShape(Rectangle())
                        .contextMenu { folderMenu(dir: state.shown) }
                }
                .padding(.horizontal, 4)
            }
            .modifier(FTScroller(pal: pal))
            .focusable()
            .focused($focused)
            .focusEffectDisabled()
            .onKeyPress(phases: .down) { press in keys(press) }
            .onChange(of: state.selected) { _, path in
                // a row picked by Rust (a file made, a path shown) comes into view; a click on a row does not scroll
                if let path, path != clicked { withAnimation(.easeOut(duration: 0.15)) { proxy.scrollTo(path, anchor: .center) } }
            }
        }
    }

    // MARK: actions

    private func handle(_ action: FTRowAction, _ row: FTRow) {
        switch action {
        case .select:
            clicked = row.path
            focused = true
            FT.send("focus", state.id)
            FT.send("select", state.id, ["path": row.path])
        case .activate: FT.send("activate", state.id, ["path": row.path])
        case .toggle:
            FT.send("focus", state.id)
            FT.send("toggle", state.id, ["path": row.path])
        case .into: FT.send("into", state.id, ["path": row.path])
        case .copyPath(let relative):
            let text = relative ? ftRelative(row.path, to: state.root) : row.path
            NSPasteboard.general.clearContents()
            NSPasteboard.general.setString(text, forType: .string)
        case .diff: Task { await FT.act("diff", state.id, ["path": row.path]) }
        case .copy: FT.send("copy", state.id, ["path": row.path])
        case .cut: FT.send("cut", state.id, ["path": row.path])
        case .paste: Task { await FT.act("paste", state.id, ["dir": row.dir ? row.path : (row.path as NSString).deletingLastPathComponent]) }
        case .rename:
            let (base, _) = ftSplitName(row.name)
            ask = FTAsk(kind: .rename, at: row.path, text: base)
        case .trash: trash = [row.path]
        case .newFolder: ask = FTAsk(kind: .newFolder, at: row.dir ? row.path : (row.path as NSString).deletingLastPathComponent, text: "Nova Pasta")
        case .newFile: ask = FTAsk(kind: .newFile, at: row.dir ? row.path : (row.path as NSString).deletingLastPathComponent, text: "Sem título")
        }
    }

    /// The arrows, Enter, Delete and the file shortcuts, as the egui tree has them.
    private func keys(_ press: KeyPress) -> KeyPress.Result {
        let rows = state.rows
        let index = rows.firstIndex { $0.path == state.selected }
        let row = index.map { rows[$0] }
        let command = press.modifiers.contains(.command)
        func go(_ to: Int) -> KeyPress.Result {
            guard rows.indices.contains(to) else { return .handled }
            clicked = nil
            FT.send("select", state.id, ["path": rows[to].path])
            return .handled
        }
        switch press.key {
        case .downArrow: return go(index.map { min($0 + 1, rows.count - 1) } ?? 0)
        case .upArrow: return go(index.map { max($0 - 1, 0) } ?? 0)
        case .home: return go(0)
        case .end: return go(rows.count - 1)
        case .rightArrow:
            guard let row, row.dir else { return .ignored }
            if row.open { return go((index ?? 0) + 1) }
            FT.send("toggle", state.id, ["path": row.path])
            return .handled
        case .leftArrow:
            guard let row, let index else { return .ignored }
            if row.dir && row.open {
                FT.send("toggle", state.id, ["path": row.path])
            } else if let parent = rows[..<index].last(where: { $0.depth < row.depth && $0.dir }) {
                clicked = nil
                FT.send("select", state.id, ["path": parent.path])
            }
            return .handled
        case .return:
            guard let row else { return .ignored }
            if command { handle(.rename, row) } else { FT.send("activate", state.id, ["path": row.path]) }
            return .handled
        case .delete, .deleteForward:
            guard let row, command || press.key == .deleteForward else { return .ignored }
            handle(.trash, row)
            return .handled
        default: break
        }
        guard command, let row else { return .ignored }
        switch press.characters {
        case "c":
            handle(press.modifiers.contains(.option) ? .copyPath(press.modifiers.contains(.shift)) : .copy, row)
            return .handled
        case "x": handle(.cut, row); return .handled
        case "v" where state.clipboard != nil: handle(.paste, row); return .handled
        case "d": handle(.diff, row); return .handled
        case "n": handle(press.modifiers.contains(.option) ? .newFolder : .newFile, row); return .handled
        default: return .ignored
        }
    }

    @ViewBuilder
    private func folderMenu(dir: String) -> some View {
        Button(pasteLabel) { Task { await FT.act("paste", state.id, ["dir": dir]) } }.disabled(state.clipboard == nil)
        Divider()
        Button("Nova Pasta") { ask = FTAsk(kind: .newFolder, at: dir, text: "Nova Pasta") }
        Button("Novo Arquivo") { ask = FTAsk(kind: .newFile, at: dir, text: "Sem título") }
    }

    private var pasteLabel: String { ftPasteLabel(state.clipboard) }
}

func ftPasteLabel(_ clip: FTClip?) -> String {
    guard let clip else { return "Colar" }
    switch (clip.count, clip.cut) {
    case (1, false): return "Colar 1 Item"
    case (let n, false): return "Colar \(n) Itens"
    case (1, true): return "Mover 1 Item para Cá"
    case (let n, true): return "Mover \(n) Itens para Cá"
    }
}

enum FTRowAction {
    case select, activate, toggle, into
    case copyPath(Bool)
    case diff, copy, cut, paste, rename, trash, newFolder, newFile
}

/// One row: the indent guides, the chevron, the icon, the name (in its git color), the git letter.
struct FTRowView: View {
    let state: FTState
    let row: FTRow
    let searching: Bool
    let selected: Bool
    let ring: Bool
    let act: (FTRowAction) -> Void
    @State private var hover = false
    @State private var overChevron = false

    private var pal: FTPalette { FTPalette(dark: state.dark) }

    var body: some View {
        HStack(spacing: 0) {
            if searching { hit } else { tree }
        }
        .frame(height: searching ? 38 : 24)
        .background(RoundedRectangle(cornerRadius: 4).fill(selected || (searching && hover) ? Color(pal.hover) : Color.clear))
        .overlay(RoundedRectangle(cornerRadius: 4).strokeBorder(Color(pal.ring), lineWidth: 2).opacity(selected && ring ? 1 : 0))
        .contentShape(Rectangle())
        .onHover { hover = $0 }
        .onTapGesture(count: 2) { act(.activate) }
        .onTapGesture { act(.select) }
        .contextMenu { menu }
        .help(row.path)
    }

    private var nameColor: Color { row.color.map { Color(hex: $0) } ?? Color(pal.fg) }

    private var tree: some View {
        HStack(spacing: 0) {
            // a line down from each folder above, under its chevron
            ForEach(0..<row.depth, id: \.self) { _ in
                Rectangle().fill(Color(pal.border)).frame(width: 1).padding(.leading, 6).padding(.trailing, 9)
            }
            ZStack {
                if row.dir {
                    RoundedRectangle(cornerRadius: 2).fill(Color(pal.fg).opacity(overChevron ? 0.1 : 0))
                    FTGlyph(name: row.open ? "chevron-down" : "chevron-right", size: 9.75, color: pal.muted)
                }
            }
            .frame(width: 13, height: 13)
            .contentShape(Rectangle())
            .onHover { overChevron = $0 }
            .onTapGesture { if row.dir { act(.toggle) } }
            FTIcon(icon: row.icon, dark: state.dark).padding(.leading, 4.875).padding(.trailing, 3.375)
            Text(row.name)
                .font(.app(13))
                .foregroundStyle(nameColor)
                .underline(row.problem, pattern: .dot, color: nameColor)
                .lineLimit(1).truncationMode(.middle)
            if !row.hint.isEmpty {
                Text(row.hint).font(.app(11)).foregroundStyle(Color(pal.muted)).lineLimit(1).padding(.leading, 6)
            }
            Spacer(minLength: 4)
            if let letter = row.letter, let color = row.letterColor {
                // the app's mono is wider than the reference's: 8.4 points, struck twice for the weight
                Text(letter).font(.custom("Hack", size: 8.4)).foregroundStyle(Color(hex: color))
                    .overlay(Text(letter).font(.custom("Hack", size: 8.4)).foregroundStyle(Color(hex: color)).offset(x: 0.35))
                    .frame(width: 16.25).padding(.trailing, 6.5)
            }
        }
        .padding(.leading, 4)
    }

    /// A filename search's hit: the icon, the name and, under it, the folder it is in.
    private var hit: some View {
        HStack(spacing: 6.5) {
            FTIcon(icon: row.icon, dark: state.dark, size: 13)
            VStack(alignment: .leading, spacing: 1) {
                Text(row.name).font(.app(12)).foregroundStyle(nameColor).lineLimit(1)
                if !row.hint.isEmpty { Text(row.hint).font(.app(10.5)).foregroundStyle(Color(pal.muted)).lineLimit(1) }
            }
            Spacer(minLength: 0)
        }
        .padding(.horizontal, 8.125)
    }

    @ViewBuilder
    private var menu: some View {
        Button("Abrir") { act(row.dir ? .into : .activate) }
        Divider()
        Button("Copiar Caminho") { act(.copyPath(false)) }
        Button("Copiar Caminho Relativo") { act(.copyPath(true)) }
        if !row.dir, let letter = row.letter, ["M", "R", "!"].contains(letter) {
            Divider()
            Button("Mostrar Diff") { act(.diff) }
        }
        Divider()
        Button("Copiar") { act(.copy) }
        Button("Recortar") { act(.cut) }
        Button(ftPasteLabel(state.clipboard)) { act(.paste) }.disabled(state.clipboard == nil)
        Divider()
        Button("Renomear") { act(.rename) }
        Button("Mover para a Lixeira") { act(.trash) }
        Divider()
        Button("Nova Pasta") { act(.newFolder) }
        Button("Novo Arquivo") { act(.newFile) }
    }
}

/// The Alterações panel over the rows: the files changed against the HEAD, each with its git letter and color.
struct FTChangesPanel: View {
    let state: FTState
    private var pal: FTPalette { FTPalette(dark: state.dark) }

    var body: some View {
        VStack(spacing: 0) {
            HStack {
                Text("Alterações").font(.app(13))
                Spacer()
                Text(state.changes.count == 1 ? "1 arquivo alterado" : "\(state.changes.count) arquivos alterados").font(.app(11)).foregroundStyle(Color(pal.muted))
            }
            .padding(.horizontal, 8).frame(height: 26)
            FTRule(color: pal.border)
            if state.changes.isEmpty {
                Text("Nenhuma alteração contra o HEAD.").font(.app(12)).foregroundStyle(Color(pal.muted))
                    .frame(maxWidth: .infinity, alignment: .leading).padding(.horizontal, 10).padding(.top, 6)
                Spacer()
            } else {
                ScrollView {
                    LazyVStack(spacing: 0) {
                        ForEach(state.changes) { change in FTChangeRow(state: state, change: change) }
                    }
                    .padding(.horizontal, 4)
                }
            }
        }
        .background(Color(pal.bg))
    }
}

struct FTChangeRow: View {
    let state: FTState
    private var pal: FTPalette { FTPalette(dark: state.dark) }
    let change: FTChange
    @State private var hover = false

    var body: some View {
        HStack(spacing: 6) {
            Text(change.letter).font(.custom("Hack", size: 10)).foregroundStyle(Color(hex: change.color)).frame(width: 20)
            FTIcon(icon: change.icon, dark: state.dark)
            Text(change.name).font(.app(13)).foregroundStyle(Color(hex: change.color)).lineLimit(1)
            if !change.dir.isEmpty { Text(change.dir).font(.app(11)).foregroundStyle(Color(pal.muted)).lineLimit(1).truncationMode(.head) }
            Spacer(minLength: 0)
        }
        .padding(.horizontal, 4).frame(height: 24)
        .background(RoundedRectangle(cornerRadius: 4).fill(hover ? Color(pal.hover) : Color.clear))
        .contentShape(Rectangle())
        .onHover { hover = $0 }
        .onTapGesture {
            FT.send("focus", state.id)
            FT.send("open_change", state.id, ["path": change.path])
        }
        .help("\(change.path) (\(ftStatusWord(change.letter)))")
    }
}

/// The name asked for a new file or folder, or a rename; Rust refuses a name the system would (the words come back here).
struct FTNameSheet: View {
    let treeId: String
    let ask: FTAsk
    @Environment(\.dismiss) private var dismiss
    @State private var text = ""
    @State private var problem: String?
    @State private var busy = false
    @FocusState private var focused: Bool

    private var title: String {
        switch ask.kind {
        case .newFile: "Novo Arquivo"
        case .newFolder: "Nova Pasta"
        case .rename: "Renomear"
        }
    }

    var body: some View {
        VStack(alignment: .leading, spacing: 10) {
            Text(title).font(.headline)
            TextField(ask.kind == .newFile ? "Nome do novo arquivo" : ask.kind == .newFolder ? "Nome da nova pasta" : "Novo nome", text: $text)
                .textFieldStyle(.roundedBorder).focused($focused).onSubmit(commit)
            if let problem { Text(problem).font(.app(11)).foregroundStyle(.red) }
            HStack {
                Spacer()
                Button("Cancelar", role: .cancel) { dismiss() }.keyboardShortcut(.cancelAction)
                Button(ask.kind == .rename ? "Renomear" : "Criar", action: commit)
                    .keyboardShortcut(.defaultAction)
                    .disabled(text.trimmingCharacters(in: .whitespaces).isEmpty || busy)
            }
        }
        .padding(16).frame(width: 300)
        .onAppear {
            text = ask.text
            focused = true
        }
    }

    private func commit() {
        guard !busy else { return }
        busy = true
        Task {
            let reply: MagoReply
            switch ask.kind {
            case .newFile: reply = await FT.act("create", treeId, ["dir": ask.at, "name": text, "folder": false])
            case .newFolder: reply = await FT.act("create", treeId, ["dir": ask.at, "name": text, "folder": true])
            case .rename:
                // the extension stays when only the base was edited
                let (_, ext) = ftSplitName((ask.at as NSString).lastPathComponent)
                reply = await FT.act("rename", treeId, ["path": ask.at, "name": text + ext])
            }
            busy = false
            if reply.ok { dismiss() } else { problem = reply.text }
        }
    }
}
