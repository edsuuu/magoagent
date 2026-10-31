//! Ajustes (⌘,), after the reference app's: a modal with the sections on the left (Geral,
//! Terminal, Chat, API, Avatar, Sobre) and the section's options on the right.

use crate::prelude::*;

pub(crate) const SETTINGS_TABS: [&str; 6] = ["Geral", "Terminal", "Chat", "API", "Avatar", "Sobre"];
pub(crate) const LANGUAGES: [&str; 3] = ["Automático", "Português", "English"];

/// The chat's agent run on an API key instead of its own login: the variable and the key, when
/// the person turned that on and typed one.
pub(crate) fn api_env(prefs: &Prefs, agent: &str) -> Option<(&'static str, String)> {
    if !prefs.use_api_keys {
        return None;
    }
    let (name, key) = if agent == "codex" { ("OPENAI_API_KEY", &prefs.openai_key) } else { ("ANTHROPIC_API_KEY", &prefs.anthropic_key) };
    let key = key.trim();
    (!key.is_empty()).then(|| (name, key.to_string()))
}

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
                    ui.set_width(DIALOG_WIDTH + 190.0);
                    ui.horizontal(|ui| {
                        ui.label(egui::RichText::new("Ajustes").size(15.0).strong().color(c.text));
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            let x = square_button(ui, 26.0, false, Color32::TRANSPARENT, &c);
                            for (a, b) in [(Vec2::new(-4.5, -4.5), Vec2::new(4.5, 4.5)), (Vec2::new(-4.5, 4.5), Vec2::new(4.5, -4.5))] {
                                ui.painter().line_segment([x.rect.center() + a, x.rect.center() + b], Stroke::new(1.5, c.muted));
                            }
                            close |= x.on_hover_text("Fechar (Esc)").clicked();
                        });
                    });
                    ui.add_space(10.0);
                    ui.horizontal_top(|ui| {
                        // the sections, down the left
                        ui.vertical(|ui| {
                            ui.set_width(130.0);
                            for (i, name) in SETTINGS_TABS.iter().enumerate() {
                                let (rect, response) = ui.allocate_exact_size(Vec2::new(130.0, 30.0), Sense::click());
                                let chosen = self.settings_tab == i;
                                if chosen || response.hovered() {
                                    ui.painter().rect_filled(rect, 8, if chosen { c.selected } else { c.surface_low });
                                }
                                let color = if chosen { c.text } else { c.muted };
                                ui.painter().text(
                                    rect.left_center() + Vec2::new(12.0, 0.0),
                                    egui::Align2::LEFT_CENTER,
                                    name,
                                    FontId::proportional(13.0),
                                    color,
                                );
                                if response.on_hover_cursor(CursorIcon::PointingHand).clicked() {
                                    self.settings_tab = i;
                                    self.settings_edit = None;
                                }
                            }
                        });
                        ui.add_space(16.0);
                        egui::ScrollArea::vertical().max_height(440.0).show(ui, |ui| {
                            ui.set_width(DIALOG_WIDTH);
                            match self.settings_tab {
                                0 => self.settings_general(ui, &c),
                                1 => (remove, add, reset) = self.settings_terminal(ui, &c),
                                2 => self.settings_chat(ui, &c),
                                3 => self.settings_api(ui, &c),
                                4 => self.settings_avatar(ui, &c),
                                _ => settings_about(ui, &c),
                            }
                        });
                    });
                });
        });
        if let Some(i) = remove {
            self.prefs.presets.remove(i);
            self.settings_edit = None;
        }
        if add {
            self.prefs.presets.push(Preset { name: String::new(), command: String::new(), on: true });
            self.settings_edit = Some(self.prefs.presets.len() - 1);
            ctx.memory_mut(|m| m.request_focus(preset_field(self.prefs.presets.len() - 1, 0)));
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

    fn settings_general(&mut self, ui: &mut egui::Ui, c: &UiColors) {
        section(ui, "IDIOMA", "O Maestro fala no idioma em que você fala com ele; aqui você fixa um.", c);
        segmented(ui, &mut self.prefs.language, &LANGUAGES, c);
        section(ui, "APARÊNCIA", "Sistema segue a configuração do seu sistema operacional.", c);
        let mut theme = match self.prefs.theme {
            Theme::System => 0,
            Theme::Light => 1,
            Theme::Dark => 2,
        };
        if segmented(ui, &mut theme, &["Sistema", "Claro", "Escuro"], c) {
            self.prefs.theme = [Theme::System, Theme::Light, Theme::Dark][theme];
        }
        section(ui, "NAVEGAÇÃO", "", c);
        choice(ui, "Movimento da câmera", ["Deslizar", "Instantâneo"], &mut self.prefs.instant_camera, c);
        choice(ui, "Abrir o app em", ["Canvas", "Chat"], &mut self.prefs.start_in_chat, c);
        section(ui, "ARQUIVOS", "", c);
        toggle(
            ui,
            "Confirmar antes de excluir arquivos",
            "Perguntar antes de uma árvore de arquivos mover arquivos para a lixeira.",
            &mut self.prefs.confirm_trash,
            c,
        );
        toggle(ui, "Mostrar dicas", "Dicas sobre os botões e atalhos.", &mut self.prefs.show_hints, c);
        toggle(ui, "Ligaduras no código", "=>, != e :: como um só sinal na fonte do IDE.", &mut self.prefs.ligatures, c);
    }

    fn settings_terminal(&mut self, ui: &mut egui::Ui, c: &UiColors) -> (Option<usize>, bool, bool) {
        let (mut remove, mut add, mut reset) = (None, false, false);
        ui.label(egui::RichText::new("PRESETS DE INÍCIO RÁPIDO").size(11.0).strong().color(dialog_label(&c)));
        ui.label(egui::RichText::new("Até 10 presets ativos aparecem ao criar um terminal.").size(12.0).color(dialog_label(&c)));
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
                        for (k, (text, hint, width)) in
                            [(&mut preset.name, "Nome", 120.0), (&mut preset.command, "comando (vazio: shell)", 170.0)].into_iter().enumerate()
                        {
                            let field = egui::TextEdit::singleline(text).id(preset_field(i, k)).hint_text(hint).desired_width(width);
                            ui.add(field.margin(egui::Margin::symmetric(8, 5)).background_color(c.island));
                        }
                    } else {
                        ui.vertical(|ui| {
                            ui.spacing_mut().item_spacing.y = 0.0;
                            let (name, color) = if preset.name.trim().is_empty() { ("sem nome", c.muted) } else { (preset.name.as_str(), c.text) };
                            ui.label(egui::RichText::new(name).size(13.5).color(color));
                            let command = if preset.command.is_empty() { "shell" } else { &preset.command };
                            ui.label(egui::RichText::new(command).size(11.5).monospace().color(c.muted));
                        });
                    }
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        // ⊖, red under the pointer, as in the reference app
                        let (rect, button) = ui.allocate_exact_size(Vec2::splat(26.0), Sense::click());
                        let tint = if button.hovered() { Color32::from_rgb(0xe0, 0x31, 0x31) } else { c.muted };
                        ui.painter().circle_stroke(rect.center(), 7.5, Stroke::new(1.2, tint));
                        ui.painter().hline(rect.center().x - 3.5..=rect.center().x + 3.5, rect.center().y, Stroke::new(1.4, tint));
                        if button.on_hover_cursor(CursorIcon::PointingHand).on_hover_text("Remover").clicked() {
                            remove = Some(i);
                        }
                        let (icon, tip) = if editing { (Icon::Check, "Pronto") } else { (Icon::Edit, "Editar") };
                        if icon_button(ui, 26.0, editing, Color32::TRANSPARENT, icon, c.muted, &c).on_hover_text(tip).clicked() {
                            self.settings_edit = if editing { None } else { Some(i) };
                            // straight into the name, ready to type
                            ui.memory_mut(|m| m.request_focus(preset_field(i, 0)));
                        }
                    });
                });
            }
        });
        ui.add_space(10.0);
        ui.horizontal(|ui| {
            let button = |text: &str| {
                egui::Button::new(egui::RichText::new(text).size(12.5).color(c.text)).fill(c.surface_low).corner_radius(8).min_size(Vec2::new(0.0, 28.0))
            };
            add = ui.add(button("+  Adicionar preset")).clicked();
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                reset = ui.add(button("Redefinir para padrão")).clicked();
            });
        });
        (remove, add, reset)
    }

    fn settings_chat(&mut self, ui: &mut egui::Ui, c: &UiColors) {
        section(ui, "CHAT", "Um chat novo já começa assim, sem perguntar. Modelo e esforço ficam nos chips sob a caixa do chat.", c);
        ui.label(egui::RichText::new("Agente").size(11.5).color(dialog_label(c)));
        ui.add_space(4.0);
        let mut agent = (self.prefs.chat_agent == "codex") as usize;
        if segmented(ui, &mut agent, &["Claude Code", "Codex"], c) {
            self.prefs.chat_agent = ["claude", "codex"][agent].into();
        }
        ui.add_space(8.0);
        dialog_folder(ui, "Pasta de trabalho", &mut self.prefs.chat_folder, "~/Desktop/MagoAgent", "Pasta dos chats", c);
        ui.add_space(8.0);
        ui.label(egui::RichText::new("O agente pode").size(11.5).color(dialog_label(c)));
        ui.add_space(4.0);
        segmented(ui, &mut self.prefs.chat_mode, &crate::app::chat_direct::CHAT_MODES, c);
    }

    fn settings_api(&mut self, ui: &mut egui::Ui, c: &UiColors) {
        section(
            ui,
            "LOCAL OU API",
            "Desligado, os chats rodam o claude e o codex com o login deles (sua assinatura). Ligado, rodam com a chave de API abaixo, cobrada por uso.",
            c,
        );
        toggle(
            ui,
            "Usar as chaves de API nos chats",
            "Vale a partir da próxima mensagem; o claude recebe ANTHROPIC_API_KEY e o codex OPENAI_API_KEY.",
            &mut self.prefs.use_api_keys,
            c,
        );
        section(ui, "CHAVES DE API", "Ficam só neste computador, em prefs.json.", c);
        dialog_secret(ui, "Anthropic", &mut self.prefs.anthropic_key, "sk-ant-…", c);
        dialog_secret(ui, "OpenAI", &mut self.prefs.openai_key, "sk-…", c);
    }

    fn settings_avatar(&mut self, ui: &mut egui::Ui, c: &UiColors) {
        section(ui, "AVATAR", "A barra do Maestro, por cima de tudo na tela, com o mascote do agente do chat.", c);
        toggle(ui, "Mostrar o avatar", "Também no menu ☰ › Ir › Avatar. O × da barra o esconde.", &mut self.prefs.avatar, c);
    }
}

fn settings_about(ui: &mut egui::Ui, c: &UiColors) {
    section(ui, "SOBRE", "", c);
    ui.label(egui::RichText::new(format!("{APP_TITLE} {}", env!("CARGO_PKG_VERSION"))).size(13.5).color(c.text));
    ui.label(
        egui::RichText::new(
            "Tudo roda neste computador: os agentes são o claude e o codex instalados, com o login deles, salvo as chaves de API em Ajustes › API.",
        )
        .size(12.0)
        .color(dialog_label(c)),
    );
}

/// A section's title and, when there is one, its line of explanation.
fn section(ui: &mut egui::Ui, title: &str, hint: &str, c: &UiColors) {
    ui.add_space(12.0);
    ui.label(egui::RichText::new(title).size(11.0).strong().color(dialog_label(c)));
    if !hint.is_empty() {
        ui.label(egui::RichText::new(hint).size(12.0).color(dialog_label(c)));
    }
    ui.add_space(6.0);
}

/// A two-way choice as pills: the second label is `true`.
fn choice(ui: &mut egui::Ui, label: &str, labels: [&str; 2], second: &mut bool, c: &UiColors) {
    ui.add_space(4.0);
    ui.label(egui::RichText::new(label).size(11.5).color(dialog_label(c)));
    let mut i = *second as usize;
    if segmented(ui, &mut i, &labels, c) {
        *second = i == 1;
    }
}

/// A switch with its label and explanation.
fn toggle(ui: &mut egui::Ui, label: &str, hint: &str, value: &mut bool, c: &UiColors) {
    ui.add_space(6.0);
    ui.horizontal(|ui| {
        let (rect, response) = ui.allocate_exact_size(Vec2::new(36.0, 20.0), Sense::click());
        let on = *value;
        ui.painter().rect_filled(rect, 10.0, if on { PRIMARY } else { c.surface_high });
        let knob = Pos2::new(if on { rect.right() - 10.0 } else { rect.left() + 10.0 }, rect.center().y);
        ui.painter().circle_filled(knob, 8.0, Color32::WHITE);
        if response.on_hover_cursor(CursorIcon::PointingHand).clicked() {
            *value = !on;
        }
        ui.add_space(6.0);
        ui.vertical(|ui| {
            ui.spacing_mut().item_spacing.y = 2.0;
            ui.label(egui::RichText::new(label).size(13.0).color(c.text));
            if !hint.is_empty() {
                ui.label(egui::RichText::new(hint).size(11.5).color(dialog_label(c)));
            }
        });
    });
}

/// The name (0) or command (1) box of preset `i` while it is edited.
fn preset_field(i: usize, k: usize) -> egui::Id {
    egui::Id::unique(("preset_field", i, k))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_api_keys_reach_the_agents_only_when_turned_on() {
        let mut prefs = Prefs { anthropic_key: " sk-ant-1 ".into(), openai_key: String::new(), ..Default::default() };
        assert_eq!(api_env(&prefs, "claude"), None, "off by default");
        prefs.use_api_keys = true;
        assert_eq!(api_env(&prefs, "claude"), Some(("ANTHROPIC_API_KEY", "sk-ant-1".to_string())));
        assert_eq!(api_env(&prefs, "codex"), None, "no key typed");
        prefs.openai_key = "sk-2".into();
        assert_eq!(api_env(&prefs, "codex"), Some(("OPENAI_API_KEY", "sk-2".to_string())));
    }

    #[test]
    fn every_section_of_the_settings_shows() {
        let mut app = DrawingApp::default();
        app.settings_open = true;
        let ctx = Context::default();
        for tab in 0..SETTINGS_TABS.len() {
            app.settings_tab = tab;
            ctx.run_ui(egui::RawInput::default(), |ui| app.settings_ui(ui.ctx())).textures_delta.clear();
        }
        assert!(app.settings_open);
    }
}
