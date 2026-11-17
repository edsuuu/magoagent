# Pendentes da ilha (PR #55, ramo `feat/ilha-abas`)

Para o próximo agente: o que já está no ramo, o que falta fazer e o contexto da conversa com o dono. Siga o
`AGENTS.md` (regras de código, pt-BR, testes, `rustfmt --edition 2024`, commits sem linha de coautor). Ao terminar,
apague este arquivo no mesmo PR.

## Estado do ramo

**Os últimos commits não foram testados.** O commit deste arquivo leva junto mudanças que ninguém compilou nem testou
depois de prontas (o dono pediu para parar e passar adiante). Antes de qualquer coisa:

```bash
cargo test --release
cargo clippy --all-targets --release -- -D warnings
cargo check --release --target x86_64-unknown-linux-gnu   # ou no Linux/WSL
```

O que entrou e precisa ser conferido:

- **Ilha compacta** 264 × 38,5 px (era 288 × 40), `island/mod.rs` (`COMPACT_W`, `COMPACT_H`). Os minis passaram a 14 px
  (`island/minis.rs`) para caber. O teste de `target` em `island/mod.rs` já espera 264 × 38,5.
- **Cursor de mãozinha** em toda a ilha fechada ou compacta (o clique abre), `paint.rs` (o `grip`).
- **Luna no notch** com 18 px e centrada (`paint.rs`, `island_small`).
- **Clique na capa ou nas barras** abre a ilha na aba Música (`pill::spotify` devolve o clique). Teste:
  `a_click_on_the_songs_cover_opens_the_island_on_the_music`.
- **Luna com a cor da capa** (`luna_colour` em `groove/mod.rs`) e dançando também com agente trabalhando ou terminado
  (`paint::dances_through`); só pergunta, permissão ou erro a param.
- **Aba Claude vazia** sabe se os hooks estão instalados (`home.rs`, `empty_words` e `hooks_installed`, que relê o
  `settings.json` a cada 3 s).
- **Aba Música:** sem tooltip nos botões do player (só hover; o nome fica para leitores de tela) e um botão "Abrir
  Ajustes › Integrações" nas mensagens que mandam conectar a conta (`music.rs`, `needs`).
- **Ajustes pela ilha:** abrir os Ajustes do app (engrenagem › Ajustes…, ou o botão da aba Música) devolve a ilha à
  pílula compacta e fecha a página da engrenagem (`actions.rs`, `music.rs`). **Falta o teste disso.**
- **Iniciar com o sistema** nos três sistemas: Windows pelo registro (`Run`), Linux por `~/.config/autostart`, macOS por
  LaunchAgent (`app/system/run_at_login.rs`), em Ajustes › Geral e no MCP (`set_setting run_at_login`). No Linux, os
  testes e o clippy passaram pelo WSL.
- **Spotify › Criar app…**: abre `https://developer.spotify.com/dashboard/create`, onde nasce o Client ID
  (`ui/settings/integrations.rs`, `spotify/account.rs::CREATE_APP`).
- **Troca de música:** a escuta zera a análise do som quando a música muda (`Dancer::new_song`, `Shared::new_song`,
  `capture.rs`). Antes as barras ficavam erradas por uns 20 s, medidas contra o pico de volume da música anterior.
  Teste: `a_new_song_tells_the_listening_to_start_over`.
- **Transição e degradê** (feito pelo opencode, revisado): a capa e as cores antigas ficam até a capa nova chegar e
  passam para as novas num fade de 0,3 s; as barras vão de uma cor da capa a outra (`spotify/mod.rs`: `accents`,
  `CoverFade`; `pill.rs`: `picture_over`). A Luna mantém a cor antiga até a nova chegar.
- **Volume do Spotify no Windows** (feito pelo opencode, revisado): pelo mixer do Windows (Core Audio, a sessão do
  `Spotify.exe`), em `spotify/smtc.rs`; a rota em `playback.rs` passou a `Route::Player`. Só funciona enquanto o
  Spotify está tocando som. Não foi testado com o Spotify de verdade:
  `cargo test --release the_spotify_open_here_is_read -- --ignored --nocapture`.

## O que falta fazer

### 1. Expressões da Luna com a música
A Luna (chama azul com olhos) dança com a música: `src/app/mascot/mago/dance.rs` (as poses por `Dance`), `motion.rs`,
`draw.rs` e `mod.rs` (`paint_mago_dancing`). O som chega em `Groove` (`spotify/groove/mod.rs`): `bpm`, `phase` (0 na
batida), `energy`, `bass`, `bands`, `dance`, `beat`, `since`. O dono quer **mais expressões** enquanto ela dança: olhos
e boca que mudam com a música (olhos fechados curtindo num trecho calmo, sorriso aberto na batida forte, piscadas no
ritmo, um "uau" num pico de energia), sem perder as poses de hoje. Deixe testes em `mascot/mago/tests.rs`.

### 2. Aba Claude: sessões do canvas e de fora do app
Hoje a aba Claude (`Tab::Claude`, ícone asterisco) mostra só as sessões do Claude Code **fora** do app (as dos hooks
externos: `outside_pills`, `maestro/outside.rs`, `maestro/demands.rs`). O dono quer **as duas**: as do canvas
(terminais do app rodando Claude Code e chats do app com agente claude) e as de fora. Cada linha diz de onde é ("no
canvas" ou "fora do app"), e o clique numa do canvas leva até ela (reaproveite o que as abas Início e Conversas fazem).
Textos: dica da aba "Claude Code: as sessões no canvas e fora do app"; vazio "Nenhuma sessão do Claude Code", mantendo
a dica dos hooks (`empty_words`); e a descrição de `island_page` claude em `mcp/app_tools.rs`. Teste em
`app/tests/island.rs`: um agente claude do canvas aparece na aba Claude.

### 3. Conversa da ilha: texto selecionável
Na página de conversa (a que mostra as mensagens de uma sessão, com "Mensagem para esta sessão…" e "Ir ao terminal"),
as mensagens do agente **e as da pessoa** precisam poder ser selecionadas com o mouse e copiadas (Ctrl+C). Hoje são só
pintadas. Use texto selecionável do egui sem perder o visual (cores, fonte, balões, quebra de linha).

### 4. Recentes da aba Início abrem o chat
Na aba Início, a lista à direita: clicar numa linha que é um chat ou sessão precisa **abrir a conversa dela na hora**,
não só destacar. Veja `feed.rs`, `home.rs` e `out.focus`/`out.tab`/`out.wake` em `actions.rs`. Teste do clique.

### 5. Claude Desktop × Claude Code (CLI)
As sessões de fora do app devem dizer de onde vêm. O hook já marca: `maestro/relay.rs::tag` põe
`mago_origin = "claude-desktop"` quando `CLAUDE_CODE_ENTRYPOINT=claude-desktop` (conferido: a aba Code do Claude
Desktop manda exatamente esse valor), e `maestro/outside.rs::origin` dá à pílula o nome "Claude Desktop". Mesmo assim, na
ilha as sessões do Claude Desktop aparecem como "Claude Code · <pasta>". Descubra de onde vêm essas linhas (pista:
`src/transcript/` lê `~/.claude/sessions/*.json`, que têm `entrypoint` "cli" ou "claude-desktop"; veja
`transcript/mod.rs` por volta da linha 201 e quem monta as linhas do feed) e faça cada linha dizer
"Claude Desktop · <pasta>" ou "Claude Code · <pasta>". É como o Coucou (app de referência) faz: a origem vem do
`CLAUDE_CODE_ENTRYPOINT` que o hook herda do processo do Claude. Teste da origem no nome.

### 6. Teste que ficou faltando
Abrir os Ajustes do app pela ilha devolve a ilha à pílula compacta (`out.open_app_settings` em `actions.rs`).

## Depois de tudo

O dono quer: **merge do PR e release do Linux primeiro (prioridade), depois a do Windows.** A release sai por tag `v*`
(o CI gera o MSI e o tar.gz/deb num rascunho de release); a tag precisa bater com a `version` do `Cargo.toml` (hoje
0.0.21, já publicada: suba para 0.0.22).

## Contexto da conversa (para não repetir erros)

- **Ponte dos hooks e o Claude Desktop.** O Claude Desktop é um app empacotado (MSIX). Um processo aberto de dentro dele
  grava em `%APPDATA%` numa cópia virtual (`%LOCALAPPDATA%\Packages\Claude_pzs8sxrjxfjjc\LocalCache\Roaming\…`), e os
  hooks das sessões do Claude Desktop leem essa cópia. Um build do MagoAgent aberto de dentro do Claude deixou lá um
  `bridge.json` velho, de um app já fechado, e as perguntas (AskUserQuestion) se perdiam ("Falhou" ou "[No preference]").
  Abra os builds de teste **pelo Explorer** (`explorer.exe arquivo.cmd`), nunca direto do terminal do agente. Um build
  compilado no repo usa os dados em `MagoAgent-dev`; para ele atender os hooks, abra com
  `MAGOAGENT_HOOK_ENDPOINT=%APPDATA%\MagoAgent\bridge.json`.
  Possível melhoria: o relay ignorar um `bridge.json` cujo `pid` já morreu, em vez de mandar para lá.
- **Não mate o app do dono.** Pare só o processo cujo `ExecutablePath` é o do seu build.
- **Teste visual numa cena de teste** (`MAGOAGENT_SCENE`); nessa cena o app, de propósito, não atende os hooks.
- **WSL:** o `wsl.exe` expande `$variáveis`; rode scripts por arquivo (`MSYS_NO_PATHCONV=1 wsl.exe bash /mnt/c/...`),
  sem CRLF. O WSLg não faz janela sempre por cima nem transparência: a janela da ilha não dá para testar lá.
- **macOS** não compila no Windows (falta o `cc` para o objc2): fica sem conferir.
- **Uma queda do Windows zerou arquivos não commitados** (bytes NUL). Commite com frequência.
- **Pedidos de estilo do dono para a ilha:** Luna à esquerda, capa e frequência no meio, mini agentes à direita; sem
  mini agentes, a capa e a frequência vão para a direita. As barras crescem a partir da linha do meio, seguem a música
  de verdade (bandas do som), são finas e pouco sensíveis, no estilo da Dynamic Island do iPhone. O tempo para a
  compacta virar notch é configurável (Ajustes › Geral, `island_compact_after`).
- **A ilha lateral foi tirada de propósito** (commit `142b257`): perto de uma borda ela fica como pílula livre.

## Pausa de 16/11 à noite

O trabalho dos agentes ficou em dois ramos, ainda não juntados ao `feat/ilha-pendentes`:
- `pend-deepseek`: item 2 pronto (commit da aba Claude) e um commit "WIP" com o item 4 pela metade (e o que mais os agentes deixaram), sem testar.
- `pend-qwen`: um commit "WIP" com o item 1 (expressões) pela metade, sem testar.
Para continuar: junte os dois ramos neste, confira os WIP (`cargo test`, clippy), termine os itens 1, 3, 4, 5 e 6, e abra o PR deste ramo para o `feat/ilha-abas` (sem merge: o dono valida antes). O OmniRoute (`omniroute serve`) precisa estar ligado para o opencode usar `omniroute/livre`; o plano grátis do opencode estourou.
