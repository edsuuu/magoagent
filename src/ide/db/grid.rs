//! Painted data and result grids.
use super::*;

pub(super) fn result_grid(ui: &mut Ui, result: &Grid, id: Id) {
    let columns: Vec<&str> = result.columns.iter().map(String::as_str).collect();
    let rows: Vec<&[Cell]> = result.rows.iter().map(Vec::as_slice).collect();
    let arrows = vec![""; columns.len()];
    grid(ui, id, &columns, &arrows, &[], &rows, 0, None, &Default::default(), None, &|_, _| Color32::TRANSPARENT, None);
}

/// What the person did in a grid this frame.
#[derive(Default)]
pub(super) struct Clicks {
    pub(super) header: Option<usize>,
    /// a row number (and whether Cmd/Ctrl was held, to add to the selection)
    pub(super) gutter: Option<(usize, bool)>,
    pub(super) cell: Option<(usize, usize)>,
    pub(super) double: Option<(usize, usize)>,
}

/// PhpStorm's data grid, painted: a header that stays on top, a gutter of row numbers, thin lines
/// between cells, text on the left (numbers on the right), `<null>` muted, the chosen cell in the
/// selection blue. Only the rows in view are drawn. `edit` puts a text box over the cell being
/// edited.
#[allow(clippy::too_many_arguments)]
pub(super) fn grid(
    ui: &mut Ui,
    id: Id,
    columns: &[&str],
    arrows: &[&str],
    keys: &[bool],
    rows: &[&[Cell]],
    first: usize,
    reveal: Option<usize>,
    selected: &std::collections::HashSet<usize>,
    cell: Option<(usize, usize)>,
    fill: &dyn Fn(usize, usize) -> Color32,
    mut edit: Option<((usize, usize), &mut dyn FnMut(&mut Ui, Rect))>,
) -> Clicks {
    const ROW: f32 = 24.0;
    const HEAD: f32 = 26.0;
    let font = t::mono(12.5);
    let cw = ui.fonts_mut(|f| f.glyph_width(&font, '0'));
    let gutter = (first + rows.len()).max(1).to_string().len() as f32 * cw + 18.0;
    // each column fits its header and its widest value, within bounds
    let widths: Vec<f32> = (0..columns.len())
        .map(|c| {
            let longest = rows.iter().filter_map(|r| r.get(c)).map(|v| v.text().chars().count()).max().unwrap_or(0);
            // room for the mark before the name and the sort arrows after it
            (longest.max(columns[c].chars().count() + 6).min(40) as f32 * cw + 24.0).max(72.0)
        })
        .collect();
    let size = vec2(gutter + widths.iter().sum::<f32>(), HEAD + rows.len() as f32 * ROW);
    let line = Stroke::new(1.0, t::POPUP_BORDER);
    let mut clicks = Clicks::default();
    let mut area = egui::ScrollArea::both().id_salt(id).auto_shrink(false);
    if let Some(n) = reveal {
        area = area.vertical_scroll_offset((n as f32 * ROW - 200.0).max(0.0));
    }
    area.show_viewport(ui, |ui, view| {
        let (rect, _) = ui.allocate_exact_size(size, Sense::hover());
        let origin = rect.min;
        let p = ui.painter().clone();
        let multi = ui.input(|i| i.modifiers.command);
        let first_row = ((view.min.y - HEAD) / ROW).floor().max(0.0) as usize;
        let last_row = ((((view.max.y - HEAD) / ROW).ceil().max(0.0)) as usize + 1).min(rows.len());
        for n in first_row..last_row {
            let y = origin.y + HEAD + n as f32 * ROW;
            let band = Rect::from_min_size(pos2(origin.x, y), vec2(size.x, ROW));
            if selected.contains(&n) {
                p.rect_filled(band, 0, t::SELECTION_INACTIVE);
            } else if n % 2 == 1 {
                p.rect_filled(band, 0, t::INLINE);
            }
            let number = Rect::from_min_size(band.min, vec2(gutter, ROW));
            p.text(number.right_center() - vec2(8.0, 0.0), Align2::RIGHT_CENTER, (first + n + 1).to_string(), font.clone(), t::LINE_NUMBER);
            if ui.interact(number, id.with(("row", n)), Sense::click()).clicked() {
                clicks.gutter = Some((n, multi));
            }
            let mut x = origin.x + gutter;
            for (c, width) in widths.iter().enumerate() {
                let at = Rect::from_min_size(pos2(x, y), vec2(*width, ROW));
                let tint = fill(n, c);
                if tint != Color32::TRANSPARENT {
                    p.rect_filled(at, 0, tint);
                }
                let value = rows[n].get(c).unwrap_or(&Cell::Null);
                match edit.as_mut().filter(|(e, _)| *e == (n, c)) {
                    Some((_, put)) => put(ui, at.shrink(1.0)),
                    None => {
                        if cell == Some((n, c)) {
                            p.rect(at.shrink(0.5), 0, t::SELECTION, Stroke::new(1.0, t::ACCENT), egui::StrokeKind::Inside);
                        }
                        let text: String = value.text().chars().take(200).collect();
                        let numeric = matches!(value, Cell::Text(s) if s.parse::<f64>().is_ok());
                        let color = if *value == Cell::Null { t::SECONDARY } else { t::TEXT };
                        let (anchor, align) = if numeric { (at.right_center() - vec2(8.0, 0.0), Align2::RIGHT_CENTER) } else { (at.left_center() + vec2(8.0, 0.0), Align2::LEFT_CENTER) };
                        p.with_clip_rect(at.shrink2(vec2(6.0, 0.0)).intersect(p.clip_rect())).text(anchor, align, text, font.clone(), color);
                        let r = ui.interact(at, id.with(("cell", n, c)), Sense::click());
                        if r.clicked() {
                            clicks.cell = Some((n, c));
                        }
                        if r.double_clicked() {
                            clicks.double = Some((n, c));
                        }
                        if value.text().chars().count() as f32 * cw > width - 16.0 {
                            r.on_hover_text(value.text());
                        }
                    }
                }
                x += width;
                p.vline(x, at.y_range(), line);
            }
            p.hline(band.x_range(), band.bottom(), line);
        }
        // the header stays on top while the rows scroll under it
        let top = origin.y + view.min.y;
        let head = Rect::from_min_size(pos2(origin.x, top), vec2(size.x.max(view.width()), HEAD));
        p.rect_filled(head, 0, t::FRAME);
        let mut x = origin.x + gutter;
        for (c, width) in widths.iter().enumerate() {
            let at = Rect::from_min_size(pos2(x, top), vec2(*width, HEAD));
            let r = ui.interact(at, id.with(("head", c)), Sense::click());
            if r.hovered() {
                p.rect_filled(at, 0, t::ROW_HOVER);
            }
            let inner = p.with_clip_rect(at.shrink2(vec2(4.0, 0.0)).intersect(p.clip_rect()));
            look::mark(&inner, at.left_center() + vec2(12.0, 0.0), look::Mark::Column { pk: keys.get(c).copied().unwrap_or(false) });
            inner.text(at.left_center() + vec2(24.0, 0.0), Align2::LEFT_CENTER, columns[c], t::ui(12.5), t::TEXT);
            // ⇕, the active half lit when the column orders the rows
            let (up, down) = (arrows[c] == "↑", arrows[c] == "↓");
            let a = at.right_center() - vec2(10.0, 0.0);
            for (dir, lit) in [(-1.0, up), (1.0, down)] {
                let color = if lit { t::LINK } else { t::SECONDARY };
                inner.add(egui::Shape::convex_polygon(vec![a + vec2(-3.0, 1.5 * dir), a + vec2(3.0, 1.5 * dir), a + vec2(0.0, 4.5 * dir)], color, Stroke::NONE));
            }
            if r.on_hover_text("Clique para ordenar (ASC, DESC, nada)").clicked() {
                clicks.header = Some(c);
            }
            x += width;
            p.vline(x, at.y_range(), line);
        }
        p.hline(head.x_range(), head.bottom(), Stroke::new(1.0, t::CONTROL_BORDER));
    });
    clicks
}

