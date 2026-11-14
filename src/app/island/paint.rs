//! What the island draws, as Coucou's: the black notch (closed: the wizard and one mini wizard per agent), and open the
//! header (tabs at the left, buttons at the right) over Home or the page of a tab. It only draws and says what was asked
//! (`Out`); `island_apply` does it.

use super::{COMPACT_W, EXPANDED_W, Line, Mood, NewForm, Tab, agent_color, main_state};
use crate::app::chat::list::{CROSS, Look, clipped, glyph};
use crate::app::island::forms;
use crate::app::mascot::mago::{MagoState, paint_mago_for};
use crate::app::ui::agents::{AgentRow, AgentState};
use crate::prelude::*;
use crate::voice::VoiceState;
use std::sync::atomic::Ordering;

pub(super) const INK: Color32 = Color32::from_rgb(0xf5, 0xf6, 0xf8);
pub(super) const DIM: Color32 = Color32::from_rgb(0x8e, 0x93, 0x9c);
pub(super) const DIM_3: Color32 = Color32::from_rgb(0x6b, 0x70, 0x79);
const CARD: Color32 = Color32::from_rgb(0x14, 0x15, 0x18);
const CARD_FLAT: Color32 = Color32::from_rgb(0x0e, 0x0f, 0x11);
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
    pub(super) tab: Tab,
    pub(super) voice_on: bool,
    pub(super) voice_state: VoiceState,
    pub(super) mic: f32,
    pub(super) voice_note: String,
    pub(super) celebrating: bool,
    /// The agent the Home card shows.
    pub(super) focus: Option<String>,
    /// The row of the conversation shown.
    pub(super) chat_row: Option<AgentRow>,
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
    pub(super) hovered: bool,
    /// The agent whose pill the pointer is on.
    pub(super) focus: Option<String>,
    /// The height of the page's content (for the pages that grow with it).
    pub(super) body: f32,
}

impl DrawingApp {
    pub(super) fn island_view(&self, rows: Vec<AgentRow>, asks: Vec<String>, mood: Mood, time: f64) -> View {
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
                Line {
                    id: row.id.clone(),
                    name: self.chat_short_name(row),
                    detail,
                    state: row.state,
                    mago: MagoState::from_agent(row.state, self.now),
                    color: agent_color(&program, &row.id),
                    chat: self.is_chat(&row.id),
                }
            })
            .collect();
        // who needs the person first, then who works
        lines.sort_by_key(|line| match line.state {
            AgentState::Waiting => 0,
            AgentState::Working(_) => 1,
            _ => 2,
        });
        let island = &self.island;
        let focus = island.focus.clone().filter(|id| lines.iter().any(|line| &line.id == id)).or_else(|| lines.first().map(|line| line.id.clone()));
        let chat_row = self.chat_agent.as_ref().and_then(|id| rows.iter().find(|row| &row.id == id)).cloned();
        View {
            mood,
            lines,
            asks,
            tab: island.tab,
            voice_on: self.voice.on,
            voice_state: self.voice.state,
            mic: f32::from_bits(self.voice.shared.level.load(Ordering::Relaxed)).min(1.0),
            voice_note: self.voice.status.clone(),
            celebrating: time < island.celebrate_until,
            focus,
            chat_row,
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
        ui.painter().rect_filled(rect, egui::CornerRadius { nw: 0, ne: 0, sw: round, se: round }, Color32::BLACK);
        out.hovered = ui.input(|input| input.pointer.hover_pos()).is_some_and(|pointer| rect.contains(pointer));
        // the whole island moves by the press; the closed one opens by the click
        let grip = ui.interact(rect, ui.scope_id().with("island_grip"), Sense::click_and_drag());
        out.drag = grip.drag_started();
        out.wake = grip.clicked() && anim.x < (COMPACT_W + EXPANDED_W) / 2.0;
        // the closed content gives way to the open one as the island grows
        let grown = ((anim.x - COMPACT_W) / (EXPANDED_W - COMPACT_W)).clamp(0.0, 1.0);
        let (closed, open) = (((0.5 - grown) * 2.0).clamp(0.0, 1.0), ((grown - 0.5) * 2.0).clamp(0.0, 1.0));
        if closed > 0.0 {
            ui.scope_builder(egui::UiBuilder::new().max_rect(rect), |ui| {
                ui.set_opacity(closed);
                self.island_closed(ui, rect, view, time);
            });
        }
        if open > 0.0 {
            let body = ui
                .scope_builder(egui::UiBuilder::new().max_rect(rect), |ui| {
                    ui.set_opacity(open);
                    ui.set_clip_rect(rect.intersect(ui.clip_rect()));
                    self.island_open(ui, rect, view, form, look, colors, time, &mut out);
                })
                .response;
            let _ = body;
        }
        out
    }

    /// The closed island: the wizard at the left, a mini one per agent in a 2 by 2 grid at the right, in its colour.
    fn island_closed(&self, ui: &mut egui::Ui, rect: Rect, view: &View, time: f64) {
        let state = main_state(view.mood, view.celebrating);
        paint_mago_for(ui.painter(), "island-main", Pos2::new(rect.left() + 40.0, rect.center().y + 1.0), 26.0, state, time);
        let origin = Pos2::new(rect.right() - 40.0 - 14.5, rect.center().y - 14.5);
        for (index, line) in view.lines.iter().take(4).enumerate() {
            let center = origin + Vec2::new((index % 2) as f32 * 16.0 + 6.5, (index / 2) as f32 * 16.0 + 6.5);
            ui.painter().circle(center, 6.5, line.color.gamma_multiply(0.28), Stroke::new(1.0, line.color));
            paint_mago_for(ui.painter(), &format!("island-mini-{}", line.id), center + Vec2::new(0.0, 0.5), 12.0, line.mago, time);
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
            Tab::Home => self.island_home(ui, page, view, time, out),
            Tab::Chat => match &view.chat_row {
                Some(row) => {
                    self.refresh_transcript();
                    self.mini.window = true;
                    self.mini_panel(ui.ctx(), row, page, look, colors, time);
                    self.mini.window = false;
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
    }

    /// Home: the agent in focus in a card at the left (the wizard big, over a glow of its state's colour) and the grid of
    /// agents at the right; a click on one opens it.
    fn island_home(&self, ui: &mut egui::Ui, page: Rect, view: &View, time: f64, out: &mut Out) {
        let left = Rect::from_min_size(page.min, Vec2::new(322.0, page.height()));
        let right = Rect::from_min_max(Pos2::new(left.right() + 10.0, page.top()), page.max);
        for card in [left, right] {
            ui.painter().rect(card, 20, CARD, Stroke::new(1.0, Color32::from_white_alpha(9)), StrokeKind::Inside);
        }
        let focus = view.focus.as_ref().and_then(|id| view.lines.iter().find(|line| &line.id == id));
        let state = focus.map_or(main_state(view.mood, view.celebrating), |line| line.mago);
        let big = Pos2::new(left.left() + 58.0, left.center().y);
        glow(&ui.painter().with_clip_rect(left), big, 46.0, state);
        paint_mago_for(ui.painter(), "island-big", big, 62.0, state, time);
        let text_x = left.left() + 98.0;
        let width = left.right() - text_x - 14.0;
        match focus {
            Some(line) => {
                ui.painter().circle_filled(Pos2::new(text_x + 4.0, left.top() + 22.0), 4.0, line.color);
                let name = clipped(ui.painter(), line.name.clone(), FontId::proportional(13.0), INK, width - 14.0);
                ui.painter().galley(Pos2::new(text_x + 14.0, left.top() + 14.0), name.clone(), INK);
                ui.painter().galley(Pos2::new(text_x + 14.5, left.top() + 14.0), name, INK);
                let color = if line.state == AgentState::Waiting { AMBER } else { DIM };
                let detail = clipped(ui.painter(), line.detail.clone(), FontId::proportional(11.5), color, width);
                ui.painter().galley(Pos2::new(text_x, left.top() + 36.0), detail, color);
                let mut x = text_x;
                let y = left.bottom() - 38.0;
                if forms::chip(ui, Pos2::new(x, y), "Abrir", true).clicked() {
                    out.open_agent = Some(line.id.clone());
                }
                x += 66.0;
                if line.chat && forms::chip(ui, Pos2::new(x, y), "Conversar", false).clicked() {
                    out.talk = Some(line.id.clone());
                }
            }
            None => {
                ui.painter().text(Pos2::new(text_x, left.top() + 26.0), egui::Align2::LEFT_CENTER, "Nenhum agente ainda", FontId::proportional(13.0), INK);
                ui.painter().text(Pos2::new(text_x, left.top() + 46.0), egui::Align2::LEFT_CENTER, "Crie um chat em  +", FontId::proportional(11.5), DIM);
            }
        }
        // the grid: two columns of pills, a mini wizard and the name
        let inner = right.shrink2(Vec2::new(8.0, 6.0));
        ui.scope_builder(egui::UiBuilder::new().max_rect(inner), |ui| {
            ui.spacing_mut().item_spacing = Vec2::splat(4.0);
            ui.spacing_mut().scroll = egui::style::ScrollStyle::floating();
            let rows = view.lines.len().div_ceil(2) as f32;
            let content = rows * 28.0 + (rows - 1.0).max(0.0) * 4.0;
            ui.add_space(((inner.height() - content) / 2.0).max(0.0));
            egui::ScrollArea::vertical().id_salt("island_grid").auto_shrink([false, false]).show(ui, |ui| {
                let width = (inner.width() - 4.0) / 2.0;
                for pair in view.lines.chunks(2) {
                    ui.horizontal(|ui| {
                        for line in pair {
                            pill(ui, line, width, time, out);
                        }
                    });
                }
            });
        });
    }

    /// A question or a permission took the island over: the wizard asking at the left, the cards at the right.
    fn island_questions(&self, ui: &mut egui::Ui, page: Rect, view: &View, look: &Look, time: f64, out: &mut Out) {
        let big = Pos2::new(page.left() + 52.0, page.top() + 54.0);
        glow(&ui.painter().with_clip_rect(page), big, 46.0, MagoState::NeedsYou);
        paint_mago_for(ui.painter(), "island-big", big, 58.0, MagoState::NeedsYou, time);
        let cards = Rect::from_min_max(Pos2::new(page.left() + 98.0, page.top()), page.max);
        ui.scope_builder(egui::UiBuilder::new().max_rect(cards), |ui| {
            ui.spacing_mut().scroll = egui::style::ScrollStyle::floating();
            let scroll = egui::ScrollArea::vertical().id_salt("island_asks").auto_shrink([false, true]).max_height(cards.height()).show(ui, |ui| {
                ui.set_width(cards.width() - 6.0);
                ui.spacing_mut().item_spacing.y = 8.0;
                for chat in &view.asks {
                    self.decide_card(ui, chat, look, time);
                }
            });
            out.body = scroll.content_size.y;
        });
    }
}

/// One pill of the grid: its mini wizard in its colour and its name; clicked it opens the agent.
fn pill(ui: &mut egui::Ui, line: &Line, width: f32, time: f64, out: &mut Out) {
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
    let mini = Pos2::new(rect.left() + 17.0, rect.center().y);
    ui.painter().circle(mini, 9.0, line.color.gamma_multiply(0.25), Stroke::new(1.0, line.color));
    paint_mago_for(ui.painter(), &format!("island-pill-{}", line.id), mini + Vec2::new(0.0, 0.5), 17.0, line.mago, time);
    let label = clipped(ui.painter(), line.name.clone(), FontId::proportional(10.5), if hovered { INK } else { DIM }, rect.width() - 42.0);
    ui.painter().galley(Pos2::new(rect.left() + 34.0, rect.center().y - label.size().y / 2.0), label, DIM);
    if line.state == AgentState::Waiting {
        ui.painter().circle(Pos2::new(rect.right() - 8.0, rect.top() + 2.0), 5.0, AMBER, Stroke::new(2.0, Color32::BLACK));
    }
    if hovered {
        out.focus = Some(line.id.clone());
    }
    if response.clicked() {
        out.open_agent = Some(line.id.clone());
    }
}

/// The glow behind the wizard: the colour and strength of its state (Coucou's `botGlowColor` and `botGlowOpacity`).
fn glow(painter: &Painter, center: Pos2, radius: f32, state: MagoState) {
    let hex = |value: u32| Color32::from_rgb((value >> 16) as u8, (value >> 8) as u8, value as u8);
    let (color, strength) = match state {
        MagoState::Working | MagoState::Listening | MagoState::Speaking => (hex(0x3b9eff), 0.65),
        MagoState::NeedsYou => (hex(0xf5a524), 0.65),
        MagoState::Asking => (hex(0x6366f1), 0.65),
        MagoState::Done => (hex(0x34d399), 0.65),
        MagoState::Error => (hex(0xf4505e), 0.65),
        MagoState::Idle | MagoState::Sleeping => (Color32::WHITE, 0.15),
    };
    // concentric discs, each a little fainter and wider: a soft falloff
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
