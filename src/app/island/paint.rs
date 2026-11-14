//! What the island draws: the pill (the mascot and the summary) and, open, the panel: the pending questions, the new
//! chat's form and the agents. It only draws and says what was asked (`Out`); `island_apply` does it.

use super::{Mood, NewForm, PANEL_WIDTH, PILL_HEIGHT};
use crate::app::chat::direct::folder_label;
use crate::app::chat::list::{AMBER, CROSS, Look, clipped, glyph, pulse_dot};
use crate::app::chat::thinking::{CLAUDE, spinner};
use crate::app::ui::agents::{AgentRow, AgentState};
use crate::prelude::*;
use crate::voice::VoiceState;
use std::sync::atomic::Ordering;

/// The open panel's header, and the most its body shows before it scrolls.
const HEAD: f32 = 52.0;
const MAX_BODY: f32 = 440.0;
const PLUS: &[&[(f32, f32)]] = &[&[(12.0, 6.0), (12.0, 18.0)], &[(6.0, 12.0), (18.0, 12.0)]];

/// One agent or chat in the list.
pub(super) struct Line {
    pub(super) id: String,
    pub(super) name: String,
    pub(super) detail: String,
    pub(super) state: AgentState,
}

/// What the island shows this frame.
pub(super) struct View {
    pub(super) mood: Mood,
    pub(super) lines: Vec<Line>,
    /// The chats waiting on a question or a permission (`decide_card`).
    pub(super) asks: Vec<String>,
    pub(super) open: bool,
    pub(super) voice_on: bool,
    pub(super) voice_state: VoiceState,
    pub(super) mic: f32,
    pub(super) voice_note: String,
    /// Seconds since the oldest agent at work began.
    pub(super) working_secs: f64,
    pub(super) working_seed: u64,
    pub(super) celebrating: bool,
    /// The panel's body as drawn last frame.
    pub(super) body: f32,
}

/// What was done on the island this frame.
#[derive(Default)]
pub(crate) struct Out {
    pub(super) open_agent: Option<String>,
    pub(super) voice: bool,
    pub(super) toggle_form: bool,
    pub(super) create: bool,
    pub(super) pick_folder: bool,
    /// The press that moves the window: the system takes it from here.
    pub(super) drag: bool,
    pub(super) hovered: bool,
    pub(super) size: Vec2,
    pub(super) body: f32,
}

impl DrawingApp {
    pub(super) fn island_view(&self, rows: Vec<AgentRow>, asks: Vec<String>, mood: Mood, open: bool, time: f64) -> View {
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
                Line { id: row.id.clone(), name: self.chat_short_name(row), detail, state: row.state }
            })
            .collect();
        // who needs the person first, then who works
        lines.sort_by_key(|line| match line.state {
            AgentState::Waiting => 0,
            AgentState::Working(_) => 1,
            _ => 2,
        });
        let oldest = lines.iter().filter_map(|line| if let AgentState::Working(since) = line.state { Some(since) } else { None }).fold(f64::MAX, f64::min);
        let oldest = if oldest == f64::MAX { time } else { oldest.min(time) };
        View {
            mood,
            lines,
            asks,
            open,
            voice_on: self.voice.on,
            voice_state: self.voice.state,
            mic: f32::from_bits(self.voice.shared.level.load(Ordering::Relaxed)).min(1.0),
            voice_note: self.voice.status.clone(),
            working_secs: time - oldest,
            working_seed: oldest.to_bits(),
            celebrating: time < self.island.celebrate_until,
            body: if self.island.body > 0.0 { self.island.body } else { 120.0 },
        }
    }

    /// Draws the island with its top left at `at`; the returned `Out` has what was done and the size it took.
    pub(super) fn island_draw(&self, ui: &mut egui::Ui, at: Pos2, view: &View, form: &mut Option<NewForm>, look: &Look, time: f64) -> Out {
        let mut out = Out::default();
        let needs = matches!(view.mood, Mood::Needs(_));
        let label = view.mood.label();
        let galley = ui.painter().layout_no_wrap(label, FontId::proportional(13.5), if needs { AMBER } else { look.text });
        let head = if view.open { HEAD } else { PILL_HEIGHT };
        let width = if view.open { PANEL_WIDTH } else { 10.0 + 34.0 + 8.0 + galley.size().x + if view.voice_on { 40.0 } else { 0.0 } + 16.0 };
        let height = if view.open { head + 4.0 + view.body + 12.0 } else { PILL_HEIGHT };
        let rect = Rect::from_min_size(at, Vec2::new(width, height));
        let radius = if view.open { 24 } else { 22 };
        ui.painter().add(egui::epaint::Shadow { offset: [0, 4], blur: 16, spread: 0, color: Color32::from_black_alpha(70) }.as_shape(rect, radius));
        ui.painter().rect_filled(rect, radius, look.bg);
        ui.painter().rect_stroke(rect, radius, Stroke::new(1.0, if needs { AMBER } else { look.line }), StrokeKind::Inside);
        // the whole header moves the island
        let head_rect = Rect::from_min_size(at, Vec2::new(width, head));
        let grip = ui.interact(head_rect, ui.scope_id().with("island_grip"), Sense::click_and_drag());
        out.drag = grip.drag_started();
        let center_y = head_rect.center().y;
        let pose = super::pose(view.mood, view.celebrating, view.working_secs, view.working_seed);
        let icon = Pos2::new(at.x + 10.0 + 17.0, center_y);
        crate::app::mascot::poses::paint_pose(ui.painter(), pose, icon, 44.0, time, matches!(view.mood, Mood::Working(_)), 1.0);
        ui.painter().galley(Pos2::new(at.x + 10.0 + 34.0 + 8.0, center_y - galley.size().y / 2.0), galley, look.text);
        if view.open {
            self.island_buttons(ui, rect, center_y, view, form.is_some(), look, time, &mut out);
        } else if view.voice_on {
            let bars = crate::app::voice::call_bars(view.voice_state, view.mic, time);
            crate::app::voice::paint_bars(ui.painter(), Pos2::new(rect.right() - 14.0 - 10.0, center_y), bars, look.text);
        }
        let mut shown = rect;
        if view.open {
            let top = at.y + head + 4.0;
            let body = Rect::from_min_size(Pos2::new(at.x + 10.0, top), Vec2::new(PANEL_WIDTH - 20.0, MAX_BODY));
            let used = ui.scope_builder(egui::UiBuilder::new().max_rect(body), |ui| self.island_body(ui, view, form, look, time, &mut out)).inner;
            out.body = used;
            shown = Rect::from_min_size(at, Vec2::new(width, head + 4.0 + used + 12.0));
        }
        out.size = shown.size();
        out.hovered = ui.input(|input| input.pointer.hover_pos()).is_some_and(|pointer| shown.contains(pointer));
        out
    }

    /// The panel's two buttons: the voice and the new chat.
    #[allow(clippy::too_many_arguments)]
    fn island_buttons(&self, ui: &mut egui::Ui, rect: Rect, center_y: f32, view: &View, form_open: bool, look: &Look, time: f64, out: &mut Out) {
        let button = |ui: &mut egui::Ui, name: &str, x: f32, tip: &str, on: bool| {
            let area = Rect::from_center_size(Pos2::new(x, center_y), Vec2::splat(30.0));
            let response = ui.interact(area, ui.scope_id().with(name), Sense::click()).on_hover_cursor(CursorIcon::PointingHand).tip(tip);
            let ground = if on || response.hovered() { look.selected } else { look.raised };
            ui.painter().circle_filled(area.center(), 15.0, ground);
            (response, area.center())
        };
        let (plus, at) = button(ui, "island_plus", rect.right() - 26.0, if form_open { "Fechar" } else { "Novo chat num espaço novo" }, form_open);
        glyph(ui.painter(), at, 16.0, 2.2, look.text, if form_open { CROSS } else { PLUS });
        out.toggle_form = plus.clicked();
        let tip = if view.voice_on { "Encerrar o chat por voz" } else { "Chat por voz" };
        let (voice, at) = button(ui, "island_voice", rect.right() - 26.0 - 36.0, tip, view.voice_on);
        if view.voice_on {
            crate::app::voice::paint_bars(ui.painter(), at, crate::app::voice::call_bars(view.voice_state, view.mic, time), look.text);
        } else {
            crate::app::voice::paint_mic(ui.painter(), at, look.text, false);
        }
        out.voice = voice.clicked();
    }

    /// The open panel's body, scrolling past `MAX_BODY`: the call's note, the questions, the form, the agents. Its
    /// height as shown.
    fn island_body(&self, ui: &mut egui::Ui, view: &View, form: &mut Option<NewForm>, look: &Look, time: f64, out: &mut Out) -> f32 {
        ui.spacing_mut().scroll = egui::style::ScrollStyle::floating();
        let scroll = egui::ScrollArea::vertical().id_salt("island_scroll").max_height(MAX_BODY).auto_shrink([false, true]).show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.spacing_mut().item_spacing.y = 6.0;
            if !view.voice_note.is_empty() {
                ui.add(egui::Label::new(egui::RichText::new(&view.voice_note).size(12.0).color(look.muted)).wrap());
            }
            for chat in &view.asks {
                self.decide_card(ui, chat, look, time);
            }
            if let Some(form) = form.as_mut() {
                let open = new_chat_form(ui, form, look, out);
                if !open {
                    out.toggle_form = true;
                }
            }
            ui.add_space(2.0);
            ui.label(egui::RichText::new("AGENTES").size(11.0).color(look.muted));
            if view.lines.is_empty() {
                ui.label(egui::RichText::new("Nenhum agente ainda.").size(13.0).color(look.muted));
            }
            for line in &view.lines {
                if agent_row(ui, line, look, time) {
                    out.open_agent = Some(line.id.clone());
                }
            }
        });
        scroll.inner_rect.height()
    }
}

/// One line of the list: its state, its name and what it is doing. True when clicked.
fn agent_row(ui: &mut egui::Ui, line: &Line, look: &Look, time: f64) -> bool {
    let (rect, response) = ui.allocate_exact_size(Vec2::new(ui.available_width(), 44.0), Sense::click());
    let response = response.on_hover_cursor(CursorIcon::PointingHand);
    let painter = ui.painter();
    if response.hovered() {
        painter.rect_filled(rect, 10, look.hover);
    }
    let dot = Pos2::new(rect.left() + 18.0, rect.center().y);
    match line.state {
        AgentState::Working(_) => spinner(painter, dot, 15.0, CLAUDE, time),
        AgentState::Waiting => pulse_dot(painter, dot, 4.5, AMBER, 2.0, time),
        _ => {
            painter.circle_filled(dot, 4.0, look.faint);
        }
    }
    let room = rect.width() - 38.0 - 10.0;
    let name = clipped(painter, line.name.clone(), FontId::proportional(13.5), look.text, room);
    painter.galley(Pos2::new(rect.left() + 38.0, rect.top() + 6.0), name, look.text);
    let color = if line.state == AgentState::Waiting { AMBER } else { look.muted };
    let detail = clipped(painter, line.detail.clone(), FontId::proportional(12.0), color, room);
    painter.galley(Pos2::new(rect.left() + 38.0, rect.top() + 24.0), detail, color);
    response.clicked()
}

/// The new chat's form: the workspace's name, the folder, the worktree, Criar. False when it was cancelled.
fn new_chat_form(ui: &mut egui::Ui, form: &mut NewForm, look: &Look, out: &mut Out) -> bool {
    let mut keep = true;
    egui::Frame::new().fill(look.panel).stroke(Stroke::new(1.0, look.line)).corner_radius(12).inner_margin(12).show(ui, |ui| {
        ui.set_width(ui.available_width());
        ui.spacing_mut().item_spacing.y = 8.0;
        ui.label(egui::RichText::new("Chat novo").size(14.0).strong().color(look.text));
        ui.add(
            egui::Label::new(
                egui::RichText::new("Abre um espaço de trabalho novo: o que está em andamento no espaço de agora segue como está.")
                    .size(12.0)
                    .color(look.muted),
            )
            .wrap(),
        );
        egui::Frame::new().fill(look.surface).stroke(Stroke::new(1.0, look.line)).corner_radius(8).inner_margin(egui::Margin::symmetric(10, 6)).show(
            ui,
            |ui| {
                let hint = format!("Nome do espaço (vazio: {})", folder_label(&form.folder));
                ui.add(egui::TextEdit::singleline(&mut form.name).hint_text(hint).frame(egui::Frame::NONE).text_color(look.text).desired_width(f32::INFINITY));
            },
        );
        ui.horizontal(|ui| {
            let room = ui.available_width() - 90.0;
            let path = clipped(ui.painter(), form.folder.clone(), FontId::proportional(12.0), look.muted, room);
            let (rect, _) = ui.allocate_exact_size(Vec2::new(room, 22.0), Sense::hover());
            ui.painter().galley(Pos2::new(rect.left(), rect.center().y - path.size().y / 2.0), path, look.muted);
            if button(ui, "Pasta…", false, look).clicked() {
                out.pick_folder = true;
            }
        });
        ui.horizontal(|ui| {
            let (rect, response) = ui.allocate_exact_size(Vec2::new(32.0, 20.0), Sense::click());
            let response = response.on_hover_cursor(CursorIcon::PointingHand);
            if response.clicked() {
                form.worktree = !form.worktree;
            }
            let t = ui.ctx().animate_bool_with_time(response.id, form.worktree, 0.12);
            ui.painter().rect_filled(rect, 10, look.raised.lerp_to_gamma(CLAUDE, t));
            ui.painter().circle_filled(Pos2::new(rect.left() + 10.0 + 12.0 * t, rect.center().y), 7.5, Color32::WHITE);
            ui.vertical(|ui| {
                ui.spacing_mut().item_spacing.y = 1.0;
                ui.label(egui::RichText::new("Worktree próprio (git)").size(13.0).color(look.text));
                ui.label(egui::RichText::new("O chat trabalha numa cópia do repositório, sem mexer nas alterações atuais.").size(11.5).color(look.muted));
            });
        });
        ui.horizontal(|ui| {
            out.create = button(ui, "Criar", true, look).clicked();
            keep = !button(ui, "Cancelar", false, look).clicked();
        });
    });
    keep
}

/// A round button of the panel; `strong` is the one that does it.
fn button(ui: &mut egui::Ui, text: &str, strong: bool, look: &Look) -> egui::Response {
    let galley = ui.painter().layout_no_wrap(text.to_string(), FontId::proportional(13.0), look.text);
    let (rect, response) = ui.allocate_exact_size(Vec2::new(galley.size().x + 24.0, 28.0), Sense::click());
    let response = response.on_hover_cursor(CursorIcon::PointingHand);
    let ground = match (strong, response.hovered()) {
        (true, hovered) => CLAUDE.gamma_multiply(if hovered { 0.4 } else { 0.28 }),
        (false, true) => look.selected,
        (false, false) => look.raised,
    };
    ui.painter().rect(rect, 14, ground, Stroke::new(1.0, if strong { CLAUDE } else { look.line }), StrokeKind::Inside);
    ui.painter().galley(Pos2::new(rect.center().x - galley.size().x / 2.0, rect.center().y - galley.size().y / 2.0), galley, look.text);
    response
}
