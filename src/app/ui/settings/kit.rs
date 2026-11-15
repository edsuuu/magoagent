//! The controls of Ajustes in the new design: groups of rows in cards (#1e1f23, radius 12) under a small
//! uppercase title, rows at least 48 high with the label and its hint at the left and the control at the right,
//! and the switch, segments, select, stepper, buttons and fields 30 high.

use crate::prelude::*;
use egui::{Role, WidgetInfo};
use std::cell::RefCell;
use std::sync::Arc;

/// The width of the content column: the main area (880 less the 200 of the sidebar) less its 32 at each side and
/// the cards' 16 inside.
pub(in crate::app::ui) const COLUMN: f32 = 584.0;
/// How far a card reaches past the column at each side.
pub(in crate::app::ui) const INSET: f32 = 16.0;
/// The gap between two groups.
pub(in crate::app::ui) const GROUP_GAP: f32 = 18.0;
const ROW: f32 = 48.0;
const CONTROL_HEIGHT: f32 = 30.0;
pub(in crate::app::ui) const BODY: f32 = 13.5;
pub(in crate::app::ui) const BODY_LINE: f32 = 18.0;
pub(in crate::app::ui) const SMALL: f32 = 12.0;
pub(in crate::app::ui) const SMALL_LINE: f32 = 16.0;
pub(in crate::app::ui) const CONTROL: f32 = 13.0;
pub(in crate::app::ui) const TITLE: f32 = 11.5;
pub(in crate::app::ui) const TITLE_LINE: f32 = 15.0;
const RADIUS: f32 = 8.0;

/// The design's tokens for one theme (it is drawn dark; the light ones take the same steps the other way).
pub(in crate::app::ui) struct K {
    pub(in crate::app::ui) dark: bool,
    /// The window (#17181b).
    pub(in crate::app::ui) bg: Color32,
    pub(in crate::app::ui) fg: Color32,
    pub(in crate::app::ui) muted_fg: Color32,
    pub(in crate::app::ui) border: Color32,
    pub(in crate::app::ui) input: Color32,
    pub(in crate::app::ui) muted: Color32,
    /// A group's card (#1e1f23).
    pub(in crate::app::ui) card: Color32,
    pub(in crate::app::ui) primary: Color32,
    pub(in crate::app::ui) destructive: Color32,
    /// The sidebar and the inside of a field, a select, a stepper or the segments (#111215).
    pub(in crate::app::ui) well: Color32,
    /// The line around a field (white 8%); between rows and around a card (6%); around a button (10%).
    pub(in crate::app::ui) well_line: Color32,
    pub(in crate::app::ui) hairline: Color32,
    pub(in crate::app::ui) button: Color32,
    pub(in crate::app::ui) button_line: Color32,
    /// The chosen item of the sidebar (#24262b) and the chosen segment (#2c2e34).
    pub(in crate::app::ui) chosen: Color32,
    pub(in crate::app::ui) segment: Color32,
    /// The sidebar's and the segments' text (#a1a1aa).
    pub(in crate::app::ui) soft_fg: Color32,
    /// Rows go into cards only in Ajustes (the dialogs draw the same rows without them).
    cards: bool,
    group: RefCell<Option<Group>>,
}

/// The card being filled: the placeholder painted under it when it closes, where it starts, its column, how many
/// rows it has and whether the last thing in it was one (rows carry their own padding).
struct Group {
    shape: egui::layers::ShapeIdx,
    top: f32,
    left: f32,
    right: f32,
    rows: usize,
    last_was_row: bool,
}

impl K {
    pub(in crate::app::ui) fn new(dark: bool) -> Self {
        let rgba = Color32::from_rgba_unmultiplied;
        let rgb = Color32::from_rgb;
        if dark {
            Self {
                dark,
                bg: rgb(0x17, 0x18, 0x1b),
                fg: rgb(0xf5, 0xf5, 0xf7),
                muted_fg: rgb(0x8e, 0x8e, 0x93),
                border: rgba(0x54, 0x54, 0x58, 0x80),
                input: rgba(0x76, 0x76, 0x80, 0x3d),
                muted: rgb(0x24, 0x26, 0x2b),
                card: rgb(0x1e, 0x1f, 0x23),
                primary: rgb(0x0a, 0x84, 0xff),
                destructive: rgb(0xff, 0x45, 0x3a),
                well: rgb(0x11, 0x12, 0x15),
                well_line: Color32::from_white_alpha(20),
                hairline: Color32::from_white_alpha(15),
                button: rgb(0x26, 0x28, 0x2d),
                button_line: Color32::from_white_alpha(26),
                chosen: rgb(0x24, 0x26, 0x2b),
                segment: rgb(0x2c, 0x2e, 0x34),
                soft_fg: rgb(0xa1, 0xa1, 0xaa),
                cards: false,
                group: RefCell::new(None),
            }
        } else {
            Self {
                dark,
                bg: rgb(0xf5, 0xf5, 0xf7),
                fg: rgb(0x1d, 0x1d, 0x1f),
                muted_fg: rgb(0x6e, 0x6e, 0x73),
                border: rgba(0, 0, 0, 0x1f),
                input: rgba(0, 0, 0, 0x1a),
                muted: rgb(0xe8, 0xe8, 0xec),
                card: Color32::WHITE,
                primary: rgb(0x00, 0x7a, 0xff),
                destructive: rgb(0xff, 0x3b, 0x30),
                well: rgb(0xec, 0xec, 0xf0),
                well_line: Color32::from_black_alpha(20),
                hairline: Color32::from_black_alpha(15),
                button: Color32::WHITE,
                button_line: Color32::from_black_alpha(31),
                chosen: rgb(0xdc, 0xdc, 0xe2),
                segment: Color32::WHITE,
                soft_fg: rgb(0x5a, 0x5a, 0x62),
                cards: false,
                group: RefCell::new(None),
            }
        }
    }

    /// The tokens of Ajustes, whose rows go into cards.
    pub(in crate::app::ui) fn with_cards(dark: bool) -> Self {
        Self { cards: true, ..Self::new(dark) }
    }

    /// `border/50` and `border/60`, the lines between things.
    pub(in crate::app::ui) fn line(&self, alpha: f32) -> Color32 {
        self.border.gamma_multiply(alpha)
    }
}

/// `text` laid out at `size` points with `line` points between lines, wrapped at `wrap`.
pub(in crate::app::ui) fn galley(ui: &egui::Ui, text: &str, size: f32, line: f32, color: Color32, wrap: f32) -> Arc<egui::Galley> {
    galley_in(ui, text, FontId::proportional(size), line, color, wrap)
}

fn galley_in(ui: &egui::Ui, text: &str, font: FontId, line: f32, color: Color32, wrap: f32) -> Arc<egui::Galley> {
    let mut job = egui::text::LayoutJob::simple(text.into(), font, color, wrap);
    job.sections[0].format.line_height = Some(line);
    ui.painter().layout_job(job)
}

/// The weight 600 of the system face.
pub(in crate::app::ui) fn semibold(size: f32) -> FontId {
    FontId::new(size, egui::FontFamily::Name(crate::app::ui::SYSTEM_SEMIBOLD.into()))
}

/// Starts a card at `top` when none is open (only in Ajustes).
fn open_group(ui: &egui::Ui, k: &K, top: f32) {
    if !k.cards || k.group.borrow().is_some() {
        return;
    }
    let shape = ui.painter().add(Shape::Noop);
    let left = ui.cursor().left();
    *k.group.borrow_mut() = Some(Group { shape, top, left, right: left + ui.available_width(), rows: 0, last_was_row: false });
}

/// Ends the open card: it is painted under what was drawn in it. True when there was one.
pub(in crate::app::ui) fn close_group(ui: &mut egui::Ui, k: &K) -> bool {
    let Some(group) = k.group.borrow_mut().take() else { return false };
    if !group.last_was_row {
        ui.add_space(10.0);
    }
    let bottom = ui.min_rect().bottom();
    let rect = Rect::from_min_max(Pos2::new(group.left - INSET, group.top), Pos2::new(group.right + INSET, bottom));
    ui.painter().set(group.shape, egui::epaint::RectShape::new(rect, 12, k.card, Stroke::new(1.0, k.hairline), StrokeKind::Inside));
    true
}

/// A row starting at `top` in the open card (one opens when there is none), with a line over it when it is not the
/// first.
fn enter_row(ui: &egui::Ui, k: &K, top: f32) {
    open_group(ui, k, top);
    if let Some(group) = k.group.borrow_mut().as_mut() {
        if group.rows > 0 {
            let y = top - ui.spacing().item_spacing.y / 2.0;
            ui.painter().hline(group.left - INSET..=group.right + INSET, y, Stroke::new(1.0, k.hairline));
        }
        group.rows += 1;
        group.last_was_row = true;
    }
}

/// A group's title (11.5, weight 600, uppercase, 4 in from the card's edge) and the card that follows it.
pub(in crate::app::ui) fn title(ui: &mut egui::Ui, k: &K, text: &str) {
    if close_group(ui, k) {
        ui.add_space(GROUP_GAP - ui.spacing().item_spacing.y);
    }
    let g = galley_in(ui, text, semibold(TITLE), TITLE_LINE, k.muted_fg, f32::INFINITY);
    let (rect, _) = ui.allocate_exact_size(Vec2::new(ui.available_width(), g.size().y), Sense::hover());
    let left = if k.cards { rect.left() - INSET + 4.0 } else { rect.left() };
    ui.painter().galley(Pos2::new(left, rect.top()), g, k.muted_fg);
    if k.cards {
        ui.add_space(6.0 - ui.spacing().item_spacing.y);
        let top = ui.cursor().top();
        open_group(ui, k, top);
    }
}

/// The end of a group and the gap to the next one (a line between them where there are no cards).
pub(in crate::app::ui) fn divider(ui: &mut egui::Ui, k: &K) {
    if k.cards {
        if close_group(ui, k) {
            ui.add_space(GROUP_GAP - ui.spacing().item_spacing.y);
        }
        return;
    }
    ui.add_space(16.25);
    let (rect, _) = ui.allocate_exact_size(Vec2::new(ui.available_width(), 1.0), Sense::hover());
    ui.painter().hline(rect.x_range(), rect.center().y, Stroke::new(1.0, k.line(0.5)));
    ui.add_space(16.25);
}

/// A paragraph of the muted kind that explains a group; in a card, a row of its own.
pub(in crate::app::ui) fn note(ui: &mut egui::Ui, k: &K, text: &str) {
    let top = ui.cursor().top();
    enter_row(ui, k, top);
    let pad = if k.cards { 12.0 } else { 0.0 };
    let g = galley(ui, text, SMALL, SMALL_LINE, k.muted_fg, ui.available_width());
    let (rect, _) = ui.allocate_exact_size(Vec2::new(ui.available_width(), g.size().y + 2.0 * pad), Sense::hover());
    ui.painter().galley(rect.min + Vec2::new(0.0, pad), g, k.muted_fg);
}

/// Something other than rows drawn into the card (a list, a row of buttons): the card's padding comes around it.
pub(in crate::app::ui) fn block<R>(ui: &mut egui::Ui, k: &K, add: impl FnOnce(&mut egui::Ui) -> R) -> R {
    let top = ui.cursor().top();
    open_group(ui, k, top);
    if let Some(group) = k.group.borrow_mut().as_mut() {
        group.last_was_row = false;
    }
    ui.add_space(10.0 - ui.spacing().item_spacing.y);
    add(ui)
}

/// The label and its hint wrapped at `room`, and the height of a row that holds them.
fn label_column(ui: &egui::Ui, k: &K, label: &str, hint: &str, room: f32) -> (Arc<egui::Galley>, Option<Arc<egui::Galley>>, f32) {
    let text = galley(ui, label, BODY, BODY_LINE, k.fg, room);
    let hint_galley = (!hint.is_empty()).then(|| galley(ui, hint, SMALL, SMALL_LINE, k.muted_fg, room));
    let block = text.size().y + hint_galley.as_ref().map_or(0.0, |g| 2.0 + g.size().y);
    (text, hint_galley, (block + 16.0).max(ROW))
}

/// The label over its hint, in the middle of the row's height.
fn paint_label_column(ui: &egui::Ui, k: &K, rect: Rect, text: Arc<egui::Galley>, hint: Option<Arc<egui::Galley>>) {
    let block = text.size().y + hint.as_ref().map_or(0.0, |g| 2.0 + g.size().y);
    let top = rect.center().y - block / 2.0;
    let below = top + text.size().y + 2.0;
    ui.painter().galley(Pos2::new(rect.left(), top), text, k.fg);
    if let Some(g) = hint {
        ui.painter().galley(Pos2::new(rect.left(), below), g, k.muted_fg);
    }
}

/// `SettingsRow`: the label (and its hint under it) at the left, the control at the right in the middle of the
/// first 48. `add` lays the control out from the right edge inwards.
pub(in crate::app::ui) fn row<R>(ui: &mut egui::Ui, k: &K, label: &str, hint: &str, add: impl FnOnce(&mut egui::Ui) -> R) -> R {
    let width = ui.available_width();
    let top = ui.cursor().top();
    enter_row(ui, k, top);
    let line = Rect::from_min_size(Pos2::new(ui.cursor().left(), top + (ROW - CONTROL_HEIGHT) / 2.0), Vec2::new(width, CONTROL_HEIGHT));
    let mut child = ui.new_child(egui::UiBuilder::new().max_rect(line).layout(egui::Layout::right_to_left(egui::Align::Center)));
    child.spacing_mut().item_spacing.x = 6.0;
    let result = add(&mut child);
    let control = child.min_rect().width();
    let room = (width - control - 16.0).max(80.0);
    let (text, hint_galley, height) = label_column(ui, k, label, hint, room);
    let (rect, _) = ui.allocate_exact_size(Vec2::new(width, height), Sense::hover());
    paint_label_column(ui, k, rect, text, hint_galley);
    result
}

/// `SettingsToggleRow`: label and hint at the left, the switch in the middle at the right. True when it was flipped.
pub(in crate::app::ui) fn toggle(ui: &mut egui::Ui, k: &K, label: &str, hint: &str, value: &mut bool) -> bool {
    let width = ui.available_width();
    let top = ui.cursor().top();
    enter_row(ui, k, top);
    let (text, hint_galley, height) = label_column(ui, k, label, hint, width - 38.0 - 16.0);
    let (rect, _) = ui.allocate_exact_size(Vec2::new(width, height), Sense::hover());
    paint_label_column(ui, k, rect, text, hint_galley);
    switch(ui, k, Pos2::new(rect.right() - 38.0, rect.center().y - 11.0), label, value)
}

/// The 38 x 22 switch at `at`; the knob slides.
pub(in crate::app::ui) fn switch(ui: &mut egui::Ui, k: &K, at: Pos2, label: &str, value: &mut bool) -> bool {
    let rect = Rect::from_min_size(at, Vec2::new(38.0, 22.0));
    let response = ui.interact(rect, ui.scope_id().with(("switch", label)), Sense::click()).on_hover_cursor(CursorIcon::PointingHand);
    response.widget_info(|| WidgetInfo::selected(Role::Switch, true, *value, label));
    let t = ui.ctx().animate_bool_with_time(response.id, *value, 0.12);
    let off = if k.dark { Color32::from_white_alpha(46) } else { Color32::from_black_alpha(46) };
    ui.painter().rect_filled(rect, 11.0, off.lerp_to_gamma(k.primary, t));
    let x = rect.left() + 11.0 + 16.0 * t;
    ui.painter().circle_filled(Pos2::new(x, rect.center().y), 9.0, Color32::WHITE);
    let clicked = response.clicked();
    if clicked {
        *value = !*value;
    }
    clicked
}

/// The widths of the segments for `labels` (the text and 12 at each side), and of the track around them.
fn segment_widths(ui: &egui::Ui, k: &K, labels: &[&str]) -> (Vec<f32>, f32) {
    let font = FontId::proportional(12.5);
    let widths: Vec<f32> = labels.iter().map(|l| ui.painter().layout_no_wrap(l.to_string(), font.clone(), k.fg).size().x + 24.0).collect();
    let total = widths.iter().sum::<f32>() + 2.0 * (labels.len() as f32 - 1.0).max(0.0) + 4.0;
    (widths, total)
}

/// The segments painted in `track` (#111215, radius 8, 2 inside and between them); the one picked.
fn segments(ui: &mut egui::Ui, k: &K, id: egui::Id, track: Rect, labels: &[&str], widths: &[f32], chosen: usize) -> usize {
    ui.painter().rect_filled(track, RADIUS, k.well);
    let font = FontId::proportional(12.5);
    let mut x = track.left() + 2.0;
    let mut pick = chosen;
    for (i, (label, width)) in labels.iter().zip(widths).enumerate() {
        let segment = Rect::from_min_size(Pos2::new(x, track.top() + 2.0), Vec2::new(*width, track.height() - 4.0));
        let response = ui.interact(segment, id.with(i), Sense::click()).on_hover_cursor(CursorIcon::PointingHand);
        response.widget_info(|| WidgetInfo::selected(Role::RadioButton, true, i == chosen, *label));
        if i == chosen {
            ui.painter().rect_filled(segment, 6.0, k.segment);
        }
        let color = if i == chosen || response.hovered() { k.fg } else { k.soft_fg };
        ui.painter().text(segment.center(), egui::Align2::CENTER_CENTER, label, font.clone(), color);
        if response.clicked() {
            pick = i;
        }
        x += width + 2.0;
    }
    pick
}

/// A choice of a few short options side by side (the design's `seg`), 30 high. Returns the option picked.
pub(in crate::app::ui) fn choice(ui: &mut egui::Ui, k: &K, label: &str, options: &[&str], chosen: usize) -> Option<usize> {
    let (widths, total) = segment_widths(ui, k, options);
    let (track, _) = ui.allocate_exact_size(Vec2::new(total, CONTROL_HEIGHT), Sense::hover());
    let pick = segments(ui, k, ui.scope_id().with(("choice", label)), track, options, &widths, chosen);
    (pick != chosen).then_some(pick)
}

/// `SelectField`: 30 high, at least 120 wide, the options in a menu. Returns the option picked.
pub(in crate::app::ui) fn select(ui: &mut egui::Ui, k: &K, label: &str, options: &[&str], chosen: usize) -> Option<usize> {
    let font = FontId::proportional(CONTROL);
    let widest = options.iter().map(|o| ui.painter().layout_no_wrap(o.to_string(), font.clone(), k.fg).size().x).fold(0.0, f32::max);
    let width = (widest + 10.0 + 28.0).max(120.0);
    let (rect, response) = ui.allocate_exact_size(Vec2::new(width, CONTROL_HEIGHT), Sense::click());
    let response = response.on_hover_cursor(CursorIcon::PointingHand);
    let value = options.get(chosen).copied().unwrap_or("");
    response.widget_info(|| WidgetInfo::labeled(Role::Button, true, format!("{label}: {value}")));
    ui.painter().rect_filled(rect, RADIUS, k.well);
    ui.painter().rect_stroke(rect, RADIUS, Stroke::new(1.0, if response.hovered() { k.button_line } else { k.well_line }), StrokeKind::Inside);
    ui.painter().text(Pos2::new(rect.left() + 10.0, rect.center().y), egui::Align2::LEFT_CENTER, value, font.clone(), k.fg);
    // the chevron, at 14 from the right
    let c = Pos2::new(rect.right() - 14.0, rect.center().y);
    ui.painter().add(Shape::line(vec![c + Vec2::new(-3.5, -1.75), c + Vec2::new(0.0, 1.75), c + Vec2::new(3.5, -1.75)], Stroke::new(1.4, k.muted_fg)));
    let mut pick = None;
    egui::Popup::menu(&response).frame(popup_frame(k)).width(width).show(|ui| {
        ui.spacing_mut().item_spacing.y = 0.0;
        for (i, option) in options.iter().enumerate() {
            let (rect, response) = ui.allocate_exact_size(Vec2::new(ui.available_width(), 28.0), Sense::click());
            let response = response.on_hover_cursor(CursorIcon::PointingHand);
            response.widget_info(|| WidgetInfo::labeled(Role::Button, true, *option));
            if response.hovered() {
                ui.painter().rect_filled(rect, 6.0, k.chosen);
            }
            if i == chosen {
                paint_icon_scaled(ui.painter(), Icon::Check, Pos2::new(rect.left() + 13.0, rect.center().y), k.fg, 13.0 / 16.0, 0.0);
            }
            ui.painter().text(Pos2::new(rect.left() + 26.0, rect.center().y), egui::Align2::LEFT_CENTER, option, FontId::proportional(CONTROL), k.fg);
            if response.clicked() {
                pick = Some(i);
            }
        }
    });
    pick.filter(|i| *i != chosen)
}

/// The frame of every menu inside the window: the card's colour, radius 8, a line around, flat.
pub(in crate::app::ui) fn popup_frame(k: &K) -> egui::Frame {
    egui::Frame::new().fill(k.card).corner_radius(RADIUS).inner_margin(4.0).stroke(Stroke::new(1.0, k.button_line)).shadow(egui::epaint::Shadow::NONE)
}

/// `NumberStepper`: − the value + in a well 30 high, the buttons 30 wide. True when it changed.
pub(in crate::app::ui) fn stepper(ui: &mut egui::Ui, k: &K, label: &str, value: &mut u8, range: std::ops::RangeInclusive<u8>, shown: &str) -> bool {
    let (rect, _) = ui.allocate_exact_size(Vec2::new(CONTROL_HEIGHT * 2.0 + 48.0, CONTROL_HEIGHT), Sense::hover());
    ui.painter().rect_filled(rect, RADIUS, k.well);
    ui.painter().rect_stroke(rect, RADIUS, Stroke::new(1.0, k.well_line), StrokeKind::Inside);
    let mut changed = false;
    // laid out right to left, so the plus is the first one made
    for (i, (sign, delta)) in [(Icon::Plus, 1i16), (Icon::Minus, -1)].into_iter().enumerate() {
        let side = if i == 0 { rect.right() - CONTROL_HEIGHT } else { rect.left() };
        let button = Rect::from_min_size(Pos2::new(side, rect.top()), Vec2::splat(CONTROL_HEIGHT));
        let next = i16::from(*value) + delta;
        let live = range.contains(&(next as u8)) && next >= 0;
        let response = ui.interact(button, ui.scope_id().with((label, delta)), if live { Sense::click() } else { Sense::hover() });
        let name = format!("{label} {}", if delta > 0 { "+" } else { "−" });
        response.widget_info(|| WidgetInfo::labeled(Role::Button, live, &name));
        let color = if !live {
            k.muted_fg.gamma_multiply(0.4)
        } else if response.hovered() {
            k.fg
        } else {
            k.soft_fg
        };
        paint_icon(ui.painter(), sign, button.center(), color);
        if live && response.on_hover_cursor(CursorIcon::PointingHand).clicked() {
            *value = next as u8;
            changed = true;
        }
    }
    ui.painter().text(rect.center(), egui::Align2::CENTER_CENTER, shown, FontId::proportional(CONTROL), k.fg);
    changed
}

/// A button 30 high (#26282d, a line of white 10%, radius 8).
pub(in crate::app::ui) fn button(ui: &mut egui::Ui, k: &K, text: &str) -> egui::Response {
    let g = ui.painter().layout_no_wrap(text.into(), FontId::proportional(12.5), k.fg);
    let (rect, response) = ui.allocate_exact_size(Vec2::new(g.size().x + 24.0, CONTROL_HEIGHT), Sense::click());
    let response = response.on_hover_cursor(CursorIcon::PointingHand);
    response.widget_info(|| WidgetInfo::labeled(Role::Button, true, text));
    let fill = if response.hovered() { k.button.lerp_to_gamma(k.fg, 0.06) } else { k.button };
    ui.painter().rect_filled(rect, RADIUS, fill);
    ui.painter().rect_stroke(rect, RADIUS, Stroke::new(1.0, k.button_line), StrokeKind::Inside);
    ui.painter().galley(Pos2::new(rect.center().x - g.size().x / 2.0, rect.center().y - g.size().y / 2.0), g, k.fg);
    response
}

/// A 24 px ghost button with an icon; red under the pointer when `danger`.
pub(in crate::app::ui) fn ghost_button(ui: &mut egui::Ui, k: &K, icon: impl Into<Glyph>, name: &str, enabled: bool, danger: bool) -> egui::Response {
    let (rect, response) = ui.allocate_exact_size(Vec2::splat(24.0), if enabled { Sense::click() } else { Sense::hover() });
    let response = response.on_hover_cursor(CursorIcon::PointingHand);
    response.widget_info(|| WidgetInfo::labeled(Role::Button, enabled, name));
    let hot = enabled && response.hovered();
    if hot {
        ui.painter().rect_filled(rect, 6.0, k.chosen);
    }
    let color = if !enabled {
        k.muted_fg.gamma_multiply(0.4)
    } else if hot && danger {
        k.destructive
    } else if hot {
        k.fg
    } else {
        k.muted_fg
    };
    match icon.into() {
        Glyph::Icon(icon) => paint_icon(ui.painter(), icon, rect.center(), color),
        Glyph::Chevron(up) => {
            let y = if up { 2.0 } else { -2.0 };
            let c = rect.center();
            ui.painter().add(Shape::line(vec![c + Vec2::new(-4.5, y), c + Vec2::new(0.0, -y), c + Vec2::new(4.5, y)], Stroke::new(1.4, color)));
        }
    }
    response
}

/// What an `icon_button` shows: one of the app's icons or a chevron (true: up).
pub(in crate::app::ui) enum Glyph {
    Icon(Icon),
    Chevron(bool),
}

impl From<Icon> for Glyph {
    fn from(icon: Icon) -> Self {
        Glyph::Icon(icon)
    }
}

/// `Input`: a well 30 high with the text inside, the line going to the accent on focus.
pub(in crate::app::ui) fn input(
    ui: &mut egui::Ui,
    k: &K,
    id: egui::Id,
    label: &str,
    text: &mut String,
    hint: &str,
    width: f32,
    password: bool,
) -> egui::Response {
    let (rect, _) = ui.allocate_exact_size(Vec2::new(width, CONTROL_HEIGHT), Sense::hover());
    let inner = Rect::from_min_max(Pos2::new(rect.left() + 10.0, rect.top()), Pos2::new(rect.right() - 10.0, rect.bottom()));
    let focused = ui.memory(|m| m.has_focus(id));
    ui.painter().rect_filled(rect, RADIUS, k.well);
    ui.painter().rect_stroke(rect, RADIUS, Stroke::new(1.0, if focused { k.primary } else { k.well_line }), StrokeKind::Inside);
    let edit = egui::TextEdit::singleline(text)
        .id(id)
        .password(password)
        .hint_text(egui::RichText::new(hint).color(k.muted_fg))
        .font(FontId::proportional(CONTROL))
        .text_color(k.fg)
        .frame(egui::Frame::NONE)
        .align(egui::Align2::LEFT_CENTER)
        .desired_width(inner.width());
    let response = ui.put(inner, edit);
    response.widget_info(|| WidgetInfo::labeled(Role::TextInput, true, label));
    response
}

/// `SegmentedTabs`: the segments across the top of a panel (or a dialog), at its left. Returns the pick.
pub(in crate::app::ui) fn segmented_tabs(ui: &mut egui::Ui, k: &K, id: egui::Id, labels: &[&str], chosen: usize) -> usize {
    let (widths, total) = segment_widths(ui, k, labels);
    let (strip, _) = ui.allocate_exact_size(Vec2::new(ui.available_width(), CONTROL_HEIGHT), Sense::hover());
    let left = if k.cards { strip.left() - INSET } else { strip.center().x - total / 2.0 };
    let track = Rect::from_min_size(Pos2::new(left, strip.top()), Vec2::new(total, CONTROL_HEIGHT));
    segments(ui, k, id, track, labels, &widths, chosen)
}
