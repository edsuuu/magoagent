# MagoAgent — guia para agentes

Quadro branco nativo em Rust (eframe/egui 0.37, edition 2024) com servidor MCP embutido.
Tudo voltado ao usuário (textos da interface, mensagens do MCP, perguntas e resumos) é em pt-BR.

## Estrutura

- `src/main.rs` — o app inteiro: modelo (`Element`, `ElementKind`, `Style`, `Scene`), desenho
  estilo rough.js, gestos, ferramentas, interface (barra, painel, menus, minimapa), arquivos
  (`.rea`, interop `.excalidraw`, PNG/SVG via resvg), sincronização com o MCP e os testes
  (`mod interaction_tests` no fim).
- `src/mcp.rs` — servidor MCP stdio (JSON-RPC): ferramentas, `INSTRUCTIONS` e testes próprios.
- `assets/` — fontes (mão = `MagoHand`, interface = Assistant) e licenças OFL.
- `build-macos-app.sh` — gera `MagoAgent.app` (bundle `com.local.magoagent`) e abre.

## Fluxo de trabalho

1. **Paridade com o Excalidraw.** Antes de implementar ou mudar um comportamento, leia o código
   de referência (clone em `~/Desktop/excalidraw/packages`, app web em `http://127.0.0.1:3001/`)
   e copie valores e regras de lá (constantes, cores, tamanhos, textos do `locales/pt-BR.json`).
   Nunca use "Excalidraw" em nomes visíveis; o formato de arquivo interop continua compatível.
2. **Implementar** com o menor diff que resolve, no estilo do arquivo. Atalho deliberado com
   limite conhecido leva um comentário `// ponytail: <limite>, <como evoluir>`.
3. **Testar:** `cargo test` deve passar. Toda lógica nova deixa um teste em
   `interaction_tests` (os gestos usam os helpers `frame` e `drag`) ou nos testes de `mcp.rs`.
4. **Gerar o app:**
   `pkill -f "MagoAgent.app/Contents/MacOS/magoagent$"; ./build-macos-app.sh`
5. **Testar no app** (computer use, bundle `com.local.magoagent`). O app abre o desenho real
   do usuário: antes, copie
   `~/Library/Application Support/MagoAgent/autosave.rea` para um backup, ou rode uma
   instância numa cena separada:
   `MAGOAGENT_SCENE=/tmp/teste.rea ./MagoAgent.app/Contents/MacOS/magoagent &`
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

Commits sem linha de coautor. Não versionar `target/`, `MagoAgent.app` nem arquivos `.rea`.
