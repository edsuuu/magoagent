//! The sidebar's groups (Novo grupo) and the rest of a space's menu: Fixar, Mover para, Duplicar,
//! Copiar caminho, Exportar, Importar, Descarregar. A group is a named, collapsible heading in the
//! sidebar; a space sits in one (`Workspace::group`) or in none, and the list's order is the order
//! inside it. Deleting a group only frees its spaces. Saved in `espacos.json` (workspaces.rs).

use crate::app::workspaces::{SpaceGroup, Workspace, space_colors};
use crate::prelude::*;
use std::path::Path;

/// What an exported space's file says it is.
const EXPORT_FORMAT: &str = "espaco-de-trabalho";

impl DrawingApp {
    /// A group by id or name (any case).
    pub(crate) fn group_named(&self, name: &str) -> Result<String, String> {
        let wanted = name.trim().to_lowercase();
        let found = self.spaces.groups.iter().find(|g| g.id == name.trim() || g.name.to_lowercase() == wanted);
        found.map(|g| g.id.clone()).ok_or_else(|| format!("Nenhum grupo se chama {name}."))
    }

    fn group_name_free(&self, id: &str, name: &str) -> Result<(), String> {
        if name.is_empty() {
            return Err("Diga o nome do grupo.".into());
        }
        match self.spaces.groups.iter().any(|g| g.id != id && g.name.to_lowercase() == name.to_lowercase()) {
            true => Err(format!("Já existe um grupo chamado {name}.")),
            false => Ok(()),
        }
    }

    pub(crate) fn create_group(&mut self, name: &str) -> Result<String, String> {
        self.load_spaces();
        let name = name.trim();
        self.group_name_free("", name)?;
        let id = new_id();
        self.spaces.groups.push(SpaceGroup { id: id.clone(), name: name.into(), open: true });
        self.save_spaces();
        Ok(id)
    }

    pub(crate) fn rename_group(&mut self, id: &str, name: &str) -> Result<(), String> {
        let name = name.trim();
        self.group_name_free(id, name)?;
        let group = self.spaces.groups.iter_mut().find(|g| g.id == id).ok_or("Esse grupo não existe.")?;
        group.name = name.into();
        self.save_spaces();
        Ok(())
    }

    /// Recolher or Expandir.
    pub(crate) fn toggle_group(&mut self, id: &str) {
        if let Some(group) = self.spaces.groups.iter_mut().find(|g| g.id == id) {
            group.open = !group.open;
            self.save_spaces();
        }
    }

    /// Apagar Grupo: the spaces in it become ungrouped; none is removed.
    pub(crate) fn remove_group(&mut self, id: &str) -> Result<String, String> {
        let group = self.spaces.groups.iter().find(|g| g.id == id).cloned().ok_or("Esse grupo não existe.")?;
        self.spaces.list.iter_mut().filter(|w| w.group == id).for_each(|w| w.group.clear());
        self.spaces.groups.retain(|g| g.id != id);
        self.save_spaces();
        Ok(format!("Grupo \"{}\" apagado; os espaços dele ficaram sem grupo.", group.name))
    }

    /// Mover para: into group `group`, or out of any (None). It goes to the end of that group.
    pub(crate) fn move_space(&mut self, id: &str, group: Option<&str>) -> Result<(), String> {
        if let Some(g) = group.filter(|g| !self.spaces.groups.iter().any(|x| x.id == *g)) {
            return Err(format!("O grupo {g} não existe."));
        }
        let at = self.spaces.list.iter().position(|w| w.id == id).ok_or("Esse espaço não existe.")?;
        let mut space = self.spaces.list.remove(at);
        space.group = group.unwrap_or_default().into();
        // after the last space of that group (or of the list), so it lands at the group's end
        let end = self.spaces.list.iter().rposition(|w| w.group == space.group).map_or(self.spaces.list.len(), |i| i + 1);
        self.spaces.list.insert(end, space);
        // a group it joins shows it
        if let Some(g) = self.spaces.groups.iter_mut().find(|x| Some(x.id.as_str()) == group) {
            g.open = true;
        }
        self.save_spaces();
        Ok(())
    }

    /// Fixar or Desfixar: whether it is pinned now.
    pub(crate) fn toggle_pin(&mut self, id: &str) -> Result<bool, String> {
        let space = self.spaces.list.iter_mut().find(|w| w.id == id).ok_or("Esse espaço não existe.")?;
        space.pinned = !space.pinned;
        let pinned = space.pinned;
        self.save_spaces();
        Ok(pinned)
    }

    /// The file of space `id`'s drawing.
    fn space_file(&self, id: &str) -> Result<PathBuf, String> {
        let space = self.spaces.list.iter().find(|w| w.id == id).ok_or("Esse espaço não existe.")?;
        let base = self.autosave.as_ref().ok_or("Sem pasta de dados: o desenho de cada espaço não teria onde ficar.")?;
        Ok(base.with_file_name(&space.scene))
    }

    /// Space `id`'s drawing as it is now: the one on screen is written first.
    fn space_scene(&mut self, id: &str) -> Result<Scene, String> {
        let file = self.space_file(id)?;
        if id == self.spaces.current {
            // twice: the first may only merge what an agent wrote meanwhile (as on a switch)
            for _ in 0..2 {
                if self.dirty {
                    self.write_autosave();
                }
            }
            if self.dirty {
                return Err("O desenho deste espaço não pôde ser gravado.".into());
            }
        }
        match read_scene_file(&file) {
            Ok((scene, _)) => Ok(scene),
            Err(SceneError::Missing) => Ok(Scene::default()),
            Err(_) => Err("O desenho deste espaço não pôde ser lido.".into()),
        }
    }

    /// `name`, or `name 2`, `name 3`... when a space already has it.
    fn free_space_name(&self, name: &str) -> String {
        let taken = |n: &str| self.spaces.list.iter().any(|w| w.name.to_lowercase() == n.to_lowercase());
        (1..).map(|i| if i == 1 { name.to_string() } else { format!("{name} {i}") }).find(|n| !taken(n)).unwrap_or_default()
    }

    /// A space made of `scene` (fresh element ids, so nothing running is shared), after `after` in the list.
    fn add_space_from(&mut self, name: &str, root: &str, color: [u8; 3], group: &str, scene: &Scene, after: Option<&str>) -> Result<String, String> {
        let id = new_id();
        let file = format!("espaco-{}.mago", portal_profile_name(&id));
        let base = self.autosave.as_ref().ok_or("Sem pasta de dados: o desenho de cada espaço não teria onde ficar.")?;
        let scene = Scene { elements: Self::fresh_copies(&scene.elements, Vec2::ZERO), ..scene.clone() };
        write_scene_file(&base.with_file_name(&file), &scene).map_err(|e| format!("Não foi possível gravar o desenho: {e}"))?;
        let space =
            Workspace { id: id.clone(), name: self.free_space_name(name), root: root.into(), scene: file, color, group: group.into(), ..Default::default() };
        let at = after.and_then(|a| self.spaces.list.iter().position(|w| w.id == a)).map_or(self.spaces.list.len(), |i| i + 1);
        self.spaces.list.insert(at, space);
        self.save_spaces();
        Ok(id)
    }

    /// Duplicar: a copy of the space and its drawing, beside it, named "<name> cópia".
    pub(crate) fn duplicate_space(&mut self, id: &str) -> Result<String, String> {
        let scene = self.space_scene(id)?;
        let w = self.spaces.list.iter().find(|w| w.id == id).cloned().ok_or("Esse espaço não existe.")?;
        self.add_space_from(&format!("{} cópia", w.name), &w.root, w.color, &w.group, &scene, Some(id))
    }

    /// Exportar: the space's name, folder, color and drawing in one file at `path`.
    pub(crate) fn export_space(&mut self, id: &str, path: &Path) -> Result<String, String> {
        let scene = self.space_scene(id)?;
        let w = self.spaces.list.iter().find(|w| w.id == id).cloned().ok_or("Esse espaço não existe.")?;
        let file = json!({ "formato": EXPORT_FORMAT, "nome": w.name, "raiz": w.root, "cor": w.color, "desenho": scene });
        let text = serde_json::to_string_pretty(&file).map_err(|e| e.to_string())?;
        fs::write(path, text).map_err(|e| format!("Falha ao exportar o espaço de trabalho: {e}"))?;
        Ok(format!("Espaço \"{}\" exportado para {}.", w.name, path.display()))
    }

    /// Importar: a new space from an exported file. Its terminals and portals come in unloaded (Recarregar
    /// starts them), and a folder this machine lacks is dropped.
    pub(crate) fn import_space(&mut self, path: &Path) -> Result<String, String> {
        self.load_spaces();
        let bad = || "Não foi possível importar o espaço de trabalho. O arquivo pode estar danificado ou não ser um espaço de trabalho.".to_string();
        let file: Value = serde_json::from_slice(&fs::read(path).map_err(|_| bad())?).map_err(|_| bad())?;
        if file["formato"] != EXPORT_FORMAT {
            return Err(bad());
        }
        let mut scene = parse_scene(&file["desenho"].to_string()).map_err(|_| bad())?;
        scene.elements.iter_mut().for_each(|e| {
            e.set_unloaded(true);
        });
        let name = file["nome"].as_str().unwrap_or("Importado").trim();
        let root = file["raiz"].as_str().unwrap_or_default();
        let root = if root.is_empty() || crate::app::chat_direct::folder(root).is_some_and(|p| p.is_dir()) { root } else { "" };
        let color: [u8; 3] = serde_json::from_value(file["cor"].clone()).unwrap_or(space_colors()[self.spaces.list.len() % space_colors().len()]);
        let id = self.add_space_from(if name.is_empty() { "Importado" } else { name }, root, color, "", &scene, None)?;
        Ok(format!("Espaço \"{}\" importado.", self.space_name(&id)))
    }

    /// Descarregar: ends the shells of the space's terminals and the pages of its portals without taking
    /// them off the drawing (Recarregar on one starts it again). Returns how many it unloaded.
    pub(crate) fn unload_space(&mut self, id: &str) -> Result<usize, String> {
        if id == self.spaces.current {
            let all: Vec<usize> = (0..self.scene.elements.len()).collect();
            return Ok(all.into_iter().filter(|i| self.unload_node(*i, true)).count());
        }
        let file = self.space_file(id)?;
        let ended: Vec<String> = self.away.iter().filter(|(_, (s, _))| s == id).map(|(t, _)| t.clone()).collect();
        for terminal in &ended {
            self.terminals.remove(terminal);
            self.away.remove(terminal);
        }
        if let Some(bridge) = &self.bridge {
            bridge.shared.lock().unwrap().away.remove(id);
        }
        let Ok((mut scene, _)) = read_scene_file(&file) else { return Ok(0) };
        let n = scene.elements.iter_mut().map(|e| e.set_unloaded(true)).filter(|changed| *changed).count();
        write_scene_file(&file, &scene).map_err(|e| format!("Não foi possível gravar o desenho: {e}"))?;
        Ok(n)
    }

    /// The MCP's `workspace_groups`: list, create, rename, toggle, remove, move.
    pub(crate) fn act_space_groups(&mut self, args: &Value) -> Result<String, String> {
        let text = |key: &str| args[key].as_str().unwrap_or_default().trim().to_string();
        self.load_spaces();
        match text("action").as_str() {
            "list" | "" => {
                let line = |g: &SpaceGroup| {
                    let inside: Vec<&str> = self.spaces.list.iter().filter(|w| w.group == g.id).map(|w| w.name.as_str()).collect();
                    format!("- {}{}: {}", g.name, if g.open { "" } else { " (recolhido)" }, if inside.is_empty() { "vazio".into() } else { inside.join(", ") })
                };
                Ok(if self.spaces.groups.is_empty() { "Nenhum grupo.".into() } else { self.spaces.groups.iter().map(line).collect::<Vec<_>>().join("\n") })
            }
            "create" => self.create_group(&text("name")).map(|_| format!("Grupo \"{}\" criado.", text("name"))),
            "rename" => {
                let id = self.group_named(&text("name"))?;
                self.rename_group(&id, &text("new_name")).map(|_| format!("Grupo agora se chama \"{}\".", text("new_name")))
            }
            "toggle" => {
                let id = self.group_named(&text("name"))?;
                self.toggle_group(&id);
                let open = self.spaces.groups.iter().any(|g| g.id == id && g.open);
                Ok(format!("Grupo \"{}\" {}.", text("name"), if open { "expandido" } else { "recolhido" }))
            }
            "remove" => {
                let id = self.group_named(&text("name"))?;
                self.remove_group(&id)
            }
            // `group` empty takes the space out of its group
            "move" => {
                let space = self.space_named(&text("space"))?;
                let group = if text("group").is_empty() { None } else { Some(self.group_named(&text("group"))?) };
                self.move_space(&space, group.as_deref())?;
                Ok(match group {
                    Some(_) => format!("Espaço \"{}\" movido para o grupo \"{}\".", self.space_name(&space), text("group")),
                    None => format!("Espaço \"{}\" ficou sem grupo.", self.space_name(&space)),
                })
            }
            other => Err(format!("ação desconhecida: {other}")),
        }
    }

    /// The MCP's `workspace_menu`: pin, unpin, duplicate, copy_path, export, import, unload.
    pub(crate) fn act_space_menu(&mut self, args: &Value) -> Result<String, String> {
        let text = |key: &str| args[key].as_str().unwrap_or_default().trim().to_string();
        self.load_spaces();
        let action = text("action");
        if action == "import" {
            return self.import_space(Path::new(&text("path")));
        }
        let id = self.space_named(&text("name"))?;
        let name = self.space_name(&id);
        match action.as_str() {
            "pin" | "unpin" => {
                if self.spaces.list.iter().any(|w| w.id == id && w.pinned) != (action == "unpin") {
                    return Ok(format!("Espaço \"{name}\" já estava {}.", if action == "pin" { "fixado" } else { "desfixado" }));
                }
                let pinned = self.toggle_pin(&id)?;
                Ok(format!("Espaço \"{name}\" {}.", if pinned { "fixado" } else { "desfixado" }))
            }
            "duplicate" => self.duplicate_space(&id).map(|new| format!("Espaço \"{name}\" duplicado como \"{}\".", self.space_name(&new))),
            "copy_path" => match self.spaces.list.iter().find(|w| w.id == id).map(|w| w.root.clone()).filter(|r| !r.is_empty()) {
                Some(root) => Ok(root),
                None => Err(format!("O espaço \"{name}\" não tem pasta raiz.")),
            },
            "export" => self.export_space(&id, Path::new(&text("path"))),
            "unload" => self.unload_space(&id).map(|n| format!("Espaço \"{name}\" descarregado ({n} terminal(is) ou portal(is)).")),
            other => Err(format!("ação desconhecida: {other}")),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::workspaces::read_spaces;

    fn app_in(name: &str) -> (DrawingApp, PathBuf) {
        let dir = std::env::temp_dir().join(format!("mago-grupos-{name}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        let mut app = DrawingApp { autosave: Some(dir.join("autosave.mago")), ..Default::default() };
        app.load_spaces();
        app.load_autosave();
        (app, dir)
    }

    fn names(app: &DrawingApp, group: &str) -> Vec<String> {
        app.spaces.list.iter().filter(|w| w.group == group).map(|w| w.name.clone()).collect()
    }

    #[test]
    fn a_group_holds_spaces_folds_and_frees_them_when_deleted() {
        let (mut app, dir) = app_in("grupo");
        let a = app.create_space("A", "").unwrap();
        let b = app.create_space("B", "").unwrap();
        let g = app.create_group("Clientes").unwrap();
        assert!(app.create_group("clientes").is_err() && app.create_group("  ").is_err(), "names are unique and not empty");
        app.move_space(&a, Some(&g)).unwrap();
        app.move_space("principal", Some(&g)).unwrap();
        assert_eq!(names(&app, &g), ["A", "Principal"], "each goes to the group's end");
        assert_eq!(names(&app, ""), ["B"]);
        assert!(app.move_space(&b, Some("nada")).is_err());
        app.toggle_group(&g);
        assert!(!app.spaces.groups[0].open);
        app.move_space(&b, Some(&g)).unwrap();
        assert!(app.spaces.groups[0].open, "joining a folded group opens it");
        app.move_space(&a, None).unwrap();
        assert_eq!(names(&app, &g), ["Principal", "B"]);
        app.rename_group(&g, "Lojas").unwrap();
        // saved, and read back
        let (saved, _) = read_spaces(&dir.join("autosave.mago"));
        assert_eq!((saved.groups[0].name.as_str(), saved.list.iter().filter(|w| w.group == g).count()), ("Lojas", 2));
        // deleting the group frees its spaces, removes none
        assert!(app.remove_group(&g).unwrap().contains("sem grupo"));
        assert!(app.spaces.groups.is_empty() && app.spaces.list.len() == 3 && app.spaces.list.iter().all(|w| w.group.is_empty()));
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn an_old_list_opens_and_an_orphan_is_freed() {
        let dir = std::env::temp_dir().join(format!("mago-grupos-velho-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        fs::write(dir.join("espacos.json"), r#"{"espacos":[{"id":"x","nome":"X","cena":"x.mago"}],"atual":"x"}"#).unwrap();
        let (old, bad) = read_spaces(&dir.join("autosave.mago"));
        assert!(bad.is_none() && old.groups.is_empty() && !old.list[0].pinned && old.list[0].group.is_empty());
        fs::write(dir.join("espacos.json"), r#"{"espacos":[{"id":"x","nome":"X","cena":"x.mago","grupo":"some"}],"atual":"x"}"#).unwrap();
        assert!(read_spaces(&dir.join("autosave.mago")).0.list[0].group.is_empty());
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn pin_duplicate_export_import_and_unload() {
        let (mut app, dir) = app_in("menu");
        let e = app.new_element(ElementKind::Rectangle { x: 0.0, y: 0.0, width: 50.0, height: 50.0 });
        let mut t = app.new_element(ElementKind::Rectangle { x: 100.0, y: 0.0, width: 50.0, height: 50.0 });
        t.terminal = Some(TerminalSpec { name: "Sh".into(), ..Default::default() });
        let (eid, tid) = (e.id.clone(), t.id.clone());
        app.scene.elements.extend([e, t]);
        app.dirty = true;
        assert!(app.toggle_pin("principal").unwrap() && !app.toggle_pin("principal").unwrap());
        // Duplicar: its own file and fresh ids
        let copy = app.duplicate_space("principal").unwrap();
        assert_eq!(app.space_name(&copy), "Principal cópia");
        assert_eq!(app.spaces.list[1].id, copy, "beside the original");
        let copied = read_scene_file(&dir.join(&app.spaces.list[1].scene)).unwrap().0;
        assert_eq!(copied.elements.len(), 2);
        assert!(copied.elements.iter().all(|c| c.id != eid && c.id != tid));
        let second = app.duplicate_space("principal").unwrap();
        assert_eq!(app.space_name(&second), "Principal cópia 2");
        // Exportar / Importar
        let out = dir.join("p.espaco");
        app.export_space("principal", &out).unwrap();
        let imported = app.import_space(&out).unwrap();
        assert!(imported.contains("Principal 2"), "{imported}");
        let new = app.spaces.list.last().unwrap().clone();
        let scene = read_scene_file(&dir.join(&new.scene)).unwrap().0;
        assert!(scene.elements.iter().any(|e| e.terminal.as_ref().is_some_and(|t| t.unloaded)), "a terminal comes in unloaded");
        fs::write(dir.join("lixo.espaco"), "{}").unwrap();
        assert!(app.import_space(&dir.join("lixo.espaco")).is_err() && app.import_space(&dir.join("nada")).is_err());
        // Descarregar: the one on screen, then one off view
        assert_eq!(app.unload_space("principal").unwrap(), 1);
        assert!(app.scene.elements.iter().find(|e| e.id == tid).unwrap().unloaded());
        assert_eq!(app.unload_space(&copy).unwrap(), 1);
        assert!(read_scene_file(&dir.join(&app.spaces.list.iter().find(|w| w.id == copy).unwrap().scene)).unwrap().0.elements.iter().any(|e| e.unloaded()));
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn the_mcp_drives_groups_and_the_menu() {
        let (mut app, dir) = app_in("mcp");
        let act = |app: &mut DrawingApp, tool: &str, args: Value| app.act(tool, &args);
        act(&mut app, "workspace_groups", json!({ "action": "create", "name": "Clientes" })).unwrap();
        act(&mut app, "workspace_groups", json!({ "action": "move", "space": "principal", "group": "clientes" })).unwrap();
        let list = act(&mut app, "workspace_groups", json!({ "action": "list" })).unwrap();
        assert!(list.contains("Clientes: Principal"), "{list}");
        assert!(act(&mut app, "workspaces", json!({ "action": "list" })).unwrap().contains("grupo Clientes"));
        act(&mut app, "workspace_groups", json!({ "action": "toggle", "name": "Clientes" })).unwrap();
        assert!(act(&mut app, "workspace_groups", json!({ "action": "list" })).unwrap().contains("recolhido"));
        assert!(act(&mut app, "workspace_menu", json!({ "action": "pin", "name": "Principal" })).unwrap().contains("fixado"));
        assert!(act(&mut app, "workspace_menu", json!({ "action": "pin", "name": "Principal" })).unwrap().contains("já estava"));
        assert!(act(&mut app, "workspace_menu", json!({ "action": "copy_path", "name": "Principal" })).is_err(), "no root folder");
        let out = dir.join("m.espaco").to_string_lossy().to_string();
        act(&mut app, "workspace_menu", json!({ "action": "export", "name": "Principal", "path": out })).unwrap();
        assert!(act(&mut app, "workspace_menu", json!({ "action": "import", "path": out })).is_ok());
        act(&mut app, "workspace_groups", json!({ "action": "remove", "name": "Clientes" })).unwrap();
        assert!(act(&mut app, "workspace_menu", json!({ "action": "voar", "name": "Principal" })).is_err());
        let tools = crate::mcp::handle(&json!({ "jsonrpc": "2.0", "id": 1, "method": "tools/list" })).unwrap();
        let has = |n: &str| tools["result"]["tools"].as_array().unwrap().iter().any(|t| t["name"] == n);
        assert!(has("workspace_groups") && has("workspace_menu"));
        let _ = fs::remove_dir_all(&dir);
    }
}
