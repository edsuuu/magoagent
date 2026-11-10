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
    /// Where the Ajustes button sits in the bar.
    pub(crate) fn title_settings_button() -> Rect {
        Rect::from_center_size(Pos2::new(24.0, TITLE_BAR / 2.0), Vec2::splat(30.0))
    }

    /// Where the chat's button sits: beside Ajustes, or in its place while the open sidebar holds
    /// Ajustes at its foot.
    fn title_chat_button(&self) -> Rect {
        let first = Self::title_settings_button();
        if self.sidebar_width() > 0.0 { first } else { first.translate(Vec2::new(34.0, 0.0)) }
    }

    /// The bar over the canvas's top, see-through like the Mac's: the canvas runs under it and the
    /// toolbar sits in its middle. Two foreground areas, one each side of the toolbar (which keeps
    /// its clicks), so a portal's page leaves them uncovered like any floating UI.
    pub(crate) fn title_bar(&mut self, ctx: &Context) {
        let ink = if self.dark() { Color32::from_rgb(0xf5, 0xf5, 0xf7) } else { Color32::from_rgb(0x1d, 0x1d, 0x1f) };
        let hover = ink.gamma_multiply(0.1);
        let maximized = ctx.input(|i| i.viewport().maximized == Some(true));
        let menu_button = Self::title_settings_button();
        let chat_button = self.title_chat_button();
        let mut menu_clicked = false;
        // the chat beside Ajustes: in sight over the canvas and the chat alike (the agents table is ⌘J)
        let mut chat_clicked = false;
        let chat_open = self.chat_open;
        let width = ctx.content_rect().width();
        let toolbar = self.toolbar_span;
        let (left, right) =
            if toolbar.is_positive() && toolbar.top() < TITLE_BAR { (toolbar.left() - 4.0, toolbar.right() + 4.0) } else { (width / 2.0, width / 2.0) };
        // an empty stretch of the bar (its press is `caption_drag`'s)
        let side = |name: &str, from: f32, to: f32, add: &mut dyn FnMut(&mut egui::Ui, Rect)| {
            egui::Area::new(egui::Id::unique(name)).fixed_pos(Pos2::new(from, 0.0)).order(egui::Order::Foreground).show(ctx, |ui| {
                let (bar, _) = ui.allocate_exact_size(Vec2::new((to - from).max(0.0), TITLE_BAR), Sense::hover());
                add(ui, bar);
            });
        };
        side("title_bar", 0.0, left, &mut |ui, bar| {
            let p = ui.painter().clone();
            if chat_button != menu_button {
                let menu = ui.interact(menu_button, egui::Id::unique("title_menu"), Sense::click()).on_hover_text(keys_label("Ajustes — Ctrl+,"));
                if self.settings_open || menu.hovered() {
                    p.rect_filled(menu_button, 8, hover);
                }
                paint_icon(&p, Icon::Sliders, menu_button.center(), ink);
                menu_clicked = menu.clicked();
            }
            let rect = chat_button;
            let button = ui.interact(rect, egui::Id::unique("title_chat"), Sense::click()).on_hover_text(keys_label("Chat — ⌘1 (canvas: ⌘2)"));
            if chat_open || button.hovered() {
                p.rect_filled(rect, 8, if chat_open { ink.gamma_multiply(0.16) } else { hover });
            }
            paint_icon(&p, Icon::Composer, rect.center(), ink);
            chat_clicked = button.clicked();
            let right = rect.right();
            let title = Rect::from_min_max(Pos2::new(right + 10.0, bar.top()), Pos2::new(bar.right(), bar.bottom()));
            p.with_clip_rect(title).text(title.left_center(), egui::Align2::LEFT_CENTER, APP_TITLE, FontId::proportional(13.0), ink.gamma_multiply(0.75));
        });
        side("title_bar_right", right, width, &mut |ui, bar| {
            let p = ui.painter().clone();
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
        // on Windows the empty stretches are the window's own caption: the system moves it there
        // (and maximizes on a double-click, snaps at the screen's edges) even while a frame is slow
        #[cfg(windows)]
        {
            let stretch = |from: f32, to: f32| Rect::from_min_max(Pos2::new(from, EDGE), Pos2::new(to, TITLE_BAR));
            let bare = [stretch(chat_button.right() + 4.0, left), stretch(right, width - 3.0 * CAPTION_WIDTH)];
            native_caption::set(&bare, ctx.pixels_per_point());
        }
        if menu_clicked {
            self.settings_open = !self.settings_open;
        }
        if chat_clicked {
            self.chat_open = !self.chat_open;
        }
        // the edges show the resize arrows a native border would (the press is `edge_resize`'s)
        let edge = ctx.input(|i| i.pointer.hover_pos()).and_then(|p| resize_edge(ctx.content_rect(), p));
        if let (false, Some((_, cursor))) = (maximized, edge) {
            ctx.set_cursor_icon(cursor);
        }
    }
}

impl DrawingApp {
    /// A press on an empty stretch of the bar moves the window right away, as a native title bar
    /// does, and a second one soon after maximizes (or restores). Runs on the raw input like
    /// `edge_resize`: waiting for egui to see a drag, the move came too late while the first
    /// frames are slow (pages and shells starting) and never started.
    pub(crate) fn caption_drag(&mut self, ctx: &Context, raw: &mut egui::RawInput) {
        let (Some(screen), time) = (raw.screen_rect, raw.time.unwrap_or(0.0)) else { return };
        let maximized = raw.viewport().maximized == Some(true);
        let buttons = Self::title_settings_button().union(self.title_chat_button());
        let empty = |pos: Pos2| {
            let bar = [egui::Id::unique("title_bar"), egui::Id::unique("title_bar_right")];
            pos.y < TITLE_BAR
                && ctx.layer_id_at(pos).is_some_and(|l| bar.contains(&l.id))
                && !buttons.contains(pos)
                && pos.x < screen.right() - 3.0 * CAPTION_WIDTH
        };
        raw.events.retain(|e| {
            let egui::Event::PointerButton { pos, button: egui::PointerButton::Primary, pressed: true, .. } = e else { return true };
            if !empty(*pos) {
                return true;
            }
            let again = self.caption_press.is_some_and(|(t, p)| time - t < 0.5 && p.distance(*pos) < 6.0);
            self.caption_press = if again { None } else { Some((time, *pos)) };
            ctx.send_viewport_cmd(if again { egui::ViewportCommand::Maximized(!maximized) } else { egui::ViewportCommand::StartDrag });
            false
        });
    }
}

/// The window's caption on Windows: the empty stretches of the drawn bar answer the system's hit
/// test as its title bar, so moving the window never waits for a frame (a press there used to be
/// missed while the app was starting).
#[cfg(windows)]
pub(crate) mod native_caption {
    use crate::prelude::*;
    use std::sync::Mutex;

    /// The stretches, in the window's pixels.
    static BARE: Mutex<Vec<[i32; 4]>> = Mutex::new(Vec::new());

    #[repr(C)]
    struct Point {
        x: i32,
        y: i32,
    }

    type SubclassProc = unsafe extern "system" fn(isize, u32, usize, isize, usize, usize) -> isize;
    #[link(name = "comctl32")]
    unsafe extern "system" {
        fn SetWindowSubclass(hwnd: isize, proc: SubclassProc, id: usize, data: usize) -> i32;
        fn DefSubclassProc(hwnd: isize, msg: u32, wparam: usize, lparam: isize) -> isize;
    }
    #[link(name = "user32")]
    unsafe extern "system" {
        fn ScreenToClient(hwnd: isize, point: *mut Point) -> i32;
    }
    const WM_NCHITTEST: u32 = 0x84;
    const HTCLIENT: isize = 1;
    const HTCAPTION: isize = 2;

    pub(crate) fn set(bare: &[Rect], pixels_per_point: f32) {
        let px = |v: f32| (v * pixels_per_point).round() as i32;
        let rects = bare.iter().filter(|r| r.is_positive()).map(|r| [px(r.min.x), px(r.min.y), px(r.max.x), px(r.max.y)]).collect();
        *BARE.lock().unwrap() = rects;
    }

    /// Hooks the window once; true when it took.
    pub(crate) fn install(window: &impl wry::raw_window_handle::HasWindowHandle) -> bool {
        let Ok(handle) = window.window_handle() else { return false };
        let wry::raw_window_handle::RawWindowHandle::Win32(win) = handle.as_raw() else { return false };
        // SAFETY: the app's own window, hooked for its whole life; the procedure only reads BARE
        unsafe { SetWindowSubclass(win.hwnd.get(), hit_test, 1, 0) != 0 }
    }

    unsafe extern "system" fn hit_test(hwnd: isize, msg: u32, wparam: usize, lparam: isize, _: usize, _: usize) -> isize {
        // SAFETY: the system's own message, passed on unchanged
        let hit = unsafe { DefSubclassProc(hwnd, msg, wparam, lparam) };
        if msg != WM_NCHITTEST || hit != HTCLIENT {
            return hit;
        }
        let mut at = Point { x: (lparam & 0xffff) as i16 as i32, y: ((lparam >> 16) & 0xffff) as i16 as i32 };
        // SAFETY: a plain point on the stack
        unsafe { ScreenToClient(hwnd, &mut at) };
        let inside = |r: &[i32; 4]| at.x >= r[0] && at.x < r[2] && at.y >= r[1] && at.y < r[3];
        if BARE.lock().map(|b| b.iter().any(inside)).unwrap_or(false) { HTCAPTION } else { hit }
    }
    #[test]
    fn the_bare_stretches_go_to_the_system_in_pixels() {
        set(&[Rect::from_min_max(Pos2::new(80.0, 5.0), Pos2::new(400.0, 60.0)), Rect::NOTHING], 1.5);
        assert_eq!(*BARE.lock().unwrap(), vec![[120, 8, 600, 90]]);
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
