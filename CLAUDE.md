# MagoAgent — guia para agentes

Quadro branco nativo em Rust (eframe/egui 0.37, edition 2024) com servidor MCP embutido.
Alvo: macOS, Windows e Linux. Nada específico de um sistema sem o equivalente dos outros
(`cfg!(target_os = ...)`); veja `file.rs::data_dir`, `mcp/calls.rs::show_app` e
`render/svg.rs::SYMBOL_FONTS`.
Tudo voltado ao usuário (textos da interface, mensagens do MCP, perguntas e resumos) é em pt-BR.

As referências e o roteiro de validação no Windows ficam no repo
privado `edsuuu/magoagent-rea`; comece pelo `CONTINUAR.md` dele.

## Estrutura

```
src/
  main.rs          entrada (app, `magoagent mcp` ou a CLI dos agentes), o `prelude` que todo
                   módulo importa e, no macOS, os botões da janela na linha da barra
  terminal.rs      sessão de terminal (pty + emulação do alacritty_terminal), teclas, cores,
                   pintura da grade
  bridge.rs        ponte dos agentes: servidor local com token, `magoagent list|ask|check|note|
                   portal|recruit|connect|notify` e o cliente que o MCP usa
  transcript.rs    a conversa do Claude Code de um terminal, lida dos arquivos dele (chat); transcript/watch
                   acha a sessão de cada terminal fora da thread da interface
  ide/             o editor do nó IDE (port do Ilha, a IDE do usuário): mod (estado `Ide`, abas,
                   comandos, atalhos, background), chrome (painéis, barra de cima, menus,
                   problemas), tree, editor, text (posições, declaração, comentar/duplicar),
                   highlight + langs, git + commit (janela de commit e diff; `agent_commit`, o commit do `ide_git`), search (pesquisar
                   em tudo, Shift Shift) + find (localizar nos arquivos), index, excluded (as pastas excluídas por projeto, em `ide_excluidas.json` nos dados do app; padrão node_modules, vendor, target…, mais o .gitignore; "Marcar Diretório como" na árvore e `ide_exclude` no MCP), textindex (o texto do projeto em memória, lido numa thread, para a aba Texto e o Localizar nos arquivos), lsp, claude (protocolo de IDE
                   do Claude Code), icons (SVGs JetBrains via resvg), theme (Islands Dark local),
                   dbhost (as abas de banco do IDE sobre o `db/`)
  db.rs + db/      o painel Banco de dados (`Db`, hospedado pelo IDE e pela Árvore, devolve eventos de abrir/fechar aba):
                   SQLite do projeto embutido (rusqlite, sem o binário `sqlite3`) e sem ATTACH, MySQL, Postgres,
                   Mongo, Redis; o "+" abre o menu no visual dos menus do app (`menu_look`), só com os motores que conectam; a leitura abre o arquivo só para leitura, o Submit da tabela e o console escrevem numa
                   transação
  app/filetree*    o nó Árvore de Arquivos (`filetree`: spec e estado vivo, `_view`: listagens lidas fora da thread da interface e o git,
                   `_ui`: o cartão e as áreas sobre ele, `_input`: ações, diálogos e teclas, `_menu`: os menus de clique direito e de exibição e o popover de nome (Renomear, Nova Pasta, Novo Arquivo), `_ops`: arquivos e a ferramenta `file_tree` do MCP, que também busca por nome, localiza nos arquivos e lista usos e histórico; `_pane`: o painel de editor, `_tabs`: as abas dele, `_editor`: texto, foto, PDF e diff, `_code`: o código como o do IDE, `_vision`: os popups de usos e de histórico de uma declaração, `_find`: o Localizar nos Arquivos, `_db`: o painel Banco de Dados, as abas de grade/console e as ações `db*` do MCP, só leitura, `_diff`: os marcadores de mudança na margem do editor contra o HEAD (barra verde, azul e triângulo cinza), o popup deles e o rollback de uma faixa, `_compare`: a aba do diff lado a lado/unificado contra o HEAD, Ctrl+D, `_problems` e `_problems_ui`: os erros e avisos do código (LSP do `ide/lsp.rs` mais uma checagem de sintaxe local; sublinhado ondulado, semáforo, faixa de marcas, F2, nomes vermelhos na árvore, ação `problems` do MCP))
  rope.rs          cordas das conexões (corrente de Verlet com gravidade)
  model.rs         Element, ElementKind, Style, Scene, Prefs, cores
  geometry.rs      limites, contornos, distâncias, hit-test
  binding.rs       setas presas às formas (orbit / inside)
  labels.rs        rótulos centrados, máscara de rótulo de seta, contorno de texto
  recognize.rs     desenhar para criar forma
  tests.rs         formatos de arquivo, exportação, paleta
  file.rs          JSON dos elementos (área de transferência e MCP), autosave, preferências,
                   merge, leitura dos chats (`read_chats`: um chats.json ilegível vai para o lado)
  render/          pintura no canvas: mod (elementos, imagens, notas adesivas), rough (traço
                   à mão), replay (IA desenhando ao vivo), svg (exportação SVG/PNG)
  app/             DrawingApp: mod (estado + loop por frame), tool, files, editing,
                   shortcuts (letras de ferramenta: `tool_keys_live`, só com a fileira de desenho aberta, menos Mão e Seleção), selection, pointer, arrows (setas presas), paint (cena, prévia
                   e cursor), terminal (terminais, conexões, foco, modo ampliado), portal
                   (navegador com abas sobre o canvas, via wry; páginas casadas às abas pelo endereço, `match_portal_tabs`), stack (pilha de notas: arquivar, virar a página, desempilhar, soltar uma nota na pilha; `note_stack` no MCP), portal_console (o console da página para os agentes), portal_shot (a imagem da página para os agentes, só no Windows), ide (nó IDE: diálogo, card,
                   editor ampliado, terminal do Claude Code), chat (⌘1), design (modo
                   Design do portal), maestro (o agente recruta, abre portal, conecta, notifica), timeline (a linha do tempo dos eventos para o Maestro), usage (o consumo: o limite do plano lido da linha de status do Claude Code, `magoagent statusline`, e do stream dos chats, guardado em `consumo.json`; teto de agentes, trava de recrutamento perto do limite e modelo dos recrutas), tasks (as tarefas: o que foi pedido a quem, o estado e as dependências, em `tarefas.json` ao lado da cena; as travadas viram demandas; tests em tasks/tests.rs), binder (o fichário: notas com nome em `fichario/`, que o Maestro e os agentes leem e escrevem entre sessões), skills (a pasta de skills do app, `skills/` ao lado da cena, entregue ao Claude Code do chat e dos terminais como plugin local, `--plugin-dir`), environments (o ambiente próprio de um agente recrutado: um git worktree do repositório, num ramo dele), workspaces (os espaços de trabalho: cada um um projeto com a sua cena, pasta raiz e cor, em `espacos.json` ao lado da cena; trocar muda o que se vê, não o que roda: os shells de um espaço fora da vista ficam em `away` e seguem rodando), actions (ações do app pelo
                   MCP via `act` da ponte: IDE, arquivos, janelas, chats, chamada, git, abas, ajustes, áreas, desfazer;
                   ação nova = um caso aqui e a ferramenta em mcp/app_tools.rs), script
                   (MAGOAGENT_SCRIPT), chat_direct (chat sem terminal: claude -p / codex exec em JSON,
                   chats.json; o que os chats do Codex disseram fica em chats/<id>.json), chat_footer (chips de pasta/modelo/esforço/permissão sob a caixa),
                   chat_tools (o Maestro age: MCP do app no chat, prompt de sistema, memória,
                   show_screen/set_folder/screenshot/review), avatar (a barra do Maestro numa
                   janela própria por cima de tudo, com o mascote do agente; arrastável por qualquer ponto e guardada em `avatar_at`, ou seguindo a janela em uso; tem o botão da chamada e o uso da janela de 5 h do plano, e com ela na tela o popup da chamada não abre), mascot (o mascote do Claude Code em pixel art
                   animado por partes: corpo, pernas, olhos e brilhos lidos dos quadradinhos dos SVGs), voice_orb (o orbe da chamada: o ponto que
                   pulsa e abre em orbe com mola, o cartão de quando não inicia), voice_popup (a chamada numa janela
                   pequena por cima de tudo, arrastável: o lápis abre o chat num popup próprio, microfone, mascote ou orbe com o X no hover, alto-falantes), voice (a chamada:
                   comandos "Claude, se muta/pode falar/desmuta", standby com toque, o avatar
                   acompanha; `call_engine` escolhe Codex, API ou local, e sem chat aberto a chamada abre um do agente de Ajustes), tests (gestos; tests/locks: bloqueio)
  voice.rs         a voz: microfone (cpal) cortado por nível (pausa de Ajustes › Voz) → whisper.cpp
                   local (build CUDA e modelo turbo no Windows com NVIDIA, Metal no Mac), o resumo
                   da resposta falado pelo Piper ou pela voz do sistema (`spoken`: sem caminhos nem
                   código); falar por cima interrompe (`BargeIn`); comandos de voz (`voice_command`); o app baixa tudo em segundo plano ao abrir (`fetch_voice_in_background`)
  voice_openai.rs  a voz pela API da OpenAI (o padrão): gpt-transcribe ouve, gpt-4o-mini-tts fala em PCM
                   de 24 kHz; a chave vai ao curl pela entrada padrão (`-K -`); Local cai no voice.rs
  voice_codex.rs   a voz do Codex (o padrão, fora do Linux): `codex app-server` com o login do ChatGPT,
                   `thread/realtime/start` v3 por WebRTC; os pedidos vão ao Codex na pasta do chat com
                   o MCP do app; o som fica numa página fora da vista (app/voice_page.rs, wry)
  install.rs       o instalador (o mesmo programa chamado `MagoAgent-Instalador` ou `magoagent
                   instalar`; no Windows o instalador é o MSI de installer/): copia para `install_dir`, move os
                   dados antigos para `data`, atalho no menu. Nada de voz: whisper.cpp e Piper o app baixa na
                   primeira vez que o ditado local pede (`fetch_voice_in_background`)
  app/updates.rs   a última release do GitHub (edsuuu/magoagent) contra a versão do Cargo.toml, por curl numa
                   thread; ao abrir (no máximo uma vez por dia) avisa, e Ajustes › Sobre mostra as notas e o
                   botão Atualizar, que baixa `MagoAgent-<versão>.msi` da release para a pasta temporária e
                   o abre com o msiexec; ação `update` no MCP
  app/ui/          interface flutuante: mod (widgets, fontes), icons, toolbar, menus,
                   palette, properties, nodes (barra de contexto, diálogo de terminal,
                   compositor de prompts), agents (painel de agentes, ⌘J), tasks (as abas Tarefas e Fichário desse painel), sidebar (a barra lateral dos espaços de trabalho), folders (pastas recentes e
                   favoritas do campo de pasta e do chip de pasta do chat; `Prefs.folders`)
  mcp/             servidor MCP: mod (JSON-RPC, INSTRUCTIONS), calls (ferramentas, view,
                   save_file, show_app), app_tools (as ações do app), elements (add/update/rótulos), tests
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
  Toda ação do app que a pessoa faz precisa ter a sua ferramenta no MCP (o Maestro faz tudo pela voz).

## Fluxo de trabalho

1. **Paridade com o app de referência.** Antes de implementar ou mudar um comportamento, leia o
   código de referência (onde ele fica está no `CLAUDE.local.md`, fora do git) e copie valores e
   regras de lá (constantes, cores, tamanhos, textos do `locales/pt-BR.json`). O nome dele não
   aparece em lugar nenhum do projeto: código, textos, nomes de arquivo ou formatos.
2. **Implementar** com o menor diff que resolve, no estilo do arquivo. Atalho deliberado com
   limite conhecido leva um comentário `// ponytail: <limite>, <como evoluir>`. Rode `cargo fmt`
   antes de commitar (`rustfmt.toml`: linhas de até 160 colunas).
3. **Testar:** `cargo test` deve passar, e a compilação dos outros sistemas também:
   `cargo check --release --target x86_64-pc-windows-msvc` e
   `cargo check --release --target x86_64-unknown-linux-gnu`. Toda lógica nova deixa um teste em
   `interaction_tests` (os gestos usam os helpers `frame` e `drag`) ou nos testes de `mcp.rs`.
4. **Gerar o app:**
   `pkill -f "MagoAgent.app/Contents/MacOS/magoagent$"; ./build-macos-app.sh`
   No Windows, no dia a dia, teste em modo dev com `cargo run --release` (sem instalador). Só quando o
   dono pedir um build: suba a versão em `Cargo.toml` (cada instalador gerado tem a sua: 0.0.1, 0.0.2, …),
   e rode `sh installer/build-msi.sh` (WiX 3 em `~/tools/wix314/bin`, ou `WIX=<pasta>`): sai
   `target/release/MagoAgent-<versão>.msi`, que vai como asset da release do GitHub (o Atualizar do app o
   procura). O MSI instala por máquina em `C:\Program Files\MagoAgent` (pede administrador), troca a versão
   anterior (MajorUpgrade), grava `versao`, libera a pasta e `data/` para quem instala, põe o atalho no menu
   Iniciar e o Desinstalar do Windows; a pasta fica em `HKCU\Software\MagoAgent\InstallDir`
   (`install_dir`). Tudo do app fica nela: `data/` com desenhos, chats, preferências e modelos;
   `data/projetos` é a pasta dos chats. Um build do repo usa os dados da instalação.
   Nada vai para a Área de Trabalho.
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

No Windows, as teclas injetadas pelo Windows MCP não chegam ao app (o mouse chega): teclas vão pelo
`MAGOAGENT_SCRIPT` (`key`, `type`). Com `MAGOAGENT_SCRIPT` o app não baixa a voz ao abrir. A cena de
teste leva junto preferências, chats, conexões e a pasta `recuperados`; o resto dos dados (projetos,
memória do Maestro, modelos de voz) continua na pasta de dados de verdade.

## Terminais, agentes e portais

- Terminal = retângulo com `terminal: TerminalSpec` (nome, pasta, comando); portal = retângulo
  com `portal: PortalSpec` (abas). O shell e as páginas vivem só enquanto o app roda.
- Conexão = linha `rope` presa nas duas pontas (`binding.rs`); é o que `list` mostra.
- Cada terminal recebe `MAGOAGENT_BRIDGE`, `MAGOAGENT_TOKEN`, `MAGOAGENT_TERMINAL_ID` e a pasta do
  app no PATH. Fora de um terminal, o cliente usa `bridge.json` ao lado da cena. O token de um terminal é
  só dele (`Shared::terminal_token`): a ponte sabe por ele quem fala (`authorize`) e ele deixa de valer quando
  o shell acaba; o do `bridge.json` é o geral.
- Teste de verdade da ponte: `bridge_asks_an_agent_over_tcp` (um `sh` responde a um `ask`).
- `magoagent recruit --worktree` (ou `worktree: true` no `recruit_agent`) põe o agente novo num git worktree
  próprio (`app/environments.rs`): `<repo>/.magoagent/worktrees/<nome>`, ramo `mago/<nome>`. O app não junta
  o ramo nem apaga a pasta.
- O Maestro deixa de recrutar acima do teto de agentes ou com a janela de 5 h do plano perto do fim
  (`app/usage.rs`, Ajustes › Desempenho); as tarefas (`tasks`) e o fichário (`binder`) guardam o que foi
  pedido a quem e o que foi decidido.
- O MCP rodando dentro de um terminal usa `MAGOAGENT_TERMINAL_NAME` no cursor e grava
  `agent_terminal` na cena; o app prende o maior desenho novo a esse terminal (`tie_to_terminal`).
- `portal click|fill` mandam um script (`cursor_script`) que põe o cursor do agente na página.
- Cordas são pintadas antes de tudo e o clique só as acha por último; a ponta sai sempre pela
  borda voltada ao outro elemento (`bound_endpoint`).
- No macOS o wry prende a página à borda de baixo da janela; o `new_tab_view` tira essa máscara e o
  canvas posiciona a página (`sync_portals`). O `shot` do roteiro grava as páginas para conferir.

## Árvore de Arquivos

- Árvore = retângulo com `filetree: FileTreeSpec` (raiz, pasta mostrada, ocultos, ordem). As pastas abertas, a rolagem, as
  listagens (lidas numa thread, relidas a cada 2 s) e o git ficam no `TreeView` de `DrawingApp.filetrees`, só enquanto o app roda.
- O canvas pinta o cartão (`paint_filetree`); botões, linhas e busca são áreas do egui por cima (`filetree_overlay`), menos a faixa
  do título, que continua do canvas e arrasta o nó. Com a árvore focada (`card_focus`), as setas, Enter, F2, Delete e ⌘C/X/V são dela.
- Clique duplo num arquivo abre no IDE do projeto (aba do cartão) ou, sem IDE, no programa padrão do sistema.
- O código no painel de editor da árvore (`filetree_code.rs`) é desenhado como o editor do IDE, sem nó IDE por trás: usa as partes
  puras de `ide/` (highlight, hints, index, text, tema e fonte "ide-mono") sobre um `Project` da raiz da árvore, lido numa thread,
  para os nomes de parâmetro, os usos e o autor sobre as declarações e o Ctrl+clique. O que pede servidor de linguagem fica só no IDE.
- Os rótulos sobre uma declaração são links (`filetree_vision.rs`): "N usos" (e o Ctrl+clique na própria declaração) abre o popup dos
  usos, o autor liga a coluna de anotação (git blame: data dd/mm/aaaa e autor por linha, à esquerda dos números, `filetree_annotate.rs`;
  o mouse numa linha dela pinta a linha de verde; o autor de novo, ou o menu de clique direito da margem, desliga; no MCP `file_tree` com
  `annotate`); o histórico das linhas (`git log -L`) segue no MCP (`history`), e um commit dele abre o diff numa aba (`Body::Diff`).
  Ctrl+Shift+F abre o Localizar nos Arquivos (`filetree_find.rs`) sobre a raiz, sem as pastas excluídas (`ide/excluded.rs`), com a
  busca de `ide/find.rs` sobre o texto em memória (`ide/textindex.rs`). No MCP: `file_tree` com `usages`, `history` e `find_in_files`.
  O botão Alterações da barra de baixo (`filetree_bar.rs` guarda os três botões; `filetree_changes.rs`) abre um painel com os arquivos alterados contra
  o HEAD (letra e cor do estado) no lugar do Banco de Dados; clicar abre o diff dele numa aba (`Editor::open_change`: novo contra nada, apagado contra o HEAD);
  no MCP, `file_tree` com `changes`.
  O chip do ramo no cabeçalho (`filetree_branch.rs`) abre o menu dos ramos locais e troca com `git checkout`, recusando com a
  razão se há mudanças que a troca perderia; no MCP, `file_tree` com `branches` e `checkout`.
  "Vincular" (na pílula da árvore) liga a árvore por corda a um terminal ou chat: ela aparece no `magoagent list` desse agente
  (`NodeKind::Tree`, bridge/tree.rs), o `file_tree` sem `tree` fala dela e o `ide_open` sem IDE abre o arquivo no painel de editor dela.
  Ctrl+D (árvore ou código) e "Mostrar Diff" no menu de um arquivo modificado abrem o diff contra o HEAD; no MCP, `file_tree` com `diff`, `hunks` e `revert_hunk`.

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
- Conflito e recuperação: `ide_edit` (`agent_edit`) recusa aba com mudanças não salvas e não deixa nada
  pela metade. Aba com mudanças suas que o arquivo mudou no disco ganha uma faixa no editor
  (Recarregar do disco ou Manter a minha versão). Ao fechar, o texto que não pôde ser gravado vai para
  `data/recuperados` e o app avisa onde; na primeira vez, a janela não fecha, para mostrar o aviso (`recover`).
- Claude Code: cada IDE abre o servidor do protocolo de IDE (`~/.claude/ide/<porta>.lock` +
  WebSocket). Um terminal ligado ao IDE por corda recebe `ENABLE_IDE_INTEGRATION=true` e
  `CLAUDE_CODE_SSE_PORT` ao iniciar (`ide_env`); ligado depois, precisa de Reiniciar (ou `/ide`
  dentro do claude). O botão Claude Code do IDE cria o terminal já ligado. Nos testes o lock vai
  para uma pasta temporária, nunca para o `~/.claude` de verdade.

## MCP

- O app e o servidor só conversam pelo arquivo da cena (escrita atômica: grava e renomeia).
  O app relê a cada 0,4 s e mescla em 3 vias (`merge_scenes`); elementos novos vindos da IA
  são desenhados traço a traço com o nome do agente (`Scene.agent`, vindo do `clientInfo`).
- `ide_git` com `commit`: salva as abas antes e recusa se o git do IDE estiver ocupado ou se alguma aba
  não puder ser salva. Sem `files`, leva só o que já é versionado (`add -u`); com `files` (relativos à
  pasta do IDE), leva só esses, inclusive não versionados. A resposta diz o que entrou e os não
  versionados que ficaram de fora (`agent_commit`).
- Para testar o MCP sem cliente de IA, mande JSON-RPC pelo stdin de `magoagent mcp`
  (`initialize` com `clientInfo.name`, depois `tools/call`).
- O caminho registrado no Claude Code (`~/.claude.json`) e no Codex (`~/.codex/config.toml`)
  aponta para `~/projects/magoagent/MagoAgent.app/Contents/MacOS/magoagent`; se a pasta mudar,
  atualize os dois.

## Git

Commits sem linha de coautor. Não versionar `target/`, `MagoAgent.app` nem desenhos (`.mago`, `.rea`); a exceção é o exemplo `docs/arquitetura.mago`.
