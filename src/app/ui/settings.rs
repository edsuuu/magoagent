//! Ajustes (⌘,), after the reference app's: for now the terminal's quick starts, the presets the
//! new terminal dialog offers. Each can be switched off, edited or removed; up to 10 show.

use crate::prelude::*;

impl DrawingApp {
    pub(crate) fn settings_ui(&mut self, ctx: &Context) {
        let c = self.colors();
        let mut close = ctx.input(|i| i.key_pressed(Key::Escape));
        let (mut remove, mut add, mut reset) = (None, false, false);
        egui::Area::new(egui::Id::unique("settings")).order(egui::Order::Foreground).anchor(egui::Align2::CENTER_CENTER, Vec2::ZERO).show(ctx, |ui| {
            egui::Frame::new()
                .fill(c.island)
                .corner_radius(14)
                .inner_margin(egui::Margin::symmetric(22, 18))
                .stroke(Stroke::new(1.0, c.surface_high))
                .shadow(egui::epaint::Shadow { offset: [0, 12], blur: 36, spread: 0, color: Color32::from_black_alpha(90) })
                .show(ui, |ui| {
                    ui.set_width(DIALOG_WIDTH + 40.0);
                    ui.horizontal(|ui| {
                        ui.label(egui::RichText::new("Ajustes · Terminal").size(15.0).strong().color(c.text));
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            let x = square_button(ui, 26.0, false, Color32::TRANSPARENT, &c);
                            for (a, b) in [(Vec2::new(-4.5, -4.5), Vec2::new(4.5, 4.5)), (Vec2::new(-4.5, 4.5), Vec2::new(4.5, -4.5))] {
                                ui.painter().line_segment([x.rect.center() + a, x.rect.center() + b], Stroke::new(1.5, c.muted));
                            }
                            close |= x.on_hover_text("Fechar (Esc)").clicked();
                        });
                    });
                    ui.add_space(10.0);
                    ui.label(egui::RichText::new("PRESETS DE INÍCIO RÁPIDO").size(11.0).strong().color(c.muted));
                    ui.label(egui::RichText::new("Até 10 presets ativos aparecem ao criar um terminal.").size(12.0).color(c.muted));
                    ui.add_space(6.0);
                    group_frame(&c).inner_margin(6).show(ui, |ui| {
                        ui.set_width(ui.available_width());
                        for i in 0..self.prefs.presets.len() {
                            let editing = self.settings_edit == Some(i);
                            ui.horizontal(|ui| {
                                ui.set_min_height(38.0);
                                let preset = &mut self.prefs.presets[i];
                                // the check: a filled circle when it shows in the dialog
                                let (dot, check) = ui.allocate_exact_size(Vec2::splat(22.0), Sense::click());
                                if preset.on {
                                    ui.painter().circle_filled(dot.center(), 8.0, PRIMARY);
                                    paint_icon(ui.painter(), Icon::Check, dot.center(), Color32::WHITE);
                                } else {
                                    ui.painter().circle_stroke(dot.center(), 7.5, Stroke::new(1.2, c.muted));
                                }
                                if check.on_hover_text(if preset.on { "Esconder do diálogo" } else { "Mostrar no diálogo" }).clicked() {
                                    preset.on = !preset.on;
                                }
                                let (mark, _) = ui.allocate_exact_size(Vec2::splat(24.0), Sense::hover());
                                terminal_icon(ui.painter(), &TerminalSpec { command: preset.command.clone(), ..Default::default() }, mark.center(), c.text);
                                if editing {
                                    for (text, hint, width) in [(&mut preset.name, "Nome", 120.0), (&mut preset.command, "comando (vazio: shell)", 170.0)] {
                                        ui.add(egui::TextEdit::singleline(text).hint_text(hint).desired_width(width).margin(egui::Margin::symmetric(8, 5)).background_color(c.island));
                                    }
                                } else {
                                    ui.vertical(|ui| {
                                        ui.spacing_mut().item_spacing.y = 0.0;
                                        ui.label(egui::RichText::new(&preset.name).size(13.5).color(c.text));
                                        let command = if preset.command.is_empty() { "shell" } else { &preset.command };
                                        ui.label(egui::RichText::new(command).size(11.5).monospace().color(c.muted));
                                    });
                                }
                                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                    if icon_button(ui, 26.0, false, Color32::TRANSPARENT, Icon::Minus, c.muted, &c).on_hover_text("Remover").clicked() {
                                        remove = Some(i);
                                    }
                                    let (icon, tip) = if editing { (Icon::Check, "Pronto") } else { (Icon::Edit, "Editar") };
                                    if icon_button(ui, 26.0, editing, Color32::TRANSPARENT, icon, c.muted, &c).on_hover_text(tip).clicked() {
                                        self.settings_edit = if editing { None } else { Some(i) };
                                    }
                                });
                            });
                        }
                    });
                    ui.add_space(10.0);
                    ui.horizontal(|ui| {
                        let button = |text: &str| egui::Button::new(egui::RichText::new(text).size(12.5).color(c.text)).fill(c.surface_low).corner_radius(8).min_size(Vec2::new(0.0, 28.0));
                        add = ui.add(button("+  Adicionar preset")).clicked();
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            reset = ui.add(button("Redefinir para padrão")).clicked();
                        });
                    });
                });
        });
        if let Some(i) = remove {
            self.prefs.presets.remove(i);
            self.settings_edit = None;
        }
        if add {
            self.prefs.presets.push(Preset { name: "Novo preset".into(), command: String::new(), on: true });
            self.settings_edit = Some(self.prefs.presets.len() - 1);
        }
        if reset {
            self.prefs.presets = default_presets();
            self.settings_edit = None;
        }
        if close {
            self.settings_open = false;
            self.settings_edit = None;
        }
    }
}
