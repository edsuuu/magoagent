//! The timeline for the Maestro: what the agents did lately, read from the events ("o que aconteceu
//! na última hora?", "o que o Revisor fez hoje?").

use crate::prelude::*;

impl DrawingApp {
    /// The last `limit` events that pass the filters, oldest first: from the database, and the newest
    /// from the app's own list (the writer's thread may not have got to them).
    pub(crate) fn timeline_events(&self, agent: Option<&str>, kind: Option<EventKind>, since: Option<i64>, limit: usize) -> Vec<AgentEvent> {
        let mut events = match self.events_path().filter(|_| self.hooks.store.is_some()) {
            // ponytail: the kind is filtered in memory, over the last 1000 events; put it in the SQL if a rare kind goes missing
            Some(path) => recent_events_since(&path, 1000, agent, since),
            None => Vec::new(),
        };
        let after = events.last().map_or(i64::MIN, |e| e.at);
        events.extend(self.agent_events.iter().filter(|e| e.at > after).cloned());
        filter_timeline(events, agent, kind, since, limit)
    }

    /// The Maestro's `timeline`: one line per event with its local time in front and, when they are
    /// many, the counts first.
    pub(crate) fn act_timeline(&self, agent: &str, kind: &str, minutes: Option<i64>, limit: usize) -> Result<String, String> {
        let agent = match agent {
            "" => None,
            name => {
                let rows = self.agent_rows();
                let row = rows.iter().find(|r| r.id == name || r.spec.label().to_lowercase() == name.to_lowercase());
                Some(row.map(|r| r.id.clone()).ok_or_else(|| format!("Nenhum agente chamado {name}."))?)
            }
        };
        let kind = match kind {
            "" => None,
            code => {
                let unknown = || format!("Tipo de evento desconhecido: {code}. Use um destes: {}.", KINDS.map(|(_, c)| c).join(", "));
                Some(KINDS.iter().find(|(_, c)| *c == code).map(|(k, _)| *k).ok_or_else(unknown)?)
            }
        };
        let events = self.timeline_events(agent.as_deref(), kind, minutes.map(|m| now_ms() - m * 60_000), limit);
        if events.is_empty() {
            return Ok("Nada aconteceu nesse período.".into());
        }
        let mut lines = Vec::new();
        if events.len() > 5 {
            lines.push(timeline_summary(&events));
        }
        for e in &events {
            // ponytail: the hour alone, though the window goes up to a day; add the date when yesterday's events confuse
            let clock = crate::app::chat::viewer::local_clock(e.at).unwrap_or_else(|| "??:??".into());
            lines.push(format!("{clock} {}", self.event_line(e)));
        }
        Ok(lines.join("\n"))
    }
}

/// The events of `agent` (what it did, sent or got), of `kind` and not older than `since`: the last
/// `limit`, oldest first.
pub(crate) fn filter_timeline(mut events: Vec<AgentEvent>, agent: Option<&str>, kind: Option<EventKind>, since: Option<i64>, limit: usize) -> Vec<AgentEvent> {
    events.retain(|e| agent.is_none_or(|id| e.id == id || e.to == id) && kind.is_none_or(|k| e.kind == k) && since.is_none_or(|t| e.at >= t));
    events.sort_by_key(|e| e.at);
    events.drain(..events.len().saturating_sub(limit));
    events
}

/// One line of counts, only of what is in `events`: nothing in it is guessed.
pub(crate) fn timeline_summary(events: &[AgentEvent]) -> String {
    let count = |kind: EventKind| events.iter().filter(|e| e.kind == kind).count();
    let agents = events.iter().map(|e| e.id.as_str()).collect::<std::collections::HashSet<_>>().len();
    let some = |n: usize, one: &str, many: &str| format!("{n} {}", if n == 1 { one } else { many });
    format!(
        "Resumo: {}, {}, {} e {}.",
        some(count(EventKind::ToolStarted), "ferramenta", "ferramentas"),
        some(count(EventKind::Permission), "pedido de permissão", "pedidos de permissão"),
        some(count(EventKind::Error), "erro", "erros"),
        some(agents, "agente", "agentes")
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn event(at: i64, id: &str, kind: EventKind) -> AgentEvent {
        AgentEvent { at, ..AgentEvent::new(id, id, kind, "algo") }
    }

    #[test]
    fn the_timeline_filters_by_agent_kind_and_time_and_keeps_the_last_ones() {
        let events = || {
            vec![
                event(300, "b", EventKind::Error),
                event(100, "a", EventKind::ToolStarted),
                event(200, "a", EventKind::Permission),
                AgentEvent { to: "a".into(), ..event(400, "b", EventKind::Message) },
            ]
        };
        let ats = |list: Vec<AgentEvent>| list.iter().map(|e| e.at).collect::<Vec<_>>();
        assert_eq!(ats(filter_timeline(events(), None, None, None, 20)), [100, 200, 300, 400], "oldest first");
        assert_eq!(ats(filter_timeline(events(), Some("a"), None, None, 20)), [100, 200, 400], "what it did and what it got");
        assert_eq!(ats(filter_timeline(events(), None, Some(EventKind::Error), None, 20)), [300]);
        assert_eq!(ats(filter_timeline(events(), None, None, Some(200), 20)), [200, 300, 400]);
        assert_eq!(ats(filter_timeline(events(), None, None, None, 2)), [300, 400], "the last ones");
        assert!(filter_timeline(events(), Some("a"), Some(EventKind::Error), None, 20).is_empty());
    }

    #[test]
    fn the_summary_counts_only_what_came() {
        let events = [
            event(1, "a", EventKind::ToolStarted),
            event(2, "a", EventKind::ToolStarted),
            event(3, "a", EventKind::Permission),
            event(4, "b", EventKind::Error),
            event(5, "b", EventKind::Finished),
        ];
        assert_eq!(timeline_summary(&events), "Resumo: 2 ferramentas, 1 pedido de permissão, 1 erro e 2 agentes.");
        assert_eq!(timeline_summary(&[]), "Resumo: 0 ferramentas, 0 pedidos de permissão, 0 erros e 0 agentes.");
    }

    #[test]
    fn the_maestro_reads_the_timeline_with_the_time_in_front() {
        let mut app = DrawingApp::default();
        let mut e = app.new_element(ElementKind::Rectangle { x: 0.0, y: 0.0, width: 400.0, height: 300.0 });
        e.terminal = Some(TerminalSpec { name: "Revisor".into(), command: "claude".into(), ..Default::default() });
        let id = e.id.clone();
        app.scene.elements.push(e);
        assert_eq!(app.act("timeline", &json!({})).unwrap(), "Nada aconteceu nesse período.");
        app.agent_events.push_back(AgentEvent { at: now_ms() - 2 * 3_600_000, ..AgentEvent::new(&id, "Revisor", EventKind::Error, "antigo") });
        app.agent_events.push_back(AgentEvent::new(&id, "Revisor", EventKind::ToolStarted, "Bash: cargo test"));
        let hour = app.act("timeline", &json!({ "agent": "revisor", "minutes": 60 })).unwrap();
        assert!(hour.contains("Revisor Bash: cargo test") && hour.as_bytes()[0].is_ascii_digit(), "{hour}");
        assert!(!hour.contains("antigo"), "{hour}");
        let all = app.act("timeline", &json!({})).unwrap();
        assert!(all.find("antigo").unwrap() < all.find("cargo test").unwrap(), "oldest first: {all}");
        assert_eq!(app.act("timeline", &json!({ "kind": "error" })).unwrap().lines().count(), 1);
        // more than five: the counts come first
        for n in 0..5 {
            app.agent_events.push_back(AgentEvent::new(&id, "Revisor", EventKind::ToolStarted, format!("Read: {n}")));
        }
        let many = app.act("timeline", &json!({ "limit": 6 })).unwrap();
        assert!(many.starts_with("Resumo: 6 ferramentas, 0 pedidos de permissão, 0 erros e 1 agente.\n"), "{many}");
        assert_eq!(many.lines().count(), 7);
        assert!(app.act("timeline", &json!({ "agent": "ninguém" })).unwrap_err().contains("Nenhum agente"));
        assert!(app.act("timeline", &json!({ "kind": "nada" })).unwrap_err().contains("tool_started, tool_ended"));
    }
}
