//! The file tree node's face. The canvas paints the card (frame, title); the live parts (back and
//! forward, the view menu, the git pill, the rows, the search bar) sit over it in small areas, so
//! the title strip is still the canvas's and drags the node. Sizes are the reference app's, at
//! 13 px to the rem, times the zoom.

use crate::app::filetree::menu::{G, Popover, arc, name_popover, paint_g, row_menu, view_menu};
use crate::app::filetree::pane::{Editor, pane_ui, parts};
use crate::app::filetree::problems::propagate;
use crate::app::filetree::problems_ui::{error_color, wave};
use crate::app::filetree::*;
use crate::ide::{Status, icons};
use crate::prelude::*;
use std::path::Path;
mod draw;
use draw::*;

/// The reference app's `--bg`, `--border`, `--foreground`, `--muted-foreground`, `--bg-hover`.
pub(crate) struct Palette {
    pub(crate) bg: Color32,
    pub(crate) border: Color32,
    pub(crate) fg: Color32,
    pub(crate) muted: Color32,
    pub(crate) hover: Color32,
    /// `--border-strong`: the scrollbar's thumb.
    pub(crate) strong: Color32,
    /// `--ring`: the focus ring.
    pub(crate) ring: Color32,
    /// The border over the card's own surface, solid (the git pill's).
    pub(crate) edge: Color32,
}

impl Palette {
    pub(crate) fn new(dark: bool) -> Self {
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
pub(crate) fn status_color(s: Status, dark: bool) -> Color32 {
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
pub(crate) fn cr(r: f32) -> u8 {
    r.round().clamp(0.0, 255.0) as u8
}

/// `text` cut to `max` points wide, with an ellipsis.
pub(crate) fn fit(p: &Painter, text: &str, font: &FontId, max: f32) -> String {
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
pub(crate) fn outline(p: &Painter, c: Pos2, size: f32, grid: &[(f32, f32)], close: bool, stroke: Stroke) {
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
pub(crate) enum Act {
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
    ToggleDb,
    ToggleChanges,
    /// A branch picked in the chip's menu: check it out.
    Branch(String),
    /// "Mostrar Diff" of a file.
    Diff(PathBuf),
}

/// An area of the node over its card, clipped to the canvas.
pub(crate) fn area<R>(ctx: &Context, key: (&str, &str), rect: Rect, clip: Rect, add: impl FnOnce(&mut egui::Ui) -> R) -> R {
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
        // enlarged: a press outside the node, on the bare canvas, puts it back (as the enlarged terminal's margin)
        if let Some(element) = lifted.as_ref().and_then(|id| self.scene.elements.iter().find(|e| &e.id == id)) {
            let frame = self.terminal_frame(element).0;
            let press = ctx
                .input(|i| (i.pointer.primary_pressed() || i.pointer.secondary_pressed()).then(|| i.pointer.interact_pos()).flatten())
                .filter(|_| !self.panning);
            if self.dock.is_none() && press.is_some_and(|p| !frame.contains(p) && ctx.layer_id_at(p).is_none_or(|l| l.order == egui::Order::Background)) {
                self.set_elevated(None);
                return;
            }
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
            // the files with an error and the folders over them
            view.problems = self.filetrees.editors.get(&id).map(|ed| propagate(&ed.problems.error_files(), &spec.root_path())).unwrap_or_default();
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
            // the Banco de Dados panel over the rows (the search bar and its buttons stay)
            let foot = FT_BAR * z + if view.dropped > 0 { FT_NOTE * z } else { 0.0 };
            self.filetrees.views.insert(id.clone(), view);
            if spec.db {
                let list = Rect::from_min_max(Pos2::new(tree.left() + 4.0, tree.top() + FT_BAR * z + 1.0), Pos2::new(tree.right() - 4.0, tree.bottom() - foot));
                let mut db = self.filetrees.dbs.remove(&id).unwrap_or_else(|| crate::db::Db::with_store(self.filetrees.db_store.clone()));
                let events = crate::app::filetree::db::db_ui(ctx, &id, list, tree.intersect(canvas), z, &mut db, self.filetrees.editors.get(&id));
                self.filetrees.dbs.insert(id.clone(), db);
                // the sources' window, over everything
                let more = self.filetrees.dbs.get_mut(&id).map(|db| db.dialog(ctx)).unwrap_or_default();
                self.ft_db_events(&id, events.into_iter().chain(more).collect());
            }
            // the Alterações panel over the rows, in the Banco de Dados' place
            if spec.changes {
                let list = Rect::from_min_max(Pos2::new(tree.left() + 4.0, tree.top() + FT_BAR * z + 1.0), Pos2::new(tree.right() - 4.0, tree.bottom() - foot));
                let files = self.filetrees.views.get(&id).map(|v| v.git.files.clone()).unwrap_or_default();
                if let Some(file) = crate::app::filetree::changes::changes_ui(ctx, &id, list, tree.intersect(canvas), z, dark, &spec.root_path(), &files) {
                    self.ft_open_change(&id, &file);
                }
            }
            // the editor pane, when shown
            if let Some(pane) = pane {
                let mut ed = self.filetrees.editors.remove(&id).unwrap_or_else(|| Editor::new(ctx.clone()));
                let mut pane_acts = Vec::new();
                pane_ui(ctx, &id, frame, tree, pane, z, canvas, dark, lifted.is_some(), &mut ed, &mut pane_acts);
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
