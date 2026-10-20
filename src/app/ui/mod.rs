//! Floating UI building blocks, fonts and visuals.

use crate::prelude::*;

pub(crate) mod icons;
pub(crate) mod palette;
mod menus;
mod properties;
mod toolbar;

pub(crate) struct UiColors {
    pub(crate) island: Color32,
    pub(crate) surface_low: Color32,
    pub(crate) surface_high: Color32,
    pub(crate) selected: Color32,
    pub(crate) text: Color32,
    pub(crate) muted: Color32,
}

impl UiColors {
    // css/theme.scss
    pub(crate) fn new(dark: bool) -> Self {
        if dark {
            Self {
                island: Color32::from_rgb(0x23, 0x23, 0x29),
                // lifted above the island (#232329) so buttons and toggles stay visible
                surface_low: Color32::from_rgb(0x31, 0x30, 0x3c),
                surface_high: Color32::from_rgb(0x3d, 0x3c, 0x4b),
                selected: Color32::from_rgb(0x40, 0x3e, 0x6a),
                text: Color32::from_rgb(0xe3, 0xe3, 0xe8),
                muted: Color32::from_rgb(0x7a, 0x7a, 0x7a),
            }
        } else {
            Self {
                island: Color32::WHITE,
                surface_low: Color32::from_rgb(0xec, 0xec, 0xf4),
                surface_high: Color32::from_rgb(0xf1, 0xf0, 0xff),
                selected: Color32::from_rgb(0xe0, 0xdf, 0xff),
                text: Color32::from_rgb(0x1b, 0x1b, 0x1f),
                muted: Color32::from_rgb(0x99, 0x99, 0x99),
            }
        }
    }
}

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

pub(crate) fn separator(ui: &mut egui::Ui, c: &UiColors) {
    let (rect, _) = ui.allocate_exact_size(Vec2::new(1.0, 36.0), Sense::hover());
    ui.painter().line_segment(
        [rect.center_top() + Vec2::new(0.0, 8.0), rect.center_bottom() - Vec2::new(0.0, 8.0)],
        Stroke::new(1.0, c.surface_low),
    );
}

pub(crate) fn section(ui: &mut egui::Ui, title: &str, c: &UiColors) {
    ui.add_space(6.0);
    ui.label(egui::RichText::new(title).size(12.0).color(c.text));
    ui.add_space(2.0);
}

pub(crate) fn prop_button(ui: &mut egui::Ui, selected: bool, icon: Icon, tip: &str, c: &UiColors) -> bool {
    let response = square_button(ui, 32.0, selected, c.surface_low, c);
    let fg = if selected && !ui.visuals().dark_mode { Color32::from_rgb(0x03, 0x03, 0x5c) } else { c.text };
    paint_icon(ui.painter(), icon, response.rect.center(), fg);
    response.on_hover_text(tip).clicked()
}

/// .dropdown-menu-item: 32px row, 8px padding, 16px icon, 10px gap, 14px text, shortcut at 50%.
pub(crate) fn menu_item(ui: &mut egui::Ui, icon: Icon, label: &str, shortcut: &str, color: Color32, c: &UiColors) -> bool {
    let (rect, response) = ui.allocate_exact_size(Vec2::new(ui.available_width(), 32.0), Sense::click());
    let response = response.on_hover_cursor(CursorIcon::PointingHand);
    let p = ui.painter();
    if response.hovered() {
        p.rect_filled(rect, 6, c.surface_high);
    }
    paint_icon(p, icon, Pos2::new(rect.left() + 16.0, rect.center().y), color);
    p.text(Pos2::new(rect.left() + 34.0, rect.center().y), egui::Align2::LEFT_CENTER, label, FontId::proportional(14.0), color);
    p.text(rect.right_center() - Vec2::new(8.0, 0.0), egui::Align2::RIGHT_CENTER, shortcut, FontId::proportional(14.0), color.gamma_multiply(0.5));
    response.clicked()
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
                    p.rect_filled(
                        Rect::from_min_size(inner.min + Vec2::new(col as f32, row as f32) * cell, Vec2::splat(cell)),
                        0,
                        Color32::from_gray(225),
                    );
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
    if ui.color_edit_button_srgba(&mut color).on_hover_text("Mais cores").changed() {
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
    v.window_stroke = Stroke::new(1.0, Color32::from_black_alpha(20));
    v.override_text_color = Some(c.text);
    v.slider_trailing_fill = true;
    v.interact_cursor = Some(CursorIcon::PointingHand);
    ctx.set_visuals(v);
}

pub(crate) fn install_fonts(ctx: &Context) {
    let mut fonts = egui::FontDefinitions::default();
    fonts
        .font_data
        .insert("Assistant".into(), egui::FontData::from_static(include_bytes!(concat!(env!("CARGO_MANIFEST_DIR"), "/assets/Assistant.ttf"))).into());
    fonts.families.get_mut(&egui::FontFamily::Proportional).unwrap().insert(0, "Assistant".into());
    let sources: [&[u8]; 7] = [
        include_bytes!(concat!(env!("CARGO_MANIFEST_DIR"), "/assets/hand-1.ttf")),
        include_bytes!(concat!(env!("CARGO_MANIFEST_DIR"), "/assets/hand-2.ttf")),
        include_bytes!(concat!(env!("CARGO_MANIFEST_DIR"), "/assets/hand-3.ttf")),
        include_bytes!(concat!(env!("CARGO_MANIFEST_DIR"), "/assets/hand-4.ttf")),
        include_bytes!(concat!(env!("CARGO_MANIFEST_DIR"), "/assets/hand-5.ttf")),
        include_bytes!(concat!(env!("CARGO_MANIFEST_DIR"), "/assets/hand-6.ttf")),
        include_bytes!(concat!(env!("CARGO_MANIFEST_DIR"), "/assets/hand-7.ttf")),
    ];
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
            fonts.families.get_mut(&egui::FontFamily::Proportional).unwrap().push(name);
        }
    }
    let egui_fallbacks: Vec<String> = fonts.families[&egui::FontFamily::Proportional]
        .iter()
        .filter(|n| !names.contains(n))
        .cloned()
        .collect();
    names.extend(egui_fallbacks);
    fonts.families.insert(egui::FontFamily::Name(HAND_FONT.into()), names);
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

// ---------- geometry & rendering ----------
