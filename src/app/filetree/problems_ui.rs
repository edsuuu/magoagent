//! What the editor pane of a file tree node draws of the problems, as the reference IDE does: the wavy underline (red for an
//! error, yellow for a warning) with the message in a tooltip, the traffic light at the top right (the counts of errors
//! and warnings, the list they open, the arrows to the previous and next problem) and the error stripe beside the scroll
//! bar (a mark per problem, pressed it goes there). The tree's red names use `wave` too. The logic is in
//! filetree_problems.rs.

use crate::app::filetree::problems::{ERROR, counts, next_problem};
use crate::ide::lsp::Diag;
use crate::ide::theme as t;
use crate::prelude::*;
use egui::text::CCursor;
use std::ops::Range;

/// The error stripe's width, in the node's own points.
pub(crate) const STRIPE: f32 = 12.0;

/// The red of a name or a wave with a problem under it, on the dark or the light card.
pub(crate) fn error_color(dark: bool) -> Color32 {
    if dark { t::ERROR } else { Color32::from_rgb(0xDB, 0x58, 0x60) }
}

fn severity_color(d: &Diag) -> Color32 {
    if d.severity == ERROR { t::ERROR } else { t::WARNING }
}

/// A wavy line from `x0` to `x1` at height `y`: the reference's error and warning effect.
pub(crate) fn wave(p: &Painter, x0: f32, x1: f32, y: f32, z: f32, color: Color32) {
    let step = (1.6 * z).max(1.0);
    let mut points = vec![];
    let (mut x, mut up) = (x0, false);
    while x < x1 {
        points.push(Pos2::new(x, if up { y - step } else { y }));
        (x, up) = (x + step, !up);
    }
    points.push(Pos2::new(x1, if up { y - step } else { y }));
    p.add(Shape::line(points, Stroke::new((1.0 * z).max(1.0), color)));
}

/// The chars each problem on `lines` covers: (first, one past the last, its index in `diags`). One that has no width (or runs
/// over lines) covers its first line from the start; at least one char.
pub(crate) fn spans(text: &str, diags: &[Diag], lines: Range<usize>) -> Vec<(usize, usize, usize)> {
    let mut starts = vec![0];
    let total = text
        .chars()
        .enumerate()
        .inspect(|(i, c)| {
            if *c == '\n' {
                starts.push(i + 1)
            }
        })
        .count();
    let end_of = |line: usize| starts.get(line + 1).map_or(total, |n| n - 1);
    diags
        .iter()
        .enumerate()
        .filter(|(_, d)| lines.contains(&d.line) && d.line < starts.len())
        .map(|(i, d)| {
            let (first, last) = (starts[d.line], end_of(d.line));
            let a = (first + d.col).min(last);
            let b = if d.end_line == d.line && d.end_col > d.col {
                first + d.end_col
            } else if d.end_line > d.line {
                last
            } else {
                a + 1
            };
            (a, b.clamp(a + 1, last.max(a + 1)), i)
        })
        .collect()
}

/// Underlines the problems in `spans` and gives back the one under `pointer`, if any.
pub(crate) fn waves<'a>(
    p: &Painter,
    galley: &egui::Galley,
    origin: Pos2,
    diags: &'a [Diag],
    spans: &[(usize, usize, usize)],
    z: f32,
    pointer: Option<Pos2>,
) -> Option<&'a Diag> {
    let mut under = None;
    // warnings first: an error over the same place stays on top
    let mut order: Vec<&(usize, usize, usize)> = spans.iter().collect();
    order.sort_by_key(|s| diags[s.2].severity != ERROR);
    for &&(a, b, i) in &order {
        let (ra, rb) = (galley.pos_from_cursor(CCursor::new(a)), galley.pos_from_cursor(CCursor::new(b)));
        // one row: to the end; over a wrapped row: to its end
        let x1 = if (ra.min.y - rb.min.y).abs() < 1.0 { rb.min.x } else { galley.rect.right() };
        let (x0, x1) = (origin.x + ra.min.x, origin.x + x1.max(ra.min.x + 3.0 * z));
        wave(p, x0, x1, origin.y + ra.max.y - 1.0 * z, z, severity_color(&diags[i]));
        let row = Rect::from_min_max(Pos2::new(x0, origin.y + ra.min.y), Pos2::new(x1, origin.y + ra.max.y));
        if pointer.is_some_and(|q| row.contains(q)) && under.is_none_or(|d: &Diag| d.severity != ERROR) {
            under = Some(&diags[i]);
        }
    }
    under
}

/// The tooltip of a problem: what the reference's hover popup says, its source under the message.
pub(crate) fn tip(ui: &mut egui::Ui, d: &Diag) {
    ui.set_max_width(420.0);
    ui.visuals_mut().override_text_color = Some(t::TEXT);
    ui.label(egui::RichText::new(&d.message).color(severity_color(d)));
    if !d.source.is_empty() {
        ui.label(egui::RichText::new(&d.source).color(t::SECONDARY).small());
    }
}

fn paint_icon(p: &Painter, c: Pos2, size: f32, error: bool) {
    if error {
        p.circle_filled(c, size / 2.0, t::ERROR);
        p.line_segment([c + Vec2::new(0.0, -size * 0.22), c + Vec2::new(0.0, size * 0.08)], Stroke::new(1.3, Color32::WHITE));
        p.circle_filled(c + Vec2::new(0.0, size * 0.25), 0.8, Color32::WHITE);
    } else {
        let h = size / 2.0;
        p.add(Shape::convex_polygon(vec![c + Vec2::new(0.0, -h), c + Vec2::new(h, h * 0.85), c + Vec2::new(-h, h * 0.85)], t::WARNING, Stroke::NONE));
        p.line_segment([c + Vec2::new(0.0, -h * 0.25), c + Vec2::new(0.0, h * 0.25)], Stroke::new(1.3, t::ISLAND));
        p.circle_filled(c + Vec2::new(0.0, h * 0.55), 0.7, t::ISLAND);
    }
}

fn paint_check(p: &Painter, c: Pos2, size: f32) {
    let s = Stroke::new(1.6, Color32::from_rgb(0x4E, 0x9D, 0x6C));
    let k = size / 2.0;
    p.line_segment([c + Vec2::new(-k * 0.7, 0.0), c + Vec2::new(-k * 0.2, k * 0.55)], s);
    p.line_segment([c + Vec2::new(-k * 0.2, k * 0.55), c + Vec2::new(k * 0.75, -k * 0.5)], s);
}

fn paint_arrow(p: &Painter, c: Pos2, size: f32, up: bool, color: Color32) {
    let (k, d) = (size * 0.32, if up { -1.0 } else { 1.0 });
    let s = Stroke::new(1.3, color);
    p.line_segment([c + Vec2::new(-k, k * 0.5 * -d), c + Vec2::new(0.0, k * 0.5 * d)], s);
    p.line_segment([c + Vec2::new(0.0, k * 0.5 * d), c + Vec2::new(k, k * 0.5 * -d)], s);
}

/// The traffic light over the code at its top right and the error stripe at `stripe`, for a text of `lines` lines whose
/// caret is at `at`. Back: the place a press asked to go to (a mark, a line of the list, an arrow).
#[allow(clippy::too_many_arguments)]
pub(crate) fn chrome(
    ui: &mut egui::Ui,
    code: Rect,
    stripe: Rect,
    lines: usize,
    diags: &[Diag],
    at: (usize, usize),
    z: f32,
    salt: egui::Id,
) -> Option<(usize, usize)> {
    let p = ui.painter().with_clip_rect(code.union(stripe).intersect(ui.clip_rect()));
    let mut go = None;
    // the error stripe: a mark per problem, errors over warnings
    p.rect_filled(stripe, 0, t::ISLAND);
    p.vline(stripe.left(), stripe.y_range(), Stroke::new(1.0, t::INDENT_GUIDE));
    let y_of = |line: usize| stripe.top() + (line as f32 + 0.5) / lines.max(1) as f32 * stripe.height();
    let mut order: Vec<&Diag> = diags.iter().collect();
    order.sort_by_key(|d| d.severity == ERROR);
    for d in order {
        let mark = Rect::from_center_size(Pos2::new(stripe.center().x + 0.5, y_of(d.line)), Vec2::new(stripe.width() - 5.0 * z, (3.0 * z).max(2.0)));
        p.rect_filled(mark, 1, severity_color(d));
    }
    let r = ui.interact(stripe, salt.with("stripe"), Sense::click());
    if let Some(q) = r.hover_pos().filter(|_| !diags.is_empty()) {
        let near =
            diags.iter().min_by(|a, b| (y_of(a.line) - q.y).abs().total_cmp(&(y_of(b.line) - q.y).abs())).filter(|d| (y_of(d.line) - q.y).abs() <= 5.0 * z);
        if let Some(d) = near {
            ui.ctx().set_cursor_icon(CursorIcon::PointingHand);
            if r.clicked() {
                go = Some((d.line, d.col));
            }
            r.on_hover_ui_at_pointer(|ui| tip(ui, d));
        }
    }
    // the traffic light
    let (errors, warnings) = counts(diags);
    let font = t::ui(12.0 * z);
    let icon = 11.0 * z;
    let mut parts: Vec<(bool, std::sync::Arc<egui::Galley>)> = vec![];
    for (on, n, error) in [(errors > 0, errors, true), (warnings > 0, warnings, false)] {
        if on {
            parts.push((error, p.layout_no_wrap(n.to_string(), font.clone(), t::TEXT)));
        }
    }
    let arrows = 13.0 * z;
    let body_w = if parts.is_empty() { icon } else { parts.iter().map(|(_, g)| icon + 3.0 * z + g.size().x + 7.0 * z).sum::<f32>() - 7.0 * z };
    let (pad, h) = (6.0 * z, 20.0 * z);
    let width = pad * 2.0 + body_w + 4.0 * z + arrows * 2.0;
    let rect = Rect::from_min_size(Pos2::new(stripe.left() - 6.0 * z - width, code.top() + 4.0 * z), Vec2::new(width, h));
    p.rect(rect, 6, t::POPUP, Stroke::new(1.0, t::POPUP_BORDER), StrokeKind::Inside);
    let main = Rect::from_min_max(rect.min, Pos2::new(rect.left() + pad * 2.0 + body_w, rect.bottom()));
    let mut x = rect.left() + pad;
    if parts.is_empty() {
        paint_check(&p, Pos2::new(x + icon / 2.0, rect.center().y), icon);
    }
    for (error, g) in parts {
        paint_icon(&p, Pos2::new(x + icon / 2.0, rect.center().y), icon, error);
        p.galley(Pos2::new(x + icon + 3.0 * z, rect.center().y - g.size().y / 2.0), g.clone(), t::TEXT);
        x += icon + 3.0 * z + g.size().x + 7.0 * z;
    }
    let light = ui.interact(main, salt.with("light"), Sense::click()).on_hover_cursor(CursorIcon::PointingHand);
    let light = light.on_hover_text("Problemas do arquivo");
    egui::Popup::menu(&light)
        .align(egui::RectAlign::BOTTOM_END)
        .frame(egui::Frame::new().fill(t::POPUP).stroke(Stroke::new(1.0, t::POPUP_BORDER)).corner_radius(6).inner_margin(6))
        .show(|ui| {
            ui.set_width(380.0);
            ui.visuals_mut().override_text_color = Some(t::TEXT);
            if diags.is_empty() {
                ui.label("Nenhum problema encontrado");
            }
            egui::ScrollArea::vertical().max_height(220.0).show(ui, |ui| {
                for d in diags {
                    let label = format!("{}:{}  {}", d.line + 1, d.col + 1, d.message.lines().next().unwrap_or_default());
                    let row = ui.add(egui::Button::new(egui::RichText::new(label).color(severity_color(d))).frame(false).truncate());
                    if row.on_hover_text(&d.message).clicked() {
                        go = Some((d.line, d.col));
                        ui.close();
                    }
                }
            });
        });
    for (k, up) in [(0.0, true), (1.0, false)] {
        let a = Rect::from_min_size(Pos2::new(main.right() + 4.0 * z + k * arrows, rect.top()), Vec2::new(arrows, h));
        let target = next_problem(diags, at, !up);
        let r = ui.interact(a, salt.with(("arrow", up)), if target.is_some() { Sense::click() } else { Sense::hover() });
        if r.hovered() && target.is_some() {
            p.rect_filled(a.shrink(1.0), 4, t::HOVER);
        }
        paint_arrow(&p, a.center(), arrows, up, if target.is_some() { t::TEXT } else { t::SECONDARY });
        let r = r.on_hover_text(if up { "Problema anterior (Shift+F2)" } else { "Próximo problema (F2)" });
        if let (true, Some(d)) = (r.clicked(), target) {
            go = Some((d.line, d.col));
        }
    }
    go
}
