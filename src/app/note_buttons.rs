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
fn button_width(b: &NoteButton) -> f32 {
    b.label.chars().count() as f32 * 6.5 + 18.0
}

/// The buttons' rects (from the area's top-left, wrapping at `avail`) and the area's height.
pub(crate) fn layout(buttons: &[NoteButton], avail: f32) -> (Vec<Rect>, f32) {
    let (mut rects, mut x, mut y) = (Vec::new(), 0.0_f32, 0.0_f32);
    for b in buttons {
        let w = button_width(b).min(avail.max(1.0));
        if x > 0.0 && x + w > avail {
            (x, y) = (0.0, y + BUTTON_H + GAP);
        }
        rects.push(Rect::from_min_size(Pos2::new(x, y), Vec2::new(w, BUTTON_H)));
        x += w + GAP;
    }
    (rects, if buttons.is_empty() { 0.0 } else { y + BUTTON_H })
}

/// What the buttons take from a note of `width`: their rows and the gap above them.
pub(crate) fn height(buttons: &[NoteButton], width: f32) -> f32 {
    if buttons.is_empty() { 0.0 } else { layout(buttons, width - 2.0 * NOTE_PAD).1 + ABOVE }
}

/// The hint of a button: the action and its arguments.
pub(crate) fn describe(b: &NoteButton) -> String {
    if b.args.is_null() { b.action.clone() } else { format!("{} {}", b.action, b.args) }
}

impl DrawingApp {
    /// Paints each visible note's buttons over the canvas and handles their clicks.
    pub(crate) fn note_buttons_ui(&mut self, ctx: &Context) {
        let (mut run, mut remove) = (None, None);
        for i in 0..self.scene.elements.len() {
            let e = &self.scene.elements[i];
            if e.created.is_none() || e.note.buttons.is_empty() {
                continue;
            }
            let (frame, zoom) = self.terminal_frame(e);
            let b = local_bounds(e);
            let (rects, h) = layout(&e.note.buttons, b.width() - 2.0 * NOTE_PAD);
            let origin = frame.min + Vec2::new(NOTE_PAD, b.height() - NOTE_PAD - h) * zoom;
            if !self.canvas_rect.intersects(Rect::from_min_size(origin, Vec2::new(b.width(), h) * zoom)) {
                continue;
            }
            let (buttons, id) = (e.note.buttons.clone(), e.id.clone());
            let (_, ink) = note_colors(e, e.style.opacity);
            egui::Area::new(egui::Id::unique(("note_buttons", &id))).order(egui::Order::Foreground).fixed_pos(origin).show(ctx, |ui| {
                for (k, (button, rect)) in buttons.iter().zip(&rects).enumerate() {
                    let r = Rect::from_min_size(origin + rect.min.to_vec2() * zoom, rect.size() * zoom);
                    let response = ui.interact(r, egui::Id::unique(("note_button", &id, k)), Sense::click());
                    let response = response.on_hover_cursor(CursorIcon::PointingHand).tip(describe(button));
                    let fill = ink.gamma_multiply(if response.hovered() { 0.22 } else { 0.12 });
                    ui.painter().rect(r, 10.0 * zoom, fill, Stroke::new(zoom, ink.gamma_multiply(0.35)), StrokeKind::Inside);
                    ui.painter().with_clip_rect(r).text(r.center(), egui::Align2::CENTER_CENTER, &button.label, FontId::proportional(11.0 * zoom), ink);
                    if response.clicked() {
                        run = Some((i, k));
                    }
                    response.context_menu(|ui| {
                        if ui.button("Remover botão").clicked() {
                            remove = Some((i, k));
                            ui.close();
                        }
                    });
                }
            });
        }
        if let Some((note, k)) = remove {
            self.remove_note_button(note, k);
        } else if let Some((note, k)) = run {
            self.run_note_button(note, k);
        }
    }

    /// Right-click, Remover botão.
    pub(crate) fn remove_note_button(&mut self, note: usize, k: usize) {
        if self.scene.elements.get(note).is_some_and(|e| k < e.note.buttons.len()) {
            self.snapshot();
            self.scene.elements[note].note.buttons.remove(k);
        }
    }

    /// Runs button `k` of note `note`: its action by the same dispatcher the MCP uses. A failure
    /// shows its message and nothing else happens.
    pub(crate) fn run_note_button(&mut self, note: usize, k: usize) {
        let Some(button) = self.scene.elements.get(note).and_then(|e| e.note.buttons.get(k)).cloned() else { return };
        if let Err(error) = self.act(&button.action, &button.args) {
            self.toast(format!("{}: {error}", button.label));
        }
    }
}
