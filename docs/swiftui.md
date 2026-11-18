# Interface SwiftUI no macOS

O núcleo continua em Rust; no Mac a interface passa para SwiftUI, área por área. Enquanto uma área não fica pronta,
o egui continua desenhando ela. Windows e Linux não mudam: tudo daqui fica atrás de `cfg(target_os = "macos")` e o
`build.rs` não faz nada fora do Mac.

## Peças

| Onde | O quê |
|---|---|
| `macos/MagoUI/` | pacote SwiftPM (swift-tools 6.0, macOS 14+), produto estático `MagoUI` |
| `macos/MagoUI/Sources/MagoBridgeC/include/mago_ui.h` | o contrato C (a struct `MagoCallbacks` e as três funções) |
| `macos/MagoUI/Sources/MagoUI/Bridge/` | `Exports.swift` (o `@_cdecl`), `MagoStore`, `MagoWindow`, `Areas.swift` |
| `macos/MagoUI/Sources/MagoUI/<Área>/` | uma pasta por área: `Island/`, `Chat/`, `Canvas/`, `Terminal/`, `Shell/`, `Settings/`, `FileTree/` |
| `src/app/native/mod.rs` | o lado Rust: a chave, a fila de ações, `publish`, as ações `ui.*` |
| `build.rs` | no Mac roda `swift build -c release --package-path macos/MagoUI` e liga a biblioteca e o runtime do Swift |

## O contrato C

```c
typedef struct MagoCallbacks {
    void (*act)(const char *json);   // Swift → Rust: uma ação (envelope abaixo)
} MagoCallbacks;

void mago_ui_start(const MagoCallbacks *callbacks);        // Rust → Swift, uma vez, na criação do app
void mago_ui_publish(const char *topic, const char *json); // Rust → Swift, a cada tópico que mudou
void mago_ui_stop(void);                                   // Rust → Swift, ao fechar
```

- Textos em UTF-8 terminados em zero, válidos só durante a chamada (quem recebe copia).
- **Tudo na thread principal.** O Rust chama as três a partir do quadro do eframe; o Swift chama `act` de código
  `@MainActor`. Do lado Swift, `MainActor.assumeIsolated` derruba o app se alguém chamar de outra thread.
- Campo novo em `MagoCallbacks`: só no fim, mudando junto o `mago_ui.h` e o `MagoCallbacks` do Rust.

## Ações (Swift → Rust)

Envelope: `{"id": <u64>, "action": "<nome>", "args": {...}}` (`args` pode faltar).

O callback só põe o envelope numa fila e pede um quadro (`request_repaint`). O `DrawingApp::native_tick`, chamado no
`logic` de cada quadro (roda também com a janela escondida), esvazia a fila:

- `action` começando com `ui.`: ação só de interface, tratada em `src/app/native/mod.rs` (`ui_action`). Hoje:
  `ui.ping` → `"pong"`.
- qualquer outro nome: `DrawingApp::act(action, &args)`, **as mesmas ações do MCP** (`src/mcp/actions/`). Não duplique
  lógica no Swift: se a pessoa faz algo, a ação já existe (ou deve existir) ali, com a ferramenta no MCP. As que
  esperam rede ou um CLI (`act_later`) rodam numa thread, e a resposta vem num quadro seguinte.

A resposta volta no tópico `reply`: `{"id": <u64>, "ok": bool, "text": "<o que a ação disse, ou o erro>"}`. Um
envelope que não é JSON válido responde com `id` 0 e `ok` falso.

No Swift:

```swift
let reply = await MagoStore.shared.act("chat_new", ["agent": "claude", "folder": "/Users/eu/projeto"])
if !reply.ok { erro = reply.text }
```

## Tópicos (Rust → Swift)

`native::publish(topic, &valor)` serializa com serde, compara com o último JSON mandado naquele tópico e só manda
quando mudou (pode chamar a cada quadro). Só na thread principal, de dentro do quadro.

Tópicos de agora:

| Tópico | Conteúdo |
|---|---|
| `areas` | as áreas ligadas, `["probe"]`; o Swift abre cada uma (`Areas.swift`) |
| `reply` | a resposta de uma ação (acima) |
| `app` | só com `probe`: `{"version": "0.0.30", "agents": 2}` |

No Swift, `MagoStore.shared.topics[topico]` guarda o último JSON e `MagoStore.shared.decode(Tipo.self, topic: "...")`
decodifica (nil antes do primeiro; um JSON que não bate com o tipo aparece no Console como `MagoUI: o tópico ...`).
`MagoStore` é `@Observable`: uma vista que lê um tópico redesenha quando ele muda.

## A chave `MAGOAGENT_SWIFTUI`

Desligada por padrão. `all` liga todas as áreas; uma lista separada por vírgulas liga só essas:

```bash
MAGOAGENT_SWIFTUI=island,chat target/release/magoagent
```

Áreas: `island`, `chat`, `canvas`, `terminal`, `shell` (a moldura da janela principal), `settings`, `filetree`.
`probe` (a sonda da ponte) só pelo nome, nunca no `all`. No Rust, `native::enabled("island")` diz se a área é do
SwiftUI: a área do egui deixa de desenhar quando ela é.

## Como uma área entra

1. **Rust** (`src/app/<area>/`, ou um arquivo `native.rs` na pasta da área, atrás de `#[cfg(target_os = "macos")]`):
   uma struct `#[derive(Serialize)]` com o que a tela mostra, publicada a cada quadro:
   ```rust
   #[cfg(target_os = "macos")]
   if crate::app::native::enabled("island") {
       crate::app::native::publish("island", &IslandTopic { agents: ..., pending: ... });
   }
   ```
   e o egui pula o desenho dela quando `enabled` é verdadeiro.
2. **Swift** (`macos/MagoUI/Sources/MagoUI/Island/`): a struct `Codable` com os mesmos campos, as vistas e um
   `enum Island { static func open() }` que abre a janela com `MagoWindow.open(...)`; um caso a mais em
   `Bridge/Areas.swift` (`case "island": Island.open()`).
3. **Ações**: o que a pessoa faz vai por `MagoStore.shared.act(nome, args)` com as ações do MCP. O que só existe para a
   janela (posição de um painel, um hover) é um `ui.<nome>` novo em `ui_action`, com teste em `src/app/native/`.
4. **Teste**: a struct do tópico e as ações novas ganham teste no Rust (`cargo test --release`).

`MagoWindow.open(conteudo, frame:, title:, panel:, borderless:, transparent:, level:)` cria a janela (ou um `NSPanel`
que não ativa o app nem some quando ele perde o foco) com um `NSHostingView`, na frente sem roubar o foco. As janelas
abertas por ele fecham no `mago_ui_stop`.

## Como compilar

`cargo build --release` faz tudo: o `build.rs` roda o `swift build` (o cache fica em `macos/MagoUI/.build`, fora do
git) e liga `libMagoUI.a`, o runtime do Swift (`<SDK>/usr/lib/swift`, o `lib/swift/macosx` do toolchain, rpath
`/usr/lib/swift`) e os frameworks SwiftUI, AppKit, Foundation, Combine e QuartzCore. Mexer em `macos/MagoUI/Sources`
ou no `Package.swift` recompila.

Só o Swift, para ver os erros mais rápido:

```bash
swift build -c release --package-path macos/MagoUI
```

- Basta o Command Line Tools (sem Xcode). A partir do SDK do macOS 27 o `@State` do SwiftUI é uma macro cujo plugin
  (SwiftUIMacros) só vem com o Xcode; com só o Command Line Tools o `build.rs` usa o `MacOSX26.sdk` ao lado. Para
  rodar o `swift build` na mão nessa situação: `SDKROOT=/Library/Developer/CommandLineTools/SDKs/MacOSX26.sdk swift build ...`.
  Um `SDKROOT` definido sempre vence.
- O app exige macOS 14 (`@Observable`): `LSMinimumSystemVersion` 14.0 no `build-macos-app.sh`.

## Sonda

```bash
printf '{"version":1,"elements":[],"background":[255,255,255,255]}' > /tmp/swiftui-probe.mago
MAGOAGENT_DEV=1 MAGOAGENT_SCENE=/tmp/swiftui-probe.mago MAGOAGENT_SWIFTUI=probe \
  MAGOAGENT_SCRIPT="size 1000 700; wait 3000; quit" target/release/magoagent
```

Abre um painel "Sonda SwiftUI" com a versão e o número de agentes (tópico `app`) e manda um `ui.ping` sozinho ao abrir
(e no botão Ping); `pong` na tela prova a ida e a volta. O `shot` do roteiro só pega o que o egui desenha: o painel
SwiftUI não sai na captura.
