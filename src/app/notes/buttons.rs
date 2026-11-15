//! The action buttons at the foot of a note: small rounded buttons in the paper's ink, wrapping
//! onto rows. A click runs the button's action through the MCP's dispatcher (`act`), never on
//! load; a failure goes to the app's toast.

use crate::prelude::*;

const BUTTON_H: f32 = 20.0;
const GAP: f32 = 4.0;
/// The space between the text and the buttons.
const ABOVE: f32 = 6.0;

// ponytail: the width is estimated from the label's length (11 pt text), not measured; measure with the
// font if a wide-glyph label ever clips.
fn button_width(button: &NoteButton) -> f32 {
    button.label.chars().count() as f32 * 6.5 + 18.0
}

/// The buttons' rects (from the area's top-left, wrapping at `avail`) and the area's height.
pub(crate) fn layout(buttons: &[NoteButton], available: f32) -> (Vec<Rect>, f32) {
    let (mut rects, mut x, mut y) = (Vec::new(), 0.0_f32, 0.0_f32);
    for button in buttons {
        let width = button_width(button).min(available.max(1.0));
        if x > 0.0 && x + width > available {
            (x, y) = (0.0, y + BUTTON_H + GAP);
        }
        rects.push(Rect::from_min_size(Pos2::new(x, y), Vec2::new(width, BUTTON_H)));
        x += width + GAP;
    }
    (rects, if buttons.is_empty() { 0.0 } else { y + BUTTON_H })
}

/// What the buttons take from a note of `width`: their rows and the gap above them.
pub(crate) fn height(buttons: &[NoteButton], width: f32) -> f32 {
    if buttons.is_empty() { 0.0 } else { layout(buttons, width - 2.0 * NOTE_PAD).1 + ABOVE }
}

/// The hint of a button: the action and its arguments.
pub(crate) fn describe(button: &NoteButton) -> String {
    if button.args.is_null() { button.action.clone() } else { format!("{} {}", button.action, button.args) }
}

impl DrawingApp {
    /// Paints each visible note's buttons over the canvas and handles their clicks.
    pub(crate) fn note_buttons_ui(&mut self, ctx: &Context) {
        let (mut run, mut remove) = (None, None);
        for note_index in 0..self.scene.elements.len() {
            let element = &self.scene.elements[note_index];
            if element.created.is_none() || element.note.buttons.is_empty() {
                continue;
            }
            let (frame, zoom) = self.terminal_frame(element);
            let bounds = local_bounds(&self.shown_note(element));
            let (rects, height) = layout(&element.note.buttons, bounds.width() - 2.0 * NOTE_PAD);
            let origin = frame.min + Vec2::new(NOTE_PAD, bounds.height() - NOTE_PAD - height) * zoom;
            if !self.canvas_rect.intersects(Rect::from_min_size(origin, Vec2::new(bounds.width(), height) * zoom)) {
                continue;
            }
            let (buttons, id) = (element.note.buttons.clone(), element.id.clone());
            let (_, ink) = note_colors(element, element.style.opacity);
            egui::Area::new(egui::Id::unique(("note_buttons", &id))).order(egui::Order::Foreground).fixed_pos(origin).show(ctx, |ui| {
                for (button_index, (button, rect)) in buttons.iter().zip(&rects).enumerate() {
                    let button_rect = Rect::from_min_size(origin + rect.min.to_vec2() * zoom, rect.size() * zoom);
                    let response = ui.interact(button_rect, egui::Id::unique(("note_button", &id, button_index)), Sense::click());
                    let response = response.on_hover_cursor(CursorIcon::PointingHand).tip(describe(button));
                    let fill = ink.gamma_multiply(if response.hovered() { 0.22 } else { 0.12 });
                    ui.painter().rect(button_rect, 10.0 * zoom, fill, Stroke::new(zoom, ink.gamma_multiply(0.35)), StrokeKind::Inside);
                    ui.painter().with_clip_rect(button_rect).text(
                        button_rect.center(),
                        egui::Align2::CENTER_CENTER,
                        &button.label,
                        FontId::proportional(11.0 * zoom),
                        ink,
                    );
                    if response.clicked() {
                        run = Some((note_index, button_index));
                    }
                    crate::app::ui::menu_look::context_menu(&response, |ui| {
                        let danger = crate::app::ui::UiColors::new(ui.visuals().dark_mode).danger;
                        if crate::app::ui::menu_look::menu_row(ui, Some(Icon::Trash), "Remover botão", "", Some(danger)) {
                            remove = Some((note_index, button_index));
                            ui.close();
                        }
                    });
                }
            });
        }
        if let Some((note, button_index)) = remove {
            self.remove_note_button(note, button_index);
        } else if let Some((note, button_index)) = run {
            self.run_note_button(note, button_index);
        }
    }

    /// Right-click, Remover botão.
    pub(crate) fn remove_note_button(&mut self, note: usize, button_index: usize) {
        if self.scene.elements.get(note).is_some_and(|element| button_index < element.note.buttons.len()) {
            self.snapshot();
            self.scene.elements[note].note.buttons.remove(button_index);
        }
    }

    /// Runs button `k` of note `note`: its action by the same dispatcher the MCP uses. A failure
    /// shows its message and nothing else happens.
    pub(crate) fn run_note_button(&mut self, note: usize, button_index: usize) {
        let Some(button) = self.scene.elements.get(note).and_then(|element| element.note.buttons.get(button_index)).cloned() else { return };
        if !crate::mcp::app_tools::is_action(&button.action) {
            return self.toast(format!("{}: ação desconhecida ({})", button.label, button.action));
        }
        if let Err(error) = self.act(&button.action, &button.args) {
            self.toast(format!("{}: {error}", button.label));
        }
    }
}
