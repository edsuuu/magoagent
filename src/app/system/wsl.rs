//! The agents of a WSL distro, for a Windows whose Claude Code and Codex live only inside WSL: the app finds them
//! there (a probe in the background, never on the frame), runs them by `wsl.exe` with the folders and paths translated
//! (`C:\x` is `/mnt/c/x`, `\\wsl.localhost\<distro>\home\x` is `/home/x`), and they reach the app back through
//! interop: `magoagent` in WSL is a script that runs this Windows exe, the bridge's variables cross by `WSLENV`, and
//! the hooks and the MCP call the exe by its `/mnt/c/…` path. Ajustes › Agentes › WSL (`Prefs.wsl`) picks the distro.

use crate::prelude::*;
use std::process::Command;
use std::sync::{Mutex, OnceLock};

/// `Prefs.wsl`: "" looks in the default distro when neither agent is on Windows, `WSL_OFF` never looks, and a
/// distro's name runs the agents there even when Windows has them.
pub(crate) const WSL_OFF: &str = "off";

/// What the probe found in a distro: its name, home, Claude Code's folder and login PATH (Linux paths), and which
/// agents it has.
#[derive(Clone, Debug, Default, PartialEq)]
pub(crate) struct WslDistro {
    pub(crate) name: String,
    pub(crate) home: String,
    pub(crate) claude_dir: String,
    pub(crate) path: String,
    pub(crate) agents: Vec<String>,
}

/// How an agent's CLI runs: here, by its path, or inside a WSL distro.
#[derive(Clone, Debug, PartialEq)]
pub(crate) enum AgentCli {
    Here(PathBuf),
    Wsl(WslDistro, String),
}

/// The bridge's variables a WSL agent needs, and back to the Windows exe it runs (the scene's path translated).
const CROSSING: [&str; 6] =
    ["MAGOAGENT_BRIDGE", "MAGOAGENT_TOKEN", "MAGOAGENT_TERMINAL_ID", "MAGOAGENT_TERMINAL_NAME", "MAGOAGENT_CHAT_ID", "MAGOAGENT_SCENE/p"];

/// The probe's marker: what a profile prints before it is not the probe's.
const MARK: &str = "MAGOAGENT-WSL";

#[derive(Default)]
struct State {
    choice: String,
    /// When the last probe ended (None: never, or the choice changed) and whether one runs.
    at: Option<std::time::Instant>,
    running: bool,
    distro: Option<WslDistro>,
    distros: Vec<String>,
}

fn state() -> std::sync::MutexGuard<'static, State> {
    static STATE: OnceLock<Mutex<State>> = OnceLock::new();
    STATE.get_or_init(Default::default).lock().unwrap_or_else(|e| e.into_inner())
}

/// Ajustes' choice, as the app has it now: a new one probes again.
pub(crate) fn wsl_choose(choice: &str) {
    let mut state = state();
    if state.choice != choice {
        (state.choice, state.at, state.distro) = (choice.to_string(), None, None);
    }
}

/// The distros WSL has, as the last probe listed them.
pub(crate) fn wsl_distros() -> Vec<String> {
    probe_soon();
    state().distros.clone()
}

/// The distro the agents run in, as the last probe found it; None off Windows, with WSL off or not found. A probe older
/// than a minute is done again in the background (an agent installed meanwhile shows up).
pub(crate) fn wsl_distro() -> Option<WslDistro> {
    probe_soon();
    state().distro.clone()
}

fn probe_soon() {
    if cfg!(test) || !cfg!(windows) {
        return;
    }
    let choice = {
        let mut state = state();
        if state.choice == WSL_OFF || state.running || state.at.is_some_and(|at| at.elapsed().as_secs() < 60) {
            return;
        }
        // automatic, with an agent on Windows: no distro is started (looked at again in a minute)
        if state.choice.is_empty() && windows_has_agent() {
            (state.at, state.distro) = (Some(std::time::Instant::now()), None);
            return;
        }
        state.running = true;
        state.choice.clone()
    };
    std::thread::spawn(move || {
        let distros = list_distros();
        let distro = if distros.is_empty() { None } else { probe(&choice) };
        if distro.is_some() {
            write_shim();
        }
        let mut state = state();
        if state.choice == choice {
            (state.distro, state.distros, state.at) = (distro, distros, Some(std::time::Instant::now()));
        }
        state.running = false;
    });
}

/// `wsl.exe -l -q`: the names, in UTF-16 (the docker ones are Docker's, not the person's). Quick: it starts no distro.
pub(crate) fn list_distros() -> Vec<String> {
    if cfg!(test) || !cfg!(windows) {
        return Vec::new();
    }
    let Ok(output) = crate::editor::command("wsl.exe").args(["-l", "-q"]).output() else { return Vec::new() };
    if !output.status.success() {
        return Vec::new();
    }
    parse_wsl_distros(&output.stdout)
}

pub(crate) fn parse_wsl_distros(bytes: &[u8]) -> Vec<String> {
    let units: Vec<u16> = bytes.as_chunks::<2>().0.iter().map(|pair| u16::from_le_bytes(*pair)).collect();
    String::from_utf16_lossy(&units)
        .lines()
        .map(|line| line.trim_matches(|c: char| c.is_whitespace() || c == '\0').to_string())
        .filter(|name| !name.is_empty() && !name.starts_with("docker-desktop"))
        .collect()
}

/// Asks the distro by a login, interactive bash (where `~/.local/bin` and nvm's node join the PATH).
// ponytail: bash only (Alpine has none), and the default /mnt automount; read /etc/wsl.conf if one shows up
fn probe(choice: &str) -> Option<WslDistro> {
    let script = format!(
        "printf '%s\\n' {MARK} \"$WSL_DISTRO_NAME\" \"$HOME\" \"${{CLAUDE_CONFIG_DIR:-$HOME/.claude}}\" \"$PATH\"; \
         for a in claude codex; do printf '%s=%s\\n' \"$a\" \"$(command -v $a)\"; done"
    );
    let mut command = crate::editor::command("wsl.exe");
    if !choice.is_empty() {
        command.args(["-d", choice]);
    }
    let output = command.args(["--exec", "bash", "-lic", &script]).stdin(std::process::Stdio::null()).output().ok()?;
    parse_wsl_probe(&String::from_utf8_lossy(&output.stdout))
}

pub(crate) fn parse_wsl_probe(text: &str) -> Option<WslDistro> {
    let mut lines = text.lines().skip_while(|line| line.trim() != MARK).skip(1);
    let mut next = || lines.next().map(|line| line.trim_end_matches('\r').to_string());
    let (name, home, claude_dir, path) = (next()?, next()?, next()?, next()?);
    let mut agents = Vec::new();
    while let Some(line) = next() {
        // one under /mnt is Windows' own, through the Windows PATH
        if let Some((agent, found)) = line.split_once('=')
            && !found.is_empty()
            && !found.starts_with("/mnt/")
        {
            agents.push(agent.to_string());
        }
    }
    (!name.is_empty()).then_some(WslDistro { name, home, claude_dir, path, agents })
}

/// Where the agent `program` runs: with WSL off or not chosen, this computer's (`which`); with a distro chosen, that
/// distro's when it has it; with WSL automatic, the default distro's only when Windows has neither agent.
pub(crate) fn agent_cli(program: &str) -> Option<AgentCli> {
    let choice = state().choice.clone();
    let wsl = || wsl_distro().filter(|d| d.agents.iter().any(|a| a == program)).map(|d| AgentCli::Wsl(d, program.to_string()));
    if cfg!(windows) && !choice.is_empty() && choice != WSL_OFF {
        return wsl().or_else(|| crate::editor::lsp::which(program).map(AgentCli::Here));
    }
    if let Some(path) = crate::editor::lsp::which(program) {
        return Some(AgentCli::Here(path));
    }
    if cfg!(windows) && choice.is_empty() && !windows_has_agent() { wsl() } else { None }
}

fn windows_has_agent() -> bool {
    ["claude", "codex"].iter().any(|agent| crate::editor::lsp::which(agent).is_some())
}

impl AgentCli {
    pub(crate) fn wsl(&self) -> Option<&WslDistro> {
        match self {
            AgentCli::Wsl(distro, _) => Some(distro),
            AgentCli::Here(_) => None,
        }
    }

    /// The command to add the agent's arguments to; `finish` makes it run where the agent is.
    pub(crate) fn command(&self) -> Command {
        match self {
            AgentCli::Here(path) => crate::editor::command(path),
            AgentCli::Wsl(_, program) => crate::editor::command(program),
        }
    }

    /// In WSL: `wsl.exe -d <distro> --cd <folder> --exec env PATH=<its PATH, magoagent first> <program> <args>`, the
    /// Windows paths among the arguments translated and the bridge's variables crossing by `WSLENV`. Here, as it is.
    pub(crate) fn finish(&self, command: Command) -> Command {
        let AgentCli::Wsl(distro, _) = self else { return command };
        let mut wsl = crate::editor::command("wsl.exe");
        wsl.args(wsl_args(distro, command.get_current_dir().map(|dir| dir.to_string_lossy().into_owned()).as_deref()));
        wsl.arg(command.get_program());
        wsl.args(command.get_args().map(|arg| wsl_arg(&arg.to_string_lossy())));
        for (key, value) in command.get_envs() {
            match value {
                Some(value) => wsl.env(key, value),
                None => wsl.env_remove(key),
            };
        }
        wsl.env("WSLENV", wslenv(std::env::var("WSLENV").ok().as_deref()));
        wsl
    }
}

/// A terminal's shell that is WSL (`wsl.exe -d Ubuntu`, or by its path).
pub(crate) fn is_wsl_shell(shell: &str) -> bool {
    let program = shell.split_whitespace().next().unwrap_or_default();
    let name = program.rsplit(['\\', '/']).next().unwrap_or_default().to_ascii_lowercase();
    matches!(name.as_str(), "wsl" | "wsl.exe")
}

/// The shell a terminal opens a distro with.
pub(crate) fn wsl_shell(distro: &str) -> String {
    format!("wsl.exe -d {distro}")
}

/// A command line typed into a WSL shell: the Windows paths in quotes (`--settings "C:\…"`) as WSL sees them.
pub(crate) fn wsl_quoted_paths(line: &str) -> String {
    line.split('"').enumerate().map(|(i, part)| if i % 2 == 1 { wsl_arg(part) } else { part.to_string() }).collect::<Vec<_>>().join("\"")
}

/// `wsl.exe`'s own arguments before the program's.
pub(crate) fn wsl_args(distro: &WslDistro, cwd: Option<&str>) -> Vec<String> {
    let mut args = vec!["-d".to_string(), distro.name.clone()];
    if let Some(cwd) = cwd {
        args.extend(["--cd".into(), linux_path(cwd)]);
    }
    let path = match shim_dir() {
        Some(dir) => format!("{}:{}", linux_path(&dir.to_string_lossy()), distro.path),
        None => distro.path.clone(),
    };
    args.extend(["--exec".into(), "env".into(), format!("PATH={path}")]);
    args
}

/// `WSLENV` with the bridge's variables added to what it had.
pub(crate) fn wslenv(had: Option<&str>) -> String {
    let had: Vec<&str> = had.unwrap_or_default().split(':').filter(|v| !v.is_empty()).collect();
    let name = |entry: &str| entry.split('/').next().unwrap_or_default().to_string();
    let mut all: Vec<String> = had.iter().map(|v| v.to_string()).collect();
    for entry in CROSSING {
        if !had.iter().any(|v| name(v) == name(entry)) {
            all.push(entry.into());
        }
    }
    all.join(":")
}

/// An argument that is a Windows path, as WSL sees it; anything else as it is.
pub(crate) fn wsl_arg(arg: &str) -> String {
    let bytes = arg.as_bytes();
    let drive = bytes.len() >= 3 && bytes[0].is_ascii_alphabetic() && bytes[1] == b':' && matches!(bytes[2], b'\\' | b'/');
    if drive || arg.starts_with(r"\\wsl") || arg.starts_with("//wsl") { linux_path(arg) } else { arg.to_string() }
}

/// `C:\x\y` is `/mnt/c/x/y`; `\\wsl.localhost\<distro>\home\x` (or `\\wsl$\…`) is `/home/x`; a Linux path stays.
pub(crate) fn linux_path(path: &str) -> String {
    let path = path.replace('\\', "/");
    let bytes = path.as_bytes();
    if bytes.len() >= 2 && bytes[0].is_ascii_alphabetic() && bytes[1] == b':' {
        let rest = path[2..].trim_start_matches('/');
        let drive = path[..1].to_ascii_lowercase();
        return if rest.is_empty() { format!("/mnt/{drive}") } else { format!("/mnt/{drive}/{rest}") };
    }
    for share in ["//wsl.localhost/", "//wsl$/"] {
        if let Some(rest) = path.strip_prefix(share) {
            return rest.find('/').map_or_else(|| "/".to_string(), |at| rest[at..].to_string());
        }
    }
    path
}

/// The other way: `/mnt/c/x` is `C:\x`, any other Linux path is under `\\wsl.localhost\<distro>`.
pub(crate) fn windows_path(distro: &str, path: &str) -> String {
    if let Some(rest) = path.strip_prefix("/mnt/")
        && let Some(drive) = rest.chars().next().filter(|c| c.is_ascii_alphabetic())
        && rest.get(1..2).is_none_or(|c| c == "/")
    {
        return format!("{}:\\{}", drive.to_ascii_uppercase(), rest.get(2..).unwrap_or_default().replace('/', "\\"));
    }
    if path.starts_with('/') {
        return format!(r"\\wsl.localhost\{distro}{}", path.replace('/', "\\"));
    }
    path.to_string()
}

/// Claude Code's folder in the distro, as Windows reaches it.
pub(crate) fn wsl_claude_dir(distro: &WslDistro) -> PathBuf {
    PathBuf::from(windows_path(&distro.name, &distro.claude_dir))
}

/// This exe as WSL runs it (by interop).
pub(crate) fn wsl_exe() -> Option<String> {
    Some(linux_path(&std::env::current_exe().ok()?.to_string_lossy()))
}

/// The folder of the `magoagent` script WSL runs (`write_shim`), in the app's data.
pub(crate) fn shim_dir() -> Option<PathBuf> {
    data_dir().map(|d| d.join("wsl"))
}

/// `magoagent` for WSL: a script that runs this exe, so `magoagent list|ask|…` work there as on Windows. An extensionless
/// file on Windows' PATH is nothing to PowerShell or cmd.
fn write_shim() {
    let (Some(dir), Some(exe)) = (shim_dir(), wsl_exe()) else { return };
    let _ = fs::create_dir_all(&dir);
    let _ = write_atomic(&dir.join("magoagent"), wsl_shim_text(&exe));
}

pub(crate) fn wsl_shim_text(exe: &str) -> String {
    format!("#!/bin/sh\nexec '{}' \"$@\"\n", exe.replace('\'', r"'\''"))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ubuntu() -> WslDistro {
        WslDistro {
            name: "Ubuntu".into(),
            home: "/home/eu".into(),
            claude_dir: "/home/eu/.claude".into(),
            path: "/home/eu/.local/bin:/usr/bin".into(),
            agents: vec!["claude".into()],
        }
    }

    #[test]
    fn windows_and_wsl_paths_translate_both_ways() {
        assert_eq!(linux_path(r"C:\Users\eu\proj"), "/mnt/c/Users/eu/proj");
        assert_eq!(linux_path("D:/dados"), "/mnt/d/dados");
        assert_eq!(linux_path(r"C:\"), "/mnt/c");
        assert_eq!(linux_path(r"\\wsl.localhost\Ubuntu\home\eu\proj"), "/home/eu/proj");
        assert_eq!(linux_path(r"\\wsl$\Ubuntu\home\eu"), "/home/eu");
        assert_eq!(linux_path("/home/eu"), "/home/eu");
        assert_eq!(windows_path("Ubuntu", "/mnt/c/Users/eu/proj"), r"C:\Users\eu\proj");
        assert_eq!(windows_path("Ubuntu", "/mnt/d"), r"D:\");
        assert_eq!(windows_path("Ubuntu", "/home/eu/.claude"), r"\\wsl.localhost\Ubuntu\home\eu\.claude");
        // /mnt/wsl is no drive
        assert_eq!(windows_path("Ubuntu", "/mnt/wsl/x"), r"\\wsl.localhost\Ubuntu\mnt\wsl\x");
        assert_eq!(windows_path("Ubuntu", "relativo"), "relativo");
        for path in [r"C:\Users\eu\proj", r"\\wsl.localhost\Ubuntu\home\eu"] {
            assert_eq!(windows_path("Ubuntu", &linux_path(path)), path);
        }
        // only whole Windows paths among the arguments
        assert_eq!(wsl_arg(r"C:\dados\hooks.json"), "/mnt/c/dados/hooks.json");
        assert_eq!(wsl_arg("--settings"), "--settings");
        assert_eq!(wsl_arg("a:b"), "a:b");
        assert_eq!(
            wsl_quoted_paths(r#"claude --settings "C:\d\hooks.json" --plugin-dir "C:\d\skills" -p "oi""#),
            r#"claude --settings "/mnt/c/d/hooks.json" --plugin-dir "/mnt/c/d/skills" -p "oi""#
        );
        assert!(is_wsl_shell(&wsl_shell("Ubuntu")) && is_wsl_shell(r"C:\Windows\System32\wsl.exe") && is_wsl_shell("wsl"));
        assert!(!is_wsl_shell("pwsh") && !is_wsl_shell("") && !is_wsl_shell("/usr/bin/wslview"));
    }

    #[test]
    fn the_probe_and_the_distro_list_are_read() {
        let text = "bash: no job control\nMAGOAGENT-WSL\nUbuntu\n/home/eu\n/home/eu/.claude\n/home/eu/.local/bin:/usr/bin\nclaude=/home/eu/.local/bin/claude\ncodex=/mnt/c/Users/eu/AppData/Roaming/npm/codex\n";
        assert_eq!(parse_wsl_probe(text), Some(ubuntu()), "Windows' codex seen through the PATH is not the distro's");
        assert_eq!(parse_wsl_probe("nada\n"), None);
        assert_eq!(parse_wsl_probe("MAGOAGENT-WSL\n\n/root\n/root/.claude\n/usr/bin\n"), None, "no name, no distro");
        let utf16: Vec<u8> = "Ubuntu-26.04\r\ndocker-desktop\r\nDebian\r\n\0".encode_utf16().flat_map(u16::to_le_bytes).collect();
        assert_eq!(parse_wsl_distros(&utf16), ["Ubuntu-26.04", "Debian"]);
    }

    #[test]
    fn a_wsl_agent_runs_by_wsl_exe_with_the_bridge_crossing() {
        let args = wsl_args(&ubuntu(), Some(r"C:\Users\eu\proj"));
        assert_eq!(args[..5], ["-d", "Ubuntu", "--cd", "/mnt/c/Users/eu/proj", "--exec"]);
        assert_eq!(args[5], "env");
        assert!(args[6].starts_with("PATH=") && args[6].ends_with(":/home/eu/.local/bin:/usr/bin"), "{}", args[6]);
        assert_eq!(wsl_args(&ubuntu(), None)[2], "--exec");
        assert_eq!(wslenv(None), "MAGOAGENT_BRIDGE:MAGOAGENT_TOKEN:MAGOAGENT_TERMINAL_ID:MAGOAGENT_TERMINAL_NAME:MAGOAGENT_CHAT_ID:MAGOAGENT_SCENE/p");
        assert!(wslenv(Some("USERPROFILE/p:MAGOAGENT_TOKEN/u")).starts_with("USERPROFILE/p:MAGOAGENT_TOKEN/u:MAGOAGENT_BRIDGE:MAGOAGENT_TERMINAL_ID"));
        // the command the app built, moved into the distro
        let cli = AgentCli::Wsl(ubuntu(), "claude".into());
        let mut command = cli.command();
        command.args(["-p", "--settings", r"C:\dados\hooks-claude-wsl.json"]).env("MAGOAGENT_TOKEN", "t").current_dir(r"C:\proj");
        let wsl = cli.finish(command);
        assert_eq!(wsl.get_program(), "wsl.exe");
        let args: Vec<String> = wsl.get_args().map(|a| a.to_string_lossy().into_owned()).collect();
        assert_eq!(args[..4], ["-d", "Ubuntu", "--cd", "/mnt/c/proj"]);
        assert_eq!(args[7..], ["claude", "-p", "--settings", "/mnt/c/dados/hooks-claude-wsl.json"]);
        let envs: Vec<_> = wsl.get_envs().map(|(k, v)| (k.to_string_lossy().into_owned(), v.map(|v| v.to_string_lossy().into_owned()))).collect();
        assert!(envs.contains(&("MAGOAGENT_TOKEN".into(), Some("t".into()))));
        assert!(envs.iter().any(|(k, v)| k == "WSLENV" && v.as_deref().is_some_and(|v| v.contains("MAGOAGENT_TOKEN"))));
        // an agent here runs as it was
        let here = AgentCli::Here(PathBuf::from("claude"));
        assert_eq!(here.finish(here.command()).get_program(), "claude");
        assert_eq!(wsl_shim_text("/mnt/c/Program Files/O'Mago/magoagent.exe"), "#!/bin/sh\nexec '/mnt/c/Program Files/O'\\''Mago/magoagent.exe' \"$@\"\n");
    }
}
