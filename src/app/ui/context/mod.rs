//! Right-click menus: the canvas, a terminal, a note, a portal, a file tree and several selected
//! elements. They share the reference app's native dark look: a box, 24pt rows, a blue highlight
//! and ">" submenus that open beside their row and stay open while the pointer goes to them. Also
//! the Renomear dialog they open.

use crate::app::browser::extras::{SizeReadout, StorageScope};
use crate::app::notes::rename_note_text;
use crate::app::ui::menu_look::*;
use crate::prelude::*;
mod actions;
mod entries;
mod rename;

/// The alignments, in the order of the Alinhar submenu (the index is the id's `align:N`).
const ALIGNS: [&str; 8] = [
    "Alinhar à esquerda",
    "Centralizar horizontalmente",
    "Alinhar à direita",
    "Alinhar ao topo",
    "Centralizar verticalmente",
    "Alinhar embaixo",
    "Distribuir horizontalmente",
    "Distribuir verticalmente",
];

/// What a menu is about: the canvas point it was opened at, or an element by id.
#[derive(Clone, Debug, PartialEq)]
pub(crate) enum Target {
    Canvas(Pos2),
    Element(String),
    /// A canvas group's pill, by group id.
    Group(String),
}

/// A row of a menu: an action (its `id` goes to `context_action`), a submenu, or a separator.
#[derive(Clone, Debug)]
pub(crate) enum Entry {
    /// `keys`: the shortcut shown in gray at the right ("⌘K"), or empty.
    Item {
        id: String,
        label: String,
        check: Option<bool>,
        // the reference app's menus don't tint a destructive row
        #[allow(dead_code)]
        danger: bool,
        enabled: bool,
        keys: &'static str,
    },
    Sub {
        label: String,
        items: Vec<Entry>,
    },
    Sep,
}

pub(crate) fn entry(id: &str, label: &str, check: Option<bool>, danger: bool, enabled: bool) -> Entry {
    Entry::Item { id: id.into(), label: label.into(), check, danger, enabled, keys: "" }
}

/// A row with its shortcut at the right, as the macOS menus show it.
fn keyed(id: &str, label: &str, keys: &'static str) -> Entry {
    Entry::Item { id: id.into(), label: label.into(), check: None, danger: false, enabled: true, keys }
}

pub(crate) fn item(id: &str, label: &str) -> Entry {
    entry(id, label, None, false, true)
}

pub(crate) fn tick(id: &str, label: &str, on: bool) -> Entry {
    entry(id, label, Some(on), false, true)
}

pub(crate) fn red(id: &str, label: &str) -> Entry {
    entry(id, label, None, true, true)
}

pub(crate) fn sub(label: &str, items: Vec<Entry>) -> Entry {
    Entry::Sub { label: label.into(), items }
}

/// The menu open now: its target, its rows when they are not the target's own (the note bar's
/// popovers), the submenu open at each depth, and the panels drawn last frame.
pub(crate) struct ContextMenu {
    pub(crate) at: Pos2,
    pub(crate) target: Target,
    pub(crate) items: Option<Vec<Entry>>,
    path: Vec<usize>,
    rects: Vec<Rect>,
}

impl ContextMenu {
    pub(crate) fn new(at: Pos2, target: Target, items: Option<Vec<Entry>>) -> Self {
        Self { at, target, items, path: Vec::new(), rects: Vec::new() }
    }
}

/// One panel at `at` and, when a row of it is open (`path[depth]`), its submenu beside it, and
/// so on. Returns the id of the row picked. Drawn as the reference app's context menu (menu_look.rs):
/// as wide as its widest row, rows 26 high, an icon where the row has one, a tick where it is on.
fn panel(ctx: &Context, name: &str, at: Pos2, entries: &[Entry], path: &mut Vec<usize>, depth: usize, rects: &mut Vec<Rect>) -> Option<String> {
    let colors = menu_colors(ctx.global_style().visuals.dark_mode);
    let font = FontId::proportional(MENU_TEXT_SIZE);
    let has_ticks = entries.iter().any(|e| matches!(e, Entry::Item { check: Some(_), .. }));
    let mut picked = None;
    let mut subs: Vec<(usize, Rect)> = Vec::new();
    let area = egui::Area::new(egui::Id::unique((name, depth))).fixed_pos(at).order(egui::Order::Foreground).show(ctx, |ui| {
        // the icon column is there when any row has an icon or a tick: the labels line up after it
        // one icon column for the whole menu: a row without an icon still lines its label up with the others
        let column = has_ticks
            || entries.iter().any(|e| match e {
                Entry::Item { id, label, .. } => menu_icon(id, label).is_some(),
                Entry::Sub { label, .. } => menu_icon("", label).is_some(),
                Entry::Sep => false,
            });
        let lead = |_: &str, _: &str| column;
        let galley = |ui: &egui::Ui, text: &str| ui.painter().layout_no_wrap(text.into(), font.clone(), colors.text);
        let mut inner = if depth == 0 { MENU_MIN } else { SUB_MIN } - 2.0 * MENU_PAD - 2.0;
        for entry in entries {
            let width = match entry {
                Entry::Sep => 0.0,
                Entry::Item { id, label, keys, .. } => {
                    let lead = if lead(id, label) { ICON_SIZE + ROW_GAP } else { 0.0 };
                    let keys = if keys.is_empty() { 0.0 } else { ROW_GAP + galley(ui, &keys_label(keys)).size().x };
                    2.0 * ROW_PAD + lead + galley(ui, label).size().x + keys
                }
                Entry::Sub { label, .. } => {
                    let lead = if lead("", label) { ICON_SIZE + ROW_GAP } else { 0.0 };
                    2.0 * ROW_PAD + lead + galley(ui, label).size().x + ROW_GAP + ICON_SIZE
                }
            };
            inner = inner.max(width.ceil());
        }
        menu_frame(ctx.global_style().visuals.dark_mode).show(ui, |ui| {
            ui.set_width(inner);
            ui.spacing_mut().item_spacing.y = 0.0;
            for (k, row_entry) in entries.iter().enumerate() {
                match row_entry {
                    Entry::Sep => {
                        // -mx-1 my-1 h-px bg-border: the hairline runs under the padding to the box's edges
                        let (line, _) = ui.allocate_exact_size(Vec2::new(ui.available_width(), SEP_HEIGHT), Sense::hover());
                        let y = line.center().y.round();
                        ui.painter().hline(line.left() - MENU_PAD..=line.right() + MENU_PAD, y, Stroke::new(1.0, colors.border));
                    }
                    Entry::Item { id, label, check, enabled, keys, .. } => {
                        let (row, response) = ui.allocate_exact_size(Vec2::new(ui.available_width(), ROW_HEIGHT), Sense::click());
                        let lit = response.hovered() && *enabled;
                        if lit {
                            path.truncate(depth);
                        }
                        // disabled: half the opacity, no hover
                        let fade = |c: Color32| if *enabled { c } else { c.gamma_multiply(0.5) };
                        let p = ui.painter();
                        if lit {
                            p.rect_filled(row, ITEM_RADIUS, colors.hover);
                        }
                        let ink = fade(colors.text);
                        let icon_at = Pos2::new(row.left() + ROW_PAD + ICON_SIZE / 2.0, row.center().y);
                        if *check == Some(true) {
                            paint_menu_icon(p, MenuIcon::Check, icon_at, ink);
                        } else if let Some(icon) = menu_icon(id, label) {
                            paint_menu_icon(p, icon, icon_at, fade(if lit { colors.text } else { colors.muted }));
                        }
                        let label_x = row.left() + ROW_PAD + if lead(id, label) { ICON_SIZE + ROW_GAP } else { 0.0 };
                        p.text(Pos2::new(label_x, row.center().y), egui::Align2::LEFT_CENTER, label, font.clone(), ink);
                        if !keys.is_empty() {
                            p.text(
                                Pos2::new(row.right() - ROW_PAD, row.center().y),
                                egui::Align2::RIGHT_CENTER,
                                keys_label(keys),
                                font.clone(),
                                fade(colors.muted),
                            );
                        }
                        if *enabled && response.clicked() {
                            picked = Some(id.clone());
                        }
                        if *enabled {
                            response.on_hover_cursor(CursorIcon::PointingHand);
                        }
                    }
                    Entry::Sub { label, .. } => {
                        let (row, response) = ui.allocate_exact_size(Vec2::new(ui.available_width(), ROW_HEIGHT), Sense::click());
                        // hovering the row opens its submenu; it stays open while the pointer is in it
                        if response.hovered() {
                            path.truncate(depth);
                            path.push(k);
                        }
                        let lit = path.get(depth) == Some(&k);
                        let p = ui.painter();
                        if lit {
                            p.rect_filled(row, ITEM_RADIUS, colors.hover);
                        }
                        let ink = if lit { colors.text } else { colors.muted };
                        if let Some(icon) = menu_icon("", label) {
                            paint_menu_icon(p, icon, Pos2::new(row.left() + ROW_PAD + ICON_SIZE / 2.0, row.center().y), ink);
                        }
                        let label_x = row.left() + ROW_PAD + if lead("", label) { ICON_SIZE + ROW_GAP } else { 0.0 };
                        p.text(Pos2::new(label_x, row.center().y), egui::Align2::LEFT_CENTER, label, font.clone(), colors.text);
                        paint_menu_icon(p, MenuIcon::Chevron, Pos2::new(row.right() - ROW_PAD - ICON_SIZE / 2.0, row.center().y), ink);
                        subs.push((k, row));
                    }
                }
            }
        });
    });
    rects.push(area.response.rect);
    if picked.is_some() {
        return picked;
    }
    let open = subs.iter().copied().find(|(k, _)| path.get(depth) == Some(k))?;
    let Entry::Sub { items, .. } = &entries[open.0] else { return None };
    // beside its row, its first row level with it (the border and the padding above it are 4.25)
    panel(ctx, name, Pos2::new(open.1.right() + MENU_PAD, open.1.top() - MENU_PAD - 1.0), items, path, depth + 1, rects)
}

impl DrawingApp {}
