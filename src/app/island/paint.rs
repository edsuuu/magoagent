//! What the island draws, as Coucou's: the black notch (closed: the wizard and one mini wizard per agent), and open the
//! header (tabs at the left, buttons at the right) over Home or the page of a tab. It only draws and says what was asked
//! (`Out`); `island_apply` does it.

use super::entry;
use super::minis::mini_cells;
use super::stage::{self, Activity, Phase};
use super::{EXPANDED_W, Line, Mood, NewForm, Tab, agent_color, alphas, main_state};
use crate::app::chat::list::{CROSS, Look, clipped, glyph};
use crate::app::island::forms;
use crate::app::maestro::outside::Badge;
use crate::app::mascot::mago::{MagoState, mago_pokes, paint_mago_toward, paint_mini};
use crate::app::ui::agents::{AgentRow, AgentState};
use crate::prelude::*;
use crate::voice::VoiceState;
use std::collections::HashMap;
use std::sync::atomic::Ordering;

pub(super) const INK: Color32 = Color32::from_rgb(0xf5, 0xf6, 0xf8);
pub(super) const DIM: Color32 = Color32::from_rgb(0x8e, 0x93, 0x9c);
pub(super) const DIM_3: Color32 = Color32::from_rgb(0x6b, 0x70, 0x79);
pub(super) const CARD: Color32 = Color32::from_rgb(0x14, 0x15, 0x18);
pub(super) const CARD_FLAT: Color32 = Color32::from_rgb(0x0e, 0x0f, 0x11);
const TAB_ON: Color32 = Color32::from_rgb(0x1d, 0x1f, 0x23);
pub(super) const AMBER: Color32 = Color32::from_rgb(0xf5, 0xa5, 0x24);
/// The header's height under the 8 above it: what the pages start under.
pub(super) const PAGE_TOP: f32 = 42.0;
const HOUSE: &[&[(f32, f32)]] = &[&[(3.5, 11.0), (12.0, 4.0), (20.5, 11.0)], &[(6.0, 9.5), (6.0, 20.0), (18.0, 20.0), (18.0, 9.5)]];
const PLUS: &[&[(f32, f32)]] = &[&[(12.0, 5.0), (12.0, 19.0)], &[(5.0, 12.0), (19.0, 12.0)]];
const BUBBLE: &[&[(f32, f32)]] =
    &[&[(12.0, 4.5), (17.0, 6.0), (19.5, 10.5), (18.5, 15.5), (14.5, 18.5), (10.0, 19.0), (6.0, 20.5), (6.8, 17.0), (4.8, 12.5), (6.5, 7.0), (12.0, 4.5)]];

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
    /// Where the wizard's eyes look, in the window's coordinates: the cursor when it is near (`cursor.rs`).
    pub(super) pointer: Option<Pos2>,
    pub(super) sound_on: bool,
    /// The agent the Home card shows.
    pub(super) focus: Option<String>,
    /// The row of the conversation shown.
    pub(super) chat_row: Option<AgentRow>,
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
    /// Dragged away from the top edge: all four corners round, a pill, not a notch.
    pub(super) floating: bool,
}

/// What was done on the island this frame.
#[derive(Default)]
pub(crate) struct Out {
    pub(super) open_agent: Option<String>,
    /// "Conversar" on the Home card: this chat in the island.
    pub(super) talk: Option<String>,
    pub(super) voice: bool,
    pub(super) tab: Option<Tab>,
    pub(super) close: bool,
    pub(super) settings: bool,
    pub(super) wake: bool,
    pub(super) create: bool,
    pub(super) pick_folder: bool,
    /// The press that moves the window: the system takes it from here.
    pub(super) drag: bool,
    /// The conversation pulled by an edge to this size, and the big/small button.
    pub(super) resize: Option<Vec2>,
    pub(super) big: bool,
    /// The sound button.
    pub(super) sound: bool,
    pub(super) hovered: bool,
    /// A button of the mouse is down.
    pub(super) holding: bool,
    /// The agent whose pill the pointer is on.
    pub(super) focus: Option<String>,
    /// The height of the page's content (for the pages that grow with it).
    pub(super) body: f32,
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
                let mut mago = MagoState::from_agent(row.state, self.now);
                if activity.as_ref().is_some_and(|activity| activity.phase == Phase::Failed) && row.state != AgentState::Waiting {
                    mago = MagoState::Error;
                }
                activities.extend(activity.map(|activity| (row.id.clone(), activity)));
                Line {
                    id: row.id.clone(),
                    name: self.chat_short_name(row),
                    detail,
                    state: row.state,
                    mago,
                    color: agent_color(&program, &row.id),
                    chat: self.is_chat(&row.id),
                    badge: None,
                    kind: stage::program_name(&program),
                }
            })
            .collect();
        // the sessions outside the app, one pill per origin
        lines.extend(self.outside_pills().into_iter().map(|pill| Line {
            kind: if pill.name == "Hermes" { "Hermes Agent" } else { "Claude Code" }.into(),
            mago: MagoState::from_agent(pill.state, self.now),
            id: pill.id,
            name: pill.name,
            detail: pill.state.label().to_string(),
            state: pill.state,
            color: pill.color,
            chat: false,
            badge: pill.badge,
        }));
        lines.extend(self.integration_lines());
        // who needs the person first, then who works
        lines.sort_by_key(|line| match line.state {
            AgentState::Waiting => 0,
            AgentState::Working(_) => 1,
            _ => 2,
        });
        let island = &self.island;
        let focus = island.focus.clone().filter(|id| lines.iter().any(|line| &line.id == id)).or_else(|| lines.first().map(|line| line.id.clone()));
        let chat_row = self.chat_agent.as_ref().and_then(|id| rows.iter().find(|row| &row.id == id)).cloned();
        let activity = focus.as_ref().and_then(|id| activities.remove(id));
        let failures = lines.iter().filter(|line| line.mago == MagoState::Error).count();
        let (pokes, dizzy) = mago_pokes("island-big", time);
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
            pointer: island.pointer,
            sound_on: self.prefs.island_sound,
            focus,
            chat_row,
            activity,
            since,
            glimpse,
            failures,
            pokes,
            dizzy,
            floating: island.floating,
            entrance: island.greeting.map(|start| (time - start) as f32).filter(|t| *t < entry::END),
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
        let top = if view.floating { round } else { 0 };
        ui.painter().rect_filled(rect, egui::CornerRadius { nw: top, ne: top, sw: round, se: round }, Color32::BLACK);
        out.hovered = ui.input(|input| input.pointer.hover_pos()).is_some_and(|pointer| rect.contains(pointer));
        // the whole island moves by the press; the closed and compact ones open by the click
        let grip = ui.interact(rect, ui.scope_id().with("island_grip"), Sense::click_and_drag());
        out.drag = grip.drag_started();
        out.holding = ui.input(|input| input.pointer.any_down()) && out.hovered;
        out.wake = grip.clicked() && anim.x < EXPANDED_W - 40.0;
        // each content gives way to the next as the island grows
        let (face, compact, open) = alphas(anim.x);
        for (alpha, mode) in [(face, 0), (compact, 1)] {
            if alpha > 0.0 {
                ui.scope_builder(egui::UiBuilder::new().max_rect(rect), |ui| {
                    ui.set_opacity(alpha);
                    ui.set_clip_rect(rect.intersect(ui.clip_rect()));
                    self.island_small(ui, rect, view, mode == 1, time);
                });
            }
        }
        if open > 0.0 {
            ui.scope_builder(egui::UiBuilder::new().max_rect(rect), |ui| {
                ui.set_opacity(open);
                ui.set_clip_rect(rect.intersect(ui.clip_rect()));
                self.island_open(ui, rect, view, form, look, colors, time, &mut out);
            });
        }
        self.island_keys(ui, view, &mut out);
        out
    }

    /// The closed notch: only the wizard's face, in the middle, looking around by itself (the cursor is not followed
    /// there, as in Coucou). The compact island: the wizard at the left, its eyes on the cursor, and a mini one per agent
    /// in a grid at the right (`mini_cells`), in its colour, with a badge on the ones that wait.
    fn island_small(&self, ui: &mut egui::Ui, rect: Rect, view: &View, compact: bool, time: f64) {
        let state = main_state(view.mood, view.celebrating);
        if !compact {
            paint_mago_toward(ui.painter(), "island-main", Pos2::new(rect.center().x, rect.center().y + 1.0), 22.0, state, time, None);
            return;
        }
        mago(ui.painter(), view.pointer, "island-main", Pos2::new(rect.left() + 40.0, rect.center().y + 1.0), 26.0, state, time);
        for (line, cell) in view.lines.iter().zip(mini_cells(rect, view.lines.len())) {
            paint_mini(ui.painter(), &format!("island-mini-{}", line.id), cell.center(), cell.width(), line.color, line.mago, time);
            badge(ui.painter(), Pos2::new(cell.right() - 1.5, cell.top() + 1.5), line, 3.5);
        }
        if view.voice_on {
            let bars = crate::app::voice::call_bars(view.voice_state, view.mic, time);
            crate::app::voice::paint_bars(ui.painter(), Pos2::new(rect.center().x, rect.center().y), bars, INK);
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
        if !view.asks.is_empty() {
            return self.island_questions(ui, page, view, look, time, out);
        }
        match view.tab {
            Tab::Home => self.island_home(ui, rect, page, view, time, out),
            Tab::Chat => match &view.chat_row {
                Some(row) => {
                    self.refresh_transcript();
                    (self.mini.window, self.mini.in_island) = (true, true);
                    self.mini_panel(ui.ctx(), row, page, look, colors, time);
                    (self.mini.window, self.mini.in_island) = (false, false);
                    // its ✕ (and Esc) and its full screen: the island folds, the chat going to the app with the latter
                    out.close |= std::mem::take(&mut self.mini.left);
                    self.island_edges(ui, rect, out);
                }
                None => {
                    let card = Rect::from_min_size(page.min, page.size());
                    ui.painter().rect_filled(card, 20, CARD);
                    ui.painter().text(card.center(), egui::Align2::CENTER_CENTER, "Escolha um agente na Home para conversar.", FontId::proportional(12.0), DIM);
                }
            },
            Tab::New => forms::new_chat_page(ui, page, form, look, out),
        }
    }

    /// The tabs (Home, conversation, new chat) at the left; the voice, the settings and the close at the right.
    fn island_header(&self, ui: &mut egui::Ui, rect: Rect, view: &View, out: &mut Out, time: f64) {
        let center_y = rect.top() + 8.0 + 17.0;
        let pinned = !view.asks.is_empty();
        let tabs = [(Tab::Home, HOUSE, 17.0, "Início"), (Tab::Chat, BUBBLE, 19.0, "Conversa"), (Tab::New, PLUS, 16.0, "Chat novo num espaço novo")];
        for (index, (tab, icon, size, tip)) in tabs.into_iter().enumerate() {
            let area = Rect::from_center_size(Pos2::new(rect.left() + 14.0 + 15.0 + index as f32 * 35.0, center_y), Vec2::new(30.0, 22.0));
            let response = ui.interact(area, ui.scope_id().with(("island_tab", index)), Sense::click()).on_hover_cursor(CursorIcon::PointingHand).tip(tip);
            let on = view.tab == tab && !pinned;
            if on {
                ui.painter().rect_filled(area, 11, TAB_ON);
            } else if response.hovered() {
                ui.painter().rect_filled(area, 11, Color32::from_white_alpha(18));
            }
            glyph(
                ui.painter(),
                area.center(),
                size,
                1.7,
                if on {
                    INK
                } else if response.hovered() {
                    Color32::from_rgb(0xb0, 0xb5, 0xbe)
                } else {
                    DIM
                },
                icon,
            );
            if response.clicked() {
                out.tab = Some(tab);
            }
        }
        // the call's note between them
        if !view.voice_note.is_empty() {
            let galley = clipped(ui.painter(), view.voice_note.clone(), FontId::proportional(11.0), DIM_3, rect.width() - 300.0);
            ui.painter().galley(Pos2::new(rect.left() + 130.0, center_y - galley.size().y / 2.0), galley, DIM_3);
        }
        // right to left: close, settings, voice
        let mut x = rect.right() - 16.0 - 9.0;
        let mut button = |ui: &mut egui::Ui, name: &str, tip: &str, on: bool| {
            let area = Rect::from_center_size(Pos2::new(x, center_y), Vec2::splat(24.0));
            x -= 32.0;
            let response = ui.interact(area, ui.scope_id().with(name), Sense::click()).on_hover_cursor(CursorIcon::PointingHand).tip(tip);
            let ink = if on || response.hovered() { INK } else { DIM };
            (response, area.center(), ink)
        };
        if !pinned {
            let (response, at, ink) = button(ui, "island_close", "Recolher", false);
            glyph(ui.painter(), at, 17.0, 1.8, ink, CROSS);
            out.close = response.clicked();
        }
        let (response, at, ink) = button(ui, "island_sound", if view.sound_on { "Silenciar os sons" } else { "Ligar os sons" }, false);
        speaker(ui.painter(), at, ink, view.sound_on);
        out.sound = response.clicked();
        let (response, at, ink) = button(ui, "island_settings", "Ajustes", false);
        gear(ui.painter(), at, ink);
        out.settings = response.clicked();
        let (response, at, ink) = button(ui, "island_voice", if view.voice_on { "Encerrar o chat por voz" } else { "Chat por voz" }, view.voice_on);
        if view.voice_on {
            crate::app::voice::paint_bars(ui.painter(), at, crate::app::voice::call_bars(view.voice_state, view.mic, time), ink);
        } else {
            crate::app::voice::paint_mic(ui.painter(), at, ink, false);
        }
        out.voice = response.clicked();
        // the conversation: its big and small size
        if view.tab == Tab::Chat && !pinned {
            let (response, at, ink) = button(ui, "island_big", "Aumentar ou voltar ao tamanho", false);
            glyph(ui.painter(), at, 16.0, 1.6, ink, EXPAND);
            out.big = response.clicked();
        }
    }

    /// A question or a permission took the island over: the wizard asking at the left, the cards at the right.
    fn island_questions(&self, ui: &mut egui::Ui, page: Rect, view: &View, look: &Look, time: f64, out: &mut Out) {
        let big = Pos2::new(page.left() + 52.0, page.top() + 54.0);
        ui.painter().rect_filled(page, 20, CARD);
        wash(&ui.painter().with_clip_rect(page), page, MagoState::NeedsYou);
        glow(&ui.painter().with_clip_rect(page), big, 46.0, MagoState::NeedsYou);
        mago(ui.painter(), view.pointer, "island-big", big, 58.0, MagoState::NeedsYou, time);
        let cards = Rect::from_min_max(Pos2::new(page.left() + 98.0, page.top()), page.max);
        ui.scope_builder(egui::UiBuilder::new().max_rect(cards), |ui| {
            ui.spacing_mut().scroll = egui::style::ScrollStyle::floating();
            let scroll = egui::ScrollArea::vertical().id_salt("island_asks").auto_shrink([false, true]).max_height(cards.height()).show(ui, |ui| {
                ui.set_width(cards.width() - 6.0);
                ui.spacing_mut().item_spacing.y = 8.0;
                for chat in &view.asks {
                    self.decide_card(ui, chat, look, time);
                }
                if view.queued > 0 {
                    ui.label(egui::RichText::new(format!("+{} na fila", view.queued)).size(11.0).color(DIM));
                }
            });
            out.body = scroll.content_size.y;
        });
    }
}

/// One pill of the grid: its mini wizard in its colour and its name; clicked it opens the agent.
pub(super) fn pill(ui: &mut egui::Ui, line: &Line, width: f32, time: f64, out: &mut Out) {
    let (rect, response) = ui.allocate_exact_size(Vec2::new(width, 28.0), Sense::click());
    let response = response.on_hover_cursor(CursorIcon::PointingHand);
    let hovered = response.hovered();
    let rect = if hovered { rect.expand(1.0) } else { rect };
    ui.painter().rect(
        rect,
        14,
        CARD_FLAT,
        Stroke::new(1.0, if hovered { line.color.gamma_multiply(0.7) } else { Color32::from_white_alpha(36) }),
        StrokeKind::Inside,
    );
    // Coucou's pill: a mini of 22 points, no ring around it
    paint_mini(ui.painter(), &format!("island-pill-{}", line.id), Pos2::new(rect.left() + 19.0, rect.center().y), 22.0, line.color, line.mago, time);
    let label = clipped(ui.painter(), line.name.clone(), FontId::proportional(10.5), if hovered { INK } else { DIM }, rect.width() - 42.0);
    ui.painter().galley(Pos2::new(rect.left() + 34.0, rect.center().y - label.size().y / 2.0), label, DIM);
    badge(ui.painter(), Pos2::new(rect.right() - 10.0, rect.top() + 2.0), line, 7.0);
    if hovered {
        out.focus = Some(line.id.clone());
    }
    if response.clicked() {
        out.open_agent = Some(line.id.clone());
    }
}

/// The glow behind the wizard: the colour and strength of its state (Coucou's `botGlowColor` and `botGlowOpacity`).
pub(super) fn glow(painter: &Painter, center: Pos2, radius: f32, state: MagoState) {
    let hex = |value: u32| Color32::from_rgb((value >> 16) as u8, (value >> 8) as u8, value as u8);
    let (color, strength) = match state {
        MagoState::Working | MagoState::Listening | MagoState::Speaking => (hex(0x3b9eff), 0.65),
        MagoState::NeedsYou => (hex(0xf5a524), 0.65),
        MagoState::Asking => (hex(0x6366f1), 0.65),
        MagoState::Done => (hex(0x34d399), 0.65),
        MagoState::Error => (hex(0xf4505e), 0.65),
        MagoState::Idle | MagoState::Sleeping => (Color32::WHITE, 0.15),
        MagoState::Waving => (Color32::from_rgb(0xe8, 0xc3, 0x9a), 0.4),
    };
    glow_with(painter, center, radius, color, strength);
}

/// A glow of `color` and `strength`: concentric discs, each a little fainter and wider, a soft falloff.
pub(super) fn glow_with(painter: &Painter, center: Pos2, radius: f32, color: Color32, strength: f32) {
    for step in 0..10 {
        let t = step as f32 / 10.0;
        painter.circle_filled(center, radius * (1.0 - t * 0.85), color.gamma_multiply(strength * 0.085));
    }
}

/// The settings' gear: a ring with teeth.
fn gear(painter: &Painter, center: Pos2, ink: Color32) {
    painter.circle_stroke(center, 5.5, Stroke::new(1.6, ink));
    painter.circle_stroke(center, 2.0, Stroke::new(1.4, ink));
    for tooth in 0..8 {
        let angle = tooth as f32 * std::f32::consts::FRAC_PI_4;
        let direction = Vec2::new(angle.cos(), angle.sin());
        painter.line_segment([center + direction * 6.5, center + direction * 8.5], Stroke::new(2.0, ink));
    }
}

/// The dot on a pill's corner when something waits there (Coucou's `pillBadge`): an amber "!" for a question or a
/// permission, a green ✓ for a turn that just ended. `radius` 7 is the 14 px of the pill's own.
fn badge(painter: &Painter, center: Pos2, line: &Line, radius: f32) {
    let (color, mark) = match (line.badge, line.state, line.mago) {
        (Some(Badge::Error), ..) => (Color32::from_rgb(0xf4, 0x50, 0x5e), "✕"),
        (Some(Badge::Approval), ..) | (_, AgentState::Waiting, _) => (AMBER, "!"),
        (Some(Badge::Finished), ..) | (_, _, MagoState::Done) => (Color32::from_rgb(0x22, 0xc5, 0x5e), "✓"),
        (_, _, MagoState::Error) => (Color32::from_rgb(0xf4, 0x50, 0x5e), "✕"),
        _ => return,
    };
    painter.circle_filled(center, radius, Color32::from_rgb(0x0b, 0x0c, 0x0e));
    painter.circle_filled(center, radius - 1.0, color);
    if radius > 5.0 {
        painter.text(center, egui::Align2::CENTER_CENTER, mark, FontId::proportional(radius), Color32::BLACK);
    }
}

const EXPAND: &[&[(f32, f32)]] =
    &[&[(14.0, 4.5), (19.5, 4.5), (19.5, 10.0)], &[(19.5, 4.5), (13.0, 11.0)], &[(10.0, 19.5), (4.5, 19.5), (4.5, 14.0)], &[(4.5, 19.5), (11.0, 13.0)]];

impl DrawingApp {
    /// The conversation's edges (the sides and the bottom, and the bottom corners): pulled, they size it, the sides
    /// symmetrically around the centre, so the island stays hung from the same point. They lie over the panel.
    fn island_edges(&self, ui: &mut egui::Ui, rect: Rect, out: &mut Out) {
        let band = 8.0;
        let (left, right, bottom) = (rect.left(), rect.right(), rect.bottom());
        let top = rect.top() + PAGE_TOP;
        // (zone, cursor, pulls the width, pulls the height)
        let zones = [
            (Rect::from_min_max(Pos2::new(left, top), Pos2::new(left + band, bottom - band)), CursorIcon::ResizeHorizontal, true, false),
            (Rect::from_min_max(Pos2::new(right - band, top), Pos2::new(right, bottom - band)), CursorIcon::ResizeHorizontal, true, false),
            (Rect::from_min_max(Pos2::new(left + band, bottom - band), Pos2::new(right - band, bottom)), CursorIcon::ResizeVertical, false, true),
            (Rect::from_min_max(Pos2::new(left, bottom - band), Pos2::new(left + band, bottom)), CursorIcon::ResizeNeSw, true, true),
            (Rect::from_min_max(Pos2::new(right - band, bottom - band), Pos2::new(right, bottom)), CursorIcon::ResizeNwSe, true, true),
        ];
        egui::Area::new(egui::Id::unique("island_edges")).order(egui::Order::Tooltip).fixed_pos(Pos2::ZERO).constrain(false).show(ui.ctx(), |ui| {
            for (index, (zone, cursor, width, height)) in zones.into_iter().enumerate() {
                let response = ui.interact(zone, ui.scope_id().with(("island_edge", index)), Sense::drag()).on_hover_cursor(cursor);
                let Some(pointer) = ui.input(|input| input.pointer.latest_pos()).filter(|_| response.dragged()) else { continue };
                // the island hangs from its top centre: the pointer's distance from it is the new half width and the height
                let size = Vec2::new(
                    if width { 2.0 * (pointer.x - rect.center().x).abs() } else { rect.width() },
                    if height { pointer.y - rect.top() } else { rect.height() },
                );
                out.resize = Some(size);
            }
        });
    }
}

/// The wizard looking at `pointer`, or the window's own pointer when there is none.
pub(super) fn mago(painter: &Painter, pointer: Option<Pos2>, id: &str, center: Pos2, size: f32, state: MagoState, time: f64) {
    if size < 1.0 {
        return;
    }
    paint_mago_toward(painter, id, center, size, state, time, pointer.or_else(|| painter.ctx().pointer_latest_pos()));
}

/// The sound button: a speaker, crossed out when the island's sounds are off.
fn speaker(painter: &Painter, center: Pos2, ink: Color32, on: bool) {
    let stroke = Stroke::new(1.6, ink);
    let cone = vec![
        center + Vec2::new(-8.0, -3.0),
        center + Vec2::new(-4.0, -3.0),
        center + Vec2::new(0.5, -7.0),
        center + Vec2::new(0.5, 7.0),
        center + Vec2::new(-4.0, 3.0),
        center + Vec2::new(-8.0, 3.0),
    ];
    painter.add(Shape::closed_line(cone, stroke));
    if on {
        painter.add(Shape::line(vec![center + Vec2::new(4.0, -3.5), center + Vec2::new(6.0, 0.0), center + Vec2::new(4.0, 3.5)], stroke));
    } else {
        painter.line_segment([center + Vec2::new(3.5, -3.5), center + Vec2::new(8.5, 3.5)], stroke);
        painter.line_segment([center + Vec2::new(8.5, -3.5), center + Vec2::new(3.5, 3.5)], stroke);
    }
}

/// The colour a card washes over its bottom with, by state (Coucou's `washRGBA`): amber for a permission, green for
/// done, red for an error; none for the rest.
pub(super) fn wash(painter: &Painter, card: Rect, state: MagoState) {
    let (color, strength) = match state {
        MagoState::NeedsYou | MagoState::Asking => (Color32::from_rgb(0xf5, 0xa5, 0x24), 0.42),
        MagoState::Done => (Color32::from_rgb(0x34, 0xd3, 0x99), 0.5),
        MagoState::Error => (Color32::from_rgb(0xf4, 0x50, 0x5e), 0.55),
        _ => return,
    };
    wash_with(painter, card, color, strength, 0.0);
}

/// A wash of `color`, `shift` points to the side of the card's middle.
pub(super) fn wash_with(painter: &Painter, card: Rect, color: Color32, strength: f32, shift: f32) {
    let center = Pos2::new(card.center().x + shift, card.bottom() + card.height() * 0.3);
    for step in 0..24 {
        painter.circle_filled(center, 150.0 * (1.0 - step as f32 / 24.0 * 0.9), color.gamma_multiply(strength * 0.055));
    }
}
