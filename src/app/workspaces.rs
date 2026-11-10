//! Workspaces (espaços de trabalho): each one a project with its own drawing, root folder and
//! color, listed in `espacos.json` beside the drawings. Switching changes what shows, not what
//! runs: the shells stay in `terminals` (by element id), and the ones whose drawing is off view
//! are listed in `away` with their space, which is all the loops need to keep them alive.

use crate::app::chat_direct::folder;
use crate::prelude::*;
use std::path::Path;

/// The space everyone had before workspaces: the drawing of today (`autosave.mago`).
const MAIN: &str = "principal";

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub(crate) struct Workspace {
    pub(crate) id: String,
    #[serde(rename = "nome")]
    pub(crate) name: String,
    #[serde(rename = "raiz", default)]
    pub(crate) root: String,
    /// Its drawing's file name, beside `espacos.json`.
    #[serde(rename = "cena")]
    pub(crate) scene: String,
    #[serde(rename = "cor", default)]
    pub(crate) color: [u8; 3],
}

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
pub(crate) struct Workspaces {
    #[serde(rename = "espacos")]
    pub(crate) list: Vec<Workspace>,
    #[serde(rename = "atual")]
    pub(crate) current: String,
}

/// The new / remove workspace cards (ui/sidebar.rs).
#[derive(Clone, Debug)]
pub(crate) enum SpaceDialog {
    New { name: String, folder: String },
    Remove(String),
}

/// The tints a space wears: the terminals' eight, in turn.
pub(crate) fn space_colors() -> Vec<[u8; 3]> {
    TERMINAL_COLORS.iter().flatten().copied().collect()
}

fn spaces_file(scene: &Path) -> PathBuf {
    scene.with_file_name("espacos.json")
}

/// A drawing's name as `espacos.json` keeps it: a bare file name, never a path out of the folder.
fn plain(name: &str) -> bool {
    !name.is_empty() && Path::new(name).file_name().is_some_and(|f| f == name)
}

fn main_space(scene: &Path) -> Workspaces {
    let file = scene.file_name().map_or("autosave.mago".into(), |f| f.to_string_lossy().into_owned());
    let main = Workspace { id: MAIN.into(), name: "Principal".into(), root: String::new(), scene: file, color: space_colors()[0] };
    Workspaces { list: vec![main], current: MAIN.into() }
}

/// The workspaces beside `scene` (the drawing the app opened). None saved: one, "Principal", on that
/// very drawing. An unreadable file is set aside like `read_chats` does, and where it went comes back.
pub(crate) fn read_spaces(scene: &Path) -> (Workspaces, Option<PathBuf>) {
    let path = spaces_file(scene);
    let Ok(data) = fs::read(&path) else { return (main_space(scene), None) };
    match serde_json::from_slice::<Workspaces>(&data) {
        Ok(mut spaces) if !spaces.list.is_empty() && spaces.list.iter().all(|w| plain(&w.scene)) => {
            if !spaces.list.iter().any(|w| w.id == spaces.current) {
                spaces.current = spaces.list[0].id.clone();
            }
            (spaces, None)
        }
        _ => (main_space(scene), set_aside(&path).ok()),
    }
}

/// The drawing of the workspace on screen, for an MCP outside the terminals (the chat's Maestro):
/// a terminal's own comes in MAGOAGENT_SCENE.
pub(crate) fn current_scene(base: &Path) -> PathBuf {
    let saved = fs::read(spaces_file(base)).ok().and_then(|d| serde_json::from_slice::<Workspaces>(&d).ok());
    let current = saved.and_then(|s| s.list.into_iter().find(|w| w.id == s.current)).filter(|w| plain(&w.scene));
    current.map_or(base.to_path_buf(), |w| base.with_file_name(w.scene))
}

impl DrawingApp {
    /// The workspaces, read once (main calls it before reading the drawing, which then is the
    /// current space's).
    pub(crate) fn load_spaces(&mut self) {
        if !self.spaces.list.is_empty() {
            return;
        }
        let Some(scene) = self.autosave.clone() else {
            // no data folder: the one drawing, in memory
            self.spaces = main_space(Path::new("autosave.mago"));
            return;
        };
        let (spaces, bad) = read_spaces(&scene);
        if let Some(kept) = bad {
            self.toast(format!("Não foi possível ler espacos.json: ficou guardado em {} e os espaços voltaram ao Principal.", kept.display()));
        }
        if let Some(space) = spaces.list.iter().find(|w| w.id == spaces.current) {
            self.autosave = Some(scene.with_file_name(&space.scene));
        }
        self.spaces = spaces;
    }

    fn save_spaces(&self) {
        if let Some(path) = self.autosave.as_deref().map(spaces_file) {
            let _ = serde_json::to_string_pretty(&self.spaces).map(|json| write_atomic(&path, json));
        }
    }

    pub(crate) fn current_space(&self) -> Option<&Workspace> {
        self.spaces.list.iter().find(|w| w.id == self.spaces.current)
    }

    /// Where new terminals and IDEs of the space on screen start.
    pub(crate) fn space_root(&self) -> String {
        self.current_space().map_or(String::new(), |w| w.root.clone())
    }

    pub(crate) fn space_name(&self, id: &str) -> String {
        self.spaces.list.iter().find(|w| w.id == id).map_or(String::new(), |w| w.name.clone())
    }

    /// A space by id or name (any case).
    pub(crate) fn space_named(&self, name: &str) -> Result<String, String> {
        let wanted = name.trim().to_lowercase();
        let found = self.spaces.list.iter().find(|w| w.id == name.trim() || w.name.to_lowercase() == wanted);
        found.map(|w| w.id.clone()).ok_or_else(|| format!("Nenhum espaço se chama {name}."))
    }

    /// Per space: its agents running, and how many of them need the person (waiting or done unseen).
    pub(crate) fn space_counts(&self) -> std::collections::HashMap<String, (usize, usize)> {
        let mut counts: std::collections::HashMap<String, (usize, usize)> = Default::default();
        for row in self.agent_rows().into_iter().filter(|r| !r.space.is_empty()) {
            let running = self.terminals.get(&row.id).is_some_and(|s| s.as_ref().is_ok_and(|s| !s.exited()));
            let needs = row.state == crate::app::ui::agents::AgentState::Waiting || self.terminal_unread.contains(&row.id);
            let entry = counts.entry(row.space).or_default();
            entry.0 += usize::from(running);
            entry.1 += usize::from(needs);
        }
        counts
    }

    /// A new space (not opened): its drawing is `espaco-<id>.mago`, written on its first change.
    pub(crate) fn create_space(&mut self, name: &str, root: &str) -> Result<String, String> {
        self.load_spaces();
        let name = name.trim();
        if name.is_empty() {
            return Err("Diga o nome do espaço.".into());
        }
        if self.spaces.list.iter().any(|w| w.name.to_lowercase() == name.to_lowercase()) {
            return Err(format!("Já existe um espaço chamado {name}."));
        }
        if !root.trim().is_empty() && !folder(root).is_some_and(|p| p.is_dir()) {
            return Err(format!("A pasta {} não existe.", root.trim()));
        }
        let id = new_id();
        let color = space_colors()[self.spaces.list.len() % space_colors().len()];
        let scene = format!("espaco-{}.mago", portal_profile_name(&id));
        self.spaces.list.push(Workspace { id: id.clone(), name: name.into(), root: root.trim().into(), scene, color });
        self.save_spaces();
        Ok(id)
    }

    pub(crate) fn rename_space(&mut self, id: &str, name: &str) -> Result<(), String> {
        let name = name.trim();
        if name.is_empty() {
            return Err("Diga o nome novo.".into());
        }
        if self.spaces.list.iter().any(|w| w.id != id && w.name.to_lowercase() == name.to_lowercase()) {
            return Err(format!("Já existe um espaço chamado {name}."));
        }
        let space = self.spaces.list.iter_mut().find(|w| w.id == id).ok_or("Esse espaço não existe.")?;
        space.name = name.into();
        self.save_spaces();
        Ok(())
    }

    pub(crate) fn color_space(&mut self, id: &str, color: [u8; 3]) {
        if let Some(space) = self.spaces.list.iter_mut().find(|w| w.id == id) {
            space.color = color;
            self.save_spaces();
        }
    }

    /// Writes the drawing on screen to its file, parks its shells in `away` (they keep running and
    /// reaching each other over the bridge), and opens space `id`'s drawing from its file: what
    /// its agents drew meanwhile is there. Its IDEs save and close and its pages go (sync_ides,
    /// sync_portals) since their nodes left the scene; they come back with it.
    pub(crate) fn switch_space(&mut self, id: &str) -> Result<(), String> {
        self.load_spaces();
        let target = self.spaces.list.iter().find(|w| w.id == id).cloned().ok_or("Esse espaço não existe.")?;
        if target.id == self.spaces.current {
            return Ok(());
        }
        let base = self.autosave.clone().ok_or("Sem pasta de dados: o desenho de cada espaço não teria onde ficar.")?;
        if self.in_gesture() {
            self.cancel_gesture();
        }
        self.finish_text();
        // twice: the first may only merge what an agent wrote meanwhile (as on exit)
        for _ in 0..2 {
            if self.dirty {
                self.write_autosave();
            }
        }
        if self.dirty {
            return Err("O desenho deste espaço não pôde ser gravado; a troca foi cancelada.".into());
        }
        let leaving = self.spaces.current.clone();
        for e in &self.scene.elements {
            if let (Some(spec), true) = (&e.terminal, self.terminals.contains_key(&e.id)) {
                self.away.insert(e.id.clone(), (leaving.clone(), spec.clone()));
            }
        }
        if let Some(bridge) = &self.bridge {
            let nodes = self.agent_nodes();
            bridge.shared.lock().unwrap().away.insert(leaving.clone(), (self.space_name(&leaving), nodes));
        }
        let selected = self.selected.iter().filter_map(|i| self.scene.elements.get(*i)).map(|e| e.id.clone()).collect();
        self.space_views.insert(leaving, (self.zoom, self.pan, selected));
        self.reset_view();
        self.spaces.current = target.id.clone();
        self.autosave = Some(base.with_file_name(&target.scene));
        (self.scene, self.synced, self.synced_stamp, self.dirty) = (Scene::default(), Scene::default(), None, false);
        self.load_autosave();
        self.away.retain(|_, (space, _)| *space != target.id);
        if let Some(bridge) = &self.bridge {
            bridge.shared.lock().unwrap().away.remove(&target.id);
        }
        let (zoom, pan, selected) = self.space_views.remove(&target.id).unwrap_or((1.0, Vec2::ZERO, Vec::new()));
        (self.zoom, self.pan) = (zoom, pan);
        self.selected = selected.iter().filter_map(|id| self.scene.elements.iter().position(|e| &e.id == id)).collect();
        self.save_spaces();
        self.publish_nodes();
        Ok(())
    }

    /// What belonged to the drawing on screen and means nothing in another one.
    fn reset_view(&mut self) {
        self.undo.clear();
        self.redo.clear();
        self.selected.clear();
        (self.line_editor, self.multi, self.selected_point) = (None, None, None);
        (self.terminal_focus, self.terminal_elevated, self.leaving, self.dock, self.dock_drag) = (None, None, None, None, None);
        (self.ide_focus, self.card_focus, self.composer, self.connecting) = (None, None, None, None);
        (self.context, self.rename, self.links_popover, self.palette_links, self.note_color) = (None, None, None, None, None);
        (self.portal_url_edit, self.active_locked, self.tidy_last, self.glide, self.file_path) = (None, None, None, None, None);
        (self.terminal_dialog, self.portal_dialog, self.ide_dialog) = (None, None, None);
        self.reveal.clear();
        self.agent = None;
    }

    /// Takes space `id` off the list and ends its shells; its drawing stays on disk. The one on
    /// screen first gives way to another; the last one stays.
    pub(crate) fn remove_space(&mut self, id: &str) -> Result<String, String> {
        self.load_spaces();
        if self.spaces.list.len() <= 1 {
            return Err("O último espaço não pode ser removido.".into());
        }
        let space = self.spaces.list.iter().find(|w| w.id == id).cloned().ok_or("Esse espaço não existe.")?;
        if space.id == self.spaces.current {
            let other = self.spaces.list.iter().find(|w| w.id != id).map(|w| w.id.clone()).unwrap_or_default();
            self.switch_space(&other)?;
        }
        let ended: Vec<String> = self.away.iter().filter(|(_, (s, _))| *s == space.id).map(|(t, _)| t.clone()).collect();
        for terminal in &ended {
            self.terminals.remove(terminal);
            self.away.remove(terminal);
        }
        if let Some(bridge) = &self.bridge {
            bridge.shared.lock().unwrap().away.remove(&space.id);
        }
        self.space_views.remove(&space.id);
        self.spaces.list.retain(|w| w.id != space.id);
        self.save_spaces();
        let ended = match ended.len() {
            0 => "nenhum terminal rodava nele".to_string(),
            1 => "1 terminal encerrado".to_string(),
            n => format!("{n} terminais encerrados"),
        };
        Ok(format!("Espaço \"{}\" removido ({ended}). O desenho dele continua no disco, em {}.", space.name, space.scene))
    }

    /// The MCP's `workspaces`: list, switch, create, rename, color, remove.
    pub(crate) fn act_workspaces(&mut self, args: &Value) -> Result<String, String> {
        let text = |key: &str| args[key].as_str().unwrap_or_default().trim().to_string();
        self.load_spaces();
        match text("action").as_str() {
            "list" | "" => Ok(self.spaces_listing()),
            "switch" => {
                let id = self.space_named(&text("name"))?;
                self.switch_space(&id)?;
                self.show_canvas();
                Ok(format!("Espaço \"{}\" aberto.", self.space_name(&id)))
            }
            "create" => {
                let id = self.create_space(&text("name"), &text("folder"))?;
                self.prefs.folders.remember(&text("folder"));
                self.switch_space(&id)?;
                self.show_canvas();
                Ok(format!("Espaço \"{}\" criado e aberto.", self.space_name(&id)))
            }
            "rename" => {
                let id = self.space_named(&text("name"))?;
                self.rename_space(&id, &text("new_name"))?;
                Ok(format!("Espaço agora se chama \"{}\".", text("new_name")))
            }
            "color" => {
                let id = self.space_named(&text("name"))?;
                let [r, g, b, _] = parse_hex(&args["color"]).ok_or("Diga a cor em hexadecimal (#3d7ef0).")?;
                self.color_space(&id, [r, g, b]);
                Ok(format!("Cor do espaço \"{}\" trocada.", self.space_name(&id)))
            }
            "remove" => {
                let id = self.space_named(&text("name"))?;
                self.remove_space(&id)
            }
            other => Err(format!("ação desconhecida: {other}")),
        }
    }

    fn spaces_listing(&self) -> String {
        let counts = self.space_counts();
        let line = |w: &Workspace| {
            let open = if w.id == self.spaces.current { " (na tela)" } else { "" };
            let root = if w.root.is_empty() { String::new() } else { format!(" · {}", w.root) };
            let (running, needs) = counts.get(&w.id).copied().unwrap_or_default();
            let needs = if needs > 0 { format!(", {needs} precisam de você") } else { String::new() };
            format!("- {}{open}{root} · {running} agente(s) rodando{needs}", w.name)
        };
        self.spaces.list.iter().map(line).collect::<Vec<_>>().join("\n")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// An app on a drawing of its own in a fresh folder.
    fn app_in(name: &str) -> (DrawingApp, PathBuf) {
        let dir = std::env::temp_dir().join(format!("mago-espacos-{name}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        let mut app = DrawingApp { autosave: Some(dir.join("autosave.mago")), ..Default::default() };
        app.load_spaces();
        app.load_autosave();
        (app, dir)
    }

    fn shape(app: &mut DrawingApp, x: f32) -> String {
        let e = app.new_element(ElementKind::Rectangle { x, y: 0.0, width: 50.0, height: 50.0 });
        let id = e.id.clone();
        app.snapshot();
        app.scene.elements.push(e);
        id
    }

    #[test]
    fn without_the_file_there_is_the_main_space_on_todays_drawing() {
        let (app, dir) = app_in("principal");
        assert_eq!(app.spaces.list.len(), 1);
        assert_eq!((app.spaces.list[0].name.as_str(), app.spaces.list[0].scene.as_str()), ("Principal", "autosave.mago"));
        assert_eq!(app.autosave.as_deref(), Some(dir.join("autosave.mago").as_path()));
        assert!(!dir.join("espacos.json").exists(), "nothing written until something changes");
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn each_space_keeps_its_drawing_in_its_own_file() {
        let (mut app, dir) = app_in("trocar");
        let a = shape(&mut app, 0.0);
        app.zoom = 2.0;
        app.selected = vec![0];
        let b_space = app.create_space("Loja", "").unwrap();
        app.switch_space(&b_space).unwrap();
        assert!(app.scene.elements.is_empty() && app.undo.is_empty() && app.zoom == 1.0);
        let b = shape(&mut app, 100.0);
        app.switch_space("principal").unwrap();
        assert_eq!(app.scene.elements.iter().map(|e| e.id.clone()).collect::<Vec<_>>(), vec![a.clone()]);
        assert_eq!((app.zoom, app.selected.clone()), (2.0, vec![0]), "the camera and the selection are the space's");
        let read = |file: &str| read_scene_file(&dir.join(file)).unwrap().0.elements.into_iter().map(|e| e.id).collect::<Vec<_>>();
        let b_file = app.spaces.list[1].scene.clone();
        assert_eq!((read("autosave.mago"), read(&b_file)), (vec![a], vec![b]));
        // the list and the space on screen survive a restart
        let (saved, bad) = read_spaces(&dir.join("autosave.mago"));
        assert!(bad.is_none());
        assert_eq!((saved.list.len(), saved.current.as_str()), (2, "principal"));
        assert_eq!(current_scene(&dir.join("autosave.mago")), dir.join("autosave.mago"));
        app.switch_space(&b_space).unwrap();
        assert_eq!(current_scene(&dir.join("autosave.mago")), dir.join(&b_file), "the chat's MCP draws on the space on screen");
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn what_an_agent_wrote_off_view_shows_on_return() {
        let (mut app, dir) = app_in("fora");
        let other = app.create_space("Outro", "").unwrap();
        let file = dir.join(&app.spaces.list[1].scene);
        // an agent of that space draws through its MCP while it is off view
        let mut external = Scene { agent: Some("Claude Code".into()), ..Scene::default() };
        external.elements.push(app.new_element(ElementKind::Text { x: 0.0, y: 0.0, value: "feito lá".into() }));
        write_scene_file(&file, &external).unwrap();
        app.switch_space(&other).unwrap();
        assert!(app.scene.elements.iter().any(|e| matches!(&e.kind, ElementKind::Text { value, .. } if value == "feito lá")));
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_shell_keeps_running_off_view_and_ends_with_its_space() {
        let ctx = Context::default();
        let (mut app, dir) = app_in("sessao");
        let program = if cfg!(windows) { ("powershell", &["-NoProfile", "-Command", "Start-Sleep 30"][..]) } else { ("sh", &["-c", "sleep 30"][..]) };
        let terminal = |app: &mut DrawingApp| {
            let mut e = app.new_element(ElementKind::Rectangle { x: 0.0, y: 0.0, width: 400.0, height: 300.0 });
            e.terminal = Some(TerminalSpec { name: "Revisor".into(), ..Default::default() });
            let id = e.id.clone();
            app.scene.elements.push(e);
            app.dirty = true;
            let mut session = TerminalSession::spawn("", "", (40, 5), None, vec![], Some(program), 10_000).unwrap();
            session.started_ms = 1; // marks this very shell
            app.terminals.insert(id.clone(), Ok(session));
            id
        };
        let id = terminal(&mut app);
        let same = |app: &DrawingApp, id: &str| app.terminals.get(id).is_some_and(|s| s.as_ref().is_ok_and(|s| s.started_ms == 1 && !s.exited()));
        let other = app.create_space("Outro", "").unwrap();
        app.switch_space(&other).unwrap();
        app.sync_terminals(&ctx);
        assert!(same(&app, &id), "its shell runs off view");
        assert_eq!(app.agent_rows().iter().find(|r| r.id == id).map(|r| r.space.as_str()), Some("principal"));
        assert_eq!(app.space_counts().get("principal"), Some(&(1, 0)));
        app.switch_space("principal").unwrap();
        app.sync_terminals(&ctx);
        assert!(same(&app, &id) && app.away.is_empty(), "and is there on return");
        // a space removed ends its shells; its drawing stays
        app.switch_space(&other).unwrap();
        let theirs = terminal(&mut app);
        let file = dir.join(&app.spaces.list[1].scene);
        app.switch_space("principal").unwrap();
        assert!(app.remove_space(&other).unwrap().contains("1 terminal encerrado"));
        assert!(!app.terminals.contains_key(&theirs) && same(&app, &id));
        assert!(file.exists(), "the drawing stays on disk");
        assert_eq!(app.spaces.list.len(), 1);
        assert_eq!(app.remove_space("principal"), Err("O último espaço não pode ser removido.".into()));
        app.terminals.clear();
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn an_unreadable_list_goes_aside() {
        let dir = std::env::temp_dir().join(format!("mago-espacos-ilegivel-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        fs::write(dir.join("espacos.json"), "{ não é json").unwrap();
        let mut app = DrawingApp { autosave: Some(dir.join("autosave.mago")), ..Default::default() };
        app.load_spaces();
        assert_eq!(app.spaces.list.len(), 1);
        assert!(app.status.contains("espacos.json"), "{}", app.status);
        assert!(!dir.join("espacos.json").exists());
        assert!(fs::read_dir(&dir).unwrap().flatten().any(|f| f.file_name().to_string_lossy().starts_with("espacos.json.bad-")));
        // a drawing named out of the folder is no list either
        fs::write(dir.join("espacos.json"), r#"{"espacos":[{"id":"x","nome":"X","cena":"../fora.mago"}],"atual":"x"}"#).unwrap();
        assert_eq!(read_spaces(&dir.join("autosave.mago")).0.list[0].id, MAIN);
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn the_mcp_drives_the_workspaces() {
        let (mut app, dir) = app_in("mcp");
        app.chat_open = true;
        let folder = dir.to_string_lossy().to_string();
        let act = |app: &mut DrawingApp, args: Value| app.act("workspaces", &args);
        assert!(act(&mut app, json!({ "action": "create", "name": "Site", "folder": folder })).unwrap().contains("Site"));
        assert_eq!(app.current_space().map(|w| w.name.as_str()), Some("Site"));
        assert!(!app.chat_open, "the canvas shows it");
        assert_eq!(app.new_folder(), folder, "new terminals and IDEs start in its folder");
        assert!(act(&mut app, json!({ "action": "create", "name": "site" })).is_err(), "names are unique");
        assert!(act(&mut app, json!({ "action": "create", "name": "X", "folder": dir.join("nada").to_string_lossy() })).is_err());
        act(&mut app, json!({ "action": "rename", "name": "site", "new_name": "Loja" })).unwrap();
        act(&mut app, json!({ "action": "color", "name": "Loja", "color": "#e03131" })).unwrap();
        assert_eq!(app.current_space().map(|w| (w.name.as_str(), w.color)), Some(("Loja", [0xe0, 0x31, 0x31])));
        let list = act(&mut app, json!({ "action": "list" })).unwrap();
        assert!(list.contains("Principal") && list.contains("Loja (na tela)"), "{list}");
        act(&mut app, json!({ "action": "switch", "name": "principal" })).unwrap();
        assert_eq!(app.spaces.current, "principal");
        assert!(act(&mut app, json!({ "action": "remove", "name": "Loja" })).unwrap().contains("continua no disco"));
        assert!(act(&mut app, json!({ "action": "remove", "name": "Principal" })).is_err());
        assert!(act(&mut app, json!({ "action": "voar" })).is_err());
        // the MCP lists it as one of the app's tools
        let tools = crate::mcp::handle(&json!({ "jsonrpc": "2.0", "id": 1, "method": "tools/list" })).unwrap();
        assert!(tools["result"]["tools"].as_array().unwrap().iter().any(|t| t["name"] == "workspaces"));
        let _ = fs::remove_dir_all(&dir);
    }
}
