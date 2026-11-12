//! How the right-click menus look, in the light and the dark theme: the reference app's context
//! menu (a popover surface with a hairline border, 26-high rows, 13px icons, 11.4pt text, the
//! row under the pointer in the text color's 10%, the shortcut or the submenu's arrow at the
//! right). The rows, their widths and the panels are drawn by `context::panel` from these.

use crate::prelude::*;

/// 1rem is 13px in the reference app: `min-w-[10rem]`, `min-w-[8rem]`, `p-1`, `py-1.5`, `gap-2`, `text-sm`.
pub(crate) const MENU_MIN: f32 = 130.0;
pub(crate) const SUB_MIN: f32 = 104.0;
pub(crate) const MENU_PAD: f32 = 3.25;
pub(crate) const ROW_HEIGHT: f32 = 26.0;
pub(crate) const ROW_PAD: f32 = 6.5;
pub(crate) const ROW_GAP: f32 = 6.5;
pub(crate) const ICON_SIZE: f32 = 13.0;
pub(crate) const MENU_TEXT_SIZE: f32 = 11.4;
/// A separator: `my-1`, a hairline, `-mx-1`.
pub(crate) const SEP_HEIGHT: f32 = MENU_PAD * 2.0 + 1.0;
pub(crate) const MENU_RADIUS: u8 = 6;
pub(crate) const ITEM_RADIUS: u8 = 4;

/// The popover's tokens: `--popover`, `--border`, `--foreground`, `--muted-foreground`, `--bg-active`.
pub(crate) struct MenuColors {
    pub(crate) bg: Color32,
    pub(crate) border: Color32,
    pub(crate) text: Color32,
    pub(crate) muted: Color32,
    pub(crate) hover: Color32,
}

pub(crate) fn menu_colors(dark: bool) -> MenuColors {
    if dark {
        MenuColors {
            bg: Color32::from_rgb(0x2c, 0x2c, 0x2e),
            border: Color32::from_rgba_unmultiplied(0x54, 0x54, 0x58, 0x80),
            text: Color32::from_rgb(0xf5, 0xf5, 0xf7),
            muted: Color32::from_rgba_unmultiplied(0xeb, 0xeb, 0xf5, 0x99),
            hover: Color32::from_rgba_unmultiplied(0xff, 0xff, 0xff, 0x1a),
        }
    } else {
        MenuColors {
            bg: Color32::WHITE,
            border: Color32::from_rgba_unmultiplied(0, 0, 0, 0x1f),
            text: Color32::from_rgb(0x1d, 0x1d, 0x1f),
            muted: Color32::from_rgba_unmultiplied(0x3c, 0x3c, 0x43, 0x99),
            hover: Color32::from_rgba_unmultiplied(0, 0, 0, 0x14),
        }
    }
}

/// A menu's box: the popover surface, a hairline border, the radius and the padding. No shadow:
/// nothing in the app has one.
pub(crate) fn menu_frame(dark: bool) -> egui::Frame {
    let colors = menu_colors(dark);
    egui::Frame::new()
        .fill(colors.bg)
        .corner_radius(MENU_RADIUS)
        .inner_margin(MENU_PAD)
        .stroke(Stroke::new(1.0, colors.border))
        .shadow(egui::epaint::Shadow::NONE)
}

/// A right click on `response` opens this menu, in the app's menu box (egui's own is the old look).
pub(crate) fn context_menu<R>(response: &egui::Response, add: impl FnOnce(&mut egui::Ui) -> R) -> Option<R> {
    let dark = response.ctx.global_style().visuals.dark_mode;
    egui::Popup::context_menu(response).frame(menu_frame(dark)).show(|ui| menu_body(ui, add)).map(|r| r.inner)
}

/// A click on `response` (a ⋯ button) opens this menu, in the app's menu box.
pub(crate) fn click_menu<R>(response: &egui::Response, add: impl FnOnce(&mut egui::Ui) -> R) -> Option<R> {
    let dark = response.ctx.global_style().visuals.dark_mode;
    egui::Popup::menu(response).frame(menu_frame(dark)).show(|ui| menu_body(ui, add)).map(|r| r.inner)
}

/// The rows touch, as in every menu of the app, and the menu is at least its minimum wide.
fn menu_body<R>(ui: &mut egui::Ui, add: impl FnOnce(&mut egui::Ui) -> R) -> R {
    ui.spacing_mut().item_spacing.y = 0.0;
    ui.set_min_width(MENU_MIN);
    add(ui)
}

/// A row of any menu in the app: its icon (13px) and label, the shortcut at the right, the text
/// color's 10% under the pointer. `ink` is the label's color (the foreground when none).
pub(crate) fn menu_row(ui: &mut egui::Ui, icon: Option<Icon>, label: &str, shortcut: &str, ink: Option<Color32>) -> bool {
    let colors = menu_colors(ui.visuals().dark_mode);
    let (rect, response) = ui.allocate_exact_size(Vec2::new(ui.available_width(), ROW_HEIGHT), Sense::click());
    let response = response.on_hover_cursor(CursorIcon::PointingHand);
    let p = ui.painter();
    if response.hovered() {
        p.rect_filled(rect, ITEM_RADIUS, colors.hover);
    }
    let text = ink.unwrap_or(colors.text);
    let mut x = rect.left() + ROW_PAD;
    if let Some(icon) = icon {
        let tint = if ink.is_some() || response.hovered() { text } else { colors.muted };
        paint_icon_scaled(p, icon, Pos2::new(x + ICON_SIZE / 2.0, rect.center().y), tint, ICON_SIZE / 16.0, 0.0);
        x += ICON_SIZE + ROW_GAP;
    }
    p.text(Pos2::new(x, rect.center().y), egui::Align2::LEFT_CENTER, label, FontId::proportional(MENU_TEXT_SIZE), text);
    if !shortcut.is_empty() {
        p.text(
            Pos2::new(rect.right() - ROW_PAD, rect.center().y),
            egui::Align2::RIGHT_CENTER,
            keys_label(shortcut),
            FontId::proportional(MENU_TEXT_SIZE),
            colors.muted,
        );
    }
    response.clicked()
}

/// A choice of a menu: the check in the icon's place when it is the current one; a choice not
/// `offered` is greyed and takes no click.
pub(crate) fn menu_choice(ui: &mut egui::Ui, on: bool, label: &str, offered: bool) -> bool {
    let colors = menu_colors(ui.visuals().dark_mode);
    let sense = if offered { Sense::click() } else { Sense::hover() };
    let (rect, response) = ui.allocate_exact_size(Vec2::new(ui.available_width(), ROW_HEIGHT), sense);
    let p = ui.painter();
    if offered && response.hovered() {
        p.rect_filled(rect, ITEM_RADIUS, colors.hover);
    }
    let text = if offered { colors.text } else { colors.muted.gamma_multiply(0.6) };
    let x = rect.left() + ROW_PAD;
    if on {
        paint_icon_scaled(p, Icon::Check, Pos2::new(x + ICON_SIZE / 2.0, rect.center().y), text, ICON_SIZE / 16.0, 0.0);
    }
    p.text(Pos2::new(x + ICON_SIZE + ROW_GAP, rect.center().y), egui::Align2::LEFT_CENTER, label, FontId::proportional(MENU_TEXT_SIZE), text);
    offered && response.on_hover_cursor(CursorIcon::PointingHand).clicked()
}

/// The icons the menus' rows show, by what they do.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) enum MenuIcon {
    Copy,
    CopyPlus,
    Paste,
    SelectAll,
    Pencil,
    Sliders,
    Reload,
    Moon,
    Defaults,
    Eye,
    EyeOff,
    Lock,
    LockOpen,
    Trash,
    Search,
    Server,
    Align,
    Grid,
    Group,
    Ungroup,
    Chevron,
    Check,
    Plus,
    Image,
    Folder,
    Palette,
    /// lucide's lock-keyhole: the note's content lock.
    KeyLock,
    /// A toolbar tool's own icon (the "Adicionar" submenu's rows).
    Tool(Tool),
    /// A chat's card: the context bar's Chat button.
    Chat,
}

/// The icon of a row: by its id, or by its label when the id doesn't tell (a submenu has none).
pub(crate) fn menu_icon(id: &str, label: &str) -> Option<MenuIcon> {
    Some(match id {
        "copy_sel" | "copy_el" | "copy_content" => MenuIcon::Copy,
        "dup" => MenuIcon::CopyPlus,
        "paste" | "paste_text" => MenuIcon::Paste,
        "select_all" | "select_all_text" => MenuIcon::SelectAll,
        "rename" => MenuIcon::Pencil,
        "edit" => MenuIcon::Sliders,
        "reload" => MenuIcon::Reload,
        "unload" => MenuIcon::Moon,
        "blur" => MenuIcon::EyeOff,
        "unblur" => MenuIcon::Eye,
        "lock" if label == "Desbloquear" => MenuIcon::LockOpen,
        "lock" => MenuIcon::Lock,
        "lock_content" if label.starts_with("Desbloquear") => MenuIcon::LockOpen,
        "lock_content" => MenuIcon::KeyLock,
        "show_folder" | "folder" => MenuIcon::Folder,
        "delete" | "clear_chat" => MenuIcon::Trash,
        "find" => MenuIcon::Search,
        "processes" => MenuIcon::Server,
        "tidy" => MenuIcon::Grid,
        "group" => MenuIcon::Group,
        "ungroup" => MenuIcon::Ungroup,
        "add_note" => MenuIcon::Tool(Tool::Sticky),
        "add_file" => MenuIcon::Image,
        "add_terminal" if label == "Terminal" => MenuIcon::Tool(Tool::Terminal),
        "add_filetree" => MenuIcon::Tool(Tool::FileTree),
        "add_ide" => MenuIcon::Tool(Tool::Ide),
        "add_portal" => MenuIcon::Tool(Tool::Portal),
        "add_text" => MenuIcon::Tool(Tool::Text),
        "add_chat" | "open_chat" => MenuIcon::Chat,
        _ => match label {
            "Padrões para Novos Elementos" => MenuIcon::Defaults,
            "Alinhar" => MenuIcon::Align,
            "Adicionar" => MenuIcon::Plus,
            "Cor" => MenuIcon::Palette,
            "Terminal" => MenuIcon::Tool(Tool::Terminal),
            "Chat" => MenuIcon::Chat,
            _ => return None,
        },
    })
}

/// Draws `icon` 13px wide around `c`: lucide's 24-unit drawings at 2 units of stroke.
pub(crate) fn paint_menu_icon(p: &Painter, icon: MenuIcon, c: Pos2, color: Color32) {
    paint_menu_icon_sized(p, icon, c, color, ICON_SIZE);
}

/// `paint_menu_icon` at another size (the card's footer button is 10).
pub(crate) fn paint_menu_icon_sized(p: &Painter, icon: MenuIcon, c: Pos2, color: Color32, size: f32) {
    let k = size / 24.0;
    let s = Stroke::new(2.0 * k, color);
    let at = |x: f32, y: f32| c + Vec2::new(x - 12.0, y - 12.0) * k;
    let line = |pts: &[(f32, f32)]| {
        p.add(Shape::line(pts.iter().map(|&(x, y)| at(x, y)).collect(), s));
    };
    let closed = |pts: &[(f32, f32)]| {
        p.add(Shape::closed_line(pts.iter().map(|&(x, y)| at(x, y)).collect(), s));
    };
    let rect = |x: f32, y: f32, w: f32, h: f32, r: f32| {
        p.rect_stroke(Rect::from_min_max(at(x, y), at(x + w, y + h)), r * k, s, StrokeKind::Middle);
    };
    let circle = |x: f32, y: f32, r: f32| {
        p.circle_stroke(at(x, y), r * k, s);
    };
    // degrees clockwise from east, as the screen's y axis runs down
    let arc = |cx: f32, cy: f32, r: f32, from: f32, to: f32| -> Vec<(f32, f32)> {
        let n = 24;
        (0..=n)
            .map(|i| {
                let a = (from + (to - from) * i as f32 / n as f32).to_radians();
                (cx + r * a.cos(), cy + r * a.sin())
            })
            .collect()
    };
    let corners = || {
        for pts in [
            [(5.0, 3.0), (3.5, 3.5), (3.0, 5.0)],
            [(19.0, 3.0), (20.5, 3.5), (21.0, 5.0)],
            [(21.0, 19.0), (20.5, 20.5), (19.0, 21.0)],
            [(5.0, 21.0), (3.5, 20.5), (3.0, 19.0)],
        ] {
            line(&pts);
        }
        for pts in [
            [(9.0, 3.0), (10.0, 3.0)],
            [(14.0, 3.0), (15.0, 3.0)],
            [(9.0, 21.0), (10.0, 21.0)],
            [(14.0, 21.0), (15.0, 21.0)],
            [(3.0, 9.0), (3.0, 10.0)],
            [(21.0, 9.0), (21.0, 10.0)],
            [(3.0, 14.0), (3.0, 15.0)],
            [(21.0, 14.0), (21.0, 15.0)],
        ] {
            line(&pts);
        }
    };
    let back_sheet = || line(&[(4.0, 16.0), (3.0, 16.0), (2.0, 15.0), (2.0, 3.0), (3.0, 2.0), (15.0, 2.0), (16.0, 3.0), (16.0, 4.0)]);
    let eye = || {
        let upper: Vec<(f32, f32)> = (0..=24)
            .map(|i| (12.0 - 10.0 * (i as f32 / 24.0 * std::f32::consts::PI).cos(), 12.0 - 6.5 * (i as f32 / 24.0 * std::f32::consts::PI).sin()))
            .collect();
        let lower: Vec<(f32, f32)> = upper.iter().map(|&(x, y)| (x, 24.0 - y)).collect();
        line(&upper);
        line(&lower);
    };
    match icon {
        MenuIcon::Plus => paint_icon_scaled(p, Icon::Plus, c, color, size / 16.0, 0.0),
        MenuIcon::Image => paint_icon_scaled(p, Icon::Image, c, color, size / 16.0, 0.0),
        MenuIcon::Folder => paint_icon_scaled(p, Icon::Open, c, color, size / 16.0, 0.0),
        MenuIcon::Palette => {
            // lucide's palette: the blob with its thumb notch, four paint dots
            let mut blob = arc(12.0, 12.0, 10.0, 90.0, 270.0);
            blob.extend((1..=12).map(|i| {
                let a = (270.0 + 90.0 * i as f32 / 12.0).to_radians();
                (12.0 + 10.0 * a.cos(), 11.0 + 9.0 * a.sin())
            }));
            blob.extend(arc(17.0, 11.0, 5.0, 0.0, 90.0));
            blob.extend([(14.75, 16.0), (13.6, 16.5), (13.1, 17.5), (13.35, 18.8), (13.65, 19.2), (13.9, 20.2), (13.5, 21.4), (12.25, 22.0), (12.0, 22.0)]);
            line(&blob);
            for (x, y) in [(13.5, 6.5), (17.5, 10.5), (6.5, 12.5), (8.5, 7.5)] {
                p.circle_filled(at(x, y), 1.5 * k, color);
            }
        }
        MenuIcon::KeyLock => {
            rect(3.0, 11.0, 18.0, 11.0, 2.0);
            line(&arc(12.0, 7.0, 5.0, 180.0, 360.0));
            line(&[(7.0, 7.0), (7.0, 11.0)]);
            line(&[(17.0, 7.0), (17.0, 11.0)]);
            circle(12.0, 16.0, 1.0);
        }
        MenuIcon::Tool(tool) => paint_icon_scaled(p, Icon::Tool(tool), c, color, size / 16.0, 0.0),
        MenuIcon::Chat => paint_icon_scaled(p, Icon::Agents, c, color, size / 16.0, 0.0),
        MenuIcon::Copy => {
            rect(8.0, 8.0, 14.0, 14.0, 2.0);
            back_sheet();
        }
        MenuIcon::CopyPlus => {
            rect(8.0, 8.0, 14.0, 14.0, 2.0);
            back_sheet();
            line(&[(15.0, 12.0), (15.0, 18.0)]);
            line(&[(12.0, 15.0), (18.0, 15.0)]);
        }
        MenuIcon::Paste => {
            rect(8.0, 2.0, 8.0, 4.0, 1.0);
            line(&[(8.0, 4.0), (6.0, 4.0), (4.0, 6.0), (4.0, 20.0), (6.0, 22.0), (10.0, 22.0)]);
            line(&[(16.0, 4.0), (18.0, 4.0), (20.0, 6.0), (20.0, 8.0)]);
            line(&[(11.0, 14.0), (21.0, 14.0)]);
            line(&[(17.0, 10.0), (21.0, 14.0), (17.0, 18.0)]);
        }
        MenuIcon::SelectAll => {
            corners();
            line(&[(7.0, 8.0), (15.0, 8.0)]);
            line(&[(7.0, 12.0), (17.0, 12.0)]);
            line(&[(7.0, 16.0), (13.0, 16.0)]);
        }
        MenuIcon::Pencil => {
            closed(&[(3.0, 21.0), (4.5, 16.0), (17.5, 3.0), (21.0, 6.5), (8.0, 19.5)]);
            line(&[(15.0, 5.0), (19.0, 9.0)]);
        }
        MenuIcon::Sliders => {
            for pts in [
                [(10.0, 5.0), (3.0, 5.0)],
                [(12.0, 19.0), (3.0, 19.0)],
                [(14.0, 3.0), (14.0, 7.0)],
                [(16.0, 17.0), (16.0, 21.0)],
                [(21.0, 12.0), (12.0, 12.0)],
                [(21.0, 19.0), (16.0, 19.0)],
                [(21.0, 5.0), (14.0, 5.0)],
                [(8.0, 10.0), (8.0, 14.0)],
                [(8.0, 12.0), (3.0, 12.0)],
            ] {
                line(&pts);
            }
        }
        MenuIcon::Reload => {
            let mut pts = arc(12.0, 12.0, 9.0, 0.0, 270.0);
            pts.extend([(14.5, 3.0), (18.74, 5.74), (21.0, 8.0)]);
            line(&pts);
            line(&[(21.0, 3.0), (21.0, 8.0), (16.0, 8.0)]);
        }
        MenuIcon::Moon => {
            let mut pts = arc(12.0, 12.0, 9.0, 3.1, 266.9);
            pts.push((11.91, 3.82));
            pts.extend(arc(16.98, 6.74, 6.0, 212.5, 57.7));
            pts.extend([(20.2, 12.1), (20.99, 12.49)]);
            line(&pts);
        }
        MenuIcon::Defaults => {
            line(&[(14.0, 17.0), (5.0, 17.0)]);
            line(&[(19.0, 7.0), (10.0, 7.0)]);
            circle(17.0, 17.0, 3.0);
            circle(7.0, 7.0, 3.0);
        }
        MenuIcon::Eye => {
            eye();
            circle(12.0, 12.0, 3.0);
        }
        MenuIcon::EyeOff => {
            eye();
            circle(12.0, 12.0, 3.0);
            line(&[(3.0, 3.0), (21.0, 21.0)]);
        }
        MenuIcon::Lock => corners(),
        MenuIcon::LockOpen => {
            rect(3.0, 11.0, 18.0, 11.0, 2.0);
            let mut pts = vec![(7.0, 11.0), (7.0, 7.0)];
            pts.extend(arc(11.95, 7.0, 4.95, 180.0, 280.0));
            line(&pts);
        }
        MenuIcon::Trash => {
            line(&[(3.0, 6.0), (21.0, 6.0)]);
            line(&[(19.0, 6.0), (19.0, 20.0), (18.0, 22.0), (6.0, 22.0), (5.0, 20.0), (5.0, 6.0)]);
            line(&[(8.0, 6.0), (8.0, 4.0), (9.0, 2.0), (15.0, 2.0), (16.0, 4.0), (16.0, 6.0)]);
            line(&[(10.0, 11.0), (10.0, 17.0)]);
            line(&[(14.0, 11.0), (14.0, 17.0)]);
        }
        MenuIcon::Search => {
            circle(11.0, 11.0, 8.0);
            line(&[(21.0, 21.0), (16.65, 16.65)]);
        }
        MenuIcon::Server => {
            rect(2.0, 2.0, 20.0, 8.0, 2.0);
            rect(2.0, 14.0, 20.0, 8.0, 2.0);
            line(&[(6.0, 6.0), (6.01, 6.0)]);
            line(&[(6.0, 18.0), (6.01, 18.0)]);
        }
        MenuIcon::Align => {
            line(&[(4.0, 2.0), (4.0, 22.0)]);
            rect(6.0, 6.0, 9.0, 6.0, 2.0);
            rect(6.0, 14.0, 13.0, 6.0, 2.0);
        }
        MenuIcon::Grid => {
            rect(3.0, 3.0, 7.0, 7.0, 1.0);
            rect(14.0, 3.0, 7.0, 7.0, 1.0);
            rect(14.0, 14.0, 7.0, 7.0, 1.0);
            rect(3.0, 14.0, 7.0, 7.0, 1.0);
        }
        MenuIcon::Group => {
            line(&[(3.0, 7.0), (3.0, 5.0), (5.0, 3.0), (7.0, 3.0)]);
            line(&[(17.0, 3.0), (19.0, 3.0), (21.0, 5.0), (21.0, 7.0)]);
            line(&[(21.0, 17.0), (21.0, 19.0), (19.0, 21.0), (17.0, 21.0)]);
            line(&[(7.0, 21.0), (5.0, 21.0), (3.0, 19.0), (3.0, 17.0)]);
            rect(7.0, 7.0, 7.0, 5.0, 1.0);
            rect(10.0, 12.0, 7.0, 5.0, 1.0);
        }
        MenuIcon::Ungroup => {
            rect(5.0, 4.0, 14.0, 6.0, 2.0);
            rect(5.0, 14.0, 14.0, 6.0, 2.0);
        }
        MenuIcon::Chevron => line(&[(9.0, 18.0), (15.0, 12.0), (9.0, 6.0)]),
        MenuIcon::Check => line(&[(20.0, 6.0), (9.0, 17.0), (4.0, 12.0)]),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_rows_are_the_reference_apps_size() {
        // text-sm's line (16.25) plus py-1.5 twice (9.75)
        assert_eq!(ROW_HEIGHT, 26.0);
        assert_eq!(SEP_HEIGHT, 7.5);
        assert_eq!(MENU_MIN, 10.0 * 13.0);
        assert_eq!(SUB_MIN, 8.0 * 13.0);
    }

    #[test]
    fn rows_get_their_icons_by_id_and_label() {
        assert_eq!(menu_icon("copy_sel", "Copiar"), Some(MenuIcon::Copy));
        assert_eq!(menu_icon("lock", "Desbloquear"), Some(MenuIcon::LockOpen));
        assert_eq!(menu_icon("lock", "Bloquear"), Some(MenuIcon::Lock));
        assert_eq!(menu_icon("", "Padrões para Novos Elementos"), Some(MenuIcon::Defaults));
        assert_eq!(menu_icon("color:3", "Azul"), None);
    }

    #[test]
    fn every_icon_draws() {
        let ctx = Context::default();
        ctx.run_ui(Default::default(), |ui| {
            let p = ui.painter().clone();
            for icon in [
                MenuIcon::Copy,
                MenuIcon::CopyPlus,
                MenuIcon::Paste,
                MenuIcon::SelectAll,
                MenuIcon::Pencil,
                MenuIcon::Sliders,
                MenuIcon::Reload,
                MenuIcon::Moon,
                MenuIcon::Defaults,
                MenuIcon::Eye,
                MenuIcon::EyeOff,
                MenuIcon::Lock,
                MenuIcon::LockOpen,
                MenuIcon::Trash,
                MenuIcon::Search,
                MenuIcon::Server,
                MenuIcon::Align,
                MenuIcon::Grid,
                MenuIcon::Group,
                MenuIcon::Ungroup,
                MenuIcon::Chevron,
                MenuIcon::Check,
            ] {
                paint_menu_icon(&p, icon, Pos2::new(20.0, 20.0), Color32::WHITE);
            }
        })
        .textures_delta
        .clear();
    }
}

#[cfg(test)]
mod choice_tests {
    use super::*;

    /// Two frames of a menu with one choice under a click at its middle: what the click picked.
    fn clicked(offered: bool) -> bool {
        let ctx = Context::default();
        let at = Pos2::new(60.0, ROW_HEIGHT / 2.0);
        let mut picked = false;
        for (k, events) in [vec![], vec![egui::Event::PointerMoved(at)], vec![button(at, true)], vec![button(at, false)]].into_iter().enumerate() {
            let screen = Rect::from_min_size(Pos2::ZERO, Vec2::new(400.0, 300.0));
            let raw = egui::RawInput { events, screen_rect: Some(screen), time: Some(k as f64 * 0.05), ..Default::default() };
            let mut out = ctx.run_ui(raw, |ui| {
                egui::Area::new(egui::Id::unique("choice")).fixed_pos(Pos2::ZERO).show(ui.ctx(), |ui| {
                    ui.set_width(200.0);
                    picked |= menu_choice(ui, true, "Automático", offered);
                });
            });
            out.textures_delta.clear();
        }
        picked
    }

    fn button(at: Pos2, pressed: bool) -> egui::Event {
        egui::Event::PointerButton { pos: at, button: egui::PointerButton::Primary, pressed, modifiers: Modifiers::NONE }
    }

    #[test]
    fn a_choice_takes_a_click_only_when_offered() {
        assert!(clicked(true));
        assert!(!clicked(false));
    }
}
