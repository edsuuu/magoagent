<p align="center">
<img src="assets/icon.png" width="120" alt="MagoAgent">
</p>

# MagoAgent

**Um canvas para orquestrar agentes de IA.** Cada agente (Claude Code, Codex ou qualquer CLI) roda
num terminal de verdade em cima de um quadro infinito. Você liga os agentes uns aos outros, e às
notas, páginas e projetos de que eles precisam, puxando uma corda. Eles enxergam o que está ligado,
conversam entre si, recrutam ajudantes e avisam quando precisam de você. O app é nativo em Rust
(`eframe/egui`), sem Electron, e roda em macOS, Windows e Linux.

## Arquitetura

![Arquitetura do MagoAgent](docs/arquitetura.png)

## O que faz

- **Canvas:** formas com traço à mão, setas presas às formas, texto, imagens, camadas, minimapa e exportação PNG/SVG. É onde você desenha o plano e o agente desenha a resposta.
- **Terminais (agentes):** shells de verdade (zsh, bash, PowerShell) com cores, acentos, Claude Code ou Codex rodando dentro. Cada terminal tem nome, cor e pasta próprios.
- **Conexões:** uma corda entre duas coisas do canvas diz quem fala com quem. Pelo terminal, o `magoagent` lista conexões, manda prompts, lê telas, controla portais e monta equipes.
- **Notas adesivas:** Markdown em pilha; notas ligadas formam correntes acessíveis por clique.
- **Portais:** navegador (WebKit no macOS, WebView2 no Windows) sobre o canvas. O agente circula trechos da página e recebe coordenadas, elementos e comentários.
- **Árvore de arquivos:** pasta virada card com git (ramos, diff, autor), painel de código realce e Localizar.
- **Chamada de voz:** janela pequena por cima de tudo (microfone, alto-falante, orbe). A voz pode ser do Codex (tempo real), da API da OpenAI ou de um servidor VoiceStudio seu.
- **Ilha do Maestro:** pílula fixa no topo da tela com resumo dos agentes, perguntas pendentes e chat novo.

![O canvas: um Maestro ligado ao plano da sprint e a dois agentes](docs/canvas.png)

## Instalar e executar

```sh
cargo run --release       # roda direto
cargo test                # testes (gestos, arquivos, exportação, MCP, ponte)
./build-macos-app.sh      # macOS: gera e abre o MagoAgent.app
```

- **Windows:** use o `MagoAgent-<versão>.msi` da release do GitHub. Instala em `C:\Program Files\MagoAgent`. Ajustes › Sobre › Atualizar baixa o MSI novo. Dados ficam em `data/` dentro da pasta de instalação.
- **Linux (Debian/Ubuntu):** pelo repositório apt do projeto ([docs/apt.md](docs/apt.md)), ou o `.deb` da release; atualiza pelo apt. Adicione o repositório uma vez e instale:

  ```sh
  curl -fsSL https://edsuuu.github.io/magoagent/apt/magoagent.gpg | sudo tee /usr/share/keyrings/magoagent.gpg > /dev/null
  echo "deb [arch=amd64 signed-by=/usr/share/keyrings/magoagent.gpg] https://edsuuu.github.io/magoagent/apt ./" | sudo tee /etc/apt/sources.list.d/magoagent.list
  sudo apt update && sudo apt install magoagent
  ```

- **macOS e Linux:** dados em `~/Library/Application Support/MagoAgent/` (macOS) ou `~/.local/share/magoagent/` (Linux).
- **Linux:** o diálogo de arquivos usa xdg-desktop-portal; portais ainda não abrem no Linux.

## A ilha do Maestro

A pílula flutuante (`Prefs.island`) mostra em tempo real quem trabalha, quem espera aprovação e quem está parado. De dentro dela você:

- inicia um chat novo com Claude Code ou Codex (sem terminal);
- responde perguntas pendentes dos agentes;
- vê chamadas de voz ativas e controla microfone/alto-falante;
- muda entre terminais e abas de portal.

## O MCP

`magoagent mcp` é um servidor [MCP](https://modelcontextprotocol.io) (stdio, JSON-RPC 2.0). Com ele, qualquer IA:

- lê e desenha no canvas (`get_scene`, `add_elements`, `update_elements`, `delete_elements`, `view`);
- fala com agentes (`ask_agent`, `read_terminal`), lê/escreve notas;
- usa portais (`portal_navigate`, `portal_click`, `portal_fill`);
- monta equipes (`recruit_agent`, `create_portal`, `connect`);
- controla o app (arquivos, janelas, git, abas, ajustes).

Todas as ferramentas, com os parâmetros, em [docs/mcp.md](docs/mcp.md). Modelos locais (Ollama) e o OmniRoute para o opencode: [docs/llm-local.md](docs/llm-local.md).

### Configuração

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

Claude Desktop, Cursor e outros:

```json
{ "mcpServers": { "magoagent": { "command": "/caminho/para/magoagent", "args": ["mcp"] } } }
```

## CLI dos agentes (dentro do canvas)

Dentro de qualquer terminal, o `magoagent` já está no PATH:

| Comando | O que faz |
| --- | --- |
| `magoagent list` | conexões deste terminal |
| `magoagent ask <nome> <prompt>` | manda prompt ao agente e espera resposta |
| `magoagent check <nome>` | lê a tela do agente sem enviar nada |
| `magoagent note read/write/create` | gerencia notas ligadas |
| `magoagent portal navigate/text/eval/click/fill` | usa portais ligados |
| `magoagent recruit [comando]` | cria um agente abaixo de você (padrão: claude) |
| `magoagent connect <a> [b]` | liga `<a>` a `<b>` (ou a este terminal) |
| `magoagent notify <texto>` | avisa a pessoa no app e na notificação do sistema |

## Primeiro passo rápido

1. Clone e rode: `cargo run --release`.
2. Abra **Painel de agentes** (Ctrl/⌘+J), inicie um terminal com Claude Code ou Codex.
3. No terminal, rode `magoagent recruit` para criar outro agente ligado ao primeiro.
4. A Ilha do Maestro mostra quem está trabalhando; use-a para dar ordens por voz ou texto.

## Compilar do código

```sh
# Formatação
rustfmt --edition 2024 --check $(find src -name "*.rs")

# Lints
cargo clippy --all-targets --release -- -D warnings

# Testes
cargo test --release

# Checar outros alvos
cargo check --release --target x86_64-unknown-linux-gnu
cargo check --release --target x86_64-pc-windows-msvc
cargo check --release --target aarch64-apple-darwin
```

O guia de quem mexe no código (estrutura, fluxo de trabalho e testes) fica no [`AGENTS.md`](AGENTS.md).

## Licenças

As fontes em `assets/` são OFL (`Excalifont-license.txt`, `Assistant-license.txt`). Os ícones em `assets/editor-icons/` são da JetBrains (Apache 2.0; veja `NOTICE`). O código da Árvore de Arquivos usa Inter e JetBrains Mono de uma instalação JetBrains local, quando houver.
