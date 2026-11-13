//! The tab that compares a file with the last commit, as the reference IDE's diff viewer: two columns (the HEAD's text and the
//! file's), one scroll for both, the changed lines colored (green added, gray removed, blue modified) and, in a modified
//! line, its changed words stronger. F7 and Shift+F7 (and Alt+↓ and Alt+↑) walk the changes; "Unificado" puts both
//! texts in one column. Nothing in it is written: the file is edited in its own tab, which the viewer follows.

use crate::ide::theme as t;
use crate::ide::{DiffRow, RowKind, diff_rows, highlight, unified_rows, word_marks};
use crate::prelude::*;
use std::ops::Range;
use std::path::Path;

/// Darcula's DIFF_* colors, as the IDE's viewer paints them.
const INSERTED: Color32 = Color32::from_rgb(0x29, 0x44, 0x36);
const DELETED: Color32 = Color32::from_rgb(0x48, 0x4A, 0x4A);
const CHANGED: Color32 = Color32::from_rgb(0x38, 0x55, 0x70);

pub(crate) struct Compare {
    pub(crate) file: PathBuf,
    pub(crate) old: String,
    pub(crate) new: String,
    pub(crate) unified: bool,
    /// The change the walk is on (counted from 0).
    pub(crate) cur: usize,
    lang: &'static highlight::Lang,
    old_lines: Vec<String>,
    new_lines: Vec<String>,
    /// What is shown, each row with the line it pairs with when it is half of a modified row (unified).
    rows: Vec<(DiffRow, Option<usize>)>,
    /// The rows of each change.
    blocks: Vec<Range<usize>>,
    scroll_to: Option<usize>,
    /// How far both columns are scrolled sideways (Shift+wheel, the touchpad or the bar under them).
    hscroll: f32,
}

/// The soft tint of a modified line: the word color mixed into the editor's background.
fn soft(c: Color32) -> Color32 {
    let (bg, mix) = (t::ISLAND, |a: u8, b: u8| (a as f32 * 0.4 + b as f32 * 0.6).round() as u8);
    Color32::from_rgb(mix(c.r(), bg.r()), mix(c.g(), bg.g()), mix(c.b(), bg.b()))
}

impl Compare {
    pub(crate) fn new(file: &Path, old: String, new: String) -> Self {
        let mut c = Compare {
            file: file.to_path_buf(),
            old,
            new,
            unified: false,
            cur: 0,
            lang: highlight::detect(file),
            old_lines: vec![],
            new_lines: vec![],
            rows: vec![],
            blocks: vec![],
            scroll_to: Some(0),
            hscroll: 0.0,
        };
        c.rebuild();
        c
    }

    pub(crate) fn set_new(&mut self, new: String) {
        self.new = new;
        self.rebuild();
    }

    fn rebuild(&mut self) {
        self.old_lines = self.old.lines().map(String::from).collect();
        self.new_lines = self.new.lines().map(String::from).collect();
        let rows = diff_rows(&self.old, &self.new);
        self.rows = if self.unified { unified_rows(&rows) } else { rows.into_iter().map(|r| (r, None)).collect() };
        self.blocks.clear();
        let mut i = 0;
        while i < self.rows.len() {
            let end = i + self.rows[i..].iter().take_while(|r| r.0.kind != RowKind::Same).count();
            if end > i {
                self.blocks.push(i..end);
            }
            i = end.max(i + 1);
        }
        self.cur = self.cur.min(self.blocks.len().saturating_sub(1));
    }

    /// How many changes there are.
    pub(crate) fn count(&self) -> usize {
        self.blocks.len()
    }

    /// The walk's step to the next change (1) or the previous one (-1); the first and the last stop it.
    pub(crate) fn go(&mut self, by: isize) {
        if !self.blocks.is_empty() {
            self.cur = (self.cur as isize + by).clamp(0, self.blocks.len() as isize - 1) as usize;
            self.scroll_to = Some(self.blocks[self.cur].start);
        }
    }

    /// Side by side or one column.
    pub(crate) fn set_unified(&mut self, unified: bool) {
        if self.unified != unified {
            self.unified = unified;
            self.rebuild();
            self.scroll_to = self.blocks.get(self.cur).map(|b| b.start);
        }
    }
}

/// Line `i` of `v` (empty when there is none).
fn at(v: &[String], i: Option<usize>) -> &str {
    i.and_then(|i| v.get(i)).map_or("", String::as_str)
}

/// The compare tab in `body`.
pub(crate) fn compare_ui(ui: &mut egui::Ui, body: Rect, c: &mut Compare, z: f32) {
    let font = t::mono(t::EDITOR_SIZE * z);
    let (natural, char_w) = ui.fonts_mut(|f| (f.row_height(&font), f.glyph_width(&font, '0')));
    let row_h = (natural * 1.2).round();
    ui.painter().with_clip_rect(body.intersect(ui.clip_rect())).rect_filled(body.shrink2(Vec2::new(1.0, 1.0)), 0, t::ISLAND);
    // F7 / Shift+F7 walk the changes (with no field taking the keys)
    // ponytail: two nodes showing a compare tab at once both take the key; gate on the node under the pointer if that bites
    if !ui.ctx().egui_wants_keyboard_input() {
        let key = |m: Modifiers, k: Key| ui.input_mut(|i| i.consume_key(m, k));
        if key(Modifiers::SHIFT, Key::F7) || key(Modifiers::ALT, Key::ArrowUp) {
            c.go(-1);
        } else if key(Modifiers::NONE, Key::F7) || key(Modifiers::ALT, Key::ArrowDown) {
            c.go(1);
        }
    }
    let bar = Rect::from_min_size(body.min, Vec2::new(body.width(), 28.0 * z));
    let head = Rect::from_min_size(Pos2::new(body.left(), bar.bottom()), Vec2::new(body.width(), 20.0 * z));
    let view = Rect::from_min_max(Pos2::new(body.left(), head.bottom()), Pos2::new(body.right() - 1.0, body.bottom() - 1.0));
    let small = t::ui((t::UI_SIZE - 1.0) * z);
    ui.scope_builder(egui::UiBuilder::new().max_rect(bar.shrink2(Vec2::new(8.0 * z, 0.0))), |ui| {
        ui.set_clip_rect(bar.intersect(ui.clip_rect()));
        ui.horizontal_centered(|ui| {
            let n = c.count();
            let text = |s: &str, on: bool| egui::RichText::new(s).font(small.clone()).color(if on { t::TEXT } else { t::SECONDARY });
            if ui.add_enabled(c.cur > 0, egui::Button::new(text("Anterior", c.cur > 0)).frame(false)).on_hover_text("Mudança anterior (Shift+F7)").clicked() {
                c.go(-1);
            }
            if ui.add_enabled(c.cur + 1 < n, egui::Button::new(text("Próxima", c.cur + 1 < n)).frame(false)).on_hover_text("Próxima mudança (F7)").clicked()
            {
                c.go(1);
            }
            let label = if n == 0 { "sem diferenças".to_string() } else { format!("{} de {n}", c.cur + 1) };
            ui.label(egui::RichText::new(label).font(small.clone()).color(t::MUTED));
            ui.add_space(12.0 * z);
            let side = ui.selectable_label(!c.unified, text("Lado a lado", true)).clicked();
            let one = ui.selectable_label(c.unified, text("Unificado", true)).clicked();
            if side || one {
                c.set_unified(one);
            }
        });
    });
    let p = ui.painter().with_clip_rect(body.intersect(ui.clip_rect()));
    p.hline(body.x_range(), bar.bottom(), Stroke::new(1.0, t::POPUP_BORDER));
    let name = c.file.file_name().unwrap_or_default().to_string_lossy().into_owned();
    let (half, num_w) = (view.width() / 2.0, (c.old_lines.len().max(c.new_lines.len()).to_string().len().max(3) as f32 + 1.0) * char_w);
    if c.unified {
        p.text(Pos2::new(head.left() + 8.0 * z, head.center().y), egui::Align2::LEFT_CENTER, format!("HEAD ↔ {name}"), small.clone(), t::MUTED);
    } else {
        p.text(Pos2::new(head.left() + 8.0 * z, head.center().y), egui::Align2::LEFT_CENTER, "HEAD", small.clone(), t::MUTED);
        p.text(Pos2::new(head.left() + half + 8.0 * z, head.center().y), egui::Align2::LEFT_CENTER, format!("{name} (arquivo atual)"), small.clone(), t::MUTED);
    }
    let mut sa = egui::ScrollArea::vertical().id_salt(("ft_compare", c.file.to_string_lossy().as_ref())).auto_shrink(false);
    if let Some(r) = c.scroll_to.take() {
        sa = sa.vertical_scroll_offset((r as f32 * row_h - view.height() / 3.0).max(0.0));
    }
    // sideways: the longest line against the room a column leaves for its text
    let room = if c.unified { view.width() - 2.0 * num_w - 4.0 } else { half - num_w - 4.0 };
    let widest = c.old_lines.iter().chain(&c.new_lines).map(|l| l.chars().count()).max().unwrap_or(0) as f32 * char_w;
    let most = (widest + 4.0 * char_w - room).max(0.0);
    if ui.rect_contains_pointer(view) {
        c.hscroll -= ui.input(|i| i.smooth_scroll_delta.x);
    }
    let track = Rect::from_min_max(Pos2::new(view.left(), view.bottom() - 8.0 * z), view.max);
    if most > 0.0 {
        let thumb_w = (track.width() * room / (room + most)).max(24.0 * z);
        let r = ui.interact(track, ui.scope_id().with(("ft_compare_h", c.file.to_string_lossy().as_ref())), Sense::click_and_drag());
        if let Some(x) = r.interact_pointer_pos().filter(|_| r.dragged() || r.clicked()) {
            c.hscroll = (x.x - track.left() - thumb_w / 2.0) / (track.width() - thumb_w) * most;
        }
    }
    c.hscroll = c.hscroll.clamp(0.0, most);
    let shift = c.hscroll;
    let lines = (&c.old_lines, &c.new_lines);
    let (rows, blocks, cur, lang, unified) = (&c.rows, &c.blocks, c.cur, c.lang, c.unified);
    ui.scope_builder(egui::UiBuilder::new().max_rect(view), |ui| {
        ui.set_clip_rect(view.intersect(ui.clip_rect()));
        sa.show_rows(ui, row_h, rows.len(), |ui, range| {
            for i in range {
                let (r, partner) = rows[i];
                let (rect, _) = ui.allocate_exact_size(Vec2::new(ui.available_width(), row_h), Sense::hover());
                let p = ui.painter().clone();
                let (left, right) = if unified { (rect, rect) } else { (rect.with_max_x(rect.left() + half), rect.with_min_x(rect.left() + half)) };
                let (lbg, rbg) = match r.kind {
                    RowKind::Same => (Color32::TRANSPARENT, Color32::TRANSPARENT),
                    RowKind::Insert => (Color32::TRANSPARENT, INSERTED),
                    RowKind::Delete => (DELETED, Color32::TRANSPARENT),
                    RowKind::Change => (soft(CHANGED), soft(CHANGED)),
                };
                if unified {
                    p.rect_filled(rect, 0, if r.left.is_some() { lbg } else { rbg });
                } else {
                    p.rect_filled(left, 0, lbg);
                    p.rect_filled(right, 0, rbg);
                    p.vline(right.left(), rect.y_range(), Stroke::new(1.0, t::POPUP_BORDER));
                }
                // the words of a modified line that differ from its pair

                let (lw, rw) = match (r.kind, r.left, r.right) {
                    (RowKind::Change, Some(l), Some(n)) => word_marks(at(lines.0, Some(l)), at(lines.1, Some(n))),
                    (RowKind::Change, Some(l), None) => (word_marks(at(lines.0, Some(l)), at(lines.1, partner)).0, vec![]),
                    (RowKind::Change, None, Some(n)) => (vec![], word_marks(at(lines.0, partner), at(lines.1, Some(n))).1),
                    _ => Default::default(),
                };
                // the change the walk is on gets a frame
                if let Some(b) = blocks.get(cur).filter(|b| b.contains(&i)) {
                    let stroke = Stroke::new(1.0, t::LINK.gamma_multiply(0.7));
                    p.vline(rect.left() + 0.5, rect.y_range(), stroke);
                    p.vline(rect.right() - 0.5, rect.y_range(), stroke);
                    if i == b.start {
                        p.hline(rect.x_range(), rect.top() + 0.5, stroke);
                    }
                    if i + 1 == b.end {
                        p.hline(rect.x_range(), rect.bottom() - 0.5, stroke);
                    }
                }
                let mut sides = vec![];
                if unified {
                    // both columns of numbers; the text is the removed or the added line
                    if let Some(l) = r.left {
                        p.text(
                            Pos2::new(rect.left() + num_w - 4.0, rect.center().y),
                            egui::Align2::RIGHT_CENTER,
                            (l + 1).to_string(),
                            font.clone(),
                            t::LINE_NUMBER,
                        );
                    }
                    if let Some(n) = r.right {
                        p.text(
                            Pos2::new(rect.left() + 2.0 * num_w - 4.0, rect.center().y),
                            egui::Align2::RIGHT_CENTER,
                            (n + 1).to_string(),
                            font.clone(),
                            t::LINE_NUMBER,
                        );
                    }
                    let (idx, from, marks) = if r.left.is_some() { (r.left, lines.0, lw) } else { (r.right, lines.1, rw) };
                    sides.push((rect.left() + 2.0 * num_w + 4.0, rect, idx, from, marks));
                } else {
                    for (side, idx, from, marks) in [(left, r.left, lines.0, lw), (right, r.right, lines.1, rw)] {
                        if let Some(n) = idx {
                            p.text(
                                Pos2::new(side.left() + num_w - 4.0, side.center().y),
                                egui::Align2::RIGHT_CENTER,
                                (n + 1).to_string(),
                                font.clone(),
                                t::LINE_NUMBER,
                            );
                        }
                        sides.push((side.left() + num_w + 4.0, side, idx, from, marks));
                    }
                }
                for (x, side, idx, from, marks) in sides {
                    let Some(line) = idx.and_then(|n| from.get(n)) else { continue };
                    let g = ui.fonts_mut(|f| f.layout_job(highlight::layout(lang, line, font.clone(), row_h)));
                    // the text slides under the numbers, which stay
                    let p = p.with_clip_rect(side.with_min_x(x).intersect(ui.clip_rect()));
                    let x = x - shift;
                    for w in marks {
                        let at = |b: usize| g.pos_from_cursor(egui::text::CCursor::new(line[..b.min(line.len())].chars().count())).left();
                        p.rect_filled(Rect::from_min_max(Pos2::new(x + at(w.start), side.top()), Pos2::new(x + at(w.end), side.bottom())), 0, CHANGED);
                    }
                    p.galley(Pos2::new(x, side.top()), g, t::EDITOR_TEXT);
                }
            }
        });
    });
    if most > 0.0 {
        let thumb_w = (track.width() * room / (room + most)).max(24.0 * z);
        let left = track.left() + (track.width() - thumb_w) * shift / most;
        let thumb = Rect::from_min_size(Pos2::new(left, track.top() + 2.0 * z), Vec2::new(thumb_w, track.height() - 3.0 * z));
        ui.painter().with_clip_rect(view.intersect(ui.clip_rect())).rect_filled(thumb, 3.0 * z, t::SECONDARY.gamma_multiply(0.45));
    }
}
