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
mod forms;
mod paint;

/// The sizes of Coucou's island (core/layout.ts): the closed one, the open one and their corners.
pub(crate) const COMPACT_W: f32 = 288.0;
pub(crate) const COMPACT_H: f32 = 32.0;
pub(crate) const EXPANDED_W: f32 = 640.0;
const COMPACT_CORNER: f32 = 14.0;
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

/// The island's size and corner for what it shows: closed, or open on `tab` (`pinned`: a question waits, whose card
/// is `body` tall).
pub(crate) fn target(open: bool, tab: Tab, pinned: bool, body: f32) -> (Vec2, f32) {
    if !open {
        return (Vec2::new(COMPACT_W, COMPACT_H), COMPACT_CORNER);
    }
    let height = match (pinned, tab) {
        (true, _) => (42.0 + body + 12.0).clamp(HOME_H, 400.0),
        (_, Tab::Home) => HOME_H,
        (_, Tab::Chat) => CHAT_H,
        (_, Tab::New) => (42.0 + body + 12.0).clamp(220.0, 420.0),
    };
    (Vec2::new(EXPANDED_W, height), EXPANDED_CORNER)
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
    /// The window's visible size: the target, or the larger of the two while the island shrinks.
    reach: Vec2,
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
        let island = &mut self.island;
        let busy = matches!(mood, Mood::Needs(_) | Mood::Working(_));
        if island.was_busy && mood == Mood::Ready {
            island.celebrate_until = time + 6.0;
        }
        island.was_busy = busy;
        island.asked = asks.len();
        // the pointer resting on the closed island opens it; leaving, it folds after a moment (a question keeps it open)
        let was_open = island.anim.x > COMPACT_W + 1.0 && island.hold_until > 0.0;
        if island.hovered && !island.suppress {
            if was_open {
                island.hold_until = time + if island.tab == Tab::Home { LINGER } else { LINGER_LONG };
            } else {
                let since = *island.hover_since.get_or_insert(time);
                if time - since >= OPEN_AFTER {
                    island.hold_until = time + LINGER;
                }
            }
        } else if !island.hovered {
            island.hover_since = None;
            island.suppress = false;
        }
        let open = pinned || time < island.hold_until || island.form.is_some();
        if !open {
            island.hold_until = 0.0;
            island.tab = Tab::Home;
        }
        let (aim, radius) = target(open, island.tab, pinned, island.body);
        // easing: it grows quickly and folds a little faster
        let rate = if aim.x < island.anim.x { 22.0 } else { 15.0 };
        let step = 1.0 - (-rate * ctx.input(|input| input.stable_dt).min(0.05)).exp();
        if island.anim == Vec2::ZERO {
            (island.anim, island.radius) = (aim, radius);
        }
        island.anim += (aim - island.anim) * step;
        island.radius += (radius - island.radius) * step;
        let animating = (aim - island.anim).abs().max_elem() > 0.5;
        if animating {
            island.reach = island.reach.max(aim).max(island.anim.ceil());
            ctx.request_repaint();
        } else {
            (island.anim, island.radius, island.reach) = (aim, radius, aim);
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
        island.hovered = out.hovered;
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
        let visible = self.island.reach;
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
        let id = egui::ViewportId::from_hash_of("island_window");
        let (anim, radius) = (self.island.anim, self.island.radius);
        let (out, placed) = ctx.show_viewport_immediate(id, builder, |ui, _| {
            let at = Pos2::new((ui.max_rect().center().x - anim.x / 2.0).round(), ui.max_rect().top() + TOP_PAD);
            let out = self.island_draw(ui, at, anim, radius, view, form, look, colors, time);
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
    fn closed_it_is_coucous_notch_and_open_it_is_as_wide_as_coucous_panel() {
        assert_eq!(target(false, Tab::Chat, false, 0.0), (Vec2::new(288.0, 32.0), 14.0));
        assert_eq!(target(true, Tab::Home, false, 0.0), (Vec2::new(640.0, 160.0), 22.0));
        assert_eq!(target(true, Tab::Chat, false, 0.0).0.x, 640.0);
        // a question takes the island over, as tall as its card (within limits)
        assert_eq!(target(true, Tab::Home, true, 500.0).0.y, 400.0);
        assert_eq!(target(true, Tab::Home, true, 10.0).0.y, 160.0);
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
