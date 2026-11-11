//! The canvas's top-left buttons (☰, the chat's, the space in use), and the title bar the app
//! draws off the Mac, where the window has no native frame: those buttons, and minimize,
//! maximize/restore and close on a ground of their own. The empty bar moves the window, a
//! double-click on it maximizes, and the window's edges resize it.

use crate::prelude::*;

/// Windows' caption buttons are 46 wide; the close one turns this red under the pointer.
const CAPTION_WIDTH: f32 = 46.0;
/// How tall they are: their block is the reference app's window overlay, a tone apart from the canvas.
const CAPTION_HEIGHT: f32 = 38.0;
const CLOSE_RED: Color32 = Color32::from_rgb(0xe8, 0x11, 0x23);

/// The band along the window's edge that resizes it, and how far a corner reaches along the sides.
const EDGE: f32 = 5.0;
const CORNER: f32 = 12.0;

/// The top-left buttons: a 32.5 round glass around a 26 button, 6.5 apart, their top on the toolbar's.
const TOP_BUTTON: f32 = 32.5;
const TOP_GAP: f32 = 6.5;
const TOP_ROW_Y: f32 = if cfg!(target_os = "macos") { LIGHTS_Y } else { TOP_ROW + TOP_BUTTON / 2.0 };

impl DrawingApp {
    /// Where a top-left button sits, 12 past the sidebar: 0 is ☰ (Ajustes over the chat, which
    /// has no sidebar), 1 the chat's.
    pub(crate) fn top_button(&self, k: usize) -> Rect {
        let left = (self.sidebar_width() + 12.0).max(MENU_LEFT) + k as f32 * (TOP_BUTTON + TOP_GAP);
        Rect::from_min_size(Pos2::new(left, TOP_ROW_Y - TOP_BUTTON / 2.0), Vec2::splat(TOP_BUTTON))
    }

    /// The canvas's top-left buttons, as in the reference app: ☰ folds and opens the sidebar, the
    /// chat's beside it and, with the sidebar folded, the space in use by name (a click opens the
    /// sidebar, where Ajustes is). Gives where they end.
    pub(crate) fn top_buttons(&mut self, ui: &mut egui::Ui) -> f32 {
        let (dark, chat) = (self.dark(), self.chat_open);
        let ink = glass_ink(dark);
        let p = ui.painter().clone();
        let round = |ui: &mut egui::Ui, rect: Rect, id: &str, icon: Icon, on: bool, tip: &str| {
            let button = ui.interact(rect, egui::Id::unique(id), Sense::click()).on_hover_cursor(CursorIcon::PointingHand).tip(keys_label(tip));
            paint_glass(&p, rect, TOP_BUTTON / 2.0, dark, false);
            if button.hovered() {
                p.circle_filled(rect.center(), GLASS_BUTTON / 2.0, ink.gamma_multiply(0.1));
            }
            paint_icon_scaled(&p, icon, rect.center(), if on { glass_accent(dark) } else { ink.gamma_multiply(0.85) }, GLASS_ICON, 0.0);
            button.clicked()
        };
        let (first, second) = (self.top_button(0), self.top_button(1));
        let folded = self.prefs.sidebar_hidden;
        if chat {
            if round(ui, first, "top_settings", Icon::Gear, self.settings_open, "Ajustes — ⌘,") {
                self.settings_open = !self.settings_open;
            }
        } else if round(ui, first, "top_menu", Icon::Menu, false, if folded { "Abrir barra lateral" } else { "Fechar barra lateral" }) {
            self.prefs.sidebar_hidden = !folded;
        }
        if round(ui, second, "top_chat", Icon::Composer, chat, if chat { "Voltar ao canvas — ⌘2" } else { "Abrir o chat — ⌘1" }) {
            self.chat_open = !chat;
        }
        let space = self.spaces.list.iter().find(|w| w.id == self.spaces.current).filter(|_| folded && !chat);
        let Some(space) = space else { return second.right() };
        // up to the toolbar, which shares the row
        let toolbar = self.toolbar_span;
        let limit = if toolbar.is_positive() { toolbar.left() } else { ui.ctx().content_rect().width() / 2.0 } - 12.0;
        let name = p.layout_no_wrap(space.name.clone(), FontId::proportional(14.0), ink.gamma_multiply(0.9));
        let left = second.right() + TOP_GAP;
        // 11 in from each end, a 12 folder, 6 to the name
        let pill = Rect::from_min_max(Pos2::new(left, TOP_ROW_Y - 16.0), Pos2::new((left + 11.0 + 18.0 + name.size().x + 11.0).min(limit), TOP_ROW_Y + 16.0));
        if pill.width() < 60.0 {
            return second.right();
        }
        let button = ui.interact(pill, egui::Id::unique("top_space"), Sense::click()).on_hover_cursor(CursorIcon::PointingHand).tip("Abrir barra lateral");
        paint_glass(&p, pill, 16.0, dark, false);
        if button.hovered() {
            p.rect_filled(pill, 16, ink.gamma_multiply(0.06));
        }
        let [r, g, b] = space.color;
        paint_icon_scaled(&p, Icon::Open, Pos2::new(pill.left() + 17.0, pill.center().y), Color32::from_rgb(r, g, b), 0.75, 0.0);
        let text = Pos2::new(pill.left() + 29.0, pill.center().y - name.size().y / 2.0);
        p.with_clip_rect(pill.shrink2(Vec2::new(11.0, 0.0))).galley(text, name, ink);
        if button.clicked() {
            self.prefs.sidebar_hidden = false;
        }
        pill.right()
    }

    /// The bar over the canvas's top, see-through like the Mac's: the canvas runs under it and the
    /// toolbar sits in its middle. Two foreground areas, one each side of the toolbar (which keeps
    /// its clicks), so a portal's page leaves them uncovered like any floating UI. It starts past
    /// the sidebar, which runs the window's whole height.
    pub(crate) fn title_bar(&mut self, ctx: &Context) {
        let ink = glass_ink(self.dark());
        let hover = ink.gamma_multiply(0.1);
        // the caption's ground, the reference app's: darker than the canvas in the dark, grayer in the light
        let ground = if self.dark() { Color32::from_rgb(0x1a, 0x1a, 0x1c) } else { Color32::from_rgb(0xf6, 0xf6, 0xf6) };
        let maximized = ctx.input(|i| i.viewport().maximized == Some(true));
        let width = ctx.content_rect().width();
        let toolbar = self.toolbar_span;
        let (left, right) =
            if toolbar.is_positive() && toolbar.top() < TITLE_BAR { (toolbar.left() - 4.0, toolbar.right() + 4.0) } else { (width / 2.0, width / 2.0) };
        let from = self.sidebar_width().min(left);
        // an empty stretch of the bar (its press is `caption_drag`'s)
        let side = |name: &str, from: f32, to: f32, add: &mut dyn FnMut(&mut egui::Ui, Rect)| {
            egui::Area::new(egui::Id::unique(name)).fixed_pos(Pos2::new(from, 0.0)).order(egui::Order::Foreground).show(ctx, |ui| {
                let (bar, _) = ui.allocate_exact_size(Vec2::new((to - from).max(0.0), TITLE_BAR), Sense::hover());
                add(ui, bar);
            });
        };
        let first = self.top_button(0);
        side("title_bar", from, left, &mut |ui, _| {
            let end = self.top_buttons(ui);
            self.title_buttons = Rect::from_min_max(first.min, Pos2::new(end, first.bottom()));
        });
        side("title_bar_right", right, width, &mut |ui, bar| {
            let p = ui.painter().clone();
            p.rect_filled(Rect::from_min_max(Pos2::new(bar.right() - 3.0 * CAPTION_WIDTH, 0.0), Pos2::new(bar.right(), CAPTION_HEIGHT)), 0, ground);
            // right to left: close, maximize / restore, minimize
            let names = ["Fechar", if maximized { "Restaurar" } else { "Maximizar" }, "Minimizar"];
            for (k, name) in names.into_iter().enumerate() {
                let rect = Rect::from_min_size(Pos2::new(bar.right() - CAPTION_WIDTH * (k + 1) as f32, 0.0), Vec2::new(CAPTION_WIDTH, CAPTION_HEIGHT));
                let button = ui.interact(rect, egui::Id::unique(("caption", k)), Sense::click()).tip(name);
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
            let bare = [stretch(self.title_buttons.right() + 4.0, left), stretch(right, width - 3.0 * CAPTION_WIDTH)];
            native_caption::set(&bare, ctx.pixels_per_point());
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
        let buttons = self.title_buttons;
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
