# MagoAgent — guia para agentes

Quadro branco nativo em Rust (eframe/egui 0.37, edition 2024) com servidor MCP embutido.
Alvo: macOS, Windows e Linux. Nada específico de um sistema sem o equivalente dos outros
(`cfg!(target_os = ...)`); veja `file.rs::data_dir`, `mcp/calls.rs::show_app` e
`render/svg.rs::SYMBOL_FONTS`.
Tudo voltado ao usuário (textos da interface, mensagens do MCP, perguntas e resumos) é em pt-BR.

## Estrutura

```
src/
  main.rs          entrada (app, `magoagent mcp` ou a CLI dos agentes), o `prelude` que todo
                   módulo importa e, no macOS, os botões da janela na linha da barra
  terminal.rs      sessão de terminal (pty + emulação do alacritty_terminal), teclas, cores,
                   pintura da grade
  bridge.rs        ponte dos agentes: servidor local com token, `magoagent list|ask|check|note|
                   portal|recruit|connect|notify` e o cliente que o MCP usa
  transcript.rs    a conversa do Claude Code de um terminal, lida dos arquivos dele (chat)
  ide/             o editor do nó IDE (port do Ilha, a IDE do usuário): mod (estado `Ide`, abas,
                   comandos, atalhos, background), chrome (painéis, barra de cima, menus,
                   problemas), tree, editor, text (posições, declaração, comentar/duplicar),
                   highlight + langs, git + commit (janela de commit e diff), search (pesquisar
                   em tudo) + find (localizar nos arquivos), index, lsp, claude (protocolo de IDE
                   do Claude Code), icons (SVGs JetBrains via resvg), theme (Islands Dark local),
                   db (painel Banco de dados e as abas de tabela/console: SQLite do projeto
                   pelo `sqlite3` com `-safe`; leitura com `-readonly`, o Submit da tabela e
                   o console escrevem numa transação)
  rope.rs          cordas das conexões (corrente de Verlet com gravidade)
  model.rs         Element, ElementKind, Style, Scene, Prefs, cores
  geometry.rs      limites, contornos, distâncias, hit-test
  binding.rs       setas presas às formas (orbit / inside)
  labels.rs        rótulos centrados, máscara de rótulo de seta, contorno de texto
  recognize.rs     desenhar para criar forma
  tests.rs         formatos de arquivo, exportação, paleta
  file.rs          JSON dos elementos (área de transferência e MCP), autosave, preferências,
                   merge
  render/          pintura no canvas: mod (elementos, imagens, notas adesivas), rough (traço
                   à mão), replay (IA desenhando ao vivo), svg (exportação SVG/PNG)
  app/             DrawingApp: mod (estado + loop por frame), tool, files, editing,
                   shortcuts, selection, pointer, arrows (setas presas), paint (cena, prévia
                   e cursor), terminal (terminais, conexões, foco, modo ampliado), portal
                   (navegador com abas sobre o canvas, via wry), ide (nó IDE: diálogo, card,
                   editor ampliado, terminal do Claude Code), chat (⌘1), pencil (lápis do
                   portal), maestro (o agente recruta, abre portal, conecta, notifica), script
                   (MAGOAGENT_SCRIPT), tests (gestos; tests/locks: bloqueio)
  app/ui/          interface flutuante: mod (widgets, fontes), icons, toolbar, menus,
                   palette, properties, nodes (barra de contexto, diálogo de terminal,
                   compositor de prompts), agents (painel de agentes, ⌘J)
  mcp/             servidor MCP: mod (JSON-RPC, INSTRUCTIONS), calls (ferramentas, view,
                   save_file, show_app), elements (add/update/rótulos), tests
assets/            fontes (mão = MagoHand, interface = Assistant) e licenças OFL; ide-icons/
                   (ícones JetBrains, Apache 2.0, com o NOTICE)
build-macos-app.sh gera MagoAgent.app (bundle com.local.magoagent) e abre
```

- Cada arquivo começa com `use crate::prelude::*;`. Item compartilhado é `pub(crate)` e entra
  no `prelude` pelo módulo dele; os métodos de `DrawingApp` ficam espalhados em `app/` em
  blocos `impl DrawingApp` por assunto.
- Arquivo que passar de ~600 linhas: divida por assunto antes de crescer mais.
- Ferramenta nova do MCP: definição, despacho e saída (imagem, arquivo) em `mcp/calls.rs`, lógica
  dos elementos em `elements.rs`.

## Fluxo de trabalho

1. **Paridade com o app de referência.** Antes de implementar ou mudar um comportamento, leia o
   código de referência (onde ele fica está no `CLAUDE.local.md`, fora do git) e copie valores e
   regras de lá (constantes, cores, tamanhos, textos do `locales/pt-BR.json`). O nome dele não
   aparece em lugar nenhum do projeto: código, textos, nomes de arquivo ou formatos.
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

## Testes de interface sem mexer na tela

`MAGOAGENT_SCRIPT` (src/app/script.rs) dirige o app por um roteiro de passos (`click x y`, `drag`,
`type`, `key J cmd`, `wait ms`, `shot arquivo.png`, `quit`) injetados no raw input, o mesmo caminho do
mouse real. Funciona com a tela do Mac bloqueada. Rode sempre numa cena de teste:

```bash
MAGOAGENT_SCENE=/tmp/teste.mago MAGOAGENT_SCRIPT="wait 2000; click 146 30; wait 500; shot /tmp/a.png; quit" target/release/magoagent
```

`shot` só tem o que o egui desenha; páginas de portal (nativas) não aparecem nele.

## Terminais, agentes e portais

- Terminal = retângulo com `terminal: TerminalSpec` (nome, pasta, comando); portal = retângulo
  com `portal: PortalSpec` (abas). O shell e as páginas vivem só enquanto o app roda.
- Conexão = linha `rope` presa nas duas pontas (`binding.rs`); é o que `list` mostra.
- Cada terminal recebe `MAGOAGENT_BRIDGE`, `MAGOAGENT_TOKEN`, `MAGOAGENT_TERMINAL_ID` e a pasta do
  app no PATH. Fora de um terminal, o cliente usa `bridge.json` ao lado da cena.
- Teste de verdade da ponte: `bridge_asks_an_agent_over_tcp` (um `sh` responde a um `ask`).
- O MCP rodando dentro de um terminal usa `MAGOAGENT_TERMINAL_NAME` no cursor e grava
  `agent_terminal` na cena; o app prende o maior desenho novo a esse terminal (`tie_to_terminal`).
- `portal click|fill` mandam um script (`cursor_script`) que põe o cursor do agente na página.
- Cordas são pintadas antes de tudo e o clique só as acha por último; a ponta sai sempre pela
  borda voltada ao outro elemento (`bound_endpoint`).
- No macOS a página do portal fica 52 pt acima da moldura (WebKit numa janela com conteúdo sob a
  barra de título); `sync_portals` compensa. Se a janela mudar, meça de novo.

## IDE

- IDE = retângulo com `ide: IdeSpec` (nome, pasta). O editor (`ide::Ide`, em `DrawingApp.ides`)
  vive só enquanto o app roda: `sync_ides` (antes de `sync_terminals`) abre um por nó, roda o
  `background` de todos a cada quadro (o Claude Code espera resposta) e salva e fecha os que somem
  ou mudam de pasta.
- No canvas o nó é um card estático (`paint_ide`); o editor de verdade só roda ampliado
  (`set_elevated`, como o terminal): `ide_overlay` mostra `Ide::ui` num `Area` do tamanho de
  `elevated_rect`. Ampliado, ele tem o teclado (`ide_focus`) e os atalhos do canvas ficam
  desligados; Esc (sem nada aberto no IDE), a margem ou o botão da barra voltam ao canvas.
- `src/ide/` é código portado: importa egui direto e usa `super::*`, não o `prelude` (os nomes
  `Tab`, `Cmd`, `Kind` colidiriam). O tema (Islands Dark) vale só no `Ui` do IDE e nos popups dele
  (`theme::style`, `theme::menu`); as fontes são as famílias "ide" e "ide-mono" (Inter e JetBrains
  Mono de uma instalação JetBrains, senão as fontes do app). Nada entra na família Monospace: a
  grade do terminal depende da largura da Hack.
- Claude Code: cada IDE abre o servidor do protocolo de IDE (`~/.claude/ide/<porta>.lock` +
  WebSocket). Um terminal ligado ao IDE por corda recebe `ENABLE_IDE_INTEGRATION=true` e
  `CLAUDE_CODE_SSE_PORT` ao iniciar (`ide_env`); ligado depois, precisa de Reiniciar (ou `/ide`
  dentro do claude). O botão Claude Code do IDE cria o terminal já ligado. Nos testes o lock vai
  para uma pasta temporária, nunca para o `~/.claude` de verdade.

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
