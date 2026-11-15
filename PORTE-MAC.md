# PORTE-MAC: levar o MagoAgent para o macOS (Rust + objc2)

Prompt para o agente que vai fazer o porte. Leia também `AGENTS.md` e `CLAUDE.md` na raiz. Os `arquivo:linha` abaixo
foram conferidos no código do ramo `porte-referencia`; se o código andou, procure o trecho pelo texto citado.

## 1. Objetivo e decisões do dono (obrigatórias)

- O app continua **100% Rust** (eframe/egui 0.37). **Nada de Swift, nada de SwiftUI**, nenhum alvo de Xcode.
- As partes nativas pequenas do Mac usam a crate **`objc2`** (com `objc2-app-kit` e `objc2-foundation`): vidro nativo
  (`NSVisualEffectView`, e `NSGlassEffectView` no macOS 26) atrás de botões e painéis; a geometria do notch
  (`NSScreen.safeAreaInsets`, `auxiliaryTopLeftArea`, `auxiliaryTopRightArea`) para a ilha encaixar no notch real;
  e o click-through da ilha.
- A ilha (`src/app/island/`) é uma **janela própria, transparente, sempre por cima, de tamanho FIXO**, com click-through
  fora da ilha. **Não redimensione a janela do sistema a cada quadro** (o código atual só refaz a janela quando o tamanho
  pedido muda: `src/app/island/window.rs:90-95`).
- O Mac já tem parte do porte feito (WKWebView, lixeira, tabela de processos, atalhos, aviso por `osascript`...). Não
  refaça: complete o que falta. A tabela da seção 4 marca com prioridade **0** o que já funciona.

## 2. Compilar e empacotar no Mac

- `cargo build --release` e `cargo test --release` (o CI faz `cargo check --release --all-targets` e `cargo test --release`,
  `.github/workflows/ci.yml:46-59`). As dependências só do Mac estão em `Cargo.toml:45-50` (`objc2` 0.6, `objc2-app-kit`
  0.3, `objc2-foundation` 0.3, `objc2-web-kit` 0.3, todas com `default-features = false`). **Para vidro, notch e
  click-through, acrescente as features que faltam** nessas linhas (cada classe é uma feature: `NSVisualEffectView`,
  `NSScreen`, `NSEvent`, `NSColor`, `NSApplication`, `NSWorkspace`...).
- `./build-macos-app.sh` (`build-macos-app.sh:1-27`) compila, monta `MagoAgent.app/Contents/MacOS/magoagent`, escreve o
  `Info.plist` e abre o app. Pontos de atenção:
  - ele copia o binário como arquivo novo (`rm -f` antes do `cp`, com o comentário "macOS refuses to launch a binary
    replaced in place"). Mantenha.
  - **falta a assinatura**: depois do `cp`, rode `codesign --force --deep --sign - MagoAgent.app` (ad-hoc). Sem isso, as
    permissões (microfone) se perdem a cada build, porque o macOS as amarra à assinatura.
  - o `Info.plist` atual só tem `CFBundleExecutable`, `CFBundleIdentifier`, nomes, `CFBundlePackageType`, versão,
    `LSMinimumSystemVersion` 12.0 e `NSHighResolutionCapable`. Acrescente `NSMicrophoneUsageDescription` (obrigatória: sem
    ela o macOS derruba o app ao pedir o microfone; texto em pt-BR) e `CFBundleIconFile` (o ícone já existe em
    `assets/icon.icns`; copiar para `Contents/Resources/`). Chaves de câmera, localização ou Apple Events **não** são
    necessárias: o app não usa.
  - não ponha `LSUIElement`: o app tem janela principal e Dock. A ilha não aparece no Dock por ser só outra janela
    (`.with_taskbar(false)`, `src/app/island/window.rs:106`).
- `installer/` só tem o MSI do Windows (`installer/build-msi.sh`, `installer/magoagent.wxs`): não se aplica ao Mac. O
  equivalente é o `.app` do `build-macos-app.sh` (`.dmg` só se o dono pedir). O instalador interno (`src/install/mod.rs`)
  já separa por sistema; no Mac a pasta padrão é `~/Applications/MagoAgent` (`src/file/mod.rs:96-103`).
- `.github/workflows/ci.yml:45-46` roda a matriz só em `windows-latest` e `ubuntu-latest`; `release.yml` só gera MSI e
  pacote Linux (`release.yml:1`, jobs em `:14` e `:34`). **Prioridade média:** acrescentar `macos-latest` à matriz do CI
  (os `apt-get` já são condicionados a `runner.os == 'Linux'`, `ci.yml:52-54`). A release do Mac fica para o dono decidir
  (a release é rascunho por decisão dele).

## 3. Permissões do macOS e o que já é multiplataforma

| Recurso | Quando precisa | O que fazer |
|---|---|---|
| Microfone | voz (`src/voice/`, `cpal`) | `NSMicrophoneUsageDescription` no `Info.plist`. O app já sabe abrir o painel certo: `mic_settings` em `src/app/voice/orb.rs:93-100` (`x-apple.systempreferences:com.apple.preference.security?Privacy_Microphone`) |
| Acessibilidade | **só se** a ilha lesse o cursor por um monitor global de eventos | `NSEvent.mouseLocation` (método de classe) **não** pede permissão: use-o e não peça Acessibilidade |
| Gravação de Tela | só se algum recurso capturasse outras janelas | a captura de página usa `WKWebView.takeSnapshot` (`src/app/browser/design.rs:186-200`), que não pede; **não** use `CGWindowListCreateImage` |
| Automação (Apple Events) | não | o aviso usa `osascript display notification` (`src/app/maestro/mod.rs:186-196`), sem pedir |

Já multiplataforma, sem trabalho: `keyring` 4 (`Cargo.toml:23`, usa o Keychain), `cpal` 0.16 (`:42`, CoreAudio), `wry`
0.57 (`:63-64`, WKWebView), PTY do terminal (`src/terminal/session.rs:62-76` já escolhe `/bin/zsh -l`), git
(`src/editor/git/mod.rs:72-78` já lista `/opt/homebrew/bin/git`), fontes de símbolos (`src/render/svg.rs:150-151`),
pasta de dados (`src/file/mod.rs:137-143`, `~/Library/Application Support/MagoAgent`), lixeira
(`src/editor/fileops.rs`, `NSFileManager.trashItemAtURL`), tabela de processos (`src/transcript/process.rs:35-45` e
`:104-105`, `kill(pid, 0)` e `ps`).

## 4. Tabela de portabilidade

Grep refeito depois do rebase (117 ocorrências): `grep -rn "cfg(windows)\|cfg!(windows)\|target_os = \"windows\"" src --include=*.rs`.
Prioridade: **A** = sem isso o recurso não funciona no Mac; **M** = funciona degradado; **B** = só conferir; **0** = já
funciona, nada a fazer.

### 4.1 A ilha e as janelas sem moldura (o núcleo do porte)

| arquivo:linha | o que faz no Windows | o que fazer no macOS | prio |
|---|---|---|---|
| `src/app/island/cursor.rs:30-41` | `GetCursorPos` (user32): cursor na tela em **pixels**, origem em cima à esquerda. Fora do Windows `screen_cursor()` devolve `None` (`:43-46`) | Implementar com `NSEvent.mouseLocation` (**origem embaixo à esquerda, em pontos**). Converta: `x_pixel = x * escala`, `y_pixel = (altura_da_tela - y) * escala`, onde a tela é a `NSScreen` que contém o ponto (não a `mainScreen`; em multi-monitor percorra `NSScreen.screens`). `eye_target` (`:17-20`) divide por `scale` e espera pixels: entregue pixels e ele não muda | A |
| `src/app/island/window.rs:120` | `let capture = !cfg!(windows) \|\| cursor::captures(...)`: só no Windows a ilha decide sozinha se pega o clique | No Mac `capture` deve vir de `cursor::captures` (com o cursor de `screen_cursor`) | A |
| `src/app/island/window.rs:130-132` | fora do Windows manda `ViewportCommand::MousePassthrough(!capture)` | No Mac, troque por `NSWindow.setIgnoresMouseEvents(!capture)` via objc2, só quando `capture` muda (`send_capture`, `:121-122`). Com a janela ignorando eventos o winit não a recebe, então o `MousePassthrough` não serve | A |
| `src/app/island/window.rs:138-146` | depois de criar a janela: `plain::reveal` (`DWMWA_CLOAK`, tira o quadro do sistema) e `plain::click_through("Ilha do Maestro", !capture)` | No Mac não há quadro a tirar: configure a `NSWindow` uma vez (seção 5). `click_through` vira o `setIgnoresMouseEvents` acima | A |
| `src/app/island/window.rs:110` | `.with_visible(!cfg!(windows) \|\| plain > 0)`: no Windows nasce escondida até o quadro sair | No Mac nasce visível (já). Se aparecer um quadro de uma imagem, nasça invisível e mostre após configurar a `NSWindow` | B |
| `src/app/island/window.rs:11` | `TOP_PAD = 1.0` no Windows (ele esconde a 1ª linha da janela sem moldura); 0 no resto | Mac: 0 (já). O notch entra na âncora, não no `TOP_PAD` (seção 5) | B |
| `src/app/island/mod.rs:433-435` | só no Windows repinta a cada 33/250 ms para ler o cursor global quando a ilha não é viewport embutido | Valer também no Mac assim que `screen_cursor()` existir lá: a janela ignorando eventos não recebe nada, então só o repaint periódico a faz reagir | A |
| `src/app/voice/popup.rs:198-400` (`mod plain`) | tudo Win32: `DwmSetWindowAttribute` (`:356-362`, sem borda/cantos/sombra), `SetWindowRgn`, `SetWindowSubclass`, `reveal` (`:379`), `work_area` (`:304`) e `click_through` (`:254-265`: a janela fica sempre *layered* com alpha 255 e só o bit `WS_EX_TRANSPARENT` muda) | Crie um `mod plain` equivalente para o Mac, com as assinaturas que os chamadores usam (`click_through(title, through)`, `work_area(x, y)`, `strip(title)`, `reveal(title, stage)`). Achar a `NSWindow`: `NSApplication.sharedApplication().windows()` com `title()`, ou o `AppKitWindowHandle` do viewport | A |
| `src/app/voice/popup.rs:150-153` | `plain::strip("Chat por voz")` na janela do chat por voz | `setHasShadow(false)` e fundo transparente, pelo mesmo `mod plain` | M |
| `src/app/chat/mini.rs:338`, `:352-354` | pílula do mini chat: `with_visible(!cfg!(windows)...)` e `plain::reveal` | mesma configuração da ilha (transparente, sem sombra, nível flutuante) | M |
| `src/app/chat/mini_window.rs:78`, `:89`, `:99` | mini chat: idem, e `area.with_min_y(top + 1.0)` porque o Windows corta a 1ª linha | no Mac sem o `+1` (já é assim) | 0 |
| `src/app/chat/mini.rs:635-642` (`screen_work`) | no Windows pergunta à API a área de trabalho do monitor; no resto usa `monitor_size` do egui, que ignora Dock e barra de menus e dá sempre o monitor da janela | no Mac: `NSScreen.visibleFrame` (desconta Dock e barra de menus; **origem embaixo à esquerda**, converta) da tela que contém o ponto. Para a ilha no notch use `frame` (tela inteira): o topo da ilha é o topo físico da tela | A |
| `src/app/maestro/avatar.rs:50`, `:193`, `:253` | a janela do avatar do Maestro também perde a borda do Windows 11 (`plain`) | `setHasShadow(false)` e fundo transparente pelo mesmo `mod plain` | M |
| `src/app/ui/titlebar.rs:145-149`, `:186` (`native_caption`) | no Windows os trechos vazios da barra são a legenda nativa (mover e maximizar sem esperar quadro) | no Mac os botões já ficam na barra (`Cargo.toml:44`, `LIGHTS_BOTTOM` em `src/app/chat/list/mod.rs:18`); confira `caption_drag` (`titlebar.rs:159`) mover a janela; se falhar, `performWindowDragWithEvent` | M |
| `src/app/canvas/paint.rs:496` | no Windows a mão é sempre "apontando" | no Mac o `Grabbing` do winit funciona: já está certo | 0 |

### 4.2 Navegador (wry) e capturas

O Mac usa WKWebView; várias travas "só Windows" existem por causa do WebView2.

| arquivo:linha | o que faz no Windows | o que fazer no macOS | prio |
|---|---|---|---|
| `src/app/browser/shot.rs:22-24`, `:97-99`, `:116-117` | `PrintWindow`/WebView2 devolvem a imagem da página (para a ferramenta MCP e "Copiar Captura de Tela") | o próprio `shot.rs:114` diz: "o Mac tem `browser::design::snapshot`, que responde depois; ligue quando chegar a vez". Ligar `page_rgba` ao `WKWebView.takeSnapshot` (`design.rs:186-200`), que é assíncrono: devolva o resultado por canal no quadro seguinte | A |
| `src/app/browser/extras.rs:255-257` (`portal_copy_shot`) | recusa fora do Windows ("só existe no Windows por enquanto") | depois do `page_rgba` do Mac, remova a recusa | A (depende da linha acima) |
| `src/app/ui/context/entries.rs:175` | o item "Copiar Captura de Tela" só habilitado no Windows | habilitar no Mac junto | A (idem) |
| `src/app/browser/extras.rs:171-176` | User-Agent do Chrome no lugar do `Edg/` do WebView2 | não precisa: o WKWebView manda o agente do Safari (`DEFAULT_AGENT`, `:95`, "Padrão (WebKit)") | 0 |
| `src/app/browser/paint.rs:100` | dicas (tooltips) só no Windows: fora dele um "buraco" esconde a página inteira | manter; reavalie com o vidro (seção 6) | B |
| `src/app/browser/sync.rs:166`, `:214`; `src/app/browser/types.rs:216-226` (`cut_page`), `:258-260` (`PAGE_SETTLE`) | recorta a página com regiões (`SetWindowRgn`) para menus do egui passarem por cima; espera 0,15 s depois de o cartão mover | o WKWebView é uma `NSView` filha: não há região. Fora do Windows um buraco esconde a página toda; aceitar, ou `setHidden` nos buracos. Confira pelo roteiro | M |
| `src/app/browser/types.rs:105` (`open_external`) | abre direto pelo shell | no Mac já cai no ramo `open` (`types.rs:121`) | 0 |
| `src/app/browser/types.rs:144` (`polls_design`) | o Design lê as mensagens da página a cada quadro, só no Windows | no Mac o Design usa `design.rs` com WKWebView; confira se `polls_design` precisa ser `true` também | M |
| `src/app/voice/page.rs:90` | URL `http://magovoz.localhost/` no Windows | `magovoz://localhost/` (protocolo próprio) já é o ramo do Mac | 0 |

### 4.3 Processos, terminal, arquivos e caminhos

| arquivo:linha | o que faz no Windows | o que fazer no macOS | prio |
|---|---|---|---|
| `src/editor/mod.rs:48-53` (`command`) | `CREATE_NO_WINDOW` para nenhum processo abrir console | no Mac nada abre console | 0 |
| `src/editor/fileops.rs:29` (`trash`) | `SHFileOperation` | o Mac já tem ramo próprio (`NSFileManager.trashItemAtURL`) | 0 |
| `src/transcript/process.rs:13`, `:55` | `OpenProcess` e snapshot de processos | o Mac já tem ramo (`kill(pid, 0)` em `:35-45`; `ps -A -o pid=,ppid=,comm=` em `:104`) | 0 |
| `src/app/system/memory.rs:110`, `:151-154` | `GetProcessMemoryInfo`; no Mac `process_memory` devolve `None` (`ponytail` no código) | `proc_pidinfo(PROC_PIDTASKINFO)` ou `ps -o rss= -p PID` | B |
| `src/app/terminal/procs.rs:60` | `taskkill` da árvore, sem janela | o ramo `not(windows)` está em `:70`; confira se mata os filhos | B |
| `src/app/chat/direct/mod.rs:90`; `src/app/usage/probe.rs:355` | `taskkill /T /F` porque o `kill` só mata o invólucro (`.cmd` do npm) | no Mac o `kill` basta | 0 |
| `src/app/terminal/mod.rs:158` (`edit_key_held`) | `GetKeyState` distingue Ctrl+C de copiar | no Mac copiar é ⌘C; `src/app/terminal/overlay.rs:210-226` já ramifica por `target_os = "macos"` | 0 |
| `src/terminal/session.rs:67-68`, `:83`; `src/terminal/handle.rs:167` | shell padrão (pwsh), pid do ConPTY e ^L extra por causa do ConPTY | o Mac tem ramo (`/bin/zsh -l`, `:62-76`, e `shell_pid` com `cfg(unix)`, `:85`) | 0 |
| `src/terminal/select.rs:174-175`; `src/app/terminal/actions.rs:280-281`; `find.rs:203`; `menu.rs:248`, `:366`; `procs.rs:293` | `cmd /c` no Windows, `sh -c` no resto | já cai no `sh` | 0 |
| `src/app/terminal/prefs.rs:11` (`detected_shells`) | lista pwsh, PowerShell, cmd, Git Bash | confira o ramo do Mac: `/bin/zsh`, `/bin/bash` e `$SHELL` | M |
| `src/app/chat/viewer.rs:347` | `FileTimeToSystemTime` para a hora local | confira o ramo `not(windows)` do mesmo trecho (fuso local) | B |
| `src/editor/lsp/rpc.rs:41`, `:73`, `:79`; `src/editor/lsp/mod.rs:276`, `:281`; `src/mcp/bridge/lookup.rs:85`; `src/app/ui/folders.rs:18`, `:132`, `:177`, `:212` | sufixos `.exe/.cmd`, `C:` e comparação sem caixa, porque o Windows ignora maiúsculas | **decisão**: o APFS padrão do Mac também ignora maiúsculas, mas o código usa `cfg!(windows)`. Compare sem caixa também no Mac (`cfg!(any(windows, target_os = "macos"))`) só onde fizer sentido; os testes `src/mcp/bridge/tests.rs:169-176` fixam o comportamento atual | M |
| `src/file/mod.rs:86-88`, `:97-99`, `:106-108`, `:137` | registro do Windows guarda a pasta da instalação; `ProgramFiles` | no Mac já: `~/Applications/MagoAgent` e `~/Library/Application Support/MagoAgent`; sem registro (o `ponytail` de `:84` diz isso) | 0 |
| `src/file/mod.rs:127` (`projects_dir`); `src/editor/theme.rs:139`; `src/model/scene.rs:213`; `src/app/maestro/claude_hooks.rs:29` | `USERPROFILE` no Windows | já usam `HOME` | 0 |
| `src/app/maestro/claude_hooks.rs:27-31`, `:34-37` | `settings.json` do Claude Code em `~/.claude` (ou `CLAUDE_CONFIG_DIR`); o comando do hook troca `\` por `/` e põe o exe entre aspas | no Mac é o mesmo `~/.claude/settings.json`; o exe vira `.../MagoAgent.app/Contents/MacOS/magoagent`. Teste com o app dentro de `~/Applications` | B |
| `src/mcp/bridge/types.rs:248-251` | `bridge.json` em `legacy_data_dir()` (`%APPDATA%\MagoAgent`) | no Mac: `~/Library/Application Support/MagoAgent/bridge.json`. Lido por `src/app/maestro/outside.rs:171-174` (`endpoint_from`) e por `src/main.rs:231`. Confira que `magoagent hook external` o acha | B |
| `src/app/chat/tools.rs:43`, `:128`, `:188` | registra o MCP `windows-mcp`, fala dele no prompt e achata o prompt em uma linha | no Mac fica de fora, e está certo. Um equivalente de controle do Mac seria recurso novo: não faça | 0 |
| `src/app/system/updates.rs:143-145`; `src/app/update.rs:212`; `src/app/ui/settings/about.rs:27` | atualização baixa e roda o MSI; no resto só abre a página da release | no Mac abre a página (OK). Atualização automática do `.app` fica para depois | B |
| `src/main.rs:4`, `:164-190` (`attach_console`) | religa o console no Windows | não precisa | 0 |
| `src/voice/microphone.rs:4` | acrescenta `.exe` ao nome do programa | sem efeito | 0 |
| `src/install/mod.rs:164`, `:256`, `:287`, `:331`, `:359`, `:369`, `:410` | instalador: elevar como administrador, `explorer`, `tasklist`/`taskkill`, atalho `.lnk` no Menu Iniciar | os ramos `else` rodam no Mac; confira `shortcut` (`:408`): no Mac pode ficar sem atalho ou criar um alias | M |
| `src/app/system/tray.rs:14`, `:21` | bandeja do sistema (`tray_icon`), só no Windows | seria o ícone da barra de menus; **opcional**, não faça sem o dono pedir | B |
| `src/render/svg.rs:152` | fontes de símbolos do Windows | o Mac tem as suas (`:150-151`) | 0 |

### 4.4 Testes com ramos por sistema

`src/app/tests/session.rs:3`, `:39`, `:109`; `src/app/tests/terminals.rs:12`, `:64`; `src/tests/shells.rs:42`, `:57`;
`src/app/tests/chat.rs:241-246`; `src/app/tests/gestures.rs:397`; `src/tests/transcripts.rs:71`;
`src/app/floors/mod.rs:347`; `src/app/workspaces/mod.rs:489`; `src/app/maestro/demands.rs:434`, `:446`, `:511`;
`src/app/chat/segments.rs:152`, `:294`; `src/mcp/bridge/tests.rs:169`, `:176`. Rode `cargo test --release` no Mac e **conserte
o que quebrar** (suspeitos: o caso de maiúsculas do APFS e o que supõe `sh` sem `-l`). O que é só do Windows fica como
está; os que escolhem programa (`powershell` ou `sh`) já têm o ramo do Mac.

## 5. A ilha no Mac em detalhe

A janela é um viewport do eframe criado por `show_viewport_immediate` (`src/app/island/window.rs:101-127`), com título
"Ilha do Maestro", `with_transparent`, `with_decorations(false)`, `with_always_on_top`, `with_taskbar(false)`,
`with_resizable(false)`. Falta configurar a `NSWindow` **uma vez**, depois que ela existe. Tudo na thread principal
(`objc2::MainThreadMarker::new()`, como em `design.rs:191`).

1. **Transparência:** `setOpaque(false)`, `setBackgroundColor(NSColor.clearColor())`, `setHasShadow(false)`.
2. **Acima da barra de menus:** `setLevel(NSMainMenuWindowLevel + 1)` (ou `NSStatusWindowLevel`, 25). O nível flutuante
   comum (3) fica **abaixo** da barra de menus, justamente onde o notch está.
3. **Todos os Spaces e tela cheia:** `setCollectionBehavior(canJoinAllSpaces | fullScreenAuxiliary | stationary | ignoresCycle)`.
   `fullScreenAuxiliary` a deixa aparecer sobre apps em tela cheia; sem `canJoinAllSpaces` ela fica presa ao Space atual.
   Não mude a `NSApplicationActivationPolicy` do app (tiraria o Dock da janela principal).
4. **Click-through:** `setIgnoresMouseEvents(bool)`. A janela tem tamanho fixo e é maior que a ilha (`ROOM` 720x420,
   `window.rs:13`), então fora da ilha o clique tem que atravessar para o app de baixo.
   - A cada quadro: `NSEvent.mouseLocation` -> coordenadas da janela (pontos, topo à esquerda) -> `cursor::captures`
     (`cursor.rs:26-28`: a ilha mais `HIT_MARGIN` 14, ou um clique segurado, `holding`) -> só se `capture` mudou, chame
     `setIgnoresMouseEvents(!capture)`.
   - Com `setIgnoresMouseEvents(true)` a janela deixa de receber mouse e o egui não vê o cursor entrar: por isso o cursor
     vem de `NSEvent.mouseLocation` e não do egui, e a janela repinta mesmo parada (`island/mod.rs:433`).
   - **Origem embaixo à esquerda:** `y_topo = altura_da_tela - y`, e a "tela" é a que contém o cursor.
   - Enquanto o botão está apertado dentro da ilha (`holding`) mantenha a captura mesmo que o cursor saia (arrastar a ilha).
5. **Notch real:** numa `NSScreen` com `safeAreaInsets().top > 0`, o notch ocupa o meio do topo; a faixa livre à esquerda é
   `auxiliaryTopLeftArea()` e à direita `auxiliaryTopRightArea()` (retângulos em coordenadas da tela, origem embaixo).
   Largura do notch = `tela.width - esquerda.width - direita.width`; centro = `esquerda.max_x` mais metade da largura.
   Use isso em `island_anchor` (`window.rs:62-77`): em vez de `[work.center().x, work.top()]` (`:72`), ancore em
   `[centro_do_notch_x, topo_da_tela]` e dê à ilha, fechada, uma largura mínima maior que o notch para ela "sair" dele.
   Sem notch (`safeAreaInsets().top == 0`, monitor externo) mantenha o comportamento atual (centro e topo da tela).
6. **Multi-monitor:** a posição salva (`prefs.island_at`, `window.rs:67-68`) passa por `keep_on_screen` com a área da tela
   onde está, e `screen_work` (seção 4.1) tem que devolver a tela certa do ponto, não a principal. O encaixe no topo
   (`SNAP` 60, `window.rs:9`) vale por tela. Se a tela some, `keep_on_screen` traz a ilha de volta.
7. **Mudança de tela/resolução:** escute `NSApplicationDidChangeScreenParametersNotification` e zere `island.anchor`
   (`window.rs:63-65`) para recalcular.
8. **Retina:** `native_pixels_per_point` do egui já é o `backingScaleFactor`; `eye_target` (`cursor.rs:17-20`) espera o
   cursor em pixels e divide pela escala: com `NSEvent.mouseLocation` (pontos) multiplique antes de entregar.

## 6. Vidro nativo

Onde: atrás dos botões e painéis que hoje desenham um "vidro" falso com cor translúcida (barra superior, painéis do mini
chat, pílulas da ilha). O egui desenha tudo numa única view OpenGL (`glow`), então o vidro tem que estar **atrás** dela:

1. Janela transparente (`setOpaque(false)`, fundo `clearColor`; as janelas `with_transparent(true)` já limpam com alpha 0).
2. Criar `NSVisualEffectView` (material `.hudWindow` ou `.underWindowBackground`, `blendingMode = .behindWindow`,
   `state = .active`) do tamanho da janela e inseri-lo **abaixo** da view do egui:
   `contentView.addSubview_positioned_relativeTo(vidro, NSWindowBelow, Some(&view_do_egui))`. A ordem importa: se o vidro
   ficar acima, cobre o egui e some tudo. `autoresizingMask` de largura e altura.
3. No macOS 26, se a classe existir (`NSClassFromString("NSGlassEffectView")` não nula), use-a no lugar, com o mesmo
   posicionamento. **Detecte em tempo de execução**, nunca por versão fixa: o `Info.plist` declara mínimo 12.0.
4. O vidro só aparece onde o egui pinta transparente. Para formas não retangulares (a pílula da ilha) use
   `layer.cornerRadius` com `masksToBounds`, com o mesmo raio que a ilha desenha (`self.island.radius`, `window.rs:125`).
5. Cuidado com o navegador: a `NSView` do WKWebView é filha da mesma janela (`browser/types.rs:207-208`); o vidro não pode
   ficar acima dela. Teste um cartão de navegador sobre o vidro.
6. Crie o vidro uma vez e só mude o frame quando o tamanho mudar. Respeite "Reduzir transparência"
   (`NSWorkspace.accessibilityDisplayShouldReduceTransparency`): então não crie o vidro.

Faça o vidro numa função pequena (`mod glass`, arquivo próprio, `cfg(target_os = "macos")`) com um stub vazio nos outros
sistemas, para o resto do código não ter `cfg`.

## 7. Testes

- `cargo test --release` no Mac passa, **sem avisos**. O CI não cobre o Mac hoje (seção 2): acrescente `macos-latest`.
- `cargo check --release --target x86_64-unknown-linux-gnu` e o do Windows continuam passando (você mexe em arquivos
  compartilhados; todo código novo do Mac fica sob `cfg(target_os = "macos")`).
- Toda lógica nova deixa um teste. Coisas puras testam em qualquer sistema: a conversão de coordenadas (origem embaixo ->
  topo), a âncora do notch a partir de três retângulos, e `captures` (já testada em `cursor.rs`, módulo `tests`). Deixe a
  parte AppKit fina e a matemática em funções puras.
- Roteiro: `MAGOAGENT_SCENE=/tmp/teste.mago MAGOAGENT_SCRIPT="size 1440 900; wait 2000; shot /tmp/a.png; quit" target/release/magoagent`
  (`src/app/system/script.rs`; no Mac o `shot` também salva as páginas nativas ao lado, `script.rs:22-24`). Sempre numa
  cena de teste, nunca no desenho do dono.
- Conferir à mão (para o item 3, a máquina precisa ter notch):
  1. a ilha aparece transparente, sem sombra nem quadro;
  2. o clique **atravessa** fora da ilha e **pega** em cima dela (teste com um app aberto atrás);
  3. com notch: a ilha encaixa nele; sem notch (monitor externo): no meio do topo;
  4. continua visível ao trocar de Space e sobre um app em tela cheia, e **não** rouba o foco (`with_active(false)`);
  5. arrastar a ilha entre dois monitores; desligar um monitor;
  6. Retina e não-Retina;
  7. microfone: o macOS pede permissão e o painel certo abre se negada;
  8. o hook do Claude Code (`magoagent hook external`) chega ao app (`bridge.json`);
  9. vidro: claro e escuro, e com "Reduzir transparência" ligado.
- O que depende do cursor global (item 2) é manual; o roteiro não move o mouse real.

## 8. Regras do projeto (do `AGENTS.md`)

- **pt-BR** em tudo que o usuário vê (interface, mensagens do MCP, perguntas, resumos) e no relatório final.
- **Nomes por extenso** (`index`, `response`, `colors`): nada de variáveis de uma letra fora de fechamentos curtos.
- Formate o que mexer com `rustfmt --edition 2024` (`rustfmt.toml`: até 160 colunas).
- Atalho deliberado com limite conhecido leva `// ponytail: <limite>, <como evoluir>`.
- Arquivo passando de ~600 linhas: divida por assunto (o `mod plain` e o `mod glass` do Mac vão em arquivos próprios).
- **Commits sem linha de coautor.** Não versionar `target/`, `MagoAgent.app` nem desenhos (`.mago`, `.rea`).
- Nada abre janela de console; processos saem por `crate::editor::command`.
- Toda ação nova do app tem a sua ferramenta no MCP (`mcp/calls/tools.rs`, `mcp/calls/mod.rs`, `mcp/actions/`,
  `mcp/app_tools.rs`); este porte não cria ações novas, então em princípio não se aplica.
- Fluxo: implementar com o menor diff, testar, ver no app pelo roteiro, e **deixar para o dono** `cargo run --release` no
  target padrão. Relatar em pt-BR o que mudou, o que foi testado e o que não foi.
