# MagoAgent

Quadro branco nativo em Rust (`eframe/egui`), sem navegador nem JavaScript, com traço à mão
estilo rough.js e um servidor [MCP](https://modelcontextprotocol.io) embutido para que qualquer
IA desenhe, planeje e veja o resultado. O visual, as fontes, os ícones e os atalhos seguem o
Excalidraw.

## Executar

Funciona em macOS, Windows e Linux.

```sh
cargo run                 # roda direto, em qualquer sistema
cargo build --release     # binário em target/release/magoagent (magoagent.exe no Windows)
./build-macos-app.sh      # macOS: gera e abre o MagoAgent.app
cargo test                # testes (gestos, arquivos, exportação, MCP)
```

No Linux, o diálogo de arquivos usa o portal do desktop (xdg-desktop-portal).

## Funções

- **Ferramentas:** mão, seleção, retângulo, losango, elipse, seta, linha, desenho livre, texto,
  nota adesiva (N) e borracha. No menu ⋮ ficam inserir imagem (9), quadro (F), desenhar para criar
  forma (Shift+X), ponteiro de laser (K), preenchimento (B) e seleção de laço.
- **Propriedades:** contorno, fundo, preenchimento (hachura, cruzada, sólido), espessura, estilo
  do traço, precisão, arestas, fonte, opacidade, camadas e ações.
- **Edição:** rotação pela alça, redimensionamento (Shift mantém proporção), agrupar, duplicar,
  copiar/colar e editor de pontos de linhas e setas.
- **Setas:** presas às formas (modos orbit e inside) e pontas configuráveis (flecha, triângulo,
  círculo, diamante, barra).
- **Quadros (cards):** levam junto o que está dentro ao mover.
- **Bloqueio:** com algo selecionado, o cadeado da barra (ou Cmd+Shift+L) bloqueia a seleção,
  levando junto o grupo inteiro e o conteúdo dos cards; sem seleção, o mesmo cadeado mantém a
  ferramenta ativa (Q). Clicar num bloqueado mostra um cadeado que desbloqueia; o menu ☰ lista
  todos os bloqueados.
- **Imagens:** PNG, JPEG, GIF, WebP e BMP, pelo menu ou arrastando do Finder.
- **Arquivos:** formato próprio `.mago` (abre também o `.rea` antigo), abre e salva `.excalidraw`,
  exporta PNG (1×–3×) e SVG. O salvamento automático (`autosave.mago`) e as preferências ficam em:
  macOS `~/Library/Application Support/MagoAgent/`, Windows `%APPDATA%\MagoAgent\`,
  Linux `~/.local/share/magoagent/` (ou `$XDG_DATA_HOME/magoagent/`).
- **Interface:** tema claro/escuro/sistema (o escuro usa o mesmo filtro de cor do Excalidraw),
  minimapa, paleta de comandos (Cmd+/) e preferências (grade, encaixe, modo zen, modo de
  visualização, estatísticas), salvas em `preferences.json`.

## MCP: qualquer IA desenha no MagoAgent

`magoagent mcp` vira um servidor MCP (stdio). A IA lê e altera o desenho compartilhado (o
autosave, ou o arquivo em `MAGOAGENT_SCENE`): funciona com o app fechado, e com ele aberto você vê
a IA desenhando traço a traço, com um cursor e o nome do agente ("Claude Code", "Codex"…). Cmd+Z
desfaz o que a IA fez.

Ferramentas: `get_scene`, `add_elements`, `update_elements`, `delete_elements`, `clear_canvas`,
`view` (devolve um PNG para a IA conferir), `save_file`, `open_file`, `set_background` e `show_app`.

Os elementos usam o formato do Excalidraw (`type`, `x`, `y`, `width`, `height`, `strokeColor`,
`backgroundColor`, `text`, `points`…), com atalhos: `label: {text}` centraliza um texto na forma
ou na seta, `start: {id}` / `end: {id}` prendem a seta às formas, e `type: "frame"` cria um card.

O comando do servidor é o próprio binário com o argumento `mcp`: no macOS
`MagoAgent.app/Contents/MacOS/magoagent`, no Windows `target\release\magoagent.exe`, no Linux
`target/release/magoagent`.

Claude Code:

```sh
claude mcp add magoagent -- "$PWD/MagoAgent.app/Contents/MacOS/magoagent" mcp
```

Codex (`~/.codex/config.toml`):

```toml
[mcp_servers.magoagent]
command = "/caminho/para/magoagent/MagoAgent.app/Contents/MacOS/magoagent"
args = ["mcp"]
```

Claude Desktop, Cursor e outros (`mcpServers` no JSON de configuração):

```json
{ "mcpServers": { "magoagent": { "command": "/caminho/para/magoagent/MagoAgent.app/Contents/MacOS/magoagent", "args": ["mcp"] } } }
```

## Ainda não feito

Seta em cotovelo, preenchimento de regiões formadas por várias linhas, recorte do conteúdo pela
borda do quadro, recorte de imagens, biblioteca e colaboração.

## Licenças

As fontes em `assets/` são OFL; veja `assets/hand-font-license.txt` e
`assets/Assistant-license.txt`.
