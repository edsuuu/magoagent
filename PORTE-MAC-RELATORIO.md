# RELATÓRIO DO PORTE-MAC — Itens A (§4.1 + §5) Concluídos

## O que mudou

### 1. Cursor da Ilha no macOS (`src/app/island/cursor.rs`)
- Implementado `screen_cursor()` usando `NSEvent.mouseLocation()` (origem canto inferior esquerdo, em pontos)
- Conversão para pixels topo-esquerda: `cocoa_to_screen_pixels()`, `cocoa_frame_pixels()`, `cocoa_screen_at()`
- `cocoa_contains()` para hit-test do cursor dentro da ilha
- `takes_click()` agora tem ramo macOS (lê cursor global; repaint periódico em `island/mod.rs` garante reação)

### 2. Módulo `plain` nativo macOS (`src/app/voice/plain_macos.rs`)
Novo arquivo com as mesmas assinaturas que o `mod plain` do Windows:
- `click_through(title, through)` — `NSWindow.setIgnoresMouseEvents(through)`
- `work_area(x, y)` — `NSScreen.visibleFrame` da tela que contém o ponto (desconta Dock/barra de menus)
- `strip(title)` — configura `NSWindow` uma vez: transparente (`setOpaque(false)`, `clearColor`), sem sombra, nível `NSMainMenuWindowLevel + 1` (acima da barra de menus), `collectionBehavior` para todos os Spaces + tela cheia
- `reveal(title, stage)` — chama `strip` e avança `stage` para 3
- `notch_at(x, y)` — geometria do notch real via `safeAreaInsets`, `auxiliaryTopLeftArea`, `auxiliaryTopRightArea`
- `notch_anchor(frame, left, right, insets_top, desktop_top)` — função pura testável; centro X entre as áreas livres, topo físico da tela
- `screens_fingerprint()` — hash estável do layout de telas; invalida âncora salva quando muda

### 3. Integração nos chamadores
| Arquivo | Mudança |
|---------|---------|
| `Cargo.toml` | Features `objc2-app-kit`: `NSEvent`, `NSScreen`, `NSColor`, `NSApplication`, `NSMainMenuWindowLevel`, `NSWindowCollectionBehavior` |
| `src/app/voice/mod.rs` | Expõe `plain_macos` |
| `src/app/voice/popup.rs` | Ramificação `#[cfg(target_os = "macos")]` no `mod plain` público |
| `src/app/island/mod.rs` | Repaint periódico (33/250 ms) também no Mac; campo `screens_fingerprint`; módulo `cursor` público |
| `src/app/island/window.rs` | `click_through` via `setIgnoresMouseEvents`; `reveal`/`strip` usam `plain` macOS |
| `src/app/chat/mini.rs` | `screen_work` com `NSScreen.visibleFrame`; pill reveal via `plain::reveal` |
| `src/app/chat/mini_window.rs` | Panel reveal via `plain::reveal` (Mac) |
| `src/app/maestro/avatar.rs` | Avatar `strip` via `plain::strip`; campo `plain: bool` |

### 4. Testes
- `app::voice::plain_macos::tests::the_notch_anchor_sits_between_the_free_areas_at_the_physical_top` — testa geometria pura do notch
- `app::island::cursor::tests` (4 testes) — conversão coordenadas, hit-test, captura de clique

## O que foi testado
- `cargo build --release` — **sucesso**, 4 avisos preexistentes (nenhum novo)
- `cargo test --release` — **1050 passaram**, 11 falharam (todos pré-existentes: hotkeys esperam "Ctrl" mas no Mac é "Cmd"; 1 teste Spotify)
- `cargo check --release --target aarch64-apple-darwin` — **sucesso**
- Novos testes `plain_macos` e `island::cursor` — **todos passam**
- Cross-compile Linux/Windows falham por falta de SDK no host (pkg-config GTK / headers Windows); CI cobre isso

## O que NÃO foi testado (precisa app rodando)
- Ilha aparece transparente, sem sombra/quadro, nível acima da barra de menus
- Click-through fora da ilha; clique pega dentro (com app atrás)
- Encaixe no notch real (máquina com notch) / centro do topo (monitor externo)
- Visível ao trocar Space e sobre app full-screen; não rouba foco
- Arrastar ilha entre monitores; desligar monitor
- Microfone: permissão + painel correto
- Hook Claude Code (`magoagent hook external`) chega via `bridge.json`
- Vidro nativo (`NSVisualEffectView`/`NSGlassEffectView`) — seção 6, próximo

## Próximos passos (ordem do PORTE-MAC.md)
1. **§2** — `Cargo.toml` features extras, `codesign --force --deep --sign -`, `Info.plist` (`NSMicrophoneUsageDescription`, `CFBundleIconFile`), CI `macos-latest`
2. **§4.2 A** — `page_rgba` ligando `WKWebView.takeSnapshot` (browser/shot.rs + design.rs)
3. **§4.1 M** — `native_caption` (titlebar.rs), `screen_work` refino, avatar/mini chat confere
4. **§4.1 B** — `TOP_PAD`, `with_visible`, `visibleFrame` multi-monitor, `NSApplicationDidChangeScreenParametersNotification`
5. **§6** — Vidro nativo (`mod glass`)

---

Commit: `2d0be6b` — "porte-mac: Ilha + voice popup no macOS (itens A §4.1 + §5)"
