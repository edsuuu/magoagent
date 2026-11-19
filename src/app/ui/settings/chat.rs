//! Ajustes › Chat: what a new chat starts with (agent, folder, leave) and the context panel. The style skill is in
//! Skills (`skills.rs`).

use super::kit::*;
use crate::prelude::*;

impl DrawingApp {
    pub(super) fn settings_chat(&mut self, ui: &mut egui::Ui, k: &K) {
        title(ui, k, "CHAT");

        note(
            ui,
            k,
            "Um chat novo já começa assim, sem perguntar; a permissão segue a do próprio agente quando ele define uma (o defaultMode do Claude Code). Modelo e esforço ficam nos chips sob a caixa do chat.",
        );

        ui.add_space(3.25);
        let agent = usize::from(self.prefs.chat_agent == "codex");

        if let Some(i) = row(ui, k, "Agente", "", |ui| select(ui, k, "Agente", &["Claude Code", "Codex"], agent)) {
            self.prefs.chat_agent = ["claude", "codex"][i].into();
        }

        self.folder_row(ui, k);
        let mode = self.prefs.chat_mode.min(crate::app::chat::direct::CHAT_MODES.len() - 1);

        if let Some(i) = row(ui, k, "O agente pode", "", |ui| select(ui, k, "O agente pode", &crate::app::chat::direct::CHAT_MODES, mode)) {
            self.prefs.chat_mode = i;
        }

        divider(ui, k);
        title(ui, k, "CONTEXTO");
        // "Padrão" is the model's own; the others are `claude --autocompact`
        let mut choices = vec!["Padrão do modelo"];

        choices.extend(crate::app::chat::context::COMPACT_CHOICES);
        let now = crate::app::chat::context::COMPACT_CHOICES.iter().position(|c| *c == self.prefs.chat_autocompact).map_or(0, |i| i + 1);
        let hint = "Com quantos tokens o Claude Code compacta a conversa sozinho. Vale para a próxima mensagem de cada chat.";

        if let Some(i) = row(ui, k, "Compactar automaticamente em", hint, |ui| select(ui, k, "Compactar automaticamente em", &choices, now)) {
            self.prefs.chat_autocompact = if i == 0 { String::new() } else { choices[i].into() };
        }

        toggle(ui, k, "Painel Contexto aberto", "O painel ao lado do chat com o uso da janela, as ferramentas e os MCPs.", &mut self.prefs.chat_panel);
    }

    /// "Pasta de trabalho": the path, Recentes (once there are folders to offer) and Procurar…
    fn folder_row(&mut self, ui: &mut egui::Ui, k: &K) {
        let c = UiColors::new(k.dark);
        let prefs = &mut self.prefs;

        row(ui, k, "Pasta de trabalho", "", |ui| {
            if button(ui, k, "Procurar…").clicked()
                && let Some(dir) = rfd::FileDialog::new().set_title("Pasta dos chats").pick_folder()
            {
                prefs.chat_folder = dir.to_string_lossy().into();
            }

            if !prefs.folders.is_empty() {
                let response = button(ui, k, "Recentes").tip("Pastas recentes e favoritas");

                // starring keeps the menu open; picking a folder closes it
                egui::Popup::menu(&response).frame(popup_frame(k)).close_behavior(egui::PopupCloseBehavior::CloseOnClickOutside).show(|ui| {
                    ui.set_width(320.0);
                    ui.spacing_mut().item_spacing.y = 1.0;

                    match folder_rows(ui, &prefs.folders, &c) {
                        Some(FolderPick::Use(path)) => {
                            prefs.chat_folder = path;
                            ui.close();
                        }
                        Some(FolderPick::Star(path)) => _ = prefs.folders.toggle_favorite(&path),
                        None => {}
                    }
                });
            }

            input(ui, k, egui::Id::unique("settings_chat_folder"), "Pasta de trabalho", &mut prefs.chat_folder, "~/.magoagent", 230.0, false);
        });
    }
}
