//! The topic "chat_context": the Contexto panel beside the open chat (chat/context_view.rs) while it is open, from the
//! same reads (chat/context_ui.rs); null while it is closed. Its header's ×, the ring and Atualizar go back as the MCP's
//! `chat_context` (close, open, refresh); Compactar, Tentar and "Compactar automaticamente em" as `ui.chat.context`.

use crate::app::chat::context::*;
use crate::app::chat::context_ui::{Info, connectors_note, summary_note};
use crate::app::chat::context_view::{ORDER, initial, initial_ink, key_style};
use crate::prelude::*;

#[derive(Serialize, PartialEq, Debug)]
pub(super) struct PanelSnapshot {
    pub(super) chat: String,
    /// Atualizar turns while the `/context` is read.
    pub(super) loading: bool,
    /// What the body says while no `/context` was read ("" once one was).
    pub(super) note: String,
    pub(super) report: Option<ReportSnapshot>,
    /// "3 de 5 conectados" ("" with none), the connectors with the ones that need attention first, and what the list
    /// says when there is none.
    pub(super) connected: String,
    pub(super) connectors: Vec<ConnectorSnapshot>,
    pub(super) connectors_note: &'static str,
    /// More than 6: past the first 5 they fold into this ("+ 3 conectados").
    pub(super) more: String,
}

#[derive(Serialize, PartialEq, Debug)]
pub(super) struct ReportSnapshot {
    /// "75%" and "301,8k de 400k tokens".
    pub(super) percent: String,
    pub(super) used: String,
    /// The stacked bar: each category that counts, its share of the window and its colour (rgba).
    pub(super) bar: Vec<(f32, [u8; 4])>,
    pub(super) legend: Vec<LegendRow>,
    /// "Compacta sozinho em" this many tokens.
    pub(super) until: String,
    /// "Compactar automaticamente em": the value picked ("" Automático) and the others.
    pub(super) autocompact: String,
    pub(super) choices: [&'static str; 8],
    /// The MCP tools one by one: "server · name" and their tokens.
    pub(super) tools: Vec<(String, String)>,
    /// The footer: "Memória: CLAUDE.md e mais 1" ("" without memory files), their tokens and each file in its tip.
    pub(super) memory: String,
    pub(super) memory_tokens: String,
    pub(super) memory_tip: String,
}

#[derive(Serialize, PartialEq, Debug)]
pub(super) struct LegendRow {
    pub(super) name: &'static str,
    pub(super) color: [u8; 4],
    /// Counts in the window: a filled square and its share in the tip; else an empty one.
    pub(super) counts: bool,
    pub(super) tokens: String,
    pub(super) tip: String,
}

#[derive(Serialize, PartialEq, Debug)]
pub(super) struct ConnectorSnapshot {
    pub(super) name: String,
    pub(super) letter: String,
    pub(super) ink: [u8; 3],
    /// "12 ferramentas", "Precisa entrar", "Não respondeu"...
    pub(super) note: String,
    /// connected, failed (Tentar), needs_auth or pending.
    pub(super) status: &'static str,
    /// Its tools, shown when the row is clicked.
    pub(super) tools: Vec<String>,
}

fn report_snapshot(info: &Info, report: &Report, autocompact: &str) -> ReportSnapshot {
    let since = info.since();
    let used = report.used + since;
    let window = report.window.max(1);
    let value = |key: &str| if key == "Since" { since } else { report.cat(key) };
    let share = |key: &str| value(key) as f32 / window as f32;
    let known = |key: &&&str| if **key == "Since" { since > 0 } else { report.cats.iter().any(|cat| cat.name == **key) };

    let legend = ORDER
        .iter()
        .filter(known)
        .map(|key| {
            let (name, color, counts) = key_style(key);
            let tip = if counts { format!("{} da janela", percent(share(key) * 100.0)) } else { String::new() };

            LegendRow { name, color: color.to_srgba_unmultiplied(), counts, tokens: tokens(value(key)), tip }
        })
        .collect();

    ReportSnapshot {
        percent: format!("{:.0}%", used as f64 / window as f64 * 100.0),
        used: format!("{} de {} tokens", tokens(used), tokens(report.window)),
        bar: ORDER.iter().filter(|key| key_style(key).2 && value(key) > 0).map(|key| (share(key), key_style(key).1.to_srgba_unmultiplied())).collect(),
        legend,
        until: tokens(report.until_compact().saturating_sub(since)),
        autocompact: autocompact.to_string(),
        choices: COMPACT_CHOICES,
        tools: report.tools.iter().map(|tool| (format!("{} · {}", tool.server, tool.name), tokens(tool.tokens))).collect(),
        memory: report.memory_words(),
        memory_tokens: tokens(report.memory.iter().map(|(_, count)| count).sum()),
        memory_tip: report.memory_tip(),
    }
}

impl DrawingApp {
    /// The panel of the chat open, while it is open (`prefs.chat_panel`, a Claude Code chat).
    pub(super) fn context_snapshot(&self) -> Option<PanelSnapshot> {
        let id = self.chat_agent.as_ref().filter(|id| self.prefs.chat_panel && self.context_wanted(id))?;
        let info = self.chat_ctx.info.get(id);
        let sorted = info.map(Info::ranked).unwrap_or_default();
        let tools = info.map_or(&[][..], Info::tools);

        let connectors = sorted
            .iter()
            .map(|server| {
                let ink = initial_ink(&server.name);

                ConnectorSnapshot {
                    name: server.name.clone(),
                    letter: initial(&server.name),
                    ink: [ink.r(), ink.g(), ink.b()],
                    note: connector_note(server, tools),
                    status: match server.status {
                        Status::Connected => "connected",
                        Status::Failed => "failed",
                        Status::NeedsAuth => "needs_auth",
                        Status::Pending => "pending",
                    },
                    tools: server_tools(tools, &server.name).into_iter().map(String::from).collect(),
                }
            })
            .collect();

        Some(PanelSnapshot {
            chat: id.clone(),
            loading: info.is_some_and(|info| info.loading),
            note: summary_note(info).unwrap_or_default(),
            report: info.and_then(|info| info.report.as_ref().map(|report| report_snapshot(info, report, &self.prefs.chat_autocompact))),
            connected: if sorted.is_empty() { String::new() } else { connected_words(&sorted) },
            connectors,
            connectors_note: connectors_note(info),
            more: if sorted.len() > 6 { more_label(&sorted[5..]) } else { String::new() },
        })
    }
}
