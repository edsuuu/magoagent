//! Floating UI building blocks, fonts and visuals.

use crate::prelude::*;

pub(crate) mod agents;
pub(crate) mod context;
pub(crate) mod icons;
pub(crate) mod links;
mod menus;
mod nodes;
pub(crate) mod palette;
mod properties;
pub(crate) mod settings;
mod titlebar;
mod toolbar;

pub(crate) use titlebar::edge_resize;
#[cfg(test)]
pub(crate) use {menus::MENU_SECTIONS, titlebar::resize_edge};

/// The title bar the app draws off the Mac, tall enough for the toolbar; macOS keeps its native one.
pub(crate) const TITLE_BAR: f32 = if cfg!(target_os = "macos") { 0.0 } else { 60.0 };
/// Top of the toolbar row: it shares the row with the window buttons (the title bar off the Mac),
/// as one piece with the canvas.
pub(crate) const TOP_ROW: f32 = if cfg!(target_os = "macos") { 8.0 } else { 6.0 };
/// The left edge of the canvas's top-left buttons: after the window buttons on macOS.
pub(crate) const MENU_LEFT: f32 = if cfg!(target_os = "macos") { 84.0 } else { 16.0 };

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

/// The node dialogs (new terminal, portal, IDE) in the reference app's look: a card in the middle
/// with the title, the fields, a line, and Cancelar / Criar centered at the bottom. Esc cancels,
/// Enter confirms. Returns (confirm, cancel).
pub(crate) fn dialog_ui(ctx: &Context, c: &UiColors, title: &str, confirm: &str, body: impl FnOnce(&mut egui::Ui)) -> (bool, bool) {
    let (mut ok, mut cancel) = (false, false);
    egui::Area::new(egui::Id::unique(("dialog", title))).order(egui::Order::Foreground).anchor(egui::Align2::CENTER_CENTER, Vec2::ZERO).show(ctx, |ui| {
        egui::Frame::new()
            .fill(c.island)
            .corner_radius(14)
            .inner_margin(egui::Margin::symmetric(22, 18))
            .stroke(Stroke::new(1.0, c.surface_high))
            .shadow(egui::epaint::Shadow { offset: [0, 12], blur: 36, spread: 0, color: Color32::from_black_alpha(90) })
            .show(ui, |ui| {
                ui.set_width(DIALOG_WIDTH);
                ui.spacing_mut().item_spacing.y = 4.0;
                ui.vertical_centered(|ui| ui.label(egui::RichText::new(title).size(15.0).strong().color(c.text)));
                ui.add_space(12.0);
                body(ui);
                ui.add_space(14.0);
                let (line, _) = ui.allocate_exact_size(Vec2::new(DIALOG_WIDTH, 1.0), Sense::hover());
                ui.painter().hline(line.x_range(), line.center().y, Stroke::new(1.0, c.surface_high));
                ui.add_space(12.0);
                ui.horizontal(|ui| {
                    let button = |text: egui::RichText, fill| egui::Button::new(text).fill(fill).min_size(Vec2::new(96.0, 30.0)).corner_radius(8);
                    ui.add_space((DIALOG_WIDTH - 2.0 * 96.0 - ui.spacing().item_spacing.x) / 2.0);
                    cancel = ui.add(button(egui::RichText::new("Cancelar").color(c.text), c.surface_low)).clicked();
                    ok = ui.add(button(egui::RichText::new(confirm).color(Color32::WHITE), PRIMARY)).clicked();
                });
            });
    });
    ok |= ctx.input(|i| i.key_pressed(Key::Enter));
    cancel |= ctx.input(|i| i.key_pressed(Key::Escape));
    (ok, cancel)
}

pub(crate) const DIALOG_WIDTH: f32 = 420.0;

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

/// A field of a dialog: its name small and muted above, then the box.
pub(crate) fn dialog_field(ui: &mut egui::Ui, label: &str, text: &mut String, hint: &str, width: f32, c: &UiColors) -> egui::Response {
    ui.add_space(6.0);
    ui.label(egui::RichText::new(label).size(11.5).color(dialog_label(c)));
    let field = ui.add(
        egui::TextEdit::singleline(text)
            .hint_text(hint)
            .desired_width(width)
            .margin(egui::Margin::symmetric(10, 7))
            .background_color(c.surface_low)
            .font(FontId::proportional(13.5)),
    );
    // a dialog opens typing in its first field
    if ui.memory(|m| m.focused().is_none()) {
        field.request_focus();
    }
    field
}

/// A field for a key: the text hidden, and no focus taken (it sits low in a long dialog).
pub(crate) fn dialog_secret(ui: &mut egui::Ui, label: &str, text: &mut String, hint: &str, c: &UiColors) {
    ui.add_space(6.0);
    ui.label(egui::RichText::new(label).size(11.5).color(dialog_label(c)));
    ui.add(
        egui::TextEdit::singleline(text)
            .password(true)
            .hint_text(hint)
            .desired_width(DIALOG_WIDTH)
            .margin(egui::Margin::symmetric(10, 7))
            .background_color(c.surface_low)
            .font(FontId::proportional(13.5)),
    );
}

/// A folder field with Procurar… beside it.
pub(crate) fn dialog_folder(ui: &mut egui::Ui, label: &str, folder: &mut String, hint: &str, title: &str, c: &UiColors) {
    ui.horizontal(|ui| {
        let field = ui.vertical(|ui| dialog_field(ui, label, folder, hint, DIALOG_WIDTH - 110.0, c)).inner.rect;
        // the button on the field's line, as tall as it, and ending where the fields above end
        let spot = Rect::from_min_max(Pos2::new(field.right() + 8.0, field.top()), Pos2::new(field.left() + DIALOG_WIDTH, field.bottom()));
        let browse = egui::Button::new(egui::RichText::new("Procurar…").size(12.5).color(c.text)).fill(c.surface_low).corner_radius(8);
        if ui.put(spot, browse).clicked()
            && let Some(dir) = rfd::FileDialog::new().set_title(title).pick_folder()
        {
            *folder = dir.to_string_lossy().into();
        }
    });
}

/// The gray of a dialog's small labels: lighter than the muted gray, which is too faint for text
/// this small (WCAG AA wants 4.5:1).
pub(crate) fn dialog_label(c: &UiColors) -> Color32 {
    c.muted.lerp_to_gamma(c.text, 0.35)
}

/// A checkbox in the dialogs' look: a rounded square filled with the accent when on.
pub(crate) fn dialog_check(ui: &mut egui::Ui, on: &mut bool, label: &str, c: &UiColors) -> egui::Response {
    let galley = ui.painter().layout_no_wrap(label.into(), FontId::proportional(13.5), c.text);
    let (rect, response) = ui.allocate_exact_size(Vec2::new(24.0 + galley.size().x, 22.0), Sense::click());
    let box_ = Rect::from_min_size(Pos2::new(rect.left(), rect.center().y - 8.0), Vec2::splat(16.0));
    if *on {
        ui.painter().rect_filled(box_, 4, PRIMARY);
        paint_icon(ui.painter(), Icon::Check, box_.center(), Color32::WHITE);
    } else {
        ui.painter().rect_stroke(box_, 4, Stroke::new(1.2, if response.hovered() { c.text } else { c.muted }), StrokeKind::Inside);
    }
    ui.painter().galley(Pos2::new(rect.left() + 24.0, rect.center().y - galley.size().y / 2.0), galley, c.text);
    if response.clicked() {
        *on = !*on;
    }
    response.on_hover_cursor(CursorIcon::PointingHand)
}

/// The muted line under a dialog's fields.
pub(crate) fn dialog_note(ui: &mut egui::Ui, text: &str, c: &UiColors) {
    ui.add_space(8.0);
    ui.label(egui::RichText::new(text).size(11.0).color(dialog_label(c)));
}

/// The zoom's width (−, the percentage, +), for the buttons left of it.
pub(crate) const ZOOM_WIDTH: f32 = 36.0 + 58.0 + 36.0;

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
    separator_height(ui, c, 36.0);
}

pub(crate) fn separator_height(ui: &mut egui::Ui, c: &UiColors, height: f32) {
    let (rect, _) = ui.allocate_exact_size(Vec2::new(1.0, height), Sense::hover());
    ui.painter().line_segment([rect.center_top() + Vec2::new(0.0, 8.0), rect.center_bottom() - Vec2::new(0.0, 8.0)], Stroke::new(1.0, c.surface_low));
}

pub(crate) fn section(ui: &mut egui::Ui, title: &str, c: &UiColors) {
    ui.add_space(6.0);
    ui.label(egui::RichText::new(title).size(12.0).color(c.text));
    ui.add_space(2.0);
}

/// A `square_button` with an icon in `fg` on it.
pub(crate) fn icon_button(ui: &mut egui::Ui, size: f32, selected: bool, idle: Color32, icon: Icon, fg: Color32, c: &UiColors) -> egui::Response {
    let response = square_button(ui, size, selected, idle, c);
    paint_icon(ui.painter(), icon, response.rect.center(), fg);
    response
}

pub(crate) fn prop_button(ui: &mut egui::Ui, selected: bool, icon: Icon, tip: &str, c: &UiColors) -> bool {
    let fg = if selected && !ui.visuals().dark_mode { Color32::from_rgb(0x03, 0x03, 0x5c) } else { c.text };
    icon_button(ui, 32.0, selected, c.surface_low, icon, fg, c).on_hover_text(keys_label(tip)).clicked()
}

/// Shortcuts are written the Mac way ("Cmd+S", "Option+/", "⌘K"); off the Mac the same keys are
/// Ctrl and Alt (egui's `command` is Ctrl there).
pub(crate) fn keys_label(text: &str) -> String {
    if cfg!(target_os = "macos") { text.into() } else { text.replace('⌘', "Ctrl+").replace("Cmd", "Ctrl").replace("Option", "Alt") }
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
    p.text(rect.right_center() - Vec2::new(8.0, 0.0), egui::Align2::RIGHT_CENTER, keys_label(shortcut), FontId::proportional(14.0), color.gamma_multiply(0.5));
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
