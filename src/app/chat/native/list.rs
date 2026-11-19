//! The topic "chats": the list of the Chats screen as Conversas lays it out (chat/list/view.rs): the chips and their
//! counts, the groups by space with their dot, each row with its state, how long ago and its last line, the answers it
//! takes right there, and a Maestro's agents under it behind their fold (list/team.rs).

use super::state_key;
use crate::app::chat::list::running::{Entry, Show, counts, kept, running_counts, space_groups};
use crate::app::chat::list::team::{team_summary, under_maestros};
use crate::app::ui::agents::{AgentRow, AgentState};
use crate::prelude::*;

/// The conversations list ("Chats").
#[derive(Serialize, PartialEq, Debug)]
pub(crate) struct ChatsSnapshot {
    /// The chat screen is up (not the canvas).
    pub(super) open: bool,
    pub(super) current: Option<String>,
    pub(super) show: &'static str,
    pub(super) filter: String,
    /// Todos, Trabalhando, Esperando, Prontos.
    pub(super) counts: [usize; 4],
    pub(super) running: usize,
    pub(super) waiting: usize,
    /// A chip or a search picks what shows: the teams show open, with no fold.
    pub(super) looking: bool,
    pub(super) groups: Vec<GroupSnapshot>,
}

#[derive(Serialize, PartialEq, Debug)]
pub(super) struct GroupSnapshot {
    pub(super) name: String,
    pub(super) color: [u8; 3],
    pub(super) entries: Vec<EntrySnapshot>,
}

#[derive(Serialize, PartialEq, Debug)]
pub(super) struct EntrySnapshot {
    pub(super) id: String,
    pub(super) name: String,
    /// working, waiting or ready.
    pub(super) state: &'static str,
    pub(super) ended: bool,
    pub(super) last: String,
    pub(super) ago: String,
    pub(super) color: [u8; 3],
    /// A chat of the app (renamed, removed); else a terminal or a session outside.
    pub(super) chat: bool,
    /// The buttons that answer it right on its row (`row_answers`): the label and whether it is the main one.
    pub(super) answers: Vec<(String, bool)>,
    /// A Maestro's agents, under it; how many they are and who works ("3 agentes · 1 esperando"), and whether they show.
    pub(super) team: Vec<EntrySnapshot>,
    #[serde(skip_serializing_if = "String::is_empty")]
    pub(super) team_summary: String,
    pub(super) team_open: bool,
}

impl DrawingApp {
    pub(crate) fn native_chats_snapshot(&self) -> ChatsSnapshot {
        let entries = self.running_entries();
        let (running, waiting) = running_counts(entries.iter().map(|entry| entry.state));
        let show = self.chat_listing.show;
        let shown = kept(&entries, show, &self.chat_listing.filter.to_lowercase());
        let looking = show != Show::All || !self.chat_listing.filter.trim().is_empty();
        let keyed: Vec<(&str, Option<&AgentRow>)> = shown.iter().map(|entry| (entry.id.as_str(), entry.row.as_ref())).collect();
        let parents = self.team_parents(&keyed);
        let order: Vec<String> = self.spaces.list.iter().map(|space| space.id.clone()).collect();

        let groups = space_groups(&shown, &order)
            .into_iter()
            .map(|(space, members)| {
                let (name, color) = self.group_look(&space);

                let entries = under_maestros(&members, &parents)
                    .into_iter()
                    .map(|(top, team)| {
                        let mut entry = self.entry_snapshot(shown[top]);

                        if !team.is_empty() {
                            let states: Vec<AgentState> = team.iter().map(|&index| shown[index].state).collect();

                            entry.team_summary = team_summary(&states);
                            entry.team_open = looking || self.team_open(&entry.id);
                            entry.team = team.into_iter().map(|index| self.entry_snapshot(shown[index])).collect();
                        }

                        entry
                    })
                    .collect();

                GroupSnapshot { name, color: [color.r(), color.g(), color.b()], entries }
            })
            .collect();

        ChatsSnapshot {
            open: self.chat_open,
            current: self.chat_agent.clone(),
            show: show.key(),
            filter: self.chat_listing.filter.clone(),
            counts: counts(&entries),
            running,
            waiting,
            looking,
            groups,
        }
    }

    fn entry_snapshot(&self, entry: &Entry) -> EntrySnapshot {
        let (answers, _) = self.row_answers(&entry.id, self.now);

        EntrySnapshot {
            id: entry.id.clone(),
            name: entry.name.clone(),
            state: state_key(&entry.state),
            ended: entry.state == AgentState::Ended,
            last: entry.last.clone(),
            ago: entry.ago.map(crate::app::ui::agents::ago).unwrap_or_default(),
            color: [entry.color.r(), entry.color.g(), entry.color.b()],
            chat: self.is_chat(&entry.id),
            answers: answers.into_iter().map(|(label, main, _)| (label, main)).collect(),
            team: Vec::new(),
            team_summary: String::new(),
            team_open: false,
        }
    }
}
