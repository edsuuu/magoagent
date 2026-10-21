//! How the database pieces look, after PhpStorm's Database tool window and data editor: the
//! tree's rows and marks, the editor's icon buttons. Painted, so no new assets.

use super::*;
use egui::{Pos2, Vec2};

const KEY_GOLD: Color32 = Color32::from_rgb(0xE0, 0xB4, 0x4C);
const FOLDER_BLUE: Color32 = Color32::from_rgb(0x6E, 0xA0, 0xF2);
const MARK_GRAY: Color32 = Color32::from_rgb(0x9A, 0xA1, 0xAD);
const RUN_GREEN: Color32 = Color32::from_rgb(0x5F, 0xB8, 0x65);

/// What a tree row shows before its name.
#[derive(Clone, Copy)]
pub(super) enum Mark {
    Source,
    Schema,
    Folder,
    Table,
    Column { pk: bool },
    Key(Color32),
    Index,
}

pub(super) const FOREIGN_KEY: Color32 = FOLDER_BLUE;
pub(super) const PRIMARY_KEY: Color32 = KEY_GOLD;

pub(super) fn mark(p: &egui::Painter, at: Pos2, mark: Mark) {
    let thin = |c| Stroke::new(1.1, c);
    match mark {
        Mark::Source => icons::at(p, icons::FT_SQL, at),
        Mark::Schema => {
            // three linked boxes
            for (dx, dy) in [(-3.5, -3.0), (3.5, -3.0), (-3.5, 3.5)] {
                p.rect_stroke(Rect::from_center_size(at + vec2(dx, dy), vec2(5.0, 5.0)), 1.0, thin(MARK_GRAY), egui::StrokeKind::Inside);
            }
            p.line_segment([at + vec2(-1.0, -3.0), at + vec2(1.0, -3.0)], thin(MARK_GRAY));
        }
        Mark::Folder => {
            let r = Rect::from_center_size(at + vec2(0.0, 0.5), vec2(13.0, 10.0));
            p.rect_stroke(r, 1.5, thin(FOLDER_BLUE), egui::StrokeKind::Inside);
            p.line_segment([r.left_top() + vec2(1.0, 0.0), r.left_top() + vec2(5.0, -2.0)], thin(FOLDER_BLUE));
            p.line_segment([r.left_top() + vec2(5.0, -2.0), r.left_top() + vec2(6.5, 0.0)], thin(FOLDER_BLUE));
        }
        Mark::Table => {
            let r = Rect::from_center_size(at, vec2(13.0, 11.0));
            p.rect_stroke(r, 1.5, thin(MARK_GRAY), egui::StrokeKind::Inside);
            p.line_segment([pos2(r.left(), r.top() + 3.5), pos2(r.right(), r.top() + 3.5)], thin(MARK_GRAY));
            for x in [r.left() + 4.5, r.left() + 8.5] {
                p.line_segment([pos2(x, r.top() + 3.5), pos2(x, r.bottom())], thin(MARK_GRAY));
            }
        }
        Mark::Column { pk } => {
            // a cell with a bar on its left; the primary key wears a gold key over it
            let r = Rect::from_center_size(at, vec2(11.0, 12.0));
            p.rect_stroke(r, 1.5, thin(MARK_GRAY), egui::StrokeKind::Inside);
            p.rect_filled(Rect::from_min_size(r.min, vec2(3.0, r.height())), 1.0, MARK_GRAY);
            if pk {
                key(p, at + vec2(3.0, 3.5), KEY_GOLD);
            }
        }
        Mark::Key(color) => key(p, at, color),
        Mark::Index => {
            let r = Rect::from_center_size(at, vec2(11.0, 12.0));
            p.rect_stroke(r, 1.5, thin(MARK_GRAY), egui::StrokeKind::Inside);
            for y in [r.top() + 4.0, r.top() + 8.0] {
                p.line_segment([pos2(r.left() + 3.0, y), pos2(r.right() - 3.0, y)], thin(MARK_GRAY));
            }
        }
    }
}

/// A small key: a ring and a toothed shaft.
fn key(p: &egui::Painter, at: Pos2, color: Color32) {
    let s = Stroke::new(1.3, color);
    p.circle_stroke(at + vec2(-3.0, 0.0), 2.4, s);
    p.line_segment([at + vec2(-0.6, 0.0), at + vec2(5.0, 0.0)], s);
    p.line_segment([at + vec2(3.5, 0.0), at + vec2(3.5, 2.2)], s);
}

/// A row of the Database tree: indented by `depth`, a chevron when it can open (`open`), the
/// mark, the name and, muted after it, `note` (a count, a column's type). `badge` boxes a number
/// the way PhpStorm counts a source's schemas.
pub(super) fn tree_row(ui: &mut Ui, depth: usize, open: Option<bool>, m: Mark, name: &str, note: &str, badge: Option<usize>) -> egui::Response {
    let (resp, rect) = row(ui, false, 22.0);
    let x = rect.left() + 10.0 + depth as f32 * 16.0;
    let y = rect.center().y;
    let p = ui.painter();
    if let Some(open) = open {
        icons::paint(p, if open { icons::CHEVRON_DOWN } else { icons::CHEVRON_RIGHT }, Rect::from_center_size(pos2(x, y), vec2(14.0, 14.0)));
    }
    mark(p, pos2(x + 16.0, y), m);
    let g = p.text(pos2(x + 28.0, y), Align2::LEFT_CENTER, name, t::ui(t::UI_SIZE), t::TEXT);
    let mut after = g.right() + 7.0;
    if let Some(n) = badge {
        let text = p.layout_no_wrap(n.to_string(), t::ui(11.0), t::SECONDARY);
        let r = Rect::from_min_size(pos2(after, y - 8.0), vec2(text.size().x + 8.0, 16.0));
        p.rect_stroke(r, 3.0, Stroke::new(1.0, t::CONTROL_BORDER), egui::StrokeKind::Inside);
        p.galley(r.center() - text.size() / 2.0, text, t::SECONDARY);
        after = r.right() + 7.0;
    }
    if !note.is_empty() {
        p.text(pos2(after, y), Align2::LEFT_CENTER, note, t::ui(t::UI_SIZE - 1.0), t::SECONDARY);
    }
    resp
}

/// The editor toolbar's glyphs.
#[derive(Clone, Copy)]
pub(super) enum Glyph {
    Refresh,
    Add,
    Remove,
    Revert,
    Preview,
    Submit,
    Run,
    History,
    First,
    Previous,
    Next,
    Last,
}

/// An icon button of the data editor's toolbar (28×28, hover fill, dimmed when off).
pub(super) fn tool(ui: &mut Ui, glyph: Glyph, tip: &str, enabled: bool) -> bool {
    let (rect, resp) = ui.allocate_exact_size(vec2(26.0, 26.0), if enabled { Sense::click() } else { Sense::hover() });
    if enabled && resp.hovered() {
        ui.painter().rect_filled(rect, 5, t::HOVER);
    }
    let color = if enabled { t::TEXT } else { t::SECONDARY.gamma_multiply(0.6) };
    let p = ui.painter();
    let c = rect.center();
    let s = Stroke::new(1.4, color);
    match glyph {
        Glyph::Refresh => icons::paint(p, icons::REFRESH, Rect::from_center_size(c, vec2(16.0, 16.0))),
        Glyph::Add => {
            p.hline(c.x - 6.0..=c.x + 6.0, c.y, s);
            p.vline(c.x, c.y - 6.0..=c.y + 6.0, s);
        }
        Glyph::Remove => {
            p.hline(c.x - 6.0..=c.x + 6.0, c.y, s);
        }
        Glyph::Revert => {
            // a U-turn arrow
            let points: Vec<Pos2> = (0..=10).map(|k| c + vec2(1.0, 1.0) + Vec2::angled(-std::f32::consts::FRAC_PI_2 + k as f32 / 10.0 * std::f32::consts::PI) * 5.0).collect();
            p.add(egui::Shape::line(points, s));
            p.line_segment([c + vec2(1.0, -4.0), c + vec2(-5.0, -4.0)], s);
            p.line_segment([c + vec2(-5.0, -4.0), c + vec2(-2.5, -6.5)], s);
            p.line_segment([c + vec2(-5.0, -4.0), c + vec2(-2.5, -1.5)], s);
        }
        Glyph::Preview => {
            let eye: Vec<Pos2> = (0..=16).map(|k| { let a = k as f32 / 16.0 * std::f32::consts::TAU; c + vec2(a.cos() * 7.0, a.sin() * 4.0) }).collect();
            p.add(egui::Shape::closed_line(eye, s));
            p.circle_filled(c, 2.0, color);
        }
        Glyph::Submit => {
            p.vline(c.x, c.y - 6.0..=c.y + 6.0, s);
            p.line_segment([c + vec2(0.0, -6.0), c + vec2(-4.5, -1.5)], s);
            p.line_segment([c + vec2(0.0, -6.0), c + vec2(4.5, -1.5)], s);
        }
        Glyph::Run => {
            let fill = if enabled { RUN_GREEN } else { color };
            p.add(egui::Shape::convex_polygon(vec![c + vec2(-4.0, -6.0), c + vec2(6.0, 0.0), c + vec2(-4.0, 6.0)], fill, Stroke::NONE));
        }
        Glyph::History => {
            p.circle_stroke(c, 6.0, s);
            p.line_segment([c, c + vec2(0.0, -4.0)], s);
            p.line_segment([c, c + vec2(3.0, 1.5)], s);
        }
        Glyph::First | Glyph::Previous | Glyph::Next | Glyph::Last => {
            let dir = if matches!(glyph, Glyph::Next | Glyph::Last) { 1.0 } else { -1.0 };
            p.line_segment([c + vec2(-2.0 * dir, -4.5), c + vec2(2.5 * dir, 0.0)], s);
            p.line_segment([c + vec2(2.5 * dir, 0.0), c + vec2(-2.0 * dir, 4.5)], s);
            if matches!(glyph, Glyph::First | Glyph::Last) {
                p.vline(c.x + 4.5 * dir, c.y - 5.0..=c.y + 5.0, s);
            }
        }
    }
    enabled && resp.on_hover_text(tip).clicked()
}

/// The thin line under a toolbar.
pub(super) fn rule(ui: &mut Ui) {
    let (r, _) = ui.allocate_exact_size(vec2(ui.available_width(), 1.0), Sense::hover());
    ui.painter().hline(r.x_range(), r.center().y, Stroke::new(1.0, t::POPUP_BORDER));
}

/// The WHERE box's funnel or the ORDER BY box's sort lines.
pub(super) fn funnel_or_sort(p: &egui::Painter, c: Pos2, funnel: bool) {
    let s = Stroke::new(1.2, MARK_GRAY);
    if funnel {
        p.add(egui::Shape::closed_line(vec![c + vec2(-6.0, -5.0), c + vec2(6.0, -5.0), c + vec2(1.0, 1.0), c + vec2(1.0, 5.0), c + vec2(-1.0, 6.0), c + vec2(-1.0, 1.0)], s));
    } else {
        for (k, w) in [6.0, 4.0, 2.0].into_iter().enumerate() {
            let y = c.y - 4.0 + k as f32 * 4.0;
            p.hline(c.x - 6.0..=c.x - 6.0 + w * 2.0, y, s);
        }
    }
}
