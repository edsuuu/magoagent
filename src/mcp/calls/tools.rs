use super::*;

pub(crate) fn tools() -> Value {
    let elements = json!({ "type": "array", "items": { "type": "object" } });
    let ids = json!({ "type": "array", "items": { "type": "string" } });
    let agent_name = json!({ "type": "string", "description": "o nome do agente, como list_agents mostra" });
    let ide_name = json!({ "type": "string", "description": "o nome do IDE, como list_agents mostra; opcional com um só ligado a você" });
    json!([
        {
            "name": "get_scene",
            "description": "Lista os elementos do desenho atual (id, tipo, posição, tamanho, cores, texto, vínculos de setas) e a área ocupada.",
            "inputSchema": { "type": "object", "properties": {} },
        },
        {
            "name": "add_elements",
            "description": "Adiciona elementos (objetos JSON com type, x, y, width, height, cores, text, points). Extras: `label: {text, fontSize?}` em formas e setas; \
                `start: {id}` / `end: {id}` em setas para ligá-las a formas (os pontos são calculados sozinhos). \
                Devolve os ids criados.",
            "inputSchema": { "type": "object", "properties": { "elements": elements }, "required": ["elements"] },
        },
        {
            "name": "update_elements",
            "description": "Altera elementos existentes: cada item tem `id` mais os campos a trocar (x, y, width, height, text, \
                strokeColor, backgroundColor, angle, start/end...). Setas ligadas acompanham as formas movidas. Também move \
                e redimensiona janelas (terminais, navegadores e IDEs) por x, y, width e height, sem mexer no que rodam.",
            "inputSchema": { "type": "object", "properties": { "elements": elements }, "required": ["elements"] },
        },
        {
            "name": "delete_elements",
            "description": "Apaga elementos pelos ids.",
            "inputSchema": { "type": "object", "properties": { "ids": ids }, "required": ["ids"] },
        },
        {
            "name": "clear_canvas",
            "description": "Apaga todo o desenho.",
            "inputSchema": { "type": "object", "properties": {} },
        },
        {
            "name": "view",
            "description": "Renderiza o desenho (ou só os ids informados) como PNG para você ver como ficou.",
            "inputSchema": { "type": "object", "properties": {
                "ids": ids,
                "scale": { "type": "number", "description": "1 a 3 (padrão 1)" },
            } },
        },
        {
            "name": "save_file",
            "description": "Salva o desenho em um arquivo: .mago (formato do MagoAgent), .png ou .svg.",
            "inputSchema": { "type": "object", "properties": { "path": { "type": "string" } }, "required": ["path"] },
        },
        {
            "name": "open_file",
            "description": "Substitui o desenho atual pelo conteúdo de um arquivo .mago.",
            "inputSchema": { "type": "object", "properties": { "path": { "type": "string" } }, "required": ["path"] },
        },
        {
            "name": "set_background",
            "description": "Muda a cor de fundo da tela (hex, ex.: #ffffff).",
            "inputSchema": { "type": "object", "properties": { "color": { "type": "string" } }, "required": ["color"] },
        },
        {
            "name": "show_app",
            "description": "Abre o app MagoAgent para o usuário ver o desenho ao vivo.",
            "inputSchema": { "type": "object", "properties": {} },
        },
        {
            "name": "show_screen",
            "description": "Mostra à pessoa uma tela do MagoAgent: canvas (o quadro com os agentes trabalhando), chat ou ide.",
            "inputSchema": { "type": "object", "properties": { "screen": { "type": "string", "enum": ["canvas", "chat", "ide"] } }, "required": ["screen"] },
        },
        {
            "name": "focus_canvas",
            "description": "Mostra o canvas à pessoa centralizado no que ela quer ver: `at` são ids ou nomes (de terminais, navegadores, IDEs ou textos), separados por vírgula; vazio é o desenho todo. O zoom se ajusta para caber, ou use `zoom` (1 = 100%, 0.1 a 4).",
            "inputSchema": { "type": "object", "properties": {
                "at": { "type": "string" },
                "zoom": { "type": "number" },
            } },
        },
        {
            "name": "set_folder",
            "description": "Muda a pasta de trabalho da conversa atual para `path` (uma conversa que já tem sessão continua onde está e outra abre na pasta nova).",
            "inputSchema": { "type": "object", "properties": { "path": { "type": "string" } }, "required": ["path"] },
        },
        {
            "name": "screenshot",
            "description": "Um print da janela do MagoAgent como ela está agora (imagem), para ver e descrever à pessoa.",
            "inputSchema": { "type": "object", "properties": {} },
        },
        {
            "name": "review",
            "description": "Abre no IDE, ampliado, o painel de commit com o diff das mudanças da pasta `path` (a da conversa, se vazio), para a pessoa revisar com você e pedir ajustes (aplique-os com ide_edit).",
            "inputSchema": { "type": "object", "properties": { "path": { "type": "string" } } },
        },
        {
            "name": "remember",
            "description": "Guarda uma coisa aprendida sobre a pessoa e o jeito dela de trabalhar; entra nas próximas conversas. Uma frase curta.",
            "inputSchema": { "type": "object", "properties": { "text": { "type": "string" } }, "required": ["text"] },
        },
        {
            "name": "list_agents",
            "description": "Lista os agentes (terminais e chats do canvas), notas e navegadores ligados a você por uma seta, com o estado \
                de cada agente (trabalhando, parado, encerrado; um chat é o Maestro e diz que é chat). Fora de um terminal do MagoAgent, ou de um \
                chat ainda sem corda, lista todos.",
            "inputSchema": { "type": "object", "properties": {} },
        },
        {
            "name": "ask_agent",
            "description": "Manda um prompt a um agente conectado (outro terminal do canvas, ou um chat), espera ele terminar e devolve a \
                tela dele (de um chat, a resposta; a pergunta entra na conversa dele com o seu nome e, se ele estiver ocupado, espera na \
                fila). Se o tempo acabar antes, acompanhe com read_terminal; não reenvie o prompt. `raw: true` envia \
                teclas cruas (\\n Enter, \\t Tab, \\e Esc, \\x03 Ctrl+C) para menus e comandos de shell (só a terminais).",
            "inputSchema": { "type": "object", "properties": {
                "name": agent_name,
                "prompt": { "type": "string" },
                "raw": { "type": "boolean" },
                "timeout": { "type": "number", "description": "segundos de espera (padrão 50)" },
            }, "required": ["name", "prompt"] },
        },
        {
            "name": "read_terminal",
            "description": "Lê a tela atual de um agente conectado (de um chat, o fim da conversa), sem mandar nada.",
            "inputSchema": { "type": "object", "properties": { "name": agent_name }, "required": ["name"] },
        },
        {
            "name": "read_note",
            "description": "Lê uma nota adesiva conectada (markdown). O nome de uma nota é a primeira linha dela.",
            "inputSchema": { "type": "object", "properties": { "name": { "type": "string" } }, "required": ["name"] },
        },
        {
            "name": "write_note",
            "description": "Troca o texto de uma nota conectada. Mudar a primeira linha muda o nome da nota.",
            "inputSchema": { "type": "object", "properties": {
                "name": { "type": "string" },
                "text": { "type": "string" },
            }, "required": ["name", "text"] },
        },
        {
            "name": "portal_navigate",
            "description": "Abre um endereço num navegador (página web do canvas) conectado a você.",
            "inputSchema": { "type": "object", "properties": {
                "name": { "type": "string", "description": "o nome do navegador, como list_agents mostra" },
                "url": { "type": "string" },
            }, "required": ["name", "url"] },
        },
        {
            "name": "portal_read",
            "description": "Lê o texto visível da página de um navegador conectado.",
            "inputSchema": { "type": "object", "properties": { "name": { "type": "string" } }, "required": ["name"] },
        },
        {
            "name": "portal_console",
            "description": "Lê as últimas mensagens e erros do console da página de um navegador conectado. Pode filtrar por nível e limpar as entradas depois da leitura.",
            "inputSchema": { "type": "object", "properties": {
                "name": { "type": "string" },
                "level": { "type": "string", "enum": ["error", "warn", "info", "log", "debug"] },
                "clear": { "type": "boolean" }
            }, "required": ["name"] },
        },
        {
            "name": "portal_screenshot",
            "description": "Tira uma imagem do que a página de um navegador conectado mostra agora, para você ver o resultado de verdade (layout, cores, o que apareceu). O navegador precisa estar na tela. Só no Windows por enquanto.",
            "inputSchema": { "type": "object", "properties": { "name": { "type": "string" } }, "required": ["name"] },
        },
        {
            "name": "portal_click",
            "description": "Clica num elemento da página de um navegador conectado. Seu cursor, com seu nome, aparece na \
                página e desliza até o elemento antes do clique, para quem estiver olhando acompanhar.",
            "inputSchema": { "type": "object", "properties": {
                "name": { "type": "string" },
                "selector": { "type": "string", "description": "seletor CSS do elemento, ex.: button[type=submit]" },
            }, "required": ["name", "selector"] },
        },
        {
            "name": "portal_fill",
            "description": "Digita um texto num campo da página de um navegador conectado, com seu cursor indo até ele.",
            "inputSchema": { "type": "object", "properties": {
                "name": { "type": "string" },
                "selector": { "type": "string" },
                "text": { "type": "string" },
            }, "required": ["name", "selector", "text"] },
        },
        {
            "name": "portal_eval",
            "description": "Roda JavaScript na página de um navegador conectado e devolve o resultado (JSON). O valor de uma Promise não é esperado; para esperar algo, use portal_wait. Útil para \
                clicar (document.querySelector(...).click()), preencher campos ou ler partes da página.",
            "inputSchema": { "type": "object", "properties": {
                "name": { "type": "string" },
                "script": { "type": "string" },
            }, "required": ["name", "script"] },
        },
        {
            "name": "portal_wait",
            "description": "Espera até um seletor CSS existir e estar visível na página do navegador, ou informa que o prazo acabou.",
            "inputSchema": { "type": "object", "properties": {
                "name": { "type": "string" }, "selector": { "type": "string" },
                "timeout_ms": { "type": "integer", "description": "prazo em milissegundos (padrão 5000, máximo 30000)" }
            }, "required": ["name", "selector"] },
        },
        {
            "name": "portal_history",
            "description": "Volta, avança ou recarrega a página atual do navegador.",
            "inputSchema": { "type": "object", "properties": {
                "name": { "type": "string" }, "action": { "type": "string", "enum": ["back", "forward", "reload"] }
            }, "required": ["name", "action"] },
        },
        {
            "name": "portal_scroll",
            "description": "Rola a página do navegador pelos pixels informados em dy ou traz um elemento CSS à vista usando selector.",
            "inputSchema": { "type": "object", "properties": {
                "name": { "type": "string" }, "selector": { "type": "string" }, "dy": { "type": "integer" }
            }, "required": ["name"] },
        },
        {
            "name": "create_note",
            "description": "Cria uma nota adesiva ligada ao seu terminal (só de dentro de um terminal do MagoAgent).",
            "inputSchema": { "type": "object", "properties": { "text": { "type": "string" } }, "required": ["text"] },
        },
        {
            "name": "recruit_agent",
            "description": "Cria um agente novo (um terminal) abaixo do seu, na mesma pasta, já ligado a você; ele ganha \
                um nome (a resposta diz qual) e você fala com ele com ask_agent. Só de dentro de um terminal do MagoAgent. \
                Com worktree true ele trabalha num ambiente próprio: um git worktree do seu repositório, num ramo dele, para \
                dois agentes mexerem no mesmo código sem se atropelar (a resposta diz a pasta e o ramo; nada é juntado nem apagado sozinho).",
            "inputSchema": { "type": "object", "properties": {
                "command": { "type": "string", "description": "o que roda nele; padrão claude (ex.: codex)" },
                "worktree": { "type": "boolean", "description": "true: num git worktree próprio, num ramo dele" },
                "mode": { "type": "string", "enum": ["terminal", "chat"], "description": "terminal: Claude Code completo num terminal; chat: conversa leve num chat do app (usa o login do CLI, gasta bem menos memória); sem isto vale o ajuste Invocação dos agentes" },
            } },
        },
        {
            "name": "create_portal",
            "description": "Abre um navegador (página web) à direita do seu terminal, já ligado a você. Só de dentro de um terminal do MagoAgent.",
            "inputSchema": { "type": "object", "properties": {
                "url": { "type": "string" },
                "name": { "type": "string", "description": "opcional; padrão o domínio" },
            }, "required": ["url"] },
        },
        {
            "name": "connect",
            "description": "Liga duas coisas do canvas com uma corda (agentes, chats, notas, navegadores), pelo nome. Só com `a`, liga \
                você a `a`. O que está ligado se alcança: ask_agent, notas, navegadores. disconnect corta a corda.",
            "inputSchema": { "type": "object", "properties": {
                "a": { "type": "string" },
                "b": { "type": "string" },
            }, "required": ["a"] },
        },
        {
            "name": "notify",
            "description": "Avisa a pessoa com o seu nome: um aviso no app e uma notificação do sistema (no Windows, o ícone do app pisca na barra de tarefas).",
            "inputSchema": { "type": "object", "properties": { "text": { "type": "string" } }, "required": ["text"] },
        },
        {
            "name": "ide_open",
            "description": "Abre, no IDE do canvas ligado a você (sem IDE, na árvore de arquivos ligada a você, no painel de editor dela), uma pasta (ela se expande na árvore) ou um arquivo (ele aparece \
                no painel ao lado da árvore, rolado até `line` e com a linha destacada), para a pessoa acompanhar o que \
                você está vendo. `path` é relativo à pasta do projeto.",
            "inputSchema": { "type": "object", "properties": {
                "ide": ide_name,
                "path": { "type": "string" },
                "line": { "type": "number", "description": "linha a mostrar, a partir de 1" },
            }, "required": ["path"] },
        },
        {
            "name": "ide_search",
            "description": "Procura um texto (sem diferenciar maiúsculas) nos arquivos do projeto de um IDE ligado a você e \
                devolve até 200 linhas como caminho:linha: texto (pastas excluídas, como node_modules e vendor, ficam de fora: veja ide_exclude). O IDE avisa a busca na tela; abra o que interessar com ide_open.",
            "inputSchema": { "type": "object", "properties": {
                "ide": ide_name,
                "query": { "type": "string" },
            }, "required": ["query"] },
        },
        {
            "name": "ide_edit",
            "description": "Edita um arquivo pelo IDE do canvas, à vista da pessoa: o arquivo abre no painel, `old` (que precisa \
                aparecer uma vez só no arquivo) vira `new`, o arquivo é salvo e as linhas trocadas ficam destacadas.",
            "inputSchema": { "type": "object", "properties": {
                "ide": ide_name,
                "path": { "type": "string" },
                "old": { "type": "string" },
                "new": { "type": "string" },
            }, "required": ["path", "old", "new"] },
        },
        {
            "name": "vault_list",
            "description": "As contas do cofre de senhas da pessoa (site, usuário e tipo de login), nunca a senha. Use para saber \
                com que conta ela entra num site.",
            "inputSchema": { "type": "object", "properties": {} },
        },
        {
            "name": "vault_accounts",
            "description": "As contas de um site no cofre. Com mais de uma, pergunte à pessoa qual usar antes de entrar.",
            "inputSchema": { "type": "object", "properties": {
                "site": { "type": "string", "description": "domínio ou nome do app, ex.: github.com" },
            }, "required": ["site"] },
        },
        {
            "name": "vault_fill",
            "description": "Digita a senha de uma conta do cofre no campo focado da página do navegador aberto nesse site, sem \
                mostrá-la a você: clique no campo de senha antes (portal_click) ou diga o seletor em `field`. Num login via \
                Google/Apple digita o e-mail; o resto segue pelo botão do provedor. Nunca peça a senha em texto.",
            "inputSchema": { "type": "object", "properties": {
                "site": { "type": "string", "description": "domínio ou nome do app, como vault_accounts mostra" },
                "user": { "type": "string", "description": "a conta, quando o site tem mais de uma" },
                "field": { "type": "string", "description": "seletor CSS do campo; vazio: o campo focado" },
            }, "required": ["site"] },
        },
    ])
    .as_array()
    .into_iter()
    .flatten()
    .cloned()
    .chain(super::app_tools::app_tools())
    .chain(note_tools())
    .collect()
}
