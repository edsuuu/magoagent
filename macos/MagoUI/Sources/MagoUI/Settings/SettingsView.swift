import SwiftUI

private let tabIcons = [
    "gearshape", "terminal", "person.3", "bubble.left", "waveform", "wand.and.stars",
    "link", "command", "folder", "info.circle",
]

/// The window: the pages in a sidebar at the left, the open page's groups at the right as a grouped Form.
struct SettingsView: View {
    var body: some View {
        if let snapshot = MagoStore.shared.decode(SettingsSnapshot.self, topic: "settings") {
            HStack(spacing: 0) {
                SettingsSidebar(snapshot: snapshot)
                    .frame(width: 200)
                Divider()
                SettingsPage(snapshot: snapshot)
            }
            .frame(minWidth: 820, minHeight: 560)
            .onExitCommand { settingsSend("close") }
            .sheet(isPresented: Binding(get: { snapshot.hooksPreview != nil }, set: { if !$0 { settingsSend("preview_no") } })) {
                if let preview = snapshot.hooksPreview { HooksPreviewSheet(preview: preview) }
            }
            .sheet(isPresented: Binding(get: { snapshot.wipe != nil }, set: { if !$0 { settingsSend("wipe_no") } })) {
                if let lines = snapshot.wipe { WipeSheet(lines: lines) }
            }
        } else {
            ProgressView().frame(minWidth: 820, minHeight: 560)
        }
    }
}

private struct SettingsSidebar: View {
    let snapshot: SettingsSnapshot

    var body: some View {
        VStack(alignment: .leading, spacing: 2) {
            Label("Ajustes", systemImage: "gearshape")
                .font(.headline)
                .padding(.horizontal, 10)
                .padding(.top, 20)
                .padding(.bottom, 14)
            ForEach(Array(snapshot.tabs.enumerated()), id: \.offset) { index, name in
                Button {
                    settingsSend("tab", ["index": index])
                } label: {
                    Label(name, systemImage: tabIcons[min(index, tabIcons.count - 1)])
                        .frame(maxWidth: .infinity, alignment: .leading)
                        .padding(.vertical, 6)
                        .padding(.horizontal, 10)
                        .background(index == snapshot.tab ? Color.primary.opacity(0.1) : .clear, in: RoundedRectangle(cornerRadius: 8))
                        .contentShape(Rectangle())
                }
                .buttonStyle(.plain)
                .foregroundStyle(index == snapshot.tab ? .primary : .secondary)
                .accessibilityLabel("Ajustes: \(name)")
            }
            Spacer()
        }
        .padding(.horizontal, 12)
        .background(.regularMaterial)
    }
}

private struct SettingsPage: View {
    let snapshot: SettingsSnapshot

    var body: some View {
        ScrollView {
            VStack(alignment: .leading, spacing: 4) {
                Text(snapshot.title).font(.title2.weight(.semibold))
                Text(snapshot.subtitle).font(.callout).foregroundStyle(.secondary)
                    .fixedSize(horizontal: false, vertical: true)
                if snapshot.tab == 7 { HotkeySearch() }
                VStack(spacing: 22) {
                    ForEach(Array(snapshot.groups.enumerated()), id: \.offset) { _, group in
                        GroupCard(group: group)
                    }
                }
                .padding(.top, 14)
            }
            .padding(28)
            .frame(maxWidth: 680, alignment: .leading)
            .frame(maxWidth: .infinity)
        }
        .id(snapshot.tab)
    }
}

/// The search over the shortcuts: kept here, read by every hotkey row.
@MainActor @Observable final class HotkeyQuery {
    static let shared = HotkeyQuery()
    var text = ""
}

private struct HotkeySearch: View {
    @State private var query = HotkeyQuery.shared

    var body: some View {
        TextField("Buscar pelo nome ou pela tecla", text: $query.text)
            .textFieldStyle(.roundedBorder)
            .accessibilityLabel("Buscar atalho")
            .padding(.top, 10)
    }
}

/// A group: its small caps title, then its rows in one rounded card with a line between them.
private struct GroupCard: View {
    let group: SettingsGroup

    private var items: [(Int, SettingsItem)] {
        let query = HotkeyQuery.shared.text.trimmingCharacters(in: .whitespaces).lowercased()
        let all = Array(group.items.enumerated()).map { ($0.offset, $0.element) }
        guard !query.isEmpty else { return all }
        // a search narrows the shortcuts: their name or their keys; the notes go
        return all.filter { $0.1.kind == "hotkey" && ($0.1.label.lowercased().contains(query) || ($0.1.keys?.text.lowercased().contains(query) ?? false)) }
    }

    var body: some View {
        let shown = items
        let searching = !HotkeyQuery.shared.text.trimmingCharacters(in: .whitespaces).isEmpty
        if !(searching && shown.isEmpty) {
            VStack(alignment: .leading, spacing: 6) {
                Text(group.title).font(.caption.weight(.semibold)).foregroundStyle(.secondary)
                    .padding(.leading, 4)
                VStack(spacing: 0) {
                    ForEach(Array(shown.enumerated()), id: \.offset) { position, entry in
                        if position > 0 { Divider() }
                        ItemRow(item: entry.1)
                            .padding(.horizontal, 14)
                            .padding(.vertical, 9)
                    }
                }
                .background(Color(nsColor: .controlBackgroundColor), in: RoundedRectangle(cornerRadius: 10))
                .overlay(RoundedRectangle(cornerRadius: 10).stroke(Color.primary.opacity(0.08)))
            }
        }
    }
}
