//! Ajustes › Integrações: `magoagent hook external` in the person's own `~/.claude/settings.json` (or
//! `CLAUDE_CONFIG_DIR`'s), so the island sees every Claude Code session on the machine, Claude Desktop's included.
//! The file is merged, never replaced: what was there stays, a dated backup is kept beside it, and removing takes out
//! only the entries that are ours (a command with `magoagent` and ` hook external`). Nothing here runs by itself: the
//! button asks for it.
//!
//! O comando instalado roda uma cópia fixa do relay (`relay_path`), numa pasta só deste usuário ao lado dos dados do
//! app: o exe do app muda de lugar a cada build e a cada atualização, e o settings.json da pessoa não pode apontar para
//! um caminho que some. A cópia é refeita quando a versão do app muda (`ensure_relay`).

use super::outside::WAIT_TIMEOUT;
use crate::prelude::*;
use std::path::Path;

/// (evento, matcher, timeout em s, espera a pessoa). O PreToolUse tem dois: toda ferramenta (o trabalho da pílula) e o
/// AskUserQuestion, que espera; um pedido de permissão também espera, menos o do AskUserQuestion, que é do hook que espera.
const EVENTS: [(&str, Option<&str>, u64, bool); 13] = [
    ("SessionStart", None, 10, false),
    ("SessionEnd", None, 10, false),
    ("UserPromptSubmit", None, 10, false),
    ("PreToolUse", None, 10, false),
    ("PreToolUse", Some("AskUserQuestion"), WAIT_TIMEOUT, true),
    ("PostToolUse", None, 10, false),
    ("PostToolUseFailure", None, 10, false),
    ("Notification", None, 10, false),
    ("SubagentStart", None, 10, false),
    ("SubagentStop", None, 10, false),
    ("Stop", None, 10, false),
    ("StopFailure", None, 10, false),
    ("PermissionRequest", None, WAIT_TIMEOUT, false),
];

/// A folder of the tests' own for `what`, one per test (each runs on its thread): a test that installs the hooks never
/// changes the page another test is clicking on, and never the person's.
pub(crate) fn test_folder(what: &str) -> PathBuf {
    let thread: String = format!("{:?}", std::thread::current().id()).chars().filter(char::is_ascii_digit).collect();

    std::env::temp_dir().join(format!("mago-{what}-proprio-{}-{thread}", std::process::id()))
}

/// Claude Code's settings file: `CLAUDE_CONFIG_DIR` when the person moved it there, else `~/.claude`. In the tests, a folder
/// of their own (`test_folder`): Ajustes reads it on every frame of Integrações and its buttons write it.
pub(crate) fn settings_path() -> Option<PathBuf> {
    if cfg!(test) {
        return Some(test_folder("claude").join("settings.json"));
    }

    let var = |name: &str| std::env::var_os(name).filter(|value| !value.is_empty()).map(PathBuf::from);
    let folder = var("CLAUDE_CONFIG_DIR").or_else(|| var(if cfg!(windows) { "USERPROFILE" } else { "HOME" }).map(|home| home.join(".claude")))?;

    Some(folder.join("settings.json"))
}

/// Onde fica a cópia do relay que os hooks rodam: `%LOCALAPPDATA%\MagoAgent\hook` no Windows (um exe não vai na pasta
/// que acompanha o usuário pela rede), a pasta de dados do usuário nos outros sistemas. Nos testes, uma pasta só deles:
/// um botão Instalar apertado num teste copiaria o executável de teste por cima do relay que os hooks da pessoa rodam
/// (o cargo novo põe esse executável fundo em `target/<perfil>/build/`, e ele não se vê como build do repo).
pub(crate) fn relay_path() -> Option<PathBuf> {
    if cfg!(test) {
        return Some(test_folder("relay").join("magoagent-hook"));
    }

    let local = std::env::var_os("LOCALAPPDATA").filter(|value| !value.is_empty()).map(|local| PathBuf::from(local).join("MagoAgent"));
    let folder = if cfg!(windows) { local } else { crate::file::legacy_data_dir() }?;

    Some(folder.join("hook").join(if cfg!(windows) { "magoagent-hook.exe" } else { "magoagent-hook" }))
}

/// O carimbo de um executável: a versão do app, o tamanho e a data dele. Mudou, a cópia do relay é refeita.
fn stamp(exe: &Path) -> Option<String> {
    let meta = fs::metadata(exe).ok()?;
    let modified = meta.modified().ok()?.duration_since(SystemTime::UNIX_EPOCH).ok()?.as_secs();

    Some(format!("{} {} {modified}", env!("CARGO_PKG_VERSION"), meta.len()))
}

/// Deixa em `relay` uma cópia de `exe` (o app em uso), refeita quando o carimbo dele muda. A cópia nova é gravada ao
/// lado e renomeada por cima, para um hook nunca rodar meio arquivo. Com a antiga em uso por um hook (o Windows não
/// troca um exe rodando) ela fica, que é o mesmo relay, e a próxima abertura do app tenta de novo.
pub(crate) fn ensure_relay(exe: &Path, relay: &Path) -> Result<(), String> {
    let (stamp_file, current) = (relay.with_extension("version"), stamp(exe));

    if relay.is_file() && current.is_some() && fs::read_to_string(&stamp_file).ok() == current {
        return Ok(());
    }

    let failed = |error: std::io::Error| format!("Não consegui copiar o relay dos hooks para {}: {error}", relay.display());
    let folder = relay.parent().ok_or_else(|| failed(std::io::ErrorKind::NotFound.into()))?;

    fs::create_dir_all(folder).map_err(failed)?;

    // ninguém mais troca o relay que o Claude Code roda: a pasta é só deste usuário (no Windows a %LOCALAPPDATA% já é)
    #[cfg(unix)]
    let _ = fs::set_permissions(folder, std::os::unix::fs::PermissionsExt::from_mode(0o700));

    let fresh = relay.with_extension("new");

    match fs::copy(exe, &fresh).and_then(|_| fs::rename(&fresh, relay)) {
        Ok(()) => {
            let _ = fs::write(&stamp_file, current.unwrap_or_default());

            Ok(())
        }
        Err(error) => {
            let _ = fs::remove_file(&fresh);

            if relay.is_file() { Ok(()) } else { Err(failed(error)) }
        }
    }
}

/// Na abertura do app: se os hooks já usam a cópia do relay, ela acompanha a versão do app. Nada é instalado aqui: sem
/// cópia, nada a fazer.
pub(crate) fn refresh_relay() {
    if let (Some(relay), Ok(exe)) = (relay_path().filter(|relay| relay.is_file()), std::env::current_exe()) {
        let _ = ensure_relay(&exe, &relay);
    }
}

/// Claude Code runs a hook through the shell (Git Bash on Windows): the exe's path quoted, with forward slashes.
pub(crate) fn preview_at(path: &Path, exe: &str) -> Result<(Value, Value), String> {
    let existing = read(path)?;

    Ok((existing.clone(), merged(&existing, exe)?))
}

fn command(exe: &str, wait: bool) -> String {
    format!("\"{}\" hook external{}", exe.replace('\\', "/"), if wait { " wait" } else { "" })
}

/// An entry of ours: a command with `magoagent` and ` hook external` (in the Codex's hooks.json too).
pub(crate) fn ours(entry: &Value) -> bool {
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
pub(crate) fn read(path: &std::path::Path) -> Result<Value, String> {
    read_named(path, "settings.json")
}

/// A hooks file as JSON, as `read`; `name` is how the messages call it (settings.json, the Codex's hooks.json).
pub(crate) fn read_named(path: &std::path::Path, name: &str) -> Result<Value, String> {
    match fs::read_to_string(path) {
        Ok(text) if text.trim().is_empty() => Ok(Value::Null),
        Ok(text) => serde_json::from_str(text.trim_start_matches('\u{feff}')).map_err(|e| format!("{name} não é um JSON válido ({e}): não mexi nele.")),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(Value::Null),
        Err(e) => Err(format!("Não consegui ler {}: {e}", path.display())),
    }
}

/// Writes `new` over `path` after copying what is there to `<name>.magoagent-<ms>.bak`.
pub(crate) fn write_with_backup(path: &std::path::Path, name: &str, new: &Value) -> Result<(), String> {
    if path.is_file() {
        let backup = path.with_file_name(format!("{name}.magoagent-{}.bak", now_ms()));

        fs::copy(path, &backup).map_err(|e| format!("Sem backup, não mexi no {name} ({e})."))?;
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

/// As nossas entradas no arquivo, (evento, entrada), na ordem dos eventos.
fn ours_in(settings: &Value) -> Vec<(String, Value)> {
    let mut found = Vec::new();

    for (event, entries) in settings["hooks"].as_object().into_iter().flatten() {
        found.extend(entries.as_array().into_iter().flatten().filter(|entry| ours(entry)).map(|entry| (event.clone(), entry.clone())));
    }

    found.sort_by(|(a, _), (b, _)| a.cmp(b));
    found
}

/// Os nossos hooks estão no arquivo em `path`, mas não são os que esta versão instalaria com `exe`: apontam para outro
/// executável (o do app, que muda a cada build) ou falta um evento. Só instalar de novo conserta: nada roda sozinho.
pub(crate) fn outdated_at(path: &Path, exe: &str) -> bool {
    read(path).is_ok_and(|existing| state_in(&existing, exe) == HooksState::Outdated)
}

/// Como os hooks do MagoAgent estão num settings.json: ausentes, os desta versão ou os de outra.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum HooksState {
    Missing,
    Installed,
    Outdated,
}

/// O estado dos nossos hooks em `settings` (o arquivo já lido), para o relay em `exe`.
pub(crate) fn state_in(settings: &Value, exe: &str) -> HooksState {
    let installed = ours_in(settings);

    if installed.is_empty() {
        HooksState::Missing
    } else if merged(&Value::Null, exe).is_ok_and(|wanted| installed == ours_in(&wanted)) {
        HooksState::Installed
    } else {
        HooksState::Outdated
    }
}

/// O estado no arquivo em `path`; um arquivo que não se lê conta como sem hooks (instalar diz por quê).
pub(crate) fn state_at(path: &Path, exe: &str) -> HooksState {
    read(path).map_or(HooksState::Missing, |settings| state_in(&settings, exe))
}

pub(crate) fn install_at(path: &std::path::Path, exe: &str) -> Result<String, String> {
    let new = merged(&read(path)?, exe)?;

    install_preview_at(path, &new)
}

pub(crate) fn install_preview_at(path: &std::path::Path, preview: &Value) -> Result<String, String> {
    write_with_backup(path, "settings.json", preview)?;
    Ok(format!("Hooks instalados em {}: o Claude Code e o Claude Desktop passam a avisar a ilha (vale para as sessões novas).", path.display()))
}

pub(crate) fn remove_at(path: &std::path::Path) -> Result<String, String> {
    if !installed_at(path) {
        return Ok("Os hooks do MagoAgent não estavam instalados.".into());
    }

    write_with_backup(path, "settings.json", &without_ours(&read(path)?)?)?;
    Ok(format!("Hooks removidos de {}: o resto do arquivo ficou como estava.", path.display()))
}

impl DrawingApp {
    /// Ajustes' buttons and the MCP's `set_setting claude_hooks`: `install`, `remove` or `status`, on the person's real file.
    pub(crate) fn act_claude_hooks(&mut self, action: &str) -> Result<String, String> {
        let path = settings_path().ok_or("Não achei a pasta do Claude Code (.claude).")?;
        let relay = relay_path().ok_or("Não achei a pasta de dados do MagoAgent para o relay dos hooks.")?;

        match action {
            "install" | "instalar" => {
                ensure_relay(&std::env::current_exe().map_err(|e| e.to_string())?, &relay)?;
                install_at(&path, &relay.to_string_lossy())
            }
            "remove" | "remover" => remove_at(&path),
            "status" if outdated_at(&path, &relay.to_string_lossy()) => Ok("Instalados, mas de outra versão do MagoAgent: instale de novo.".into()),
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

    #[test]
    fn the_relay_is_copied_to_its_own_folder_and_only_again_when_the_app_changes() {
        let home = temp_home("relay");
        let (exe, relay) = (home.join("magoagent.exe"), home.join("dados").join("hook").join("magoagent-hook.exe"));

        fs::write(&exe, "versão um").unwrap();
        ensure_relay(&exe, &relay).unwrap();
        assert_eq!(fs::read_to_string(&relay).unwrap(), "versão um");
        // o mesmo app: a cópia fica como está (até mexida por fora)
        fs::write(&relay, "mexida").unwrap();
        ensure_relay(&exe, &relay).unwrap();
        assert_eq!(fs::read_to_string(&relay).unwrap(), "mexida");
        // o app mudou (outro tamanho): a cópia é refeita, sem sobra ao lado
        fs::write(&exe, "versão dois, maior").unwrap();
        ensure_relay(&exe, &relay).unwrap();
        assert_eq!(fs::read_to_string(&relay).unwrap(), "versão dois, maior");
        assert!(!relay.with_extension("new").exists());
        // sem o app para copiar: a cópia que existe serve; sem cópia nenhuma, erro
        let missing = home.join("sumiu.exe");

        assert!(ensure_relay(&missing, &relay).is_ok());
        assert!(ensure_relay(&missing, &home.join("outra").join("magoagent-hook.exe")).unwrap_err().contains("Não consegui copiar"));
        let _ = fs::remove_dir_all(&home);
    }

    #[test]
    fn installing_and_removing_preserve_the_order_of_settings_keys() {
        let home = temp_home("ordem");
        let path = home.join(".claude/settings.json");
        let original = r#"{"model":"opus","permissions":{"allow":[]},"enabledPlugins":{},"hooks":{"Stop":[]}}"#;

        fs::write(&path, original).unwrap();
        install_at(&path, "/usr/bin/magoagent").unwrap();
        let installed = fs::read_to_string(&path).unwrap();

        assert!(installed.find("\"model\"").unwrap() < installed.find("\"permissions\"").unwrap());
        assert!(installed.find("\"permissions\"").unwrap() < installed.find("\"enabledPlugins\"").unwrap());
        assert!(installed.find("\"enabledPlugins\"").unwrap() < installed.find("\"hooks\"").unwrap());
        remove_at(&path).unwrap();
        let removed = fs::read_to_string(&path).unwrap();

        assert!(removed.find("\"model\"").unwrap() < removed.find("\"permissions\"").unwrap());
        assert!(removed.find("\"permissions\"").unwrap() < removed.find("\"enabledPlugins\"").unwrap());
        assert!(removed.find("\"enabledPlugins\"").unwrap() > removed.find("\"permissions\"").unwrap());
        assert!(!removed.contains("\"hooks\""), "o objeto hooks vazio deve ser removido");
        let _ = fs::remove_dir_all(&home);
    }

    #[test]
    fn preview_does_not_write_until_it_is_explicitly_applied() {
        let home = temp_home("preview");
        let path = home.join(".claude/settings.json");
        let original = json!({ "model": "opus", "hooks": { "Stop": [] } });

        fs::write(&path, original.to_string()).unwrap();
        let (before, preview) = preview_at(&path, "/tmp/magoagent-hook").unwrap();

        assert_eq!(before, original);
        assert_eq!(serde_json::from_str::<Value>(&fs::read_to_string(&path).unwrap()).unwrap(), original);
        assert!(preview["hooks"]["SessionStart"].is_array());
        install_preview_at(&path, &preview).unwrap();
        assert!(installed_at(&path));
        assert_eq!(serde_json::from_str::<Value>(&fs::read_to_string(&path).unwrap()).unwrap(), preview);
        let _ = fs::remove_dir_all(&home);
    }

    #[test]
    fn hooks_of_another_version_are_told_apart() {
        let home = temp_home("versao");
        let path = home.join(".claude/settings.json");

        assert!(!outdated_at(&path, "/dados/hook/magoagent-hook"), "nada instalado não é desatualizado");
        // os hooks de quem instalou antes da cópia fixa apontam para o exe do app
        install_at(&path, "/repo/target/release/magoagent").unwrap();
        assert!(outdated_at(&path, "/dados/hook/magoagent-hook"));
        install_at(&path, "/dados/hook/magoagent-hook").unwrap();
        assert!(!outdated_at(&path, "/dados/hook/magoagent-hook"));
        // um hook da pessoa depois dos nossos não muda nada; um evento nosso a menos, sim
        let mut settings: Value = serde_json::from_str(&fs::read_to_string(&path).unwrap()).unwrap();

        settings["hooks"]["Stop"].as_array_mut().unwrap().push(json!({ "hooks": [{ "type": "command", "command": "afplay done.wav" }] }));
        fs::write(&path, settings.to_string()).unwrap();
        assert!(!outdated_at(&path, "/dados/hook/magoagent-hook"));
        settings["hooks"].as_object_mut().unwrap().remove("SubagentStart");
        fs::write(&path, settings.to_string()).unwrap();
        assert!(outdated_at(&path, "/dados/hook/magoagent-hook"));
        let _ = fs::remove_dir_all(&home);
    }

    #[test]
    fn the_state_says_missing_installed_or_of_another_version() {
        let home = temp_home("estado");
        let path = home.join(".claude/settings.json");
        let relay = "/dados/hook/magoagent-hook";

        assert_eq!(state_at(&path, relay), HooksState::Missing, "no file");
        fs::write(&path, json!({ "model": "opus" }).to_string()).unwrap();
        assert_eq!(state_at(&path, relay), HooksState::Missing, "a file without ours");
        install_at(&path, relay).unwrap();
        assert_eq!(state_at(&path, relay), HooksState::Installed);
        assert_eq!(state_in(&read(&path).unwrap(), "/outro/magoagent-hook"), HooksState::Outdated);
        remove_at(&path).unwrap();
        assert_eq!(state_at(&path, relay), HooksState::Missing);
        let _ = fs::remove_dir_all(&home);
    }
}
