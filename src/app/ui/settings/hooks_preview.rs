use crate::prelude::*;

impl DrawingApp {
    pub(crate) fn claude_hooks_preview_ui(&mut self, ctx: &Context) {
        let Some((path, relay, before, after)) = self.claude_hooks_preview.clone() else { return };
        let colors = self.colors();
        let (confirm, cancel) = crate::app::ui::dialog_ui(ctx, &colors, "Instalar hooks do Claude Code?", "Instalar", |ui| {
            ui.label("Estas entradas serão acrescentadas ao settings.json. O arquivo atual será preservado e terá backup.");
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
            self.claude_hooks_preview = None;
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
}
