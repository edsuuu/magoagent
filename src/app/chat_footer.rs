//! Under the chat's box: the chat's folder, model, effort and permission, each a chip that
//! changes it for the next message.

use crate::app::chat_direct::{CHAT_MODES, Chat, folder_label};
use crate::app::chat_list::Look;
use crate::prelude::*;

// ponytail: fixed lists ("" = the agent's default); a text field when these go stale
pub(crate) const CLAUDE_MODELS: [&str; 4] = ["", "opus", "sonnet", "haiku"];
pub(crate) const CODEX_MODELS: [&str; 3] = ["", "gpt-5-codex", "gpt-5"];
pub(crate) const CLAUDE_EFFORTS: [&str; 5] = ["", "low", "medium", "high", "max"];
pub(crate) const CODEX_EFFORTS: [&str; 5] = ["", "low", "medium", "high", "xhigh"];

#[derive(Clone, Copy, Debug, Hash, PartialEq, Eq)]
pub(crate) enum FooterAct {
    Folder,
    Model,
    Effort,
    Mode,
}

pub(crate) fn models(agent: &str) -> &'static [&'static str] {
    if agent == "codex" { &CODEX_MODELS } else { &CLAUDE_MODELS }
}

pub(crate) fn efforts(agent: &str) -> &'static [&'static str] {
    if agent == "codex" { &CODEX_EFFORTS } else { &CLAUDE_EFFORTS }
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

fn next(list: &[&'static str], current: &str) -> &'static str {
    let i = list.iter().position(|m| *m == current).unwrap_or(0);
    list[(i + 1) % list.len()]
}

impl DrawingApp {
    /// The chips, left to right, on `row`; what was clicked is applied outside the closure
    /// (`chat_footer_act`).
    pub(crate) fn chat_footer(&self, ui: &mut egui::Ui, row: Rect, chat: &Chat, look: &Look) -> Option<FooterAct> {
        let fixed = chat.session.is_some();
        let folder_hint = if fixed { "A pasta fica fixa depois da primeira mensagem" } else { "Pasta de trabalho · clique para mudar" };
        let model = if chat.model.is_empty() { "Modelo padrão".to_string() } else { format!("Modelo {}", chat.model) };
        let effort = if chat.effort.is_empty() { "Esforço padrão".to_string() } else { format!("Esforço {}", chat.effort) };
        let mode = CHAT_MODES[chat.mode.min(CHAT_MODES.len() - 1)].to_string();
        let chips = [
            (FooterAct::Folder, format!("Pasta {}", folder_label(&chat.cwd)), folder_hint, !fixed),
            (FooterAct::Model, model, "Modelo do agente · clique para trocar", true),
            (FooterAct::Effort, effort, "Esforço do agente · clique para trocar", true),
            (FooterAct::Mode, mode, "O que o agente pode fazer · clique para trocar", true),
        ];
        let font = FontId::proportional(12.0);
        let mut x = row.left() + 10.0;
        let mut act = None;
        for (what, text, hint, enabled) in chips {
            let galley = ui.painter().layout_no_wrap(text, font.clone(), look.muted);
            let chip = Rect::from_min_size(Pos2::new(x, row.top()), Vec2::new(galley.size().x + 12.0, row.height()));
            // the paste hint keeps the right end of the row
            if chip.right() > row.right() - 230.0 {
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
            if response.clicked() {
                act = Some(what);
            }
            x = chip.right() + 4.0;
        }
        act
    }

    /// A chip clicked: the folder opens the picker; the others go to the next value.
    pub(crate) fn chat_footer_act(&mut self, act: FooterAct) {
        if act == FooterAct::Folder {
            if let Some(dir) = rfd::FileDialog::new().set_title("Pasta de trabalho").pick_folder() {
                self.chat_set_folder(&dir.to_string_lossy());
            }
            return;
        }
        let Some(chat) = self.current_chat_mut() else { return };
        match act {
            FooterAct::Model => chat.model = next(models(&chat.agent), &chat.model).into(),
            FooterAct::Effort => chat.effort = next(efforts(&chat.agent), &chat.effort).into(),
            FooterAct::Mode => chat.mode = (chat.mode + 1) % CHAT_MODES.len(),
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
        app.chat_footer_act(FooterAct::Model);
        app.chat_footer_act(FooterAct::Effort);
        app.chat_footer_act(FooterAct::Effort);
        app.chat_footer_act(FooterAct::Mode);
        let chat = app.chats[0].clone();
        assert_eq!((chat.model.as_str(), chat.effort.as_str(), chat.mode), ("opus", "medium", 1));
        assert_eq!(tuning_args("claude", &chat.model, &chat.effort), ["--model", "opus", "--effort", "medium"]);
        assert_eq!(tuning_args("codex", "gpt-5", "high"), ["-m", "gpt-5", "-c", "model_reasoning_effort=high"]);
        assert_eq!(tuning_args("claude", "", ""), Vec::<String>::new());
        // the lists wrap back to the default
        for _ in 0..3 {
            app.chat_footer_act(FooterAct::Model);
        }
        assert_eq!(app.chats[0].model, "");
        app.chat_set_folder("C:\\trabalho\\loja");
        assert_eq!((app.chats[0].cwd.as_str(), app.chats[0].name.as_str()), ("C:\\trabalho\\loja", "Claude Code · loja"));
        app.chats[0].session = Some("s1".into());
        app.chat_set_folder("~/outra");
        assert_eq!(app.chats[0].cwd, "C:\\trabalho\\loja", "fixed once the session exists");
    }
}
