import SwiftUI

/// Nota rápida (app/panes/quick_note.rs): plain text in the code face (its hint in the interface's, as egui's `hint_text`),
/// kept with the preferences (the MCP's `quick_note`), ready to type when the panel opens.
struct QuickNoteBody: View {
    let note: String
    let look: PaneLook
    @State private var text = ""
    @FocusState private var focused: Bool

    var body: some View {
        TextEditor(text: $text)
            .font(.code(13))
            .foregroundStyle(Color(rgba: look.text))
            .scrollContentBackground(.hidden)
            .background(.clear)
            .focused($focused)
            .overlay(alignment: .topLeading) {
                if text.isEmpty {
                    Text("Escreva aqui. Texto simples, sem markdown; fica guardado até você levar para o canvas.")
                        .font(.app(14)).foregroundStyle(Color(rgba: look.muted))
                        .padding(.leading, 5)
                        .allowsHitTesting(false)
                }
            }
            .synced(note, $text) { MagoStore.shared.send("quick_note", ["action": "set", "text": $0]) }
            .onAppear { focused = true }
            .accessibilityLabel("Nota rápida")
    }
}

/// The bar under the text: how long it is and Levar para o canvas (a note on the canvas, as typed; one Cmd+Z takes it back).
struct QuickNoteBar: View {
    let note: String
    let look: PaneLook

    private var lines: Int {
        let text = note.trimmingCharacters(in: .whitespacesAndNewlines)
        return text.isEmpty ? 0 : text.split(separator: "\n", omittingEmptySubsequences: false).count
    }

    var body: some View {
        HStack {
            Text(lines == 0 ? "Vazia" : lines == 1 ? "1 linha" : "\(lines) linhas")
                .font(.app(12)).foregroundStyle(Color(rgba: look.muted))
            Spacer()
            Button("Levar para o canvas") { paneVerb("to_canvas") }
                .buttonStyle(PaneButtonStyle(look: look))
                .disabled(lines == 0)
                .help("Vira uma nota no canvas, com o texto como está")
        }
        .padding(.horizontal, 12).padding(.vertical, 8)
        .frame(height: 48)
    }
}
