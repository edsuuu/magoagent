//! A Maestro's team in the lists: the agents it recruited go right under it, folded behind one line with a chevron and
//! how many they are ("› 3 agentes · 1 esperando") that opens and shuts them. Folded until opened, and remembered
//! (`Prefs::teams_open`). Conversas (list/view.rs) and Agentes deste espaço (ui/attention.rs) share it; the MCP's
//! `chats_view` and `sidebar` open and shut it too.

use super::*;
use crate::app::chat::accordion::chevron;

/// The height of a team's fold in Conversas, and how far right its agents go.
const FOLD: f32 = 26.0;
pub(super) const SUBCHAT_INDENT: f32 = 18.0;

/// "1 agente", "3 agentes · 1 esperando · 2 trabalhando": how many a team has, and who of them waits or works.
pub(crate) fn team_summary(states: &[AgentState]) -> String {
    let count = |keep: fn(&AgentState) -> bool| states.iter().filter(|state| keep(state)).count();
    let mut words = vec![if states.len() == 1 { "1 agente".to_string() } else { format!("{} agentes", states.len()) }];
    let waiting = count(|state| *state == AgentState::Waiting);
    let working = count(|state| matches!(state, AgentState::Working(_) | AgentState::Active(_)));
    if waiting > 0 {
        words.push(format!("{waiting} esperando"));
    }
    if working > 0 {
        words.push(format!("{working} trabalhando"));
    }
    words.join(" · ")
}

/// `members` (indexes, in their order) as the rows on top, each with the agents under it: an agent whose Maestro
/// (`parents`) is among them goes under it, one whose Maestro is not shown stays on top.
pub(crate) fn under_maestros(members: &[usize], parents: &[Option<usize>]) -> Vec<(usize, Vec<usize>)> {
    let on_top = |index: &usize| parents[*index].is_none_or(|parent| !members.contains(&parent));
    members.iter().copied().filter(on_top).map(|top| (top, members.iter().copied().filter(|&index| parents[index] == Some(top)).collect())).collect()
}

impl DrawingApp {
    /// A Maestro of the lists: a chat of yours (not one of its recruits) or a terminal set as Maestro.
    pub(crate) fn is_maestro_row(&self, row: &AgentRow) -> bool {
        row.spec.maestro || self.chats.iter().any(|chat| chat.id == row.id && !chat.recruit)
    }

    /// For each of `rows` (an id, and its agent's row; none for a session outside the app), the index of the Maestro
    /// among them whose agent it is (`subchat_parents`).
    pub(crate) fn team_parents(&self, rows: &[(&str, Option<&AgentRow>)]) -> Vec<Option<usize>> {
        let ids: Vec<&str> = rows.iter().map(|(id, _)| *id).collect();
        let maestro: Vec<bool> = rows.iter().map(|(_, row)| row.is_some_and(|row| self.is_maestro_row(row))).collect();
        subchat_parents(&ids, &maestro, &self.links())
    }

    /// Whether Maestro `id`'s agents show under it.
    pub(crate) fn team_open(&self, id: &str) -> bool {
        self.prefs.teams_open.iter().any(|open| open == id)
    }

    /// Opens or folds Maestro `id`'s agents (kept with the preferences).
    pub(crate) fn set_team_open(&mut self, id: &str, open: bool) {
        self.prefs.teams_open.retain(|other| other != id);
        if open {
            self.prefs.teams_open.push(id.to_string());
        }
    }

    /// The agents of every Maestro that has some, (Maestro, its agents), over all the app's agents.
    pub(crate) fn teams(&self) -> Vec<(AgentRow, Vec<AgentRow>)> {
        let rows = self.agent_rows();
        let keyed: Vec<(&str, Option<&AgentRow>)> = rows.iter().map(|row| (row.id.as_str(), Some(row))).collect();
        let parents = self.team_parents(&keyed);
        let members: Vec<usize> = (0..rows.len()).collect();
        let teams = under_maestros(&members, &parents).into_iter().filter(|(_, team)| !team.is_empty());
        teams.map(|(top, team)| (rows[top].clone(), team.into_iter().map(|index| rows[index].clone()).collect())).collect()
    }

    /// The MCP's open and fold of a team, by its Maestro's name (as the lists show it, its id, or "Luna"); `all` for
    /// every one. What it says.
    pub(crate) fn act_team(&mut self, name: &str, open: bool) -> Result<String, String> {
        let teams = self.teams();
        let done = if open { "abertos" } else { "recolhidos" };
        if matches!(name.trim().to_lowercase().as_str(), "all" | "todos") {
            for (maestro, _) in &teams {
                self.set_team_open(&maestro.id, open);
            }
            return Ok(format!("Agentes de {} Maestro(s) {done}.", teams.len()));
        }
        let wanted = name.trim().to_lowercase();
        let names = |row: &AgentRow| {
            [row.id.to_lowercase(), row.spec.label().to_lowercase(), self.chat_title(row).to_lowercase(), self.maestro_name(&row.spec.name).to_lowercase()]
        };
        let found = teams
            .iter()
            .find(|(row, _)| names(row).contains(&wanted))
            .or_else(|| teams.iter().find(|(row, _)| names(row).iter().any(|name| name.contains(&wanted))));
        let Some((maestro, team)) = found else {
            let listed: Vec<String> = teams.iter().map(|(row, _)| self.chat_title(row)).collect();
            let listed =
                if listed.is_empty() { "nenhum Maestro tem agentes recrutados agora".to_string() } else { format!("os que têm: {}", listed.join(", ")) };
            return Err(format!("Nenhum Maestro com agentes se chama {name} ({listed})."));
        };
        self.set_team_open(&maestro.id, open);
        let states: Vec<AgentState> = team.iter().map(|row| row.state).collect();
        Ok(format!("Agentes de {} {done} ({}).", self.chat_title(maestro), team_summary(&states)))
    }

    /// A team's fold under its Maestro's row in Conversas: the chevron turning with `openness` and how many they are, at
    /// the agents' indent. True when clicked.
    pub(super) fn team_fold(&self, ui: &mut egui::Ui, maestro: &str, summary: &str, openness: f32, look: &Look) -> bool {
        let (rect, _) = ui.allocate_exact_size(Vec2::new(ui.available_width(), FOLD), Sense::hover());
        let response = ui.interact(rect, ui.scope_id().with(("chat_team", maestro)), Sense::click());
        let p = ui.painter();
        if response.hovered() {
            p.rect_filled(rect, 8, look.hover);
        }
        // the chevron under the agents' wizards, the words where their names start
        let left = rect.left() + SUBCHAT_INDENT;
        chevron(p, Pos2::new(left + 19.0, rect.center().y), 14.0, 2.0, look.muted, openness);
        let text = clipped(p, summary.into(), FontId::proportional(12.0), look.soft, (rect.right() - 10.0 - (left + 40.0)).max(0.0));
        p.galley(Pos2::new(left + 40.0, rect.center().y - text.size().y / 2.0), text, look.soft);
        let tip = if openness > 0.5 { "Recolher os agentes" } else { "Mostrar os agentes" };
        response.on_hover_cursor(CursorIcon::PointingHand).tip(tip).clicked()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_teams_line_counts_its_agents_and_says_who_waits_or_works() {
        assert_eq!(team_summary(&[AgentState::Idle(None)]), "1 agente");
        let states = [AgentState::Working(0.0), AgentState::Waiting, AgentState::Idle(None), AgentState::Active(1.0)];
        assert_eq!(team_summary(&states), "4 agentes · 1 esperando · 2 trabalhando");
    }

    #[test]
    fn the_agents_go_under_their_maestro_and_one_whose_maestro_is_not_shown_stays_on_top() {
        // 0 is a Maestro with 1 and 3; 4's Maestro (2) is filtered out of the members
        let parents = [None, Some(0), None, Some(0), Some(2), None];
        assert_eq!(under_maestros(&[0, 1, 3, 4, 5], &parents), [(0, vec![1, 3]), (4, vec![]), (5, vec![])]);
    }
}
