//! The file tree node's face. The canvas paints the card (frame, title); the live parts (back and
//! forward, the view menu, the git pill, the rows, the search bar) sit over it in small areas, so
//! the title strip is still the canvas's and drags the node. Sizes are the reference app's, at
//! 13 px to the rem, times the zoom.

use crate::app::filetree::*;
use crate::app::filetree_menu::{G, Popover, arc, name_popover, paint_g, row_menu, view_menu};
use crate::app::filetree_pane::{Editor, pane_ui, parts};
use crate::ide::{Status, icons};
use crate::prelude::*;
use std::path::Path;

/// The reference app's `--bg`, `--border`, `--foreground`, `--muted-foreground`, `--bg-hover`.
pub(super) struct Palette {
    pub(super) bg: Color32,
    pub(super) border: Color32,
    pub(super) fg: Color32,
    pub(super) muted: Color32,
    pub(super) hover: Color32,
    /// `--border-strong`: the scrollbar's thumb.
    pub(super) strong: Color32,
    /// `--ring`: the focus ring.
    pub(super) ring: Color32,
    /// The border over the card's own surface, solid (the git pill's).
    pub(super) edge: Color32,
}

impl Palette {
    pub(super) fn new(dark: bool) -> Self {
        if dark {
            Palette {
                bg: Color32::from_rgb(0x2c, 0x2c, 0x2e),
                border: Color32::from_rgba_unmultiplied(0x54, 0x54, 0x58, 0x80),
                fg: Color32::from_rgb(0xf5, 0xf5, 0xf7),
                muted: Color32::from_rgba_unmultiplied(0xeb, 0xeb, 0xf5, 0x99),
                hover: Color32::from_white_alpha(0x0f),
                strong: Color32::from_white_alpha(0x24),
                ring: Color32::from_rgba_unmultiplied(0x0a, 0x84, 0xff, 0x99),
                edge: Color32::from_rgb(0x40, 0x40, 0x43),
            }
        } else {
            Palette {
                bg: Color32::WHITE,
                border: Color32::from_black_alpha(0x1f),
                fg: Color32::from_rgb(0x1d, 0x1d, 0x1f),
                muted: Color32::from_rgba_unmultiplied(0x3c, 0x3c, 0x43, 0x99),
                hover: Color32::from_black_alpha(0x0d),
                strong: Color32::from_black_alpha(0x2e),
                ring: Color32::from_rgba_unmultiplied(0x00, 0x7a, 0xff, 0x80),
                edge: Color32::from_rgb(0xe0, 0xe0, 0xe0),
            }
        }
    }
}

/// The letter a git mark wears in its row.
pub(crate) fn badge(s: Status) -> char {
    match s {
        Status::Modified => 'M',
        Status::Added => 'A',
        Status::Deleted => 'D',
        Status::Renamed => 'R',
        Status::Untracked => 'U',
        Status::Conflict => '!',
    }
}

/// A git mark's color, as the IDE node's tree: its own (Islands Dark) on the dark card, IntelliJ
/// Light's on the light one. A change is blue, a new file green.
fn status_color(s: Status, dark: bool) -> Color32 {
    if dark {
        return s.color();
    }
    let hex: u32 = match s {
        Status::Modified | Status::Renamed => 0x0032a0,
        Status::Added => 0x0a7700,
        Status::Deleted => 0x616161,
        Status::Untracked => 0x993300,
        Status::Conflict => 0xff0000,
    };
    Color32::from_rgb((hex >> 16) as u8, (hex >> 8) as u8, hex as u8)
}

/// The color of a row's name: its git mark's (a folder that only lost a file is one with a change:
/// blue), the reference app's `--git-ignored` gray for what git ignores, None for the card's own.
pub(crate) fn name_color(mark: Option<(Status, bool)>, ignored: bool, dark: bool) -> Option<Color32> {
    match mark {
        Some((Status::Deleted, true)) => Some(status_color(Status::Modified, dark)),
        Some((s, _)) => Some(status_color(s, dark)),
        None => ignored.then(|| Color32::from_gray(if dark { 0x8c } else { 0x8e })),
    }
}

/// A corner radius in points.
pub(super) fn cr(r: f32) -> u8 {
    r.round().clamp(0.0, 255.0) as u8
}

/// `text` cut to `max` points wide, with an ellipsis.
pub(super) fn fit(p: &Painter, text: &str, font: &FontId, max: f32) -> String {
    let width = |s: &str| p.layout_no_wrap(s.to_string(), font.clone(), Color32::WHITE).size().x;
    if max <= 0.0 {
        return String::new();
    }
    if width(text) <= max {
        return text.into();
    }
    let mut s = text.to_string();
    while !s.is_empty() && width(&format!("{s}…")) > max {
        s.pop();
    }
    format!("{s}…")
}

/// Where the header's parts are.
struct Head {
    back: Rect,
    forward: Rect,
    name: Rect,
    view: Rect,
    pill: Option<(Rect, String)>,
}

/// The header of a card in `frame` at zoom `z`: back and forward, the name, the view menu and the
/// git pill (`h-10`, `px-2`, `gap-1.5`).
fn head(p: &Painter, frame: Rect, z: f32, git: &TreeGit) -> Head {
    let (pad, gap, size) = (6.5 * z, 4.875 * z, 19.5 * z);
    let cy = frame.top() + FT_BAR * z / 2.0;
    let slot = |left: f32, width: f32| Rect::from_center_size(Pos2::new(left + width / 2.0, cy), Vec2::new(width, size));
    let back = slot(frame.left() + pad, size);
    let forward = slot(back.right() + z, size);
    let mut right = frame.right() - pad;
    let pill = git.repo.then(|| {
        let font = FontId::proportional(11.0 * z);
        let branch = fit(p, if git.branch.is_empty() { "HEAD" } else { &git.branch }, &font, 78.0 * z);
        let text = |s: &str| p.layout_no_wrap(s.to_string(), font.clone(), Color32::WHITE).size().x;
        let count = if git.changes > 0 { 3.25 * z + text(&git.changes.to_string()) } else { 0.0 };
        let width = 2.0 * pad + 2.0 * z + 9.75 * z + 3.25 * z + text(&branch) + count;
        right -= width;
        let rect = slot(right, width);
        right -= gap;
        (rect, branch)
    });
    let view = slot(right - 32.5 * z, 32.5 * z);
    let name = Rect::from_min_max(Pos2::new(forward.right() + gap, cy - size / 2.0), Pos2::new(view.left() - gap, cy + size / 2.0));
    Head { back, forward, name, view, pill }
}

/// A lucide-style outline from points on its 24-grid, scaled into a square of `size` at `c`.
pub(super) fn outline(p: &Painter, c: Pos2, size: f32, grid: &[(f32, f32)], close: bool, stroke: Stroke) {
    let points: Vec<Pos2> = grid.iter().map(|(x, y)| c + Vec2::new(x - 12.0, y - 12.0) * (size / 24.0)).collect();
    p.add(if close { Shape::closed_line(points, stroke) } else { Shape::line(points, stroke) });
}

/// A chevron in a box of `size`: pointing right, or down when `open`; left for `left`.
fn chevron(p: &Painter, c: Pos2, size: f32, dir: u8, stroke: Stroke) {
    let grid: [(f32, f32); 3] = match dir {
        0 => [(9.0, 18.0), (15.0, 12.0), (9.0, 6.0)],
        1 => [(6.0, 9.0), (12.0, 15.0), (18.0, 9.0)],
        _ => [(15.0, 18.0), (9.0, 12.0), (15.0, 6.0)],
    };
    outline(p, c, size, &grid, false, stroke);
}

/// Something a press in the node asks of the app (applied after the areas are drawn).
pub(super) enum Act {
    Focus,
    Select(PathBuf),
    Toggle(PathBuf),
    Activate(PathBuf),
    Into(PathBuf),
    Back,
    Forward,
    Hidden,
    Sort(SortBy),
    Reverse,
    CollapseAll,
    CopyPath(PathBuf, bool),
    Copy(PathBuf),
    Cut(PathBuf),
    Paste(PathBuf),
    Ask(TreeAsk),
    ToggleEditor,
}

/// An area of the node over its card, clipped to the canvas.
pub(super) fn area<R>(ctx: &Context, key: (&str, &str), rect: Rect, clip: Rect, add: impl FnOnce(&mut egui::Ui) -> R) -> R {
    egui::Area::new(egui::Id::unique(key))
        .fixed_pos(rect.min)
        .constrain(false)
        .order(egui::Order::Middle)
        .show(ctx, |ui| {
            ui.set_clip_rect(rect.intersect(clip));
            ui.allocate_exact_size(rect.size(), Sense::hover());
            add(ui)
        })
        .inner
}

impl DrawingApp {
    /// The card on the canvas: its frame, the line under the header and the folder's name. The live
    /// parts are over it (`filetree_overlay`) while the node is big enough on screen.
    pub(crate) fn paint_filetree(&self, painter: &Painter, element: &Element, rect: Rect, z: f32) {
        let Some(spec) = &element.filetree else { return };
        let c = Palette::new(self.dark());
        painter.rect(rect, cr(8.125 * z), c.bg, Stroke::new(1.0, c.border), StrokeKind::Inside);
        let git = self.filetrees.views.get(&element.id).map(|v| v.git.clone()).unwrap_or_default();
        // the tree's column: the whole card, or what the editor pane leaves
        let (tree, pane) = parts(spec, rect, z);
        let h = head(painter, tree, z, &git);
        let line = rect.top() + FT_BAR * z;
        painter.hline(rect.left() + 1.0..=tree.right() - 1.0, line, Stroke::new(1.0, c.border));
        if pane.is_some() {
            painter.vline(tree.right() + 0.5 * z, rect.top() + 1.0..=rect.bottom() - 1.0, Stroke::new(1.0, c.border));
        }
        let font = FontId::proportional(13.0 * z);
        let name = fit(painter, &spec.name(), &font, h.name.width());
        // ponytail: the app has one weight of its font; the reference's medium name is struck twice, a hair apart
        for dx in [0.0, 0.45 * z] {
            painter.text(h.name.left_center() + Vec2::new(dx, 0.0), egui::Align2::LEFT_CENTER, &name, font.clone(), c.fg);
        }
    }

    /// Every file tree's live parts, over its card.
    pub(crate) fn filetree_overlay(&mut self, ctx: &Context) {
        // enlarged, only the enlarged one (when it is a tree)
        let lifted = self.terminal_elevated.clone();
        if lifted.as_ref().is_some_and(|id| !self.scene.elements.iter().any(|e| &e.id == id && e.filetree.is_some())) {
            return;
        }
        let (dark, canvas) = (self.dark(), self.canvas_rect);
        let nodes: Vec<(String, FileTreeSpec, Rect, f32)> = self
            .scene
            .elements
            .iter()
            .filter(|e| lifted.as_ref().is_none_or(|id| *id == e.id))
            .filter_map(|e| {
                let (frame, z) = self.terminal_frame(e);
                Some((e.id.clone(), e.filetree.clone()?, frame, z))
            })
            .filter(|(_, _, frame, z)| *z >= 0.3 && frame.width() * 1.0 >= 150.0 && frame.intersects(canvas))
            .collect();
        for (id, spec, frame, z) in nodes {
            let Some(mut view) = self.filetrees.views.remove(&id) else { continue };
            let mut acts = Vec::new();
            let clip = self.filetrees.clipboard.as_ref().map(|(paths, cut)| (paths.len(), *cut));
            let focused = self.card_focus.as_deref() == Some(id.as_str());
            let (tree, pane) = parts(&spec, frame, z);
            draw(ctx, &id, &spec, tree, z, canvas, dark, clip, focused, &mut view, &mut acts);
            // a name is asked in a small popover under the row it is about
            let pending = match &self.filetrees.ask {
                Some(TreeAsk::Name { tree, at, .. }) if *tree == id => Some(at.clone()),
                _ => None,
            };
            if let (Some(at), Some(mut ask)) = (pending, self.filetrees.ask.clone()) {
                match name_popover(ctx, &mut ask, ask_anchor(&view, tree, z, &at), dark) {
                    Popover::Open => self.filetrees.ask = Some(ask),
                    Popover::Commit(text) => {
                        self.filetrees.ask = None;
                        self.ft_answer(ask, text);
                    }
                    Popover::Cancel => self.filetrees.ask = None,
                }
            }
            self.filetrees.views.insert(id.clone(), view);
            // the editor pane, when shown
            if let Some(pane) = pane {
                let mut ed = self.filetrees.editors.remove(&id).unwrap_or_else(|| Editor::new(ctx.clone()));
                let mut pane_acts = Vec::new();
                pane_ui(ctx, &id, frame, tree, pane, z, canvas, dark, &mut ed, &mut pane_acts);
                self.filetrees.editors.insert(id.clone(), ed);
                for act in pane_acts {
                    self.ft_pane_apply(&id, act);
                }
            }
            for act in acts {
                self.ft_apply(ctx, &id, act);
            }
        }
        if !matches!(self.filetrees.ask, Some(TreeAsk::Name { .. })) {
            ctx.data_mut(|d| d.remove::<bool>(egui::Id::unique("ft_name_popover")));
        }
    }
}

/// Where the popover hangs: under the row of `at` (the file renamed, or the folder asked in) when it
/// is listed, otherwise under the header.
fn ask_anchor(view: &TreeView, frame: Rect, z: f32, at: &Path) -> Rect {
    let top = frame.top() + FT_BAR * z + 1.0;
    let row = Some(at).filter(|_| view.filter.trim().is_empty()).and_then(|at| view.rows.iter().position(|r| r.path == at));
    match row {
        Some(i) => Rect::from_min_size(Pos2::new(frame.left() + 4.0, top + (i as f32 * FT_ROW - view.scroll) * z), Vec2::new(frame.width() - 8.0, FT_ROW * z)),
        None => Rect::from_min_size(Pos2::new(frame.left() + 4.0, top - 6.0), Vec2::new(frame.width() - 8.0, 0.0)),
    }
}

/// A row of a filename search's results (two lines), and the note over and under them.
const FT_HIT: f32 = 38.0;
const FT_NOTE: f32 = 23.0;
/// The scrollbar's width (`::-webkit-scrollbar`).
const FT_SCROLL: f32 = 8.0;

/// "N ocorrências", as the reference's header over the results.
pub(crate) fn hits_note(n: usize) -> String {
    if n == 1 { "1 ocorrência".into() } else { format!("{n} ocorrências") }
}

/// The right-click menu of `r`: the reference's popover surface.
fn context(r: &egui::Response, dark: bool, add: impl FnOnce(&mut egui::Ui)) {
    egui::Popup::context_menu(r).frame(crate::app::ui::menu_look::menu_frame(dark)).show(add);
}

/// One node's areas: the header's buttons, then the rows with the search bar under them.
#[allow(clippy::too_many_arguments)]
fn draw(
    ctx: &Context,
    id: &str,
    spec: &FileTreeSpec,
    frame: Rect,
    z: f32,
    canvas: Rect,
    dark: bool,
    clip_board: Option<(usize, bool)>,
    focused: bool,
    view: &mut TreeView,
    acts: &mut Vec<Act>,
) {
    let c = Palette::new(dark);
    let clip = frame.intersect(canvas);
    let measure = ctx.layer_painter(egui::LayerId::new(egui::Order::Middle, egui::Id::unique(("ft_measure", id))));
    let h = head(&measure, frame, z, &view.git);
    let stroke = |color: Color32| Stroke::new((1.0 * z).max(0.75), color);
    let (root, shown) = (spec.root_path(), spec.shown());
    // back (up one folder) and forward
    let nav = Rect::from_min_max(h.back.min, h.forward.max);
    area(ctx, ("ft_nav", id), nav, clip, |ui| {
        let p = ui.painter().clone();
        let divider = Rect::from_center_size(Pos2::new((h.back.right() + h.forward.left()) / 2.0, nav.center().y), Vec2::new(1.0, 11.375 * z));
        p.rect_filled(divider, 0, c.border);
        for (rect, dir, can, tip, act) in [(h.back, 2, shown != root, "Voltar", Act::Back), (h.forward, 0, !view.forward.is_empty(), "Avançar", Act::Forward)]
        {
            let r = ui.interact(rect, ui.scope_id().with(tip), Sense::click());
            if r.hovered() && can {
                p.rect_filled(rect, cr(rect.width() / 2.0), c.hover);
            }
            let color = if can { c.fg } else { c.fg.gamma_multiply(0.3) };
            chevron(&p, rect.center(), 13.0 * z, dir, stroke(color));
            if r.on_hover_cursor(if can { CursorIcon::PointingHand } else { CursorIcon::Default }).tip(tip).clicked() && can {
                acts.push(act);
                acts.push(Act::Focus);
            }
        }
    });
    // the view menu and the git pill
    let tools = h.pill.as_ref().map_or(h.view, |(pill, _)| h.view.union(*pill));
    let expanded = !view.expanded.is_empty();
    area(ctx, ("ft_tools", id), tools, clip, |ui| {
        let p = ui.painter().clone();
        let r = ui.interact(h.view, ui.scope_id().with("view"), Sense::click()).on_hover_cursor(CursorIcon::PointingHand);
        // no tip over its own menu
        let r = if egui::Popup::is_any_open(ui.ctx()) { r } else { r.tip("Opções de Exibição") };
        if r.hovered() {
            p.rect_filled(h.view, cr(h.view.height() / 2.0), c.hover);
        }
        // lucide's list (size-4) and chevrons-up-down (size-2.5)
        paint_g(&p, G::List, Pos2::new(h.view.left() + 4.875 * z + 6.5 * z, h.view.center().y), 13.0 * z, c.fg);
        let updown = Pos2::new(h.view.right() - 4.875 * z - 4.0625 * z, h.view.center().y);
        outline(&p, updown, 8.125 * z, &[(7.0, 15.0), (12.0, 20.0), (17.0, 15.0)], false, stroke(c.muted));
        outline(&p, updown, 8.125 * z, &[(7.0, 9.0), (12.0, 4.0), (17.0, 9.0)], false, stroke(c.muted));
        egui::Popup::menu(&r)
            .align(egui::RectAlign::BOTTOM_END)
            .gap(6.0)
            .frame(crate::app::ui::menu_look::menu_frame(dark))
            .close_behavior(egui::PopupCloseBehavior::CloseOnClick)
            .show(|ui| view_menu(ui, spec, expanded, acts));
        if let Some((rect, branch)) = &h.pill {
            let r = ui.interact(*rect, ui.scope_id().with("pill"), Sense::hover());
            p.rect(*rect, cr(rect.height() / 2.0), Color32::TRANSPARENT, Stroke::new(1.0, c.edge), StrokeKind::Inside);
            // lucide's git-branch (size-3)
            let icon = Pos2::new(rect.left() + 6.5 * z + z + 4.875 * z, rect.center().y);
            let size = 9.75 * z;
            let tint = stroke(c.muted);
            outline(&p, icon, size, &[(6.0, 3.0), (6.0, 15.0)], false, tint);
            outline(&p, icon, size, &arc(18.0, 6.0, 3.0, 0.0, 360.0), true, tint);
            outline(&p, icon, size, &arc(6.0, 18.0, 3.0, 0.0, 360.0), true, tint);
            outline(&p, icon, size, &arc(9.0, 9.0, 9.0, 0.0, 90.0), false, tint);
            let font = FontId::proportional(11.0 * z);
            let text =
                p.text(Pos2::new(icon.x + 4.875 * z + 3.25 * z, rect.center().y), egui::Align2::LEFT_CENTER, branch, font.clone(), c.fg.gamma_multiply(0.8));
            if view.git.changes > 0 {
                p.text(Pos2::new(text.right() + 3.25 * z, rect.center().y), egui::Align2::LEFT_CENTER, view.git.changes.to_string(), font, c.muted);
            }
            let n = view.git.changes;
            r.tip(format!("Branch {}, {} arquivo{} alterado{}", view.git.branch, n, if n == 1 { "" } else { "s" }, if n == 1 { "" } else { "s" }));
        }
    });
    // the rows and the search bar: inset from the edges, which stay the canvas's to resize by
    let searching = !view.filter.trim().is_empty();
    let foot = FT_BAR * z + if view.dropped > 0 { FT_NOTE * z } else { 0.0 };
    let body = Rect::from_min_max(Pos2::new(frame.left() + 4.0, frame.top() + FT_BAR * z + 1.0), Pos2::new(frame.right() - 4.0, frame.bottom() - 4.0));
    let list = Rect::from_min_max(body.min, Pos2::new(body.right(), body.bottom() - foot + 4.0));
    // a search's results have their count over them, and two-line rows
    let (row_h, note_h) = if searching { (FT_HIT, if view.rows.is_empty() { 0.0 } else { FT_NOTE }) } else { (FT_ROW, 0.0) };
    let cut_h = if searching && view.hits_cut { FT_NOTE } else { 0.0 };
    let top = list.top() + note_h * z;
    let shown_h = (list.bottom() - top) / z;
    view.body_h = shown_h;
    let total = view.rows.len() as f32 * row_h + cut_h;
    let bar_w = if total > shown_h { FT_SCROLL * z } else { 0.0 };
    area(ctx, ("ft_body", id), body, clip, |ui| {
        let p = ui.painter().with_clip_rect(clip);
        if ui.input(|i| i.pointer.any_pressed()) && ui.rect_contains_pointer(body) {
            acts.push(Act::Focus);
        }
        if ui.rect_contains_pointer(list) {
            view.scroll -= ui.input(|i| i.smooth_scroll_delta.y) / z;
        }
        if let Some((path, left)) = view.reveal.take() {
            match view.rows.iter().position(|r| r.path == path) {
                Some(i) => view.scroll = (i as f32 * row_h - (shown_h - row_h) / 2.0).max(0.0),
                None if left > 1 => view.reveal = Some((path, left - 1)),
                None => {}
            }
        }
        view.scroll = view.scroll.clamp(0.0, (total - shown_h).max(0.0));
        // empty space: the menu of the folder shown
        let blank = ui.interact(list, ui.scope_id().with("blank"), Sense::click());
        context(&blank, dark, |ui| row_menu(ui, id, None, &shown, clip_board, acts));
        if let Some(note) = view.empty_note(spec) {
            if searching {
                // the reference centers it
                let galley = p.layout(note, FontId::proportional(13.0 * z), c.muted, list.width() - 19.5 * z);
                p.galley(list.center() - galley.size() / 2.0, galley, c.muted);
            } else {
                let galley = p.layout(note, FontId::proportional(12.0 * z), c.muted, list.width() - 19.5 * z);
                p.galley(Pos2::new(list.left() + 9.75 * z, list.top() + 6.5 * z), galley, c.muted);
            }
        }
        let width = list.width() - bar_w;
        let first = (view.scroll / row_h).floor() as usize;
        let last = (((view.scroll + shown_h) / row_h).ceil() as usize + 1).min(view.rows.len());
        let list_clip = Rect::from_min_max(Pos2::new(list.left(), top), list.right_bottom()).intersect(clip);
        let ring = focused && view.kb;
        if note_h > 0.0 {
            let galley = p.layout_no_wrap(hits_note(view.rows.len()), FontId::proportional(11.0 * z), c.muted);
            p.galley(Pos2::new(list.left() + 8.125 * z, list.top() + (note_h * z - galley.size().y) / 2.0), galley, c.muted);
            p.hline(frame.left() + 1.0..=frame.right() - 1.0, list.top() + note_h * z, Stroke::new(1.0, c.border));
        }
        for (i, row) in view.rows.iter().enumerate().take(last).skip(first) {
            let rect = Rect::from_min_size(Pos2::new(list.left(), top + (i as f32 * row_h - view.scroll) * z), Vec2::new(width, row_h * z));
            let hit = rect.intersect(list_clip);
            if hit.height() < 1.0 {
                continue;
            }
            let r = ui.interact(hit, ui.scope_id().with(("row", i)), Sense::click());
            let p = p.with_clip_rect(list_clip);
            let selected = view.selected.as_ref() == Some(&row.path);
            if selected || (searching && r.hovered()) {
                p.rect_filled(rect, cr(4.0 * z), c.hover);
            }
            if selected && ring {
                p.rect_stroke(rect, cr(4.0 * z), Stroke::new(2.0 * z, c.ring), StrokeKind::Inside);
            }
            let cy = rect.center().y;
            if searching {
                // the file's icon (the IDE's, 13 px here), its name and, under it, the folder it is in
                let icon = Pos2::new(rect.left() + 8.125 * z + 5.6875 * z, cy);
                icons::file_on(&p, &row.path, icon, 0.8125 * z, !dark);
                let left = rect.left() + 8.125 * z + 11.375 * z + 6.5 * z;
                let font = FontId::proportional(12.0 * z);
                let name = fit(&p, &row.name, &font, rect.right() - 8.125 * z - left);
                // ponytail: every hit is two lines high, a file in the root too; the reference's is one when it has no folder
                let line = if row.hint.is_empty() { cy } else { cy - 7.35 * z };
                p.text(Pos2::new(left, line), egui::Align2::LEFT_CENTER, name, font, c.fg);
                if !row.hint.is_empty() {
                    let font = FontId::proportional(10.5 * z);
                    p.text(Pos2::new(left, cy + 8.4 * z), egui::Align2::LEFT_CENTER, fit(&p, &row.hint, &font, rect.right() - 8.125 * z - left), font, c.muted);
                }
            } else {
                let mark = view.git.of(&row.path, row.dir);
                let color = name_color(mark, view.git.ignored(&row.path), dark);
                // the indentation's guides: a line down from each folder above, under its chevron
                for level in 0..row.depth {
                    p.vline(rect.left() + (4.0 + level as f32 * 16.0 + 6.5) * z, rect.y_range(), Stroke::new(1.0, c.border));
                }
                let slot = Rect::from_center_size(Pos2::new(rect.left() + (4.0 + row.depth as f32 * 16.0) * z + 6.5 * z, cy), Vec2::splat(13.0 * z));
                if row.dir {
                    let chev = ui.interact(slot.intersect(list_clip), ui.scope_id().with(("chevron", i)), Sense::click());
                    if chev.hovered() {
                        p.rect_filled(slot, cr(2.0 * z), c.fg.gamma_multiply(0.1));
                    }
                    chevron(&p, slot.center(), 9.75 * z, if row.open { 1 } else { 0 }, stroke(c.muted));
                    if chev.clicked() {
                        acts.push(Act::Toggle(row.path.clone()));
                        acts.push(Act::Focus);
                    }
                }
                // the IDE's icons, in their own colors: only the name wears the git mark
                let icon = Pos2::new(slot.right() + 4.875 * z + 6.5 * z, cy);
                if row.dir {
                    icons::paint_on(&p, icons::FOLDER, Rect::from_center_size(icon, Vec2::splat(16.0 * z)), !dark);
                } else {
                    icons::file_on(&p, &row.path, icon, z, !dark);
                }
                let badge_w = if mark.is_some_and(|(_, rolled)| !rolled) { 16.25 * z + 4.875 * z } else { 0.0 };
                let left = icon.x + 6.5 * z + 4.875 * z;
                let font = FontId::proportional(13.0 * z);
                let name = fit(&p, &row.name, &font, rect.right() - 6.5 * z - badge_w - left);
                let text = p.text(Pos2::new(left, cy), egui::Align2::LEFT_CENTER, name, font, color.unwrap_or(c.fg));
                if !row.hint.is_empty() {
                    let font = FontId::proportional(11.0 * z);
                    let room = rect.right() - 6.5 * z - badge_w - text.right() - 6.0 * z;
                    p.text(Pos2::new(text.right() + 6.0 * z, cy), egui::Align2::LEFT_CENTER, fit(&p, &row.hint, &font, room), font, c.muted);
                }
                if let Some((s, false)) = mark {
                    // the reference's 10px bold mono letter: smaller in the app's wider one, struck twice for the weight
                    for dx in [0.0, 0.35 * z] {
                        p.text(
                            Pos2::new(rect.right() - 6.5 * z - 8.125 * z + dx, cy),
                            egui::Align2::CENTER_CENTER,
                            badge(s),
                            FontId::monospace(8.4 * z),
                            status_color(s, dark),
                        );
                    }
                }
            }
            if r.clicked() || r.secondary_clicked() {
                view.kb = false;
                acts.push(Act::Select(row.path.clone()));
                acts.push(Act::Focus);
            }
            if r.double_clicked() {
                acts.push(Act::Activate(row.path.clone()));
            }
            context(&r, dark, |ui| row_menu(ui, id, Some(row), &shown, clip_board, acts));
        }
        if cut_h > 0.0 {
            let note = Rect::from_min_size(Pos2::new(list.left(), top + (view.rows.len() as f32 * row_h - view.scroll) * z), Vec2::new(width, cut_h * z));
            if note.intersects(list_clip) {
                let p = p.with_clip_rect(list_clip);
                p.hline(frame.left() + 1.0..=frame.right() - 1.0, note.top(), Stroke::new(1.0, c.border));
                p.text(
                    Pos2::new(note.left() + 8.125 * z, note.center().y),
                    egui::Align2::LEFT_CENTER,
                    format!("Parou em {}. Existem mais.", hits_note(view.rows.len())),
                    FontId::proportional(11.0 * z),
                    c.muted,
                );
            }
        }
        // the scrollbar: 8 wide, its thumb the reference's `--border-strong`
        if bar_w > 0.0 {
            let track = Rect::from_min_max(Pos2::new(list.right() - bar_w, top), Pos2::new(list.right(), list.bottom()));
            let span = (total - shown_h).max(1.0);
            let thumb_h = (shown_h / total * track.height()).max(20.0 * z).min(track.height());
            let thumb = Rect::from_min_size(Pos2::new(track.left(), track.top() + view.scroll / span * (track.height() - thumb_h)), Vec2::new(bar_w, thumb_h));
            let drag = ui.interact(thumb.intersect(clip), ui.scope_id().with("thumb"), Sense::drag());
            if drag.dragged() {
                view.scroll = (view.scroll + drag.drag_delta().y / (track.height() - thumb_h).max(1.0) * span).clamp(0.0, span);
            }
            let color = if drag.hovered() || drag.dragged() { c.muted } else { c.strong };
            p.rect_filled(thumb, cr(4.0 * z), color);
        }
        // the search bar under the rows
        let bar = Rect::from_min_size(Pos2::new(frame.left(), list.bottom()), Vec2::new(frame.width(), FT_BAR * z));
        p.hline(frame.left() + 1.0..=frame.right() - 1.0, bar.top(), Stroke::new(1.0, c.border));
        let mut edge = bar.right() - 6.5 * z;
        // the editor's button, at the bar's end: lit while the pane is shown
        let toggle = Rect::from_center_size(Pos2::new(edge - 11.375 * z, bar.center().y), Vec2::new(22.75 * z, 19.5 * z));
        let r = ui.interact(toggle, ui.scope_id().with("editor"), Sense::click()).on_hover_cursor(CursorIcon::PointingHand);
        let label = if spec.editor { "Ocultar Editor" } else { "Mostrar Editor" };
        if r.hovered() {
            p.rect_filled(toggle, cr(6.0 * z), c.hover);
        }
        let accent = if dark { Color32::from_rgb(0x0a, 0x84, 0xff) } else { Color32::from_rgb(0x00, 0x7a, 0xff) };
        paint_g(&p, G::PanelRight, toggle.center(), 11.375 * z, if spec.editor { accent } else { c.muted });
        if r.tip(label).clicked() {
            acts.push(Act::ToggleEditor);
            acts.push(Act::Focus);
        }
        edge = toggle.left() - 4.875 * z;
        // the count of the tree's rows, outside the field
        if searching {
            let count = format!("{} exibidos", view.tree_rows);
            let galley = p.layout_no_wrap(count, FontId::proportional(11.0 * z), c.muted);
            p.galley(Pos2::new(edge - galley.size().x, bar.center().y - galley.size().y / 2.0), galley.clone(), c.muted);
            edge -= galley.size().x + 6.5 * z;
        }
        let pill = Rect::from_min_max(Pos2::new(bar.left() + 6.5 * z, bar.center().y - 9.75 * z), Pos2::new(edge, bar.center().y + 9.75 * z));
        p.rect_filled(pill, cr(pill.height() / 2.0), c.hover);
        let lens = Pos2::new(pill.left() + 6.5 * z + 4.875 * z, pill.center().y);
        p.circle_stroke(lens + Vec2::new(-0.6 * z, -0.6 * z), 3.4 * z, stroke(c.muted));
        p.line_segment([lens + Vec2::new(1.9 * z, 1.9 * z), lens + Vec2::new(4.4 * z, 4.4 * z)], stroke(c.muted));
        let mut right = pill.right() - 6.5 * z;
        if !view.filter.is_empty() {
            let x = Rect::from_center_size(Pos2::new(right - 6.5 * z, pill.center().y), Vec2::splat(13.0 * z));
            let r = ui.interact(x, ui.scope_id().with("clear"), Sense::click()).on_hover_cursor(CursorIcon::PointingHand).tip("Limpar filtro");
            for (a, b) in [((-2.5, -2.5), (2.5, 2.5)), ((-2.5, 2.5), (2.5, -2.5))] {
                p.line_segment([x.center() + Vec2::new(a.0, a.1) * z, x.center() + Vec2::new(b.0, b.1) * z], stroke(c.muted));
            }
            if r.clicked() {
                view.filter.clear();
                view.dirty = true;
            }
            right = x.left() - 4.875 * z;
        }
        let field = Rect::from_min_max(Pos2::new(lens.x + 9.75 * z / 2.0 + 4.875 * z, pill.top()), Pos2::new(right, pill.bottom()));
        let edit = egui::TextEdit::singleline(&mut view.filter)
            .frame(egui::Frame::NONE)
            .margin(egui::Margin::ZERO)
            .align(egui::Align2::LEFT_CENTER)
            .font(FontId::proportional(12.0 * z))
            .text_color(c.fg)
            .hint_text(egui::RichText::new("Buscar").size(12.0 * z).color(c.muted))
            .desired_width(field.width());
        // as tall as its text, in the middle of the pill
        let line = Rect::from_center_size(field.center(), Vec2::new(field.width(), 15.0 * z));
        let typed = ui.scope(|ui| {
            ui.spacing_mut().interact_size.y = 0.0;
            ui.put(line, edit).changed()
        });
        if typed.inner {
            view.dirty = true;
            view.scroll = 0.0;
        }
        if view.dropped > 0 {
            let note = Rect::from_min_size(Pos2::new(frame.left(), bar.bottom()), Vec2::new(frame.width(), FT_NOTE * z));
            p.hline(frame.left() + 1.0..=frame.right() - 1.0, note.top(), Stroke::new(1.0, c.border));
            let text = format!("Exibindo um número limitado de itens. Mais {} não estão listados.", view.dropped);
            let font = FontId::proportional(11.0 * z);
            p.text(
                Pos2::new(note.left() + 8.125 * z, note.center().y),
                egui::Align2::LEFT_CENTER,
                fit(&p, &text, &font, note.width() - 16.25 * z),
                font,
                c.muted,
            );
        }
    });
}
