# MagoAgent — guia para agentes

Quadro branco nativo em Rust (eframe/egui 0.37, edition 2024) com terminais, chats, navegador, árvore de arquivos e um
servidor MCP embutido. Alvo: Windows, macOS e Linux. Nada específico de um sistema sem o equivalente dos outros
(`cfg!(target_os = ...)`; exemplos: `file/mod.rs::data_dir`, `mcp/calls/mod.rs::show_app`, `render/svg.rs::SYMBOL_FONTS`).
Tudo voltado ao usuário (textos da interface, mensagens do MCP, perguntas e resumos) é em pt-BR.

As referências e o roteiro de validação no Windows ficam no repo privado `edsuuu/magoagent-rea` (comece pelo `CONTINUAR.md`).

## Estrutura

```
src/
  main.rs        entrada (o app, `magoagent mcp`, a CLI dos agentes, `hook`, `statusline`, `instalar`) e o `prelude`
  app/           o `DrawingApp`: mod/state/init/update (estado e o loop por frame) e uma pasta por assunto
    canvas/      o desenho: ferramentas (tool), edição, atalhos, seleção, ponteiro, setas presas, pintura, drops, dock, minimapa
    terminal/    o cartão de terminal: diálogo Novo Chat/Terminal, foco, ampliado, menu, busca, processos, cordas, prefs
    chat/        o chat: direct/ (claude -p / codex exec em JSON; título pelo Haiku), composer, conversation, footer (chips),
                 tools (o MCP do app e o prompt do Maestro), commands/, context*, node* (o chat no canvas), mini* (mini chat)
    filetree/    a Árvore de Arquivos: lista, git (diff, blame, ramos, alterações), painel de editor, Localizar, problemas
    browser/     o navegador (`PortalSpec`, wry; no Linux o WebKitGTK na thread do GTK, webkit.rs, só no X11): abas, console e imagem da página para os agentes, modo Design, cofre (vault)
    maestro/     o Maestro: recrutar, linha do tempo, demandas, eventos dos hooks, fichário, skills, worktrees, avatar
    voice/       a chamada e o ditado: orbe, popup, o som do Codex (page)
    island/      a ilha: pílula fixa no topo da tela (janela própria por cima de tudo) com o resumo dos agentes, perguntas pendentes, voz e chat novo em espaço próprio (`Prefs.island`)
    integrations/ Ajustes › Integrações: um arquivo por serviço de fora mostrado na ilha (GitHub, Spotify, Hermes Agent); http (curl, segredos pelo stdin)
    notes/       notas adesivas e o fichário em pilha; groups/ (grupos com quadro); floors/ (andares); workspaces/ (espaços)
    tasks/       tarefas pedidas aos agentes (`tarefas.json`); usage/ (consumo do plano, teto de agentes); mascot/
    system/      arquivos, atualizações (release do GitHub; download.rs confere o sha256; update_flow.rs e updater.rs salvam, param os processos, atualizam e reabrem, no Windows; no Linux instalado pelo .deb, deb_update.rs instala o .deb da release pelo pkexec e reabre), memória, bandeja, a thread do GTK no Linux (gtk.rs: bandeja e páginas), `MAGOAGENT_SCRIPT` (script.rs)
    ui/          a interface flutuante: widgets e fontes (mod), ícones, barra, menus (`menu_look`), propriedades,
                 estado dos agentes, barra lateral, diálogos, settings/ (Ajustes)
    tests/       testes de interface, um arquivo por assunto (gestos com `frame` e `drag`)
  mcp/           tudo do MCP: mod (JSON-RPC, INSTRUCTIONS), calls/ (ferramentas: lista em tools.rs, despacho em mod.rs),
                 elements (adicionar/atualizar elementos), app_tools + actions/ (o que o app faz a pedido do MCP:
                 arquivos, janelas, chats, ajustes), bridge/ (a ponte local com token entre app, terminais e o MCP;
                 `magoagent list|ask|check|note|portal|recruit|terminal|connect|notify`)
  model/         Element, ElementKind, Style, specs (TerminalSpec, PortalSpec, NoteSpec), Scene, Prefs
  geometry/      limites, contornos e hit-test (mod), setas presas (binding), rótulos (labels), desenhar para criar forma
                 (recognize), cordas de Verlet (rope); `HAND_FACE` e `ASSISTANT_FONT`
  render/        pintura dos elementos, traço à mão (rough), IA desenhando ao vivo (replay), exportação SVG/PNG, markdown
  file/          formato da cena e de troca (interop: área de transferência e MCP), autosave, preferências, merge
  terminal/      a sessão de terminal: pty + alacritty_terminal, teclas, seleção, cores, pintura da grade
  transcript/    a conversa do Claude Code de um terminal, lida dos arquivos dele; watch acha a sessão fora da thread da UI
  editor/        código portado usado como biblioteca pela Árvore (highlight, langs, git, lsp, find, merge, tema, ícones)
  voice/         microfone (cpal), voz pelo VoiceStudio (servidor do usuário) e pela OpenAI, e pelo Codex (realtime); nada de voz é baixado
  install/       o instalador (`MagoAgent-Instalador` / `magoagent instalar`); no Windows é o MSI de installer/
  tests/         testes gerais: formatos, exportação, desenho, transcrições, shells
assets/          fontes (mão = Excalifont do Excalidraw, interface = Assistant; licenças OFL), editor-icons/ (JetBrains,
                 Apache 2.0, com NOTICE), mascot/, ícones do app
installer/       MSI do Windows (WiX 3: build-msi.sh, magoagent.wxs)
docs/            imagens do README e o exemplo arquitetura.mago
build-macos-app.sh  gera MagoAgent.app (bundle com.edsuuu.magoagent)
.claude/skills/  skills dos agentes, versionadas (o resto do .claude fica fora do git)
```

## Regras de código

- Uma pasta por assunto; o arquivo dentro dela não repete o nome do assunto (`chat/footer.rs`), o principal é o `mod.rs`.
  Assunto novo com mais de um arquivo ganha pasta. A raiz de `src/` tem só o `main.rs`.
- Cada arquivo começa com `use crate::prelude::*;`. Item compartilhado é `pub(crate)` e entra no `prelude` pelo módulo
  dele. Os métodos de `DrawingApp` ficam em blocos `impl DrawingApp` no arquivo do assunto.
- `src/editor/` é a exceção: código portado, importa egui direto e usa `super::*`.
- Linhas em branco separam as partes de um bloco (`src/tests/spacing.rs`, o `cargo test` confere): depois de um grupo
  de `let`, antes e depois de uma instrução de várias linhas e antes de `return`, `break` e `continue`. O corretor:
  `MAGOAGENT_FIX_SPACING=1 cargo test --release statements_have_their_blank_lines`.
- Arquivo passando de ~600 linhas: divida por assunto.
- Nomes por extenso (`index`, `response`, `colors`), nada de variáveis de uma letra fora de fechamentos curtos.
- Atalho deliberado com limite conhecido leva `// ponytail: <limite>, <como evoluir>`.
- Comentário explica o código, não conta a história do pedido: nada de citar o dono (`Asked: "…"`, `(asked: "…")`). O
  pedido vai na descrição do PR.
- Toda ação que a pessoa faz no app tem a sua ferramenta no MCP (o Maestro faz tudo pela voz): a ferramenta em
  `mcp/calls/tools.rs`, o despacho em `mcp/calls/mod.rs`, e a ação do app em `mcp/actions/` (um caso a mais) com a
  entrada em `mcp/app_tools.rs`.
- No Windows nada abre janela de console: processos saem por `crate::editor::command`.
- egui 0.37: `Id::unique` (o `Id::new` está obsoleto), `ui.scope_id()` (o `ui.id()` está obsoleto; `ui.unique_id()`
  não é estável), `Panel` e `.show(ui, ..)`; nada bloqueia no `ui`/`logic` (I/O e processos vão para uma thread que
  chama `request_repaint` ao terminar); repaint só enquanto algo se mexe; fontes e texturas criadas uma vez. O resto
  está na skill `egui`.

## Skills

Ficam em `.claude/skills/<nome>/SKILL.md`, versionadas: quem clona o repo já tem. O Claude Code as acha sozinho; no
Codex (ou em outro agente), leia o `SKILL.md` quando o assunto bater. Este arquivo manda quando uma skill divergir dele.

- `egui`: as regras de egui/eframe 0.37 com as convenções do projeto (APIs que mudaram, IDs, loop do frame, repaint).
- `egui-review`: revisão só de leitura de código de interface (diff ou pasta), com relatório em pt-BR.
- `rust-best-practices` (Apollo) e `rust-testing`: Rust idiomático e testes. Cópias sem mudança, com a licença MIT de
  origem; o projeto não usa async (threads e `std::sync::mpsc`), então as partes de async não se aplicam.

As adaptadas (`egui`, `egui-review`) vêm do egui-agent-skills (MIT/BSD-3, `LICENSE` em cada pasta). Skill nova: uma
pasta com `SKILL.md` (front matter com `name` e `description`) e a licença, se vier de fora; leia tudo antes de
versionar, porque ela roda com as permissões do agente.

## Fluxo de trabalho

1. **Paridade com o app de referência.** Antes de mudar um comportamento, leia o código de referência (onde ele fica
   está no `CLAUDE.local.md`, fora do git) e copie valores e regras de lá (constantes, cores, tamanhos, textos do
   `locales/pt-BR.json`). O nome dele não aparece em lugar nenhum do projeto.
2. **Implementar** com o menor diff que resolve, no estilo do arquivo. Formate os arquivos mexidos com
   `rustfmt --edition 2024` (`rustfmt.toml`: até 160 colunas).
3. **Testar:** `cargo test` passa sem avisos. Toda lógica nova deixa um teste (`app/tests/`, `tests/` ou `mcp/tests.rs`).
   Os outros sistemas também compilam: `cargo check --release --target x86_64-unknown-linux-gnu` (e o do Windows/macOS).
4. **Ver no app** pelo roteiro (abaixo), numa cena de teste, nunca no desenho real do usuário.
5. **Deixar para o dono:** no fim, `cargo run --release` no target padrão.
6. **Relatar** em pt-BR: o que mudou, o que foi testado e o que não foi (no PR, pelas seções do modelo, Git abaixo).

Nada de trabalho na `/tmp` (nem na pasta temporária do sistema): no Linux ela é tmpfs e some ao reiniciar (o dono
perdeu testes num outro PC assim). Worktrees, clones, rascunhos e builds ficam no disco: worktrees ao lado do repo
(`../magoagent-wt/<nome>`) ou em `<repo>/.magoagent/worktrees`, com o target compartilhado se faltar espaço. A `/tmp` só
para o que pode sumir: cenas de teste e capturas descartáveis, saídas de comando.

Builds de instalação só quando o dono pedir: suba a versão em `Cargo.toml` e rode `sh installer/build-msi.sh`
(sai `target/release/MagoAgent-<versão>.msi`, asset da release do GitHub que o Atualizar procura). O MSI instala em
`C:\Program Files\MagoAgent` com os dados em `data/`. No macOS, `./build-macos-app.sh`.

Um build do repo (o executável dentro de `target/`) é o app em desenvolvimento (`file::development`): janela
"MagoAgent (development)", classe `magoagent-dev` e dados próprios (`magoagent-dev` / `MagoAgent-dev` na pasta de dados
do sistema), então roda ao lado do app instalado sem gravar o mesmo desenho. `MAGOAGENT_DEV=0` usa os dados reais;
`MAGOAGENT_DEV=1` força o modo.

No Linux o app roda em X11 sempre que há um servidor X (`DISPLAY`), o XWayland numa sessão Wayland: só lá a ilha fica
por cima de tudo, no lugar pedido, e deixa o clique passar fora dela (a forma de entrada da janela, `island/x11.rs`).
`MAGOAGENT_WAYLAND=1` mantém o Wayland (a ilha vira uma janela comum que o compositor posiciona).

## Testes de interface sem mexer na tela

`MAGOAGENT_SCRIPT` (`app/system/script.rs`) dirige o app por passos (`size w h`, `click x y`, `drag`, `type`,
`key J cmd`, `wait ms`, `shot arquivo.png`, `quit`) injetados no raw input, o mesmo caminho do mouse real:

```bash
MAGOAGENT_SCENE=/tmp/teste.mago MAGOAGENT_SCRIPT="size 1440 900; wait 2000; click 146 30; shot /tmp/a.png; quit" target/release/magoagent
```

- A cena de teste leva junto preferências, chats, conexões e `recuperados`; o resto (projetos, memória do Maestro,
  modelos de voz) continua na pasta de dados real. Com o roteiro o app não baixa a voz.
- `shot` só tem o que o egui desenha; páginas do navegador (nativas) não aparecem. `shot-ilha arquivo.png` salva a
  janela da ilha, só enquanto ela se desenha sozinha (em repouso, `island/replay.rs`).
- No Windows as teclas injetadas de fora não chegam ao app (o mouse chega): teclas vão pelo roteiro.

## Terminais, agentes e conexões

- Terminal = retângulo com `terminal: TerminalSpec`; navegador = `portal: PortalSpec`; chat = `ChatSpec`; árvore =
  `filetree: FileTreeSpec`. Shells, páginas e listagens vivem só enquanto o app roda.
- Conexão = linha `rope` presa nas duas pontas (`geometry/binding.rs`); é o que `magoagent list` mostra. Cordas são
  pintadas antes de tudo e o clique só as acha por último.
- Cada terminal recebe `MAGOAGENT_BRIDGE`, `MAGOAGENT_TOKEN`, `MAGOAGENT_TERMINAL_ID` e a pasta do app no PATH. O token
  de um terminal é só dele (`Shared::terminal_token`) e morre com o shell; fora de um terminal, o cliente usa o
  `bridge.json` ao lado da cena. Teste de verdade da ponte: `bridge_asks_an_agent_over_tcp`.
- `recruit` com `worktree: true` põe o agente num git worktree próprio (`<repo>/.magoagent/worktrees/<nome>`, ramo
  `mago/<nome>`); o app não junta o ramo nem apaga a pasta.
- Ajustes › Agentes › "Invocação dos agentes" (`Prefs.agent_mode`): Terminal recruta um Claude Code num terminal; Chat
  recruta um cartão de chat (`chat/recruit_card.rs`). O Maestro para de recrutar acima do teto de agentes ou perto do
  fim da janela de 5 h do plano (`app/usage/`).
- WSL (`app/system/wsl.rs`; Ajustes › Agentes › WSL, `set_setting wsl`): no Windows com o claude/codex só dentro de uma
  distro, chats, recrutas e o uso do plano rodam por `wsl.exe -d <distro> --cd <pasta> --exec env PATH=… <agente>`, com os
  caminhos traduzidos (`C:\x` ↔ `/mnt/c/x`, `\\wsl.localhost\<distro>\…`). Hooks e MCP chamam o exe por `/mnt/c/…`, a
  ponte cruza por `WSLENV` e o `magoagent` do WSL é o script em `<dados>/wsl`. Automático só olha o WSL quando o Windows
  não tem nenhum dos dois agentes. Um Claude Code de fora, no WSL: `claude mcp add magoagent -- /mnt/c/…/magoagent.exe mcp`.
- Não existe mais nó IDE: o campo `ide` de cenas antigas vira uma Árvore na mesma pasta (`Element::take_legacy_ide`).

## MCP

- O app e o servidor só conversam pelo arquivo da cena (escrita atômica). O app relê a cada 0,4 s e mescla em 3 vias
  (`merge_scenes`); o que a IA desenha aparece traço a traço com o nome do agente (`Scene.agent`, do `clientInfo`).
  Ações que precisam do app vivo vão pela ponte (`act`) até `mcp/actions/`.
- Os chats do app recebem o MCP sozinhos (`chat/tools.rs`: `--mcp-config` no Claude, `-c mcp_servers…` no Codex),
  apontando para o executável em uso. Um Claude Code de fora precisa registrar: `claude mcp add magoagent -- <exe> mcp`.
- Para testar sem cliente de IA, mande JSON-RPC pelo stdin de `magoagent mcp` (`initialize` com `clientInfo.name`,
  `tools/list`, `tools/call`).

## Qualidade

A esteira de CI roda a cada push na `main` e em cada pull request (uma vez por commit; um push novo no PR cancela o
anterior), com jobs paralelos de formatação, clippy, auditoria de dependências, testes nos dois sistemas e um teste de
ponta a ponta no Linux. PR em rascunho não roda: o CI começa quando ele fica pronto para revisão. Uma tag `v*` roda o
mesmo CI e, ao mesmo tempo, compila os pacotes; a release (rascunho) só sai se o CI passar, já com as notas ("## Novo",
um item por PR juntado desde a última release; revise antes de publicar) e o `SHA256SUMS.txt`.

O Rust é o do `rust-toolchain.toml`, o mesmo na CI e nas máquinas. Subir de versão é um commit de propósito: muda o
arquivo e o `dtolnay/rust-toolchain@<versão>` dos workflows junto, e corrige o que o clippy novo apontar. Os alvos da
compilação cruzada se instalam uma vez por versão (`rustup target add …`, o comando está no arquivo).

Rodar local antes de commitar:

```bash
# formatação (rustfmt.toml: até 160 colunas)
rustfmt --edition 2024 --check $(find src -name "*.rs")

# lints (os permitidos estão em [lints.clippy] no Cargo.toml)
cargo clippy --all-targets --release -- -D warnings

# auditoria de dependências (vulnerabilidades, licenças, fontes; deny.toml)
cargo deny check

# testes unitários e de integração
cargo test --release

# compilação para os outros sistemas
cargo check --release --target x86_64-unknown-linux-gnu
cargo check --release --target x86_64-pc-windows-msvc
cargo check --release --target aarch64-apple-darwin
```

O teste de ponta a ponta do CI roda o app numa cena vazia com `MAGOAGENT_SCRIPT` (size, wait, shot, quit) sob xvfb e
confere que a captura existe. Rodar local (Linux ou macOS):

```bash
printf '{"version":1,"elements":[],"background":[255,255,255,255]}' > /tmp/teste.mago
MAGOAGENT_SCENE=/tmp/teste.mago MAGOAGENT_SCRIPT="size 1000 700; wait 3000; shot /tmp/e2e.png; quit" target/release/magoagent
test -s /tmp/e2e.png && echo "passou"
```

## Git

Branches: o trabalho dos agentes vai em pull requests para a `dev` (sem CI); lá ele é testado, validado e juntado, com os
conflitos resolvidos, e a `dev` vai para a `main` num pull request (o CI roda nele e o auto-merge junta; versão nova no
`Cargo.toml` gera a tag e a release).

Commits sem linha de coautor. Não versionar `target/`, `MagoAgent.app` nem desenhos (`.mago`, `.rea`); a exceção é
`docs/arquitetura.mago`.

O PR segue o modelo `.github/pull_request_template.md` (o que mudou, como foi testado, antes e depois, desempenho,
riscos). Issues de quem usa o app vêm pelos modelos de `.github/ISSUE_TEMPLATE/` (bug, travamento, recurso); as dos
agentes e do dono podem ser em branco, no formato Hoje / Proposta / Pronto quando.
