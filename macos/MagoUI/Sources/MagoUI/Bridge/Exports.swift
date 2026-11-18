// O lado Swift do contrato C (Sources/MagoBridgeC/include/mago_ui.h): o Rust chama estas três funções, sempre na
// thread principal (`MainActor.assumeIsolated` derruba o app se não for, de propósito).
import Foundation
import MagoBridgeC

/// Liga a interface: guarda as funções do Rust. As áreas abrem quando chega o tópico "areas".
@_cdecl("mago_ui_start")
public func magoUIStart(_ callbacks: UnsafePointer<MagoCallbacks>) {
    let callbacks = callbacks.pointee
    MainActor.assumeIsolated { MagoStore.shared.start(callbacks) }
}

/// Um tópico novo do Rust (o JSON inteiro, só quando mudou); "reply" responde a um `act`.
@_cdecl("mago_ui_publish")
public func magoUIPublish(_ topic: UnsafePointer<CChar>, _ json: UnsafePointer<CChar>) {
    let topic = String(cString: topic)
    let json = Data(String(cString: json).utf8)
    MainActor.assumeIsolated { MagoStore.shared.receive(topic: topic, json: json) }
}

/// O app vai fechar: fecha as janelas e responde o que ainda esperava.
@_cdecl("mago_ui_stop")
public func magoUIStop() {
    MainActor.assumeIsolated { MagoStore.shared.stop() }
}
