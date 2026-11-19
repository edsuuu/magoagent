//! The island: the Maestro as a black notch glued to the top edge of the screen, over every other window.
//! Closed it is small (the wizard at the left, a mini wizard per agent at the right, in the
//! agent's colour); with the pointer resting on it, or a question waiting, it grows into a wide panel: Home (the agent
//! in focus and a grid of the agents), the conversation, the new chat (a workspace of its own, `actions.rs`) and the
//! questions and permissions. It is dragged anywhere on the screen, another monitor too, and sticks to the top edge
//! when let go near it; the place is kept (`Prefs.island_at`).

use crate::app::mascot::mago::MagoState;
use crate::app::ui::agents::{AgentRow, AgentState};
use crate::prelude::*;

mod actions;
mod ask;
mod cursor;
mod done;
mod entry;
pub(super) mod feed;
mod forms;
mod fsm;
mod github;
#[cfg(target_os = "linux")]
pub(crate) mod gnome;
pub(crate) mod home;
mod icons;
mod integrations;
mod keymap;
mod keys;
mod log;
mod luna;
#[cfg(target_os = "macos")]
pub(crate) mod macos;
mod marquee;
pub(crate) mod minis;
pub(crate) mod music;
pub(crate) mod paint;
pub(crate) mod picture;
pub(crate) mod pill;
mod prompt;
pub(crate) mod replay;
mod session;
mod sounds;
mod spring;
mod stage;
mod window;
#[cfg(target_os = "linux")]
pub(crate) mod x11;
pub(crate) use window::*;

/// The camera notch of the screen `at` is on, in screen points (macOS; none elsewhere).
pub(crate) fn notch_at(at: Pos2) -> Option<Rect> {
    #[cfg(target_os = "macos")]
    return macos::notch_at(at);
    #[cfg(not(target_os = "macos"))]
    {
        let _ = at;
        None
    }
}

/// The sizes of the island: the closed one, the open one and their corners.
// about 15 each with two minis
pub(crate) const COMPACT_W: f32 = 188.0;
/// How far the middle of the wizard (30 wide) is from the compact island's left side: 12 of room to the edge, as the
/// minis have at the right.
pub(crate) const COMPACT_SIDE: f32 = 27.0;
/// The same for the minis' grid at the right, which keeps 12 of room to the edge.
pub(crate) const MINIS_SIDE: f32 = 27.0;
pub(crate) const COMPACT_H: f32 = 38.5;
/// Standing up on a side of the screen, the compact island is this tall (`COMPACT_H` wide): the wizard at the top, the
/// minis at the bottom.
pub(crate) const COMPACT_TALL: f32 = 128.0;
/// The same while a song shows: its cover and its bars go between the wizard and the minis.
pub(crate) const COMPACT_TALL_SONG: f32 = 176.0;
pub(crate) const EXPANDED_W: f32 = 760.0;
pub(crate) const CLOSED_W: f32 = 80.0;
pub(crate) const CLOSED_H: f32 = 24.0;
const CLOSED_CORNER: f32 = 10.0;
const COMPACT_CORNER: f32 = 18.0;
/// Around a camera notch, the wing each side of it: closed, the wizard's face (with a song, its cover beside her and
/// the bars on the other side); compact, the wizard and the minis (with a song or the voice, the cover and the bars
/// split by the notch too).
const CLOSED_WING: f32 = 30.0;
const CLOSED_WING_SONG: f32 = 48.0;
const COMPACT_WING: f32 = 54.0;
const COMPACT_WING_WIDE: f32 = 104.0;
/// Open around a notch, its header is the notch's row: each wing holds the tabs at the left (seven at most, 35 apart
/// from 14 in) or the buttons at the right.
const HEADER_WING: f32 = 14.0 + 7.0 * 35.0 + 6.0;
/// The conversation at least this big, in points.
const CHAT_MIN: (f32, f32) = (420.0, 300.0);
const EXPANDED_CORNER: f32 = 22.0;
/// Home's height (the reference's 160), and the conversation's.
const HOME_H: f32 = 160.0;
/// Início's height: the summary and the list of what happened, a few lines of it.
const FEED_H: f32 = 270.0;
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
    /// Início: the summary of every tab, what needs the person first.
    #[default]
    Home,
    Chats,
    Chat,
    New,
    /// Claude Code: the app's agents and chats running it, and the sessions outside (Claude Desktop, another
    /// terminal): the hooks' pills.
    Claude,
    GitHub,
    /// Spotify.
    Music,
}

impl Tab {
    /// The page a pill lives on: the sessions outside the app on Claude Code's, GitHub's and Spotify's on theirs, the
    /// other integrations (WhatsApp) on Início's, and the app's own agents and chats on Chats.
    pub(crate) fn of(id: &str) -> Tab {
        if id.starts_with(crate::app::maestro::outside::PILL_PREFIX) {
            return Tab::Claude;
        }
        match id.strip_prefix(crate::app::integrations::ITEM_PREFIX) {
            Some("github") => Tab::GitHub,
            Some("spotify") => Tab::Music,
            Some(_) => Tab::Home,
            None => Tab::Chats,
        }
    }

    /// Whether this page shows `line`: its own, and Claude Code's the app's agents and chats running it too (they live
    /// on Chats).
    pub(super) fn shows(self, line: &Line) -> bool {
        Tab::of(&line.id) == self || (self == Tab::Claude && line.kind.starts_with("Claude Code"))
    }

    /// The grid this page shows the focus of: its own, the conversation's and the new chat's the agents'.
    pub(super) fn grid(self) -> Tab {
        match self {
            Tab::Chat | Tab::New => Tab::Chats,
            tab => tab,
        }
    }

    /// The pages the MCP and a test scene open by name (`MAGOAGENT_ISLAND_OPEN`).
    pub(crate) fn named(name: &str) -> Option<Tab> {
        Some(match name {
            "home" | "inicio" | "início" => Tab::Home,
            "chats" | "conversas" => Tab::Chats,
            "chat" => Tab::Chat,
            "new" | "novo" => Tab::New,
            "claude" | "claude_code" => Tab::Claude,
            "github" => Tab::GitHub,
            "spotify" | "music" | "musica" | "música" => Tab::Music,
            _ => return None,
        })
    }
}

/// What the island shows of itself: the closed notch (the wizard's face), the compact one (the wizard and a mini wizard
/// per agent, while the pointer is about) or the open panel.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) enum Shape {
    Closed,
    Compact,
    Open,
}

impl Shape {
    /// The compact and open island run at the rate asked in Ajustes while the person is on them, as the reference's
    /// requestAnimationFrame; left alone, at what they show asks for (`update::AMBIENT`).
    fn animates_every_frame(self) -> bool {
        self != Shape::Closed
    }
}

/// When the app's next frame comes for the island `shape`. While the person is on the compact or open one (`touched`),
/// at the rate asked in Ajustes (the window has no vsync to hold it back: a bare `request_repaint` ran at 230 a second,
/// 55% of a core). Left alone, what it shows asks for its own frames (the wizard 15 a second at rest, the springs every
/// frame, the rest `update::AMBIENT`), and while it `moves` the app comes at the ambient rate; each of these frames paints
/// the whole app. Drawing itself (`replaying`, `replay.rs`), what moves asks for the island's window alone: the app comes
/// for what changes, and when the stage gives way by the clock (`due`, in seconds from now).
fn next_frame(shape: Shape, touched: bool, moving: bool, replaying: bool, due: Option<f64>) -> Option<std::time::Duration> {
    if shape.animates_every_frame() && touched {
        Some(crate::app::update::frame_spacing())
    } else if replaying {
        due.map(|due| std::time::Duration::from_secs_f64(due.max(0.0)))
    } else {
        moving.then_some(crate::app::update::AMBIENT)
    }
}

/// The closed or compact island's `size` standing up on a side of the screen: the notch turned, the compact one
/// `COMPACT_TALL` tall (`COMPACT_TALL_SONG` while a `song` shows).
pub(crate) fn upright(shape: Shape, size: Vec2, song: bool) -> Vec2 {
    match shape {
        Shape::Compact => Vec2::new(COMPACT_H, if song { COMPACT_TALL_SONG } else { COMPACT_TALL }),
        _ => Vec2::new(size.y, size.x),
    }
}

/// The island's size and corner for its `shape`; open, for what `tab` shows (`pinned`: a question waits, whose card is
/// `body` tall; `tall`: an edit's change needs room on Home; `chat`: the conversation's size).
pub(crate) fn target(shape: Shape, tab: Tab, pinned: bool, settings: bool, tall: bool, body: f32, chat: Vec2) -> (Vec2, f32) {
    match shape {
        Shape::Closed => return (Vec2::new(CLOSED_W, CLOSED_H), CLOSED_CORNER),
        Shape::Compact => return (Vec2::new(COMPACT_W, COMPACT_H), COMPACT_CORNER),
        Shape::Open => {}
    }
    match (pinned, tab) {
        (true, _) => (Vec2::new(EXPANDED_W, 42.0 + body + 10.0), EXPANDED_CORNER),
        // its settings are a short page of their own, whatever tab is under them
        _ if settings => (Vec2::new(EXPANDED_W, HOME_H), EXPANDED_CORNER),
        (_, Tab::Home) => (Vec2::new(EXPANDED_W, FEED_H), EXPANDED_CORNER),
        (_, Tab::GitHub) => (Vec2::new(EXPANDED_W, github::HEIGHT), EXPANDED_CORNER),
        (_, Tab::Music) => (Vec2::new(EXPANDED_W, music::HEIGHT), EXPANDED_CORNER),
        (_, Tab::Chats | Tab::Claude) => (Vec2::new(EXPANDED_W, if tall { home::EDIT_H } else { HOME_H }), EXPANDED_CORNER),
        (_, Tab::Chat) => (chat, EXPANDED_CORNER),
        (_, Tab::New) => (Vec2::new(EXPANDED_W, HOME_H), EXPANDED_CORNER),
    }
}

/// The island of `size` on a screen whose camera notch is `notch` big: it grows out of the notch, which it hides, as
/// black as it is. Closed and compact it is the notch with a wing each side, as tall as it, wider while a `song` shows or
/// the `voice` talks; open, its header is the notch's row, so it is as tall as anywhere and at least wide enough for the
/// tabs and the buttons each side of the camera.
pub(crate) fn around_notch(shape: Shape, size: Vec2, notch: Vec2, song: bool, voice: bool) -> Vec2 {
    let wings = |wing: f32| notch.x + 2.0 * wing;
    match shape {
        Shape::Closed => Vec2::new(wings(if song { CLOSED_WING_SONG } else { CLOSED_WING }), notch.y),
        Shape::Compact => Vec2::new(wings(if song || voice { COMPACT_WING_WIDE } else { COMPACT_WING }), notch.y),
        // ponytail: a conversation narrower than this widens around a notch; split the header by its real width if it bothers
        Shape::Open => Vec2::new(size.x.max(wings(HEADER_WING)), size.y),
    }
}

/// What shows of each of the island's contents, each on its own transition from the reference's style.css: the closed
/// face (`#island-closed`), the compact one's (`#mini-grid`), the open panel (`#content`) and its page (`.view`, from
/// nothing again whenever the page changes, growing from 97% on a curve that overshoots a little).
pub(crate) struct Layers {
    pub(crate) face: spring::Fade,
    pub(crate) compact: spring::Fade,
    pub(crate) open: spring::Fade,
    pub(crate) page: spring::Fade,
    shown: Option<(Shape, Tab, bool, bool)>,
    greeting: bool,
}

impl Default for Layers {
    fn default() -> Self {
        use spring::{Fade, Way, ease_in, ease_in_out, ease_out};
        let way = |seconds: f64, delay: f64, ease: fn(f32) -> f32| Way { seconds, delay, ease };
        Layers {
            face: Fade::new(way(0.3, 0.12, ease_out), way(0.18, 0.0, ease_in)),
            compact: Fade::new(way(0.25, 0.0, ease_in_out), way(0.25, 0.0, ease_in_out)),
            open: Fade::new(way(0.2, 0.0, ease_in_out), way(0.2, 0.0, ease_in_out)),
            page: Fade::new(way(0.3, 0.16, ease_out), way(0.16, 0.0, ease_in)),
            shown: None,
            greeting: false,
        }
    }
}

impl Layers {
    /// The island is `shape`, on the page of `tab` (or its settings, or a question), or plays the entrance (`greeting`):
    /// each content turns on or off.
    pub(crate) fn show(&mut self, shape: Shape, page: (Tab, bool, bool), greeting: bool, time: f64) {
        if std::mem::replace(&mut self.greeting, greeting) != greeting || greeting {
            self.page.jump(greeting);
            // its end lets whatever page follows come in as a new one
            self.shown = None;
        }
        self.face.set(shape == Shape::Closed, time);
        self.compact.set(shape == Shape::Compact, time);
        self.open.set(shape == Shape::Open, time);
        let shown = (shape, page.0, page.1, page.2);
        if self.shown.replace(shown) != Some(shown) {
            if shape == Shape::Open && !greeting {
                self.page.restart(time)
            } else if shape != Shape::Open {
                self.page.set(false, time)
            }
        }
    }

    /// (face, compact, open, page) at `time`.
    pub(crate) fn values(&self, time: f64) -> [f32; 4] {
        [self.face.value(time), self.compact.value(time), self.open.value(time), self.page.value(time)]
    }

    /// How big the page is drawn: 0.97 to 1 on `cubic-bezier(.3, 1.2, .4, 1)`, with its opacity's delay.
    pub(crate) fn page_scale(&self, time: f64) -> f32 {
        0.97 + 0.03 * spring::cubic_bezier(0.3, 1.2, 0.4, 1.0, self.page.progress(time))
    }

    pub(crate) fn moving(&self, time: f64) -> bool {
        [&self.face, &self.compact, &self.open, &self.page].iter().any(|fade| fade.moving(time))
    }
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

/// The least the short conversation (an agent's log and its box) is pulled to.
const TALK_MIN: (f32, f32) = (420.0, 220.0);

/// The short conversation's size: as the person pulled it, within the monitor's work area `work`; never pulled, it
/// grows with the `said` messages.
pub(crate) fn talk_size(saved: Option<[f32; 2]>, said: usize, work: Rect) -> Vec2 {
    let Some([width, height]) = saved else { return Vec2::new(EXPANDED_W, prompt::prompt_height(said)) };
    let most = chat_most(work);
    Vec2::new(width, height).clamp(Vec2::new(TALK_MIN.0, TALK_MIN.1).min(most), most)
}

/// One agent or chat of the grid.
#[derive(Clone)]
pub(super) struct Line {
    pub(super) id: String,
    pub(super) name: String,
    pub(super) detail: String,
    pub(super) state: AgentState,
    pub(super) mago: MagoState,
    pub(super) color: Color32,
    pub(super) chat: bool,
    /// A card of the canvas (a terminal or a chat there, this workspace's or another's): its card offers "No canvas".
    pub(super) canvas: bool,
    /// The dot on its corner (a session outside the app: `maestro/outside.rs`); the chats' come from their state.
    pub(super) badge: Option<crate::app::maestro::outside::Badge>,
    /// The program as its card names it ("Claude Code").
    pub(super) kind: String,
    /// When it last changed (ms; 0 unknown): Início's list says how long ago.
    pub(super) at: i64,
    /// It is stopped (a session outside whose turn ended and was seen): its mini sleeps.
    pub(super) asleep: bool,
}

#[derive(Default)]
pub(crate) struct Island {
    /// The island's clock-driven hidden/compact/open/greeting lifecycle.
    machine: fsm::Machine,
    pub(crate) tab: Tab,
    /// Whether the header gear opened the island's own settings page.
    settings: bool,
    /// The agent the Home card shows when the pointer is on its grid.
    focus: Option<String>,
    /// How many questions there were last frame.
    asked: usize,
    /// Something was working or waiting last frame: the end of it is news.
    was_busy: bool,
    /// The agents and chats at work last frame (`done.rs`).
    working: std::collections::HashSet<String>,
    celebrate_until: f64,
    /// An agent finished since the island was last open: Luna wears a check until the person opens it (`done.rs`).
    finished: bool,
    /// The island as drawn: its size and corner follow the target, easing.
    anim: Vec2,
    radius: f32,
    /// The room the window has for the island (`window_cap`): the island is drawn inside it.
    cap: Vec2,
    /// A button of the mouse is down over it.
    holding: bool,
    /// A field of the island has the keyboard (`Out::typing`): it holds the island open.
    typing: bool,
    /// Its window had the system's focus last frame (`Out::focused`): losing it folds the open island.
    focused: bool,
    /// The cursor, read on the whole screen, is over it; `None` where the system gives no cursor (only egui's pointer).
    pub(crate) over: Option<bool>,
    /// Whether the window takes the cursor (it passes through outside the island), as last sent.
    captured: Option<bool>,
    /// Whether the panel was open last frame.
    open: bool,
    /// The conversation at the size that is kept, big (70% of the monitor), and being pulled by an edge now.
    chat_big: bool,
    /// The whole mini chat on the Chat tab (its ⤢) instead of the compact conversation, until the island closes.
    pub(crate) chat_full: bool,
    resizing: bool,
    /// The conversation's size while an edge is pulled: it goes to the prefs (and their file) once the edge is let go.
    pulled: Option<[f32; 2]>,
    /// A conversation asked for from outside the island ("Abrir em miniatura"): opened on its Chat page next frame.
    pub(crate) chat_asked: bool,
    /// A session outside the app on the conversation page, as its Abrir left it (`session.rs`).
    pub(crate) session: Option<session::SessionView>,
    /// Where the menus open from the island were last frame (`window::menus`): they take the clicks too.
    menus: Option<Rect>,
    /// Where the eyes look (window coordinates), the sounds playing, and what the last frame was, to tell what changed.
    pointer: Option<Pos2>,
    /// Where the eyes looked when the last frame was asked for: a cursor at rest asks for few.
    looked: Option<Pos2>,
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
    /// The asks folded away by "Depois — deixar esperando" (or the close): they wait without holding it open.
    folded: Vec<String>,
    /// Where it hangs from (centre x, top) and where its window was put for the size it was built at.
    anchor: Option<[f32; 2]>,
    origin: Pos2,
    built: Vec2,
    /// Where the window was seen after the person moved it, and since when: let go once it stays; and the cursor then
    /// (points), on the island being dragged: its monitor is the one it sticks to.
    moved: Option<(Pos2, f64, Option<Pos2>)>,
    /// X11: the window is the island's size since a drag began (the size it was), until the island grows once it is let
    /// go. In its room the system stops the window at the top bar with the island still below it.
    slim: Option<Vec2>,
    /// The system's drag asked for, sent once the window has its drag size (`slim`) and is where it was put
    /// (`window::drag_ready`), and since when it waits.
    drag_next: bool,
    drag_since: Option<f64>,
    /// Where the system had the window last frame (screen points).
    placed: Option<Rect>,
    /// Início's lines hidden by their ×, with what they were at (`feed::signature`): back once that changes.
    // ponytail: only while the app runs, the list starts whole again; keep it in the prefs if the person misses it
    hidden: std::collections::HashMap<String, u64>,
    /// Início's lines as last built, with what each is at: what the MCP's "Ocultar tudo" hides.
    listed: Vec<(String, u64)>,
    /// The GitHub tab's filter, the pull request picked and what is written there.
    github: github::Page,
    /// The Spotify tab's list, its search and the volume dragged.
    music: music::Page,
    pub(crate) form: Option<NewForm>,
    /// The window on its way onto the screen without Windows' border (`plain::reveal`), and the pass it was last shown in.
    plain: u8,
    /// The island's room on the screen, for the thread that reads the cursor (`cursor::watch`).
    #[cfg(any(windows, target_os = "macos"))]
    room: Option<std::sync::Arc<std::sync::Mutex<Option<Rect>>>>,
    pass: u64,
    /// When the conversations of the agents at work were last read, and what the card in focus tells (`stage.rs`): the
    /// step it is on, since when, and the glimpse of the file read.
    polled: f64,
    stage_key: String,
    stage_since: f64,
    glimpse_for: Option<PathBuf>,
    glimpse: Vec<(usize, String)>,
    /// What shows of each content (`Layers`), and the island's one wizard: its place (from the island's top left) and
    /// size on the spring, after the page that shows (`paint::Aim`); not placed before the first frame.
    layers: Layers,
    bot: [spring::Spring; 3],
    bot_placed: bool,
    /// The entrance (`entry.rs`) started at this time; played once, when the island first shows.
    greeting: Option<f64>,
    greeted: bool,
    /// How many pokes of the wizard and failures were counted, for their sounds; the page is tall (an edit is on).
    pokes: u32,
    failures: usize,
    tall: bool,
    /// The edge of its monitor it is on (`window::Dock`), and the camera notch it hangs around there (its size).
    dock: window::Dock,
    notch: Option<Vec2>,
    /// A place asked in Ajustes or by the MCP (`window::Place`), taken on the next frame.
    place_ask: Option<window::Place>,
    /// A picture of the conversation on the whole screen (`picture.rs`).
    picture: Option<picture::Shown>,
    /// Nothing moves it but what it shows (`at_rest`): the frame the app draws is taken down (`replay.rs`), the window
    /// draws it again by itself until the app draws again (`replaying`, as last drawn).
    at_rest: bool,
    replay: std::sync::Arc<std::sync::Mutex<replay::Replay>>,
    replaying: bool,
    /// The screen's scale and the window's place, for the eyes of what the window draws again by itself.
    eyes: Option<(f32, Pos2)>,
}

/// The form of a new chat.
#[derive(Clone, Debug, Default, PartialEq)]
pub(crate) struct NewForm {
    /// The name of the workspace it makes (empty: the folder's).
    pub(crate) name: String,
    pub(crate) folder: String,
    /// In a git worktree of its own (`maestro/environments.rs`), so the repository's changes stay as they are.
    pub(crate) worktree: bool,
    /// What the agent should do: the chat's first message.
    pub(crate) prompt: String,
    /// The recent projects offered (`Prefs.island_projects`), and whether their list shows in place of the text box.
    pub(crate) projects: Vec<String>,
    pub(crate) choosing: bool,
    /// Why the last send did not start the chat.
    pub(crate) failed: Option<String>,
}

impl Island {
    /// Compact at once: the app took what was pressed on it (Ajustes, a page).
    pub(crate) fn fold_now(&mut self, time: f64) {
        self.machine.force_petit(time);
    }

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
    pub(crate) fn settings_open(&self) -> bool {
        self.settings
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

    /// Hangs it from `anchor` (screen points): on a side of the screen it stands up.
    #[cfg(test)]
    pub(crate) fn hang_from(&mut self, anchor: [f32; 2]) {
        self.anchor = Some(anchor);
    }

    /// Its window draws it again by itself between the app's frames (`replay.rs`): that window is cleared see-through.
    pub(crate) fn draws_itself(&self) -> bool {
        self.replaying
    }

    /// Nothing moves it but what it shows, as last drawn (`replay.rs`).
    #[cfg(test)]
    pub(crate) fn at_rest(&self) -> bool {
        self.at_rest
    }

    /// The island at rest as the app last drew it, which its window draws again by itself.
    #[cfg(test)]
    pub(crate) fn taken(&self) -> std::sync::MutexGuard<'_, replay::Replay> {
        self.replay.lock().unwrap_or_else(std::sync::PoisonError::into_inner)
    }
}

impl DrawingApp {
    /// Every frame: the island, when it is on.
    pub(crate) fn island_ui(&mut self, ctx: &Context) {
        let now = crate::app::system::tray::unix_seconds();
        if crate::app::system::tray::alerts_paused(self.prefs.pause_alerts_until, now) {
            self.island = Island::default();
            #[cfg(target_os = "linux")]
            x11::forget();
            return;
        }
        if self.prefs.pause_alerts_until.is_some() {
            self.prefs.pause_alerts_until = None;
        }
        // turned off, or the welcome still on: the island waits until the person is through it (its entrance plays then)
        if !self.prefs.island || self.welcome.is_some() {
            self.island = Island::default();
            #[cfg(target_os = "linux")]
            x11::forget();
            return;
        }
        let time = ctx.input(|input| input.time);
        let rows: Vec<AgentRow> = self.agent_rows().into_iter().filter(|row| row.state != AgentState::Ended).collect();
        let asks: Vec<String> = self.person_asks.borrow().iter().map(|ask| ask.chat.clone()).collect();
        let outside = self.outside_pills();
        let mood = mood(rows.iter().map(|row| &row.state).chain(outside.iter().map(|pill| &pill.state)));
        // an ask holds the island open on its card until it is answered or folded ("Depois"): folded, it keeps waiting
        self.island.folded.retain(|chat| asks.contains(chat));
        let pinned = asks.iter().any(|chat| !self.island.folded.contains(chat));
        // the first time it shows, the wizard makes its entrance (`entry.rs`) with the panel open
        let mut tones: Vec<crate::voice::Sound> = Vec::new();
        if !self.island.greeted && !cfg!(test) {
            (self.island.greeted, self.island.greeting) = (true, Some(time));
            self.island.machine.launch();
            tones.push(sounds::GREET);
            // a test scene opens it on a page, to be seen without the mouse: MAGOAGENT_ISLAND_OPEN=home|chats|chat|new|claude|github|spotify|settings
            let page = std::env::var_os("MAGOAGENT_SCENE").and_then(|_| std::env::var("MAGOAGENT_ISLAND_OPEN").ok());
            if let Some(tab) = page.as_deref().and_then(Tab::named) {
                self.island.tab = tab;
                self.island.form = (tab == Tab::New).then(|| self.island_form());
                self.island.machine.force_home(time);
            }
            self.island.settings = page.as_deref() == Some("settings");
            if self.island.settings {
                self.island.machine.force_home(time);
            }
        }
        if std::mem::take(&mut self.island.chat_asked) {
            // "Abrir em miniatura" asks for the mini chat itself
            self.island.chat_full = true;
            self.island.tab = Tab::Chat;
            self.island.settings = false;
            self.island.machine.force_home(time);
        }
        if self.island.greeting.is_some_and(|start| (time - start) as f32 >= entry::END) {
            self.island.machine.greeted(time);
        }
        // the monitor's work area bounds the conversation's size
        let (work, anchor) = if ctx.embed_viewports() {
            (ctx.content_rect(), None)
        } else {
            let anchor = self.island_anchor(ctx);
            (crate::app::chat::mini::screen_work(ctx, window::probe(anchor)), Some(anchor))
        };
        // the compact conversation grows with what was said (`chatPromptHeight`); the whole mini chat has its own size
        let chat = if self.island.chat_full {
            chat_size(self.island.pulled.or(self.prefs.island_chat), self.island.chat_big, work)
        } else {
            let said = match &self.island.session {
                Some(session) => session.said(),
                None => self.chat_agent.as_ref().and_then(|id| self.transcripts.get(id)).map_or(0, |transcript| log::said(&transcript.items).count()),
            };
            // the size the person pulled it to is the open conversation's; with none open, the page is its plain small
            // self
            let open = self.island.session.is_some() || self.chat_agent.as_ref().is_some_and(|id| self.chats.iter().any(|chat| &chat.id == id));
            talk_size(self.island.pulled.or(self.prefs.island_talk).filter(|_| open), said, work)
        };
        self.island.dock = self.island.anchor.map_or(window::Dock::Top, |anchor| window::Dock::of(anchor, work));
        // on the top edge it hangs from the middle of a notch there (`window::to_notch`)
        self.island.notch = anchor
            .filter(|_| self.island.dock == window::Dock::Top)
            .and_then(|anchor| notch_at(window::probe(anchor)).filter(|notch| (notch.center().x - anchor[0]).abs() < 1.0))
            .map(|notch| notch.size());
        self.island.cap = window_cap(chat, chat_most(work), self.island.resizing);
        // GitHub's alerts, as the reference sounds them: a review asked for and checks that passed (a failure is the error of
        // its red wizard, below); one that waited more than a few seconds (the island was off) is dropped
        if let Some((alert, at)) = self.integrations.github.ring.take()
            && now_ms() - at < 10_000
        {
            match alert {
                crate::app::integrations::github::Alert::ReviewRequested(_) => tones.push(sounds::ASK),
                crate::app::integrations::github::Alert::CheckPassed(_) => tones.push(sounds::FINISH),
                _ => {}
            }
        }
        let close_after = self.prefs.island_close_after as f64;
        // a page whose integration was turned off (Spotify's, GitHub's) gives its place to Início
        let music_on = self.spotify_sources() != crate::app::integrations::spotify::Sources::default();
        let github_on = self.prefs.integrations.github;
        let song = self.integrations.spotify.shown(time).is_some();
        let voice = self.voice.on;
        let island = &mut self.island;
        if (island.tab == Tab::Music && !music_on) || (island.tab == Tab::GitHub && !github_on) {
            island.tab = Tab::Home;
        }
        let busy = matches!(mood, Mood::Needs(_) | Mood::Working(_));
        if busy && !island.was_busy {
            island.machine.reveal(time);
        }
        // the end is celebrated quietly: no chime
        if island.was_busy && mood == Mood::Ready {
            island.celebrate_until = time + 6.0;
        }
        island.was_busy = busy;
        // an agent done marks Luna with a check until the island is opened ("poderia só mudar a Luna com um check, aí eu
        // clico e vejo"); one that stopped on the person opens it, as a question does, and it folds after "Fechar
        // sozinho" as usual
        island.finished |= done::turn_done(&island.working, &rows);
        if done::turn_waits(&island.working, &rows) {
            island.machine.force_home(time);
        }
        island.working = rows.iter().filter(|row| matches!(row.state, AgentState::Working(_))).map(|row| row.id.clone()).collect();
        let asked_before = std::mem::replace(&mut island.asked, asks.len());
        if asks.len() < asked_before {
            tones.push(sounds::APPROVE);
        }
        if (pinned || island.form.is_some()) && island.machine.stage() != fsm::Stage::Home {
            island.machine.force_home(time);
        }
        island.machine.set_compact_after(self.prefs.island_compact_after);
        island.machine.step(time, island.hovered, island.holding || island.typing || pinned || island.form.is_some(), close_after, false);
        let was_open = island.open;
        let stage = island.machine.stage();
        let open = matches!(stage, fsm::Stage::Home | fsm::Stage::Greeting);
        if was_open && !open {
            island.tab = Tab::Home;
            island.chat_big = false;
            island.chat_full = false;
            island.session = None;
        }
        island.open = open;
        // open, what finished is in sight: the check goes
        if stage == fsm::Stage::Home {
            island.finished = false;
        }
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
        let (mut aim, radius) = target(shape, island.tab, !asks.is_empty(), island.settings, island.tall, island.body, chat);
        // on a side of the screen the closed and compact ones stand up; open, it is as anywhere
        if island.dock.upright() && shape != Shape::Open {
            aim = upright(shape, aim, song);
        }
        if let Some(notch) = island.notch {
            aim = around_notch(shape, aim, notch, song, voice);
        }
        // it grows on a spring (it overshoots a little and settles) and shrinks along its curve in 340 ms, with no
        // overshoot (`spring.rs`); pulled by an edge it follows the pointer at once
        if island.resizing {
            island.geometry.jump(aim, radius);
        } else {
            island.geometry.aim(aim, radius, time);
            island.geometry.step(ctx.input(|input| input.stable_dt).min(0.05), time);
        }
        // never past its window, which would cut it
        (island.anim, island.radius) = (island.geometry.size().min(island.cap), island.geometry.corner());
        island.layers.show(shape, (island.tab, island.settings, !asks.is_empty()), stage == fsm::Stage::Greeting, time);
        if island.geometry.moving() || island.layers.moving(time) {
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
        // nothing moves it but what it shows (Luna, the song's bars, the minis): the person is not on it, it does not
        // spring, fade or play the end, and nothing else is drawn with it (a menu, the call's bars, a cover fading in)
        let fading = crate::app::integrations::spotify::CoverFade::read(ctx).fading(time);
        island.at_rest = shape != Shape::Open
            && !island.geometry.moving()
            && !island.layers.moving(time)
            && island.bot.iter().all(spring::Spring::settled)
            && !(island.hovered || island.holding || island.typing || island.resizing || island.drag_next)
            && island.moved.is_none()
            && island.slim.is_none()
            && island.menus.is_none()
            && island.picture.is_none()
            && time >= island.celebrate_until
            && !voice
            && !fading
            && (!cfg!(windows) || island.plain >= 3);
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
        // the one wizard springs to where the page that shows put it; with none, the closed
        // island's spot (46, 16, 6) and not drawn
        let wanted = view.aim.get().map_or([46.0, 16.0, 6.0], |aim| [aim.at.x, aim.at.y, aim.size]);
        let delta = ctx.input(|input| input.stable_dt).min(0.05);
        for (spring, target) in island.bot.iter_mut().zip(wanted) {
            if !island.bot_placed {
                spring.value = target;
            }
            spring.target = target;
            spring.step(delta);
        }
        island.bot_placed = true;
        if island.bot.iter().any(|spring| !spring.settled()) {
            ctx.request_repaint();
        }
        // the window lets the cursor through outside the island, so egui never hears it left: the system's cursor wins
        island.hovered = island.over.unwrap_or(out.hovered);
        island.holding = out.holding;
        island.typing = out.typing;
        // its window lost the focus: a click elsewhere (another program, the app's window, the desktop). The open island
        // folds, as a menu does; a question waiting on the person, or a new agent's form half filled, keeps it open as
        // before ("se for pergunta, mantém como está")
        let blurred = std::mem::replace(&mut island.focused, out.focused) && !out.focused;
        if blurred && island.open && !pinned && island.form.is_none() && !island.holding {
            island.machine.force_petit(time);
            ctx.request_repaint();
        }
        if out.hovered && !was_open && !open {
            ctx.request_repaint_after(std::time::Duration::from_millis(100));
        }
        if (out.body - island.body).abs() > 1.0 {
            island.body = out.body;
            ctx.request_repaint();
        }
        // the wizards move while they work or wait; the fold after the pointer left comes by the clock
        // open or compact (and while the entrance plays) every frame, as the reference's requestAnimationFrame; closed, the
        // wizards move while they work or wait, and the fold after the pointer left comes by the clock
        let moving = busy || time < island.celebrate_until || island.hovered;
        let touched = island.hovered || island.typing || island.holding;
        if let Some(delay) = next_frame(shape, touched, moving, island.replaying, island.machine.due().map(|due| due - time)) {
            ctx.request_repaint_after(delay);
        }
        // the window lets the clicks through, so only the frames see the cursor: often while it moves (the eyes follow
        // it), seldom while it rests (`cursor::watch` wakes it when the cursor comes onto the island)
        if cfg!(any(windows, target_os = "macos")) && !ctx.embed_viewports() && !self.island.replaying {
            let moved = std::mem::replace(&mut self.island.looked, self.island.pointer) != self.island.pointer;
            ctx.request_repaint_after(std::time::Duration::from_millis(if moved { 33 } else { 250 }));
        }
        // the clicks of the buttons; the pointer over a pill makes no sound
        if out.tab.is_some()
            || out.open_agent.is_some()
            || out.canvas.is_some()
            || out.talk.is_some()
            || out.terminal.is_some()
            || out.voice
            || out.close
            || out.big
            || out.full
            || out.sound
            || out.picked
        {
            tones.push(sounds::BLIP);
        }
        for tone in tones {
            self.island_chime(tone, time);
        }
        self.island_apply(ctx, out, time);
        self.island_picture_window(ctx);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Drawing itself (`replay.rs`), the island at rest no longer brings the app's frames at the ambient rate, each of
    /// which painted the whole app: only when its stage gives way by the clock; the person on it brings them.
    #[test]
    fn drawing_itself_the_island_brings_the_apps_frames_only_by_the_clock() {
        let ambient = Some(crate::app::update::AMBIENT);
        assert_eq!(next_frame(Shape::Compact, false, true, false, Some(5.0)), ambient, "the app draws it: the ambient rate");
        assert_eq!(next_frame(Shape::Compact, false, true, true, Some(5.0)), Some(std::time::Duration::from_secs(5)), "drawing itself: when it folds");
        assert_eq!(next_frame(Shape::Closed, false, true, true, None), None, "drawing itself, nothing due: none");
        assert_eq!(next_frame(Shape::Compact, true, true, true, None), Some(crate::app::update::frame_spacing()), "on it: the rate in Ajustes");
        assert_eq!(next_frame(Shape::Closed, false, false, false, None), None, "nothing moves: none");
    }

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

    /// The pills and the cards show the session's folder, its program beside it (the card's kind); the app's agents in
    /// colours of their own, the same as in the lists.
    #[test]
    fn the_pills_show_each_agent_by_its_own_name_and_colour() {
        let mut app = DrawingApp::default();
        app.chats = vec![chat("a"), chat("b")];
        for (session, origin) in [("s1", ""), ("s2", "claude-desktop")] {
            let payload = json!({ "hook_event_name": "UserPromptSubmit", "session_id": session, "cwd": "/w/loja", "mago_origin": origin });
            app.take_hook(&format!("ext:{session}"), &payload, None);
        }
        let rows = app.agent_rows();
        let view = app.island_view(rows, vec![], Mood::Ready, 1.0);
        let line = |id: &str| view.lines.iter().find(|line| line.id == id).unwrap();
        // the two chats are both "Claude Code · loja", and both sessions are of a folder "loja": four names, no program
        // before any, none twice
        let names: std::collections::HashSet<&str> = view.lines.iter().map(|line| line.name.as_str()).collect();
        assert!(names.iter().all(|name| name.starts_with("loja")), "{names:?}");
        assert_eq!((names.len(), view.lines.len()), (4, 4), "{names:?}");
        assert_eq!(line("a").name, "loja", "the first keeps its own");
        // and colours: the chats' their own, the same as everywhere else
        assert_ne!(line("a").color, line("b").color);
        assert_eq!(line("a").color, app.agent_color("a"));
        // the program goes to the card's kind
        let (code, desktop) = (line("pill:code:/w/loja"), line("pill:desktop:/w/loja"));
        assert_eq!((code.kind.as_str(), desktop.kind.as_str()), ("Claude Code · fora do app", "Claude Desktop · fora do app"));
    }

    fn chat(id: &str) -> crate::app::chat::direct::Chat {
        crate::app::chat::direct::Chat {
            id: id.into(),
            agent: "claude".into(),
            name: "Claude Code · loja".into(),
            cwd: "C:/p".into(),
            session: None,
            mode: 0,
            model: String::new(),
            effort: String::new(),
            recruit: false,
            named: false,
            draft: false,
        }
    }

    /// A chat of the app is on the island as its band shows it, working, talking while it writes, and done when its
    /// answer ends.
    #[test]
    fn a_chat_of_the_app_is_on_the_island_as_its_band_shows_it() {
        use crate::transcript::{ChatItem, Transcript};
        let mut app = DrawingApp::default();
        app.chats = vec![chat("a")];
        let mut transcript = Transcript::new(PathBuf::new());
        transcript.items = vec![ChatItem::User("oi".into()), ChatItem::Agent("Já vejo".into())];
        app.transcripts.insert("a".into(), transcript);
        let mut row = app.agent_rows().into_iter().find(|row| row.id == "a").expect("the chat is one of the island's rows");
        assert_eq!(row.state, AgentState::Idle(None), "never answered");
        row.state = AgentState::Working(0.0);
        let view = app.island_view(vec![row], Vec::new(), Mood::Working(1), 1.0);
        assert_eq!(view.lines[0].mago, MagoState::Speaking, "writing its answer");
        assert_eq!(main_state(view.mood, false), MagoState::Working, "the island's own wizard works");
        // the answer ended: done, as a terminal's agent is when its turn ends
        app.now = 10.0;
        app.chat_ended.insert("a".into(), 9.0);
        let row = app.agent_rows().into_iter().find(|row| row.id == "a").unwrap();
        assert_eq!(row.state, AgentState::Idle(Some(9.0)));
        let view = app.island_view(vec![row], Vec::new(), Mood::Ready, 10.0);
        assert_eq!(view.lines[0].mago, MagoState::Done);
    }

    /// A turn that ends leaves the island as it was and quiet, Luna with a check until it is opened; one stopped by Esc
    /// gets none; one that stops on a question still opens it.
    #[test]
    fn a_turn_that_ends_puts_a_check_on_luna_quietly_until_the_island_is_opened() {
        let frame = |app: &mut DrawingApp, ctx: &Context, time: f64| {
            let raw = egui::RawInput { screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(1200.0, 800.0))), time: Some(time), ..Default::default() };
            ctx.run_ui(raw, |_| app.island_ui(ctx)).textures_delta.clear();
        };
        let island = || {
            let (ctx, mut app) = (Context::default(), DrawingApp::default());
            app.prefs.island = true;
            app.chats = vec![chat("a")];
            frame(&mut app, &ctx, 1.0);
            assert_eq!(app.island.machine.stage(), fsm::Stage::Hidden);
            // the chat was at work last frame
            app.island.working.insert("a".into());
            app.island.was_busy = true;
            (ctx, app)
        };
        for (ended, checked) in [(Some(4.0), true), (None, false)] {
            let (ctx, mut app) = island();
            // its answer ended (or Esc stopped it)
            if let Some(at) = ended {
                app.chat_ended.insert("a".into(), at);
            }
            let chimes = app.island.chimes();
            frame(&mut app, &ctx, 5.0);
            assert_eq!(app.island.machine.stage(), fsm::Stage::Hidden, "it does not open, ended at {ended:?}");
            assert_eq!(app.island.chimes(), chimes, "nor sound");
            assert_eq!(app.island.finished, checked, "ended at {ended:?}");
            assert!(app.island.working.is_empty(), "no one at work now");
        }
        // the check stays until the person opens the island, and goes once it is open
        let (ctx, mut app) = island();
        app.chat_ended.insert("a".into(), 4.0);
        frame(&mut app, &ctx, 5.0);
        frame(&mut app, &ctx, 90.0);
        assert!(app.island.finished && app.island.machine.stage() != fsm::Stage::Home);
        app.island.machine.click(91.0);
        frame(&mut app, &ctx, 91.0);
        assert!(!app.island.finished, "seen");
        // a question opens it, as before
        let (ctx, mut app) = island();
        let (reply, _answer) = std::sync::mpsc::channel();
        app.person_asks.borrow_mut().push(crate::app::chat::decide::PersonAsk::new("a".into(), &json!({ "tool_name": "Bash" }), reply));
        frame(&mut app, &ctx, 5.0);
        assert_eq!(app.island.machine.stage(), fsm::Stage::Home);
    }

    #[test]
    fn the_wizard_loops_at_every_frame_compact_or_open_but_not_closed() {
        assert!(Shape::Compact.animates_every_frame() && Shape::Open.animates_every_frame());
        assert!(!Shape::Closed.animates_every_frame());
    }

    #[test]
    fn the_short_conversation_grows_with_what_was_said_until_it_is_pulled() {
        let work = Rect::from_min_max(Pos2::ZERO, Pos2::new(1920.0, 1040.0));
        assert_eq!(talk_size(None, 0, work), Vec2::new(EXPANDED_W, 240.0));
        assert_eq!(talk_size(None, 9, work), Vec2::new(EXPANDED_W, 300.0));
        assert_eq!(talk_size(Some([900.0, 600.0]), 0, work), Vec2::new(900.0, 600.0), "pulled: its size, whatever was said");
        assert_eq!(talk_size(Some([100.0, 50.0]), 0, work), Vec2::new(420.0, 220.0), "never too small for its box");
        assert_eq!(talk_size(Some([5000.0, 5000.0]), 0, work), chat_most(work), "nor past the monitor");
    }

    #[test]
    fn on_a_side_of_the_screen_the_closed_and_compact_islands_stand_up() {
        assert_eq!(upright(Shape::Closed, Vec2::new(CLOSED_W, CLOSED_H), true), Vec2::new(CLOSED_H, CLOSED_W));
        assert_eq!(upright(Shape::Compact, Vec2::new(COMPACT_W, COMPACT_H), false), Vec2::new(COMPACT_H, COMPACT_TALL));
        assert_eq!(upright(Shape::Compact, Vec2::new(COMPACT_W, COMPACT_H), true), Vec2::new(COMPACT_H, COMPACT_TALL_SONG), "a song: taller");
    }

    #[test]
    fn closed_it_is_a_small_notch_then_the_compact_island_and_the_open_panel() {
        let chat = Vec2::new(640.0, 360.0);
        assert_eq!(target(Shape::Closed, Tab::Chat, false, false, false, 0.0, chat), (Vec2::new(80.0, 24.0), 10.0));
        assert_eq!(target(Shape::Compact, Tab::Chat, false, false, false, 0.0, chat), (Vec2::new(188.0, 38.5), 18.0));
        assert_eq!(target(Shape::Open, Tab::Home, false, false, false, 0.0, chat), (Vec2::new(760.0, 270.0), 22.0), "Início: the summary and the list");
        assert_eq!(target(Shape::Open, Tab::Music, false, true, false, 0.0, chat).0.y, 160.0, "its settings: short, whatever the tab");
        assert_eq!(target(Shape::Open, Tab::Home, false, true, false, 0.0, chat).0.y, 160.0);
        assert_eq!(target(Shape::Open, Tab::Claude, false, false, false, 0.0, chat), (Vec2::new(760.0, 160.0), 22.0), "a page of pills");
        assert_eq!(
            target(Shape::Open, Tab::Chat, false, false, false, 0.0, Vec2::new(900.0, 500.0)).0,
            Vec2::new(900.0, 500.0),
            "the conversation: its own size"
        );
        // a question takes the island over at its card's height: 200 with options to pick, 160 for a permission
        assert_eq!(target(Shape::Open, Tab::Home, true, false, false, ask::QUESTION_H - 52.0, chat).0.y, 200.0);
        assert_eq!(target(Shape::Open, Tab::Home, true, false, false, ask::APPROVAL_H - 52.0, chat).0.y, 160.0);
        // an edit in progress makes a page of pills taller, for its change
        assert_eq!(target(Shape::Open, Tab::Claude, false, false, true, 0.0, chat).0.y, 290.0);
    }

    #[test]
    fn each_content_comes_and_goes_on_the_references_transitions() {
        let mut layers = Layers::default();
        layers.show(Shape::Closed, (Tab::Home, false, false), false, 0.0);
        assert_eq!(layers.values(0.11)[0], 0.0, "the face waits 0.12 s");
        assert_eq!(layers.values(0.5), [1.0, 0.0, 0.0, 0.0]);
        layers.show(Shape::Open, (Tab::Home, false, false), false, 1.0);
        let [face, _, open, page] = layers.values(1.1);
        assert!(face < 1.0 && open > 0.0 && open < 1.0 && page == 0.0, "the face goes, the panel comes, the page waits");
        assert_eq!(layers.values(1.5), [0.0, 0.0, 1.0, 1.0]);
        assert!(!layers.moving(1.5) && layers.page_scale(1.5) == 1.0);
        // another tab: the page again from nothing, from 97%
        layers.show(Shape::Open, (Tab::Chats, false, false), false, 2.0);
        assert_eq!(layers.values(2.0)[3], 0.0);
        assert!(layers.page_scale(2.0) < 0.98 && layers.moving(2.2));
        // the entrance's page goes at once when it ends, as the reference's canvas
        layers.show(Shape::Open, (Tab::Home, false, false), true, 3.0);
        assert_eq!(layers.values(3.5)[3], 1.0);
        layers.show(Shape::Compact, (Tab::Home, false, false), false, 4.0);
        assert_eq!(layers.values(4.0)[3], 0.0);
        // and when a question kept it open, the page after it still comes in
        layers.show(Shape::Open, (Tab::Home, false, false), true, 5.0);
        layers.show(Shape::Open, (Tab::Home, false, true), false, 6.0);
        assert_eq!(layers.values(6.6)[3], 1.0);
    }

    #[test]
    fn the_conversation_is_sized_within_the_monitor_and_big_is_seventy_percent() {
        let work = Rect::from_min_size(Pos2::ZERO, Vec2::new(1920.0, 1040.0));
        assert_eq!(chat_size(None, false, work), Vec2::new(760.0, 360.0));
        assert_eq!(chat_size(Some([800.0, 500.0]), false, work), Vec2::new(800.0, 500.0));
        assert_eq!(chat_size(Some([100.0, 100.0]), false, work), Vec2::new(420.0, 300.0), "not smaller than the least");
        assert_eq!(chat_size(Some([9000.0, 9000.0]), false, work), Vec2::new(1824.0, 988.0), "nor bigger than the monitor");
        assert_eq!(chat_size(Some([800.0, 500.0]), true, work), Vec2::new(1344.0, 728.0), "big: 70% of the work area");
    }

    /// Around a 179x32 notch, closed it is the notch with a wing each side and as tall; compact, wider; open, its panel
    /// under the notch's row.
    #[test]
    fn around_a_notch_the_island_is_the_notch_grown() {
        let notch = Vec2::new(179.0, 32.0);
        let chat = Vec2::new(EXPANDED_W, CHAT_H);
        let sized =
            |shape: Shape, tab: Tab, song: bool, voice: bool| around_notch(shape, target(shape, tab, false, false, false, 0.0, chat).0, notch, song, voice);
        assert_eq!(sized(Shape::Closed, Tab::Home, false, false), Vec2::new(179.0 + 60.0, 32.0), "the notch and the face's wings, as tall");
        assert_eq!(sized(Shape::Closed, Tab::Home, true, false), Vec2::new(179.0 + 96.0, 32.0), "a song: room for its cover");
        assert_eq!(sized(Shape::Compact, Tab::Home, false, false), Vec2::new(179.0 + 108.0, 32.0), "compact as tall as the notch too");
        assert_eq!(sized(Shape::Compact, Tab::Home, false, true), sized(Shape::Compact, Tab::Home, true, false), "the voice and a song: wide");
        assert_eq!(sized(Shape::Open, Tab::Home, false, false), Vec2::new(EXPANDED_W, FEED_H), "the header on the notch's row: no band over it");
        // a narrow conversation widens until the tabs and the buttons fit beside the camera
        let narrow = around_notch(Shape::Open, Vec2::new(CHAT_MIN.0, CHAT_MIN.1), notch, false, false);
        assert_eq!(narrow, Vec2::new(179.0 + 2.0 * HEADER_WING, CHAT_MIN.1));
        assert!(narrow.x <= EXPANDED_W, "never wider than the open island is anyway");
        // the wings hold what goes in them: the face (18) with room, the wizard (30) and the minis' grid (30) 12 from the edges
        const { assert!(CLOSED_WING >= 18.0 + 12.0 && COMPACT_WING >= 12.0 + 30.0 + 12.0) };
    }
}
