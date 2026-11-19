// O lado Swift do contrato C (Sources/MagoBridgeC/include/mago_ui.h): o Rust chama estas funções, sempre na
// thread principal (`MainActor.assumeIsolated` derruba o app se não for, de propósito).
import CoreText
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

/// A NSView do canvas onde o Rust põe as páginas dos navegadores (Browser/PageHost.swift); nil fora de uma janela.
@_cdecl("mago_ui_page_host")
public func magoUIPageHost() -> UnsafeMutableRawPointer? {
    let address = MainActor.assumeIsolated { PageHostView.live.map { UInt(bitPattern: Unmanaged.passUnretained($0).toOpaque()) } }
    return address.flatMap { UnsafeMutableRawPointer(bitPattern: $0) }
}

/// Uma fonte do app vinda do Rust (Assistant, Excalifont, Hack): registrada para o processo, as vistas a usam pelo nome
/// (`Font.app`, `CTFontCreateWithName("Hack")`).
@_cdecl("mago_ui_font")
public func magoUIFont(_ bytes: UnsafePointer<UInt8>, _ length: UInt) {
    let data = Data(bytes: bytes, count: Int(length)) as CFData
    guard let provider = CGDataProvider(data: data), let font = CGFont(provider) else { return }
    CTFontManagerRegisterGraphicsFont(font, nil)
}
