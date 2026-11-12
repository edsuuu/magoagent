//! What the agents do on the app through the MCP's `act` (bridge.rs): the IDE on a folder with
//! its structure, a file or a PDF opened where it reads, a window resized. Each action is a name
//! and its JSON arguments; a new one is a case here and a tool in mcp/calls.rs.

use crate::app::chat::direct::folder;
use crate::prelude::*;
use std::path::Path;
mod chats;
mod ide;
mod settings;
#[cfg(test)]
mod tests;
mod windows;

/// Files a page shows better than the editor: they open in a portal.
const PAGE_KINDS: [&str; 9] = ["pdf", "png", "jpg", "jpeg", "gif", "webp", "svg", "html", "htm"];

impl DrawingApp {
    /// The bridge's `act`: one of the app's actions, by name. What opens something shows it: the
    /// app's window comes to the front with it.
    pub(crate) fn act(&mut self, action: &str, args: &Value) -> Result<String, String> {
        let result = self.act_on(action, args);
        if result.is_ok() && matches!(action, "ide_folder" | "open_path" | "chat_new" | "chat_open" | "chat_node") {
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
            "update" => self.act_update(args["install"].as_bool().unwrap_or(false)),
            "memory" => Ok(crate::app::system::memory::format(&self.memory.snapshot())),
            "ide_folder" => self.act_ide_folder(&text("path")),
            "open_default" => self.act_open_default(&text("path")),
            "canvas_drop" => {
                let paths: Vec<PathBuf> = args["paths"].as_array().into_iter().flatten().filter_map(|p| folder(p.as_str()?)).collect();
                if paths.is_empty() {
                    return Err("diga `paths`: os caminhos a soltar no canvas".into());
                }
                let middle = self.to_world(self.canvas_rect.center());
                let at = Pos2::new(args["x"].as_f64().map_or(middle.x, |x| x as f32), args["y"].as_f64().map_or(middle.y, |y| y as f32));
                self.drop_on_canvas(&paths, at)
            }
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
            "note_stack" => {
                let names: Vec<String> = args["notes"].as_array().into_iter().flatten().filter_map(|n| n.as_str().map(String::from)).collect();
                if ["blur", "move_file"].contains(&text("action").as_str()) {
                    return self.act_note_file(&text("action"), names.first().map(String::as_str).unwrap_or_default(), args);
                }
                self.act_note_stack(&text("action"), &names)
            }
            "canvas_group" => self.act_canvas_group(args),
            "portal_tab" => self.act_portal_tab(&text("portal"), &text("action"), &text("url"), args["index"].as_u64().map(|i| i as usize)),
            "portal_settings" => self.act_portal_settings(args),
            "portal_copy_screenshot" => {
                let id = self.scene.elements[self.node_named(&text("portal"))?].id.clone();
                self.portal_copy_shot(&id)
            }
            "chat_node" => self.act_chat_node(args),
            "disconnect" => self.act_disconnect(&text("a"), &text("b")),
            "chat_list" => Ok(self.act_chat_list()),
            "chat_new" => self.act_chat_new(&text("agent"), &text("folder")),
            "chat_provider" => {
                let id = if text("chat").is_empty() { self.chat_agent.clone().ok_or("nenhum chat aberto")? } else { self.chat_named(&text("chat"))? };
                self.chat_switch_agent(&id, &text("agent"))
            }
            "chat_open" => {
                let id = self.chat_named(&text("chat"))?;
                self.chat_agent = Some(id);
                self.show_chat();
                Ok("Chat aberto.".into())
            }
            "mini_chat" => {
                let open = text("state") != "close";
                if open {
                    let id = if text("chat").is_empty() { self.chat_agent.clone().ok_or("nenhum chat")? } else { self.chat_named(&text("chat"))? };
                    self.chat_agent = Some(id);
                    self.chat_open = false;
                }
                self.mini_set(open);
                Ok(if open { "Mini chat aberto." } else { "Mini chat recolhido." }.into())
            }
            "chat_send" => {
                let id = self.chat_named(&text("chat"))?;
                if text("text").is_empty() {
                    return Err("diga o texto".into());
                }
                self.chat_outbox.push((id, text("text")));
                Ok("Mensagem enviada ao chat.".into())
            }
            "chat_command" => {
                let id = self.chat_named(&text("chat"))?;
                let command = text("command");
                let command = if command.starts_with('/') { command } else { format!("/{command}") };
                let agent = self.chats.iter().find(|c| c.id == id).map(|c| c.agent.clone()).unwrap_or_default();
                match crate::app::chat::commands::parse(&command, &agent) {
                    Some(Ok(_)) => self.chat_cmd_outbox.push((id, command)),
                    Some(Err(error)) => return Err(error),
                    None => return Err("comando desconhecido: use chat_send para mandar ao agente".into()),
                }
                Ok("Comando rodando no chat.".into())
            }
            "chat_context" => self.act_chat_context(&text("chat"), &text("state")),
            "chat_search" => self.act_chat_search(&text("query"), &text("chat"), args["limit"].as_u64().unwrap_or(20).clamp(1, 50) as usize),
            "skills" => self.act_skills(&text("action")),
            "voice_call" => self.act_voice(&text("state")),
            "ide_git" if crate::ide::more_git_action(&text("action")) => self.act_ide_git_more(&text("ide"), args),
            "ide_git" => {
                let files: Vec<String> = args["files"].as_array().into_iter().flatten().filter_map(|f| f.as_str().map(String::from)).collect();
                let diff = (text("base"), text("mode"));
                self.act_ide_git(&text("ide"), &text("action"), &text("message"), &files, diff)
            }
            "ide_conflict" => self.act_ide_conflict(&text("ide"), &text("path"), &text("action")),
            "maestro_comment" => self.act_maestro_comment(
                &text("ide"),
                &text("path"),
                args["from_line"].as_u64().unwrap_or(0) as usize,
                args["to_line"].as_u64().unwrap_or(0) as usize,
                &text("comment"),
            ),
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
            "ide_exclude" => {
                let (index, id) = self.ide_named(&text("ide"))?;
                self.show_ide(index);
                self.ides.get_mut(&id).ok_or("o IDE ainda está abrindo")?.agent_exclude(&text("action"), &text("path"))
            }
            // a folder's path keeps its case; the other values are words
            "set_setting" if text("name") == "favorite_folder" => self.act_setting("favorite_folder", &text("value")),
            "set_setting" => self.act_setting(&text("name"), &text("value").to_lowercase()),
            "demands" => self.act_demands(&text("agent"), &text("answer"), &text("reason")),
            "connection_history" => self.act_connection_history(&text("a"), &text("b"), args["limit"].as_u64().unwrap_or(10).clamp(1, 50) as usize),
            "usage" => self.act_usage(),
            "usage_refresh" => self.act_usage_refresh(),
            "timeline" => self.act_timeline(
                &text("agent"),
                &text("kind"),
                args["minutes"].as_f64().map(|m| (m as i64).clamp(1, 1440)),
                args["limit"].as_u64().unwrap_or(20).clamp(1, 100) as usize,
            ),
            "history" => {
                match text("action").as_str() {
                    "redo" => self.redo(),
                    _ => self.undo(),
                }
                Ok("Feito.".into())
            }
            "minimap" => {
                self.prefs.minimap = args["open"].as_bool().unwrap_or(!self.prefs.minimap);
                Ok(if self.prefs.minimap { "Minimapa aberto." } else { "Minimapa fechado." }.into())
            }
            "file_tree" => self.act_file_tree(args),
            "tasks" => self.act_tasks(args),
            "binder" => self.act_binder(&text("action"), &text("name"), args["text"].as_str().unwrap_or_default()),
            "workspaces" => self.act_workspaces(args),
            "floors" => self.act_floors(args),
            "workspace_groups" => self.act_space_groups(args),
            "workspace_menu" => self.act_space_menu(args),
            "terminal_menu" => self.act_terminal_menu(args),
            "terminal_find" => self.act_terminal_find(args),
            "terminal_look" => self.act_terminal_look(args),
            "terminal_processes" => self.act_terminal_processes(args),
            other => Err(format!("ação desconhecida: {other}")),
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
