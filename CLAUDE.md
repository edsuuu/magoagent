# MagoAgent — guia para agentes

Quadro branco nativo em Rust (eframe/egui 0.37, edition 2024) com servidor MCP embutido.
Alvo: macOS, Windows e Linux. Nada específico de um sistema sem o equivalente dos outros
(`cfg!(target_os = ...)`); veja `file/storage.rs::data_dir`, `mcp/output.rs::show_app` e
`render/svg.rs::SYMBOL_FONTS`.
Tudo voltado ao usuário (textos da interface, mensagens do MCP, perguntas e resumos) é em pt-BR.

## Estrutura

```
src/
  main.rs          entrada (app ou `magoagent mcp`) e o `prelude` que todo módulo importa
  model.rs         Element, ElementKind, Style, Scene, Prefs, cores
  geometry.rs      limites, contornos, distâncias, hit-test
  binding.rs       setas presas às formas (orbit / inside)
  labels.rs        rótulos centrados, máscara de rótulo de seta, contorno de texto
  recognize.rs     desenhar para criar forma
  tests.rs         formatos de arquivo, exportação, paleta
  render/          pintura no canvas: rough (traço à mão), sticky, images, replay (IA
                   desenhando ao vivo), svg (exportação SVG/PNG)
  file/            interop (.excalidraw) e storage (autosave, preferências, merge)
  app/             DrawingApp: mod (estado + loop por frame), tool, files, editing,
                   shortcuts, selection, pointer, paint, tests (gestos)
  app/ui/          interface flutuante: mod (widgets, fontes), icons, toolbar, menus,
                   palette, properties
  mcp/             servidor MCP: mod (JSON-RPC, INSTRUCTIONS), calls (ferramentas),
                   elements (add/update/rótulos), output (view, save_file, show_app), tests
assets/            fontes (mão = MagoHand, interface = Assistant) e licenças OFL
build-macos-app.sh gera MagoAgent.app (bundle com.local.magoagent) e abre
```

- Cada arquivo começa com `use crate::prelude::*;`. Item compartilhado é `pub(crate)` e entra
  no `prelude` pelo módulo dele; os métodos de `DrawingApp` ficam espalhados em `app/` em
  blocos `impl DrawingApp` por assunto.
- Arquivo que passar de ~600 linhas: divida por assunto antes de crescer mais.
- Ferramenta nova do MCP: definição e despacho em `mcp/calls.rs`, lógica em `elements.rs` ou
  `output.rs`.

## Fluxo de trabalho

1. **Paridade com o Excalidraw.** Antes de implementar ou mudar um comportamento, leia o código
   de referência (clone em `~/Desktop/excalidraw/packages`, app web em `http://127.0.0.1:3001/`)
   e copie valores e regras de lá (constantes, cores, tamanhos, textos do `locales/pt-BR.json`).
   Nunca use "Excalidraw" em nomes visíveis; o formato de arquivo interop continua compatível.
2. **Implementar** com o menor diff que resolve, no estilo do arquivo. Atalho deliberado com
   limite conhecido leva um comentário `// ponytail: <limite>, <como evoluir>`.
3. **Testar:** `cargo test` deve passar, e a compilação dos outros sistemas também:
   `cargo check --release --target x86_64-pc-windows-msvc` e
   `cargo check --release --target x86_64-unknown-linux-gnu`. Toda lógica nova deixa um teste em
   `interaction_tests` (os gestos usam os helpers `frame` e `drag`) ou nos testes de `mcp.rs`.
4. **Gerar o app:**
   `pkill -f "MagoAgent.app/Contents/MacOS/magoagent$"; ./build-macos-app.sh`
5. **Testar no app** (computer use, bundle `com.local.magoagent`). O app abre o desenho real
   do usuário: antes, copie
   `~/Library/Application Support/MagoAgent/autosave.mago` para um backup, ou rode uma
   instância numa cena separada:
   `MAGOAGENT_SCENE=/tmp/teste.mago ./MagoAgent.app/Contents/MacOS/magoagent &`
   No canvas egui, cliques precisam ir como arrasto real (`app_drag` com o mesmo ponto); o
   `app_click` só aciona botões. Diálogos de arquivo (rfd) são de outro processo e não podem
   ser controlados.
6. **Relatar** em pt-BR: o que mudou, o que foi testado e o que não foi.

## MCP

- O app e o servidor só conversam pelo arquivo da cena (escrita atômica: grava e renomeia).
  O app relê a cada 0,4 s e mescla em 3 vias (`merge_scenes`); elementos novos vindos da IA
  são desenhados traço a traço com o nome do agente (`Scene.agent`, vindo do `clientInfo`).
- Para testar o MCP sem cliente de IA, mande JSON-RPC pelo stdin de `magoagent mcp`
  (`initialize` com `clientInfo.name`, depois `tools/call`).
- O caminho registrado no Claude Code (`~/.claude.json`) e no Codex (`~/.codex/config.toml`)
  aponta para `~/projects/magoagent/MagoAgent.app/Contents/MacOS/magoagent`; se a pasta mudar,
  atualize os dois.

## Git

Commits sem linha de coautor. Não versionar `target/`, `MagoAgent.app` nem desenhos (`.mago`, `.rea`).
