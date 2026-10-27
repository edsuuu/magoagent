//! The IDE node's card on the canvas: the folder tree, and beside it the file panel once a file
//! is open. Both are painted and hit in the card's own points (world units), so they scale with
//! the canvas and take no focus.

use super::*;
use eframe::egui::{Align2, Painter, Pos2, Stroke, StrokeKind, Vec2};

/// One tree row, one line of the panel, the card's title strip, the tree's top under it, the
/// footer, the panel's tab strip, its line-number gutter, and how far the node grows for the panel.
pub(crate) const ROW: f32 = 22.0;
pub(crate) const LINE: f32 = 16.0;
pub(crate) const HEADER: f32 = 26.0;
pub(crate) const TREE_TOP: f32 = 34.0;
pub(crate) const FOOTER: f32 = 22.0;
pub(crate) const PANEL_TABS: f32 = 30.0;
pub(crate) const PANEL_W: f32 = 520.0;
const GUTTER: f32 = 44.0;
/// How narrow a panel tab gets when many are open: its close button and a few letters.
const TAB_MIN: f32 = 36.0;

/// The card's parts from its world box: the tree column, and the panel while `tree_width` is set
/// (a card resized narrower than its tree has the tree only).
pub(crate) fn parts(local: Rect, tree_width: Option<f32>) -> (Rect, Option<Rect>) {
    let split = local.left() + tree_width.unwrap_or(local.width()).min(local.width());
    let tree = Rect::from_min_max(local.min, Pos2::new(split, local.bottom()));
    let panel = tree_width.map(|_| Rect::from_min_max(Pos2::new(split, local.top()), local.max));
    (tree, panel)
}

/// The panel's text area: below its tab strip and above the footer.
pub(crate) fn panel_body(panel: Rect) -> Rect {
    Rect::from_min_max(Pos2::new(panel.left(), panel.top() + HEADER + PANEL_TABS), Pos2::new(panel.right(), panel.bottom() - FOOTER))
}

/// A panel tab's close button, at its right end.
pub(crate) fn tab_close(tab: Rect) -> Rect {
    Rect::from_center_size(Pos2::new(tab.right() - 11.0, tab.center().y), Vec2::splat(14.0))
}

impl Ide {
    /// The panel's tabs: one per open text file, with its index in `tabs` (each closes on its ×;
    /// the last one closed takes the panel away).
    pub(crate) fn panel_tabs(&self, panel: Rect) -> Vec<(Rect, usize)> {
        let open: Vec<(usize, f32)> = self.tabs.iter().enumerate().filter(|(_, t)| t.is_text()).map(|(i, t)| (i, 52.0 + file_name(&t.path).chars().count() as f32 * 7.0)).collect();
        // more than the strip holds: all narrow alike, their names cut short
        // ponytail: past ~12 tabs even the narrowest run off the strip; a "more" menu then
        let room = panel.width() - 16.0 - 4.0 * open.len().saturating_sub(1) as f32;
        let scale = (room / open.iter().map(|(_, w)| w).sum::<f32>()).min(1.0);
        let mut x = panel.left() + 8.0;
        open.into_iter()
            .map(|(i, w)| {
                let w = (w * scale).max(TAB_MIN);
                let tab = Rect::from_min_size(Pos2::new(x, panel.top() + HEADER + 4.0), Vec2::new(w, PANEL_TABS - 8.0));
                x += w + 4.0;
                (tab, i)
            })
            .collect()
    }

    /// The text tab the panel shows: the active one, or the last text tab while a table or a diff
    /// is active.
    pub(crate) fn card_tab(&self) -> Option<usize> {
        if self.tabs.get(self.active).is_some_and(Tab::is_text) { Some(self.active) } else { self.tabs.iter().rposition(Tab::is_text) }
    }

    /// Whether a text file is open: the panel is there only then.
    pub(crate) fn has_text_tabs(&self) -> bool {
        self.tabs.iter().any(|t| t.is_text())
    }

    /// Lines in the file the panel shows, for its scroll limit.
    pub(crate) fn active_lines(&self) -> usize {
        self.card_tab().map_or(0, |i| self.tabs[i].text.lines().count())
    }

    /// The file the panel shows.
    pub(crate) fn active_path(&self) -> Option<PathBuf> {
        self.card_tab().map(|i| self.tabs[i].path.clone())
    }

    /// The tree rows under the card's top, scrolled by `card_scroll`. `map` takes a world rect to
    /// the screen. With the keys (`focused`) the selected row is lit as active and `hover` shows.
    pub(crate) fn paint_tree(&self, painter: &Painter, tree: Rect, map: &dyn Fn(Rect) -> Rect, zoom: f32, focused: bool, hover: Option<usize>) {
        let scroll = self.card_scroll;
        let area = Rect::from_min_max(Pos2::new(tree.left(), tree.top() + TREE_TOP), Pos2::new(tree.right(), tree.bottom() - FOOTER));
        let painter = painter.with_clip_rect(map(area).intersect(painter.clip_rect()));
        for (i, (depth, name, dir, open)) in self.card_rows().into_iter().enumerate() {
            let top = tree.top() + TREE_TOP + i as f32 * ROW - scroll;
            if top > area.bottom() {
                break;
            }
            let row = map(Rect::from_min_size(Pos2::new(tree.left(), top), Vec2::new(tree.width(), ROW)));
            let fill = match (self.card_sel == Some(i), focused) {
                (true, true) => t::SELECTION,
                (true, false) => t::SELECTION_INACTIVE,
                _ if focused && hover == Some(i) => t::HOVER,
                _ => egui::Color32::TRANSPARENT,
            };
            painter.rect_filled(row, 0, fill);
            let (x, y) = (row.left() + (8.0 + depth as f32 * 16.0) * zoom, row.center().y);
            if dir {
                let chevron = if open { icons::CHEVRON_DOWN } else { icons::CHEVRON_RIGHT };
                icons::paint(&painter, chevron, Rect::from_center_size(Pos2::new(x + 6.0 * zoom, y), Vec2::splat(14.0 * zoom)));
                icons::paint(&painter, icons::FOLDER, Rect::from_center_size(Pos2::new(x + 23.0 * zoom, y), Vec2::splat(16.0 * zoom)));
            } else {
                icons::file(&painter, Path::new(&name), Pos2::new(x + 23.0 * zoom, y), zoom);
            }
            painter.text(Pos2::new(x + 36.0 * zoom, y), Align2::LEFT_CENTER, &name, t::ui(13.0 * zoom), t::TEXT);
        }
    }

    /// The panel beside the tree: a tab per open text file (the active one lit) with its ×, then
    /// the active file with line numbers, scrolled by `scroll`.
    // ponytail: the whole file is one galley; slice to the visible lines past ~a few thousand
    pub(crate) fn paint_panel(&self, painter: &Painter, panel: Rect, map: &dyn Fn(Rect) -> Rect, zoom: f32, scroll: f32) {
        let painter = &painter.with_clip_rect(map(panel).intersect(painter.clip_rect()));
        let shown = self.card_tab();
        for (r, i) in self.panel_tabs(panel) {
            let active = Some(i) == shown;
            let close = map(tab_close(r));
            let r = map(r);
            if active {
                painter.rect(r, (6.0 * zoom) as u8, t::TAB_SELECTED, Stroke::new(1.0, t::TAB_BORDER), StrokeKind::Inside);
            }
            let color = if active { t::TEXT } else { t::MUTED };
            // a narrowed tab cuts its name before the ×
            let room = Rect::from_min_max(r.min, Pos2::new(close.left() - 2.0 * zoom, r.bottom()));
            let name = painter.with_clip_rect(room.intersect(painter.clip_rect()));
            name.text(Pos2::new(r.left() + 15.0 * zoom, r.center().y), Align2::LEFT_CENTER, file_name(&self.tabs[i].path), t::ui(12.0 * zoom), color);
            icons::paint(painter, icons::CLOSE_SMALL, close);
        }
        let body = map(panel_body(panel));
        let painter = painter.with_clip_rect(body.intersect(painter.clip_rect()));
        let Some(tab) = shown.map(|i| &self.tabs[i]) else {
            painter.text(body.center(), Align2::CENTER_CENTER, "Clique num arquivo da árvore", t::ui(13.0 * zoom), t::MUTED);
            return;
        };
        let line_h = LINE * zoom;
        let origin = Pos2::new(body.left() + GUTTER * zoom, body.top() - scroll * zoom);
        // the lines an agent just opened or changed
        if let Some((_, first, last)) = self.panel_mark.as_ref().filter(|(p, ..)| *p == tab.path) {
            let band = Rect::from_min_max(Pos2::new(body.left(), origin.y + *first as f32 * line_h), Pos2::new(body.right(), origin.y + (*last + 1) as f32 * line_h));
            painter.rect_filled(band, 0, t::TAB_SELECTED);
        }
        // colored once per text and zoom (egui keeps the laid-out galley for the same job)
        let key = hash(&tab.text, 12.0 * zoom) ^ (tab.lang as *const Lang as u64);
        let mut cache = self.panel_job.borrow_mut();
        if cache.as_ref().is_none_or(|(k, _)| *k != key) {
            *cache = Some((key, highlight::layout(tab.lang, &tab.text, t::mono(12.0 * zoom), line_h)));
        }
        let galley = painter.layout_job(cache.as_ref().map(|(_, job)| job.clone()).unwrap_or_default());
        painter.galley(origin, galley, t::TEXT);
        let first = (scroll / LINE).floor().max(0.0) as usize;
        let shown = (body.height() / line_h).ceil() as usize + 1;
        for n in first..(first + shown).min(self.active_lines()) {
            let y = origin.y + (n as f32 + 0.5) * line_h;
            painter.text(Pos2::new(body.left() + (GUTTER - 8.0) * zoom, y), Align2::RIGHT_CENTER, (n + 1).to_string(), t::mono(11.0 * zoom), t::MUTED);
        }
    }
}
