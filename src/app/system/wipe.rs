//! Ajustes › Dados › Limpar todos os dados. The app stops its agents and terminals and closes; `magoagent limpar`, a
//! copy of itself, waits for it to be gone (so nothing the closing app writes is left), takes the app's secrets out of
//! the system keychain, deletes its folders and opens it again, as new. The development build only cleans its own
//! folder: the keychain and `~/.magoagent` are the installed app's too.

use crate::app::browser::vault::{read_vault, remove_secret};
use crate::file::{data_dir, development, legacy_data_dir, projects_dir};
use crate::prelude::*;
use std::path::{Path, PathBuf};
use std::time::Duration;

/// The keychain items that are not the vault's: the preferences' keys and the integrations' tokens.
const SECRETS: [&str; 5] = [
    "prefs-openai-key",
    "prefs-voice-studio-key",
    crate::app::integrations::github::TOKEN_ID,
    crate::app::integrations::spotify::account::TOKEN_ID,
    crate::app::integrations::whatsapp::CREDENTIAL_ID,
];

/// What a cleaning deletes.
#[derive(Clone, Debug, Default, PartialEq)]
pub(crate) struct Plan {
    /// The folders, each one the app's own (`owned`).
    pub(crate) folders: Vec<PathBuf>,
    /// The keychain too (the vault's passwords and the tokens).
    pub(crate) keychain: bool,
}

/// A folder the cleaning may delete: an absolute path at least two folders deep whose name is the app's
/// (`MagoAgent`, `magoagent-dev`, `.magoagent`) or its install folder's `data`. Anything else is never deleted,
/// whatever a broken setting or argument says.
pub(crate) fn owned(folder: &Path) -> bool {
    let named = |path: &Path| path.file_name().is_some_and(|name| name.to_string_lossy().to_lowercase().contains("magoagent"));
    let deep = folder.components().filter(|part| matches!(part, std::path::Component::Normal(_))).count() >= 2;
    folder.is_absolute() && deep && (named(folder) || (folder.file_name().is_some_and(|name| name == "data") && folder.parent().is_some_and(named)))
}

/// What this app cleans: its data folders (the one in use and the older place, when they are not the same), and,
/// installed, `~/.magoagent` and the keychain.
pub(crate) fn plan() -> Plan {
    let mut folders: Vec<PathBuf> = [data_dir(), legacy_data_dir()].into_iter().flatten().collect();
    let installed = !development() && !cfg!(test);
    if installed {
        folders.extend(projects_dir());
    }
    folders.dedup();
    folders.retain(|folder| owned(folder));
    Plan { folders, keychain: installed }
}

/// The keychain items to take out: the fixed ones and each password of the vaults in `folders`.
pub(crate) fn secret_ids(folders: &[PathBuf]) -> Vec<String> {
    let mut ids: Vec<String> = SECRETS.iter().map(|id| id.to_string()).collect();
    for folder in folders {
        ids.extend(read_vault(&folder.join("vault.json")).into_iter().map(|entry| entry.id));
    }
    ids.sort();
    ids.dedup();
    ids
}

/// Deletes each folder the app owns; the ones that could not be, with why. The install folder's `data` is emptied,
/// never deleted: the installer made it (on Windows with the person's right to write under Program Files), and
/// `data_dir` uses it only while it is there; gone, the app would quietly start over somewhere else (%APPDATA%).
pub(crate) fn delete_folders(folders: &[PathBuf]) -> Vec<String> {
    let mut failed = Vec::new();
    for folder in folders.iter().filter(|folder| folder.exists()) {
        if !owned(folder) {
            failed.push(format!("{}: não é uma pasta do MagoAgent", folder.display()));
        } else if folder.file_name().is_some_and(|name| name == "data") {
            failed.extend(empty_folder(folder));
        } else if let Err(error) = fs::remove_dir_all(folder) {
            failed.push(format!("{}: {error}", folder.display()));
        }
    }
    failed
}

/// Deletes what is inside `folder`, which stays; what could not go, with why.
fn empty_folder(folder: &Path) -> Vec<String> {
    let entries = match fs::read_dir(folder) {
        Ok(entries) => entries,
        Err(error) => return vec![format!("{}: {error}", folder.display())],
    };
    let mut failed = Vec::new();
    for path in entries.flatten().map(|entry| entry.path()) {
        let gone = if path.is_dir() { fs::remove_dir_all(&path) } else { fs::remove_file(&path) };
        if let Err(error) = gone {
            failed.push(format!("{}: {error}", path.display()));
        }
    }
    failed
}

/// The arguments of `magoagent limpar`: the app's pid, `--cofre` for the keychain, the folders.
pub(crate) fn arguments(pid: u32, plan: &Plan) -> Vec<String> {
    let mut args = vec!["limpar".to_string(), pid.to_string()];
    if plan.keychain {
        args.push("--cofre".into());
    }
    args.extend(plan.folders.iter().map(|folder| folder.display().to_string()));
    args
}

/// `magoagent limpar <pid> [--cofre] <pasta>…`: waits up to a minute for the app to close, cleans, opens the app again.
/// Its exit code: 0 cleaned, 1 something stayed (said on stderr), 2 wrong arguments or the app never closed.
pub(crate) fn cli(args: &[String]) -> i32 {
    let Some((pid, rest)) = args.split_first() else { return 2 };
    let Ok(pid) = pid.parse::<u32>() else { return 2 };
    let keychain = rest.first().is_some_and(|word| word == "--cofre");
    let folders: Vec<PathBuf> = rest.iter().skip(usize::from(keychain)).map(PathBuf::from).collect();
    let start = std::time::Instant::now();
    while crate::transcript::process::alive(pid) {
        if start.elapsed() > Duration::from_secs(60) {
            eprintln!("O MagoAgent não fechou: nada foi apagado.");
            return 2;
        }
        std::thread::sleep(Duration::from_millis(200));
    }
    let mut failed = Vec::new();
    if keychain {
        // the vaults are read before their folders go
        for id in secret_ids(&folders) {
            if let Err(error) = remove_secret(&id) {
                failed.push(format!("cofre ({id}): {error}"));
            }
        }
    }
    failed.extend(delete_folders(&folders));
    for line in &failed {
        eprintln!("{line}");
    }
    if let Ok(program) = std::env::current_exe() {
        let _ = crate::editor::command(program).stdin(std::process::Stdio::null()).spawn();
    }
    i32::from(!failed.is_empty())
}

impl DrawingApp {
    /// Apagar tudo, confirmed: the helper starts, the app stops everything and closes without saving.
    pub(crate) fn wipe_all_data(&mut self) -> Result<(), String> {
        let plan = plan();
        if plan.folders.is_empty() {
            return Err("não achei as pastas do MagoAgent".into());
        }
        let program = std::env::current_exe().map_err(|error| error.to_string())?;
        // the copy waits outside every folder it deletes
        let mut helper = crate::editor::command(program);
        helper.args(arguments(std::process::id(), &plan)).current_dir(std::env::temp_dir());
        helper.stdin(std::process::Stdio::null()).stdout(std::process::Stdio::null()).stderr(std::process::Stdio::null());
        // a group of its own: it outlives the app
        #[cfg(unix)]
        std::os::unix::process::CommandExt::process_group(&mut helper, 0);
        if !cfg!(test) {
            helper.spawn().map_err(|error| format!("não foi possível começar a limpeza: {error}"))?;
        }
        self.stop_everything();
        self.quitting = true;
        self.viewport_commands.push(egui::ViewportCommand::Close);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_the_apps_own_folders_are_ever_deleted() {
        let home = if cfg!(windows) { PathBuf::from(r"C:\Users\ana") } else { PathBuf::from("/home/ana") };
        for folder in
            [home.join(".magoagent"), home.join(".local/share/magoagent-dev"), home.join("AppData/Roaming/MagoAgent"), home.join("Apps/MagoAgent/data")]
        {
            assert!(owned(&folder), "{}", folder.display());
        }
        for folder in
            [home.clone(), home.join("projetos"), home.join("data"), PathBuf::from("magoagent"), home.join(".magoagent").parent().unwrap().to_path_buf()]
        {
            assert!(!owned(&folder), "{}", folder.display());
        }
    }

    #[test]
    fn the_helper_is_told_the_pid_the_keychain_and_the_folders() {
        let plan = Plan { folders: vec![PathBuf::from("/home/ana/.local/share/magoagent")], keychain: true };
        assert_eq!(arguments(42, &plan), ["limpar", "42", "--cofre", "/home/ana/.local/share/magoagent"]);
        let dev = Plan { keychain: false, ..plan };
        assert_eq!(arguments(42, &dev), ["limpar", "42", "/home/ana/.local/share/magoagent"]);
        assert_eq!(cli(&["nao-e-pid".into()]), 2);
    }

    #[test]
    fn a_cleaning_deletes_the_folders_and_names_the_vaults_passwords() {
        let root = std::env::temp_dir().join(format!("mago-limpar-{}", uuid::Uuid::new_v4().simple()));
        let folder = root.join("magoagent-teste");
        fs::create_dir_all(folder.join("chats")).unwrap();
        fs::write(folder.join("chats.json"), "[]").unwrap();
        let entry = crate::app::browser::vault::VaultEntry { site: "github.com".into(), user: "ana".into(), ..crate::app::browser::vault::VaultEntry::new() };
        crate::app::browser::vault::write_vault(&folder.join("vault.json"), std::slice::from_ref(&entry)).unwrap();
        let ids = secret_ids(std::slice::from_ref(&folder));
        assert!(ids.contains(&entry.id) && ids.iter().any(|id| id == "integration-github-token"), "{ids:?}");
        // the installer's `data`: emptied, it stays
        let installed = root.join("MagoAgent").join("data");
        fs::create_dir_all(installed.join("chats")).unwrap();
        fs::write(installed.join("preferences.json"), "{}").unwrap();
        assert!(delete_folders(std::slice::from_ref(&installed)).is_empty());
        assert!(installed.is_dir() && fs::read_dir(&installed).unwrap().next().is_none(), "the install's data stays, empty");
        let stranger = root.join("projetos");
        fs::create_dir_all(&stranger).unwrap();
        let failed = delete_folders(&[folder.clone(), stranger.clone()]);
        assert!(!folder.exists(), "the app's folder goes");
        assert!(stranger.exists() && failed.len() == 1, "a folder not the app's stays: {failed:?}");
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn the_test_build_plans_only_its_data_folder_and_never_the_keychain() {
        let plan = plan();
        assert!(!plan.keychain);
        assert!(plan.folders.iter().all(|folder| owned(folder)));
    }
}
