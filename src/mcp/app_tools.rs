//! The app's own actions as MCP tools: each goes to the open app by the bridge's `act` and runs in
//! app/actions.rs. Whatever the person can do in the app, the Maestro can do with one of these.

use super::*;

/// Tools that are app actions (dispatched as `act`).
pub(crate) fn is_action(name: &str) -> bool {
    app_tools().iter().any(|t| t["name"] == name)
}

pub(crate) fn app_tools() -> Vec<Value> {
    let name = |what: &str| json!({ "type": "string", "description": what });
    let chat = name("o nome do chat (ou parte dele), como chat_list mostra; vazio: o chat aberto");
    vec![
        json!({
            "name": "update",
            "description": "Atualizações do MagoAgent: pede a verificação da última release no GitHub e diz em que pé está (a versão, se há uma mais nova, se faltou rede); com `install`, baixa o instalador da release encontrada e o abre, e ele atualiza o app.",
            "inputSchema": { "type": "object", "properties": { "install": { "type": "boolean", "description": "baixar e abrir o instalador da atualização encontrada" } } },
        }),
        json!({ "name": "memory", "description": "Mostra a memória do MagoAgent e dos processos dos agentes por terminal.", "inputSchema": { "type": "object", "properties": {} } }),
        json!({
            "name": "ide_folder",
            "description": "Abre uma pasta na Árvore de Arquivos do canvas, ampliada, com o painel de código (usa a árvore que já está nela, ou cria uma), e devolve a estrutura de pastas e arquivos (3 níveis).",
            "inputSchema": { "type": "object", "properties": { "path": { "type": "string" } }, "required": ["path"] },
        }),
        json!({
            "name": "open_path",
            "description": "Abre à vista da pessoa um arquivo ou pasta pelo caminho: uma pasta na Árvore de Arquivos, um PDF, imagem ou página HTML num navegador do canvas, qualquer outro arquivo no painel de código da Árvore do projeto dele (na `line`, se dada).",
            "inputSchema": { "type": "object", "properties": {
                "path": { "type": "string" },
                "line": { "type": "number", "description": "linha a mostrar, a partir de 1" },
            }, "required": ["path"] },
        }),
        json!({
            "name": "open_default",
            "description": "Abre um arquivo ou pasta no programa padrão do sistema (programas executáveis, como .exe, .bat e .sh, não são iniciados).",
            "inputSchema": { "type": "object", "properties": { "path": { "type": "string" } }, "required": ["path"] },
        }),
        json!({
            "name": "canvas_drop",
            "description": "Solta arquivos e pastas no canvas, como arrastá-los para lá: .md e .txt viram nota com o conteúdo, .html um navegador, pasta uma árvore de arquivos, imagem (inclusive SVG) uma imagem, e qualquer outro arquivo um cartão com nome, tamanho e botões que o abrem. `x` e `y` (coordenadas do canvas) dizem onde; sem eles, no meio da vista.",
            "inputSchema": { "type": "object", "properties": {
                "paths": { "type": "array", "items": { "type": "string" } },
                "x": { "type": "number" },
                "y": { "type": "number" },
            }, "required": ["paths"] },
        }),
        json!({
            "name": "resize_window",
            "description": "Redimensiona uma janela: `target` é o nome de um terminal, navegador ou árvore de arquivos do canvas (state enlarge amplia, shrink volta ao canvas, ou width/height em pontos), ou `app` para a janela do MagoAgent (state maximize, restore, minimize, ou width/height).",
            "inputSchema": { "type": "object", "properties": {
                "target": { "type": "string" },
                "state": { "type": "string", "enum": ["enlarge", "shrink", "maximize", "restore", "minimize"] },
                "width": { "type": "number" },
                "height": { "type": "number" },
            }, "required": ["target"] },
        }),
        json!({
            "name": "close_window",
            "description": "Fecha (apaga do canvas) um terminal, navegador, árvore de arquivos ou nota pelo nome; dá para desfazer com history undo.",
            "inputSchema": { "type": "object", "properties": { "name": { "type": "string" } }, "required": ["name"] },
        }),
        json!({
            "name": "terminal_restart",
            "description": "Reinicia o terminal com esse nome (o shell e o comando dele começam de novo).",
            "inputSchema": { "type": "object", "properties": { "name": { "type": "string" } }, "required": ["name"] },
        }),
        json!({
            "name": "terminal_menu",
            "description": "As opções do cartão de um terminal, as mesmas do menu do botão direito: copy devolve (e põe na área de transferência) o que está selecionado, paste cola `text`, select_all e clear_selection, rename dá o nome `value`, monitor liga ou desliga (`value` on ou off) o aviso de quando o trabalho termina, blur esconde a tela do terminal atrás de um véu (off tira), reload e unload (descarrega o shell até recarregar), use_size, use_font e use_theme fazem do tamanho, do tamanho de fonte e do tema deste terminal os padrões dos novos, copy_element, duplicate, lock (`value` on ou off) e delete.",
            "inputSchema": { "type": "object", "properties": {
                "name": { "type": "string", "description": "o nome do terminal" },
                "action": { "type": "string", "enum": ["copy", "paste", "select_all", "clear_selection", "rename", "monitor", "blur", "reload", "unload", "use_size", "use_font", "use_theme", "copy_element", "duplicate", "lock", "delete"] },
                "value": { "type": "string", "description": "rename: o nome novo; monitor, blur, lock: on ou off" },
                "text": { "type": "string", "description": "paste: o texto" },
            }, "required": ["name", "action"] },
        }),
        json!({
            "name": "terminal_find",
            "description": "Busca texto na tela e no histórico de um terminal (a barra Buscar no Terminal): find procura `query` (texto; com `regex` true, uma regex; letras minúsculas ignoram a caixa) e diz quantos resultados há, marcando-os no terminal; next e previous vão de um a outro; close fecha a barra.",
            "inputSchema": { "type": "object", "properties": {
                "name": { "type": "string", "description": "o nome do terminal" },
                "action": { "type": "string", "enum": ["find", "next", "previous", "close"] },
                "query": { "type": "string" },
                "regex": { "type": "boolean" },
            }, "required": ["name"] },
        }),
        json!({
            "name": "terminal_look",
            "description": "O tema, o tamanho da fonte (só o tamanho: a fonte é sempre a Hack) e o shell de um terminal (`name`), ou os padrões de todos (sem `name`; também `copy_on_select`). `theme` é o id de um tema (padrão, system, nord, dracula…) ou inherit para voltar ao padrão; `font_size` vai de 8 a 32 pt; `shell` é o programa ou caminho (vazio: o padrão) e reinicia o terminal; `reset` devolve ao terminal os padrões; action list diz os temas e os shells que há.",
            "inputSchema": { "type": "object", "properties": {
                "name": { "type": "string", "description": "o nome do terminal; vazio: os padrões" },
                "action": { "type": "string", "enum": ["set", "list"] },
                "theme": { "type": "string" },
                "font_size": { "type": "number" },
                "shell": { "type": "string" },
                "copy_on_select": { "type": "boolean", "description": "só nos padrões: copiar o que se seleciona" },
                "reset": { "type": "boolean" },
            } },
        }),
        json!({
            "name": "terminal_processes",
            "description": "O que roda sob o shell de um terminal (o botão de processos do cartão): nome, PID e memória de cada um; `stop_pid` encerra um deles (só os daquele terminal).",
            "inputSchema": { "type": "object", "properties": {
                "name": { "type": "string", "description": "o nome do terminal" },
                "stop_pid": { "type": "number" },
            }, "required": ["name"] },
        }),
        json!({
            "name": "note_stack",
            "description": "Fichário (pilha de notas): `stack` junta as notas de `notes` numa pilha onde só a página da frente aparece, com uma aba por página à direita (se uma delas já está numa pilha, as outras entram nela); com a primeira nota de `notes`: `next` e `prev` viram a página, `show` traz aquela página para a frente, `new_page` cria uma página em branco na frente, `remove` tira aquela página da pilha e `unstack` espalha todas de volta. Valem para qualquer nota (a primeira de `notes`): `blur` desfoca o texto (Desfocar; `value` on/off) e `move_file` leva o arquivo .md dela para a `folder` (Mover Arquivo para…; `folder` vazia traz de volta).",
            "inputSchema": { "type": "object", "properties": {
                "action": { "type": "string", "enum": ["stack", "unstack", "next", "prev", "show", "new_page", "remove", "uniform", "blur", "move_file"] },
                "value": { "type": "string", "description": "blur: on, off (padrão: alterna)" },
                "folder": { "type": "string", "description": "move_file: a pasta para onde vai o arquivo .md da nota (vazio: de volta à pasta do app)" },
                "notes": { "type": "array", "items": { "type": "string" } },
            }, "required": ["action", "notes"] },
        }),
        json!({
            "name": "canvas_group",
            "description": "Grupos do canvas (o quadro tracejado com nome em volta de vários elementos; o menu \"Grupo\"): `create` agrupa os `elements` (ids ou nomes de nós; 2 ou mais) com o `name` (padrão \"Grupo\"), `rename` dá o `name` ao `group`, `color` pinta o `group` com um papel de nota (`color`: Amarelo, Rosa, Azul…, ou `none` para sem cor), `ungroup` desfaz o `group` (os elementos ficam), `list` mostra os grupos. `group` é o nome ou o id.",
            "inputSchema": { "type": "object", "properties": {
                "action": { "type": "string", "enum": ["list", "create", "rename", "color", "ungroup"] },
                "elements": { "type": "array", "items": { "type": "string" } },
                "group": { "type": "string" },
                "name": { "type": "string" },
                "color": { "type": "string" },
            }, "required": ["action"] },
        }),
        json!({
            "name": "portal_tab",
            "description": "As abas de um navegador: new abre uma aba com `url`, close fecha a aba `index` (vazio: a aberta), select mostra a aba `index` (a partir de 0).",
            "inputSchema": { "type": "object", "properties": {
                "portal": { "type": "string" },
                "action": { "type": "string", "enum": ["new", "close", "select"] },
                "url": { "type": "string" },
                "index": { "type": "number" },
            }, "required": ["portal", "action"] },
        }),
        json!({
            "name": "portal_settings",
            "description": "Ajustes de um navegador: `user_agent` (Agente de Usuário: Chrome, Firefox, Microsoft Edge, Safari (iPhone), Chrome (Android), Firefox (Android), Microsoft Edge (Android), \"Padrão\" ou o texto de um personalizado; recarrega o navegador), `storage` (Armazenamento: isolated, workspace ou global; recarrega o navegador), `muted` (Silenciar o áudio e o vídeo da página) e `size_readout` (Mostrar Tamanho: never, onresize ou always; vale para todos os navegadores).",
            "inputSchema": { "type": "object", "properties": {
                "portal": { "type": "string" },
                "user_agent": { "type": "string" },
                "storage": { "type": "string", "enum": ["isolated", "workspace", "global"] },
                "muted": { "type": "boolean" },
                "size_readout": { "type": "string", "enum": ["never", "onresize", "always"] },
            }, "required": ["portal"] },
        }),
        json!({
            "name": "portal_copy_screenshot",
            "description": "Copiar Captura de Tela: põe a imagem da página aberta do navegador na área de transferência da pessoa (só no Windows por enquanto). Para ver a página você mesmo, use portal_screenshot.",
            "inputSchema": { "type": "object", "properties": { "portal": { "type": "string" } }, "required": ["portal"] },
        }),
        json!({
            "name": "chat_list",
            "description": "Os chats do MagoAgent: nome, agente, pasta, se está respondendo e qual está aberto.",
            "inputSchema": { "type": "object", "properties": {} },
        }),
        json!({
            "name": "chat_new",
            "description": "Abre um chat novo e o mostra: `agent` claude ou codex (vazio: o de Ajustes), na pasta `folder` (vazio: a de Ajustes). O chat nasce com um cartão no canvas (chat_node).",
            "inputSchema": { "type": "object", "properties": {
                "agent": { "type": "string", "enum": ["claude", "codex"] },
                "folder": { "type": "string" },
            } },
        }),
        json!({
            "name": "chat_provider",
            "description": "Troca o provedor de um chat antes da primeira mensagem: `agent` claude ou codex.",
            "inputSchema": { "type": "object", "properties": {
                "chat": chat,
                "agent": { "type": "string", "enum": ["claude", "codex"] },
            }, "required": ["agent"] },
        }),
        json!({
            "name": "chat_node",
            "description": "O cartão de um chat no canvas: ali o chat é o Maestro (list_agents o chama de Maestro), e o que se liga a ele por cordas, terminais e outros chats (connect, disconnect), é o time que ele comanda e que o alcança com ask_agent. action show (o padrão) põe o cartão do chat no canvas deste espaço, se ainda não está, e mostra o canvas nele; new cria um chat novo (`agent` e `folder` como em chat_new) já com o cartão, e mostra o canvas; remove tira o cartão do canvas (a conversa continua na lista de chats); fold recolhe o cartão de um agente recrutado em chat ao nome e à tarefa, ou o abre de novo mostrando o que ele faz.",
            "inputSchema": { "type": "object", "properties": {
                "chat": chat,
                "action": { "type": "string", "enum": ["show", "new", "remove", "fold"] },
                "agent": { "type": "string", "enum": ["claude", "codex"] },
                "folder": { "type": "string" },
            } },
        }),
        json!({
            "name": "disconnect",
            "description": "Corta a corda entre duas coisas do canvas (terminais, chats, notas, navegadores, árvores de arquivos), pelo nome como list_agents mostra: deixam de se alcançar. O contrário de connect.",
            "inputSchema": { "type": "object", "properties": { "a": { "type": "string" }, "b": { "type": "string" } }, "required": ["a", "b"] },
        }),
        json!({
            "name": "chat_open",
            "description": "Mostra um chat à pessoa.",
            "inputSchema": { "type": "object", "properties": { "chat": chat }, "required": ["chat"] },
        }),
        json!({
            "name": "mini_chat",
            "description": "Abre (state open, o padrão) o chat em miniatura flutuante sobre o canvas, no chat `chat` (vazio: o último aberto), ou o recolhe para a pílula (state close).",
            "inputSchema": { "type": "object", "properties": { "chat": chat, "state": { "type": "string", "enum": ["open", "close"] } } },
        }),
        json!({
            "name": "chat_send",
            "description": "Manda uma mensagem a um chat, como se a pessoa a escrevesse lá; o agente dele responde no chat.",
            "inputSchema": { "type": "object", "properties": { "chat": chat, "text": { "type": "string" } }, "required": ["chat", "text"] },
        }),
        json!({
            "name": "chat_command",
            "description": "Roda um comando com barra do chat, como se a pessoa o digitasse na caixa: login (abre o navegador; console ou sso depois), logout, status, mcp, doctor, update, clear (conversa nova), model e effort (com o nome ou nível), help. A saída aparece no chat, sem ir ao modelo. Outros /comandos (compact, os de .claude/commands) são mensagens ao agente: use chat_send.",
            "inputSchema": { "type": "object", "properties": { "chat": chat, "command": { "type": "string", "description": "Ex.: \"status\", \"/model opus\", \"login sso\"" } }, "required": ["command"] },
        }),
        json!({
            "name": "chat_context",
            "description": "O painel Contexto do chat do Claude Code: open e close mostram ou escondem o painel da direita, refresh lê de novo; sempre devolve quanto da janela está em uso, o valor de compactar automaticamente, as categorias e os conectores (servidores MCP) com estado e quantidade de ferramentas.",
            "inputSchema": { "type": "object", "properties": { "chat": chat, "state": { "type": "string", "enum": ["open", "close", "refresh"] } } },
        }),
        json!({
            "name": "chat_search",
            "description": "Procura texto nas conversas carregadas, ignorando maiúsculas e acentos; `chat` pode ser nome ou id e `limit` vai de 1 a 50 (padrão 20).",
            "inputSchema": { "type": "object", "properties": {
                "query": { "type": "string", "description": "Texto a procurar" },
                "chat": { "type": "string", "description": "Nome ou id; vazio procura em todos os chats" },
                "limit": { "type": "integer", "minimum": 1, "maximum": 50, "default": 20 },
            }, "required": ["query"] },
        }),
        json!({
            "name": "skills",
            "description": "Lista as skills disponíveis para o Maestro ou abre a pasta delas no gerenciador de arquivos; action list (padrão) ou open.",
            "inputSchema": { "type": "object", "properties": {
                "action": { "type": "string", "enum": ["list", "open"], "default": "list" },
            } },
        }),
        json!({
            "name": "voice_call",
            "description": "A chamada de voz do chat aberto: on liga, off desliga, mute e unmute o microfone, speakers_off e speakers_on o som. dictate_on e dictate_off ligam e desligam o Ditar da caixa: o que a pessoa fala vira texto na caixa, sem enviar.",
            "inputSchema": { "type": "object", "properties": {
                "state": { "type": "string", "enum": ["on", "off", "mute", "unmute", "speakers_off", "speakers_on", "dictate_on", "dictate_off"] },
            }, "required": ["state"] },
        }),
        json!({
            "name": "maestro_comment",
            "description": "Comentar para o Maestro: envia ao chat do Maestro um comentário sobre linhas de um arquivo (caminho, trecho em bloco de código e o comentário). Vai ao chat ligado à Árvore, senão ao chat em uso, senão a um novo, e abre o mini chat nele. Sem `tree`, usa a Árvore selecionada ou a primeira.",
            "inputSchema": { "type": "object", "properties": {
                "tree": name("o nome da Árvore de Arquivos; vazio: a selecionada, ou a única"), "path": { "type": "string", "description": "relativo à pasta da Árvore" }, "from_line": { "type": "number" }, "to_line": { "type": "number" }, "comment": { "type": "string" }
            }, "required": ["path", "from_line", "to_line", "comment"] },
        }),
        json!({
            "name": "set_setting",
            "description": "Muda um ajuste do app: theme (light, dark, system), voice (codex, openai, local), terminal_history (1000, 5000, 10000), avatar (on, off: a barra do Maestro por cima de tudo na tela), avatar_follow (on: a barra segue a janela em uso; off: fica onde está) zoom_step (de 2 a 50: quantos pontos de zoom um passo da roda do mouse muda) ou favorite_folder (o caminho de uma pasta: entra nas favoritas do campo de pasta, no topo da lista Recentes, ou sai delas se já estava). Consumo: max_agents (quantos agentes podem rodar para o Maestro recrutar mais um; 0 tira o teto), usage_stop (a porcentagem da janela de 5 horas do plano a partir da qual o app não recruta; 0 nunca para) e agent_mode (terminal ou chat: como os agentes recrutados rodam, num terminal com o Claude Code completo ou num chat leve do app) e recruit_model (o modelo dos agentes recrutados, por exemplo sonnet ou haiku; cli usa o da CLI).",
            "inputSchema": { "type": "object", "properties": {
                "name": { "type": "string", "enum": ["theme", "voice", "terminal_history", "avatar", "avatar_follow", "favorite_folder", "zoom_step", "max_agents", "usage_stop", "agent_mode", "recruit_model"] },
                "value": { "type": "string" },
            }, "required": ["name", "value"] },
        }),
        json!({
            "name": "demands",
            "description": "O que espera a pessoa, do mais urgente ao menos: pedidos de permissão, perguntas, erros, agentes que terminaram ou encerraram; cada um com os pontos de atenção calculados dos dados (comando destrutivo, caminho fora da pasta do agente, rede, credenciais, parado há muito tempo). Com `agent` e `answer` (allow ou deny) responde ao pedido de permissão desse agente, se ele ainda espera pelo app (`reason` vai ao agente quando nega); só com `agent`, traz também os últimos eventos dele.",
            "inputSchema": { "type": "object", "properties": {
                "agent": { "type": "string", "description": "o nome do terminal ou do chat" },
                "answer": { "type": "string", "enum": ["allow", "deny"] },
                "reason": { "type": "string" },
            } },
        }),
        json!({
            "name": "connection_history",
            "description": "O que dois agentes conversaram pela conexão entre eles (ask_agent / magoagent ask): as últimas mensagens, das mais antigas às mais novas, cada uma com quando, o sentido (quem → quem) e o resumo (a pergunta, ou como a resposta voltou).",
            "inputSchema": { "type": "object", "properties": {
                "a": { "type": "string", "description": "o nome de um dos terminais" },
                "b": { "type": "string", "description": "o nome do outro" },
                "limit": { "type": "number", "description": "quantas mensagens, no máximo (padrão 10, até 50)" },
            }, "required": ["a", "b"] },
        }),
        json!({
            "name": "usage",
            "description": "O consumo: quanto do limite do plano já foi (janela de 5 horas, com a hora do reinício, e semana), quantos agentes rodam e o teto, e o modelo, o contexto e o custo estimado de cada agente. Vem do que as sessões do Claude Code informam; sem leitura ainda, diz isso. Consulte antes de recrutar ou de delegar algo grande.",
            "inputSchema": { "type": "object", "properties": {} },
        }),
        json!({
            "name": "usage_refresh",
            "description": "Lê de novo o uso dos planos (o botão Atualizar do popover Uso dos Agentes): pergunta ao Claude Code e ao Codex quanto da sessão e da semana já foi. O app já faz isso a cada 5 minutos; use quando o número estiver velho. A resposta vem em alguns segundos: depois consulte `usage`.",
            "inputSchema": { "type": "object", "properties": {} },
        }),
        json!({
            "name": "timeline",
            "description": "A linha do tempo dos agentes (\"o que aconteceu na última hora?\", \"o que o Revisor fez hoje?\"): os eventos recentes, do mais antigo ao mais novo, com a hora local na frente; filtra por agente, tipo e minutos. Com mais de 5 eventos, começa por um resumo das contagens (ferramentas, pedidos de permissão, erros, agentes).",
            "inputSchema": { "type": "object", "properties": {
                "agent": { "type": "string", "description": "nome ou id do agente" },
                "kind": { "type": "string", "enum": ["tool_started", "tool_ended", "permission", "question", "working", "finished", "error", "started", "ended", "message", "task"] },
                "minutes": { "type": "number", "description": "janela em minutos (1 a 1440)" },
                "limit": { "type": "number", "description": "quantidade máxima (padrão 20, de 1 a 100)" }
            } },
        }),
        json!({
            "name": "history",
            "description": "Desfaz (undo) ou refaz (redo) a última mudança no canvas.",
            "inputSchema": { "type": "object", "properties": { "action": { "type": "string", "enum": ["undo", "redo"] } }, "required": ["action"] },
        }),
        json!({
            "name": "minimap",
            "description": "Abre ou fecha o minimapa do canvas (o painel no canto de baixo à direita). Sem `open`, alterna.",
            "inputSchema": { "type": "object", "properties": { "open": { "type": "boolean" } } },
        }),
        json!({
            "name": "file_tree",
            "description": "A árvore de arquivos do canvas (a árvore de uma pasta, com o estado do git de cada arquivo). create cria uma na pasta `path`; list mostra a pasta, o ramo e as linhas à vista (> pasta fechada, v aberta; M modificado, A adicionado, D excluído, R renomeado, U não rastreado, ! em conflito); expand e collapse abrem e fecham a pasta `path`; collapse_all recolhe tudo; navigate entra na pasta `path` e back volta para a de cima (forward desfaz); refresh lê de novo; open abre o arquivo `path` como o clique duplo (no painel de código da árvore); root troca a pasta raiz por `path`; hidden mostra ou esconde os arquivos ocultos (`show`); sort ordena por `by` (`reverse` inverte); new_file e new_folder criam `name` na pasta `path`; rename renomeia `path` para `name`; delete manda `path` para a lixeira; copy, cut e paste copiam, recortam e colam `path`; editor mostra, esconde ou alterna (`show`) o painel de edição à direita da árvore (open abre o arquivo `path` nele: texto editável, foto ou PDF), save salva o arquivo da aba à frente e close_file fecha a aba de `path`; changes lista os arquivos alterados contra o HEAD (uma linha cada: a letra do git e o caminho; show true abre o painel Alterações na árvore, show false fecha; com `path` abre o diff daquele arquivo); annotate liga (show false desliga) a coluna de autor e data de cada linha do arquivo `path`, aberto no painel; branches lista os ramos locais do repositório (o atual com *) e checkout `name` troca para um deles (recusa, dizendo por quê, se há mudanças que a troca sobrescreveria); search procura arquivos pelo nome sob a pasta mostrada (`query`; vazio limpa a busca) e devolve as ocorrências com a pasta de cada uma; find_in_files é o Localizar nos Arquivos (Ctrl+Shift+F): procura o texto `query` nos arquivos da raiz, sem as pastas excluídas (`case` diferencia maiúsculas, `words` só palavras inteiras, `regex` expressão regular, `mask` limita os arquivos, ex.: *.rs, e `path` limita a uma pasta), abre a janela com a mesma busca e devolve as ocorrências (arquivo:linha: texto); usages lista os usos do nome `name` declarado ou usado no arquivo `path`, como o Ctrl+clique na declaração (arquivo:linha: texto); history lista os commits que mexeram na declaração `name` do arquivo `path` (ou nas linhas `from` a `to` dele), com autor, quando e a mensagem, e com `commit` (o hash) devolve o que esse commit mudou no arquivo e abre isso numa aba; diff abre numa aba o diff lado a lado do arquivo `path` contra o HEAD (como Ctrl+D), com destaque por linha e por palavra, e lista as mudanças; hunks só lista as faixas alteradas do arquivo contra o HEAD (as mesmas dos marcadores verdes, azuis e cinza da margem do editor: número, tipo e linhas); revert_hunk desfaz a mudança de número `index` (a de hunks, a partir de 1) e grava o arquivo, recusando se ele tiver mudanças não salvas no editor; problems lista os problemas (erros e avisos, como o IntelliJ: o servidor de linguagem, quando instalado, e a checagem de sintaxe local de PHP, JS/TS, JSON, Rust, Go, Python, C/C++, C#, Java, Kotlin, Swift, Dart e CSS: parêntese, chave, colchete, aspas ou comentário aberto e JSON inválido) do arquivo `path` ou, sem `path` (ou com uma pasta), dos arquivos da raiz, com a linha e a coluna (a primeira é 1) e a mensagem de cada um; no editor eles aparecem sublinhados em ondas, com a contagem no canto e marcas na barra de rolagem, F2 e Shift+F2 vão ao próximo e ao anterior, e na árvore o arquivo e as pastas dele ficam em vermelho; open com `line` abre o arquivo nessa linha; abrir na árvore um arquivo .db, .sqlite ou .sqlite3 o torna uma fonte do painel Banco de Dados; db mostra, esconde ou alterna (`show`) o painel das fontes de dados sobre as linhas da árvore, db_add acrescenta o arquivo SQLite `path` como fonte, db_sources lista as fontes, db_tables lista as tabelas e colunas de `source` (nome ou id; vazio: a única), db_open abre numa aba a grade da tabela `name` de `source` (sem `name`, um console SQL) e db_query roda em `source` a consulta de leitura `query` (só SELECT, uma instrução; escrita é recusada) e devolve até `limit` linhas; close fecha a árvore. Conflitos de um merge ou de um stash na pasta da árvore: conflicts lista os arquivos em conflito e, com `path`, os conflitos desse arquivo numerados, com as linhas de cada lado; resolve resolve o arquivo `path` e já o marca como resolvido (git add): `side` mine fica com a minha versão inteira e theirs com a deles, ou `picks` responde conflito por conflito, na ordem (as alterações sem conflito dos dois lados entram sozinhas); merge abre para a pessoa a janela de mesclagem de `path`, com as três versões lado a lado, ou, sem `path`, a janela Conflitos com a lista. `tree` é o nome da pasta, o caminho ou o id da árvore (vazio: a selecionada ou a única). `path` sem caminho completo vale dentro da pasta mostrada.",
            "inputSchema": { "type": "object", "properties": {
                "action": { "type": "string", "enum": ["create", "list", "expand", "collapse", "collapse_all", "navigate", "back", "forward", "refresh", "open", "root", "hidden", "sort", "new_file", "new_folder", "rename", "delete", "copy", "cut", "paste", "search", "branches", "checkout", "annotate", "changes", "find_in_files", "usages", "history", "problems", "diff", "hunks", "revert_hunk", "editor", "save", "close_file", "db", "db_add", "db_sources", "db_tables", "db_query", "db_open", "close", "conflicts", "resolve", "merge"] },
                "tree": name("o nome da pasta da árvore, o caminho dela ou o id; vazio: a ligada a você por corda, a selecionada, ou a única do canvas"),
                "path": { "type": "string" },
                "name": { "type": "string", "description": "new_file, new_folder, rename: o nome; checkout: o ramo; usages, history: o nome declarado" },
                "query": { "type": "string", "description": "search: parte do nome do arquivo; vazio limpa a busca. find_in_files: o texto a localizar. db_query: use `query` para a consulta SELECT" },
                "case": { "type": "boolean", "description": "find_in_files: diferencia maiúsculas de minúsculas" },
                "words": { "type": "boolean", "description": "find_in_files: só palavras inteiras" },
                "regex": { "type": "boolean", "description": "find_in_files: `query` é uma expressão regular" },
                "mask": { "type": "string", "description": "find_in_files: máscara de arquivo, ex.: *.rs, *.toml" },
                "source": { "type": "string", "description": "db_tables, db_open, db_query: o nome da fonte de dados ou o id; vazio: a única" },
                "limit": { "type": "integer", "description": "db_query: o máximo de linhas (padrão 100)" },
                "line": { "type": "integer", "description": "open: a linha (a primeira é 1); annotate: a linha cujo commit se quer" },
                "index": { "type": "integer", "description": "revert_hunk: o número da mudança, como hunks a lista (a primeira é 1)" },
                "from": { "type": "integer", "description": "history: a primeira linha do trecho (a primeira do arquivo é 1)" },
                "to": { "type": "integer", "description": "history: a última linha do trecho" },
                "commit": { "type": "string", "description": "history: o hash do commit cujo diff do arquivo se quer" },
                "show": { "type": "boolean", "description": "hidden: true mostra os arquivos ocultos, false esconde" },
                "by": { "type": "string", "enum": ["name", "kind", "modified", "created", "size"] },
                "reverse": { "type": "boolean", "description": "sort: ordem invertida" },
                "side": { "type": "string", "enum": ["mine", "theirs"], "description": "resolve: o arquivo inteiro fica com a minha versão ou com a deles" },
                "picks": { "type": "array", "items": { "type": "string", "enum": ["mine", "theirs", "both", "both_theirs_first", "base"] }, "description": "resolve: uma escolha por conflito do arquivo, na ordem em que conflicts os lista" },
            }, "required": ["action"] },
        }),
        json!({
            "name": "tasks",
            "description": "As tarefas: o que foi pedido a quem, em que estado está e do que depende. create cria (title; owner é o nome de um terminal ou chat; send true entrega o texto da tarefa ao responsável, no terminal ou no chat dele, com o id e como avisar o estado); update muda state, note (por que bloqueou ou falhou), owner, title, description, parent, deps (substitui) ou artifacts (acrescenta), e send true entrega de novo; list lista uma tarefa por linha, as travadas primeiro, filtrando por state e owner; get traz uma inteira, com os pontos de atenção. Regras: concluída, falhou e cancelada só reabrem para pending; in_progress com dependência não concluída fica blocked com a razão e volta a pending quando as dependências concluem; ciclo de dependências e tarefa-mãe inexistente são recusados. Um agente num terminal usa esta mesma ferramenta para dizer em que pé está a tarefa dele.",
            "inputSchema": { "type": "object", "properties": {
                "action": { "type": "string", "enum": ["create", "update", "list", "get"] },
                "id": { "type": "string", "description": "update, get: o id da tarefa (t1, t2…)" },
                "title": { "type": "string" },
                "description": { "type": "string" },
                "owner": { "type": "string", "description": "o nome do terminal ou do chat responsável; em list, filtra por ele" },
                "state": { "type": "string", "enum": ["pending", "in_progress", "waiting_agent", "waiting_approval", "in_review", "in_test", "blocked", "done", "failed", "cancelled"], "description": "update: o estado novo; list: filtra por ele" },
                "note": { "type": "string", "description": "por que está nesse estado (por que bloqueou, por que falhou)" },
                "parent": { "type": "string", "description": "o id da tarefa-mãe; vazio tira" },
                "deps": { "type": "array", "items": { "type": "string" }, "description": "os ids das tarefas de que esta depende" },
                "artifacts": { "type": "array", "items": { "type": "string" }, "description": "arquivos, diff, navegador ou conversa que a tarefa produziu" },
                "requester": { "type": "string", "description": "create: quem pediu, se não foi você" },
                "send": { "type": "boolean", "description": "entrega a tarefa ao responsável" },
            }, "required": ["action"] },
        }),
        json!({
            "name": "binder",
            "description": "O fichário: notas com nome, em Markdown, guardadas entre as sessões (decisões, pendências, contexto de um projeto), para o Maestro e os agentes lerem e escreverem. Não são as notas adesivas do canvas. list traz o nome, o tamanho e a primeira linha de cada nota; read lê uma; write grava (substitui) e append acrescenta ao fim, até 64 KB por nota; delete tira a nota do fichário e a guarda na pasta recuperados.",
            "inputSchema": { "type": "object", "properties": {
                "action": { "type": "string", "enum": ["list", "read", "write", "append", "delete"] },
                "name": { "type": "string", "description": "o nome da nota (vira um nome de arquivo seguro: sem acentos, minúsculo)" },
                "text": { "type": "string", "description": "write, append: o texto, em Markdown" },
            }, "required": ["action"] },
        }),
        json!({
            "name": "workspaces",
            "description": "Os espaços de trabalho, cada um um projeto com o seu desenho, a sua pasta raiz e uma cor: list mostra todos (o que está na tela e quantos agentes rodam e precisam da pessoa em cada um); switch mostra o espaço `name`; create cria o espaço `name` na pasta `folder` e o mostra; rename dá a ele o nome `new_name`; color troca a cor (`color`, #rrggbb); remove tira o espaço da lista e encerra os terminais dele (o desenho fica no disco; o último espaço não sai). Trocar de espaço não para os agentes: os terminais dos outros espaços continuam rodando.",
            "inputSchema": { "type": "object", "properties": {
                "action": { "type": "string", "enum": ["list", "switch", "create", "rename", "color", "remove"] },
                "name": { "type": "string", "description": "o nome do espaço (o novo, em create)" },
                "new_name": { "type": "string", "description": "rename: o nome novo" },
                "folder": { "type": "string", "description": "create: a pasta raiz, onde os terminais e árvores novos dele começam" },
                "color": { "type": "string", "description": "color: a cor nova, #rrggbb" },
            }, "required": ["action"] },
        }),
        json!({
            "name": "floors",
            "description": "Os andares do espaço de trabalho na tela: o Térreo (o desenho do espaço) e os andares acima dele, cada um um desenho completo com nome, cor, câmera e nós. list mostra todos (o que está na tela e quantos itens); switch mostra o andar `name` (\"Térreo\" volta ao térreo); create cria o andar `name` e o mostra (`clone_ground` copia o layout do Térreo); rename dá a ele o nome `new_name`; delete apaga o andar e tudo o que está nele; move põe o andar na `position` (1 é o logo acima do Térreo). Trocar de andar não para os agentes: os terminais dos andares fora da vista continuam rodando.",
            "inputSchema": { "type": "object", "properties": {
                "action": { "type": "string", "enum": ["list", "switch", "create", "rename", "delete", "move"] },
                "name": { "type": "string", "description": "o nome do andar (o novo, em create)" },
                "new_name": { "type": "string", "description": "rename: o nome novo" },
                "clone_ground": { "type": "boolean", "description": "create: copia o layout do Térreo para o andar novo" },
                "position": { "type": "integer", "description": "move: a posição, contada do Térreo para cima (1 é o primeiro andar acima)" },
            }, "required": ["action"] },
        }),
        json!({
            "name": "workspace_groups",
            "description": "Os grupos da barra lateral dos espaços de trabalho (Novo grupo): um título com nome, que recolhe e expande, sob o qual ficam espaços. list mostra os grupos e o que há em cada um; create cria o grupo `name`; rename dá a ele `new_name`; toggle recolhe ou expande; remove apaga o grupo (os espaços dele ficam sem grupo, nenhum é removido); move põe o espaço `space` dentro do grupo `group`, ou, sem `group`, fora de qualquer grupo.",
            "inputSchema": { "type": "object", "properties": {
                "action": { "type": "string", "enum": ["list", "create", "rename", "toggle", "remove", "move"] },
                "name": { "type": "string", "description": "o nome do grupo (o novo, em create)" },
                "new_name": { "type": "string", "description": "rename: o nome novo" },
                "space": { "type": "string", "description": "move: o nome do espaço" },
                "group": { "type": "string", "description": "move: o nome do grupo; vazio tira o espaço do grupo" },
            }, "required": ["action"] },
        }),
        json!({
            "name": "workspace_menu",
            "description": "O menu de um espaço de trabalho: pin e unpin (Fixar e Desfixar: o espaço fixado vai para Fixados, no alto da lista); duplicate (cópia dele e do desenho, \"<nome> cópia\"); copy_path (devolve a pasta raiz); export (grava o espaço, com o desenho, no arquivo `path`); import (cria um espaço novo do arquivo `path`, com os terminais e navegadores descarregados); unload (Descarregar: encerra os terminais e navegadores do espaço sem tirá-los do desenho; Recarregar num deles o inicia).",
            "inputSchema": { "type": "object", "properties": {
                "action": { "type": "string", "enum": ["pin", "unpin", "duplicate", "copy_path", "export", "import", "unload"] },
                "name": { "type": "string", "description": "o nome do espaço (não vale em import)" },
                "path": { "type": "string", "description": "export, import: o arquivo" },
            }, "required": ["action"] },
        }),
    ]
}
