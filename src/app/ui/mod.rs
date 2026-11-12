//! Floating UI building blocks, fonts and visuals.

use crate::prelude::*;

pub(crate) mod agents;
pub(crate) mod context;
mod dialog;
pub(crate) use dialog::*;
pub(crate) mod hsv;
pub(crate) mod icons;
pub(crate) mod links;
pub(crate) mod menu_look;
pub(crate) use menu_look::*;
mod menus;
pub(crate) mod nodes;
pub(crate) mod palette;
mod properties;
pub(crate) mod settings;
pub(crate) mod sidebar;
pub(crate) mod titlebar;
mod toolbar;

mod folders;

pub(crate) use folders::*;
pub(crate) use menus::menu_line;
pub(crate) use titlebar::edge_resize;
#[cfg(test)]
pub(crate) use titlebar::resize_edge;

/// The title bar the app draws off the Mac, tall enough for the toolbar; macOS keeps its native one.
pub(crate) const TITLE_BAR: f32 = if cfg!(target_os = "macos") { 0.0 } else { 60.0 };
/// Top of the toolbar row: it shares the row with the window buttons (the title bar off the Mac),
/// as one piece with the canvas.
pub(crate) const TOP_ROW: f32 = 12.0;
/// The middle of that row, and of the window buttons on macOS (`place_window_buttons`).
pub(crate) const LIGHTS_Y: f32 = TOP_ROW + GLASS_HEIGHT / 2.0;
/// The left edge of the canvas's top-left buttons: after the window buttons on macOS.
pub(crate) const MENU_LEFT: f32 = if cfg!(target_os = "macos") { 84.0 } else { 12.0 };
/// How far the corner pills sit from the window's edge, and from one another.
pub(crate) const CORNER_GAP: f32 = 10.0;
pub(crate) const PILL_GAP: f32 = 6.5;

/// The canvas's ground in the dark while the drawing keeps its white one (the reference app's).
pub(crate) const CANVAS_DARK: Color32 = Color32::from_rgb(0x1e, 0x1e, 0x1e);

/// The glass buttons: 30 round (the reference app has 26; asked bigger) and 5 apart, in a pill 5 above and below them and
/// 6.5 past its ends.
pub(crate) const GLASS_BUTTON: f32 = 30.0;
pub(crate) const GLASS_GAP: f32 = 5.0;
pub(crate) const GLASS_HEIGHT: f32 = 40.0;
const GLASS_END: f32 = 6.5;
/// Their icons, a little over the 16 the app's icons are drawn at.
pub(crate) const GLASS_ICON: f32 = 1.05;
/// Where the toolbar's row ends.
pub(crate) const TOOLBAR_BOTTOM: f32 = TOP_ROW + GLASS_HEIGHT;

/// The ink on glass and of the window's own row (the reference app's): nearly white in the dark,
/// nearly black in the light.
pub(crate) fn glass_ink(dark: bool) -> Color32 {
    if dark { Color32::from_rgb(0xf5, 0xf5, 0xf7) } else { Color32::from_rgb(0x1d, 0x1d, 0x1f) }
}

/// The reference app's accent: the tool in use, the open minimap, the windows on it.
pub(crate) fn glass_accent(dark: bool) -> Color32 {
    if dark { Color32::from_rgb(0x0a, 0x84, 0xff) } else { Color32::from_rgb(0x00, 0x7a, 0xff) }
}

/// The reference app's glass at `rect`: a hair of light around it, and the fill fading down it
/// (96% at the top, 90% near the middle, 74% at three quarters, 42% at the foot), `strong` the
/// same fill evenly at 78%. No shadow under it: the owner wants it flat for now.
// ponytail: egui can't blur what is behind it nor fill with a gradient, so the fade is a row of
// one-point bands of the same rounded shape; a backdrop pass in the renderer would do both
pub(crate) fn paint_glass(p: &Painter, rect: Rect, radius: f32, dark: bool, strong: bool) {
    let [r, g, b] = if dark { [44, 44, 46] } else { [255, 255, 255] };
    let fade = |t: f32| match t {
        _ if strong => 0.78,
        t if t < 0.48 => 0.96 - 0.06 * t / 0.48,
        t if t < 0.76 => 0.90 - 0.16 * (t - 0.48) / 0.28,
        t => 0.74 - 0.32 * (t - 0.76) / 0.24,
    };
    let rows = rect.height().ceil().max(1.0) as usize;
    for row in 0..rows {
        // the first and last bands take the edge's feather with them
        let top = if row == 0 { rect.top() - 1.0 } else { rect.top() + row as f32 };
        let bottom = if row + 1 == rows { rect.bottom() + 1.0 } else { rect.top() + row as f32 + 1.0 };
        let band = Rect::from_min_max(Pos2::new(rect.left() - 1.0, top), Pos2::new(rect.right() + 1.0, bottom)).intersect(p.clip_rect());
        let alpha = (fade((row as f32 + 0.5) / rows as f32) * 255.0).round() as u8;
        p.with_clip_rect(band).rect_filled(rect, radius, Color32::from_rgba_unmultiplied(r, g, b, alpha));
    }
    let rim = if dark { Color32::from_white_alpha(0x24) } else { Color32::from_black_alpha(14) };
    p.rect_stroke(rect, radius, Stroke::new(1.0, rim), StrokeKind::Inside);
}

/// How wide a glass pill is around `buttons` buttons and the `others` (each thing a gap from the next).
pub(crate) fn glass_width(buttons: usize, others: &[f32]) -> f32 {
    let things = buttons + others.len();
    buttons as f32 * GLASS_BUTTON + others.iter().sum::<f32>() + things.saturating_sub(1) as f32 * GLASS_GAP + 2.0 * GLASS_END
}

/// A pill of glass `width` wide, and its things in a row inside it.
pub(crate) fn glass_row<R>(ui: &mut egui::Ui, width: f32, dark: bool, add: impl FnOnce(&mut egui::Ui) -> R) -> R {
    let (rect, _) = ui.allocate_exact_size(Vec2::new(width, GLASS_HEIGHT), Sense::hover());
    paint_glass(ui.painter(), rect, GLASS_HEIGHT / 2.0, dark, false);
    let inside = rect.shrink2(Vec2::new(GLASS_END, (GLASS_HEIGHT - GLASS_BUTTON) / 2.0));
    let row = egui::UiBuilder::new().max_rect(inside).layout(egui::Layout::left_to_right(egui::Align::Center));
    ui.scope_builder(row, |ui| {
        ui.spacing_mut().item_spacing.x = GLASS_GAP;
        add(ui)
    })
    .inner
}

/// A round button on glass: a faint disc under the pointer, its icon in the accent while `on`,
/// and a swell when clicked (to 122% and back in 0.38 s).
pub(crate) fn glass_button(ui: &mut egui::Ui, on: bool, icon: Icon, dark: bool) -> egui::Response {
    let (rect, response) = ui.allocate_exact_size(Vec2::splat(GLASS_BUTTON), Sense::click());
    let ink = glass_ink(dark);
    if response.hovered() {
        ui.painter().circle_filled(rect.center(), GLASS_BUTTON / 2.0, ink.gamma_multiply(0.1));
    }
    let now = ui.input(|i| i.time);
    if response.clicked() {
        ui.data_mut(|d| d.insert_temp(response.id, now));
    }
    let since = ((now - ui.data(|d| d.get_temp::<f64>(response.id)).unwrap_or(f64::MIN)) / 0.38) as f32;
    let scale = match since {
        t if t >= 1.0 => 1.0,
        t if t < 0.32 => 1.0 + 0.22 * t / 0.32,
        t if t < 0.72 => 1.22 - 0.25 * (t - 0.32) / 0.4,
        t => 0.97 + 0.03 * (t - 0.72) / 0.28,
    };
    if since < 1.0 {
        ui.ctx().request_repaint();
    }
    paint_icon_scaled(ui.painter(), icon, rect.center(), if on { glass_accent(dark) } else { ink.gamma_multiply(0.85) }, GLASS_ICON * scale, 0.0);
    response.on_hover_cursor(CursorIcon::PointingHand)
}

/// A tip under a widget in the app's look: round, small, after the pointer rests on it, fading
/// and rising into place; the next one comes at once while the pointer goes from button to button.
pub(crate) trait Tip: Sized {
    fn tip(self, text: impl Into<String>) -> Self;
}

/// How long the pointer rests before a tip shows, and how long the tip takes to come and go.
const TIP_DELAY: f64 = 0.45;
const TIP_FADE: f32 = 0.14;

impl Tip for egui::Response {
    fn tip(self, text: impl Into<String>) -> Self {
        let (ctx, id, last) = (self.ctx.clone(), self.id.with("tip"), egui::Id::unique("tip_last"));
        let now = ctx.input(|i| i.time);
        // a menu open this frame or the last (ui/context.rs) or an egui popup hides the tips
        let menu = ctx.data(|d| d.get_temp::<u64>(egui::Id::unique("menu_frame"))).is_some_and(|f| f + 1 >= ctx.cumulative_frame_nr());
        let over = self.hovered() && !ctx.input(|i| i.pointer.any_down()) && !menu && !egui::Popup::is_any_open(&ctx);
        // since when the pointer rests here
        let since = ctx.data_mut(|d| {
            if over {
                *d.get_temp_mut_or_insert_with(id, || now)
            } else {
                d.remove::<f64>(id);
                now
            }
        });
        let warm = ctx.data(|d| d.get_temp::<f64>(last)).is_some_and(|t| now - t < 0.4);
        let shown = over && (warm || now - since >= TIP_DELAY);
        if over && !shown {
            ctx.request_repaint_after(std::time::Duration::from_secs_f64(TIP_DELAY - (now - since)));
        }
        let t = ctx.animate_bool_with_time(id, shown, TIP_FADE);
        if shown {
            ctx.data_mut(|d| d.insert_temp(last, now));
        }
        // its text is the widget's name for the system's accessibility too (screen readers, agents)
        let text = text.into();
        self.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Button, true, &text));
        if t > 0.0 {
            paint_tip(&ctx, id, self.rect, &text, t);
        }
        self
    }
}

/// The tip itself: centered under `widget` (over it when there is no room), kept inside the window.
fn paint_tip(ctx: &Context, id: egui::Id, widget: Rect, text: &str, t: f32) {
    let dark = ctx.global_style().visuals.dark_mode;
    let (fill, ink) =
        if dark { (Color32::from_rgb(0x3a, 0x3a, 0x3c), Color32::from_rgb(0xf5, 0xf5, 0xf7)) } else { (Color32::from_rgb(0x1d, 0x1d, 0x1f), Color32::WHITE) };
    let p = ctx.layer_painter(egui::LayerId::new(egui::Order::Tooltip, id));
    let screen = ctx.content_rect();
    let galley = p.layout(text.to_string(), FontId::proportional(12.0), ink.gamma_multiply(t), 320.0);
    let size = galley.size() + Vec2::new(18.0, 10.0);
    let below = widget.bottom() + 8.0 + size.y <= screen.bottom();
    // rises (or drops) 4 points into place as it fades in
    let slide = (1.0 - t) * 4.0;
    let top = if below { widget.bottom() + 8.0 - slide } else { widget.top() - 8.0 - size.y + slide };
    let left = (widget.center().x - size.x / 2.0).clamp(screen.left() + 6.0, (screen.right() - 6.0 - size.x).max(screen.left() + 6.0));
    let rect = Rect::from_min_size(Pos2::new(left, top), size);
    p.rect_filled(rect, 8, fill.gamma_multiply(t));
    p.rect_stroke(rect, 8, Stroke::new(1.0, Color32::from_white_alpha(if dark { 0x1f } else { 0 }).gamma_multiply(t)), StrokeKind::Inside);
    p.galley(rect.min + Vec2::new(9.0, 5.0), galley, ink);
}

/// The hair between two groups of buttons on glass.
pub(crate) fn glass_divider(ui: &mut egui::Ui, dark: bool) {
    let (rect, _) = ui.allocate_exact_size(Vec2::new(1.0, GLASS_BUTTON), Sense::hover());
    ui.painter().vline(rect.center().x, rect.shrink2(Vec2::new(0.0, 5.0)).y_range(), Stroke::new(1.0, glass_ink(dark).gamma_multiply(0.2)));
}

pub(crate) struct UiColors {
    pub(crate) island: Color32,
    pub(crate) surface_low: Color32,
    pub(crate) surface_high: Color32,
    pub(crate) selected: Color32,
    pub(crate) text: Color32,
    pub(crate) muted: Color32,
    // agent states: error, waiting for you, working; as text on the panels too
    pub(crate) danger: Color32,
    pub(crate) warning: Color32,
    pub(crate) success: Color32,
}

impl UiColors {
    // css/theme.scss
    pub(crate) fn new(dark: bool) -> Self {
        if dark {
            Self {
                island: Color32::from_rgb(0x23, 0x23, 0x29),
                // lifted above the island (#232329) so buttons and toggles stay visible
                surface_low: Color32::from_rgb(0x30, 0x30, 0x33),
                surface_high: Color32::from_rgb(0x3c, 0x3c, 0x40),
                selected: Color32::from_rgb(0x1c, 0x3d, 0x66),
                text: Color32::from_rgb(0xe3, 0xe3, 0xe8),
                muted: Color32::from_rgb(0x7a, 0x7a, 0x7a),
                danger: Color32::from_rgb(0xf8, 0x51, 0x49),
                warning: Color32::from_rgb(0xf0, 0xa0, 0x20),
                success: Color32::from_rgb(0x3f, 0xb9, 0x50),
            }
        } else {
            Self {
                island: Color32::WHITE,
                surface_low: Color32::from_rgb(0xec, 0xec, 0xee),
                surface_high: Color32::from_rgb(0xf2, 0xf2, 0xf5),
                selected: Color32::from_rgb(0xd6, 0xe8, 0xff),
                text: Color32::from_rgb(0x1b, 0x1b, 0x1f),
                muted: Color32::from_rgb(0x99, 0x99, 0x99),
                danger: Color32::from_rgb(0xc9, 0x2a, 0x2a),
                warning: Color32::from_rgb(0x90, 0x64, 0x20),
                success: Color32::from_rgb(0x28, 0x70, 0x37),
            }
        }
    }
}

/// The marks a terminal can wear (Aparência), by key; "" picks one from its command.
pub(crate) const TERMINAL_ICONS: [&str; 12] = ["", "terminal", "claude", "codex", "agents", "monitor", "search", "composer", "palette", "grid", "eye", "map"];
/// What each mark is called, for its tooltip.
pub(crate) const TERMINAL_ICON_NAMES: [&str; 12] =
    ["Pelo comando", "Terminal", "Claude", "Codex", "Agentes", "Monitor", "Busca", "Mensagem", "Raio", "Grade", "Olho", "Mapa"];
/// Its tints: none (the strip's light gray) and the reference app's eight.
pub(crate) const TERMINAL_COLORS: [Option<[u8; 3]>; 9] = [
    None,
    Some([0x3d, 0x7e, 0xf0]),
    Some([0x2f, 0x9e, 0x44]),
    Some([0xe8, 0x59, 0x0c]),
    Some([0x9c, 0x5d, 0xe0]),
    Some([0xe0, 0x31, 0x31]),
    Some([0x2a, 0xac, 0xb8]),
    Some([0xf0, 0xb4, 0x00]),
    Some([0xd6, 0x33, 0x84]),
];

/// A terminal's mark at `at`: the chosen one, else Claude's, a four-point star for Codex, or the
/// terminal.
pub(crate) fn terminal_icon(p: &Painter, spec: &TerminalSpec, at: Pos2, color: Color32) {
    let command = spec.command.split_whitespace().next().unwrap_or_default();
    let key = match spec.icon.as_str() {
        "" if command == "claude" || command == "codex" => command,
        "" => "terminal",
        key => key,
    };
    match key {
        // the agents' own marks (assets/ide-icons/NOTICE): Claude's in its orange, OpenAI's in the text color
        "claude" => crate::ide::icons::paint(p, crate::ide::icons::CLAUDE_MARK, Rect::from_center_size(at, Vec2::splat(16.0))),
        "codex" => crate::ide::icons::paint_tint(p, crate::ide::icons::OPENAI, Rect::from_center_size(at, Vec2::splat(16.0)), color),
        "agents" => paint_icon(p, Icon::Agents, at, color),
        "monitor" => paint_icon(p, Icon::Monitor, at, color),
        "search" => paint_icon(p, Icon::Search, at, color),
        "composer" => paint_icon(p, Icon::Composer, at, color),
        "palette" => paint_icon(p, Icon::Palette, at, color),
        "grid" => paint_icon(p, Icon::Grid, at, color),
        "eye" => paint_icon(p, Icon::Eye, at, color),
        "map" => paint_icon(p, Icon::Map, at, color),
        _ => paint_icon(p, Icon::Tool(Tool::Terminal), at, color),
    }
}

/// `terminal_icon` scaled around `at` by `scale` (a card's title strip: the mark follows the canvas's zoom, as the name does).
pub(crate) fn terminal_icon_scaled(p: &Painter, spec: &TerminalSpec, at: Pos2, color: Color32, scale: f32) {
    let layer = p.layer_id();
    let next = || p.ctx().graphics_mut(|g| g.entry(layer).next_idx());
    let start = next();
    terminal_icon(p, spec, at, color);
    let end = next();
    let transform = egui::emath::TSTransform::new(at.to_vec2() * (1.0 - scale), scale);
    p.ctx().graphics_mut(|g| g.entry(layer).transform_range(start, end, transform));
}

/// The Maestro's mark in a terminal's title strip: a small crown, drawn from convex pieces.
pub(crate) fn maestro_crown(p: &Painter, at: Pos2, zoom: f32, color: Color32) {
    let pt = |x: f32, y: f32| at + Vec2::new(x, y) * zoom;
    p.rect_filled(Rect::from_min_max(pt(-6.0, 1.0), pt(6.0, 5.0)), 0, color);
    for spike in [[(-6.5, 1.0), (-6.0, -4.0), (-3.0, 1.0)], [(-3.5, 1.0), (0.0, -6.0), (3.5, 1.0)], [(3.0, 1.0), (6.0, -4.0), (6.5, 1.0)]] {
        p.add(Shape::convex_polygon(spike.map(|(x, y)| pt(x, y)).to_vec(), color, Stroke::NONE));
    }
}

/// Pills in a row, centered; the chosen one filled. Returns true when the choice changed.
pub(crate) fn segmented(ui: &mut egui::Ui, chosen: &mut usize, labels: &[&str], c: &UiColors) -> bool {
    let mut changed = false;
    ui.horizontal(|ui| {
        let widths: Vec<f32> = labels.iter().map(|l| ui.painter().layout_no_wrap(l.to_string(), FontId::proportional(12.5), c.text).size().x + 28.0).collect();
        ui.add_space(((ui.available_width() - widths.iter().sum::<f32>() - 4.0) / 2.0).max(0.0));
        group_frame(c).inner_margin(2).show(ui, |ui| {
            ui.spacing_mut().item_spacing.x = 0.0;
            for (i, (label, width)) in labels.iter().zip(widths).enumerate() {
                let (rect, response) = ui.allocate_exact_size(Vec2::new(width, 24.0), Sense::click());
                if i == *chosen {
                    ui.painter().rect_filled(rect, 6, PRIMARY);
                }
                let color = if i == *chosen { Color32::WHITE } else { c.text };
                ui.painter().text(rect.center(), egui::Align2::CENTER_CENTER, label, FontId::proportional(12.5), color);
                if response.on_hover_cursor(CursorIcon::PointingHand).clicked() && i != *chosen {
                    *chosen = i;
                    changed = true;
                }
            }
        });
    });
    changed
}

/// The zoom's width (−, the percentage, + in their glass), for the pills left of it.
pub(crate) const ZOOM_WIDTH: f32 = 2.0 * GLASS_BUTTON + ZOOM_TEXT + 2.0 * GLASS_GAP + 2.0 * GLASS_END;
pub(crate) const ZOOM_TEXT: f32 = 48.0;

pub(crate) fn group_frame(c: &UiColors) -> egui::Frame {
    egui::Frame::new().fill(c.surface_low).corner_radius(8)
}

pub(crate) fn square_button(ui: &mut egui::Ui, size: f32, selected: bool, idle: Color32, c: &UiColors) -> egui::Response {
    let (rect, response) = ui.allocate_exact_size(Vec2::splat(size), Sense::click());
    let fill = if selected {
        c.selected
    } else if response.hovered() {
        c.surface_high
    } else {
        idle
    };
    ui.painter().rect_filled(rect, 8, fill);
    response.on_hover_cursor(CursorIcon::PointingHand)
}

/// A `square_button` with an icon in `fg` on it.
pub(crate) fn icon_button(ui: &mut egui::Ui, size: f32, selected: bool, idle: Color32, icon: Icon, fg: Color32, c: &UiColors) -> egui::Response {
    let response = square_button(ui, size, selected, idle, c);
    paint_icon(ui.painter(), icon, response.rect.center(), fg);
    response
}

/// Shortcuts are written the Mac way ("Cmd+S", "Option+/", "⌘K"); off the Mac the same keys are
/// Ctrl and Alt (egui's `command` is Ctrl there).
pub(crate) fn keys_label(text: &str) -> String {
    if cfg!(target_os = "macos") { text.into() } else { text.replace('⌘', "Ctrl+").replace("Cmd", "Ctrl").replace("Option", "Alt") }
}

/// A row of the app's menus (the sidebar's, the toolbar's "Mais ferramentas"): the same look as the
/// context menus (ui/menu_look.rs). `color` is the label's: the text color's own keeps the icon muted.
pub(crate) fn menu_item(ui: &mut egui::Ui, icon: Icon, label: &str, shortcut: &str, color: Color32, c: &UiColors) -> bool {
    menu_row(ui, Some(icon), label, shortcut, (color != c.text).then_some(color))
}

pub(crate) fn swatch(ui: &mut egui::Ui, color: [u8; 4], selected: bool, size: f32, dark: bool) -> egui::Response {
    let (rect, response) = ui.allocate_exact_size(Vec2::splat(size), Sense::click());
    let response = response.on_hover_cursor(CursorIcon::PointingHand);
    let p = ui.painter();
    let inner = rect.shrink(1.0);
    if color[3] == 0 {
        // checkerboard for "transparent"
        p.rect_filled(inner, 4, Color32::WHITE);
        let cell = inner.width() / 4.0;
        for row in 0..4 {
            for col in 0..4 {
                if (row + col) % 2 == 0 {
                    p.rect_filled(Rect::from_min_size(inner.min + Vec2::new(col as f32, row as f32) * cell, Vec2::splat(cell)), 0, Color32::from_gray(225));
                }
            }
        }
    } else {
        p.rect_filled(inner, 4, themed(rgba(color), dark));
    }
    p.rect_stroke(inner, 4, Stroke::new(1.0, Color32::from_black_alpha(25)), StrokeKind::Inside);
    if selected {
        p.rect_stroke(rect.expand(1.5), 5, Stroke::new(1.0, PRIMARY), StrokeKind::Outside);
    }
    response
}

pub(crate) fn color_picker(ui: &mut egui::Ui, rgba_value: &mut [u8; 4]) {
    let mut color = rgba(*rgba_value);
    if ui.color_edit_button_srgba(&mut color).tip("Mais cores").changed() {
        *rgba_value = [color.r(), color.g(), color.b(), color.a()];
    }
}

pub(crate) fn configure_visuals(ctx: &Context, dark: bool) {
    let c = UiColors::new(dark);
    let mut v = if dark { egui::Visuals::dark() } else { egui::Visuals::light() };
    v.selection.bg_fill = c.selected;
    v.selection.stroke = Stroke::new(1.0, PRIMARY);
    v.widgets.inactive.weak_bg_fill = c.surface_low;
    v.widgets.inactive.bg_fill = c.surface_low;
    v.widgets.inactive.bg_stroke = Stroke::NONE;
    v.widgets.hovered.bg_fill = c.surface_high;
    v.widgets.hovered.weak_bg_fill = c.surface_high;
    v.widgets.hovered.bg_stroke = Stroke::NONE;
    v.widgets.active.bg_fill = PRIMARY;
    v.widgets.inactive.corner_radius = 6.into();
    v.widgets.hovered.corner_radius = 6.into();
    v.window_fill = c.island;
    v.panel_fill = c.island;
    v.window_corner_radius = 12.into();
    // menus and egui's own tips (the ones not drawn by `Tip`)
    v.menu_corner_radius = MENU_RADIUS.into();
    // no shadows anywhere: the owner wants the chrome flat
    (v.window_shadow, v.popup_shadow) = (egui::epaint::Shadow::NONE, egui::epaint::Shadow::NONE);
    v.window_stroke = Stroke::new(1.0, Color32::from_black_alpha(20));
    v.override_text_color = Some(c.text);
    v.slider_trailing_fill = true;
    v.interact_cursor = Some(CursorIcon::PointingHand);
    ctx.set_visuals(v);
}

/// The app's fonts; `ligatures` keeps the code font's (=>, !=, ::) joined.
pub(crate) fn install_fonts(ctx: &Context, ligatures: bool) {
    let mut fonts = egui::FontDefinitions::default();
    fonts.font_data.insert("Assistant".into(), egui::FontData::from_static(ASSISTANT_FONT).into());
    fonts.families.get_mut(&egui::FontFamily::Proportional).unwrap().insert(0, "Assistant".into());
    let sources = HAND_FACES;
    let mut names = vec![];
    for (i, data) in sources.into_iter().enumerate() {
        let name = format!("hand{i}");
        fonts.font_data.insert(name.clone(), egui::FontData::from_static(data).into());
        names.push(name);
    }
    names.push("Assistant".into());
    // symbols the bundled fonts lack (→ ✓ ★ …) come from the system, then egui's emoji font
    for (i, path) in SYMBOL_FONTS.iter().enumerate() {
        if let Ok(data) = fs::read(path) {
            let name = format!("symbols{i}");
            fonts.font_data.insert(name.clone(), egui::FontData::from_owned(data).into());
            names.push(name.clone());
            fonts.families.get_mut(&egui::FontFamily::Proportional).unwrap().push(name.clone());
            // terminals draw agents' ✻ ⏺ glyphs too
            fonts.families.get_mut(&egui::FontFamily::Monospace).unwrap().push(name);
        }
    }
    let egui_fallbacks: Vec<String> = fonts.families[&egui::FontFamily::Proportional].iter().filter(|n| !names.contains(n)).cloned().collect();
    names.extend(egui_fallbacks);
    fonts.families.insert(egui::FontFamily::Name(HAND_FONT.into()), names);
    crate::ide::theme::add_fonts(&mut fonts, ligatures);
    ctx.set_fonts(fonts);
    ctx.all_styles_mut(|s| {
        s.text_styles.insert(egui::TextStyle::Body, FontId::proportional(14.0));
        s.text_styles.insert(egui::TextStyle::Button, FontId::proportional(14.0));
        s.text_styles.insert(egui::TextStyle::Heading, FontId::proportional(18.0));
        s.spacing.button_padding = Vec2::new(12.0, 6.0);
        s.spacing.interact_size.y = 28.0;
    });
    ctx.options_mut(|o| o.zoom_with_keyboard = false);
}
