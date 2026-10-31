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
         alta. Pasta de trabalho desta conversa: {}. Se o pedido é sobre outra pasta, use set_folder antes de começar. \
         Ferramentas do MagoAgent (MCP magoagent): show_screen mostra o canvas, o chat ou o IDE à pessoa (\"me mostra no canvas\"); \
         screenshot tira um print do app para você ver como está (\"como está?\" → tire o print e descreva); review abre o diff \
         das mudanças da pasta no IDE para a pessoa revisar com você, e ide_edit aplica o que ela pedir em cada trecho; \
         remember guarda o que você aprende sobre como ela trabalha (preferências, jeito, nomes), para as próximas conversas; \
         add_elements e view desenham no canvas; recruit_agent, ask_agent e connect montam e dirigem um time de agentes.",
        chat.cwd
    );
    if cfg!(windows) {
        prompt.push_str(" No Windows você também tem o Windows MCP para agir no computador (janelas, cliques, teclado).");
    }
    let memory = memory_text();
    if !memory.trim().is_empty() {
        prompt.push_str("\n\nO que você já sabe sobre a pessoa:\n");
        prompt.push_str(memory.trim());
    }
    prompt
}

/// The agent's flags for the Maestro's hands: Claude Code takes the MCP config and the system
/// prompt; Codex only the MCP (its instructions go in front of the first message).
pub(crate) fn maestro_args(agent: &str, prefs: &Prefs, chat: &Chat) -> Vec<String> {
    if agent == "codex" {
        let exe = std::env::current_exe().map(|p| p.to_string_lossy().replace('\\', "/")).unwrap_or_else(|_| "magoagent".into());
        return vec!["-c".into(), format!("mcp_servers.magoagent.command=\"{exe}\""), "-c".into(), "mcp_servers.magoagent.args=[\"mcp\"]".into()];
    }
    let mut args = Vec::new();
    if let Some(config) = mcp_config() {
        args.extend(["--mcp-config".to_string(), config.to_string_lossy().into()]);
    }
    args.extend(["--append-system-prompt".to_string(), system_prompt(prefs, chat)]);
    args
}

impl DrawingApp {
    /// show_screen: the canvas, the chat or the IDE.
    pub(crate) fn maestro_show(&mut self, screen: &str) {
        match screen.trim().to_lowercase().as_str() {
            "chat" => self.show_chat(),
            "ide" => self.open_ide(),
            _ => self.show_canvas(),
        }
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
        let root = folder(path.trim()).filter(|p| p.is_dir()).ok_or_else(|| format!("A pasta {path} não existe."))?;
        let root = root.to_string_lossy().trim_end_matches(['/', '\\']).to_string();
        let same = |e: &Element| e.ide.as_ref().is_some_and(|i| folder(&i.root).is_some_and(|p| p.to_string_lossy().trim_end_matches(['/', '\\']) == root));
        let index = match self.scene.elements.iter().position(same) {
            Some(i) => i,
            None => {
                let at = Rect::from_center_size(self.to_world(self.canvas_rect.center()), IDE_SIZE);
                let mut ide = self.new_element(ElementKind::Rectangle { x: at.min.x, y: at.min.y, width: at.width(), height: at.height() });
                ide.style = node_style(ide.style);
                let name = std::path::Path::new(&root).file_name().map_or("IDE".to_string(), |n| n.to_string_lossy().to_string());
                ide.ide = Some(IdeSpec { name, root: root.clone(), ..Default::default() });
                self.snapshot();
                self.scene.elements.push(ide);
                self.scene.elements.len() - 1
            }
        };
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
        assert!(prompt.contains("no idioma em que a pessoa fala"));
        prefs.language = 2;
        assert!(system_prompt(&prefs, &chat("x")).contains("Always speak English"));
        let args = maestro_args("codex", &prefs, &chat("x"));
        assert_eq!(args[0], "-c");
        assert!(args[1].starts_with("mcp_servers.magoagent.command="));
        let servers = mcp_servers();
        assert_eq!(servers["magoagent"]["args"], json!(["mcp"]));
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
    fn remember_appends_lines_to_the_memory() {
        // the memory file lives in the data folder: only the pure line-making is checked here
        let text = "  gosta de respostas curtas\n\n  e de commits em pt-BR  ";
        let line = text.trim().lines().map(str::trim).filter(|l| !l.is_empty()).collect::<Vec<_>>().join(" ");
        assert_eq!(line, "gosta de respostas curtas e de commits em pt-BR");
    }
}
