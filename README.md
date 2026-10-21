# MagoAgent

Quadro branco nativo em Rust (`eframe/egui`), sem navegador nem JavaScript, com traço à mão
estilo rough.js e um servidor [MCP](https://modelcontextprotocol.io) embutido para que qualquer
IA desenhe, planeje e veja o resultado.

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
- **Terminais no canvas:** shell de verdade (zsh, bash, PowerShell) com cores, acentos e Claude
  Code/Codex rodando dentro. Atalhos rápidos (Claude Code, Codex, Shell), pasta e comando; clique
  duplo para digitar, clique duplo no título para ampliar, compositor de prompts e barra de
  contexto (editar, conectar, reiniciar, ampliar, apagar).
- **Agentes conversando:** ligue terminais, notas e portais com o botão Conectar (uma corda com
  peso). Dentro de um terminal, `magoagent list`, `ask <nome> <prompt>`, `check <nome>`,
  `note read|write|create` e `portal navigate|text|eval|click|fill`; as mesmas funções aparecem no
  MCP (`list_agents`, `ask_agent`, `read_terminal`, `read_note`, `write_note`, `create_note`,
  `portal_navigate`, `portal_read`, `portal_eval`, `portal_click`, `portal_fill`). Quando o agente
  clica ou digita numa página, o cursor dele aparece lá com o nome do terminal.
- **Painel de agentes (⌘J):** todos os terminais numa tabela, com quem está trabalhando, quem
  está ocioso, quem terminou sem você ver e quem espera uma aprovação; clique para ir até ele.
- **Desenhos ligados a quem desenhou:** o que um agente desenha pelo MCP aparece com o cursor
  dele e fica preso por uma corda ao terminal de onde veio.
- **IDE no canvas:** um projeto (pasta) vira um card com a árvore e a branch; clique duplo abre o
  editor (o Ilha, estilo PhpStorm) ampliado: árvore com ícones JetBrains, abas, realce de sintaxe,
  marcas e diff do git, localizar no arquivo e nos arquivos, pesquisar em tudo (Shift duas vezes),
  ir para a declaração, problemas e commit/push/pull. O botão Claude Code abre um terminal ligado
  ao IDE: o Claude Code de lá se conecta a ele e as mudanças que propõe abrem como diff para
  aceitar ou rejeitar (um terminal ligado ao IDE depois de iniciado precisa de Reiniciar, ou de
  `/ide` dentro do claude). Soltar uma pasta no canvas também cria um IDE. Com um servidor de
  linguagem instalado (rust-analyzer, intelephense…): Ctrl/⌘+clique e ⌘B levam à declaração,
  Alt+F7 lista os usos, ⌘⌥B a implementação, o mouse parado mostra a documentação e `(` mostra
  os parâmetros. No diff (de uma proposta do Claude ou de uma alteração no git) dá para escrever
  um comentário: ele vai como mensagem ao agente ligado ao IDE, com o arquivo e as linhas
  ("Rejeitar e comentar" na proposta do Claude; "Comentar ao agente" numa alteração já gravada).
- **Banco de dados na IDE:** a janela Banco de dados acha os SQLite do projeto (o
  `database/database.sqlite` do Laravel, por exemplo), mostra tabelas, colunas e chave primária,
  "Ver dados" e um console SQL com o resultado em grade (⌘Enter). Abre só para leitura, pelo
  `sqlite3` da máquina.
- **Portais:** navegador com abas sobre o canvas, usando o WebKit do macOS ou o WebView2 do
  Windows (no Linux ainda não abre).
- **Arquivos:** formato próprio `.mago` (abre também o `.rea` antigo) e exporta PNG (1×–3×) e SVG. O salvamento automático (`autosave.mago`) e as preferências ficam em:
  macOS `~/Library/Application Support/MagoAgent/`, Windows `%APPDATA%\MagoAgent\`,
  Linux `~/.local/share/magoagent/` (ou `$XDG_DATA_HOME/magoagent/`).
- **Interface:** tema claro/escuro/sistema (o escuro inverte a luminosidade do desenho e mantém o tom das cores),
  minimapa, paleta de comandos (Cmd+/) e preferências (grade, encaixe, modo zen, modo de
  visualização, estatísticas), salvas em `preferences.json`.

## MCP: qualquer IA desenha no MagoAgent

`magoagent mcp` vira um servidor MCP (stdio). A IA lê e altera o desenho compartilhado (o
autosave, ou o arquivo em `MAGOAGENT_SCENE`): funciona com o app fechado, e com ele aberto você vê
a IA desenhando traço a traço, com um cursor e o nome do agente ("Claude Code", "Codex"…). Cmd+Z
desfaz o que a IA fez.

Ferramentas: `get_scene`, `add_elements`, `update_elements`, `delete_elements`, `clear_canvas`,
`view` (devolve um PNG para a IA conferir), `save_file`, `open_file`, `set_background` e `show_app`,
mais as dos agentes acima (elas falam com o app aberto pela ponte local).

Os elementos são objetos JSON (`type`, `x`, `y`, `width`, `height`, `strokeColor`,
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
`assets/Assistant-license.txt`. Os ícones em `assets/ide-icons/` são da JetBrains (Apache 2.0);
veja `assets/ide-icons/NOTICE`. O IDE usa Inter e JetBrains Mono de uma instalação JetBrains
local, quando houver.
