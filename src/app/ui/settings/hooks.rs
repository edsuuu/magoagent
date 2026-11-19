//! Ajustes › Integrações, first: the hooks that bring the sessions of Claude Code and Codex outside the app to the island
//! (`maestro/claude_hooks.rs`, `maestro/codex_hooks.rs`). Each says whether they are installed and offers what fits:
//! Instalar when they are not, Reinstalar (or Atualizar, of another version) and Remover when they are. Nothing is
//! written until a button asks for it, and Claude Code's file is shown before (`hooks_preview.rs`).

use super::kit::*;
use crate::app::maestro::claude_hooks::{self, HooksState};
use crate::prelude::*;

/// The row of a hooks file: its state in a word, and the buttons right to left (the main one at the right edge). The
/// Claude Code ones end in "…": they show the file before writing it.
pub(super) fn hooks_row(state: HooksState, preview: bool) -> (&'static str, &'static [&'static str]) {
    match (state, preview) {
        (HooksState::Missing, true) => ("Não instalados", &["Instalar…"]),
        (HooksState::Missing, false) => ("Não instalados", &["Instalar"]),
        (HooksState::Installed, true) => ("Instalados", &["Reinstalar…", "Remover"]),
        (HooksState::Installed, false) => ("Instalados", &["Reinstalar", "Remover"]),
        (HooksState::Outdated, true) => ("Instalados, de outra versão", &["Atualizar…", "Remover"]),
        (HooksState::Outdated, false) => ("Instalados, de outra versão", &["Atualizar", "Remover"]),
    }
}

/// The row's line under the state: where the file is and what follows from it.
pub(super) fn hooks_hint(state: HooksState, path: Option<&std::path::Path>) -> String {
    let Some(path) = path else { return "Não achei a pasta de configuração.".into() };
    let path = path.display();

    match state {
        HooksState::Missing => format!("Nada do MagoAgent em {path}."),
        HooksState::Installed => format!("Em {path}: a ilha recebe as sessões que começarem depois."),
        HooksState::Outdated => format!("Em {path}, mas de outra versão do MagoAgent: atualize para a ilha receber tudo."),
    }
}

/// The state row with its buttons; the one pressed, if any.
fn hooks_buttons(ui: &mut egui::Ui, k: &K, state: HooksState, path: Option<&std::path::Path>, preview: bool) -> Option<&'static str> {
    let (label, buttons) = hooks_row(state, preview);
    let mut pressed = None;

    row(ui, k, label, &hooks_hint(state, path), |ui| {
        for name in buttons {
            if button(ui, k, name).clicked() {
                pressed = Some(*name);
            }
        }
    });

    pressed
}

impl DrawingApp {
    /// The question before Claude Code's settings.json is written: the file as it is and as it will be.
    pub(super) fn claude_hooks_open_preview(&mut self) {
        let preview = (|| {
            let path = claude_hooks::settings_path().ok_or("Não achei a pasta do Claude Code (.claude).")?;
            let relay = claude_hooks::relay_path().ok_or("Não achei a pasta de dados do MagoAgent para o relay dos hooks.")?;
            let (before, after) = claude_hooks::preview_at(&path, &relay.to_string_lossy())?;

            Ok::<_, String>((path, relay.to_string_lossy().into_owned(), before, after))
        })();

        match preview {
            Ok(preview) => self.claude_hooks_preview = Some(preview),
            Err(error) => self.toast(error),
        }
    }

    /// The app's hooks in the person's own Claude Code settings and Codex hooks file: what is there, and the buttons.
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

        let (path, relay) = (claude_hooks::settings_path(), claude_hooks::relay_path());

        let state = match (&path, &relay) {
            (Some(path), Some(relay)) => claude_hooks::state_at(path, &relay.to_string_lossy()),
            _ => HooksState::Missing,
        };

        match hooks_buttons(ui, k, state, path.as_deref(), true) {
            Some("Remover") => {
                let message = self.act_claude_hooks("remove").unwrap_or_else(|why| why);

                self.toast(message);
            }
            // Instalar, Reinstalar or Atualizar: the file as it is and as it will be, before anything is written
            Some(_) => self.claude_hooks_open_preview(),
            None => {}
        }

        title(ui, k, "CODEX FORA DO APP");

        note(
            ui,
            k,
            "Instalar acrescenta os hooks do MagoAgent ao hooks.json do Codex, com backup antes e sem apagar nada; Remover tira só os do MagoAgent. Vale para as sessões novas.",
        );

        let path = crate::app::maestro::codex_hooks::codex_settings_path();
        let installed = path.as_ref().is_some_and(|path| crate::app::maestro::codex_hooks::installed_at(path));
        let state = if installed { HooksState::Installed } else { HooksState::Missing };

        match hooks_buttons(ui, k, state, path.as_deref(), false) {
            Some("Remover") => {
                let message = self.act_codex_hooks("remove").unwrap_or_else(|why| why);

                self.toast(message);
            }
            // installing again replaces the app's entries with this version's, as the first time
            Some(_) => {
                let message = self.act_codex_hooks("install").unwrap_or_else(|why| why);

                self.toast(message);
            }
            None => {}
        }
    }
}
