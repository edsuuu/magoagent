//! What the agents do on the app through the MCP's `act` (mcp/bridge/mod.rs): a file tree on a folder with
//! its structure, a file or a PDF opened where it reads, a window resized. Each action is a name
//! and its JSON arguments; a new one is a case here and a tool in mcp/calls.rs.

use crate::app::chat::direct::folder;
use crate::prelude::*;
use std::path::Path;
mod chats;
mod dictionary;
mod email;
mod files;
mod integrations;
mod settings;
mod sidebar;
mod spotify;
#[cfg(test)]
mod tests;
mod windows;

/// Files a page shows better than the editor: they open in a portal.
const PAGE_KINDS: [&str; 9] = ["pdf", "png", "jpg", "jpeg", "gif", "webp", "svg", "html", "htm"];

impl DrawingApp {
    /// The bridge's `act`: one of the app's actions, by name. What it opens shows in the app without bringing the window
    /// to the front: minimized or behind another program, the app stays where the person left it. Only a question or a
    /// permission an agent waits on brings it (`ask_person`).
    pub(crate) fn act(&mut self, action: &str, args: &Value) -> Result<String, String> {
        self.act_on(action, args)
    }

    /// The app's window to the front, out of the taskbar if minimized.
    pub(crate) fn bring_to_front(&mut self) {
        self.viewport_commands.extend([egui::ViewportCommand::Minimized(false), egui::ViewportCommand::Focus]);
    }

    fn act_on(&mut self, action: &str, args: &Value) -> Result<String, String> {
        let text = |key: &str| args[key].as_str().unwrap_or_default().trim().to_string();

        match action {
            "update" if args["later"].as_bool() == Some(true) || args["skip"].as_bool() == Some(true) => {
                self.update_put_off(args["skip"].as_bool() == Some(true))
            }
            "update" => self.act_update(args["install"].as_bool().unwrap_or(false), args["confirmed"].as_bool().unwrap_or(false)),
            "tour" => {
                let steps = crate::app::ui::tour::STEPS.len();

                self.start_tour_at(args["step"].as_u64().map_or(0, |step| step as usize).min(steps - 1));

                Ok(format!(
                    "A apresentação abriu na tela, no passo {} de {steps}; a pessoa avança, volta ou pula.",
                    self.tour.as_ref().map_or(1, |tour| tour.step + 1)
                ))
            }
            // the bridge's ask goes by `memory_job` (act_later), off the interface's thread
            "memory" => self.memory_job()(),
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
            "open_path" => self.act_open_path(&text("path"), args["line"].as_u64().map(|l| l as usize), &text("asker")),
            "file_link" if text("action") == "back" => self.link_go_back(),
            "file_link" => self.act_file_link(&text("path"), args["line"].as_u64().map(|line| line as usize), &text("asker")),
            "open_as_note" => {
                let path = folder(&text("path")).filter(|_| !text("path").is_empty()).ok_or("diga `path`: o arquivo a abrir na nota")?;

                let chat = match text("chat").as_str() {
                    "" => self.chat_agent.clone().filter(|id| self.is_chat(id)),
                    name => Some(self.chat_named(name)?),
                };

                self.open_file_as_note(&path, chat.as_deref())
            }
            "resize_window" => self.act_resize(&text("target"), &text("state"), args["width"].as_f64(), args["height"].as_f64()),
            "close_window" => {
                let index = match self.node_named(&text("name")) {
                    Ok(index) => index,
                    // not on the canvas shown: a terminal running in a workspace off view closes there
                    Err(why) => return self.close_away(&text("name")).unwrap_or(Err(why)),
                };

                let left = self.left_running(&self.scene.elements[index].id);

                self.selected = vec![index];
                self.delete_selected();
                Ok(format!("{} fechado (dá para desfazer).{left}", text("name")))
            }
            "create_terminal" => self.act_create_terminal(args),
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
            "organize_canvas" => {
                let names = args["elements"].as_array().into_iter().flatten().filter_map(Value::as_str);

                let only: Vec<usize> = names
                    .map(|name| self.scene.elements.iter().position(|element| element.id == name.trim()).map_or_else(|| self.node_named(name), Ok))
                    .collect::<Result<_, _>>()?;

                match self.arrange_canvas((!only.is_empty()).then_some(only.as_slice())) {
                    0 => Err("Nada para organizar: são precisos 2 ou mais elementos soltos (os bloqueados ficam onde estão).".into()),
                    moved => Ok(format!("{moved} itens organizados em fileiras, com 60 px entre eles.")),
                }
            }
            "portal_tab" => self.act_portal_tab(&text("portal"), &text("action"), &text("url"), args["index"].as_u64().map(|i| i as usize)),
            "portal_settings" => self.act_portal_settings(args),
            "portal_copy_screenshot" => {
                let id = self.scene.elements[self.node_named(&text("portal"))?].id.clone();

                self.portal_copy_shot(&id)
            }
            "chat_node" => self.act_chat_node(args),
            "chat_decide" => self.act_chat_decide(args),
            "disconnect" => self.act_disconnect(&text("a"), &text("b")),
            "chat_list" => Ok(self.act_chat_list()),
            "chats_view" => {
                let team = Some(text("team")).filter(|team| !team.is_empty());

                self.act_chats_view(&text("state"), &text("filter"), args["query"].as_str(), team.as_deref().map(|team| (team, args["team_open"] != false)))
            }
            "chat_new" => self.act_chat_new(&text("agent"), &text("folder")),
            "chat_provider" => {
                let id = if text("chat").is_empty() { self.chat_agent.clone().ok_or("nenhum chat aberto")? } else { self.chat_named(&text("chat"))? };

                self.chat_switch_agent(&id, &text("agent"))
            }
            "chat_open" if text("place") == "island" => {
                // as the Agentes button of the island's conversation: the chat in the island, no window over the screen
                let id = self.chat_named(&text("chat"))?;

                if !self.prefs.island {
                    return Err("A ilha está desligada (set_setting island on liga).".into());
                }

                self.chat_agent = Some(id);
                self.island_show_page(crate::app::island::Tab::Chat);
                Ok("Chat aberto na conversa da ilha.".into())
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

                    // with the island on, the small conversation is the island's: no panel over the canvas
                    if self.prefs.island {
                        self.open_mini_chat(&id);

                        return Ok("Chat aberto na ilha.".into());
                    }

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

                // another agent's words go signed with its name: in the chat they are not taken for the person's
                let asker = text("asker");

                // the boss to its recruit: a question in the recruit's line, whose answer comes back to the boss's chat
                // when it is done (ask.rs), as a recruit's first task does
                if self.is_chat(&asker) && self.boss_of(&id).as_deref() == Some(asker.as_str()) {
                    let (reply, _) = std::sync::mpsc::channel();
                    let name = self.speaker(&asker);

                    self.ask_chat(&id, &asker, &name, &text("text"), reply);

                    return Ok("Mensagem enviada ao agente do seu time; a resposta dele chega neste chat quando ele terminar.".into());
                }

                let words = if asker.is_empty() || asker == id { text("text") } else { crate::app::maestro::signed(&self.speaker(&asker), &text("text")) };

                self.chat_outbox.push((id, words));
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
            "chat_queue" => self.act_chat_queue(&text("agent"), &text("action")),
            "chat_rename" => {
                let id = self.chat_named(&text("chat"))?;

                match self.rename_chat(&id, &text("name")) {
                    true => Ok(format!("O chat agora se chama {}.", text("name").trim())),
                    false => Err("Diga o nome novo do chat em `name`.".into()),
                }
            }
            "chat_context" => self.act_chat_context(&text("chat"), &text("state")),
            "chat_search" => self.act_chat_search(&text("query"), &text("chat"), args["limit"].as_u64().unwrap_or(20).clamp(1, 50) as usize),
            "skills" => self.act_skills(&text("action")),
            "voice_call" => self.act_voice(&text("state")),
            "maestro_comment" => self.act_maestro_comment(
                &text("tree"),
                &text("path"),
                args["from_line"].as_u64().unwrap_or(0) as usize,
                args["to_line"].as_u64().unwrap_or(0) as usize,
                &text("comment"),
            ),
            // a folder's path keeps its case; the other values are words
            "set_setting" if matches!(text("name").as_str(), "favorite_folder" | "style_skill" | "chat_folder") => {
                self.act_setting(&text("name"), &text("value"))
            }
            "set_setting" => self.act_setting(&text("name"), &text("value").to_lowercase()),
            "integrations" => self.act_integrations(args),
            "dictionary" => self.act_dictionary(args),
            "email" => self.act_email(args),
            "demands" => self.act_demands(&text("agent"), &text("answer"), &text("reason")),
            "outside_message" => self.act_outside_message(&text("session"), &text("text")),
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
            "sidebar" => self.act_sidebar(&text("action"), &text("team")),
            "panel" => self.act_panel(args),
            "quick_note" => self.act_quick_note(args),
            "changes_review" => self.act_changes_review(args),
            "file_tree" => self.act_file_tree(args),
            "tasks" => self.act_tasks(args),
            "doubts" => self.act_doubts(args),
            "stop_all" => Ok(self.stop_all()),
            "flow" => self.act_flow(args),
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
