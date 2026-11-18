// The list of the Chats screen (chat/list/): the filter chips, the search, everything running grouped by space, and
// Nova conversa. A row opens its conversation; a chat's row renames or removes it.
import SwiftUI

struct ChatList: View {
    let list: ChatsSnapshot?
    private let store = MagoStore.shared
    @State private var query = ""
    @State private var renaming: ChatEntry?
    @State private var newName = ""

    private static let filters: [(key: String, label: String)] = [("all", "Todos"), ("working", "Trabalhando"), ("waiting", "Esperando"), ("ready", "Prontos")]

    var body: some View {
        VStack(alignment: .leading, spacing: 8) {
            HStack {
                Text("Chats").font(.headline)
                if let list {
                    Text("\(list.running) rodando · \(list.waiting) esperando").font(.caption).foregroundStyle(.secondary)
                }
                Spacer()
                Button {
                    store.send("ui.chat.new")
                } label: {
                    Image(systemName: "square.and.pencil")
                }
                .buttonStyle(.borderless)
                .help("Nova conversa")
            }
            TextField("Buscar", text: $query)
                .textFieldStyle(.roundedBorder)
                .onSubmit { store.send("chats_view", ["query": query]) }
                .onChange(of: query) { _, value in store.send("chats_view", ["query": value]) }
            ScrollView(.horizontal, showsIndicators: false) {
                HStack(spacing: 6) {
                    ForEach(Array(Self.filters.enumerated()), id: \.offset) { index, filter in
                        let on = list?.show == filter.key
                        Button {
                            store.send("chats_view", ["filter": filter.key])
                        } label: {
                            Text("\(filter.label) \(list?.counts[safe: index] ?? 0)")
                                .font(.caption)
                                .padding(.horizontal, 10)
                                .padding(.vertical, 4)
                                .background(Capsule().fill(on ? Color.accentColor.opacity(0.2) : Color.secondary.opacity(0.1)))
                        }
                        .buttonStyle(.plain)
                    }
                }
            }
            List {
                ForEach(list?.groups ?? []) { group in
                    Section(group.name) {
                        ForEach(group.entries) { entry in
                            row(entry)
                                .contentShape(Rectangle())
                                .onTapGesture { store.send("ui.chat.select", ["chat": entry.id]) }
                                .listRowBackground(entry.id == list?.current ? Color.accentColor.opacity(0.15) : Color.clear)
                                .contextMenu {
                                    if entry.chat {
                                        Button("Renomear") {
                                            newName = entry.name
                                            renaming = entry
                                        }
                                        Button("Remover", role: .destructive) { store.send("ui.chat.remove", ["chat": entry.id]) }
                                    }
                                    Button("Ver no canvas") { store.send("ui.chat.on_canvas", ["chat": entry.id]) }
                                }
                        }
                    }
                }
                if list?.groups.isEmpty ?? true {
                    Text(query.isEmpty ? "Nada rodando." : "Nada com esse filtro.").foregroundStyle(.secondary)
                }
            }
            .listStyle(.sidebar)
            .animation(.default, value: list?.groups)
        }
        .padding(12)
        .onAppear { query = list?.filter ?? "" }
        .alert("Renomear", isPresented: Binding(get: { renaming != nil }, set: { if !$0 { renaming = nil } })) {
            TextField("Nome", text: $newName)
            Button("Renomear") {
                if let entry = renaming { store.send("chat_rename", ["chat": entry.id, "name": newName]) }
                renaming = nil
            }
            Button("Cancelar", role: .cancel) { renaming = nil }
        }
    }

    private func row(_ entry: ChatEntry) -> some View {
        HStack(alignment: .top, spacing: 8) {
            Circle()
                .fill(Color(red: Double(entry.color[safe: 0] ?? 128) / 255, green: Double(entry.color[safe: 1] ?? 128) / 255, blue: Double(entry.color[safe: 2] ?? 128) / 255))
                .frame(width: 10, height: 10)
                .padding(.top, 4)
            VStack(alignment: .leading, spacing: 2) {
                HStack {
                    Text(entry.name).lineLimit(1)
                    Spacer()
                    Text(entry.ago).font(.caption2).foregroundStyle(.secondary)
                }
                HStack(spacing: 4) {
                    Circle().fill(stateColor(entry.state)).frame(width: 6, height: 6)
                    Text(entry.last).font(.caption).foregroundStyle(.secondary).lineLimit(1)
                }
            }
        }
        .padding(.vertical, 2)
    }
}

fileprivate extension Array {
    subscript(safe index: Int) -> Element? { indices.contains(index) ? self[index] : nil }
}
