import SwiftUI

/// The window, as the egui sheet (`src/app/ui/settings/mod.rs`): 880 wide and at most 720 high, radius 14, the pages in
/// a 200 wide sidebar at the left and the open page's groups in cards at the right.
struct SettingsView: View {
    @Environment(\.colorScheme) private var scheme

    var body: some View {
        let k = SK.of(scheme)
        Group {
            if let snapshot = MagoStore.shared.decode(SettingsSnapshot.self, topic: "settings") {
                HStack(spacing: 0) {
                    SettingsSidebar(snapshot: snapshot)
                    SettingsPage(snapshot: snapshot)
                }
                .sheet(isPresented: Binding(get: { snapshot.hooksPreview != nil }, set: { if !$0 { settingsSend("preview_no") } })) {
                    if let preview = snapshot.hooksPreview { HooksPreviewSheet(preview: preview) }
                }
                .sheet(isPresented: Binding(get: { snapshot.wipe != nil }, set: { if !$0 { settingsSend("wipe_no") } })) {
                    if let lines = snapshot.wipe { WipeSheet(lines: lines) }
                }
            } else {
                ProgressView().frame(maxWidth: .infinity, maxHeight: .infinity)
            }
        }
        .frame(maxWidth: 880, maxHeight: 720)
        .background(k.bg)
        .overlay { RoundedRectangle(cornerRadius: 14).strokeBorder(k.hairline) }
        .onExitCommand { settingsSend("close") }
    }
}

/// The sidebar (#111215, a line at its right): the brand, then a row for each page, icon and name, the chosen one lit.
private struct SettingsSidebar: View {
    let snapshot: SettingsSnapshot
    @Environment(\.colorScheme) private var scheme

    var body: some View {
        let k = SK.of(scheme)
        VStack(alignment: .leading, spacing: 2) {
            // the brand: a 26 square in the app's grey and the name. egui's semibold here is SFNS.ttf, a variable font it
            // draws at its default instance: the regular weight (so are the page's name and the groups' titles)
            HStack(spacing: 10) {
                SettingsIcon(name: "gear", color: k.fg, scale: 14 / 16)
                    .frame(width: 26, height: 26)
                    .background(k.segment, in: .rect(cornerRadius: 7))
                    .overlay { RoundedRectangle(cornerRadius: 7).strokeBorder(k.hairline) }
                Text("Ajustes").font(.system(size: 14)).foregroundStyle(k.fg)
            }
            .padding(.leading, 10)
            .padding(.top, 24)
            .padding(.bottom, 16)
            ForEach(snapshot.tabs, id: \.self) { name in
                SidebarItem(name: name, chosen: name == snapshot.title)
            }
            Spacer(minLength: 0)
        }
        .padding(.horizontal, 12)
        .frame(width: 200)
        .frame(maxHeight: .infinity)
        .background(k.well)
        .overlay(alignment: .trailing) { Rectangle().fill(k.hairline).frame(width: 1) }
    }
}

private struct SidebarItem: View {
    let name: String
    let chosen: Bool
    @Environment(\.colorScheme) private var scheme
    @State private var hover = false

    var body: some View {
        let k = SK.of(scheme)
        Button(action: open) {
            HStack(spacing: 10) {
                SettingsIcon(name: tabIcon(name), color: chosen ? k.fg : k.softFg)
                Text(name).font(.kit(SK.body)).foregroundStyle(chosen ? k.fg : k.softFg)
            }
            .padding(.horizontal, 10)
            .frame(maxWidth: .infinity, minHeight: 34, maxHeight: 34, alignment: .leading)
            .background(chosen ? k.chosen : hover ? k.chosen.opacity(0.5) : .clear, in: .rect(cornerRadius: 8))
            .contentShape(Rectangle())
        }
        .buttonStyle(.plain)
        .onHover { hover = $0 }
        .handCursor()
        .accessibilityLabel("Ajustes: \(name)")
        .accessibilityAddTraits(chosen ? .isSelected : [])
    }

    private func open() {
        if !chosen { settingsSend("tab", ["name": name]) }
    }
}

/// The page: its name and what it holds, then its groups, the cards 32 in from the sides (the rows 16 more).
private struct SettingsPage: View {
    let snapshot: SettingsSnapshot
    @Environment(\.colorScheme) private var scheme

    var body: some View {
        let k = SK.of(scheme)
        ScrollViewReader { proxy in
            ScrollView {
                VStack(alignment: .leading, spacing: 0) {
                    VStack(alignment: .leading, spacing: 4) {
                        Text(snapshot.title).font(.system(size: 20)).foregroundStyle(k.fg)
                        Text(snapshot.subtitle).kitLine(12.5, 17).foregroundStyle(k.mutedFg).fixedSize(horizontal: false, vertical: true)
                            .padding(.trailing, 40)
                    }
                    if snapshot.title == "Atalhos" { HotkeySearch() }
                    VStack(alignment: .leading, spacing: 18) {
                        ForEach(Array(snapshot.groups.enumerated()), id: \.offset) { _, group in
                            GroupCard(group: group).id(group.scrollId)
                        }
                        if snapshot.title == "Atalhos" && !HotkeyQuery.shared.matchesAny(snapshot.groups) {
                            NoteCard(words: "Nenhum atalho com esse nome ou essa tecla.")
                        }
                    }
                    .padding(.top, 18)
                }
                .padding(.top, 18)
                .padding(.bottom, 18)
                .padding(.leading, 32)
                .padding(.trailing, 32)
            }
            .padding(.vertical, 10)
            .padding(.trailing, 2)
            .id(snapshot.title)
            .onChange(of: snapshot.scrollTo, initial: true) { _, card in
                if let card { withAnimation { proxy.scrollTo(card, anchor: .top) } }
            }
        }
    }
}

/// The search over the shortcuts: kept here, read by every hotkey row.
@MainActor @Observable final class HotkeyQuery {
    static let shared = HotkeyQuery()
    var text = ""

    var words: String { text.trimmingCharacters(in: .whitespaces).lowercased() }

    /// Whether a shortcut answers to the search: its name or its keys as shown, any case.
    func matches(_ item: SettingsItem) -> Bool {
        words.isEmpty || item.label.lowercased().contains(words) || (item.keys?.text.lowercased().contains(words) ?? false)
    }

    func matchesAny(_ groups: [SettingsGroup]) -> Bool {
        groups.contains { group in group.items.contains { $0.kind == "hotkey" && matches($0) } }
    }
}

/// The search field, the column's width, over the groups.
private struct HotkeySearch: View {
    @State private var query = HotkeyQuery.shared
    @FocusState private var focused: Bool
    @Environment(\.colorScheme) private var scheme

    var body: some View {
        let k = SK.of(scheme)
        GeometryReader { geometry in
            TextField("Buscar atalho", text: $query.text, prompt: Text("Buscar pelo nome ou pela tecla").foregroundStyle(k.mutedFg))
                .focused($focused)
                .kitWell(k, width: geometry.size.width, focused: focused)
        }
        .frame(height: 30)
        .padding(.horizontal, 16)
        .padding(.top, 18)
        .padding(.bottom, 1.5)
    }
}

/// A group: its title (11.5, weight 600, 4 in from the card's edge) and its rows in a card (#1e1f23, radius 12), a line
/// between rows.
private struct GroupCard: View {
    let group: SettingsGroup
    @Environment(\.colorScheme) private var scheme

    /// What the card shows: a search over the shortcuts narrows them (their notes stay while one answers).
    private var items: [SettingsItem] {
        let query = HotkeyQuery.shared
        guard !query.words.isEmpty, group.items.contains(where: { $0.kind == "hotkey" }) else { return group.items }
        guard group.items.contains(where: { $0.kind == "hotkey" && query.matches($0) }) else { return [] }
        return group.items.filter { $0.kind != "hotkey" || query.matches($0) }
    }

    var body: some View {
        let k = SK.of(scheme)
        let shown = items
        if !shown.isEmpty {
            VStack(alignment: .leading, spacing: 6) {
                if !group.title.isEmpty {
                    Text(group.title).font(.system(size: 11.5)).foregroundStyle(k.mutedFg).frame(height: 15).padding(.leading, 4)
                }
                CardRows(items: shown)
            }
        }
    }
}

/// A paragraph alone in a card (the search that found nothing).
private struct NoteCard: View {
    let words: String
    @Environment(\.colorScheme) private var scheme

    var body: some View {
        let k = SK.of(scheme)
        Text(words).font(.kit(SK.small)).foregroundStyle(k.mutedFg)
            .frame(maxWidth: .infinity, alignment: .leading)
            .padding(.vertical, 12)
            .padding(.horizontal, 16)
            .background(k.card, in: .rect(cornerRadius: 12))
            .overlay { RoundedRectangle(cornerRadius: 12).strokeBorder(k.hairline) }
    }
}

/// The rows of a card as the egui kit lays them: a line over a row when a row came before it, 3.25 between things, and
/// the card's 10 under what is not a row. The presets (`terminal.rs`) touch each other, a line over the top of each, and
/// the list ends with a line and then the 3.25.
private struct CardRows: View {
    let items: [SettingsItem]
    @Environment(\.colorScheme) private var scheme

    private static let rows: Set<String> = ["toggle", "choice", "select", "stepper", "text", "button", "info", "note", "hotkey"]
    private static let blocks: Set<String> = ["block", "ends", "notes", "head"]

    typealias Gap = (line: Bool, above: CGFloat, below: CGFloat)

    /// Before each item: the room above and below a line, or (no line) just the room above.
    private var gaps: [Gap] {
        var gaps: [Gap] = []
        var rowsBefore = 0
        for (index, item) in items.enumerated() {
            let after = index == 0 ? "" : items[index - 1].kind
            let gap: Gap
            if index == 0 {
                gap = (false, 0, 0)
            } else if item.kind == "preset" {
                gap = (false, after == "preset" ? 0 : 3.25, 0)
            } else if after == "preset" {
                gap = (true, 0, 3.25)
            } else if Self.rows.contains(item.kind) && rowsBefore > 0 {
                gap = (true, 1.125, 1.125)
            } else {
                gap = (false, 3.25, 0)
            }
            gaps.append(gap)
            if Self.rows.contains(item.kind) { rowsBefore += 1 }
        }
        return gaps
    }

    /// The card's padding under its last thing when it was not a row.
    private var bottom: CGFloat {
        var lastWasRow = false
        for item in items {
            if Self.rows.contains(item.kind) { lastWasRow = true } else if Self.blocks.contains(item.kind) { lastWasRow = false }
        }
        return lastWasRow ? 0 : 10
    }

    var body: some View {
        let k = SK.of(scheme)
        let spaces = gaps
        VStack(alignment: .leading, spacing: 0) {
            ForEach(Array(items.enumerated()), id: \.offset) { index, item in
                let gap = spaces[index]
                if gap.line {
                    Rectangle().fill(k.hairline).frame(height: 1).padding(.top, gap.above).padding(.bottom, gap.below)
                } else if gap.above > 0 {
                    Color.clear.frame(height: gap.above)
                }
                ItemRow(item: item, first: index == 0).padding(.horizontal, 16)
                    .overlay(alignment: .top) { if item.kind == "preset" { Rectangle().fill(k.hairline).frame(height: 1) } }
            }
            Color.clear.frame(height: bottom)
        }
        .background(k.card, in: .rect(cornerRadius: 12))
        .overlay { RoundedRectangle(cornerRadius: 12).strokeBorder(k.hairline) }
    }
}
