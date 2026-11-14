//! The island: the Maestro in a pill at the top of the screen, over every other window, with the mascot and a
//! summary ("Precisa de você", "2 trabalhando", "Tudo pronto"). With the pointer over it (or a question waiting on the
//! person) it opens into a panel: the agents and chats with their state, the pending questions and permissions, the
//! voice button and a new chat, which goes into a workspace of its own (`actions.rs`). It is dragged anywhere on the
//! screen, another monitor too, and stays where it was left (`Prefs.island_at`).

use crate::app::ui::agents::{AgentRow, AgentState};
use crate::prelude::*;

mod actions;
mod paint;

/// The open panel's width, and the room the window keeps around the island for its shadow.
pub(crate) const PANEL_WIDTH: f32 = 380.0;
const SIDE: f32 = 20.0;
const TOP: f32 = 6.0;
const BOTTOM: f32 = 26.0;
/// The pill's height.
pub(crate) const PILL_HEIGHT: f32 = 44.0;
/// How long the island stays open after the pointer left it, and after a question came.
const LINGER: f64 = 0.5;
const ASKED_FOR: f64 = 8.0;

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

impl Mood {
    pub(crate) fn label(self) -> String {
        match self {
            Mood::Needs(1) => "Precisa de você".into(),
            Mood::Needs(count) => format!("{count} precisam de você"),
            Mood::Working(count) => format!("{count} trabalhando"),
            Mood::Ready => "Tudo pronto".into(),
            Mood::Empty => "Sem agentes".into(),
        }
    }
}

/// The mascot's pose for a mood: the sign while someone waits, thinking while they work (`working_secs` since the
/// oldest began, `seed` its start), celebrating when everything just finished, else resting.
pub(crate) fn pose(mood: Mood, celebrating: bool, working_secs: f64, seed: u64) -> &'static str {
    match mood {
        Mood::Needs(_) => "it-depends-sign",
        Mood::Working(_) => crate::app::mascot::poses::working(None, "", working_secs, seed),
        Mood::Ready if celebrating => "happy-pixel-celebrating",
        _ => "artboard-24cla",
    }
}

/// An even number of points at least `value`: the window and the island inside it then share one centre.
fn even(value: f32) -> f32 {
    (value / 2.0).ceil() * 2.0
}

/// The window around an island of `visible` size: room for its shadow at the sides, under it and a little above.
pub(crate) fn window_size(visible: Vec2) -> Vec2 {
    Vec2::new(even(visible.x + 2.0 * SIDE), even(visible.y + TOP + BOTTOM))
}

/// Where the window's top left goes for an island of `visible` size hanging from `anchor` (its centre x and its top):
/// whatever its size, the centre stays.
pub(crate) fn window_origin(anchor: [f32; 2], visible: Vec2) -> Pos2 {
    Pos2::new((anchor[0] - window_size(visible).x / 2.0).round(), anchor[1].round())
}

/// The anchor of a window as it is on the screen.
pub(crate) fn anchor_of(window: Rect) -> [f32; 2] {
    [window.center().x, window.top()]
}

/// An anchor kept on the screen (`work`: the monitor less its taskbar): a place saved on a monitor that is gone.
pub(crate) fn keep_on_screen(anchor: [f32; 2], work: Rect) -> [f32; 2] {
    [anchor[0].clamp(work.left() + 60.0, (work.right() - 60.0).max(work.left() + 60.0)), anchor[1].clamp(work.top(), (work.bottom() - 80.0).max(work.top()))]
}

/// Where a mini chat of `size` opens below the island hanging from `anchor`: centred under it, inside `work`.
pub(crate) fn below_island(anchor: [f32; 2], size: Vec2, work: Rect) -> Rect {
    let size = size.min(work.size());
    let left = (anchor[0] - size.x / 2.0).clamp(work.left(), work.right() - size.x);
    let top = (anchor[1] + TOP + PILL_HEIGHT + 10.0).clamp(work.top(), work.bottom() - size.y);
    Rect::from_min_size(Pos2::new(left.round(), top.round()), size)
}

/// The form of a new chat in the open panel.
#[derive(Clone, Debug, Default, PartialEq)]
pub(crate) struct NewForm {
    /// The name of the workspace it makes (empty: the folder's).
    pub(crate) name: String,
    pub(crate) folder: String,
    /// In a git worktree of its own (`maestro/environments.rs`), so the repository's changes stay as they are.
    pub(crate) worktree: bool,
}

#[derive(Default)]
pub(crate) struct Island {
    /// The island stays open until then (the pointer was over it, a question came).
    hold_until: f64,
    /// How many questions there were last frame: more is news, and opens it.
    asked: usize,
    /// Something was working or waiting last frame: the end of it is news.
    was_busy: bool,
    celebrate_until: f64,
    /// The island as drawn last frame: its size sets the window's, its body the open panel's.
    size: Vec2,
    body: f32,
    /// Where it hangs from (centre x, top) and where its window was put for the size it was built at.
    anchor: Option<[f32; 2]>,
    origin: Pos2,
    built: Vec2,
    pub(crate) form: Option<NewForm>,
    /// The window on its way onto the screen without Windows' border (`plain::reveal`), and the pass it was last shown in.
    plain: u8,
    pass: u64,
}

impl Island {
    /// The island as drawn last frame.
    #[cfg(test)]
    pub(crate) fn size(&self) -> Vec2 {
        self.size
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
        let island = &mut self.island;
        let busy = matches!(mood, Mood::Needs(_) | Mood::Working(_));
        if island.was_busy && mood == Mood::Ready {
            island.celebrate_until = time + 6.0;
        }
        island.was_busy = busy;
        if asks.len() > island.asked {
            island.hold_until = island.hold_until.max(time + ASKED_FOR);
        }
        island.asked = asks.len();
        let open = time < island.hold_until || island.form.is_some();
        let view = self.island_view(rows, asks, mood, open, time);
        let (colors, dark) = (self.colors(), self.dark());
        let look = crate::app::chat::list::Look::new(dark, &colors);
        let mut form = self.island.form.take();
        let out =
            if ctx.embed_viewports() { self.island_area(ctx, &view, &mut form, &look, time) } else { self.island_window(ctx, &view, &mut form, &look, time) };
        self.island.form = form;
        let island = &mut self.island;
        if out.hovered {
            island.hold_until = time + LINGER;
        }
        if (out.size - island.size).abs().max_elem() > 0.5 || (out.body - island.body).abs() > 1.0 {
            (island.size, island.body) = (out.size, out.body);
            ctx.request_repaint();
        }
        if out.hovered && !open {
            ctx.request_repaint();
        }
        // the mascot moves while they work or wait; the close after the pointer left comes by the clock
        let animated = busy || time < island.celebrate_until;
        if animated || open {
            ctx.request_repaint_after(std::time::Duration::from_millis(if animated { 33 } else { 100 }));
        }
        self.island_apply(ctx, out);
    }

    /// Where the island hangs from: as it was left, kept on a monitor that is there; the first time, the top centre of
    /// the screen the app is on.
    fn island_anchor(&mut self, ctx: &Context) -> [f32; 2] {
        if let Some(anchor) = self.island.anchor {
            return anchor;
        }
        let anchor = match self.prefs.island_at {
            Some(saved) => keep_on_screen(saved, crate::app::chat::mini::screen_work(ctx, Pos2::new(saved[0], saved[1]))),
            None => {
                let app = ctx.input(|input| input.viewport().outer_rect).unwrap_or(Rect::from_min_size(Pos2::ZERO, Vec2::new(1280.0, 800.0)));
                let work = crate::app::chat::mini::screen_work(ctx, app.center());
                [work.center().x.round(), work.top()]
            }
        };
        self.island.anchor = Some(anchor);
        anchor
    }

    /// The island in a window of its own, over every other: dragged by the system, out of the app's too.
    fn island_window(&mut self, ctx: &Context, view: &paint::View, form: &mut Option<NewForm>, look: &crate::app::chat::list::Look, time: f64) -> paint::Out {
        let anchor = self.island_anchor(ctx);
        let visible = if self.island.size.x > 0.0 { self.island.size } else { Vec2::new(200.0, PILL_HEIGHT) };
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
        let (out, placed) = ctx.show_viewport_immediate(id, builder, |ui, _| {
            let at = Pos2::new(ui.max_rect().center().x - visible.x / 2.0, ui.max_rect().top() + TOP);
            let out = self.island_draw(ui, at, view, form, look, time);
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
        // moved by the person: the window is where it was not put, at the size it was made for
        if let Some(placed) = placed.filter(|rect| (rect.size() - size).abs().max_elem() < 1.5 && (rect.min - origin).abs().max_elem() > 2.0) {
            self.island.origin = placed.min;
            let anchor = anchor_of(placed);
            self.island.anchor = Some(anchor);
            self.prefs.island_at = Some(anchor);
        }
        out
    }

    /// With no windows of its own (the tests, a backend without them): over the canvas, at the top centre of the app.
    fn island_area(&mut self, ctx: &Context, view: &paint::View, form: &mut Option<NewForm>, look: &crate::app::chat::list::Look, time: f64) -> paint::Out {
        let visible = if self.island.size.x > 0.0 { self.island.size } else { Vec2::new(200.0, PILL_HEIGHT) };
        let content = ctx.content_rect();
        let at = Pos2::new((content.center().x - visible.x / 2.0).round(), content.top() + crate::app::ui::TITLE_BAR + 6.0);
        let mut out = paint::Out::default();
        egui::Area::new(egui::Id::unique("island_area")).order(egui::Order::Foreground).fixed_pos(at).constrain(false).show(ctx, |ui| {
            out = self.island_draw(ui, at, view, form, look, time);
        });
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_island_says_what_the_agents_are_doing_and_the_mascot_follows() {
        let working = AgentState::Working(10.0);
        let idle = AgentState::Idle(None);
        assert_eq!(mood([]), Mood::Empty);
        assert_eq!(mood([&AgentState::Ended]), Mood::Empty, "the ended are not counted");
        assert_eq!(mood([&idle, &AgentState::Active(1.0)]), Mood::Ready);
        assert_eq!(mood([&working, &idle, &working]), Mood::Working(2));
        // someone waiting wins over the ones working
        assert_eq!(mood([&working, &AgentState::Waiting, &idle]), Mood::Needs(1));
        assert_eq!(Mood::Needs(1).label(), "Precisa de você");
        assert_eq!(Mood::Needs(3).label(), "3 precisam de você");
        assert_eq!(Mood::Working(2).label(), "2 trabalhando");
        assert_eq!(Mood::Ready.label(), "Tudo pronto");
        assert_eq!(pose(Mood::Needs(1), false, 0.0, 0), "it-depends-sign");
        assert_eq!(pose(Mood::Ready, false, 0.0, 0), "artboard-24cla");
        assert_eq!(pose(Mood::Ready, true, 0.0, 0), "happy-pixel-celebrating");
        assert_eq!(pose(Mood::Working(1), true, 3.0, 7), crate::app::mascot::poses::working(None, "", 3.0, 7));
    }

    #[test]
    fn the_window_keeps_the_islands_centre_when_it_opens_and_closes() {
        let anchor = [960.0, 0.0];
        let (pill, panel) = (Vec2::new(207.0, PILL_HEIGHT), Vec2::new(PANEL_WIDTH, 311.0));
        for size in [pill, panel] {
            let (origin, window) = (window_origin(anchor, size), window_size(size));
            assert_eq!(origin.x + window.x / 2.0, 960.0, "{size:?}");
            assert_eq!(origin.y, 0.0);
            assert!(window.x >= size.x + 2.0 * SIDE && window.y >= size.y + TOP + BOTTOM, "room for its shadow");
        }
        assert_eq!(anchor_of(Rect::from_min_size(window_origin(anchor, panel), window_size(panel))), anchor, "read back, the same anchor");
    }

    #[test]
    fn a_saved_place_on_a_monitor_that_is_gone_comes_back_and_the_mini_chat_opens_below() {
        let work = Rect::from_min_max(Pos2::ZERO, Pos2::new(1920.0, 1040.0));
        assert_eq!(keep_on_screen([960.0, 0.0], work), [960.0, 0.0], "where it is, it stays");
        assert_eq!(keep_on_screen([5000.0, -300.0], work), [1860.0, 0.0]);
        assert_eq!(keep_on_screen([-40.0, 9000.0], work), [60.0, 960.0]);
        let size = Vec2::new(340.0, 600.0);
        let below = below_island([960.0, 0.0], size, work);
        assert_eq!((below.center().x, below.top()), (960.0, TOP + PILL_HEIGHT + 10.0));
        assert!(work.contains_rect(below_island([10.0, 900.0], size, work)), "pulled in whole");
    }
}
