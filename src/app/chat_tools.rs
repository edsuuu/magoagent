//! The Maestro's hands: the chat's agent gets the MagoAgent MCP (and, on Windows, the Windows
//! MCP) and a system prompt that says who it is, what it remembers of the person and how to act on
//! the app; the app answers its show, set_folder, screenshot and review commands.

use crate::app::chat_direct::{CHAT_MODES, Chat, NewChat, folder};
use crate::bridge::UiCommand;
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
pub(crate) fn mcp_servers() -> Value {
    let exe = std::env::current_exe().map(|p| p.to_string_lossy().replace('\\', "/")).unwrap_or_else(|_| "magoagent".into());
    let mut servers = json!({ "magoagent": { "command": exe, "args": ["mcp"] } });
    if cfg!(windows) && !claude_has("windows-mcp") && crate::ide::lsp::which("uvx").is_some() {
        servers["windows-mcp"] = json!({ "command": "uvx", "args": ["windows-mcp", "serve"] });
    }
    servers
}

/// Claude Code's own list (`~/.claude.json`) has the server.
fn claude_has(server: &str) -> bool {
    let file = crate::transcript::claude_dir().and_then(|d| d.parent().map(|h| h.join(".claude.json")));
    file.and_then(|p| fs::read_to_string(p).ok()).and_then(|d| serde_json::from_str::<Value>(&d).ok()).is_some_and(|v| v["mcpServers"][server].is_object())
}

/// The MCP config file for `claude --mcp-config`, written fresh (the app may have moved).
pub(crate) fn mcp_config() -> Option<PathBuf> {
    let path = data_dir()?.join("chat-mcp.json");
    let _ = fs::create_dir_all(path.parent()?);
    write_atomic(&path, json!({ "mcpServers": mcp_servers() }).to_string()).ok()?;
    Some(path)
}

/// Who the chat's agent is for the person, in the language Ajustes asks for.
pub(crate) fn system_prompt(prefs: &Prefs, chat: &Chat) -> String {
    let language = match prefs.language {
        1 => "Fale sempre em português do Brasil.",
        2 => "Always speak English.",
        _ => "Fale no idioma em que a pessoa fala com você (português do Brasil ou inglês).",
    };
    let mut prompt = format!(
        "Você é o Maestro do MagoAgent, o assistente de voz e de chat da pessoa: um comandante de agentes que trabalha \
         com ela no computador dela. {language} Respostas curtas e diretas, como numa conversa falada: elas serão lidas em voz \
         alta. Comece sempre com um resumo de uma a três frases que se possa dizer em voz alta, sem caminhos, nomes de \
         arquivo, código, comandos ou links (eles ficam só na tela); os detalhes, se houver, vêm depois. Pasta de trabalho desta conversa: {}. Se o pedido é sobre outra pasta, use set_folder antes de começar. \
         Ferramentas do MagoAgent (MCP magoagent): show_screen mostra o canvas, o chat ou o IDE à pessoa (\"me mostra no canvas\"); \
         focus_canvas mostra o canvas centralizado e com zoom no que ela pedir (\"abre o mapa\", \"dá zoom no terminal\"); \
         ide_folder abre uma pasta no IDE (a árvore de arquivos) e te dá a estrutura dela; code_comment comenta linhas de um arquivo e envia a referência exata ao agente ligado ao IDE; open_path abre um arquivo no IDE ou um PDF, \
         imagem ou página num portal; resize_window amplia, volta ou redimensiona terminais, portais e IDEs, e maximiza a janela do app; \
         memory mostra a memória do app e dos agentes por terminal; skills lista as skills que você tem e abre a pasta delas; chat_list, chat_search, chat_new, chat_open e chat_send cuidam dos chats; voice_call liga, desliga e muta a chamada; ide_git faz commit (dos arquivos já versionados, \
         ou só dos que você nomear em files), push e pull pelo IDE, e também ramos (criar, trocar, excluir), fetch, publicar, merge, stash e o histórico de commits, ide_database abre o banco e ide_exclude tira pastas da busca e do índice; ide_conflict resolve conflitos de arquivo; portal_tab mexe nas abas; close_window e terminal_restart fecham e \
         reiniciam janelas; portal_wait espera um elemento, portal_history volta, avança ou recarrega a página, portal_scroll rola até um elemento ou pelos pixels pedidos, portal_console lê os registros e erros da página e portal_screenshot tira uma imagem dela para você ver; set_setting troca tema e voz, mostra o avatar e o deixa parado ou seguindo a janela; history desfaz e refaz; \
         screenshot tira um print do app para você ver como está (\"como está?\" → tire o print e descreva); review abre o diff \
         das mudanças da pasta no IDE para a pessoa revisar com você, e ide_edit aplica o que ela pedir em cada trecho; \
         remember guarda o que você aprende sobre como ela trabalha (preferências, jeito, nomes), para as próximas conversas; \
         add_elements e view desenham no canvas; recruit_agent, ask_agent e connect montam e dirigem um time de agentes; \
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
         show_screen, chat_open), sem perguntar e sem só dizer onde está; o app vem para a frente com o que abriu.",
        chat.cwd
    );
    prompt.push_str(
        " Tarefas: ao delegar trabalho a um agente, crie a tarefa com tasks (action create, owner com o nome dele e send true para entregá-la); \
         ao trazer as demandas, cite as tarefas travadas (aguardando aprovação, bloqueadas, que falharam) com os pontos de atenção que demands e tasks trouxeram, \
         nunca outros; ao terminar algo seu, atualize o estado com tasks update. \
         Fichário: binder guarda notas com nome entre as sessões (decisões, pendências, o contexto de um projeto); consulte-o (binder list e read) \
         antes de perguntar à pessoa algo que pode já estar anotado, e anote as decisões dela (binder write ou append).",
    );
    if cfg!(windows) {
        prompt.push_str(" No Windows você também tem o Windows MCP para agir no computador (janelas, cliques, teclado).");
    }
    let memory = memory_text();
    if !memory.trim().is_empty() {
        prompt.push_str("\n\nO que você já sabe sobre a pessoa:\n");
        prompt.push_str(memory.trim());
    }
    // the names and first lines only: the notes themselves are read when needed
    let binder = if cfg!(test) { None } else { crate::app::binder::binder_dir() };
    let index = binder.map(|dir| crate::app::binder::index(&dir)).unwrap_or_default();
    if !index.is_empty() {
        prompt.push_str("\n\nO seu fichário (nome: primeira linha; leia com binder read):\n");
        prompt.push_str(&index);
    }
    prompt
}

/// The agent's flags for the Maestro's hands: Claude Code takes the MCP config and the system
/// prompt; Codex only the MCP (its instructions go in front of the first message).
pub(crate) fn maestro_args(agent: &str, prefs: &Prefs, chat: &Chat) -> Vec<String> {
    if agent == "codex" {
        let exe = std::env::current_exe().map(|p| p.to_string_lossy().replace('\\', "/")).unwrap_or_else(|_| "magoagent".into());
        // the app's own tools need no one's approval: the chat and the call have no one to ask
        return vec![
            "-c".into(),
            format!("mcp_servers.magoagent.command=\"{exe}\""),
            "-c".into(),
            "mcp_servers.magoagent.args=[\"mcp\"]".into(),
            "-c".into(),
            "mcp_servers.magoagent.default_tools_approval_mode=\"approve\"".into(),
        ];
    }
    let mut args = Vec::new();
    if let Some(config) = mcp_config() {
        args.extend(["--mcp-config".to_string(), config.to_string_lossy().into()]);
    }
    // by a file beside the drawing, one per chat; a test writes nothing
    let file = if cfg!(test) { None } else { autosave_path().map(|p| p.with_file_name("prompts").join(format!("{}.md", chat.id))) };
    args.extend(prompt_args(system_prompt(prefs, chat), file.as_deref()));
    // the app's skills as a local plugin (skills.rs): nothing while the folder has none, and never
    // the real folder in a test
    let skills = if cfg!(test) { None } else { crate::app::skills::skills_path() };
    args.extend(crate::app::skills::plugin_args(agent, skills.as_deref()));
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
    /// show_screen: the canvas, the chat or the IDE.
    pub(crate) fn maestro_show(&mut self, screen: &str) {
        match screen.trim().to_lowercase().as_str() {
            "chat" => self.show_chat(),
            "ide" => self.open_ide(),
            _ => self.show_canvas(),
        }
        self.bring_to_front();
    }

    /// focus_canvas: the canvas, centered on what `at` names (ids, or the names of terminals,
    /// portals and IDEs, or a text's words; comma-separated, empty for everything), at `zoom` or
    /// fitting it.
    pub(crate) fn maestro_look(&mut self, at: &str, zoom: Option<f32>) -> Result<String, String> {
        let wanted: Vec<String> = at.split(',').map(|w| w.trim().to_lowercase()).filter(|w| !w.is_empty()).collect();
        let named = |e: &Element| {
            let names = [e.terminal.as_ref().map(|t| t.label()), e.portal.as_ref().map(|p| p.name.as_str()), e.ide.as_ref().map(|i| i.name.as_str())];
            let text = match &e.kind {
                ElementKind::Text { value, .. } => value.to_lowercase(),
                _ => String::new(),
            };
            wanted.iter().any(|w| e.id.to_lowercase() == *w || names.iter().flatten().any(|n| n.to_lowercase() == *w) || text.contains(w.as_str()))
        };
        let bounds = self.scene.elements.iter().filter(|e| wanted.is_empty() || named(e)).map(element_bounds).reduce(|a, b| a.union(b));
        let Some(bounds) = bounds else {
            return Err(if wanted.is_empty() { "O canvas está vazio.".into() } else { format!("Nada no canvas se chama {at}.") });
        };
        self.show_canvas();
        self.bring_to_front();
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

    /// review: the IDE of `path` (made on the canvas when there is none), enlarged, on its Commit
    /// panel with the diff of what changed.
    pub(crate) fn maestro_review(&mut self, path: &str) -> Result<String, String> {
        let index = self.ide_for_folder(path.trim())?;
        let root = self.scene.elements[index].ide.as_ref().map(|i| i.root.clone()).unwrap_or_default();
        let id = self.scene.elements[index].id.clone();
        self.chat_open = false;
        self.enlarge_ide(index);
        self.ide_focus = Some(id.clone());
        // the editor opens on the next frame (sync_ides): the Commit panel waits for it
        self.review_pending = Some(id);
        Ok(format!("O diff de {root} está aberto no IDE para a revisão."))
    }

    /// Each frame: the screenshots asked for go out, and a review opens its Commit panel once its
    /// editor is there.
    pub(crate) fn maestro_tick(&mut self, ctx: &Context) {
        for reply in std::mem::take(&mut self.shots) {
            ctx.send_viewport_cmd(egui::ViewportCommand::Screenshot(egui::ScreenshotCallback::new(move |image| {
                let _ = reply.send(png_base64(&image));
            })));
        }
        if let Some(id) = self.review_pending.clone()
            && let Some(ide) = self.ides.get_mut(&id)
        {
            ide.run(crate::ide::Cmd::Commit);
            self.review_pending = None;
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
            UiCommand::Act { action, args, reply } => {
                let _ = reply.send(self.act(&action, &args));
            }
            UiCommand::Hook { from, payload, reply } => self.take_hook(&from, &payload, reply),
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
        }
    }

    #[test]
    fn the_maestro_prompt_names_the_folder_the_tools_and_the_language() {
        let mut prefs = Prefs::default();
        let prompt = system_prompt(&prefs, &chat("C:/trabalho/loja"));
        assert!(prompt.contains("C:/trabalho/loja") && prompt.contains("set_folder") && prompt.contains("screenshot") && prompt.contains("review"));
        assert!(prompt.contains("O que tem para mim?") && prompt.contains("demands"));
        assert!(prompt.contains("tasks (action create") && prompt.contains("binder list e read"));
        assert!(prompt.contains("skills lista as skills"));
        assert!(prompt.contains("no idioma em que a pessoa fala"));
        prefs.language = 2;
        assert!(system_prompt(&prefs, &chat("x")).contains("Always speak English"));
        let args = maestro_args("codex", &prefs, &chat("x"));
        assert_eq!(args[0], "-c");
        assert!(args[1].starts_with("mcp_servers.magoagent.command="));
        assert_eq!(args[5], "mcp_servers.magoagent.default_tools_approval_mode=\"approve\"");
        let servers = mcp_servers();
        assert_eq!(servers["magoagent"]["args"], json!(["mcp"]));
    }

    #[test]
    fn the_instructions_go_by_a_file_since_a_batch_file_takes_no_line_break() {
        let dir = std::env::temp_dir().join(format!("mago-prompt-{}", new_id()));
        let file = dir.join("prompts").join("c1.md");
        let prompt = "Você é o Maestro.\n\nO seu fichário:\n- decisoes: Pix".to_string();
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
        let installed = vec![Preset { name: "Claude Code".into(), command: "claude".into(), on: true }];
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
    fn review_puts_an_ide_of_the_folder_on_the_canvas_enlarged() {
        let mut app = DrawingApp::default();
        app.canvas_rect = Rect::from_min_size(Pos2::ZERO, Vec2::new(800.0, 600.0));
        let dir = std::env::temp_dir();
        let said = app.maestro_review(&dir.to_string_lossy()).unwrap();
        assert!(said.contains("diff"));
        let ides: Vec<&Element> = app.scene.elements.iter().filter(|e| e.ide.is_some()).collect();
        assert_eq!(ides.len(), 1);
        assert_eq!(app.terminal_elevated.as_ref(), Some(&ides[0].id));
        assert!(!app.chat_open);
        // again: the same IDE, not a second one
        app.maestro_review(&dir.to_string_lossy()).unwrap();
        assert_eq!(app.scene.elements.iter().filter(|e| e.ide.is_some()).count(), 1);
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
    }

    #[test]
    fn remember_appends_lines_to_the_memory() {
        // the memory file lives in the data folder: only the pure line-making is checked here
        let text = "  gosta de respostas curtas\n\n  e de commits em pt-BR  ";
        let line = text.trim().lines().map(str::trim).filter(|l| !l.is_empty()).collect::<Vec<_>>().join(" ");
        assert_eq!(line, "gosta de respostas curtas e de commits em pt-BR");
    }
}
