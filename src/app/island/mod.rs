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
mod entry;
mod forms;
mod fsm;
mod home;
mod integrations;
mod keymap;
mod keys;
mod minis;
mod paint;
mod sounds;
mod spring;
mod stage;
mod window;
pub(crate) use window::*;

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
const EXPANDED_CORNER: f32 = 22.0;
/// Home's height, and the conversation's.
const HOME_H: f32 = 160.0;
const CHAT_H: f32 = 360.0;

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
/// `body` tall; `tall`: an edit's change needs room on Home; `chat`: the conversation's size).
pub(crate) fn target(shape: Shape, tab: Tab, pinned: bool, tall: bool, body: f32, chat: Vec2) -> (Vec2, f32) {
    match shape {
        Shape::Closed => return (Vec2::new(CLOSED_W, CLOSED_H), CLOSED_CORNER),
        Shape::Compact => return (Vec2::new(COMPACT_W, COMPACT_H), COMPACT_CORNER),
        Shape::Open => {}
    }
    match (pinned, tab) {
        (true, _) => (Vec2::new(EXPANDED_W, (42.0 + body + 12.0).clamp(HOME_H, 400.0)), EXPANDED_CORNER),
        (_, Tab::Home) => (Vec2::new(EXPANDED_W, if tall { home::EDIT_H } else { HOME_H }), EXPANDED_CORNER),
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

/// The largest the conversation gets on a monitor whose work area is `work`.
pub(crate) fn chat_most(work: Rect) -> Vec2 {
    work.size() * 0.95
}

/// The conversation's size kept in `Prefs`, within what the monitor's work area `work` allows; `big`: 70% of it.
pub(crate) fn chat_size(saved: Option<[f32; 2]>, big: bool, work: Rect) -> Vec2 {
    let most = chat_most(work);
    let least = Vec2::new(CHAT_MIN.0, CHAT_MIN.1).min(most);
    let size = if big { work.size() * 0.7 } else { saved.map_or(Vec2::new(EXPANDED_W, CHAT_H), |[w, h]| Vec2::new(w, h)) };
    size.clamp(least, most)
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
    /// The dot on its corner (a session outside the app: `maestro/outside.rs`); the chats' come from their state.
    pub(super) badge: Option<crate::app::maestro::outside::Badge>,
    /// The program as its card names it ("Claude Code").
    pub(super) kind: String,
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
    /// The island's clock-driven hidden/compact/open/greeting lifecycle.
    machine: fsm::Machine,
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
    /// The room the window has for the island (`window_cap`): the island is drawn inside it.
    cap: Vec2,
    /// A button of the mouse is down over it.
    holding: bool,
    /// The cursor, read on the whole screen, is over it.
    over: bool,
    /// Whether the window takes the cursor (it passes through outside the island), as last sent.
    captured: Option<bool>,
    /// Whether the panel was open last frame.
    open: bool,
    /// The conversation at the size that is kept, big (70% of the monitor), and being pulled by an edge now.
    chat_big: bool,
    resizing: bool,
    /// Where the eyes look (window coordinates), the sounds playing, and what the last frame was, to tell what changed.
    pointer: Option<Pos2>,
    tones: Vec<(cpal::Stream, f64)>,
    last_shape: Option<Shape>,
    /// How many sounds were asked for (tests: the hover of a pill asks for none).
    #[cfg(test)]
    chimes: u32,
    /// Its size and corner on their way to the target (`spring.rs`): `anim` and `radius` are where they are now.
    geometry: spring::Geometry,
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
    /// When the conversations of the agents at work were last read, and what the card in focus tells (`stage.rs`): the
    /// step it is on, since when, and the glimpse of the file read.
    polled: f64,
    stage_key: String,
    stage_since: f64,
    glimpse_for: Option<PathBuf>,
    glimpse: Vec<(usize, String)>,
    /// The entrance (`entry.rs`) started at this time; played once, when the island first shows.
    greeting: Option<f64>,
    greeted: bool,
    /// How many pokes of the wizard and failures were counted, for their sounds; the page is tall (an edit is on).
    pokes: u32,
    failures: usize,
    tall: bool,
    /// Hung away from the top edge of its monitor.
    floating: bool,
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

    /// The window around it, as it is asked for.
    #[cfg(test)]
    pub(crate) fn window(&self) -> Vec2 {
        window_size(self.cap)
    }

    #[cfg(test)]
    pub(crate) fn tab(&self) -> Tab {
        self.tab
    }

    #[cfg(test)]
    pub(crate) fn chat_big(&self) -> bool {
        self.chat_big
    }

    #[cfg(test)]
    pub(crate) fn chimes(&self) -> u32 {
        self.chimes
    }

    /// The agent whose pill the pointer is on.
    #[cfg(test)]
    pub(crate) fn focus(&self) -> Option<&str> {
        self.focus.as_deref()
    }
}

impl DrawingApp {
    /// Every frame: the island, when it is on.
    pub(crate) fn island_ui(&mut self, ctx: &Context) {
        let now = crate::app::system::tray::unix_seconds();
        if crate::app::system::tray::alerts_paused(self.prefs.pause_alerts_until, now) {
            self.island = Island::default();
            return;
        }
        if self.prefs.pause_alerts_until.is_some() {
            self.prefs.pause_alerts_until = None;
        }
        if !self.prefs.island {
            self.island = Island::default();
            return;
        }
        let time = ctx.input(|input| input.time);
        let rows: Vec<AgentRow> = self.agent_rows().into_iter().filter(|row| row.state != AgentState::Ended).collect();
        let asks: Vec<String> = self.person_asks.borrow().iter().map(|ask| ask.chat.clone()).collect();
        let outside = self.outside_pills();
        let mood = mood(rows.iter().map(|row| &row.state).chain(outside.iter().map(|pill| &pill.state)));
        let pinned = !asks.is_empty();
        // the first time it shows, the wizard makes its entrance (`entry.rs`) with the panel open
        let mut tones: Vec<crate::voice::Sound> = Vec::new();
        if !self.island.greeted && !cfg!(test) {
            (self.island.greeted, self.island.greeting) = (true, Some(time));
            self.island.machine.launch();
            tones.push(sounds::GREET);
        }
        if self.island.greeting.is_some_and(|start| (time - start) as f32 >= entry::END) {
            self.island.machine.greeted(time);
        }
        // the monitor's work area bounds the conversation's size
        let work = if ctx.embed_viewports() {
            ctx.content_rect()
        } else {
            let anchor = self.island_anchor(ctx);
            crate::app::chat::mini::screen_work(ctx, Pos2::new(anchor[0], anchor[1]))
        };
        let chat = chat_size(self.prefs.island_chat, self.island.chat_big, work);
        self.island.floating = floats(self.island.anchor, work);
        self.island.cap = window_cap(chat, chat_most(work), self.island.resizing);
        let island = &mut self.island;
        let busy = matches!(mood, Mood::Needs(_) | Mood::Working(_));
        if busy && !island.was_busy {
            island.machine.reveal(time);
        }
        if island.was_busy && mood == Mood::Ready {
            island.celebrate_until = time + 6.0;
            tones.push(sounds::FINISH);
        }
        island.was_busy = busy;
        let asked_before = std::mem::replace(&mut island.asked, asks.len());
        if asks.len() < asked_before {
            tones.push(sounds::APPROVE);
        }
        if (pinned || island.form.is_some()) && island.machine.stage() != fsm::Stage::Home {
            island.machine.force_home(time);
        }
        island.machine.step(time, island.hovered || island.over, island.holding || pinned || island.form.is_some(), 15.0, false);
        let was_open = island.open;
        let stage = island.machine.stage();
        let open = matches!(stage, fsm::Stage::Home | fsm::Stage::Greeting);
        if was_open && !open {
            island.tab = Tab::Home;
            island.chat_big = false;
        }
        island.open = open;
        let shape = match stage {
            fsm::Stage::Hidden => Shape::Closed,
            fsm::Stage::Petit => Shape::Compact,
            fsm::Stage::Home | fsm::Stage::Greeting => Shape::Open,
        };
        match (island.last_shape.replace(shape), shape) {
            (Some(Shape::Closed), Shape::Compact) => tones.push(sounds::PEEK),
            (Some(Shape::Open), Shape::Compact | Shape::Closed) => tones.push(sounds::CLOSE),
            (Some(Shape::Closed | Shape::Compact), Shape::Open) => tones.push(if asks.len() > asked_before { sounds::ASK } else { sounds::OPEN }),
            (Some(Shape::Open), Shape::Open) if asks.len() > asked_before => tones.push(sounds::ASK),
            _ => {}
        }
        let (aim, radius) = target(shape, island.tab, pinned, island.tall, island.body, chat);
        // it grows on Coucou's spring (it overshoots a little and settles) and shrinks along its curve in 340 ms, with no
        // overshoot (`spring.rs`); pulled by an edge it follows the pointer at once
        if island.resizing {
            island.geometry.jump(aim, radius);
        } else {
            island.geometry.aim(aim, radius, time);
            island.geometry.step(ctx.input(|input| input.stable_dt).min(0.05), time);
        }
        // never past its window, which would cut it
        (island.anim, island.radius) = (island.geometry.size().min(island.cap), island.geometry.corner());
        if island.geometry.moving() {
            ctx.request_repaint();
        }
        let view = self.island_view(rows, asks, mood, time);
        let island = &mut self.island;
        island.tall = home::tall(view.activity.as_ref());
        if view.failures > std::mem::replace(&mut island.failures, view.failures) {
            tones.push(sounds::ERROR);
        }
        if view.pokes > std::mem::replace(&mut island.pokes, view.pokes) {
            tones.push(if view.dizzy { sounds::DIZZY } else { sounds::POKE });
        }
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
        let moving = busy || time < island.celebrate_until || open || island.hovered;
        if moving {
            ctx.request_repaint_after(std::time::Duration::from_millis(33));
        }
        if cfg!(windows) && !ctx.embed_viewports() {
            ctx.request_repaint_after(std::time::Duration::from_millis(if self.island.pointer.is_some() { 33 } else { 250 }));
        }
        // the clicks of the buttons; the pointer over a pill makes no sound, as in Coucou
        if out.tab.is_some() || out.open_agent.is_some() || out.talk.is_some() || out.voice || out.close || out.big || out.sound {
            tones.push(sounds::BLIP);
        }
        for tone in tones {
            self.island_chime(tone, time);
        }
        self.island_apply(ctx, out, time);
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
        assert_eq!(target(Shape::Closed, Tab::Chat, false, false, 0.0, chat), (Vec2::new(80.0, 24.0), 10.0));
        assert_eq!(target(Shape::Compact, Tab::Chat, false, false, 0.0, chat), (Vec2::new(288.0, 32.0), 14.0));
        assert_eq!(target(Shape::Open, Tab::Home, false, false, 0.0, chat), (Vec2::new(640.0, 160.0), 22.0));
        assert_eq!(target(Shape::Open, Tab::Chat, false, false, 0.0, Vec2::new(900.0, 500.0)).0, Vec2::new(900.0, 500.0), "the conversation: its own size");
        // a question takes the island over, as tall as its card (within limits)
        assert_eq!(target(Shape::Open, Tab::Home, true, false, 500.0, chat).0.y, 400.0);
        assert_eq!(target(Shape::Open, Tab::Home, true, false, 10.0, chat).0.y, 160.0);
        // an edit in progress makes Home taller, for its change
        assert_eq!(target(Shape::Open, Tab::Home, false, true, 0.0, chat).0.y, 290.0);
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
    fn each_agent_has_its_colour() {
        assert_eq!(agent_color("claude", "a"), Color32::from_rgb(0xd9, 0x77, 0x57));
        assert_eq!(agent_color("codex", "a"), Color32::from_rgb(0x2d, 0xd4, 0xbf));
        assert_eq!(agent_color("outro", "x"), agent_color("outro", "x"), "the same every time");
    }
}
