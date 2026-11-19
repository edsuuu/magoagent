//! The sidebar of topic "shell": what `ui/sidebar.rs`, `ui/attention.rs` and `usage/foot.rs` draw, as data.

use crate::app::chat::list::team::{team_summary, under_maestros};
use crate::app::island::native::mago_name;
use crate::app::mascot::identity::distinct_names;
use crate::app::mascot::mago::MagoState;
use crate::app::ui::agents::{AgentRow, AgentState};
use crate::prelude::*;

#[derive(Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub(super) struct Sidebar {
    hidden: bool,
    /// Agents waiting on the person, in every space.
    waiting: usize,
    agents_closed: bool,
    next_keys: String,
    /// "Agentes deste espaço" in the list's order: a Maestro's agents under it while its team is open.
    agents: Vec<AgentModel>,
    spaces: Vec<SpaceRow>,
    groups: Vec<GroupRow>,
    /// The plans' use at the foot, a line per agent whose CLI answered.
    usage: Vec<UsageLine>,
    usage_open: bool,
    /// "Maestro: 3 de 6 agentes; …", under the usage card's agents.
    maestro: String,
}

#[derive(Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
struct AgentModel {
    id: String,
    /// The name no other row shows ("Claude Code 2").
    name: String,
    /// working, waiting, idle or ended.
    state: &'static str,
    label: &'static str,
    /// What the row says after the state: the agent's mission, else its last words.
    note: Option<String>,
    mission: Option<String>,
    chat: bool,
    needs_you: bool,
    program: String,
    /// Its mascot: the colour and the pose (`LunaState`'s names).
    color: [u8; 3],
    mago: &'static str,
    /// Under a Maestro, at the agents' indent.
    member: bool,
    /// A Maestro's fold under its row: "2 agentes · 1 esperando".
    team: Option<TeamFold>,
}

#[derive(Serialize, PartialEq)]
struct TeamFold {
    summary: String,
    open: bool,
}

#[derive(Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
struct SpaceRow {
    id: String,
    name: String,
    color: [u8; 3],
    root: String,
    pinned: bool,
    group: String,
    running: usize,
    needs: usize,
    current: bool,
    floors: usize,
}

#[derive(Serialize, PartialEq)]
struct GroupRow {
    id: String,
    name: String,
    open: bool,
}

/// One agent's line at the foot (its 5-hour session) and, for the card, every window it read.
#[derive(Serialize, PartialEq)]
struct UsageLine {
    agent: &'static str,
    /// "Claude Code · sessão 5 h".
    title: String,
    used: f64,
    plan: Option<String>,
    meters: Vec<UsageMeter>,
    /// When they renew, or since when it failed to read.
    note: Option<String>,
}

#[derive(Serialize, PartialEq)]
struct UsageMeter {
    name: String,
    used: f64,
}

/// The state word of a row (`attention.rs::state_word`).
fn state_word(state: AgentState) -> (&'static str, &'static str) {
    match state {
        AgentState::Working(_) | AgentState::Active(_) => ("working", "Trabalhando"),
        AgentState::Waiting => ("waiting", "Esperando você"),
        AgentState::Idle(_) => ("idle", "Pronto"),
        AgentState::Ended => ("ended", "Encerrado"),
    }
}

impl DrawingApp {
    fn agent_model(&self, row: &AgentRow, name: String, needing: &[String], member: bool, team: Option<TeamFold>) -> AgentModel {
        let (state, label) = state_word(row.state);
        let [r, g, b, _] = self.agent_color(&row.id).to_array();

        AgentModel {
            id: row.id.clone(),
            name,
            state,
            label,
            note: row.mission.clone().or_else(|| self.state_note(row)),
            mission: row.mission.clone(),
            chat: self.is_chat(&row.id),
            needs_you: needing.contains(&row.id),
            program: row.spec.command.split_whitespace().next().unwrap_or_default().to_string(),
            color: [r, g, b],
            mago: mago_name(MagoState::from_agent(row.state, self.now)),
            member,
            team,
        }
    }

    /// The agents as `attention_ui` lists them: each Maestro's team under it, folded until opened.
    fn sidebar_agents(&self, needing: &[String]) -> Vec<AgentModel> {
        let agents = self.space_agents();
        let names = distinct_names(&agents.iter().map(|row| row.spec.label().to_string()).collect::<Vec<_>>());
        let keyed: Vec<(&str, Option<&AgentRow>)> = agents.iter().map(|row| (row.id.as_str(), Some(row))).collect();
        let members: Vec<usize> = (0..agents.len()).collect();
        let mut rows = vec![];

        for (top, team) in under_maestros(&members, &self.team_parents(&keyed)) {
            let open = self.team_open(&agents[top].id);
            let states: Vec<AgentState> = team.iter().map(|&index| agents[index].state).collect();
            let fold = (!team.is_empty()).then(|| TeamFold { summary: team_summary(&states), open });

            rows.push(self.agent_model(&agents[top], names[top].clone(), needing, false, fold));

            for index in team.into_iter().filter(|_| open) {
                rows.push(self.agent_model(&agents[index], names[index].clone(), needing, true, None));
            }
        }

        rows
    }

    fn usage_lines(&self) -> Vec<UsageLine> {
        use crate::app::usage::foot::{meter_name, renew_text};
        let wall = now_ms() / 1000;

        self.usage_readings()
            .into_iter()
            .filter_map(|(agent, reading)| {
                let session = reading.meters.iter().find(|meter| meter.title == "Sessão").or(reading.meters.first())?;
                let slot = self.usage_ui.probe.slot(agent);
                let note = if slot.failed { Some(crate::app::usage::probe::updated_text(slot.at, wall)) } else { renew_text(&reading, wall) };

                Some(UsageLine {
                    agent: agent.name(),
                    title: format!("{} · {}", agent.name(), meter_name(session).to_lowercase()),
                    used: session.used,
                    plan: reading.plan.clone(),
                    meters: reading.meters.iter().map(|meter| UsageMeter { name: meter_name(meter).to_string(), used: meter.used }).collect(),
                    note,
                })
            })
            .collect()
    }

    pub(super) fn sidebar_model(&self) -> Sidebar {
        let counts = self.space_counts();
        let needing = self.needing_you();

        let spaces = self
            .spaces
            .list
            .iter()
            .map(|space| {
                let (running, needs) = counts.get(&space.id).copied().unwrap_or_default();

                SpaceRow {
                    id: space.id.clone(),
                    name: space.name.clone(),
                    color: space.color,
                    root: space.root.clone(),
                    pinned: space.pinned,
                    group: space.group.clone(),
                    running,
                    needs,
                    current: space.id == self.spaces.current,
                    floors: space.floors.len(),
                }
            })
            .collect();

        let usage = self.usage_lines();

        Sidebar {
            hidden: self.prefs.sidebar_hidden,
            waiting: needing.len(),
            agents_closed: self.prefs.sidebar_agents_closed,
            next_keys: self.app_keys_text(crate::app::hotkeys::local::AppAction::NextNeedingYou),
            agents: self.sidebar_agents(&needing),
            spaces,
            groups: self.spaces.groups.iter().map(|g| GroupRow { id: g.id.clone(), name: g.name.clone(), open: g.open }).collect(),
            usage_open: self.usage_ui.open && !usage.is_empty(),
            usage,
            maestro: self.maestro_line(),
        }
    }
}
