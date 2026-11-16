//! The island's window: a see-through one of its own over every other, as big as the island gets, put where the
//! island's centre stays; the person drags it anywhere (the system does), and it sticks to the top edge. With no
//! windows of its own (the tests, a backend without them) the island is drawn over the canvas.

use super::{NewForm, cursor, paint};
use crate::prelude::*;

/// How near the top edge a drop sticks to it.
const SNAP: f32 = 60.0;
/// Windows hides the top row of a window without a frame (`plain::strip`): the window starts a row above the screen.
const TOP_PAD: f32 = if cfg!(windows) { 1.0 } else { 0.0 };
/// The least room the window has for the island: Home, the questions and the forms fit in it.
const ROOM: Vec2 = Vec2::new(720.0, 420.0);

/// The room the window has for the island: a little more than the conversation `chat` (the spring that opens it
/// overshoots), and while an edge is pulled (`resizing`) the largest it can be pulled to (`most`), so that the window
/// changes when the pull starts and ends and never during it: the island is not cut as it grows, and a window that
/// followed the pointer would fight it.
pub(crate) fn window_cap(chat: Vec2, most: Vec2, resizing: bool) -> Vec2 {
    (if resizing { most } else { chat * 1.05 }).max(ROOM)
}

/// Whether an island hung from `anchor` is away from the top edge of the monitor (`work`): then it is a pill, not a notch.
pub(crate) fn floats(anchor: Option<[f32; 2]>, work: Rect) -> bool {
    anchor.is_some_and(|anchor| anchor[1] > work.top() + 2.0)
}

/// An even number of points at least `value`: the window and the island inside it then share one centre.
fn even(value: f32) -> f32 {
    (value / 2.0).ceil() * 2.0
}

/// The window around an island of `visible` size: exactly it (a row more on Windows, above the screen).
pub(crate) fn window_size(visible: Vec2) -> Vec2 {
    Vec2::new(even(visible.x), even(visible.y + TOP_PAD))
}

/// Where the window's top left goes for an island of `visible` size hanging from `anchor` (its centre x and the top
/// of the island): whatever its size, the centre stays.
pub(crate) fn window_origin(anchor: [f32; 2], visible: Vec2) -> Pos2 {
    Pos2::new((anchor[0] - window_size(visible).x / 2.0).round(), (anchor[1] - TOP_PAD).round())
}

/// The window's origin kept on the monitor of the anchor (`work`: the monitor less its taskbar), as the reference keeps
/// its desktop wizard (`clamp_origin`): near a side the window is pushed in, and the island in it with it, so that an
/// open island never spills onto the next monitor. A monitor narrower than the window leaves it centred.
pub(crate) fn kept_origin(anchor: [f32; 2], visible: Vec2, work: Rect) -> Pos2 {
    let origin = window_origin(anchor, visible);
    let width = window_size(visible).x;
    if work.width() < width {
        return origin;
    }
    Pos2::new(origin.x.clamp(work.left(), work.right() - width).round(), origin.y)
}

/// Where the island's centre is in its window: at the anchor, but never with the island past the window's sides (a
/// window pushed in near the monitor's side carries the open island in; the folded one stays where it hangs).
pub(crate) fn island_center_x(anchor: [f32; 2], origin: Pos2, window: Vec2, island: Vec2) -> f32 {
    let half = (island.x / 2.0).min(window.x / 2.0);
    (anchor[0] - origin.x).clamp(half, window.x - half)
}

/// The anchor of a window as it is on the screen.
pub(crate) fn anchor_of(window: Rect) -> [f32; 2] {
    [window.center().x, window.top() + TOP_PAD]
}

/// An anchor kept on the screen (`work`: the monitor less its taskbar): a place saved on a monitor that is gone.
pub(crate) fn keep_on_screen(anchor: [f32; 2], work: Rect) -> [f32; 2] {
    [anchor[0].clamp(work.left() + 60.0, (work.right() - 60.0).max(work.left() + 60.0)), anchor[1].clamp(work.top(), (work.bottom() - 80.0).max(work.top()))]
}

/// Let go near the top edge of the monitor, the island sticks to it.
pub(crate) fn snap_top(anchor: [f32; 2], work: Rect) -> [f32; 2] {
    if anchor[1] - work.top() < SNAP { [anchor[0], work.top()] } else { anchor }
}

/// Whether the island's window can let the clicks through all of it but the island: Windows, and Linux on X11.
fn clicks_through_part() -> bool {
    #[cfg(target_os = "linux")]
    return crate::app::island::x11::active();
    #[cfg(not(target_os = "linux"))]
    cfg!(windows)
}

impl DrawingApp {
    /// Where the island hangs from: as it was left, kept on a monitor that is there; the first time, the top centre of
    /// the screen the app is on.
    pub(super) fn island_anchor(&mut self, ctx: &Context) -> [f32; 2] {
        if let Some(anchor) = self.island.anchor {
            return anchor;
        }
        let work_of = |at: Pos2| crate::app::chat::mini::screen_work(ctx, at);
        let anchor = match self.prefs.island_at {
            Some(saved) => keep_on_screen(saved, work_of(Pos2::new(saved[0], saved[1]))),
            None => {
                let app = ctx.input(|input| input.viewport().outer_rect).unwrap_or(Rect::from_min_size(Pos2::ZERO, Vec2::new(1280.0, 800.0)));
                let work = work_of(app.center());
                [work.center().x.round(), work.top()]
            }
        };
        self.island.anchor = Some(anchor);
        anchor
    }

    /// The island in a window of its own, over every other: dragged by the system, out of the app's too.
    pub(super) fn island_window(
        &mut self,
        ctx: &Context,
        view: &paint::View,
        form: &mut Option<NewForm>,
        look: &crate::app::chat::list::Look,
        colors: &UiColors,
        time: f64,
    ) -> paint::Out {
        let anchor = self.island_anchor(ctx);
        // Windows and X11 let the clicks through all of the window but the island: the window keeps its room and is not
        // resized as the island springs. Elsewhere (Wayland, macOS) a window cannot let them through part of it: there it
        // is as big as the island is now, so no see-through room around the island swallows the clicks meant for what is
        // under it (while an edge is pulled it keeps the room it has, so that it does not fight the pull)
        let clicks_through_part = clicks_through_part();
        let visible = if !clicks_through_part && self.island.anim != Vec2::ZERO && !self.island.resizing {
            self.island.anim.ceil()
        } else if self.island.cap == Vec2::ZERO {
            ROOM
        } else {
            self.island.cap
        };
        // the window is put where the island's centre stays only when its size changes: a builder that followed the
        // window would fight the system's drag
        if self.island.built != visible {
            self.island.built = visible;
            let work = crate::app::chat::mini::screen_work(ctx, Pos2::new(anchor[0], anchor[1] + 1.0));
            self.island.origin = kept_origin(anchor, visible, work);
        }
        if crate::app::chat::mini::fresh(ctx, &mut self.island.pass) {
            self.island.plain = 0;
        }
        let (origin, size) = (self.island.origin, window_size(visible));
        let builder = egui::ViewportBuilder::default()
            .with_title("Ilha do Maestro")
            // the main window's class: the island of the app in development is not the installed app's for the dock
            .with_app_id(crate::window_names().1)
            .with_transparent(true)
            .with_decorations(false)
            .with_always_on_top()
            .with_taskbar(false)
            // X11: a utility window, out of Alt+Tab and the dock (`with_taskbar` is only Windows'), that still takes the keys
            .with_window_type(egui::X11WindowType::Utility)
            .with_resizable(false)
            .with_active(false)
            // shown once the system's frame is off it (`plain::reveal`): no glass square for a frame
            .with_visible(!cfg!(windows) || self.island.plain > 0)
            .with_position(origin)
            .with_inner_size(size);
        // the eyes follow the cursor over the whole screen
        let scale = ctx.input(|input| input.viewport().native_pixels_per_point).unwrap_or(1.0);
        let center_x = island_center_x(anchor, origin, size, self.island.anim);
        let shown = Rect::from_min_size(Pos2::new(center_x - self.island.anim.x / 2.0, TOP_PAD), self.island.anim);
        self.island.pointer = cursor::eye_cursor().and_then(|at| cursor::eye_target(at, scale, origin));
        // outside the island the click goes through to what is under it; on it (a margin around it too), or while a press
        // is held on it, the window takes it (Windows: the cursor is read on the whole screen)
        let cursor = cursor::screen_cursor().map(|at| Pos2::new(at.x / scale - origin.x, at.y / scale - origin.y));
        let holding = self.island.holding || self.island.resizing;
        let capture = cursor::takes_click(cursor::screen_cursor, scale, origin, shown, holding);
        self.island.over = cursor.map(|at| cursor::captures(Some(at), shown, false));
        #[cfg(windows)]
        {
            let room = self.island.room.get_or_insert_with(|| cursor::watch(ctx));
            let pixels = Rect::from_min_max((shown.min + origin.to_vec2()) * scale, (shown.max + origin.to_vec2()) * scale);
            *room.lock().unwrap_or_else(|poisoned| poisoned.into_inner()) = Some(pixels.expand(cursor::HIT_MARGIN * scale));
        }
        let send_capture = self.island.captured != Some(capture);
        self.island.captured = Some(capture);
        let id = egui::ViewportId::from_hash_of("island_window");
        let (anim, radius) = (self.island.anim, self.island.radius);
        let (out, placed) = ctx.show_viewport_immediate(id, builder, |ui, _| {
            let at = Pos2::new((ui.max_rect().left() + center_x - anim.x / 2.0).round(), ui.max_rect().top() + TOP_PAD);
            let out = self.island_draw(ui, at, anim, radius, view, form, look, colors, time);
            // Windows: `plain::click_through`, below; X11: the input shape, below
            if send_capture && !clicks_through_part {
                ui.ctx().send_viewport_cmd(egui::ViewportCommand::MousePassthrough(!capture));
            }
            if out.drag {
                ui.ctx().send_viewport_cmd(egui::ViewportCommand::StartDrag);
            }
            (out, ui.ctx().input(|input| input.viewport().outer_rect))
        });
        // X11: only the island (a margin around it) takes the clicks; all of the window while a press is held on it
        #[cfg(target_os = "linux")]
        if clicks_through_part {
            let area = if holding { Rect::from_min_size(Pos2::ZERO, size) } else { shown.expand(cursor::HIT_MARGIN) };
            crate::app::island::x11::input_shape("Ilha do Maestro", area, scale);
        }
        #[cfg(windows)]
        if self.island.plain < 3 {
            crate::app::voice::popup::plain::reveal("Ilha do Maestro", &mut self.island.plain);
            ctx.request_repaint();
        }
        // asked again: the thread of `cursor::watch` takes the click-through off the moment the cursor comes in, and the
        // answer from before the frame would put it back for as long as the frame takes
        #[cfg(windows)]
        crate::app::voice::popup::plain::click_through("Ilha do Maestro", !cursor::takes_click(cursor::screen_cursor, scale, origin, shown, holding));
        self.island_moved(ctx, placed, size, time);
        out
    }

    /// The person moved the window (it is where it was not put, at the size it was made for): once it stays put, that
    /// is where the island hangs from, stuck to the top edge when it is near it.
    fn island_moved(&mut self, ctx: &Context, placed: Option<Rect>, size: Vec2, time: f64) {
        let origin = self.island.origin;
        let Some(placed) = placed.filter(|rect| (rect.size() - size).abs().max_elem() < 1.5 && (rect.min - origin).abs().max_elem() > 2.0) else {
            self.island.moved = None;
            return;
        };
        match self.island.moved {
            Some((seen, since)) if seen == placed.min => {
                if time - since > 0.3 {
                    // the island moved with its window: the anchor keeps its place in it (off centre when it was pushed in)
                    let old = self.island.anchor.unwrap_or(anchor_of(Rect::from_min_size(origin, size)));
                    let moved = [placed.left() + old[0] - origin.x, placed.top() + TOP_PAD];
                    let work = crate::app::chat::mini::screen_work(ctx, Pos2::new(moved[0], moved[1] + 1.0));
                    let anchor = snap_top(keep_on_screen(moved, work), work);
                    self.island.anchor = Some(anchor);
                    self.island.origin = kept_origin(anchor, self.island.built, work);
                    self.prefs.island_at = Some(anchor);
                    self.island.moved = None;
                }
            }
            _ => self.island.moved = Some((placed.min, time)),
        }
        ctx.request_repaint_after(std::time::Duration::from_millis(100));
    }

    /// With no windows of its own (the tests, a backend without them): over the canvas, at the top centre of the app.
    pub(super) fn island_area(
        &mut self,
        ctx: &Context,
        view: &paint::View,
        form: &mut Option<NewForm>,
        look: &crate::app::chat::list::Look,
        colors: &UiColors,
        time: f64,
    ) -> paint::Out {
        let (anim, radius) = (self.island.anim, self.island.radius);
        let content = ctx.content_rect();
        let at = Pos2::new((content.center().x - anim.x / 2.0).round(), content.top());
        let mut out = paint::Out::default();
        egui::Area::new(egui::Id::unique("island_area")).order(egui::Order::Foreground).fixed_pos(at).constrain(false).show(ctx, |ui| {
            out = self.island_draw(ui, at, anim, radius, view, form, look, colors, time);
        });
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_window_keeps_the_islands_centre_when_it_opens_and_closes_and_hangs_from_the_top() {
        let anchor = [960.0, 0.0];
        let (closed, open) = (Vec2::new(288.0, 32.0), Vec2::new(640.0, 160.0));
        for size in [closed, open] {
            let (origin, window) = (window_origin(anchor, size), window_size(size));
            assert_eq!(origin.x + window.x / 2.0, 960.0, "{size:?}");
            assert_eq!(origin.y, -TOP_PAD, "no gap above the island");
            assert!(window.x >= size.x && window.y >= size.y + TOP_PAD);
        }
        assert_eq!(anchor_of(Rect::from_min_size(window_origin(anchor, open), window_size(open))), anchor, "read back, the same anchor");
    }

    #[test]
    fn near_the_side_of_its_monitor_the_window_is_pushed_in_and_the_open_island_never_reaches_the_next_one() {
        // two monitors side by side; the island hangs 220 from the right side of the first
        let work = Rect::from_min_max(Pos2::new(0.0, 0.0), Pos2::new(1920.0, 1040.0));
        let anchor = [1700.0, 0.0];
        let room = Vec2::new(720.0, 420.0);
        let origin = kept_origin(anchor, room, work);
        let window = window_size(room);
        assert_eq!(origin.x + window.x, 1920.0, "the window ends at the monitor's side");
        let (open, folded) = (Vec2::new(640.0, 160.0), Vec2::new(288.0, 32.0));
        let open_right = origin.x + island_center_x(anchor, origin, window, open) + open.x / 2.0;
        assert!(open_right <= 1920.0, "the open island stays on its monitor: {open_right}");
        assert_eq!(origin.x + island_center_x(anchor, origin, window, folded), 1700.0, "the folded one hangs where it was left");
        // in the middle nothing moves
        let middle = kept_origin([960.0, 0.0], room, work);
        assert_eq!(middle, window_origin([960.0, 0.0], room));
        assert_eq!(middle.x + island_center_x([960.0, 0.0], middle, window, open), 960.0);
    }

    #[test]
    fn a_saved_place_on_a_monitor_that_is_gone_comes_back_and_a_drop_near_the_top_sticks() {
        let work = Rect::from_min_max(Pos2::new(0.0, 40.0), Pos2::new(1920.0, 1040.0));
        assert_eq!(keep_on_screen([960.0, 300.0], work), [960.0, 300.0], "where it is, it stays");
        assert_eq!(keep_on_screen([5000.0, -300.0], work), [1860.0, 40.0]);
        assert_eq!(keep_on_screen([-40.0, 9000.0], work), [60.0, 960.0]);
        assert_eq!(snap_top([700.0, 75.0], work), [700.0, 40.0], "near the top edge: on it");
        assert_eq!(snap_top([700.0, 140.0], work), [700.0, 140.0], "farther: where it was let go");
        assert!(!floats(Some([700.0, 40.0]), work) && !floats(None, work), "on the top edge: a notch");
        assert!(floats(Some([700.0, 300.0]), work), "away from it: a pill with four round corners");
    }

    #[test]
    fn the_window_has_room_for_the_conversation_and_for_all_it_can_be_pulled_to() {
        let most = Vec2::new(1824.0, 988.0);
        assert_eq!(window_cap(Vec2::new(640.0, 360.0), most, false), Vec2::new(720.0, 420.0), "small: the least room");
        let big = window_cap(Vec2::new(1344.0, 728.0), most, false);
        assert!(big.x > 1344.0 && big.y > 728.0, "the big conversation and its overshoot fit: {big:?}");
        assert_eq!(window_cap(Vec2::new(640.0, 360.0), most, true), most, "an edge pulled: the largest it can get, from the start");
    }
}
