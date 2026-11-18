import AppKit
import SwiftUI

/// A card over a dimmed window: its title, its content and the buttons (Esc cancels, Return confirms).
struct DialogCard<Content: View>: View {
    let title: String
    var confirm = "OK"
    var danger = false
    var canConfirm = true
    var width: CGFloat = 480
    var onConfirm: (() -> Void)?
    let onCancel: () -> Void
    @ViewBuilder var content: Content

    var body: some View {
        ZStack {
            Color.black.opacity(0.28).ignoresSafeArea().onTapGesture(perform: onCancel)
            VStack(alignment: .leading, spacing: 14) {
                Text(title).font(.system(size: 16, weight: .semibold))
                content
                HStack {
                    Spacer()
                    Button(onConfirm == nil ? "Fechar" : "Cancelar", action: onCancel).keyboardShortcut(.cancelAction)
                    if let onConfirm {
                        Button(confirm, action: onConfirm).keyboardShortcut(.defaultAction).buttonStyle(.borderedProminent)
                            .tint(danger ? .red : .accentColor).disabled(!canConfirm)
                    }
                }
            }
            .padding(22).frame(width: width)
            .background(.regularMaterial, in: RoundedRectangle(cornerRadius: 14, style: .continuous))
            .overlay(RoundedRectangle(cornerRadius: 14, style: .continuous).stroke(Color.primary.opacity(0.1), lineWidth: 1))
            .shadow(color: .black.opacity(0.25), radius: 24, y: 8)
        }
        .transition(.opacity)
    }
}

/// A small caption over a field.
private struct Field<Content: View>: View {
    let caption: String
    @ViewBuilder var content: Content
    var body: some View {
        VStack(alignment: .leading, spacing: 4) {
            Text(caption).font(.system(size: 12)).foregroundStyle(.secondary)
            content
        }
    }
}

/// A folder's field: its text and a native picker.
private struct FolderField: View {
    let caption: String
    let placeholder: String
    @Binding var text: String

    var body: some View {
        Field(caption: caption) {
            HStack {
                TextField(placeholder, text: $text).textFieldStyle(.roundedBorder).font(.system(size: 13, design: .monospaced))
                Button("Escolher…") {
                    let panel = NSOpenPanel()
                    panel.canChooseFiles = false
                    panel.canChooseDirectories = true
                    panel.canCreateDirectories = true
                    if panel.runModal() == .OK, let path = panel.url?.path { text = path }
                }
            }
        }
    }
}

/// Everything the window asks over the canvas: the app's dialogs (Rust keeps them) and the sidebar's sheets (Swift keeps them).
struct DialogsView: View {
    let state: ShellState
    @Binding var sheet: SpaceSheet?

    var body: some View {
        ZStack {
            if let wipe = state.dialogs.wipe {
                WipeView(wipe: wipe)
            } else if let changelog = state.dialogs.changelog {
                ChangelogView(changelog: changelog)
            } else if let tour = state.dialogs.tour {
                TourView(tour: tour)
            } else if let welcome = state.dialogs.welcome {
                WelcomeView(welcome: welcome)
            } else if let terminal = state.dialogs.terminal {
                TerminalDialogView(dialog: terminal)
            } else if let export = state.dialogs.export {
                ExportView(export: export)
            } else if let sheet {
                SpaceSheetView(sheet: sheet) { self.sheet = nil }
            }
            if let palette = state.dialogs.palette {
                PaletteView(palette: palette)
            }
        }
        .animation(.easeOut(duration: 0.15), value: state.dialogs)
    }
}

// MARK: Novo Chat / Novo Terminal

private struct TerminalDialogView: View {
    let dialog: TerminalDialog
    @State private var name = ""
    @State private var command = ""
    @State private var cwd = ""

    var body: some View {
        let title = dialog.editing ? "Editar Terminal" : dialog.chat ? "Novo Chat" : "Novo Terminal"
        DialogCard(title: title, confirm: dialog.editing ? "Salvar" : "Criar", width: 560, onConfirm: { send("confirm") }, onCancel: { send("cancel") }) {
            if !dialog.editing {
                Picker("", selection: Binding(get: { dialog.chat }, set: { set("chat", $0) })) {
                    Text("Chat").tag(true)
                    Text("Terminal").tag(false)
                }
                .pickerStyle(.segmented).labelsHidden()
            }
            HStack {
                Text("INÍCIO RÁPIDO").font(.system(size: 12)).foregroundStyle(.secondary)
                Spacer()
                Button("Editar presets…") { shellVerb("settings", ["open": true]) }.buttonStyle(.link).font(.system(size: 12))
            }
            LazyVGrid(columns: Array(repeating: GridItem(.flexible(), spacing: 4), count: 5), spacing: 4) {
                ForEach(Array(dialog.presets.enumerated()), id: \.offset) { index, preset in
                    Button { set("preset", index) } label: {
                        VStack(spacing: 4) {
                            Image(systemName: symbol(preset)).font(.system(size: 18)).frame(height: 24)
                            Text(preset.name).font(.system(size: 12)).lineLimit(1)
                        }
                        .frame(maxWidth: .infinity).frame(height: 55)
                        .foregroundStyle(preset.selected ? Color.primary : Color.secondary)
                        .background(RoundedRectangle(cornerRadius: 11).fill(Color.accent007aff.opacity(preset.selected ? 0.12 : 0)))
                        .contentShape(Rectangle())
                    }
                    .buttonStyle(.plain)
                }
            }
            if dialog.chat {
                FolderField(caption: "Pasta", placeholder: "ex. ~/projeto ou /home/usuario/projeto", text: $cwd)
            } else {
                Field(caption: "Nome do Terminal") { TextField("Nome do terminal", text: $name).textFieldStyle(.roundedBorder) }
                Field(caption: "Comando") {
                    TextField("ex: claude, codex, ou deixe vazio para shell", text: $command).textFieldStyle(.roundedBorder).font(.system(size: 13, design: .monospaced))
                }
                FolderField(caption: "Diretório de Trabalho", placeholder: "ex. ~/projeto ou /home/usuario/projeto", text: $cwd)
                Toggle(isOn: Binding(get: { dialog.maestro }, set: { set("maestro", $0) })) {
                    VStack(alignment: .leading) {
                        Text("Luna")
                        Text("Faz deste terminal a Luna, capaz de reger o restante do canvas.").font(.system(size: 11)).foregroundStyle(.secondary)
                    }
                }
                Toggle(isOn: Binding(get: { dialog.startInChat }, set: { set("startInChat", $0) })) {
                    VStack(alignment: .leading) {
                        Text("Iniciar no modo de chat")
                        Text("Um clique nele abre o chat em vez da tela.").font(.system(size: 11)).foregroundStyle(.secondary)
                    }
                }
            }
        }
        .onAppear { (name, command, cwd) = (dialog.name, dialog.command, dialog.cwd) }
        // a quick start changes the name and the command from Rust's side
        .onChange(of: dialog.name) { _, next in if next != name { name = next } }
        .onChange(of: dialog.command) { _, next in if next != command { command = next } }
        .onChange(of: name) { _, next in if next != dialog.name { set("name", next) } }
        .onChange(of: command) { _, next in if next != dialog.command { set("command", next) } }
        .onChange(of: cwd) { _, next in if next != dialog.cwd { set("cwd", next) } }
    }

    private func send(_ action: String) { shellVerb("dialog", ["dialog": "terminal", "action": action]) }
    private func set(_ field: String, _ value: Any) { shellVerb("dialog", ["dialog": "terminal", "action": "set", "field": field, "value": value]) }

    private func symbol(_ preset: PresetRow) -> String {
        switch preset.icon.isEmpty ? (preset.command.split(separator: " ").first.map(String.init) ?? "") : preset.icon {
        case "claude": "sparkle"
        case "codex": "chevron.left.forwardslash.chevron.right"
        case "opencode": "rectangle.portrait"
        case "agents": "person.3"
        case "monitor": "gauge"
        case "search": "magnifyingglass"
        case "composer": "text.bubble"
        case "palette": "bolt"
        case "grid": "square.grid.3x3"
        case "eye": "eye"
        case "map": "map"
        default: "terminal"
        }
    }
}

// MARK: Exportar, confirmations, onboarding

private struct ExportView: View {
    let export: ExportDialog

    var body: some View {
        DialogCard(title: "Exportar imagem", confirm: "Exportar", width: 360, onConfirm: { send("confirm") }, onCancel: { send("cancel") }) {
            Field(caption: "Formato") {
                Picker("", selection: Binding(get: { export.svg }, set: { set("svg", $0) })) { Text("PNG").tag(false); Text("SVG").tag(true) }
                    .pickerStyle(.segmented).labelsHidden()
            }
            Field(caption: "Escala") {
                Picker("", selection: Binding(get: { Int(export.scale) }, set: { set("scale", $0) })) { Text("1×").tag(1); Text("2×").tag(2); Text("3×").tag(3) }
                    .pickerStyle(.segmented).labelsHidden()
            }
            Toggle("Fundo", isOn: Binding(get: { export.background }, set: { set("background", $0) }))
        }
    }

    private func send(_ action: String) { shellVerb("dialog", ["dialog": "export", "action": action]) }
    private func set(_ field: String, _ value: Any) { shellVerb("dialog", ["dialog": "export", "action": "set", "field": field, "value": value]) }
}

/// Limpar todos os dados asks first: what goes, where, and that it can't be undone.
private struct WipeView: View {
    let wipe: WipeDialog

    var body: some View {
        DialogCard(title: "Apagar todos os dados do MagoAgent?", confirm: "Apagar tudo", danger: true, width: 520,
                   onConfirm: { send("confirm") }, onCancel: { send("cancel") }) {
            VStack(alignment: .leading, spacing: 4) {
                Text("O MagoAgent fecha os agentes e os terminais, apaga e abre de novo como na primeira vez. Vão embora:")
                Text("• os desenhos, os espaços, as notas e as tarefas")
                Text("• os chats e as conversas guardadas")
                Text("• os ajustes e a memória da Luna")
                if wipe.keychain {
                    Text("• o cofre de senhas do navegador e os tokens do GitHub, do Spotify e do WhatsApp")
                    Text("• a pasta ~/.magoagent, onde trabalham os chats sem pasta escolhida")
                } else {
                    Text("Esta é a versão de desenvolvimento: só a pasta dela é apagada (o cofre e o ~/.magoagent são do app instalado).")
                }
                Text("Os hooks instalados no Claude Code e no Codex ficam como estão. Não dá para desfazer.")
                Text(wipe.folders.joined(separator: "\n")).font(.system(size: 11, design: .monospaced)).foregroundStyle(.secondary).padding(.top, 4)
            }
            .font(.system(size: 13)).fixedSize(horizontal: false, vertical: true)
        }
    }

    private func send(_ action: String) { shellVerb("dialog", ["dialog": "wipe", "action": action]) }
}

private struct WelcomeView: View {
    let welcome: WelcomeDialog
    @State private var name = ""
    @State private var folder = ""

    var body: some View {
        DialogCard(title: "Bem-vindo ao MagoAgent", confirm: "Começar", width: 520, onConfirm: finish, onCancel: finish) {
            Text("Um quadro branco com terminais, chats e agentes que trabalham juntos.").font(.system(size: 13)).foregroundStyle(.secondary)
            if welcome.noSpace {
                Field(caption: "Nome do primeiro espaço") { TextField("Projeto", text: $name).textFieldStyle(.roundedBorder) }
                FolderField(caption: "Pasta raiz", placeholder: "~/projetos/loja", text: $folder)
            }
            if !welcome.error.isEmpty { Text(welcome.error).font(.system(size: 12)).foregroundStyle(.red) }
        }
    }

    private func finish() { shellVerb("welcome", ["name": name, "folder": folder]) }
}

private struct TourView: View {
    let tour: TourDialog

    var body: some View {
        DialogCard(title: "Apresentação", confirm: tour.step + 1 < tour.steps ? "Próximo" : "Concluir", width: 380,
                   onConfirm: { shellVerb("tour", ["action": "next"]) }, onCancel: { shellVerb("tour", ["action": "end"]) }) {
            Text("Passo \(tour.step + 1) de \(tour.steps)").font(.system(size: 13)).foregroundStyle(.secondary)
            if tour.step > 0 { Button("Voltar") { shellVerb("tour", ["action": "back"]) }.buttonStyle(.link) }
        }
    }
}

private struct ChangelogView: View {
    let changelog: ChangelogDialog

    var body: some View {
        let title = changelog.installed ? "O que há de novo na \(changelog.version)" : "Versão \(changelog.version) disponível"
        DialogCard(title: title, confirm: confirmLabel, width: 520, onConfirm: confirm, onCancel: close) {
            ScrollView { Text(changelog.notes).font(.system(size: 13)).frame(maxWidth: .infinity, alignment: .leading) }.frame(maxHeight: 260)
            if !changelog.installed && !changelog.confirming {
                HStack {
                    Button("Atualizar depois") { shellVerb("changelog", ["choice": "later"]) }
                    Button("Pular esta versão") { shellVerb("changelog", ["choice": "skip"]) }
                }
                .buttonStyle(.link).font(.system(size: 12))
            }
        }
    }

    private var confirmLabel: String { changelog.installed ? "Fechar" : changelog.confirming ? "Atualizar e reabrir" : "Atualizar agora" }

    private func confirm() {
        if changelog.installed { close() } else { shellVerb("changelog", ["choice": changelog.confirming ? "restart" : "now"]) }
    }

    private func close() { shellVerb("changelog", ["choice": changelog.confirming ? "cancel" : "later"]) }
}

// MARK: space and group sheets

private struct SpaceSheetView: View {
    let sheet: SpaceSheet
    let close: () -> Void
    @State private var name = ""
    @State private var folder = ""

    var body: some View {
        switch sheet {
        case .newSpace:
            DialogCard(title: "Novo espaço", confirm: "Criar", canConfirm: !name.trimmingCharacters(in: .whitespaces).isEmpty, onConfirm: {
                appAction("workspaces", ["action": "create", "name": name, "folder": folder])
                close()
            }, onCancel: close) {
                Field(caption: "Nome") { TextField("Projeto", text: $name).textFieldStyle(.roundedBorder) }
                FolderField(caption: "Pasta raiz", placeholder: "~/projetos/loja", text: $folder)
                Text("Terminais e árvores novos deste espaço começam nessa pasta.").font(.system(size: 12)).foregroundStyle(.secondary)
            }
        case .removeSpace(let id, let name, let running):
            DialogCard(title: "Remover espaço", confirm: "Remover", danger: true, onConfirm: {
                appAction("workspaces", ["action": "remove", "name": id])
                close()
            }, onCancel: close) {
                let ends = running == 0 ? "" : running == 1 ? " e o terminal que roda nele é encerrado" : " e os \(running) terminais que rodam nele são encerrados"
                Text("\"\(name)\" sai da lista\(ends). O desenho dele continua no disco.").font(.system(size: 13)).fixedSize(horizontal: false, vertical: true)
            }
        case .newGroup:
            DialogCard(title: "Novo grupo", confirm: "Criar", canConfirm: !name.trimmingCharacters(in: .whitespaces).isEmpty, onConfirm: {
                appAction("workspace_groups", ["action": "create", "name": name])
                close()
            }, onCancel: close) {
                Field(caption: "Nome do grupo") { TextField("Novo grupo", text: $name).textFieldStyle(.roundedBorder) }
            }
            .onAppear { name = "Novo grupo" }
        case .renameGroup(let id, let current):
            DialogCard(title: "Renomear grupo", confirm: "Renomear", canConfirm: !name.trimmingCharacters(in: .whitespaces).isEmpty, onConfirm: {
                appAction("workspace_groups", ["action": "rename", "name": id, "new_name": name])
                close()
            }, onCancel: close) {
                Field(caption: "Nome do grupo") { TextField("Nome do grupo", text: $name).textFieldStyle(.roundedBorder) }
            }
            .onAppear { name = current }
        case .removeGroup(let id, _):
            DialogCard(title: "Apagar Grupo", confirm: "Apagar", danger: true, onConfirm: {
                appAction("workspace_groups", ["action": "remove", "name": id])
                close()
            }, onCancel: close) {
                Text("Este grupo será excluído permanentemente. Os espaços de trabalho dentro dele serão movidos para não agrupados.")
                    .font(.system(size: 13)).fixedSize(horizontal: false, vertical: true)
            }
        }
    }
}

// MARK: the palette

/// Cmd+/: the searchable list of commands, Rust's own rows.
private struct PaletteView: View {
    let palette: PaletteState
    @State private var query = ""
    @State private var selected = 0
    @FocusState private var focused: Bool

    var body: some View {
        ZStack(alignment: .top) {
            Color.black.opacity(0.35).ignoresSafeArea().onTapGesture { shellVerb("palette", ["action": "close"]) }
            VStack(spacing: 12) {
                TextField(palette.nodes ? "Buscar nós para conectar…" : "Procure menus, comandos e descubra gemas ocultas", text: $query)
                    .textFieldStyle(.plain).font(.system(size: 16)).focused($focused)
                    .padding(.horizontal, 12).frame(height: 40)
                    .background(RoundedRectangle(cornerRadius: 8).fill(Color.primary.opacity(0.06)))
                    .overlay(RoundedRectangle(cornerRadius: 8).stroke(Color.accent007aff, lineWidth: 1))
                    .onSubmit(run)
                    .onKeyPress(.downArrow) { selected = min(selected + 1, max(palette.rows.count - 1, 0)); return .handled }
                    .onKeyPress(.upArrow) { selected = max(selected - 1, 0); return .handled }
                    .onKeyPress(.escape) { shellVerb("palette", ["action": "close"]); return .handled }
                ScrollViewReader { proxy in
                    ScrollView {
                        LazyVStack(alignment: .leading, spacing: 0) {
                            if palette.rows.isEmpty { Text("Nenhum resultado").foregroundStyle(.secondary).padding(8) }
                            ForEach(Array(palette.rows.enumerated()), id: \.offset) { index, row in
                                if index == 0 || palette.rows[index - 1].group != row.group, !row.group.isEmpty {
                                    Text(row.group).font(.system(size: 15, weight: .semibold)).padding(.top, index == 0 ? 0 : 14).padding(.bottom, 4)
                                }
                                Button { selected = index; run() } label: {
                                    HStack {
                                        Text(row.label).font(.system(size: 15))
                                        Spacer()
                                        Text(row.keys).font(.system(size: 11)).foregroundStyle(.secondary)
                                    }
                                    .padding(.horizontal, 12).frame(height: 38)
                                    .background(RoundedRectangle(cornerRadius: 8).fill(Color.primary.opacity(index == selected ? 0.08 : 0)))
                                    .contentShape(Rectangle())
                                }
                                .buttonStyle(.plain).id(index)
                            }
                        }
                    }
                    .onChange(of: selected) { _, next in proxy.scrollTo(next) }
                }
                .frame(maxHeight: 420)
            }
            .padding(20).frame(width: 672)
            .background(.regularMaterial, in: RoundedRectangle(cornerRadius: 12, style: .continuous))
            .overlay(RoundedRectangle(cornerRadius: 12, style: .continuous).stroke(Color.primary.opacity(0.1), lineWidth: 1))
            .shadow(color: .black.opacity(0.25), radius: 24, y: 8)
            .padding(.top, 47)
        }
        .onAppear { query = palette.query; focused = true }
        .onChange(of: query) { _, next in
            if next != palette.query { selected = 0; shellVerb("palette", ["action": "query", "query": next]) }
        }
    }

    private func run() { shellVerb("palette", ["action": "run", "index": selected]) }
}
