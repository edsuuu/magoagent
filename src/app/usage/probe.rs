//! The use of each agent's plan, asked of its own CLI (the reference app's way): Claude Code answers a
//! `get_usage` control request, Codex's app server `account/rateLimits/read`. Each read runs on a thread
//! of its own with a time limit and kills the CLI at the end; no CLI, or one not signed in, is an agent
//! with nothing to show.

use crate::prelude::*;
use std::io::{BufRead, BufReader, Write};
use std::sync::mpsc;
use std::time::{Duration, Instant};

/// How often the plans are read again, in seconds, and how long one read may take.
pub(crate) const REFRESH_SECS: f64 = 300.0;
const READ_LIMIT: Duration = Duration::from_secs(25);

/// The agents whose plans are read.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub(crate) enum Agent {
    Claude,
    Codex,
}

impl Agent {
    pub(crate) const ALL: [Agent; 2] = [Agent::Claude, Agent::Codex];

    pub(crate) fn name(self) -> &'static str {
        match self {
            Agent::Claude => "Claude Code",
            Agent::Codex => "Codex",
        }
    }

    /// The ring's base color (the reference app's).
    pub(crate) fn color(self) -> Color32 {
        match self {
            Agent::Claude => Color32::from_rgb(0xD9, 0x77, 0x57),
            Agent::Codex => Color32::from_rgb(0x10, 0xA3, 0x7F),
        }
    }
}

/// One window of a plan: how much is used (0 to 1) and when it starts over (Unix seconds).
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct Meter {
    pub(crate) title: String,
    pub(crate) used: f64,
    pub(crate) resets_at: Option<i64>,
}

/// What a CLI said of the plan: its name and the windows, session first, then weekly, then the special one.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct Reading {
    pub(crate) plan: Option<String>,
    pub(crate) meters: Vec<Meter>,
}

/// An agent's last answer: the last reading, when it came (Unix seconds) and whether the latest try failed.
#[derive(Clone, Debug, Default)]
pub(crate) struct Slot {
    pub(crate) reading: Option<Reading>,
    pub(crate) at: i64,
    pub(crate) failed: bool,
    busy: bool,
}

/// The agents' readings and the threads that bring them.
pub(crate) struct Probe {
    pub(crate) slots: [Slot; 2],
    /// The egui time of the next read of everything (0: at once).
    pub(crate) next: f64,
    pub(crate) tx: mpsc::Sender<(Agent, Result<Reading, String>)>,
    rx: mpsc::Receiver<(Agent, Result<Reading, String>)>,
}

impl Default for Probe {
    fn default() -> Self {
        let (tx, rx) = mpsc::channel();
        Probe { slots: Default::default(), next: 0.0, tx, rx }
    }
}

impl Probe {
    pub(crate) fn slot(&self, agent: Agent) -> &Slot {
        &self.slots[agent as usize]
    }

    /// Takes the answers that came; true if any did.
    pub(crate) fn collect(&mut self) -> bool {
        let mut any = false;
        while let Ok((agent, answer)) = self.rx.try_recv() {
            let slot = &mut self.slots[agent as usize];
            slot.busy = false;
            slot.failed = answer.is_err();
            if let Ok(reading) = answer {
                (slot.reading, slot.at) = (Some(reading), now_ms() / 1000);
            }
            any = true;
        }
        any
    }

    /// Starts a read of each agent not already being read; the next one comes `REFRESH_SECS` after `now`.
    pub(crate) fn refresh(&mut self, now: f64) {
        self.next = now + REFRESH_SECS;
        for agent in Agent::ALL {
            let slot = &mut self.slots[agent as usize];
            if slot.busy {
                continue;
            }
            slot.busy = true;
            let tx = self.tx.clone();
            std::thread::spawn(move || {
                let _ = tx.send((agent, read(agent)));
            });
        }
    }

    /// Whether any read is under way.
    pub(crate) fn busy(&self) -> bool {
        self.slots.iter().any(|s| s.busy)
    }

    /// The agents with something to show, with what they said.
    pub(crate) fn shown(&self) -> Vec<(Agent, &Reading)> {
        Agent::ALL.into_iter().filter_map(|a| self.slot(a).reading.as_ref().map(|r| (a, r))).collect()
    }

    /// "Claude Code (Max): Sessão 98% (redefine em 5 horas), Semanal 43%" per agent, for the Maestro.
    pub(crate) fn lines(&self, now: i64) -> Vec<String> {
        self.shown()
            .into_iter()
            .map(|(agent, reading)| {
                let plan = reading.plan.as_ref().map(|p| format!(" ({p})")).unwrap_or_default();
                let meters: Vec<String> = reading
                    .meters
                    .iter()
                    .map(|m| match m.resets_at {
                        Some(at) => format!("{} {:.0}% ({})", m.title, m.used * 100.0, reset_text(at, now).to_lowercase()),
                        None => format!("{} {:.0}%", m.title, m.used * 100.0),
                    })
                    .collect();
                format!("{}{plan}: {}", agent.name(), meters.join(", "))
            })
            .collect()
    }
}

/// "Max", "Plus": a plan's name as the CLI writes it, in words.
fn plan_name(plan: &str) -> String {
    plan.replace('_', " ").split_whitespace().map(|w| w[..1].to_uppercase() + &w[1..]).collect::<Vec<_>>().join(" ")
}

/// Unix seconds of an ISO 8601 time ("2026-11-11T06:00:00.9+00:00", the offset or Z at its end).
pub(crate) fn iso_seconds(text: &str) -> Option<i64> {
    let (date, rest) = text.split_once('T')?;
    let mut d = date.split('-').map(|n| n.parse::<i64>().ok());
    let (y, m, day) = (d.next()??, d.next()??, d.next()??);
    let clock: String = rest.chars().take_while(|c| c.is_ascii_digit() || *c == ':').collect();
    let mut t = clock.split(':').map(|n| n.parse::<i64>().ok());
    let (h, min, s) = (t.next()??, t.next()??, t.next().flatten().unwrap_or(0));
    let zone = &rest[clock.len()..];
    let zone = &zone[zone.find(['+', '-', 'Z']).unwrap_or(zone.len())..];
    let offset = match zone.as_bytes().first() {
        Some(b'+') | Some(b'-') => {
            let (hh, mm) = zone[1..].split_once(':')?;
            (hh.parse::<i64>().ok()? * 3600 + mm.parse::<i64>().ok()? * 60) * if zone.starts_with('-') { -1 } else { 1 }
        }
        _ => 0,
    };
    // days since 1970-01-01 of a civil date (Howard Hinnant's algorithm)
    let y = if m <= 2 { y - 1 } else { y };
    let era = y.div_euclid(400);
    let yoe = y - era * 400;
    let doy = (153 * (if m > 2 { m - 3 } else { m + 9 }) + 2) / 5 + day - 1;
    let days = era * 146097 + yoe * 365 + yoe / 4 - yoe / 100 + doy - 719468;
    Some(days * 86400 + h * 3600 + min * 60 + s - offset)
}

/// Claude Code's `get_usage` answer (the control response's inner `response`).
pub(crate) fn parse_claude(response: &Value) -> Result<Reading, String> {
    let limits = &response["rate_limits"];
    let window = |title: &str, v: &Value| {
        Some(Meter { title: title.to_string(), used: v["utilization"].as_f64()? / 100.0, resets_at: v["resets_at"].as_str().and_then(iso_seconds) })
    };
    let mut meters: Vec<Meter> = [window("Sessão", &limits["five_hour"]), window("Semanal", &limits["seven_day"])].into_iter().flatten().collect();
    if meters.is_empty() {
        return Err("sem limites de plano".into());
    }
    // the special ring: the first limit of a model alone
    let scoped = &limits["model_scoped"].as_array().or(response["model_scoped"].as_array()).and_then(|a| a.first()).cloned().unwrap_or(Value::Null);
    let title = scoped["display_name"].as_str().or(scoped["name"].as_str()).unwrap_or("Especial");
    meters.extend(window(title, scoped));
    Ok(Reading { plan: response["subscription_type"].as_str().map(plan_name), meters })
}

/// Codex's `account/rateLimits/read` result.
pub(crate) fn parse_codex(result: &Value) -> Result<Reading, String> {
    let limits = &result["rateLimits"];
    let window = |v: &Value| {
        let title = match v["windowDurationMins"].as_i64() {
            Some(m) if m <= 1440 => "Sessão".to_string(),
            Some(m) if m >= 10080 => "Semanal".to_string(),
            Some(m) => format!("{} h", m / 60),
            None => "Sessão".to_string(),
        };
        Some(Meter { title, used: v["usedPercent"].as_f64()? / 100.0, resets_at: v["resetsAt"].as_i64() })
    };
    let meters: Vec<Meter> = [window(&limits["primary"]), window(&limits["secondary"])].into_iter().flatten().collect();
    if meters.is_empty() {
        return Err("sem limites de plano".into());
    }
    Ok(Reading { plan: limits["planType"].as_str().map(plan_name), meters })
}

/// "Redefine em 2 horas", "Redefine amanhã", "Redefinindo agora": when a window starts over, as the
/// reference app says it (Intl's long, automatic relative time in Portuguese).
pub(crate) fn reset_text(at: i64, now: i64) -> String {
    if at <= now {
        return "Redefinindo agora".into();
    }
    format!("Redefine {}", relative(at - now))
}

/// When a window starts over, on the local clock: "hoje às 22:30", "amanhã às 05:00",
/// "seg, 17/11 às 05:00". `offset`: the local zone's seconds from UTC.
pub(crate) fn reset_when(at: i64, now: i64, offset: i64) -> String {
    let (local, today) = (at + offset, (now + offset).div_euclid(86_400));
    let day = local.div_euclid(86_400);
    let minutes = local.rem_euclid(86_400) / 60;
    let clock = format!("{:02}:{:02}", minutes / 60, minutes % 60);
    match day - today {
        0 => format!("hoje às {clock}"),
        1 => format!("amanhã às {clock}"),
        _ => {
            // the civil date of the day count (days from 1970-01-01), Howard Hinnant's algorithm
            let z = day + 719_468;
            let era = z.div_euclid(146_097);
            let doe = z - era * 146_097;
            let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
            let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
            let mp = (5 * doy + 2) / 153;
            let (d, m) = (doy - (153 * mp + 2) / 5 + 1, if mp < 10 { mp + 3 } else { mp - 9 });
            let week = ["qui", "sex", "sáb", "dom", "seg", "ter", "qua"][day.rem_euclid(7) as usize];
            format!("{week}, {d:02}/{m:02} às {clock}")
        }
    }
}

/// "Atualizada há 5 minutos".
pub(crate) fn updated_text(at: i64, now: i64) -> String {
    let ago = now - at;
    if ago < 1 { "Atualizada agora".into() } else { format!("Atualizada {}", relative(-ago)) }
}

/// Seconds from now (negative: ago) in the largest unit that fits, rounded: "em 3 dias", "há 5 minutos".
fn relative(seconds: i64) -> String {
    // (size in seconds, singular, plural, next one, the one before)
    const UNITS: [(i64, &str, &str, &str, &str); 7] = [
        (31536000, "ano", "anos", "próximo ano", "ano passado"),
        (2592000, "mês", "meses", "próximo mês", "mês passado"),
        (604800, "semana", "semanas", "próxima semana", "semana passada"),
        (86400, "dia", "dias", "amanhã", "ontem"),
        (3600, "hora", "horas", "", ""),
        (60, "minuto", "minutos", "", ""),
        (1, "segundo", "segundos", "", ""),
    ];
    let abs = seconds.abs();
    let (size, one, many, next, last) = *UNITS.iter().find(|u| abs >= u.0).unwrap_or(&UNITS[6]);
    let n = (abs as f64 / size as f64).round() as i64;
    match (n, seconds > 0) {
        (1, true) if !next.is_empty() => next.to_string(),
        (1, false) if !last.is_empty() => last.to_string(),
        (n, true) => format!("em {n} {}", if n == 1 { one } else { many }),
        (n, false) => format!("há {n} {}", if n == 1 { one } else { many }),
    }
}

/// Reads one agent's plan on this thread: the CLI is asked, and killed at the end.
fn read(agent: Agent) -> Result<Reading, String> {
    let program = crate::ide::lsp::which(match agent {
        Agent::Claude => "claude",
        Agent::Codex => "codex",
    })
    .ok_or("CLI não encontrada")?;
    match agent {
        Agent::Claude => exchange(
            &program,
            &["-p", "--input-format", "stream-json", "--output-format", "stream-json", "--verbose", "--no-session-persistence"],
            &[json!({ "type": "control_request", "request_id": "usage", "request": { "subtype": "get_usage", "skip_behaviors": true } })],
            |line| {
                (line["type"] == "control_response").then(|| match &line["response"] {
                    r if r["subtype"] == "success" => parse_claude(&r["response"]),
                    r => Err(r["error"].as_str().unwrap_or("a CLI recusou o pedido").to_string()),
                })
            },
        ),
        Agent::Codex => exchange(
            &program,
            &["app-server", "--stdio"],
            &[
                json!({ "id": 1, "method": "initialize", "params": { "clientInfo": { "name": "magoagent", "version": "1" } } }),
                json!({ "method": "initialized" }),
                json!({ "id": 2, "method": "account/rateLimits/read", "params": { "excludeResetCreditDetails": true } }),
            ],
            |line| {
                (line["id"] == 2).then(|| match line.get("result") {
                    Some(result) => parse_codex(result),
                    None => Err(line["error"]["message"].as_str().unwrap_or("a CLI recusou o pedido").to_string()),
                })
            },
        ),
    }
}

/// Starts `program`, sends it the `requests` (one JSON per line) and waits for the first line `answer`
/// takes as its own, up to `READ_LIMIT`; the program is killed whatever comes.
fn exchange(program: &std::path::Path, args: &[&str], requests: &[Value], answer: fn(&Value) -> Option<Result<Reading, String>>) -> Result<Reading, String> {
    let mut child = crate::ide::command(program)
        .args(args)
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::null())
        .spawn()
        .map_err(|e| format!("não foi possível iniciar a CLI: {e}"))?;
    let mut stdin = child.stdin.take();
    for request in requests {
        if let Some(stdin) = stdin.as_mut() {
            let _ = writeln!(stdin, "{request}");
            let _ = stdin.flush();
        }
    }
    let (lines, incoming) = mpsc::channel();
    if let Some(stdout) = child.stdout.take() {
        std::thread::spawn(move || {
            for line in BufReader::new(stdout).lines().map_while(Result::ok) {
                if lines.send(line).is_err() {
                    break;
                }
            }
        });
    }
    let end = Instant::now() + READ_LIMIT;
    let result = loop {
        match incoming.recv_timeout(end.saturating_duration_since(Instant::now())) {
            Ok(line) => {
                if let Some(found) = serde_json::from_str::<Value>(&line).ok().and_then(|v| answer(&v)) {
                    break found;
                }
            }
            Err(mpsc::RecvTimeoutError::Timeout) => break Err("a CLI não respondeu a tempo".into()),
            Err(mpsc::RecvTimeoutError::Disconnected) => break Err("a CLI fechou sem responder (sem login?)".into()),
        }
    };
    drop(stdin);
    // the npm shims are a shell around node: on Windows the whole tree goes
    if cfg!(windows) {
        let _ = crate::ide::command("taskkill").args(["/PID", &child.id().to_string(), "/T", "/F"]).output();
    }
    let _ = child.kill();
    let _ = child.wait();
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn claude_answers_with_sessions_weeks_and_a_model() {
        let said = json!({ "subscription_type": "max", "rate_limits_available": true, "rate_limits": {
            "five_hour": { "utilization": 98, "resets_at": "2026-11-11T06:00:00.923850+00:00" },
            "seven_day": { "utilization": 43, "resets_at": "2026-11-17T05:00:00.923870+00:00" },
            "seven_day_opus": null,
            "model_scoped": [{ "display_name": "Fable", "utilization": 79, "resets_at": "2026-11-17T05:00:00+00:00" }] } });
        let reading = parse_claude(&said).unwrap();
        assert_eq!(reading.plan.as_deref(), Some("Max"));
        let titles: Vec<_> = reading.meters.iter().map(|m| (m.title.as_str(), m.used)).collect();
        assert_eq!(titles, [("Sessão", 0.98), ("Semanal", 0.43), ("Fable", 0.79)]);
        assert_eq!(reading.meters[0].resets_at, Some(1794376800));
        // no subscription: nothing to show
        assert!(parse_claude(&json!({ "rate_limits_available": false, "rate_limits": null })).is_err());
    }

    #[test]
    fn codex_answers_with_two_windows() {
        let said = json!({ "ordinaryUsageAllowed": true, "rateLimits": { "planType": "plus",
            "primary": { "usedPercent": 0, "windowDurationMins": 300, "resetsAt": 1794379303 },
            "secondary": { "usedPercent": 4, "windowDurationMins": 10080, "resetsAt": 1794442229 } } });
        let reading = parse_codex(&said).unwrap();
        assert_eq!(reading.plan.as_deref(), Some("Plus"));
        assert_eq!(reading.meters[0], Meter { title: "Sessão".into(), used: 0.0, resets_at: Some(1794379303) });
        assert_eq!((reading.meters[1].title.as_str(), reading.meters[1].used), ("Semanal", 0.04));
        assert!(parse_codex(&json!({ "rateLimits": { "primary": null } })).is_err());
    }

    #[test]
    fn iso_times_and_resets_read_in_portuguese() {
        assert_eq!(iso_seconds("1970-01-02T00:00:00Z"), Some(86400));
        assert_eq!(iso_seconds("2026-11-11T03:00:00-03:00"), iso_seconds("2026-11-11T06:00:00+00:00"));
        assert_eq!(iso_seconds("lixo"), None);
        let now = 1_000_000;
        assert_eq!(reset_text(now - 5, now), "Redefinindo agora");
        assert_eq!(reset_text(now + 40, now), "Redefine em 40 segundos");
        assert_eq!(reset_text(now + 5 * 3600 + 600, now), "Redefine em 5 horas");
        assert_eq!(reset_text(now + 3600, now), "Redefine em 1 hora");
        assert_eq!(reset_text(now + 86400, now), "Redefine amanhã");
        assert_eq!(reset_text(now + 3 * 86400, now), "Redefine em 3 dias");
        // 2026-11-11 21:00 UTC is 18:00 in Brasília (-3 h)
        let (now, br) = (1_794_430_800, -3 * 3600);
        assert_eq!(reset_when(now + 4 * 3600 + 1800, now, br), "hoje às 22:30");
        assert_eq!(reset_when(now + 11 * 3600, now, br), "amanhã às 05:00");
        assert_eq!(reset_when(now + 5 * 86400 + 11 * 3600, now, br), "ter, 17/11 às 05:00");
        assert_eq!(updated_text(now - 300, now), "Atualizada há 5 minutos");
        assert_eq!(updated_text(now - 86400, now), "Atualizada ontem");
    }

    #[test]
    fn answers_come_in_and_a_failure_keeps_the_last_reading() {
        let mut probe = Probe::default();
        let reading = Reading { plan: None, meters: vec![Meter { title: "Sessão".into(), used: 0.5, resets_at: None }] };
        probe.tx.send((Agent::Codex, Ok(reading.clone()))).unwrap();
        assert!(probe.collect());
        assert_eq!(probe.shown().len(), 1);
        assert_eq!(probe.lines(0), ["Codex: Sessão 50%"]);
        probe.tx.send((Agent::Codex, Err("x".into()))).unwrap();
        probe.collect();
        assert!(probe.slot(Agent::Codex).failed && probe.slot(Agent::Codex).reading == Some(reading), "the last reading stays, marked");
        assert!(probe.slot(Agent::Claude).reading.is_none());
    }
}
