//! What the work costs: how much of the plan's limits is used (Claude Code says it in its status line
//! and in the chats' stream), each agent's model, context and cost, and the rules that hold the Maestro
//! back: a ceiling of agents, no recruiting near the limit, a cheaper model for the recruits.

use crate::app::demands::{Demand, DemandKind};
use crate::prelude::*;
use std::io::Read;

/// One of the plan's windows: how much of it is used (0 to 100) and when it starts over (Unix seconds).
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct Window {
    pub(crate) used: f64,
    pub(crate) resets_at: i64,
}

/// The plan's limits as last told by a Claude Code session.
#[derive(Clone, Debug, Default, PartialEq)]
pub(crate) struct Plan {
    pub(crate) five_hour: Option<Window>,
    pub(crate) seven_day: Option<Window>,
}

/// What one agent's session said of itself.
#[derive(Clone, Debug, Default, PartialEq)]
pub(crate) struct AgentUsage {
    pub(crate) model: String,
    /// Of its context window, 0 to 100.
    pub(crate) context: Option<f64>,
    /// The session so far, in dollars at list price (the CLI's own estimate, not a bill).
    pub(crate) cost: Option<f64>,
}

/// The app's side: the plan, and each agent by its terminal's or chat's id.
#[derive(Clone, Debug, Default)]
pub(crate) struct Usage {
    pub(crate) plan: Plan,
    pub(crate) agents: std::collections::HashMap<String, AgentUsage>,
}

/// The first of `keys` the object has: the CLI's names come in snake_case or camelCase.
fn field<'a>(value: &'a Value, keys: [&str; 2]) -> &'a Value {
    keys.iter().map(|k| &value[*k]).find(|v| !v.is_null()).unwrap_or(&Value::Null)
}

fn seconds(value: &Value) -> Option<i64> {
    value.as_i64().or_else(|| value.as_f64().map(|f| f as i64))
}

/// The plan's windows in a status line's input (`rate_limits`): each may be missing (no subscription,
/// no answer from the API yet in that session).
pub(crate) fn plan_of_status(status: &Value) -> Plan {
    let window = |v: &Value| Some(Window { used: v["used_percentage"].as_f64()?, resets_at: seconds(&v["resets_at"])? });
    Plan { five_hour: window(&status["rate_limits"]["five_hour"]), seven_day: window(&status["rate_limits"]["seven_day"]) }
}

/// The session's model, context and cost in a status line's input.
pub(crate) fn agent_of_status(status: &Value) -> AgentUsage {
    let model = status["model"]["display_name"].as_str().or_else(|| status["model"]["id"].as_str());
    AgentUsage {
        model: model.unwrap_or_default().to_string(),
        context: status["context_window"]["used_percentage"].as_f64(),
        cost: status["cost"]["total_cost_usd"].as_f64(),
    }
}

/// A limit event of a chat's stream (`rate_limit_info`, one per answer): the plan's windows, with
/// the use from 0 to 100.
// The CLI's stream (seen in 2.1.321) tells both windows at once, in `unifiedWindows`; the SDK's
// documented type tells one, named by `rate_limit_type`. Both are read, under either spelling.
pub(crate) fn plan_of_event(line: &Value) -> Plan {
    let info = field(line, ["rate_limit_info", "rateLimitInfo"]);
    let window = |v: &Value| Some(Window { used: v["utilization"].as_f64()? * 100.0, resets_at: seconds(field(v, ["resets_at", "resetsAt"]))? });
    let unified = field(info, ["unified_windows", "unifiedWindows"]);
    if unified.is_object() {
        return Plan { five_hour: window(field(unified, ["five_hour", "fiveHour"])), seven_day: window(field(unified, ["seven_day", "sevenDay"])) };
    }
    match field(info, ["rate_limit_type", "rateLimitType"]).as_str() {
        Some("five_hour") => Plan { five_hour: window(info), seven_day: None },
        Some("seven_day") => Plan { five_hour: None, seven_day: window(info) },
        _ => Plan::default(),
    }
}

impl Plan {
    /// Takes what a session told: a window it didn't speak of stays as it was, and one whose time
    /// passed goes (its use started over).
    pub(crate) fn merge(&mut self, new: Plan, now: i64) {
        self.five_hour = new.five_hour.or(self.five_hour).filter(|w| w.resets_at > now);
        self.seven_day = new.seven_day.or(self.seven_day).filter(|w| w.resets_at > now);
    }

    /// "5 h 41%" for the Maestro's bar; None with nothing read, or once that window started over.
    pub(crate) fn short(&self, now: i64) -> Option<String> {
        self.five_hour.filter(|w| w.resets_at > now).map(|w| format!("5 h {:.0}%", w.used))
    }

    /// "5 h em 41% (reinicia às 23:20) · semana em 48%"; None with nothing read. `clock` gives the
    /// local "HH:MM" of a time in Unix seconds.
    pub(crate) fn line(&self, clock: impl Fn(i64) -> String) -> Option<String> {
        let five = self.five_hour.map(|w| format!("5 h em {:.0}% (reinicia às {})", w.used, clock(w.resets_at)));
        let week = self.seven_day.map(|w| format!("semana em {:.0}%", w.used));
        let parts: Vec<String> = [five, week].into_iter().flatten().collect();
        (!parts.is_empty()).then(|| parts.join(" · "))
    }
}

impl AgentUsage {
    /// "Opus · contexto 23% · US$ 1,20", with what the session told.
    pub(crate) fn line(&self) -> String {
        let context = self.context.map(|c| format!("contexto {c:.0}%"));
        let cost = self.cost.map(|c| format!("US$ {}", format!("{c:.2}").replace('.', ",")));
        [Some(self.model.clone()).filter(|m| !m.is_empty()), context, cost].into_iter().flatten().collect::<Vec<_>>().join(" · ")
    }
}

/// Why the Maestro doesn't start another agent now, when it doesn't: the ceiling of agents at work
/// (`max_agents`, 0 for none) or the 5-hour window at `stop_at` percent or more (0 for never).
pub(crate) fn recruit_block(running: usize, max_agents: usize, plan: &Plan, stop_at: u8, clock: impl Fn(i64) -> String) -> Option<String> {
    if max_agents > 0 && running >= max_agents {
        return Some(format!(
            "Já há {running} agentes rodando e o teto é {max_agents} (Ajustes › Desempenho). Espere um terminar ou feche um antes de recrutar outro."
        ));
    }
    let window = plan.five_hour.filter(|w| stop_at > 0 && w.used >= stop_at as f64)?;
    Some(format!(
        "A janela de 5 h do plano está em {:.0}% (reinicia às {}) e o app não recruta a partir de {stop_at}% (Ajustes › Desempenho). \
         Faça você mesmo, espere o reinício ou avise a pessoa.",
        window.used,
        clock(window.resets_at)
    ))
}

/// A recruit's command with the cheaper model of Ajustes: only a `claude` that names no model.
pub(crate) fn recruit_command(command: &str, model: &str) -> String {
    let command = command.trim();
    let mut words = command.split_whitespace();
    let claude = words.next() == Some("claude");
    if model.is_empty() || !claude || words.any(|w| w == "--model" || w.starts_with("--model=")) {
        return command.to_string();
    }
    format!("{command} --model {model}")
}

/// The line under the prompt of a session the app started: its model, its context and the plan.
pub(crate) fn status_text(status: &Value, clock: impl Fn(i64) -> String) -> String {
    let agent = AgentUsage { cost: None, ..agent_of_status(status) };
    [Some(agent.line()).filter(|l| !l.is_empty()), plan_of_status(status).line(clock)].into_iter().flatten().collect::<Vec<_>>().join(" · ")
}

fn local_clock(seconds: i64) -> String {
    super::chat_viewer::local_clock(seconds * 1000).unwrap_or_else(|| "??:??".into())
}

/// `magoagent statusline`: Claude Code's status line in the sessions the app starts (the `--settings`
/// of the hooks). What the CLI tells (JSON on stdin) goes to the app by the bridge, as that terminal's
/// or chat's agent, and one short line comes out; without the app the line still does.
pub(crate) fn statusline_cli() -> i32 {
    let mut input = String::new();
    let _ = std::io::stdin().read_to_string(&mut input);
    let Ok(mut status) = serde_json::from_str::<Value>(&input) else { return 0 };
    println!("{}", status_text(&status, local_clock));
    let env = |key: &str| std::env::var(key).ok().filter(|v| !v.is_empty());
    if let (Some(address), Some(token), Some(from)) =
        (env("MAGOAGENT_BRIDGE"), env("MAGOAGENT_TOKEN"), env("MAGOAGENT_TERMINAL_ID").or_else(|| env("MAGOAGENT_CHAT_ID")))
    {
        // it rides the hooks' way to the app (`take_hook`)
        status["hook_event_name"] = STATUS_LINE.into();
        let _ = request_to(&address, &token, &from, json!({ "cmd": "hook", "payload": status, "wait": 0 }), std::time::Duration::from_secs(2));
    }
    0
}

/// The name a status line goes by on the hooks' way.
pub(crate) const STATUS_LINE: &str = "StatusLine";

impl DrawingApp {
    /// What a session's status line told: the plan's windows and that agent's model, context and cost.
    pub(crate) fn take_status(&mut self, from: &str, status: &Value) {
        self.usage.plan.merge(plan_of_status(status), now_ms() / 1000);
        self.usage.agents.insert(from.to_string(), agent_of_status(status));
    }

    /// Agents at work: the terminals that run a command and whose shell didn't end (one about to
    /// start counts).
    // ponytail: the turns of the direct chats don't count; add them if the ceiling should hold those too
    pub(crate) fn running_agents(&self) -> usize {
        let alive = |id: &str| self.terminals.get(id).is_none_or(|s| s.as_ref().is_ok_and(|s| !s.exited()));
        let here = self.scene.elements.iter().filter(|e| e.terminal.as_ref().is_some_and(|t| !t.unloaded && !t.command.trim().is_empty()) && alive(&e.id));
        // the ones whose workspace is off view run too
        let away = self.away.iter().filter(|(id, (_, spec))| !spec.command.trim().is_empty() && alive(id));
        here.count() + away.count()
    }

    /// Why a recruit is refused now, if it is (`recruit_block`).
    pub(crate) fn recruit_refusal(&mut self) -> Option<String> {
        self.usage.plan.merge(Plan::default(), now_ms() / 1000);
        recruit_block(self.running_agents(), self.prefs.max_agents, &self.usage.plan, self.prefs.usage_stop, local_clock)
    }

    /// The plan near its end, as a demand of the Maestro's: the 5-hour window at the mark of Ajustes
    /// or over it, until it starts over.
    pub(crate) fn usage_demand(&self) -> Option<Demand> {
        let stop = self.prefs.usage_stop;
        let window = self.usage.plan.five_hour.filter(|w| stop > 0 && w.used >= stop as f64 && w.resets_at > now_ms() / 1000)?;
        Some(Demand {
            id: String::new(),
            name: "Plano".into(),
            kind: DemandKind::Limit,
            what: format!("a janela de 5 h está em {:.0}% e reinicia às {}", window.used, local_clock(window.resets_at)),
            waited: None,
            points: vec!["o app não recruta agentes até o reinício".into()],
            answerable: false,
        })
    }

    /// The plan's line for the screen and the Maestro, or why there is none yet.
    pub(crate) fn plan_line(&self) -> String {
        self.usage.plan.line(local_clock).unwrap_or_else(|| {
            "ainda sem leitura (vem do Claude Code depois da primeira resposta de um agente, com Acompanhar os agentes ligado em Ajustes › Terminal)".into()
        })
    }

    /// The agents that told their use and still exist, by name: (name, "Opus · contexto 23% · US$ 1,20").
    pub(crate) fn agent_usage_lines(&self) -> Vec<(String, String)> {
        let there = |id: &String| self.scene.elements.iter().any(|e| &e.id == id) || self.chats.iter().any(|c| &c.id == id);
        let mut lines: Vec<(String, String)> = self.usage.agents.iter().filter(|(id, _)| there(id)).map(|(id, u)| (self.agent_label(id), u.line())).collect();
        lines.sort();
        lines
    }

    /// The Maestro's `usage`: the plan, how many agents run against the ceiling, and each agent's use.
    pub(crate) fn act_usage(&self) -> Result<String, String> {
        let ceiling = match self.prefs.max_agents {
            0 => "sem teto".to_string(),
            max => format!("teto de {max}"),
        };
        let mut out = format!("Plano: {}.\nAgentes rodando: {} ({ceiling}).", self.plan_line(), self.running_agents());
        if self.prefs.usage_stop > 0 {
            out += &format!("\nO app não recruta com a janela de 5 h em {}% ou mais.", self.prefs.usage_stop);
        }
        for (name, line) in self.agent_usage_lines() {
            out += &format!("\n{name}: {line}");
        }
        Ok(out)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn clock(seconds: i64) -> String {
        format!("t{seconds}")
    }

    #[test]
    fn the_status_line_tells_the_plan_and_the_agent() {
        let status = json!({
            "model": { "id": "claude-opus", "display_name": "Opus" },
            "context_window": { "used_percentage": 23.4 },
            "cost": { "total_cost_usd": 1.2 },
            "rate_limits": { "five_hour": { "used_percentage": 41.2, "resets_at": 500 }, "seven_day": { "used_percentage": 48.0, "resets_at": 9000.0 } },
        });
        let plan = plan_of_status(&status);
        assert_eq!(plan, Plan { five_hour: Some(Window { used: 41.2, resets_at: 500 }), seven_day: Some(Window { used: 48.0, resets_at: 9000 }) });
        assert_eq!(plan.line(clock).unwrap(), "5 h em 41% (reinicia às t500) · semana em 48%");
        assert_eq!((plan.short(499), plan.short(500)), (Some("5 h 41%".to_string()), None), "the bar's text, until the window starts over");
        assert_eq!(agent_of_status(&status).line(), "Opus · contexto 23% · US$ 1,20");
        assert_eq!(status_text(&status, clock), "Opus · contexto 23% · 5 h em 41% (reinicia às t500) · semana em 48%");
        // before the first answer, or without a subscription: nothing about the plan, and nothing made up
        let bare = json!({ "model": { "id": "claude-sonnet" } });
        assert_eq!(plan_of_status(&bare), Plan::default());
        assert_eq!(plan_of_status(&bare).line(clock), None);
        assert_eq!(status_text(&bare, clock), "claude-sonnet");
        assert_eq!(status_text(&json!({}), clock), "");
        // one window alone
        let week = plan_of_status(&json!({ "rate_limits": { "seven_day": { "used_percentage": 7, "resets_at": 1 } } }));
        assert_eq!(week.line(clock).unwrap(), "semana em 7%");
    }

    #[test]
    fn a_window_stays_until_its_time_and_a_chats_event_counts() {
        let mut plan = Plan::default();
        plan.merge(plan_of_status(&json!({ "rate_limits": { "five_hour": { "used_percentage": 30, "resets_at": 100 } } })), 10);
        // a session that says nothing of the plan changes nothing
        plan.merge(Plan::default(), 20);
        assert_eq!(plan.five_hour, Some(Window { used: 30.0, resets_at: 100 }));
        // the chat's stream, under either spelling
        plan.merge(
            plan_of_event(&json!({ "type": "rate_limit_event", "rate_limit_info": { "rate_limit_type": "seven_day", "utilization": 0.5, "resets_at": 900 } })),
            20,
        );
        plan.merge(plan_of_event(&json!({ "rateLimitInfo": { "rateLimitType": "five_hour", "utilization": 0.86, "resetsAt": 100 } })), 20);
        assert_eq!(plan.line(clock).unwrap(), "5 h em 86% (reinicia às t100) · semana em 50%");
        assert_eq!(plan_of_event(&json!({ "type": "assistant" })), Plan::default());
        // the event as Claude Code 2.1.321 writes it: both windows at once
        let real = json!({ "type": "rate_limit_event", "rate_limit_info": { "status": "allowed", "resetsAt": 1794295200, "rateLimitType": "five_hour", "isUsingOverage": false,
            "unifiedWindows": { "five_hour": { "utilization": 0.5, "resetsAt": 1794295200, "status": "allowed" }, "seven_day": { "utilization": 0.55, "resetsAt": 1794286800, "status": "allowed" } } } });
        let read = plan_of_event(&real);
        assert_eq!(read.five_hour, Some(Window { used: 50.0, resets_at: 1794295200 }));
        assert_eq!(read.seven_day.map(|w| (w.used.round(), w.resets_at)), Some((55.0, 1794286800)));
        assert_eq!(plan_of_event(&json!({ "rate_limit_info": { "rate_limit_type": "overage", "utilization": 1.0, "resets_at": 5 } })), Plan::default());
        // its time passed: the window starts over, so what was read is no longer true
        plan.merge(Plan::default(), 100);
        assert_eq!((plan.five_hour, plan.seven_day.map(|w| w.used)), (None, Some(50.0)));
    }

    #[test]
    fn the_maestro_is_held_by_the_ceiling_and_by_the_limit() {
        let plan = |used: f64| Plan { five_hour: Some(Window { used, resets_at: 7 }), seven_day: None };
        assert_eq!(recruit_block(2, 6, &plan(50.0), 90, clock), None);
        assert!(recruit_block(6, 6, &plan(50.0), 90, clock).unwrap().starts_with("Já há 6 agentes rodando e o teto é 6"));
        assert_eq!(recruit_block(40, 0, &plan(50.0), 90, clock), None, "no ceiling");
        let near = recruit_block(1, 6, &plan(90.0), 90, clock).unwrap();
        assert!(near.contains("está em 90% (reinicia às t7)") && near.contains("a partir de 90%"), "{near}");
        assert_eq!(recruit_block(1, 6, &plan(99.0), 0, clock), None, "never stops");
        assert_eq!(recruit_block(1, 6, &Plan::default(), 90, clock), None, "nothing read holds nothing");
        // the recruits' model: only a claude that names none
        assert_eq!(recruit_command(" claude ", "sonnet"), "claude --model sonnet");
        assert_eq!(recruit_command("claude \"revise o diff\"", "haiku"), "claude \"revise o diff\" --model haiku");
        assert_eq!(recruit_command("claude --model opus", "sonnet"), "claude --model opus");
        assert_eq!(recruit_command("claude --model=opus -c", "sonnet"), "claude --model=opus -c");
        assert_eq!(recruit_command("codex", "sonnet"), "codex");
        assert_eq!(recruit_command("claude", ""), "claude");
    }

    #[test]
    fn the_app_keeps_what_the_sessions_tell_and_refuses_a_recruit_over_the_ceiling() {
        let mut app = DrawingApp::default();
        let add = |app: &mut DrawingApp, name: &str, command: &str| {
            let mut e = app.new_element(ElementKind::Rectangle { x: 0.0, y: 0.0, width: 400.0, height: 300.0 });
            e.terminal = Some(TerminalSpec { name: name.into(), command: command.into(), ..Default::default() });
            let id = e.id.clone();
            app.scene.elements.push(e);
            id
        };
        let maestro = add(&mut app, "Maestro", "claude");
        add(&mut app, "Shell", "");
        assert_eq!(app.running_agents(), 1, "a plain shell is no agent");
        let reset = now_ms() / 1000 + 3600;
        let status = json!({ "hook_event_name": STATUS_LINE, "model": { "display_name": "Opus" }, "context_window": { "used_percentage": 12 },
            "rate_limits": { "five_hour": { "used_percentage": 95, "resets_at": reset } } });
        app.take_hook(&maestro, &status, None);
        assert!(app.agent_events.is_empty(), "a status line is no event");
        let said = app.act("usage", &json!({})).unwrap();
        assert!(
            said.starts_with("Plano: 5 h em 95% (reinicia às ")
                && said.contains("Agentes rodando: 1 (teto de 6).")
                && said.contains("Maestro: Opus · contexto 12%"),
            "{said}"
        );
        // near the limit the Maestro recruits nobody; with the rule off, or under the mark, it does
        let recruit = |app: &mut DrawingApp| {
            let (reply, answer) = std::sync::mpsc::channel();
            app.recruit(&maestro, "claude", reply);
            answer.try_recv().unwrap()
        };
        assert!(recruit(&mut app).unwrap_err().contains("janela de 5 h"));
        assert_eq!(app.running_agents(), 1);
        // and it is a demand the Maestro brings, with what follows from it
        let demand = app.demands().into_iter().find(|d| d.kind == DemandKind::Limit).unwrap();
        assert!(
            demand.name == "Plano" && demand.what.starts_with("a janela de 5 h está em 95%") && demand.points == ["o app não recruta agentes até o reinício"]
        );
        assert!(app.demand_line().unwrap().contains("Plano a janela de 5 h está em 95%"), "{:?}", app.demand_line());
        app.prefs.usage_stop = 0;
        app.prefs.recruit_model = "sonnet".into();
        assert!(recruit(&mut app).is_ok());
        let recruited = app.scene.elements.iter().filter_map(|e| e.terminal.as_ref()).find(|t| t.command.contains("--model")).unwrap();
        assert_eq!(recruited.command, "claude --model sonnet");
        app.prefs.max_agents = 2;
        assert!(recruit(&mut app).unwrap_err().starts_with("Já há 2 agentes rodando"));
        // the settings by the MCP
        app.act("set_setting", &json!({ "name": "max_agents", "value": "0" })).unwrap();
        app.act("set_setting", &json!({ "name": "usage_stop", "value": "80" })).unwrap();
        app.act("set_setting", &json!({ "name": "recruit_model", "value": "haiku" })).unwrap();
        assert_eq!((app.prefs.max_agents, app.prefs.usage_stop, app.prefs.recruit_model.as_str()), (0, 80, "haiku"));
        assert!(app.act("set_setting", &json!({ "name": "usage_stop", "value": "150" })).is_err());
    }
}
