import SwiftUI

/// "Localizar nos Arquivos" (Cmd+Shift+F): the text to find (case, whole words, regular expression, across lines), an optional file mask,
/// the scope, the matches one to a line. The search is Rust's (`editor::find` over the root's text); a click shows a match in the pane as the
/// preview tab, a double-click or Enter opens it for good, "Abrir na janela de busca" keeps the list in a tab.
struct FTFindSheet: View {
    let treeId: String
    let root: String
    @State private var query = ""
    @State private var mask = ""
    @State private var folder = ""
    @FocusState private var focused: Bool
    @State private var seenFocus: UInt64 = 0

    private var find: FTFind? { MagoStore.shared.decode(FTFind.self, topic: "filetree:\(treeId):find") }

    var body: some View {
        let find = self.find
        VStack(spacing: 0) {
            header(find)
            Divider()
            searchRow(find)
            Divider()
            scopes(find)
            Divider()
            if let error = find?.error {
                Text(error).font(.app(12)).foregroundStyle(.red).frame(maxWidth: .infinity, alignment: .leading).padding(10)
            }
            FTHitList(
                treeId: treeId, hits: find?.hits ?? [], selected: find?.selected,
                pick: { index, _ in FT.send("find_pick", treeId, ["index": index, "open": false]) },
                open: { hit in
                    if let index = find?.hits.firstIndex(of: hit) { FT.send("find_pick", treeId, ["index": index, "open": true]) }
                })
            Divider()
            HStack {
                Text("Enter abre a ocorrência; Cmd+Enter guarda a lista numa aba.").font(.app(11)).foregroundStyle(.secondary)
                Spacer()
                Button("Abrir na janela de busca") { FT.send("find_tab", treeId) }.disabled(find?.hits.isEmpty ?? true)
                Button("Fechar") { FT.send("find_close", treeId) }.keyboardShortcut(.cancelAction)
            }
            .controlSize(.small).padding(10)
        }
        .frame(minWidth: 620, idealWidth: 820, minHeight: 440, idealHeight: 620)
        .onAppear {
            if let find { (query, mask, folder) = (find.query, find.mask, find.folder) }
            focused = true
        }
        .onChange(of: find?.focusSeq) { _, seq in
            if let seq, seq != seenFocus {
                seenFocus = seq
                focused = true
            }
        }
        .onChange(of: find?.query) { _, text in if let text, text != query { query = text } }
    }

    private func header(_ find: FTFind?) -> some View {
        HStack(spacing: 8) {
            Text("Localizar nos Arquivos").font(.app(15))
            Text(find?.countNote ?? "").font(.app(13)).foregroundStyle(.secondary)
            if find?.searching == true { ProgressView().controlSize(.small) }
            Spacer()
            Toggle("Máscara de arquivo", isOn: Binding(get: { find?.maskOn ?? false }, set: { FT.send("find_set", treeId, ["maskOn": $0]) }))
                .toggleStyle(.switch).controlSize(.small)
            TextField("*.rs", text: $mask).textFieldStyle(.roundedBorder).frame(width: 110).disabled(!(find?.maskOn ?? false))
                .onChange(of: mask) { _, text in FT.send("find_set", treeId, ["mask": text]) }
        }
        .padding(.horizontal, 14).frame(height: 46)
    }

    private func searchRow(_ find: FTFind?) -> some View {
        HStack(spacing: 8) {
            Image(systemName: "magnifyingglass").foregroundStyle(.secondary)
            TextField("Texto a localizar", text: $query)
                .textFieldStyle(.plain).font(.app(14)).focused($focused)
                .onChange(of: query) { _, text in FT.send("find_set", treeId, ["query": text]) }
                .onSubmit { if let find, !find.hits.isEmpty { FT.send("find_pick", treeId, ["index": find.selected, "open": true]) } }
            chip("Cc", "Diferenciar maiúsculas de minúsculas", find?.case ?? false, "case")
            chip("W", "Palavras inteiras", find?.words ?? false, "words")
            chip(".*", "Expressão regular", find?.regex ?? false, "regex")
            chip("↵", "Várias linhas", find?.multiline ?? false, "multiline")
        }
        .padding(.horizontal, 14).frame(height: 46)
    }

    private func scopes(_ find: FTFind?) -> some View {
        HStack(spacing: 8) {
            Toggle("Diretório", isOn: Binding(get: { find?.inDir ?? false }, set: { FT.send("find_set", treeId, ["inDir": $0]) })).toggleStyle(.checkbox)
            TextField("Pasta (a partir da raiz)", text: $folder).textFieldStyle(.roundedBorder).disabled(!(find?.inDir ?? false))
                .onSubmit { FT.send("find_set", treeId, ["folder": folder]) }
            if find?.inDir != true { Text("A busca cobre a raiz inteira.").font(.app(11)).foregroundStyle(.secondary) }
        }
        .controlSize(.small).padding(.horizontal, 14).frame(height: 40)
    }

    private func chip(_ label: String, _ tip: String, _ on: Bool, _ key: String) -> some View {
        Button { FT.send("find_set", treeId, [key: !on]) } label: {
            Text(label).font(.custom("Hack", size: 12))
                .frame(width: 28, height: 22)
                .background(RoundedRectangle(cornerRadius: 5).fill(on ? Color.accentColor.opacity(0.3) : Color.clear))
                .overlay(RoundedRectangle(cornerRadius: 5).stroke(Color.primary.opacity(on ? 0.35 : 0.12)))
        }
        .buttonStyle(.plain).help(tip)
    }
}
