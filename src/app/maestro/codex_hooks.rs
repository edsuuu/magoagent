//! Ajustes › Integrações: instalação explícita dos hooks externos no `~/.codex/hooks.json`.

use crate::prelude::*;
use std::path::Path;

const EVENTS: [(&str, u64); 10] = [
    ("SessionStart", 10),
    ("UserPromptSubmit", 10),
    ("PreToolUse", 10),
    ("PermissionRequest", 120),
    ("PostToolUse", 10),
    ("Stop", 10),
    ("SubagentStart", 10),
    ("SubagentStop", 10),
    ("Interrupt", 3),
    ("SessionEnd", 3),
];

/// Codex's hooks file: `CODEX_CONFIG_DIR` or `~/.codex`; in the tests, a folder of their own, never the person's.
pub(crate) fn codex_settings_path() -> Option<PathBuf> {
    if cfg!(test) {
        return Some(std::env::temp_dir().join(format!("mago-codex-proprio-{}", std::process::id())).join("hooks.json"));
    }
    let folder = std::env::var_os("CODEX_CONFIG_DIR")
        .filter(|value| !value.is_empty())
        .map(PathBuf::from)
        .or_else(|| std::env::var_os(if cfg!(windows) { "USERPROFILE" } else { "HOME" }).map(|home| PathBuf::from(home).join(".codex")))?;
    Some(folder.join("hooks.json"))
}

fn ours(entry: &Value) -> bool {
    entry["hooks"].as_array().is_some_and(|hooks| {
        hooks.iter().any(|hook| hook["command"].as_str().is_some_and(|command| command.contains("magoagent") && command.contains(" hook external")))
    })
}

fn read(path: &Path) -> Result<Value, String> {
    match fs::read_to_string(path) {
        Ok(text) if text.trim().is_empty() => Ok(Value::Null),
        Ok(text) => serde_json::from_str(text.trim_start_matches('\u{feff}')).map_err(|e| format!("hooks.json não é um JSON válido ({e}): não mexi nele.")),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(Value::Null),
        Err(error) => Err(format!("Não consegui ler {}: {error}", path.display())),
    }
}

fn without_ours(existing: &Value) -> Result<Value, String> {
    let mut root = match existing {
        Value::Object(root) => root.clone(),
        Value::Null => return Ok(json!({})),
        _ => return Err("hooks.json não é um objeto JSON: não mexi nele.".into()),
    };
    let Some(hooks) = root.get("hooks") else { return Ok(Value::Object(root)) };
    let hooks = hooks.as_object().ok_or("hooks.json: \"hooks\" não é um objeto: não mexi nele.")?;
    let mut kept = serde_json::Map::new();
    for (event, entries) in hooks {
        let list = entries.as_array().ok_or_else(|| format!("hooks.json: \"hooks\".\"{event}\" não é uma lista: não mexi nele."))?;
        let rest: Vec<Value> = list.iter().filter(|entry| !ours(entry)).cloned().collect();
        if !rest.is_empty() {
            kept.insert(event.clone(), Value::Array(rest));
        }
    }
    if kept.is_empty() {
        root.remove("hooks");
    } else {
        root.insert("hooks".into(), Value::Object(kept));
    }
    Ok(Value::Object(root))
}

fn merged(existing: &Value, exe: &str) -> Result<Value, String> {
    let mut root = without_ours(existing)?.as_object().cloned().unwrap_or_default();
    let mut hooks = root.get("hooks").and_then(Value::as_object).cloned().unwrap_or_default();
    let exe = exe.replace('\\', "/");
    for (event, timeout) in EVENTS {
        let wait = if event == "PermissionRequest" { " wait" } else { "" };
        let command = format!("\"{exe}\" hook external{wait}");
        let mut hook = json!({ "type": "command", "command": command, "timeout": timeout });
        if event == "PermissionRequest" {
            hook["statusMessage"] = "Aguardando sua resposta no MagoAgent".into();
        }
        let entry = json!({ "hooks": [hook] });
        match hooks.entry(event).or_insert_with(|| json!([])) {
            Value::Array(entries) => entries.push(entry),
            _ => return Err(format!("hooks.json: \"hooks\".\"{event}\" não é uma lista: não mexi nele.")),
        }
    }
    root.insert("hooks".into(), Value::Object(hooks));
    Ok(Value::Object(root))
}

fn write_with_backup(path: &Path, value: &Value) -> Result<(), String> {
    if path.is_file() {
        let stamp = SystemTime::now().duration_since(SystemTime::UNIX_EPOCH).map_or(0, |time| time.as_millis());
        let backup = path.with_file_name(format!("hooks.json.magoagent-{stamp}.bak"));
        fs::copy(path, backup).map_err(|error| format!("Sem backup, não mexi no hooks.json ({error})."))?;
    }
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|error| format!("Não consegui criar {}: {error}", parent.display()))?;
    }
    let text = serde_json::to_string_pretty(value).map_err(|error| error.to_string())?;
    let temporary = path.with_extension("json.new");
    fs::write(&temporary, text).and_then(|_| fs::rename(&temporary, path)).map_err(|error| {
        let _ = fs::remove_file(&temporary);
        format!("Não consegui gravar {}: {error}", path.display())
    })
}

pub(crate) fn installed_at(path: &Path) -> bool {
    read(path).ok().and_then(|root| root["hooks"].as_object().cloned()).is_some_and(|hooks| hooks.values().filter_map(Value::as_array).flatten().any(ours))
}

fn install_at(path: &Path, exe: &str) -> Result<String, String> {
    let merged = merged(&read(path)?, exe)?;
    write_with_backup(path, &merged)?;
    Ok(format!("Hooks do Codex instalados em {}: as sessões novas passam a avisar a ilha.", path.display()))
}

fn remove_at(path: &Path) -> Result<String, String> {
    if !installed_at(path) {
        return Ok("Os hooks do MagoAgent não estavam instalados no Codex.".into());
    }
    write_with_backup(path, &without_ours(&read(path)?)?)?;
    Ok(format!("Hooks removidos de {}: o resto do arquivo ficou como estava.", path.display()))
}

impl DrawingApp {
    pub(crate) fn act_codex_hooks(&mut self, action: &str) -> Result<String, String> {
        let path = codex_settings_path().ok_or("Não achei a pasta do Codex (.codex).")?;
        match action {
            "install" | "instalar" => {
                let relay = crate::app::maestro::claude_hooks::relay_path().ok_or("Não achei a pasta de dados do MagoAgent para o relay dos hooks.")?;
                let exe = std::env::current_exe().map_err(|error| error.to_string())?;
                crate::app::maestro::claude_hooks::ensure_relay(&exe, &relay)?;
                install_at(&path, &relay.to_string_lossy())
            }
            "remove" | "remover" => remove_at(&path),
            "status" => Ok(if installed_at(&path) { "Hooks do Codex instalados." } else { "Hooks do Codex não instalados." }.into()),
            other => Err(format!("codex_hooks é install, remove ou status, não {other}.")),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn install_and_remove_preserve_other_codex_hooks() {
        let path = std::env::temp_dir().join(format!("mago-codex-hooks-{}.json", std::process::id()));
        let _ = fs::remove_file(&path);
        let theirs = json!({"description":"mine", "hooks":{"PreToolUse":[{"matcher":"Bash", "hooks":[{"type":"command", "command":"guard.sh"}]}]}});
        fs::write(&path, theirs.to_string()).unwrap();
        install_at(&path, "C:\\MagoAgent\\magoagent-hook.exe").unwrap();
        let installed: Value = serde_json::from_str(&fs::read_to_string(&path).unwrap()).unwrap();
        assert_eq!(installed["description"], "mine");
        assert_eq!(installed["hooks"]["PreToolUse"][0], theirs["hooks"]["PreToolUse"][0]);
        assert_eq!(installed["hooks"]["PermissionRequest"][0]["hooks"][0]["timeout"], 120);
        assert!(installed["hooks"]["PermissionRequest"][0]["hooks"][0]["command"].as_str().unwrap().ends_with("hook external wait"));
        assert!(installed_at(&path));
        remove_at(&path).unwrap();
        let removed: Value = serde_json::from_str(&fs::read_to_string(&path).unwrap()).unwrap();
        assert_eq!(removed, theirs);
        let _ = fs::remove_file(path);
    }

    #[test]
    fn invalid_file_is_not_overwritten() {
        let path = std::env::temp_dir().join(format!("mago-codex-invalid-{}.json", std::process::id()));
        let original = "{bad";
        fs::write(&path, original).unwrap();
        assert!(install_at(&path, "/usr/bin/magoagent").is_err());
        assert_eq!(fs::read_to_string(&path).unwrap(), original);
        let _ = fs::remove_file(path);
    }
}
