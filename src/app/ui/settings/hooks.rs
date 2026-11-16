//! Ajustes › Integrações, first: the hooks that bring the sessions of Claude Code and Codex outside the app to the island
//! (`maestro/claude_hooks.rs`, `maestro/codex_hooks.rs`). Nothing is written until a button asks for it.

use super::kit::*;
use crate::prelude::*;

impl DrawingApp {
    /// The button that puts the app's hooks in the person's own Claude Code settings, and the one that takes them out.
    pub(super) fn settings_hooks(&mut self, ui: &mut egui::Ui, k: &K) {
        title(ui, k, "CLAUDE CODE FORA DO APP");
        note(
            ui,
            k,
            "A ilha mostra as perguntas e os pedidos de permissão de qualquer sessão do Claude Code (o Claude Desktop, o VS Code, outro terminal) e você responde por ela. Instalar acrescenta os hooks do MagoAgent ao seu settings.json, com backup antes e sem apagar nada; Remover tira só os do MagoAgent. Vale para as sessões que começarem depois.",
        );
        if !self.prefs.island {
            let hint = "A ilha está desligada: sem ela, nada disso aparece. É a mesma chave de Ajustes › Geral.";
            toggle(ui, k, "Mostrar a ilha", hint, &mut self.prefs.island);
        }
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
        if action == "remove" {
            let message = self.act_claude_hooks(action).unwrap_or_else(|why| why);
            self.toast(message);
        } else if action == "install" {
            let preview = (|| {
                let path = settings_path().ok_or("Não achei a pasta do Claude Code (.claude).")?;
                let relay = relay_path().ok_or("Não achei a pasta de dados do MagoAgent para o relay dos hooks.")?;
                let (before, after) = crate::app::maestro::claude_hooks::preview_at(&path, &relay.to_string_lossy())?;
                Ok::<_, String>((path, relay.to_string_lossy().into_owned(), before, after))
            })();
            match preview {
                Ok(preview) => self.claude_hooks_preview = Some(preview),
                Err(error) => self.toast(error),
            }
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
}
