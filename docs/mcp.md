# O MCP do MagoAgent

<!-- gerado por docs/mcp.py a partir do tools/list do app: não edite à mão -->

O servidor MCP vem no próprio executável (`magoagent mcp`, magoagent 0.0.29) e tem **106 ferramentas**. Ele fala com o app aberto pela ponte local (com token) e com o desenho pelo arquivo da cena; o que a IA desenha aparece traço a traço com o nome do agente.

## Ligar

- **Chats do app**: já recebem o MCP sozinhos (`--mcp-config` no Claude Code, `-c mcp_servers…` no Codex).
- **Claude Code de fora**: `claude mcp add magoagent -- <caminho do magoagent> mcp`.
- **Codex**: em `~/.codex/config.toml`, `[mcp_servers.magoagent]` com `command = "<caminho do magoagent>"` e `args = ["mcp"]`.
- **opencode**: em `opencode.json`, `"mcp": { "magoagent": { "type": "local", "command": ["<caminho do magoagent>", "mcp"] } }`.
- **Testar sem cliente**: mande JSON-RPC pelo stdin de `magoagent mcp` (`initialize` com `clientInfo.name`, `tools/list`, `tools/call`).

Um terminal comum (que não é o da Luna) só mexe nas árvores ligadas a ele por corda; criar terminais, navegadores e conexões é da Luna e dos chats. O que uma ação abre para quem pediu (um navegador do `open_path`, por exemplo) fica ligado a ele por corda.

## Canvas e desenho

### `add_elements`

Adiciona elementos (objetos JSON com type, x, y, width, height, cores, text, points). Extras: `label: {text, fontSize?}` em formas e setas; `start: {id}` / `end: {id}` em setas para ligá-las a formas (os pontos são calculados sozinhos). Um terminal, navegador ou árvore de arquivos que você abre aqui (type terminal, portal, filetree) já nasce ligado a você por uma corda (ao seu terminal ou ao seu cartão de chat): list_agents o mostra e read_terminal lê a tela do terminal; posto em cima de outra janela ou nota, ele vai para o lugar livre mais perto de você. Para rodar testes ou uma API e acompanhar, prefira create_terminal. Devolve os ids criados (e em `linked` os ligados a você).

| Parâmetro | Tipo | O que é |
|---|---|---|
| `elements` (obrigatório) | array |  |

### `update_elements`

Altera elementos existentes: cada item tem `id` mais os campos a trocar (x, y, width, height, text, strokeColor, backgroundColor, angle, start/end...). Setas ligadas acompanham as formas movidas. Também move e redimensiona janelas (terminais, navegadores e árvores de arquivos) por x, y, width e height, sem mexer no que rodam.

| Parâmetro | Tipo | O que é |
|---|---|---|
| `elements` (obrigatório) | array |  |

### `delete_elements`

Apaga elementos pelos ids.

| Parâmetro | Tipo | O que é |
|---|---|---|
| `ids` (obrigatório) | array |  |

### `get_scene`

Lista os elementos do desenho atual (id, tipo, posição, tamanho, cores, texto, vínculos de setas) e a área ocupada.

Sem parâmetros.

### `clear_canvas`

Apaga todo o desenho.

Sem parâmetros.

### `set_background`

Muda a cor de fundo da tela (hex, ex.: #ffffff).

| Parâmetro | Tipo | O que é |
|---|---|---|
| `color` (obrigatório) | string |  |

### `view`

Renderiza o desenho (ou só os ids informados) como PNG para você ver como ficou.

| Parâmetro | Tipo | O que é |
|---|---|---|
| `ids` | array |  |
| `scale` | number | 1 a 3 (padrão 1) |

### `screenshot`

Um print da janela do MagoAgent como ela está agora (imagem), para ver e descrever à pessoa.

Sem parâmetros.

### `focus_canvas`

Mostra o canvas à pessoa centralizado no que ela quer ver: `at` são ids ou nomes (de terminais, navegadores, árvores de arquivos ou textos), separados por vírgula; vazio é o desenho todo. O zoom se ajusta para caber, ou use `zoom` (1 = 100%, 0.1 a 4).

| Parâmetro | Tipo | O que é |
|---|---|---|
| `at` | string |  |
| `zoom` | number |  |

### `canvas_drop`

Solta arquivos e pastas no canvas, como arrastá-los para lá: .md e .txt viram nota com o conteúdo, .html um navegador, pasta uma árvore de arquivos, imagem (inclusive SVG) uma imagem, e qualquer outro arquivo um cartão com nome, tamanho e botões que o abrem. `x` e `y` (coordenadas do canvas) dizem onde; sem eles, no meio da vista.

| Parâmetro | Tipo | O que é |
|---|---|---|
| `paths` (obrigatório) | array |  |
| `x` | number |  |
| `y` | number |  |

### `canvas_group`

Grupos do canvas (o quadro tracejado com nome em volta de vários elementos; o menu "Grupo"): `create` agrupa os `elements` (ids ou nomes de nós; 2 ou mais) com o `name` (padrão "Grupo"), `rename` dá o `name` ao `group`, `color` pinta o `group` com um papel de nota (`color`: Amarelo, Rosa, Azul…, ou `none` para sem cor), `ungroup` desfaz o `group` (os elementos ficam), `list` mostra os grupos, `tidy` é o Organizar (⇧Ctrl T): os `elements` (2 ou mais) numa grade alinhada; de novo com os mesmos, numa linha, depois numa coluna. `group` é o nome ou o id.

| Parâmetro | Tipo | O que é |
|---|---|---|
| `action` (obrigatório) | string: `list`, `create`, `rename`, `color`, `ungroup`, `tidy` |  |
| `elements` | array |  |
| `group` | string |  |
| `name` | string |  |
| `color` | string |  |

### `minimap`

Abre ou fecha o minimapa do canvas (o painel no canto de baixo à direita). Sem `open`, alterna.

| Parâmetro | Tipo | O que é |
|---|---|---|
| `open` | boolean |  |

### `sidebar`

A lateral dos espaços, como a pessoa a usa: `next` vai ao próximo agente que precisa dela (o botão "Ir para o próximo" do cartão Precisa de você, o mesmo que Ctrl+Shift+A: troca de espaço se preciso e centraliza o cartão dele); `agents_open` e `agents_close` abrem ou recolhem a seção Agentes deste espaço (fica salvo); `notice_hide` e `notice_show` ocultam ou trazem de volta o aviso verde "Nada esperando por você" (o × dele; fica salvo, e o cartão de quem espera aparece mesmo assim); `usage_open` e `usage_close` abrem ou fecham o detalhe do uso dos planos no rodapé; `status` diz quantos agentes precisam da pessoa (em todos os espaços) e lista os agentes deste espaço com o estado. Para centralizar um agente da lista, use `focus_canvas`.

| Parâmetro | Tipo | O que é |
|---|---|---|
| `action` (obrigatório) | string: `status`, `next`, `agents_open`, `agents_close`, `notice_hide`, `notice_show`, `usage_open`, `usage_close` |  |

### `panel`

O painel à direita da janela (o botão na barra de título): changes (Alterações: os arquivos mudados nas pastas das Árvores do canvas, com o diff colorido, a busca por nome ou trecho de código e a revisão), ao lado do canvas. As tarefas dos agentes ficam na ferramenta tasks, sem painel. `open` true abre, false fecha, sem `open` alterna; sem `name` e sem `open` (ou false) fecha o que estiver aberto. `width` muda a largura do painel, como puxar a borda esquerda dele (0 volta à padrão, 340). `worktree` abre Alterações num worktree dos repositórios (o nome da pasta, o ramo ou o caminho; todos volta às Árvores do canvas); a lista está em changes_review worktrees.

| Parâmetro | Tipo | O que é |
|---|---|---|
| `name` | string: `changes` |  |
| `open` | boolean |  |
| `width` | number | a largura do painel em pixels (de 300 até deixar 360 ao canvas); 0 volta à padrão |
| `worktree` | string | Alterações: o worktree a mostrar (nome da pasta, ramo ou caminho); todos: as Árvores do canvas |

### `changes_review`

A revisão do painel Alterações, como num pull request. worktrees lista os worktrees dos repositórios das Árvores do canvas (o principal e os dos agentes, em .magoagent/worktrees) com o ramo, o escolhido com *; com `worktree`, escolhe um (todos volta às Árvores). comment junta à revisão um comentário nas linhas `from` a `to` do arquivo alterado `path` (o caminho a partir da pasta, ou o fim dele), com o trecho do diff e o que está errado em `note`; list mostra os comentários numerados; remove tira o de número `index`; clear tira todos. send manda a revisão ao agente `agent` (nome ou id; vazio: o escolhido no painel ou o primeiro que trabalha na pasta) numa mensagem só: cada comentário com arquivo:linhas, o trecho e a nota, e a observação geral `note`; usa os comentários juntados ou, com `comments`, só esses (e os juntados ficam). pr pede ao agente uma análise de PR de todo o diff do worktree escolhido (ou das pastas das Árvores): bugs, riscos, testes faltando, por arquivo:linha. `worktree` em qualquer ação escolhe antes o worktree.

| Parâmetro | Tipo | O que é |
|---|---|---|
| `action` (obrigatório) | string: `worktrees`, `comment`, `list`, `remove`, `clear`, `send`, `pr` |  |
| `worktree` | string | o worktree (nome da pasta, ramo ou caminho); todos: as Árvores do canvas |
| `path` | string | comment: o arquivo alterado |
| `from` | integer | comment: a primeira linha (a primeira do arquivo é 1) |
| `to` | integer | comment: a última linha (vazio: só `from`) |
| `note` | string | comment: o que está errado; send: a observação geral |
| `index` | integer | remove: o número do comentário, como list mostra |
| `agent` | string | send, pr: o agente que recebe (nome ou id) |
| `comments` | array | send: os comentários a mandar no lugar dos juntados |

### `history`

Desfaz (undo) ou refaz (redo) a última mudança no canvas.

| Parâmetro | Tipo | O que é |
|---|---|---|
| `action` (obrigatório) | string: `undo`, `redo` |  |

### `open_file`

Substitui o desenho atual pelo conteúdo de um arquivo .mago.

| Parâmetro | Tipo | O que é |
|---|---|---|
| `path` (obrigatório) | string |  |

### `save_file`

Salva o desenho em um arquivo: .mago (formato do MagoAgent), .png ou .svg.

| Parâmetro | Tipo | O que é |
|---|---|---|
| `path` (obrigatório) | string |  |

## Notas e fichário

### `create_note`

Cria uma nota adesiva ligada ao seu terminal (só de dentro de um terminal do MagoAgent).

| Parâmetro | Tipo | O que é |
|---|---|---|
| `text` (obrigatório) | string |  |

### `read_note`

Lê uma nota adesiva conectada (markdown). O nome de uma nota é a primeira linha dela.

| Parâmetro | Tipo | O que é |
|---|---|---|
| `name` (obrigatório) | string |  |

### `write_note`

Troca o texto de uma nota conectada. Mudar a primeira linha muda o nome da nota.

| Parâmetro | Tipo | O que é |
|---|---|---|
| `name` (obrigatório) | string |  |
| `text` (obrigatório) | string |  |

### `note`

Cria uma nota no canvas (sem `id`) ou muda uma (com `id`): `text` (markdown), `title` (a primeira linha), `color` (Amarelo, Rosa, Azul, Verde, Laranja, Roxo, Branco, Carvão, Ardósia, Meia-noite; o texto acompanha), x, y, width, height e `buttons`: [{label, action, args}], botões no pé da nota que a pessoa clica para rodar uma ação do app (substituem os que ela tem).

| Parâmetro | Tipo | O que é |
|---|---|---|
| `id` | string |  |
| `text` | string |  |
| `title` | string |  |
| `color` | string |  |
| `x` | number |  |
| `y` | number |  |
| `width` | number |  |
| `height` | number |  |
| `buttons` | array |  |

### `note_buttons`

Define os botões de ação de uma nota (substituem os que ela tem; lista vazia tira todos). Cada botão tem `label`, `action` (o nome de uma ferramenta do app, como chat_new, open_path, ide_folder, chat_send) e `args` (os argumentos dela). Rodam só quando a pessoa clica; ação inválida mostra o erro num aviso.

| Parâmetro | Tipo | O que é |
|---|---|---|
| `id` (obrigatório) | string | o id da nota |
| `buttons` (obrigatório) | array |  |

### `note_stack`

Fichário (pilha de notas): `stack` junta as notas de `notes` numa pilha onde só a página da frente aparece, com uma aba por página à direita (se uma delas já está numa pilha, as outras entram nela); com a primeira nota de `notes`: `next` e `prev` viram a página, `show` traz aquela página para a frente, `new_page` cria uma página em branco na frente, `remove` tira aquela página da pilha e `unstack` espalha todas de volta. Valem para qualquer nota (a primeira de `notes`): `blur` desfoca o texto (Desfocar; `value` on/off) e `move_file` leva o arquivo .md dela para a `folder` (Mover Arquivo para…; `folder` vazia traz de volta).

| Parâmetro | Tipo | O que é |
|---|---|---|
| `action` (obrigatório) | string: `stack`, `unstack`, `next`, `prev`, `show`, `new_page`, `remove`, `uniform`, `blur`, `move_file` |  |
| `value` | string | blur: on, off (padrão: alterna) |
| `folder` | string | move_file: a pasta para onde vai o arquivo .md da nota (vazio: de volta à pasta do app) |
| `notes` (obrigatório) | array |  |

### `binder`

O fichário: notas com nome, em Markdown, guardadas entre as sessões (decisões, pendências, contexto de um projeto), para a Luna e os agentes lerem e escreverem. Não são as notas adesivas do canvas. list traz o nome, o tamanho e a primeira linha de cada nota; read lê uma; write grava (substitui) e append acrescenta ao fim, até 64 KB por nota; delete tira a nota do fichário e a guarda na pasta recuperados.

| Parâmetro | Tipo | O que é |
|---|---|---|
| `action` (obrigatório) | string: `list`, `read`, `write`, `append`, `delete` |  |
| `name` | string | o nome da nota (vira um nome de arquivo seguro: sem acentos, minúsculo) |
| `text` | string | write, append: o texto, em Markdown |

### `open_as_note`

Abre um arquivo de texto (.md, .markdown ou .txt, até 1 MB) no canvas numa nota com o conteúdo, como o clique no cartão do arquivo no chat: do tamanho de ler (560 de largura, alta como o texto até quase a altura da vista), ao lado do cartão do chat `chat` (ou no meio da vista), selecionada e com o canvas nela. O mesmo arquivo de novo vai à nota que já o tem. A nota é um editor (markdown) que a pessoa redimensiona pelas bordas e cantos e rola com a roda; para mudar o tamanho dela use note com o id, width e height.

| Parâmetro | Tipo | O que é |
|---|---|---|
| `path` (obrigatório) | string |  |
| `chat` | string | o chat ao lado de cujo cartão a nota abre; vazio: o chat aberto, ou o meio da vista |

## Agentes, terminais e a Luna

### `list_agents`

Lista os agentes (terminais e chats do canvas), notas e navegadores ligados a você por uma seta, com o estado de cada agente (trabalhando, parado, encerrado; um chat é a Luna e diz que é chat). Fora de um terminal do MagoAgent, ou de um chat ainda sem corda, lista todos.

Sem parâmetros.

### `ask_agent`

Manda um prompt a um agente conectado (outro terminal do canvas, ou um chat), espera ele terminar e devolve a tela dele (de um chat, a resposta; a pergunta entra na conversa dele com o seu nome e, se ele estiver ocupado, espera na fila). Se o tempo acabar antes, acompanhe com read_terminal; não reenvie o prompt. `raw: true` envia teclas cruas (\n Enter, \t Tab, \e Esc, \x03 Ctrl+C) para menus e comandos de shell (só a terminais).

| Parâmetro | Tipo | O que é |
|---|---|---|
| `name` (obrigatório) | string | o nome do agente, como list_agents mostra |
| `prompt` (obrigatório) | string |  |
| `raw` | boolean |  |
| `timeout` | number | segundos de espera (padrão 50) |

### `recruit_agent`

Cria um agente novo (um terminal) abaixo do seu (com o lugar tomado, no lugar livre mais perto de você), na mesma pasta, já ligado a você; ele ganha um nome (a resposta diz qual) e você fala com ele com ask_agent. Só de dentro de um terminal do MagoAgent. Com worktree true ele trabalha num ambiente próprio: um git worktree do seu repositório, num ramo dele, para dois agentes mexerem no mesmo código sem se atropelar (a resposta diz a pasta e o ramo; nada é juntado nem apagado sozinho).

| Parâmetro | Tipo | O que é |
|---|---|---|
| `command` | string | o que roda nele; padrão claude (ex.: codex) |
| `worktree` | boolean | true: num git worktree próprio, num ramo dele |
| `mode` | string: `terminal`, `chat` | terminal: Claude Code completo num terminal; chat: conversa leve num chat do app (usa o login do CLI, gasta bem menos memória); sem isto vale o ajuste Invocação dos agentes |

### `create_terminal`

Abre um terminal no canvas à sua direita (com o lugar tomado, no lugar livre mais perto de você; a resposta diz onde), já ligado a você por uma corda (ao seu terminal ou ao seu cartão de chat), rodando `command` como se fosse digitado (ex.: cargo test, npm run dev): use para rodar testes, uma API ou um servidor que você vai acompanhar. A resposta diz o nome dele. Ligado a você, ele aparece em list_agents com o estado (trabalhando, parado, encerrado), read_terminal lê a tela, ask_agent com raw manda teclas (\x03 para parar) e terminal_processes mostra o que roda nele. Sem corda não há terminal: só de dentro de um terminal da Luna ou de um chat do MagoAgent.

| Parâmetro | Tipo | O que é |
|---|---|---|
| `command` | string | o que roda nele; vazio: só o shell |
| `name` | string | opcional; padrão o programa do comando (cargo, npm…) |
| `cwd` | string | a pasta; padrão a sua |

### `connect`

Liga duas coisas do canvas com uma corda (agentes, chats, notas, navegadores), pelo nome. Só com `a`, liga você a `a`. O que está ligado se alcança: ask_agent, notas, navegadores. disconnect corta a corda.

| Parâmetro | Tipo | O que é |
|---|---|---|
| `a` (obrigatório) | string |  |
| `b` | string |  |

### `disconnect`

Corta a corda entre duas coisas do canvas (terminais, chats, notas, navegadores, árvores de arquivos), pelo nome como list_agents mostra: deixam de se alcançar. O contrário de connect.

| Parâmetro | Tipo | O que é |
|---|---|---|
| `a` (obrigatório) | string |  |
| `b` (obrigatório) | string |  |

### `connection_history`

O que dois agentes conversaram pela conexão entre eles (ask_agent / magoagent ask): as últimas mensagens, das mais antigas às mais novas, cada uma com quando, o sentido (quem → quem) e o resumo (a pergunta, ou como a resposta voltou).

| Parâmetro | Tipo | O que é |
|---|---|---|
| `a` (obrigatório) | string | o nome de um dos terminais |
| `b` (obrigatório) | string | o nome do outro |
| `limit` | number | quantas mensagens, no máximo (padrão 10, até 50) |

### `read_terminal`

Lê a tela atual de um agente ou terminal conectado (de um chat, o fim da conversa), sem mandar nada. Os terminais que você abre (create_terminal, add_elements) ficam conectados a você: é por aqui que você acompanha os testes ou a API que rodam neles.

| Parâmetro | Tipo | O que é |
|---|---|---|
| `name` (obrigatório) | string | o nome do agente, como list_agents mostra |

### `terminal_find`

Busca texto na tela e no histórico de um terminal (a barra Buscar no Terminal): find procura `query` (texto; com `regex` true, uma regex; letras minúsculas ignoram a caixa) e diz quantos resultados há, marcando-os no terminal; next e previous vão de um a outro; close fecha a barra.

| Parâmetro | Tipo | O que é |
|---|---|---|
| `name` (obrigatório) | string | o nome do terminal |
| `action` | string: `find`, `next`, `previous`, `close` |  |
| `query` | string |  |
| `regex` | boolean |  |

### `terminal_look`

O tema, o tamanho da fonte (só o tamanho: a fonte é sempre a Hack) e o shell de um terminal (`name`), ou os padrões de todos (sem `name`; também `copy_on_select`). `theme` é o id de um tema (padrão, system, nord, dracula…) ou inherit para voltar ao padrão; `font_size` vai de 8 a 32 pt; `shell` é o programa ou caminho (vazio: o padrão) e reinicia o terminal; `reset` devolve ao terminal os padrões; action list diz os temas e os shells que há.

| Parâmetro | Tipo | O que é |
|---|---|---|
| `name` | string | o nome do terminal; vazio: os padrões |
| `action` | string: `set`, `list` |  |
| `theme` | string |  |
| `font_size` | number |  |
| `shell` | string |  |
| `copy_on_select` | boolean | só nos padrões: copiar o que se seleciona |
| `reset` | boolean |  |

### `terminal_menu`

As opções do cartão de um terminal, as mesmas do menu do botão direito: copy devolve (e põe na área de transferência) o que está selecionado, paste cola `text`, select_all e clear_selection, rename dá o nome `value`, monitor liga ou desliga (`value` on ou off) o aviso de quando o trabalho termina, blur esconde a tela do terminal atrás de um véu (off tira), reload e unload (descarrega o shell até recarregar), use_size, use_font e use_theme fazem do tamanho, do tamanho de fonte e do tema deste terminal os padrões dos novos, copy_element, duplicate (a cópia fica ligada a você por corda), lock (`value` on ou off) e delete.

| Parâmetro | Tipo | O que é |
|---|---|---|
| `name` (obrigatório) | string | o nome do terminal |
| `action` (obrigatório) | string: `copy`, `paste`, `select_all`, `clear_selection`, `rename`, `monitor`, `blur`, `reload`, `unload`, `use_size`, `use_font`, `use_theme`, `copy_element`, `duplicate`, `lock`, `delete` |  |
| `value` | string | rename: o nome novo; monitor, blur, lock: on ou off |
| `text` | string | paste: o texto |

### `terminal_processes`

O que roda sob o shell de um terminal (o botão de processos do cartão): nome, PID e memória de cada um; `stop_pid` encerra um deles (só os daquele terminal).

| Parâmetro | Tipo | O que é |
|---|---|---|
| `name` (obrigatório) | string | o nome do terminal |
| `stop_pid` | number |  |

### `terminal_restart`

Reinicia o terminal com esse nome (o shell e o comando dele começam de novo).

| Parâmetro | Tipo | O que é |
|---|---|---|
| `name` (obrigatório) | string |  |

### `notify`

Avisa a pessoa com o seu nome: um aviso no app e uma notificação do sistema (no Windows, o ícone do app pisca na barra de tarefas).

| Parâmetro | Tipo | O que é |
|---|---|---|
| `text` (obrigatório) | string |  |

### `ask_person`

Uso interno do app: o Claude Code de um chat do MagoAgent passa por aqui o que precisa da pessoa (as perguntas do AskUserQuestion e as permissões), e o chat mostra as opções. Não chame diretamente.

| Parâmetro | Tipo | O que é |
|---|---|---|
| `tool_name` (obrigatório) | string |  |
| `input` (obrigatório) | object |  |
| `tool_use_id` | string |  |

### `remember`

Guarda uma coisa aprendida sobre a pessoa e o jeito dela de trabalhar; entra nas próximas conversas. Uma frase curta.

| Parâmetro | Tipo | O que é |
|---|---|---|
| `text` (obrigatório) | string |  |

### `review`

Abre na Árvore de Arquivos da pasta `path` (a da conversa, se vazio), ampliada, o painel Alterações com os arquivos mudados, para a pessoa revisar com você e pedir ajustes (aplique-os nos arquivos).

| Parâmetro | Tipo | O que é |
|---|---|---|
| `path` | string |  |

### `demands`

O que espera a pessoa, do mais urgente ao menos: pedidos de permissão, perguntas, erros, agentes que terminaram ou encerraram; cada um com os pontos de atenção calculados dos dados (comando destrutivo, caminho fora da pasta do agente, rede, credenciais, parado há muito tempo). Com `agent` e `answer` (allow ou deny) responde ao pedido de permissão desse agente, se ele ainda espera pelo app (`reason` vai ao agente quando nega); só com `agent`, traz também os últimos eventos dele. No fim vêm as sessões de fora do app (as que os hooks do Claude Code avisam), cada uma com o estado e a última atividade.

| Parâmetro | Tipo | O que é |
|---|---|---|
| `agent` | string | o nome do terminal ou do chat |
| `answer` | string: `allow`, `deny` |  |
| `reason` | string |  |

### `outside_message`

Manda uma mensagem da pessoa a uma sessão do Claude Code de fora do app (as que demands lista no fim), como a caixa da conversa dela na ilha. Se a sessão roda no tmux e nada espera a pessoa na tela dela, o texto é digitado no terminal com o Enter na hora; senão fica na fila e entra quando o Claude terminar a vez (uma sessão parada só recebe quando trabalhar de novo e terminar). A resposta diz qual foi. Claude Desktop, Cursor e Hermes não recebem: responda na janela deles.

| Parâmetro | Tipo | O que é |
|---|---|---|
| `session` | string | o nome da sessão como demands mostra (Claude Code · pasta), a pasta ou parte dela; vazio: a única |
| `text` (obrigatório) | string | a mensagem |

### `timeline`

A linha do tempo dos agentes ("o que aconteceu na última hora?", "o que o Revisor fez hoje?"): os eventos recentes, do mais antigo ao mais novo, com a hora local na frente; filtra por agente, tipo e minutos. Com mais de 5 eventos, começa por um resumo das contagens (ferramentas, pedidos de permissão, erros, agentes).

| Parâmetro | Tipo | O que é |
|---|---|---|
| `agent` | string | nome ou id do agente |
| `kind` | string: `tool_started`, `tool_ended`, `permission`, `question`, `working`, `finished`, `error`, `started`, `ended`, `message`, `task` |  |
| `minutes` | number | janela em minutos (1 a 1440) |
| `limit` | number | quantidade máxima (padrão 20, de 1 a 100) |

### `tasks`

As tarefas: o que foi pedido a quem, em que estado está e do que depende. create cria (title; owner é o nome de um terminal ou chat; send true entrega o texto da tarefa ao responsável, no terminal ou no chat dele, com o id e como avisar o estado); update muda state, note (por que bloqueou ou falhou), owner, title, description, parent, deps (substitui) ou artifacts (acrescenta), e send true entrega de novo; list lista uma tarefa por linha, as travadas primeiro, filtrando por state e owner; get traz uma inteira, com os pontos de atenção. Regras: concluída, falhou e cancelada só reabrem para pending; in_progress com dependência não concluída fica blocked com a razão e volta a pending quando as dependências concluem; ciclo de dependências e tarefa-mãe inexistente são recusados. Um agente num terminal usa esta mesma ferramenta para dizer em que pé está a tarefa dele.

| Parâmetro | Tipo | O que é |
|---|---|---|
| `action` (obrigatório) | string: `create`, `update`, `list`, `get` |  |
| `id` | string | update, get: o id da tarefa (t1, t2…) |
| `title` | string |  |
| `description` | string |  |
| `owner` | string | o nome do terminal ou do chat responsável; em list, filtra por ele |
| `state` | string: `pending`, `in_progress`, `waiting_agent`, `waiting_approval`, `in_review`, `in_test`, `blocked`, `done`, `failed`, `cancelled` | update: o estado novo; list: filtra por ele |
| `note` | string | por que está nesse estado (por que bloqueou, por que falhou) |
| `parent` | string | o id da tarefa-mãe; vazio tira |
| `deps` | array | os ids das tarefas de que esta depende |
| `artifacts` | array | arquivos, diff, navegador ou conversa que a tarefa produziu |
| `requester` | string | create: quem pediu, se não foi você |
| `send` | boolean | entrega a tarefa ao responsável |

### `skills`

Lista as skills disponíveis para a Luna ou abre a pasta delas no gerenciador de arquivos; action list (padrão) ou open.

| Parâmetro | Tipo | O que é |
|---|---|---|
| `action` | string: `list`, `open` |  |

### `maestro_comment`

Comentar para a Luna: envia ao chat da Luna um comentário sobre linhas de um arquivo (caminho, trecho em bloco de código e o comentário). Vai ao chat ligado à Árvore, senão ao chat em uso, senão a um novo, e abre o mini chat nele. Sem `tree`, usa a Árvore selecionada ou a primeira.

| Parâmetro | Tipo | O que é |
|---|---|---|
| `tree` | string | o nome da Árvore de Arquivos; vazio: a selecionada, ou a única |
| `path` (obrigatório) | string | relativo à pasta da Árvore |
| `from_line` (obrigatório) | number |  |
| `to_line` (obrigatório) | number |  |
| `comment` (obrigatório) | string |  |

### `flow`

O fluxo de 12 etapas de uma implementação (uma tarefa com `flow`): Planejar, Analisar codebase, Arquitetar, Dúvidas (a pessoa), Delegação e implementação, Testes, Validar onde é usado, Padronização, Revisor ponytail, Navegador e cenários, Code review do agente, Code review humano (a pessoa). O app guarda o estado e devolve o brief de cada etapa; quem executa é você e os agentes. start cria a tarefa-mãe (title, description, folder: o repositório) na etapa 1 e devolve o brief. advance anda: result ok exige a nota `<id>-<código da etapa>` no fichário (binder), grava o evento e devolve o brief da próxima; result fail (com note, o motivo) volta à implementação nas etapas 6 a 12 e repete a etapa nas de 1 a 4, e grava a lição em `licoes-<projeto>`; 3 falhas seguidas na mesma etapa bloqueiam a tarefa. Na etapa 5 passe `branches` (os ramos mago/<nome> dos agentes); na 12, `ok` junta esses ramos no ramo atual de `folder` (git merge --no-ff); num conflito o merge é desfeito e a tarefa fica bloqueada com os arquivos. owner e send entregam o brief ao agente. status mostra as 12 etapas de uma tarefa (id) ou uma linha por fluxo.

| Parâmetro | Tipo | O que é |
|---|---|---|
| `action` (obrigatório) | string: `start`, `advance`, `status` |  |
| `id` | string | advance, status: o id da tarefa-mãe do fluxo |
| `title` | string | start |
| `description` | string | start: o pedido da pessoa |
| `folder` | string | start, advance: o repositório do projeto (onde o diff e a junção dos ramos acontecem) |
| `result` | string: `ok`, `fail` | advance |
| `note` | string | advance: o resumo (ok) ou o motivo (fail) |
| `branches` | array | advance: os ramos mago/<nome> das fatias; acrescenta |
| `owner` | string | advance: o terminal ou chat que faz a próxima etapa |
| `send` | boolean | advance: entrega o brief ao responsável (owner) |

### `usage`

O consumo: quanto do limite do plano já foi (janela de 5 horas, com a hora do reinício, e semana), quantos agentes rodam e o teto, e o modelo, o contexto e o custo estimado de cada agente. Vem do que as sessões do Claude Code informam; sem leitura ainda, diz isso. Consulte antes de recrutar ou de delegar algo grande.

Sem parâmetros.

### `usage_refresh`

Lê de novo o uso dos planos (o botão Atualizar do detalhe Uso dos agentes, no rodapé da lateral): pergunta ao Claude Code e ao Codex quanto da sessão e da semana já foi. O app já faz isso a cada 5 minutos; use quando o número estiver velho. A resposta vem em alguns segundos: depois consulte `usage`.

Sem parâmetros.

### `memory`

Mostra a memória do MagoAgent e dos processos dos agentes por terminal.

Sem parâmetros.

## Chats

### `chat_new`

Abre um chat novo e o mostra: `agent` claude ou codex (vazio: o de Ajustes), na pasta `folder` (vazio: a de Ajustes). O chat nasce com um cartão no canvas (chat_node).

| Parâmetro | Tipo | O que é |
|---|---|---|
| `agent` | string: `claude`, `codex` |  |
| `folder` | string |  |

### `chat_open`

Mostra um chat à pessoa: na tela de chat do app (place app, o padrão) ou na conversa da ilha (place island), como escolher um agente da Luna no botão Agentes da conversa da ilha (ou Voltar à Luna), sem abrir janela flutuante.

| Parâmetro | Tipo | O que é |
|---|---|---|
| `chat` (obrigatório) | string | o nome do chat (ou parte dele), como chat_list mostra; vazio: o chat aberto |
| `place` | string: `app`, `island` | onde mostrar: app (o padrão) ou island (a conversa da ilha) |

### `chats_view`

A área de Chats fora do canvas (o botão de chat da barra de título): tudo o que está rodando em todos os espaços (os chats do app, os terminais com Claude Code ou Codex e as sessões de fora do app), agrupado por espaço, com o estado (Trabalhando, Esperando, Pronto) e a última linha de cada um. `state` open (o padrão) abre a área e devolve a lista; close volta ao canvas. `filter` escolhe o chip: all (Todos), working (Trabalhando), waiting (Esperando) ou ready (Prontos); `query` busca nos nomes e nas últimas linhas (vazio limpa a busca).

| Parâmetro | Tipo | O que é |
|---|---|---|
| `state` | string: `open`, `close` |  |
| `filter` | string: `all`, `working`, `waiting`, `ready` |  |
| `query` | string |  |

### `chat_send`

Manda uma mensagem a um chat, como se a pessoa a escrevesse lá; o agente dele responde no chat.

| Parâmetro | Tipo | O que é |
|---|---|---|
| `chat` (obrigatório) | string | o nome do chat (ou parte dele), como chat_list mostra; vazio: o chat aberto |
| `text` (obrigatório) | string |  |

### `chat_queue`

A mensagem que a pessoa deixou na fila de um agente (escrita enquanto um chat ainda respondia, ou enquanto um terminal esperava numa pergunta ou permissão), mostrada no fim da conversa com Enviar agora e Editar. `action` list (o padrão) diz o que está na fila; send_now a manda já, como o botão Enviar agora (Ctrl+Shift+Enter): num chat a resposta em curso para onde está e a mensagem vai como a próxima; num terminal esperando um menu, ele recebe Esc (recusa o pedido) e a mensagem entra assim que ele voltar ao prompt; edit a devolve à caixa da conversa, como Editar.

| Parâmetro | Tipo | O que é |
|---|---|---|
| `agent` | string | o nome do chat ou do terminal, como list_agents mostra; vazio: a conversa aberta |
| `action` | string: `list`, `send_now`, `edit` |  |

### `chat_list`

Os chats do MagoAgent: nome, agente, pasta, se está respondendo e qual está aberto.

Sem parâmetros.

### `chat_search`

Procura texto nas conversas carregadas, ignorando maiúsculas e acentos; `chat` pode ser nome ou id e `limit` vai de 1 a 50 (padrão 20).

| Parâmetro | Tipo | O que é |
|---|---|---|
| `query` (obrigatório) | string | Texto a procurar |
| `chat` | string | Nome ou id; vazio procura em todos os chats |
| `limit` | integer |  |

### `chat_rename`

Dá um nome a um chat (Renomear, no ⋯ dele na lista): o nome fica no lugar do título que a conversa tem.

| Parâmetro | Tipo | O que é |
|---|---|---|
| `chat` (obrigatório) | string | o nome do chat (ou parte dele), como chat_list mostra; vazio: o chat aberto |
| `name` (obrigatório) | string |  |

### `chat_provider`

Troca o provedor de um chat antes da primeira mensagem: `agent` claude ou codex.

| Parâmetro | Tipo | O que é |
|---|---|---|
| `chat` | string | o nome do chat (ou parte dele), como chat_list mostra; vazio: o chat aberto |
| `agent` (obrigatório) | string: `claude`, `codex` |  |

### `chat_command`

Roda um comando com barra do chat, como se a pessoa o digitasse na caixa: login (abre o navegador; console ou sso depois), logout, status, mcp, doctor, update, clear (conversa nova), model e effort (com o nome ou nível), help. A saída aparece no chat, sem ir ao modelo. Outros /comandos (compact, os de .claude/commands) são mensagens ao agente: use chat_send.

| Parâmetro | Tipo | O que é |
|---|---|---|
| `chat` | string | o nome do chat (ou parte dele), como chat_list mostra; vazio: o chat aberto |
| `command` (obrigatório) | string | Ex.: "status", "/model opus", "login sso" |

### `chat_context`

O painel Contexto do chat do Claude Code: open e close mostram ou escondem o painel da direita, refresh lê de novo; sempre devolve quanto da janela está em uso, o valor de compactar automaticamente, as categorias e os conectores (servidores MCP) com estado e quantidade de ferramentas.

| Parâmetro | Tipo | O que é |
|---|---|---|
| `chat` | string | o nome do chat (ou parte dele), como chat_list mostra; vazio: o chat aberto |
| `state` | string: `open`, `close`, `refresh` |  |

### `chat_decide`

Responde pela pessoa o que o agente de um chat espera decidir (as perguntas com opções do AskUserQuestion ou uma permissão), como o cartão acima da caixa do chat: `answers` leva cada pergunta (o texto dela) à resposta (o rótulo de uma opção, ou outro texto); `allow` false recusa.

| Parâmetro | Tipo | O que é |
|---|---|---|
| `chat` | string | o nome do chat (ou parte dele), como chat_list mostra; vazio: o chat aberto |
| `answers` | object | pergunta → resposta |
| `allow` | boolean | false recusa; padrão true |

### `chat_node`

O cartão de um chat no canvas: ali o chat é a Luna (list_agents a chama de Luna), e o que se liga a ele por cordas, terminais e outros chats (connect, disconnect), é o time que ele comanda e que o alcança com ask_agent. action show (o padrão) põe o cartão do chat no canvas deste espaço, se ainda não está, e mostra o canvas nele; new cria um chat novo (`agent` e `folder` como em chat_new) já com o cartão, e mostra o canvas; remove tira o cartão do canvas (a conversa continua na lista de chats); fold recolhe o cartão de um agente recrutado em chat ao nome e à tarefa, ou o abre de novo mostrando o que ele faz.

| Parâmetro | Tipo | O que é |
|---|---|---|
| `chat` | string | o nome do chat (ou parte dele), como chat_list mostra; vazio: o chat aberto |
| `action` | string: `show`, `new`, `remove`, `fold` |  |
| `agent` | string: `claude`, `codex` |  |
| `folder` | string |  |

### `mini_chat`

Abre (state open, o padrão) o chat em miniatura flutuante sobre o canvas, no chat `chat` (vazio: o último aberto), ou o recolhe para a pílula (state close).

| Parâmetro | Tipo | O que é |
|---|---|---|
| `chat` | string | o nome do chat (ou parte dele), como chat_list mostra; vazio: o chat aberto |
| `state` | string: `open`, `close` |  |

## Árvore de Arquivos e editor

### `file_tree`

A árvore de arquivos do canvas (a árvore de uma pasta, com o estado do git de cada arquivo). create cria uma na pasta `path`; list mostra a pasta, o ramo e as linhas à vista (> pasta fechada, v aberta; M modificado, A adicionado, D excluído, R renomeado, U não rastreado, ! em conflito); expand e collapse abrem e fecham a pasta `path`; collapse_all recolhe tudo; navigate entra na pasta `path` e back volta para a de cima (forward desfaz); refresh lê de novo; open abre o arquivo `path` como o clique duplo (no painel de código da árvore); root troca a pasta raiz por `path`; hidden mostra ou esconde os arquivos ocultos (`show`); sort ordena por `by` (`reverse` inverte); new_file e new_folder criam `name` na pasta `path`; rename renomeia `path` para `name`; delete manda `path` para a lixeira; copy, cut e paste copiam, recortam e colam `path`; editor mostra, esconde ou alterna (`show`) o painel de edição à direita da árvore (open abre o arquivo `path` nele: texto editável, foto ou PDF), save salva o arquivo da aba à frente e close_file fecha a aba de `path`; changes lista os arquivos alterados contra o HEAD (uma linha cada: a letra do git e o caminho; show true abre o painel Alterações na árvore, show false fecha; com `path` abre o diff daquele arquivo); annotate liga (show false desliga) a coluna de autor e data de cada linha do arquivo `path`, aberto no painel; branches lista os ramos locais do repositório (o atual com *) e checkout `name` troca para um deles (recusa, dizendo por quê, se há mudanças que a troca sobrescreveria); search procura arquivos pelo nome sob a pasta mostrada (`query`; vazio limpa a busca) e devolve as ocorrências com a pasta de cada uma; find_in_files é o Localizar nos Arquivos (Ctrl+Shift+F): procura o texto `query` nos arquivos da raiz, sem as pastas excluídas (`case` diferencia maiúsculas, `words` só palavras inteiras, `regex` expressão regular, `mask` limita os arquivos, ex.: *.rs, `multiline` deixa a busca atravessar linhas, e `path` ou `scope` directory limitam a uma pasta), abre a janela com a mesma busca e devolve as ocorrências (arquivo:linha: texto); usages lista os usos do nome `name` declarado ou usado no arquivo `path`, como o Ctrl+clique na declaração (arquivo:linha: texto); history lista os commits que mexeram na declaração `name` do arquivo `path` (ou nas linhas `from` a `to` dele), com autor, quando e a mensagem, e com `commit` (o hash) devolve o que esse commit mudou no arquivo e abre isso numa aba; diff abre numa aba o diff lado a lado do arquivo `path` contra o HEAD (como Ctrl+D), com destaque por linha e por palavra, as duas colunas rolando juntas, a da direita (o arquivo atual) editável, e lista as mudanças (`collapse` false mostra os trechos iguais inteiros; o padrão, true, os recolhe deixando 4 linhas de contexto em volta de cada mudança, como o "Recolher trechos sem mudança"); hunks só lista as faixas alteradas do arquivo contra o HEAD (as mesmas dos marcadores verdes, azuis e cinza da margem do editor: número, tipo e linhas); revert_hunk desfaz a mudança de número `index` (a de hunks, a partir de 1) e grava o arquivo, recusando se ele tiver mudanças não salvas no editor; problems lista os problemas (erros e avisos, como o IntelliJ: o servidor de linguagem, quando instalado, e a checagem de sintaxe local de PHP, JS/TS, JSON, Rust, Go, Python, C/C++, C#, Java, Kotlin, Swift, Dart e CSS: parêntese, chave, colchete, aspas ou comentário aberto e JSON inválido) do arquivo `path` ou, sem `path` (ou com uma pasta), dos arquivos da raiz, com a linha e a coluna (a primeira é 1) e a mensagem de cada um; no editor eles aparecem sublinhados em ondas, com a contagem no canto e marcas na barra de rolagem, F2 e Shift+F2 vão ao próximo e ao anterior, e na árvore o arquivo e as pastas dele ficam em vermelho; open com `line` abre o arquivo nessa linha; close fecha a árvore. Conflitos de um merge ou de um stash na pasta da árvore: conflicts lista os arquivos em conflito e, com `path`, os conflitos desse arquivo numerados, com as linhas de cada lado; resolve resolve o arquivo `path` e já o marca como resolvido (git add): `side` mine fica com a minha versão inteira e theirs com a deles, ou `picks` responde conflito por conflito, na ordem (as alterações sem conflito dos dois lados entram sozinhas); merge abre para a pessoa a janela de mesclagem de `path`, com as três versões lado a lado, ou, sem `path`, a janela Conflitos com a lista. `tree` é o nome da pasta, o caminho ou o id da árvore (vazio: a selecionada ou a única). `path` sem caminho completo vale dentro da pasta mostrada.

| Parâmetro | Tipo | O que é |
|---|---|---|
| `action` (obrigatório) | string: `create`, `list`, `expand`, `collapse`, `collapse_all`, `navigate`, `back`, `forward`, `refresh`, `open`, `root`, `hidden`, `sort`, `new_file`, `new_folder`, `rename`, `delete`, `copy`, `cut`, `paste`, `search`, `branches`, `checkout`, `annotate`, `changes`, `find_in_files`, `usages`, `history`, `problems`, `diff`, `hunks`, `revert_hunk`, `editor`, `save`, `close_file`, `close`, `conflicts`, `resolve`, `merge` |  |
| `tree` | string | o nome da pasta da árvore, o caminho dela ou o id; vazio: a ligada a você por corda, a selecionada, ou a única do canvas |
| `path` | string |  |
| `name` | string | new_file, new_folder, rename: o nome; checkout: o ramo; usages, history: o nome declarado |
| `query` | string | search: parte do nome do arquivo; vazio limpa a busca. find_in_files: o texto a localizar. db_query: use `query` para a consulta SELECT |
| `case` | boolean | find_in_files: diferencia maiúsculas de minúsculas |
| `words` | boolean | find_in_files: só palavras inteiras |
| `regex` | boolean | find_in_files: `query` é uma expressão regular |
| `mask` | string | find_in_files: máscara de arquivo, ex.: *.rs, *.toml |
| `multiline` | boolean | find_in_files: a busca atravessa linhas (a quebra de linha vale como texto, ^ e $ valem em cada linha) |
| `scope` | string: `project`, `directory` | find_in_files: project (padrão sem path) procura a raiz inteira; directory procura só a pasta `path` (padrão com path), como a aba Diretório |
| `line` | integer | open: a linha (a primeira é 1); annotate: a linha cujo commit se quer |
| `index` | integer | revert_hunk: o número da mudança, como hunks a lista (a primeira é 1) |
| `from` | integer | history: a primeira linha do trecho (a primeira do arquivo é 1) |
| `to` | integer | history: a última linha do trecho |
| `commit` | string | history: o hash do commit cujo diff do arquivo se quer |
| `collapse` | boolean | diff: true (padrão) recolhe os trechos sem mudança, false mostra tudo |
| `show` | boolean | hidden: true mostra os arquivos ocultos, false esconde |
| `by` | string: `name`, `kind`, `modified`, `created`, `size` |  |
| `reverse` | boolean | sort: ordem invertida |
| `side` | string: `mine`, `theirs` | resolve: o arquivo inteiro fica com a minha versão ou com a deles |
| `picks` | array | resolve: uma escolha por conflito do arquivo, na ordem em que conflicts os lista |

### `ide_open`

Abre, na árvore de arquivos do canvas ligada a você (no painel de editor dela), uma pasta (ela se expande na árvore) ou um arquivo (ele aparece no painel ao lado da árvore, rolado até `line` e com a linha destacada), para a pessoa acompanhar o que você está vendo. `path` é relativo à pasta do projeto.

| Parâmetro | Tipo | O que é |
|---|---|---|
| `tree` | string | o nome da Árvore de Arquivos, como list_agents mostra; opcional com uma só ligada a você |
| `path` (obrigatório) | string |  |
| `line` | number | linha a mostrar, a partir de 1 |

### `ide_search`

Procura um texto (sem diferenciar maiúsculas) nos arquivos do projeto de uma árvore de arquivos ligada a você e devolve até 200 linhas como caminho:linha: texto (pastas excluídas, como node_modules e vendor, ficam de fora). A árvore mostra a busca na tela (Localizar nos Arquivos); abra o que interessar com ide_open.

| Parâmetro | Tipo | O que é |
|---|---|---|
| `tree` | string | o nome da Árvore de Arquivos, como list_agents mostra; opcional com uma só ligada a você |
| `query` (obrigatório) | string |  |

### `ide_folder`

Abre uma pasta na Árvore de Arquivos do canvas, ampliada, com o painel de código (usa a árvore que já está nela, ou cria uma), e devolve a estrutura de pastas e arquivos (3 níveis).

| Parâmetro | Tipo | O que é |
|---|---|---|
| `path` (obrigatório) | string |  |

### `open_path`

Abre à vista da pessoa um arquivo ou pasta pelo caminho: uma pasta na Árvore de Arquivos, um PDF, imagem ou página HTML num navegador do canvas, qualquer outro arquivo no painel de código da Árvore do projeto dele (na `line`, se dada). O navegador aberto fica ligado por corda a quem pediu, que pode lê-lo e controlá-lo (portal_*).

| Parâmetro | Tipo | O que é |
|---|---|---|
| `path` (obrigatório) | string |  |
| `line` | number | linha a mostrar, a partir de 1 |

### `open_default`

Abre um arquivo ou pasta no programa padrão do sistema (programas executáveis, como .exe, .bat e .sh, não são iniciados).

| Parâmetro | Tipo | O que é |
|---|---|---|
| `path` (obrigatório) | string |  |

### `set_folder`

Muda a pasta de trabalho da conversa atual para `path` (uma conversa que já tem sessão continua onde está e outra abre na pasta nova).

| Parâmetro | Tipo | O que é |
|---|---|---|
| `path` (obrigatório) | string |  |

## Navegador (portais)

### `create_portal`

Abre um navegador (página web) à direita do seu terminal (com o lugar tomado, no lugar livre mais perto de você), já ligado a você. Só de dentro de um terminal do MagoAgent.

| Parâmetro | Tipo | O que é |
|---|---|---|
| `url` (obrigatório) | string |  |
| `name` | string | opcional; padrão o domínio |

### `portal_navigate`

Abre um endereço num navegador (página web do canvas) conectado a você.

| Parâmetro | Tipo | O que é |
|---|---|---|
| `name` (obrigatório) | string | o nome do navegador, como list_agents mostra |
| `url` (obrigatório) | string |  |

### `portal_read`

Lê o texto visível da página de um navegador conectado.

| Parâmetro | Tipo | O que é |
|---|---|---|
| `name` (obrigatório) | string |  |

### `portal_click`

Clica num elemento da página de um navegador conectado. Seu cursor, com seu nome, aparece na página e desliza até o elemento antes do clique, para quem estiver olhando acompanhar.

| Parâmetro | Tipo | O que é |
|---|---|---|
| `name` (obrigatório) | string |  |
| `selector` (obrigatório) | string | seletor CSS do elemento, ex.: button[type=submit] |

### `portal_fill`

Digita um texto num campo da página de um navegador conectado, com seu cursor indo até ele.

| Parâmetro | Tipo | O que é |
|---|---|---|
| `name` (obrigatório) | string |  |
| `selector` (obrigatório) | string |  |
| `text` (obrigatório) | string |  |

### `portal_eval`

Roda JavaScript na página de um navegador conectado e devolve o resultado (JSON). O valor de uma Promise não é esperado; para esperar algo, use portal_wait. Útil para clicar (document.querySelector(...).click()), preencher campos ou ler partes da página.

| Parâmetro | Tipo | O que é |
|---|---|---|
| `name` (obrigatório) | string |  |
| `script` (obrigatório) | string |  |

### `portal_scroll`

Rola a página do navegador pelos pixels informados em dy ou traz um elemento CSS à vista usando selector.

| Parâmetro | Tipo | O que é |
|---|---|---|
| `name` (obrigatório) | string |  |
| `selector` | string |  |
| `dy` | integer |  |

### `portal_wait`

Espera até um seletor CSS existir e estar visível na página do navegador, ou informa que o prazo acabou.

| Parâmetro | Tipo | O que é |
|---|---|---|
| `name` (obrigatório) | string |  |
| `selector` (obrigatório) | string |  |
| `timeout_ms` | integer | prazo em milissegundos (padrão 5000, máximo 30000) |

### `portal_screenshot`

Tira uma imagem do que a página de um navegador conectado mostra agora, para você ver o resultado de verdade (layout, cores, o que apareceu). O navegador precisa estar na tela. No Windows e no Linux (no Linux só no X11); ainda não no macOS.

| Parâmetro | Tipo | O que é |
|---|---|---|
| `name` (obrigatório) | string |  |

### `portal_copy_screenshot`

Copiar Captura de Tela: põe a imagem da página aberta do navegador na área de transferência da pessoa (no Windows e no Linux; ainda não no macOS). Para ver a página você mesmo, use portal_screenshot.

| Parâmetro | Tipo | O que é |
|---|---|---|
| `portal` (obrigatório) | string |  |

### `portal_console`

Lê as últimas mensagens e erros do console da página de um navegador conectado. Pode filtrar por nível e limpar as entradas depois da leitura.

| Parâmetro | Tipo | O que é |
|---|---|---|
| `name` (obrigatório) | string |  |
| `level` | string: `error`, `warn`, `info`, `log`, `debug` |  |
| `clear` | boolean |  |

### `portal_history`

Volta, avança ou recarrega a página atual do navegador.

| Parâmetro | Tipo | O que é |
|---|---|---|
| `name` (obrigatório) | string |  |
| `action` (obrigatório) | string: `back`, `forward`, `reload` |  |

### `portal_tab`

As abas de um navegador: new abre uma aba com `url`, close fecha a aba `index` (vazio: a aberta), select mostra a aba `index` (a partir de 0).

| Parâmetro | Tipo | O que é |
|---|---|---|
| `portal` (obrigatório) | string |  |
| `action` (obrigatório) | string: `new`, `close`, `select` |  |
| `url` | string |  |
| `index` | number |  |

### `portal_settings`

Ajustes de um navegador: `user_agent` (Agente de Usuário: Chrome, Firefox, Microsoft Edge, Safari (iPhone), Chrome (Android), Firefox (Android), Microsoft Edge (Android), "Padrão" ou o texto de um personalizado; recarrega o navegador), `storage` (Armazenamento: isolated, workspace ou global; recarrega o navegador), `muted` (Silenciar o áudio e o vídeo da página) e `size_readout` (Mostrar Tamanho: never, onresize ou always; vale para todos os navegadores).

| Parâmetro | Tipo | O que é |
|---|---|---|
| `portal` (obrigatório) | string |  |
| `user_agent` | string |  |
| `storage` | string: `isolated`, `workspace`, `global` |  |
| `muted` | boolean |  |
| `size_readout` | string: `never`, `onresize`, `always` |  |

### `vault_list`

As contas do cofre de senhas da pessoa (site, usuário e tipo de login), nunca a senha. Use para saber com que conta ela entra num site.

Sem parâmetros.

### `vault_accounts`

As contas de um site no cofre. Com mais de uma, pergunte à pessoa qual usar antes de entrar.

| Parâmetro | Tipo | O que é |
|---|---|---|
| `site` (obrigatório) | string | domínio ou nome do app, ex.: github.com |

### `vault_fill`

Digita a senha de uma conta do cofre no campo focado da página do navegador aberto nesse site, sem mostrá-la a você: clique no campo de senha antes (portal_click) ou diga o seletor em `field`. Num login via Google/Apple digita o e-mail; o resto segue pelo botão do provedor. Nunca peça a senha em texto.

| Parâmetro | Tipo | O que é |
|---|---|---|
| `site` (obrigatório) | string | domínio ou nome do app, como vault_accounts mostra |
| `user` | string | a conta, quando o site tem mais de uma |
| `field` | string | seletor CSS do campo; vazio: o campo focado |

## Janelas, espaços e andares

### `show_app`

Abre o app MagoAgent para o usuário ver o desenho ao vivo.

Sem parâmetros.

### `show_screen`

Mostra à pessoa uma tela do MagoAgent: canvas (o quadro com os agentes trabalhando), chat ou tree (a primeira Árvore de Arquivos).

| Parâmetro | Tipo | O que é |
|---|---|---|
| `screen` (obrigatório) | string: `canvas`, `chat`, `tree` |  |

### `resize_window`

Redimensiona uma janela: `target` é o nome de um terminal, navegador ou árvore de arquivos do canvas (state enlarge amplia, shrink volta ao canvas, ou width/height em pontos), ou `app` para a janela do MagoAgent (state maximize, restore, minimize, ou width/height).

| Parâmetro | Tipo | O que é |
|---|---|---|
| `target` (obrigatório) | string |  |
| `state` | string: `enlarge`, `shrink`, `maximize`, `restore`, `minimize` |  |
| `width` | number |  |
| `height` | number |  |

### `close_window`

Fecha (apaga do canvas) um terminal, navegador, árvore de arquivos ou nota pelo nome; dá para desfazer com history undo.

| Parâmetro | Tipo | O que é |
|---|---|---|
| `name` (obrigatório) | string |  |

### `workspaces`

Os espaços de trabalho, cada um um projeto com o seu desenho, a sua pasta raiz e uma cor: list mostra todos (o que está na tela e quantos agentes rodam e precisam da pessoa em cada um); switch mostra o espaço `name`; create cria o espaço `name` na pasta `folder` e o mostra; rename dá a ele o nome `new_name`; color troca a cor (`color`, #rrggbb); remove tira o espaço da lista e encerra os terminais dele (o desenho fica no disco; o último espaço não sai). Trocar de espaço não para os agentes: os terminais dos outros espaços continuam rodando.

| Parâmetro | Tipo | O que é |
|---|---|---|
| `action` (obrigatório) | string: `list`, `switch`, `create`, `rename`, `color`, `remove` |  |
| `name` | string | o nome do espaço (o novo, em create) |
| `new_name` | string | rename: o nome novo |
| `folder` | string | create: a pasta raiz, onde os terminais e árvores novos dele começam |
| `color` | string | color: a cor nova, #rrggbb |

### `workspace_groups`

Os grupos da barra lateral dos espaços de trabalho (Novo grupo): um título com nome, que recolhe e expande, sob o qual ficam espaços. list mostra os grupos e o que há em cada um; create cria o grupo `name`; rename dá a ele `new_name`; toggle recolhe ou expande; remove apaga o grupo (os espaços dele ficam sem grupo, nenhum é removido); move põe o espaço `space` dentro do grupo `group`, ou, sem `group`, fora de qualquer grupo.

| Parâmetro | Tipo | O que é |
|---|---|---|
| `action` (obrigatório) | string: `list`, `create`, `rename`, `toggle`, `remove`, `move` |  |
| `name` | string | o nome do grupo (o novo, em create) |
| `new_name` | string | rename: o nome novo |
| `space` | string | move: o nome do espaço |
| `group` | string | move: o nome do grupo; vazio tira o espaço do grupo |

### `workspace_menu`

O menu de um espaço de trabalho: pin e unpin (Fixar e Desfixar: o espaço fixado vai para Fixados, no alto da lista); duplicate (cópia dele e do desenho, "<nome> cópia"); copy_path (devolve a pasta raiz); export (grava o espaço, com o desenho, no arquivo `path`); import (cria um espaço novo do arquivo `path`, com os terminais e navegadores descarregados); unload (Descarregar: encerra os terminais e navegadores do espaço sem tirá-los do desenho; Recarregar num deles o inicia).

| Parâmetro | Tipo | O que é |
|---|---|---|
| `action` (obrigatório) | string: `pin`, `unpin`, `duplicate`, `copy_path`, `export`, `import`, `unload` |  |
| `name` | string | o nome do espaço (não vale em import) |
| `path` | string | export, import: o arquivo |

### `floors`

Os andares do espaço de trabalho na tela: o Térreo (o desenho do espaço) e os andares acima dele, cada um um desenho completo com nome, cor, câmera e nós. list mostra todos (o que está na tela e quantos itens); switch mostra o andar `name` ("Térreo" volta ao térreo); create cria o andar `name` e o mostra (`clone_ground` copia o layout do Térreo); rename dá a ele o nome `new_name`; delete apaga o andar e tudo o que está nele; move põe o andar na `position` (1 é o logo acima do Térreo). Trocar de andar não para os agentes: os terminais dos andares fora da vista continuam rodando.

| Parâmetro | Tipo | O que é |
|---|---|---|
| `action` (obrigatório) | string: `list`, `switch`, `create`, `rename`, `delete`, `move` |  |
| `name` | string | o nome do andar (o novo, em create) |
| `new_name` | string | rename: o nome novo |
| `clone_ground` | boolean | create: copia o layout do Térreo para o andar novo |
| `position` | integer | move: a posição, contada do Térreo para cima (1 é o primeiro andar acima) |

## Ajustes, integrações, voz e app

### `set_setting`

Muda um ajuste do app: theme (light, dark, system), voice (voicestudio, openai, codex: o motor da chamada; com o Codex a chamada não liga), island (on, off: a ilha da Luna, um notch no topo da tela por cima de tudo com o resumo dos agentes, as perguntas pendentes, o chat novo e a voz), island_sound (on, off: os sons da ilha), island_volume (0 a 0.2; padrão 0.1), island_close_after (5, 10, 15 ou 30 segundos: quanto a ilha aberta espera para ficar compacta), island_compact_after (15, 30, 60, 300 ou 0 segundos: quanto a ilha compacta espera para virar o notch pequeno; 0 nunca), run_at_login (on, off: o MagoAgent abre ao entrar na conta do Windows, do macOS ou do Linux), island_place (top, bottom, left, right: a ilha no topo, embaixo crescendo para cima, ou grudada na lateral esquerda ou direita, onde fechada ou compacta fica em pé; reset: volta ao lugar padrão, o meio do topo do monitor do app, quando ela sumiu ou ficou fora da tela), island_page (home, chats, chat, new, claude, github, spotify: abre a ilha nessa aba; home é o resumo, claude as sessões do Claude Code, no canvas e fora do app), island_feed (hide_all: oculta as linhas do resumo da aba Início até mudarem; show_hidden: traz as ocultas de volta), frame_rate (30, 60, 120, 144 ou 240: os quadros por segundo da ilha e das animações; mais fica mais suave e gasta mais processador), gnome_cursor (install, remove, status: no Linux com GNOME, a extensão do GNOME que diz onde o mouse está para os olhos da ilha seguirem ele fora das janelas do app; o GNOME a carrega no próximo login; só a pedido da pessoa), island_color (cor #rrggbb), claude_hooks (install, remove, status: os hooks do MagoAgent no settings.json do Claude Code da pessoa, com backup e sem apagar nada, para a ilha mostrar as perguntas e permissões de qualquer sessão, o Claude Desktop incluso; só a pedido dela), register_mcp (claude ou codex: registra o MCP do MagoAgent no Claude Code, com --scope user, ou no Codex da pessoa, trocando um registro anterior de mesmo nome, para as sessões de fora do app usarem o canvas; no WSL quando o agente roda lá; só a pedido dela), voice_studio_url (o endereço do servidor do VoiceStudio, padrão http://127.0.0.1:3900), voice_studio_voice (o id da voz dele, da lista de Ajustes › Voz; alloy é a padrão), voice_studio_key (a chave de API dele, se pedir; vazio tira), terminal_history (1000, 5000, 10000), avatar (on, off: a barra da Luna por cima de tudo na tela), avatar_follow (on: a barra segue a janela em uso; off: fica onde está) zoom_step (de 2 a 50: quantos pontos de zoom um passo da roda do mouse muda) voice_mic (o microfone que a chamada e o Ditar ouvem, por parte do nome; auto usa o do sistema, menos um cabo virtual), ou favorite_folder (o caminho de uma pasta: entra nas favoritas do campo de pasta, no topo da lista Recentes, ou sai delas se já estava). Consumo: max_agents (quantos agentes podem rodar para a Luna recrutar mais um; 0 tira o teto), usage_stop (a porcentagem da janela de 5 horas do plano a partir da qual o app não recruta; 0 nunca para) e agent_mode (terminal ou chat: como os agentes recrutados rodam, num terminal com o Claude Code completo ou num chat leve do app) update_check (sempre, ao abrir ou nunca: quando o app procura sozinho uma versão nova no GitHub; sempre é ao abrir e a cada hora) e recruit_model (o modelo dos agentes recrutados, por exemplo sonnet ou haiku; cli usa o da CLI). style_skill (o caminho do SKILL.md do seu estilo de código, que a etapa Padronização do fluxo manda o agente ler). Chat novo (Ajustes › Chat): chat_agent (claude ou codex), chat_folder (a pasta de trabalho; vazio: a padrão), chat_mode (auto ou bypass: o que o agente pode), chat_autocompact (100k a 1M: onde o Claude Code compacta sozinho; padrão volta ao do modelo), chat_panel (on, off: o painel Contexto) e agent_hooks (on, off: os agentes do app avisam a ilha por hooks). No Windows, wsl (auto: a distro padrão do WSL quando o Windows não tem o claude nem o codex; off: nunca; ou o nome de uma distro, como Ubuntu, para os agentes rodarem sempre nela). Voz (Ajustes › Ok Mago): ok_mago (on, off: dizer Ok Mago chama a Luna na chamada) e speak_answers (on, off: a chamada começa com os alto-falantes ligados). Seleção: window_outline (a borda azul de uma janela selecionada: animated, still ou none). Árvore de Arquivos: hidden_shown (os ocultos que ela ainda lista com os ocultos escondidos, separados por vírgula, como .claude, .idea; a .git fica oculta). Atalhos globais da ilha, que valem com qualquer programa na frente: shortcut_openChat (abre o chat da ilha), shortcut_goToAlert (vai à pergunta ou permissão pendente), shortcut_jumpToTerminal (abre o terminal da sessão), shortcut_nextPill, shortcut_prevPill e shortcut_muteToggle; o valor são as teclas (Ctrl+Alt+K: Ctrl, Alt ou Win com uma tecla), off ou on para desligar e ligar, ou default para voltar ao padrão. wipe_data (Limpar todos os dados) não é feito por aqui: só a pessoa, em Ajustes › Dados. Atalhos do app, que valem na janela dele: shortcut_settings (ajustes), shortcut_palette (paleta de comandos), shortcut_showChat (tela do chat), shortcut_showCanvas (voltar ao canvas), shortcut_openTree (abrir a árvore de arquivos), shortcut_nextNeedingYou (próximo agente que precisa de você), shortcut_floors (visão dos andares), shortcut_grid (mostrar ou esconder a grade), shortcut_theme (tema claro ou escuro), shortcut_save (salvar), shortcut_saveAs (salvar como), shortcut_open (abrir), shortcut_export (exportar imagem), shortcut_undo (desfazer), shortcut_redo (refazer), shortcut_selectAll (selecionar tudo), shortcut_duplicate (duplicar), shortcut_clearCanvas (limpar o canvas), shortcut_group (agrupar), shortcut_ungroup (desagrupar), shortcut_lock (travar ou destravar), shortcut_tidy (arrumar a seleção), shortcut_editPoints (editar os pontos da linha), shortcut_layerForward (uma camada à frente), shortcut_layerBackward (uma camada atrás), shortcut_layerFront (trazer para a frente de tudo), shortcut_layerBack (enviar para trás de tudo), shortcut_zoomIn (aproximar), shortcut_zoomOut (afastar), shortcut_zoomReset (voltar a 100%), shortcut_fitAll (ver tudo), shortcut_fitSelection (ver a seleção), shortcut_sendNow (enviar agora a mensagem da fila). O valor são as teclas (Ctrl+S, Ctrl+Shift+K, Shift+1, F2), none para deixar sem tecla ou default para o padrão; vale também para os globais. Teclas que já são de outro atalho, as fixas (as letras das ferramentas, Esc, Enter, Delete, as setas, Ctrl+C, Ctrl+X, Ctrl+V) e as do sistema (Alt+Tab, Alt+F4) são recusadas, dizendo de quem são.

| Parâmetro | Tipo | O que é |
|---|---|---|
| `name` (obrigatório) | string: `theme`, `voice`, `voice_studio_url`, `voice_studio_voice`, `voice_studio_key`, `terminal_history`, `avatar`, `avatar_follow`, `island`, `island_sound`, `island_volume`, `island_close_after`, `island_compact_after`, `run_at_login`, `island_place`, `island_page`, `wipe_data`, `island_feed`, `frame_rate`, `gnome_cursor`, `island_color`, `favorite_folder`, `zoom_step`, `max_agents`, `usage_stop`, `agent_mode`, `update_check`, `recruit_model`, `voice_mic`, `hidden_shown`, `window_outline`, `style_skill`, `chat_agent`, `chat_folder`, `chat_mode`, `chat_autocompact`, `chat_panel`, `agent_hooks`, `wsl`, `ok_mago`, `speak_answers`, `claude_hooks`, `codex_hooks`, `register_mcp`, `shortcut_openChat`, `shortcut_goToAlert`, `shortcut_jumpToTerminal`, `shortcut_nextPill`, `shortcut_prevPill`, `shortcut_muteToggle`, `shortcut_settings`, `shortcut_palette`, `shortcut_showChat`, `shortcut_showCanvas`, `shortcut_openTree`, `shortcut_nextNeedingYou`, `shortcut_floors`, `shortcut_grid`, `shortcut_theme`, `shortcut_save`, `shortcut_saveAs`, `shortcut_open`, `shortcut_export`, `shortcut_undo`, `shortcut_redo`, `shortcut_selectAll`, `shortcut_duplicate`, `shortcut_clearCanvas`, `shortcut_group`, `shortcut_ungroup`, `shortcut_lock`, `shortcut_tidy`, `shortcut_editPoints`, `shortcut_layerForward`, `shortcut_layerBackward`, `shortcut_layerFront`, `shortcut_layerBack`, `shortcut_zoomIn`, `shortcut_zoomOut`, `shortcut_zoomReset`, `shortcut_fitAll`, `shortcut_fitSelection`, `shortcut_sendNow` |  |
| `value` (obrigatório) | string |  |

### `dictionary`

O dicionário do ouvido: as palavras que o ouvido da chamada e do Ditar deve escrever certo (nomes, marcas, termos) e as trocas (o que é ouvido vira o que escrever). `action` list (o padrão) mostra tudo; add_word e remove_word com `word`; add_replacement com `heard` (o que o ouvido escreve, como cloud code) e `written` (o que deve sair, como Claude Code), remove_replacement com `heard`. As palavras vão ao motor de voz como dica e valem na chamada pelo VoiceStudio ou pela OpenAI e no Ditar.

| Parâmetro | Tipo | O que é |
|---|---|---|
| `action` | string: `list`, `add_word`, `remove_word`, `add_replacement`, `remove_replacement` |  |
| `word` | string | a palavra |
| `heard` | string | o que o ouvido escreve |
| `written` | string | o que deve ser escrito no lugar |

### `email`

O servidor MCP de email (Gmail ou outro) que os agentes dos chats ganham ao lado do MCP do app. O app não instala nem baixa nada: o programa precisa estar no computador e o servidor cuida do próprio login. `action` status (o padrão) diz como está; set com `command` (o programa, como npx) e `args` (os argumentos numa linha) o configura; gmail_example preenche o servidor de Gmail mais usado (o npx baixa o pacote na primeira execução; o login do Google segue as instruções do pacote); on e off dão ou tiram o servidor dos agentes (vale para as próximas mensagens dos chats).

| Parâmetro | Tipo | O que é |
|---|---|---|
| `action` | string: `status`, `set`, `gmail_example`, `on`, `off` |  |
| `command` | string | o programa do servidor, como npx |
| `args` | string | os argumentos numa linha, como -y pacote |

### `integrations`

Ajustes › Integrações, os serviços de fora que a ilha mostra. `integration` github: status (o padrão) diz quem está conectado e lista os pull requests abertos da pessoa com o estado dos checks e da revisão, os que pedem a revisão dela, o CI da branch principal dos repositórios dela, a atividade e como foi a última escrita; on e off ligam e desligam; test pergunta ao GitHub quem é o token e busca os pull requests de agora; remove_token tira o token do cofre do sistema; prs lista os pull requests da aba GitHub da ilha (os mudados por último, abertos, mesclados ou fechados, e os que pedem a revisão dela) com estado, branches, quando mudou, checks, conflito e se precisa dela (`filter` all, open, needs_you ou closed; needs_you: checks falharam, conflito, mudanças pedidas ou revisão pedida a ela); pr com `pr` (dono/repo#número ou o endereço) lê a descrição em Markdown e o que bloqueia o pull request (a primeira leitura só pede ao GitHub: chame de novo em alguns segundos); pr_body com `pr` e `text` troca a descrição inteira por `text`; pr_comment com `pr` e `text` comenta; pr_approve com `pr` aprova o pull request de outra pessoa; pr_merge com `pr` mescla pelo método padrão do repositório dela (leia com `pr` antes: só mescla um pronto, sem rascunho nem conflito, e só se o último commit ainda for o lido). pr_body, pr_comment, pr_approve e pr_merge escrevem no GitHub em nome da pessoa: só a pedido dela; o resultado aparece em status. O token nunca passa por aqui: a pessoa cola em Ajustes › Integrações e ele fica no cofre do sistema. `integration` spotify (sem conta e sem chave: o app do Spotify aberto neste computador; no Linux, também o Spotifast) e, só no Linux, `integration` soloist (o receptor do Spotify Connect, aberto com a API local -w ou pelo Spotifast), cada um com o seu on e off; a ilha mostra o que estiver tocando: status diz o que toca (de onde, o tempo, embaralhar, repetir, volume, se é curtida, a conta); on e off; playpause, next e previous tocam ou pausam e pulam a faixa; shuffle (`value` on/off), repeat (`value` off, context ou track), volume (`value` 0 a 100) e seek (`value` em segundos ou 1:23) mudam o player que toca; queue lista o que vem a seguir; play toca `uri` (spotify:track:…, spotify:playlist:…, spotify:album:…; `value` liked para as Músicas curtidas) ou o primeiro achado de `query` (`kind` playlist para a primeira playlist); enqueue põe na fila `uri` de uma música ou o primeiro achado de `query`; like e unlike põem ou tiram a música que toca (ou `uri`) das Músicas curtidas; search busca músicas e playlists por `query`; playlists lista as playlists da pessoa; liked as últimas Músicas curtidas; songs lista as músicas de uma playlist (`uri` a playlist ou `value` liked; com `value` um número, a partir dali, 50 por vez); play_in toca a música `uri` dentro da playlist `value` (spotify:playlist:… ou liked), e a lista segue depois dela, como um clique numa música da playlist aberta na ilha; devices lista os aparelhos do Spotify e device (`value` o nome ou o id) passa a música para um deles; dance_on e dance_off ligam e desligam "A Luna dança com a música" (na ilha, ela dança no ritmo do que toca: ouve o som do computador só para achar a batida, nada é gravado nem enviado; no macOS balança no ritmo do relógio) e dance diz se está ligado e o passo de agora. Fila de um player que não é o Soloist, busca, playlists, curtidas, coração e aparelhos precisam da conta do Spotify conectada: client_id (`value` o Client ID do app da pessoa no painel do Spotify) guarda o Client ID, connect abre o navegador no login do Spotify (a pessoa autoriza lá), disconnect desconecta e account diz como está; o token nunca passa por aqui. O Configurar do soloist (Linux, sem sudo): setup diz os 5 passos (baixar, chave, nome, salvar e ligar com o computador, escolher no celular), cada um feito, agora ou a fazer; update (ou install) baixa a versão nova do Spotify (cada uma vale 90 dias) e religa o serviço; enable liga o soloist.service do usuário com a chave já salva; start e stop ligam e param o serviço. A chave do Soloist nunca passa por aqui (key só responde isso): a pessoa cola em Ajustes › Integrações › Soloist › Configurar. `integration` hermes (o Hermes Agent na ilha) está desativado por enquanto: responde só que está desativado. `integration` whatsapp (gateway GOWA local): status diz se o gateway está conectado, precisa parear ou está indisponível; on e off; test testa a conexão com o gateway; open faz o que um clique na linha do WhatsApp na ilha faz (com o gateway conectado, abre a página dele no navegador; senão abre Ajustes › Integrações no WhatsApp); remove_credential tira a credencial do cofre do sistema. A credencial e a URL nunca passam por aqui: a pessoa cola em Ajustes › Integrações. Sem `integration`, status resume todas.

| Parâmetro | Tipo | O que é |
|---|---|---|
| `integration` | string: `github`, `spotify`, `soloist`, `hermes`, `whatsapp` |  |
| `action` | string: `status`, `on`, `off`, `test`, `remove_token`, `remove_credential`, `playpause`, `next`, `previous`, `dance`, `dance_on`, `dance_off`, `install`, `remove`, `setup`, `update`, `enable`, `start`, `stop`, `key`, `prs`, `pr`, `pr_body`, `pr_comment`, `pr_approve`, `pr_merge`, `shuffle`, `repeat`, `volume`, `seek`, `queue`, `search`, `play`, `enqueue`, `like`, `unlike`, `playlists`, `liked`, `songs`, `play_in`, `devices`, `device`, `client_id`, `connect`, `disconnect`, `account`, `open` |  |
| `filter` | string: `all`, `open`, `needs_you`, `closed` | github prs: quais listar (o padrão é all) |
| `pr` | string | github: o pull request, como dono/repo#42 ou https://github.com/dono/repo/pull/42 |
| `text` | string | github: a descrição nova inteira em Markdown (pr_body) ou o comentário (pr_comment) |
| `value` | string | o valor da ação do Spotify: on/off, off/context/track, 0 a 100, segundos, liked, o aparelho, o Client ID |
| `query` | string | o que buscar no Spotify (search, play, enqueue) |
| `uri` | string | o endereço do Spotify: spotify:track:…, spotify:playlist:…, spotify:album:… |
| `kind` | string: `track`, `playlist` | play com query: tocar a primeira música (o padrão) ou a primeira playlist |

### `voice_call`

A chamada de voz do chat aberto: on liga, off desliga, mute e unmute o microfone, speakers_off e speakers_on o som. dictate_on e dictate_off ligam e desligam o Ditar da caixa: o que a pessoa fala vira texto na caixa, sem enviar.

| Parâmetro | Tipo | O que é |
|---|---|---|
| `state` (obrigatório) | string: `on`, `off`, `mute`, `unmute`, `speakers_off`, `speakers_on`, `dictate_on`, `dictate_off` |  |

### `update`

Atualizações do MagoAgent: pede a verificação da última release no GitHub e diz em que pé está (a versão, se há uma mais nova, se faltou rede); com `install`, baixa o instalador da release encontrada, confere o sha256 do GitHub e atualiza: no Windows o app SALVA o trabalho, ENCERRA os terminais e agentes (inclusive este chat/agente e o `magoagent mcp` de cada Claude Code ou Codex), FECHA, instala e REABRE sozinho. Por isso `install` sozinho só avisa; avise a pessoa e, com o aval dela, chame de novo com `confirmed` true. Com `later`, o aviso da versão nova fecha e volta em 6 horas (ou quando o computador reiniciar), como "Atualizar depois"; com `skip`, como "Pular esta versão", ele só volta numa versão mais nova.

| Parâmetro | Tipo | O que é |
|---|---|---|
| `install` | boolean | baixar, conferir e instalar a atualização encontrada (o app fecha e reabre) |
| `confirmed` | boolean | a pessoa soube que o app vai fechar e reabrir e aceitou |
| `later` | boolean | Atualizar depois: o aviso volta em 6 horas ou quando o computador reiniciar |
| `skip` | boolean | Pular esta versão: o aviso só volta numa versão mais nova |

### `tour`

Abre na tela a apresentação do MagoAgent (a mesma de Ajustes › Geral › Ver a apresentação de novo): escurece a tela e destaca, passo a passo, a barra de ferramentas, o botão do chat, a lateral dos espaços, os cantos do canvas e Ajustes, com Voltar, Próximo e Pular. `step` começa num passo (0 é o primeiro, 5 o último).

| Parâmetro | Tipo | O que é |
|---|---|---|
| `step` | integer | o passo em que começa (0 a 6); sem ele, o primeiro |
