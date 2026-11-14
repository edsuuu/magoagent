//! Ajustes › Agentes › Hooks: `magoagent hook external` in the person's own `~/.claude/settings.json` (or
//! `CLAUDE_CONFIG_DIR`'s), so the island sees every Claude Code session on the machine, Claude Desktop's included.
//! The file is merged, never replaced: what was there stays, a dated backup is kept beside it, and removing takes out
//! only the entries that are ours (a command with `magoagent` and ` hook external`). Nothing here runs by itself: the
//! button asks for it.

use super::outside::WAIT_TIMEOUT;
use crate::prelude::*;

/// (event, matcher, timeout in s, waits for the person). PreToolUse has two: every tool (a pill's work) and the
/// AskUserQuestion, which waits; a permission request waits too.
const EVENTS: [(&str, Option<&str>, u64, bool); 11] = [
    ("SessionStart", None, 10, false),
    ("SessionEnd", None, 10, false),
    ("UserPromptSubmit", None, 10, false),
    ("PreToolUse", None, 10, false),
    ("PreToolUse", Some("AskUserQuestion"), WAIT_TIMEOUT, true),
    ("PostToolUse", None, 10, false),
    ("PostToolUseFailure", None, 10, false),
    ("Notification", None, 10, false),
    ("Stop", None, 10, false),
    ("StopFailure", None, 10, false),
    ("PermissionRequest", None, WAIT_TIMEOUT, false),
];

/// Claude Code's settings file: `CLAUDE_CONFIG_DIR` when the person moved it there, else `~/.claude`.
pub(crate) fn settings_path() -> Option<PathBuf> {
    let var = |name: &str| std::env::var_os(name).filter(|value| !value.is_empty()).map(PathBuf::from);
    let folder = var("CLAUDE_CONFIG_DIR").or_else(|| var(if cfg!(windows) { "USERPROFILE" } else { "HOME" }).map(|home| home.join(".claude")))?;
    Some(folder.join("settings.json"))
}

/// Claude Code runs a hook through the shell (Git Bash on Windows): the exe's path quoted, with forward slashes.
fn command(exe: &str, wait: bool) -> String {
    format!("\"{}\" hook external{}", exe.replace('\\', "/"), if wait { " wait" } else { "" })
}

fn ours(entry: &Value) -> bool {
    entry["hooks"].as_array().is_some_and(|hooks| {
        hooks.iter().any(|hook| hook["command"].as_str().is_some_and(|text| text.contains("magoagent") && text.contains(" hook external")))
    })
}

fn root_of(existing: &Value) -> Result<serde_json::Map<String, Value>, String> {
    match existing {
        Value::Object(root) => Ok(root.clone()),
        Value::Null => Ok(Default::default()),
        _ => Err("settings.json não é um objeto JSON: não mexi nele.".into()),
    }
}

/// `existing` less every entry that is ours, the rest as it was.
pub(crate) fn without_ours(existing: &Value) -> Result<Value, String> {
    let mut root = root_of(existing)?;
    let hooks = match root.get("hooks") {
        None => return Ok(Value::Object(root)),
        Some(Value::Object(hooks)) => hooks.clone(),
        Some(_) => return Err("settings.json: \"hooks\" não é o que o Claude Code define: não mexi nele.".into()),
    };
    let mut kept = serde_json::Map::new();
    for (event, entries) in hooks {
        let Some(list) = entries.as_array() else {
            kept.insert(event, entries);
            continue;
        };
        let rest: Vec<Value> = list.iter().filter(|entry| !ours(entry)).cloned().collect();
        if !rest.is_empty() {
            kept.insert(event, Value::Array(rest));
        }
    }
    if kept.is_empty() {
        root.remove("hooks");
    } else {
        root.insert("hooks".into(), Value::Object(kept));
    }
    Ok(Value::Object(root))
}

/// `existing` with our entries added (the ones of an older install replaced), everything else untouched.
pub(crate) fn merged(existing: &Value, exe: &str) -> Result<Value, String> {
    let mut root = root_of(&without_ours(existing)?)?;
    let mut hooks = root.get("hooks").and_then(Value::as_object).cloned().unwrap_or_default();
    for (event, matcher, timeout, wait) in EVENTS {
        let mut entry = json!({ "hooks": [{ "type": "command", "command": command(exe, wait), "timeout": timeout }] });
        if let Some(matcher) = matcher {
            entry["matcher"] = matcher.into();
        }
        match hooks.entry(event).or_insert_with(|| json!([])) {
            Value::Array(list) => list.push(entry),
            _ => return Err(format!("settings.json: \"hooks\".\"{event}\" não é uma lista: não mexi nele.")),
        }
    }
    root.insert("hooks".into(), Value::Object(hooks));
    Ok(Value::Object(root))
}

/// The settings file as JSON: absent reads as empty; one that does not parse is an error, never overwritten.
fn read(path: &std::path::Path) -> Result<Value, String> {
    match fs::read_to_string(path) {
        Ok(text) if text.trim().is_empty() => Ok(Value::Null),
        Ok(text) => serde_json::from_str(text.trim_start_matches('\u{feff}')).map_err(|e| format!("settings.json não é um JSON válido ({e}): não mexi nele.")),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(Value::Null),
        Err(e) => Err(format!("Não consegui ler {}: {e}", path.display())),
    }
}

/// Writes `new` over `path` after copying what is there to `settings.json.magoagent-<ms>.bak`.
fn write_with_backup(path: &std::path::Path, new: &Value) -> Result<(), String> {
    if path.is_file() {
        let backup = path.with_file_name(format!("settings.json.magoagent-{}.bak", now_ms()));
        fs::copy(path, &backup).map_err(|e| format!("Sem backup, não mexi no settings.json ({e})."))?;
    }
    if let Some(folder) = path.parent() {
        let _ = fs::create_dir_all(folder);
    }
    write_atomic(path, serde_json::to_string_pretty(new).unwrap_or_default()).map_err(|e| format!("Não consegui gravar {}: {e}", path.display()))
}

/// Our hooks are in the file at `path`.
pub(crate) fn installed_at(path: &std::path::Path) -> bool {
    read(path)
        .ok()
        .and_then(|settings| settings["hooks"].as_object().cloned())
        .is_some_and(|hooks| hooks.values().filter_map(Value::as_array).flatten().any(ours))
}

pub(crate) fn install_at(path: &std::path::Path, exe: &str) -> Result<String, String> {
    let new = merged(&read(path)?, exe)?;
    write_with_backup(path, &new)?;
    Ok(format!("Hooks instalados em {}: o Claude Code e o Claude Desktop passam a avisar a ilha (vale para as sessões novas).", path.display()))
}

pub(crate) fn remove_at(path: &std::path::Path) -> Result<String, String> {
    if !installed_at(path) {
        return Ok("Os hooks do MagoAgent não estavam instalados.".into());
    }
    write_with_backup(path, &without_ours(&read(path)?)?)?;
    Ok(format!("Hooks removidos de {}: o resto do arquivo ficou como estava.", path.display()))
}

impl DrawingApp {
    /// Ajustes' buttons and the MCP's `set_setting claude_hooks`: `install`, `remove` or `status`, on the person's real file.
    pub(crate) fn act_claude_hooks(&mut self, action: &str) -> Result<String, String> {
        let path = settings_path().ok_or("Não achei a pasta do Claude Code (.claude).")?;
        match action {
            "install" | "instalar" => install_at(&path, &std::env::current_exe().map_err(|e| e.to_string())?.to_string_lossy()),
            "remove" | "remover" => remove_at(&path),
            "status" => Ok(if installed_at(&path) { "Instalados." } else { "Não instalados." }.into()),
            other => Err(format!("claude_hooks é install, remove ou status, não {other}.")),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_home(name: &str) -> PathBuf {
        let home = std::env::temp_dir().join(format!("mago-hooks-{name}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&home);
        fs::create_dir_all(home.join(".claude")).unwrap();
        home
    }

    #[test]
    fn installing_keeps_what_was_there_and_removing_takes_only_ours() {
        let home = temp_home("merge");
        let path = home.join(".claude/settings.json");
        let theirs = json!({
            "model": "opus",
            "permissions": { "allow": ["Bash(ls)"] },
            "hooks": {
                "Stop": [{ "hooks": [{ "type": "command", "command": "afplay done.wav" }] }],
                "PreToolUse": [{ "matcher": "Bash", "hooks": [{ "type": "command", "command": "check.sh" }] }],
                "Custom": { "not": "a list" },
            }
        });
        fs::write(&path, theirs.to_string()).unwrap();
        // not a list under "Custom" is theirs to keep, so it does not stop the install: only our events are lists
        let message = install_at(&path, "C:\\Programas\\MagoAgent\\magoagent.exe").unwrap();
        assert!(message.contains("Hooks instalados"), "{message}");
        let now: Value = serde_json::from_str(&fs::read_to_string(&path).unwrap()).unwrap();
        assert_eq!((&now["model"], &now["permissions"]), (&theirs["model"], &theirs["permissions"]));
        assert_eq!(now["hooks"]["Custom"], theirs["hooks"]["Custom"]);
        assert_eq!(now["hooks"]["Stop"][0], theirs["hooks"]["Stop"][0], "their hook is first and untouched");
        assert_eq!(now["hooks"]["PreToolUse"][0], theirs["hooks"]["PreToolUse"][0]);
        assert_eq!(now["hooks"]["Stop"].as_array().unwrap().len(), 2);
        assert_eq!(now["hooks"]["PreToolUse"].as_array().unwrap().len(), 3, "theirs, every tool, and the AskUserQuestion that waits");
        let wait = &now["hooks"]["PreToolUse"][2];
        assert_eq!(wait["matcher"], "AskUserQuestion");
        assert_eq!(wait["hooks"][0]["command"], "\"C:/Programas/MagoAgent/magoagent.exe\" hook external wait", "quoted, forward slashes");
        assert_eq!(wait["hooks"][0]["timeout"], WAIT_TIMEOUT);
        assert_eq!(now["hooks"]["PermissionRequest"][0]["hooks"][0]["timeout"], WAIT_TIMEOUT);
        assert!(installed_at(&path));
        let backups = fs::read_dir(home.join(".claude")).unwrap().flatten().filter(|file| file.file_name().to_string_lossy().ends_with(".bak")).count();
        assert_eq!(backups, 1, "a backup before writing");
        // twice does not double it
        install_at(&path, "C:/Programas/MagoAgent/magoagent.exe").unwrap();
        let again: Value = serde_json::from_str(&fs::read_to_string(&path).unwrap()).unwrap();
        assert_eq!(again["hooks"]["Stop"].as_array().unwrap().len(), 2);
        // removing: back to what they had
        remove_at(&path).unwrap();
        let removed: Value = serde_json::from_str(&fs::read_to_string(&path).unwrap()).unwrap();
        assert_eq!(removed, theirs);
        assert!(!installed_at(&path));
        let _ = fs::remove_dir_all(&home);
    }

    #[test]
    fn a_file_that_is_absent_is_made_and_one_that_is_broken_is_left_alone() {
        let home = temp_home("novo");
        let path = home.join(".claude/settings.json");
        install_at(&path, "/usr/bin/magoagent").unwrap();
        assert!(installed_at(&path));
        remove_at(&path).unwrap();
        let after: Value = serde_json::from_str(&fs::read_to_string(&path).unwrap()).unwrap();
        assert_eq!(after, json!({}), "no empty hooks left behind");
        fs::write(&path, "{ \"model\": ").unwrap();
        assert!(install_at(&path, "/usr/bin/magoagent").unwrap_err().contains("não mexi"));
        assert_eq!(fs::read_to_string(&path).unwrap(), "{ \"model\": ", "untouched");
        fs::write(&path, r#"{ "hooks": ["x"] }"#).unwrap();
        assert!(install_at(&path, "/usr/bin/magoagent").is_err());
        let _ = fs::remove_dir_all(&home);
    }
}
