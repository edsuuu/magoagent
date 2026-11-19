use super::fold::CONTEXT;
use super::{ADDED, ADDED_WORD, Compare, Item, REMOVED, REMOVED_WORD, word_spans};
use crate::app::filetree::ui::outline;
use crate::editor::theme as t;
use crate::editor::{DiffRow, RowKind, highlight, word_marks};
use crate::prelude::*;

/// The text of the counts in the bar: lines removed and added.
const REMOVED_TEXT: Color32 = Color32::from_rgb(0xF0, 0x71, 0x78);
const ADDED_TEXT: Color32 = Color32::from_rgb(0x6A, 0xC4, 0x7F);

/// Line `index` of `lines` (empty when there is none).
fn at(lines: &[String], index: Option<usize>) -> &str {
    index.and_then(|index| lines.get(index)).map_or("", String::as_str)
}

/// A line's background: the HEAD's side of a change red, the file's green; the empty row across from a line has none.
fn background(row: DiffRow) -> Color32 {
    match (row.kind, row.left, row.right) {
        (RowKind::Same, ..) | (_, None, None) => Color32::TRANSPARENT,
        (_, Some(_), None) => REMOVED,
        _ => ADDED,
    }
}

/// The empty row across from a line only the other side has: thin diagonal lines that run on from row to row.
fn hatch(painter: &egui::Painter, rect: Rect, z: f32) {
    let painter = painter.with_clip_rect(rect.intersect(painter.clip_rect()));
    let (step, stroke) = (7.0 * z, Stroke::new(1.0, t::POPUP_BORDER));
    // each line keeps x + y on a multiple of `step`, so the rows' pieces meet
    let mut sum = ((rect.left() + rect.top()) / step).floor() * step;

    while sum < rect.right() + rect.bottom() {
        painter.line_segment([Pos2::new(sum - rect.bottom(), rect.bottom()), Pos2::new(sum - rect.top(), rect.top())], stroke);
        sum += step;
    }
}

/// The bar's and the arrow's glyphs, on lucide's 24 grid: `u`p, `d`own, `s`ide by side, `o`ne column, `f`old, `r`evert.
fn glyph(painter: &egui::Painter, center: Pos2, size: f32, stroke: Stroke, kind: char) {
    let line = |points: &[(f32, f32)], close: bool| outline(painter, center, size, points, close, stroke);
    let frame = [(3.0, 4.0), (21.0, 4.0), (21.0, 20.0), (3.0, 20.0)];

    match kind {
        'u' => line(&[(6.0, 15.0), (12.0, 9.0), (18.0, 15.0)], false),
        'd' => line(&[(6.0, 9.0), (12.0, 15.0), (18.0, 9.0)], false),
        's' => {
            line(&frame, true);
            line(&[(12.0, 4.0), (12.0, 20.0)], false);
        }
        'o' => {
            line(&frame, true);
            line(&[(7.0, 9.0), (17.0, 9.0)], false);
            line(&[(7.0, 15.0), (14.0, 15.0)], false);
        }
        'f' => {
            line(&[(7.0, 20.0), (12.0, 15.0), (17.0, 20.0)], false);
            line(&[(7.0, 4.0), (12.0, 9.0), (17.0, 4.0)], false);
        }
        _ => line(&[(9.0, 6.0), (15.0, 12.0), (9.0, 18.0)], false),
    }
}

/// A button of the bar: its glyph and its label, filled when `on` (the mode in use). True when pressed.
fn tool(ui: &mut egui::Ui, kind: char, label: &str, on: bool, enabled: bool, tip: &str, z: f32) -> bool {
    let font = t::ui(12.0 * z);
    let text_w = ui.painter().layout_no_wrap(label.into(), font.clone(), t::TEXT).size().x;
    let size = Vec2::new(31.0 * z + text_w, 24.0 * z);
    let (rect, response) = ui.allocate_exact_size(size, if enabled { Sense::click() } else { Sense::hover() });
    let hovered = enabled && response.hovered();

    let fill = if on {
        t::HOVER
    } else if hovered {
        t::ROW_HOVER
    } else {
        Color32::TRANSPARENT
    };

    ui.painter().rect_filled(rect, 6.0 * z, fill);

    let ink = if !enabled {
        t::SECONDARY.gamma_multiply(0.6)
    } else if on || hovered {
        t::TEXT
    } else {
        t::MUTED
    };

    glyph(ui.painter(), Pos2::new(rect.left() + 15.0 * z, rect.center().y), 14.0 * z, Stroke::new(1.4 * z, ink), kind);
    ui.painter().text(Pos2::new(rect.left() + 25.0 * z, rect.center().y), egui::Align2::LEFT_CENTER, label, font, ink);
    let response = if enabled { response.on_hover_cursor(CursorIcon::PointingHand) } else { response };

    response.tip(tip).clicked()
}

/// A thin upright line between the bar's groups.
fn divider(ui: &mut egui::Ui, z: f32) {
    let (rect, _) = ui.allocate_exact_size(Vec2::new(9.0 * z, 16.0 * z), Sense::hover());

    ui.painter().vline(rect.center().x, rect.y_range(), Stroke::new(1.0, t::POPUP_BORDER));
}

/// The small arrow that puts a change's HEAD lines back in the file, centered at `at`. True when pressed.
fn revert(ui: &egui::Ui, id: egui::Id, at: Pos2, z: f32) -> bool {
    let rect = Rect::from_center_size(at, Vec2::splat(16.0 * z));
    let hit = ui.interact(rect, id, Sense::click()).on_hover_cursor(CursorIcon::PointingHand).tip("Reverter este trecho (volta as linhas do HEAD ao arquivo)");
    let painter = ui.painter().with_clip_rect(ui.clip_rect());

    painter.rect(rect, 4.0 * z, if hit.hovered() { t::CONTROL_BORDER } else { t::POPUP }, Stroke::new(1.0, t::CONTROL_BORDER), egui::StrokeKind::Inside);
    glyph(&painter, rect.center(), 12.0 * z, Stroke::new(1.5 * z, if hit.hovered() { t::TEXT } else { t::MUTED }), 'r');
    hit.clicked()
}

/// What the rows are painted with.
struct Look<'a> {
    font: egui::FontId,
    row_h: f32,
    lead: f32,
    num_w: f32,
    z: f32,
    lang: &'static highlight::Lang,
    old: &'a [String],
    new: &'a [String],
}

impl Look<'_> {
    /// One line in `rect` (the text's room): its background, words that changed and text. Side by side its number goes in
    /// `numbers` (the column's own cell at its left edge); in one column (`numbers` empty) both numbers are inside `rect`.
    /// `current` marks the change the walk is on; `shift` slides the text sideways.
    #[allow(clippy::too_many_arguments)]
    fn line(&self, ui: &egui::Ui, painter: &egui::Painter, rect: Rect, numbers: Option<Rect>, row: DiffRow, partner: Option<usize>, current: bool, shift: f32) {
        let color = background(row);
        let whole = numbers.map_or(rect, |cell| cell.union(rect));

        if row.left.is_none() && row.right.is_none() {
            hatch(painter, rect, self.z);
        } else {
            painter.rect_filled(whole, 0, color);
        }

        if current {
            let edge = numbers.map_or(rect.left() + self.lead, |cell| cell.left());

            painter.rect_filled(Rect::from_x_y_ranges(edge..=edge + 2.0 * self.z, rect.y_range()), 0, crate::model::PRIMARY);
        }

        let ink = if row.kind == RowKind::Same { t::LINE_NUMBER } else { t::MUTED };

        let number = |right: f32, line: usize| {
            painter.text(Pos2::new(right, rect.center().y), egui::Align2::RIGHT_CENTER, (line + 1).to_string(), self.font.clone(), ink);
        };

        match numbers {
            Some(cell) => row.left.or(row.right).into_iter().for_each(|line| number(cell.right() - 10.0 * self.z, line)),
            None => {
                row.left.into_iter().for_each(|line| number(rect.left() + self.lead + self.num_w, line));
                row.right.into_iter().for_each(|line| number(rect.left() + self.lead + 2.0 * self.num_w, line));
            }
        }

        // the words of a modified line that differ from its pair, as spans as tall as the row (word_spans)
        let marks = match (row.kind, row.left, row.right, partner) {
            (RowKind::Change, Some(line), _, Some(other)) => word_marks(at(self.old, Some(line)), at(self.new, Some(other))).0,
            (RowKind::Change, None, Some(line), Some(other)) => word_marks(at(self.old, Some(other)), at(self.new, Some(line))).1,
            _ => vec![],
        };

        let (index, from, word) = if row.left.is_some() { (row.left, self.old, REMOVED_WORD) } else { (row.right, self.new, ADDED_WORD) };
        let Some(text) = index.and_then(|index| from.get(index)) else { return };
        let spans = word_spans(text, marks);
        // side by side the text is the columns' own text edit (edit.rs, its first row at the room's left + 4), over this
        // background: only the spans go here
        let side_by_side = numbers.is_some();

        if side_by_side && spans.is_empty() {
            return;
        }

        let left = if side_by_side { rect.left() } else { rect.left() + self.lead + 2.0 * self.num_w + 12.0 * self.z };
        let x = left + if side_by_side { 4.0 } else { 0.0 } - shift;
        let galley = ui.fonts_mut(|fonts| fonts.layout_job(highlight::layout(self.lang, text, self.font.clone(), self.row_h)));
        // the text slides under the numbers, which stay
        let painter = painter.with_clip_rect(rect.with_min_x(left).intersect(painter.clip_rect()));
        let edge = |byte: usize| galley.pos_from_cursor(egui::text::CCursor::new(text[..byte.min(text.len())].chars().count())).left();

        for span in spans {
            painter.rect_filled(Rect::from_x_y_ranges(x + edge(span.start)..=x + edge(span.end), rect.y_range()), 2.0 * self.z, word);
        }

        if !side_by_side {
            painter.galley(Pos2::new(x, rect.top()), galley, t::EDITOR_TEXT);
        }
    }
}

/// The compare tab in `body`; the change whose arrow (the HEAD's lines) was pressed, counted from 0.
/// `.1` is the file's whole text when it was typed in (the right column is editable).
pub(crate) fn compare_ui(ui: &mut egui::Ui, body: Rect, compare: &mut Compare, z: f32) -> (Option<usize>, Option<String>) {
    let font = t::mono(t::EDITOR_SIZE * z);
    let (natural, char_w) = ui.fonts_mut(|fonts| (fonts.row_height(&font), fonts.glyph_width(&font, '0')));
    let row_h = (natural * 1.2).round();
    let clip = body.intersect(ui.clip_rect());

    ui.painter().with_clip_rect(clip).rect_filled(body.shrink2(Vec2::new(1.0, 1.0)), 0, t::ISLAND);
    // F7 / Shift+F7 walk the changes (with no field taking the keys)
    // ponytail: two nodes showing a compare tab at once both take the key; gate on the node under the pointer if that bites
    let file_key = compare.file.to_string_lossy().into_owned();
    let edit_ids = [0, 1].map(|column| ui.scope_id().with(("ft_compare_edit", file_key.as_str(), column)));

    if !ui.ctx().egui_wants_keyboard_input() || ui.memory(|memory| edit_ids.iter().any(|id| memory.has_focus(*id))) {
        let key = |modifiers: Modifiers, key: Key| ui.input_mut(|input| input.consume_key(modifiers, key));

        if key(Modifiers::SHIFT, Key::F7) || key(Modifiers::ALT, Key::ArrowUp) {
            compare.go(-1);
        } else if key(Modifiers::NONE, Key::F7) || key(Modifiers::ALT, Key::ArrowDown) {
            compare.go(1);
        }
    }

    let bar = Rect::from_min_size(body.min, Vec2::new(body.width(), 36.0 * z));
    let head = Rect::from_min_size(Pos2::new(body.left(), bar.bottom()), Vec2::new(body.width(), 26.0 * z));
    let view = Rect::from_min_max(Pos2::new(body.left(), head.bottom()), Pos2::new(body.right() - 1.0, body.bottom() - 1.0));
    let small = t::ui(12.0 * z);

    ui.scope_builder(egui::UiBuilder::new().max_rect(bar.shrink2(Vec2::new(8.0 * z, 0.0))), |ui| {
        ui.set_clip_rect(bar.intersect(ui.clip_rect()));
        ui.spacing_mut().item_spacing.x = 2.0 * z;

        ui.horizontal_centered(|ui| {
            let count = compare.count();

            if tool(ui, 'u', "Anterior", false, compare.cur > 0, "Mudança anterior (Shift+F7)", z) {
                compare.go(-1);
            }

            if tool(ui, 'd', "Próxima", false, compare.cur + 1 < count, "Próxima mudança (F7)", z) {
                compare.go(1);
            }

            ui.add_space(6.0 * z);
            let label = if count == 0 { "Sem diferenças".to_string() } else { format!("Mudança {} de {count}", compare.cur + 1) };

            ui.label(egui::RichText::new(label).font(small.clone()).color(t::MUTED));
            ui.add_space(6.0 * z);
            divider(ui, z);
            let side = tool(ui, 's', "Lado a lado", !compare.unified, true, "As duas versões em colunas, linha com linha", z);
            let one = tool(ui, 'o', "Unificado", compare.unified, true, "As duas versões numa coluna só", z);

            if side || one {
                compare.set_unified(one);
            }

            divider(ui, z);
            let tip = format!("Esconde os trechos iguais longos, deixando {CONTEXT} linhas de contexto em volta de cada mudança");

            if tool(ui, 'f', "Recolher iguais", compare.collapse, true, &tip, z) {
                compare.set_collapse(!compare.collapse);
            }

            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                let (removed, added) = compare.totals();

                ui.label(egui::RichText::new(format!("−{removed}")).font(small.clone()).color(REMOVED_TEXT));
                ui.add_space(4.0 * z);
                ui.label(egui::RichText::new(format!("+{added}")).font(small.clone()).color(ADDED_TEXT));
            });
        });
    });

    let painter = ui.painter().with_clip_rect(clip);

    painter.hline(body.x_range(), bar.bottom(), Stroke::new(1.0, t::POPUP_BORDER));
    painter.rect_filled(head, 0, t::INLINE);
    painter.hline(body.x_range(), head.bottom(), Stroke::new(1.0, t::POPUP_BORDER));
    let name = compare.file.file_name().unwrap_or_default().to_string_lossy().into_owned();
    // side by side: each column is its numbers (at its left edge) and its text; a narrow strip between them marks the
    // changes. In one column, the arrows' gutter before the two numbers
    let unified = compare.unified;
    let (gap, lead) = (24.0 * z, if unified { 22.0 * z } else { 0.0 });
    let digits = compare.old_lines.len().max(compare.new_lines.len()).to_string().len().max(2) as f32 + 1.0;
    let num_w = digits * char_w;
    let half = (view.width() - gap) / 2.0;

    let (columns, numbers) = if unified {
        (vec![view], vec![])
    } else {
        let cells = [view.left(), view.left() + half + gap].map(|left| Rect::from_x_y_ranges(left..=left + num_w + 22.0 * z, view.y_range()));
        let texts = [cells[0].right()..=view.left() + half, cells[1].right()..=view.right()].map(|range| Rect::from_x_y_ranges(range, view.y_range()));

        (texts.to_vec(), cells.to_vec())
    };

    // the headers: which version each column is
    let title = |at: Pos2, parts: &[(&str, egui::FontId, Color32)]| {
        parts
            .iter()
            .fold(at.x, |x, (text, font, color)| painter.text(Pos2::new(x, at.y), egui::Align2::LEFT_CENTER, *text, font.clone(), *color).right() + 8.0 * z)
    };

    let hash = t::mono(11.5 * z);
    let head_parts = [("HEAD", small.clone(), t::TEXT), (compare.head.as_str(), hash.clone(), t::SECONDARY)];
    let file_parts = [(name.as_str(), small.clone(), t::TEXT), ("arquivo atual", small.clone(), t::SECONDARY)];

    if unified {
        let x = title(Pos2::new(head.left() + 12.0 * z, head.center().y), &head_parts);
        let x = title(Pos2::new(x, head.center().y), &[("→", small.clone(), t::SECONDARY)]);

        title(Pos2::new(x, head.center().y), &file_parts);
    } else {
        title(Pos2::new(numbers[0].left() + 12.0 * z, head.center().y), &head_parts);
        title(Pos2::new(numbers[1].left() + 12.0 * z, head.center().y), &file_parts);
        let middle = (columns[0].right() + numbers[1].left()) / 2.0;

        painter.vline(middle, head.y_range(), Stroke::new(1.0, t::POPUP_BORDER));
    }

    // vertical: rows, both columns on the same one side by side
    let (view_rows, focal) = (view.height() / row_h, view.height() / 3.0 / row_h);
    let most = [0, 1].map(|column| (compare.items[column].len() as f32 - view_rows).max(0.0));
    let main = if unified { 0 } else { 1 };

    if let Some(change) = compare.scroll_to.take() {
        for column in 0..2 {
            let start = compare.blocks.get(change).map_or(0, |block| block.rows[column].start);

            compare.scroll[column] = (start as f32 - focal).clamp(0.0, most[column]);
        }
    }

    for column in 0..2 {
        compare.scroll[column] = compare.scroll[column].clamp(0.0, most[column]);
    }

    // sideways, each column on its own: its longest line against the room it leaves for its text
    let room = if unified { view.width() - lead - 2.0 * num_w - 12.0 * z } else { columns[0].width() - 4.0 };
    let longest = |lines: &[String]| lines.iter().map(|line| line.chars().count()).max().unwrap_or(0) as f32 * char_w;

    let widths =
        if unified { [longest(&compare.old_lines).max(longest(&compare.new_lines)), 0.0] } else { [longest(&compare.old_lines), longest(&compare.new_lines)] };

    let sideways = widths.map(|widest| (widest + 4.0 * char_w - room).max(0.0));
    let pointer = ui.input(|input| input.pointer.hover_pos()).filter(|at| view.contains(*at));

    if let Some(at) = pointer {
        let delta = ui.input(|input| input.smooth_scroll_delta);
        let under = if unified || at.x >= view.center().x { main } else { 0 };

        compare.hscroll[under] -= delta.x;

        if delta.y != 0.0 {
            compare.scroll_driver(under, compare.scroll[under] - delta.y / row_h, most);
            ui.input_mut(|input| input.smooth_scroll_delta.y = 0.0);
        }
    }

    let tracks: Vec<Rect> =
        columns.iter().map(|area| Rect::from_min_max(Pos2::new(area.left(), view.bottom() - 8.0 * z), Pos2::new(area.right(), view.bottom()))).collect();

    for (column, track) in tracks.iter().enumerate() {
        if sideways[column] > 0.0 {
            let thumb_w = (track.width() * room / (room + sideways[column])).max(24.0 * z);
            let response = ui.interact(*track, ui.scope_id().with(("ft_compare_h", file_key.as_str(), column)), Sense::click_and_drag());

            if let Some(at) = response.interact_pointer_pos().filter(|_| response.dragged() || response.clicked()) {
                compare.hscroll[column] = (at.x - track.left() - thumb_w / 2.0) / (track.width() - thumb_w) * sideways[column];
            }
        }

        compare.hscroll[column] = compare.hscroll[column].clamp(0.0, sideways[column]);
    }

    // the vertical bar, over the right edge of the last column
    let total = compare.items[main].len() as f32;

    let thumb = (total > view_rows).then(|| {
        let bar_track = Rect::from_min_max(Pos2::new(view.right() - 8.0 * z, view.top()), Pos2::new(view.right(), view.bottom() - 8.0 * z));
        let thumb_h = (bar_track.height() * view_rows / total).max(24.0 * z);
        let response = ui.interact(bar_track, ui.scope_id().with(("ft_compare_v", file_key.as_str())), Sense::click_and_drag());

        if let Some(at) = response.interact_pointer_pos().filter(|_| response.dragged() || response.clicked()) {
            let fraction = ((at.y - bar_track.top() - thumb_h / 2.0) / (bar_track.height() - thumb_h)).clamp(0.0, 1.0);

            compare.scroll_driver(main, fraction * (total - view_rows), most);
        }

        let top = bar_track.top() + (bar_track.height() - thumb_h) * compare.scroll[main] / (total - view_rows).max(1.0);

        Rect::from_min_size(Pos2::new(bar_track.left() + 2.0 * z, top), Vec2::new(bar_track.width() - 3.0 * z, thumb_h))
    });

    let (scroll, shifts) = (compare.scroll, compare.hscroll);

    // side by side: each column's colored text and the row of each of its lines; the file's text is taken out while the edit changes it
    let (jobs, line_rows) = if unified {
        (None, [vec![], vec![]])
    } else {
        (Some([compare.colored(0, &font, row_h), compare.colored(1, &font, row_h)]), [compare.line_rows(0), compare.line_rows(1)])
    };

    let carets = compare.caret;
    let mut new_text = std::mem::take(&mut compare.new);
    let (mut typed_in, mut follow, mut carets_now) = (false, [None, None], carets);
    let look = Look { font: font.clone(), row_h, lead, num_w, z, lang: compare.lang, old: &compare.old_lines, new: &compare.new_lines };
    let (items, blocks, current) = (&compare.items, &compare.blocks, compare.cur);
    let (mut revert_pick, mut open) = (None, None);
    let top_of = |column: usize, row: usize| view.top() + (row as f32 - scroll[column]) * row_h;

    ui.scope_builder(egui::UiBuilder::new().max_rect(view), |ui| {
        ui.set_clip_rect(view.intersect(ui.clip_rect()));

        for (column, area) in columns.iter().enumerate() {
            let full = numbers.get(column).map_or(*area, |cell| area.union(*cell));
            let painter = ui.painter().with_clip_rect(full.intersect(ui.clip_rect()));
            let current_rows = blocks.get(current).map(|block| block.rows[column].clone());

            for index in (scroll[column].floor() as usize)..items[column].len() {
                let top = top_of(column, index);

                if top >= view.bottom() {
                    break;
                }

                let row_rect = |rect: Rect| Rect::from_min_size(Pos2::new(rect.left(), top), Vec2::new(rect.width(), row_h));

                if let Item::Line { row, partner } = items[column][index] {
                    let on = current_rows.as_ref().is_some_and(|rows| rows.contains(&index));

                    look.line(ui, &painter, row_rect(*area), numbers.get(column).map(|cell| row_rect(*cell)), row, partner, on, shifts[column]);
                }
            }
        }

        // side by side the columns' text is a text edit each: the HEAD's only selects and copies, the file's is typed in
        if let Some(jobs) = &jobs {
            for column in 0..2 {
                let area = columns[column];
                let lines = if column == 0 { look.old.len() } else { look.new.len().max(new_text.matches('\n').count() + 1) };
                let size = Vec2::new((widths[column] + room + 40.0 * z).max(room), (items[column].len().max(lines) + 2) as f32 * row_h);
                let origin = Pos2::new(area.left() + 4.0 - shifts[column], view.top() - scroll[column] * row_h);
                let args = (edit_ids[column], &jobs[column], compare.lang, &line_rows[column], &font, row_h, origin, size, area, z, carets[column]);

                let (typed, caret) = if column == 0 {
                    let mut old_text: &str = &compare.old;

                    super::edit::column(ui, args.0, &mut old_text, args.1, args.2, args.3, args.4, args.5, args.6, args.7, args.8, args.9, args.10)
                } else {
                    super::edit::column(ui, args.0, &mut new_text, args.1, args.2, args.3, args.4, args.5, args.6, args.7, args.8, args.9, args.10)
                };

                typed_in |= typed.changed;
                carets_now[column] = caret;
                follow[column] = typed.moved_caret.filter(|_| typed.focused);
            }
        }

        // the markers of the hidden stretches, over the rows they hide, across the whole width (both columns hide the same)
        let painter = ui.painter().with_clip_rect(view.intersect(ui.clip_rect()));

        for index in (scroll[0].floor() as usize)..items[0].len() {
            let top = top_of(0, index);

            if top >= view.bottom() {
                break;
            }

            if let Item::Fold { key, count, .. } = items[0][index] {
                let rect = Rect::from_x_y_ranges(view.x_range(), top..=top + row_h);
                let hit = ui.interact(rect.intersect(view), ui.scope_id().with(("ft_compare_fold", key)), Sense::click());
                let hit = hit.on_hover_cursor(CursorIcon::PointingHand).tip("Mostrar essas linhas");

                painter.rect_filled(rect, 0, if hit.hovered() { t::POPUP_BORDER } else { t::INLINE });
                let label = if count == 1 { "1 linha sem mudança".to_string() } else { format!("{count} linhas sem mudança") };
                let ink = if hit.hovered() { t::TEXT } else { t::MUTED };

                painter.text(rect.center(), egui::Align2::CENTER_CENTER, format!("⋯  {label}"), small.clone(), ink);

                if hit.clicked() {
                    open = Some(key);
                }
            }
        }

        // the strip between the columns (in one column, the gutter before the numbers): each change a straight band, red
        // where the HEAD lost lines and green where the file gained them, with its arrow on hover or on the current change
        let strip = if unified {
            Rect::from_x_y_ranges(view.left()..=view.left() + lead, view.y_range())
        } else {
            Rect::from_x_y_ranges(columns[0].right()..=numbers[1].left(), view.y_range())
        };

        let painter = ui.painter().with_clip_rect(strip.intersect(ui.clip_rect()));

        if !unified {
            painter.vline(strip.left() + 0.5, strip.y_range(), Stroke::new(1.0, t::POPUP_BORDER));
            painter.vline(strip.right() - 0.5, strip.y_range(), Stroke::new(1.0, t::POPUP_BORDER));
        }

        for (index, block) in blocks.iter().enumerate() {
            let (top, bottom) = (top_of(0, block.rows[0].start), top_of(0, block.rows[0].end.max(block.rows[0].start + 1)));

            if bottom < view.top() || top > view.bottom() {
                continue;
            }

            let has = |column: usize, side: fn(&DiffRow) -> Option<usize>| {
                items[column][block.rows[column].clone()].iter().any(|item| matches!(item, Item::Line { row, .. } if side(row).is_some()))
            };

            let (removed, added) = if unified { (has(0, |row| row.left), has(0, |row| row.right)) } else { (has(0, |row| row.left), has(1, |row| row.right)) };
            let band = Rect::from_x_y_ranges(strip.x_range(), top..=bottom).shrink2(Vec2::new(if unified { 4.0 * z } else { 1.0 }, 0.0));
            let (left_half, right_half) = band.split_left_right_at_fraction(0.5);

            painter.rect_filled(left_half, 0, if removed { REMOVED } else { ADDED });
            painter.rect_filled(right_half, 0, if added { ADDED } else { REMOVED });
            let over = pointer.is_some_and(|at| at.y >= top && at.y < bottom);
            let at = Pos2::new(strip.center().x, top + row_h / 2.0);

            if (over || index == current) && view.contains(at) && revert(ui, ui.scope_id().with(("ft_compare_pick", index)), at, z) {
                revert_pick = Some(index);
            }
        }
    });

    compare.new = new_text;
    compare.caret = carets_now;
    let mut edited = None;

    if typed_in {
        edited = Some(compare.new.clone());
        compare.rebuild();
        ui.ctx().request_repaint();
    }

    // the caret that moved is kept in view
    for (column, rect) in follow.iter().enumerate() {
        let Some(rect) = rect else { continue };
        let row = rect.min.y / row_h;

        let top = if row < compare.scroll[column] {
            row
        } else if row + 1.0 > compare.scroll[column] + view_rows {
            row + 1.0 - view_rows
        } else {
            compare.scroll[column]
        };

        compare.scroll_driver(column, top, most);
        let x = rect.min.x;
        let hscroll = &mut compare.hscroll[column];

        if x < *hscroll {
            *hscroll = (x - 4.0 * char_w).max(0.0)
        } else if x > *hscroll + room - 2.0 * char_w {
            *hscroll = x - room + 4.0 * char_w
        }
    }

    for (column, track) in tracks.iter().enumerate().filter(|(column, _)| sideways[*column] > 0.0) {
        let thumb_w = (track.width() * room / (room + sideways[column])).max(24.0 * z);
        let left = track.left() + (track.width() - thumb_w) * shifts[column] / sideways[column];
        let rect = Rect::from_min_size(Pos2::new(left, track.top() + 2.0 * z), Vec2::new(thumb_w, track.height() - 3.0 * z));

        ui.painter().with_clip_rect(view.intersect(ui.clip_rect())).rect_filled(rect, 3.0 * z, t::SECONDARY.gamma_multiply(0.45));
    }

    if let Some(rect) = thumb {
        ui.painter().with_clip_rect(view.intersect(ui.clip_rect())).rect_filled(rect, 3.0 * z, t::SECONDARY.gamma_multiply(0.45));
    }

    if let Some(key) = open {
        compare.open_fold(key);
    }

    (revert_pick, edited)
}
