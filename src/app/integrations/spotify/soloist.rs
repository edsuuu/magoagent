//! Spotify's Soloist on Linux. It has no MPRIS, but started with its local API (`--ws`, as `-w 127.0.0.1:0`) it answers
//! `soloist ctl` on this computer, with no account and no key of ours: `now --json` prints its `playback_state`
//! (`from_soloist`), and play, pause, next and prev control it. Its data folder holds `ws.port` only while it runs with
//! that API: a closed Soloist costs a look at a file, not a process.

use super::{Command, Now, soloist_command, soloist_data, soloist_program};
use std::process::Stdio;

fn env(name: &str) -> Option<String> {
    std::env::var(name).ok()
}

/// Its program: on the PATH, or where its download goes (`~/.local/bin`).
pub(super) fn program() -> Option<std::path::PathBuf> {
    crate::editor::lsp::which("soloist").or_else(|| soloist_program(env).filter(|program| program.is_file()))
}

/// What `soloist ctl <args>` printed, when it ran and answered; None without its local API running.
fn ctl(args: &[&str]) -> Option<String> {
    let data = soloist_data(env)?;
    if !data.join("ws.port").is_file() {
        return None;
    }
    let output = crate::editor::command(program()?).arg("ctl").args(args).arg("-D").arg(&data).stdin(Stdio::null()).stderr(Stdio::null()).output().ok()?;
    output.status.success().then(|| String::from_utf8_lossy(&output.stdout).to_string())
}

pub(super) fn read() -> Option<Now> {
    super::from_soloist(&serde_json::from_str(&ctl(&["now", "--json"])?).ok()?)
}

/// `playing`: whether it played when last read (play and pause are two commands).
pub(super) fn control(command: Command, playing: bool) -> bool {
    ctl(&[soloist_command(command, playing)]).is_some()
}
