// As áreas ligadas por MAGOAGENT_SWIFTUI: o Rust publica a lista no tópico "areas" e cada uma abre aqui.
// Área nova: um caso a mais, chamando o `open()` da pasta dela.

@MainActor
enum Areas {
    static func open(_ names: [String]) {
        for name in names {
            switch name {
            case "probe": Probe.open()
            default: break
            }
        }
    }
}
