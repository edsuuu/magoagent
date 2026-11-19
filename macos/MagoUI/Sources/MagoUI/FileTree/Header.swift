import AppKit
import SwiftUI

/// The header, one row as the egui tree's (`head`, `h-10`): back and forward, the folder's name, the view options and the branch.
/// Its bare part has no fill and no gesture: the press goes to the canvas under it, which picks the node and drags it.
struct FTHeader: View {
    let state: FTState

    private var pal: FTPalette { FTPalette(dark: state.dark) }

    var body: some View {
        HStack(spacing: 4.875) {
            nav
            FTMedium(text: state.name, color: Color(pal.fg)).fixedSize().allowsHitTesting(false)
            Spacer(minLength: 0).allowsHitTesting(false)
            viewMenu
            if state.git.repo { branch }
        }
        .padding(.horizontal, 6.5)
        .frame(height: 32.5)
    }

    /// Back and forward: two round slots with a hair of divider between.
    private var nav: some View {
        HStack(spacing: 1) {
            navButton("chevron-left", on: state.canBack, tip: "Voltar") { FT.send("back", state.id) }
            navButton("chevron-right", on: state.canForward, tip: "Avançar") { FT.send("forward", state.id) }
        }
        .overlay(Rectangle().fill(Color(pal.border)).frame(width: 1, height: 11.375).allowsHitTesting(false))
    }

    private func navButton(_ glyph: String, on: Bool, tip: String, _ action: @escaping () -> Void) -> some View {
        FTHoverButton(pal: pal, radius: 9.75, enabled: on) {
            FT.send("focus", state.id)
            action()
        } label: {
            FTGlyph(name: glyph, size: 13, color: on ? pal.fg : pal.fg.withAlphaComponent(0.3)).frame(width: 19.5, height: 19.5)
        }
        .help(tip)
    }

    private var viewMenu: some View {
        Menu {
            Button(state.editorOpen ? "Ocultar Editor" : "Mostrar Editor") { FT.send("editor", state.id) }
            Divider()
            Button(state.hideHidden ? "Mostrar Arquivos Ocultos" : "Ocultar Arquivos Ocultos") { FT.send("hidden", state.id) }
            Divider()
            Menu("Ordenar por") {
                ForEach(state.sorts, id: \.key) { sort in
                    Toggle(sort.label, isOn: Binding(get: { state.sort == sort.key }, set: { _ in FT.send("sort", state.id, ["by": sort.key]) }))
                }
                Divider()
                Toggle("Inverter Ordem", isOn: Binding(get: { state.reversed }, set: { _ in FT.send("reverse", state.id) }))
            }
            Divider()
            Button("Recolher Tudo") { FT.send("collapse_all", state.id) }.disabled(!state.rows.contains { $0.dir && $0.open })
        } label: {
            // lucide's list (size-4) and chevrons-up-down (size-2.5)
            HStack(spacing: 0) {
                FTGlyph(name: "list", size: 13, color: pal.fg)
                Spacer(minLength: 0)
                FTGlyph(name: "chevrons-up-down", size: 8.125, color: pal.muted)
            }
            .padding(.horizontal, 4.875)
            .frame(width: 32.5, height: 19.5)
            .contentShape(Rectangle())
        }
        .menuStyle(.button).buttonStyle(.plain).menuIndicator(.hidden).fixedSize()
        .help("Opções de Exibição")
    }

    private var branch: some View {
        Menu {
            ForEach(state.branches, id: \.self) { name in
                Button { Task { await FT.act("checkout", state.id, ["branch": name]) } } label: {
                    if name == state.git.branch { Label(name, systemImage: "checkmark") } else { Text(name) }
                }
            }
        } label: {
            HStack(spacing: 3.25) {
                FTGlyph(name: "git-branch", size: 9.75, color: pal.muted)
                Text(state.git.branch.isEmpty ? "HEAD" : state.git.branch)
                    .font(.app(11)).foregroundStyle(Color(pal.fg).opacity(0.8)).lineLimit(1).frame(maxWidth: 78, alignment: .leading)
                if state.git.changes > 0 { Text(verbatim: String(state.git.changes)).font(.app(11)).foregroundStyle(Color(pal.muted)) }
            }
            .padding(.leading, 7.5).padding(.trailing, 6.5)
            .frame(height: 19.5)
            .overlay(Capsule().strokeBorder(Color(pal.edge), lineWidth: 1))
            .contentShape(Capsule())
        }
        .menuStyle(.button).buttonStyle(.plain).menuIndicator(.hidden).fixedSize()
        .help("Branch \(state.git.branch), \(state.git.changes) arquivo\(state.git.changes == 1 ? "" : "s") alterado\(state.git.changes == 1 ? "" : "s"); clique para trocar")
        .onAppear { FT.send("branches", state.id) }
        .onChange(of: state.git.branch) { FT.send("branches", state.id) }
    }
}

/// A flat button with the egui tree's hover fill (`c.hover`) and a pointing hand.
struct FTHoverButton<Label: View>: View {
    let pal: FTPalette
    var radius: CGFloat = 6
    var enabled = true
    let action: () -> Void
    @ViewBuilder let label: () -> Label
    @State private var hover = false

    var body: some View {
        Button(action: { if enabled { action() } }) {
            label()
                .background(RoundedRectangle(cornerRadius: radius).fill(hover && enabled ? Color(pal.hover) : Color.clear))
                .contentShape(Rectangle())
        }
        .buttonStyle(.plain)
        .onContinuousHover { phase in
            switch phase {
            case .active: if enabled { NSCursor.pointingHand.set() }; hover = true
            case .ended: hover = false
            }
        }
    }
}

/// The search bar under the rows: the field in a round pill, and at its end the editor and Alterações buttons (`bar.rs`).
struct FTSearchBar: View {
    let state: FTState
    @State private var query = ""

    private var pal: FTPalette { FTPalette(dark: state.dark) }

    var body: some View {
        HStack(spacing: 0) {
            HStack(spacing: 4.875) {
                FTGlyph(name: "search", size: 9.75, color: pal.muted)
                TextField("", text: $query, prompt: Text("Buscar").font(.app(12)).foregroundColor(Color(pal.muted)))
                    .textFieldStyle(.plain).font(.app(12)).foregroundStyle(Color(pal.fg))
                if !query.isEmpty {
                    Button { query = "" } label: { FTGlyph(name: "x", size: 13, color: pal.muted) }
                        .buttonStyle(.plain).help("Limpar filtro")
                }
            }
            .padding(.horizontal, 6.5).frame(height: 19.5)
            .background(Capsule().fill(Color(pal.hover)))
            .padding(.leading, 6.5)
            if state.searching {
                Text("\(state.treeRows) exibidos").font(.app(11)).foregroundStyle(Color(pal.muted)).padding(.leading, 6.5).fixedSize()
            }
            // from the bar's right end the editor comes first, so the changes sit to its left (`bar.rs`)
            toggle("git-compare", on: state.changesOpen, tip: state.changesOpen ? "Ocultar Alterações" : "Mostrar Alterações") {
                FT.send("changes", state.id)
            }
            .padding(.leading, 4.875)
            toggle("panel-right", on: state.editorOpen, tip: state.editorOpen ? "Ocultar Editor" : "Mostrar Editor") { FT.send("editor", state.id) }
                .padding(.leading, 4.875).padding(.trailing, 6.5)
        }
        .frame(height: 32.5)
        .onAppear { query = state.filter }
        .onChange(of: query) { _, text in if text != state.filter { FT.send("filter", state.id, ["text": text]) } }
        .onChange(of: state.filter) { _, text in if text != query { query = text } }
    }

    private func toggle(_ glyph: String, on: Bool, tip: String, _ action: @escaping () -> Void) -> some View {
        FTHoverButton(pal: pal) {
            FT.send("focus", state.id)
            action()
        } label: {
            FTGlyph(name: glyph, size: 11.375, color: on ? pal.accent : pal.muted).frame(width: 22.75, height: 19.5)
        }
        .help(tip)
    }
}
