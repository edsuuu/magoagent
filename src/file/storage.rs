//! Autosave and preferences on disk, shared with the MCP server.

use crate::prelude::*;

/// Where the app keeps its data on each system: Application Support on macOS, %APPDATA% on
/// Windows, $XDG_DATA_HOME (or ~/.local/share) elsewhere.
pub(crate) fn data_dir() -> Option<PathBuf> {
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
        return Some(PathBuf::from(path));
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

pub(crate) fn modified(path: &std::path::Path) -> Option<SystemTime> {
    fs::metadata(path).and_then(|m| m.modified()).ok()
}

pub(crate) fn read_scene_file(path: &std::path::Path) -> Option<(Scene, Option<SystemTime>)> {
    let stamp = modified(path);
    let scene = parse_scene(&fs::read_to_string(path).ok()?).ok()?;
    Some((scene, stamp))
}

/// Write-then-rename, so the other process never reads half a file.
pub(crate) fn write_scene_file(path: &std::path::Path, scene: &Scene) -> std::io::Result<Option<SystemTime>> {
    if let Some(dir) = path.parent() {
        fs::create_dir_all(dir)?;
    }
    let tmp = path.with_extension(format!("tmp-{}", std::process::id()));
    fs::write(&tmp, serde_json::to_string(scene).map_err(std::io::Error::other)?)?;
    fs::rename(&tmp, path)?;
    Ok(modified(path))
}

/// Three-way merge by element id: whatever changed in `external` since `base` wins,
/// local edits to untouched elements and locally new elements survive.
pub(crate) fn merge_scenes(base: &Scene, local: &Scene, external: &Scene) -> Scene {
    let find = |scene: &Scene, id: &str| scene.elements.iter().find(|e| e.id == id).cloned();
    let mut elements: Vec<Element> = external
        .elements
        .iter()
        .filter_map(|ext| match find(base, &ext.id) {
            // untouched outside: keep the local version (or the local deletion)
            Some(old) if old == *ext => find(local, &ext.id),
            _ => Some(ext.clone()),
        })
        .collect();
    elements.extend(
        local
            .elements
            .iter()
            .filter(|e| find(base, &e.id).is_none() && find(external, &e.id).is_none())
            .cloned(),
    );
    let background = if external.background != base.background { external.background } else { local.background };
    Scene { version: external.version, elements, background, agent: None }
}
