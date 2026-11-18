//! Ajustes › Agentes › MCP em outros agentes: puts this exe's MCP (`<exe> mcp`) in the person's own Claude Code
//! (`claude mcp add --scope user`) or Codex (`codex mcp add`), where the app finds them (system/wsl.rs: in WSL the exe
//! goes by its `/mnt/c/…` path, `AgentCli::finish` translates it). The CLIs run on a thread, never on the frame.

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

#[derive(Default)]
struct State {
    status: [McpStatus; 2],
    /// An agent being looked at or registered in: its button waits.
    busy: [bool; 2],
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

/// Whether `mcp list` lists ours: a line whose first word is `magoagent` (Codex's table) or `magoagent:` (Claude's).
pub(crate) fn mcp_listed(output: &str) -> bool {
    output.lines().any(|line| line.split_whitespace().next().is_some_and(|word| word.trim_end_matches(':') == "magoagent"))
}

pub(crate) fn mcp_status(agent: &str) -> McpStatus {
    index(agent).map_or(McpStatus::Missing, |at| state().status[at])
}

pub(crate) fn mcp_busy(agent: &str) -> bool {
    index(agent).is_some_and(|at| state().busy[at])
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
        Some(cli) => match run(&cli, &["mcp".into(), "list".into()]) {
            Ok(output) if mcp_listed(&output) => McpStatus::Registered,
            _ => McpStatus::NotRegistered,
        },
    }
}

/// Looks at both agents again on a thread (the page opened); `ctx` draws the answer.
pub(crate) fn mcp_check(ctx: &Context) {
    for (at, (agent, _)) in MCP_AGENTS.iter().enumerate() {
        let mut held = state();
        if held.busy[at] {
            continue;
        }
        (held.busy[at], held.status[at]) = (true, McpStatus::Checking);
        drop(held);
        let ctx = ctx.clone();
        std::thread::spawn(move || {
            let status = look(agent);
            let mut held = state();
            (held.busy[at], held.status[at]) = (false, status);
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
    state().busy[at] = true;
    let [remove, add] = mcp_register_args(agent, &crate::app::chat::tools::mcp_exe(false));
    // the first registration has nothing to remove
    let _ = run(&cli, &remove);
    let result = run(&cli, &add);
    let status = look(agent);
    (state().busy[at], state().status[at]) = (false, status);
    let place = if cli.wsl().is_some() { " (no WSL)" } else { "" };
    match result {
        Ok(_) if status == McpStatus::Registered => Ok(format!("O MCP do MagoAgent está registrado no {name}{place}; vale nas próximas sessões.")),
        Ok(_) => Err(format!("O {name}{place} aceitou o registro, mas o `mcp list` dele não mostra o magoagent.")),
        Err(why) => Err(format!("O {name}{place} não registrou o MCP: {why}")),
    }
}

/// The page's button: registers on a thread and leaves what it said for a toast.
pub(crate) fn mcp_register_soon(agent: &'static str, ctx: &Context) {
    if mcp_busy(agent) {
        return;
    }
    if let Some(at) = index(agent) {
        state().busy[at] = true;
    }
    let ctx = ctx.clone();
    std::thread::spawn(move || {
        let said = mcp_register(agent).unwrap_or_else(|why| why);
        let mut held = state();
        if let Some(at) = index(agent) {
            held.busy[at] = false;
        }
        held.said.push(said);
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
    }

    #[test]
    fn the_mcp_list_of_each_agent_says_whether_ours_is_there() {
        let claude = "Checking MCP server health…\n\nplugin:magoagent:x: npx y - ✔ Connected\nrea: node rea.mjs mcp - ✔ Connected\n";
        assert!(!mcp_listed(claude), "a plugin's server is not ours");
        assert!(mcp_listed(&format!("{claude}magoagent: C:/MagoAgent/magoagent.exe mcp - ✔ Connected\n")));
        let codex = "Name         Command      Args  Env  Cwd  Status   Auth\nrea          node.exe     rea.mjs mcp  -  -  enabled  Unsupported\n";
        assert!(!mcp_listed(codex));
        assert!(mcp_listed(&format!("{codex}magoagent    C:/MagoAgent/magoagent.exe  mcp  -  -  enabled  Unsupported\n")));
        assert!(!mcp_listed("magoagentx: other mcp"));
    }
}
