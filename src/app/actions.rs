//! What the agents do on the app through the MCP's `act` (bridge.rs): the IDE on a folder with
//! its structure, a file or a PDF opened where it reads, a window resized. Each action is a name
//! and its JSON arguments; a new one is a case here and a tool in mcp/calls.rs.

use crate::app::chat_direct::folder;
use crate::prelude::*;
use std::path::Path;

/// Files a page shows better than the editor: they open in a portal.
const PAGE_KINDS: [&str; 9] = ["pdf", "png", "jpg", "jpeg", "gif", "webp", "svg", "html", "htm"];

impl DrawingApp {
    /// The bridge's `act`: one of the app's actions, by name. What opens something shows it: the
    /// app's window comes to the front with it.
    pub(crate) fn act(&mut self, action: &str, args: &Value) -> Result<String, String> {
        let result = self.act_on(action, args);
        if result.is_ok() && matches!(action, "ide_folder" | "open_path" | "chat_new" | "chat_open") {
            self.bring_to_front();
        }
        result
    }

    /// The app's window to the front, out of the taskbar if minimized.
    pub(crate) fn bring_to_front(&mut self) {
        self.viewport_commands.extend([egui::ViewportCommand::Minimized(false), egui::ViewportCommand::Focus]);
    }

    fn act_on(&mut self, action: &str, args: &Value) -> Result<String, String> {
        let text = |key: &str| args[key].as_str().unwrap_or_default().trim().to_string();
        match action {
            "memory" => Ok(crate::app::memory::format(&self.memory.snapshot())),
            "ide_folder" => self.act_ide_folder(&text("path")),
            "open_path" => self.act_open_path(&text("path"), args["line"].as_u64().map(|l| l as usize)),
            "resize_window" => self.act_resize(&text("target"), &text("state"), args["width"].as_f64(), args["height"].as_f64()),
            "close_window" => {
                let index = self.node_named(&text("name"))?;
                self.selected = vec![index];
                self.delete_selected();
                Ok(format!("{} fechado (dá para desfazer).", text("name")))
            }
            "terminal_restart" => {
                let index = self.node_named(&text("name"))?;
                if self.scene.elements[index].terminal.is_none() {
                    return Err(format!("{} não é um terminal", text("name")));
                }
                self.restart_node(self.scene.elements[index].id.clone());
                Ok(format!("{} reiniciado.", text("name")))
            }
            "portal_tab" => self.act_portal_tab(&text("portal"), &text("action"), &text("url"), args["index"].as_u64().map(|i| i as usize)),
            "chat_list" => Ok(self.act_chat_list()),
            "chat_new" => self.act_chat_new(&text("agent"), &text("folder")),
            "chat_open" => {
                let id = self.chat_named(&text("chat"))?;
                self.chat_agent = Some(id);
                self.show_chat();
                Ok("Chat aberto.".into())
            }
            "chat_send" => {
                let id = self.chat_named(&text("chat"))?;
                if text("text").is_empty() {
                    return Err("diga o texto".into());
                }
                self.chat_outbox.push((id, text("text")));
                Ok("Mensagem enviada ao chat.".into())
            }
            "voice_call" => self.act_voice(&text("state")),
            "ide_git" => {
                let files: Vec<String> = args["files"].as_array().into_iter().flatten().filter_map(|f| f.as_str().map(String::from)).collect();
                let diff = (text("base"), text("mode"));
                self.act_ide_git(&text("ide"), &text("action"), &text("message"), &files, diff)
            }
            "ide_conflict" => self.act_ide_conflict(&text("ide"), &text("path"), &text("action")),
            "code_comment" => self.act_code_comment(
                &text("ide"),
                &text("path"),
                args["from"].as_u64().unwrap_or(0) as usize,
                args["to"].as_u64().unwrap_or(0) as usize,
                &text("comment"),
            ),
            "ide_database" => {
                let (index, id) = self.ide_named(&text("ide"))?;
                self.show_ide(index);
                self.ides.get_mut(&id).ok_or("o IDE ainda está abrindo")?.run(crate::ide::Cmd::Database);
                Ok("Painel Banco de dados do IDE alternado.".into())
            }
            "set_setting" => self.act_setting(&text("name"), &text("value").to_lowercase()),
            "demands" => self.act_demands(&text("agent"), &text("answer"), &text("reason")),
            "connection_history" => self.act_connection_history(&text("a"), &text("b"), args["limit"].as_u64().unwrap_or(10).clamp(1, 50) as usize),
            "history" => {
                match text("action").as_str() {
                    "redo" => self.redo(),
                    _ => self.undo(),
                }
                Ok("Feito.".into())
            }
            other => Err(format!("ação desconhecida: {other}")),
        }
    }

    /// The MCP's `connection_history`: the last `n` messages between terminals `a` and `b`, by name.
    fn act_connection_history(&self, a: &str, b: &str, n: usize) -> Result<String, String> {
        let rows = self.agent_rows();
        let find = |name: &str| {
            rows.iter()
                .find(|r| r.id == name || r.spec.label().to_lowercase() == name.to_lowercase())
                .map(|r| (r.id.clone(), r.spec.label().to_string()))
                .ok_or_else(|| format!("Nenhum agente chamado {name}."))
        };
        let ((a, a_name), (b, b_name)) = (find(a)?, find(b)?);
        let events = self.connection_events(&a, &b, n);
        if events.is_empty() {
            return Ok(format!("{a_name} e {b_name} ainda não conversaram pela conexão."));
        }
        let mut out = format!("Mensagens entre {a_name} e {b_name}, da mais antiga à mais nova:");
        for e in &events {
            out += &format!("\n- {} · {}", crate::app::ui::agents::ago(e.age()), self.event_line(e));
        }
        Ok(out)
    }

    /// The IDE on `path` (the one already there, or a new one), enlarged, and the folder's
    /// structure for the agent.
    fn act_ide_folder(&mut self, path: &str) -> Result<String, String> {
        let index = self.ide_for_folder(path)?;
        self.show_ide(index);
        let root = self.scene.elements[index].ide.as_ref().map(|i| i.root.clone()).unwrap_or_default();
        Ok(format!("A pasta {root} está aberta no IDE. Estrutura:\n{}", folder_tree(Path::new(&root), 3, 300)))
    }

    /// The IDE whose project is `path`, made in the middle of the view when there is none.
    pub(crate) fn ide_for_folder(&mut self, path: &str) -> Result<usize, String> {
        let root = folder(path).filter(|p| p.is_dir()).ok_or_else(|| format!("A pasta {path} não existe."))?;
        let root = root.to_string_lossy().trim_end_matches(['/', '\\']).to_string();
        let same = |e: &Element| e.ide.as_ref().is_some_and(|i| folder(&i.root).is_some_and(|p| p.to_string_lossy().trim_end_matches(['/', '\\']) == root));
        if let Some(index) = self.scene.elements.iter().position(same) {
            return Ok(index);
        }
        let at = Rect::from_center_size(self.to_world(self.canvas_rect.center()), IDE_SIZE);
        let mut ide = self.new_element(ElementKind::Rectangle { x: at.min.x, y: at.min.y, width: at.width(), height: at.height() });
        ide.style = node_style(ide.style);
        let name = Path::new(&root).file_name().map_or("IDE".to_string(), |n| n.to_string_lossy().to_string());
        ide.ide = Some(IdeSpec { name, root, ..Default::default() });
        self.snapshot();
        self.scene.elements.push(ide);
        Ok(self.scene.elements.len() - 1)
    }

    fn show_ide(&mut self, index: usize) {
        self.chat_open = false;
        self.enlarge_ide(index);
        self.ide_focus = Some(self.scene.elements[index].id.clone());
    }

    /// A folder opens its IDE; a PDF, a picture or a page opens in a portal; any other file in the
    /// IDE of its project (or of its folder), at `line`.
    fn act_open_path(&mut self, path: &str, line: Option<usize>) -> Result<String, String> {
        let full = folder(path).filter(|p| p.exists()).ok_or_else(|| format!("{path} não existe."))?;
        if full.is_dir() {
            return self.act_ide_folder(path);
        }
        let ext = full.extension().map(|e| e.to_string_lossy().to_lowercase()).unwrap_or_default();
        if PAGE_KINDS.contains(&ext.as_str()) {
            let url = format!("file:///{}", full.to_string_lossy().replace('\\', "/").trim_start_matches('/'));
            let name = full.file_name().map_or(String::new(), |n| n.to_string_lossy().to_string());
            let at = self.free_spot(Rect::from_center_size(self.to_world(self.canvas_rect.center()), Vec2::new(720.0, 860.0)));
            let mut portal = self.new_element(ElementKind::Rectangle { x: at.min.x, y: at.min.y, width: at.width(), height: at.height() });
            portal.style = node_style(portal.style);
            portal.portal = Some(PortalSpec { name: name.clone(), url, ..Default::default() });
            self.snapshot();
            self.scene.elements.push(portal);
            self.publish_nodes();
            self.show_canvas();
            self.fit_view(at, None);
            return Ok(format!("{name} aberto num portal no canvas."));
        }
        // the IDE whose project holds it, else one on its folder
        let inside = |e: &Element| e.ide.as_ref().and_then(|i| folder(&i.root)).is_some_and(|root| full.starts_with(root));
        let index = match self.scene.elements.iter().position(inside) {
            Some(index) => index,
            None => self.ide_for_folder(&full.parent().unwrap_or(Path::new("")).to_string_lossy())?,
        };
        self.show_ide(index);
        // a new IDE's editor opens on the next frame (sync_ides): the file waits for it
        self.open_pending = Some((self.scene.elements[index].id.clone(), full.clone(), line));
        Ok(format!("{} aberto no IDE.", full.display()))
    }

    /// The app's window (`app`: maximize, restore, minimize, or a size) or a terminal, portal or
    /// IDE on the canvas (enlarge, shrink, or a size).
    fn act_resize(&mut self, target: &str, state: &str, width: Option<f64>, height: Option<f64>) -> Result<String, String> {
        let size = |w: f32, h: f32| Vec2::new(width.map_or(w, |v| v as f32).max(200.0), height.map_or(h, |v| v as f32).max(120.0));
        if target.is_empty() || target.eq_ignore_ascii_case("app") {
            let command = match state {
                "maximize" => egui::ViewportCommand::Maximized(true),
                "restore" => egui::ViewportCommand::Maximized(false),
                "minimize" => egui::ViewportCommand::Minimized(true),
                _ if width.is_some() || height.is_some() => egui::ViewportCommand::InnerSize(size(1280.0, 800.0)),
                _ => return Err("diga state (maximize, restore, minimize) ou width/height".into()),
            };
            self.viewport_commands.push(command);
            return Ok("Janela do MagoAgent ajustada.".into());
        }
        let index = self.node_named(target)?;
        let id = self.scene.elements[index].id.clone();
        match state {
            "enlarge" | "maximize" => {
                self.chat_open = false;
                if self.scene.elements[index].ide.is_some() {
                    self.show_ide(index)
                } else {
                    self.set_elevated(Some(id))
                }
                Ok(format!("{target} ampliado."))
            }
            "shrink" | "restore" => {
                if self.terminal_elevated.as_deref() == Some(id.as_str()) {
                    self.set_elevated(None);
                }
                Ok(format!("{target} de volta ao canvas."))
            }
            _ => {
                let ElementKind::Rectangle { width: w, height: h, .. } = &mut self.scene.elements[index].kind else {
                    return Err(format!("{target} não é uma janela"));
                };
                let new = size(*w, *h);
                self.snapshot();
                if let ElementKind::Rectangle { width: w, height: h, .. } = &mut self.scene.elements[index].kind {
                    (*w, *h) = (new.x, new.y);
                }
                Ok(format!("{target} agora tem {:.0}×{:.0}.", new.x, new.y))
            }
        }
    }

    /// A terminal, portal, IDE or note on the canvas by id or name (as list_agents shows it).
    fn node_named(&self, name: &str) -> Result<usize, String> {
        let wanted = name.trim().to_lowercase();
        let node = self.agent_nodes().into_iter().find(|n| n.id == name.trim() || n.name.to_lowercase() == wanted);
        node.and_then(|n| self.scene.elements.iter().position(|e| e.id == n.id)).ok_or_else(|| format!("Nada no canvas se chama {name}."))
    }

    /// The IDE by name; unnamed, the one enlarged, else the only one.
    fn ide_named(&self, name: &str) -> Result<(usize, String), String> {
        let ides: Vec<usize> = (0..self.scene.elements.len()).filter(|&i| self.scene.elements[i].ide.is_some()).collect();
        let pick = if name.is_empty() {
            let enlarged = ides.iter().copied().find(|&i| self.terminal_elevated.as_deref() == Some(self.scene.elements[i].id.as_str()));
            enlarged.or(if ides.len() == 1 { Some(ides[0]) } else { None })
        } else {
            ides.iter().copied().find(|&i| self.scene.elements[i].ide.as_ref().is_some_and(|s| s.name.eq_ignore_ascii_case(name)))
        };
        let index = pick.ok_or_else(|| if ides.is_empty() { "Não há IDE no canvas.".to_string() } else { "Diga qual IDE.".to_string() })?;
        Ok((index, self.scene.elements[index].id.clone()))
    }

    /// The chat whose name has `name` (empty: the one shown).
    fn chat_named(&self, name: &str) -> Result<String, String> {
        if name.trim().is_empty() {
            return self.chat_agent.clone().filter(|id| self.is_chat(id)).ok_or_else(|| "Nenhum chat aberto.".into());
        }
        let wanted = name.trim().to_lowercase();
        let found = self.chats.iter().find(|c| c.name.to_lowercase() == wanted).or_else(|| self.chats.iter().find(|c| c.name.to_lowercase().contains(&wanted)));
        found.map(|c| c.id.clone()).ok_or_else(|| format!("Nenhum chat se chama {name}."))
    }

    fn act_chat_list(&self) -> String {
        if self.chats.is_empty() {
            return "Não há chats.".into();
        }
        let line = |c: &crate::app::chat_direct::Chat| {
            let open = if self.chat_agent.as_deref() == Some(c.id.as_str()) { " (aberto)" } else { "" };
            let busy = if self.chat_runs.contains_key(&c.id) { ", respondendo" } else { "" };
            format!("{}{open}: {} em {}{busy}", c.name, c.agent, c.cwd)
        };
        self.chats.iter().map(line).collect::<Vec<_>>().join("\n")
    }

    fn act_chat_new(&mut self, agent: &str, folder_path: &str) -> Result<String, String> {
        let installed = self.installed_agents();
        if installed.is_empty() {
            return Err("Nenhum agente instalado (claude ou codex).".into());
        }
        let mut form = crate::app::chat_direct::NewChat::from_prefs(&self.prefs, &installed);
        if !agent.is_empty() {
            form.agent =
                installed.iter().position(|p| p.command.split_whitespace().next() == Some(agent)).ok_or_else(|| format!("{agent} não está instalado."))?;
        }
        if !folder_path.is_empty() {
            folder(folder_path).filter(|p| p.is_dir()).ok_or_else(|| format!("A pasta {folder_path} não existe."))?;
            form.cwd = folder_path.to_string();
        }
        self.create_chat(&form, &installed);
        self.show_chat();
        Ok(format!("Chat novo aberto: {}.", self.chats.last().map_or("", |c| c.name.as_str())))
    }

    fn act_voice(&mut self, state: &str) -> Result<String, String> {
        use std::sync::atomic::Ordering;
        let muted = self.voice.shared.muted.load(Ordering::Relaxed);
        match state {
            "on" | "off" if self.voice.on == (state == "on") => {}
            "on" | "off" => self.voice_toggle(),
            "mute" | "unmute" if !self.voice.on => return Err("A chamada não está ligada.".into()),
            "mute" | "unmute" if muted != (state == "mute") => self.voice_mute(),
            "speakers_off" | "speakers_on" if self.voice.speakers_off != (state == "speakers_off") => self.voice_speakers(),
            "mute" | "unmute" | "speakers_off" | "speakers_on" => {}
            other => return Err(format!("estado desconhecido: {other}")),
        }
        Ok("Chamada ajustada.".into())
    }

    /// `diff` is the base and the mode of the action diff.
    fn act_ide_git(&mut self, name: &str, action: &str, message: &str, files: &[String], diff: (String, String)) -> Result<String, String> {
        let (index, id) = self.ide_named(name)?;
        self.show_ide(index);
        let ide = self.ides.get_mut(&id).ok_or("o IDE ainda está abrindo")?;
        match action {
            "panel" => ide.run(crate::ide::Cmd::Commit),
            "commit" if message.trim().is_empty() => return Err("diga a mensagem do commit".into()),
            "commit" => return ide.agent_commit(message.trim(), files),
            "push" => ide.run(crate::ide::Cmd::Push),
            "pull" => ide.run(crate::ide::Cmd::Pull),
            "diff" => return ide.agent_diff(&diff.0, &diff.1),
            other => return Err(format!("ação desconhecida: {other}")),
        }
        Ok(format!("{action} começou no IDE; o resultado aparece nele."))
    }

    fn act_ide_conflict(&mut self, name: &str, path: &str, action: &str) -> Result<String, String> {
        let (index, id) = self.ide_named(name)?;
        self.show_ide(index);
        self.ides.get_mut(&id).ok_or("o IDE ainda está abrindo")?.resolve_conflict(path, action)
    }

    fn act_code_comment(&mut self, name: &str, path: &str, from: usize, to: usize, comment: &str) -> Result<String, String> {
        if comment.trim().is_empty() {
            return Err("diga o comentário".into());
        }
        let (index, id) = self.ide_named(name)?;
        self.show_ide(index);
        let ide = self.ides.get_mut(&id).ok_or("o IDE ainda está abrindo")?;
        let full = Path::new(&ide.folder).join(path);
        let text = std::fs::read_to_string(&full).map_err(|e| e.to_string())?;
        let reference = ide.code_reference(&full, &text, from, to, comment, "editor", "local")?;
        if !ide.has_agent {
            return Err("Ligue um agente a este IDE para comentar.".into());
        }
        ide.to_agent.push(crate::ide::coderef::message(&reference));
        Ok("Comentário enviado ao agente ligado ao IDE.".into())
    }

    fn act_setting(&mut self, name: &str, value: &str) -> Result<String, String> {
        match (name, value) {
            ("theme", "light" | "claro") => self.prefs.theme = Theme::Light,
            ("theme", "dark" | "escuro") => self.prefs.theme = Theme::Dark,
            ("theme", "system" | "sistema") => self.prefs.theme = Theme::System,
            ("terminal_history", "1000" | "1.000") => self.prefs.terminal_history = 1_000,
            ("terminal_history", "5000" | "5.000") => self.prefs.terminal_history = 5_000,
            ("terminal_history", "10000" | "10.000") => self.prefs.terminal_history = 10_000,
            ("voice", "codex") => self.prefs.voice_codex = true,
            ("voice", "openai") => (self.prefs.voice_codex, self.prefs.voice_openai) = (false, true),
            ("voice", "local") => (self.prefs.voice_codex, self.prefs.voice_openai) = (false, false),
            ("avatar", "on" | "ligado") => self.prefs.avatar = true,
            ("avatar", "off" | "desligado") => self.prefs.avatar = false,
            ("avatar_follow", "on" | "ligado") => self.avatar_track_window(true),
            ("avatar_follow", "off" | "desligado") => self.avatar_track_window(false),
            _ => return Err(format!("ajuste desconhecido: {name} = {value}")),
        }
        Ok(format!("{name} agora é {value}."))
    }

    fn act_portal_tab(&mut self, portal: &str, action: &str, url: &str, index: Option<usize>) -> Result<String, String> {
        let at = self.node_named(portal)?;
        let spec = self.scene.elements[at].portal.as_ref().ok_or_else(|| format!("{portal} não é um portal"))?;
        let (mut tabs, active) = (spec.tab_urls(), spec.active_tab());
        let (tabs, active) = match action {
            "new" => {
                tabs.push(crate::app::portal::normalize_url(url));
                let last = tabs.len() - 1;
                (tabs, last)
            }
            "close" if tabs.len() == 1 => return Err("é a única aba; feche o portal com close_window".into()),
            "close" => {
                let i = index.unwrap_or(active).min(tabs.len() - 1);
                tabs.remove(i);
                // the one after the closed tab moves into its place
                let next = if active > i { active - 1 } else { active };
                let last = tabs.len() - 1;
                (tabs, next.min(last))
            }
            "select" => {
                let i = index.ok_or("diga o index da aba")?;
                if i >= tabs.len() {
                    return Err(format!("o portal tem {} aba(s)", tabs.len()));
                }
                (tabs, i)
            }
            other => return Err(format!("ação desconhecida: {other}")),
        };
        self.snapshot();
        if let Some(spec) = self.scene.elements[at].portal.as_mut() {
            spec.set_tabs(tabs, active);
        }
        Ok(format!("Abas de {portal} ajustadas."))
    }

    /// Each frame: a file waiting for its new IDE's editor, the window's own commands, the
    /// messages for chats.
    pub(crate) fn actions_tick(&mut self, ctx: &Context) {
        for (id, text) in std::mem::take(&mut self.chat_outbox) {
            self.chat_send(ctx, &id, &text);
        }
        for command in std::mem::take(&mut self.viewport_commands) {
            ctx.send_viewport_cmd(command);
        }
        if let Some((id, path, line)) = self.open_pending.clone()
            && self.ides.contains_key(&id)
        {
            self.open_pending = None;
            let (reply, _) = std::sync::mpsc::channel();
            self.ide_agent(&id, IdeAction::Open { path, line }, reply);
        }
    }
}

/// A folder's structure as indented lines, `depth` levels and `cap` lines at most; hidden
/// entries and build folders left out.
pub(crate) fn folder_tree(root: &Path, depth: usize, cap: usize) -> String {
    fn walk(dir: &Path, level: usize, depth: usize, lines: &mut Vec<String>, cap: usize) {
        let Ok(read) = fs::read_dir(dir) else { return };
        let mut entries: Vec<(bool, String, PathBuf)> = read
            .flatten()
            .map(|e| (e.path().is_dir(), e.file_name().to_string_lossy().to_string(), e.path()))
            .filter(|(_, name, _)| !name.starts_with('.') && !["target", "node_modules", "__pycache__"].contains(&name.as_str()))
            .collect();
        entries.sort_by(|a, b| b.0.cmp(&a.0).then(a.1.to_lowercase().cmp(&b.1.to_lowercase())));
        for (is_dir, name, path) in entries {
            if lines.len() >= cap {
                lines.push("…".into());
                return;
            }
            lines.push(format!("{}{name}{}", "  ".repeat(level), if is_dir { "/" } else { "" }));
            if is_dir && level + 1 < depth {
                walk(&path, level + 1, depth, lines, cap);
            }
        }
    }
    let mut lines = Vec::new();
    walk(root, 0, depth, &mut lines, cap);
    if lines.last().is_some_and(|l| l == "…") {
        lines.dedup();
    }
    lines.join("\n")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_agents_open_a_folder_a_pdf_and_resize_windows() {
        let dir = std::env::temp_dir().join(format!("mago-acts-{}", std::process::id()));
        fs::create_dir_all(dir.join("src/fundo")).unwrap();
        fs::write(dir.join("src/main.rs"), "fn main() {}\n").unwrap();
        fs::write(dir.join("manual.pdf"), "%PDF-1.4\n").unwrap();
        fs::write(dir.join(".oculto"), "").unwrap();
        let tree = folder_tree(&dir, 3, 300);
        assert_eq!(tree, "src/\n  fundo/\n  main.rs\nmanual.pdf");
        let mut app = DrawingApp::default();
        app.canvas_rect = Rect::from_min_size(Pos2::ZERO, Vec2::new(800.0, 600.0));
        app.chat_open = true;
        let path = dir.to_string_lossy().to_string();
        // the folder: an IDE on it, enlarged, and its structure
        assert!(app.act("ide_folder", &json!({ "path": path })).unwrap().contains("main.rs"));
        let ide = app.scene.elements.iter().position(|e| e.ide.is_some()).unwrap();
        assert_eq!(app.terminal_elevated.as_ref(), Some(&app.scene.elements[ide].id));
        assert!(!app.chat_open);
        // a source file goes to that same IDE, once its editor is up
        app.act("open_path", &json!({ "path": dir.join("src/main.rs").to_string_lossy(), "line": 1 })).unwrap();
        assert_eq!(app.scene.elements.iter().filter(|e| e.ide.is_some()).count(), 1);
        assert!(app.open_pending.as_ref().is_some_and(|(id, ..)| *id == app.scene.elements[ide].id));
        // a PDF opens in a portal
        app.act("open_path", &json!({ "path": dir.join("manual.pdf").to_string_lossy() })).unwrap();
        let portal = app.scene.elements.iter().find_map(|e| e.portal.as_ref()).unwrap();
        assert!(portal.url.starts_with("file:///") && portal.url.ends_with("manual.pdf"), "{}", portal.url);
        // resized by name, enlarged, back, and the app's window
        let name = app.scene.elements[ide].ide.as_ref().unwrap().name.clone();
        app.act("resize_window", &json!({ "target": name, "width": 900, "height": 700 })).unwrap();
        assert!(matches!(app.scene.elements[ide].kind, ElementKind::Rectangle { width: 900.0, height: 700.0, .. }));
        app.act("resize_window", &json!({ "target": name, "state": "shrink" })).unwrap();
        assert!(app.terminal_elevated.is_none());
        // what opened brought the app's window to the front
        assert!(std::mem::take(&mut app.viewport_commands).contains(&egui::ViewportCommand::Focus));
        app.act("resize_window", &json!({ "target": "app", "state": "maximize" })).unwrap();
        assert_eq!(app.viewport_commands, vec![egui::ViewportCommand::Maximized(true)]);
        assert!(app.act("resize_window", &json!({ "target": "nada" })).is_err());
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn an_ide_commit_names_its_files() {
        let dir = std::env::temp_dir().join(format!("mago-acts-git-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        assert!(std::process::Command::new("git").args(["init", "-q"]).current_dir(&dir).status().unwrap().success());
        let mut app = DrawingApp { canvas_rect: Rect::from_min_size(Pos2::ZERO, Vec2::new(800.0, 600.0)), ..Default::default() };
        app.act("ide_folder", &json!({ "path": dir.to_string_lossy() })).unwrap();
        app.sync_ides(&Context::default());
        // `files` reaches the IDE: a file with no change is refused, nothing runs
        let err = app.act("ide_git", &json!({ "action": "commit", "message": "m", "files": ["nada.txt"] })).unwrap_err();
        assert!(err.contains("nada.txt"), "{err}");
        assert!(app.act("ide_git", &json!({ "action": "commit" })).is_err(), "no message");
        app.ides.clear();
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn an_ide_diff_takes_a_base_and_a_mode() {
        let dir = std::env::temp_dir().join(format!("mago-acts-diff-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        assert!(std::process::Command::new("git").args(["init", "-q"]).current_dir(&dir).status().unwrap().success());
        let mut app = DrawingApp { canvas_rect: Rect::from_min_size(Pos2::ZERO, Vec2::new(800.0, 600.0)), ..Default::default() };
        app.act("ide_folder", &json!({ "path": dir.to_string_lossy() })).unwrap();
        app.sync_ides(&Context::default());
        let err = app.act("ide_git", &json!({ "action": "diff", "mode": "lado" })).unwrap_err();
        assert!(err.contains("lado"), "{err}");
        assert!(app.act("ide_git", &json!({ "action": "diff", "base": "nao-existe" })).unwrap_err().contains("nao-existe"));
        // empty base is HEAD; the mode sticks
        assert!(app.act("ide_git", &json!({ "action": "diff", "base": "", "mode": "unified" })).unwrap().contains("HEAD"));
        assert!(app.ides.values().all(|ide| ide.unified_diff));
        app.ides.clear();
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn the_agents_drive_chats_tabs_settings_and_history() {
        let mut app = DrawingApp::default();
        app.canvas_rect = Rect::from_min_size(Pos2::ZERO, Vec2::new(800.0, 600.0));
        let chat = |id: &str, name: &str| crate::app::chat_direct::Chat {
            id: id.into(),
            agent: "claude".into(),
            name: name.into(),
            cwd: "C:/p".into(),
            session: None,
            mode: 0,
            model: String::new(),
            effort: String::new(),
        };
        app.chats = vec![chat("a", "Claude Code · loja"), chat("b", "Codex · site")];
        // chats: listed, opened by part of the name, a message queued for the next frame
        app.act("chat_open", &json!({ "chat": "site" })).unwrap();
        assert!(app.chat_open && app.chat_agent.as_deref() == Some("b"));
        assert!(app.act("chat_list", &json!({})).unwrap().contains("Codex · site (aberto)"));
        app.act("chat_send", &json!({ "chat": "loja", "text": "roda os testes" })).unwrap();
        assert_eq!(app.chat_outbox, vec![("a".to_string(), "roda os testes".to_string())]);
        assert!(app.act("chat_open", &json!({ "chat": "nada" })).is_err());
        assert!(app.act("voice_call", &json!({ "state": "mute" })).is_err(), "no call on");
        // a portal's tabs
        let mut portal = app.new_element(ElementKind::Rectangle { x: 0.0, y: 0.0, width: 600.0, height: 400.0 });
        portal.portal = Some(PortalSpec { name: "Docs".into(), url: "https://a.dev".into(), ..Default::default() });
        app.scene.elements.push(portal);
        app.act("portal_tab", &json!({ "portal": "docs", "action": "new", "url": "b.dev" })).unwrap();
        let spec = |app: &DrawingApp| app.scene.elements[0].portal.clone().unwrap();
        assert_eq!((spec(&app).tab_urls(), spec(&app).active_tab()), (vec!["https://a.dev".to_string(), "https://b.dev".to_string()], 1));
        app.act("portal_tab", &json!({ "portal": "Docs", "action": "select", "index": 0 })).unwrap();
        app.act("portal_tab", &json!({ "portal": "Docs", "action": "close", "index": 0 })).unwrap();
        assert_eq!((spec(&app).url.as_str(), spec(&app).tab_urls().len()), ("https://b.dev", 1));
        assert!(app.act("portal_tab", &json!({ "portal": "Docs", "action": "close" })).is_err(), "the last tab stays");
        assert!(app.act("terminal_restart", &json!({ "name": "Docs" })).is_err());
        // settings, closing a window and undoing it
        app.act("set_setting", &json!({ "name": "theme", "value": "claro" })).unwrap();
        app.act("set_setting", &json!({ "name": "voice", "value": "local" })).unwrap();
        assert!(app.prefs.theme == Theme::Light && !app.prefs.voice_codex && !app.prefs.voice_openai);
        // the avatar: shown, and left where it is instead of following the window in use
        app.act("set_setting", &json!({ "name": "avatar", "value": "on" })).unwrap();
        app.act("set_setting", &json!({ "name": "avatar_follow", "value": "off" })).unwrap();
        assert!(app.prefs.avatar && app.prefs.avatar_at.is_some());
        app.act("set_setting", &json!({ "name": "avatar_follow", "value": "on" })).unwrap();
        assert_eq!(app.prefs.avatar_at, None);
        app.act("close_window", &json!({ "name": "docs" })).unwrap();
        assert!(app.scene.elements.is_empty());
        app.act("history", &json!({ "action": "undo" })).unwrap();
        assert_eq!(app.scene.elements.len(), 1);
    }
}
