//! A note's text being typed over its card (asked: "a barra de edição diminui e o texto fica cortado da nota").

use crate::prelude::*;

impl DrawingApp {
    /// A note's text being typed: the card's text box, the same markdown, scrolling in place.
    pub(crate) fn note_editor(&mut self, ctx: &Context, index: usize, note: usize) {
        let shown = self.shown_note(&self.scene.elements[note]).into_owned();
        let body = note_text_box(&shown);
        let (frame, zoom) = self.terminal_frame(&self.scene.elements[note]);
        let at = |point: Pos2| frame.min + (point - local_bounds(&shown).min) * zoom;
        let screen = Rect::from_two_pos(at(body.min), at(body.max));
        let (_, ink) = note_colors(&self.scene.elements[note], 100);
        // zoom from the note's frame: an enlarged note grows its text too
        let size = self.scene.elements[note].note.font_size * zoom;
        // a popover or the rename dialog has the keys: the editor waits for them
        let want_focus = self.rename.is_none() && self.note_color.is_none();
        let mut finish = false;
        // typing starts where the resting note was scrolled to (notes/scroll.rs), not where the last note typed was
        let note_id = self.scene.elements[note].id.clone();
        let session = egui::Id::unique("note_editor_of");
        let frame_number = ctx.cumulative_frame_nr();
        let fresh = ctx.data(|data| data.get_temp::<(String, u64)>(session)).is_none_or(|(id, at)| id != note_id || at + 1 < frame_number);
        ctx.data_mut(|data| data.insert_temp(session, (note_id.clone(), frame_number)));
        let mut scroll = egui::ScrollArea::vertical().id_salt("note_text").auto_shrink(false);
        if fresh {
            scroll = scroll.vertical_scroll_offset(self.note_scroll_offset(&note_id) * zoom);
        }
        let mut scrolled = None;
        egui::Area::new(egui::Id::unique("note_editor")).fixed_pos(screen.min).constrain(false).order(egui::Order::Foreground).show(ctx, |ui| {
            // off the screen with its note, never clamped back onto it; and never over the sidebar
            ui.shrink_clip_rect(self.canvas_rect);
            // the whole text box, whatever size the area had the last time a note was typed: an area keeps its last
            // size, and the scroll area inside it never grew past it (asked: "a barra de edição diminui e o texto fica
            // cortado da nota")
            // and its scroll bar in the note's right margin, where the resting note's is: the text keeps the box's width
            let margin = NOTE_PAD * zoom;
            ui.spacing_mut().scroll.floating = true;
            ui.spacing_mut().scroll.floating_allocated_width = margin;
            ui.set_width(screen.width() + margin);
            ui.set_height(screen.height());
            let shown = scroll.show(ui, |ui| {
                if let Some(Element { kind: ElementKind::Text { value, .. }, .. }) = self.scene.elements.get_mut(index) {
                    let mut layouter = |ui: &egui::Ui, text: &dyn egui::TextBuffer, wrap: f32| {
                        let job = markdown::edit_job(text.as_str(), size, ink, wrap);
                        ui.ctx().fonts_mut(|fonts| fonts.layout_job(job))
                    };
                    let response = ui.add(
                        egui::TextEdit::multiline(value)
                            .id(crate::app::notes::editor_id())
                            .frame(egui::Frame::NONE)
                            .margin(Vec2::ZERO)
                            .text_color(ink)
                            .background_color(Color32::TRANSPARENT)
                            .layouter(&mut layouter)
                            .desired_width(screen.width())
                            .desired_rows(1)
                            // a click under the last line is the text's (its end), not the canvas's
                            .min_size(screen.size()),
                    );
                    // the resting note's placeholder, in the same letters and ink (egui's own hint_text takes the theme's
                    // weak color, near white on the paper)
                    if value.is_empty() {
                        let format = egui::TextFormat { font_id: FontId::monospace(size), color: ink.gamma_multiply(0.4), italics: true, ..Default::default() };
                        let galley = ui.fonts_mut(|fonts| fonts.layout_job(egui::text::LayoutJob::single_section("Clique para editar…".into(), format)));
                        ui.painter().galley(response.rect.min, galley, ink);
                    }
                    // the click that focuses the note releases on the next frame and counts as a
                    // click elsewhere, which takes the focus back: keep asking for it while editing
                    if want_focus && !ui.memory(|memory| memory.has_focus(response.id)) {
                        response.request_focus();
                    } else if let Some(range) = egui::TextEdit::load_state(ui.ctx(), response.id).and_then(|state| state.cursor.char_range()) {
                        let (primary, secondary) = (range.primary.index.0, range.secondary.index.0);
                        self.note_selection = Some((index, primary.min(secondary), primary.max(secondary)));
                    }
                    // the modifiers ride on the key event (scripted keys set them only there)
                    finish = ui.input(|input| {
                        input.events.iter().any(|event| {
                            matches!(event, egui::Event::Key { key: Key::Escape, pressed: true, .. })
                                || matches!(event, egui::Event::Key { key: Key::Enter, pressed: true, modifiers, .. } if modifiers.command)
                        })
                    });
                }
            });
            scrolled = Some(shown.state.clamped_offset().y);
        });
        // and the resting note goes on from where the typing left it
        if let Some(offset) = scrolled.filter(|_| zoom > 0.0) {
            self.note_scroll.borrow_mut().entry(note_id).or_default().offset = offset / zoom;
        }
        if finish {
            // a frame without the editor right away: it covers the text box, and a click there next is the note's
            ctx.request_repaint();
            // Esc or Cmd+Enter in a lifted note also puts it back on the canvas (set_elevated ends the typing)
            if self.elevated_index().is_some_and(|index| self.scene.elements[index].created.is_some()) {
                self.set_elevated(None);
            } else {
                self.finish_text();
            }
        }
    }
}
