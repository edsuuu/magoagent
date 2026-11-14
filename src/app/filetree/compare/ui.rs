use super::band::paint_band;
use super::fold::CONTEXT;
use super::{CHANGED, Compare, DELETED, INSERTED, Item, soft};
use crate::editor::theme as t;
use crate::editor::{DiffRow, RowKind, highlight, word_marks};
use crate::prelude::*;

/// Line `index` of `lines` (empty when there is none).
fn at(lines: &[String], index: Option<usize>) -> &str {
    index.and_then(|index| lines.get(index)).map_or("", String::as_str)
}

/// The » of a change centered at `at`: its HEAD's lines into the file. True when pressed.
fn arrow(ui: &egui::Ui, id: egui::Id, at: Pos2, z: f32) -> bool {
    let rect = Rect::from_center_size(at, Vec2::splat(18.0 * z));
    let hit = ui
        .interact(rect, id, Sense::click())
        .on_hover_cursor(CursorIcon::PointingHand)
        .tip("Usar a versão do HEAD neste trecho (reverte a mudança no arquivo)");
    let painter = ui.painter().with_clip_rect(ui.clip_rect());
    if hit.hovered() {
        painter.rect_filled(rect, 4.0 * z, t::POPUP_BORDER);
    }
    painter.text(at, egui::Align2::CENTER_CENTER, "»", t::ui(15.0 * z), if hit.hovered() { t::TEXT } else { t::LINK });
    hit.clicked()
}

/// The checkbox of a change centered at `at`. True when pressed.
fn checkbox(ui: &egui::Ui, id: egui::Id, at: Pos2, on: bool, z: f32) -> bool {
    let rect = Rect::from_center_size(at, Vec2::splat(12.0 * z));
    let hit = ui.interact(rect.expand(3.0 * z), id, Sense::click()).on_hover_cursor(CursorIcon::PointingHand).tip("Marcar ou desmarcar este trecho");
    let painter = ui.painter().with_clip_rect(ui.clip_rect());
    if on {
        painter.rect_filled(rect, 2.0 * z, t::LINK);
        let stroke = Stroke::new(1.6 * z, t::ISLAND);
        let (left, bottom, right) = (rect.lerp_inside(Vec2::new(0.22, 0.52)), rect.lerp_inside(Vec2::new(0.43, 0.74)), rect.lerp_inside(Vec2::new(0.78, 0.28)));
        painter.line_segment([left, bottom], stroke);
        painter.line_segment([bottom, right], stroke);
    } else {
        painter.rect_stroke(rect, 2.0 * z, Stroke::new(1.0, if hit.hovered() { t::TEXT } else { t::SECONDARY }), egui::StrokeKind::Inside);
    }
    hit.clicked()
}

fn lighter(color: Color32) -> Color32 {
    let up = |part: u8| part.saturating_add(45);
    Color32::from_rgb(up(color.r()), up(color.g()), up(color.b()))
}

fn background(kind: RowKind) -> Color32 {
    match kind {
        RowKind::Same => Color32::TRANSPARENT,
        RowKind::Insert => INSERTED,
        RowKind::Delete => DELETED,
        RowKind::Change => soft(CHANGED),
    }
}

/// What the rows are painted with.
struct Look<'a> {
    font: egui::FontId,
    row_h: f32,
    lead: f32,
    num_w: f32,
    lang: &'static highlight::Lang,
    old: &'a [String],
    new: &'a [String],
}

impl Look<'_> {
    /// One line in `rect` (the text's room): its background, words that changed and text. Side by side its number goes in
    /// `numbers` (its own cell in the middle); in one column (`numbers` empty) both numbers are inside `rect`. `frame` has
    /// whether it is the first and the last row of the change the walk is on; `shift` slides the text sideways.
    #[allow(clippy::too_many_arguments)]
    fn line(
        &self,
        ui: &egui::Ui,
        painter: &egui::Painter,
        rect: Rect,
        numbers: Option<Rect>,
        row: DiffRow,
        partner: Option<usize>,
        frame: Option<(bool, bool)>,
        shift: f32,
    ) {
        let color = background(row.kind);
        painter.rect_filled(rect, 0, color);
        if let Some(cell) = numbers {
            painter.rect_filled(cell, 0, color);
        }
        if let Some((first, last)) = frame {
            let stroke = Stroke::new(1.0, t::LINK.gamma_multiply(0.7));
            painter.vline(rect.left() + 0.5, rect.y_range(), stroke);
            painter.vline(rect.right() - 0.5, rect.y_range(), stroke);
            if first {
                painter.hline(rect.x_range(), rect.top() + 0.5, stroke);
            }
            if last {
                painter.hline(rect.x_range(), rect.bottom() - 0.5, stroke);
            }
        }
        let number = |right: f32, line: usize| {
            painter.text(Pos2::new(right - 4.0, rect.center().y), egui::Align2::RIGHT_CENTER, (line + 1).to_string(), self.font.clone(), t::LINE_NUMBER);
        };
        match numbers {
            Some(cell) => row.left.or(row.right).into_iter().for_each(|line| number(cell.right(), line)),
            None => {
                row.left.into_iter().for_each(|line| number(rect.left() + self.lead + self.num_w, line));
                row.right.into_iter().for_each(|line| number(rect.left() + self.lead + 2.0 * self.num_w, line));
            }
        }
        // side by side the text is the columns' own text edit (edit.rs), over this background
        if numbers.is_some() {
            return;
        }
        // the words of a modified line that differ from its pair
        let marks = match (row.kind, row.left, row.right, partner) {
            (RowKind::Change, Some(line), _, Some(other)) => word_marks(at(self.old, Some(line)), at(self.new, Some(other))).0,
            (RowKind::Change, None, Some(line), Some(other)) => word_marks(at(self.old, Some(other)), at(self.new, Some(line))).1,
            _ => vec![],
        };
        let (index, from) = if row.left.is_some() { (row.left, self.old) } else { (row.right, self.new) };
        let Some(text) = index.and_then(|index| from.get(index)) else { return };
        let x = rect.left() + if numbers.is_some() { 0.0 } else { self.lead + 2.0 * self.num_w } + 4.0;
        let galley = ui.fonts_mut(|fonts| fonts.layout_job(highlight::layout(self.lang, text, self.font.clone(), self.row_h)));
        // the text slides under the numbers, which stay
        let painter = painter.with_clip_rect(rect.with_min_x(x).intersect(painter.clip_rect()));
        let x = x - shift;
        for word in marks {
            let edge = |byte: usize| galley.pos_from_cursor(egui::text::CCursor::new(text[..byte.min(text.len())].chars().count())).left();
            painter.rect_filled(Rect::from_min_max(Pos2::new(x + edge(word.start), rect.top()), Pos2::new(x + edge(word.end), rect.bottom())), 0, CHANGED);
        }
        painter.galley(Pos2::new(x, rect.top()), galley, t::EDITOR_TEXT);
    }
}

/// The compare tab in `body`; the change whose » (the HEAD's lines) was pressed, counted from 0.
/// `.1` is the file's whole text when it was typed in (the right column is editable).
pub(crate) fn compare_ui(ui: &mut egui::Ui, body: Rect, compare: &mut Compare, z: f32) -> (Option<usize>, Option<String>) {
    let font = t::mono(t::EDITOR_SIZE * z);
    let (natural, char_w) = ui.fonts_mut(|fonts| (fonts.row_height(&font), fonts.glyph_width(&font, '0')));
    let row_h = (natural * 1.2).round();
    ui.painter().with_clip_rect(body.intersect(ui.clip_rect())).rect_filled(body.shrink2(Vec2::new(1.0, 1.0)), 0, t::ISLAND);
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
    let bar = Rect::from_min_size(body.min, Vec2::new(body.width(), 28.0 * z));
    let head = Rect::from_min_size(Pos2::new(body.left(), bar.bottom()), Vec2::new(body.width(), 20.0 * z));
    let view = Rect::from_min_max(Pos2::new(body.left(), head.bottom()), Pos2::new(body.right() - 1.0, body.bottom() - 1.0));
    let small = t::ui((t::UI_SIZE - 1.0) * z);
    ui.scope_builder(egui::UiBuilder::new().max_rect(bar.shrink2(Vec2::new(8.0 * z, 0.0))), |ui| {
        ui.set_clip_rect(bar.intersect(ui.clip_rect()));
        ui.horizontal_centered(|ui| {
            let count = compare.count();
            let text = |label: &str, on: bool| egui::RichText::new(label).font(small.clone()).color(if on { t::TEXT } else { t::SECONDARY });
            let previous = ui.add_enabled(compare.cur > 0, egui::Button::new(text("Anterior", compare.cur > 0)).frame(false));
            if previous.tip("Mudança anterior (Shift+F7)").clicked() {
                compare.go(-1);
            }
            let next = ui.add_enabled(compare.cur + 1 < count, egui::Button::new(text("Próxima", compare.cur + 1 < count)).frame(false));
            if next.tip("Próxima mudança (F7)").clicked() {
                compare.go(1);
            }
            let label = if count == 0 { "sem diferenças".to_string() } else { format!("{} de {count}", compare.cur + 1) };
            ui.label(egui::RichText::new(label).font(small.clone()).color(t::MUTED));
            ui.add_space(12.0 * z);
            let side = ui.selectable_label(!compare.unified, text("Lado a lado", true)).clicked();
            let one = ui.selectable_label(compare.unified, text("Unificado", true)).clicked();
            if side || one {
                compare.set_unified(one);
            }
            let fold = ui.selectable_label(compare.collapse, text("Recolher trechos sem mudança", true));
            if fold.tip(format!("Esconde os trechos iguais longos, deixando {CONTEXT} linhas de contexto em volta de cada mudança")).clicked() {
                compare.set_collapse(!compare.collapse);
            }
        });
    });
    let painter = ui.painter().with_clip_rect(body.intersect(ui.clip_rect()));
    painter.hline(body.x_range(), bar.bottom(), Stroke::new(1.0, t::POPUP_BORDER));
    let name = compare.file.file_name().unwrap_or_default().to_string_lossy().into_owned();
    // side by side: the HEAD's text | its numbers | the strip | the file's numbers (with the checkbox) | the file's text, the
    // texts touching the numbers; in one column, the »'s gutter before the two numbers
    let unified = compare.unified;
    let (gap, lead) = (34.0 * z, if unified { 22.0 * z } else { 0.0 });
    let digits = compare.old_lines.len().max(compare.new_lines.len()).to_string().len().max(3) as f32 + 1.0;
    let num_w = digits * char_w;
    let (numbers_left, numbers_right) = (num_w, num_w + 16.0 * z);
    let text_w = (view.width() - numbers_left - numbers_right - gap) / 2.0;
    let (columns, numbers) = if unified {
        (vec![view], vec![])
    } else {
        let first = view.with_max_x(view.left() + text_w);
        let left_cell = Rect::from_x_y_ranges(first.right()..=first.right() + numbers_left, view.y_range());
        let right_cell = Rect::from_x_y_ranges(left_cell.right() + gap..=left_cell.right() + gap + numbers_right, view.y_range());
        (vec![first, view.with_min_x(right_cell.right())], vec![left_cell, right_cell])
    };
    if unified {
        painter.text(Pos2::new(head.left() + 8.0 * z, head.center().y), egui::Align2::LEFT_CENTER, format!("HEAD ↔ {name}"), small.clone(), t::MUTED);
    } else {
        painter.text(Pos2::new(head.left() + 8.0 * z, head.center().y), egui::Align2::LEFT_CENTER, "HEAD", small.clone(), t::MUTED);
        painter.text(
            Pos2::new(columns[1].left() + 8.0 * z, head.center().y),
            egui::Align2::LEFT_CENTER,
            format!("{name} (arquivo atual)"),
            small.clone(),
            t::MUTED,
        );
    }
    // vertical: rows, each column with its own first row; the other follows the one the person moves
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
    let room = if unified { view.width() - lead - 2.0 * num_w - 4.0 } else { text_w - 4.0 };
    let longest = |lines: &[String]| lines.iter().map(|line| line.chars().count()).max().unwrap_or(0) as f32 * char_w;
    let widths =
        if unified { [longest(&compare.old_lines).max(longest(&compare.new_lines)), 0.0] } else { [longest(&compare.old_lines), longest(&compare.new_lines)] };
    let sideways = widths.map(|widest| (widest + 4.0 * char_w - room).max(0.0));
    let pointer = ui.input(|input| input.pointer.hover_pos());
    if let Some(at) = pointer.filter(|at| view.contains(*at)) {
        let delta = ui.input(|input| input.smooth_scroll_delta);
        let under = if unified || at.x >= view.center().x { main } else { 0 };
        compare.hscroll[under] -= delta.x;
        if delta.y != 0.0 {
            compare.scroll_driver(under, compare.scroll[under] - delta.y / row_h, most, focal);
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
            compare.scroll_driver(main, fraction * (total - view_rows), most, focal);
        }
        let top = bar_track.top() + (bar_track.height() - thumb_h) * compare.scroll[main] / (total - view_rows).max(1.0);
        Rect::from_min_size(Pos2::new(bar_track.left() + 2.0 * z, top), Vec2::new(bar_track.width() - 3.0 * z, thumb_h))
    });
    let (scroll, shifts) = (compare.scroll, compare.hscroll);
    // side by side: each column's colored text and the lines it hides; the file's text is taken out while the edit changes it
    let (jobs, hidden) = if unified {
        (None, [vec![], vec![]])
    } else {
        (Some([compare.colored(0, &font, row_h), compare.colored(1, &font, row_h)]), [compare.hidden(0), compare.hidden(1)])
    };
    let carets = compare.caret;
    let mut new_text = std::mem::take(&mut compare.new);
    let (mut typed_in, mut follow, mut carets_now) = (false, [None, None], carets);
    let look = Look { font: font.clone(), row_h, lead, num_w, lang: compare.lang, old: &compare.old_lines, new: &compare.new_lines };
    let (items, blocks, current, unchecked) = (&compare.items, &compare.blocks, compare.cur, &compare.unchecked);
    let (mut revert, mut open, mut toggled) = (None, None, None);
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
                    let frame = current_rows.as_ref().filter(|rows| rows.contains(&index)).map(|rows| (index == rows.start, index + 1 == rows.end));
                    look.line(ui, &painter, row_rect(*area), numbers.get(column).map(|cell| row_rect(*cell)), row, partner, frame, shifts[column]);
                }
            }
        }
        // side by side the columns' text is a text edit each: the HEAD's only selects and copies, the file's is typed in
        if let Some(jobs) = &jobs {
            for column in 0..2 {
                let area = columns[column];
                let lines = if column == 0 { look.old.len() } else { look.new.len().max(new_text.matches('\n').count() + 1) };
                let size = Vec2::new((widths[column] + room + 40.0 * z).max(room), (lines + 2) as f32 * row_h);
                let origin = Pos2::new(area.left() + 4.0 - shifts[column], view.top() - scroll[column] * row_h);
                let args = (edit_ids[column], &jobs[column], compare.lang, &hidden[column], &font, row_h, origin, size, area, z, carets[column]);
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
        // the markers of the hidden stretches, over the rows they hide
        for (column, area) in columns.iter().enumerate() {
            let full = numbers.get(column).map_or(*area, |cell| area.union(*cell));
            let painter = ui.painter().with_clip_rect(full.intersect(ui.clip_rect()));
            for index in (scroll[column].floor() as usize)..items[column].len() {
                let top = top_of(column, index);
                if top >= view.bottom() {
                    break;
                }
                if let Item::Fold { key, count, .. } = items[column][index] {
                    let rect = Rect::from_min_size(Pos2::new(full.left(), top), Vec2::new(full.width(), row_h));
                    let hit = ui.interact(rect.intersect(view), ui.scope_id().with(("ft_compare_fold", key, column)), Sense::click());
                    let hit = hit.on_hover_cursor(CursorIcon::PointingHand).tip("Mostrar essas linhas");
                    painter.rect_filled(rect, 0, t::ISLAND);
                    painter.rect_filled(rect, 0, if hit.hovered() { t::POPUP_BORDER } else { t::POPUP_BORDER.gamma_multiply(0.45) });
                    let label = if count == 1 { "1 linha sem mudança".to_string() } else { format!("{count} linhas sem mudança") };
                    let ink = if hit.hovered() { t::TEXT } else { t::MUTED };
                    painter.text(rect.center(), egui::Align2::CENTER_CENTER, format!("⋯  {label}"), small.clone(), ink);
                    if hit.clicked() {
                        open = Some(key);
                    }
                }
            }
        }
        if unified {
            // the » in the gutter, on each change's first row
            for (index, block) in blocks.iter().enumerate() {
                let y = top_of(0, block.rows[0].start) + row_h / 2.0;
                if y > view.top() - row_h
                    && y < view.bottom() + row_h
                    && arrow(ui, ui.scope_id().with(("ft_compare_pick", index)), Pos2::new(view.left() + lead / 2.0, y), z)
                {
                    revert = Some(index);
                }
            }
        } else {
            let strip = Rect::from_x_y_ranges(numbers[0].right()..=numbers[1].left(), view.y_range());
            let painter = ui.painter().with_clip_rect(strip.intersect(ui.clip_rect()));
            for (index, block) in blocks.iter().enumerate() {
                let left = (top_of(0, block.rows[0].start), top_of(0, block.rows[0].end));
                let right = (top_of(1, block.rows[1].start), top_of(1, block.rows[1].end));
                if left.1.min(right.1) > view.bottom() || left.0.max(right.0) < view.top() {
                    continue;
                }
                let kind = if block.rows[0].is_empty() {
                    RowKind::Insert
                } else if block.rows[1].is_empty() {
                    RowKind::Delete
                } else {
                    RowKind::Change
                };
                let fill = background(kind);
                let edge = Stroke::new(1.0, if index == current { t::LINK } else { lighter(fill) });
                paint_band(&painter, (strip.left(), strip.right()), left, right, fill, edge);
                // the » at the left end of the strip, the checkbox on the file's numbers
                let (arrow_y, box_y) = (left.0 + row_h / 2.0, right.0 + row_h / 2.0);
                let inside = |y: f32| y > view.top() && y < view.bottom();
                if inside(arrow_y) && arrow(ui, ui.scope_id().with(("ft_compare_pick", index)), Pos2::new(strip.left() + 11.0 * z, arrow_y), z) {
                    revert = Some(index);
                }
                let at = Pos2::new(numbers[1].left() + 10.0 * z, box_y);
                if inside(box_y) && checkbox(ui, ui.scope_id().with(("ft_compare_box", index)), at, !unchecked.contains(&index), z) {
                    toggled = Some(index);
                }
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
        compare.scroll_driver(column, top, most, focal);
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
    if let Some(index) = toggled
        && !compare.unchecked.remove(&index)
    {
        compare.unchecked.insert(index);
    }
    (revert, edited)
}
