//! The question before Claude Code's settings.json is written (Ajustes › Integrações): whether the app's hooks are already
//! there, and the file as it is and as it will be. Over Ajustes, with a veil, a line and a shadow (`dialog_over`).

use crate::app::maestro::claude_hooks::{HooksState, state_in};
use crate::prelude::*;

/// The question's title, its confirm, and what it says first: whether the hooks are there and what the button does.
pub(super) fn preview_words(state: HooksState, unchanged: bool) -> (&'static str, &'static str, &'static str) {
    match state {
        HooksState::Missing => (
            "Instalar hooks do Claude Code?",
            "Instalar",
            "Os hooks do MagoAgent ainda não estão instalados. Estas entradas serão acrescentadas ao settings.json; o arquivo atual será preservado e terá backup.",
        ),
        HooksState::Installed if unchanged => (
            "Reinstalar hooks do Claude Code?",
            "Reinstalar",
            "Os hooks do MagoAgent já estão instalados, iguais aos desta versão: nada muda no arquivo. Reinstalar só grava de novo, com backup.",
        ),
        HooksState::Installed => (
            "Reinstalar hooks do Claude Code?",
            "Reinstalar",
            "Os hooks do MagoAgent já estão instalados. Reinstalar troca as entradas do MagoAgent pelas desta versão; o resto do arquivo fica como está e terá backup.",
        ),
        HooksState::Outdated => (
            "Atualizar hooks do Claude Code?",
            "Atualizar",
            "Os hooks instalados são de outra versão do MagoAgent. Atualizar troca as entradas antigas pelas desta versão; o resto do arquivo fica como está e terá backup.",
        ),
    }
}

impl DrawingApp {
    pub(crate) fn claude_hooks_preview_ui(&mut self, ctx: &Context) {
        // the native Ajustes (Swift) asks its own question
        if super::native_on() {
            return;
        }
        let Some((path, relay, before, after)) = self.claude_hooks_preview.clone() else { return };
        let colors = self.colors();
        let (title, confirm_word, state_line) = preview_words(state_in(&before, &relay), before == after);
        let (confirm, cancel) = crate::app::ui::dialog_over(ctx, &colors, title, confirm_word, |ui| {
            let k = super::kit::K::new(self.dark());
            let line = super::kit::galley(ui, state_line, super::kit::BODY, super::kit::BODY_LINE, k.fg, ui.available_width());
            let (rect, _) = ui.allocate_exact_size(Vec2::new(ui.available_width(), line.size().y), Sense::hover());
            ui.painter().galley(rect.min, line, k.fg);
            ui.add_space(4.0);
            let file = super::kit::galley(ui, &path.display().to_string(), super::kit::SMALL, super::kit::SMALL_LINE, k.muted_fg, ui.available_width());
            let (rect, _) = ui.allocate_exact_size(Vec2::new(ui.available_width(), file.size().y), Sense::hover());
            ui.painter().galley(rect.min, file, k.muted_fg);
            ui.add_space(8.0);
            let before_text = serde_json::to_string_pretty(&before).unwrap_or_default();
            let after_text = serde_json::to_string_pretty(&after).unwrap_or_default();
            let rows = crate::editor::diff_rows(&before_text, &after_text);
            let lines_before: Vec<&str> = before_text.lines().collect();
            let lines_after: Vec<&str> = after_text.lines().collect();
            egui::ScrollArea::both().max_height(360.0).show(ui, |ui| {
                for row in rows {
                    if row.kind == crate::editor::RowKind::Change {
                        if let Some(index) = row.left {
                            let line = lines_before.get(index).copied().unwrap_or_default();
                            ui.label(egui::RichText::new(format!("− {line}")).monospace().color(Color32::from_rgb(210, 90, 90)));
                        }
                        if let Some(index) = row.right {
                            let line = lines_after.get(index).copied().unwrap_or_default();
                            ui.label(egui::RichText::new(format!("+ {line}")).monospace().color(Color32::from_rgb(100, 190, 130)));
                        }
                        continue;
                    }
                    let (prefix, line, color) = match (row.left, row.right) {
                        (Some(index), Some(_)) => (" ", lines_before.get(index).copied().unwrap_or_default(), ui.visuals().text_color()),
                        (Some(index), None) => ("−", lines_before.get(index).copied().unwrap_or_default(), Color32::from_rgb(210, 90, 90)),
                        (None, Some(index)) => ("+", lines_after.get(index).copied().unwrap_or_default(), Color32::from_rgb(100, 190, 130)),
                        _ => continue,
                    };
                    ui.label(egui::RichText::new(format!("{prefix} {line}")).monospace().color(color));
                }
            });
        });
        if cancel {
            self.claude_hooks_preview = None;
        } else if confirm {
            self.claude_hooks_confirm();
        }
    }

    /// The question's yes: the file written, unless it changed since the preview.
    pub(super) fn claude_hooks_confirm(&mut self) {
        let Some((path, relay, before, after)) = self.claude_hooks_preview.take() else { return };
        let result = (|| {
            let (current, _) = crate::app::maestro::claude_hooks::preview_at(&path, &relay)?;
            if current != before {
                return Err("settings.json mudou desde a prévia; confira de novo antes de instalar.".into());
            }
            let executable = std::env::current_exe().map_err(|error| error.to_string())?;
            crate::app::maestro::claude_hooks::ensure_relay(&executable, std::path::Path::new(&relay))?;
            crate::app::maestro::claude_hooks::install_preview_at(&path, &after)
        })();
        self.toast(result.unwrap_or_else(|error| error));
    }
}
