//! What is already done of Soloist's setup, only looked at: the program and its `--version`, whether the key's file is
//! there and who may read it (its contents are never read), the user's unit and what systemd says of it, whether it
//! answers `ctl now` logged in, and the tunnels (a VPN) that may hide this computer from the phone.

use super::{EXPIRED, Runner, SERVICE};
use crate::prelude::*;
use std::path::Path;

/// What `--version` says of a program.
#[derive(Clone, Debug, Default, PartialEq)]
pub(crate) enum Program {
    #[default]
    Missing,
    /// It answered: its first line ("soloist 1.3.10.74 build …").
    Ready(String),
    /// It ended with code 10: its 90 days passed, a new one is downloaded.
    Expired,
    /// It did not run: why.
    Broken(String),
}

/// The key's file, by its existence and its mode alone.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub(crate) enum KeyFile {
    #[default]
    Missing,
    /// Only its owner reads it (600, or tighter).
    Private,
    /// Others may read it: its mode.
    Open(u32),
}

/// Whether Soloist entered the person's account (someone chose it in Spotify Connect).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub(crate) enum Login {
    /// It does not answer (closed, or opened without its local API).
    #[default]
    Off,
    /// It answers, but no one chose it yet.
    Waiting,
    In,
}

/// One look at the whole setup.
#[derive(Clone, Debug, Default, PartialEq)]
pub(crate) struct Status {
    /// The computer is not x86_64: its architecture (Soloist is built for x86_64 only).
    pub(crate) foreign: Option<String>,
    pub(crate) program: Program,
    pub(crate) key: KeyFile,
    /// `~/.config/systemd/user/soloist.service` is there.
    pub(crate) unit: bool,
    /// `systemctl --user is-enabled` said enabled.
    pub(crate) enabled: bool,
    /// What `systemctl --user is-active` said: "active", "inactive", "failed", "activating"…; empty when it did not.
    pub(crate) active: String,
    pub(crate) login: Login,
    /// The tunnels that are up: "wg0", "tailscale0".
    pub(crate) vpn: Vec<String>,
}

impl Status {
    pub(crate) fn running(&self) -> bool {
        self.active == "active"
    }
}

/// Whether Soloist exists for this computer (`arch`: `std::env::consts::ARCH`).
pub(crate) fn arch_ok(arch: &str) -> Result<(), String> {
    match arch {
        "x86_64" => Ok(()),
        other => Err(format!("O Soloist só existe para computadores x86_64 (este é {other}).")),
    }
}

/// What `program --version` says (code 10: expired).
pub(crate) fn version_of(program: &Path, runner: &dyn Runner) -> Program {
    if !program.is_file() {
        return Program::Missing;
    }

    match runner.run(program, &["--version"]) {
        Ok(ran) if ran.code == Some(0) => Program::Ready(ran.output.lines().next().unwrap_or_default().trim().to_string()),
        Ok(ran) if ran.code == Some(EXPIRED) => Program::Expired,
        Ok(ran) => Program::Broken(ran.code.map_or("fechou sem código".into(), |code| format!("código {code}"))),
        Err(why) => Program::Broken(why),
    }
}

/// The key's file by its metadata alone: it is never opened.
pub(crate) fn key_file(path: &Path) -> KeyFile {
    let Ok(meta) = fs::metadata(path) else { return KeyFile::Missing };

    match super::install::mode_of(&meta) {
        Some(mode) if mode & 0o077 != 0 => KeyFile::Open(mode),
        _ => KeyFile::Private,
    }
}

/// Whether the Soloist of `data` answers logged in: `ctl now` answers without "not logged in" (with `ctl status`'s
/// "logged in: yes" when `now` had nothing to say, as with nothing loaded).
pub(crate) fn login(program: &Path, data: &Path, runner: &dyn Runner) -> Login {
    let Some(data) = data.to_str() else { return Login::Off };
    let said = |what: &str| runner.run(program, &["ctl", what, "-D", data]).ok();
    let now = said("now");
    let refused = |text: &str| text.to_lowercase().contains("not logged in");

    match &now {
        Some(ran) if refused(&ran.output) => return Login::Waiting,
        Some(ran) if ran.code == Some(0) => return Login::In,
        _ => {}
    }

    let status = said("status").map(|ran| ran.output.to_lowercase()).unwrap_or_default();

    if status.contains("logged in: yes") {
        Login::In
    } else if status.contains("logged in: no") {
        Login::Waiting
    } else {
        Login::Off
    }
}

/// The tunnels that are up in `net` (`/sys/class/net`): a VPN may keep the phone from finding this computer.
pub(crate) fn tunnels(net: &Path) -> Vec<String> {
    const KINDS: [&str; 7] = ["tun", "wg", "tap", "ppp", "tailscale", "proton", "nordlynx"];

    let up = |name: &str| {
        let flags = fs::read_to_string(net.join(name).join("flags")).unwrap_or_default();

        u32::from_str_radix(flags.trim().trim_start_matches("0x"), 16).is_ok_and(|flags| flags & 1 == 1)
    };

    let mut found: Vec<String> = fs::read_dir(net)
        .into_iter()
        .flatten()
        .flatten()
        .map(|entry| entry.file_name().to_string_lossy().to_string())
        .filter(|name| KINDS.iter().any(|kind| name.starts_with(kind)) && up(name))
        .collect();

    found.sort();
    found
}

/// The whole setup, looked at: `arch` this computer's, `net` where the network's interfaces are listed.
pub(crate) fn detect(paths: &super::Paths, runner: &dyn Runner, arch: &str, net: &Path) -> Status {
    let program = version_of(&paths.program, runner);

    let systemctl = |what: &str| {
        let ran = runner.run(Path::new("systemctl"), &["--user", what, SERVICE]).ok();

        ran.map(|ran| ran.output.lines().next().unwrap_or_default().trim().to_string()).unwrap_or_default()
    };

    let unit = fs::symlink_metadata(&paths.unit).is_ok();
    let login = if matches!(program, Program::Ready(_)) { login(&paths.program, &paths.data, runner) } else { Login::Off };

    Status {
        foreign: arch_ok(arch).err().map(|_| arch.to_string()),
        program,
        key: key_file(&paths.env_file),
        unit,
        enabled: unit && systemctl("is-enabled").starts_with("enabled"),
        active: if unit { systemctl("is-active") } else { String::new() },
        login,
        vpn: tunnels(net),
    }
}

/// The computer's short name ("edsu-linux" of "edsu-linux.casa"), from `hostname` as the system has it.
pub(crate) fn short_host(hostname: &str) -> String {
    hostname.trim().split('.').next().unwrap_or_default().chars().filter(|letter| !letter.is_control()).collect()
}

/// The device's name until the person gives another: "Soloist " and the computer's short name.
pub(crate) fn default_name(hostname: &str) -> String {
    match short_host(hostname) {
        short if short.is_empty() => "Soloist".into(),
        short => format!("Soloist {short}"),
    }
}

/// This computer's name, as the kernel has it.
pub(crate) fn this_host() -> String {
    ["/proc/sys/kernel/hostname", "/etc/hostname"].iter().find_map(|path| fs::read_to_string(path).ok()).unwrap_or_default()
}
