# MagoAgent

**Um canvas para orquestrar agentes de IA.** Cada agente (Claude Code, Codex ou qualquer CLI) roda
num terminal de verdade em cima de um quadro infinito. Você liga os agentes uns aos outros, e às
notas, páginas e projetos de que eles precisam, puxando uma corda. Eles enxergam o que está ligado,
conversam entre si, recrutam ajudantes e avisam quando precisam de você. O app é nativo em Rust
(`eframe/egui`), sem Electron, e roda em macOS, Windows e Linux.

![O canvas: um Maestro ligado ao plano da sprint e a dois agentes; o Back-end, à árvore de arquivos do projeto](docs/canvas.png)

*O Maestro lê o plano da sprint (`magoagent note read`) e vê quem está ligado a ele
(`magoagent list`). O Back-end enxerga a árvore de arquivos do projeto ligada a ele.*

## A ideia

Um agente sozinho num terminal não sabe dos outros. No MagoAgent o canvas é o mapa da equipe:

- **Agentes:** cada terminal é um agente com nome, pasta, comando e cor. O shell é de verdade
  (zsh, bash, PowerShell), com cores, acentos e o Claude Code ou o Codex rodando dentro.
- **Conexões:** uma corda entre duas coisas do canvas diz quem fala com quem. Dentro do terminal,
  o agente usa a CLI `magoagent` (ou as mesmas ferramentas pelo MCP) para:
  - mandar um prompt a outro agente e esperar a resposta;
  - ler a tela de outro agente;
  - ler e escrever notas;
  - navegar e clicar em páginas.
- **Maestro:** um agente Maestro monta o próprio time. Ele recruta agentes novos abaixo dele, abre
  portais, liga as peças e manda notificações para você.
- **Você no comando:** o painel de agentes mostra quem trabalha, quem está parado e quem espera uma
  aprovação. O chat mostra a conversa de cada um, e a chamada de voz deixa você dar ordens falando.

![O painel de agentes (Ctrl/⌘+J) sobre o canvas](docs/agentes.png)

## O que tem no canvas

- **Terminais (agentes):**
  - atalhos para Claude Code, Codex e Shell;
  - compositor de prompts e barra de contexto (editar, conectar, reiniciar, ampliar, apagar);
  - clique duplo no título amplia; "Iniciar no modo de chat" abre a conversa ao clicar;
  - "Descarregar" encerra o shell e guarda o lugar.
- **Notas adesivas:** o plano, a especificação, o checklist, tudo com Markdown. Notas ligadas a
  notas formam uma corrente: quem alcança a primeira alcança todas.
- **Portais:** um navegador com abas sobre o canvas (WebKit no macOS, WebView2 no Windows; no Linux
  ainda não abre). Quando o agente clica ou digita na página, o cursor dele aparece lá com o nome
  do terminal. Com o lápis da barra você circula algo na página, escreve o que mudar, e o agente
  ligado recebe o endereço, a região, os elementos e o comentário.
- **Árvore de arquivos:** uma pasta vira um card com a árvore, o ramo do git e um painel de código ao lado.
  - **Código:** abas, realce de sintaxe, usos e autor sobre cada declaração, Ctrl+clique e
    Localizar nos arquivos (Ctrl+Shift+F).
  - **Git:** os arquivos alterados, o diff contra o HEAD (Ctrl+D), o autor de cada linha e a troca de ramo.
  - **Claude Code:** o botão da pílula abre um terminal com o Claude Code na pasta, já ligado à
    árvore; Vincular liga a árvore a um terminal ou chat que já existe.
- **Quadro branco:** formas com traço à mão, setas presas às formas, texto, imagens, quadros,
  desenho livre, bloqueio, camadas, minimapa e exportação PNG/SVG. É onde você desenha o plano e o
  agente desenha a resposta.

![A árvore de arquivos com o painel de código aberto](docs/arvore.png)

## Três telas e a voz

| Atalho | Tela |
| --- | --- |
| Ctrl/⌘+1 | **Chat:** a conversa do agente como num app de chat (Markdown, código, ferramentas usadas, imagens). Também abre chats diretos com o Claude Code ou o Codex, sem terminal. |
| Ctrl/⌘+2 | **Canvas:** o mapa da equipe. |
| Ctrl/⌘+3 | **Árvore de Arquivos:** a primeira do canvas, ampliada (um aviso se não houver). |
| Ctrl/⌘+J | **Painel de agentes:** estado de cada um. Ctrl/⌘+Shift+A vai ao próximo que precisa de você. |

**Chamada de voz:** a chamada fica numa janela pequena por cima de tudo, que você arrasta para onde
quiser. Nela ficam o microfone, os alto-falantes e o mascote do agente (ou o orbe), que mostra se
ele está ouvindo, pensando ou falando; o lápis abre o chat num popup. A voz pode ser do Codex (tempo
real, com o login do ChatGPT), da API da OpenAI ou de um servidor VoiceStudio seu (o app não baixa modelos de voz).
O que você pede por voz vira ação no app: abrir uma pasta, um chat, um portal ou um arquivo.

## A CLI dos agentes

Dentro de qualquer terminal do canvas, o `magoagent` já está no PATH:

```text
magoagent list                         o que está conectado a este terminal
magoagent ask <nome> <prompt>          manda o prompt ao agente e espera ele terminar
magoagent check <nome>                 lê a tela do agente sem mandar nada
magoagent note read|write|create       lê, troca ou cria notas ligadas
magoagent portal navigate|text|eval|click|fill   usa um portal ligado
magoagent recruit [comando]            cria um agente abaixo de você, já ligado (padrão: claude)
magoagent portal create <url> [nome]   abre um portal à sua direita, ligado a você
magoagent connect <a> [b]              liga você a <a>, ou <a> a <b>
magoagent notify <texto>               avisa a pessoa (no app e numa notificação do sistema)
```

## MCP: qualquer IA usa o canvas

`magoagent mcp` é um servidor [MCP](https://modelcontextprotocol.io) (stdio). Com ele a IA:

- lê e desenha no canvas, e você vê o desenho aparecer traço a traço com o cursor e o nome do
  agente (Ctrl/⌘+Z desfaz);
- fala com os outros agentes: `list_agents`, `ask_agent`, `read_terminal`, `read_note`,
  `write_note`, `create_note`;
- usa os portais: `portal_navigate`, `portal_read`, `portal_eval`, `portal_click`, `portal_fill`;
- monta o time: `recruit_agent`, `create_portal`, `connect`, `notify`;
- faz tudo o que você faz no app: abrir pastas e arquivos na árvore, chats e janelas, git, abas, ajustes e áreas.

As ferramentas de desenho são `get_scene`, `add_elements`, `update_elements`, `delete_elements`,
`clear_canvas`, `view` (devolve um PNG para a IA conferir), `save_file`, `open_file` e
`set_background`. Elas funcionam com o app fechado. As dos agentes e as do app falam com o app
aberto por uma ponte local com token.

O comando do servidor é o próprio programa com o argumento `mcp`.

Claude Code:

```sh
claude mcp add magoagent -- /caminho/para/magoagent mcp
```

Codex (`~/.codex/config.toml`):

```toml
[mcp_servers.magoagent]
command = "/caminho/para/magoagent"
args = ["mcp"]
```

Claude Desktop, Cursor e outros (`mcpServers` no JSON de configuração):

```json
{ "mcpServers": { "magoagent": { "command": "/caminho/para/magoagent", "args": ["mcp"] } } }
```

## Arquitetura

```mermaid
flowchart TD

subgraph canvas["Canvas e agentes"]
  app["App do canvas<br/>src/app/mod.rs"]
  tools["Edição do canvas<br/>src/app/canvas/tool.rs"]
  agents["Estado dos agentes<br/>src/app/ui/agents.rs"]
  maestro["Maestro e equipes<br/>src/app/maestro/mod.rs"]
  terminals["Terminais dos agentes<br/>src/app/terminal/mod.rs"]
  pty["PTY<br/>src/terminal/mod.rs"]
  chat["Chat dos agentes<br/>src/app/chat/mod.rs"]
  notes["Notas adesivas<br/>src/app/notes/stack.rs"]
end

subgraph project["Projeto"]
  portals["Navegador<br/>src/app/browser/mod.rs"]
  webview["Página (wry)"]
  filetree["Árvore de Arquivos<br/>src/app/filetree/mod.rs"]
  compare["Diff do git<br/>src/app/filetree/compare/mod.rs"]
  problems["Problemas<br/>src/app/filetree/problems.rs"]
  editor["Biblioteca do editor<br/>src/editor/mod.rs"]
  text["Edição de texto<br/>src/editor/text.rs"]
  hints["Dicas de código<br/>src/editor/hints.rs"]
  merge["Merge do git<br/>src/editor/merge.rs"]
end

subgraph interfaces["Interfaces dos agentes"]
  server["Ponte local<br/>src/mcp/bridge/server.rs"]
  client["Cliente da ponte<br/>src/mcp/bridge/client.rs"]
  mcp["Servidor MCP<br/>src/mcp/mod.rs"]
  app_tools["Ferramentas do app<br/>src/mcp/app_tools.rs"]
end

subgraph core["Modelo e desenho"]
  model["Cena<br/>src/model/scene.rs"]
  geometry["Geometria<br/>src/geometry/mod.rs"]
  storage["Arquivo da cena<br/>src/file/mod.rs"]
  render["Pintura<br/>src/render/mod.rs"]
  replay["IA desenhando ao vivo<br/>src/render/replay.rs"]
  rough["Traço à mão<br/>src/render/rough.rs"]
  svg["Exportação SVG<br/>src/render/svg.rs"]
end

subgraph voice["Voz"]
  chamada["Chamada e ditado<br/>src/app/voice/mod.rs"]
  speech["Microfone e motores de voz<br/>src/voice/mod.rs"]
end

person(("Pessoa"))

person -->|usa| app
app -.->|edita| tools
app -->|mostra o estado| agents
app -->|hospeda| terminals
terminals -->|roda o shell| pty
app -->|conversas| chat
app -->|hospeda| notes
maestro -->|recruta| terminals
maestro -->|recruta| chat
terminals -->|CLI magoagent| client
chat -->|MCP e perguntas| server
client -.->|pedidos| server
server -->|ações| app
person -->|conecta um cliente de IA| mcp
mcp -->|despacha| app_tools
app_tools -->|controla| app
mcp -->|lê e grava| storage
app -->|hospeda| filetree
filetree -->|diff| compare
filetree -->|análise| problems
filetree -->|painel de código| editor
editor --> text
editor --> hints
editor --> merge
app -->|hospeda| portals
portals -.->|mostra| webview
app -.->|edita| model
model --> geometry
app -->|salva| storage
app -->|desenha| render
render --> replay
render --> rough
render --> svg
person -->|fala| chamada
chamada --> speech
chamada -->|aciona| app
```

## Instalar e executar

```sh
cargo run --release       # roda direto, em qualquer sistema
cargo test                # testes (gestos, arquivos, exportação, MCP, ponte dos agentes)
./build-macos-app.sh      # macOS: gera e abre o MagoAgent.app
```

- **Windows:** o instalador é o `MagoAgent-<versão>.msi` da release do GitHub. Ele pede
  administrador, instala em `C:\Program Files\MagoAgent`, cria o atalho no menu Iniciar e troca a
  versão anterior; Ajustes › Sobre › Atualizar baixa o MSI novo. Os desenhos, chats, preferências ficam
  em `data/`, dentro da pasta da instalação.
- **macOS e Linux:** antes de instalar, os dados ficam em `~/Library/Application Support/MagoAgent/`
  (macOS) ou `~/.local/share/magoagent/` (Linux).
- **Linux:** o diálogo de arquivos usa o portal do desktop (xdg-desktop-portal).

## Desenvolvimento

O guia de quem mexe no código (estrutura, fluxo de trabalho, testes e convenções) fica no
[`AGENTS.md`](AGENTS.md), o mesmo para pessoas e para os agentes de IA (Codex, Cursor e outros o
leem direto; o `CLAUDE.md` só o importa).

## Licenças

As fontes em `assets/` são OFL; veja `assets/Excalifont-license.txt` e
`assets/Assistant-license.txt`. Os ícones em `assets/editor-icons/` são da JetBrains (Apache 2.0);
veja `assets/editor-icons/NOTICE`. O código da Árvore de Arquivos usa Inter e JetBrains Mono de uma instalação JetBrains
local, quando houver.
