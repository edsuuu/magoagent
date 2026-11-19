import SwiftUI

/// Design's Enviar (egui's `design_confirm_ui`): the text the page put together goes to the agent only by a click on Enviar,
/// once the card has been up a moment. No Return: this is the app's own answer, out of the page's reach.
struct DesignConfirm: View {
    let ask: DesignAsk
    @State private var settled = false

    var body: some View {
        ZStack {
            Color.black.opacity(0.28).onTapGesture { answer(false) }
            VStack(alignment: .leading, spacing: 14) {
                Text("Enviar a \(ask.name)?").font(.system(size: 16, weight: .semibold))
                Text("Este texto \(ask.chat ? "vai para o chat do agente" : "vai ser digitado no terminal do agente"). Confira e clique em Enviar:")
                    .font(.app(13))
                    .foregroundStyle(.secondary)
                ScrollView {
                    Text(ask.prompt).font(.custom("Hack", size: 12)).textSelection(.enabled).frame(maxWidth: .infinity, alignment: .leading)
                }
                .scrollIndicators(.visible)
                .frame(maxHeight: 260)
                HStack {
                    Spacer()
                    Button("Cancelar") { answer(false) }.keyboardShortcut(.cancelAction)
                    Button("Enviar") { answer(true) }.buttonStyle(.borderedProminent).disabled(!settled)
                }
            }
            .padding(22)
            .frame(width: 520)
            .background(.regularMaterial, in: RoundedRectangle(cornerRadius: 14, style: .continuous))
            .overlay(RoundedRectangle(cornerRadius: 14, style: .continuous).stroke(Color.primary.opacity(0.1), lineWidth: 1))
            .shadow(color: .black.opacity(0.25), radius: 24, y: 8)
        }
        .task(id: ask) {
            settled = false
            try? await Task.sleep(for: .milliseconds(400))
            settled = true
        }
    }

    private func answer(_ send: Bool) {
        browserVerb("send", ["send": send])
    }
}
