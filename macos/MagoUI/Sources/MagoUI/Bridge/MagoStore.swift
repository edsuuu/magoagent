// O estado que o Rust publica, por tópico, e as ações que a interface manda de volta.
import AppKit
import MagoBridgeC
import Observation

/// A resposta de uma ação (tópico "reply"): `text` é o que a ação disse, ou o erro quando `ok` é falso.
public struct Reply: Codable, Sendable {
    public let id: UInt64
    public let ok: Bool
    public let text: String
}

@MainActor @Observable
public final class MagoStore {
    public static let shared = MagoStore()

    /// O último JSON de cada tópico. ponytail: uma leitura observa o dicionário inteiro, então um tópico novo redesenha
    /// toda vista que lê algum; quando pesar, um `@Observable` por tópico
    public private(set) var topics: [String: Data] = [:]

    @ObservationIgnored private var callbacks: MagoCallbacks?
    @ObservationIgnored private var nextId: UInt64 = 0
    @ObservationIgnored private var waiting: [UInt64: CheckedContinuation<Reply, Never>] = [:]
    /// As janelas abertas pelas áreas (MagoWindow), fechadas no `stop`.
    @ObservationIgnored var windows: [NSWindow] = []

    func start(_ callbacks: MagoCallbacks) {
        self.callbacks = callbacks
    }

    func receive(topic: String, json: Data) {
        if topic == "reply" {
            guard let reply = try? JSONDecoder().decode(Reply.self, from: json) else { return }
            waiting.removeValue(forKey: reply.id)?.resume(returning: reply)
            return
        }
        topics[topic] = json
        if topic == "areas", let names = try? JSONDecoder().decode([String].self, from: json) {
            Areas.open(names)
        }
    }

    func stop() {
        for window in windows { window.close() }
        windows.removeAll()
        for (id, continuation) in waiting { continuation.resume(returning: Reply(id: id, ok: false, text: "a interface foi fechada")) }
        waiting.removeAll()
        callbacks = nil
    }

    /// O tópico decodificado no tipo da área (`store.decode(Island.self, topic: "island")`); nil antes do primeiro.
    public func decode<T: Decodable>(_ type: T.Type, topic: String) -> T? {
        guard let json = topics[topic] else { return nil }
        do {
            return try JSONDecoder().decode(type, from: json)
        } catch {
            NSLog("MagoUI: o tópico %@ não bate com %@: %@", topic, String(describing: type), String(describing: error))
            return nil
        }
    }

    /// Uma ação do app (as do MCP, `DrawingApp::act`, ou as de interface, "ui.*"), respondida no próximo quadro.
    public func act(_ action: String, _ args: [String: Any]? = nil) async -> Reply {
        nextId += 1
        let id = nextId
        let envelope: [String: Any] = ["id": id, "action": action, "args": args ?? [:]]
        guard let send = callbacks?.act else { return Reply(id: id, ok: false, text: "a interface não está ligada ao app") }
        guard JSONSerialization.isValidJSONObject(envelope), let json = try? JSONSerialization.data(withJSONObject: envelope) else {
            return Reply(id: id, ok: false, text: "os argumentos de \(action) não viram JSON")
        }
        return await withCheckedContinuation { continuation in
            waiting[id] = continuation
            String(decoding: json, as: UTF8.self).withCString { send($0) }
        }
    }
}
