//! What the app does to a workspace off view, in its drawing's file (the truth while it is off screen: its agents'
//! MCP writes there too): closing a terminal that runs in it, moving a chat's card to it. And what an answer says
//! about the workspace something was made in.

use crate::app::chat::direct::folder;
use crate::app::chat::node::CHAT_NODE_SIZE;
use crate::app::floors::scope_key;
use crate::app::workspaces::Workspace;
use crate::prelude::*;

impl DrawingApp {
    /// The drawing's file of scope `key` ("space", or "space#floor").
    pub(crate) fn scope_file(&self, key: &str) -> Option<PathBuf> {
        let (id, floor) = key.split_once('#').unwrap_or((key, ""));
        let space = self.spaces.list.iter().find(|w| w.id == id)?;

        let file = match floor {
            "" => &space.scene,
            floor => &space.floors.iter().find(|f| f.id == floor)?.scene,
        };

        Some(self.autosave.as_ref()?.with_file_name(file))
    }

    /// Changes the drawing of a workspace off view, under the lock its agents' MCP takes.
    fn edit_away_scene(&self, key: &str, change: impl FnOnce(&mut Scene)) -> Result<(), String> {
        let path = self.scope_file(key).ok_or("Esse espaço não existe.")?;
        let _lock = crate::mcp::calls::scene_lock(&path)?;

        let mut scene = match read_scene_file(&path) {
            Ok((scene, _)) => scene,
            Err(SceneError::Missing) => Scene::default(),
            Err(error) => return Err(error.to_string()),
        };

        change(&mut scene);
        write_scene_file(&path, &scene).map(drop).map_err(|error| format!("não foi possível salvar: {error}"))
    }

    /// The pages kept off view (`away_portals`) whose workspace (or floor) still exists and that were not unloaded.
    pub(crate) fn prune_away_portals(&mut self) {
        let scopes: Vec<String> = self.away_portals.values().map(|(key, _)| key.clone()).collect();
        let gone: Vec<String> = scopes.into_iter().filter(|key| self.scope_file(key).is_none()).collect();

        self.away_portals.retain(|_, (key, spec)| !gone.contains(key) && !spec.unloaded);
    }

    /// The workspace a chat in folder `cwd` belongs to when it is not the one on screen: the one whose root holds the
    /// folder most closely, when the one on screen does not hold it.
    pub(crate) fn folder_space(&self, cwd: &str) -> Option<Workspace> {
        let here = self.current_space()?;

        let inside =
            |root: &str| !root.trim().is_empty() && !cwd.trim().is_empty() && folder(cwd).zip(folder(root)).is_some_and(|(cwd, root)| cwd.starts_with(root));

        if inside(&here.root) {
            return None;
        }

        self.spaces.list.iter().filter(|w| w.id != here.id && inside(&w.root)).max_by_key(|w| w.root.len()).cloned()
    }

    /// Where a recruit born on screen was born, for the answer its Maestro reads: nothing with one workspace.
    pub(crate) fn born_here(&self) -> String {
        match self.current_space().filter(|_| self.spaces.list.len() > 1) {
            Some(here) => format!(" (no espaço \"{}\", o que está na tela)", here.name),
            None => String::new(),
        }
    }

    /// A recruit of chat `from` born in workspace `home`, off view: in that space's drawing, under the chat's card there
    /// (put there first when it has none), roped to it. A terminal's shell starts now, off view (`sync_terminals`), and
    /// the bridge's view of that space has both, so the chat reaches its recruit there.
    pub(crate) fn place_away(&mut self, home: &Workspace, from: &str, mut recruit: Element) -> Result<(), String> {
        let key = scope_key(home, &home.floor);
        let card = self.chat_card(from, Rect::from_min_size(Pos2::ZERO, CHAT_NODE_SIZE)).ok_or("Esse chat não existe mais.")?;
        let (id, size) = (recruit.id.clone(), local_bounds(&recruit).size());
        let mut new_card = false;

        self.edit_away_scene(&key, |scene| {
            let at = match scene.elements.iter().find(|e| e.id == from && e.chat.is_some()) {
                Some(existing) => local_bounds(existing),
                None => {
                    let mut card = card.clone();
                    let want = local_bounds(&card);
                    let spot = free_spot_among(scene.elements.iter(), want, want);

                    card.move_by(spot.min - want.min);
                    scene.elements.push(card);
                    new_card = true;
                    spot
                }
            };

            let spot = free_spot_among(scene.elements.iter(), Rect::from_min_size(Pos2::new(at.min.x, at.max.y + 60.0), size), at);

            recruit.move_by(spot.min - local_bounds(&recruit).min);
            let rope = self.rope_element(from, at.center(), &id, spot.center());

            scene.elements.push(recruit.clone());
            scene.elements.push(rope);
        })?;

        if let Some(spec) = &recruit.terminal {
            self.away.insert(id.clone(), (key.clone(), spec.clone()));
        }

        if let Some(bridge) = &self.bridge {
            let chat_name = self.chats.iter().find(|c| c.id == from).map(|c| self.maestro_name(&c.name)).unwrap_or_default();
            let recruit_name = recruit.terminal.as_ref().map(|t| t.name.clone()).or(recruit.chat.as_ref().map(|c| c.name.clone())).unwrap_or_default();
            let kind = if recruit.chat.is_some() { NodeKind::Chat } else { NodeKind::Terminal };
            let mut shared = locked(&bridge.shared);
            let (_, nodes) = shared.away.entry(key.clone()).or_insert_with(|| (home.name.clone(), Vec::new()));

            if new_card || !nodes.iter().any(|n| n.id == from) {
                nodes.push(Node { id: from.into(), kind: NodeKind::Chat, name: chat_name, text: String::new(), links: Vec::new(), maestro: true });
            }

            nodes.iter_mut().filter(|n| n.id == from).for_each(|n| n.links.push(id.clone()));
            nodes.push(Node { id: id.clone(), kind, name: recruit_name, text: String::new(), links: vec![from.into()], maestro: false });
        }

        Ok(())
    }

    /// What ran under terminal `id`'s shell, for the answer of who closes it: closing ends all of it (`end_tree`), a
    /// command it ran inside a container included.
    pub(crate) fn left_running(&self, id: &str) -> String {
        let spec = self.scene.elements.iter().find(|e| e.id == id).and_then(|e| e.terminal.as_ref()).or(self.away.get(id).map(|(_, spec)| spec));
        let agent = spec.and_then(|spec| spec.command.split_whitespace().next()).unwrap_or_default();

        // ponytail: the agent's own program is told by its command's first word; one that shows by another name (node) is listed too; a process already dead (<defunct>) is not
        let running = self.terminal_processes(id);
        let container = running.iter().any(|p| crate::app::terminal::procs::container_cli(&p.name));

        let names: Vec<String> =
            running.into_iter().filter(|p| p.name != "<defunct>" && (agent.is_empty() || p.name != agent)).map(|p| format!("{} ({})", p.name, p.pid)).collect();

        if names.is_empty() {
            return String::new();
        }

        let more = if names.len() > 6 { format!(" e mais {}", names.len() - 6) } else { String::new() };

        let inside = if container { ", e o comando que ele rodava dentro do container" } else { "" };

        format!(" Encerrei o que rodava nele: {}{more}{inside}.", names.iter().take(6).cloned().collect::<Vec<_>>().join(", "))
    }

    /// `close_window` on something the canvas shown does not have: a terminal running in a workspace off view ends,
    /// and leaves that workspace's drawing with its ropes; anything else there is said to be there. None: no such name.
    pub(crate) fn close_away(&mut self, name: &str) -> Option<Result<String, String>> {
        let wanted = name.trim().to_lowercase();
        let named = |id: &str, label: &str| id == name.trim() || label.to_lowercase() == wanted;

        let terminal =
            self.away.iter().find(|(id, (_, spec))| named(id, spec.label())).map(|(id, (key, spec))| (id.clone(), key.clone(), spec.label().to_string()));

        let Some((id, key, label)) = terminal else {
            // a note, a page, a tree, a chat's card: the bridge knows what each space had when it left the screen
            let bridge = self.bridge.as_ref()?;
            let shared = locked(&bridge.shared);
            let (space, node) = shared.away.values().find_map(|(space, nodes)| Some((space, nodes.iter().find(|n| named(&n.id, &n.name))?)))?;

            return Some(Err(format!("\"{}\" está no espaço \"{space}\", fora da tela: abra-o (workspaces switch) para fechar.", node.name)));
        };

        let left = self.left_running(&id);
        let tied = |e: &Element| e.rope && [&e.ends.start, &e.ends.end].into_iter().flatten().any(|end| end.element_id == id);

        if let Err(why) = self.edit_away_scene(&key, |scene| scene.elements.retain(|e| e.id != id && !tied(e))) {
            return Some(Err(why));
        }

        self.terminals.remove(&id);
        self.away.remove(&id);
        self.release_tasks(&id);

        if let Some(bridge) = &self.bridge
            && let Some((_, nodes)) = locked(&bridge.shared).away.get_mut(&key)
        {
            nodes.retain(|n| n.id != id);
            nodes.iter_mut().for_each(|n| n.links.retain(|link| *link != id));
        }

        Some(Ok(format!("{label} fechado no espaço \"{}\" (fora da tela; lá não dá para desfazer).{left}", self.space_name(&key))))
    }

    /// `chat_node move`: the chat's card leaves the canvas shown and goes to workspace `space` (by name), on the floor
    /// it was left on. Its ropes stay behind: what they tied it to is in this drawing.
    pub(crate) fn move_chat_card(&mut self, id: &str, space: &str) -> Result<String, String> {
        if space.trim().is_empty() {
            return Err("Diga o espaço de destino (space).".into());
        }

        self.load_spaces();
        let target = self.space_named(space)?;
        let target = self.spaces.list.iter().find(|w| w.id == target).cloned().ok_or("Esse espaço não existe.")?;
        let key = scope_key(&target, &target.floor);
        let name = self.agent_label(id);

        if key == self.scope_key() {
            return self.show_chat_on_canvas(id).map(|shown| format!("{shown} O espaço \"{}\" já é o que está na tela.", target.name));
        }

        let want = Rect::from_min_size(Pos2::ZERO, CHAT_NODE_SIZE);
        let mut card = self.chat_card(id, want).ok_or("Esse chat não existe mais.")?;

        self.edit_away_scene(&key, |scene| {
            if !scene.elements.iter().any(|e| e.id == id && e.chat.is_some()) {
                let at = free_spot_among(scene.elements.iter(), want, want);

                card.move_by(at.min - want.min);
                scene.elements.push(card);
            }
        })?;

        // the bridge's view of that space has the card too: its agent reaches the space by it
        if let Some(bridge) = &self.bridge
            && let Some((_, nodes)) = locked(&bridge.shared).away.get_mut(&key)
            && !nodes.iter().any(|n| n.id == id)
        {
            let chat_name = self.chats.iter().find(|c| c.id == id).map(|c| self.maestro_name(&c.name)).unwrap_or_default();

            nodes.push(Node {
                id: id.into(),
                kind: NodeKind::Chat,
                name: chat_name,
                text: String::new(),
                links: Vec::new(),
                maestro: !self.is_recruit_chat(id),
            });
        }

        let had = self.chat_node(id);

        if let Some(index) = had {
            self.selected = vec![index];
            self.delete_selected();
        }

        let ropes = if had.is_some() { " Saiu deste canvas (dá para desfazer); as cordas que ele tinha aqui não foram junto." } else { "" };

        Ok(format!("Cartão do chat {name} agora está no espaço \"{}\".{ropes} Para vê-lo, abra o espaço (workspaces switch).", target.name))
    }
}
