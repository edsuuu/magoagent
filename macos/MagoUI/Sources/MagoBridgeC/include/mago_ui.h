// O contrato C entre o app em Rust (src/app/native/) e a interface SwiftUI (MagoUI). Tudo roda na thread principal.
// Os textos são UTF-8 terminados em zero, válidos só durante a chamada: quem recebe copia.
#ifndef MAGO_UI_H
#define MAGO_UI_H

// O que o Rust oferece ao Swift. Campo novo só no fim, e o Rust (`MagoCallbacks` em src/app/native/mod.rs) muda junto.
typedef struct MagoCallbacks {
    // uma ação para o app: {"id": <u64>, "action": "<nome>", "args": {...}}; a resposta volta no tópico "reply"
    void (*act)(const char *json);
} MagoCallbacks;

// Exportadas pelo Swift (@_cdecl em Bridge/Exports.swift), chamadas pelo Rust.
void mago_ui_start(const MagoCallbacks *callbacks);
void mago_ui_publish(const char *topic, const char *json);
void mago_ui_stop(void);
// uma fonte do app (os bytes do .ttf que o Rust embute), registrada no CoreText para as vistas a usarem pelo nome
void mago_ui_font(const unsigned char *bytes, unsigned long length);

#endif
