# Roteiro de teste no Linux

Nota para o agente que valida o MagoAgent num Linux de verdade (máquina física, não WSL). O objetivo é passar por
**todos os pontos do app** e devolver um relatório. Leia antes o `AGENTS.md` (regras e o roteiro `MAGOAGENT_SCRIPT`).

## Regras

- Teste o ramo `porte-referencia` (PR #45). Anote o commit testado (`git log --oneline -1`) no topo do relatório.
- Use sempre uma **cena de teste** (`MAGOAGENT_SCENE=/tmp/mago-teste/teste.mago`), nunca o desenho real do usuário.
  A cena leva junto as preferências, os chats e as conexões; o resto da pasta de dados (`~/.local/share/magoagent/`)
  é real: não apague nada dela.
- Não escreva em `~/.claude/settings.json`, `~/.codex` nem `~/.hermes` (ler pode). Para os hooks, use um
  `--settings` próprio do teste (abaixo).
- Prefira o roteiro (`MAGOAGENT_SCRIPT`: `size`, `click`, `drag`, `type`, `key`, `wait`, `shot`, `quit`) a mexer no
  mouse e no teclado de quem está usando a máquina. No Wayland o `size` é ignorado; o `shot` grava o PNG mesmo assim.
- Não peça nem digite senha de `sudo`: se faltar pacote, escreva o comando no relatório para a pessoa rodar.
- Cada item recebe **PASSOU**, **FALHOU**, **PARCIAL** ou **NÃO TESTADO**, com a evidência (captura, saída de
  comando, log) e, se falhou, como reproduzir. Não conserte nada sem dizer; se consertar, um commit por correção, sem
  linha de coautor, e liste os commits.

## Preparar

```bash
sudo apt-get install -y pkg-config libgtk-3-dev libxdo-dev libayatana-appindicator3-dev libasound2-dev \
  libdbus-1-dev libxkbcommon-dev libwayland-dev libssl-dev        # Debian/Ubuntu; em outra distro, os equivalentes
git clone -b porte-referencia https://github.com/edsuuu/magoagent && cd magoagent
cargo test --release                                              # tudo tem de passar, sem avisos
cargo clippy --all-targets --release -- -D warnings
rustfmt --edition 2024 --check $(find src -name "*.rs")
cargo build --release
mkdir -p /tmp/mago-teste
printf '{"version":1,"elements":[],"background":[255,255,255,255]}' > /tmp/mago-teste/teste.mago
MAGOAGENT_SCENE=/tmp/mago-teste/teste.mago MAGOAGENT_SCRIPT="size 1200 800; wait 3000; shot /tmp/mago-teste/abre.png; quit" target/release/magoagent
```

Anote a distro, o ambiente (GNOME/KDE…, X11 ou Wayland), a GPU e o tempo de build.

## O que testar

### 1. Abrir, fechar e dados
- Abre sem aviso no terminal; fecha pelo X e pelo menu; autosave (mexa, feche, abra: está lá).
- Pasta de dados em `~/.local/share/magoagent/`; preferências salvas e relidas.
- Bandeja do sistema (ícone, menu, pausar alertas). Iniciar com o sistema, se houver a opção.
- Atualizações: a verificação da release do GitHub não quebra (sem instalar nada).

### 2. Canvas
- Ferramentas da barra: seleção, mão, retângulo, elipse, losango, seta, linha, texto, desenho livre, imagem, borracha.
- Selecionar, mover, redimensionar, girar, agrupar (Grupo com quadro tracejado e pílula), copiar/colar, desfazer/refazer.
- Setas presas aos elementos; cordas (`rope`) entre cartões; minimapa; zoom e rolagem; grade.
- Notas adesivas, fichário em pilha (abas, Nova Página, Remover do Fichário), andares (Térreo embaixo), espaços.
- Exportar SVG e PNG; colar imagem e texto da área de transferência; arrastar arquivo para o canvas.

### 3. Terminais e agentes
- Novo Terminal; shell padrão do sistema; cores, seleção, colar, rolagem, busca, ampliar.
- Novo Chat (Claude Code e Codex, se instalados): enviar, resposta em tempo real, ferramentas, permissão, interromper.
- Variáveis no terminal: `MAGOAGENT_BRIDGE`, `MAGOAGENT_TOKEN`, `MAGOAGENT_TERMINAL_ID`; `magoagent list|ask|check|note`.
- Recrutar agente (Luna), com e sem `worktree: true`; teto de agentes; consumo do plano (pílula e Agentes › Uso).

### 4. Chat
- Lista de conversas, chat novo, composer, comandos com barra, anexos/imagens, painel Contexto, Ditar,
  "Abrir em miniatura" (mini chat), modelos e provedores, permissão seguindo o `defaultMode` do agente.

### 5. Árvore de Arquivos e editor
- Abrir pasta, navegar, criar/renomear/apagar (para a lixeira), abrir arquivo, realce de sintaxe, Localizar
  (Ctrl+Shift+F com prévia), erros de sintaxe, diff estilo IntelliJ, blame, ramos, alterações, conflitos, banco.
- LSP: se houver servidor instalado (rust-analyzer…), avisos aparecem.

### 6. Navegador (portal)
- O README diz que portais **ainda não abrem no Linux**: confirme o comportamento (mensagem clara, sem travar).

### 7. Ilha (pílula no topo da tela)
Ligue em Ajustes (`Prefs.island`). É uma janela própria por cima de tudo; no Wayland a posição pode ser do compositor.
- Intro ao abrir: o mago cai no centro, acena à esquerda, volta ao centro, o brilho fica azul e a ilha encolhe para a
  compacta com o mago voando até a orelha (~5,3 s). Grave em vídeo ou em capturas seguidas.
- Abrir com um clique (cresce na mola), fechar sozinha depois do "Fechar auto" (padrão 15 s), barra de contagem nos
  últimos segundos, fechar pelo ✕ e pelo Esc.
- Abas na ordem Início, Chat atual, Conversas e +; a página nova entra com fade e de 97% do tamanho.
- Compacta: um mini por agente, com o selo de estado (azul trabalhando, âmbar esperando, ciano perguntando,
  verde pronto, vermelho erro).
- Ajustes da ilha (engrenagem): som, volume, Fechar auto 5/10/15/30 s, Ajustes….
- Click-through: fora da ilha o clique passa para a janela de baixo; sobre ela, a ilha recebe o clique.
- Hooks do Claude Code, com settings próprio do teste (não toque no `~/.claude/settings.json`):
  crie `/tmp/mago-teste/hooks.json` com os eventos (SessionStart, UserPromptSubmit, PreToolUse, PostToolUse,
  PermissionRequest, Notification, Stop, SubagentStop, SessionEnd…) apontando para
  `<repo>/target/release/magoagent hook external` (e `hook external wait` em PermissionRequest e no PreToolUse de
  AskUserQuestion, com `timeout` 600), rode o app com `MAGOAGENT_HOOK_ENDPOINT=/tmp/mago-teste/hook-bridge.json` e:
  - `claude -p --settings /tmp/mago-teste/hooks.json --permission-mode default "rode echo oi > /tmp/mago-teste/x.txt"`:
    o cartão de permissão abre a ilha sozinho; **Permitir** cria o arquivo; repita com **Negar**.
  - `claude --settings /tmp/mago-teste/hooks.json` interativo pedindo uma AskUserQuestion com 3 opções: o cartão
    mostra as opções; um clique responde (anote se o primeiro clique se perde).
  - A sessão de fora aparece como pílula e some no SessionEnd.

### 8. Integrações (Ajustes › Integrações)
- GitHub (token no chaveiro do sistema: Secret Service/libsecret), Spotify (sem o app instalado deve dizer
  "O Spotify não está instalado neste computador."; com snap/flatpak, achar o app), Hermes, WhatsApp.

### 9. Voz
- Microfone (cpal/ALSA/PipeWire), ditado, chamada com a Luna; voz pela OpenAI só com chave. Nada é baixado.

### 10. MCP e ponte
- `magoagent mcp`: `initialize`, `tools/list` (conte as ferramentas; o README diz 95), `tools/call` de algumas
  (adicionar elemento, ler a cena, abrir chat, ajustes). O desenho aparece traço a traço no app.
- `claude mcp add magoagent -- <repo>/target/release/magoagent mcp` num Claude Code de teste e uma chamada real.
- Teste da ponte: `cargo test --release bridge_asks_an_agent_over_tcp`.

### 11. Instalação (.deb e apt)
- `sh installer/build-deb.sh` (precisa de `dpkg-dev`): gera `target/release/magoagent_<versão>_amd64.deb`. Confira
  `dpkg-deb -I` (dependências calculadas, descrição) e `dpkg-deb -c` (`/usr/bin/magoagent`, o `.desktop`, o ícone).
- A pessoa instala com `sudo apt install ./target/release/magoagent_<versão>_amd64.deb` (escreva o comando, não
  digite senha): o app aparece no menu de aplicativos com o ícone, abre por `magoagent` no terminal, os dados ficam em
  `~/.local/share/magoagent`, e `sudo apt remove magoagent` o tira sem apagar os dados.
- Em Ajustes › Sobre, com uma versão mais nova publicada, o app instalado em `/usr/bin` manda usar o apt em vez de
  se atualizar.
- Repositório apt pelo GitHub (`docs/apt.md`): se o dono já publicou uma release com o `pages.yml` configurado, teste a
  instalação pelos três comandos de lá; se não, anote como NÃO TESTADO.
- `magoagent instalar` no Linux: o que faz hoje (atalho em `~/.local/share/applications`).

### 12. Desempenho
- CPU e memória com o app parado, com a ilha compacta e com ela aberta (`top`/`pidstat` por 20 s cada).
- Fluidez da ilha (quadros por segundo aproximados) e do canvas com muitos elementos.

## Em andamento (ainda não no ramo quando esta nota foi escrita)

Ramos sendo feitos em paralelo, que entram no `porte-referencia` depois de revisados; se já estiverem lá, teste
também:
- `ilha-mascote-claude`: mascote do Claude Code na ilha quando a sessão/pergunta é dele.
- `ajustes-design`: Ajustes no design novo (Geral, Agentes, Chat, Ok Mago, Integrações, Email via MCP do Gmail,
  Dicionário), ligados de ponta a ponta.
- `ilha-primeiro-clique`: o primeiro clique numa opção de pergunta que se perdia.
- `ilha-mago-quadros`: o mago da ilha compacta animando a cada quadro sem repintar o app inteiro.
- `atualizar-seguro`: a atualização em 4 etapas (salva o trabalho, para os processos, atualiza, reabre) com
  conferência do sha256; no Linux instalado pelo apt o app só manda usar o apt.

## Relatório

Um arquivo `docs/relatorio-linux-<data>.md` (ou a resposta do agente) com: ambiente, commit, a tabela
item → PASSOU/FALHOU/PARCIAL/NÃO TESTADO → evidência, os bugs com passos para reproduzir, e o que não deu para testar
e por quê.
