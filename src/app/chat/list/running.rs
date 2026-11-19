//! Everything running, for the chat's list: the app's chats, the terminals running Claude Code or Codex (in every space)
//! and the sessions outside the app (maestro/outside.rs, what the island shows), grouped by space; the filter chips over
//! them and the answers given right on a row, by the same ways the island and the cards answer.

use super::*;
use crate::app::chat::decide::PersonAsk;
use crate::app::maestro::outside::PILL_PREFIX;
use crate::app::mascot::mago::MagoState;

/// The group of the sessions outside the app.
pub(crate) const OUTSIDE: &str = "fora";

/// The filter chips over the list.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub(crate) enum Show {
    #[default]
    All,
    Working,
    Waiting,
    Ready,
}

impl Show {
    pub(crate) const ALL: [Show; 4] = [Show::All, Show::Working, Show::Waiting, Show::Ready];

    pub(crate) fn label(self) -> &'static str {
        match self {
            Show::All => "Todos",
            Show::Working => "Trabalhando",
            Show::Waiting => "Esperando",
            Show::Ready => "Prontos",
        }
    }

    /// The MCP's name for it (`chats_view`), or the chip's own.
    pub(crate) fn named(name: &str) -> Option<Show> {
        let name = name.trim().to_lowercase();

        Show::ALL.into_iter().find(|show| show.label().to_lowercase() == name || show.key() == name)
    }

    pub(crate) fn key(self) -> &'static str {
        match self {
            Show::All => "all",
            Show::Working => "working",
            Show::Waiting => "waiting",
            Show::Ready => "ready",
        }
    }

    pub(crate) fn keeps(self, state: &AgentState) -> bool {
        match self {
            Show::All => true,
            Show::Working => chip(state).0 == "Trabalhando",
            Show::Waiting => chip(state).0 == "Esperando",
            Show::Ready => chip(state).0 == "Pronto",
        }
    }
}

/// A state as its chip says it, with its colour: working blue, waiting amber, ready green (an idle or ended agent).
pub(crate) fn chip(state: &AgentState) -> (&'static str, Color32) {
    match state {
        AgentState::Working(_) | AgentState::Active(_) => ("Trabalhando", ACCENT),
        AgentState::Waiting => ("Esperando", AMBER),
        AgentState::Idle(_) | AgentState::Ended => ("Pronto", GREEN),
    }
}

/// A line of the list.
#[derive(Clone, Debug)]
pub(crate) struct Entry {
    /// The agent's id, or the island's pill id of a session outside.
    pub(crate) id: String,
    pub(crate) name: String,
    pub(crate) state: AgentState,
    /// What it said or does last, in one line.
    pub(crate) last: String,
    /// Since when, in seconds.
    pub(crate) ago: Option<f64>,
    /// Its mini wizard's colour.
    pub(crate) color: Color32,
    /// The agent it runs, as its header tags it ("Claude Code", "Codex"), or the program of a session outside.
    pub(crate) program: String,
    /// The space it runs in (its id, no floor); "" none; `OUTSIDE` outside the app.
    pub(crate) space: String,
    /// What the conversation and the menus need; none for a session outside.
    pub(crate) row: Option<AgentRow>,
    /// Asleep (a session outside whose turn ended and was seen).
    pub(crate) asleep: bool,
}

/// `entries` the chip `show` and the search `query` (lowercase) keep.
pub(crate) fn kept<'a>(entries: impl IntoIterator<Item = &'a Entry>, show: Show, query: &str) -> Vec<&'a Entry> {
    let says = |entry: &Entry| query.is_empty() || format!("{} {}", entry.name, entry.last).to_lowercase().contains(query);

    entries.into_iter().filter(|entry| show.keeps(&entry.state) && says(entry)).collect()
}

/// How many of `entries` each chip has.
pub(crate) fn counts<'a>(entries: impl IntoIterator<Item = &'a Entry>) -> [usize; 4] {
    let states: Vec<AgentState> = entries.into_iter().map(|entry| entry.state).collect();

    Show::ALL.map(|show| states.iter().filter(|state| show.keeps(state)).count())
}

/// "N rodando · M esperando" of the title bar: working or waiting, and waiting.
pub(crate) fn running_counts(states: impl IntoIterator<Item = AgentState>) -> (usize, usize) {
    states.into_iter().fold((0, 0), |(running, waiting), state| match chip(&state).0 {
        "Pronto" => (running, waiting),
        "Esperando" => (running + 1, waiting + 1),
        _ => (running + 1, waiting),
    })
}

/// The groups of `entries` by space: the spaces in `order` (the sidebar's), then the ones of no space, then the
/// sessions outside; each with its entries' indexes, in their order.
pub(crate) fn space_groups(entries: &[&Entry], order: &[String]) -> Vec<(String, Vec<usize>)> {
    let rank = |space: &str| match space {
        OUTSIDE => order.len() + 1,
        "" => order.len(),
        space => order.iter().position(|id| id == space).unwrap_or(order.len()),
    };

    let mut groups: Vec<(String, Vec<usize>)> = Vec::new();

    for (index, entry) in entries.iter().enumerate() {
        let key = if rank(&entry.space) == order.len() { "" } else { entry.space.as_str() };

        match groups.iter_mut().find(|(space, _)| space == key) {
            Some((_, members)) => members.push(index),
            None => groups.push((key.to_string(), vec![index])),
        }
    }

    groups.sort_by_key(|(space, _)| rank(space));
    groups
}

/// An answer given on a row.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) enum Answer {
    /// Permitir or Negar a permission.
    Allow(bool),
    /// An option of the question shown.
    Pick(usize),
    /// A choice of the menu on a terminal's screen: (the marked one, the one picked).
    Menu(usize, usize),
}

/// The ask of row `id`: a chat's (its id) or a session outside's (its pill).
fn asks_for(ask: &PersonAsk, id: &str) -> bool {
    ask.chat == id || ask.source.as_ref().is_some_and(|source| id.strip_prefix(PILL_PREFIX) == Some(source.pill.as_str()))
}

impl DrawingApp {
    /// A terminal running an agent: Claude Code or Codex by its command, its session, or the title Claude gives it.
    fn runs_agent(&self, row: &AgentRow) -> bool {
        self.row_program(row).is_some()
    }

    /// The agent a row runs, as its header tags it: Claude Code or Codex by its command, its session, or the title Claude
    /// gives its terminal.
    fn row_program(&self, row: &AgentRow) -> Option<&'static str> {
        let title = self.terminals.get(&row.id).and_then(|s| s.as_ref().ok()).and_then(|s| s.title()).unwrap_or_default();
        let claude = self.agent_sessions.contains_key(&row.id) || crate::app::ui::agents::claude_title(&title);

        crate::app::chat::header::program(&row.spec, claude)
    }

    /// The list's lines: the chats (not a draft), the terminals running an agent (and the one whose conversation is
    /// open) while they run, in every space, and the sessions outside the app.
    pub(crate) fn running_entries(&self) -> Vec<Entry> {
        let open = self.chat_agent.as_deref();

        let mut entries: Vec<Entry> = self
            .agent_rows()
            .into_iter()
            .filter(|row| self.is_chat(&row.id) || open == Some(row.id.as_str()) || (row.state != AgentState::Ended && self.runs_agent(row)))
            .map(|row| Entry {
                id: row.id.clone(),
                name: self.chat_title(&row),
                state: row.state,
                last: self.last_line(&row),
                ago: self.row_ago(&row),
                color: self.agent_color(&row.id),
                program: self.row_program(&row).unwrap_or_default().to_string(),
                space: self.row_space(&row),
                row: Some(row),
                asleep: false,
            })
            .collect();

        let now = now_ms();

        entries.extend(self.outside_pills().into_iter().map(|pill| {
            let last = self.waiting_line(&pill.id).unwrap_or(if pill.activity.is_empty() { "Sem atividade ainda".to_string() } else { pill.activity });

            Entry {
                // the name is the folder's; a program other than Claude Code, the group's usual one, says itself before
                // what it does
                last: if pill.kind == "Claude Code" || pill.kind == pill.name { last } else { format!("{} · {last}", pill.kind) },
                ago: (pill.at > 0).then(|| (now - pill.at).max(0) as f64 / 1000.0),
                id: pill.id,
                name: pill.name,
                state: pill.state,
                color: pill.color,
                program: pill.kind,
                space: OUTSIDE.into(),
                row: None,
                asleep: pill.asleep,
            }
        }));

        // no two by the same name: the first keeps it, the next take a number
        crate::app::mascot::identity::make_distinct(&mut entries, |entry| &mut entry.name);
        entries
    }

    /// The space a row runs in (its id, no floor; "" none): a terminal's, or a chat's, which belongs to none, the one
    /// whose canvas has its card.
    pub(crate) fn row_space(&self, row: &AgentRow) -> String {
        let key = match self.is_chat(&row.id) {
            true if self.chat_node(&row.id).is_some() => self.scope_key(),
            true => String::new(),
            false => row.space.clone(),
        };

        key.split('#').next().unwrap_or_default().to_string()
    }

    /// "Chats · N rodando · M esperando" of the title bar, and its button's badge.
    pub(crate) fn chats_counts(&self) -> (usize, usize) {
        let open = self.chat_agent.as_deref();
        let rows = self.agent_rows().into_iter().filter(|row| self.is_chat(&row.id) || open == Some(row.id.as_str()) || self.runs_agent(row));

        running_counts(rows.map(|row| row.state).chain(self.outside_pills().into_iter().map(|pill| pill.state)))
    }

    /// The space's name and colour of a group: grey for none and for the sessions outside the app.
    pub(crate) fn group_look(&self, space: &str) -> (String, Color32) {
        let grey = Color32::from_gray(0x8e);

        match self.spaces.list.iter().find(|workspace| workspace.id == space) {
            Some(workspace) => (workspace.name.clone(), Color32::from_rgb(workspace.color[0], workspace.color[1], workspace.color[2])),
            None if space == OUTSIDE => ("Fora do app".into(), grey),
            None => ("Sem espaço".into(), grey),
        }
    }

    /// What a row says under its name: what waits on the person, else the last words of its conversation, else what its
    /// terminal says it does, else its state.
    fn last_line(&self, row: &AgentRow) -> String {
        if let Some(asked) = self.waiting_line(&row.id) {
            return asked;
        }

        let items = self.transcripts.get(&row.id).map(|transcript| transcript.items.as_slice()).unwrap_or_default();

        let said = items.iter().rev().find_map(|item| match item {
            ChatItem::User(text) | ChatItem::Agent(text) | ChatItem::Error(text) => text.lines().map(str::trim).rfind(|line| !line.is_empty()),
            _ => None,
        });

        // nothing said since /clear: the new session's line ("Nova sessão · 14:32")
        let said = said.or_else(|| {
            items.iter().rev().find_map(|item| match item {
                ChatItem::System(text) if crate::app::chat::direct::is_new_session_mark(text) => Some(text.as_str()),
                _ => None,
            })
        });

        let quiet = || if self.is_chat(&row.id) { "Nenhuma mensagem ainda".to_string() } else { self.list_note(row, None) };

        said.map(str::to_string).or_else(|| row.mission.clone()).unwrap_or_else(quiet)
    }

    /// The question or the command an ask of row `id` waits on, or the menu on its terminal's screen.
    fn waiting_line(&self, id: &str) -> Option<String> {
        if let Some(ask) = self.person_asks.borrow().iter().find(|ask| asks_for(ask, id)) {
            return Some(match ask.questions.get(ask.step) {
                Some(question) => question.text.clone(),
                None => format!("Permissão: {}", ask.summary()),
            });
        }

        let menu = self.terminal_menu(id)?;

        Some(if menu.question.is_empty() { "Escolha uma opção".into() } else { menu.question.join(" ") })
    }

    /// The menu a waiting terminal shows.
    fn terminal_menu(&self, id: &str) -> Option<crate::terminal::Menu> {
        let session = self.terminals.get(id)?.as_ref().ok().filter(|_| self.terminal_waiting.contains(id) || self.waiting_secs(id).is_some())?;

        crate::terminal::menu_choices(&session.handle.screen_text())
    }

    /// Since when a row is in its state (or, a chat at rest, since it last changed), in seconds.
    fn row_ago(&self, row: &AgentRow) -> Option<f64> {
        let since = match row.state {
            AgentState::Working(since) | AgentState::Active(since) | AgentState::Idle(Some(since)) => Some(self.now - since),
            AgentState::Waiting => self.waiting_secs(&row.id),
            _ => None,
        };

        since.or_else(|| self.chat_when(&row.id).map(|when| (now_ms() / 1000 - when).max(0) as f64))
    }

    /// The buttons that answer row `id` there: Permitir and Negar a permission, the options of a question (one to pick;
    /// a multiple choice is answered in the conversation), the choices of the menu on a terminal's screen. Each with
    /// whether it is the main one; and whether they don't take clicks yet (the ask just came, `PersonAsk::guarded`).
    pub(crate) fn row_answers(&self, id: &str, time: f64) -> (Vec<(String, bool, Answer)>, bool) {
        if let Some(ask) = self.person_asks.borrow_mut().iter_mut().find(|ask| asks_for(ask, id)) {
            let guarded = ask.guarded(time);

            let Some(question) = ask.questions.get(ask.step) else {
                return (vec![("Permitir".into(), true, Answer::Allow(true)), ("Negar".into(), false, Answer::Allow(false))], guarded);
            };

            if question.multiple {
                return (Vec::new(), false);
            }

            let options = question.options.iter().enumerate().map(|(index, (label, _))| (label.clone(), index == 0, Answer::Pick(index)));

            return (options.collect(), guarded);
        }

        let Some(menu) = self.terminal_menu(id) else { return (Vec::new(), false) };
        let choices = menu.choices.into_iter().enumerate().map(|(index, choice)| (choice, index == menu.marked, Answer::Menu(menu.marked, index)));

        (choices.collect(), false)
    }

    /// An answer given on row `id`: the ask's (as the island's card sends it) or the keys of its terminal's menu.
    pub(crate) fn answer_row(&mut self, id: &str, answer: Answer) {
        if let Answer::Menu(from, to) = answer {
            if let Some(Ok(session)) = self.terminals.get(id) {
                let handle = session.handle.clone();

                std::thread::spawn(move || handle.pick(from, to));
            }

            return;
        }

        let mut asks = self.person_asks.borrow_mut();
        let Some(index) = asks.iter().position(|ask| asks_for(ask, id)) else { return };

        let send = match answer {
            Answer::Allow(allow) => Some(allow),
            Answer::Pick(option) => asks[index].pick(option).then_some(true),
            Answer::Menu(..) => None,
        };

        if let Some(allow) = send {
            asks.remove(index).send(allow);
        }
    }

    /// A row's mini wizard: the island's, by its state.
    pub(crate) fn entry_mago(&self, entry: &Entry) -> MagoState {
        if entry.asleep { MagoState::Sleeping } else { MagoState::from_agent(entry.state, self.now) }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entry(id: &str, state: AgentState, space: &str) -> Entry {
        Entry {
            id: id.into(),
            name: id.into(),
            state,
            last: String::new(),
            ago: None,
            color: GREEN,
            program: "Claude Code".into(),
            space: space.into(),
            row: None,
            asleep: false,
        }
    }

    #[test]
    fn the_chips_count_and_keep_their_states_and_the_search_finds_the_name_or_the_last_line() {
        let mut entries = vec![
            entry("a", AgentState::Working(0.0), ""),
            entry("b", AgentState::Waiting, ""),
            entry("c", AgentState::Idle(None), ""),
            entry("d", AgentState::Active(0.0), ""),
        ];

        entries[2].last = "Testes passaram".into();
        assert_eq!(counts(&entries), [4, 2, 1, 1]);
        let ids = |show: Show, query: &str| kept(&entries, show, query).iter().map(|entry| entry.id.clone()).collect::<Vec<_>>();

        assert_eq!(ids(Show::Working, ""), ["a", "d"]);
        assert_eq!(ids(Show::Waiting, ""), ["b"]);
        assert_eq!(ids(Show::Ready, ""), ["c"]);
        assert_eq!(ids(Show::All, "testes"), ["c"]);
        assert_eq!(ids(Show::Working, "testes"), Vec::<String>::new());
        assert_eq!(running_counts(entries.iter().map(|entry| entry.state)), (3, 1));
        assert_eq!((Show::named("waiting"), Show::named("Prontos"), Show::named("x")), (Some(Show::Waiting), Some(Show::Ready), None));
    }

    #[test]
    fn the_groups_follow_the_spaces_then_none_then_outside() {
        let entries = [
            entry("x", AgentState::Waiting, OUTSIDE),
            entry("b", AgentState::Waiting, "s2"),
            entry("n", AgentState::Waiting, ""),
            entry("a", AgentState::Waiting, "s1"),
            entry("a2", AgentState::Waiting, "s1"),
            entry("z", AgentState::Waiting, "gone"),
        ];

        let refs: Vec<&Entry> = entries.iter().collect();
        let groups = space_groups(&refs, &["s1".into(), "s2".into()]);

        // a space no longer listed goes with the ones of none
        assert_eq!(groups, [("s1".to_string(), vec![3, 4]), ("s2".into(), vec![1]), ("".into(), vec![2, 5]), (OUTSIDE.into(), vec![0])]);
    }
}
