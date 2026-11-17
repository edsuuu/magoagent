//! The top of the sidebar's list: "Precisa de você" (the agents waiting on the person in every workspace, and Ir para o
//! próximo, the same as ⌘⇧A) and "Agentes deste espaço" (one row per agent of the space on screen, foldable).

use crate::app::hotkeys::local::AppAction;
use crate::app::island::agent_color;
use crate::app::mascot::mago::{MagoState, paint_mini};
use crate::app::ui::agents::{AMBER, AgentRow, AgentState};
use crate::app::ui::sidebar::Tones;
use crate::prelude::*;

/// What the top of the list asked for.
pub(crate) enum Attention {
    Next,
    ToggleAgents,
    Agent(String),
}

/// A row of "Agentes deste espaço", and the heading's.
const AGENT_ROW: f32 = 40.0;
const HEADING: f32 = 24.0;

/// The card's header: "1 agente precisa de você", "3 agentes precisam de você", "Nada esperando por você".
pub(crate) fn waiting_text(count: usize) -> String {
    match count {
        0 => "Nada esperando por você".into(),
        1 => "1 agente precisa de você".into(),
        n => format!("{n} agentes precisam de você"),
    }
}

/// What the row says an agent is doing (the mockup's three words, and the two others).
fn state_word(state: AgentState) -> &'static str {
    match state {
        AgentState::Working(_) | AgentState::Active(_) => "Trabalhando",
        AgentState::Waiting => "Esperando você",
        AgentState::Idle(_) => "Pronto",
        AgentState::Ended => "Encerrado",
    }
}

/// The chevron of a foldable heading at `center`: down while open, right while folded.
pub(crate) fn chevron(painter: &Painter, center: Pos2, open: bool, color: Color32) {
    let (x, y) = (center.x, center.y);
    let points = if open {
        [Pos2::new(x - 3.5, y - 1.5), Pos2::new(x, y + 2.0), Pos2::new(x + 3.5, y - 1.5)]
    } else {
        [Pos2::new(x - 1.5, y - 3.5), Pos2::new(x + 2.0, y), Pos2::new(x - 1.5, y + 3.5)]
    };
    painter.add(egui::Shape::line(points.to_vec(), Stroke::new(1.3, color)));
}

impl DrawingApp {
    /// The agents of the workspace on screen (all its floors), and the chats whose card is on its canvas.
    pub(crate) fn space_agents(&self) -> Vec<AgentRow> {
        let current = self.spaces.current.clone();
        self.agent_rows()
            .into_iter()
            .filter(|row| if self.is_chat(&row.id) { self.chat_node(&row.id).is_some() } else { row.space.split('#').next() == Some(current.as_str()) })
            .collect()
    }

    /// The card and the section, at the top of the scrolling list.
    pub(crate) fn attention_ui(&self, ui: &mut egui::Ui, t: &Tones) -> Option<Attention> {
        let width = ui.available_width();
        let dark = self.dark();
        let mut picked = None;
        // Precisa de você: amber with somebody waiting, a green notice without
        let waiting = self.needing_you().len();
        let (wash, ink) = match (waiting > 0, dark) {
            (true, true) => (Color32::from_rgba_unmultiplied(0xf0, 0xa0, 0x20, 38), AMBER),
            (true, false) => (Color32::from_rgba_unmultiplied(0xf0, 0xa0, 0x20, 41), Color32::from_rgb(0xb8, 0x73, 0x0a)),
            (false, true) => (Color32::from_rgba_unmultiplied(0x30, 0xd1, 0x58, 36), Color32::from_rgb(0x30, 0xd1, 0x58)),
            (false, false) => (Color32::from_rgba_unmultiplied(0x24, 0x8a, 0x3d, 31), Color32::from_rgb(0x24, 0x8a, 0x3d)),
        };
        let (card, _) = ui.allocate_exact_size(Vec2::new(width, if waiting > 0 { 74.0 } else { 38.0 }), Sense::hover());
        let painter = ui.painter().clone();
        painter.rect_filled(card, 10, wash);
        let header = Pos2::new(card.left() + 10.0, card.top() + 19.0);
        if waiting > 0 {
            painter.circle_filled(header + Vec2::new(5.0, 0.0), 4.0, ink);
        } else {
            paint_icon_scaled(&painter, Icon::Check, header + Vec2::new(6.0, 0.0), ink, 0.8, 0.0);
        }
        painter.text(header + Vec2::new(18.0, 0.0), egui::Align2::LEFT_CENTER, waiting_text(waiting), FontId::proportional(13.0), ink);
        if waiting > 0 {
            let spot = Rect::from_min_max(Pos2::new(card.left() + 10.0, card.top() + 36.0), Pos2::new(card.right() - 10.0, card.top() + 64.0));
            let button = ui.interact(spot, egui::Id::unique("sidebar_next"), Sense::click()).on_hover_cursor(CursorIcon::PointingHand);
            painter.rect_filled(spot, 7, if button.hovered() { AMBER.gamma_multiply(0.85) } else { AMBER });
            let dark_ink = Color32::from_rgb(0x1d, 0x1d, 0x1f);
            painter.text(spot.left_center() + Vec2::new(10.0, 0.0), egui::Align2::LEFT_CENTER, "Ir para o próximo", FontId::proportional(13.0), dark_ink);
            // the keys as Ajustes › Atalhos has them, the mockup's way: "Ctrl ⇧ A"
            let keys = self.app_keys_text(AppAction::NextNeedingYou).replace("Shift", "⇧").replace('+', " ");
            if !keys.is_empty() {
                let label = painter.layout_no_wrap(keys, FontId::monospace(10.0), dark_ink.gamma_multiply(0.75));
                let frame = Rect::from_min_size(spot.right_center() - Vec2::new(10.0 + label.size().x + 8.0, 8.0), Vec2::new(label.size().x + 8.0, 16.0));
                painter.rect_stroke(frame, 4, Stroke::new(1.0, dark_ink.gamma_multiply(0.5)), StrokeKind::Inside);
                painter.galley(frame.center() - label.size() / 2.0, label, dark_ink);
            }
            if button.tip(self.tip_with_keys("Próximo agente que precisa de você", AppAction::NextNeedingYou)).clicked() {
                picked = Some(Attention::Next);
            }
        }
        ui.add_space(8.0);
        // Agentes deste espaço: the heading folds it to its count
        let agents = self.space_agents();
        let open = !self.prefs.sidebar_agents_closed;
        let (heading, response) = ui.allocate_exact_size(Vec2::new(width, HEADING), Sense::click());
        if ui.rect_contains_pointer(heading) {
            painter.rect_filled(heading, 6, t.hover);
        }
        chevron(&painter, Pos2::new(heading.left() + 14.5, heading.center().y), open, t.muted);
        painter.text(
            Pos2::new(heading.left() + 27.0, heading.center().y),
            egui::Align2::LEFT_CENTER,
            "AGENTES DESTE ESPAÇO",
            FontId::proportional(12.0),
            t.muted,
        );
        let count = painter.layout_no_wrap(agents.len().to_string(), FontId::proportional(11.0), t.text);
        let badge = Rect::from_min_size(heading.right_center() - Vec2::new(8.0 + count.size().x + 12.0, 8.0), Vec2::new(count.size().x + 12.0, 16.0));
        painter.rect_filled(badge, 8, t.active);
        painter.galley(badge.center() - count.size() / 2.0, count, t.text);
        let tip = if open { "Recolher agentes deste espaço" } else { "Mostrar agentes deste espaço" };
        if response.on_hover_cursor(CursorIcon::PointingHand).tip(tip).clicked() {
            picked = Some(Attention::ToggleAgents);
        }
        if open {
            if agents.is_empty() {
                let (rect, _) = ui.allocate_exact_size(Vec2::new(width, 26.0), Sense::hover());
                painter.text(
                    rect.left_center() + Vec2::new(12.0, 0.0),
                    egui::Align2::LEFT_CENTER,
                    "Nenhum agente aqui ainda",
                    FontId::proportional(12.0),
                    t.muted,
                );
            }
            for row in &agents {
                if self.agent_row_ui(ui, t, row) {
                    picked = Some(Attention::Agent(row.id.clone()));
                }
            }
        }
        ui.add_space(8.0);
        picked
    }

    /// One agent: its mini wizard in its color, its name, and its state with the last thing it said. True when clicked.
    fn agent_row_ui(&self, ui: &mut egui::Ui, t: &Tones, row: &AgentRow) -> bool {
        let (rect, response) = ui.allocate_exact_size(Vec2::new(ui.available_width(), AGENT_ROW), Sense::click());
        let painter = ui.painter().clone();
        let on = self.selected.iter().any(|&index| self.scene.elements.get(index).is_some_and(|element| element.id == row.id));
        if on || ui.rect_contains_pointer(rect) {
            painter.rect_filled(rect, 7, if on { t.active } else { t.hover });
        }
        let program = row.spec.command.split_whitespace().next().unwrap_or_default().to_lowercase();
        let mini = Pos2::new(rect.left() + 19.0, rect.center().y);
        paint_mini(&painter, &format!("sidebar-{}", row.id), mini, 20.0, agent_color(&program, &row.id), MagoState::from_agent(row.state, self.now), self.now);
        let text = Rect::from_min_max(Pos2::new(rect.left() + 36.0, rect.top()), Pos2::new(rect.right() - 6.0, rect.bottom()));
        let clipped = painter.with_clip_rect(text);
        clipped.text(Pos2::new(text.left(), rect.top() + 12.0), egui::Align2::LEFT_CENTER, row.spec.label(), FontId::proportional(14.0), t.text);
        let last = row.mission.clone().or_else(|| self.state_note(row));
        let sub = match last {
            Some(last) => format!("{} · {last}", state_word(row.state)),
            None => state_word(row.state).to_string(),
        };
        clipped.text(Pos2::new(text.left(), rect.top() + 28.0), egui::Align2::LEFT_CENTER, sub, FontId::proportional(12.0), t.muted);
        response.on_hover_cursor(CursorIcon::PointingHand).clicked()
    }

    /// Does what the top of the list asked for.
    pub(crate) fn attention_act(&mut self, picked: Attention) {
        match picked {
            Attention::Next => {
                self.next_needing_attention();
            }
            Attention::ToggleAgents => self.prefs.sidebar_agents_closed = !self.prefs.sidebar_agents_closed,
            Attention::Agent(id) => self.open_agent(&id),
        }
    }
}
