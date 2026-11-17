//! Ajustes › Geral › "Iniciar com o sistema" (asked: "o MagoAgent iniciar com o sistema, Linux, Windows e Mac"): the
//! app opens when the person logs in. Windows: the account's Run key; Linux: an XDG autostart entry
//! (`~/.config/autostart`); macOS: a LaunchAgent (`~/Library/LaunchAgents`).

use crate::prelude::*;

use std::path::Path;

/// The LaunchAgent's label, the bundle's id (`build-macos-app.sh`).
#[cfg_attr(windows, allow(dead_code))] // Windows uses the Run key; the tests still check the others
const LABEL: &str = "com.edsuuu.magoagent";

/// The system's name, for the setting's words.
pub(crate) fn system_name() -> &'static str {
    if cfg!(windows) {
        "Windows"
    } else if cfg!(target_os = "macos") {
        "macOS"
    } else {
        "Linux"
    }
}

/// The file that opens `exe` at login on Linux or on the Mac, and what it says: `home` is the person's, `config` the
/// XDG_CONFIG_HOME when set.
#[cfg_attr(windows, allow(dead_code))]
pub(crate) fn login_entry(mac: bool, home: &Path, config: Option<&Path>, exe: &Path) -> (PathBuf, String) {
    let exe = exe.display().to_string();
    if mac {
        let exe = exe.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;");
        let contents = format!(
            "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<!DOCTYPE plist PUBLIC \"-//Apple//DTD PLIST 1.0//EN\" \"http://www.apple.com/DTDs/PropertyList-1.0.dtd\">\n<plist version=\"1.0\">\n<dict>\n  <key>Label</key><string>{LABEL}</string>\n  <key>ProgramArguments</key><array><string>{exe}</string></array>\n  <key>RunAtLoad</key><true/>\n</dict>\n</plist>\n"
        );
        return (home.join("Library/LaunchAgents").join(format!("{LABEL}.plist")), contents);
    }
    // the Exec key's quoting (Desktop Entry spec): inside the quotes, \ " ` $ are escaped
    let mut quoted = String::new();
    for letter in exe.chars() {
        if matches!(letter, '\\' | '"' | '`' | '$') {
            quoted.push('\\');
        }
        quoted.push(letter);
    }
    let contents = format!("[Desktop Entry]\nType=Application\nName=MagoAgent\nExec=\"{quoted}\"\nX-GNOME-Autostart-enabled=true\n");
    let config = config.map(Path::to_path_buf).unwrap_or_else(|| home.join(".config"));
    (config.join("autostart").join("magoagent.desktop"), contents)
}

#[cfg(windows)]
fn write_entry(enabled: bool) -> Result<(), String> {
    let mut command = crate::editor::command("reg.exe");
    if enabled {
        let executable = std::env::current_exe().map_err(|error| error.to_string())?;
        command.args(["add", "HKCU\\Software\\Microsoft\\Windows\\CurrentVersion\\Run", "/v", "MagoAgent", "/t", "REG_SZ", "/d"]);
        command.arg(format!("\"{}\"", executable.display()));
        command.arg("/f");
    } else {
        command.args(["delete", "HKCU\\Software\\Microsoft\\Windows\\CurrentVersion\\Run", "/v", "MagoAgent", "/f"]);
    }
    let output = command.output().map_err(|error| format!("Não consegui atualizar o início com o Windows: {error}"))?;
    if output.status.success() || (!enabled && output.status.code() == Some(1)) {
        Ok(())
    } else {
        Err("Não consegui atualizar o início com o Windows.".into())
    }
}

#[cfg(not(windows))]
fn write_entry(enabled: bool) -> Result<(), String> {
    let failed = |error: std::io::Error| format!("Não consegui atualizar o início com o {}: {error}", system_name());
    let home = std::env::var_os("HOME").filter(|home| !home.is_empty()).ok_or("Sem a pasta pessoal (HOME).")?;
    let config = std::env::var_os("XDG_CONFIG_HOME").filter(|config| !config.is_empty()).map(PathBuf::from);
    let exe = std::env::current_exe().map_err(failed)?;
    let (path, contents) = login_entry(cfg!(target_os = "macos"), Path::new(&home), config.as_deref(), &exe);
    if enabled {
        fs::create_dir_all(path.parent().unwrap_or(Path::new("."))).map_err(failed)?;
        fs::write(&path, contents).map_err(failed)
    } else {
        match fs::remove_file(&path) {
            Err(error) if error.kind() != std::io::ErrorKind::NotFound => Err(failed(error)),
            _ => Ok(()),
        }
    }
}

/// Opens the app at login, or stops it.
pub(crate) fn set_run_at_login(enabled: bool) -> Result<(), String> {
    // shortcut: the tests never write the person's login entries; `login_entry` is what they check
    if cfg!(test) {
        return Ok(());
    }
    write_entry(enabled)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn linux_opens_the_app_from_autostart_with_its_path_quoted() {
        let (path, contents) = login_entry(false, Path::new("/home/ana"), None, Path::new("/opt/Mago $1/magoagent"));
        assert_eq!(path, Path::new("/home/ana/.config/autostart/magoagent.desktop"));
        assert!(contents.contains("Exec=\"/opt/Mago \\$1/magoagent\"\n"), "{contents}");
        let (path, _) = login_entry(false, Path::new("/home/ana"), Some(Path::new("/cfg")), Path::new("/x"));
        assert_eq!(path, Path::new("/cfg/autostart/magoagent.desktop"), "XDG_CONFIG_HOME wins");
    }

    #[test]
    fn the_mac_opens_the_app_from_a_launch_agent() {
        let (path, contents) = login_entry(true, Path::new("/Users/ana"), None, Path::new("/Applications/A&B.app/Contents/MacOS/magoagent"));
        assert_eq!(path, Path::new("/Users/ana/Library/LaunchAgents/com.edsuuu.magoagent.plist"));
        assert!(contents.contains("<string>/Applications/A&amp;B.app/Contents/MacOS/magoagent</string>"), "{contents}");
        assert!(contents.contains("<key>RunAtLoad</key><true/>"));
    }
}
