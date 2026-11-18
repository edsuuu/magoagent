import AppKit
import SwiftUI

/// The editor pane of a node: the strip of tabs with its buttons, then the active tab's body (code, a picture, a PDF, a diff...).
struct FTEditorPane: View {
    let state: FTState

    var body: some View {
        VStack(spacing: 0) {
            FTTabStrip(state: state)
            Divider()
            FTBody(state: state)
        }
        .background(Color(nsColor: .textBackgroundColor).opacity(0.0))
    }
}

/// The tabs, small as the IDE's: the file's icon and name, the one forward on a rounded chip, an unsaved text's dot where the X goes.
struct FTTabStrip: View {
    let state: FTState
    @State private var problemsShown = false

    private var activeTab: FTTab? { state.tabs.first { $0.path == state.active } }

    var body: some View {
        HStack(spacing: 0) {
            ScrollViewReader { proxy in
                ScrollView(.horizontal, showsIndicators: false) {
                    HStack(spacing: 2) {
                        ForEach(state.tabs) { tab in FTTabChip(state: state, tab: tab).id(tab.path) }
                    }
                    .padding(.horizontal, 4)
                }
                .onChange(of: state.active) { _, path in if let path { proxy.scrollTo(path) } }
            }
            // the strip's bare part is the node's: a click picks it on the canvas
            Color.clear.frame(minWidth: 4).contentShape(Rectangle()).onTapGesture { FT.send("focus", state.id) }
            buttons
        }
        .frame(height: 30)
        .background(Color.primary.opacity(0.05))
    }

    @ViewBuilder
    private var buttons: some View {
        HStack(spacing: 2) {
            if state.errorCount + state.warningCount > 0 || state.problemsOpen {
                Button {
                    problemsShown.toggle()
                    FT.send("problems_panel", state.id, ["open": problemsShown])
                } label: {
                    HStack(spacing: 4) {
                        if state.errorCount > 0 { Label("\(state.errorCount)", systemImage: "xmark.circle.fill").foregroundStyle(.red) }
                        if state.warningCount > 0 { Label("\(state.warningCount)", systemImage: "exclamationmark.triangle.fill").foregroundStyle(.yellow) }
                    }
                    .font(.system(size: 11)).labelStyle(.titleAndIcon)
                }
                .buttonStyle(.plain).help("Problemas")
                .popover(isPresented: $problemsShown, arrowEdge: .bottom) { FTProblemsList(state: state) }
                .onChange(of: problemsShown) { _, open in if !open { FT.send("problems_panel", state.id, ["open": false]) } }
            }
            if let tab = activeTab {
                switch tab.kind {
                case "text":
                    iconButton("square.and.arrow.down", tip: "Salvar", on: tab.dirty) { FT.send("save", state.id) }
                    Menu {
                        Toggle("Quebrar Linhas", isOn: Binding(get: { state.wrap }, set: { _ in FT.send("wrap", state.id) }))
                    } label: { Image(systemName: "slider.horizontal.3").font(.system(size: 11)) }
                        .menuStyle(.borderlessButton).menuIndicator(.hidden).fixedSize().help("Configurações do Editor")
                case "media", "external":
                    iconButton("arrow.up.right.square", tip: "Abrir no App Padrão", on: true) { FT.send("open_default", state.id, ["path": tab.path]) }
                default: EmptyView()
                }
            }
            iconButton("xmark", tip: "Ocultar Editor", on: true) { FT.send("editor", state.id, ["show": false]) }
        }
        .padding(.trailing, 4)
    }

    private func iconButton(_ symbol: String, tip: String, on: Bool, _ action: @escaping () -> Void) -> some View {
        Button(action: action) {
            Image(systemName: symbol).font(.system(size: 11)).foregroundStyle(on ? Color.secondary : Color.secondary.opacity(0.4)).frame(width: 20, height: 20)
        }
        .buttonStyle(.plain).disabled(!on).help(tip)
    }
}

struct FTTabChip: View {
    let state: FTState
    let tab: FTTab
    @State private var hover = false
    @State private var overClose = false

    private var active: Bool { state.active == tab.path }

    var body: some View {
        HStack(spacing: 5) {
            FTIcon(icon: tab.icon, dark: state.dark, size: 14)
            Text(tab.name).font(tab.preview ? Font.system(size: 12).italic() : Font.system(size: 12)).lineLimit(1).frame(maxWidth: 170).foregroundStyle(active ? Color.primary : Color.secondary)
            ZStack {
                if tab.dirty && !overClose {
                    Circle().fill(Color(red: 0.94, green: 0.53, blue: 0.24)).frame(width: 6, height: 6)
                } else if hover || active {
                    Image(systemName: "xmark").font(.system(size: 8, weight: .semibold)).foregroundStyle(.secondary)
                }
            }
            .frame(width: 14, height: 14)
            .background(RoundedRectangle(cornerRadius: 3).fill(overClose ? Color.primary.opacity(0.15) : Color.clear))
            .onHover { overClose = $0 }
            .onTapGesture { FT.send("tab_close", state.id, ["path": tab.path]) }
        }
        .padding(.leading, 6).padding(.trailing, 4).frame(height: 22)
        .background(
            RoundedRectangle(cornerRadius: 6)
                .fill(active ? Color.accentColor.opacity(0.2) : (hover ? Color.primary.opacity(0.07) : Color.clear))
        )
        .overlay(RoundedRectangle(cornerRadius: 6).stroke(active ? Color.accentColor.opacity(0.45) : Color.clear))
        .contentShape(Rectangle())
        .onHover { hover = $0 }
        .onTapGesture { FT.send("tab_select", state.id, ["path": tab.path]) }
        .contextMenu {
            if tab.preview { Button("Manter Aberto") { FT.send("tab_pin", state.id, ["path": tab.path]) } }
            Button("Fechar Aba") { FT.send("tab_close", state.id, ["path": tab.path]) }
            Button("Fechar Outras Abas") { FT.send("tab_close_others", state.id, ["path": tab.path]) }
            Button("Fechar Todas as Abas") { FT.send("tab_close_all", state.id) }
            Button("Copiar Caminho") {
                NSPasteboard.general.clearContents()
                NSPasteboard.general.setString(tab.path, forType: .string)
            }
            Button("Mostrar na Árvore") { FT.send("reveal_in_tree", state.id, ["path": tab.path]) }
        }
        .help(tab.dirty ? "\(tab.path), alterações não salvas" : tab.path)
    }
}

/// The list of the project's problems: pressed, one opens its file at its line.
struct FTProblemsList: View {
    let state: FTState

    var body: some View {
        ScrollView {
            LazyVStack(alignment: .leading, spacing: 0) {
                if state.problems.isEmpty {
                    Text("Nenhum problema encontrado.").font(.system(size: 12)).foregroundStyle(.secondary).padding(10)
                }
                ForEach(state.problems) { file in
                    Text(file.rel).font(.system(size: 12, weight: .medium)).padding(.horizontal, 10).padding(.top, 8).padding(.bottom, 2)
                    ForEach(Array(file.problems.enumerated()), id: \.offset) { _, problem in
                        Button {
                            FT.send("goto", state.id, ["path": file.path, "line": problem.line, "col": problem.col, "pinned": true])
                        } label: {
                            HStack(alignment: .top, spacing: 6) {
                                Image(systemName: problem.severity == 1 ? "xmark.circle.fill" : "exclamationmark.triangle.fill")
                                    .foregroundStyle(problem.severity == 1 ? Color.red : Color.yellow).font(.system(size: 11))
                                Text(problem.message).font(.system(size: 12)).multilineTextAlignment(.leading).lineLimit(3)
                                Spacer(minLength: 6)
                                Text("\(problem.line + 1):\(problem.col + 1)").font(.system(size: 11)).foregroundStyle(.secondary)
                            }
                            .padding(.horizontal, 10).padding(.vertical, 3).contentShape(Rectangle())
                        }
                        .buttonStyle(.plain)
                    }
                }
            }
        }
        .frame(width: 460, height: 320)
    }
}

/// The active tab's body.
struct FTBody: View {
    let state: FTState

    var body: some View {
        if state.active == nil {
            Text("Selecione um arquivo para editá-lo ou pré-visualizá-lo aqui.")
                .font(.system(size: 12)).foregroundStyle(.secondary).multilineTextAlignment(.center)
                .padding(.horizontal, 13).frame(maxWidth: .infinity, maxHeight: .infinity)
        } else if let doc = MagoStore.shared.decode(FTDoc.self, topic: "filetree:\(state.id):doc"), doc.path == state.active || doc.kind == "none" {
            content(doc)
        } else {
            Color.clear
        }
    }

    @ViewBuilder
    private func content(_ doc: FTDoc) -> some View {
        switch doc.kind {
        case "text": FTTextBody(state: state, doc: doc)
        case "media": FTMediaView(state: state, doc: doc)
        case "refused":
            FTMessage(text: doc.message ?? "", links: [("Mostrar na Pasta", { FT.send("reveal", state.id, ["path": doc.path]) })])
        case "external":
            FTMessage(
                text: doc.message ?? "",
                links: [
                    ("Abrir no App Padrão", { FT.send("open_default", state.id, ["path": doc.path]) }),
                    ("Mostrar na Pasta", { FT.send("reveal", state.id, ["path": doc.path]) }),
                ])
        case "diff": FTCommitDiff(text: doc.diff ?? "")
        case "compare": if let compare = doc.compare { FTCompareView(state: state, tab: doc.path, compare: compare) }
        case "results": if let results = doc.results { FTResultsView(state: state, results: results) }
        default: Color.clear
        }
    }
}

/// A text: the "changed on disk" banner when it is so, and the code.
struct FTTextBody: View {
    let state: FTState
    let doc: FTDoc

    var body: some View {
        let live = MagoStore.shared.decode(FTLive.self, topic: "filetree:\(state.id):live")
        VStack(spacing: 0) {
            if live?.external == true {
                HStack {
                    Text("Este arquivo mudou no disco enquanto você editava.").font(.system(size: 11))
                    Spacer()
                    Button("Salvar o Meu") { FT.send("save", state.id) }.controlSize(.small)
                }
                .padding(.horizontal, 8).frame(height: 28).background(Color.primary.opacity(0.06))
                Divider()
            }
            if let theme = FT.theme {
                FTCodeEditor(treeId: state.id, doc: doc, live: live?.loadRev == doc.loadRev ? live : nil, wrap: state.wrap, theme: theme)
            } else {
                Color.clear
            }
        }
    }
}
