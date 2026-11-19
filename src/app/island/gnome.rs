//! GNOME on Wayland: where the pointer is anywhere on the screen, for the island's eyes. The app runs on XWayland,
//! whose X server only knows the pointer over its windows (`x11::cursor`); a small extension of GNOME Shell
//! (assets/gnome/) answers where it is over D-Bus. On by default: the app puts it in at the start
//! (`install_if_missing`) unless the switch in Ajustes › Ilha (or `set_setting gnome_cursor` for the MCP) took it out;
//! the shell loads a new extension on the next login.

use crate::prelude::*;
use gtk::gio::prelude::*;
use gtk::{gio, glib};
use std::path::{Path, PathBuf};
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, Instant};

pub(crate) const UUID: &str = "magoagent-cursor@edsuuu";
const FILES: [(&str, &str); 2] = [
    ("metadata.json", include_str!("../../../assets/gnome/magoagent-cursor@edsuuu/metadata.json")),
    ("extension.js", include_str!("../../../assets/gnome/magoagent-cursor@edsuuu/extension.js")),
];
const PATH: &str = "/io/github/edsuuu/MagoAgent/Cursor";
const INTERFACE: &str = "io.github.edsuuu.MagoAgent.Cursor";
/// How often the pointer is asked while someone wants it, how old an answer still counts, and the rest after the
/// shell did not answer (the extension is not loaded).
const EVERY: Duration = Duration::from_millis(33);
const FRESH: Duration = Duration::from_millis(250);
const RETRY: Duration = Duration::from_secs(3);
/// Nobody asked for this long: the thread stops until the next ask.
const IDLE: Duration = Duration::from_secs(1);

/// Whether this session is GNOME on Wayland, where the extension is the way to the pointer.
pub(crate) fn wanted() -> bool {
    static WANTED: OnceLock<bool> = OnceLock::new();

    *WANTED.get_or_init(|| {
        let wayland = std::env::var_os("WAYLAND_DISPLAY").is_some_and(|display| !display.is_empty());

        wayland && std::env::var("XDG_CURRENT_DESKTOP").is_ok_and(|desktop| is_gnome(&desktop))
    })
}

/// "ubuntu:GNOME", "GNOME-Classic:GNOME", "GNOME".
fn is_gnome(desktop: &str) -> bool {
    desktop.split(':').any(|name| name.eq_ignore_ascii_case("gnome") || name.to_ascii_lowercase().starts_with("gnome-"))
}

#[derive(Default)]
struct Poll {
    /// The pointer and when the shell told it.
    at: Option<(Pos2, Instant)>,
    /// When `cursor` was last called, and whether the thread that asks runs.
    asked: Option<Instant>,
    running: bool,
}

fn poll() -> &'static Mutex<Poll> {
    static POLL: OnceLock<Mutex<Poll>> = OnceLock::new();

    POLL.get_or_init(Default::default)
}

/// The pointer, in the screen's pixels, when the extension answered just now. Asking keeps a thread asking the shell
/// 30 times a second; nobody asking for a second, it stops.
// ponytail: the shell's logical pixels taken as the X server's (scale 1, or XWayland scaled by the shell), the
// extension could send the scale if a screen at 200% shows the eyes off
pub(crate) fn cursor() -> Option<Pos2> {
    if !wanted() || cfg!(test) {
        return None;
    }

    let mut poll = poll().lock().unwrap_or_else(|poisoned| poisoned.into_inner());

    poll.asked = Some(Instant::now());

    if !poll.running {
        poll.running = true;

        if std::thread::Builder::new().name("cursor-gnome".into()).spawn(ask_shell).is_err() {
            poll.running = false;
        }
    }

    poll.at.filter(|(_, when)| when.elapsed() < FRESH).map(|(at, _)| at)
}

/// Whether the extension answered in the last seconds (Ajustes shows it on).
pub(crate) fn answers() -> bool {
    cursor();
    poll().lock().unwrap_or_else(|poisoned| poisoned.into_inner()).at.is_some_and(|(_, when)| when.elapsed() < RETRY * 2)
}

fn ask_shell() {
    let stop = || poll().lock().unwrap_or_else(|poisoned| poisoned.into_inner()).running = false;
    let Ok(bus) = gio::bus_get_sync(gio::BusType::Session, None::<&gio::Cancellable>) else { return stop() };

    loop {
        {
            let mut state = poll().lock().unwrap_or_else(|poisoned| poisoned.into_inner());

            if state.asked.is_none_or(|asked| asked.elapsed() > IDLE) {
                state.running = false;

                return;
            }
        }

        let at = where_now(&bus, "org.gnome.Shell");

        if let Some(at) = at {
            poll().lock().unwrap_or_else(|poisoned| poisoned.into_inner()).at = Some((at, Instant::now()));
        }

        std::thread::sleep(if at.is_some() { EVERY } else { RETRY });
    }
}

/// The extension's answer: `Where` on the shell's bus `name`.
fn where_now(bus: &gio::DBusConnection, name: &str) -> Option<Pos2> {
    let answer = glib::VariantTy::new("(ii)").ok()?;

    let reply =
        bus.call_sync(Some(name), PATH, INTERFACE, "Where", None, Some(answer), gio::DBusCallFlags::NO_AUTO_START, 200, None::<&gio::Cancellable>).ok()?;

    let (x, y) = reply.get::<(i32, i32)>()?;

    Some(Pos2::new(x as f32, y as f32))
}

/// Where the shell looks for the person's extensions (the tests' own folder in a test).
fn folder() -> Option<PathBuf> {
    if cfg!(test) {
        return Some(std::env::temp_dir().join(format!("mago-gnome-extensions-{}", std::process::id())).join(UUID));
    }

    let data = std::env::var_os("XDG_DATA_HOME")
        .filter(|data| !data.is_empty())
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").filter(|home| !home.is_empty()).map(|home| PathBuf::from(home).join(".local/share")))?;

    Some(data.join("gnome-shell/extensions").join(UUID))
}

/// The extension's files written in `folder`.
fn write_files(folder: &Path) -> std::io::Result<()> {
    std::fs::create_dir_all(folder)?;

    for (name, text) in FILES {
        std::fs::write(folder.join(name), text)?;
    }

    Ok(())
}

/// Whether the extension's files are in `folder`, as this build has them.
fn written(folder: &Path) -> bool {
    FILES.iter().all(|(name, text)| std::fs::read_to_string(folder.join(name)).is_ok_and(|kept| kept == *text))
}

/// The shell's list of extensions on (`enabled-extensions`) with ours put in or taken out.
fn with_ours(list: &[String], on: bool) -> Vec<String> {
    let mut out: Vec<String> = list.iter().filter(|uuid| *uuid != UUID).cloned().collect();

    if on {
        out.push(UUID.into());
    }

    out
}

/// Ours on or off in the shell's settings (it reads them at login too), and through `gnome-extensions` for a shell
/// that knows it already.
fn switch(on: bool) -> Result<(), String> {
    if cfg!(test) {
        return Ok(());
    }

    let schema = gio::SettingsSchemaSource::default().and_then(|source| source.lookup("org.gnome.shell", true));

    if schema.is_none() {
        return Err("As configurações do GNOME Shell não estão neste sistema.".into());
    }

    let settings = gio::Settings::new("org.gnome.shell");

    for (key, put) in [("enabled-extensions", on), ("disabled-extensions", false)] {
        let list: Vec<String> = settings.strv(key).iter().map(|uuid| uuid.to_string()).collect();
        let list = with_ours(&list, put);

        settings.set_strv(key, list.iter().map(String::as_str).collect::<Vec<_>>()).map_err(|error| format!("Não deu para mudar {key}: {error}"))?;
    }

    gio::Settings::sync();
    let _ = std::process::Command::new("gnome-extensions").args([if on { "enable" } else { "disable" }, UUID]).output();

    Ok(())
}

/// What Ajustes says of it.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) enum Status {
    /// It answers: the eyes follow the pointer everywhere.
    On,
    /// Its files are there; the shell loads it at the next login.
    Installed,
    Missing,
}

pub(crate) fn status() -> Status {
    if answers() {
        Status::On
    } else if folder().is_some_and(|folder| written(&folder)) {
        Status::Installed
    } else {
        Status::Missing
    }
}

/// Puts the extension in the person's extensions and turns it on.
pub(crate) fn install() -> Result<String, String> {
    let folder = folder().ok_or("Sem a pasta pessoal (HOME).")?;

    write_files(&folder).map_err(|error| format!("Não deu para gravar a extensão em {}: {error}", folder.display()))?;
    switch(true)?;

    Ok(if answers() {
        "A extensão do GNOME está ativa: os olhos da ilha seguem o mouse na tela toda.".into()
    } else {
        "Extensão do GNOME instalada. O GNOME a carrega quando você sair e entrar de novo na sessão.".into()
    })
}

/// At the start, on GNOME on Wayland: puts it in when it is not there (the preference is on by default).
pub(crate) fn install_if_missing() {
    if wanted() && folder().is_some_and(|folder| !written(&folder)) {
        let _ = install();
    }
}

/// Takes it out: off in the shell's settings and its folder gone.
pub(crate) fn remove() -> Result<String, String> {
    switch(false)?;

    if let Some(folder) = folder().filter(|folder| folder.is_dir()) {
        std::fs::remove_dir_all(&folder).map_err(|error| format!("Não deu para apagar {}: {error}", folder.display()))?;
    }

    Ok("Extensão do GNOME removida: fora da ilha os olhos voltam a olhar em volta.".into())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn gnome_is_read_from_the_desktop_names() {
        assert!(is_gnome("ubuntu:GNOME") && is_gnome("GNOME") && is_gnome("GNOME-Classic:GNOME"));
        assert!(!is_gnome("KDE") && !is_gnome("XFCE") && !is_gnome(""));
    }

    #[test]
    fn the_extension_is_written_whole_and_put_on_the_list_once() {
        let folder = std::env::temp_dir().join(format!("mago-gnome-{}", std::process::id())).join(UUID);

        assert!(!written(&folder));
        write_files(&folder).unwrap();
        assert!(written(&folder), "both files, as the build has them");
        std::fs::write(folder.join("extension.js"), "antiga").unwrap();
        assert!(!written(&folder), "an older one is written again");
        let _ = std::fs::remove_dir_all(folder.parent().unwrap());
        let list = vec!["ubuntu-dock@ubuntu.com".to_string(), UUID.to_string()];

        assert_eq!(with_ours(&list, true), ["ubuntu-dock@ubuntu.com", UUID]);
        assert_eq!(with_ours(&list, false), ["ubuntu-dock@ubuntu.com"]);
        let metadata: serde_json::Value = serde_json::from_str(FILES[0].1).unwrap();

        assert_eq!(metadata["uuid"], UUID, "the folder's name is its uuid");
        assert!(FILES[1].1.contains(PATH) && FILES[1].1.contains(INTERFACE), "the app asks where it answers");
    }
}
