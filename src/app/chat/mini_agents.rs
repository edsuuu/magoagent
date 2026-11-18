//! The Maestro and its agents, in the mini chat and in the island's conversation: the Maestro's chat wears a crown;
//! when it recruited agents, a button that says so ("Agentes 2", the dot pulsing while one works) lists them (who, what
//! each does) to see one in the same place; seeing an agent, a button goes back to its Maestro. The button has its own
//! room in the header, its words say what it does (no tip over the conversation), and the press is its own: the header
//! that drags the window never takes it (asked: "Botão de ver agentes da Luna está todo bugado").

use crate::app::chat::list::{AMBER, Look, glyph, pulse_dot};
use crate::app::ui::agents::{AgentRow, AgentState};
use crate::app::ui::icons::{Icon, paint_icon};
use crate::app::ui::menu_look::{click_menu, menu_row};
use crate::prelude::*;

/// The Maestro's chat roped to agent `id`, and the agents roped to Maestro `id` (chat rows each).
pub(crate) fn kin(rows: &[AgentRow], links: &std::collections::HashMap<String, Vec<String>>, id: &str) -> (Option<AgentRow>, Vec<AgentRow>) {
    let roped = |a: &str| links.get(a).cloned().unwrap_or_default();
    let me = rows.iter().find(|r| r.id == id);
    if me.is_some_and(|r| !r.spec.maestro) {
        let maestro = rows.iter().find(|r| r.spec.maestro && roped(id).contains(&r.id)).cloned();
        return (maestro, Vec::new());
    }
    let agents = rows.iter().filter(|r| !r.spec.maestro && roped(id).contains(&r.id)).cloned().collect();
    (None, agents)
}

/// The words of the agents' button.
pub(crate) const AGENTS: &str = "Agentes";

/// The look of the button where it is drawn: its ground, lit (hovered or open), its ink, its type and height.
pub(crate) struct KinLook {
    pub(crate) ground: Color32,
    pub(crate) lit: Color32,
    pub(crate) ink: Color32,
    pub(crate) font: FontId,
    pub(crate) height: f32,
}

impl KinLook {
    /// The mini chat's, in the app's colours.
    pub(crate) fn mini(look: &Look) -> KinLook {
        KinLook { ground: look.raised, lit: look.selected, ink: look.text, font: FontId::proportional(13.0), height: 26.0 }
    }
}

/// A button on what drags its window (the island, the mini chat's header): it takes clicks and drags both, so the press
/// is the button's and never moves the window (egui gives the drag of a button that only clicks to what is under it,
/// and the system's move then shows its four-way cursor and swallows the click), with the hand over it.
pub(crate) fn pressable(ui: &mut egui::Ui, rect: Rect, id: egui::Id, name: &str) -> egui::Response {
    let response = ui.interact(rect, id, Sense::click_and_drag()).on_hover_cursor(CursorIcon::PointingHand);
    // its name for the system's accessibility (screen readers, agents), without a tip over the conversation
    response.widget_info(|| egui::WidgetInfo::labeled(egui::Role::Button, true, name));
    response
}

impl DrawingApp {
    /// The Maestro of chat `id` (seeing one of its agents) and its agents (seeing it); none for what is no chat.
    pub(crate) fn chat_kin(&self, id: &str) -> (Option<AgentRow>, Vec<AgentRow>) {
        if !self.is_chat(id) {
            return (None, Vec::new());
        }
        kin(&self.chat_rows(), &self.links(), id)
    }

    /// The agents' button (or the way back to the Maestro) of chat `row`, its right at `right` and centred on `cy`: the
    /// chat picked, to show in the same place, and the room the button took (none without one).
    pub(crate) fn kin_button(&self, ui: &mut egui::Ui, row: &AgentRow, right: f32, cy: f32, kin_look: &KinLook, time: f64) -> (Option<String>, f32) {
        let (maestro, agents) = self.chat_kin(&row.id);
        let painter = ui.painter().clone();
        let height = kin_look.height;
        let radius = (height / 2.0) as u8;
        if let Some(maestro) = maestro {
            // seeing an agent: back to the Maestro that recruited it, by its name
            let name = crate::app::chat::list::clipped(&painter, self.maestro_name(&maestro.spec.name), kin_look.font.clone(), kin_look.ink, 140.0);
            let width = 12.0 + 10.0 + 18.0 + name.size().x + 12.0;
            let back = Rect::from_min_max(Pos2::new(right - width, cy - height / 2.0), Pos2::new(right, cy + height / 2.0));
            let hit = pressable(ui, back, ui.scope_id().with("kin_back"), &format!("Voltar à conversa de {}", name.text()));
            painter.rect_filled(back, radius, if hit.hovered() { kin_look.lit } else { kin_look.ground });
            let chevron = &[&[(14.5, 6.0), (8.5, 12.0), (14.5, 18.0)][..]];
            glyph(&painter, Pos2::new(back.left() + 13.0, cy), 12.0, 2.2, kin_look.ink, chevron);
            crate::app::ui::maestro_crown(&painter, Pos2::new(back.left() + 28.0, cy + 0.5), 0.75, AMBER);
            painter.galley(Pos2::new(back.left() + 40.0, cy - name.size().y / 2.0), name, kin_look.ink);
            let pick = hit.clicked().then_some(maestro.id);
            return (pick, width + 4.0);
        }
        if agents.is_empty() {
            return (None, 0.0);
        }
        // the Maestro's agents: the words and how many, the dot pulsing while one works; the menu says who does what
        let words = painter.layout_no_wrap(AGENTS.into(), kin_look.font.clone(), kin_look.ink);
        let count = painter.layout_no_wrap(agents.len().to_string(), kin_look.font.clone(), kin_look.ink);
        let width = 10.0 + 14.0 + 6.0 + words.size().x + 6.0 + count.size().x + 11.0;
        let button = Rect::from_min_max(Pos2::new(right - width, cy - height / 2.0), Pos2::new(right, cy + height / 2.0));
        let hit = pressable(ui, button, ui.scope_id().with("kin_agents"), "Ver os agentes da Luna");
        let open = egui::Popup::is_id_open(ui.ctx(), egui::Popup::default_response_id(&hit));
        painter.rect_filled(button, radius, if hit.hovered() || open { kin_look.lit } else { kin_look.ground });
        paint_icon(&painter, Icon::Agents, Pos2::new(button.left() + 10.0 + 7.0, cy), kin_look.ink);
        let left = button.left() + 10.0 + 14.0 + 6.0;
        painter.galley(Pos2::new(left, cy - words.size().y / 2.0), words.clone(), kin_look.ink);
        painter.galley(Pos2::new(left + words.size().x + 6.0, cy - count.size().y / 2.0), count, kin_look.ink);
        if agents.iter().any(|agent| matches!(agent.state, AgentState::Working(_))) {
            pulse_dot(&painter, button.right_top() + Vec2::new(-5.0, 5.0), 3.0, AMBER, 1.2, time);
        }
        let mut pick = None;
        click_menu(&hit, |ui| {
            for agent in &agents {
                // who it is first: the name, then what it does
                let label = format!("{} · {}", agent.spec.label(), self.list_note(agent, None));
                if menu_row(ui, Some(Icon::Agents), &label, "", None) {
                    pick = Some(agent.id.clone());
                    ui.close();
                }
            }
        });
        (pick, width + 4.0)
    }

    /// In the mini chat's header, left of `right` (its expand button): the agents' button or the way back to the Maestro.
    /// The chat to show instead, when one was picked; the room the buttons took.
    pub(crate) fn mini_agents(&self, ui: &mut egui::Ui, row: &AgentRow, right: f32, cy: f32, look: &Look, time: f64) -> (Option<String>, f32) {
        self.kin_button(ui, row, right - 4.0, cy, &KinLook::mini(look), time)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_maestros_agents_are_listed_and_an_agent_knows_its_way_back() {
        let row = |id: &str, maestro: bool| AgentRow {
            id: id.into(),
            spec: TerminalSpec { name: id.into(), chat: true, maestro, ..Default::default() },
            state: AgentState::Idle(None),
            mission: None,
            space: String::new(),
        };
        let rows = [row("m", true), row("a", false), row("b", false), row("other", true)];
        let links = std::collections::HashMap::from([
            ("m".to_string(), vec!["a".to_string(), "b".to_string()]),
            ("a".to_string(), vec!["m".to_string()]),
            ("b".to_string(), vec!["m".to_string()]),
        ]);
        let (back, agents) = kin(&rows, &links, "m");
        assert!(back.is_none());
        assert_eq!(agents.iter().map(|a| a.id.as_str()).collect::<Vec<_>>(), ["a", "b"]);
        let (back, agents) = kin(&rows, &links, "a");
        assert_eq!(back.map(|m| m.id), Some("m".to_string()), "an agent goes back to its Maestro");
        assert!(agents.is_empty());
        let (back, agents) = kin(&rows, &links, "other");
        assert!(back.is_none() && agents.is_empty(), "a Maestro without agents: no button");
    }
}
