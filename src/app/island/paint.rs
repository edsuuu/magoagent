//! What the island draws: the black notch (closed: the wizard and one mini wizard per agent), and open the
//! header (tabs at the left, buttons at the right) over Home or the page of a tab. It only draws and says what was asked
//! (`Out`); `island_apply` does it.

use super::marquee::marquee;
use super::minis::mini_cells;
use super::stage::{self, Activity};
use super::{EXPANDED_W, Line, Mood, NewForm, Shape, Tab, icons, main_state};
use crate::app::chat::list::{Look, clipped, glyph};
use crate::app::chat::mini_agents::pressable;
use crate::app::integrations::spotify::Now;
use crate::app::integrations::spotify::groove::Ear;
use crate::app::island::forms;
use crate::app::maestro::outside::Badge;
use crate::app::mascot::identity::make_distinct;
use crate::app::mascot::mago::{MagoState, mago_poke, mago_pokes, paint_mago_toward, paint_mini};
use crate::app::ui::agents::{AgentRow, AgentState};
use crate::prelude::*;
use crate::voice::VoiceState;
use std::collections::HashMap;
use std::sync::atomic::Ordering;

pub(crate) const INK: Color32 = Color32::from_rgb(0xf5, 0xf6, 0xf8);
pub(crate) const DIM: Color32 = Color32::from_rgb(0x8e, 0x93, 0x9c);
pub(super) const DIM_3: Color32 = Color32::from_rgb(0x6b, 0x70, 0x79);
pub(super) const CARD: Color32 = Color32::from_rgb(0x14, 0x15, 0x18);
pub(super) const CARD_FLAT: Color32 = Color32::from_rgb(0x0e, 0x0f, 0x11);
const TAB_ON: Color32 = Color32::from_rgb(0x1d, 0x1f, 0x23);
/// The radius of the round mark under the tab shown (and the one under the pointer).
const TAB_DOT: f32 = 14.0;
pub(crate) const AMBER: Color32 = Color32::from_rgb(0xf5, 0xa5, 0x24);
pub(super) const RED: Color32 = Color32::from_rgb(0xf4, 0x50, 0x5e);
/// The island's black: the words and arrows on a white button, a badge's ring, a picture's backdrop.
pub(super) const BLACK: Color32 = Color32::from_rgb(0x0b, 0x0c, 0x0e);
/// A row under the pointer (the GitHub, music and feed lists).
pub(super) const HOVER: Color32 = Color32::from_rgb(0x1b, 0x1d, 0x21);
/// The header's height under the 8 above it: what the pages start under.
pub(super) const PAGE_TOP: f32 = 42.0;
/// The middle of the header's row, from the panel's top.
const HEADER_MID: f32 = 8.0 + 17.0;

/// What the island shows this frame.
pub(super) struct View {
    pub(super) mood: Mood,
    pub(super) lines: Vec<Line>,
    /// The chats waiting on a question or a permission (`decide_card`).
    pub(super) asks: Vec<String>,
    /// The asks behind the one shown.
    pub(super) queued: usize,
    pub(super) tab: Tab,
    pub(super) voice_on: bool,
    pub(super) voice_state: VoiceState,
    pub(super) mic: f32,
    pub(super) voice_note: String,
    pub(super) celebrating: bool,
    /// An agent finished since the island was last open: Luna wears a check (`done.rs`).
    pub(super) finished: bool,
    /// Where the wizard's eyes look, in the window's coordinates: the cursor when it is near (`cursor.rs`).
    pub(super) pointer: Option<Pos2>,
    pub(super) sound_on: bool,
    pub(super) volume: f32,
    pub(super) close_after: u8,
    pub(super) color: Color32,
    pub(super) settings: bool,
    /// How much is left of the open island's wait before it folds (1 to 0), in its last seconds.
    pub(super) countdown: Option<f32>,
    /// The agent the Home card shows.
    pub(super) focus: Option<String>,
    /// The row of the conversation shown.
    pub(super) chat_row: Option<AgentRow>,
    /// A session outside the app shown on the conversation page instead (`session.rs`).
    pub(super) session: Option<String>,
    /// The whole mini chat on the Chat tab, not the compact conversation.
    pub(super) chat_full: bool,
    /// What the agent in focus is doing (`stage.rs`), for how long (s), and a glimpse of the file it reads.
    pub(super) activity: Option<Activity>,
    pub(super) since: f32,
    pub(super) glimpse: Vec<(usize, String)>,
    /// How many agents failed now, how many times the wizard was poked, and whether it is dizzy from it.
    pub(super) failures: usize,
    pub(super) pokes: u32,
    pub(super) dizzy: bool,
    /// Seconds into the entrance, while it plays.
    pub(super) entrance: Option<f32>,
    /// The edge of the screen it is on: square on it (on none, a pill with four round corners); on a side, the closed
    /// and compact ones stand up.
    pub(super) dock: super::window::Dock,
    /// The camera notch it grows out of (its size), on the top edge of a screen that has one.
    pub(super) notch: Option<Vec2>,
    /// What the island is, and how much shows of its face, compact content, panel and page, and the page's size.
    pub(super) shape: Shape,
    pub(super) layers: [f32; 4],
    pub(super) page_scale: f32,
    /// The island's wizard where its spring has it (from the island's top left, and its size); where the page that
    /// shows asks for it (`aim`), and the island's top left this frame.
    pub(super) bot: Option<(Vec2, f32)>,
    pub(super) aim: std::cell::Cell<Option<Aim>>,
    pub(super) origin: std::cell::Cell<Pos2>,
}

/// Where a page wants the island's one mascot: from the island's top left, how big and in which
/// state. It springs there and is drawn once, over everything.
#[derive(Clone, Copy, Debug)]
pub(super) struct Aim {
    pub(super) at: Vec2,
    pub(super) size: f32,
    pub(super) state: MagoState,
}

/// The page that shows puts the island's wizard at `at`, `size` big.
pub(super) fn aim(view: &View, at: Pos2, size: f32, state: MagoState) {
    view.aim.set(Some(Aim { at: at - view.origin.get(), size, state }));
}

impl Line {
    /// Its mini character: the wizard in its colour, whatever the agent.
    fn paint_mini(&self, painter: &Painter, id: &str, center: Pos2, body: f32, time: f64) {
        paint_mini(painter, id, center, body, self.color, self.mago, time);
    }
}

/// What was done on the island this frame.
#[derive(Default)]
pub(crate) struct Out {
    pub(super) open_agent: Option<String>,
    /// "No canvas" on the Home card: the app in front, on this agent's card in the canvas.
    pub(super) canvas: Option<String>,
    /// "Conversar" on the Home card: this chat in the island.
    pub(super) talk: Option<String>,
    /// "Ir ao terminal" on a session's conversation: its terminal's window to the front.
    pub(super) terminal: Option<String>,
    pub(super) voice: bool,
    pub(super) tab: Option<Tab>,
    pub(super) close: bool,
    pub(super) toggle_settings: bool,
    pub(super) open_app_settings: bool,
    pub(super) volume: Option<f32>,
    pub(super) close_after: Option<u8>,
    pub(super) color: Option<Color32>,
    pub(super) wake: bool,
    pub(super) create: bool,
    pub(super) pick_folder: bool,
    /// The press that moves the window: the system takes it from here.
    pub(super) drag: bool,
    /// The conversation pulled by an edge to this size, and the big/small button.
    pub(super) resize: Option<Vec2>,
    pub(super) big: bool,
    /// An edge double-clicked: the size before it was pulled.
    pub(super) unpull: bool,
    /// A picture of the log clicked (its base64): on the whole screen.
    pub(super) picture: Option<String>,
    /// The conversation's ⤢ (or its model's pill): the whole mini chat in the island.
    pub(super) full: bool,
    /// The sound button.
    pub(super) sound: bool,
    /// A pill was clicked: its blip.
    pub(super) picked: bool,
    pub(super) hovered: bool,
    /// A button of the mouse is down.
    pub(super) holding: bool,
    /// A field of the island has the keyboard, in a window the system has in focus: it holds the island open.
    pub(super) typing: bool,
    /// The system has the island's window in focus: a click inside gives it, a click anywhere else takes it away.
    pub(super) focused: bool,
    /// The agent whose pill the pointer is on.
    pub(super) focus: Option<String>,
    /// The height of the page's content (for the pages that grow with it).
    pub(super) body: f32,
    /// Início's lines hidden by their × (or "Ocultar tudo"), with what they were at.
    pub(super) hide: Vec<(String, u64)>,
    /// "Mostrar ocultos".
    pub(super) show_hidden: bool,
}

impl DrawingApp {
    pub(super) fn island_view(&mut self, rows: Vec<AgentRow>, asks: Vec<String>, mood: Mood, time: f64) -> View {
        // what each agent does is read from its conversation: kept up to date for the ones at work or just done
        if self.now - self.island.polled >= 0.5 {
            self.island.polled = self.now;
            let now = self.now;
            for row in rows.iter().filter(|row| stage::recent(row.state, now)) {
                self.refresh_transcript_of(&row.id);
            }
        }
        let mut activities: HashMap<String, Activity> = HashMap::new();
        let mut lines: Vec<Line> = rows
            .iter()
            .map(|row| {
                let mut detail = match self.state_note(row) {
                    Some(note) => format!("{} · {note}", row.state.label()),
                    None => row.state.label().to_string(),
                };
                if let Some(space) = self.elsewhere(row) {
                    detail.push_str(&format!(" · {space}"));
                }
                let program = row.spec.command.split_whitespace().next().unwrap_or_default().to_lowercase();
                let working = matches!(row.state, AgentState::Working(_));
                let activity = self.transcripts.get(&row.id).and_then(|t| stage::activity(&t.items, working, stage::recent(row.state, self.now) && !working));
                // as the chat's band shows it: talking while it writes, the error of a step
                let mago = self
                    .transcripts
                    .get(&row.id)
                    .map_or(MagoState::from_agent(row.state, self.now), |transcript| MagoState::in_turn(row.state, &transcript.items, self.now));
                activities.extend(activity.map(|activity| (row.id.clone(), activity)));
                Line {
                    id: row.id.clone(),
                    name: self.chat_short_name(row),
                    detail,
                    state: row.state,
                    mago,
                    color: self.agent_color(&row.id),
                    chat: self.is_chat(&row.id),
                    canvas: self.scene.elements.iter().any(|element| element.id == row.id) || self.away.contains_key(&row.id),
                    badge: None,
                    // where it is: the two kinds of session Claude Code's page shows
                    kind: if program == "claude" { format!("{} · no canvas", stage::program_name(&program)) } else { stage::program_name(&program) },
                    at: 0,
                    asleep: false,
                }
            })
            .collect();
        // the sessions outside the app, one pill per origin and folder: the name says which folder ("loja"), the kind
        // which program and where ("Claude Desktop · fora do app")
        lines.extend(self.outside_pills().into_iter().map(|pill| Line {
            kind: if pill.kind == "Hermes" { "Hermes Agent".into() } else { format!("{} · fora do app", pill.kind) },
            mago: if pill.asleep { MagoState::Sleeping } else { MagoState::from_agent(pill.state, self.now) },
            at: pill.at,
            asleep: pill.asleep,
            id: pill.id,
            name: pill.name,
            // what it is doing, a step line (the prompt, the tool on what, how the turn ended); nothing yet: its state
            detail: if pill.activity.is_empty() { pill.state.label().to_string() } else { pill.activity.clone() },
            state: pill.state,
            color: pill.color,
            chat: false,
            canvas: false,
            badge: pill.badge,
        }));
        // no two agents by the same name on the pills and the cards
        make_distinct(&mut lines, |line| &mut line.name);
        lines.extend(self.integration_lines());
        // who needs the person first, then who works
        lines.sort_by_key(|line| match line.state {
            AgentState::Waiting => 0,
            AgentState::Working(_) => 1,
            _ => 2,
        });
        self.island.listed =
            super::feed::feed_lines(&lines, &Default::default()).0.iter().map(|line| (line.id.clone(), super::feed::signature(line))).collect();
        let island = &self.island;
        let grid = island.tab.grid();
        let in_grid = |line: &&Line| grid.shows(line);
        let focus = island
            .focus
            .clone()
            .filter(|id| lines.iter().filter(in_grid).any(|line| &line.id == id))
            .or_else(|| lines.iter().find(in_grid).map(|line| line.id.clone()));
        let chat_row = self.chat_agent.as_ref().and_then(|id| rows.iter().find(|row| &row.id == id)).cloned();
        let activity = focus.as_ref().and_then(|id| activities.remove(id));
        let failures = lines.iter().filter(|line| line.mago == MagoState::Error).count();
        let (pokes, dizzy) = mago_pokes("island-main", time);
        let (since, glimpse) = self.island_stage(focus.as_deref(), activity.as_ref(), time);
        let island = &self.island;
        View {
            mood,
            lines,
            // the oldest waits on the card, the rest in the queue (their pills carry the dot)
            queued: asks.len().saturating_sub(1),
            asks: asks.into_iter().take(1).collect(),
            tab: island.tab,
            voice_on: self.voice.on,
            voice_state: self.voice.state,
            mic: f32::from_bits(self.voice.shared.level.load(Ordering::Relaxed)).min(1.0),
            voice_note: self.voice.status.clone(),
            celebrating: time < island.celebrate_until,
            finished: island.finished,
            pointer: island.pointer,
            sound_on: self.prefs.island_sound,
            volume: self.prefs.island_volume,
            close_after: self.prefs.island_close_after,
            color: Color32::from_rgb(self.prefs.island_color[0], self.prefs.island_color[1], self.prefs.island_color[2]),
            settings: self.island.settings,
            countdown: self.island.machine.countdown(time),
            focus,
            chat_row,
            session: self.island.session.as_ref().map(|session| session.id.clone()),
            chat_full: self.island.chat_full,
            activity,
            since,
            glimpse,
            failures,
            pokes,
            dizzy,
            dock: island.dock,
            notch: island.notch,
            entrance: island.greeting.map(|start| (time - start) as f32).filter(|_| island.machine.stage() == super::fsm::Stage::Greeting),
            shape: island.last_shape.unwrap_or(Shape::Closed),
            layers: island.layers.values(time),
            page_scale: island.layers.page_scale(time),
            bot: island.bot_placed.then(|| (Vec2::new(island.bot[0].value, island.bot[1].value), island.bot[2].value)),
            aim: Default::default(),
            origin: Default::default(),
        }
    }

    /// Draws the island, `anim` big, its top centre at `at`'s top left (the top edge is straight, the bottom corners
    /// round); the returned `Out` has what was done.
    #[allow(clippy::too_many_arguments)]
    pub(super) fn island_draw(
        &mut self,
        ui: &mut egui::Ui,
        at: Pos2,
        anim: Vec2,
        radius: f32,
        view: &View,
        form: &mut Option<NewForm>,
        look: &Look,
        colors: &UiColors,
        time: f64,
    ) -> Out {
        let mut out = Out::default();
        let rect = Rect::from_min_size(at, anim);
        let round = radius.round() as u8;
        ui.painter().rect_filled(rect, corners(round, view.dock), view.color);
        if view.notch.is_some() {
            ears(ui.painter(), rect, view.color);
        }
        out.hovered = ui.input(|input| input.pointer.hover_pos()).is_some_and(|pointer| rect.contains(pointer));
        // the whole island moves by the press; the closed and compact ones open by the click
        let grip = ui.interact(rect, ui.scope_id().with("island_grip"), Sense::click_and_drag());
        out.drag = grip.drag_started();
        out.holding = ui.input(|input| input.pointer.any_down()) && out.hovered;
        // another window taking the system's focus lets it fold again
        let focused = ui.memory(|memory| memory.focused());
        out.typing = ui.input(|input| input.focused) && focused.and_then(|id| ui.ctx().read_response(id)).is_some_and(|field| rect.intersects(field.rect));
        let opens = anim.x < EXPANDED_W - 40.0;
        // what opens by the click shows the hand
        let grip = if opens { grip.on_hover_cursor(CursorIcon::PointingHand) } else { grip };
        out.wake = grip.clicked() && opens;
        // each content comes and goes on its own transition (`Layers`); the page that shows says where the wizard goes
        view.origin.set(rect.min);
        let [face, compact, open, _] = view.layers;
        let mut wanted = None;
        for (alpha, mode) in [(face, 0), (compact, 1)] {
            if alpha > 0.0 {
                ui.scope_builder(egui::UiBuilder::new().max_rect(rect), |ui| {
                    ui.set_opacity(alpha);
                    ui.set_clip_rect(rect.intersect(ui.clip_rect()));
                    self.island_small(ui, rect, view, mode == 1, time, &mut out);
                });
            }
        }
        if view.shape == Shape::Compact {
            wanted = view.aim.take();
        }
        view.aim.set(None);
        if open > 0.0 {
            ui.scope_builder(egui::UiBuilder::new().max_rect(rect), |ui| {
                ui.set_opacity(open);
                ui.set_clip_rect(rect.intersect(ui.clip_rect()));
                // around a notch the header is its row, the tabs left of the camera and the buttons right of it: the
                // panel starts above so its header's middle is the notch's
                let panel = view.notch.map_or(rect, |notch| rect.with_min_y(rect.top() + notch.y / 2.0 - HEADER_MID));
                self.island_open(ui, panel, view, form, look, colors, time, &mut out);
            });
            if view.shape == Shape::Open {
                wanted = view.aim.take();
            }
            // the reference's countdown: a thin bar at the bottom, 160 wide when it starts, gone when the island folds
            if let Some(left) = view.countdown.filter(|left| *left > 0.0) {
                let bar = Rect::from_center_size(Pos2::new(rect.center().x, rect.bottom() - 1.0), Vec2::new(160.0 * left, 2.0));
                ui.painter().rect_filled(bar, 1, Color32::from_white_alpha(89).gamma_multiply(open));
            }
        }
        // the one wizard, over everything, where its spring has it
        view.aim.set(wanted);
        if let Some(wanted) = wanted {
            let (at, size) = view.bot.unwrap_or((wanted.at, wanted.size));
            let center = rect.min + at;
            let painter = ui.painter().with_clip_rect(rect.intersect(ui.clip_rect()));
            // while the music plays and no agent wants the person, she dances to it, open too; open and paused, she
            // does not nap there. An agent at work does not stop her
            let playing = self.integrations.spotify.now.as_ref().is_some_and(|now| now.playing);
            let calm = dances_through(wanted.state) && (view.shape != Shape::Open || playing);
            // she dances only to a song heard here; not heard (or paused), she is awake and can be poked, with her own faces
            // dancing she can still be poked dizzy: the pokes go to her own motion, and dizzy she stops dancing until it
            // passes (`Luna`)
            let luna = self.island_luna(calm).with(wanted.state, view.pointer, true);
            let pointer = view.pointer.or_else(|| painter.ctx().pointer_latest_pos());
            if luna.dances(time) && pointer.is_some_and(|at| at.distance(center) < size * 0.45) && painter.ctx().input(|input| input.pointer.primary_pressed())
            {
                mago_poke("island-main", time);
            }
            super::replay::live(&painter, time, move |painter, time| luna.paint(painter, center, size, time));
            if view.finished && view.shape == Shape::Compact {
                super::done::paint_check(&painter, center, size);
            }
        }
        self.island_keys(ui, view, &mut out);
        out
    }

    /// The closed notch: only the wizard's face, in the middle, its eyes on the cursor, and around by itself while the
    /// cursor is not known. The compact island: the wizard at the left, its eyes on the cursor, and a mini one per
    /// agent in a grid at the right (`mini_cells`), in its colour, with a badge on the ones that wait. While a song
    /// plays, Spotify's pill gives its place to the song's cover and to the sound's bars (`pill.rs`).
    fn island_small(&self, ui: &mut egui::Ui, rect: Rect, view: &View, compact: bool, time: f64, out: &mut Out) {
        let state = main_state(view.mood, view.celebrating);
        // while a song plays, its cover and the sound's bars fill the pill playing or paused, as the Dynamic Island
        // keeps the song while it pauses (its bars then rest as dots) paused for a while, the song leaves the pill
        // (`Spotify::shown`) standing up on a side of the screen, the closed notch and the compact one have them one
        // under the other
        let upright = view.dock.upright();
        let now = self.integrations.spotify.shown(time);
        // the bars hear the song whether or not Luna dances to it (`Ear::groove` asks for the listening)
        let ear = now.filter(|now| now.playing).map(|_| self.integrations.spotify.dancer.ear());
        // around a notch (`super::around_notch`) the middle is under it: the wizard and the song go on the wings
        let notch = view.notch.map_or(0.0, |notch| notch.x);
        if !compact {
            let center = match view.notch {
                // the face in the left wing's middle; with a song, after its cover
                Some(_) if now.is_some() => Pos2::new(rect.left() + NOTCH_LUNA_SONG, rect.center().y),
                Some(_) => Pos2::new(rect.left() + (rect.width() - notch) / 4.0, rect.center().y),
                None => rect.center(),
            };
            // while the music plays and no agent wants the person, she dances to it
            let luna = self.island_luna(dances_through(state)).with(state, view.pointer, false);
            // the notch is 24 tall: she keeps 3 above and below
            const LUNA: f32 = 18.0;
            super::replay::live(ui.painter(), time, move |painter, time| luna.paint(painter, center, LUNA, time));
            if view.finished {
                super::done::paint_check(ui.painter(), center, LUNA);
            }
            if let Some(now) = now {
                self.island_playing(ui, rect, now, None, upright, notch, ear.as_ref(), time, out);
            }
            return;
        }
        // standing up, the wizard at the top and the minis under it
        let wizard = if upright {
            Pos2::new(rect.center().x, rect.top() + super::minis::UPRIGHT_INSET)
        } else {
            Pos2::new(rect.left() + super::COMPACT_SIDE, rect.center().y + 1.0)
        };
        // as tall as a notch (32) she keeps 3 above and below
        aim(view, wizard, view.notch.map_or(30.0, |notch| 30.0f32.min(notch.y - 6.0)), state);
        // the green mini of Spotify is the cover and the bars then, spread between the wizard and the other minis
        let spotify = |line: &Line| line.id.strip_prefix(crate::app::integrations::ITEM_PREFIX) == Some("spotify");
        let lines: Vec<&Line> = view.lines.iter().filter(|line| now.is_none() || !spotify(line)).collect();
        let cells = if upright { super::minis::upright_cells(rect, lines.len()) } else { mini_cells(rect, lines.len()) };
        let grid = cells.iter().copied().reduce(|grid, cell| grid.union(cell));
        for (line, cell) in lines.into_iter().zip(cells) {
            let (mini, id, center, body) = (line.clone(), format!("island-mini-{}", line.id), cell.center(), cell.width());
            super::replay::live(ui.painter(), time, move |painter, time| {
                mini.paint_mini(painter, &id, center, body, time);
                mini_badge(painter, center, body, &mini, time);
            });
            // a click on a mini opens the island on it: its page (Início or Conversas) with it in the card
            let response = ui
                .interact(cell.expand(1.0), ui.scope_id().with(("island_mini", &line.id)), Sense::click())
                .on_hover_cursor(CursorIcon::PointingHand)
                .tip(&line.name);
            if response.clicked() {
                (out.focus, out.tab, out.wake, out.picked) = (Some(line.id.clone()), Some(Tab::of(&line.id)), true, true);
            }
        }
        if view.voice_on {
            let bars = crate::app::voice::call_bars(view.voice_state, view.mic, time);
            // around a notch, in the left wing after the wizard
            let x = if notch > 0.0 { rect.left() + super::COMPACT_SIDE + 15.0 + 10.0 + 10.5 } else { rect.center().x };
            crate::app::voice::paint_bars(ui.painter(), Pos2::new(x, rect.center().y), bars, INK);
        }
        if let Some(now) = now {
            self.island_playing(ui, rect, now, Some(grid), upright, notch, ear.as_ref(), time, out);
        }
    }

    /// The song's cover and the sound's bars in the pill while it plays, spread with the wizard and the minis (`pill.rs`).
    fn island_playing(
        &self,
        ui: &egui::Ui,
        rect: Rect,
        now: &Now,
        compact: Option<Option<Rect>>,
        upright: bool,
        notch: f32,
        ear: Option<&Ear>,
        time: f64,
        out: &mut Out,
    ) {
        let cover = self.integrations.spotify.cover.as_ref().map(egui::TextureHandle::id);
        let (tint, title) = (self.integrations.spotify.tint, &now.track.title);
        if super::pill::spotify(ui, rect, cover, tint, title, now.playing, compact, upright, notch, ear, time) {
            (out.tab, out.wake, out.picked) = (Some(Tab::Music), true, true);
        }
    }

    /// The open island: the header and the page under it.
    #[allow(clippy::too_many_arguments)]
    fn island_open(
        &mut self,
        ui: &mut egui::Ui,
        rect: Rect,
        view: &View,
        form: &mut Option<NewForm>,
        look: &Look,
        colors: &UiColors,
        time: f64,
        out: &mut Out,
    ) {
        self.island_header(ui, rect, view, out, time);
        let page = Rect::from_min_max(rect.min + Vec2::new(10.0, PAGE_TOP), rect.max - Vec2::new(10.0, 10.0));
        // the page comes in as the reference's `.view.on`: its opacity, and from 97% of its size
        let layer = ui.layer_id();
        let start = ui.ctx().graphics_mut(|graphics| graphics.entry(layer).next_idx());
        ui.scope(|ui| {
            ui.set_opacity(view.layers[3]);
            self.island_page(ui, rect, page, view, form, look, colors, time, out);
        });
        if view.page_scale != 1.0 {
            let end = ui.ctx().graphics_mut(|graphics| graphics.entry(layer).next_idx());
            let scale = egui::emath::TSTransform::new(page.center().to_vec2() * (1.0 - view.page_scale), view.page_scale);
            ui.ctx().graphics_mut(|graphics| graphics.entry(layer).transform_range(start, end, scale));
        }
    }

    /// The page under the header: a question, the settings, or the tab's.
    #[allow(clippy::too_many_arguments)]
    fn island_page(
        &mut self,
        ui: &mut egui::Ui,
        rect: Rect,
        page: Rect,
        view: &View,
        form: &mut Option<NewForm>,
        look: &Look,
        colors: &UiColors,
        time: f64,
        out: &mut Out,
    ) {
        if !view.asks.is_empty() {
            return self.island_ask(ui, page, view, time, out);
        }
        if view.settings {
            self.island_settings_page(ui, page, view, out);
            return;
        }
        match view.tab {
            Tab::Home => self.island_feed(ui, page, view, time, out),
            Tab::GitHub => self.island_github(ui, page, colors),
            Tab::Music => self.island_music(ui, page, time),
            Tab::Chats | Tab::Claude => self.island_home(ui, rect, page, view, time, out),
            Tab::Chat if view.session.is_some() => {
                self.island_session(ui, page, view, time, out);
                self.island_edges(ui, rect, out);
            }
            Tab::Chat => match &view.chat_row {
                Some(row) if !view.chat_full => {
                    self.island_prompt(ui, page, row, view, look, time, out);
                    self.island_edges(ui, rect, out);
                }
                Some(row) => {
                    self.refresh_transcript();
                    (self.mini.window, self.mini.in_island, self.mini.island_rect) = (true, true, Some(rect));
                    self.mini_panel(ui.ctx(), row, page, look, colors, time);
                    (self.mini.window, self.mini.in_island, self.mini.island_rect) = (false, false, None);
                    // its ✕ (and Esc) and its full screen: the island folds, the chat going to the app with the latter
                    out.close |= std::mem::take(&mut self.mini.left);
                    self.island_edges(ui, rect, out);
                }
                None => {
                    let card = Rect::from_min_size(page.min, page.size());
                    ui.painter().rect_filled(card, 20, CARD);
                    ui.painter().text(card.center(), egui::Align2::CENTER_CENTER, "Escolha um agente em Conversas.", font(12.0), DIM);
                }
            },
            Tab::New => forms::new_chat_page(ui, page, form, view, out),
        }
    }

    /// The tabs at the left (Início, the conversation, Chats, Claude Code, GitHub and Spotify when they are on, the new
    /// chat); the voice, the settings and the close at the right. Each takes its own press with the hand over it; their
    /// names go to the system's accessibility, not to a tip over the page under them.
    fn island_header(&self, ui: &mut egui::Ui, rect: Rect, view: &View, out: &mut Out, time: f64) {
        let center_y = rect.top() + HEADER_MID;
        let pinned = !view.asks.is_empty();
        let tabs = header_tabs(&view.lines);
        let count = tabs.len();
        for (index, (tab, icon, size, tip)) in tabs.into_iter().enumerate() {
            let area = Rect::from_center_size(Pos2::new(rect.left() + 14.0 + 15.0 + index as f32 * 35.0, center_y), Vec2::new(30.0, 28.0));
            let response = pressable(ui, area, ui.scope_id().with(("island_tab", index)), tip);
            let on = view.tab == tab && !pinned && !view.settings;
            // a round mark under the icon, not a capsule
            if on {
                ui.painter().circle_filled(area.center(), TAB_DOT, TAB_ON);
            } else if response.hovered() {
                ui.painter().circle_filled(area.center(), TAB_DOT, Color32::from_white_alpha(18));
            }
            let ink = if on {
                INK
            } else if response.hovered() {
                super::log::REPLY
            } else {
                DIM
            };
            icons::paint(ui.painter(), icon, area.center(), size, ink);
            if response.clicked() {
                out.tab = Some(tab);
            }
        }
        // the call's note between them
        if !view.voice_note.is_empty() {
            let left = rect.left() + 14.0 + count as f32 * 35.0 + 10.0;
            // around a notch it stops before the camera
            let end = view.notch.map_or(rect.right() - 150.0, |notch| rect.center().x - notch.x / 2.0 - 8.0);
            let galley = clipped(ui.painter(), view.voice_note.clone(), font(11.0), DIM_3, end - left);
            ui.painter().galley(Pos2::new(left, center_y - galley.size().y / 2.0), galley, DIM_3);
        }
        // right to left: close, settings, voice; dim, the ink when on (`.header-actions button.on`), 14 apart
        let mut x = rect.right() - 16.0 - 9.0;
        let mut button = |ui: &mut egui::Ui, name: &str, tip: &str, on: bool| {
            let area = Rect::from_center_size(Pos2::new(x, center_y), Vec2::splat(24.0));
            x -= 32.0;
            let response = pressable(ui, area, ui.scope_id().with(name), tip);
            let ink = if on { INK } else { DIM };
            (response, area.center(), ink)
        };
        if !pinned {
            let (response, at, ink) = button(ui, "island_close", "Recolher", false);
            icons::paint(ui.painter(), icons::XMARK, at, 17.0, ink);
            out.close = response.clicked();
        }
        let (response, at, ink) = button(ui, "island_sound", if view.sound_on { "Silenciar os sons" } else { "Ligar os sons" }, false);
        icons::paint(ui.painter(), if view.sound_on { icons::SPEAKER } else { icons::SPEAKER_SLASH }, at, 18.0, ink);
        out.sound = response.clicked();
        let (response, at, ink) = button(ui, "island_settings", "Ajustes da ilha", view.settings);
        icons::paint(ui.painter(), if view.settings { icons::GEAR_FILL } else { icons::GEAR_LINE }, at, 18.0, ink);
        out.toggle_settings = response.clicked();
        // the call on hold (voice::CALL_ON): no Chat por voz; one still on can be ended
        if crate::app::voice::CALL_ON || view.voice_on {
            let (response, at, ink) = button(ui, "island_voice", if view.voice_on { "Encerrar o chat por voz" } else { "Chat por voz" }, view.voice_on);
            if view.voice_on {
                crate::app::voice::paint_bars(ui.painter(), at, crate::app::voice::call_bars(view.voice_state, view.mic, time), ink);
            } else {
                icons::paint(ui.painter(), icons::MIC, at, 18.0, ink);
            }
            out.voice = response.clicked();
        }
        // the conversation: its big and small size
        if view.tab == Tab::Chat && view.chat_full && !pinned {
            let (response, at, ink) = button(ui, "island_big", "Aumentar ou voltar ao tamanho", false);
            glyph(ui.painter(), at, 16.0, 1.6, ink, EXPAND);
            out.big = response.clicked();
        }
    }

    /// The island's settings, in the island: the wizard at the left, then
    /// rows 10 apart (padding 14 16 14 84): [switch, Som, volume] with the island's colours at the right; [timer,
    /// Fechar sozinho · N s, 5s 10s 15s 30s]; [● Claude Code, ● API, Ajustes… at the right]. The type is a size above the
    /// reference's, which read too small here.
    fn island_settings_page(&self, ui: &mut egui::Ui, page: Rect, view: &View, out: &mut Out) {
        const GREEN: Color32 = Color32::from_rgb(0x22, 0xc5, 0x5e);
        const ROW_INK: Color32 = Color32::from_rgb(0xc5, 0xc8, 0xcd);
        ui.painter().rect(page, 20, CARD, Stroke::new(1.0, Color32::from_white_alpha(9)), StrokeKind::Inside);
        let state = main_state(view.mood, view.celebrating);
        let wizard = Pos2::new(page.left() + 44.0, page.center().y);
        aim(view, wizard, 46.0, state);
        let (left, right) = (page.left() + 84.0, page.right() - 16.0);
        let rows = [20.0, 24.0, 18.0];
        let mut top = page.center().y - (rows.iter().sum::<f32>() + 20.0) / 2.0;
        let mut centers = [0.0; 3];
        for (center, height) in centers.iter_mut().zip(rows) {
            *center = top + height / 2.0;
            top += height + 10.0;
        }
        let row_font = font(12.5);
        // 1: the sound, its volume, and the colours on whole pixels and a point lower, on the line of the "Som" letters
        let switch =
            egui::emath::GuiRounding::round_to_pixels(Rect::from_min_size(Pos2::new(left, centers[0] - 8.0), Vec2::new(32.0, 18.0)), ui.pixels_per_point());
        let response = ui.interact(switch, ui.scope_id().with("island_settings_sound"), Sense::click()).on_hover_cursor(CursorIcon::PointingHand);
        let on = ui.ctx().animate_bool_with_time(response.id, view.sound_on, 0.18);
        ui.painter().rect_filled(switch, 9, Color32::from_white_alpha(38).lerp_to_gamma(GREEN, on));
        ui.painter().circle_filled(Pos2::new(switch.left() + 9.0 + 14.0 * on, switch.center().y), 7.0, Color32::WHITE);
        out.sound |= response.clicked();
        let label = ui.painter().text(Pos2::new(switch.right() + 10.0, centers[0]), egui::Align2::LEFT_CENTER, "Som", row_font.clone(), ROW_INK);
        let track = Rect::from_min_size(Pos2::new(label.right() + 10.0, centers[0] - 6.0), Vec2::new(72.0, 12.0));
        let response = ui.interact(track, ui.scope_id().with("island_volume"), Sense::click_and_drag()).on_hover_cursor(CursorIcon::PointingHand);
        if let Some(pointer) = response.interact_pointer_pos().filter(|_| response.dragged() || response.clicked()) {
            let share = ((pointer.x - track.left()) / track.width()).clamp(0.0, 1.0);
            out.volume = Some((share * 0.2 / 0.005).round() * 0.005);
        }
        let fade = if view.sound_on { 1.0 } else { 0.4 };
        let share = out.volume.unwrap_or(view.volume).clamp(0.0, 0.2) / 0.2;
        let line = Rect::from_center_size(track.center(), Vec2::new(track.width(), 3.0));
        ui.painter().rect_filled(line, 2, Color32::from_white_alpha(51).gamma_multiply(fade));
        ui.painter().circle_filled(Pos2::new(line.left() + line.width() * share, line.center().y), 5.5, Color32::WHITE.gamma_multiply(fade));
        for (index, color) in ISLAND_COLORS.into_iter().rev().enumerate() {
            let center = Pos2::new(right - 6.0 - index as f32 * 18.0, centers[0]);
            let response = ui
                .interact(Rect::from_center_size(center, Vec2::splat(16.0)), ui.scope_id().with(("island_color", index)), Sense::click())
                .on_hover_cursor(CursorIcon::PointingHand)
                .on_hover_text("Cor da ilha");
            ui.painter().circle_filled(center, 5.5, color);
            ui.painter().circle_stroke(center, 5.5, Stroke::new(1.0, Color32::from_white_alpha(40)));
            if color == view.color || response.hovered() {
                ui.painter().circle_stroke(center, 8.0, Stroke::new(1.2, if color == view.color { INK } else { DIM }));
            }
            if response.clicked() {
                out.color = Some(color);
            }
        }
        // 2: how long the open island stays after the pointer left
        icons::paint(ui.painter(), icons::TIMER, Pos2::new(left + 6.0, centers[1]), 12.0, ROW_INK);
        ui.painter().text(Pos2::new(left + 22.0, centers[1]), egui::Align2::LEFT_CENTER, format!("Fechar auto · {}s", view.close_after), row_font, ROW_INK);
        let mut x = right;
        for seconds in [30_u8, 15, 10, 5] {
            // `.seg button`: 11px, padding 3 7, a pill
            let galley = ui.painter().layout_no_wrap(format!("{seconds}s"), font(11.0), DIM_3);
            let chip = Rect::from_min_max(Pos2::new(x - galley.size().x - 14.0, centers[1] - 9.5), Pos2::new(x, centers[1] + 9.5));
            x = chip.left() - 6.0;
            let response = ui.interact(chip, ui.scope_id().with(("island_close_after", seconds)), Sense::click()).on_hover_cursor(CursorIcon::PointingHand);
            let chosen = seconds == view.close_after;
            if chosen {
                ui.painter().rect_filled(chip, 10, Color32::from_rgb(0x25, 0x28, 0x30));
            }
            let ink = if chosen { INK } else { DIM_3 };
            ui.painter().text(chip.center(), egui::Align2::CENTER_CENTER, format!("{seconds}s"), font(11.0), ink);
            if response.clicked() {
                out.close_after = Some(seconds);
            }
        }
        // 3: the hooks of Claude Code, the API, and the app's settings
        let mut x = left;
        for (name, color) in [("Claude Code", if self.prefs.agent_hooks { GREEN } else { RED }), ("API", RED)] {
            ui.painter().circle_filled(Pos2::new(x + 3.0, centers[2]), 3.0, color);
            let text = ui.painter().text(Pos2::new(x + 10.0, centers[2]), egui::Align2::LEFT_CENTER, name, font(11.0), DIM);
            x = text.right() + 14.0;
        }
        let galley = ui.painter().layout_no_wrap("Ajustes…".to_string(), font(11.5), DIM);
        let link = Rect::from_min_size(Pos2::new(right - galley.size().x, centers[2] - 8.0), Vec2::new(galley.size().x, 16.0));
        let response = ui.interact(link, ui.scope_id().with("island_app_settings"), Sense::click()).on_hover_cursor(CursorIcon::PointingHand);
        ui.painter().text(link.left_center(), egui::Align2::LEFT_CENTER, "Ajustes…", font(11.5), DIM);
        out.open_app_settings = response.clicked();
    }
}

/// A pill of the grid and its mini (the reference's 28 and 24).
pub(super) const PILL_H: f32 = 28.0;
const PILL_MINI: f32 = 24.0;

/// One pill of the grid: its mini wizard and its name. The pointer over it only lights it in its
/// colour; a click puts its agent in the card at the left, where Abrir and Conversar are.
/// The tabs of the header, in order: GitHub's and Spotify's only while their integration is on (its pill is there).
pub(super) fn header_tabs(lines: &[Line]) -> Vec<(Tab, crate::editor::icons::Svg, f32, &'static str)> {
    let on = |name: &str| lines.iter().any(|line| line.id.strip_prefix(crate::app::integrations::ITEM_PREFIX) == Some(name));
    let mut tabs = vec![
        (Tab::Home, icons::HOUSE, 17.0, "Início: o resumo"),
        (Tab::Chat, icons::BUBBLE, 19.0, "Chat atual"),
        (Tab::Chats, icons::LIST, 17.0, "Conversas: os agentes do app"),
        (Tab::Claude, icons::CLAUDE, 16.0, "Claude Code: as sessões no canvas e fora do app"),
    ];
    if on("github") {
        tabs.push((Tab::GitHub, icons::PULL, 18.0, "GitHub: os pull requests"));
    }
    if on("spotify") {
        tabs.push((Tab::Music, icons::NOTE, 17.0, "Spotify"));
    }
    tabs.push((Tab::New, icons::PLUS, 16.0, "Chat novo num espaço novo"));
    tabs
}

pub(super) fn pill(ui: &mut egui::Ui, line: &Line, width: f32, opens: bool, time: f64, out: &mut Out) {
    let (rect, response) = ui.allocate_exact_size(Vec2::new(width, PILL_H), Sense::click());
    let response = response.on_hover_cursor(CursorIcon::PointingHand);
    let hovered = response.hovered();
    // it grows on hover (scale 1.04), within its own room: past it the scroll area would cut its edge
    let rect = if hovered { rect } else { rect.shrink(1.0) };
    let edge = match (hovered, line.asleep) {
        (true, _) => 0x8c,
        (false, true) => 0x1e,
        (false, false) => 0x24,
    };
    ui.painter().rect(rect, 14, CARD_FLAT, Stroke::new(1.0, with_alpha(line.color, edge)), StrokeKind::Inside);
    if hovered {
        // `${color}2e` over the card, `0 2px 10px ${color}59` around it
        let shadow = egui::epaint::Shadow { offset: [0, 2], blur: 10, spread: 0, color: with_alpha(line.color, 0x59) };
        ui.painter().add(shadow.as_shape(rect, egui::CornerRadius::same(14)));
        ui.painter().rect_filled(rect, 14, CARD_FLAT);
        ui.painter().rect(rect, 14, with_alpha(line.color, 0x2e), Stroke::new(1.0, with_alpha(line.color, 0x8c)), StrokeKind::Inside);
    }
    // a mini of `PILL_MINI` points 7 in, no ring around it
    let mini = Pos2::new(rect.left() + 7.0 + PILL_MINI / 2.0, rect.center().y);
    line.paint_mini(ui.painter(), &format!("island-pill-{}", line.id), mini, PILL_MINI, time);
    // the name, 600 10px, centred in the room after the mini (padding 0 8 0 34), lightened 30% toward white under the
    // pointer (`lighten(task.color, 0.3)`)
    let ink = if hovered { lighten(line.color, 0.3) } else { DIM_3 };
    let room = Rect::from_min_max(Pos2::new(rect.left() + 10.0 + PILL_MINI, rect.top()), Pos2::new(rect.right() - 8.0, rect.bottom()));
    // centred when it fits; cut, it scrolls under the pointer
    let whole = ui.painter().layout_no_wrap(line.name.clone(), semi(10.0), ink).size().x;
    let left = if whole <= room.width() { room.center().x - whole / 2.0 } else { room.left() };
    marquee(ui, egui::Id::unique(("island_pill_name", &line.id)), &line.name, semi(10.0), ink, (left, room.center().y), room.width(), hovered);
    if line.id.starts_with(crate::app::maestro::outside::PILL_PREFIX) {
        shoulder_mark(ui.painter(), mini, PILL_MINI, line, time);
    } else {
        mini_badge(ui.painter(), mini, PILL_MINI, line, time);
        // `pillBadge`: an integration's on the pill's corner (what failed there)
        if line.badge.is_some() {
            badge(ui.painter(), Pos2::new(rect.right() - 10.0, rect.top() + 2.0), line, 7.0);
        }
    }
    // a click again on the one put in the card opens it, as its Abrir (`opens_on_click`)
    if response.clicked() {
        out.focus = Some(line.id.clone());
        out.picked = true;
        if opens {
            out.open_agent = Some(line.id.clone());
        }
    }
}

fn with_alpha(color: Color32, alpha: u8) -> Color32 {
    Color32::from_rgba_unmultiplied(color.r(), color.g(), color.b(), alpha)
}

/// `color` moved `amount` of the way to white.
fn lighten(color: Color32, amount: f32) -> Color32 {
    let toward = |channel: u8| (channel as f32 + (255.0 - channel as f32) * amount).round() as u8;
    Color32::from_rgb(toward(color.r()), toward(color.g()), toward(color.b()))
}

/// The island's type: the system face, as the reference's (`--font: system-ui`), whose sizes are CSS pixels, the same
/// as egui's points.
pub(super) fn font(size: f32) -> FontId {
    FontId::new(size, egui::FontFamily::Name(crate::app::ui::SYSTEM_FONT.into()))
}

/// Its weight 600.
pub(super) fn semi(size: f32) -> FontId {
    FontId::new(size, egui::FontFamily::Name(crate::app::ui::SYSTEM_SEMIBOLD.into()))
}

/// The colours the island can take (`Prefs.island_color`), black the first.
const ISLAND_COLORS: [Color32; 5] = [
    Color32::BLACK,
    Color32::from_rgb(0x22, 0x23, 0x27),
    Color32::from_rgb(0x18, 0x2a, 0x4e),
    Color32::from_rgb(0x1f, 0x3a, 0x2c),
    Color32::from_rgb(0x3a, 0x1f, 0x3f),
];

/// The dot on a pill's corner when something waits there: an amber "!" for a question or a
/// permission, a green ✓ for a turn that just ended. `radius` 7 is the 14 px of the pill's own.
fn badge(painter: &Painter, center: Pos2, line: &Line, radius: f32) {
    let (color, mark) = match (line.badge, line.state, line.mago) {
        (Some(Badge::Error), ..) => (RED, "✕"),
        (Some(Badge::Approval), ..) | (_, AgentState::Waiting, _) => (AMBER, "!"),
        (Some(Badge::Finished), ..) | (_, _, MagoState::Done) => (Color32::from_rgb(0x22, 0xc5, 0x5e), "✓"),
        (_, _, MagoState::Error) => (RED, "✕"),
        _ => return,
    };
    painter.circle_filled(center, radius, BLACK);
    painter.circle_filled(center, radius - 1.0, color);
    if radius > 5.0 {
        painter.text(center, egui::Align2::CENTER_CENTER, mark, font(radius), Color32::BLACK);
    }
}

/// What a session outside is at, on its mini's shoulder: `!` it waits on the person, `✓` its turn ended, `✕` it failed,
/// a `z` that drifts up while it is stopped; working, its mini is awake and says it.
pub(super) fn shoulder_mark(painter: &Painter, center: Pos2, body: f32, line: &Line, time: f64) {
    let at = center + Vec2::new(body * 0.42, -body * 0.4);
    let mark = match (line.badge, line.state) {
        (Some(Badge::Approval), _) | (_, AgentState::Waiting) => Some((AMBER, "!", rgb_hex(0x2b1a00))),
        (Some(Badge::Finished), _) => Some((rgb_hex(0x34d399), "✓", rgb_hex(0x062b1d))),
        (Some(Badge::Error), _) => Some((rgb_hex(0xf4505e), "✕", rgb_hex(0x2b0006))),
        _ => None,
    };
    let radius = (body * 0.23).max(4.5);
    if let Some((color, glyph, ink)) = mark {
        // the ring of the card around it (`box-shadow: 0 0 0 2px`)
        painter.circle_filled(at, radius + 2.0, CARD_FLAT);
        painter.circle_filled(at, radius, color);
        painter.text(at + Vec2::new(0.0, 0.5), egui::Align2::CENTER_CENTER, glyph, semi(radius * 1.55), ink);
    } else if line.asleep {
        for (delay, size) in [(0.0, radius * 1.6), (0.5, radius * 1.2)] {
            let phase = ((time * 0.45 + delay) % 1.0) as f32;
            let fade = (1.0 - phase) * (phase * 4.0).min(1.0);
            let drift = Vec2::new(phase * 3.0, -phase * 7.0 - 1.0);
            painter.text(at + drift, egui::Align2::CENTER_CENTER, "z", semi(size), with_alpha(DIM, (fade * 230.0) as u8));
        }
        painter.ctx().request_repaint_after(std::time::Duration::from_millis(66));
    }
}

/// The state's badge on a mini of `body` centred on `center`, as the reference's engine draws it (`drawBadge`, a mini's
/// 1.25 bigger): on its upper left, a black ring and a disc of the state's colour; working, the disc pulses.
fn mini_badge(painter: &Painter, center: Pos2, body: f32, line: &Line, time: f64) {
    // (ring, disc) in the body's radius, and the colour
    let radius = body / 2.0;
    let pulse = 1.0 + 0.25 * ((time * 2.4).fract() as f32 * std::f32::consts::TAU).sin();
    let (ring, disc, color) = match (line.badge, line.state, line.mago) {
        (Some(Badge::Error), ..) | (_, _, MagoState::Error) => (0.2, 0.135, rgb_hex(0xf4505e)),
        (Some(Badge::Approval), ..) | (_, AgentState::Waiting, _) | (_, _, MagoState::NeedsYou) => (0.3, 0.23, rgb_hex(0xf5a524)),
        (_, _, MagoState::Asking) => (0.3, 0.23, rgb_hex(0x22d3ee)),
        (Some(Badge::Finished), ..) | (_, _, MagoState::Done) => (0.2, 0.135, rgb_hex(0x34d399)),
        (_, _, MagoState::Working | MagoState::Listening | MagoState::Speaking) => (0.2, 0.22 * pulse, rgb_hex(0x3b9eff)),
        _ => return,
    };
    let at = center - Vec2::splat(radius * 0.72);
    painter.circle_filled(at, radius * ring * 1.25, Color32::BLACK);
    painter.circle_filled(at, radius * disc * 1.25, color);
}

pub(super) const EXPAND: &[&[(f32, f32)]] =
    &[&[(14.0, 4.5), (19.5, 4.5), (19.5, 10.0)], &[(19.5, 4.5), (13.0, 11.0)], &[(10.0, 19.5), (4.5, 19.5), (4.5, 14.0)], &[(4.5, 19.5), (11.0, 13.0)]];

impl DrawingApp {
    /// The pill of who asks: a session outside's (by its pill) or the chat's.
    pub(super) fn ask_line<'a>(&self, view: &'a View) -> Option<&'a Line> {
        let chat = view.asks.first()?;
        let asks = self.person_asks.borrow();
        let source = asks.iter().find(|ask| &ask.chat == chat).and_then(|ask| ask.source.as_ref());
        let line = source.map_or(chat.clone(), |source| format!("{}{}", crate::app::maestro::outside::PILL_PREFIX, source.pill));
        view.lines.iter().find(|candidate| candidate.id == line)
    }

    /// The conversation's edges (those away from the edge of the screen it is on, and their corners): pulled, they size
    /// it, around the point it hangs from (`window::hung`), so the island stays hung from it. They lie over the panel.
    fn island_edges(&self, ui: &mut egui::Ui, rect: Rect, out: &mut Out) {
        let band = 8.0;
        let zones = edge_zones(rect, self.island.dock, band);
        egui::Area::new(egui::Id::unique("island_edges")).order(egui::Order::Tooltip).fixed_pos(Pos2::ZERO).constrain(false).show(ui.ctx(), |ui| {
            for (index, (zone, cursor, width, height)) in zones.into_iter().enumerate() {
                let response = ui.interact(zone, ui.scope_id().with(("island_edge", index)), Sense::click_and_drag()).on_hover_cursor(cursor);
                out.unpull |= response.double_clicked();
                let Some(pointer) = ui.input(|input| input.pointer.latest_pos()).filter(|_| response.dragged()) else { continue };
                out.resize = Some(pulled_to(rect, self.island.dock, pointer, width, height));
            }
        });
    }
}

/// The conversation's edges that size it, as (zone, cursor, pulls the width, pulls the height): those away from the
/// edge of the screen it is on (`dock`), `band` thick, and the corners where two of them meet. The sides start under the
/// header, the top one (on the bottom edge of the screen) lies over its first points.
pub(super) fn edge_zones(rect: Rect, dock: super::window::Dock, band: f32) -> Vec<(Rect, CursorIcon, bool, bool)> {
    use super::window::Dock;
    let (left, right, top, bottom) = (rect.left(), rect.right(), rect.top(), rect.bottom());
    let zone = |x0: f32, y0: f32, x1: f32, y1: f32| Rect::from_min_max(Pos2::new(x0, y0), Pos2::new(x1, y1));
    let (pull_left, pull_right) = (dock != Dock::Left, dock != Dock::Right);
    let from_top = dock == Dock::Bottom;
    // the side zones run between the header (or the top band) and the bottom band (or the bottom)
    let (side_top, side_bottom) = if from_top { (top + band, bottom) } else { (top + PAGE_TOP, bottom - band) };
    let mut zones = Vec::new();
    if pull_left {
        zones.push((zone(left, side_top, left + band, side_bottom), CursorIcon::ResizeHorizontal, true, false));
    }
    if pull_right {
        zones.push((zone(right - band, side_top, right, side_bottom), CursorIcon::ResizeHorizontal, true, false));
    }
    let (row_top, row_bottom) = if from_top { (top, top + band) } else { (bottom - band, bottom) };
    let row_left = if pull_left { left + band } else { left };
    let row_right = if pull_right { right - band } else { right };
    zones.push((zone(row_left, row_top, row_right, row_bottom), CursorIcon::ResizeVertical, false, true));
    // the corners: up-right to down-left on the left at the bottom and on the right at the top, the other way round else
    if pull_left {
        let cursor = if from_top { CursorIcon::ResizeNwSe } else { CursorIcon::ResizeNeSw };
        zones.push((zone(left, row_top, left + band, row_bottom), cursor, true, true));
    }
    if pull_right {
        let cursor = if from_top { CursorIcon::ResizeNeSw } else { CursorIcon::ResizeNwSe };
        zones.push((zone(right - band, row_top, right, row_bottom), cursor, true, true));
    }
    zones
}

/// The conversation's size with the `pointer` on an edge that pulls its `width` and/or `height`: it grows around the
/// point it hangs from (`window::hung`), so the far side goes as far as the pointer from that point; the island centred
/// on an axis grows on both sides of it.
pub(super) fn pulled_to(rect: Rect, dock: super::window::Dock, pointer: Pos2, width: bool, height: bool) -> Vec2 {
    use super::window::Dock;
    let wide = match dock {
        Dock::Left => pointer.x - rect.left(),
        Dock::Right => rect.right() - pointer.x,
        _ => 2.0 * (pointer.x - rect.center().x).abs(),
    };
    let high = match dock {
        Dock::Bottom => rect.bottom() - pointer.y,
        Dock::Left | Dock::Right => 2.0 * (pointer.y - rect.center().y).abs(),
        Dock::Top | Dock::Free => pointer.y - rect.top(),
    };
    Vec2::new(if width { wide } else { rect.width() }, if height { high } else { rect.height() })
}

/// Where the wizard's face is on the closed island around a notch while a song shows: after the cover
/// (`pill::COVER_LEFT_SMALL` and 16 wide), 4 from it.
const NOTCH_LUNA_SONG: f32 = 4.0 + 16.0 + 4.0 + 9.0;
/// The curve of the island's top corners around a notch: the screen's own corners beside the notch have it, so the
/// island flows out of the bezel as the notch does (about 8 points in photos of a 13" Air).
const EAR: f32 = 8.0;

/// The island's top corners turned out (`EAR`): from its top edge it curves out into the bar beside it, as the notch
/// it grows out of does. Each ear is filled from its corner on the island's side, which sees all of its curve; it goes
/// a point into the island so no seam of their two soft edges shows.
pub(super) fn ears(painter: &Painter, rect: Rect, color: Color32) {
    let steps = 12;
    for side in [-1.0f32, 1.0] {
        let edge = if side < 0.0 { rect.left() } else { rect.right() };
        // the circle's centre, out beside the island
        let centre = Pos2::new(edge + side * EAR, rect.top() + EAR);
        let corner = Pos2::new(edge - side, rect.top());
        let arc = (0..=steps).map(|step| {
            let turn = std::f32::consts::FRAC_PI_2 * step as f32 / steps as f32;
            // from the island's side, at the circle's height, up to the top edge
            centre + EAR * Vec2::new(-side * turn.cos(), -turn.sin())
        });
        let mut points: Vec<Pos2> = std::iter::once(corner).chain(std::iter::once(Pos2::new(edge - side, rect.top() + EAR))).chain(arc).collect();
        // clockwise on the screen, as the fill wants it (else it turns the points round and starts elsewhere)
        if side > 0.0 {
            points[1..].reverse();
        }
        painter.add(egui::Shape::convex_polygon(points, color, Stroke::NONE));
    }
}

/// The island's corners: square on the edge of the screen it is on.
pub(super) fn corners(round: u8, dock: super::window::Dock) -> egui::CornerRadius {
    use super::window::Dock;
    let corner = |square: bool| if square { 0 } else { round };
    let (top, bottom, left, right) = (dock == Dock::Top, dock == Dock::Bottom, dock == Dock::Left, dock == Dock::Right);
    egui::CornerRadius { nw: corner(top || left), ne: corner(top || right), sw: corner(bottom || left), se: corner(bottom || right) }
}

/// The wizard looking at `pointer`, or the window's own pointer when there is none.
pub(super) fn mago(painter: &Painter, pointer: Option<Pos2>, id: &str, center: Pos2, size: f32, state: MagoState, time: f64) {
    if size < 1.0 {
        return;
    }
    paint_mago_toward(painter, id, center, size, state, time, pointer.or_else(|| painter.ctx().pointer_latest_pos()));
}

/// The colour a card washes over its bottom with, by state: amber for a permission, green for
/// done, red for an error; none for the rest.
pub(super) fn wash(painter: &Painter, card: Rect, state: MagoState) {
    let (color, strength) = match state {
        MagoState::NeedsYou | MagoState::Asking => (AMBER, 0.42),
        MagoState::Done => (Color32::from_rgb(0x34, 0xd3, 0x99), 0.5),
        MagoState::Error => (RED, 0.55),
        _ => return,
    };
    wash_with(painter, card, color, strength, 0.0);
}

/// A wash of `color` over the whole card, strongest under its bottom middle, `shift` points to the side: an ellipse
/// as wide as the card, in layers.
pub(super) fn wash_with(painter: &Painter, card: Rect, color: Color32, strength: f32, shift: f32) {
    let center = Pos2::new(card.center().x + shift, card.bottom() + card.height() * 0.15);
    let reach = wash_reach(card);
    for step in 0..24 {
        let share = 1.0 - step as f32 / 24.0 * 0.9;
        painter.add(egui::Shape::ellipse_filled(center, reach * share, color.gamma_multiply(strength * 0.055)));
    }
}

/// How far the wash reaches from its middle: past the card's sides and up over its top.
pub(crate) fn wash_reach(card: Rect) -> Vec2 {
    Vec2::new((card.width() * 0.62).max(150.0), (card.height() * 1.2).max(150.0))
}

/// Whether Luna keeps dancing to the song in `state`: only what needs the person (a question, a permission, an error)
/// stops her; an agent at work or done does not.
pub(crate) fn dances_through(state: MagoState) -> bool {
    matches!(state, MagoState::Idle | MagoState::Sleeping | MagoState::Working | MagoState::Done)
}
