//! The bar over the active terminal (or portal) and the new / edit terminal dialog.

use crate::app::ui::context::{ContextMenu, Target, item};
use crate::prelude::*;

impl DrawingApp {
    /// Actions for the focused, enlarged or selected terminal or portal, under the toolbars.
    /// Returns the bar's rect so what comes below can make room.
    pub(crate) fn context_bar(&mut self, ctx: &Context, top: f32) -> Option<Rect> {
        let element = self.active_node()?.clone();
        let c = self.colors();
        let terminal = element.terminal.is_some();
        let note = element.created.is_some();
        let portal = element.portal.clone();
        let ide = element.ide.clone();
        let links = self.links().get(&element.id).map_or(0, Vec::len);
        let annotations = self.annotations.get(&element.id).map_or(0, Vec::len);
        let enlarged = self.terminal_elevated.as_ref() == Some(&element.id);
        let (hopping, now) = (self.hop, self.now);
        let mut action = None;
        // the closures below can't write `action` while `button` holds it: they set these instead
        let (mut annotations_clicked, mut trash_clicked) = (false, false);
        let mut anchor = Rect::NOTHING;
        let mut badge = None;
        let size = element.note.font_size;
        let raw = element.note.raw;
        let index = self.scene.elements.iter().position(|e| e.id == element.id);
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
                            anchor = rect;
                        }
                    };
                    if note {
                        // the note's bar: color, size, markdown, lists, raw view, more (ui/notes.rs)
                        button(ui, Icon::Palette, self.note_color.is_some(), "Cor", "note_color");
                        bar_divider(ui, &c);
                        button(ui, Icon::Minus, false, "Diminuir tamanho da fonte", "font_down");
                        let (rect, _) = ui.allocate_exact_size(Vec2::new(22.0, 24.0), Sense::hover());
                        ui.painter().text(rect.center(), egui::Align2::CENTER_CENTER, size.round().to_string(), FontId::proportional(12.0), c.text);
                        button(ui, Icon::Plus, false, "Aumentar tamanho da fonte", "font_up");
                        bar_divider(ui, &c);
                        button(ui, Icon::Bold, false, "Negrito", "fmt:**");
                        button(ui, Icon::Italic, false, "Itálico", "fmt:*");
                        button(ui, Icon::Strike, false, "Tachado", "fmt:~~");
                        button(ui, Icon::Code, false, "Código", "fmt:`");
                        button(ui, Icon::Heading, false, "Título", "note_heading");
                        bar_divider(ui, &c);
                        button(ui, Icon::Todo, false, "Inserir tarefa", "fmt:- [ ] ");
                        button(ui, Icon::Bullet, false, "Inserir marcador", "fmt:- ");
                        button(ui, Icon::CodeBlock, false, "Bloco de código", "fmt:```");
                        bar_divider(ui, &c);
                        button(ui, Icon::Markdown, raw, if raw { "Formatado" } else { "Bruto" }, "note_raw");
                        button(ui, Icon::More, false, "Mais", "note_more");
                        bar_divider(ui, &c);
                    }
                    if portal.is_some() {
                        button(ui, Icon::Restart, false, "Recarregar", "reload");
                    } else if !note {
                        button(ui, Icon::Edit, false, "Editar", "edit");
                    }
                    button(ui, Icon::Connect, self.connecting.is_some(), "Conectar a outro terminal, nota, portal ou IDE", "connect");
                    if links > 0 {
                        // how many it reaches, a blue dot beside the button: its popover lists them
                        let (rect, r) = ui.allocate_exact_size(Vec2::new(15.0, 24.0), Sense::click());
                        ui.painter().circle_filled(rect.center(), 7.5, PRIMARY);
                        ui.painter().text(rect.center(), egui::Align2::CENTER_CENTER, links.to_string(), FontId::proportional(10.0), Color32::WHITE);
                        if r.on_hover_cursor(CursorIcon::PointingHand).on_hover_text("Gerenciar conexões").clicked() {
                            badge = Some(rect);
                        }
                    }
                    if terminal {
                        let composing = self.composer.as_ref().is_some_and(|(id, _)| id == &element.id);
                        button(ui, Icon::Composer, composing, "Compositor de prompts", "composer");
                        button(ui, Icon::Restart, false, "Reiniciar", "restart");
                    }
                    if ide.is_some() {
                        button(ui, Icon::Tool(Tool::Terminal), false, "Claude Code: terminal ligado a este IDE", "claude");
                        button(ui, Icon::Open, false, "Mostrar na pasta", "folder");
                    }
                    if terminal || ide.is_some() {
                        let tip = if enlarged { "Voltar ao canvas" } else if terminal { "Ampliar (clique duplo no título)" } else { "Abrir (clique duplo)" };
                        button(ui, Icon::Expand, enlarged, tip, "expand");
                    }
                    if let Some(spec) = &portal {
                        // the reference app's order: reload, connect | design, annotations, edit, hide bar | close
                        let (rect, _) = ui.allocate_exact_size(Vec2::new(1.0, 24.0), Sense::hover());
                        ui.painter().vline(rect.center().x, rect.center().y - 7.5..=rect.center().y + 7.5, Stroke::new(1.0, c.surface_low));
                        let armed = self.design.as_ref().is_some_and(|d| d.portal == element.id);
                        button(ui, Icon::Brush, armed, if armed { "Sair do Design" } else { "Design" }, "design");
                        if annotations > 0 {
                            // the count, as the reference app's badge
                            let (rect, r) = ui.allocate_exact_size(Vec2::new(16.0, 24.0), Sense::click());
                            ui.painter().circle_filled(rect.center(), 8.0, PRIMARY);
                            ui.painter().text(rect.center(), egui::Align2::CENTER_CENTER, annotations.to_string(), FontId::proportional(10.0), Color32::WHITE);
                            if r.on_hover_cursor(CursorIcon::PointingHand).on_hover_text("Anotações").clicked() {
                                annotations_clicked = true;
                            }
                        }
                        button(ui, Icon::Edit, false, "Editar", "edit");
                        button(ui, Icon::Panel, spec.chrome_hidden, if spec.chrome_hidden { "Mostrar barra" } else { "Ocultar barra" }, "chrome");
                    }
                    let (rect, _) = ui.allocate_exact_size(Vec2::new(1.0, 24.0), Sense::hover());
                    ui.painter().vline(rect.center().x, rect.center().y - 7.5..=rect.center().y + 7.5, Stroke::new(1.0, c.surface_low));
                    if portal.is_some() {
                        // the reference app's destructive button: red, and it closes the portal
                        let (rect, r) = ui.allocate_exact_size(Vec2::splat(24.0), Sense::click());
                        paint_icon(ui.painter(), Icon::Trash, rect.center(), Color32::from_rgb(0xf8, 0x51, 0x49));
                        if r.on_hover_cursor(CursorIcon::PointingHand).on_hover_text("Fechar portal").clicked() {
                            trash_clicked = true;
                        }
                    } else {
                        button(ui, Icon::Trash, false, "Apagar", "delete");
                    }
                });
            });
        });
        if annotations_clicked {
            action = Some("annotations");
        }
        if trash_clicked {
            action = Some("delete");
        }
        let id = element.id.clone();
        if let Some(rect) = badge {
            self.toggle_node_links(&id, rect);
        }
        if let Some(hop) = action {
            self.hop = Some((hop, self.now));
        }
        if hopping.is_some_and(|(_, at)| now - at < 0.4) {
            ctx.request_repaint();
        }
        match action {
            Some("edit") => {
                if let Some(spec) = element.terminal.clone() {
                    self.terminal_dialog = Some(TerminalDialog { target: Some(id), at: Rect::NOTHING, spec, tab: 0 });
                } else if let Some(spec) = portal {
                    self.portal_dialog = Some(PortalDialog { target: Some(id), at: Rect::NOTHING, spec });
                } else if let Some(spec) = ide {
                    self.ide_dialog = Some(IdeDialog { target: Some(id), at: Rect::NOTHING, spec });
                }
            }
            Some("design") => self.toggle_design(&id),
            Some("annotations") => {
                if self.design.as_ref().is_some_and(|d| d.portal == id) {
                    self.portal_script(&id, "window.__magoDesign&&window.__magoDesign.panel()");
                } else {
                    self.arm_design(&id, true);
                }
            }
            Some("reload") => self.portal_reload(&id),
            Some("chrome") => {
                if let Some(i) = self.scene.elements.iter().position(|e| e.id == id) {
                    self.snapshot();
                    if let Some(spec) = self.scene.elements[i].portal.as_mut() {
                        spec.chrome_hidden = !spec.chrome_hidden;
                    }
                }
            }
            Some("claude") => self.claude_for_ide(&id),
            Some("folder") => {
                // the editor's root, with `~` expanded (none when the folder doesn't exist)
                if let Some(root) = self.ides.get(&id).and_then(|i| i.root.as_ref()) {
                    open_external(&root.to_string_lossy());
                }
            }
            Some("note_color") => {
                self.note_color = if self.note_color.is_some() { None } else { Some((anchor.left_bottom() + Vec2::new(0.0, 6.0), Rect::NOTHING)) };
            }
            Some("note_heading") | Some("note_more") => {
                let (items, at) = if action == Some("note_heading") {
                    (vec![item("fmt:# ", "Título 1"), item("fmt:## ", "Título 2"), item("fmt:### ", "Título 3")], anchor)
                } else {
                    let locked = element.note.content_locked;
                    (vec![item("rename", "Renomear nota"), item("copy_content", "Copiar conteúdo"), item("lock_content", if locked { "Desbloquear conteúdo" } else { "Bloquear conteúdo" })], anchor)
                };
                self.context = Some(ContextMenu::new(at.left_bottom() + Vec2::new(0.0, 6.0), Target::Element(id.clone()), Some(items)));
            }
            Some("note_raw") => {
                if let Some(i) = index {
                    self.snapshot();
                    self.scene.elements[i].note.raw = !raw;
                }
            }
            Some("font_down") | Some("font_up") => {
                if let Some(i) = index {
                    self.note_font_step(i, if action == Some("font_up") { NOTE_FONT_STEP } else { -NOTE_FONT_STEP });
                }
            }
            Some(op) if op.starts_with("fmt:") => {
                if let Some(i) = index {
                    self.note_format(ctx, i, &op[4..]);
                }
            }
            Some("connect") => {
                if self.connecting.is_some() {
                    self.connecting = None;
                } else {
                    self.start_connect(id);
                }
            }
            Some("composer") => self.toggle_composer(id),
            Some("restart") => {
                // an unloaded one starts again too
                if let Some(i) = self.scene.elements.iter().position(|e| e.id == id) {
                    self.unload_node(i, false);
                }
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
        let editing = dialog.target.is_some();
        let title = if editing { "Editar terminal" } else { "Novo terminal" };
        // the quick starts switched on in Ajustes, 10 at most, four to a row
        let presets: Vec<Preset> = self.prefs.presets.iter().filter(|p| p.on).take(10).cloned().collect();
        let mut settings = false;
        let (confirm, cancel) = dialog_ui(ctx, &c, title, if editing { "Salvar" } else { "Criar" }, |ui| {
            ui.horizontal(|ui| {
                ui.label(egui::RichText::new("Início rápido").size(11.5).color(dialog_label(&c)));
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    let link = egui::Label::new(egui::RichText::new("Editar presets…").size(11.5).color(PRIMARY)).sense(Sense::click());
                    settings = ui.add(link).on_hover_cursor(CursorIcon::PointingHand).clicked();
                });
            });
            ui.add_space(2.0);
            const TILE: Vec2 = Vec2::new(99.0, 64.0);
            for row in presets.chunks(4) {
                ui.horizontal(|ui| {
                    ui.spacing_mut().item_spacing.x = 8.0;
                    ui.add_space((DIALOG_WIDTH - row.len() as f32 * (TILE.x + 8.0) + 8.0) / 2.0);
                    for preset in row {
                        let (rect, response) = ui.allocate_exact_size(TILE, Sense::click());
                        let on = dialog.spec.command == preset.command;
                        let fill = if on { c.selected } else if response.hovered() { c.surface_low } else { Color32::TRANSPARENT };
                        ui.painter().rect_filled(rect, 10, fill);
                        if on {
                            ui.painter().rect_stroke(rect, 10, Stroke::new(1.0, PRIMARY), StrokeKind::Inside);
                        }
                        let mark = TerminalSpec { command: preset.command.clone(), ..Default::default() };
                        terminal_icon(ui.painter(), &mark, rect.center() - Vec2::new(0.0, 10.0), c.text);
                        let name = ui.painter().layout(preset.name.clone(), FontId::proportional(12.0), c.text, TILE.x - 8.0);
                        ui.painter().galley(rect.center() + Vec2::new(-name.size().x / 2.0, 8.0), name, c.text);
                        if response.on_hover_cursor(CursorIcon::PointingHand).clicked() {
                            dialog.spec.command = preset.command.clone();
                        }
                    }
                });
            }
            ui.add_space(8.0);
            segmented(ui, &mut dialog.tab, &["Detalhes", "Aparência"], &c);
            ui.add_space(4.0);
            if dialog.tab == 0 {
                dialog_field(ui, "Nome", &mut dialog.spec.name, "Nome do terminal", DIALOG_WIDTH, &c);
                dialog_field(ui, "Comando", &mut dialog.spec.command, "ex.: claude, codex, ou vazio para shell", DIALOG_WIDTH, &c);
                ui.add_space(8.0);
                let mut monitor = !dialog.spec.quiet;
                dialog_check(ui, &mut monitor, "Monitorar atividade", &c).on_hover_text("Avisa quando o trabalho dele termina");
                dialog.spec.quiet = !monitor;
                dialog_check(ui, &mut dialog.spec.maestro, "Maestro — pode criar e conectar terminais", &c).on_hover_text("Só um terminal Maestro cria terminais, portais e conexões");
                dialog_folder(ui, "Pasta de trabalho", &mut dialog.spec.cwd, "~", "Pasta do terminal", &c);
                dialog_note(ui, "Agentes ligados a ele pelo botão Conectar conversam entre si com magoagent ask.", &c);
            } else {
                ui.add_space(6.0);
                ui.label(egui::RichText::new("Ícone").size(11.5).color(dialog_label(&c)));
                let tint = dialog.spec.color.map_or(c.text, |[r, g, b]| Color32::from_rgb(r, g, b));
                ui.horizontal_wrapped(|ui| {
                    ui.spacing_mut().item_spacing = Vec2::splat(6.0);
                    for key in TERMINAL_ICONS {
                        let (rect, response) = ui.allocate_exact_size(Vec2::splat(28.0), Sense::click());
                        let on = dialog.spec.icon == key;
                        ui.painter().rect_filled(rect, 7, if on { c.selected } else if response.hovered() { c.surface_low } else { Color32::TRANSPARENT });
                        if on {
                            ui.painter().rect_stroke(rect, 7, Stroke::new(1.0, PRIMARY), StrokeKind::Inside);
                        }
                        if key.is_empty() {
                            ui.painter().text(rect.center(), egui::Align2::CENTER_CENTER, "auto", FontId::proportional(10.0), c.muted);
                        } else {
                            terminal_icon(ui.painter(), &TerminalSpec { icon: key.into(), ..Default::default() }, rect.center(), tint);
                        }
                        let tip = TERMINAL_ICON_NAMES[TERMINAL_ICONS.iter().position(|k| *k == key).unwrap_or(0)];
                        if response.on_hover_text(tip).clicked() {
                            dialog.spec.icon = key.into();
                        }
                    }
                });
                ui.add_space(6.0);
                ui.label(egui::RichText::new("Cor").size(11.5).color(dialog_label(&c)));
                ui.horizontal(|ui| {
                    ui.spacing_mut().item_spacing.x = 8.0;
                    for color in TERMINAL_COLORS {
                        let (rect, response) = ui.allocate_exact_size(Vec2::splat(22.0), Sense::click());
                        match color {
                            Some([r, g, b]) => ui.painter().circle_filled(rect.center(), 9.0, Color32::from_rgb(r, g, b)),
                            None => ui.painter().circle_stroke(rect.center(), 8.5, Stroke::new(1.0, c.muted)),
                        };
                        if dialog.spec.color == color {
                            ui.painter().circle_stroke(rect.center(), 11.0, Stroke::new(1.5, c.text));
                        }
                        if response.on_hover_cursor(CursorIcon::PointingHand).clicked() {
                            dialog.spec.color = color;
                        }
                    }
                });
                dialog_note(ui, "O ícone e a cor aparecem no título do terminal, no canvas.", &c);
            }
        });
        if settings {
            self.settings_open = true;
        }
        if cancel {
            self.terminal_dialog = None;
            return;
        }
        self.terminal_dialog = Some(dialog);
        if confirm {
            self.confirm_terminal_dialog();
        }
    }

    /// A message box under the terminal, like the reference app's docked one: as wide as the
    /// terminal allows (up to 560), growing down with the text until 220 tall, then it scrolls.
    /// Enter sends it as a pasted prompt plus Enter (like `magoagent ask`, without waiting),
    /// Shift+Enter breaks the line, Esc closes.
    pub(crate) fn composer_ui(&mut self, ctx: &Context) {
        let Some((id, mut text)) = self.composer.clone() else { return };
        let Some(element) = self.scene.elements.iter().find(|e| e.id == id).cloned() else {
            self.composer = None;
            return;
        };
        // the chat screen covers the canvas: the composer waits under it, without the keys; so it
        // does under a dialog, the palette or the export, which type in fields of their own
        if self.chat_open || self.dialog_open() || self.palette_open {
            return;
        }
        let (frame, _) = self.terminal_frame(&element);
        let width = (frame.width() - 24.0).min(560.0);
        // too small on screen to fit under it: it waits, open, until the terminal is bigger
        if width < 160.0 {
            return;
        }
        // clear of the dashed outline around the terminal, and of the window's bottom; with the
        // terminal panned away it stays in view at the edge, where the keys it has go
        let canvas = self.canvas_rect;
        let top = (frame.bottom() + 14.0).min(canvas.bottom() - 64.0).max(canvas.top() + 8.0);
        let half = width / 2.0 + 8.0;
        let x = frame.center().x.min(canvas.right() - half).max(canvas.left() + half);
        let tallest = (canvas.bottom() - top - 24.0).clamp(20.0, 220.0);
        let name = element.terminal.as_ref().map_or("terminal".into(), |t| t.name.clone());
        let (mut send, mut close) = (false, false);
        egui::Area::new(egui::Id::unique("composer"))
            .fixed_pos(Pos2::new(x, top))
            .pivot(egui::Align2::CENTER_TOP)
            .order(egui::Order::Foreground)
            .show(ctx, |ui| {
                // a pill, like a chat box; the button stays on the last line as the text grows
                // the send button sits 4 from the top, the right and the bottom; one line of text in
                // the middle of its height
                self.island().corner_radius(22).inner_margin(egui::Margin { left: 18, right: 4, top: 4, bottom: 4 }).show(ui, |ui| {
                    ui.set_width(width - 22.0);
                    // an Area offers only last frame's size; room to grow up to the cap
                    ui.set_max_height(tallest);
                    let response = egui::ScrollArea::vertical().id_salt("composer_scroll").max_height(tallest).show(ui, |ui| {
                        let edit = egui::TextEdit::multiline(&mut text)
                            .hint_text(format!("Mensagem para {name} · Esc para fechar"))
                            .desired_rows(1)
                            .margin(egui::Margin::symmetric(0, 6))
                            .min_size(Vec2::new(0.0, 32.0))
                            .align(egui::Align2::LEFT_CENTER)
                            .desired_width(width - 64.0)
                            .return_key(Some(egui::KeyboardShortcut::new(Modifiers::SHIFT, Key::Enter)))
                            .frame(egui::Frame::NONE);
                        ui.add(edit)
                    }).inner;
                    if !response.has_focus() && !response.lost_focus() {
                        response.request_focus();
                    }
                    // egui's own focus, not has_focus(): that one also asks for the window's
                    let typing = ui.memory(|m| m.has_focus(response.id));
                    send = typing && ui.input(|i| i.events.iter().any(|e| matches!(e, egui::Event::Key { key: Key::Enter, pressed: true, modifiers, .. } if !modifiers.shift)));
                    close = ui.input(|i| i.key_pressed(Key::Escape));
                    // over the right end of the last line: a row would cap the text's height
                    let inner = ui.min_rect();
                    let rect = Rect::from_min_size(inner.max - Vec2::splat(32.0), Vec2::splat(32.0));
                    let button = ui.interact(rect, ui.scope_id().with("composer_send"), Sense::click());
                    let ready = !text.trim().is_empty();
                    let fill = match (ready, button.hovered()) {
                        (false, _) => Color32::from_gray(0x55),
                        (true, true) => PRIMARY.gamma_multiply(1.15),
                        (true, false) => PRIMARY,
                    };
                    ui.painter().circle_filled(rect.center(), 16.0, fill);
                    paint_icon(ui.painter(), Icon::Tool(Tool::Arrow), rect.center(), Color32::WHITE);
                    send |= button.on_hover_cursor(CursorIcon::PointingHand).on_hover_text("Enviar").clicked();
                });
            });
        if close {
            self.composer = None;
            return;
        }
        if send && !text.trim().is_empty() && self.send_prompt(&id, &text) {
            text.clear();
        }
        self.composer = Some((id, text));
    }

    /// The bar's composer button: opens it under the terminal, with what was left in it when its
    /// workspace was hidden, or closes it.
    pub(crate) fn toggle_composer(&mut self, id: String) {
        let open = self.composer.as_ref().is_some_and(|(open, _)| open == &id);
        self.composer = if open {
            None
        } else {
            let draft = self.drafts.remove(&id).unwrap_or_default();
            Some((id, draft))
        };
    }

    /// Types `text` into a terminal as a prompt, off the UI thread (the composer, the chat and
    /// the Design mode use it). False, with a toast saying why, when it would land somewhere else.
    pub(crate) fn send_prompt(&mut self, id: &str, text: &str) -> bool {
        let name = self.terminal_name(id);
        match self.terminals.get(id) {
            Some(Ok(session)) => match session.handle.ready() {
                Ok(()) => {
                    self.send_prompt_after(id, text, std::time::Duration::ZERO);
                    true
                }
                Err(why) => {
                    self.toast(format!("{name} {why}"));
                    false
                }
            },
            _ => {
                self.toast(format!("{name} não está rodando: abra o terminal e envie de novo."));
                false
            }
        }
    }

    /// `send_prompt` after a wait: the IDE's comments give Claude Code time to drop its diff
    /// prompt, so readiness is asked then, a few times. A refusal gives the words back
    /// (`SpaceJob::Draft`).
    pub(crate) fn send_prompt_after(&self, id: &str, text: &str, delay: std::time::Duration) {
        if let Some(Ok(session)) = self.terminals.get(id) {
            let (handle, message, name, tx, id) = (session.handle.clone(), text.trim().to_string(), self.terminal_name(id), self.space_events.0.clone(), id.to_string());
            std::thread::spawn(move || {
                std::thread::sleep(delay);
                let tries = if delay.is_zero() { 1 } else { 12 };
                let mut result = Err(String::new());
                for _ in 0..tries {
                    result = handle.type_prompt(&message);
                    if result.is_ok() {
                        return;
                    }
                    std::thread::sleep(std::time::Duration::from_millis(250));
                }
                if let Err(why) = result {
                    let _ = tx.send(crate::app::floors::SpaceJob::Draft { id, text: message, why: format!("{name} {why}") });
                }
            });
        }
    }

    /// A terminal's name as people see it ("O terminal" when it has none).
    pub(crate) fn terminal_name(&self, id: &str) -> String {
        let spec = self.all_elements().find(|e| e.id == id).and_then(|e| e.terminal.as_ref());
        spec.map(|t| t.name.trim().to_string()).filter(|n| !n.is_empty()).unwrap_or_else(|| "O terminal".into())
    }

    /// The note bar's color popover: the ten papers, and a custom one through the color picker
    /// (its HEX field). A press outside it, Esc or a pick closes it.
    pub(crate) fn note_color_ui(&mut self, ctx: &Context) {
        let Some((at, last)) = self.note_color else { return };
        let id = self.active_node().filter(|e| e.created.is_some()).map(|e| e.id.clone());
        let Some(i) = id.and_then(|id| self.scene.elements.iter().position(|e| e.id == id)) else {
            self.note_color = None;
            return;
        };
        let pressed_outside = ctx.input(|i| i.pointer.any_pressed() && i.pointer.interact_pos().is_some_and(|p| !last.contains(p)));
        if ctx.input(|i| i.key_pressed(Key::Escape)) || (pressed_outside && last != Rect::NOTHING) {
            self.note_color = None;
            return;
        }
        let c = self.colors();
        let fill = self.scene.elements[i].style.fill.unwrap_or(STICKY_PAPER);
        let (mut picked, mut custom, dark) = (None, fill, self.dark());
        let area = egui::Area::new(egui::Id::unique("note_color")).fixed_pos(at).order(egui::Order::Foreground).show(ctx, |ui| {
            self.island().inner_margin(10).show(ui, |ui| {
                ui.set_width(150.0);
                ui.label(egui::RichText::new("Cores predefinidas").size(11.5).color(dialog_label(&c)));
                ui.add_space(4.0);
                ui.horizontal_wrapped(|ui| {
                    ui.spacing_mut().item_spacing = Vec2::splat(6.0);
                    for (paper, name) in NOTE_PAPERS {
                        if swatch(ui, paper, fill == paper, 22.0, dark).on_hover_text(name).clicked() {
                            picked = Some(paper);
                        }
                    }
                });
                ui.add_space(8.0);
                ui.label(egui::RichText::new("Cor personalizada").size(11.5).color(dialog_label(&c)));
                color_picker(ui, &mut custom);
            });
        });
        self.note_color = Some((at, area.response.rect));
        let chosen = picked.or((custom != fill).then_some(custom));
        if let Some(paper) = chosen {
            self.snapshot();
            self.scene.elements[i].style.fill = Some(paper);
        }
        if picked.is_some() {
            self.note_color = None;
        }
    }
}

/// The thin line between the note bar's groups.
fn bar_divider(ui: &mut egui::Ui, c: &UiColors) {
    let (rect, _) = ui.allocate_exact_size(Vec2::new(1.0, 24.0), Sense::hover());
    ui.painter().vline(rect.center().x, rect.center().y - 7.5..=rect.center().y + 7.5, Stroke::new(1.0, c.surface_low));
}
