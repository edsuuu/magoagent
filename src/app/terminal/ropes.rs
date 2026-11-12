use super::*;

impl DrawingApp {
    /// The window that has the person's attention: a terminal being typed into, the IDE open, an
    /// IDE card's tree, or a terminal enlarged. It keeps its outline still and the top to its own bar.
    pub(crate) fn focused_window(&self) -> Option<&String> {
        // docked, the canvas around it is worked on: only typing into it counts
        let elevated = self.terminal_elevated.as_ref().filter(|_| self.dock.is_none());
        self.terminal_focus.as_ref().or(self.ide_focus.as_ref()).or(self.card_focus.as_ref()).or(elevated)
    }

    /// Lets the bridge threads see the canvas as it is now: each frame, and before an agent hears
    /// back about something it created, so its next request already finds it.
    pub(crate) fn publish_nodes(&self) {
        if let Some(bridge) = &self.bridge {
            bridge.shared.lock().unwrap().nodes = self.agent_nodes();
        }
    }

    /// A dashed line bound to both elements: what agents see as a connection.
    pub(crate) fn connect(&mut self, a: &str, b: &str) {
        self.rope_between(a, b, true);
    }

    /// `connect`, optionally without its own undo step (when it rides along another change).
    pub(crate) fn rope_between(&mut self, a: &str, b: &str, undoable: bool) {
        let center = |id: &str| self.scene.elements.iter().find(|e| e.id == id).map(|e| element_bounds(e).center());
        let (Some(from), Some(to)) = (center(a), center(b)) else { return };
        if a == b || self.links().get(a).is_some_and(|l| l.iter().any(|x| x == b)) {
            return;
        }
        let mut rope = self.new_element(ElementKind::Line { points: vec![[from.x, from.y], [to.x, to.y]], arrow: true });
        let bind = |id: &str| Some(Binding { element_id: id.into(), fixed: [0.5, 0.5], inside: false });
        rope.ends = ArrowEnds { start_head: None, end_head: None, start: bind(a), end: bind(b) };
        rope.rope = true;
        rope.style = Style {
            stroke: [0x8a, 0x8a, 0x8a, 255],
            fill: None,
            stroke_width: 2.0,
            stroke_style: StrokeStyle::Dashed,
            roughness: 0,
            round: true,
            ..rope.style
        };
        if undoable {
            self.snapshot();
        }
        self.scene.elements.push(rope);
        self.update_bindings();
    }

    /// What an agent drew through the MCP server hangs from its terminal: each new window (terminal,
    /// portal, IDE) by a rope, and one rope to the biggest new shape, unless the terminal already
    /// holds the drawing it joins.
    pub(crate) fn tie_to_terminal(&mut self, terminal: &str, fresh: &[String]) {
        if !self.scene.elements.iter().any(|e| e.id == terminal && e.terminal.is_some()) {
            return;
        }
        // born connected to its maker, as `recruit` makes its child
        let windows: Vec<String> = self.scene.elements.iter().filter(|e| e.is_node() && fresh.contains(&e.id)).map(|e| e.id.clone()).collect();
        for id in &windows {
            self.rope_between(terminal, id, false);
        }
        let area = |e: &Element| element_bounds(e).area();
        let drawn = self.scene.elements.iter().filter(|e| fresh.contains(&e.id) && bindable(e) && !e.rope && !e.is_node());
        let Some(target) = drawn.max_by(|a, b| area(a).total_cmp(&area(b))) else { return };
        let (id, bounds) = (target.id.clone(), element_bounds(target));
        let overlaps = |other: &String| self.scene.elements.iter().any(|e| &e.id == other && element_bounds(e).intersects(bounds));
        if !self.links().get(terminal).is_some_and(|linked| linked.iter().any(overlaps)) {
            // one Cmd+Z takes back the drawing and its rope together
            self.rope_between(terminal, &id, false);
        }
    }

    /// Something selected, and all of it passes `f` (only nodes: their look doesn't come from the
    /// style panel).
    pub(crate) fn only_selected(&self, f: impl Fn(&Element) -> bool) -> bool {
        !self.selected.is_empty() && self.selected.iter().all(|i| self.scene.elements.get(*i).is_some_and(&f))
    }

    /// Moves every rope on: gravity pulls its links, each link keeps its length, its ends follow
    /// what they're tied to and it slides around the windows in its way. True while any moves.
    pub(crate) fn swing_ropes(&mut self, dt: f32) -> bool {
        // a rope needs both ends: whatever removed one (Delete, eraser, MCP, undo) takes the rope too
        // (not mid-drag: an end being dragged is unbound until the release, and removing anything
        // would shift the indices the drag holds)
        let exists = |b: &Option<Binding>| b.as_ref().is_some_and(|b| self.scene.elements.iter().any(|e| e.id == b.element_id));
        let loose: Vec<String> =
            self.scene.elements.iter().filter(|e| e.rope && !(exists(&e.ends.start) && exists(&e.ends.end))).map(|e| e.id.clone()).collect();
        if !loose.is_empty() && !self.in_gesture() {
            self.keeping_selection(|app| app.scene.elements.retain(|e| !loose.contains(&e.id)));
            self.dirty = true;
        }
        let nodes: Vec<(String, Rect)> =
            self.scene.elements.iter().filter(|e| e.is_node() || e.created.is_some()).map(|e| (e.id.clone(), element_bounds(e))).collect();
        let mut seen = Vec::new();
        let mut moving = false;
        for e in self.scene.elements.iter().filter(|e| e.rope) {
            let ElementKind::Line { points, .. } = &e.kind else { continue };
            let (Some(a), Some(b)) = (points.first(), points.last()) else { continue };
            let (a, b) = (Pos2::new(a[0], a[1]), Pos2::new(b[0], b[1]));
            let tied: Vec<&str> = [&e.ends.start, &e.ends.end].into_iter().flatten().map(|t| t.element_id.as_str()).collect();
            let obstacles: Vec<Rect> = nodes.iter().filter(|(id, _)| !tied.contains(&id.as_str())).map(|(_, r)| *r).collect();
            let rope = self.ropes.entry(e.id.clone()).or_insert_with(|| RopeSim::hanging(a, b));
            moving |= rope.advance(a, b, dt, &obstacles);
            seen.push(e.id.clone());
        }
        // the rope that follows the pointer while "Conectar" waits for its other end: it leaves the
        // source's edge facing the pointer, and the retain below drops it once the mode ends
        if let (Some(from), Some(pointer)) = (self.connecting.clone(), self.last_pointer)
            && let Some(source) = self.scene.elements.iter().find(|e| e.id == from)
        {
            let b = self.to_world(pointer);
            let a = bound_endpoint(source, &Binding { element_id: from.clone(), fixed: [0.5, 0.5], inside: false }, b);
            let obstacles: Vec<Rect> = nodes.iter().filter(|(id, _)| id != &from).map(|(_, r)| *r).collect();
            let rope = self.ropes.entry(CONNECT_DRAFT.into()).or_insert_with(|| RopeSim::hanging(a, b));
            rope.advance(a, b, dt, &obstacles);
            // ponytail: redraws every frame while connecting, since the pointer lands after this frame's swing; ceiling: idle redraws until Esc or a click, upgrade: one repaint per pointer move
            moving = true;
            seen.push(CONNECT_DRAFT.into());
        }
        self.ropes.retain(|id, _| seen.contains(id));
        moving
    }

    /// A rope's curve as it is drawn right now (mid-swing included).
    pub(crate) fn rope_path(&self, rope: &Element) -> Vec<[f32; 2]> {
        match self.ropes.get(&rope.id) {
            Some(sim) => sim.points.iter().map(|p| [p.x, p.y]).collect(),
            None => line_path(rope),
        }
    }

    /// The terminal or portal the context bar is about: focused, enlarged or the one selected.
    pub(crate) fn active_node(&self) -> Option<&Element> {
        let by_id = |id: &String| self.scene.elements.iter().find(|e| &e.id == id);
        let selected = match self.selected[..] {
            [i] => self.scene.elements.get(i).filter(|e| e.is_node() || e.created.is_some()),
            _ => None,
        };
        self.terminal_focus.as_ref().and_then(by_id).or_else(|| self.terminal_elevated.as_ref().and_then(by_id)).or(selected)
    }

    /// A terminal the Maestro recruits is a new agent: a name from AGENT_NAMES no terminal in any
    /// workspace has, at random ("Fumaça 2" once all are taken).
    pub(crate) fn agent_name(&self) -> String {
        use std::hash::BuildHasher;
        let roll = std::collections::hash_map::RandomState::new().hash_one(0u8) as usize;
        let taken = |name: &str| {
            self.scene.elements.iter().any(|e| e.terminal.as_ref().is_some_and(|t| t.name == name)) || self.chats.iter().any(|c| c.recruit && c.name == name)
        };
        let free: Vec<&str> = AGENT_NAMES.iter().copied().filter(|n| !taken(n)).collect();
        match free.get(roll % free.len().max(1)) {
            Some(name) => name.to_string(),
            None => self.unique_terminal_name(AGENT_NAMES[roll % AGENT_NAMES.len()], None),
        }
    }

    /// A terminal the user opens is named after what runs in it: its quick start's name ("Claude
    /// Code", "Codex", "Shell"), else the command's first word.
    pub(crate) fn command_name(&self, command: &str) -> String {
        let command = command.trim();
        match self.prefs.presets.iter().find(|p| p.command.trim() == command && !p.name.trim().is_empty()) {
            Some(preset) => preset.name.trim().to_string(),
            None => command.split_whitespace().next().unwrap_or("Shell").to_string(),
        }
    }

    /// A name no other terminal has: "Shell", "Shell 2"…
    pub(crate) fn unique_terminal_name(&self, base: &str, except: Option<&str>) -> String {
        let taken = |name: &str| self.scene.elements.iter().any(|e| Some(e.id.as_str()) != except && e.terminal.as_ref().is_some_and(|t| t.name == name));
        (1..).map(|n| if n == 1 { base.to_string() } else { format!("{base} {n}") }).find(|name| !taken(name)).unwrap_or_default()
    }

    /// Where new terminals and IDEs start: the open drawing's folder, else the workspace's root.
    pub(crate) fn new_folder(&self) -> String {
        self.file_path.as_ref().and_then(|p| p.parent()).map(|p| p.to_string_lossy().to_string()).unwrap_or_else(|| self.space_root())
    }
}
