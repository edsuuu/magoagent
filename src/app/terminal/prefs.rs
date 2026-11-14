//! Which shells the computer has, and the Shell select of Novo/Editar terminal (the defaults of every
//! terminal are in Ajustes › Terminal › Aparência).

use crate::prelude::*;
use std::sync::OnceLock;

/// The shells this computer has, as (name, program or path), the ones a terminal can pick.
pub(crate) fn detected_shells() -> &'static [(String, String)] {
    static SHELLS: OnceLock<Vec<(String, String)>> = OnceLock::new();
    SHELLS.get_or_init(|| {
        if cfg!(windows) {
            let on_path = |exe: &str| std::env::var_os("PATH").is_some_and(|path| std::env::split_paths(&path).any(|dir| dir.join(exe).is_file()));
            let git_bash = ["ProgramFiles", "ProgramFiles(x86)"]
                .iter()
                .filter_map(|var| std::env::var_os(var))
                .map(|dir| PathBuf::from(dir).join("Git").join("bin").join("bash.exe"))
                .find(|path| path.is_file());
            let mut shells = Vec::new();
            for (name, exe) in [("PowerShell 7", "pwsh.exe"), ("Windows PowerShell", "powershell.exe"), ("Prompt de Comando", "cmd.exe")] {
                if on_path(exe) {
                    shells.push((name.to_string(), exe.trim_end_matches(".exe").to_string()));
                }
            }
            if let Some(path) = git_bash {
                shells.push(("Git Bash".into(), path.to_string_lossy().into_owned()));
            }
            shells
        } else {
            let listed = fs::read_to_string("/etc/shells").unwrap_or_default();
            let mut shells: Vec<(String, String)> = Vec::new();
            for path in listed.lines().map(str::trim).filter(|l| l.starts_with('/') && !l.contains("nologin") && !l.ends_with("/false")) {
                let name = path.rsplit('/').next().unwrap_or(path).to_string();
                if std::path::Path::new(path).is_file() && !shells.iter().any(|(n, _)| *n == name) {
                    shells.push((name, path.to_string()));
                }
            }
            shells
        }
    })
}

/// What a shell is called: its name if it is one of the detected ones, else what was typed.
fn shell_name(program: &str) -> String {
    match program.trim() {
        "" => "Padrão do sistema".into(),
        program => detected_shells().iter().find(|(_, p)| p == program).map_or_else(|| program.to_string(), |(name, _)| name.clone()),
    }
}

pub(crate) fn shell_combo(ui: &mut egui::Ui, id: &str, shell: &mut String, inherited: &str, width: f32, c: &UiColors) {
    let now = if shell.trim().is_empty() { format!("Igual ao padrão ({})", shell_name(inherited)) } else { shell_name(shell) };
    dialog_combo(ui, id, now, width, c, |ui| {
        ui.selectable_value(shell, String::new(), format!("Igual ao padrão ({})", shell_name(inherited)));
        for (name, program) in detected_shells() {
            ui.selectable_value(shell, program.clone(), name);
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_computers_shells_are_named_and_a_typed_one_keeps_its_name() {
        assert_eq!(shell_name(""), "Padrão do sistema");
        assert_eq!(shell_name("my-shell"), "my-shell");
        if let Some((name, program)) = detected_shells().first() {
            assert_eq!(&shell_name(program), name);
        }
        assert!(!detected_shells().is_empty(), "a computer has a shell");
    }
}
