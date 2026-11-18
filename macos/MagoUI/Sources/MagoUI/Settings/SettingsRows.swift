import AppKit
import SwiftUI

private func toneColor(_ tone: String) -> Color {
    switch tone {
    case "ok": Color(red: 0.20, green: 0.78, blue: 0.35)
    case "bad": .red
    case "warn": Color(red: 0.96, green: 0.65, blue: 0.14)
    default: .secondary
    }
}

/// One row, drawn by its kind.
struct ItemRow: View {
    let item: SettingsItem

    var body: some View {
        switch item.kind {
        case "toggle": ToggleRow(item: item)
        case "head": HeadRow(item: item)
        case "choice": ChoiceRow(item: item, segmented: true)
        case "select": ChoiceRow(item: item, segmented: false)
        case "stepper": StepperRow(item: item)
        case "text": TextRow(item: item)
        case "hotkey": HotkeyRow(item: item)
        case "preset": PresetRow(item: item)
        case "link": LinkRow(item: item)
        case "note": Text(item.hint).font(.callout).foregroundStyle(.secondary).fixedSize(horizontal: false, vertical: true)
            .frame(maxWidth: .infinity, alignment: .leading)
        case "notes": Text(item.hint).font(.callout).textSelection(.enabled).fixedSize(horizontal: false, vertical: true)
            .frame(maxWidth: .infinity, alignment: .leading)
        default: Shell(item: item) { Buttons(buttons: item.buttons) }
        }
    }
}

/// The name and the line under it at the left, the control at the right.
struct Shell<Control: View>: View {
    let item: SettingsItem
    @ViewBuilder var control: () -> Control

    var body: some View {
        HStack(alignment: .center, spacing: 12) {
            VStack(alignment: .leading, spacing: 2) {
                if !item.label.isEmpty { Text(item.label) }
                if !item.hint.isEmpty {
                    Text(item.hint).font(.caption).foregroundStyle(.secondary).fixedSize(horizontal: false, vertical: true)
                }
            }
            .frame(maxWidth: .infinity, alignment: .leading)
            control()
        }
    }
}

struct Buttons: View {
    let buttons: [SettingsButton]
    var typed: String = ""
    var after: () -> Void = {}

    var body: some View {
        HStack(spacing: 6) {
            ForEach(Array(buttons.enumerated()), id: \.offset) { _, button in
                Button(button.label) {
                    pressSetting(button, value: typed)
                    after()
                }
                .buttonStyle(.bordered)
                .tint(button.primary ? .accentColor : (button.destructive ? .red : nil))
            }
        }
    }
}

private struct ToggleRow: View {
    let item: SettingsItem

    var body: some View {
        Shell(item: item) {
            Toggle(item.label, isOn: Binding(get: { item.value.bool }, set: { setSetting(item.id, $0) }))
                .labelsHidden().toggleStyle(.switch)
        }
    }
}

private struct HeadRow: View {
    let item: SettingsItem

    var body: some View {
        HStack(alignment: .top, spacing: 8) {
            Circle().fill(toneColor(item.tone)).frame(width: 8, height: 8).padding(.top, 6)
            Shell(item: item) {
                Toggle(item.label, isOn: Binding(get: { item.value.bool }, set: { setSetting(item.id, $0) }))
                    .labelsHidden().toggleStyle(.switch)
            }
        }
    }
}

private struct ChoiceRow: View {
    let item: SettingsItem
    let segmented: Bool

    var body: some View {
        let selection = Binding(get: { item.value.int }, set: { setSetting(item.id, $0) })
        Shell(item: item) {
            let picker = Picker(item.label, selection: selection) {
                ForEach(Array(item.options.enumerated()), id: \.offset) { index, name in Text(name).tag(index) }
            }
            .labelsHidden()
            if segmented {
                picker.pickerStyle(.segmented).fixedSize()
            } else {
                picker.pickerStyle(.menu).fixedSize()
            }
        }
    }
}

private struct StepperRow: View {
    let item: SettingsItem

    /// The pause is kept in quarters of a second and read as seconds ("1,5 s").
    private var shown: String {
        let value = item.value.int
        if item.placeholder == "quarters" { return String(format: "%g", Double(value) / 4).replacingOccurrences(of: ".", with: ",") + item.suffix }
        return "\(value)\(item.suffix)"
    }

    var body: some View {
        Shell(item: item) {
            Stepper(shown, value: Binding(get: { item.value.int }, set: { setSetting(item.id, $0) }), in: item.min...item.max)
                .monospacedDigit()
        }
    }
}

/// A field: what is typed goes with Return or when the field is left; a secret starts empty and goes with its button.
private struct TextRow: View {
    let item: SettingsItem
    @State private var draft = ""
    @State private var edited = false
    @FocusState private var focused: Bool

    var body: some View {
        Shell(item: item) {
            HStack(spacing: 6) {
                if !item.options.isEmpty {
                    Menu("Recentes") {
                        ForEach(item.options, id: \.self) { path in Button(path) { draft = path; setSetting(item.id, path) } }
                    }
                    .fixedSize()
                }
                field.frame(width: 240).focused($focused)
                    .onSubmit(commit)
                    .onChange(of: focused) { _, now in if !now { commit() } }
                    .onChange(of: item.value) { _, now in if !focused && !item.secret { draft = now.string } }
                    .onAppear { draft = item.secret ? "" : item.value.string }
                    .onChange(of: item.id, initial: false) { _, _ in draft = item.secret ? "" : item.value.string }
                ForEach(Array(item.buttons.enumerated()), id: \.offset) { _, button in
                    Button(button.label) { press(button) }
                        .buttonStyle(.bordered)
                        .tint(button.primary ? .accentColor : nil)
                }
            }
        }
    }

    @ViewBuilder private var field: some View {
        if item.secret {
            SecureField(item.placeholder, text: $draft).textFieldStyle(.roundedBorder).accessibilityLabel(item.label)
        } else {
            TextField(item.placeholder, text: $draft).textFieldStyle(.roundedBorder).accessibilityLabel(item.label)
        }
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
            if panel.runModal() == .OK, let url = panel.url {
                draft = url.path
                setSetting(item.id, url.path)
            }
            return
        }
        if item.secret {
            let typed = draft
            // the Return of a secret that ends the editing is its Salvar
            pressSetting(button, value: typed)
            draft = ""
        } else {
            commit()
            pressSetting(button)
        }
    }
}

private struct LinkRow: View {
    let item: SettingsItem

    var body: some View {
        HStack(spacing: 8) {
            Text(item.suffix).foregroundStyle(toneColor(item.tone)).frame(width: 16)
            Text(item.label).lineLimit(1).truncationMode(.tail).frame(maxWidth: .infinity, alignment: .leading)
            ForEach(Array(item.buttons.enumerated()), id: \.offset) { _, button in
                Button(button.label) { pressSetting(button) }.buttonStyle(.link).font(.caption)
            }
        }
        .font(.callout)
    }
}

/// A preset of the new-terminal dialog: its check, name over command, and the arrows, edit and remove at the right.
private struct PresetRow: View {
    let item: SettingsItem
    @State private var editing = false

    private var on: Bool { item.value["on"].bool }

    var body: some View {
        VStack(alignment: .leading, spacing: 8) {
            HStack(spacing: 10) {
                Button {
                    setSetting(item.id + ".on", !on)
                } label: {
                    Image(systemName: on ? "checkmark.circle.fill" : "circle").foregroundStyle(on ? Color.accentColor : .secondary)
                }
                .buttonStyle(.plain)
                .accessibilityLabel("Mostrar \(item.label.isEmpty ? "sem nome" : item.label) no diálogo")
                if editing {
                    PresetField(id: item.id + ".name", value: item.value["name"].string, placeholder: "Nome").frame(width: 150)
                    PresetField(id: item.id + ".command", value: item.value["command"].string, placeholder: "comando (vazio: shell)").frame(maxWidth: .infinity)
                } else {
                    VStack(alignment: .leading, spacing: 1) {
                        Text(item.label.isEmpty ? "sem nome" : item.label).foregroundStyle(item.label.isEmpty ? .secondary : .primary)
                        Text(item.hint.isEmpty ? "shell" : item.hint).font(.caption).foregroundStyle(.secondary)
                    }
                    .frame(maxWidth: .infinity, alignment: .leading)
                }
                icon("chevron.up", "Mover para cima", disabled: item.value["first"].bool) { pressSetting(op("up")) }
                icon("chevron.down", "Mover para baixo", disabled: item.value["last"].bool) { pressSetting(op("down")) }
                icon(editing ? "checkmark" : "pencil", editing ? "Pronto" : "Editar", disabled: false) { editing.toggle() }
                icon("minus.circle", "Remover", disabled: false) { pressSetting(op("remove")) }
            }
            if editing {
                Picker("Ícone", selection: Binding(get: { item.value["icon"].int }, set: { setSetting(item.id + ".icon", $0) })) {
                    ForEach(Array(item.options.enumerated()), id: \.offset) { index, name in Text(name).tag(index) }
                }
                .pickerStyle(.menu).fixedSize()
                .padding(.leading, 28)
            }
        }
    }

    private func op(_ name: String) -> SettingsButton {
        SettingsButton(id: item.id + "." + name, label: name, arg: nil, copy: nil, primary: false, destructive: false)
    }

    private func icon(_ symbol: String, _ tip: String, disabled: Bool, _ action: @escaping () -> Void) -> some View {
        Button(action: action) { Image(systemName: symbol) }
            .buttonStyle(.borderless).disabled(disabled).help(tip).accessibilityLabel("\(tip) \(item.label)")
    }
}

private struct PresetField: View {
    let id: String
    let value: String
    let placeholder: String
    @State private var draft = ""
    @FocusState private var focused: Bool

    var body: some View {
        TextField(placeholder, text: $draft).textFieldStyle(.roundedBorder).focused($focused)
            .onAppear { draft = value }
            .onChange(of: draft) { _, now in setSetting(id, now) }
    }
}

/// The question before Claude Code's settings.json is written: the file as it is and as it will be.
struct HooksPreviewSheet: View {
    let preview: SettingsPreview

    var body: some View {
        VStack(alignment: .leading, spacing: 10) {
            Text(preview.title).font(.headline)
            Text(preview.state)
            Text(preview.path).font(.caption).foregroundStyle(.secondary)
            ScrollView([.vertical, .horizontal]) {
                VStack(alignment: .leading, spacing: 0) {
                    ForEach(Array(preview.diff.enumerated()), id: \.offset) { _, line in
                        Text(line).font(.system(.caption, design: .monospaced))
                            .foregroundStyle(line.hasPrefix("+") ? Color.green : (line.hasPrefix("−") ? Color.red : .primary))
                    }
                }
                .frame(maxWidth: .infinity, alignment: .leading)
            }
            .frame(height: 320)
            .padding(8)
            .background(Color(nsColor: .textBackgroundColor), in: RoundedRectangle(cornerRadius: 8))
            HStack {
                Spacer()
                Button("Cancelar") { settingsSend("preview_no") }.keyboardShortcut(.cancelAction)
                Button(preview.confirm) { settingsSend("preview_yes") }.keyboardShortcut(.defaultAction).buttonStyle(.borderedProminent)
            }
        }
        .padding(20)
        .frame(width: 640)
    }
}

/// Limpar todos os dados asks first: what goes and that it cannot be undone.
struct WipeSheet: View {
    let lines: [String]

    var body: some View {
        VStack(alignment: .leading, spacing: 8) {
            Text("Apagar todos os dados do MagoAgent?").font(.headline)
            ForEach(Array(lines.dropLast().enumerated()), id: \.offset) { _, line in Text(line).fixedSize(horizontal: false, vertical: true) }
            Text(lines.last ?? "").font(.caption).foregroundStyle(.secondary).textSelection(.enabled)
            HStack {
                Spacer()
                Button("Cancelar") { settingsSend("wipe_no") }.keyboardShortcut(.cancelAction)
                Button("Apagar tudo", role: .destructive) { settingsSend("wipe_yes") }.buttonStyle(.borderedProminent).tint(.red)
            }
            .padding(.top, 6)
        }
        .padding(20)
        .frame(width: 520)
    }
}
