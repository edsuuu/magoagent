# Interface SwiftUI no macOS

O núcleo continua em Rust; no Mac a interface passa para SwiftUI, área por área. Enquanto uma área não fica pronta,
o egui continua desenhando ela. Windows e Linux não mudam: tudo daqui fica atrás de `cfg(target_os = "macos")` e o
`build.rs` não faz nada fora do Mac.

## Peças

| Onde | O quê |
|---|---|
| `macos/MagoUI/` | pacote SwiftPM (swift-tools 6.0, macOS 14+), produto estático `MagoUI` |
| `macos/MagoUI/Sources/MagoBridgeC/include/mago_ui.h` | o contrato C (a struct `MagoCallbacks` e as três funções) |
| `macos/MagoUI/Sources/MagoUI/Bridge/` | `Exports.swift` (o `@_cdecl`), `MagoStore`, `MagoWindow`, `Areas.swift` (a ordem das áreas e `MagoViews`) |
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

- `action` começando com `ui.`: ação só de interface, roteada em `src/app/native/mod.rs` (`ui_action`): `ui.ping` →
  `"pong"`; `ui.<area>.<verbo>` → `DrawingApp::native_<area>_act(verbo, &args) -> Result<String, String>`, que a área
  define (um braço por área no `match`, marcado `AREAS`).
- qualquer outro nome: `DrawingApp::act(action, &args)`, **as mesmas ações do MCP** (`src/mcp/actions/`). Não duplique
  lógica no Swift: se a pessoa faz algo, a ação já existe (ou deve existir) ali, com a ferramenta no MCP. As que
  esperam rede ou um CLI (`act_later`) rodam numa thread, e a resposta vem num quadro seguinte.

A resposta volta no tópico `reply`: `{"id": <u64>, "ok": bool, "text": "<o que a ação disse, ou o erro>"}`. Um
envelope que não é JSON válido responde com `id` 0 e `ok` falso.

No Swift:

```swift
let reply = await MagoStore.shared.act("chat_new", ["agent": "claude", "folder": "/Users/eu/projeto"])
if !reply.ok { erro = reply.text }
MagoStore.shared.send("ui.island.hover", ["on": true])   // sem esperar a resposta
```

## Tópicos (Rust → Swift)

`native::publish<T: Serialize>(topic: &str, value: &T)` serializa com serde, compara com o último JSON mandado naquele tópico e só manda
quando mudou (pode chamar a cada quadro). Só na thread principal, de dentro do quadro.

Tópicos de agora:

| Tópico | Conteúdo |
|---|---|
| `areas` | as áreas ligadas, `["probe"]`; o Swift chama o `<Nome>Area.start()` de cada uma (`Bridge/Areas.swift`) |
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

1. **Rust** (um arquivo na pasta da área, atrás de `#[cfg(target_os = "macos")]`): uma struct `#[derive(Serialize)]`
   com o que a tela mostra e `impl DrawingApp { pub(crate) fn native_island(&mut self) }` que a publica:
   ```rust
   pub(crate) fn native_island(&mut self) {
       crate::app::native::publish("island", &IslandTopic { agents: ..., pending: ... });
   }
   pub(crate) fn native_island_act(&mut self, verb: &str, args: &Value) -> Result<String, String> { ... }
   ```
   O `native_tick` chama `native_<area>()` a cada quadro quando `enabled("<area>")` (uma linha por área no ponto
   marcado `AREAS`), e o egui pula o desenho dela quando `enabled` é verdadeiro.
2. **Swift** (`macos/MagoUI/Sources/MagoUI/Island/`): a struct `Decodable` com os mesmos campos, as vistas e
   `@MainActor public enum IslandArea { public static func start() }` (nomes: `IslandArea`, `ChatArea`, `CanvasArea`,
   `TerminalArea`, `ShellArea`, `SettingsArea`, `FileTreeArea`); um caso a mais no `switch` de `Bridge/Areas.swift`
   (`case "island": IslandArea.start()`). A ordem de começar é fixa ali: chat, terminal, filetree, settings e canvas
   (que registram vistas em `MagoViews`) antes de island e shell.
3. **Ações**: o que a pessoa faz vai por `MagoStore.shared.act(nome, args)` (ou `send`, sem esperar) com as ações do
   MCP. O que só existe para a janela (posição de um painel, um hover) é um `ui.<area>.<verbo>`, com teste no Rust.
4. **Teste**: a struct do tópico e as ações novas ganham teste no Rust (`cargo test --release`).

`MagoWindow.make(title:frame:borderless:transparent:level:content:) -> NSWindow` cria a janela com um `NSHostingView` e
a põe na frente sem ativar o app nem roubar o foco. `borderless: true` é um `NSPanel` sem barra que não ativa o app,
não some quando ele perde o foco e aceita o teclado; `false` é uma janela comum com barra. As janelas feitas por ele
fecham no `mago_ui_stop`.

`MagoViews` guarda as vistas que uma área oferece a outra sem dependência ao compilar: `canvas`, `chats`, `settings`
(`() -> AnyView`) e `chat`, `terminal`, `filetree` (`(id) -> AnyView`). A área registra no `start()`; quem embute lê e
mostra outra coisa enquanto é nil.

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

Abre uma janela "Sonda SwiftUI" com a versão e o número de agentes (tópico `app`) e manda um `ui.ping` sozinho ao abrir
(e no botão Ping); `pong` na tela prova a ida e a volta. O `shot` do roteiro só pega o que o egui desenha: a janela
SwiftUI não sai na captura.
