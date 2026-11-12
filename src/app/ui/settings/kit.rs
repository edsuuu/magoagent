//! The controls of Ajustes, drawn to the reference app's measures (1rem = 13px there, so
//! `py-1` is 3.25, `gap-4` 13 and so on): sections, rows, switch, select, stepper, buttons, fields.

use crate::prelude::*;
use egui::{Role, WidgetInfo};
use std::sync::Arc;

/// The width of the content column (`max-w-[700px]` less `px-5` on both sides).
pub(in crate::app::ui) const COLUMN: f32 = 668.0;
const PAD: f32 = 3.25;
const GAP: f32 = 13.0;
const ROW: f32 = 28.0;
/// The text sizes: the reference app's 13 / 11 / 11.5 were too small to read, so everything is
/// about two points up.
pub(in crate::app::ui) const BODY: f32 = 14.5;
pub(in crate::app::ui) const BODY_LINE: f32 = 20.0;
pub(in crate::app::ui) const SMALL: f32 = 13.0;
pub(in crate::app::ui) const SMALL_LINE: f32 = 18.0;
pub(in crate::app::ui) const CONTROL: f32 = 13.5;
pub(in crate::app::ui) const TITLE: f32 = 12.5;
pub(in crate::app::ui) const TITLE_LINE: f32 = 17.0;
const RADIUS: f32 = 6.125;

/// The reference app's tokens (assets/global.css) for one theme.
pub(in crate::app::ui) struct K {
    pub(in crate::app::ui) dark: bool,
    pub(in crate::app::ui) bg: Color32,
    pub(in crate::app::ui) fg: Color32,
    pub(in crate::app::ui) muted_fg: Color32,
    pub(in crate::app::ui) border: Color32,
    pub(in crate::app::ui) input: Color32,
    pub(in crate::app::ui) secondary: Color32,
    pub(in crate::app::ui) muted: Color32,
    pub(in crate::app::ui) card: Color32,
    pub(in crate::app::ui) primary: Color32,
    pub(in crate::app::ui) destructive: Color32,
}

impl K {
    pub(in crate::app::ui) fn new(dark: bool) -> Self {
        let rgba = Color32::from_rgba_unmultiplied;
        if dark {
            Self {
                dark,
                bg: Color32::from_rgb(0x1a, 0x1a, 0x1c),
                fg: Color32::from_rgb(0xf5, 0xf5, 0xf7),
                muted_fg: rgba(0xeb, 0xeb, 0xf5, 0x99),
                border: rgba(0x54, 0x54, 0x58, 0x80),
                input: rgba(0x76, 0x76, 0x80, 0x3d),
                secondary: Color32::from_rgb(0x3a, 0x3a, 0x3c),
                muted: Color32::from_rgb(0x2c, 0x2c, 0x2e),
                card: Color32::from_rgb(0x2c, 0x2c, 0x2e),
                primary: Color32::from_rgb(0x0a, 0x84, 0xff),
                destructive: Color32::from_rgb(0xff, 0x45, 0x3a),
            }
        } else {
            Self {
                dark,
                bg: Color32::from_rgb(0xf6, 0xf6, 0xf6),
                fg: Color32::from_rgb(0x1d, 0x1d, 0x1f),
                muted_fg: rgba(0x3c, 0x3c, 0x43, 0x99),
                border: rgba(0, 0, 0, 0x1f),
                input: rgba(0, 0, 0, 0x1a),
                secondary: Color32::from_rgb(0xe5, 0xe5, 0xe7),
                muted: Color32::from_rgb(0xf0, 0xf0, 0xf2),
                card: Color32::WHITE,
                primary: Color32::from_rgb(0x00, 0x7a, 0xff),
                destructive: Color32::from_rgb(0xff, 0x3b, 0x30),
            }
        }
    }

    /// `border/50` and `border/60`, the lines between things.
    pub(in crate::app::ui) fn line(&self, alpha: f32) -> Color32 {
        self.border.gamma_multiply(alpha)
    }
}

/// `text` laid out at `size` points with `line` points between lines, wrapped at `wrap`.
pub(in crate::app::ui) fn galley(ui: &egui::Ui, text: &str, size: f32, line: f32, color: Color32, wrap: f32) -> Arc<egui::Galley> {
    let mut job = egui::text::LayoutJob::simple(text.into(), FontId::proportional(size), color, wrap);
    job.sections[0].format.line_height = Some(line);
    ui.painter().layout_job(job)
}

/// A group's title (`text-[11px]`, uppercase) and the gap under it.
pub(in crate::app::ui) fn title(ui: &mut egui::Ui, k: &K, text: &str) {
    let g = galley(ui, text, TITLE, TITLE_LINE, k.muted_fg, f32::INFINITY);
    let (rect, _) = ui.allocate_exact_size(Vec2::new(ui.available_width(), g.size().y), Sense::hover());
    ui.painter().galley(rect.min, g, k.muted_fg);
    ui.add_space(PAD);
}

/// The line between two groups (`gap-5` around an `hr` with `my-1`).
pub(in crate::app::ui) fn divider(ui: &mut egui::Ui, k: &K) {
    ui.add_space(16.25);
    let (rect, _) = ui.allocate_exact_size(Vec2::new(ui.available_width(), 1.0), Sense::hover());
    ui.painter().hline(rect.x_range(), rect.center().y, Stroke::new(1.0, k.line(0.5)));
    ui.add_space(16.25);
}

/// A paragraph of the muted 11 px kind that explains a group.
pub(in crate::app::ui) fn note(ui: &mut egui::Ui, k: &K, text: &str) {
    let g = galley(ui, text, SMALL, SMALL_LINE, k.muted_fg, ui.available_width());
    let (rect, _) = ui.allocate_exact_size(g.size().max(Vec2::new(ui.available_width(), 0.0)), Sense::hover());
    ui.painter().galley(rect.min, g, k.muted_fg);
}

/// `SettingsRow`: the label (and its hint under it) at the left, the control at the right, 26 high.
/// `add` lays the control out from the right edge inwards.
pub(in crate::app::ui) fn row<R>(ui: &mut egui::Ui, k: &K, label: &str, hint: &str, add: impl FnOnce(&mut egui::Ui) -> R) -> R {
    let width = ui.available_width();
    let hint_galley = (!hint.is_empty()).then(|| galley(ui, hint, SMALL, SMALL_LINE, k.muted_fg, width - 6.5));
    let height = 2.0 * PAD + ROW + hint_galley.as_ref().map_or(0.0, |g| PAD + g.size().y);
    let (rect, _) = ui.allocate_exact_size(Vec2::new(width, height), Sense::hover());
    let line = Rect::from_min_size(Pos2::new(rect.left(), rect.top() + PAD), Vec2::new(width, ROW));
    let text = galley(ui, label, BODY, BODY_LINE, k.fg, f32::INFINITY);
    ui.painter().galley(Pos2::new(line.left(), line.center().y - text.size().y / 2.0), text, k.fg);
    if let Some(g) = hint_galley {
        ui.painter().galley(Pos2::new(rect.left(), line.bottom() + PAD), g, k.muted_fg);
    }
    let mut child = ui.new_child(egui::UiBuilder::new().max_rect(line).layout(egui::Layout::right_to_left(egui::Align::Center)));
    child.spacing_mut().item_spacing.x = 6.5;
    add(&mut child)
}

/// `SettingsToggleRow`: label and hint at the left, the switch at the top right. True when it was flipped.
pub(in crate::app::ui) fn toggle(ui: &mut egui::Ui, k: &K, label: &str, hint: &str, value: &mut bool) -> bool {
    let width = ui.available_width();
    let text = galley(ui, label, BODY, BODY_LINE, k.fg, width - 38.0 - GAP);
    let hint_galley = (!hint.is_empty()).then(|| galley(ui, hint, SMALL, SMALL_LINE, k.muted_fg, width - 38.0 - GAP));
    let pad = 4.875;
    let height = (2.0 * pad + text.size().y + hint_galley.as_ref().map_or(0.0, |g| 1.625 + g.size().y)).max(2.0 * pad + 22.0);
    let (rect, _) = ui.allocate_exact_size(Vec2::new(width, height), Sense::hover());
    let below = rect.top() + pad + text.size().y + 1.625;
    ui.painter().galley(Pos2::new(rect.left(), rect.top() + pad), text, k.fg);
    if let Some(g) = hint_galley {
        ui.painter().galley(Pos2::new(rect.left(), below), g, k.muted_fg);
    }
    switch(ui, k, Pos2::new(rect.right() - 38.0, rect.top() + pad), label, value)
}

/// The 38 x 22 switch at `at`; the knob slides.
pub(in crate::app::ui) fn switch(ui: &mut egui::Ui, k: &K, at: Pos2, label: &str, value: &mut bool) -> bool {
    let rect = Rect::from_min_size(at, Vec2::new(38.0, 22.0));
    let response = ui.interact(rect, ui.scope_id().with(("switch", label)), Sense::click()).on_hover_cursor(CursorIcon::PointingHand);
    response.widget_info(|| WidgetInfo::selected(Role::Switch, true, *value, label));
    let t = ui.ctx().animate_bool_with_time(response.id, *value, 0.12);
    let off = if k.dark { Color32::from_white_alpha(64) } else { Color32::from_black_alpha(46) };
    ui.painter().rect_filled(rect, 11.0, off.lerp_to_gamma(k.primary, t));
    let x = rect.left() + 11.0 + 15.0 * t;
    ui.painter().circle_filled(Pos2::new(x, rect.center().y), 9.0, Color32::WHITE);
    let clicked = response.clicked();
    if clicked {
        *value = !*value;
    }
    clicked
}

/// `SelectField`: 26 high, at least 104 wide, the options in a menu. Returns the option picked.
pub(in crate::app::ui) fn select(ui: &mut egui::Ui, k: &K, label: &str, options: &[&str], chosen: usize) -> Option<usize> {
    let font = FontId::proportional(BODY);
    let widest = options.iter().map(|o| ui.painter().layout_no_wrap(o.to_string(), font.clone(), k.fg).size().x).fold(0.0, f32::max);
    let width = (widest + 8.125 + 22.75).max(104.0);
    let (rect, response) = ui.allocate_exact_size(Vec2::new(width, ROW), Sense::click());
    let response = response.on_hover_cursor(CursorIcon::PointingHand);
    let value = options.get(chosen).copied().unwrap_or("");
    response.widget_info(|| WidgetInfo::labeled(Role::Button, true, format!("{label}: {value}")));
    ui.painter().rect_filled(rect, RADIUS, k.bg);
    ui.painter().rect_stroke(rect, RADIUS, Stroke::new(1.0, if response.hovered() { k.muted_fg } else { k.input }), StrokeKind::Inside);
    ui.painter().text(Pos2::new(rect.left() + 8.125, rect.center().y), egui::Align2::LEFT_CENTER, value, font.clone(), k.fg);
    // the chevron, 12 px at 8 from the right
    let c = Pos2::new(rect.right() - 14.0, rect.center().y);
    ui.painter().add(Shape::line(vec![c + Vec2::new(-3.0, -1.5), c + Vec2::new(0.0, 1.5), c + Vec2::new(3.0, -1.5)], Stroke::new(1.4, k.muted_fg)));
    let mut pick = None;
    egui::Popup::menu(&response).frame(popup_frame(k)).width(width).show(|ui| {
        ui.spacing_mut().item_spacing.y = 0.0;
        for (i, option) in options.iter().enumerate() {
            let (rect, response) = ui.allocate_exact_size(Vec2::new(ui.available_width(), 25.75), Sense::click());
            let response = response.on_hover_cursor(CursorIcon::PointingHand);
            response.widget_info(|| WidgetInfo::labeled(Role::Button, true, *option));
            if response.hovered() {
                ui.painter().rect_filled(rect, 4.125, if k.dark { Color32::from_white_alpha(26) } else { Color32::from_black_alpha(20) });
            }
            if i == chosen {
                paint_icon_scaled(ui.painter(), Icon::Check, Pos2::new(rect.left() + 13.0, rect.center().y), k.fg, 13.0 / 16.0, 0.0);
            }
            let size = FontId::proportional(CONTROL);
            ui.painter().text(Pos2::new(rect.left() + 26.0, rect.center().y), egui::Align2::LEFT_CENTER, option, size, k.fg);
            if response.clicked() {
                pick = Some(i);
            }
        }
    });
    pick.filter(|i| *i != chosen)
}

/// The frame of every menu inside the window (`DropdownMenuContent`: `rounded-md border bg-popover p-1`), flat.
pub(in crate::app::ui) fn popup_frame(k: &K) -> egui::Frame {
    egui::Frame::new().fill(k.card).corner_radius(RADIUS).inner_margin(3.25).stroke(Stroke::new(1.0, k.border)).shadow(egui::epaint::Shadow::NONE)
}

/// `NumberStepper`: − the value + in a muted pill. True when it changed.
pub(in crate::app::ui) fn stepper(ui: &mut egui::Ui, k: &K, label: &str, value: &mut u8, range: std::ops::RangeInclusive<u8>, shown: &str) -> bool {
    let (rect, _) = ui.allocate_exact_size(Vec2::new(22.75 * 2.0 + 39.0, 22.75), Sense::hover());
    ui.painter().rect_filled(rect, RADIUS, k.muted);
    let mut changed = false;
    // laid out right to left, so the plus is the first one made
    for (i, (sign, delta)) in [(Icon::Plus, 1i16), (Icon::Minus, -1)].into_iter().enumerate() {
        let side = if i == 0 { rect.right() - 22.75 } else { rect.left() };
        let button = Rect::from_min_size(Pos2::new(side, rect.top()), Vec2::splat(22.75));
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
            k.muted_fg
        };
        paint_icon(ui.painter(), sign, button.center(), color);
        if live && response.on_hover_cursor(CursorIcon::PointingHand).clicked() {
            *value = next as u8;
            changed = true;
        }
    }
    ui.painter().text(rect.center(), egui::Align2::CENTER_CENTER, shown, FontId::monospace(CONTROL), k.fg);
    changed
}

/// An outline button (`variant="outline"`, 24 high); `fill` is for the one that does the main thing.
pub(in crate::app::ui) fn button(ui: &mut egui::Ui, k: &K, text: &str) -> egui::Response {
    let g = ui.painter().layout_no_wrap(text.into(), FontId::proportional(CONTROL), k.fg);
    let (rect, response) = ui.allocate_exact_size(Vec2::new(g.size().x + 18.0, 27.0), Sense::click());
    let response = response.on_hover_cursor(CursorIcon::PointingHand);
    response.widget_info(|| WidgetInfo::labeled(Role::Button, true, text));
    let fill = if response.hovered() {
        k.muted
    } else if k.dark {
        k.input.gamma_multiply(0.3)
    } else {
        k.bg
    };
    ui.painter().rect_filled(rect, 8.125, fill);
    ui.painter().rect_stroke(rect, 8.125, Stroke::new(1.0, if k.dark { k.input } else { k.border }), StrokeKind::Inside);
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
        ui.painter().rect_filled(rect, 8.125, k.muted.gamma_multiply(if k.dark { 0.5 } else { 1.0 }));
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

/// `Input`: a box 26 high with the text inside, the border going to the accent on focus.
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
    let (rect, _) = ui.allocate_exact_size(Vec2::new(width, ROW), Sense::hover());
    let inner = Rect::from_min_max(Pos2::new(rect.left() + 8.125, rect.top()), Pos2::new(rect.right() - 8.125, rect.bottom()));
    let focused = ui.memory(|m| m.has_focus(id));
    ui.painter().rect_filled(rect, 8.125, k.bg);
    ui.painter().rect_stroke(rect, 8.125, Stroke::new(1.0, if focused { k.primary } else { k.input }), StrokeKind::Inside);
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

/// `SegmentedTabs`: pills in a `bg-secondary` track, centered; the chosen one a card. Returns the pick.
pub(in crate::app::ui) fn segmented_tabs(ui: &mut egui::Ui, k: &K, id: egui::Id, labels: &[&str], chosen: usize) -> usize {
    let font = FontId::proportional(BODY);
    let widths: Vec<f32> = labels.iter().map(|l| ui.painter().layout_no_wrap(l.to_string(), font.clone(), k.fg).size().x + 22.75).collect();
    let total = widths.iter().sum::<f32>() + 1.625 * (labels.len() as f32 - 1.0).max(0.0) + 3.25;
    let (strip, _) = ui.allocate_exact_size(Vec2::new(ui.available_width(), 29.0), Sense::hover());
    let track = Rect::from_center_size(strip.center(), Vec2::new(total, 29.0));
    ui.painter().rect_filled(track, 8.125, k.secondary);
    let mut x = track.left() + 1.625;
    let mut pick = chosen;
    for (i, (label, width)) in labels.iter().zip(widths).enumerate() {
        let tab = Rect::from_min_size(Pos2::new(x, track.top() + 1.625), Vec2::new(width, 26.0));
        let response = ui.interact(tab, id.with(i), Sense::click()).on_hover_cursor(CursorIcon::PointingHand);
        response.widget_info(|| WidgetInfo::labeled(Role::Button, true, *label));
        if i == chosen {
            ui.painter().rect_filled(tab.translate(Vec2::new(0.0, 0.5)), 7.0, Color32::from_black_alpha(if k.dark { 60 } else { 18 }));
            ui.painter().rect_filled(tab, 7.0, k.card);
        }
        let color = if i == chosen || response.hovered() { k.fg } else { k.muted_fg };
        ui.painter().text(tab.center(), egui::Align2::CENTER_CENTER, label, font.clone(), color);
        if response.clicked() {
            pick = i;
        }
        x += width + 1.625;
    }
    pick
}
