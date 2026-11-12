//! Files: the element JSON shared by the clipboard and the MCP server, and the autosave,
//! preferences and merge on disk.

use crate::prelude::*;
mod interop;
pub(crate) use interop::*;

/// Why a drawing file can't be used as is. No caller writes over the file on any of these.
#[derive(Debug)]
pub(crate) enum SceneError {
    Missing,
    /// It is there but couldn't be read (locked, no permission): left alone.
    Io(String),
    /// Not a drawing (cut short, edited by hand): set aside by `set_aside`, never written over.
    Unreadable(String),
    /// Saved by a newer MagoAgent: shown read-only (what this build understands of it), so the
    /// fields this build doesn't know survive.
    Newer(Option<Scene>),
}

impl std::fmt::Display for SceneError {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        match self {
            Self::Missing => f.write_str("arquivo não encontrado"),
            Self::Io(e) => write!(f, "não foi possível ler o arquivo: {e}"),
            Self::Unreadable(e) => write!(f, "o arquivo não é um desenho válido: {e}"),
            Self::Newer(_) => f.write_str("o arquivo foi salvo por uma versão mais nova do MagoAgent; atualize este para editá-lo"),
        }
    }
}

/// This app's own `.mago` format (`.rea` before). A file from a newer build is refused: rewriting
/// it here would drop the fields this build doesn't know.
pub(crate) fn parse_scene(data: &str) -> Result<Scene, SceneError> {
    #[derive(Deserialize)]
    struct Stamp {
        version: u32,
    }
    let newer = serde_json::from_str::<Stamp>(data).is_ok_and(|s| s.version > SCENE_VERSION);
    let parsed = serde_json::from_str::<Scene>(data);
    if newer {
        return Err(SceneError::Newer(parsed.ok()));
    }
    // what this build writes back is its own format
    parsed
        .map(|mut scene| {
            scene.elements.iter_mut().for_each(Element::take_legacy_ide);
            Scene { version: SCENE_VERSION, ..scene }
        })
        .map_err(|e| SceneError::Unreadable(e.to_string()))
}

/// Where the app is installed (install.rs). Everything of the app lives in it: the program,
/// `data` (drawings, chats, preferences, the voice's models) and `data/projetos` (the chats'
/// folder). The running program's own folder when `data` is beside it; else the folder the
/// installer recorded (a build run from the repo uses the installed app's data); else the default.
pub(crate) fn install_dir() -> Option<PathBuf> {
    static DIR: std::sync::OnceLock<Option<PathBuf>> = std::sync::OnceLock::new();
    DIR.get_or_init(|| {
        let own = std::env::current_exe().ok().and_then(|p| p.parent().map(PathBuf::from)).filter(|d| {
            migrate_data(d);
            d.join("data").is_dir()
        });
        let dir = own.or_else(recorded_install_dir).or_else(default_install_dir);
        if let Some(dir) = &dir {
            migrate_data(dir);
        }
        dir
    })
    .clone()
}

/// The data folder was called `dados` before it took an English name: renamed in place when only
/// the old one exists.
pub(crate) fn migrate_data(dir: &std::path::Path) {
    let (old, new) = (dir.join("dados"), dir.join("data"));
    if old.is_dir() && !new.exists() {
        let _ = fs::rename(old, new);
    }
}

const INSTALL_KEY: &str = r"HKCU\Software\MagoAgent";

/// The folder the installer recorded: in the registry on Windows (as Steam keeps its SteamPath).
// ponytail: the Mac and Linux record nothing; they find the app in its own folder or the default
pub(crate) fn recorded_install_dir() -> Option<PathBuf> {
    if !cfg!(windows) {
        return None;
    }
    let out = crate::ide::command("reg").args(["query", INSTALL_KEY, "/v", "InstallDir"]).output().ok()?;
    let text = String::from_utf8_lossy(&out.stdout);
    let line = text.lines().find(|l| l.trim_start().starts_with("InstallDir"))?;
    let dir = line.split_once("REG_SZ")?.1.trim();
    (!dir.is_empty()).then(|| PathBuf::from(dir))
}

pub(crate) fn record_install_dir(dir: &std::path::Path) {
    if cfg!(windows) {
        let _ = crate::ide::command("reg").args(["add", INSTALL_KEY, "/v", "InstallDir", "/t", "REG_SZ", "/d"]).arg(dir).arg("/f").output();
    }
}

/// Where the installer suggests the app: Program Files on Windows (as Steam), ~/Applications on
/// macOS, ~/.local/opt elsewhere.
pub(crate) fn default_install_dir() -> Option<PathBuf> {
    let var = |name: &str| std::env::var_os(name).filter(|v| !v.is_empty()).map(PathBuf::from);
    if cfg!(windows) {
        Some(var("ProgramFiles").unwrap_or_else(|| PathBuf::from(r"C:\Program Files")).join("MagoAgent"))
    } else if cfg!(target_os = "macos") {
        var("HOME").map(|home| home.join("Applications/MagoAgent"))
    } else {
        var("HOME").map(|home| home.join(".local/opt/MagoAgent"))
    }
}

/// Where the app keeps its data: `data` in the install folder once installed; before that (a
/// build run from the repo) the system's own place, `legacy_data_dir`.
pub(crate) fn data_dir() -> Option<PathBuf> {
    match install_dir().map(|d| d.join("data")) {
        Some(dir) if dir.is_dir() => Some(dir),
        _ => legacy_data_dir(),
    }
}

/// The chats' folder when none was picked: inside the app's own data.
pub(crate) fn projects_dir() -> Option<PathBuf> {
    data_dir().map(|d| d.join("projetos"))
}

/// Where the app kept its data before the installer: Application Support on macOS, %APPDATA% on
/// Windows, $XDG_DATA_HOME (or ~/.local/share) elsewhere. The installer moves it into `data`.
pub(crate) fn legacy_data_dir() -> Option<PathBuf> {
    let var = |name: &str| std::env::var_os(name).filter(|v| !v.is_empty()).map(PathBuf::from);
    if cfg!(target_os = "macos") {
        var("HOME").map(|home| home.join("Library/Application Support/MagoAgent"))
    } else if cfg!(windows) {
        var("APPDATA").map(|appdata| appdata.join("MagoAgent"))
    } else {
        var("XDG_DATA_HOME").or_else(|| var("HOME").map(|home| home.join(".local/share"))).map(|dir| dir.join("magoagent"))
    }
}

/// The live drawing shared by the app and the MCP server; MAGOAGENT_SCENE points elsewhere.
pub(crate) fn autosave_path() -> Option<PathBuf> {
    if let Some(path) = std::env::var_os("MAGOAGENT_SCENE") {
        // relative to where the app started, not to whatever folder a terminal is in later
        return Some(std::path::absolute(&path).unwrap_or_else(|_| PathBuf::from(path)));
    }
    let path = data_dir()?.join("autosave.mago");
    // drawings saved before the .mago extension
    let old = path.with_extension("rea");
    if !path.exists() && old.exists() {
        let _ = fs::rename(&old, &path);
    }
    Some(path)
}

pub(crate) fn prefs_path() -> Option<PathBuf> {
    autosave_path().map(|p| p.with_file_name("preferences.json"))
}

/// The direct chats (app/chat/direct.rs), beside the drawing.
pub(crate) fn chats_path() -> Option<PathBuf> {
    autosave_path().map(|p| p.with_file_name("chats.json"))
}

/// What was said in a chat whose agent keeps no file the app reads (Codex), beside chats.json.
pub(crate) fn chat_items_path(id: &str) -> Option<PathBuf> {
    autosave_path().map(|p| p.with_file_name("chats").join(format!("{id}.json")))
}

/// The password vault's entries (vault.rs), beside the drawing; the secrets are in the keychain.
pub(crate) fn vault_path() -> Option<PathBuf> {
    autosave_path().map(|p| p.with_file_name("vault.json"))
}

/// The saved preferences. An unreadable file is set aside, so the first change doesn't write over
/// it: where it went comes back instead.
pub(crate) fn read_prefs(path: &std::path::Path) -> (Option<Prefs>, Option<PathBuf>) {
    let Ok(data) = fs::read_to_string(path) else { return (None, None) };
    match serde_json::from_str(&data) {
        Ok(prefs) => (Some(prefs), None),
        Err(_) => (None, set_aside(path).ok()),
    }
}

/// The direct chats (chat/direct.rs). Same as `read_prefs`: an unreadable file is set aside, so the
/// next save doesn't write an empty list over it.
pub(crate) fn read_chats(path: &std::path::Path) -> (Vec<crate::app::chat::direct::Chat>, Option<PathBuf>) {
    let Ok(data) = fs::read(path) else { return (Vec::new(), None) };
    match serde_json::from_slice(&data) {
        Ok(chats) => (chats, None),
        Err(_) => (Vec::new(), set_aside(path).ok()),
    }
}

/// Saved database connections (no passwords); next to the drawing, so MAGOAGENT_SCENE moves it too.
pub(crate) fn connections_path() -> Option<PathBuf> {
    autosave_path().map(|p| p.with_file_name("connections.json"))
}

pub(crate) fn portal_profile_path(id: &str) -> Option<PathBuf> {
    autosave_path().map(|p| p.with_file_name("perfis").join(portal_profile_name(id)))
}

pub(crate) fn portal_profile_name(id: &str) -> String {
    id.chars().map(|c| if c.is_ascii_alphanumeric() || "-_".contains(c) { c } else { '_' }).collect()
}

#[cfg(test)]
mod portal_profile_tests {
    #[test]
    fn profile_names_are_filesystem_safe() {
        assert_eq!(super::portal_profile_name("abc-123_X"), "abc-123_X");
        assert_eq!(super::portal_profile_name("../a:b"), "___a_b");
    }
}

pub(crate) fn modified(path: &std::path::Path) -> Option<SystemTime> {
    fs::metadata(path).and_then(|m| m.modified()).ok()
}

pub(crate) fn read_scene_file(path: &std::path::Path) -> Result<(Scene, Option<SystemTime>), SceneError> {
    let stamp = modified(path);
    let bytes = fs::read(path).map_err(|e| match e.kind() {
        std::io::ErrorKind::NotFound => SceneError::Missing,
        _ => SceneError::Io(e.to_string()),
    })?;
    let data = String::from_utf8(bytes).map_err(|e| SceneError::Unreadable(e.to_string()))?;
    let mut scene = parse_scene(&data)?;
    adopt_note_files(path, &mut scene, stamp);
    Ok((scene, stamp))
}

/// Each note's text lives in `notas/<id>.md` beside the scene, written with it (or in the folder it was moved to).
pub(crate) fn note_file(path: &std::path::Path, note: &Element) -> PathBuf {
    if let Some(moved) = &note.note.file {
        return PathBuf::from(moved);
    }
    path.with_file_name("notas").join(format!("{}.md", note.id))
}

/// The label index (the text) of every note in the scene.
fn note_labels(scene: &Scene) -> Vec<(usize, usize)> {
    (0..scene.elements.len()).filter(|i| scene.elements[*i].created.is_some()).filter_map(|i| note_label(&scene.elements, i).map(|l| (i, l))).collect()
}

fn write_note_files(path: &std::path::Path, scene: &Scene) {
    for (note, label) in note_labels(scene) {
        let ElementKind::Text { value, .. } = &scene.elements[label].kind else { continue };
        let file = note_file(path, &scene.elements[note]);
        if fs::read_to_string(&file).unwrap_or_default() != *value {
            let _ = file.parent().map(fs::create_dir_all);
            let _ = write_atomic(&file, value);
        }
    }
}

/// A note's `.md` edited after the scene was written (outside the app) wins over the scene's text.
fn adopt_note_files(path: &std::path::Path, scene: &mut Scene, scene_stamp: Option<SystemTime>) {
    for (note, label) in note_labels(scene) {
        let file = note_file(path, &scene.elements[note]);
        let (Some(newer), Ok(text)) = (modified(&file).zip(scene_stamp).map(|(m, s)| m > s), fs::read_to_string(&file)) else { continue };
        if let (true, ElementKind::Text { value, .. }) = (newer, &mut scene.elements[label].kind) {
            *value = text;
        }
    }
}

/// Moves an unreadable drawing out of the way as `<name>.bad-<ms>`, so a fresh one can start
/// without losing it.
pub(crate) fn set_aside(path: &std::path::Path) -> std::io::Result<PathBuf> {
    let ms = SystemTime::now().duration_since(SystemTime::UNIX_EPOCH).unwrap_or_default().as_millis();
    let mut kept = path.as_os_str().to_owned();
    kept.push(format!(".bad-{ms}"));
    let kept = PathBuf::from(kept);
    fs::rename(path, &kept)?;
    Ok(kept)
}

/// Write-then-rename, so another process (or the next start, after a crash) never reads half a file.
pub(crate) fn write_atomic(path: &std::path::Path, bytes: impl AsRef<[u8]>) -> std::io::Result<()> {
    let tmp = path.with_extension(format!("tmp-{}", std::process::id()));
    fs::write(&tmp, bytes)?;
    fs::rename(&tmp, path)
}

pub(crate) fn write_scene_file(path: &std::path::Path, scene: &Scene) -> std::io::Result<Option<SystemTime>> {
    if let Some(dir) = path.parent() {
        fs::create_dir_all(dir)?;
    }
    // the notes' files first: the scene is then never older than them, so a newer one was edited outside
    write_note_files(path, scene);
    write_atomic(path, serde_json::to_string(scene).map_err(std::io::Error::other)?)?;
    Ok(modified(path))
}

/// Three-way merge by element id: whatever changed in `external` since `base` wins,
/// local edits to untouched elements and locally new elements survive.
pub(crate) fn merge_scenes(base: &Scene, local: &Scene, external: &Scene) -> Scene {
    // by id, the first one winning as a search would
    fn index(scene: &Scene) -> std::collections::HashMap<&str, &Element> {
        scene.elements.iter().rev().map(|e| (e.id.as_str(), e)).collect()
    }
    let (base_ids, local_ids, external_ids) = (index(base), index(local), index(external));
    let mut elements: Vec<Element> = external
        .elements
        .iter()
        .filter_map(|ext| match base_ids.get(ext.id.as_str()) {
            // untouched outside: keep the local version (or the local deletion)
            Some(old) if *old == ext => local_ids.get(ext.id.as_str()).map(|e| (*e).clone()),
            _ => Some(ext.clone()),
        })
        .collect();
    elements.extend(local.elements.iter().filter(|e| !base_ids.contains_key(e.id.as_str()) && !external_ids.contains_key(e.id.as_str())).cloned());
    let background = if external.background != base.background { external.background } else { local.background };
    // the groups the same way: an untouched outside keeps the local version
    let mut groups: Vec<FrameGroup> = external
        .groups
        .iter()
        .filter_map(|ext| match base.groups.iter().find(|g| g.id == ext.id) {
            Some(old) if old == ext => local.groups.iter().find(|g| g.id == ext.id).cloned(),
            _ => Some(ext.clone()),
        })
        .collect();
    groups.extend(local.groups.iter().filter(|g| !base.groups.iter().chain(&external.groups).any(|o| o.id == g.id)).cloned());
    Scene { version: external.version, elements, background, agent: None, agent_terminal: None, groups }
}
