//! The island's window: a see-through one of its own over every other, as big as the island gets, put where the
//! island hangs; the person drags it anywhere (the system does), and let go near an edge of its monitor it sticks to
//! it (`Dock`): the top (a notch hanging down), the bottom (it grows up), a side (folded it stands up, open it unfolds
//! from the side). Ajustes › Geral › Lugar da ilha puts it there too. With no windows of its own (the tests, a backend
//! without them) the island is drawn over the canvas.

use super::{NewForm, cursor, paint};
use crate::prelude::*;

/// How near an edge a drop sticks to it.
/// The island window's title: the system finds the window by it (the input shape, the click-through, the reveal).
pub(super) const TITLE: &str = "Ilha da Luna";

const SNAP: f32 = 60.0;
/// How long the moved window stays still before it counts as let go: where it is, and a while longer when sticking to
/// an edge moves it (`settle_for`).
const SETTLE: f64 = 0.3;
const SNAP_SETTLE: f64 = 1.0;

/// How long a window moved to `placed` waits still before the island hangs from there, its window put at `target`:
/// longer when that moves it. The system moves the window while the button is held, and a pause in the middle of the
/// drag looks the same as a drop: moved then, the window fights the system's move and the drag stops under the cursor.
pub(crate) fn settle_for(target: Pos2, placed: Pos2) -> f64 {
    if (target - placed).abs().max_elem() > 1.0 { SNAP_SETTLE } else { SETTLE }
}
/// X11: how long the drag waits at most for the window to be where it was put.
const DRAG_WAIT: f64 = 0.3;

/// X11: whether the drag asked at `since` starts now, at `time`: once the system has the window (`placed`, last frame) at
/// the drag size and where it was put (`origin`, `size`), or after `DRAG_WAIT` whatever it has. The system resizes and
/// moves the window in two steps, and a drag begun between them took the window where it was: the island, at the left
/// of the big window's room, jumped off to the left of the cursor (asked: "eu tento mover às vezes e ela salta para a
/// esquerda").
pub(crate) fn drag_ready(placed: Option<Rect>, origin: Pos2, size: Vec2, since: f64, time: f64) -> bool {
    let there = placed.is_some_and(|rect| (rect.min - origin).abs().max_elem() < 1.5 && (rect.size() - size).abs().max_elem() < 1.5);
    there || time - since > DRAG_WAIT
}

/// X11: the window at the island's size since a drag began (`slim`) stays so while the island moves and until, let go,
/// it grows past it (it opens): then the window takes its room back. In its room the system stops the window at the top
/// bar with the island still below it, and the island went up by jumps, one each time it was let go.
pub(crate) fn slim_kept(slim: Option<Vec2>, island: Vec2, moving: bool) -> Option<Vec2> {
    slim.filter(|size| moving || (island.x <= size.x + 1.0 && island.y <= size.y + 1.0))
}

/// Windows hides the top row of a window without a frame (`plain::strip`): the window starts a row above the island.
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

/// An even number of points at least `value`: the window and the island inside it then share one centre.
fn even(value: f32) -> f32 {
    (value / 2.0).ceil() * 2.0
}

/// The window around an island of `visible` size: exactly it (a row more on Windows, above the island).
pub(crate) fn window_size(visible: Vec2) -> Vec2 {
    Vec2::new(even(visible.x), even(visible.y + TOP_PAD))
}

/// Where the island is on its monitor: on an edge, or away from them all (`Free`, a pill with four round corners).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub(crate) enum Dock {
    #[default]
    Top,
    Bottom,
    Left,
    Right,
    Free,
}

impl Dock {
    /// Where an island hanging from `anchor` is on the monitor `work` (the monitor less its taskbar): on a side when
    /// the anchor is its first or last column, on the top or the bottom when it is that row, else free.
    pub(crate) fn of(anchor: [f32; 2], work: Rect) -> Dock {
        let at = |value: f32, edge: f32| (value - edge).abs() < 0.5;
        if at(anchor[0], work.left()) {
            Dock::Left
        } else if at(anchor[0], work.right() - 1.0) {
            Dock::Right
        } else if anchor[1] <= work.top() + 2.0 {
            Dock::Top
        } else if at(anchor[1], work.bottom() - 1.0) {
            Dock::Bottom
        } else {
            Dock::Free
        }
    }

    /// On a side: folded, the island stands up.
    pub(crate) fn upright(self) -> bool {
        matches!(self, Dock::Left | Dock::Right)
    }
}

/// A point of the monitor an anchor is on (the last row and column are the bottom's and the right side's anchors).
pub(crate) fn probe(anchor: [f32; 2]) -> Pos2 {
    Pos2::new(anchor[0], anchor[1] + 0.5)
}

/// The island of `size` where it hangs from `anchor` (screen points): from its top centre on the top edge and away from
/// the edges, from its bottom centre on the bottom edge, from the middle of its side on a side.
pub(crate) fn hung(anchor: [f32; 2], dock: Dock, size: Vec2) -> Rect {
    let min = match dock {
        Dock::Top | Dock::Free => Pos2::new(anchor[0] - size.x / 2.0, anchor[1]),
        Dock::Bottom => Pos2::new(anchor[0] - size.x / 2.0, anchor[1] + 1.0 - size.y),
        Dock::Left => Pos2::new(anchor[0], anchor[1] - size.y / 2.0),
        Dock::Right => Pos2::new(anchor[0] + 1.0 - size.x, anchor[1] - size.y / 2.0),
    };
    Rect::from_min_size(min, size)
}

/// `rect` moved into `room` along each axis it fits in; along one it does not fit in, centred on it (`centre`) or left.
fn kept_in(rect: Rect, room: Rect, centre: bool) -> Rect {
    let axis = |min: f32, length: f32, low: f32, high: f32| match high - low < length {
        true if centre => low + (high - low - length) / 2.0,
        true => min,
        false => min.clamp(low, high - length),
    };
    let min = Pos2::new(axis(rect.min.x, rect.width(), room.left(), room.right()), axis(rect.min.y, rect.height(), room.top(), room.bottom()));
    Rect::from_min_size(min, rect.size())
}

/// Where the window's top left goes for an island with `visible` room hanging from `anchor`, kept on the monitor of the
/// anchor (`work`), as the reference keeps its desktop wizard (`clamp_origin`): near an edge the window is pushed in, and
/// the island in it with it, so that an open island never spills onto the next monitor nor under the taskbar. A
/// monitor smaller than the window leaves it where it hangs.
pub(crate) fn kept_origin(anchor: [f32; 2], visible: Vec2, work: Rect) -> Pos2 {
    let size = window_size(visible);
    // the rows under the window's hidden top one (Windows) are the island's
    let room = kept_in(hung(anchor, Dock::of(anchor, work), Vec2::new(size.x, size.y - TOP_PAD)), work, false);
    Pos2::new(room.min.x.round(), (room.min.y - TOP_PAD).round())
}

/// Where the island of `island` size is in its window (at `origin`, `window` big): where it hangs from the anchor, but
/// never past the window's edges (a window pushed in near the monitor's edge carries the open island in; the folded one
/// stays where it hangs).
pub(crate) fn island_in(anchor: [f32; 2], dock: Dock, origin: Pos2, window: Vec2, island: Vec2) -> Rect {
    let rect = hung(anchor, dock, island).translate(-origin.to_vec2());
    kept_in(rect, Rect::from_min_max(Pos2::new(0.0, TOP_PAD), window.to_pos2()), true)
}

/// An anchor kept on the screen (`work`): a place saved on a monitor that is gone. One stuck to an edge stays on it.
pub(crate) fn keep_on_screen(anchor: [f32; 2], work: Rect) -> [f32; 2] {
    let x = anchor[0].clamp(work.left() + 60.0, (work.right() - 60.0).max(work.left() + 60.0));
    match Dock::of(anchor, work) {
        Dock::Left | Dock::Right => [anchor[0], anchor[1].clamp(work.top(), work.bottom() - 1.0)],
        Dock::Top => [x, work.top()],
        Dock::Bottom => [x, anchor[1]],
        Dock::Free => [x, anchor[1].clamp(work.top(), (work.bottom() - 80.0).max(work.top()))],
    }
}

/// An anchor on the top edge of a screen with a camera notch (`notch`, screen points) goes to the notch's middle: the
/// island grows out of it there, and anywhere else on that edge the notch would cut it.
pub(crate) fn to_notch(anchor: [f32; 2], work: Rect, notch: Option<Rect>) -> [f32; 2] {
    match notch {
        Some(notch) if Dock::of(anchor, work) == Dock::Top => [notch.center().x, work.top()],
        _ => anchor,
    }
}

/// `to_notch` on the screen the anchor is on.
fn on_notch(ctx: &Context, anchor: [f32; 2]) -> [f32; 2] {
    let at = probe(anchor);
    to_notch(anchor, crate::app::chat::mini::screen_work(ctx, at), super::notch_at(at))
}

/// Where an island let go at `island` (screen points) hangs from: on the edge of the monitor (`work`) it is near, a
/// side first (in a corner it stands on the side), then the top or the bottom; away from them, where it is.
pub(crate) fn dropped(island: Rect, work: Rect) -> [f32; 2] {
    let center = island.center().round();
    if island.left() - work.left() < SNAP {
        [work.left(), center.y]
    } else if work.right() - island.right() < SNAP {
        [work.right() - 1.0, center.y]
    } else if island.top() - work.top() < SNAP {
        [center.x, work.top()]
    } else if work.bottom() - island.bottom() < SNAP {
        [center.x, work.bottom() - 1.0]
    } else {
        [center.x, island.top().round()]
    }
}

/// The island let go with its window's top at `window_top`: the system keeps a window's top on the screen, so a window
/// pushed against the top (an island lower in it, as on a side, cannot go higher) took the island to the top edge.
pub(crate) fn pushed_to_top(island: Rect, window_top: f32, work: Rect) -> Rect {
    if window_top <= work.top() + 1.0 { island.translate(Vec2::new(0.0, work.top() - island.top())) } else { island }
}

/// Where Ajustes › Lugar da ilha puts it on the monitor (`work`): the middle of the top or of the bottom, or a side at
/// the height it is at (the side's middle when it was on the top or the bottom).
pub(crate) fn place_anchor(place: Place, anchor: [f32; 2], work: Rect) -> [f32; 2] {
    let y = match Dock::of(anchor, work) {
        Dock::Top | Dock::Bottom => work.center().y.round(),
        _ => anchor[1],
    };
    match place {
        Place::Top => [work.center().x.round(), work.top()],
        Place::Bottom => [work.center().x.round(), work.bottom() - 1.0],
        Place::Left => [work.left(), y],
        Place::Right => [work.right() - 1.0, y],
    }
}

/// The places Ajustes › Lugar da ilha offers.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) enum Place {
    Top,
    Bottom,
    Left,
    Right,
}

impl Place {
    pub(crate) const ALL: [Place; 4] = [Place::Top, Place::Bottom, Place::Left, Place::Right];
    pub(crate) const NAMES: [&str; 4] = ["Topo", "Embaixo", "Esquerda", "Direita"];

    /// The place an anchor is in, when it is one of them.
    pub(crate) fn of(anchor: [f32; 2], work: Rect) -> Option<Place> {
        match Dock::of(anchor, work) {
            Dock::Top => Some(Place::Top),
            Dock::Bottom => Some(Place::Bottom),
            Dock::Left => Some(Place::Left),
            Dock::Right => Some(Place::Right),
            Dock::Free => None,
        }
    }
}

/// Where the menus open in the island's window are (popups: `Order::Foreground`), all in one rectangle; none when no
/// menu is open.
pub(crate) fn menus(ctx: &Context) -> Option<Rect> {
    ctx.memory(|memory| {
        let layers = memory.areas().visible_layer_ids();
        layers.into_iter().filter(|layer| layer.order == egui::Order::Foreground).filter_map(|layer| memory.area_rect(layer.id)).reduce(Rect::union)
    })
}

/// Whether the island's window can let the clicks through all of it but the island: Windows, macOS, and Linux on X11.
fn clicks_through_part() -> bool {
    #[cfg(target_os = "linux")]
    return crate::app::island::x11::active();
    #[cfg(not(target_os = "linux"))]
    cfg!(any(windows, target_os = "macos"))
}

impl DrawingApp {
    /// Where the island hangs from: as it was left, kept on a monitor that is there; the first time, the top centre of
    /// the screen the app is on.
    pub(super) fn island_anchor(&mut self, ctx: &Context) -> [f32; 2] {
        if let Some(place) = self.island.place_ask.take() {
            let anchor = self.island_anchor(ctx);
            let work = crate::app::chat::mini::screen_work(ctx, probe(anchor));
            let anchor = on_notch(ctx, place_anchor(place, anchor, work));
            (self.island.anchor, self.prefs.island_at) = (Some(anchor), Some(anchor));
            // the window goes where the anchor now is
            self.island.built = Vec2::ZERO;
            return anchor;
        }
        if let Some(anchor) = self.island.anchor {
            return anchor;
        }
        let work_of = |at: Pos2| crate::app::chat::mini::screen_work(ctx, at);
        let anchor = on_notch(
            ctx,
            match self.prefs.island_at {
                Some(saved) => keep_on_screen(saved, work_of(probe(saved))),
                None => {
                    let app = ctx.input(|input| input.viewport().outer_rect).unwrap_or(Rect::from_min_size(Pos2::ZERO, Vec2::new(1280.0, 800.0)));
                    let work = work_of(app.center());
                    [work.center().x.round(), work.top()]
                }
            },
        );
        self.island.anchor = Some(anchor);
        anchor
    }

    /// Where the island is now, when it is on one of the places Ajustes offers.
    pub(crate) fn island_place(&self, ctx: &Context) -> Option<Place> {
        let anchor = self.island.anchor.or(self.prefs.island_at)?;
        Place::of(anchor, crate::app::chat::mini::screen_work(ctx, probe(anchor)))
    }

    /// Puts the island on the top edge or on a side (Ajustes, the MCP's `set_setting island_place`), on its next frame.
    pub(crate) fn put_island(&mut self, place: Place) {
        self.island.place_ask = Some(place);
    }

    /// Back to where it starts the first time, whatever was saved (Ajustes › Trazer a ilha de volta, the MCP's
    /// `island_place reset`): the middle of the top of the monitor the app's window is on, its window put there again.
    pub(crate) fn reset_island(&mut self) {
        self.prefs.island_at = None;
        self.island.anchor = None;
        self.island.slim = None;
        self.island.moved = None;
        self.island.built = Vec2::ZERO;
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
        // Windows, macOS and X11 let the clicks through all of the window but the island: the window keeps its room and is
        // not resized as the island springs. Elsewhere (Wayland) a window cannot let them through part of it: there it
        // is as big as the island is now, so no see-through room around the island swallows the clicks meant for what is
        // under it (while an edge is pulled it keeps the room it has, so that it does not fight the pull)
        let clicks_through_part = clicks_through_part();
        let moving = self.island.moved.is_some() || self.island.drag_next;
        self.island.slim = slim_kept(self.island.slim, self.island.anim, moving);
        let visible = if let Some(size) = self.island.slim {
            size
        } else if !clicks_through_part && self.island.anim != Vec2::ZERO && !self.island.resizing {
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
            let work = crate::app::chat::mini::screen_work(ctx, probe(anchor));
            self.island.origin = kept_origin(anchor, visible, work);
        }
        if crate::app::chat::mini::fresh(ctx, &mut self.island.pass) {
            self.island.plain = 0;
        }
        let (origin, size) = (self.island.origin, window_size(visible));
        let builder = egui::ViewportBuilder::default()
            .with_title(TITLE)
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
        #[cfg(target_os = "macos")]
        cursor::set_scale(scale);
        let shown = island_in(anchor, self.island.dock, origin, size, self.island.anim);
        self.island.pointer = cursor::eye_cursor().and_then(|at| cursor::eye_target(at, scale, origin));
        // outside the island the click goes through to what is under it; on it (a margin around it too), or while a press
        // is held on it, the window takes it (Windows: the cursor is read on the whole screen)
        let cursor = cursor::screen_cursor().map(|at| Pos2::new(at.x / scale - origin.x, at.y / scale - origin.y));
        let holding = self.island.holding || self.island.resizing;
        // a menu open from the island (the agents of a Maestro, a chip's choices) takes the clicks too, past its edge
        let hit = self.island.menus.map_or(shown, |menus| shown.union(menus));
        let capture = cursor::takes_click(cursor::screen_cursor, scale, origin, hit, holding);
        self.island.over = cursor.map(|at| cursor::captures(Some(at), shown, false));
        #[cfg(any(windows, target_os = "macos"))]
        {
            let room = self.island.room.get_or_insert_with(|| cursor::watch(ctx));
            let pixels = Rect::from_min_max((hit.min + origin.to_vec2()) * scale, (hit.max + origin.to_vec2()) * scale);
            *room.lock().unwrap_or_else(|poisoned| poisoned.into_inner()) = Some(pixels.expand(cursor::HIT_MARGIN * scale));
        }
        let send_capture = self.island.captured != Some(capture);
        self.island.captured = Some(capture);
        // X11: the drag starts once the window is the island's size (the frame after the press moved), so the system
        // takes the island to the top bar and not the room around it
        let slims = cfg!(target_os = "linux") && clicks_through_part && !self.island.resizing;
        // a press let go before the window was ready: no drag
        if self.island.drag_next && !self.island.holding {
            (self.island.drag_next, self.island.drag_since) = (false, None);
        }
        let drag_now = self.island.drag_next && drag_ready(self.island.placed, origin, size, self.island.drag_since.unwrap_or(time), time);
        if drag_now {
            (self.island.drag_next, self.island.drag_since) = (false, None);
        } else if self.island.drag_next {
            ctx.request_repaint();
        }
        let drag_later = slims && self.island.slim.is_none();
        let id = egui::ViewportId::from_hash_of("island_window");
        let (anim, radius) = (self.island.anim, self.island.radius);
        let out = ctx.show_viewport_immediate(id, builder, |ui, _| {
            let at = (ui.max_rect().min + shown.min.to_vec2()).round();
            let out = self.island_draw(ui, at, anim, radius, view, form, look, colors, time);
            // Windows: `plain::click_through`, below; X11: the input shape, below
            if send_capture && !clicks_through_part {
                ui.ctx().send_viewport_cmd(egui::ViewportCommand::MousePassthrough(!capture));
            }
            if (out.drag && !drag_later) || drag_now {
                ui.ctx().send_viewport_cmd(egui::ViewportCommand::StartDrag);
            }
            let focused = ui.ctx().input(|input| input.focused);
            (paint::Out { focused, ..out }, ui.ctx().input(|input| input.viewport().outer_rect), menus(ui.ctx()))
        });
        let (out, placed, menus) = out;
        self.island.menus = menus;
        if out.drag && drag_later {
            self.island.slim = Some(self.island.anim.ceil());
            (self.island.drag_next, self.island.drag_since) = (true, Some(time));
            ctx.request_repaint();
        }
        self.island.placed = placed;
        // X11: only the island (a margin around it) takes the clicks; all of the window while a press is held on it
        #[cfg(target_os = "linux")]
        if clicks_through_part {
            let area = if holding { Rect::from_min_size(Pos2::ZERO, size) } else { hit.expand(cursor::HIT_MARGIN) };
            crate::app::island::x11::input_shape(TITLE, area, scale);
        }
        #[cfg(windows)]
        if self.island.plain < 3 {
            crate::app::voice::popup::plain::reveal(TITLE, &mut self.island.plain);
            ctx.request_repaint();
        }
        // asked again: the thread of `cursor::watch` takes the click-through off the moment the cursor comes in, and the
        // answer from before the frame would put it back for as long as the frame takes
        #[cfg(windows)]
        crate::app::voice::popup::plain::click_through(TITLE, !cursor::takes_click(cursor::screen_cursor, scale, origin, hit, holding));
        // macOS: the window is set up once it is there (again when it is made anew), and lets the clicks through but on
        // the island, asked with the cursor of now as on Windows
        #[cfg(target_os = "macos")]
        {
            // made under the menu bar's level, the system had pushed it below the bar (y 65 on a notch): put where it goes
            // again, and that push is not the person moving it (it was kept as where the island hangs)
            if self.island.plain == 0 && super::macos::strip(TITLE) {
                self.island.plain = 3;
                self.island.moved = None;
                ctx.send_viewport_cmd_to(id, egui::ViewportCommand::OuterPosition(origin));
            }
            super::macos::click_through(TITLE, !cursor::takes_click(cursor::screen_cursor, scale, origin, hit, holding));
        }
        self.island_moved(ctx, placed, size, time);
        out
    }

    /// The person moved the window (it is where it was not put, at the size it was made for): once it stays put, that
    /// is where the island hangs from, stuck to the edge of the monitor it is near.
    fn island_moved(&mut self, ctx: &Context, placed: Option<Rect>, size: Vec2, time: f64) {
        let origin = self.island.origin;
        // waiting for the drag, the window is between its old place and size and the new ones: not the person moving it
        let waiting = self.island.drag_next;
        let Some(placed) = placed.filter(|rect| !waiting && (rect.size() - size).abs().max_elem() < 1.5 && (rect.min - origin).abs().max_elem() > 2.0) else {
            self.island.moved = None;
            return;
        };
        match self.island.moved {
            Some((seen, since, grab)) if seen == placed.min => {
                // the island moved with its window, where it is in it (pushed in near an edge, or against the edge it
                // was stuck to): it hangs from where it was let go, on the edge it is near
                let old = self.island.anchor.unwrap_or([origin.x + size.x / 2.0, origin.y + TOP_PAD]);
                let island = island_in(old, self.island.dock, origin, size, self.island.anim).translate(placed.min.to_vec2());
                // the monitor the cursor let it go on: by the island's centre, one dropped by the edge between two
                // monitors went to the other one (asked: "na lateral que tem outro monitor, ela não gruda")
                let work = crate::app::chat::mini::screen_work(ctx, grab.unwrap_or(island.center()));
                let island = pushed_to_top(island, placed.top(), work);
                let anchor = keep_on_screen(dropped(island, work), work);
                let anchor = to_notch(anchor, work, super::notch_at(probe(anchor)));
                let target = kept_origin(anchor, self.island.built, work);
                if time - since > settle_for(target, placed.min) {
                    self.island.anchor = Some(anchor);
                    self.island.origin = target;
                    self.prefs.island_at = Some(anchor);
                    self.island.moved = None;
                }
            }
            _ => {
                let scale = ctx.input(|input| input.viewport().native_pixels_per_point).unwrap_or(1.0);
                self.island.moved = Some((placed.min, time, cursor::screen_cursor().map(|at| at / scale)));
            }
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
        // drawn in the app's window: that window's focus
        out.focused = ctx.input(|input| input.focused);
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Asked: "eu tento mover às vezes e ela salta para a esquerda". The drag waits for the window to be where and as big
    /// as it was put: resized but not yet moved (at the big window's left), it would carry the island off to the left.
    #[test]
    fn the_drag_waits_for_the_window_to_be_where_it_was_put() {
        let (origin, size) = (Pos2::new(866.0, 0.0), Vec2::new(188.0, 32.0));
        let big = Rect::from_min_size(Pos2::new(580.0, 0.0), Vec2::new(760.0, 420.0));
        let resized_only = Rect::from_min_size(big.min, size);
        let there = Rect::from_min_size(origin, size);
        assert!(!drag_ready(Some(big), origin, size, 1.0, 1.02), "still the big window");
        assert!(!drag_ready(Some(resized_only), origin, size, 1.0, 1.05), "resized, not moved yet: at the left");
        assert!(!drag_ready(None, origin, size, 1.0, 1.05), "not known yet");
        assert!(drag_ready(Some(there), origin, size, 1.0, 1.05), "where it was put: the drag starts");
        assert!(drag_ready(Some(resized_only), origin, size, 1.0, 1.0 + DRAG_WAIT + 0.01), "the system never moved it: it starts anyway");
    }

    #[test]
    fn an_open_menu_takes_the_clicks_along_with_the_island() {
        let ctx = Context::default();
        let frame = |open: bool| {
            let mut found = None;
            let mut output = ctx.run_ui(egui::RawInput::default(), |ui| {
                if open {
                    let area = egui::Area::new(egui::Id::unique("island_test_menu")).order(egui::Order::Foreground).fixed_pos(Pos2::new(100.0, 300.0));
                    area.show(ui.ctx(), |ui| ui.allocate_exact_size(Vec2::new(120.0, 80.0), Sense::hover()));
                }
                // a tooltip takes no click
                egui::Area::new(egui::Id::unique("island_test_tip"))
                    .order(egui::Order::Tooltip)
                    .fixed_pos(Pos2::new(0.0, 0.0))
                    .show(ui.ctx(), |ui| ui.label("dica"));
                found = menus(ui.ctx());
            });
            output.textures_delta.clear();
            found
        };
        assert_eq!(frame(false), None, "no menu: only the island");
        let menu = frame(true).expect("the menu open");
        assert_eq!(menu.min, Pos2::new(100.0, 300.0));
        assert!(menu.width() >= 120.0 && menu.height() >= 80.0, "{menu:?}");
    }

    #[test]
    fn the_window_keeps_the_islands_centre_when_it_opens_and_closes_and_hangs_from_the_top() {
        let work = Rect::from_min_max(Pos2::ZERO, Pos2::new(1920.0, 1040.0));
        let anchor = [960.0, 0.0];
        assert_eq!(Dock::of(anchor, work), Dock::Top);
        for size in [Vec2::new(288.0, 32.0), Vec2::new(640.0, 160.0)] {
            let (origin, window) = (kept_origin(anchor, size, work), window_size(size));
            assert_eq!(origin.x + window.x / 2.0, 960.0, "{size:?}");
            assert_eq!(origin.y, -TOP_PAD, "no gap above the island");
            assert!(window.x >= size.x && window.y >= size.y + TOP_PAD);
            let island = island_in(anchor, Dock::Top, origin, window, size).translate(origin.to_vec2());
            assert_eq!((island.center().x, island.top()), (960.0, 0.0), "hung from its top centre: {size:?}");
        }
    }

    #[test]
    fn near_the_side_of_its_monitor_the_window_is_pushed_in_and_the_open_island_never_reaches_the_next_one() {
        // two monitors side by side; the island hangs 220 from the right side of the first
        let work = Rect::from_min_max(Pos2::new(0.0, 0.0), Pos2::new(1920.0, 1040.0));
        let anchor = [1700.0, 0.0];
        let room = Vec2::new(720.0, 420.0);
        let (origin, window) = (kept_origin(anchor, room, work), window_size(room));
        assert_eq!(origin.x + window.x, 1920.0, "the window ends at the monitor's side");
        let (open, folded) = (Vec2::new(640.0, 160.0), Vec2::new(288.0, 32.0));
        let island = |size| island_in(anchor, Dock::Top, origin, window, size).translate(origin.to_vec2());
        assert!(island(open).right() <= 1920.0, "the open island stays on its monitor: {:?}", island(open));
        assert_eq!(island(folded).center().x, 1700.0, "the folded one hangs where it was left");
        // in the middle nothing moves
        let middle = kept_origin([960.0, 0.0], room, work);
        assert_eq!(middle.x + window.x / 2.0, 960.0);
        assert_eq!(island_in([960.0, 0.0], Dock::Top, middle, window, open).center().x + middle.x, 960.0);
    }

    #[test]
    fn a_saved_place_on_a_monitor_that_is_gone_comes_back_and_a_drop_near_the_top_sticks() {
        let work = Rect::from_min_max(Pos2::new(0.0, 40.0), Pos2::new(1920.0, 1040.0));
        assert_eq!(keep_on_screen([960.0, 300.0], work), [960.0, 300.0], "where it is, it stays");
        assert_eq!(keep_on_screen([5000.0, -300.0], work), [1860.0, 40.0]);
        assert_eq!(keep_on_screen([-40.0, 9000.0], work), [60.0, 960.0]);
        let at = |x: f32, y: f32| Rect::from_min_size(Pos2::new(x - 40.0, y), Vec2::new(80.0, 24.0));
        assert_eq!(dropped(at(700.0, 75.0), work), [700.0, 40.0], "near the top edge: on it");
        assert_eq!(dropped(at(700.0, 140.0), work), [700.0, 140.0], "farther: where it was let go");
        // from a side, the island is low in its tall window: the window stops at the top of the screen, and it docks there
        let standing = Rect::from_min_size(Pos2::new(900.0, 165.0), Vec2::new(40.0, 128.0));
        assert_eq!(dropped(pushed_to_top(standing, 0.0, work), work), [920.0, 40.0]);
        assert_eq!(pushed_to_top(standing, 200.0, work), standing, "let go lower down: where it was");
        assert_eq!(Dock::of([700.0, 40.0], work), Dock::Top, "on the top edge: a notch");
        assert_eq!(Dock::of([700.0, 300.0], work), Dock::Free, "away from it: a pill with four round corners");
    }

    #[test]
    fn let_go_near_a_side_the_island_sticks_to_it_stands_up_and_its_window_and_island_go_against_it() {
        let work = Rect::from_min_max(Pos2::new(72.0, 32.0), Pos2::new(1920.0, 1080.0));
        // the compact island 288 wide, its right edge 30 from the monitor's right side
        let compact = |left: f32| Rect::from_min_size(Pos2::new(left, 480.0), Vec2::new(288.0, 40.0));
        let right = dropped(compact(1920.0 - 30.0 - 288.0), work);
        assert_eq!(right, [1919.0, 500.0], "on the right side, at the height it was let go");
        assert_eq!(Dock::of(right, work), Dock::Right);
        assert!(Dock::Right.upright() && Dock::Left.upright() && !Dock::Top.upright() && !Dock::Bottom.upright());
        assert_eq!(dropped(compact(72.0 + 20.0), work), [72.0, 500.0], "on the left side (past the dock)");
        assert_eq!(dropped(compact(800.0), work), [944.0, 480.0], "in the middle: where it was let go");
        let corner = Rect::from_min_size(Pos2::new(1900.0 - 288.0, 40.0), Vec2::new(288.0, 40.0));
        assert_eq!(dropped(corner, work), [1919.0, 60.0], "in a corner: on the side");
        assert_eq!(keep_on_screen(right, work), right, "kept on its side");
        // the window and the island in it against the side, standing up and open, in the middle of where it hangs
        let room = Vec2::new(720.0, 420.0);
        let window = window_size(room);
        for island in [Vec2::new(40.0, 128.0), Vec2::new(640.0, 160.0)] {
            let origin = kept_origin(right, room, work);
            assert_eq!(origin.x + window.x, 1920.0);
            let shown = island_in(right, Dock::Right, origin, window, island).translate(origin.to_vec2());
            assert_eq!((shown.right(), shown.center().y), (1920.0, 500.0), "flush: {island:?}");
            let left = [72.0, 500.0];
            let origin = kept_origin(left, room, work);
            let shown = island_in(left, Dock::Left, origin, window, island).translate(origin.to_vec2());
            assert_eq!((shown.left(), shown.center().y), (72.0, 500.0), "flush: {island:?}");
        }
        // high on a side, the open island comes down to stay on the monitor
        let high = [72.0, 40.0];
        let origin = kept_origin(high, room, work);
        assert_eq!(origin.y, 32.0 - TOP_PAD);
        let shown = island_in(high, Dock::Left, origin, window, Vec2::new(640.0, 300.0)).translate(origin.to_vec2());
        assert!(shown.top() >= 32.0, "{shown:?}");
    }

    #[test]
    fn on_the_bottom_edge_the_island_grows_up_and_away_from_the_edges_it_stays_on_the_monitor() {
        let work = Rect::from_min_max(Pos2::new(0.0, 32.0), Pos2::new(1920.0, 1020.0));
        // let go over the dock: on the bottom edge of the work area, above it
        let near = Rect::from_min_size(Pos2::new(900.0, 1000.0), Vec2::new(80.0, 24.0));
        let bottom = dropped(near, work);
        assert_eq!(bottom, [940.0, 1019.0]);
        assert_eq!((Dock::of(bottom, work), keep_on_screen(bottom, work)), (Dock::Bottom, bottom));
        let room = Vec2::new(720.0, 420.0);
        let (origin, window) = (kept_origin(bottom, room, work), window_size(room));
        assert_eq!(origin.y + window.y, 1020.0, "the window ends on the work area's bottom");
        for island in [Vec2::new(80.0, 24.0), Vec2::new(640.0, 160.0)] {
            let shown = island_in(bottom, Dock::Bottom, origin, window, island).translate(origin.to_vec2());
            assert_eq!((shown.bottom(), shown.center().x), (1020.0, 940.0), "on the edge, growing up: {island:?}");
        }
        // away from the edges near the bottom: the window and the open island are pushed up onto the monitor
        let free = keep_on_screen([960.0, 1000.0], work);
        assert_eq!((free, Dock::of(free, work)), ([960.0, 940.0], Dock::Free));
        let origin = kept_origin(free, room, work);
        assert_eq!(origin.y + window.y, 1020.0);
        let open = island_in(free, Dock::Free, origin, window, Vec2::new(640.0, 160.0)).translate(origin.to_vec2());
        assert_eq!(open.bottom(), 1020.0, "pushed up: {open:?}");
        let folded = island_in(free, Dock::Free, origin, window, Vec2::new(80.0, 24.0)).translate(origin.to_vec2());
        assert_eq!(folded.top(), 940.0, "the folded one where it was left");
    }

    #[test]
    fn ajustes_puts_the_island_on_an_edge_and_reads_where_it_is() {
        let work = Rect::from_min_max(Pos2::new(0.0, 32.0), Pos2::new(1920.0, 1080.0));
        let top = [960.0, 32.0];
        assert_eq!(Place::of(top, work), Some(Place::Top));
        let left = place_anchor(Place::Left, top, work);
        assert_eq!(left, [0.0, 556.0], "from the top: the middle of the side");
        assert_eq!(Place::of(left, work), Some(Place::Left));
        assert_eq!(place_anchor(Place::Right, [500.0, 600.0], work), [1919.0, 600.0], "floating: at its height");
        let bottom = place_anchor(Place::Bottom, left, work);
        assert_eq!((bottom, Place::of(bottom, work)), ([960.0, 1079.0], Some(Place::Bottom)));
        assert_eq!(place_anchor(Place::Top, left, work), top);
        assert_eq!(Place::of([500.0, 600.0], work), None, "let go elsewhere: none of them");
        assert_eq!(Place::ALL.len(), Place::NAMES.len());
    }

    #[test]
    fn the_island_is_square_on_the_edge_it_is_on() {
        use super::super::paint::corners;
        let corners = |dock| {
            let radius = corners(14, dock);
            (radius.nw, radius.ne, radius.sw, radius.se)
        };
        assert_eq!(corners(Dock::Top), (0, 0, 14, 14));
        assert_eq!(corners(Dock::Free), (14, 14, 14, 14));
        assert_eq!(corners(Dock::Bottom), (14, 14, 0, 0));
        assert_eq!(corners(Dock::Left), (0, 14, 0, 14));
        assert_eq!(corners(Dock::Right), (14, 0, 14, 0));
    }

    #[test]
    fn the_conversation_is_pulled_by_the_edges_away_from_the_screens_and_grows_from_where_it_hangs() {
        use super::super::paint::{edge_zones, pulled_to};
        let rect = Rect::from_min_size(Pos2::new(100.0, 200.0), Vec2::new(640.0, 360.0));
        let touches = |dock, x: f32| edge_zones(rect, dock, 8.0).iter().any(|zone| zone.0.x_range().contains(x) && zone.2);
        assert!(touches(Dock::Top, rect.left()) && touches(Dock::Top, rect.right() - 1.0), "both sides");
        assert!(!touches(Dock::Left, rect.left()) && touches(Dock::Left, rect.right() - 1.0), "on the left side: only the right one");
        assert!(touches(Dock::Right, rect.left()) && !touches(Dock::Right, rect.right() - 1.0));
        let pulls_height_at = |dock, y: f32| edge_zones(rect, dock, 8.0).iter().any(|zone| zone.0.y_range().contains(y) && zone.3);
        assert!(pulls_height_at(Dock::Top, rect.bottom() - 1.0) && !pulls_height_at(Dock::Top, rect.top() + 1.0));
        assert!(pulls_height_at(Dock::Bottom, rect.top() + 1.0) && !pulls_height_at(Dock::Bottom, rect.bottom() - 1.0), "on the bottom: its top");
        // the pointer 100 past the far edge
        let pointer = Pos2::new(rect.right() + 100.0, rect.bottom() + 100.0);
        assert_eq!(pulled_to(rect, Dock::Top, pointer, true, true), Vec2::new(840.0, 460.0), "centred: both sides");
        assert_eq!(pulled_to(rect, Dock::Left, pointer, true, true), Vec2::new(740.0, 560.0), "from the left side, centred on the height");
        assert_eq!(pulled_to(rect, Dock::Bottom, Pos2::new(rect.center().x, rect.top() - 100.0), false, true), Vec2::new(640.0, 460.0), "up");
        assert_eq!(pulled_to(rect, Dock::Right, Pos2::new(rect.left() - 100.0, 0.0), true, false), Vec2::new(740.0, 360.0));
    }

    /// Asked ("eu arrasto pra cima ele fica pulando ate o final"): the system stops a window at the top bar.
    #[test]
    fn a_dragged_island_has_a_window_of_its_size_until_it_opens_once_let_go() {
        let closed = Vec2::new(288.0, 40.0);
        let slim = Some(closed);
        assert_eq!(slim_kept(slim, closed, true), slim, "while it moves");
        assert_eq!(slim_kept(slim, Vec2::new(760.0, 420.0), true), slim, "growing while it moves: no resize under the drag");
        assert_eq!(slim_kept(slim, closed, false), slim, "let go and closed: the window stays small");
        assert_eq!(slim_kept(slim, Vec2::new(300.0, 40.0), false), None, "let go, it grows: the room comes back");
        assert_eq!(slim_kept(None, closed, true), None);
    }

    /// Asked ("ele nao está identificando quando a ilha está em um monitor ou está em outro").
    #[test]
    fn the_island_goes_by_the_monitor_it_is_on() {
        let first = Rect::from_min_max(Pos2::new(0.0, 32.0), Pos2::new(1920.0, 1017.0));
        let second = Rect::from_min_max(Pos2::new(1920.0, 0.0), Pos2::new(3840.0, 1080.0));
        let works = [first, second];
        let work_at = crate::app::chat::mini::work_at;
        assert_eq!(work_at(&works, Pos2::new(2800.0, 1060.0)), Some(second), "low on the second: its bottom is lower");
        assert_eq!(work_at(&works, Pos2::new(900.0, 32.5)), Some(first));
        assert_eq!(work_at(&works, Pos2::new(5200.0, 400.0)), Some(second), "off them all: the nearest");
        assert_eq!(work_at(&works, Pos2::new(900.0, 10.0)), Some(first), "over the top bar");
        assert_eq!(work_at(&[], Pos2::ZERO), None);
        // let go near the second's bottom it sticks to it, not to the first's
        let anchor = dropped(Rect::from_min_size(Pos2::new(2600.0, 1000.0), Vec2::new(288.0, 40.0)), second);
        assert_eq!(anchor[1], 1079.0);
        assert_eq!(keep_on_screen(anchor, second), [2744.0, 1079.0], "kept where it is, on the second monitor");
        // asked ("na lateral que tem outro monitor, ela não gruda"): held by Luna, at its left, the island let go by the
        // first's right edge has its centre on the second; the cursor's monitor is the first, and it sticks to its side
        let across = Rect::from_min_size(Pos2::new(1880.0, 480.0), Vec2::new(188.0, 38.5));
        let grab = Pos2::new(1900.0, 500.0);
        assert_eq!(work_at(&works, across.center()), Some(second), "by its centre, the other monitor");
        let anchor = dropped(across, work_at(&works, grab).unwrap());
        assert_eq!(Dock::of(anchor, first), Dock::Right, "on the first's right side: {anchor:?}");
    }

    /// Asked ("tento pegar para arrastar ... ele trava, preciso soltar o click e fazer novamente"): a pause in the middle
    /// of a drag must not move the window; sticking to an edge waits longer.
    #[test]
    fn a_pause_in_a_drag_does_not_move_the_window_until_it_has_been_still_a_while() {
        let placed = Pos2::new(600.0, 90.0);
        assert_eq!(settle_for(placed, placed), SETTLE, "it stays where it is: soon");
        assert_eq!(settle_for(Pos2::new(600.0, 32.0), placed), SNAP_SETTLE, "it goes up to the top edge: later");
        const { assert!(SNAP_SETTLE >= 1.0 && SETTLE < SNAP_SETTLE) };
    }

    #[test]
    fn the_window_has_room_for_the_conversation_and_for_all_it_can_be_pulled_to() {
        let most = Vec2::new(1824.0, 988.0);
        assert_eq!(window_cap(Vec2::new(640.0, 360.0), most, false), Vec2::new(720.0, 420.0), "small: the least room");
        let big = window_cap(Vec2::new(1344.0, 728.0), most, false);
        assert!(big.x > 1344.0 && big.y > 728.0, "the big conversation and its overshoot fit: {big:?}");
        assert_eq!(window_cap(Vec2::new(640.0, 360.0), most, true), most, "an edge pulled: the largest it can get, from the start");
    }

    #[test]
    fn on_the_top_edge_of_a_screen_with_a_notch_the_island_hangs_from_the_notchs_middle() {
        // a 13" Air with its Dock at the left: the work area's middle is not the notch's
        let work = Rect::from_min_max(Pos2::new(64.0, 0.0), Pos2::new(1470.0, 956.0));
        let notch = Some(Rect::from_min_max(Pos2::new(645.5, 0.0), Pos2::new(824.5, 32.0)));
        assert_eq!(to_notch([300.0, 0.0], work, notch), [735.0, 0.0], "anywhere on the top edge");
        assert_eq!(to_notch([767.0, 0.0], work, None), [767.0, 0.0], "no notch: where it is");
        for away in [[300.0, 400.0], [64.0, 300.0], [1469.0, 300.0], [300.0, 955.0]] {
            assert_eq!(to_notch(away, work, notch), away, "off the top edge: where it is");
        }
        let island = hung([735.0, 0.0], Dock::Top, Vec2::new(179.0 + 60.0, 32.0));
        assert_eq!((island.center().x, island.top()), (735.0, 0.0), "around the notch, from the very top");
    }
}
