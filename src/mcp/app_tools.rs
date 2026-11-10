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
            "name": "chat_list",
            "description": "Os chats do MagoAgent: nome, agente, pasta, se está respondendo e qual está aberto.",
            "inputSchema": { "type": "object", "properties": {} },
        }),
        json!({
            "name": "chat_new",
            "description": "Abre um chat novo e o mostra: `agent` claude ou codex (vazio: o de Ajustes), na pasta `folder` (vazio: a de Ajustes).",
            "inputSchema": { "type": "object", "properties": {
                "agent": { "type": "string", "enum": ["claude", "codex"] },
                "folder": { "type": "string" },
            } },
        }),
        json!({
            "name": "chat_open",
            "description": "Mostra um chat à pessoa.",
            "inputSchema": { "type": "object", "properties": { "chat": chat }, "required": ["chat"] },
        }),
        json!({
            "name": "chat_send",
            "description": "Manda uma mensagem a um chat, como se a pessoa a escrevesse lá; o agente dele responde no chat.",
            "inputSchema": { "type": "object", "properties": { "chat": chat, "text": { "type": "string" } }, "required": ["chat", "text"] },
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
            "name": "voice_call",
            "description": "A chamada de voz do chat aberto: on liga, off desliga, mute e unmute o microfone, speakers_off e speakers_on o som.",
            "inputSchema": { "type": "object", "properties": {
                "state": { "type": "string", "enum": ["on", "off", "mute", "unmute", "speakers_off", "speakers_on"] },
            }, "required": ["state"] },
        }),
        json!({
            "name": "ide_git",
            "description": "Git pelo IDE, à vista: panel abre o painel de commit com o diff; commit salva as abas e faz commit com `message` só das mudanças de arquivos já versionados, ou só dos `files` nomeados (inclusive não versionados), e diz o que entrou e quais não versionados ficaram de fora; push e pull; diff abre o painel de commit comparando com `base` (ramo, etiqueta ou commit; vazio é o HEAD) no modo `mode` (side, lado a lado; unified, unificado). Recusa se o git do IDE estiver ocupado ou uma aba não puder ser salva. O resultado aparece no IDE.",
            "inputSchema": { "type": "object", "properties": {
                "ide": ide,
                "action": { "type": "string", "enum": ["panel", "commit", "push", "pull", "diff"] },
                "message": { "type": "string" },
                "files": { "type": "array", "items": { "type": "string" }, "description": "commit: só estes arquivos, relativos à pasta do IDE; vazio: as mudanças dos já versionados" },
                "base": { "type": "string", "description": "diff: a revisão com que comparar (main, v0.0.8, a1b2c3d); vazio é o HEAD" },
                "mode": { "type": "string", "enum": ["side", "unified"], "description": "diff: lado a lado ou unificado; sem ele, fica como está" },
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
            "name": "set_setting",
            "description": "Muda um ajuste do app: theme (light, dark, system), voice (codex, openai, local), terminal_history (1000, 5000, 10000), avatar (on, off: a barra do Maestro por cima de tudo na tela), avatar_follow (on: a barra segue a janela em uso; off: fica onde está) ou favorite_folder (o caminho de uma pasta: entra nas favoritas do campo de pasta, no topo da lista Recentes, ou sai delas se já estava). Consumo: max_agents (quantos agentes podem rodar para o Maestro recrutar mais um; 0 tira o teto), usage_stop (a porcentagem da janela de 5 horas do plano a partir da qual o app não recruta; 0 nunca para) e recruit_model (o modelo dos agentes recrutados, por exemplo sonnet ou haiku; cli usa o da CLI).",
            "inputSchema": { "type": "object", "properties": {
                "name": { "type": "string", "enum": ["theme", "voice", "terminal_history", "avatar", "avatar_follow", "favorite_folder", "max_agents", "usage_stop", "recruit_model"] },
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
    ]
}
