//! The `ui.shell.*` verbs: each calls what the egui chrome calls on the same click, so the two never drift apart.
//! Only what has no MCP action (`mcp/actions/`) is here; Swift sends the others (workspaces, floors, sidebar, panel...) as they are.

use super::model::{tool_from_id, tool_id};
use crate::app::ui::changelog::ChangelogChoice;
use crate::app::ui::palette::{Command, Run};
use crate::app::ui::properties::apply_changed;
use crate::prelude::*;

fn color(value: &Value) -> Option<[u8; 4]> {
    let parts: Vec<u8> = value.as_array()?.iter().filter_map(|v| v.as_u64().map(|n| n.min(255) as u8)).collect();

    <[u8; 4]>::try_from(parts).ok()
}

/// A serde enum (fill style, stroke style, head) from the word Swift sends.
fn from_word<T: serde::de::DeserializeOwned>(value: &Value) -> Option<T> {
    serde_json::from_value(value.clone()).ok()
}

/// The palette's command of a menu or shortcut name.
pub(super) fn command_named(name: &str) -> Option<Command> {
    Some(match name {
        "save" => Command::Save,
        "saveAs" => Command::SaveAs,
        "open" => Command::Open,
        "export" => Command::Export,
        "undo" => Command::Undo,
        "redo" => Command::Redo,
        "selectAll" => Command::SelectAll,
        "duplicate" => Command::Duplicate,
        "delete" => Command::Delete,
        "clearCanvas" => Command::Clear,
        "group" => Command::Group,
        "ungroup" => Command::Ungroup,
        "lock" => Command::LockSelection,
        "lockAll" => Command::LockAll,
        "unlockAll" => Command::UnlockAll,
        "grid" => Command::Grid,
        "theme" => Command::DarkMode,
        "hints" => Command::ShowHints,
        "newChat" => Command::NewChat,
        "layerForward" => Command::Layer(Layer::Forward),
        "layerBackward" => Command::Layer(Layer::Backward),
        "layerFront" => Command::Layer(Layer::Front),
        "layerBack" => Command::Layer(Layer::Back),
        "zoomIn" => Command::ZoomIn,
        "zoomOut" => Command::ZoomOut,
        "zoomReset" => Command::ZoomReset,
        "fitAll" => Command::ZoomFit,
        "fitSelection" => Command::ZoomSelection,
        _ => return None,
    })
}

/// The names `shell_screen_command` answers (the test that every shortcut has a command reads them).
#[cfg(test)]
pub(super) const SCREEN_COMMANDS: [&str; 11] =
    ["settings", "palette", "showChat", "showCanvas", "openTree", "nextNeedingYou", "floors", "tidy", "minimap", "tour", "sidebar"];

impl DrawingApp {
    pub(super) fn shell_act(&mut self, verb: &str, args: &Value) -> Result<String, String> {
        let text = |key: &str| args[key].as_str().unwrap_or_default().to_string();

        match verb {
            "tool" => {
                let tool = tool_from_id(&text("id")).ok_or_else(|| format!("ferramenta desconhecida: {}", text("id")))?;

                self.select_tool(tool);
                Ok(format!("{} em mãos.", tool.label()))
            }
            // the drawing row's button: closing the row puts a drawing tool down
            "draw_toggle" => {
                let drawing = self.drawing();

                if drawing && !Tool::TOOLBAR.contains(&self.tool) {
                    self.select_tool(Tool::Select);
                }

                self.draw_open = !drawing;
                self.extra_open = false;
                Ok(String::new())
            }
            // a tool's letter, as `handle_shortcuts` takes it: only while the keys are free
            "tool_key" => self.shell_tool_key(&text("key"), args["shift"].as_bool().unwrap_or(false)),
            "command" => self.shell_command(&text("name")),
            "style" => {
                self.shell_style(args);
                Ok(String::new())
            }
            "head" => {
                let head = if text("head") == "none" { None } else { from_word::<Head>(&args["head"]) };

                self.shell_head(args["end"].as_bool().unwrap_or(true), head);
                Ok(String::new())
            }
            "frame_name" => {
                if let [index] = self.selected[..]
                    && let Some(name) = self.scene.elements.get_mut(index).and_then(|e| e.frame.as_mut())
                {
                    *name = text("name");
                    self.dirty = true;
                }

                Ok(String::new())
            }
            "zoom" => self.shell_zoom(&text("op"), args["value"].as_f64()),
            "sidebar" => {
                self.prefs.sidebar_hidden = match text("state").as_str() {
                    "show" => false,
                    "hide" => true,
                    _ => !self.prefs.sidebar_hidden,
                };

                Ok(String::new())
            }
            "screen" => {
                match text("state").as_str() {
                    "chats" => self.show_chat(),
                    "canvas" => self.show_canvas(),
                    _ if self.chat_open => self.show_canvas(),
                    _ => self.show_chat(),
                }

                Ok(String::new())
            }
            "settings" => {
                self.settings_open = args["open"].as_bool().unwrap_or(!self.settings_open);
                Ok(String::new())
            }
            "agent" => {
                self.open_agent(&text("id"));
                Ok(String::new())
            }
            // the window's close goes the way eframe's does (Fechar mantém em segundo plano, tray.rs): "hidden" tells Swift to
            // hide its window and keep the app running, anything else ends the app
            "window" => {
                if text("action") == "quit" {
                    self.quitting = true;
                }

                if crate::app::system::tray::hides_on_close(self.prefs.close_to_background, self.quitting, self.script.is_some()) {
                    self.save_on_exit();

                    return Ok("hidden".into());
                }

                self.viewport_commands.push(egui::ViewportCommand::Close);
                Ok("closing".into())
            }
            "dialog" => self.shell_dialog(&text("dialog"), &text("action"), args),
            "palette" => self.shell_palette(&text("action"), args),
            "welcome" => self.shell_welcome(&text("name"), &text("folder")),
            "tour" => {
                match (text("action").as_str(), self.tour.as_mut()) {
                    ("next", Some(tour)) if tour.step + 1 < crate::app::ui::tour::STEPS.len() => tour.step += 1,
                    ("back", Some(tour)) => tour.step = tour.step.saturating_sub(1),
                    _ => self.end_tour(),
                }

                Ok(String::new())
            }
            "changelog" => {
                let choice = match text("choice").as_str() {
                    "later" => ChangelogChoice::Later,
                    "skip" => ChangelogChoice::Skip,
                    "cancel" => ChangelogChoice::Cancel,
                    "restart" => ChangelogChoice::Restart,
                    "now" => ChangelogChoice::Now,
                    other => return Err(format!("escolha desconhecida: {other}")),
                };

                self.changelog_choose(choice);
                Ok(String::new())
            }
            other => Err(format!("ação desconhecida da janela: {other}")),
        }
    }

    fn shell_tool_key(&mut self, key: &str, shift: bool) -> Result<String, String> {
        let free = self.terminal_focus.is_none() && self.card_focus.is_none() && self.editing.is_none() && !self.dialog_open() && !self.palette_open;
        let Some(key) = Key::from_name(&key.to_uppercase()).filter(|_| free && !self.chat_open) else { return Ok(String::new()) };

        if self.tool == Tool::Fill && key == Key::B && !shift {
            // B again cycles the fill color (red, green, blue, yellow)
            let picks = &BACKGROUND_PICKS[1..];
            let at = picks.iter().position(|pick| Some(*pick) == self.style.fill).map_or(1, |index| (index + 1) % picks.len());

            self.style.fill = Some(picks[at]);

            return Ok(String::new());
        }

        if shift {
            if key == Key::X && self.drawing() {
                self.select_tool(Tool::AutoShape);
            }

            return Ok(String::new());
        }

        let tools = Tool::TOOLBAR.into_iter().chain(Tool::DRAWING).chain(Tool::EXTRA);
        let picked: Vec<Tool> = tools.filter(|tool| self.tool_keys_live(*tool) && tool.keys().contains(&key)).collect();

        for tool in &picked {
            self.select_tool(*tool);
        }

        Ok(picked.first().map_or(String::new(), |tool| tool_id(*tool)))
    }

    /// The menus' and the shortcuts' names: the ids of Ajustes › Atalhos (`hotkeys/local.rs`) and a few more.
    fn shell_command(&mut self, name: &str) -> Result<String, String> {
        match command_named(name) {
            Some(command) => {
                self.run_command(command);
                Ok(String::new())
            }
            None => self.shell_screen_command(name),
        }
    }

    /// The commands that are not the palette's: the screens, the palette itself, Ajustes, the sidebar...
    fn shell_screen_command(&mut self, name: &str) -> Result<String, String> {
        match name {
            "settings" => self.settings_open = !self.settings_open,
            "palette" if self.palette_open => self.palette_open = false,
            "palette" => self.open_palette(),
            "showChat" => self.show_chat(),
            "showCanvas" => self.show_canvas(),
            "openTree" => self.open_tree(),
            "nextNeedingYou" => {
                self.next_needing_attention();
            }
            "floors" => self.floors_ui.overview = !self.floors_ui.overview,
            "tidy" => self.tidy_selection(),
            "minimap" => self.prefs.minimap = !self.prefs.minimap,
            "tour" => self.start_tour(),
            "sidebar" => self.prefs.sidebar_hidden = !self.prefs.sidebar_hidden,
            other => return Err(format!("comando desconhecido: {other}")),
        }

        Ok(String::new())
    }

    /// The properties row's picks: the selection takes the change as one undo step (`final` ends a drag of the slider).
    fn shell_style(&mut self, args: &Value) {
        let before = self.style_kinds().first().map(|e| e.style).unwrap_or(self.style);
        let mut after = before;

        if let Some(stroke) = args.get("stroke").and_then(color) {
            after.stroke = stroke;
        }

        if let Some(fill) = args.get("fill") {
            after.fill = color(fill).filter(|c| c[3] > 0);
        }

        if let Some(value) = args.get("fillStyle").and_then(from_word) {
            after.fill_style = value;
        }

        if let Some(value) = args.get("strokeStyle").and_then(from_word) {
            after.stroke_style = value;
        }

        if let Some(width) = args["strokeWidth"].as_f64() {
            after.stroke_width = width as f32;
        }

        if let Some(roughness) = args["roughness"].as_u64() {
            after.roughness = roughness.min(2) as u8;
        }

        if let Some(round) = args["round"].as_bool() {
            after.round = round;
        }

        if let Some(opacity) = args["opacity"].as_u64() {
            after.opacity = opacity.min(100) as u8;
        }

        if let Some(size) = args["fontSize"].as_f64() {
            after.font_size = size as f32;
        }

        let arrows_only = self.styling_arrows();

        if after != before {
            if !self.selected.is_empty() {
                if !self.style_edit_active {
                    self.snapshot();
                    self.style_edit_active = true;
                }

                for index in self.selected.clone() {
                    if let Some(element) = self.scene.elements.get_mut(index) {
                        apply_changed(&mut element.style, &before, &after);
                    }
                }

                self.dirty = true;
            }

            let current = self.style;

            apply_changed(&mut self.style, &current, &after);

            if arrows_only {
                // the arrow's type is kept apart from the boxes' "edges"
                self.arrow_curved = after.round;
                self.style.round = current.round;
            }
        }

        if args["final"].as_bool().unwrap_or(true) {
            self.style_edit_active = false;
        }
    }

    fn shell_head(&mut self, end: bool, head: Option<Head>) {
        self.set_head(end, head);
    }

    fn shell_zoom(&mut self, op: &str, value: Option<f64>) -> Result<String, String> {
        let center = self.canvas_rect.center();

        match op {
            "in" => self.zoom_step(0.1),
            "out" => self.zoom_step(-0.1),
            "reset" => self.zoom_at(1.0, center),
            "fit" => self.zoom_to_fit(false),
            // dragging the percentage: a 5% step
            "set" => self.zoom_at(((value.ok_or("diga o zoom")? as f32 / 0.05).round() * 0.05).max(0.1), center),
            other => return Err(format!("zoom desconhecido: {other}")),
        }

        Ok(String::new())
    }

    fn shell_dialog(&mut self, dialog: &str, action: &str, args: &Value) -> Result<String, String> {
        match (dialog, action) {
            ("terminal", "cancel") => self.terminal_dialog = None,
            ("terminal", "confirm") => self.confirm_terminal_dialog(),
            ("terminal", "set") => {
                self.shell_terminal_field(args["field"].as_str().unwrap_or_default(), &args["value"])?;
            }
            ("export", "set") => match (args["field"].as_str().unwrap_or_default(), &args["value"]) {
                ("svg", value) => self.export_svg = value.as_bool().unwrap_or(false),
                ("scale", value) => self.export_scale = value.as_f64().unwrap_or(1.0).clamp(1.0, 3.0) as f32,
                ("background", value) => self.export_background = value.as_bool().unwrap_or(true),
                (other, _) => return Err(format!("campo desconhecido: {other}")),
            },
            ("export", "cancel") => self.export_open = false,
            ("export", "confirm") => {
                self.export_open = false;
                self.export(!self.export_svg);
            }
            ("wipe", "cancel") => self.wipe_confirm = false,
            ("wipe", "confirm") => {
                self.wipe_confirm = false;

                if let Err(why) = self.wipe_all_data() {
                    self.toast(format!("Nada foi apagado: {why}."));
                }
            }
            (dialog, action) => return Err(format!("diálogo desconhecido: {dialog} {action}")),
        }

        Ok(String::new())
    }

    /// A field of Novo / Editar Terminal, as the egui sheet sets it (terminal/dialog.rs).
    fn shell_terminal_field(&mut self, field: &str, value: &Value) -> Result<String, String> {
        let chat_agent = |preset: &Preset| matches!(preset.command.split_whitespace().next(), Some("claude" | "codex"));
        let chat_now = self.terminal_dialog.as_ref().ok_or("nenhum diálogo de terminal aberto")?.chat;
        let presets: Vec<Preset> = self.prefs.presets.iter().filter(|preset| preset.on && (!chat_now || chat_agent(preset))).take(10).cloned().collect();
        let first_chat_command = self.prefs.presets.iter().find(|preset| chat_agent(preset)).map(|preset| preset.command.clone()).unwrap_or_default();
        let dialog = self.terminal_dialog.as_mut().ok_or("nenhum diálogo de terminal aberto")?;
        let text = value.as_str().unwrap_or_default().to_string();

        match field {
            "name" => dialog.spec.name = text,
            "command" => dialog.spec.command = text,
            "cwd" => dialog.spec.cwd = text,
            "maestro" => dialog.spec.maestro = value.as_bool().unwrap_or(false),
            "startInChat" => dialog.spec.chat = value.as_bool().unwrap_or(false),
            // Chat or Terminal: a chat runs only the agents it speaks with
            "chat" => {
                let chat = value.as_bool().unwrap_or(false);

                if chat && !dialog.chat && !matches!(dialog.spec.command.split_whitespace().next(), Some("claude" | "codex")) {
                    dialog.spec.command = first_chat_command;
                }

                dialog.chat = chat;
            }
            "preset" => {
                let preset = presets.get(value.as_u64().unwrap_or(u64::MAX) as usize).ok_or("esse início rápido não existe")?;
                // the name follows the quick start, unless one was typed
                let typed = dialog.spec.name.trim();

                if typed.is_empty() || presets.iter().any(|p| p.name == typed) || typed == "Shell" {
                    dialog.spec.name = preset.name.clone();
                }

                dialog.spec.command = preset.command.clone();
                dialog.spec.icon = preset.icon.clone();
                dialog.spec.color = preset.color;
            }
            other => return Err(format!("campo desconhecido: {other}")),
        }

        Ok(String::new())
    }

    fn shell_palette(&mut self, action: &str, args: &Value) -> Result<String, String> {
        match action {
            "open" => self.open_palette(),
            "close" => {
                self.palette_open = false;
                self.palette_links = None;
            }
            "query" => {
                self.palette_query = args["query"].as_str().unwrap_or_default().to_string();
                self.palette_index = 0;
            }
            "run" => {
                let rows = self.palette_rows();
                let index = args["index"].as_u64().map_or(self.palette_index, |i| i as usize);

                match rows.get(index).map(|row| row.run.clone()) {
                    Some(Run::Command(command)) => self.run_command(command),
                    Some(Run::Node(id)) => self.pick_link_node(&id),
                    None => {}
                }

                // a connect / disconnect command keeps the palette up for its node search
                if self.palette_links.is_none() {
                    self.palette_open = false;
                }
            }
            other => return Err(format!("ação da paleta desconhecida: {other}")),
        }

        Ok(String::new())
    }

    /// The welcome's last step, as `finish_welcome` takes it (ui/welcome/mod.rs): the space asked for, created and opened.
    fn shell_welcome(&mut self, name: &str, folder: &str) -> Result<String, String> {
        if self.spaces.list.is_empty() {
            let created = self.create_space(name, folder).and_then(|id| self.switch_space(&id));

            if let Err(error) = created {
                if let Some(welcome) = self.welcome.as_mut() {
                    welcome.error = error.clone();
                }

                return Err(error);
            }

            self.prefs.folders.remember(folder);
        }

        self.welcome = None;
        self.prefs.welcome_seen = true;
        self.chat_open = self.prefs.start_in_chat;
        // then the tour, once (ui/tour.rs)
        self.tour_on_open(None);
        Ok(String::new())
    }
}
