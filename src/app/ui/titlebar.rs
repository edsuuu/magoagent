//! The title bar the app draws off the Mac, where the window has no native frame: ☰, the name, and
//! minimize, maximize/restore and close. The empty bar moves the window, a double-click on it
//! maximizes, and the window's edges resize it.

use crate::prelude::*;

/// Windows' caption buttons are 46 wide; the close one turns this red under the pointer.
const CAPTION_WIDTH: f32 = 46.0;
const CLOSE_RED: Color32 = Color32::from_rgb(0xe8, 0x11, 0x23);

/// The band along the window's edge that resizes it, and how far a corner reaches along the sides.
const EDGE: f32 = 5.0;
const CORNER: f32 = 12.0;

impl DrawingApp {
    /// Where ☰ sits in the bar.
    pub(crate) fn title_menu_button() -> Rect {
        Rect::from_center_size(Pos2::new(24.0, TITLE_BAR / 2.0), Vec2::splat(30.0))
    }

    /// The bar over the canvas's top (the canvas starts under it): a foreground area, so a portal's
    /// page leaves it uncovered like any floating UI.
    pub(crate) fn title_bar(&mut self, ctx: &Context) {
        // the reference's bar: its background and its symbols
        let (fill, ink) = if self.dark() {
            (Color32::from_rgb(0x1a, 0x1a, 0x1c), Color32::from_rgb(0xf5, 0xf5, 0xf7))
        } else {
            (Color32::from_rgb(0xf6, 0xf6, 0xf6), Color32::from_rgb(0x1d, 0x1d, 0x1f))
        };
        let hover = ink.gamma_multiply(0.1);
        let maximized = ctx.input(|i| i.viewport().maximized == Some(true));
        let menu_button = Self::title_menu_button();
        let mut menu_clicked = false;
        egui::Area::new(egui::Id::unique("title_bar")).fixed_pos(Pos2::ZERO).order(egui::Order::Foreground).show(ctx, |ui| {
            // the empty bar first: the buttons added after it take their own clicks
            let (bar, empty) = ui.allocate_exact_size(Vec2::new(ctx.content_rect().width(), TITLE_BAR), Sense::click_and_drag());
            let p = ui.painter().clone();
            p.rect_filled(bar, 0, fill);
            if empty.double_clicked() {
                ctx.send_viewport_cmd(egui::ViewportCommand::Maximized(!maximized));
            } else if empty.drag_started_by(egui::PointerButton::Primary) {
                ctx.send_viewport_cmd(egui::ViewportCommand::StartDrag);
            }
            let menu = ui.interact(menu_button, egui::Id::unique("title_menu"), Sense::click()).on_hover_text("Menu");
            if self.menu_open || menu.hovered() {
                p.rect_filled(menu_button, 6, hover);
            }
            paint_icon(&p, Icon::Menu, menu_button.center(), ink);
            menu_clicked = menu.clicked();
            p.text(Pos2::new(menu_button.right() + 10.0, bar.center().y), egui::Align2::LEFT_CENTER, APP_TITLE, FontId::proportional(13.0), ink.gamma_multiply(0.75));
            // right to left: close, maximize / restore, minimize
            let names = ["Fechar", if maximized { "Restaurar" } else { "Maximizar" }, "Minimizar"];
            for (k, name) in names.into_iter().enumerate() {
                let rect = Rect::from_min_size(Pos2::new(bar.right() - CAPTION_WIDTH * (k + 1) as f32, 0.0), Vec2::new(CAPTION_WIDTH, TITLE_BAR));
                let button = ui.interact(rect, egui::Id::unique(("caption", k)), Sense::click()).on_hover_text(name);
                let close = k == 0;
                if button.hovered() {
                    p.rect_filled(rect, 0, if close { CLOSE_RED } else { hover });
                }
                caption_glyph(&p, k, maximized, rect.center(), if close && button.hovered() { Color32::WHITE } else { ink });
                if button.clicked() {
                    ctx.send_viewport_cmd(match k {
                        0 => egui::ViewportCommand::Close,
                        1 => egui::ViewportCommand::Maximized(!maximized),
                        _ => egui::ViewportCommand::Minimized(true),
                    });
                }
            }
        });
        if menu_clicked {
            self.toggle_menu();
        }
        if self.menu_open {
            self.main_menu(ctx, menu_button);
        }
        // the edges show the resize arrows a native border would (the press is `edge_resize`'s)
        let edge = ctx.input(|i| i.pointer.hover_pos()).and_then(|p| resize_edge(ctx.content_rect(), p));
        if let (false, Some((_, cursor))) = (maximized, edge) {
            ctx.set_cursor_icon(cursor);
        }
    }
}

/// The canvas starts under the drawn title bar: its room, before the canvas's panel.
pub(crate) fn title_bar_room(root: &mut egui::Ui) {
    if TITLE_BAR > 0.0 {
        egui::Panel::top("title_bar").exact_size(TITLE_BAR).resizable(false).show_separator_line(false).frame(egui::Frame::NONE).show(root, |_| ());
    }
}

/// Off the Mac the window has no border to grab: a press on its edge starts the system's resize
/// there and never reaches the app. Runs on the raw input, before any widget sees the press.
pub(crate) fn edge_resize(ctx: &Context, raw: &mut egui::RawInput) {
    let Some(screen) = raw.screen_rect.filter(|_| raw.viewport().maximized != Some(true)) else { return };
    raw.events.retain(|e| {
        let egui::Event::PointerButton { pos, button: egui::PointerButton::Primary, pressed: true, .. } = e else { return true };
        let Some((direction, _)) = resize_edge(screen, *pos) else { return true };
        ctx.send_viewport_cmd(egui::ViewportCommand::BeginResize(direction));
        false
    });
}

/// The window edge or corner under `pos`, with its resize cursor.
pub(crate) fn resize_edge(screen: Rect, pos: Pos2) -> Option<(egui::ResizeDirection, CursorIcon)> {
    use egui::ResizeDirection as R;
    let gaps = [pos.x - screen.left(), screen.right() - pos.x, pos.y - screen.top(), screen.bottom() - pos.y];
    if gaps.iter().any(|g| *g < 0.0) || gaps.iter().all(|g| *g >= EDGE) {
        return None;
    }
    // on the band, a side's corner reaches CORNER along it
    let [west, east, north, south] = gaps.map(|g| g < CORNER);
    Some(match (north, south, west, east) {
        (true, _, true, _) => (R::NorthWest, CursorIcon::ResizeNorthWest),
        (true, _, _, true) => (R::NorthEast, CursorIcon::ResizeNorthEast),
        (_, true, true, _) => (R::SouthWest, CursorIcon::ResizeSouthWest),
        (_, true, _, true) => (R::SouthEast, CursorIcon::ResizeSouthEast),
        (true, ..) => (R::North, CursorIcon::ResizeNorth),
        (_, true, ..) => (R::South, CursorIcon::ResizeSouth),
        (.., true, _) => (R::West, CursorIcon::ResizeWest),
        _ => (R::East, CursorIcon::ResizeEast),
    })
}

/// Windows' 10 px caption glyphs: k = 0 close, 1 maximize (restore when maximized), 2 minimize.
fn caption_glyph(p: &Painter, k: usize, maximized: bool, at: Pos2, color: Color32) {
    let stroke = Stroke::new(1.0, color);
    // on the pixel grid, so the 1 px lines stay sharp
    let at = at.round() + Vec2::splat(0.5);
    let square = |min: Vec2, size: f32| Rect::from_min_size(at + min, Vec2::splat(size));
    match (k, maximized) {
        (0, _) => {
            p.line_segment([at + Vec2::new(-5.0, -5.0), at + Vec2::new(5.0, 5.0)], stroke);
            p.line_segment([at + Vec2::new(-5.0, 5.0), at + Vec2::new(5.0, -5.0)], stroke);
        }
        (1, false) => {
            p.rect_stroke(square(Vec2::splat(-5.0), 10.0), 1, stroke, StrokeKind::Middle);
        }
        (1, true) => {
            // the window in front, and the corner of the one behind it
            p.rect_stroke(square(Vec2::new(-5.0, -3.0), 8.0), 1, stroke, StrokeKind::Middle);
            p.line(vec![at + Vec2::new(-3.0, -5.0), at + Vec2::new(5.0, -5.0), at + Vec2::new(5.0, 3.0)], stroke);
        }
        _ => {
            p.hline(at.x - 5.0..=at.x + 5.0, at.y, stroke);
        }
    }
}
