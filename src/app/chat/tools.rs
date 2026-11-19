//! The Maestro's hands: the chat's agent gets the MagoAgent MCP (and, on Windows, the Windows
//! MCP) and a system prompt that says who it is, what it remembers of the person and how to act on
//! the app; the app answers its show, set_folder, screenshot and review commands.

use crate::app::chat::direct::{CHAT_MODES, Chat, NewChat, folder};
use crate::app::maestro::temp_folder::temp_rule;
use crate::mcp::bridge::UiCommand;
use crate::prelude::*;
use std::sync::mpsc::Sender;

/// Where the Maestro keeps what it learned about the person (`remember`, read into every prompt).
pub(crate) fn memory_path() -> Option<PathBuf> {
    data_dir().map(|d| d.join("maestro-memoria.md"))
}

pub(crate) fn memory_text() -> String {
    memory_path().and_then(|p| fs::read_to_string(p).ok()).unwrap_or_default()
}

/// Adds one line to the memory; what it holds now.
pub(crate) fn remember(text: &str) -> Result<String, String> {
    let path = memory_path().ok_or("sem pasta de dados")?;
    let _ = fs::create_dir_all(path.parent().unwrap_or(&path));
    let mut memory = fs::read_to_string(&path).unwrap_or_default();
    let line = text.trim().lines().map(str::trim).filter(|l| !l.is_empty()).collect::<Vec<_>>().join(" ");

    if line.is_empty() {
        return Err("nada para lembrar".into());
    }

    if !memory.is_empty() && !memory.ends_with('\n') {
        memory.push('\n');
    }

    memory.push_str("- ");
    memory.push_str(&line);
    memory.push('\n');
    write_atomic(&path, memory.clone()).map_err(|e| e.to_string())?;
    Ok(memory)
}

/// The MCP servers the chat's agent runs: this app's own, and on Windows the Windows MCP when the
/// person's Claude Code doesn't already have it (then `claude -p` loads it by itself).
pub(crate) fn mcp_servers(prefs: &Prefs, wsl: bool) -> Value {
    let mut servers = json!({ "magoagent": { "command": mcp_exe(wsl), "args": ["mcp"] } });

    // ponytail: an agent in WSL gets only the app's server (the Windows MCP and the e-mail's run on Windows); bring
    // them by interop when someone asks
    if wsl {
        return servers;
    }

    if cfg!(windows) && !claude_has("windows-mcp") && crate::editor::lsp::which("uvx").is_some() {
        servers["windows-mcp"] = json!({ "command": "uvx", "args": ["windows-mcp", "serve"] });
    }

    if let Some(server) = prefs.email.server() {
        servers[super::email::SERVER_NAME] = server;
    }

    servers
}

/// Claude Code's own list (`~/.claude.json`) has the server.
fn claude_has(server: &str) -> bool {
    let file = crate::transcript::claude_dir().and_then(|d| d.parent().map(|h| h.join(".claude.json")));

    file.and_then(|p| read_json::<Value>(&p)).is_some_and(|v| v["mcpServers"][server].is_object())
}

/// The MCP config file for `claude --mcp-config`, written fresh (the app may have moved).
pub(crate) fn mcp_config(prefs: &Prefs, wsl: bool) -> Option<PathBuf> {
    let path = data_dir()?.join(if wsl { "chat-mcp-wsl.json" } else { "chat-mcp.json" });
    let _ = fs::create_dir_all(path.parent()?);

    write_atomic(&path, json!({ "mcpServers": mcp_servers(prefs, wsl) }).to_string()).ok()?;
    Some(path)
}

/// Who the chat's agent is for the person, in the language Ajustes asks for.
pub(crate) fn system_prompt(prefs: &Prefs, chat: &Chat) -> String {
    if chat.recruit {
        let language = if prefs.language == 2 { "Always speak English." } else { "Fale em português do Brasil." };

        return format!(
            "Você é {}, um agente do time do MagoAgent, recrutado por outro agente para uma tarefa. Pasta de trabalho: {}. Faça o que for pedido e responda de forma \
             objetiva. Se outro agente te fizer uma pergunta (ask), a sua resposta volta para ele. Você não recruta agentes: só a Luna \
             recruta; se precisar de mais alguém, peça a ela. Para falar com os outros agentes use `magoagent list|ask|check` \
             (ou o MCP magoagent). Para rodar testes ou uma API e acompanhar, abra um terminal com create_terminal (ou `magoagent terminal <comando>`): ele fica \
             ligado a você por corda e você lê a tela dele com read_terminal (`magoagent check <nome>`). Para dar notícia de como você está \
             (\"estou parado esperando X\", \"terminei\", \"preciso de você\") use notify (`magoagent notify <texto>`); o que você avisa ou manda a \
             um chat (chat_send) chega com o seu nome na frente (\"{}: …\"), então a pessoa e a Luna sabem quem fala.{} {language}",
            chat.name,
            chat.cwd,
            chat.name,
            temp_rule(prefs)
        );
    }

    let language = match prefs.language {
        1 => "Fale sempre em português do Brasil.",
        2 => "Always speak English.",
        _ => "Fale no idioma em que a pessoa fala com você (português do Brasil ou inglês).",
    };

    let mut prompt = format!(
        "Você é a Luna, a assistente de voz e de chat da pessoa no MagoAgent: uma comandante de agentes que trabalha \
         com ela no computador dela. {language} Respostas curtas e diretas, como numa conversa falada: elas serão lidas em voz \
         alta. Comece sempre com um resumo de uma a três frases que se possa dizer em voz alta, sem caminhos, nomes de \
         arquivo, código, comandos ou links (eles ficam só na tela); os detalhes, se houver, vêm depois. Pasta de trabalho desta conversa: {}. Se o pedido é sobre outra pasta, use set_folder antes de começar. \
         Ferramentas do MagoAgent (MCP magoagent): show_screen mostra o canvas, o chat ou a Árvore de Arquivos (tree) à pessoa (\"me mostra no canvas\"); \
         focus_canvas mostra o canvas centralizado e com zoom no que ela pedir (\"abre o mapa\", \"dá zoom no terminal\"); \
         ide_folder abre uma pasta na Árvore de Arquivos e te dá a estrutura dela; maestro_comment leva ao chat da Luna (o ligado à Árvore) um comentário sobre linhas de um arquivo (o mesmo que a pessoa faz ao selecionar código e escolher \"Comentar para a Luna\"; ele chega como \"Comentário em arquivo:linhas\", com o trecho e o pedido); open_path abre um arquivo na Árvore de Arquivos ou um PDF, \
         imagem ou página num navegador; resize_window amplia, volta ou redimensiona terminais, navegadores e árvores, e maximiza a janela do app; \
         memory mostra a memória do app e dos agentes por terminal; skills lista as skills que você tem e abre a pasta delas; chat_list, chat_search, chat_new, chat_open e chat_send cuidam dos chats; chats_view abre a área de Chats com tudo o que roda (filtra por estado e busca); voice_call liga, desliga e muta a chamada; file_tree cuida da árvore (arquivos, ramos, conflitos, diffs); portal_tab mexe nas abas; close_window e terminal_restart fecham e \
         reiniciam janelas; portal_wait espera um elemento, portal_history volta, avança ou recarrega a página, portal_scroll rola até um elemento ou pelos pixels pedidos, portal_console lê os registros e erros da página e portal_screenshot tira uma imagem dela para você ver; set_setting troca tema e voz, mostra o avatar e o deixa parado ou seguindo a janela; history desfaz e refaz; sidebar vai ao próximo agente que precisa da pessoa e abre ou recolhe os Agentes deste espaço e o uso dos planos na lateral; \
         screenshot tira um print do app para você ver como está (\"como está?\" → tire o print e descreva); review abre as \
         mudanças da pasta na Árvore de Arquivos para a pessoa revisar com você; \
         remember guarda o que você aprende sobre como ela trabalha (preferências, jeito, nomes), para as próximas conversas; \
         add_elements e view desenham no canvas: ao desenhar sobre o código que a pessoa pediu para abrir ou ver na IDE do canvas, ponha em cada caixa ou nota \
         `link` com o caminho do arquivo de que ela fala (\"/caminho/x.rs:120\" com a linha): o desenho ganha uma marca que, clicada, já a leva ao arquivo na Árvore \
         (file_link faz o mesmo e volta); recruit_agent, ask_agent e connect montam e dirigem um time de agentes: no canvas você é o cartão da Luna, e o que está ligado a ele por corda \
         (terminais e outros chats) é o time que você comanda; o que um agente do seu time termina (a tarefa do recrutamento, o que você mandou por chat_send, um ask_agent que passou do tempo) \
         chega neste chat sozinho, numa mensagem \"Resposta de\" com o nome dele: conte à pessoa o que importa; create_terminal abre um terminal ligado ao seu cartão rodando um comando (testes, uma API) e read_terminal o acompanha; list_agents diz quem são, chat_node põe o seu cartão no canvas e disconnect tira alguém do time; \
         workspaces lista, mostra, cria, renomeia e remove os espaços de trabalho (um por projeto; trocar de espaço não para os agentes). \
         \"O que tem para mim?\", \"alguém precisa de mim?\": chame demands e diga as demandas da mais urgente para a menos, cada uma \
         com os pontos de atenção que a ferramenta trouxe (nunca invente outros); a pessoa permite ou nega um pedido de permissão \
         por voz, e você responde com demands (agent e answer allow ou deny). \
         \"O que o Revisor e o Desenvolvedor conversaram?\": chame connection_history com os dois nomes e resuma quem perguntou o quê a quem e como voltou. \
         \"O que aconteceu na última hora?\" ou \"o que o agente fez hoje?\": chame timeline, ajustando minutes e agent quando indicado, e resuma os eventos retornados. \
         Consumo: usage diz quanto do limite do plano já foi (janela de 5 horas e semana), quantos agentes rodam e o que cada um gasta. \
         Consulte antes de recrutar ou de delegar algo grande; o app recusa um recrutamento acima do teto de agentes ou com a janela de 5 horas \
         perto do fim, e aí você avisa a pessoa em vez de insistir. Diga os números como a ferramenta trouxe, sem estimar. \
         Pediu para abrir, mostrar ou ver algo: faça o que for preciso e abra na tela dela (open_path, ide_folder, focus_canvas, \
         show_screen, chat_open), sem perguntar e sem só dizer onde está; o app vem para a frente com o que abriu. \
         Só abra no canvas, num navegador do canvas ou no navegador do sistema quando ela pedir para abrir ou mostrar lá: o que você manda para ela \
         (um print, uma imagem, um arquivo) vai aqui no chat, na sua resposta (imagem como ![descrição](caminho), arquivo como [nome](caminho)). \
         \"Me manda um print\" é o screenshot mostrado aqui, sem open_path.",
        chat.cwd
    );

    // the work kept out of the temporary folder a restart empties (Ajustes › Agentes)
    prompt.push_str(&temp_rule(prefs));
    prompt.push_str(&crate::app::maestro::tuning::recruit_brief(prefs));

    prompt.push_str(
        " Tarefas: ao delegar trabalho a um agente, crie a tarefa com tasks (action create, owner com o nome dele e send true para entregá-la); \
         ao trazer as demandas, cite as tarefas travadas (aguardando aprovação, bloqueadas, que falharam) com os pontos de atenção que demands e tasks trouxeram, \
         nunca outros; ao terminar algo seu, atualize o estado com tasks update. \
         Fichário: binder guarda notas com nome entre as sessões (decisões, pendências, o contexto de um projeto); consulte-o (binder list e read) \
         antes de perguntar à pessoa algo que pode já estar anotado, e anote as decisões dela (binder write ou append).",
    );

    prompt.push_str(&format!(
        " Implementação pedida pela pessoa: use a ferramenta flow (start, advance com ok ou fail e uma nota, status), o fluxo de 12 etapas: \
         Planejar, Analisar codebase, Arquitetar, Dúvidas, Delegação e implementação, Testes unitários e de integração, Validar onde é usado, \
         Padronização (style-edsu), Revisor ponytail, Navegador e cenários (Playwright), Code review do agente, Code review humano. \
         Cada etapa grava a sua nota <id>-<etapa> no fichário; recrute com worktree true na implementação e use um revisor diferente de quem implementou; \
         só Dúvidas e o Code review humano esperam a pessoa, e o próprio flow junta os ramos ao aprovar a etapa 12. {} Revisor ponytail: {} Code review: {}",
        crate::app::maestro::skills::style_brief(&prefs.style_skill),
        crate::app::maestro::skills::PONYTAIL_BRIEF,
        crate::app::maestro::skills::CODE_REVIEW_BRIEF,
    ));

    if cfg!(windows) {
        prompt.push_str(" No Windows você também tem o Windows MCP para agir no computador (janelas, cliques, teclado).");
    }

    let memory = memory_text();

    if !memory.trim().is_empty() {
        prompt.push_str("\n\nO que você já sabe sobre a pessoa:\n");
        prompt.push_str(memory.trim());
    }

    // the names and first lines only: the notes themselves are read when needed
    let binder = if cfg!(test) { None } else { crate::app::maestro::binder::binder_dir() };
    let index = binder.map(|dir| crate::app::maestro::binder::index(&dir)).unwrap_or_default();

    if !index.is_empty() {
        prompt.push_str("\n\nO seu fichário (nome: primeira linha; leia com binder read):\n");
        prompt.push_str(&index);
    }

    prompt
}

/// The agent's flags for the Maestro's hands: Claude Code takes the MCP config and the system
/// prompt; Codex only the MCP (its instructions go in front of the first message).
/// This exe as the agent runs it: by its `/mnt/c/…` path from WSL (system/wsl.rs).
pub(crate) fn mcp_exe(wsl: bool) -> String {
    let exe = std::env::current_exe().map(|p| p.to_string_lossy().replace('\\', "/")).unwrap_or_else(|_| "magoagent".into());

    if wsl { linux_path(&exe) } else { exe }
}

/// `wsl`: the agent runs inside WSL (system/wsl.rs).
pub(crate) fn maestro_args(agent: &str, prefs: &Prefs, chat: &Chat, wsl: bool) -> Vec<String> {
    if agent == "codex" {
        let exe = mcp_exe(wsl);

        // the app's own tools need no one's approval: the chat and the call have no one to ask
        let mut args = vec![
            "-c".into(),
            format!("mcp_servers.magoagent.command=\"{exe}\""),
            "-c".into(),
            "mcp_servers.magoagent.args=[\"mcp\"]".into(),
            "-c".into(),
            "mcp_servers.magoagent.default_tools_approval_mode=\"approve\"".into(),
        ];

        if !wsl {
            args.extend(prefs.email.codex_args());
        }

        return args;
    }

    let mut args = Vec::new();

    if let Some(config) = mcp_config(prefs, wsl) {
        args.extend(["--mcp-config".to_string(), config.to_string_lossy().into()]);
        // its questions (AskUserQuestion) and the permissions it needs come to the chat as options (chat/decide.rs)
        args.extend(["--permission-prompt-tool".to_string(), crate::app::chat::decide::PROMPT_TOOL.into()]);
    }

    // by a file beside the drawing, one per chat; a test writes nothing
    let file = if cfg!(test) { None } else { autosave_path().map(|p| p.with_file_name("prompts").join(format!("{}.md", chat.id))) };

    args.extend(prompt_args(system_prompt(prefs, chat), file.as_deref()));
    // the app's skills as a local plugin (skills.rs): nothing while the folder has none, and never
    // the real folder in a test
    let skills = if cfg!(test) { None } else { crate::app::maestro::skills::skills_path() };

    args.extend(crate::app::maestro::skills::plugin_args(agent, skills.as_deref()));
    args
}

/// How the Maestro's instructions reach Claude Code. They have line breaks (what it remembers, the
/// binder's index) and on Windows `claude` is a batch file, which takes no argument with one ("batch
/// file arguments are invalid", and the chat never starts): so they go by a file,
/// `--append-system-prompt-file`. Without a file to write, they go as the argument, in one line there.
pub(crate) fn prompt_args(prompt: String, file: Option<&std::path::Path>) -> Vec<String> {
    if let Some(file) = file
        && file.parent().is_some_and(|dir| fs::create_dir_all(dir).is_ok())
        && write_atomic(file, prompt.clone()).is_ok()
    {
        return vec!["--append-system-prompt-file".into(), file.to_string_lossy().into_owned()];
    }

    let prompt = if cfg!(windows) && !cfg!(test) { one_line(&prompt) } else { prompt };

    vec!["--append-system-prompt".into(), prompt]
}

/// A text with its line breaks as spaces.
pub(crate) fn one_line(text: &str) -> String {
    text.split(['\r', '\n']).map(str::trim).filter(|line| !line.is_empty()).collect::<Vec<_>>().join(" ")
}

impl DrawingApp {
    /// show_screen: the canvas, the chat or the first file tree.
    pub(crate) fn maestro_show(&mut self, screen: &str) {
        match screen.trim().to_lowercase().as_str() {
            "chat" => self.show_chat(),
            "ide" | "tree" => self.open_tree(),
            _ => self.show_canvas(),
        }
    }

    /// focus_canvas: the canvas, centered on what `at` names (ids, or the names of terminals,
    /// portals and trees, or a text's words; comma-separated, empty for everything), at `zoom` or
    /// fitting it.
    pub(crate) fn maestro_look(&mut self, at: &str, zoom: Option<f32>) -> Result<String, String> {
        let wanted: Vec<String> = at.split(',').map(|w| w.trim().to_lowercase()).filter(|w| !w.is_empty()).collect();

        let named = |e: &Element| {
            let tree = e.filetree.as_ref().map(|t| t.name());
            let names = [e.terminal.as_ref().map(|t| t.label().to_string()), e.portal.as_ref().map(|p| p.name.clone()), tree];

            let text = match &e.kind {
                ElementKind::Text { value, .. } => value.to_lowercase(),
                _ => String::new(),
            };

            wanted.iter().any(|w| e.id.to_lowercase() == *w || names.iter().flatten().any(|n| n.to_lowercase() == *w) || text.contains(w.as_str()))
        };

        // the enlarged tree it names zooms itself, as Ctrl + wheel over it does (canvas/wheel.rs)
        let lifted_tree = self.elevated_index().filter(|index| self.scene.elements[*index].filetree.is_some() && named(&self.scene.elements[*index]));

        if let (Some(zoom), Some(_), false) = (zoom, lifted_tree, wanted.is_empty()) {
            self.set_lifted_zoom(zoom);
            // the canvas's screen, the tree kept enlarged (show_canvas would put it back)
            self.chat_open = false;

            return Ok(format!("A árvore ampliada {} está em {:.0}%.", at.trim(), self.lifted_zoom * 100.0));
        }

        let bounds = self.scene.elements.iter().filter(|e| wanted.is_empty() || named(e)).map(element_bounds).reduce(|a, b| a.union(b));

        let Some(bounds) = bounds else {
            return Err(if wanted.is_empty() { "O canvas está vazio.".into() } else { format!("Nada no canvas se chama {at}.") });
        };

        self.show_canvas();
        self.fit_view(bounds, zoom.map(|z| z.clamp(0.1, 4.0)));
        Ok(format!("Mostrando {} no canvas.", if wanted.is_empty() { "o desenho todo" } else { at.trim() }))
    }

    /// set_folder: the conversation moves to `path`; one that already has a session (the agent's
    /// memory lives in the folder) stays, and a new chat opens there.
    pub(crate) fn maestro_set_folder(&mut self, path: &str) -> Result<String, String> {
        let path = path.trim().trim_end_matches(['/', '\\']).to_string();

        if path.is_empty() {
            return Err("diga a pasta".into());
        }

        if !folder(&path).is_some_and(|p| p.is_dir()) {
            return Err(format!("A pasta {path} não existe."));
        }

        let current = self.chat_agent.clone().and_then(|id| self.chats.iter().find(|c| c.id == id).cloned());

        match current {
            Some(chat) if chat.session.is_none() => {
                self.chat_set_folder(&path);
                Ok(format!("Esta conversa agora trabalha em {path}."))
            }
            Some(chat) => {
                let installed = self.installed_agents();
                let agent = installed.iter().position(|p| p.command.starts_with(&chat.agent)).unwrap_or(0);

                self.create_chat(&NewChat { agent, cwd: path.clone(), mode: chat.mode.min(CHAT_MODES.len() - 1) }, &installed);
                Ok(format!("Abri outro chat em {path}; a conversa segue lá (esta fica na pasta de antes)."))
            }
            None => {
                self.prefs.chat_folder = path.clone();
                Ok(format!("Os próximos chats começam em {path}."))
            }
        }
    }

    /// review: the file tree of `path` (made on the canvas when there is none), enlarged, on its Alterações
    /// panel with the files that changed.
    pub(crate) fn maestro_review(&mut self, path: &str) -> Result<String, String> {
        let index = self.tree_for_folder(path.trim())?;
        let root = self.scene.elements[index].filetree.as_ref().map(|t| t.root.clone()).unwrap_or_default();
        let id = self.scene.elements[index].id.clone();

        self.filetrees.views.entry(id.clone()).or_insert_with(crate::app::filetree::view::TreeView::new);
        self.show_tree(index);
        self.ft_show_changes(&id, true);
        Ok(format!("As mudanças de {root} estão abertas na Árvore de Arquivos para a revisão."))
    }

    /// Each frame: the screenshots asked for go out.
    pub(crate) fn maestro_tick(&mut self, ctx: &Context) {
        for reply in std::mem::take(&mut self.shots) {
            ctx.send_viewport_cmd(egui::ViewportCommand::Screenshot(egui::ScreenshotCallback::new(move |image| {
                let _ = reply.send(png_base64(&image));
            })));
        }
    }

    /// The bridge's commands for the Maestro.
    pub(crate) fn apply_maestro_command(&mut self, command: UiCommand) {
        match command {
            UiCommand::Show { screen } => self.maestro_show(&screen),
            UiCommand::SetFolder { path, reply } => {
                let _ = reply.send(self.maestro_set_folder(&path));
            }
            UiCommand::Screenshot { reply } => self.shots.push(reply),
            UiCommand::Review { path, reply } => {
                let _ = reply.send(self.maestro_review(&path));
            }
            UiCommand::Look { at, zoom, reply } => {
                let _ = reply.send(self.maestro_look(&at, zoom));
            }
            // what waits for the network (Spotify's Web API) answers from a thread: the app goes on drawing
            UiCommand::Act { action, args, reply } => match self.act_later(&action, &args) {
                Some(job) => drop(std::thread::spawn(move || reply.send(job()))),
                None => {
                    let _ = reply.send(self.act(&action, &args));
                }
            },
            UiCommand::Hook { from, payload, reply } => self.take_hook(&from, &payload, reply),
            UiCommand::AskPerson { from, request, reply } => self.ask_person(&from, &request, reply),
            _ => {}
        }
    }
}

/// The screenshot as a PNG in base64, for the agent to see.
fn png_base64(image: &egui::ColorImage) -> Result<String, String> {
    let [w, h] = image.size;
    let rgba = image::RgbaImage::from_raw(w as u32, h as u32, image.as_raw().to_vec()).ok_or("imagem vazia")?;
    let mut png = Vec::new();

    rgba.write_to(&mut std::io::Cursor::new(&mut png), image::ImageFormat::Png).map_err(|e| e.to_string())?;
    Ok(crate::render::base64(&png))
}

pub(crate) type Shot = Sender<Result<String, String>>;

#[cfg(test)]
mod tests {
    use super::*;

    fn chat(cwd: &str) -> Chat {
        Chat {
            id: "c1".into(),
            agent: "claude".into(),
            name: "Claude Code · x".into(),
            cwd: cwd.into(),
            session: None,
            mode: 1,
            model: String::new(),
            effort: String::new(),
            recruit: false,
            named: false,
            space: String::new(),
            draft: false,
        }
    }

    #[test]
    fn the_maestro_and_its_recruits_keep_their_work_out_of_the_temp_folder_unless_ajustes_says_otherwise() {
        let mut prefs = Prefs::default();
        let recruit = Chat { name: "Ana".into(), recruit: true, ..chat("x") };
        let rule = temp_rule(&prefs);

        assert!(!rule.is_empty());
        assert!(system_prompt(&prefs, &chat("x")).contains(&rule) && system_prompt(&prefs, &recruit).contains(&rule));
        prefs.work_outside_temp = false;
        assert!(!system_prompt(&prefs, &chat("x")).contains("Pasta temporária") && !system_prompt(&prefs, &recruit).contains("Pasta temporária"));
    }

    #[test]
    fn the_maestro_prompt_names_the_folder_the_tools_and_the_language() {
        let mut prefs = Prefs::default();
        let prompt = system_prompt(&prefs, &chat("C:/trabalho/loja"));

        assert!(prompt.contains("C:/trabalho/loja") && prompt.contains("set_folder") && prompt.contains("screenshot") && prompt.contains("review"));
        assert!(prompt.contains("O que tem para mim?") && prompt.contains("demands"));
        assert!(prompt.contains("tasks (action create") && prompt.contains("binder list e read"));
        assert!(prompt.contains("skills lista as skills"));

        assert!(
            prompt.contains("ferramenta flow")
                && prompt.contains("Planejar")
                && prompt.contains("Code review humano")
                && prompt.contains("Padronização (style-edsu)")
        );

        // a recruit is told how to say how it is, and that its name goes in front of what it says
        let recruit = Chat { name: "Ana".into(), recruit: true, ..chat("x") };
        let told = system_prompt(&prefs, &recruit);

        assert!(told.starts_with("Você é Ana,") && told.contains("magoagent notify") && told.contains("(\"Ana: …\")"), "{told}");
        prefs.style_skill = "C:/nao/existe/SKILL.md".into();
        assert!(system_prompt(&prefs, &chat("x")).contains("não achei a skill de estilo (C:/nao/existe/SKILL.md)"));
        assert!(prompt.contains("no idioma em que a pessoa fala"));
        prefs.language = 2;
        assert!(system_prompt(&prefs, &chat("x")).contains("Always speak English"));
        let args = maestro_args("codex", &prefs, &chat("x"), false);

        assert_eq!(args[0], "-c");
        assert!(args[1].starts_with("mcp_servers.magoagent.command="));
        assert_eq!(args[5], "mcp_servers.magoagent.default_tools_approval_mode=\"approve\"");
        let servers = mcp_servers(&Prefs::default(), false);

        assert_eq!(servers["magoagent"]["args"], json!(["mcp"]));
        assert!(servers.get("email").is_none(), "the mail bridge is off until the MCP's `email` turns it on");
        prefs.email = crate::app::chat::email::EmailPrefs { on: true, command: "npx".into(), args: "-y pacote".into() };
        assert_eq!(mcp_servers(&prefs, false)["email"]["args"], json!(["-y", "pacote"]));
        assert_eq!(maestro_args("codex", &prefs, &chat("x"), false).len(), 10);
        // an agent in WSL runs the exe by its Linux path and gets only the app's server (system/wsl.rs)
        let wsl = mcp_servers(&prefs, true);

        assert_eq!(wsl["magoagent"]["command"], json!(linux_path(&mcp_exe(false))));
        assert!(wsl.get("email").is_none() && wsl.get("windows-mcp").is_none());
        let codex = maestro_args("codex", &prefs, &chat("x"), true);

        assert_eq!((codex.len(), codex[1].clone()), (6, format!("mcp_servers.magoagent.command=\"{}\"", linux_path(&mcp_exe(false)))));
    }

    #[test]
    fn the_instructions_go_by_a_file_since_a_batch_file_takes_no_line_break() {
        let dir = std::env::temp_dir().join(format!("mago-prompt-{}", new_id()));
        let file = dir.join("prompts").join("c1.md");
        let prompt = "Você é a Luna.\n\nO seu fichário:\n- decisoes: Pix".to_string();

        assert_eq!(prompt_args(prompt.clone(), Some(&file)), ["--append-system-prompt-file".to_string(), file.to_string_lossy().into_owned()]);
        assert_eq!(fs::read_to_string(&file).unwrap(), prompt, "whole, line breaks and all");
        // no file to write: the argument itself
        assert_eq!(prompt_args(prompt, None)[0], "--append-system-prompt");
        assert_eq!(one_line("a\r\n\n  b \nc"), "a b c");
        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn set_folder_moves_a_fresh_chat_and_opens_another_for_one_with_a_session() {
        let mut app = DrawingApp::default();
        let installed = vec![Preset::new("Claude Code", "claude")];

        app.create_chat(&NewChat { agent: 0, cwd: "~/a".into(), mode: 0 }, &installed);
        let dir = std::env::temp_dir();
        let path = dir.to_string_lossy().trim_end_matches(['/', '\\']).to_string();

        assert!(app.maestro_set_folder("C:/nao/existe/mesmo").is_err());
        assert!(app.maestro_set_folder(&path).unwrap().contains("agora trabalha"));
        assert_eq!(app.chats[0].cwd, path);
        app.chats[0].session = Some("s".into());
        // no agent installed here: the new chat can't be made, so nothing changes but the answer
        let answer = app.maestro_set_folder(&path);

        assert!(answer.is_ok(), "{answer:?}");
    }

    #[test]
    fn review_puts_a_file_tree_of_the_folder_on_the_canvas_enlarged() {
        let mut app = DrawingApp::default();

        app.canvas_rect = Rect::from_min_size(Pos2::ZERO, Vec2::new(800.0, 600.0));
        let dir = std::env::temp_dir();
        let said = app.maestro_review(&dir.to_string_lossy()).unwrap();

        assert!(said.contains("mudanças"));
        let trees: Vec<&Element> = app.scene.elements.iter().filter(|e| e.filetree.is_some()).collect();

        assert_eq!(trees.len(), 1);
        assert_eq!(app.terminal_elevated.as_ref(), Some(&trees[0].id));
        assert!(!app.chat_open);
        // again: the same tree, not a second one
        app.maestro_review(&dir.to_string_lossy()).unwrap();
        assert_eq!(app.scene.elements.iter().filter(|e| e.filetree.is_some()).count(), 1);
    }

    #[test]
    fn focus_canvas_centers_and_zooms_on_what_is_named() {
        let mut app = DrawingApp::default();

        app.canvas_rect = Rect::from_min_size(Pos2::ZERO, Vec2::new(800.0, 600.0));
        app.chat_open = true;
        let mapa = app.new_element(ElementKind::Text { x: 1000.0, y: 2000.0, value: "Mapa da loja".into() });

        app.scene.elements.push(mapa);
        assert!(app.maestro_look("planta", None).is_err());
        assert!(app.maestro_look("mapa", Some(2.0)).unwrap().contains("mapa"));
        assert!(!app.chat_open && app.zoom == 2.0);
        let center = element_bounds(&app.scene.elements[0]).center();

        assert!(app.to_screen(center).distance(app.canvas_rect.center()) < 1.0, "centered on it");
        // an enlarged tree named with a zoom: its own zoom, the canvas stays
        let mut tree = app.new_element(ElementKind::Rectangle { x: 0.0, y: 0.0, width: 400.0, height: 300.0 });

        tree.filetree = Some(FileTreeSpec { root: "/tmp/projeto".into(), ..Default::default() });
        let id = tree.id.clone();

        app.scene.elements.push(tree);
        app.set_elevated(Some(id.clone()));
        assert!(app.maestro_look(&id, Some(1.5)).unwrap().contains("150%"));
        assert_eq!((app.lifted_zoom, app.zoom), (1.5, 2.0));
        assert_eq!(app.terminal_elevated.as_ref(), Some(&id), "still enlarged");
        assert!(app.maestro_look(&id, Some(9.0)).unwrap().contains("300%"), "three times at most");
    }

    #[test]
    fn remember_appends_lines_to_the_memory() {
        // the memory file lives in the data folder: only the pure line-making is checked here
        let text = "  gosta de respostas curtas\n\n  e de commits em pt-BR  ";
        let line = text.trim().lines().map(str::trim).filter(|l| !l.is_empty()).collect::<Vec<_>>().join(" ");

        assert_eq!(line, "gosta de respostas curtas e de commits em pt-BR");
    }
}
