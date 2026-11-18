// A sonda (MAGOAGENT_SWIFTUI=probe): prova a ida e a volta da ponte. Mostra o tópico "app" e manda `ui.ping`.
import AppKit
import SwiftUI

/// O tópico "app" (`AppTopic` em src/app/native/mod.rs).
struct AppTopic: Codable {
    let version: String
    let agents: Int
}

struct ProbeView: View {
    @State private var reply = "…"

    var body: some View {
        let app = MagoStore.shared.decode(AppTopic.self, topic: "app")
        VStack(alignment: .leading, spacing: 8) {
            Text("MagoAgent \(app?.version ?? "?")").font(.headline)
            Text("Agentes: \(app?.agents ?? 0)")
            HStack {
                Button("Ping") { Task { await ping() } }
                Text(reply).monospaced()
            }
        }
        .padding()
        .frame(width: 240, alignment: .leading)
        // o primeiro ping sozinho: a volta aparece sem clicar
        .task { await ping() }
    }

    private func ping() async {
        let answer = await MagoStore.shared.act("ui.ping")
        reply = answer.ok ? answer.text : "erro: \(answer.text)"
    }
}

@MainActor
enum Probe {
    static func open() {
        MagoWindow.open(ProbeView(), frame: NSRect(x: 80, y: 80, width: 240, height: 110), title: "Sonda SwiftUI", panel: true, level: .floating)
    }
}
