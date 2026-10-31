//! Under the chat's box: the chat's folder, model, effort and permission, each a chip that
//! changes it for the next message. The model's chip opens a menu like the desktop app's: the
//! current models with their number keys, Esforço, and Mais modelos for the older ones.

use crate::app::chat_direct::{CHAT_MODES, Chat, folder_label};
use crate::app::chat_list::Look;
use crate::prelude::*;

/// (what the agent is told, what the menu shows). "" is the agent's own default.
pub(crate) type Choice = (&'static str, &'static str);

// ponytail: fixed lists; a text field when these go stale
pub(crate) const CLAUDE_MODELS: [Choice; 5] = [
    ("", "Padrão"),
    ("claude-opus-5-5", "Opus 5.5"),
    ("claude-fable-5-1", "Fable 5.1"),
    ("claude-sonnet-5-5", "Sonnet 5.5"),
    ("claude-haiku-5-5", "Haiku 5.5"),
];
pub(crate) const CLAUDE_MORE: [Choice; 8] = [
    ("claude-haiku-4-5", "Haiku 4.5"),
    ("claude-sonnet-5", "Sonnet 5"),
    ("claude-opus-5", "Opus 5"),
    ("claude-fable-5", "Fable 5"),
    ("claude-opus-4-8", "Opus 4.8"),
    ("claude-opus-4-7", "Opus 4.7"),
    ("claude-opus-4-6", "Opus 4.6"),
    ("claude-sonnet-4-6", "Sonnet 4.6"),
];
pub(crate) const CODEX_MODELS: [Choice; 4] = [("", "Padrão"), ("gpt-5-codex", "GPT-5 Codex"), ("gpt-5", "GPT-5"), ("gpt-5-mini", "GPT-5 mini")];
pub(crate) const CLAUDE_EFFORTS: [Choice; 6] = [("", "Padrão"), ("low", "Baixo"), ("medium", "Médio"), ("high", "Alto"), ("xhigh", "Extra"), ("max", "Máx")];
pub(crate) const CODEX_EFFORTS: [Choice; 5] = [("", "Padrão"), ("low", "Baixo"), ("medium", "Médio"), ("high", "Alto"), ("xhigh", "Extra")];

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum FooterAct {
    Folder,
    Model(String),
    Effort(String),
    Mode(usize),
}

pub(crate) fn models(agent: &str) -> &'static [Choice] {
    if agent == "codex" { &CODEX_MODELS } else { &CLAUDE_MODELS }
}

pub(crate) fn more_models(agent: &str) -> &'static [Choice] {
    if agent == "codex" { &[] } else { &CLAUDE_MORE }
}

pub(crate) fn efforts(agent: &str) -> &'static [Choice] {
    if agent == "codex" { &CODEX_EFFORTS } else { &CLAUDE_EFFORTS }
}

/// What the menu calls `value`; the id itself when it isn't on the lists.
pub(crate) fn label(list: &[&[Choice]], value: &str) -> String {
    list.iter().flat_map(|l| l.iter()).find(|(id, _)| *id == value).map_or_else(|| value.to_string(), |(_, name)| name.to_string())
}

/// The agent's flags for a model and an effort; none when they are its defaults.
pub(crate) fn tuning_args(agent: &str, model: &str, effort: &str) -> Vec<String> {
    let mut args = Vec::new();
    if !model.is_empty() {
        args.extend([if agent == "codex" { "-m" } else { "--model" }.to_string(), model.to_string()]);
    }
    if !effort.is_empty() {
        if agent == "codex" {
            args.extend(["-c".to_string(), format!("model_reasoning_effort={effort}")]);
        } else {
            args.extend(["--effort".to_string(), effort.to_string()]);
        }
    }
    args
}

/// A row of the menu: the name, a check when it is the one, a key or a note at the right.
fn row(ui: &mut egui::Ui, name: &str, on: bool, right: &str, c: &UiColors) -> bool {
    menu_item(ui, if on { Icon::Check } else { Icon::Blank }, name, right, c.text, c)
}

impl DrawingApp {
    /// The chips, left to right, on `row`; what was picked is applied outside the closure
    /// (`chat_footer_act`).
    pub(crate) fn chat_footer(&self, ui: &mut egui::Ui, row_rect: Rect, chat: &Chat, look: &Look) -> Option<FooterAct> {
        let c = self.colors();
        let fixed = chat.session.is_some();
        let folder_hint = if fixed { "A pasta fica fixa depois da primeira mensagem" } else { "Pasta de trabalho · clique para mudar" };
        let model = label(&[models(&chat.agent), more_models(&chat.agent)], &chat.model);
        let model = if chat.model.is_empty() { "Modelo padrão".to_string() } else { model };
        let effort = if chat.effort.is_empty() { "Esforço padrão".to_string() } else { format!("Esforço {}", label(&[efforts(&chat.agent)], &chat.effort)) };
        let mode = CHAT_MODES[chat.mode.min(CHAT_MODES.len() - 1)].to_string();
        let chips = [
            ("folder", format!("Pasta {}", folder_label(&chat.cwd)), folder_hint, !fixed),
            ("model", model, "Modelo do agente", true),
            ("effort", effort, "Esforço do agente", true),
            ("mode", mode, "O que o agente pode fazer", true),
        ];
        let font = FontId::proportional(12.0);
        let mut x = row_rect.left() + 10.0;
        let mut act = None;
        for (what, text, hint, enabled) in chips {
            let galley = ui.painter().layout_no_wrap(text, font.clone(), look.muted);
            let chip = Rect::from_min_size(Pos2::new(x, row_rect.top()), Vec2::new(galley.size().x + 12.0, row_rect.height()));
            // the paste hint keeps the right end of the row
            if chip.right() > row_rect.right() - 230.0 {
                break;
            }
            let sense = if enabled { Sense::click() } else { Sense::hover() };
            let response = ui.interact(chip, ui.scope_id().with(("chat_footer", what)), sense).on_hover_text(hint);
            if enabled && response.hovered() {
                ui.painter().rect_filled(chip, 8.0, look.raised);
                response.clone().on_hover_cursor(CursorIcon::PointingHand);
            }
            let color = if enabled { look.soft } else { look.muted };
            ui.painter().galley(Pos2::new(chip.left() + 6.0, chip.center().y - galley.size().y / 2.0), galley, color);
            x = chip.right() + 4.0;
            if what == "folder" {
                if response.clicked() {
                    act = Some(FooterAct::Folder);
                }
                continue;
            }
            // the others open a menu over the chip
            egui::Popup::menu(&response).show(|ui| {
                ui.set_min_width(190.0);
                ui.spacing_mut().item_spacing.y = 1.0;
                match what {
                    "model" => {
                        for (k, (id, name)) in models(&chat.agent).iter().enumerate() {
                            let key = if id.is_empty() { String::new() } else { k.to_string() };
                            if row(ui, name, chat.model == *id, &key, &c) {
                                act = Some(FooterAct::Model(id.to_string()));
                                ui.close();
                            }
                        }
                        menu_line(ui, &c);
                        let current = label(&[efforts(&chat.agent)], &chat.effort);
                        ui.menu_button(format!("Esforço   {current}"), |ui| {
                            ui.set_min_width(170.0);
                            for (id, name) in efforts(&chat.agent) {
                                let note = if *id == "high" { "Recomendado" } else { "" };
                                if row(ui, name, chat.effort == *id, note, &c) {
                                    act = Some(FooterAct::Effort(id.to_string()));
                                    ui.close();
                                }
                            }
                        });
                        if !more_models(&chat.agent).is_empty() {
                            ui.menu_button("Mais modelos", |ui| {
                                ui.set_min_width(150.0);
                                for (id, name) in more_models(&chat.agent) {
                                    if row(ui, name, chat.model == *id, "", &c) {
                                        act = Some(FooterAct::Model(id.to_string()));
                                        ui.close();
                                    }
                                }
                            });
                        }
                    }
                    "effort" => {
                        for (id, name) in efforts(&chat.agent) {
                            let note = if *id == "high" { "Recomendado" } else { "" };
                            if row(ui, name, chat.effort == *id, note, &c) {
                                act = Some(FooterAct::Effort(id.to_string()));
                                ui.close();
                            }
                        }
                    }
                    _ => {
                        for (k, name) in CHAT_MODES.iter().enumerate() {
                            if row(ui, name, chat.mode == k, "", &c) {
                                act = Some(FooterAct::Mode(k));
                                ui.close();
                            }
                        }
                    }
                }
            });
        }
        act
    }

    /// A chip's pick: the folder opens the picker; the others set the chat's next message.
    pub(crate) fn chat_footer_act(&mut self, act: FooterAct) {
        if act == FooterAct::Folder {
            if let Some(dir) = rfd::FileDialog::new().set_title("Pasta de trabalho").pick_folder() {
                self.chat_set_folder(&dir.to_string_lossy());
            }
            return;
        }
        let Some(chat) = self.current_chat_mut() else { return };
        match act {
            FooterAct::Model(model) => chat.model = model,
            FooterAct::Effort(effort) => chat.effort = effort,
            FooterAct::Mode(mode) => chat.mode = mode.min(CHAT_MODES.len() - 1),
            FooterAct::Folder => {}
        }
        self.save_chats();
    }

    /// The chat's folder, while no message went (the agent's session lives in the folder).
    pub(crate) fn chat_set_folder(&mut self, dir: &str) {
        let Some(chat) = self.current_chat_mut() else { return };
        if chat.session.is_some() {
            return;
        }
        chat.cwd = dir.trim().to_string();
        let agent = chat.name.split(" · ").next().unwrap_or(&chat.agent).to_string();
        chat.name = format!("{agent} · {}", folder_label(&chat.cwd));
        self.save_chats();
    }

    fn current_chat_mut(&mut self) -> Option<&mut Chat> {
        let id = self.chat_agent.clone()?;
        self.chats.iter_mut().find(|c| c.id == id)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::chat_direct::NewChat;

    #[test]
    fn the_chips_change_the_next_message_and_the_folder_locks_after_the_first() {
        let mut app = DrawingApp::default();
        let installed = vec![Preset { name: "Claude Code".into(), command: "claude".into(), on: true }];
        app.create_chat(&NewChat { agent: 0, cwd: "~/projeto".into(), mode: 0 }, &installed);
        app.chat_footer_act(FooterAct::Model("claude-opus-5-5".into()));
        app.chat_footer_act(FooterAct::Effort("medium".into()));
        app.chat_footer_act(FooterAct::Mode(1));
        let chat = app.chats[0].clone();
        assert_eq!((chat.model.as_str(), chat.effort.as_str(), chat.mode), ("claude-opus-5-5", "medium", 1));
        assert_eq!(tuning_args("claude", &chat.model, &chat.effort), ["--model", "claude-opus-5-5", "--effort", "medium"]);
        assert_eq!(tuning_args("codex", "gpt-5", "high"), ["-m", "gpt-5", "-c", "model_reasoning_effort=high"]);
        assert_eq!(tuning_args("claude", "", ""), Vec::<String>::new());
        // the menu names them; an id off the lists shows as it is
        assert_eq!(label(&[models("claude"), more_models("claude")], "claude-opus-4-7"), "Opus 4.7");
        assert_eq!(label(&[models("claude")], "claude-x"), "claude-x");
        app.chat_footer_act(FooterAct::Model(String::new()));
        assert_eq!(app.chats[0].model, "");
        app.chat_set_folder("C:\\trabalho\\loja");
        assert_eq!((app.chats[0].cwd.as_str(), app.chats[0].name.as_str()), ("C:\\trabalho\\loja", "Claude Code · loja"));
        app.chats[0].session = Some("s1".into());
        app.chat_set_folder("~/outra");
        assert_eq!(app.chats[0].cwd, "C:\\trabalho\\loja", "fixed once the session exists");
    }
}
