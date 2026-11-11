//! The app's own actions as MCP tools: each goes to the open app by the bridge's `act` and runs in
//! app/actions.rs. Whatever the person can do in the app, the Maestro can do with one of these.

use super::*;

/// Tools that are app actions (dispatched as `act`).
pub(crate) fn is_action(name: &str) -> bool {
    app_tools().iter().any(|t| t["name"] == name)
}

pub(crate) fn app_tools() -> Vec<Value> {
    let name = |what: &str| json!({ "type": "string", "description": what });
    let ide = name("o nome do IDE; vazio: o que está ampliado, ou o único do canvas");
    let chat = name("o nome do chat (ou parte dele), como chat_list mostra; vazio: o chat aberto");
    vec![
        json!({ "name": "memory", "description": "Mostra a memória do MagoAgent e dos processos dos agentes por terminal.", "inputSchema": { "type": "object", "properties": {} } }),
        json!({
            "name": "ide_folder",
            "description": "Abre uma pasta no IDE do canvas, ampliado, com a árvore de arquivos à vista (usa o IDE que já está nela, ou cria um), e devolve a estrutura de pastas e arquivos (3 níveis).",
            "inputSchema": { "type": "object", "properties": { "path": { "type": "string" } }, "required": ["path"] },
        }),
        json!({
            "name": "open_path",
            "description": "Abre à vista da pessoa um arquivo ou pasta pelo caminho: uma pasta no IDE, um PDF, imagem ou página HTML num portal do canvas, qualquer outro arquivo no IDE do projeto dele (na `line`, se dada).",
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
            "description": "Solta arquivos e pastas no canvas, como arrastá-los para lá: .md e .txt viram nota com o conteúdo, .html um portal, pasta uma árvore de arquivos, imagem (inclusive SVG) uma imagem, e qualquer outro arquivo um cartão com nome, tamanho e botões que o abrem. `x` e `y` (coordenadas do canvas) dizem onde; sem eles, no meio da vista.",
            "inputSchema": { "type": "object", "properties": {
                "paths": { "type": "array", "items": { "type": "string" } },
                "x": { "type": "number" },
                "y": { "type": "number" },
            }, "required": ["paths"] },
        }),
        json!({
            "name": "resize_window",
            "description": "Redimensiona uma janela: `target` é o nome de um terminal, portal ou IDE do canvas (state enlarge amplia, shrink volta ao canvas, ou width/height em pontos), ou `app` para a janela do MagoAgent (state maximize, restore, minimize, ou width/height).",
            "inputSchema": { "type": "object", "properties": {
                "target": { "type": "string" },
                "state": { "type": "string", "enum": ["enlarge", "shrink", "maximize", "restore", "minimize"] },
                "width": { "type": "number" },
                "height": { "type": "number" },
            }, "required": ["target"] },
        }),
        json!({
            "name": "close_window",
            "description": "Fecha (apaga do canvas) um terminal, portal, IDE ou nota pelo nome; dá para desfazer com history undo.",
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
            "name": "portal_tab",
            "description": "As abas de um portal: new abre uma aba com `url`, close fecha a aba `index` (vazio: a aberta), select mostra a aba `index` (a partir de 0).",
            "inputSchema": { "type": "object", "properties": {
                "portal": { "type": "string" },
                "action": { "type": "string", "enum": ["new", "close", "select"] },
                "url": { "type": "string" },
                "index": { "type": "number" },
            }, "required": ["portal", "action"] },
        }),
        json!({
            "name": "portal_settings",
            "description": "Ajustes de um portal: `user_agent` (Agente de Usuário: Chrome, Firefox, Microsoft Edge, Safari (iPhone), Chrome (Android), Firefox (Android), Microsoft Edge (Android), \"Padrão\" ou o texto de um personalizado; recarrega o portal), `storage` (Armazenamento: isolated, workspace ou global; recarrega o portal), `muted` (Silenciar o áudio e o vídeo da página) e `size_readout` (Mostrar Tamanho: never, onresize ou always; vale para todos os portais).",
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
            "description": "Copiar Captura de Tela: põe a imagem da página aberta do portal na área de transferência da pessoa (só no Windows por enquanto). Para ver a página você mesmo, use portal_screenshot.",
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
            "description": "O cartão de um chat no canvas: ali o chat é o Maestro (list_agents o chama de Maestro), e o que se liga a ele por cordas, terminais e outros chats (connect, disconnect), é o time que ele comanda e que o alcança com ask_agent. action show (o padrão) põe o cartão do chat no canvas deste espaço, se ainda não está, e mostra o canvas nele; new cria um chat novo (`agent` e `folder` como em chat_new) já com o cartão, e mostra o canvas; remove tira o cartão do canvas (a conversa continua na lista de chats).",
            "inputSchema": { "type": "object", "properties": {
                "chat": chat,
                "action": { "type": "string", "enum": ["show", "new", "remove"] },
                "agent": { "type": "string", "enum": ["claude", "codex"] },
                "folder": { "type": "string" },
            } },
        }),
        json!({
            "name": "disconnect",
            "description": "Corta a corda entre duas coisas do canvas (terminais, chats, notas, portais, IDEs), pelo nome como list_agents mostra: deixam de se alcançar. O contrário de connect.",
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
            "name": "ide_git",
            "description": "Git pelo IDE, à vista: panel abre o painel de commit com o diff; commit salva as abas e faz commit com `message` só das mudanças de arquivos já versionados, ou só dos `files` nomeados (inclusive não versionados), e diz o que entrou e quais não versionados ficaram de fora; push e pull; diff abre o painel de commit comparando com `base` (ramo, etiqueta ou commit; vazio é o HEAD) no modo `mode` (side, lado a lado; unified, unificado). Ramos: branches lista os ramos locais e remotos, o atual e o que falta enviar ou receber; branch_create cria `name` a partir de `from` (vazio: o atual) e troca para ele, a menos que `checkout` seja false; checkout troca para `name` (um ramo remoto vira local); branch_delete exclui o ramo local `name` e pede confirm; fetch busca dos remotos; publish envia o ramo atual ao remoto pela primeira vez (push -u). Merge: merge mescla o ramo `name` no atual e, se houver conflitos, os arquivos ficam marcados no IDE e na lista do painel Histórico, para resolver no editor (ide_edit); merge_finish conclui o merge depois de resolvidos; merge_abort desfaz o merge e pede confirm. Stash: stash guarda as mudanças (com `message`; `untracked` leva os não versionados), stash_list lista, stash_apply e stash_pop trazem de volta o de número `index` (0 é o mais novo; pop remove da lista), stash_drop o joga fora e pede confirm. Histórico: log lista os últimos `limit` commits de todos os ramos; show mostra o commit `name` no painel Histórico com os arquivos que mudaram e, com `path`, abre o diff desse arquivo nele. `confirm` só vale true depois de a pessoa dizer sim; nada aqui força push nem faz reset. Recusa se o git do IDE estiver ocupado ou uma aba não puder ser salva. O resultado aparece no IDE.",
            "inputSchema": { "type": "object", "properties": {
                "ide": ide,
                "action": { "type": "string", "enum": [
                    "panel", "commit", "push", "pull", "diff", "branches", "fetch", "publish", "checkout", "branch_create", "branch_delete", "merge", "merge_abort", "merge_finish",
                    "stash", "stash_list", "stash_apply", "stash_pop", "stash_drop", "log", "show"
                ] },
                "message": { "type": "string" },
                "files": { "type": "array", "items": { "type": "string" }, "description": "commit: só estes arquivos, relativos à pasta do IDE; vazio: as mudanças dos já versionados" },
                "base": { "type": "string", "description": "diff: a revisão com que comparar (main, v0.0.8, a1b2c3d); vazio é o HEAD" },
                "mode": { "type": "string", "enum": ["side", "unified"], "description": "diff: lado a lado ou unificado; sem ele, fica como está" },
                "name": { "type": "string", "description": "checkout, branch_create, branch_delete, merge: o ramo; show: o commit (ramo, etiqueta ou hash)" },
                "from": { "type": "string", "description": "branch_create: o ramo ou commit de onde sair; vazio é o atual" },
                "checkout": { "type": "boolean", "description": "branch_create: trocar para o ramo novo (padrão: sim)" },
                "index": { "type": "integer", "description": "stash_apply, stash_pop, stash_drop: o número do stash (0 é o mais novo)" },
                "untracked": { "type": "boolean", "description": "stash: levar os arquivos não versionados" },
                "confirm": { "type": "boolean", "description": "branch_delete, merge_abort, stash_drop: true só depois de a pessoa confirmar" },
                "limit": { "type": "integer", "description": "log: quantos commits (padrão 20, no máximo 200)" },
                "path": { "type": "string", "description": "show: abre o diff deste arquivo (relativo à pasta do IDE) naquele commit" },
            }, "required": ["action"] },
        }),
        json!({
            "name": "ide_conflict",
            "description": "Resolve o conflito de um arquivo aberto no IDE (mudanças não salvas na aba e o arquivo mudou no disco): reload descarta as mudanças da aba e recarrega do disco; keep grava a versão da aba por cima do disco. `path` vazio é a aba aberta. Só vale para uma aba em conflito.",
            "inputSchema": { "type": "object", "properties": {
                "ide": ide,
                "path": { "type": "string", "description": "arquivo; vazio: a aba aberta" },
                "action": { "type": "string", "enum": ["reload", "keep"] },
            }, "required": ["action"] },
        }),
        json!({
            "name": "code_comment",
            "description": "Envia ao agente ligado ao IDE um comentário estruturado sobre linhas de um arquivo do projeto.",
            "inputSchema": { "type": "object", "properties": {
                "ide": ide, "path": { "type": "string" }, "from": { "type": "number" }, "to": { "type": "number" }, "comment": { "type": "string" }
            }, "required": ["path", "from", "to", "comment"] },
        }),
        json!({
            "name": "ide_database",
            "description": "Abre (ou fecha) o painel Banco de dados do IDE.",
            "inputSchema": { "type": "object", "properties": { "ide": ide } },
        }),
        json!({
            "name": "ide_exclude",
            "description": "Pastas excluídas do IDE, como \"Marcar Diretório como Excluído\" do PhpStorm: ficam fora da busca (Shift Shift, Localizar nos arquivos, ide_search) e do índice, mas continuam na árvore, em laranja. list mostra a lista do projeto (padrão: node_modules, vendor, target, dist, build, .git, storage e afins, mais as pastas do .gitignore); exclude marca `path` (relativo ao projeto ou completo); include cancela a exclusão.",
            "inputSchema": { "type": "object", "properties": {
                "ide": ide,
                "action": { "type": "string", "enum": ["list", "exclude", "include"] },
                "path": { "type": "string", "description": "a pasta" },
            }, "required": ["action"] },
        }),
        json!({
            "name": "set_setting",
            "description": "Muda um ajuste do app: theme (light, dark, system), voice (codex, openai, local), terminal_history (1000, 5000, 10000), avatar (on, off: a barra do Maestro por cima de tudo na tela), avatar_follow (on: a barra segue a janela em uso; off: fica onde está) zoom_step (de 2 a 50: quantos pontos de zoom um passo da roda do mouse muda) ou favorite_folder (o caminho de uma pasta: entra nas favoritas do campo de pasta, no topo da lista Recentes, ou sai delas se já estava). Consumo: max_agents (quantos agentes podem rodar para o Maestro recrutar mais um; 0 tira o teto), usage_stop (a porcentagem da janela de 5 horas do plano a partir da qual o app não recruta; 0 nunca para) e recruit_model (o modelo dos agentes recrutados, por exemplo sonnet ou haiku; cli usa o da CLI).",
            "inputSchema": { "type": "object", "properties": {
                "name": { "type": "string", "enum": ["theme", "voice", "terminal_history", "avatar", "avatar_follow", "favorite_folder", "zoom_step", "max_agents", "usage_stop", "recruit_model"] },
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
            "description": "A árvore de arquivos do canvas (a árvore de uma pasta, com o estado do git de cada arquivo). create cria uma na pasta `path`; list mostra a pasta, o ramo e as linhas à vista (> pasta fechada, v aberta; M modificado, A adicionado, D excluído, R renomeado, U não rastreado, ! em conflito); expand e collapse abrem e fecham a pasta `path`; collapse_all recolhe tudo; navigate entra na pasta `path` e back volta para a de cima (forward desfaz); refresh lê de novo; open abre o arquivo `path` como o clique duplo (no IDE do projeto, ou no programa padrão); root troca a pasta raiz por `path`; hidden mostra ou esconde os arquivos ocultos (`show`); sort ordena por `by` (`reverse` inverte); new_file e new_folder criam `name` na pasta `path`; rename renomeia `path` para `name`; delete manda `path` para a lixeira; copy, cut e paste copiam, recortam e colam `path`; editor mostra, esconde ou alterna (`show`) o painel de edição à direita da árvore (open abre o arquivo `path` nele: texto editável, foto ou PDF), save salva o arquivo da aba à frente e close_file fecha a aba de `path`; search procura arquivos pelo nome sob a pasta mostrada (`query`; vazio limpa a busca) e devolve as ocorrências com a pasta de cada uma; find_in_files é o Localizar nos Arquivos (Ctrl+Shift+F): procura o texto `query` nos arquivos da raiz, sem as pastas excluídas (`case` diferencia maiúsculas, `words` só palavras inteiras, `regex` expressão regular, `mask` limita os arquivos, ex.: *.rs, e `path` limita a uma pasta), abre a janela com a mesma busca e devolve as ocorrências (arquivo:linha: texto); usages lista os usos do nome `name` declarado ou usado no arquivo `path`, como o Ctrl+clique na declaração (arquivo:linha: texto); history lista os commits que mexeram na declaração `name` do arquivo `path` (ou nas linhas `from` a `to` dele), com autor, quando e a mensagem, e com `commit` (o hash) devolve o que esse commit mudou no arquivo e abre isso numa aba; diff abre numa aba o diff lado a lado do arquivo `path` contra o HEAD (como Ctrl+D), com destaque por linha e por palavra, e lista as mudanças; hunks só lista as faixas alteradas do arquivo contra o HEAD (as mesmas dos marcadores verdes, azuis e cinza da margem do editor: número, tipo e linhas); revert_hunk desfaz a mudança de número `index` (a de hunks, a partir de 1) e grava o arquivo, recusando se ele tiver mudanças não salvas no editor; problems lista os problemas (erros e avisos, como o IntelliJ: o servidor de linguagem, quando instalado, e a checagem de sintaxe local de PHP, JS/TS, JSON, Rust, Go, Python, C/C++, C#, Java, Kotlin, Swift, Dart e CSS: parêntese, chave, colchete, aspas ou comentário aberto e JSON inválido) do arquivo `path` ou, sem `path` (ou com uma pasta), dos arquivos da raiz, com a linha e a coluna (a primeira é 1) e a mensagem de cada um; no editor eles aparecem sublinhados em ondas, com a contagem no canto e marcas na barra de rolagem, F2 e Shift+F2 vão ao próximo e ao anterior, e na árvore o arquivo e as pastas dele ficam em vermelho; open com `line` abre o arquivo nessa linha; abrir na árvore um arquivo .db, .sqlite ou .sqlite3 o torna uma fonte do painel Banco de Dados; db mostra, esconde ou alterna (`show`) o painel das fontes de dados sobre as linhas da árvore, db_add acrescenta o arquivo SQLite `path` como fonte, db_sources lista as fontes, db_tables lista as tabelas e colunas de `source` (nome ou id; vazio: a única), db_open abre numa aba a grade da tabela `name` de `source` (sem `name`, um console SQL) e db_query roda em `source` a consulta de leitura `query` (só SELECT, uma instrução; escrita é recusada) e devolve até `limit` linhas; close fecha a árvore. Conflitos de um merge ou de um stash na pasta da árvore: conflicts lista os arquivos em conflito e, com `path`, os conflitos desse arquivo numerados, com as linhas de cada lado; resolve resolve o arquivo `path` e já o marca como resolvido (git add): `side` mine fica com a minha versão inteira e theirs com a deles, ou `picks` responde conflito por conflito, na ordem (as alterações sem conflito dos dois lados entram sozinhas); merge abre para a pessoa a janela de mesclagem de `path`, com as três versões lado a lado, ou, sem `path`, a janela Conflitos com a lista. `tree` é o nome da pasta, o caminho ou o id da árvore (vazio: a selecionada ou a única). `path` sem caminho completo vale dentro da pasta mostrada.",
            "inputSchema": { "type": "object", "properties": {
                "action": { "type": "string", "enum": ["create", "list", "expand", "collapse", "collapse_all", "navigate", "back", "forward", "refresh", "open", "root", "hidden", "sort", "new_file", "new_folder", "rename", "delete", "copy", "cut", "paste", "search", "find_in_files", "usages", "history", "problems", "diff", "hunks", "revert_hunk", "editor", "save", "close_file", "db", "db_add", "db_sources", "db_tables", "db_query", "db_open", "close", "conflicts", "resolve", "merge"] },
                "tree": name("o nome da pasta da árvore, o caminho dela ou o id; vazio: a selecionada, ou a única do canvas"),
                "path": { "type": "string" },
                "name": { "type": "string", "description": "new_file, new_folder, rename: o nome; usages, history: o nome declarado" },
                "query": { "type": "string", "description": "search: parte do nome do arquivo; vazio limpa a busca. find_in_files: o texto a localizar. db_query: use `query` para a consulta SELECT" },
                "case": { "type": "boolean", "description": "find_in_files: diferencia maiúsculas de minúsculas" },
                "words": { "type": "boolean", "description": "find_in_files: só palavras inteiras" },
                "regex": { "type": "boolean", "description": "find_in_files: `query` é uma expressão regular" },
                "mask": { "type": "string", "description": "find_in_files: máscara de arquivo, ex.: *.rs, *.toml" },
                "source": { "type": "string", "description": "db_tables, db_open, db_query: o nome da fonte de dados ou o id; vazio: a única" },
                "limit": { "type": "integer", "description": "db_query: o máximo de linhas (padrão 100)" },
                "line": { "type": "integer", "description": "open: a linha (a primeira é 1)" },
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
                "artifacts": { "type": "array", "items": { "type": "string" }, "description": "arquivos, diff, portal ou conversa que a tarefa produziu" },
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
                "folder": { "type": "string", "description": "create: a pasta raiz, onde os terminais e IDEs novos dele começam" },
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
            "description": "O menu de um espaço de trabalho: pin e unpin (Fixar e Desfixar: o espaço fixado vai para Fixados, no alto da lista); duplicate (cópia dele e do desenho, \"<nome> cópia\"); copy_path (devolve a pasta raiz); export (grava o espaço, com o desenho, no arquivo `path`); import (cria um espaço novo do arquivo `path`, com os terminais e portais descarregados); unload (Descarregar: encerra os terminais e portais do espaço sem tirá-los do desenho; Recarregar num deles o inicia).",
            "inputSchema": { "type": "object", "properties": {
                "action": { "type": "string", "enum": ["pin", "unpin", "duplicate", "copy_path", "export", "import", "unload"] },
                "name": { "type": "string", "description": "o nome do espaço (não vale em import)" },
                "path": { "type": "string", "description": "export, import: o arquivo" },
            }, "required": ["action"] },
        }),
    ]
}
