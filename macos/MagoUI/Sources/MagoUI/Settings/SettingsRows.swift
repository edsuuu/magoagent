import AppKit
import SwiftUI

private func toneColor(_ tone: String, _ k: SK) -> Color {
    switch tone {
    case "ok": SK.green
    case "bad": k.destructive
    case "warn": SK.amber
    default: k.mutedFg
    }
}

/// One row, drawn by its kind as the egui kit draws it (`kit.rs`).
struct ItemRow: View {
    let item: SettingsItem
    /// The first thing in its card: a block starts 10 from the card's edge.
    let first: Bool

    var body: some View {
        switch item.kind {
        case "toggle":
            RowShell(label: item.label, hint: item.hint, centered: true) {
                Toggle(item.label, isOn: Binding(get: { item.value.bool }, set: { setSetting(item.id, $0) })).toggleStyle(SwitchStyle())
            }
        case "choice":
            RowShell(label: item.label, hint: item.hint) {
                Segments(label: item.label, options: item.options, chosen: item.value.int) { setSetting(item.id, $0) }
            }
        case "select":
            RowShell(label: item.label, hint: item.hint) {
                SelectField(label: item.label, options: item.options, chosen: item.value.int) { setSetting(item.id, $0) }
            }
        case "stepper":
            RowShell(label: item.label, hint: item.hint) {
                StepperField(label: item.label, value: item.value.int, range: item.min...item.max, shown: "\(item.value.int)\(item.suffix)") {
                    setSetting(item.id, $0)
                }
            }
        case "text": TextRow(item: item)
        case "hotkey": if item.keys != nil { HotkeyRow(item: item) }
        case "preset": SettingsPresetRow(item: item)
        case "head": HeadRow(item: item)
        case "link": LinkRow(item: item)
        case "calendar": CalendarGrid(weeks: item.value.array)
        case "usage": UsageRow(item: item)
        case "note": NoteRow(words: item.hint)
        case "notes": NotesBlock(item: item, first: first)
        case "block", "ends": ButtonsBlock(buttons: item.buttons, ends: item.kind == "ends", first: first)
        default:
            RowShell(label: item.label, hint: item.hint) { KitButtons(buttons: item.buttons) }
        }
    }
}

/// `SettingsRow`: the name over its line at the left, in the middle of the row; the control at the right in the middle
/// of the first 48 (a switch in the middle of the whole row). At least 48 high.
struct RowShell<Control: View>: View {
    let label: String
    let hint: String
    var centered = false
    @ViewBuilder let control: () -> Control
    @Environment(\.colorScheme) private var scheme

    var body: some View {
        let k = SK.of(scheme)
        HStack(alignment: centered ? .center : .top, spacing: 16) {
            VStack(alignment: .leading, spacing: 2) {
                Text(label).kitLine(SK.body, SK.bodyLine).foregroundStyle(k.fg)
                if !hint.isEmpty { Text(hint).kitLine(SK.small, SK.smallLine).foregroundStyle(k.mutedFg) }
            }
            .fixedSize(horizontal: false, vertical: true)
            .frame(maxWidth: .infinity, minHeight: 32, alignment: .leading)
            .padding(.vertical, 8)
            HStack(spacing: 6) { control() }
                .frame(minHeight: centered ? nil : 30)
                .padding(.top, centered ? 0 : 9)
        }
        .frame(minHeight: 48)
    }
}

/// A row's buttons, left to right.
struct KitButtons: View {
    let buttons: [SettingsButton]
    var spacing: CGFloat = 6

    var body: some View {
        HStack(spacing: spacing) {
            ForEach(Array(buttons.enumerated()), id: \.offset) { _, button in
                Button(button.label) { pressSetting(button) }.buttonStyle(KitButtonStyle())
            }
        }
    }
}

/// A paragraph of the muted kind that explains a group: a row of its own.
private struct NoteRow: View {
    let words: String
    @Environment(\.colorScheme) private var scheme

    var body: some View {
        Text(words).kitLine(SK.small, SK.smallLine).foregroundStyle(SK.of(scheme).mutedFg)
            .fixedSize(horizontal: false, vertical: true)
            .frame(maxWidth: .infinity, alignment: .leading)
            .padding(.vertical, 12)
    }
}

/// Buttons out of a row, in the card's padding: from the left (no room between them, as the egui block), or the first at
/// the left and the others at the right.
private struct ButtonsBlock: View {
    let buttons: [SettingsButton]
    let ends: Bool
    let first: Bool

    var body: some View {
        HStack(spacing: 0) {
            if ends, let head = buttons.first {
                KitButtons(buttons: [head])
                Spacer(minLength: 6.5)
                KitButtons(buttons: Array(buttons.dropFirst()), spacing: 6.5)
            } else {
                KitButtons(buttons: buttons, spacing: 0)
                Spacer(minLength: 0)
            }
        }
        .padding(.top, first ? 10 : 6.75)
    }
}

/// A release's notes and the button under them.
private struct NotesBlock: View {
    let item: SettingsItem
    let first: Bool
    @Environment(\.colorScheme) private var scheme

    var body: some View {
        VStack(alignment: .leading, spacing: 6.5) {
            Text(item.hint).kitLine(SK.small, SK.smallLine).foregroundStyle(SK.of(scheme).fg).textSelection(.enabled)
                .fixedSize(horizontal: false, vertical: true)
                .frame(maxWidth: .infinity, alignment: .leading)
            KitButtons(buttons: item.buttons)
        }
        .padding(.top, first ? 10 : 6.75)
    }
}

/// An agent's name over what it is using.
private struct UsageRow: View {
    let item: SettingsItem
    @Environment(\.colorScheme) private var scheme

    var body: some View {
        let k = SK.of(scheme)
        VStack(alignment: .leading, spacing: 0) {
            Text(item.label).kitLine(SK.body, SK.bodyLine).foregroundStyle(k.fg)
            Text(item.hint).kitLine(SK.small, SK.smallLine).foregroundStyle(k.mutedFg)
        }
        .fixedSize(horizontal: false, vertical: true)
        .frame(maxWidth: .infinity, alignment: .leading)
        .padding(.bottom, 6.5)
    }
}

/// An integration's head: its dot (green connected, red not), the name over what it does, and its switch at the right.
private struct HeadRow: View {
    let item: SettingsItem
    @Environment(\.colorScheme) private var scheme

    var body: some View {
        let k = SK.of(scheme)
        HStack(alignment: .top, spacing: 0) {
            Circle().fill(item.tone == "ok" ? SK.green : k.destructive).frame(width: 8, height: 8)
                .padding(.leading, 1)
                .padding(.top, 5)
                .accessibilityLabel(item.tone == "ok" ? "Conectado" : "Não conectado")
            VStack(alignment: .leading, spacing: 1.625) {
                Text(item.label).kitLine(SK.body, SK.bodyLine).foregroundStyle(k.fg)
                Text(item.hint).kitLine(SK.small, SK.smallLine).foregroundStyle(k.mutedFg)
            }
            .fixedSize(horizontal: false, vertical: true)
            .frame(maxWidth: .infinity, alignment: .leading)
            .padding(.leading, 7)
            .padding(.trailing, 13)
            Toggle(item.label, isOn: Binding(get: { item.value.bool }, set: { setSetting(item.id, $0) })).toggleStyle(SwitchStyle())
        }
        .padding(.vertical, 4.875)
        .padding(.top, 10)
    }
}

/// A line that opens a page: a mark in its color, the words cut to fit, and what is beside them at the right.
private struct LinkRow: View {
    let item: SettingsItem
    @Environment(\.colorScheme) private var scheme
    @State private var hover = false

    var body: some View {
        let k = SK.of(scheme)
        let beside = item.buttons.first
        Button(action: open) {
            HStack(spacing: 0) {
                Text(item.suffix).font(.kit(SK.small)).foregroundStyle(toneColor(item.tone, k)).frame(width: 18)
                Text(item.label).font(.kit(SK.small)).foregroundStyle(k.fg).lineLimit(1).truncationMode(.tail).padding(.leading, 4)
                Spacer(minLength: 16)
                Text(beside?.label ?? "").font(.kit(SK.small)).foregroundStyle(k.mutedFg).padding(.trailing, 6.5)
            }
            .frame(height: 24)
            .background(hover ? k.muted.opacity(scheme == .dark ? 0.5 : 1) : .clear, in: .rect(cornerRadius: 6.5))
            .contentShape(Rectangle())
        }
        .buttonStyle(.plain)
        .onHover { hover = $0 }
        .handCursor()
        .accessibilityLabel("\(item.label) (\(beside?.label ?? ""))")
        .accessibilityAddTraits(.isLink)
    }

    private func open() {
        if let button = item.buttons.first { pressSetting(button) }
    }
}

/// GitHub's contribution calendar: a square a day, a column a week, the greener the more was done.
private struct CalendarGrid: View {
    /// Each week's days as [weekday, level].
    let weeks: [JSON]
    @Environment(\.colorScheme) private var scheme

    var body: some View {
        let k = SK.of(scheme)
        Canvas { context, _ in
            for (column, week) in weeks.enumerated() {
                for day in week.array {
                    let (weekday, level) = (day.array.first?.int ?? 0, day.array.last?.int ?? 0)
                    let cell = Path(roundedRect: CGRect(x: CGFloat(column) * 10, y: CGFloat(weekday) * 10, width: 8, height: 8), cornerRadius: 2)
                    context.fill(cell, with: .color(k.muted))
                    if level > 0 { context.fill(cell, with: .color(SK.green.opacity(Double(level) / 4))) }
                }
            }
        }
        .frame(height: 70)
        .padding(.top, 3.25)
        .padding(.bottom, 14)
        .accessibilityLabel("Calendário de contribuições")
    }
}

/// A field: what is typed goes with Return or when the field is left; a secret starts empty and goes with its button
/// (Return is the rightmost one, Salvar). Recentes (when there are folders to offer) and the buttons follow it.
private struct TextRow: View {
    let item: SettingsItem
    @State private var draft = ""
    @FocusState private var focused: Bool
    @Environment(\.colorScheme) private var scheme

    var body: some View {
        let k = SK.of(scheme)
        RowShell(label: item.label, hint: item.hint) {
            field(k)
                .focused($focused)
                .kitWell(k, width: item.width, focused: focused)
                .onSubmit(submit)
                .onChange(of: focused) { _, now in if !now { commit() } }
                .onChange(of: item.value) { _, now in if !focused && !item.secret { draft = now.string } }
                .onChange(of: item.id, initial: true) { _, _ in draft = item.secret ? "" : item.value.string }
            if !item.options.isEmpty {
                Menu("Recentes") {
                    ForEach(item.options, id: \.self) { path in Button(path) { use(path) } }
                }
                .menuStyle(.button)
                .buttonStyle(KitButtonStyle())
                .menuIndicator(.hidden)
                .fixedSize()
                .help("Pastas recentes e favoritas")
            }
            ForEach(Array(item.buttons.enumerated()), id: \.offset) { _, button in
                Button(button.label) { press(button) }.buttonStyle(KitButtonStyle())
            }
        }
    }

    @ViewBuilder private func field(_ k: SK) -> some View {
        let prompt = Text(item.placeholder).foregroundStyle(k.mutedFg)
        if item.secret {
            SecureField(item.label, text: $draft, prompt: prompt)
        } else {
            TextField(item.label, text: $draft, prompt: prompt)
        }
    }

    private func use(_ path: String) {
        draft = path
        setSetting(item.id, path)
    }

    private func submit() {
        if item.secret, let save = item.buttons.last { press(save) } else { commit() }
    }

    private func commit() {
        // a secret is sent by its button, with the press: never kept
        guard !item.secret, draft != item.value.string else { return }
        setSetting(item.id, draft)
    }

    private func press(_ button: SettingsButton) {
        if button.id == "choose_folder" {
            let panel = NSOpenPanel()
            panel.title = "Pasta dos chats"
            panel.canChooseFiles = false
            panel.canChooseDirectories = true
            if panel.runModal() == .OK, let url = panel.url { use(url.path) }
            return
        }
        if item.secret {
            pressSetting(button, value: draft)
            draft = ""
        } else {
            commit()
            pressSetting(button)
        }
    }
}

/// The question's bar (`dialog.rs`): Cancelar at the left, the title in the middle, the confirm at the right, in the
/// accent, a line under it.
private struct DialogBar: View {
    let title: String
    let confirm: String
    let cancelVerb: String
    let confirmVerb: String
    @Environment(\.colorScheme) private var scheme

    var body: some View {
        let k = SK.of(scheme)
        ZStack {
            Text(title).font(.kit(15)).foregroundStyle(k.fg).lineLimit(1)
            HStack {
                BarButton(text: "Cancelar") { settingsSend(cancelVerb) }.keyboardShortcut(.cancelAction)
                Spacer()
                BarButton(text: confirm) { settingsSend(confirmVerb) }.keyboardShortcut(.defaultAction)
            }
            .padding(.horizontal, 9)
        }
        .frame(height: 44)
        .overlay(alignment: .bottom) { Rectangle().fill(k.border).frame(height: 1) }
    }
}

private struct BarButton: View {
    let text: String
    let action: () -> Void
    @Environment(\.colorScheme) private var scheme
    @State private var hover = false

    var body: some View {
        let k = SK.of(scheme)
        Button(action: action) {
            Text(text).font(.kit(15)).foregroundStyle(k.primary)
                .padding(.horizontal, 10)
                .frame(height: 27)
                .background(hover ? k.muted.opacity(scheme == .dark ? 0.5 : 1) : .clear, in: .rect(cornerRadius: 8.125))
                .contentShape(Rectangle())
        }
        .buttonStyle(.plain)
        .onHover { hover = $0 }
        .handCursor()
    }
}

/// The question before Claude Code's settings.json is written: the file as it is and as it will be.
struct HooksPreviewSheet: View {
    let preview: SettingsPreview
    @Environment(\.colorScheme) private var scheme

    var body: some View {
        let k = SK.of(scheme)
        VStack(spacing: 0) {
            DialogBar(title: preview.title, confirm: preview.confirm, cancelVerb: "preview_no", confirmVerb: "preview_yes")
            VStack(alignment: .leading, spacing: 0) {
                Text(preview.state).font(.kit(SK.body)).foregroundStyle(k.fg).fixedSize(horizontal: false, vertical: true)
                Text(preview.path).font(.kit(SK.small)).foregroundStyle(k.mutedFg).padding(.top, 4)
                ScrollView([.vertical, .horizontal]) {
                    VStack(alignment: .leading, spacing: 0) {
                        ForEach(Array(preview.diff.enumerated()), id: \.offset) { _, line in
                            Text(line).font(.custom("Hack", size: 12)).foregroundStyle(diffColor(line, k)).fixedSize()
                        }
                    }
                    .frame(maxWidth: .infinity, alignment: .leading)
                }
                .frame(maxHeight: 360)
                .padding(.top, 8)
            }
            .padding(.horizontal, 13)
            .padding(.top, 16.25)
            .padding(.bottom, 10)
        }
        .frame(width: 480)
        .background(k.bg)
    }

    private func diffColor(_ line: String, _ k: SK) -> Color {
        if line.hasPrefix("+") { return Color(red: 100 / 255, green: 190 / 255, blue: 130 / 255) }
        if line.hasPrefix("−") { return Color(red: 210 / 255, green: 90 / 255, blue: 90 / 255) }
        return k.fg
    }
}

/// Limpar todos os dados asks first: what goes and that it cannot be undone, then the folders.
struct WipeSheet: View {
    let lines: [String]
    @Environment(\.colorScheme) private var scheme

    var body: some View {
        let k = SK.of(scheme)
        VStack(spacing: 0) {
            DialogBar(title: "Apagar todos os dados do MagoAgent?", confirm: "Apagar tudo", cancelVerb: "wipe_no", confirmVerb: "wipe_yes")
            VStack(alignment: .leading, spacing: 3) {
                ForEach(Array(lines.dropLast().enumerated()), id: \.offset) { _, line in
                    Text(line).font(.kit(SK.body)).foregroundStyle(k.fg).fixedSize(horizontal: false, vertical: true)
                }
                Text(lines.last ?? "").font(.kit(SK.small)).foregroundStyle(k.mutedFg).textSelection(.enabled).padding(.top, 5)
            }
            .frame(maxWidth: .infinity, alignment: .leading)
            .padding(.horizontal, 13)
            .padding(.top, 16.25)
            .padding(.bottom, 10)
        }
        .frame(width: 480)
        .background(k.bg)
    }
}
