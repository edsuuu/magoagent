//! The editor's inspection summary, file problems popup and diagnostic navigation.

use super::*;

impl Ide {
    pub(super) fn navigate_diagnostic(&mut self, forward: bool) {
        let (Some(insp), Some(tab)) = (&self.insp, self.tabs.get(self.active).filter(|t| t.is_text())) else { return };
        let places = insp.for_file(&tab.path).into_iter().map(|d| (d.line, d.col)).filter(|&at| if forward { at > tab.cursor } else { at < tab.cursor });
        let next = if forward { places.min() } else { places.max() };
        if let Some((line, col)) = next {
            self.jump_to(tab.path.clone(), line, col);
        }
    }

    pub(super) fn inspection_widget(&mut self, ui: &mut Ui) {
        let (Some(insp), Some(tab)) = (&self.insp, self.tabs.get(self.active)) else { return };
        let mut diags = insp.for_file(&tab.path);
        diags.sort_by_key(|d| (d.line, d.col));
        let errors = diags.iter().filter(|d| d.severity == 1).count();
        let warnings = diags.iter().filter(|d| d.severity == 2).count();
        let text = match (errors, warnings) {
            (0, 0) => "✔".to_string(),
            (e, 0) => format!("✖ {e}"),
            (0, w) => format!("⚠ {w}"),
            (e, w) => format!("✖ {e}  ⚠ {w}"),
        };
        let color = if errors > 0 {
            t::ERROR
        } else if warnings > 0 {
            t::WARNING
        } else {
            Color32::from_rgb(0x4E, 0x9D, 0x6C)
        };
        let rect = ui.max_rect();
        let (mut direction, mut go, mut all) = (None, None, false);
        // Keep the widget above the text edit, including its hit testing.
        egui::Area::new(self.id.with("insp_widget"))
            .order(Order::Foreground)
            .pivot(Align2::RIGHT_TOP)
            .fixed_pos(pos2(rect.right() - 10.0, rect.top() + 44.0))
            .constrain_to(rect)
            .show(ui.ctx(), |ui| {
                theme::style(ui.style_mut());
                Frame::new().fill(t::ISLAND).corner_radius(4).inner_margin(2).show(ui, |ui| {
                    ui.horizontal(|ui| {
                        let resp = ui.small_button(egui::RichText::new(text).color(color)).on_hover_text("Problemas do arquivo");
                        t::menu(&resp).show(|ui| {
                            ui.set_width(360.0);
                            if diags.is_empty() {
                                ui.label("Nenhum problema encontrado");
                            }
                            egui::ScrollArea::vertical().max_height(180.0).show(ui, |ui| {
                                for d in &diags {
                                    let (icon, color) = match d.severity {
                                        1 => ("✖", t::ERROR),
                                        2 => ("⚠", t::WARNING),
                                        _ => ("ℹ", t::MUTED),
                                    };
                                    let label = format!("{icon} {}:{}  {}", d.line + 1, d.col + 1, d.message.lines().next().unwrap_or(""));
                                    let resp = ui.add(egui::Button::new(egui::RichText::new(label).color(color)).truncate());
                                    if resp.on_hover_text(&d.message).clicked() {
                                        go = Some((d.line, d.col));
                                        ui.close();
                                    }
                                }
                            });
                            ui.separator();
                            if ui.button("Mostrar todos os problemas").clicked() {
                                all = true;
                                ui.close();
                            }
                        });
                        let before = diags.iter().any(|d| (d.line, d.col) < tab.cursor);
                        let after = diags.iter().any(|d| (d.line, d.col) > tab.cursor);
                        if ui.add_enabled(before, egui::Button::new("↑").small()).on_hover_text("Problema anterior (Shift+F2)").clicked() {
                            direction = Some(false);
                        }
                        if ui.add_enabled(after, egui::Button::new("↓").small()).on_hover_text("Próximo problema (F2)").clicked() {
                            direction = Some(true);
                        }
                    });
                });
            });
        if let Some((line, col)) = go {
            self.jump_to(tab.path.clone(), line, col);
        }
        if let Some(forward) = direction {
            self.navigate_diagnostic(forward);
        }
        if all {
            self.bottom = Some(ToolWindow::Problems);
        }
    }
}
