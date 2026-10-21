//! The bar over the active terminal (or portal) and the new / edit terminal dialog.

use crate::prelude::*;

impl DrawingApp {
    /// Actions for the focused, enlarged or selected terminal or portal, under the toolbars.
    /// Returns the bar's rect so what comes below can make room.
    pub(crate) fn context_bar(&mut self, ctx: &Context, top: f32) -> Option<Rect> {
        let element = self.active_node()?.clone();
        let c = self.colors();
        let terminal = element.terminal.is_some();
        let portal = element.portal.clone();
        let links = self.links().get(&element.id).map_or(0, Vec::len);
        let enlarged = self.terminal_elevated.as_ref() == Some(&element.id);
        let (hopping, now) = (self.hop, self.now);
        let mut action = None;
        // sized like the reference app's: 24pt buttons with small icons in a pill, no hover
        // fill, just the tip under the button
        let area = egui::Area::new(egui::Id::unique("context_bar")).anchor(egui::Align2::CENTER_TOP, (0.0, top)).show(ctx, |ui| {
            self.island().corner_radius(18).inner_margin(egui::Margin::symmetric(12, 6)).show(ui, |ui| {
                ui.horizontal(|ui| {
                    ui.spacing_mut().item_spacing.x = 6.0;
                    let mut button = |ui: &mut egui::Ui, icon: Icon, on: bool, tip: &str, id: &'static str| {
                        let (rect, r) = ui.allocate_exact_size(Vec2::splat(24.0), Sense::click());
                        let since = hopping.filter(|(hop, _)| *hop == id).map_or(1.0, |(_, at)| now - at);
                        paint_icon_scaled(ui.painter(), icon, rect.center(), if on { PRIMARY } else { c.text }, 0.85, hop_height(since));
                        if r.on_hover_cursor(CursorIcon::PointingHand).on_hover_text(tip).clicked() {
                            action = Some(id);
                        }
                    };
                    button(ui, Icon::Edit, false, "Editar", "edit");
                    button(ui, Icon::Connect, self.connecting.is_some(), "Conectar a outro terminal, nota ou portal", "connect");
                    if links > 0 {
                        // how many it reaches, a blue dot beside the button
                        let (rect, r) = ui.allocate_exact_size(Vec2::new(15.0, 24.0), Sense::hover());
                        ui.painter().circle_filled(rect.center(), 7.5, PRIMARY);
                        ui.painter().text(rect.center(), egui::Align2::CENTER_CENTER, links.to_string(), FontId::proportional(10.0), Color32::WHITE);
                        r.on_hover_text(format!("Conectado a {links}"));
                    }
                    if terminal {
                        let composing = self.composer.as_ref().is_some_and(|(id, _)| id == &element.id);
                        button(ui, Icon::Composer, composing, "Compositor de prompts", "composer");
                        button(ui, Icon::Restart, false, "Reiniciar", "restart");
                        let tip = if enlarged { "Voltar ao canvas" } else { "Ampliar (clique duplo no título)" };
                        button(ui, Icon::Expand, enlarged, tip, "expand");
                    }
                    if portal.is_some() {
                        button(ui, Icon::Tool(Tool::Draw), false, "Lápis: circule na página o que mudar e escreva o pedido", "pencil");
                        button(ui, Icon::Tool(Tool::Portal), false, "Abrir no navegador", "external");
                    }
                    let (rect, _) = ui.allocate_exact_size(Vec2::new(1.0, 24.0), Sense::hover());
                    ui.painter().vline(rect.center().x, rect.center().y - 7.5..=rect.center().y + 7.5, Stroke::new(1.0, c.surface_low));
                    button(ui, Icon::Trash, false, "Apagar", "delete");
                });
            });
        });
        let id = element.id.clone();
        if let Some(hop) = action {
            self.hop = Some((hop, self.now));
        }
        if hopping.is_some_and(|(_, at)| now - at < 0.4) {
            ctx.request_repaint();
        }
        match action {
            Some("edit") => {
                if let Some(spec) = element.terminal.clone() {
                    self.terminal_dialog = Some(TerminalDialog { target: Some(id), at: Rect::NOTHING, spec });
                } else if let Some(spec) = portal {
                    self.portal_dialog = Some(PortalDialog { target: Some(id), at: Rect::NOTHING, spec });
                }
            }
            Some("pencil") => self.start_pencil(&id),
            Some("external") => {
                if let Some(spec) = portal {
                    open_external(&normalize_url(&spec.url));
                }
            }
            Some("connect") => self.connecting = if self.connecting.is_some() { None } else { Some(id) },
            Some("composer") => {
                let open = self.composer.as_ref().is_some_and(|(open, _)| open == &id);
                self.composer = if open { None } else { Some((id, String::new())) };
            }
            Some("restart") => {
                self.terminals.remove(&id);
                self.terminal_focus = Some(id);
            }
            Some("expand") => self.set_elevated(if enlarged { None } else { Some(id) }),
            Some("delete") => {
                if let Some(i) = self.scene.elements.iter().position(|e| e.id == id) {
                    self.selected = vec![i];
                    self.delete_selected();
                }
            }
            _ => {}
        }
        Some(area.response.rect)
    }

    pub(crate) fn terminal_dialog_ui(&mut self, ctx: &Context) {
        let Some(mut dialog) = self.terminal_dialog.clone() else { return };
        let c = self.colors();
        let (mut open, mut confirm, mut cancel) = (true, false, false);
        let editing = dialog.target.is_some();
        egui::Window::new(if editing { "Editar terminal" } else { "Novo terminal" })
            .open(&mut open)
            .collapsible(false)
            .resizable(false)
            .anchor(egui::Align2::CENTER_CENTER, Vec2::ZERO)
            .show(ctx, |ui| {
                ui.set_width(400.0);
                ui.label(egui::RichText::new("Início rápido").size(12.0).color(c.muted));
                ui.horizontal(|ui| {
                    for (name, command) in TERMINAL_PRESETS {
                        if ui.selectable_label(dialog.spec.command == command, name).clicked() {
                            dialog.spec.name = self.unique_terminal_name(name, dialog.target.as_deref());
                            dialog.spec.command = command.into();
                        }
                    }
                });
                ui.add_space(8.0);
                egui::Grid::new("terminal_fields").num_columns(2).spacing([12.0, 8.0]).show(ui, |ui| {
                    ui.label("Nome");
                    ui.add(egui::TextEdit::singleline(&mut dialog.spec.name).desired_width(290.0));
                    ui.end_row();
                    ui.label("Comando");
                    ui.add(egui::TextEdit::singleline(&mut dialog.spec.command).hint_text("ex.: claude, codex, ou vazio para shell").desired_width(290.0));
                    ui.end_row();
                    ui.label("Pasta");
                    ui.horizontal(|ui| {
                        ui.add(egui::TextEdit::singleline(&mut dialog.spec.cwd).hint_text("~").desired_width(200.0));
                        if ui.button("Procurar…").clicked() {
                            if let Some(dir) = rfd::FileDialog::new().set_title("Pasta do terminal").pick_folder() {
                                dialog.spec.cwd = dir.to_string_lossy().into();
                            }
                        }
                    });
                    ui.end_row();
                });
                ui.add_space(4.0);
                ui.label(egui::RichText::new("Agentes ligados a ele pelo botão Conectar conversam com `magoagent ask`.").size(11.0).color(c.muted));
                ui.add_space(8.0);
                ui.horizontal(|ui| {
                    cancel = ui.button("Cancelar").clicked();
                    let label = egui::RichText::new(if editing { "Salvar" } else { "Criar" }).color(Color32::WHITE);
                    confirm = ui.add(egui::Button::new(label).fill(PRIMARY)).clicked();
                });
                confirm |= ui.input(|i| i.key_pressed(Key::Enter));
            });
        if !open || cancel {
            self.terminal_dialog = None;
            return;
        }
        self.terminal_dialog = Some(dialog);
        if confirm {
            self.confirm_terminal_dialog();
        }
    }

    /// A message box under the terminal: Enter sends it as a pasted prompt plus Enter (like
    /// `magoagent ask`, without waiting), Shift+Enter breaks the line, Esc closes.
    pub(crate) fn composer_ui(&mut self, ctx: &Context) {
        let Some((id, mut text)) = self.composer.clone() else { return };
        let Some(element) = self.scene.elements.iter().find(|e| e.id == id).cloned() else {
            self.composer = None;
            return;
        };
        let c = self.colors();
        let (frame, _) = self.terminal_frame(&element);
        let width = (frame.width() - 24.0).clamp(280.0, 900.0);
        let at = Pos2::new(frame.center().x - width / 2.0, (frame.bottom() + 8.0).min(self.canvas_rect.bottom() - 64.0));
        let name = element.terminal.as_ref().map_or("terminal".into(), |t| t.name.clone());
        let (mut send, mut close) = (false, false);
        egui::Area::new(egui::Id::unique("composer")).fixed_pos(at).order(egui::Order::Foreground).show(ctx, |ui| {
            self.island().inner_margin(6).show(ui, |ui| {
                ui.set_width(width);
                ui.horizontal(|ui| {
                    let edit = egui::TextEdit::multiline(&mut text)
                        .hint_text(format!("Mensagem para {name} — Enter envia, Shift+Enter quebra a linha, Esc fecha"))
                        .desired_rows(1)
                        .desired_width(width - 40.0)
                        .return_key(Some(egui::KeyboardShortcut::new(Modifiers::SHIFT, Key::Enter)))
                        .frame(egui::Frame::NONE);
                    let response = ui.add(edit);
                    if !response.has_focus() && !response.lost_focus() {
                        response.request_focus();
                    }
                    send = response.has_focus() && ui.input(|i| i.key_pressed(Key::Enter) && !i.modifiers.shift);
                    close = ui.input(|i| i.key_pressed(Key::Escape));
                    let button = square_button(ui, 28.0, false, PRIMARY, &c);
                    paint_icon(ui.painter(), Icon::Tool(Tool::Arrow), button.rect.center(), Color32::WHITE);
                    send |= button.on_hover_text("Enviar").clicked();
                });
            });
        });
        if close {
            self.composer = None;
            return;
        }
        if send && !text.trim().is_empty() {
            self.send_prompt(&id, &text);
            text.clear();
        }
        self.composer = Some((id, text));
    }

    /// Types `text` into a terminal as a pasted prompt, then Enter on its own, as a person would
    /// (the composer and the chat use it).
    pub(crate) fn send_prompt(&self, id: &str, text: &str) {
        if let Some(Ok(session)) = self.terminals.get(id) {
            let handle = session.handle.clone();
            let message = text.trim().to_string();
            std::thread::spawn(move || {
                handle.write(paste_bytes(&message, handle.mode().contains(alacritty_terminal::term::TermMode::BRACKETED_PASTE)));
                std::thread::sleep(std::time::Duration::from_millis(150));
                handle.write(b"\r".to_vec());
            });
        }
    }
}
