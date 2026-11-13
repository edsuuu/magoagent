//! The mini chat's Maestro and its agents: the Maestro's chat wears a crown; when it recruited agents, a
//! button lists them (who, what each does) to see one there; seeing an agent, a button goes back to its Maestro.

use crate::app::chat::list::{Look, glyph};
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

impl DrawingApp {
    /// In the panel's header, left of `right` (its expand button): the agents' button or the way back to the Maestro.
    /// The chat to show instead, when one was picked; the room the buttons took.
    pub(crate) fn mini_agents(&self, ui: &mut egui::Ui, row: &AgentRow, right: f32, cy: f32, look: &Look, time: f64) -> (Option<String>, f32) {
        if !self.is_chat(&row.id) {
            return (None, 0.0);
        }
        let (maestro, agents) = kin(&self.chat_rows(), &self.links(), &row.id);
        let mut pick = None;
        if let Some(maestro) = maestro {
            // seeing an agent: back to the Maestro that recruited it
            let back = Rect::from_min_max(Pos2::new(right - 92.0, cy - 13.0), Pos2::new(right - 4.0, cy + 13.0));
            let hit = ui
                .interact(back, ui.scope_id().with("mini_back"), Sense::click())
                .on_hover_cursor(CursorIcon::PointingHand)
                .tip(format!("Voltar ao Maestro ({})", maestro.spec.label()));
            ui.painter().rect_filled(back, 13, if hit.hovered() { look.selected } else { look.raised });
            glyph(ui.painter(), Pos2::new(back.left() + 13.0, cy), 12.0, 2.2, look.text, &[&[(14.5, 6.0), (8.5, 12.0), (14.5, 18.0)]]);
            crate::app::ui::maestro_crown(ui.painter(), Pos2::new(back.left() + 27.0, cy + 0.5), 0.75, crate::app::chat::list::AMBER);
            ui.painter().text(Pos2::new(back.left() + 36.0, cy), egui::Align2::LEFT_CENTER, "Maestro", FontId::proportional(11.5), look.text);
            if hit.clicked() {
                pick = Some(maestro.id);
            }
            return (pick, 96.0);
        }
        if agents.is_empty() {
            return (None, 0.0);
        }
        // the Maestro's agents: how many, the dot pulsing while one works; the menu says who does what
        let button = Rect::from_min_max(Pos2::new(right - 56.0, cy - 13.0), Pos2::new(right - 4.0, cy + 13.0));
        let hit = ui.interact(button, ui.scope_id().with("mini_agents"), Sense::click()).on_hover_cursor(CursorIcon::PointingHand);
        let open = egui::Popup::is_id_open(ui.ctx(), egui::Popup::default_response_id(&hit));
        let hit = if open { hit } else { hit.tip("Ver os agentes deste Maestro") };
        ui.painter().rect_filled(button, 13, if hit.hovered() || open { look.selected } else { look.raised });
        paint_icon(ui.painter(), Icon::Agents, Pos2::new(button.left() + 16.0, cy), look.text);
        ui.painter().text(Pos2::new(button.left() + 29.0, cy), egui::Align2::LEFT_CENTER, agents.len().to_string(), FontId::proportional(12.0), look.text);
        if agents.iter().any(|a| matches!(a.state, AgentState::Working(_))) {
            crate::app::chat::list::pulse_dot(ui.painter(), button.right_top() + Vec2::new(-5.0, 5.0), 3.0, crate::app::chat::list::AMBER, 1.2, time);
        }
        click_menu(&hit, |ui| {
            for agent in &agents {
                let label = format!("{} · {}", agent.spec.label(), self.list_note(agent, None));
                if menu_row(ui, Some(Icon::Agents), &label, "", None) {
                    pick = Some(agent.id.clone());
                    ui.close();
                }
            }
        });
        (pick, 60.0)
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
