//! The island: the Maestro as a black notch glued to the top edge of the screen, over every other window, like
//! Coucou's (MIT) island. Closed it is small (the wizard at the left, a mini wizard per agent at the right, in the
//! agent's colour); with the pointer resting on it, or a question waiting, it grows into a wide panel: Home (the agent
//! in focus and a grid of the agents), the conversation, the new chat (a workspace of its own, `actions.rs`) and the
//! questions and permissions. It is dragged anywhere on the screen, another monitor too, and sticks to the top edge
//! when let go near it; the place is kept (`Prefs.island_at`).

use crate::app::mascot::mago::MagoState;
use crate::app::ui::agents::{AgentRow, AgentState};
use crate::prelude::*;

mod actions;
mod cursor;
mod forms;
mod paint;
mod sounds;

/// The sizes of Coucou's island (core/layout.ts): the closed one, the open one and their corners.
pub(crate) const COMPACT_W: f32 = 288.0;
pub(crate) const COMPACT_H: f32 = 32.0;
pub(crate) const EXPANDED_W: f32 = 640.0;
pub(crate) const CLOSED_W: f32 = 80.0;
pub(crate) const CLOSED_H: f32 = 24.0;
const CLOSED_CORNER: f32 = 10.0;
const COMPACT_CORNER: f32 = 14.0;
/// The conversation at least this big, in points.
const CHAT_MIN: (f32, f32) = (420.0, 300.0);
/// How long the compact island stays after the pointer was about.
const PEEK: f64 = 60.0;
const EXPANDED_CORNER: f32 = 22.0;
/// Home's height, and the conversation's.
const HOME_H: f32 = 160.0;
const CHAT_H: f32 = 360.0;
/// How near the top edge a drop sticks to it.
const SNAP: f32 = 60.0;
/// How long the pointer rests on the closed island before it opens, and how long it stays open after the pointer left
/// (Home; the conversation and the form wait longer).
const OPEN_AFTER: f64 = 0.4;
const LINGER: f64 = 0.6;
const LINGER_LONG: f64 = 15.0;
/// Windows hides the top row of a window without a frame (`plain::strip`): the window starts a row above the screen.
const TOP_PAD: f32 = if cfg!(windows) { 1.0 } else { 0.0 };

/// What the island says about the agents at a glance.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) enum Mood {
    /// This many wait on the person.
    Needs(usize),
    /// This many work.
    Working(usize),
    Ready,
    Empty,
}

/// The mood of agents in these states: someone waiting wins over the ones working; the ended are not counted.
pub(crate) fn mood<'a>(states: impl IntoIterator<Item = &'a AgentState>) -> Mood {
    let (mut needs, mut working, mut live) = (0, 0, 0);
    for state in states {
        match state {
            AgentState::Ended => continue,
            AgentState::Waiting => needs += 1,
            AgentState::Working(_) => working += 1,
            _ => {}
        }
        live += 1;
    }
    match (needs, working, live) {
        (0, 0, 0) => Mood::Empty,
        (0, 0, _) => Mood::Ready,
        (0, working, _) => Mood::Working(working),
        (needs, ..) => Mood::Needs(needs),
    }
}

/// The wizard of the island itself: the sign while someone waits, focused while they work, celebrating when everything
/// just finished, else at rest.
pub(crate) fn main_state(mood: Mood, celebrating: bool) -> MagoState {
    match mood {
        Mood::Needs(_) => MagoState::NeedsYou,
        Mood::Working(_) => MagoState::Working,
        Mood::Ready if celebrating => MagoState::Done,
        _ => MagoState::Idle,
    }
}

/// What the open island shows.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub(crate) enum Tab {
    #[default]
    Home,
    Chat,
    New,
}

/// What the island shows of itself: the closed notch (the wizard's face), the compact one (the wizard and a mini wizard
/// per agent, while the pointer is about) or the open panel.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) enum Shape {
    Closed,
    Compact,
    Open,
}

/// The island's size and corner for its `shape`; open, for what `tab` shows (`pinned`: a question waits, whose card is
/// `body` tall; `chat`: the conversation's size).
pub(crate) fn target(shape: Shape, tab: Tab, pinned: bool, body: f32, chat: Vec2) -> (Vec2, f32) {
    match shape {
        Shape::Closed => return (Vec2::new(CLOSED_W, CLOSED_H), CLOSED_CORNER),
        Shape::Compact => return (Vec2::new(COMPACT_W, COMPACT_H), COMPACT_CORNER),
        Shape::Open => {}
    }
    match (pinned, tab) {
        (true, _) => (Vec2::new(EXPANDED_W, (42.0 + body + 12.0).clamp(HOME_H, 400.0)), EXPANDED_CORNER),
        (_, Tab::Home) => (Vec2::new(EXPANDED_W, HOME_H), EXPANDED_CORNER),
        (_, Tab::Chat) => (chat, EXPANDED_CORNER),
        (_, Tab::New) => (Vec2::new(EXPANDED_W, (42.0 + body + 12.0).clamp(220.0, 420.0)), EXPANDED_CORNER),
    }
}

/// How much of each content shows at island width `width`: (closed, compact, open); the one gives way to the next as
/// the island grows.
pub(crate) fn alphas(width: f32) -> (f32, f32, f32) {
    let closed = (1.0 - (width - CLOSED_W) / 60.0).clamp(0.0, 1.0);
    let compact = ((width - CLOSED_W - 20.0) / 60.0).clamp(0.0, 1.0) * (1.0 - (width - COMPACT_W - 12.0) / 150.0).clamp(0.0, 1.0);
    let open = ((width - COMPACT_W - 90.0) / 160.0).clamp(0.0, 1.0);
    (closed, compact, open)
}

/// The conversation's size kept in `Prefs`, within what the monitor's work area `work` allows; `big`: 70% of it.
pub(crate) fn chat_size(saved: Option<[f32; 2]>, big: bool, work: Rect) -> Vec2 {
    let most = Vec2::new(work.width() * 0.95, work.height() * 0.95);
    let least = Vec2::new(CHAT_MIN.0, CHAT_MIN.1).min(most);
    let size = if big { work.size() * 0.7 } else { saved.map_or(Vec2::new(EXPANDED_W, CHAT_H), |[w, h]| Vec2::new(w, h)) };
    size.clamp(least, most)
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

/// One agent or chat of the grid.
pub(super) struct Line {
    pub(super) id: String,
    pub(super) name: String,
    pub(super) detail: String,
    pub(super) state: AgentState,
    pub(super) mago: MagoState,
    pub(super) color: Color32,
    pub(super) chat: bool,
}

/// The colour of an agent's mini wizard, from Coucou's catalogue of agents (core/agents.ts) and its fallbacks.
pub(crate) fn agent_color(program: &str, id: &str) -> Color32 {
    let hex = |value: u32| Color32::from_rgb((value >> 16) as u8, (value >> 8) as u8, value as u8);
    match program {
        "claude" => hex(0xd97757),
        "codex" => hex(0x2dd4bf),
        "gemini" => hex(0x8ab4f8),
        "cursor" => hex(0xc0c4cc),
        _ => {
            let hash = id.bytes().fold(0i32, |hash, byte| hash.wrapping_mul(31).wrapping_add(byte as i32));
            [hex(0x22c55e), hex(0xeab308), hex(0x60a5fa), hex(0xe879f9)][hash.unsigned_abs() as usize % 4]
        }
    }
}

#[derive(Default)]
pub(crate) struct Island {
    /// The island stays open until then (the pointer was over it).
    hold_until: f64,
    /// Since when the pointer rests on the closed island.
    hover_since: Option<f64>,
    /// Closed with its button: it opens again only after the pointer left it.
    suppress: bool,
    pub(crate) tab: Tab,
    /// The agent the Home card shows when the pointer is on its grid.
    focus: Option<String>,
    /// How many questions there were last frame.
    asked: usize,
    /// Something was working or waiting last frame: the end of it is news.
    was_busy: bool,
    celebrate_until: f64,
    /// The island as drawn: its size and corner follow the target, easing.
    anim: Vec2,
    radius: f32,
    /// The window's size: fixed, the largest the island gets (the chat's size when it is bigger), and the island is drawn inside it.
    cap: Vec2,
    /// A button of the mouse is down over it.
    holding: bool,
    /// The cursor, read on the whole screen, is over it.
    over: bool,
    /// Whether the window takes the cursor (it passes through outside the island), as last sent.
    captured: Option<bool>,
    /// The compact island shows until then (the pointer was about); whether the panel was open last frame.
    peek_until: f64,
    open: bool,
    /// The conversation at the size that is kept, big (70% of the monitor), and being pulled by an edge now.
    chat_big: bool,
    resizing: bool,
    /// Where the eyes look (window coordinates), the sounds playing, and what the last frame was, to tell what changed.
    pointer: Option<Pos2>,
    tones: Vec<(cpal::Stream, f64)>,
    last_shape: Option<Shape>,
    last_hover: Option<String>,
    /// The velocity of the panel opening (a spring: it overshoots a little before it settles).
    velocity: Vec2,
    /// The pointer is over it (last frame).
    hovered: bool,
    /// The open page's body as drawn last frame.
    body: f32,
    /// Where it hangs from (centre x, top) and where its window was put for the size it was built at.
    anchor: Option<[f32; 2]>,
    origin: Pos2,
    built: Vec2,
    /// Where the window was seen after the person moved it, and since when: let go once it stays.
    moved: Option<(Pos2, f64)>,
    pub(crate) form: Option<NewForm>,
    /// The window on its way onto the screen without Windows' border (`plain::reveal`), and the pass it was last shown in.
    plain: u8,
    pass: u64,
}

/// The form of a new chat.
#[derive(Clone, Debug, Default, PartialEq)]
pub(crate) struct NewForm {
    /// The name of the workspace it makes (empty: the folder's).
    pub(crate) name: String,
    pub(crate) folder: String,
    /// In a git worktree of its own (`maestro/environments.rs`), so the repository's changes stay as they are.
    pub(crate) worktree: bool,
}

impl Island {
    /// The island as drawn now.
    #[cfg(test)]
    pub(crate) fn size(&self) -> Vec2 {
        self.anim
    }
}

impl DrawingApp {
    /// Every frame: the island, when it is on.
    pub(crate) fn island_ui(&mut self, ctx: &Context) {
        if !self.prefs.island {
            self.island = Island::default();
            return;
        }
        let time = ctx.input(|input| input.time);
        let rows: Vec<AgentRow> = self.agent_rows().into_iter().filter(|row| row.state != AgentState::Ended).collect();
        let asks: Vec<String> = self.person_asks.borrow().iter().map(|ask| ask.chat.clone()).collect();
        let mood = mood(rows.iter().map(|row| &row.state));
        let pinned = !asks.is_empty();
        // the monitor's work area bounds the conversation's size
        let work = if ctx.embed_viewports() {
            ctx.content_rect()
        } else {
            let anchor = self.island_anchor(ctx);
            crate::app::chat::mini::screen_work(ctx, Pos2::new(anchor[0], anchor[1]))
        };
        let chat = chat_size(self.prefs.island_chat, self.island.chat_big, work);
        if !self.island.resizing {
            self.island.cap = Vec2::new(720f32.max(chat.x), 420f32.max(chat.y));
        }
        let island = &mut self.island;
        let busy = matches!(mood, Mood::Needs(_) | Mood::Working(_));
        let mut tones: Vec<crate::voice::Sound> = Vec::new();
        if island.was_busy && mood == Mood::Ready {
            island.celebrate_until = time + 6.0;
            tones.push(sounds::FINISH);
        }
        island.was_busy = busy;
        let asked_before = std::mem::replace(&mut island.asked, asks.len());
        if asks.len() < asked_before {
            tones.push(sounds::APPROVE);
        }
        // the pointer about wakes the notch into the compact island; resting on it opens the panel, and leaving, the
        // panel folds after a moment (a question keeps it open)
        let was_open = island.open;
        if island.hovered && !island.suppress {
            if was_open {
                island.hold_until = time + if island.tab == Tab::Home { LINGER } else { LINGER_LONG };
            } else if time < island.peek_until {
                let since = *island.hover_since.get_or_insert(time);
                if time - since >= OPEN_AFTER {
                    island.hold_until = time + LINGER;
                }
            }
            island.peek_until = time + PEEK;
        } else if !island.hovered {
            island.hover_since = None;
            island.suppress = false;
        }
        let open = pinned || time < island.hold_until || island.form.is_some();
        if open {
            island.peek_until = time + PEEK;
        } else {
            island.hold_until = 0.0;
            island.tab = Tab::Home;
            island.chat_big = false;
        }
        island.open = open;
        let shape = if open {
            Shape::Open
        } else if time < island.peek_until {
            Shape::Compact
        } else {
            Shape::Closed
        };
        match (island.last_shape.replace(shape), shape) {
            (Some(Shape::Closed), Shape::Compact) => tones.push(sounds::PEEK),
            (Some(Shape::Open), Shape::Compact | Shape::Closed) => tones.push(sounds::CLOSE),
            (Some(Shape::Closed | Shape::Compact), Shape::Open) => tones.push(if asks.len() > asked_before { sounds::ASK } else { sounds::OPEN }),
            (Some(Shape::Open), Shape::Open) if asks.len() > asked_before => tones.push(sounds::ASK),
            _ => {}
        }
        let (aim, radius) = target(shape, island.tab, pinned, island.body, chat);
        // it grows on a spring (Coucou: response 0.5 s, damping 0.72, so it overshoots a little and settles) and folds
        // smoothly in a few hundred ms; pulled by an edge it follows the pointer at once
        let dt = ctx.input(|input| input.stable_dt).min(0.05);
        if island.anim == Vec2::ZERO {
            (island.anim, island.radius) = (aim, radius);
        }
        if island.resizing {
            (island.anim, island.velocity) = (aim, Vec2::ZERO);
        } else if aim.x + aim.y >= island.anim.x + island.anim.y {
            let omega = std::f32::consts::TAU / 0.5;
            let pull = (aim - island.anim) * omega * omega - island.velocity * (2.0 * 0.72 * omega);
            island.velocity += pull * dt;
            island.anim += island.velocity * dt;
        } else {
            island.velocity = Vec2::ZERO;
            island.anim += (aim - island.anim) * (1.0 - (-22.0 * dt).exp());
        }
        island.radius += (radius - island.radius) * (1.0 - (-15.0 * dt).exp());
        let animating = (aim - island.anim).abs().max_elem() > 0.5 || island.velocity.length() > 1.0;
        if animating {
            ctx.request_repaint();
        } else {
            (island.anim, island.radius, island.velocity) = (aim, radius, Vec2::ZERO);
        }
        let view = self.island_view(rows, asks, mood, time);
        let colors = self.colors();
        let look = crate::app::chat::list::Look::new(true, &colors);
        let mut form = self.island.form.take();
        let out = if ctx.embed_viewports() {
            self.island_area(ctx, &view, &mut form, &look, &colors, time)
        } else {
            self.island_window(ctx, &view, &mut form, &look, &colors, time)
        };
        self.island.form = form;
        let island = &mut self.island;
        island.hovered = out.hovered || island.over;
        island.holding = out.holding;
        if out.hovered && !was_open && !open {
            ctx.request_repaint_after(std::time::Duration::from_millis(100));
        }
        if (out.body - island.body).abs() > 1.0 {
            island.body = out.body;
            ctx.request_repaint();
        }
        // the wizards move while they work or wait; the fold after the pointer left comes by the clock
        let moving = busy || time < island.celebrate_until || open || island.hover_since.is_some();
        if moving {
            ctx.request_repaint_after(std::time::Duration::from_millis(33));
        }
        if cfg!(windows) && !ctx.embed_viewports() {
            ctx.request_repaint_after(std::time::Duration::from_millis(if self.island.pointer.is_some() { 33 } else { 250 }));
        }
        // the hover of a pill and the clicks of the buttons
        if out.focus.is_some() && out.focus != self.island.last_hover {
            tones.push(sounds::HOVER);
        }
        self.island.last_hover = out.focus.clone();
        if out.tab.is_some() || out.open_agent.is_some() || out.talk.is_some() || out.voice || out.close || out.big || out.sound {
            tones.push(sounds::BLIP);
        }
        for tone in tones {
            self.island_chime(tone, time);
        }
        self.island_apply(ctx, out, time);
    }

    /// Where the island hangs from: as it was left, kept on a monitor that is there; the first time, the top centre of
    /// the screen the app is on.
    fn island_anchor(&mut self, ctx: &Context) -> [f32; 2] {
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
    fn island_window(
        &mut self,
        ctx: &Context,
        view: &paint::View,
        form: &mut Option<NewForm>,
        look: &crate::app::chat::list::Look,
        colors: &UiColors,
        time: f64,
    ) -> paint::Out {
        let anchor = self.island_anchor(ctx);
        let visible = if self.island.cap == Vec2::ZERO { Vec2::new(720.0, 420.0) } else { self.island.cap };
        // the window is put where the island's centre stays only when its size changes: a builder that followed the
        // window would fight the system's drag
        if self.island.built != visible {
            self.island.built = visible;
            self.island.origin = window_origin(anchor, visible);
        }
        if crate::app::chat::mini::fresh(ctx, &mut self.island.pass) {
            self.island.plain = 0;
        }
        let (origin, size) = (self.island.origin, window_size(visible));
        let builder = egui::ViewportBuilder::default()
            .with_title("Ilha do Maestro")
            .with_transparent(true)
            .with_decorations(false)
            .with_always_on_top()
            .with_taskbar(false)
            .with_resizable(false)
            .with_active(false)
            // shown once the system's frame is off it (`plain::reveal`): no glass square for a frame
            .with_visible(!cfg!(windows) || self.island.plain > 0)
            .with_position(origin)
            .with_inner_size(size);
        // the eyes follow the cursor over the whole screen while it is near
        let scale = ctx.input(|input| input.viewport().native_pixels_per_point).unwrap_or(1.0);
        let shown = Rect::from_min_size(Pos2::new(size.x / 2.0 - self.island.anim.x / 2.0, TOP_PAD), self.island.anim);
        self.island.pointer = cursor::screen_cursor().and_then(|at| cursor::eye_target(at, scale, origin, shown));
        // outside the island the click goes through to what is under it; on it (a margin around it too), or while a press
        // is held on it, the window takes it (Windows: the cursor is read on the whole screen)
        let cursor = cursor::screen_cursor().map(|at| Pos2::new(at.x / scale - origin.x, at.y / scale - origin.y));
        let capture = !cfg!(windows) || cursor::captures(cursor, shown, self.island.holding || self.island.resizing);
        self.island.over = cursor.is_some_and(|at| shown.contains(at));
        let send_capture = self.island.captured != Some(capture);
        self.island.captured = Some(capture);
        let id = egui::ViewportId::from_hash_of("island_window");
        let (anim, radius) = (self.island.anim, self.island.radius);
        let (out, placed) = ctx.show_viewport_immediate(id, builder, |ui, _| {
            let at = Pos2::new((ui.max_rect().center().x - anim.x / 2.0).round(), ui.max_rect().top() + TOP_PAD);
            let out = self.island_draw(ui, at, anim, radius, view, form, look, colors, time);
            if send_capture {
                ui.ctx().send_viewport_cmd(egui::ViewportCommand::MousePassthrough(!capture));
            }
            if out.drag {
                ui.ctx().send_viewport_cmd(egui::ViewportCommand::StartDrag);
            }
            (out, ui.ctx().input(|input| input.viewport().outer_rect))
        });
        #[cfg(windows)]
        if self.island.plain < 3 {
            crate::app::voice::popup::plain::reveal("Ilha do Maestro", &mut self.island.plain);
            ctx.request_repaint();
        }
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
                    let work = crate::app::chat::mini::screen_work(ctx, placed.center());
                    let anchor = snap_top(anchor_of(placed), work);
                    self.island.anchor = Some(anchor);
                    self.island.origin = window_origin(anchor, self.island.built);
                    self.prefs.island_at = Some(anchor);
                    self.island.moved = None;
                }
            }
            _ => self.island.moved = Some((placed.min, time)),
        }
        ctx.request_repaint_after(std::time::Duration::from_millis(100));
    }

    /// With no windows of its own (the tests, a backend without them): over the canvas, at the top centre of the app.
    fn island_area(
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
    fn the_island_says_what_the_agents_are_doing_and_the_wizard_follows() {
        let working = AgentState::Working(10.0);
        let idle = AgentState::Idle(None);
        assert_eq!(mood([]), Mood::Empty);
        assert_eq!(mood([&AgentState::Ended]), Mood::Empty, "the ended are not counted");
        assert_eq!(mood([&idle, &AgentState::Active(1.0)]), Mood::Ready);
        assert_eq!(mood([&working, &idle, &working]), Mood::Working(2));
        // someone waiting wins over the ones working
        assert_eq!(mood([&working, &AgentState::Waiting, &idle]), Mood::Needs(1));
        assert_eq!(main_state(Mood::Needs(1), false), MagoState::NeedsYou);
        assert_eq!(main_state(Mood::Working(2), true), MagoState::Working);
        assert_eq!(main_state(Mood::Ready, true), MagoState::Done);
        assert_eq!(main_state(Mood::Ready, false), MagoState::Idle);
    }

    #[test]
    fn closed_it_is_a_small_notch_then_coucous_compact_island_and_the_open_panel() {
        let chat = Vec2::new(640.0, 360.0);
        assert_eq!(target(Shape::Closed, Tab::Chat, false, 0.0, chat), (Vec2::new(80.0, 24.0), 10.0));
        assert_eq!(target(Shape::Compact, Tab::Chat, false, 0.0, chat), (Vec2::new(288.0, 32.0), 14.0));
        assert_eq!(target(Shape::Open, Tab::Home, false, 0.0, chat), (Vec2::new(640.0, 160.0), 22.0));
        assert_eq!(target(Shape::Open, Tab::Chat, false, 0.0, Vec2::new(900.0, 500.0)).0, Vec2::new(900.0, 500.0), "the conversation: its own size");
        // a question takes the island over, as tall as its card (within limits)
        assert_eq!(target(Shape::Open, Tab::Home, true, 500.0, chat).0.y, 400.0);
        assert_eq!(target(Shape::Open, Tab::Home, true, 10.0, chat).0.y, 160.0);
        // as it grows, the face gives way to the compact content and that to the panel
        assert_eq!(alphas(80.0), (1.0, 0.0, 0.0));
        assert_eq!(alphas(288.0), (0.0, 1.0, 0.0));
        assert_eq!(alphas(640.0), (0.0, 0.0, 1.0));
    }

    #[test]
    fn the_conversation_is_sized_within_the_monitor_and_big_is_seventy_percent() {
        let work = Rect::from_min_size(Pos2::ZERO, Vec2::new(1920.0, 1040.0));
        assert_eq!(chat_size(None, false, work), Vec2::new(640.0, 360.0));
        assert_eq!(chat_size(Some([800.0, 500.0]), false, work), Vec2::new(800.0, 500.0));
        assert_eq!(chat_size(Some([100.0, 100.0]), false, work), Vec2::new(420.0, 300.0), "not smaller than the least");
        assert_eq!(chat_size(Some([9000.0, 9000.0]), false, work), Vec2::new(1824.0, 988.0), "nor bigger than the monitor");
        assert_eq!(chat_size(Some([800.0, 500.0]), true, work), Vec2::new(1344.0, 728.0), "big: 70% of the work area");
    }

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
    fn a_saved_place_on_a_monitor_that_is_gone_comes_back_and_a_drop_near_the_top_sticks() {
        let work = Rect::from_min_max(Pos2::new(0.0, 40.0), Pos2::new(1920.0, 1040.0));
        assert_eq!(keep_on_screen([960.0, 300.0], work), [960.0, 300.0], "where it is, it stays");
        assert_eq!(keep_on_screen([5000.0, -300.0], work), [1860.0, 40.0]);
        assert_eq!(keep_on_screen([-40.0, 9000.0], work), [60.0, 960.0]);
        assert_eq!(snap_top([700.0, 75.0], work), [700.0, 40.0], "near the top edge: on it");
        assert_eq!(snap_top([700.0, 140.0], work), [700.0, 140.0], "farther: where it was let go");
    }

    #[test]
    fn each_agent_has_its_colour() {
        assert_eq!(agent_color("claude", "a"), Color32::from_rgb(0xd9, 0x77, 0x57));
        assert_eq!(agent_color("codex", "a"), Color32::from_rgb(0x2d, 0xd4, 0xbf));
        assert_eq!(agent_color("outro", "x"), agent_color("outro", "x"), "the same every time");
    }
}
