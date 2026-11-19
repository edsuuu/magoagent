// The list of the Chats screen as Conversas draws it (chat/list/view.rs): + (Nova conversa) and « (Recolher lista) on the
// window buttons' row, the title, the chips with their counts, the search, everything running grouped by space (its dot
// and name), each row with its mini wizard, name and state, how long ago and its last line, the answers it takes right
// there, and a Maestro's agents folded under it (list/team.rs). A chat's ⋯ renames, shows it on the canvas or removes it,
// after asking.
import SwiftUI

struct ChatList: View {
    let list: ChatsSnapshot?
    @Binding var folded: Bool
    private let store = MagoStore.shared
    @Environment(\.chatLook) private var look
    @State private var query = ""
    @State private var renaming: ChatEntry?
    @State private var newName = ""
    @State private var removing: ChatEntry?

    private static let filters: [(key: String, label: String)] = [("all", "Todos"), ("working", "Trabalhando"), ("waiting", "Esperando"), ("ready", "Prontos")]

    var body: some View {
        VStack(alignment: .leading, spacing: 0) {
            // the window buttons' row: + and « at its right
            HStack(spacing: 4) {
                Spacer()
                Button { store.send("ui.chat.new") } label: { glyph("plus") }
                    .buttonStyle(HoverGround(radius: 7, color: look.hover))
                    .help("Nova conversa")
                Button { withAnimation(.smooth(duration: 0.25)) { folded = true } } label: { glyph("chevron.left.2") }
                    .buttonStyle(HoverGround(radius: 7, color: look.hover))
                    .help("Recolher lista (⌘B)")
            }
            .frame(height: 40)
            Thick(text: "Conversas", size: 14, color: look.strong).padding(.leading, 8).padding(.top, 2)
            Flow(spacing: 6, lineSpacing: 6) {
                ForEach(Array(Self.filters.enumerated()), id: \.offset) { index, filter in
                    chip(filter.label, count: list?.counts[safe: index] ?? 0, on: (list?.show ?? "all") == filter.key) {
                        store.send("chats_view", ["filter": filter.key])
                    }
                }
            }
            .padding(.top, 9)
            .padding(.horizontal, 2)
            HStack(spacing: 8) {
                Image(systemName: "magnifyingglass").font(.system(size: 13)).foregroundStyle(look.muted)
                TextField("", text: $query, prompt: Text("Buscar nos chats").foregroundStyle(look.muted))
                    .textFieldStyle(.plain)
                    .font(.app(14))
                    .foregroundStyle(look.text)
                    .onChange(of: query) { _, value in store.send("chats_view", ["query": value]) }
            }
            .padding(.horizontal, 9)
            .frame(height: 30)
            .background(RoundedRectangle(cornerRadius: 8).fill(look.hover))
            .padding(.horizontal, 2)
            .padding(.top, 8)
            ScrollView {
                LazyVStack(alignment: .leading, spacing: 2) {
                    if list?.groups.isEmpty ?? true {
                        Text((list?.counts.first ?? 0) == 0 ? "Nada rodando agora." : "Nada com esse filtro.")
                            .font(.app(13))
                            .foregroundStyle(look.muted)
                            .padding(.top, 12)
                    }
                    ForEach(list?.groups ?? []) { group in
                        groupHead(group)
                        ForEach(group.entries) { entry in
                            row(entry, indent: 0)
                            if !entry.team.isEmpty {
                                let open = (list?.looking ?? false) || entry.teamOpen
                                if !(list?.looking ?? false) { teamFold(entry, open: open) }
                                if open {
                                    ForEach(entry.team) { member in row(member, indent: 18) }
                                }
                            }
                        }
                    }
                }
                .padding(.top, 4)
                .animation(.smooth(duration: 0.2), value: list?.groups)
            }
            .scrollIndicators(.never)
        }
        .padding(.horizontal, 8)
        .padding(.bottom, 10)
        .background(RoundedRectangle(cornerRadius: 12).fill(look.panel))
        .onAppear { query = list?.filter ?? "" }
        .alert("Renomear chat", isPresented: Binding(get: { renaming != nil }, set: { if !$0 { renaming = nil } })) {
            TextField("Nome do chat", text: $newName)
            Button("Salvar") {
                if let entry = renaming { store.send("chat_rename", ["chat": entry.id, "name": newName]) }
                renaming = nil
            }
            Button("Cancelar", role: .cancel) { renaming = nil }
        }
        // Remover chat asks first (list/view.rs `remove_chat_dialog`): the conversation goes for good
        .alert("Remover chat", isPresented: Binding(get: { removing != nil }, set: { if !$0 { removing = nil } }), presenting: removing) { entry in
            Button("Remover", role: .destructive) { store.send("ui.chat.remove", ["chat": entry.id]) }
            Button("Cancelar", role: .cancel) {}
        } message: { entry in
            Text("\"\(entry.name)\" e toda a sua conversa serão excluídos permanentemente." + (entry.state == "working" ? " O agente que está rodando nele será parado." : ""))
        }
    }

    private func glyph(_ name: String) -> some View {
        Image(systemName: name).font(.system(size: 14, weight: .medium)).foregroundStyle(look.soft).frame(width: 28, height: 28).contentShape(Rectangle())
    }

    /// A chip: its word and count, the accent's ground when on.
    private func chip(_ label: String, count: Int, on: Bool, action: @escaping () -> Void) -> some View {
        Button(action: action) {
            HStack(spacing: 6) {
                Text(label).foregroundStyle(on ? .white : look.text)
                Text("\(count)").foregroundStyle(on ? Color.white.opacity(180 / 255) : look.muted)
            }
            .font(.app(12))
            .padding(.horizontal, 10)
            .frame(height: 26)
            .background(Capsule().fill(on ? ChatLook.blue : .clear))
            .overlay(Capsule().strokeBorder(on ? ChatLook.blue : look.line))
            .contentShape(Capsule())
        }
        .buttonStyle(HoverGround(radius: 13, color: on ? .clear : look.hover))
    }

    /// A space's head: its dot and its name in capitals, spaced.
    private func groupHead(_ group: ChatGroup) -> some View {
        HStack(spacing: 5) {
            Circle().fill(Color(rgba: group.color)).frame(width: 8, height: 8)
            Text(group.name.uppercased()).font(.app(12)).tracking(0.7).foregroundStyle(look.muted)
        }
        .padding(.leading, 7)
        .padding(.bottom, 5)
        .frame(height: 28, alignment: .bottom)
    }

    /// The fold of a Maestro's agents: "3 agentes · 1 esperando", the chevron turned when open.
    private func teamFold(_ entry: ChatEntry, open: Bool) -> some View {
        Button { store.send("ui.chat.team", ["chat": entry.id, "open": !open]) } label: {
            HStack(spacing: 0) {
                Image(systemName: "chevron.right").font(.system(size: 10, weight: .bold)).foregroundStyle(look.muted).rotationEffect(.degrees(open ? 90 : 0))
                    .frame(width: 14)
                    .padding(.leading, 18 + 12)
                Text(entry.teamSummary ?? "").font(.app(12)).foregroundStyle(look.soft).lineLimit(1).padding(.leading, 7)
                Spacer(minLength: 0)
            }
            .frame(height: 26)
            .contentShape(Rectangle())
        }
        .buttonStyle(HoverGround(radius: 8, color: look.hover))
        .help(open ? "Recolher os agentes" : "Mostrar os agentes")
    }

    private func row(_ entry: ChatEntry, indent: CGFloat) -> some View {
        ListRow(entry: entry, indent: indent, on: list?.current == entry.id, rename: {
            newName = entry.name
            renaming = entry
        }, remove: { removing = entry })
    }
}

/// A line of the list (list/view.rs `list_row`).
private struct ListRow: View {
    let entry: ChatEntry
    let indent: CGFloat
    let on: Bool
    let rename: () -> Void
    let remove: () -> Void
    private let store = MagoStore.shared
    @Environment(\.chatLook) private var look
    @State private var hovered = false

    var body: some View {
        let color = ChatLook.chip(entry.state)
        // on the light look the words go darker, to read on their pale ground
        let tagInk = look.dark ? color : ChatLook.chipInkLight(entry.state)
        let word = ["working": "Trabalhando", "waiting": "Esperando"][entry.state] ?? "Pronto"
        VStack(alignment: .leading, spacing: 0) {
            HStack(spacing: 6) {
                Thick(text: entry.name, size: 14, color: entry.ended ? look.soft : look.strong).lineLimit(1)
                Text(word).font(.app(12)).foregroundStyle(tagInk).padding(.horizontal, 7).frame(height: 20).background(Capsule().fill(color.opacity(0.16))).fixedSize()
                Spacer(minLength: 4)
                if entry.chat && hovered {
                    Menu { menu } label: {
                        Image(systemName: "ellipsis").font(.system(size: 12, weight: .bold)).foregroundStyle(look.soft).frame(width: 24, height: 24).contentShape(Rectangle())
                    }
                    .menuStyle(.button)
                    .buttonStyle(HoverGround(radius: 6, color: look.raised))
                    .menuIndicator(.hidden)
                    .fixedSize()
                } else if !entry.ago.isEmpty {
                    Text(entry.ago).font(.app(12)).foregroundStyle(look.muted).fixedSize()
                }
            }
            .frame(height: 24)
            .padding(.top, 5)
            Text(entry.last).font(.app(12)).foregroundStyle(look.soft).lineLimit(1).frame(height: 18).padding(.top, 2)
            if !entry.answers.isEmpty {
                Flow(spacing: 6, lineSpacing: 6) {
                    ForEach(Array(entry.answers.enumerated()), id: \.offset) { index, answer in
                        Button { store.send("ui.chat.answer", ["chat": entry.id, "index": index]) } label: {
                            Text(answer.label).font(.app(12)).foregroundStyle(answer.main ? Color(rgb: 0x1d1d1f) : look.text).lineLimit(1)
                                .padding(.horizontal, 10)
                                .frame(height: 24)
                                .background(RoundedRectangle(cornerRadius: 6).fill(answer.main ? ChatLook.amber : look.raised))
                        }
                        .buttonStyle(.plain)
                    }
                }
                .padding(.top, 5)
                .padding(.bottom, 8)
            }
        }
        .padding(.leading, indent + 40)
        .padding(.trailing, 10)
        .frame(minHeight: 54, alignment: .top)
        .overlay(alignment: .topLeading) { mini.padding(.leading, indent + 9).padding(.top, 15) }
        .background(RoundedRectangle(cornerRadius: 10).fill(on ? look.surface : hovered ? look.hover : .clear))
        .contentShape(Rectangle())
        .onHover { hovered = $0 }
        .onTapGesture { store.send("ui.chat.select", ["chat": entry.id]) }
        .contextMenu { menu }
    }

    /// The agent's mini wizard in its colour: here its hat, breathing while it works.
    private var mini: some View {
        Image(systemName: "wand.and.stars")
            .font(.system(size: 14, weight: .semibold))
            .foregroundStyle(Color(rgba: entry.color))
            .frame(width: 20, height: 20)
            .phaseAnimator([1.0, 0.5]) { hat, phase in hat.opacity(entry.state == "working" ? phase : 1) } animation: { _ in .easeInOut(duration: 0.8) }
    }

    @ViewBuilder private var menu: some View {
        if entry.chat { Button("Renomear", systemImage: "pencil", action: rename) }
        Button("Mostrar no canvas", systemImage: "map") { store.send("ui.chat.on_canvas", ["chat": entry.id]) }
        if entry.chat { Button("Remover chat", systemImage: "trash", role: .destructive, action: remove) }
    }
}

/// Lines of views that wrap when the room runs out, as egui's `horizontal_wrapped` (the chips, a row's answers).
struct Flow: Layout {
    var spacing: CGFloat
    var lineSpacing: CGFloat

    func sizeThatFits(proposal: ProposedViewSize, subviews: Subviews, cache: inout ()) -> CGSize {
        let places = arrange(subviews, width: proposal.width ?? .infinity)
        return CGSize(width: places.map(\.maxX).max() ?? 0, height: places.map(\.maxY).max() ?? 0)
    }

    func placeSubviews(in bounds: CGRect, proposal: ProposedViewSize, subviews: Subviews, cache: inout ()) {
        for (place, view) in zip(arrange(subviews, width: bounds.width), subviews) {
            view.place(at: CGPoint(x: bounds.minX + place.minX, y: bounds.minY + place.minY), proposal: ProposedViewSize(place.size))
        }
    }

    private func arrange(_ subviews: Subviews, width: CGFloat) -> [CGRect] {
        var (x, y, line) = (CGFloat(0), CGFloat(0), CGFloat(0))
        return subviews.map { view in
            let size = view.sizeThatFits(.unspecified)
            if x > 0 && x + size.width > width {
                (x, y, line) = (0, y + line + lineSpacing, 0)
            }
            defer { (x, line) = (x + size.width + spacing, max(line, size.height)) }
            return CGRect(origin: CGPoint(x: x, y: y), size: size)
        }
    }
}
