//! Ajustes › Agentes: the chat's defaults and what each agent uses with the Maestro's limits (Uso).

use super::kit::*;
use crate::prelude::*;

/// The sub-tabs of Ajustes › Agentes.
pub(crate) const AGENT_TABS: [&str; 3] = ["Chat", "Uso", "Hooks"];

impl DrawingApp {
    pub(super) fn settings_agents(&mut self, ui: &mut egui::Ui, k: &K) {
        match super::subtabs(ui, k, "agents", &AGENT_TABS) {
            0 => self.settings_chat(ui, k),
            1 => self.settings_usage(ui, k),
            _ => self.settings_hooks(ui, k),
        }
    }

    /// The button that puts the app's hooks in the person's own Claude Code settings, and the one that takes them out.
    fn settings_hooks(&mut self, ui: &mut egui::Ui, k: &K) {
        title(ui, k, "CLAUDE CODE FORA DO APP");
        note(
            ui,
            k,
            "A ilha mostra as perguntas e os pedidos de permissão de qualquer sessão do Claude Code (o Claude Desktop, o VS Code, outro terminal) e você responde por ela. Instalar acrescenta os hooks do MagoAgent ao seu settings.json, com backup antes e sem apagar nada; Remover tira só os do MagoAgent. Vale para as sessões que começarem depois.",
        );
        ui.add_space(3.25);
        use crate::app::maestro::claude_hooks::{installed_at, outdated_at, relay_path, settings_path};
        let path = settings_path();
        let installed = path.as_ref().is_some_and(|path| installed_at(path));
        // instalados por outra versão (apontam para um exe que mudou, ou falta um evento): só o botão conserta
        let outdated = installed && path.zip(relay_path()).is_some_and(|(path, relay)| outdated_at(&path, &relay.to_string_lossy()));
        let state = match (installed, outdated) {
            (_, true) => "Hooks desatualizados: instale de novo",
            (true, _) => "Hooks instalados",
            _ => "Hooks não instalados",
        };
        let mut action = "";
        row(ui, k, state, "", |ui| {
            if button(ui, k, "Remover").clicked() {
                action = "remove";
            }
            if button(ui, k, "Instalar hooks do Claude Code").clicked() {
                action = "install";
            }
        });
        if !action.is_empty() {
            let message = self.act_claude_hooks(action).unwrap_or_else(|why| why);
            self.toast(message);
        }
        ui.add_space(12.0);
        title(ui, k, "CODEX FORA DO APP");
        note(
            ui,
            k,
            "Instalar acrescenta os hooks do MagoAgent ao hooks.json do Codex, com backup antes e sem apagar nada; Remover tira só os do MagoAgent. Vale para as sessões novas.",
        );
        let path = crate::app::maestro::codex_hooks::codex_settings_path();
        let installed = path.as_ref().is_some_and(|path| crate::app::maestro::codex_hooks::installed_at(path));
        let state = if installed { "Hooks do Codex instalados" } else { "Hooks do Codex não instalados" };
        let mut codex_action = "";
        row(ui, k, state, "", |ui| {
            if button(ui, k, "Remover").clicked() {
                codex_action = "remove";
            }
            if button(ui, k, "Instalar hooks do Codex").clicked() {
                codex_action = "install";
            }
        });
        if !codex_action.is_empty() {
            let message = self.act_codex_hooks(codex_action).unwrap_or_else(|why| why);
            self.toast(message);
        }
    }

    fn settings_chat(&mut self, ui: &mut egui::Ui, k: &K) {
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
        self.style_skill_row(ui, k);
    }

    /// "Skill de estilo": the file the flow's Padronização step reads, and Abrir to show it.
    fn style_skill_row(&mut self, ui: &mut egui::Ui, k: &K) {
        let path = &mut self.prefs.style_skill;
        let hint = "O SKILL.md com o seu estilo de código: a etapa Padronização do fluxo do Maestro manda o agente lê-lo e conferir o diff regra a regra.";
        row(ui, k, "Skill de estilo", hint, |ui| {
            if button(ui, k, "Abrir").clicked() && std::path::Path::new(path.as_str()).is_file() {
                crate::app::browser::open_external(path);
            }
            input(ui, k, egui::Id::unique("settings_style_skill"), "Skill de estilo", path, "~/Skill/SKILL.md", 230.0, false);
        });
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

    fn settings_usage(&mut self, ui: &mut egui::Ui, k: &K) {
        let agents = self.agent_usage_lines();
        if !agents.is_empty() {
            title(ui, k, "AGENTES");
            note(ui, k, "O que cada agente que já respondeu está usando.");
            ui.add_space(3.25);
            for (name, line) in agents {
                text_block(ui, &name, BODY, k.fg);
                text_block(ui, &line, SMALL, k.muted_fg);
                ui.add_space(6.5);
            }
            divider(ui, k);
        }
        title(ui, k, "LIMITES DO MAESTRO");
        note(ui, k, "O que segura o Maestro ao recrutar agentes.");
        ui.add_space(3.25);
        let mode = usize::from(self.prefs.agent_mode == AgentMode::Chat);
        let hint = "Terminal: Claude Code completo em um terminal. Chat: conversa leve pelo chat do app, usando o login do CLI; gasta bem menos memória.";
        if let Some(i) = row(ui, k, "Invocação dos agentes", hint, |ui| select(ui, k, "Invocação dos agentes", &["Terminal", "Chat"], mode)) {
            self.prefs.agent_mode = [AgentMode::Terminal, AgentMode::Chat][i];
        }
        let stop = [70, 80, 90].iter().position(|n| *n == self.prefs.usage_stop).unwrap_or(3);
        let hint = "Com a janela de 5 horas do plano nesta porcentagem, o Maestro para de recrutar.";
        if let Some(i) = row(ui, k, "Parar de recrutar em", hint, |ui| select(ui, k, "Parar de recrutar em", &["70%", "80%", "90%", "Nunca"], stop)) {
            self.prefs.usage_stop = [70, 80, 90, 0][i];
        }
    }
}

/// A paragraph at `size`, wrapped to the column.
fn text_block(ui: &mut egui::Ui, text: &str, size: f32, color: Color32) {
    let line = if size == BODY { BODY_LINE } else { SMALL_LINE };
    let g = galley(ui, text, size, line, color, ui.available_width());
    let (rect, _) = ui.allocate_exact_size(g.size().max(Vec2::new(ui.available_width(), 0.0)), Sense::hover());
    ui.painter().galley(rect.min, g, color);
}
