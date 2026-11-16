//! Floors (andares): a space has its ground floor ("Térreo", the drawing in its `scene`) and floors
//! above it, each a whole drawing of its own (name, color, camera, nodes) in `andar-<id>.mago`
//! beside `espacos.json`. A floor is a scene one more, switched like a space is (workspaces.rs
//! `switch_scope`): the shells of the floors off view keep running, listed in `away` under the
//! floor's scope key.

pub(crate) mod fx;
pub(crate) mod pill;

use crate::app::workspaces::{Workspace, space_colors};
use crate::prelude::*;

/// The ground floor's name.
pub(crate) const GROUND: &str = "Térreo";

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
pub(crate) struct Floor {
    pub(crate) id: String,
    #[serde(rename = "nome")]
    pub(crate) name: String,
    #[serde(rename = "cor", default)]
    pub(crate) color: [u8; 3],
    /// Its drawing's file name, beside `espacos.json`.
    #[serde(rename = "cena")]
    pub(crate) scene: String,
}

/// What `away` calls a drawing's shells: the space's id, or "space#floor" once the space has floors
/// ("space#" for its ground floor), so a floor of the space on view is told from its ground.
pub(crate) fn scope_key(space: &Workspace, floor: &str) -> String {
    if floor.is_empty() && space.floors.is_empty() { space.id.clone() } else { format!("{}#{floor}", space.id) }
}

impl DrawingApp {
    /// The scope key of the drawing on screen.
    pub(crate) fn scope_key(&self) -> String {
        self.current_space().map_or(String::new(), |workspace| scope_key(workspace, &workspace.floor))
    }

    /// The floors above the ground of the space on screen.
    pub(crate) fn floors(&self) -> &[Floor] {
        self.current_space().map_or(&[], |workspace| &workspace.floors)
    }

    /// The name of the floor on screen.
    pub(crate) fn floor_name(&self) -> String {
        let id = self.current_space().map_or("", |workspace| &workspace.floor);
        self.floors().iter().find(|floor| floor.id == id).map_or(GROUND.into(), |floor| floor.name.clone())
    }

    /// How many things a floor of `space` holds (the drawing on screen counts live, the others read their file).
    pub(crate) fn floor_items(&self, space: &str, floor: &str) -> usize {
        let Some(workspace) = self.spaces.list.iter().find(|workspace| workspace.id == space) else { return 0 };
        let items = |scene: &Scene| scene.elements.iter().filter(|element| !element.id.ends_with("-label")).count();
        if self.spaces.current == space && workspace.floor == floor {
            return items(&self.scene);
        }
        let file =
            if floor.is_empty() { Some(&workspace.scene) } else { workspace.floors.iter().find(|stored| stored.id == floor).map(|stored| &stored.scene) };
        let path = self.autosave.as_ref().zip(file).map(|(base, file)| base.with_file_name(file));
        path.and_then(|path| read_scene_file(&path).ok()).map_or(0, |(scene, _)| items(&scene))
    }

    /// A floor of space `space` by id or name (any case); "" and "Térreo" are the ground floor, given as "".
    pub(crate) fn floor_named(&self, space: &str, name: &str) -> Result<String, String> {
        let wanted = name.trim().to_lowercase();
        if ["", "térreo", "terreo", "ground"].contains(&wanted.as_str()) {
            return Ok(String::new());
        }
        let floors = self.spaces.list.iter().find(|workspace| workspace.id == space).map_or(&[][..], |workspace| &workspace.floors[..]);
        let found = floors.iter().find(|floor| floor.id == name.trim() || floor.name.to_lowercase() == wanted);
        found.map(|floor| floor.id.clone()).ok_or_else(|| format!("Nenhum andar se chama {}.", name.trim()))
    }

    fn floor_taken(&self, space: &str, id: &str, name: &str) -> bool {
        let wanted = name.to_lowercase();
        wanted == GROUND.to_lowercase()
            || self
                .spaces
                .list
                .iter()
                .filter(|workspace| workspace.id == space)
                .flat_map(|workspace| &workspace.floors)
                .any(|floor| floor.id != id && floor.name.to_lowercase() == wanted)
    }

    /// A new floor above the others in the space on screen (not opened). `copy_ground` fills it with the
    /// ground floor's layout, as fresh nodes (a terminal in it starts a shell of its own once opened).
    pub(crate) fn create_floor(&mut self, name: &str, copy_ground: bool) -> Result<String, String> {
        self.load_spaces();
        let name = name.trim();
        if name.is_empty() {
            return Err("Diga o nome do andar.".into());
        }
        let space = self.current_space().cloned().ok_or("Sem espaço de trabalho.")?;
        if self.floor_taken(&space.id, "", name) {
            return Err(format!("Já existe um andar chamado {name}."));
        }
        let base = self.autosave.clone().ok_or("Sem pasta de dados: o desenho de cada andar não teria onde ficar.")?;
        let (id, colors) = (new_id(), space_colors());
        let file = format!("andar-{}.mago", portal_profile_name(&id));
        if copy_ground {
            let ground = if space.floor.is_empty() {
                self.scene.clone()
            } else {
                read_scene_file(&base.with_file_name(&space.scene)).map(|(scene, _)| scene).unwrap_or_default()
            };
            let (elements, groups) = Self::fresh_copies_in(&ground.elements, Vec2::ZERO, &ground.groups);
            let copy = Scene { elements, groups, background: ground.background, ..Scene::default() };
            write_scene_file(&base.with_file_name(&file), &copy).map_err(|error| format!("Não foi possível criar o andar: {error}"))?;
        }
        let color = colors[(space.floors.len() + 1) % colors.len()];
        if let Some(workspace) = self.spaces.list.iter_mut().find(|workspace| workspace.id == space.id) {
            workspace.floors.push(Floor { id: id.clone(), name: name.into(), color, scene: file });
        }
        self.save_spaces();
        Ok(id)
    }

    /// Shows floor `id` ("" the ground) of the space on screen.
    pub(crate) fn switch_floor(&mut self, id: &str) -> Result<(), String> {
        self.load_spaces();
        let space = self.spaces.current.clone();
        self.switch_scope(&space, id)
    }

    pub(crate) fn rename_floor(&mut self, space: &str, id: &str, name: &str) -> Result<(), String> {
        let name = name.trim();
        if name.is_empty() {
            return Err("Diga o nome novo.".into());
        }
        if self.floor_taken(space, id, name) {
            return Err(format!("Já existe um andar chamado {name}."));
        }
        let floor = self
            .spaces
            .list
            .iter_mut()
            .find(|workspace| workspace.id == space)
            .and_then(|workspace| workspace.floors.iter_mut().find(|floor| floor.id == id))
            .ok_or("Esse andar não existe.")?;
        floor.name = name.into();
        self.save_spaces();
        Ok(())
    }

    /// Takes floor `id` away with everything in it: its drawing is deleted and its shells end. Deleting the
    /// one on screen goes to the ground floor first.
    pub(crate) fn delete_floor(&mut self, space: &str, id: &str) -> Result<String, String> {
        self.load_spaces();
        let found =
            self.spaces.list.iter().find(|workspace| workspace.id == space).and_then(|workspace| workspace.floors.iter().find(|floor| floor.id == id)).cloned();
        let floor = found.ok_or("Esse andar não existe.")?;
        if self.spaces.current == space && self.current_space().is_some_and(|workspace| workspace.floor == id) {
            self.switch_scope(space, "")?;
        }
        let key = format!("{space}#{id}");
        let ended: Vec<String> = self.away.iter().filter(|(_, (scope, _))| *scope == key).map(|(terminal, _)| terminal.clone()).collect();
        for terminal in &ended {
            self.terminals.remove(terminal);
            self.away.remove(terminal);
        }
        if let Some(bridge) = &self.bridge {
            locked(&bridge.shared).away.remove(&key);
        }
        self.space_views.remove(&key);
        if let Some(workspace) = self.spaces.list.iter_mut().find(|workspace| workspace.id == space) {
            workspace.floors.retain(|floor| floor.id != id);
            if workspace.floor == id {
                workspace.floor.clear();
            }
        }
        self.save_spaces();
        if let Some(base) = &self.autosave {
            let _ = fs::remove_file(base.with_file_name(&floor.scene));
        }
        Ok(format!("Andar \"{}\" apagado ({} terminal(is) encerrado(s)).", floor.name, ended.len()))
    }

    /// Puts floor `id` at `position` among the floors above the ground (1 is the one right above it).
    pub(crate) fn move_floor(&mut self, space: &str, id: &str, position: usize) -> Result<(), String> {
        let floors =
            self.spaces.list.iter_mut().find(|workspace| workspace.id == space).map(|workspace| &mut workspace.floors).ok_or("Esse espaço não existe.")?;
        let from = floors.iter().position(|floor| floor.id == id).ok_or("Esse andar não existe.")?;
        let floor = floors.remove(from);
        floors.insert(position.saturating_sub(1).min(floors.len()), floor);
        self.save_spaces();
        Ok(())
    }

    /// The MCP's `floors`: list, switch, create, rename, delete, move (all in the space on screen).
    pub(crate) fn act_floors(&mut self, args: &Value) -> Result<String, String> {
        let text = |key: &str| args[key].as_str().unwrap_or_default().trim().to_string();
        self.load_spaces();
        let space = self.spaces.current.clone();
        let on = |app: &Self, id: &str| if app.current_space().is_some_and(|workspace| workspace.floor == id) { " (na tela)" } else { "" };
        let named = |app: &Self| match app.floor_named(&space, &text("name"))? {
            id if id.is_empty() => Err("O Térreo não pode ser mudado.".to_string()),
            id => Ok(id),
        };
        match text("action").as_str() {
            "list" | "" => {
                let line = |name: &str, id: &str| format!("- {name}{} · {} item(ns)", on(self, id), self.floor_items(&space, id));
                let above = self.floors().iter().rev().map(|floor| line(&floor.name, &floor.id));
                Ok(above.chain([line(GROUND, "")]).collect::<Vec<_>>().join("\n"))
            }
            "switch" => {
                let id = self.floor_named(&space, &text("name"))?;
                self.switch_floor(&id)?;
                self.show_canvas();
                Ok(format!("Andar \"{}\" aberto.", self.floor_name()))
            }
            "create" => {
                let id = self.create_floor(&text("name"), args["clone_ground"].as_bool().unwrap_or(false))?;
                self.switch_floor(&id)?;
                self.show_canvas();
                Ok(format!("Andar \"{}\" criado e aberto.", self.floor_name()))
            }
            "rename" => {
                let id = named(self)?;
                self.rename_floor(&space, &id, &text("new_name"))?;
                Ok(format!("Andar agora se chama \"{}\".", text("new_name")))
            }
            "delete" => {
                let id = named(self)?;
                self.delete_floor(&space, &id)
            }
            "move" => {
                let id = named(self)?;
                self.move_floor(&space, &id, args["position"].as_u64().ok_or("Diga a posição (1 é o andar logo acima do Térreo).")? as usize)?;
                Ok("Andar movido.".into())
            }
            other => Err(format!("ação desconhecida: {other}")),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::workspaces::{current_scene, read_spaces};

    fn app_in(name: &str) -> (DrawingApp, PathBuf) {
        let directory = std::env::temp_dir().join(format!("mago-andares-{name}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&directory);
        fs::create_dir_all(&directory).unwrap();
        // today's drawing already there: with none, it is a first run and there is no space yet
        fs::write(directory.join("autosave.mago"), r#"{"version":1,"elements":[]}"#).unwrap();
        let mut app = DrawingApp { autosave: Some(directory.join("autosave.mago")), ..Default::default() };
        app.load_spaces();
        app.load_autosave();
        (app, directory)
    }

    fn shape(app: &mut DrawingApp, x: f32) -> String {
        let element = app.new_element(ElementKind::Rectangle { x, y: 0.0, width: 50.0, height: 50.0 });
        let id = element.id.clone();
        app.snapshot();
        app.scene.elements.push(element);
        app.dirty = true;
        id
    }

    fn ids(app: &DrawingApp) -> Vec<String> {
        app.scene.elements.iter().map(|element| element.id.clone()).collect()
    }

    #[test]
    fn each_floor_keeps_its_nodes_and_camera() {
        let (mut app, directory) = app_in("trocar");
        let ground = shape(&mut app, 0.0);
        app.zoom = 2.0;
        let up = app.create_floor("Obra", false).unwrap();
        assert_eq!(app.floor_name(), GROUND, "created, not opened");
        app.switch_floor(&up).unwrap();
        assert_eq!((app.floor_name().as_str(), app.scene.elements.len(), app.zoom), ("Obra", 0, 1.0));
        let mine = shape(&mut app, 100.0);
        app.zoom = 3.0;
        app.switch_floor("").unwrap();
        assert_eq!((ids(&app), app.zoom), (vec![ground.clone()], 2.0));
        app.switch_floor(&up).unwrap();
        assert_eq!((ids(&app), app.zoom), (vec![mine], 3.0));
        // the space keeps the floor it was left on
        let other = app.create_space("Loja", "").unwrap();
        app.switch_space(&other).unwrap();
        app.switch_space("principal").unwrap();
        assert_eq!(app.floor_name(), "Obra");
        assert_eq!(app.spaces.list.len(), 2);
        // a floor is a scene of its own on disk, and it all survives a restart
        let file = app.spaces.list[0].floors[0].scene.clone();
        app.save_on_exit();
        assert_eq!(read_scene_file(&directory.join(&file)).unwrap().0.elements.len(), 1);
        let (saved, bad) = read_spaces(&directory.join("autosave.mago"));
        assert!(bad.is_none());
        assert_eq!((saved.list[0].floors.len(), saved.list[0].floor.as_str()), (1, up.as_str()));
        assert_eq!(current_scene(&directory.join("autosave.mago")), directory.join(&file), "the chat's MCP draws on the floor on screen");
        let _ = fs::remove_dir_all(&directory);
    }

    #[test]
    fn an_old_scene_opens_the_same_and_a_new_floor_can_clone_the_ground() {
        let (mut app, directory) = app_in("clonar");
        assert!(app.floors().is_empty() && app.scope_key() == "principal", "no floors: the space as it always was");
        let mut note = app.new_element(ElementKind::Rectangle { x: 0.0, y: 0.0, width: 80.0, height: 60.0 });
        note.terminal = Some(TerminalSpec { name: "Revisor".into(), ..Default::default() });
        app.scene.elements.push(note);
        app.dirty = true;
        let original = ids(&app);
        let copy = app.create_floor("Cópia", true).unwrap();
        app.switch_floor(&copy).unwrap();
        assert_eq!(app.scene.elements.len(), 1);
        assert_ne!(ids(&app), original, "fresh ids: the shells are not shared");
        assert_eq!(app.scene.elements[0].terminal.as_ref().map(|terminal| terminal.name.as_str()), Some("Revisor"));
        assert_eq!(app.scope_key(), format!("principal#{copy}"));
        // cloning from a floor takes the ground's layout, not the floor's
        shape(&mut app, 300.0);
        let again = app.create_floor("De novo", true).unwrap();
        app.switch_floor(&again).unwrap();
        assert_eq!(app.scene.elements.len(), 1);
        let _ = fs::remove_dir_all(&directory);
    }

    #[test]
    fn rename_move_and_delete_floors() {
        let (mut app, directory) = app_in("gerir");
        let (first, second, third) = (app.create_floor("A", false).unwrap(), app.create_floor("B", false).unwrap(), app.create_floor("C", false).unwrap());
        assert!(app.create_floor("a", false).is_err() && app.create_floor("térreo", false).is_err() && app.create_floor(" ", false).is_err());
        app.rename_floor("principal", &first, "Alfa").unwrap();
        assert!(app.rename_floor("principal", &second, "alfa").is_err());
        app.move_floor("principal", &third, 1).unwrap();
        let order = |app: &DrawingApp| app.floors().iter().map(|floor| floor.name.clone()).collect::<Vec<_>>();
        assert_eq!(order(&app), ["C", "Alfa", "B"]);
        // deleting the floor on screen goes down to the ground and takes its drawing off the disk
        app.switch_floor(&second).unwrap();
        shape(&mut app, 0.0);
        let file = directory.join(&app.floors().iter().find(|floor| floor.id == second).unwrap().scene);
        app.write_autosave();
        assert!(file.exists());
        assert!(app.delete_floor("principal", &second).unwrap().contains("apagado"));
        assert!(!file.exists() && app.floor_name() == GROUND && order(&app) == ["C", "Alfa"]);
        assert!(app.delete_floor("principal", &second).is_err());
        let _ = fs::remove_dir_all(&directory);
    }

    #[test]
    fn a_shell_keeps_running_on_a_floor_off_view_and_ends_with_the_floor() {
        let ctx = Context::default();
        let (mut app, directory) = app_in("sessao");
        let program = if cfg!(windows) { ("powershell", &["-NoProfile", "-Command", "Start-Sleep 30"][..]) } else { ("sh", &["-c", "sleep 30"][..]) };
        let mut element = app.new_element(ElementKind::Rectangle { x: 0.0, y: 0.0, width: 400.0, height: 300.0 });
        element.terminal = Some(TerminalSpec { name: "Revisor".into(), ..Default::default() });
        let id = element.id.clone();
        let up = app.create_floor("Em cima", false).unwrap();
        app.scene.elements.push(element);
        app.dirty = true;
        let mut session = TerminalSession::spawn("", "", (40, 5), None, vec![], Some(program), 10_000).unwrap();
        session.started_ms = 1; // marks this very shell
        app.terminals.insert(id.clone(), Ok(session));
        // working by its session's word, not by the screen's last 1.5 s (a slow machine passes them before the count)
        app.agent_sessions.insert(id.clone(), crate::transcript::Session { status: "busy".into(), status_at: crate::model::now_ms(), ..Default::default() });
        let alive =
            |app: &DrawingApp| app.terminals.get(&id).is_some_and(|session| session.as_ref().is_ok_and(|session| session.started_ms == 1 && !session.exited()));
        app.switch_floor(&up).unwrap();
        app.sync_terminals(&ctx);
        assert!(alive(&app), "the ground's shell runs while another floor is shown");
        assert_eq!(app.agent_rows().iter().find(|row| row.id == id).map(|row| row.space.as_str()), Some("principal#"));
        assert_eq!(app.space_counts().get("principal"), Some(&(1, 0)), "the space counts the agents of all its floors");
        app.switch_floor("").unwrap();
        app.sync_terminals(&ctx);
        assert!(alive(&app) && app.away.is_empty(), "and is there on return");
        // the floor's own shell ends when the floor is deleted
        app.switch_floor(&up).unwrap();
        let mut theirs = app.new_element(ElementKind::Rectangle { x: 0.0, y: 0.0, width: 400.0, height: 300.0 });
        theirs.terminal = Some(TerminalSpec { name: "Dentro".into(), ..Default::default() });
        let theirs_id = theirs.id.clone();
        app.scene.elements.push(theirs);
        app.dirty = true;
        app.terminals.insert(theirs_id.clone(), Ok(TerminalSession::spawn("", "", (40, 5), None, vec![], Some(program), 10_000).unwrap()));
        app.switch_floor("").unwrap();
        assert!(app.away.contains_key(&theirs_id));
        assert!(app.delete_floor("principal", &up).unwrap().contains("1 terminal"));
        assert!(!app.terminals.contains_key(&theirs_id) && alive(&app));
        app.terminals.clear();
        let _ = fs::remove_dir_all(&directory);
    }

    #[test]
    fn the_mcp_drives_the_floors() {
        let (mut app, directory) = app_in("mcp");
        app.chat_open = true;
        let act = |app: &mut DrawingApp, args: Value| app.act("floors", &args);
        shape(&mut app, 0.0);
        assert!(act(&mut app, json!({ "action": "create", "name": "Obra", "clone_ground": true })).unwrap().contains("Obra"));
        assert!(!app.chat_open && app.floor_name() == "Obra" && app.scene.elements.len() == 1);
        assert!(act(&mut app, json!({ "action": "create", "name": "obra" })).is_err());
        act(&mut app, json!({ "action": "rename", "name": "obra", "new_name": "Casa" })).unwrap();
        act(&mut app, json!({ "action": "create", "name": "Telhado" })).unwrap();
        act(&mut app, json!({ "action": "move", "name": "Telhado", "position": 1 })).unwrap();
        let list = act(&mut app, json!({ "action": "list" })).unwrap();
        assert!(list.starts_with("- Casa") && list.contains("- Telhado (na tela) · 0 item(ns)") && list.contains("- Térreo · 1 item(ns)"), "{list}");
        act(&mut app, json!({ "action": "switch", "name": "Térreo" })).unwrap();
        assert_eq!(app.floor_name(), GROUND);
        assert!(act(&mut app, json!({ "action": "rename", "name": "Térreo", "new_name": "X" })).is_err());
        assert!(act(&mut app, json!({ "action": "delete", "name": "Casa" })).unwrap().contains("apagado"));
        assert!(act(&mut app, json!({ "action": "voar" })).is_err());
        let tools = crate::mcp::handle(&json!({ "jsonrpc": "2.0", "id": 1, "method": "tools/list" })).unwrap();
        assert!(tools["result"]["tools"].as_array().unwrap().iter().any(|tool| tool["name"] == "floors"));
        let _ = fs::remove_dir_all(&directory);
    }
}
