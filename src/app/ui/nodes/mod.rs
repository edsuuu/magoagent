//! The bar over the active terminal (or portal) and the new / edit terminal dialog.

use crate::app::ui::context::{ContextMenu, Target, item};
use crate::prelude::*;
mod bar;

/// A prompt that never reached terminal `id`, and why: its words come back to be sent again.
pub(crate) struct Returned {
    pub(crate) id: String,
    pub(crate) text: String,
    pub(crate) why: String,
}

/// The bar's area, one per kind of node: egui centers an area by the size it had last frame, so a
/// bar that kept another kind's width (a note's 535 under an IDE's 168) was drawn off its center
/// for a frame.
// ponytail: within a kind the bar still grows by a badge (links, annotations) and is half of it off for a frame; give the row its width up front if that shows
pub(crate) fn context_bar_id(element: &Element) -> egui::Id {
    let kind = match element {
        e if e.created.is_some() => "note",
        e if e.filetree.is_some() => "tree",
        e if e.portal.is_some() => "portal",
        e if e.ide.is_some() => "ide",
        e if e.chat.is_some() => "chat",
        _ => "terminal",
    };
    egui::Id::unique(("context_bar", kind))
}

impl DrawingApp {
    /// Two or more notes selected (or a pile and a note): the reference app's bar with the
    /// fichário button, which piles them up (Colocar / Adicionar ao Fichário).
    pub(crate) fn notes_bar(&mut self, ctx: &Context, top: f32) -> Option<Rect> {
        let loose = self.loose_selected_notes().len();
        let piles = self.selected.iter().filter(|i| self.scene.elements.get(**i).is_some_and(|e| e.note.stacked && e.note.front)).count();
        let tip = match (piles, loose) {
            (0, 2..) => "Colocar no Fichário",
            (1, 1..) => "Adicionar ao Fichário",
            _ => return None,
        };
        let dark = self.dark();
        let c = self.colors();
        let middle = self.sidebar_width() / 2.0;
        let notes = self.loose_selected_notes();
        let size = notes.first().map_or(NOTE_FONT_MIN, |n| self.scene.elements[*n].note.font_size);
        let (mut stack, mut color, mut step) = (None, None, 0.0);
        let area = egui::Area::new(egui::Id::new("notes_bar")).anchor(egui::Align2::CENTER_TOP, (middle, top)).show(ctx, |ui| {
            let size_id = egui::Id::new("notes_bar_size");
            if let Some(size) = ui.data(|d| d.get_temp::<Vec2>(size_id)) {
                paint_glass(ui.painter(), Rect::from_min_size(ui.max_rect().min, size), GLASS_HEIGHT / 2.0, dark, false);
            }
            let margin = egui::Margin::symmetric(super::GLASS_END as i8, ((GLASS_HEIGHT - GLASS_BUTTON) / 2.0) as i8);
            let shown = egui::Frame::new().inner_margin(margin).show(ui, |ui| {
                ui.horizontal(|ui| {
                    ui.spacing_mut().item_spacing.x = GLASS_GAP;
                    // as the one note's bar: color and size go to every selected note
                    let r = glass_button(ui, self.note_color.is_some(), Icon::PaintPalette, dark);
                    if r.clone().tip("Cor").clicked() {
                        color = Some(r.rect);
                    }
                    glass_divider(ui, dark);
                    if glass_button(ui, false, Icon::Minus, dark).tip("Diminuir tamanho da fonte").clicked() {
                        step = -NOTE_FONT_STEP;
                    }
                    let (rect, _) = ui.allocate_exact_size(Vec2::new(22.0, 24.0), Sense::hover());
                    ui.painter().text(rect.center(), egui::Align2::CENTER_CENTER, size.round().to_string(), FontId::proportional(12.0), c.text);
                    if glass_button(ui, false, Icon::Plus, dark).tip("Aumentar tamanho da fonte").clicked() {
                        step = NOTE_FONT_STEP;
                    }
                    glass_divider(ui, dark);
                    stack = glass_button(ui, false, Icon::Layers(false), dark).tip(tip).clicked().then_some(());
                });
            });
            ui.data_mut(|d| d.insert_temp(size_id, shown.response.rect.size()));
        });
        if let Some(anchor) = color {
            self.note_color = if self.note_color.is_some() { None } else { Some((anchor.left_bottom() + Vec2::new(0.0, 6.0), Rect::NOTHING)) };
        }
        if step != 0.0 {
            self.notes_font_step(&notes, step);
        }
        if stack.is_some() {
            self.stack_notes();
        }
        Some(area.response.rect)
    }

    /// Reiniciar: the shell (and its command) starts again; an unloaded one starts too.
    pub(crate) fn restart_node(&mut self, id: String) {
        if let Some(i) = self.scene.elements.iter().position(|e| e.id == id) {
            self.unload_node(i, false);
        }
        self.terminals.remove(&id);
        self.terminal_focus = Some(id);
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
        egui::Area::new(egui::Id::unique("composer")).fixed_pos(Pos2::new(x, top)).pivot(egui::Align2::CENTER_TOP).order(egui::Order::Foreground).show(
            ctx,
            |ui| {
                // a pill, like a chat box; the button stays on the last line as the text grows
                // the send button sits 4 from the top, the right and the bottom; one line of text in
                // the middle of its height
                self.island().corner_radius(22).inner_margin(egui::Margin { left: 18, right: 4, top: 4, bottom: 4 }).show(ui, |ui| {
                    ui.set_width(width - 22.0);
                    // an Area offers only last frame's size; room to grow up to the cap
                    ui.set_max_height(tallest);
                    let response = egui::ScrollArea::vertical()
                        .id_salt("composer_scroll")
                        .max_height(tallest)
                        .show(ui, |ui| {
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
                        })
                        .inner;
                    if !response.has_focus() && !response.lost_focus() {
                        response.request_focus();
                    }
                    // egui's own focus, not has_focus(): that one also asks for the window's
                    let typing = ui.memory(|m| m.has_focus(response.id));
                    send = typing
                        && ui.input(|i| {
                            i.events.iter().any(|e| matches!(e, egui::Event::Key { key: Key::Enter, pressed: true, modifiers, .. } if !modifiers.shift))
                        });
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
                    send |= button.on_hover_cursor(CursorIcon::PointingHand).tip("Enviar").clicked();
                });
            },
        );
        if close {
            self.composer = None;
            return;
        }
        if send && !text.trim().is_empty() && self.send_prompt(&id, &text) {
            text.clear();
        }
        self.composer = Some((id, text));
    }

    /// The bar's composer button: opens it under the terminal, with what was left in it, or closes it.
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
    /// (`Returned`).
    pub(crate) fn send_prompt_after(&self, id: &str, text: &str, delay: std::time::Duration) {
        if let Some(Ok(session)) = self.terminals.get(id) {
            let (handle, message, name, tx, id) =
                (session.handle.clone(), text.trim().to_string(), self.terminal_name(id), self.returned_prompts.0.clone(), id.to_string());
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
                    let _ = tx.send(Returned { id, text: message, why: format!("{name} {why}") });
                }
            });
        }
    }

    /// The prompts `send_prompt_after` could not type: into their composer when that is free or
    /// already theirs, else onto the clipboard.
    pub(crate) fn returned_prompts(&mut self, ctx: &Context) {
        let returned: Vec<Returned> = self.returned_prompts.1.try_iter().collect();
        for Returned { id, text, why } in returned {
            let on_screen = self.scene.elements.iter().any(|e| e.id == id);
            match self.composer.as_mut() {
                Some((on, draft)) if *on == id => {
                    if !draft.trim().is_empty() {
                        draft.push_str("\n\n");
                    }
                    draft.push_str(&text);
                    self.toast(format!("{why} O texto voltou para a caixa de mensagem do terminal."));
                }
                None if on_screen => {
                    self.composer = Some((id, text));
                    self.toast(format!("{why} O texto voltou para a caixa de mensagem do terminal."));
                }
                _ => {
                    ctx.copy_text(text);
                    self.toast(format!("{why} O texto foi copiado: cole e envie de novo."));
                }
            }
        }
    }

    /// A terminal's name as people see it ("O terminal" when it has none).
    pub(crate) fn terminal_name(&self, id: &str) -> String {
        let spec = self.scene.elements.iter().find(|e| e.id == id).and_then(|e| e.terminal.as_ref());
        spec.map(|t| t.name.trim().to_string()).filter(|n| !n.is_empty()).unwrap_or_else(|| "O terminal".into())
    }

    /// The note bar's color popover: the ten papers, and a custom one through the color picker
    /// (inline, ui/hsv.rs). A press outside it, Esc or a pick closes it.
    pub(crate) fn note_color_ui(&mut self, ctx: &Context) {
        let Some((at, last)) = self.note_color else { return };
        // the focused note, or every loose note of a selection of several (the notes bar)
        let id = self.active_node().filter(|e| e.created.is_some()).map(|e| e.id.clone());
        let notes = match id.and_then(|id| self.scene.elements.iter().position(|e| e.id == id)) {
            Some(i) => vec![i],
            None => self.loose_selected_notes(),
        };
        let Some(&i) = notes.first() else {
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
                ui.set_width(160.0);
                dialog_caption(ui, "Cores predefinidas", &c);
                ui.add_space(4.0);
                ui.horizontal_wrapped(|ui| {
                    ui.spacing_mut().item_spacing = Vec2::splat(6.0);
                    for (paper, name) in NOTE_PAPERS {
                        if swatch(ui, paper, fill == paper, 22.0, dark).tip(name).clicked() {
                            picked = Some(paper);
                        }
                    }
                });
                ui.add_space(8.0);
                dialog_caption(ui, "Cor personalizada", &c);
                // inline: the egui button's popup is a window of its own, so a press in it counted as
                // "outside" and closed this one. The Hsva stays in memory (a gray would lose its hue).
                let key = egui::Id::unique("note_hsva");
                let mut hsva = ui
                    .data(|d| d.get_temp::<(egui::ecolor::Hsva, [u8; 4])>(key))
                    .filter(|(_, last)| *last == custom)
                    .map_or_else(|| egui::ecolor::Hsva::from(rgba(fill)), |(h, _)| h);
                if super::hsv::hsv_picker(ui, &mut hsva, 160.0) {
                    custom = hsva.to_srgba_unmultiplied();
                    ui.data_mut(|d| d.insert_temp(key, (hsva, custom)));
                }
            });
        });
        self.note_color = Some((at, area.response.rect));
        let chosen = picked.or((custom != fill).then_some(custom));
        if let Some(paper) = chosen {
            self.snapshot();
            for &n in &notes {
                self.scene.elements[n].style.fill = Some(paper);
            }
        }
        if picked.is_some() {
            self.note_color = None;
        }
    }
}
