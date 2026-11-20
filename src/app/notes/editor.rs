//! A note's text being typed over its card.

use crate::prelude::*;

/// A selection in a note: a blue behind the letters, which keep their ink, on every paper (egui 0.37 paints the
/// selected letters in the theme's selection color, and its own blue is the dark theme's: dark on the paper).
const SELECTION: Color32 = Color32::from_rgba_unmultiplied_const(30, 120, 255, 90);

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
            // size, and the scroll area inside it never grew past it and its scroll bar in the note's right margin,
            // where the resting note's is: the text keeps the box's width
            let margin = NOTE_PAD * zoom;

            ui.spacing_mut().scroll.floating = true;
            ui.spacing_mut().scroll.floating_allocated_width = margin;
            ui.set_width(screen.width() + margin);
            ui.set_height(screen.height());

            let shown = scroll.show(ui, |ui| {
                if let Some(Element { kind: ElementKind::Text { value, .. }, .. }) = self.scene.elements.get_mut(index) {
                    // only what the caret is on shows its markdown (the reference's live preview): the rest of the note
                    // stays as it rests
                    let id = crate::app::notes::editor_id();
                    let caret_of = |range: egui::text::CCursorRange| markdown::Caret { head: range.primary.index.into(), anchor: range.secondary.index.into() };

                    let caret = ui
                        .memory(|memory| memory.has_focus(id))
                        .then(|| egui::TextEdit::load_state(ui.ctx(), id).and_then(|state| state.cursor.char_range()).map(caret_of))
                        .flatten();

                    // the last text laid out, kept for the code's boxes and the ornaments painted with it
                    let mut laid: Option<std::sync::Arc<markdown::LiveText>> = None;
                    let bold = crate::app::ui::note_bold(ui.ctx());
                    // the text the caret was read on: changed in this frame, the caret is where the change ended
                    let before = caret.map(|_| value.clone());

                    let mut layouter = |ui: &egui::Ui, text: &dyn egui::TextBuffer, wrap: f32| {
                        let caret = match &before {
                            Some(before) if before != text.as_str() => Some(markdown::caret_after_edit(before, text.as_str())),
                            _ => caret,
                        };

                        let live = live_text_of(ui.ctx(), text.as_str(), size, ink, wrap, caret, &bold);
                        let galley = ui.ctx().fonts_mut(|fonts| fonts.layout_job(live.job.clone()));

                        laid = Some(live);
                        galley
                    };

                    let selection = &mut ui.visuals_mut().selection;

                    (selection.bg_fill, selection.stroke.color) = (SELECTION, ink);
                    // the caret blinks in the note's ink: the dark theme's own is near white, lost on the paper
                    ui.visuals_mut().text_cursor.stroke = Stroke::new((2.0 * zoom).max(1.0), ink);
                    // the code's boxes go behind the text: their place in the painting is kept before the editor's
                    let behind = ui.painter().add(egui::Shape::Noop);

                    let output = egui::TextEdit::multiline(value)
                        .id(id)
                        .frame(egui::Frame::NONE)
                        .margin(Vec2::ZERO)
                        .text_color(ink)
                        .background_color(Color32::TRANSPARENT)
                        .layouter(&mut layouter)
                        .desired_width(screen.width())
                        .desired_rows(1)
                        // a click under the last line is the text's (its end), not the canvas's
                        .min_size(screen.size())
                        .show(ui);

                    let response = output.response.response;

                    // the code's boxes behind it, and the bullets, boxes, bars and rules over their hidden marks
                    if let Some(live) = laid.filter(|live| live.job.text == *value) {
                        let boxes = markdown::code_boxes(&output.galley, &live.code);

                        ui.painter().set(behind, egui::Shape::Vec(markdown::code_shapes(&boxes, output.galley_pos, 1.0, ink)));
                        paint_live(ui, &output.galley, output.galley_pos, screen.width(), &live, ink, zoom);
                    }

                    // the caret moved to another line: it shows its markdown on the next frame
                    if output.cursor_range.map(caret_of) != caret {
                        ui.ctx().request_repaint();
                    }

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

/// The note's text for the editor, built again only when the text, the caret, the size or the width changed: egui asks
/// for it every frame the note is on screen.
fn live_text_of(
    ctx: &Context,
    text: &str,
    size: f32,
    ink: Color32,
    wrap: f32,
    caret: Option<markdown::Caret>,
    bold: &egui::FontFamily,
) -> std::sync::Arc<markdown::LiveText> {
    let key = egui::Id::unique("note_live_text");
    let hash = egui::util::hash((text, size.to_bits(), ink, wrap.to_bits(), caret, bold));

    if let Some((_, live)) = ctx.data(|data| data.get_temp::<(u64, std::sync::Arc<markdown::LiveText>)>(key)).filter(|(at, _)| *at == hash) {
        return live;
    }

    let live = std::sync::Arc::new(markdown::live_text(text, size, ink, wrap, caret, bold));

    ctx.data_mut(|data| data.insert_temp(key, (hash, live.clone())));
    live
}

/// What the editor paints over its text, as the resting note does: a bullet, a task's box, a quote's bar or a rule
/// (across the text box's `width`) where the mark is hidden.
fn paint_live(ui: &egui::Ui, galley: &egui::Galley, at: Pos2, width: f32, live: &markdown::LiveText, ink: Color32, zoom: f32) {
    let painter = ui.painter();
    let faint = ink.gamma_multiply(0.4);

    for ornament in &live.ornaments {
        let index = match ornament {
            markdown::Ornament::Bullet(index) | markdown::Ornament::Task(index, _) | markdown::Ornament::Quote(index) | markdown::Ornament::Rule(index) => {
                *index
            }
        };

        let cell = galley.pos_from_cursor(egui::text::CCursor::new(index)).translate(at.to_vec2());

        match ornament {
            markdown::Ornament::Bullet(_) => {
                painter.circle_filled(Pos2::new(cell.left() + 4.0 * zoom, cell.center().y), 2.0 * zoom, faint);
            }
            markdown::Ornament::Task(_, done) => {
                let check = Rect::from_center_size(Pos2::new(cell.left() + 6.0 * zoom, cell.center().y), Vec2::splat(12.0 * zoom));

                if *done {
                    painter.rect_filled(check, 2.0 * zoom, ink.gamma_multiply(0.5));
                } else {
                    painter.rect_stroke(check, 2.0 * zoom, Stroke::new(zoom, ink.gamma_multiply(0.5)), StrokeKind::Inside);
                }
            }
            markdown::Ornament::Quote(_) => {
                painter.rect_filled(Rect::from_min_size(cell.left_top(), Vec2::new(2.5 * zoom, cell.height())), 0.0, ink.gamma_multiply(0.2));
            }
            markdown::Ornament::Rule(_) => {
                let y = cell.center().y;

                painter.line_segment([Pos2::new(at.x, y), Pos2::new(at.x + width, y)], Stroke::new(zoom, ink.gamma_multiply(0.2)));
            }
        }
    }
}
