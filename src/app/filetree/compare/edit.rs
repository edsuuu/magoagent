use crate::editor::highlight;
use crate::editor::text::hash;
use crate::editor::theme as t;
use crate::prelude::*;
use egui::text::LayoutJob;
use std::sync::Arc;

/// Puts the galley's rows on the grid the diff draws: line `n` at display row `n`, less the lines hidden before it. The
/// first line of each hidden stretch keeps one row (the "N linhas sem mudança" marker is painted over it); the others sit
/// on it, without glyphs. `hidden` is (first line, count), in order.
pub(super) fn squeeze(galley: &mut egui::Galley, hidden: &[(usize, usize)], row_h: f32) {
    let mut removed = 0;
    let mut next = 0;
    let mut last = 0.0;
    for (line, row) in galley.rows.iter_mut().enumerate() {
        while next < hidden.len() && line >= hidden[next].0 + hidden[next].1 {
            removed += hidden[next].1 - 1;
            next += 1;
        }
        let inside = next < hidden.len() && line >= hidden[next].0;
        let display = if inside { hidden[next].0 - removed } else { line - removed };
        row.pos.y = display as f32 * row_h;
        last = row.pos.y + row_h;
        if inside {
            let row = Arc::make_mut(&mut row.row);
            row.visuals = Default::default();
        }
    }
    galley.rect.max.y = last;
    galley.mesh_bounds.max.y = last;
}

/// What a column's text edit gave back.
pub(super) struct Typed {
    pub(super) changed: bool,
    /// The caret, in points from the text's origin, when it moved this frame.
    pub(super) moved_caret: Option<Rect>,
    pub(super) focused: bool,
}

/// One column's text, selectable (and, when `text` can be changed, editable) by the code editor's own text edit: the same
/// highlight, selection color and caret, over the rows the diff laid out. `origin` is where the text's first row goes;
/// `clip` what shows of it.
#[allow(clippy::too_many_arguments)]
pub(super) fn column(
    ui: &mut egui::Ui,
    id: egui::Id,
    text: &mut dyn egui::TextBuffer,
    job: &(u64, LayoutJob),
    lang: &'static highlight::Lang,
    hidden: &[(usize, usize)],
    font: &egui::FontId,
    row_h: f32,
    origin: Pos2,
    size: Vec2,
    clip: Rect,
    z: f32,
    previous_caret: Option<usize>,
) -> (Typed, Option<usize>) {
    let mut child = ui.new_child(egui::UiBuilder::new().max_rect(Rect::from_min_size(origin, size)).layout(egui::Layout::top_down(egui::Align::Min)));
    child.set_clip_rect(clip.intersect(ui.clip_rect()));
    child.visuals_mut().selection.bg_fill = t::EDITOR_SELECTION;
    child.visuals_mut().selection.stroke = Stroke::new(1.0, t::TEXT);
    child.visuals_mut().text_cursor.stroke = Stroke::new((2.0 * z).max(1.0), t::CARET);
    let mut layouter = |ui: &egui::Ui, buffer: &dyn egui::TextBuffer, _width: f32| {
        // the colored job is of the text before this frame's typing: a text that moved on is colored on its own
        let mut made = if hash(buffer.as_str(), font.size) == job.0 { job.1.clone() } else { highlight::layout(lang, buffer.as_str(), font.clone(), row_h) };
        made.wrap.max_width = f32::INFINITY;
        let mut galley = ui.fonts_mut(|fonts| fonts.layout_job(made));
        squeeze(Arc::make_mut(&mut galley), hidden, row_h);
        galley
    };
    let out = egui::TextEdit::multiline(text)
        .id(id)
        .frame(egui::Frame::NONE)
        .margin(egui::Margin::ZERO)
        .font(font.clone())
        .layouter(&mut layouter)
        .lock_focus(true)
        .desired_width(size.x)
        .desired_rows(1)
        .show(&mut child);
    let caret = out.state.cursor.char_range().map(|range| range.primary.index.0);
    let moved_caret = (caret != previous_caret).then(|| caret.map(|at| out.galley.pos_from_cursor(egui::text::CCursor::new(at)))).flatten();
    let focused = out.response.has_focus();
    (Typed { changed: out.response.changed(), moved_caret, focused }, caret)
}
