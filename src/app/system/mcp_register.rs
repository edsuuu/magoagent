//! Ajustes › Integrações › MCP em outros agentes: puts this exe's MCP (`<exe> mcp`) in the person's own Claude Code
//! (`claude mcp add --scope user`) or Codex (`codex mcp add`), or takes it out, where the app finds them
//! (system/wsl.rs: in WSL the exe goes by its `/mnt/c/…` path, `AgentCli::finish` translates it). Whether it is there
//! is asked once a run (`mcp get magoagent`, which tries only ours, not every server as `mcp list` does). The CLIs run
//! on a thread, never on the frame.

use crate::prelude::*;
use std::sync::{Mutex, OnceLock};

/// The agents it registers in, as `agent_cli` names them, and how the page calls them.
pub(crate) const MCP_AGENTS: [(&str, &str); 2] = [("claude", "Claude Code"), ("codex", "Codex")];

/// What the last `mcp list` said of an agent.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub(crate) enum McpStatus {
    #[default]
    Checking,
    Registered,
    NotRegistered,
    Missing,
}

/// What the page offers for an agent: register, take out, or nothing while it is asked or missing.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) enum McpAction {
    Register,
    Remove,
}

#[derive(Default)]
struct State {
    status: [McpStatus; 2],
    /// What is being done in an agent (looked at, registered, taken out), for the page's line: its button waits.
    busy: [Option<&'static str>; 2],
    /// Looked at in this run: the page asks again only after a registration or a removal.
    checked: [bool; 2],
    /// What a registration from the page said, for its toast.
    said: Vec<String>,
}

fn state() -> std::sync::MutexGuard<'static, State> {
    static STATE: OnceLock<Mutex<State>> = OnceLock::new();

    STATE.get_or_init(Default::default).lock().unwrap_or_else(|e| e.into_inner())
}

fn index(agent: &str) -> Option<usize> {
    MCP_AGENTS.iter().position(|(name, _)| *name == agent)
}

/// The CLI's arguments: first the removal of an older registration, then the registration.
pub(crate) fn mcp_register_args(agent: &str, exe: &str) -> [Vec<String>; 2] {
    let words = |list: &[&str]| list.iter().map(|word| word.to_string()).collect::<Vec<_>>();

    if agent == "codex" {
        [words(&["mcp", "remove", "magoagent"]), words(&["mcp", "add", "magoagent", "--", exe, "mcp"])]
    } else {
        [words(&["mcp", "remove", "--scope", "user", "magoagent"]), words(&["mcp", "add", "--scope", "user", "magoagent", "--", exe, "mcp"])]
    }
}

/// The CLI's arguments that take ours out.
pub(crate) fn mcp_remove_args(agent: &str) -> Vec<String> {
    mcp_register_args(agent, "")[0].clone()
}

/// The button an agent's line has: Registrar when ours is not there, Remover when it is, none while something is
/// being done or the agent is not on this computer.
pub(crate) fn mcp_action(status: McpStatus, busy: bool) -> Option<McpAction> {
    match status {
        _ if busy => None,
        McpStatus::Registered => Some(McpAction::Remove),
        McpStatus::NotRegistered => Some(McpAction::Register),
        McpStatus::Checking | McpStatus::Missing => None,
    }
}

pub(crate) fn mcp_status(agent: &str) -> McpStatus {
    index(agent).map_or(McpStatus::Missing, |at| state().status[at])
}

/// What is being done in `agent` now ("Verificando…", "Registrando…", "Removendo…").
pub(crate) fn mcp_doing(agent: &str) -> Option<&'static str> {
    index(agent).and_then(|at| state().busy[at])
}

/// What the registrations started from the page said since the last call.
pub(crate) fn mcp_said() -> Vec<String> {
    std::mem::take(&mut state().said)
}

/// The agent's CLI with `args`, where it runs (here or in WSL), from the temporary folder (no project's own MCPs).
fn run(cli: &AgentCli, args: &[String]) -> Result<String, String> {
    let mut command = cli.command();

    command.args(args).current_dir(std::env::temp_dir());
    let output = cli.finish(command).stdin(std::process::Stdio::null()).output().map_err(|error| error.to_string())?;
    let text = format!("{}{}", String::from_utf8_lossy(&output.stdout), String::from_utf8_lossy(&output.stderr));

    if output.status.success() { Ok(text) } else { Err(text.trim().to_string()) }
}

fn look(agent: &str) -> McpStatus {
    // never the person's real CLI in a test
    if cfg!(test) {
        return McpStatus::Missing;
    }

    match agent_cli(agent) {
        None => McpStatus::Missing,
        // `get` fails when there is no server by that name
        Some(cli) => match run(&cli, &["mcp".into(), "get".into(), "magoagent".into()]) {
            Ok(_) => McpStatus::Registered,
            Err(_) => McpStatus::NotRegistered,
        },
    }
}

/// Looks at the agents not looked at yet in this run, on a thread (the page opened); `ctx` draws the answer.
pub(crate) fn mcp_check(ctx: &Context) {
    for (at, (agent, _)) in MCP_AGENTS.iter().enumerate() {
        let mut held = state();

        if held.busy[at].is_some() || held.checked[at] {
            continue;
        }

        // a test never asks the real CLIs: the answer (none found) comes at once
        if cfg!(test) {
            (held.status[at], held.checked[at]) = (look(agent), true);

            continue;
        }

        (held.busy[at], held.status[at], held.checked[at]) = (Some("Verificando…"), McpStatus::Checking, true);
        drop(held);
        let ctx = ctx.clone();

        std::thread::spawn(move || {
            let status = look(agent);
            let mut held = state();

            (held.busy[at], held.status[at]) = (None, status);
            ctx.request_repaint();
        });
    }
}

/// Registers this exe's MCP in `agent` (claude or codex), the older one removed first: blocks, so off the frame.
pub(crate) fn mcp_register(agent: &str) -> Result<String, String> {
    let (at, name) = index(agent).map(|at| (at, MCP_AGENTS[at].1)).ok_or_else(|| format!("\"{agent}\" não é um agente: claude ou codex"))?;

    if cfg!(test) {
        return Err("nunca nos testes".into());
    }

    let cli = agent_cli(agent).ok_or_else(|| format!("{name} não encontrado neste computador."))?;

    state().busy[at] = Some("Registrando…");
    let [remove, add] = mcp_register_args(agent, &crate::app::chat::tools::mcp_exe(false));
    // the first registration has nothing to remove
    let _ = run(&cli, &remove);
    let result = run(&cli, &add);
    let status = look(agent);

    (state().busy[at], state().status[at]) = (None, status);
    let place = if cli.wsl().is_some() { " (no WSL)" } else { "" };

    match result {
        Ok(_) if status == McpStatus::Registered => Ok(format!("O MCP do MagoAgent está registrado no {name}{place}; vale nas próximas sessões.")),
        Ok(_) => Err(format!("O {name}{place} aceitou o registro, mas o `mcp list` dele não mostra o magoagent.")),
        Err(why) => Err(format!("O {name}{place} não registrou o MCP: {why}")),
    }
}

/// Takes this app's MCP out of `agent` (claude or codex): blocks, so off the frame.
pub(crate) fn mcp_remove(agent: &str) -> Result<String, String> {
    let (at, name) = index(agent).map(|at| (at, MCP_AGENTS[at].1)).ok_or_else(|| format!("\"{agent}\" não é um agente: claude ou codex"))?;

    if cfg!(test) {
        return Err("nunca nos testes".into());
    }

    let cli = agent_cli(agent).ok_or_else(|| format!("{name} não encontrado neste computador."))?;

    state().busy[at] = Some("Removendo…");
    let result = run(&cli, &mcp_remove_args(agent));
    let status = look(agent);

    (state().busy[at], state().status[at]) = (None, status);
    let place = if cli.wsl().is_some() { " (no WSL)" } else { "" };

    match result {
        _ if status == McpStatus::NotRegistered => Ok(format!("O MCP do MagoAgent saiu do {name}{place}; vale nas próximas sessões.")),
        Ok(_) => Err(format!("O {name}{place} aceitou a remoção, mas ainda mostra o magoagent.")),
        Err(why) => Err(format!("O {name}{place} não tirou o MCP: {why}")),
    }
}

/// The page's buttons: registers or takes out on a thread and leaves what it said for a toast.
pub(crate) fn mcp_soon(agent: &'static str, action: McpAction, ctx: &Context) {
    let Some(at) = index(agent) else { return };

    {
        let mut held = state();

        if held.busy[at].is_some() {
            return;
        }

        held.busy[at] = Some(if action == McpAction::Remove { "Removendo…" } else { "Registrando…" });
    }

    let ctx = ctx.clone();

    std::thread::spawn(move || {
        let done = if action == McpAction::Remove { mcp_remove(agent) } else { mcp_register(agent) };
        let mut held = state();

        held.busy[at] = None;
        held.said.push(done.unwrap_or_else(|why| why));
        ctx.request_repaint();
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_registration_removes_the_older_one_then_adds_this_exe() {
        let [remove, add] = mcp_register_args("claude", "C:/MagoAgent/magoagent.exe");

        assert_eq!(remove, ["mcp", "remove", "--scope", "user", "magoagent"]);
        assert_eq!(add, ["mcp", "add", "--scope", "user", "magoagent", "--", "C:/MagoAgent/magoagent.exe", "mcp"]);
        let [remove, add] = mcp_register_args("codex", "/opt/magoagent");

        assert_eq!(remove, ["mcp", "remove", "magoagent"]);
        assert_eq!(add, ["mcp", "add", "magoagent", "--", "/opt/magoagent", "mcp"]);
        assert_eq!(mcp_register("gemini").unwrap_err(), "\"gemini\" não é um agente: claude ou codex");
        assert_eq!(mcp_remove_args("claude"), ["mcp", "remove", "--scope", "user", "magoagent"]);
        assert_eq!(mcp_remove_args("codex"), ["mcp", "remove", "magoagent"]);
        assert_eq!(mcp_remove("gemini").unwrap_err(), "\"gemini\" não é um agente: claude ou codex");
    }

    #[test]
    fn registered_offers_remover_and_not_registered_offers_registrar() {
        assert_eq!(mcp_action(McpStatus::Registered, false), Some(McpAction::Remove));
        assert_eq!(mcp_action(McpStatus::NotRegistered, false), Some(McpAction::Register));
        // nothing while it is asked or something is being done, nor without the agent
        assert_eq!(mcp_action(McpStatus::Registered, true), None);
        assert_eq!(mcp_action(McpStatus::Checking, false), None);
        assert_eq!(mcp_action(McpStatus::Missing, false), None);
    }
}
