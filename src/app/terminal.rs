//! Terminals on the canvas: their shells, typing into them, painting them, and the lines that
//! connect them so agents can reach each other through the bridge.

use crate::app::ui::context::{ContextMenu, Target};
use crate::prelude::*;
use alacritty_terminal::term::TermMode;
use std::collections::HashMap;

/// The new / edit terminal dialog.
#[derive(Clone, Debug)]
pub(crate) struct TerminalDialog {
    /// The terminal being edited; None creates one in `at`.
    pub(crate) target: Option<String>,
    pub(crate) at: Rect,
    pub(crate) spec: TerminalSpec,
    /// Detalhes or Aparência
    pub(crate) tab: usize,
}

/// The key of the rope that follows the pointer while "Conectar" waits (in `ropes`, not an element).
pub(crate) const CONNECT_DRAFT: &str = "connect-draft";

/// The names new agents draw from, one each.
pub(crate) const AGENT_NAMES: &[&str] = &[
    "Sábia",
    "Fuminho",
    "Fumaça",
    "Faísca",
    "Brasa",
    "Centelha",
    "Neblina",
    "Trovão",
    "Cometa",
    "Coruja",
    "Runa",
    "Poção",
    "Feitiço",
    "Lume",
    "Orvalho",
    "Vagalume",
    "Ventania",
    "Cristal",
    "Eclipse",
    "Aurora",
    "Bruma",
    "Chama",
    "Relâmpago",
    "Corvo",
    "Grimório",
    "Amuleto",
    "Caldeirão",
    "Varinha",
    "Pirilampo",
    "Garoa",
    "Sereno",
    "Tição",
];

/// A terminal's or portal's dark box: what exports and the AI's view show (the app paints the live
/// node over it).
pub(crate) fn node_style(style: Style) -> Style {
    Style { stroke: [0x3a, 0x3a, 0x3a, 255], fill: Some([0x18, 0x18, 0x18, 255]), stroke_width: 1.0, roughness: 0, round: true, ..style }
}

/// `~` for the home folder.
pub(crate) fn display_folder(cwd: &str) -> String {
    let home = std::env::home_dir().map(|h| h.to_string_lossy().into_owned()).unwrap_or_default();
    match cwd.strip_prefix(&home) {
        _ if cwd.is_empty() => "~".into(),
        Some(rest) if !home.is_empty() => format!("~{rest}"),
        _ => cwd.into(),
    }
}

impl DrawingApp {
    /// Ids each element is connected to by a line bound at both ends.
    pub(crate) fn links(&self) -> HashMap<String, Vec<String>> {
        links_in(&self.scene)
    }

    /// What agents can reach, in every workspace open this session.
    pub(crate) fn agent_nodes(&self) -> Vec<Node> {
        let parked = self.parked.values().map(|p| (&p.scene, true));
        std::iter::once((&self.scene, false)).chain(parked).flat_map(|(scene, hidden)| self.nodes_in(scene, hidden)).collect()
    }

    /// What agents can reach in one drawing: terminals, sticky notes (their label is the text),
    /// portals and IDEs.
    fn nodes_in(&self, scene: &Scene, hidden: bool) -> Vec<Node> {
        let links = links_in(scene);
        let labels: HashMap<&str, &str> = scene
            .elements
            .iter()
            .filter_map(|e| match &e.kind {
                ElementKind::Text { value, .. } => Some((e.id.strip_suffix("-label")?, value.as_str())),
                _ => None,
            })
            .collect();
        let label = |id: &str| labels.get(id).map(|v| v.to_string());
        let named = |name: &str, fallback: &str| if name.trim().is_empty() { fallback.to_string() } else { name.trim().to_string() };
        scene
            .elements
            .iter()
            .filter_map(|e| {
                let (kind, name, text) = if let Some(t) = &e.terminal {
                    (NodeKind::Terminal, named(&t.name, "Terminal"), String::new())
                } else if let Some(p) = &e.portal {
                    (NodeKind::Portal, named(&p.name, "Portal"), p.url.clone())
                } else if e.created.is_some() {
                    let text = label(&e.id).unwrap_or_default();
                    (NodeKind::Note, note_name(&text), text)
                } else if let Some(spec) = &e.ide {
                    // the folder as the editor opened it (`~` expanded)
                    let root = self.ides.get(&e.id).and_then(|i| i.root.as_ref()).map_or(spec.root.clone(), |r| r.to_string_lossy().into());
                    (NodeKind::Ide, named(&spec.name, "IDE"), root)
                } else {
                    return None;
                };
                Some(Node {
                    id: e.id.clone(),
                    kind,
                    name,
                    text,
                    links: links.get(&e.id).cloned().unwrap_or_default(),
                    maestro: e.terminal.as_ref().is_some_and(|t| t.maestro),
                    hidden,
                })
            })
            .collect()
    }
}

/// Element ids roped together in a drawing, both ways.
fn links_in(scene: &Scene) -> HashMap<String, Vec<String>> {
    let mut links: HashMap<String, Vec<String>> = HashMap::new();
    for e in &scene.elements {
        if let (ElementKind::Line { .. }, Some(a), Some(b)) = (&e.kind, &e.ends.start, &e.ends.end) {
            if a.element_id != b.element_id {
                links.entry(a.element_id.clone()).or_default().push(b.element_id.clone());
                links.entry(b.element_id.clone()).or_default().push(a.element_id.clone());
            }
        }
    }
    links
}

impl DrawingApp {
    pub(crate) fn terminal_env(&self, id: &str, name: &str) -> Vec<(String, String)> {
        let mut env = vec![("MAGOAGENT_TERMINAL_ID".to_string(), id.to_string()), ("MAGOAGENT_TERMINAL_NAME".into(), name.into())];
        if let Some(bridge) = &self.bridge {
            env.push(("MAGOAGENT_BRIDGE".into(), bridge.address.clone()));
            env.push(("MAGOAGENT_TOKEN".into(), bridge.token.clone()));
        }
        // `magoagent list|ask|…` works in every terminal: this app's folder first on PATH
        if let Ok(exe) = std::env::current_exe() {
            let path = std::env::var_os("PATH").unwrap_or_default();
            let dirs = exe.parent().map(PathBuf::from).into_iter().chain(std::env::split_paths(&path));
            if let Ok(joined) = std::env::join_paths(dirs) {
                env.push(("PATH".into(), joined.to_string_lossy().into()));
            }
        }
        env.extend(self.ide_env(id));
        // the MCP in it draws in its own workspace (absolute: the shell may change folder)
        if let Some(scene) = self.scene_of(id) {
            env.push(("MAGOAGENT_SCENE".into(), scene.to_string_lossy().into()));
        }
        env
    }

    /// One shell per terminal element: new ones start, removed ones end, sizes follow the box.
    /// Also tells when an agent finishes and keeps the bridge's view of the canvas current.
    pub(crate) fn sync_terminals(&mut self, ctx: &Context) {
        let elevated = self.elevated_rect();
        let live: Vec<(String, TerminalSpec, Vec2)> = self
            .all_elements()
            .filter_map(|e| {
                let size = if self.terminal_elevated.as_ref() == Some(&e.id) { elevated.size() } else { local_bounds(e).size() };
                Some((e.id.clone(), e.terminal.clone().filter(|t| !t.unloaded)?, size))
            })
            .collect();
        // an undo that took the terminal away keeps its shell while a redo can bring it back; Delete
        // and the MCP take a snapshot, which clears the redo, so they still end it now. An unloaded
        // one is still drawn: it ends now
        // ponytail: scans the redo once per orphan per frame; index the ids if the redo grows large
        let drawn = |id: &str| self.all_elements().any(|e| e.id == id);
        let gone: Vec<String> = self.terminals.keys().filter(|id| !live.iter().any(|(l, ..)| l == *id) && (drawn(id) || !self.in_redo(id))).cloned().collect();
        for id in gone {
            self.terminals.remove(&id);
        }
        // what shows now (with the chat open, only its agent): the others' output waits for the idle repaint
        let shown = if self.chat_open { Rect::NOTHING } else { self.canvas_rect };
        let seen: std::collections::HashSet<String> = self
            .scene
            .elements
            .iter()
            .filter(|e| e.terminal.is_some() && self.terminal_frame(e).0.intersects(shown))
            .map(|e| e.id.clone())
            .chain(self.chat_agent.clone().filter(|_| self.chat_open))
            .collect();
        for (id, spec, size) in &live {
            match self.terminals.get_mut(id) {
                Some(Ok(session)) => session.resize(grid_size(*size)),
                Some(Err(_)) => {}
                None => {
                    let env = self.terminal_env(id, &spec.name);
                    match self.terminal_ide(id) {
                        Some((ide, _)) => self.terminal_ides.insert(id.clone(), (ide, false)),
                        None => self.terminal_ides.remove(id),
                    };
                    let session = TerminalSession::spawn(&spec.cwd, &spec.command, grid_size(*size), Some(ctx.clone()), env, None);
                    self.terminals.insert(id.clone(), session.map_err(|e| e.to_string()));
                }
            }
        }
        // a shell an undo took (kept for the redo) shows nowhere; with the window hidden, none does
        let window = ctx.input(|i| i.viewport().visible()) != Some(false);
        for (id, session) in &self.terminals {
            if let Ok(session) = session {
                session.set_visible(window && seen.contains(id));
            }
        }
        if self.leaving.as_ref().is_some_and(|(_, since)| self.now - since > 0.2) {
            self.leaving = None;
        }
        if self.terminal_focus.as_ref().is_some_and(|id| !live.iter().any(|(l, ..)| l == id)) {
            self.terminal_focus = None;
        }
        // an IDE is enlarged the same way and has no shell
        if self.terminal_elevated.as_ref().is_some_and(|id| !self.scene.elements.iter().any(|e| &e.id == id)) {
            self.terminal_elevated = None;
        }
        // "Monitorar atividade": an agent that worked a while and went quiet gets a toast
        for (id, spec, _) in live.iter().filter(|(_, spec, _)| !spec.command.trim().is_empty() && !spec.quiet) {
            let busy = matches!(self.terminals.get(id), Some(Ok(s)) if s.handle.busy());
            if busy {
                self.terminal_busy.entry(id.clone()).or_insert(self.now);
            } else if let Some(since) = self.terminal_busy.remove(id) {
                self.terminal_idle.insert(id.clone(), self.now);
                if self.now - since > 3.0 && self.terminal_focus.as_ref() != Some(id) {
                    self.toast(format!("{} terminou", if spec.name.is_empty() { "O terminal" } else { &spec.name }));
                    self.terminal_unread.insert(id.clone());
                }
            }
        }
        if let Some(id) = &self.terminal_focus {
            self.terminal_unread.remove(id);
        }
        let Some(bridge) = &self.bridge else { return };
        let terminals = self.terminals.iter().filter_map(|(id, s)| Some((id.clone(), s.as_ref().ok()?.handle.clone()))).collect();
        let mut shared = bridge.shared.lock().unwrap();
        (shared.nodes, shared.terminals) = (self.agent_nodes(), terminals);
        drop(shared);
        let commands: Vec<UiCommand> = bridge.commands.try_iter().collect();
        for command in commands {
            self.apply_agent_command(command);
        }
    }

    pub(crate) fn apply_agent_command(&mut self, command: UiCommand) {
        // ponytail: what sits in a hidden workspace is refused (the bridge already refused the
        // commands with no answer channel); upgrade: apply it to the parked drawing
        if command.targets().iter().any(|id| self.hidden(id)) {
            command.refuse(crate::bridge::HIDDEN);
            return;
        }
        match command {
            UiCommand::PortalNavigate { id, url } => self.portal_navigate(&id, &url),
            UiCommand::PortalEval { id, script, reply } => self.portal_eval(&id, &script, reply),
            UiCommand::WriteNote { id, text } => {
                let label_id = format!("{id}-label");
                self.snapshot();
                for e in &mut self.scene.elements {
                    if let (true, ElementKind::Text { value, .. }) = (e.id == label_id, &mut e.kind) {
                        *value = text.clone();
                    }
                }
            }
            UiCommand::CreateNote { from, text } => {
                let Some(terminal) = self.scene.elements.iter().find(|e| e.id == from) else { return };
                // to the terminal's left, like a sticky on the monitor's bezel
                let b = local_bounds(terminal);
                let note = self.new_note(Rect::from_min_size(Pos2::new(b.min.x - 290.0, b.min.y), Vec2::splat(250.0)), text);
                let note_id = note[0].id.clone();
                self.snapshot();
                self.scene.elements.extend(note);
                // one Cmd+Z takes back the note and its rope together
                self.rope_between(&note_id, &from, false);
            }
            UiCommand::Recruit { from, command, reply } => self.recruit(&from, &command, reply),
            UiCommand::CreatePortal { from, url, name, reply } => self.create_portal(&from, &url, &name, reply),
            UiCommand::Connect { a, b, reply } => self.connect_for_agent(&a, &b, reply),
            UiCommand::Notify { from, text } => self.notify(&from, &text),
            UiCommand::Ide { id, action, reply } => self.ide_agent(&id, action, reply),
            other => self.apply_maestro_command(other),
        }
    }

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
        if let (Some(from), Some(pointer)) = (self.connecting.clone(), self.last_pointer) {
            if let Some(source) = self.scene.elements.iter().find(|e| e.id == from) {
                let b = self.to_world(pointer);
                let a = bound_endpoint(source, &Binding { element_id: from.clone(), fixed: [0.5, 0.5], inside: false }, b);
                let obstacles: Vec<Rect> = nodes.iter().filter(|(id, _)| id != &from).map(|(_, r)| *r).collect();
                let rope = self.ropes.entry(CONNECT_DRAFT.into()).or_insert_with(|| RopeSim::hanging(a, b));
                rope.advance(a, b, dt, &obstacles);
                // ponytail: redraws every frame while connecting, since the pointer lands after this frame's swing; ceiling: idle redraws until Esc or a click, upgrade: one repaint per pointer move
                moving = true;
                seen.push(CONNECT_DRAFT.into());
            }
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
        let taken = |name: &str| self.all_elements().any(|e| e.terminal.as_ref().is_some_and(|t| t.name == name));
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

    /// Click: an 80×24 terminal centered there; drag: that box. Asks for its shell first.
    pub(crate) fn open_terminal_dialog(&mut self, start: Pos2, end: Pos2) {
        let at = if start.distance(end) * self.zoom < 10.0 {
            Rect::from_center_size(start, terminal_size(80, 24))
        } else {
            let r = Rect::from_two_pos(start, end);
            Rect::from_min_size(r.min, r.size().max(TERMINAL_MIN))
        };
        let spec = TerminalSpec { name: self.command_name(""), cwd: self.new_folder(), command: String::new(), ..Default::default() };
        self.terminal_dialog = Some(TerminalDialog { target: None, at, spec, tab: 0 });
        if !self.tool_locked {
            self.tool = Tool::Select;
        }
    }

    /// Creates the dialog's terminal, or applies it to the one being edited (restarting it
    /// when the folder or command changed).
    pub(crate) fn confirm_terminal_dialog(&mut self) {
        let Some(TerminalDialog { target, at, mut spec, .. }) = self.terminal_dialog.take() else { return };
        let name = match spec.name.trim() {
            "" => self.command_name(&spec.command),
            name => name.to_string(),
        };
        spec.name = self.unique_terminal_name(&name, target.as_deref());
        self.snapshot();
        if let Some(id) = target {
            if let Some(e) = self.scene.elements.iter_mut().find(|e| e.id == id) {
                let restart = e.terminal.as_ref().is_some_and(|old| old.cwd != spec.cwd || old.command != spec.command);
                e.terminal = Some(spec);
                if restart {
                    self.terminals.remove(&id);
                }
            }
            return;
        }
        let mut terminal = self.new_element(ElementKind::Rectangle { x: at.min.x, y: at.min.y, width: at.width(), height: at.height() });
        terminal.style = node_style(terminal.style);
        let chat = spec.chat;
        terminal.terminal = Some(spec);
        self.terminal_focus = Some(terminal.id.clone());
        let id = terminal.id.clone();
        self.scene.elements.push(terminal);
        self.selected = vec![self.scene.elements.len() - 1];
        if chat {
            self.chat_on(&id);
        }
    }

    /// Descarregar (`on`) or Recarregar terminal or portal `i`, as one undo step: false when it is
    /// neither or already so. The next frame ends or starts its shell or page.
    pub(crate) fn unload_node(&mut self, i: usize, on: bool) -> bool {
        let e = &self.scene.elements[i];
        if (e.terminal.is_none() && e.portal.is_none()) || e.unloaded() == on {
            return false;
        }
        self.snapshot();
        self.scene.elements[i].set_unloaded(on)
    }

    /// A press on a terminal's body: it takes the keyboard (the overlay types into it from here on)
    /// and restarts its shell if that had ended. Stays selected, so its bar stays up.
    pub(crate) fn focus_terminal(&mut self, index: usize) {
        let id = self.scene.elements[index].id.clone();
        if self.terminals.get(&id).is_some_and(|s| s.as_ref().map_or(true, |s| s.exited())) {
            self.terminals.remove(&id);
        }
        self.moving = false;
        self.marquee = None;
        self.selected = vec![index];
        // "Iniciar no modo de chat": its chat takes the click; Abrir no canvas or Ampliar still type into it
        if self.scene.elements[index].terminal.as_ref().is_some_and(|t| t.chat) {
            self.chat_on(&id);
            return;
        }
        self.terminal_focus = Some(id);
    }

    /// Double-click on the title strip enlarges it (or puts it back); anywhere else focuses it.
    pub(crate) fn double_click_terminal(&mut self, index: usize, world: Pos2) {
        let id = self.scene.elements[index].id.clone();
        let on_header = world.y - local_bounds(&self.scene.elements[index]).min.y < TERMINAL_HEADER;
        if on_header {
            self.set_elevated(if self.terminal_elevated.as_ref() == Some(&id) { None } else { Some(id.clone()) });
        }
        self.focus_terminal(index);
    }

    /// Enlarges a terminal (None puts it back), with a short zoom animation either way.
    pub(crate) fn set_elevated(&mut self, id: Option<String>) {
        if id == self.terminal_elevated {
            return;
        }
        // a note going back ends its typing
        if self.elevated_index().is_some_and(|i| self.scene.elements[i].created.is_some()) {
            self.finish_text();
        }
        self.ide_elevated(self.terminal_elevated.clone(), &id);
        if let Some(old) = self.terminal_elevated.take() {
            self.leaving = Some((old, self.now));
        }
        self.elevate_since = self.now;
        // each enlarging starts in the middle
        self.dock = None;
        self.dock_drag = None;
        // a double-click on the title also makes the bar's Ampliar icon hop
        self.hop = Some(("expand", self.now));
        self.terminal_elevated = id;
    }

    /// The enlarged window's element, if one is.
    pub(crate) fn elevated_index(&self) -> Option<usize> {
        let id = self.terminal_elevated.as_ref()?;
        self.scene.elements.iter().position(|e| &e.id == id)
    }

    /// Double-click on a window's title: enlarged above the canvas and in focus, or back on the
    /// canvas. A note comes up with its text ready to type.
    pub(crate) fn toggle_lift(&mut self, index: usize) {
        let id = self.scene.elements[index].id.clone();
        if self.terminal_elevated.as_ref() == Some(&id) {
            self.set_elevated(None);
            return;
        }
        self.moving = false;
        self.marquee = None;
        self.selected = vec![index];
        self.set_elevated(Some(id));
        if self.scene.elements[index].created.is_some() {
            self.begin_text_edit(index);
        }
    }

    /// 0 → 1 over the enlarge animation that started at `since`, eased.
    fn elevate_progress(&self, since: f64) -> f32 {
        let t = ((self.now - since) / 0.2).clamp(0.0, 1.0) as f32;
        t * t * (3.0 - 2.0 * t)
    }

    pub(crate) fn elevate_animating(&self) -> bool {
        let running = |since: f64| self.now - since < 0.2;
        (self.terminal_elevated.is_some() && running(self.elevate_since)) || self.leaving.as_ref().is_some_and(|(_, since)| running(*since))
    }

    /// Where an enlarged terminal goes: the canvas under its bar, leaving a strip of canvas to
    /// click back to it (the corner buttons hide while it has the focus).
    pub(crate) fn elevated_rect(&self) -> Rect {
        let rect = match self.dock {
            Some((right, width)) => self.dock_rect(right, width),
            None => self.centered_rect(),
        };
        // its title being dragged: the panel follows the pointer
        match self.dock_drag {
            Some((grab, at)) => Rect::from_min_size(at - grab, rect.size()),
            None => rect,
        }
    }

    fn centered_rect(&self) -> Rect {
        let margin = 16.0;
        let top = (self.toolbar_bottom - self.canvas_rect.top() + 12.0).max(margin);
        // an open prompt composer goes under it
        let composing = self.composer.as_ref().is_some_and(|(id, _)| self.terminal_elevated.as_ref() == Some(id));
        let bottom = if composing { 72.0 } else { margin };
        Rect::from_min_max(self.canvas_rect.min + Vec2::new(margin, top), self.canvas_rect.max - Vec2::new(margin, bottom))
    }

    /// The enlarged terminal's "back to the canvas" button, in its title strip.
    pub(crate) fn restore_button(rect: Rect) -> Rect {
        Rect::from_center_size(Pos2::new(rect.right() - 18.0, rect.top() + TERMINAL_HEADER / 2.0), Vec2::splat(22.0))
    }

    /// The terminal's box on screen and its scale: on the canvas, or enlarged at 100%, and in
    /// between while it grows or shrinks back.
    pub(crate) fn terminal_frame(&self, element: &Element) -> (Rect, f32) {
        let b = local_bounds(element);
        let on_canvas = Rect::from_min_max(self.to_screen(b.min), self.to_screen(b.max));
        // a note lifted fits its box in the enlarged frame, keeping its proportions; no animation
        if element.created.is_some() {
            if self.terminal_elevated.as_ref() == Some(&element.id) {
                let target = self.elevated_rect();
                let zoom = (target.width() / b.width()).min(target.height() / b.height());
                return (Rect::from_center_size(target.center(), b.size() * zoom), zoom);
            }
            return (on_canvas, self.zoom);
        }
        let t = if self.terminal_elevated.as_ref() == Some(&element.id) {
            self.elevate_progress(self.elevate_since)
        } else {
            match &self.leaving {
                Some((id, since)) if *id == element.id => 1.0 - self.elevate_progress(*since),
                _ => 0.0,
            }
        };
        if t <= 0.0 {
            return (on_canvas, self.zoom);
        }
        let target = self.elevated_rect();
        (Rect::from_min_max(on_canvas.min.lerp(target.min, t), on_canvas.max.lerp(target.max, t)), self.zoom + (1.0 - self.zoom) * t)
    }

    pub(crate) fn paint_terminal(&self, painter: &Painter, element: &Element, rect: Rect, zoom: f32) {
        let focused = self.terminal_focus.as_ref() == Some(&element.id);
        // being typed into shows as the still dashed outline around it (paint.rs), not here
        let radius = (8.0 * zoom).min(255.0) as u8;
        painter.rect(rect, radius, TERMINAL_BG, Stroke::new(1.0, Color32::from_gray(0x3a)), StrokeKind::Inside);
        let painter = painter.with_clip_rect(rect.shrink(1.0).intersect(painter.clip_rect()));
        let spec = element.terminal.clone().unwrap_or_default();
        let (session, error) = match self.terminals.get(&element.id) {
            Some(Ok(session)) => (Some(session), None),
            Some(Err(error)) => (None, Some(error.clone())),
            None => (None, None),
        };
        let (strip, muted, light) = (Color32::from_gray(0x22), Color32::from_gray(0x8a), Color32::from_gray(0xd0));
        // title strip: activity dot and name, the shell's own title on the right
        let header = Rect::from_min_size(rect.min, Vec2::new(rect.width(), TERMINAL_HEADER * zoom));
        painter.rect_filled(header, egui::CornerRadius { nw: radius, ne: radius, sw: 0, se: 0 }, strip);
        use crate::app::ui::agents::AgentState;
        let dot = match session.map(|_| self.agent_state(&element.id)) {
            Some(AgentState::Ended) => Color32::from_rgb(0xe0, 0x31, 0x31),
            Some(AgentState::Working(_) | AgentState::Active(_)) => PRIMARY,
            Some(AgentState::Waiting) => crate::app::ui::agents::AMBER,
            Some(AgentState::Idle(_)) => Color32::from_rgb(0x2f, 0x9e, 0x44),
            None => muted,
        };
        painter.circle_filled(header.left_center() + Vec2::new(TERMINAL_PAD + 3.0, 0.0) * zoom, 3.5 * zoom, dot);
        if zoom >= 0.6 {
            let tint = spec.color.map_or(light, |[r, g, b]| Color32::from_rgb(r, g, b));
            terminal_icon(&painter, &spec, header.left_center() + Vec2::new(TERMINAL_PAD + 19.0, 0.0) * zoom, tint);
        }
        let name_at = header.left_center() + Vec2::new(TERMINAL_PAD + 30.0, 0.0) * zoom;
        let name = painter.text(name_at, egui::Align2::LEFT_CENTER, spec.label(), FontId::proportional(12.0 * zoom), light);
        if spec.maestro && zoom >= 0.6 {
            maestro_crown(&painter, name.right_center() + Vec2::new(9.0, 0.0) * zoom, zoom, crate::app::ui::agents::AMBER);
        }
        let enlarged = self.terminal_elevated.as_ref() == Some(&element.id);
        if enlarged {
            paint_icon(&painter, Icon::Expand, Self::restore_button(rect).center(), light);
        }
        if let Some(session) = session {
            session.paint(&painter, rect.min + Vec2::new(TERMINAL_PAD, TERMINAL_HEADER + 4.0) * zoom, zoom, focused);
        }
        // folder strip, and what to do when the shell is gone
        let footer = Rect::from_min_max(Pos2::new(rect.left(), rect.bottom() - TERMINAL_FOOTER * zoom), rect.max);
        painter.rect_filled(footer, egui::CornerRadius { nw: 0, ne: 0, sw: radius, se: radius }, strip);
        let small = FontId::proportional(11.0 * zoom);
        painter.text(footer.left_center() + Vec2::new(TERMINAL_PAD * zoom, 0.0), egui::Align2::LEFT_CENTER, display_folder(&spec.cwd), small.clone(), muted);
        let note = match (session, error) {
            (_, Some(error)) => Some(format!("Não foi possível abrir o shell: {error}")),
            (Some(s), _) if s.exited() => Some("Processo encerrado — clique para reiniciar".into()),
            (None, None) if spec.unloaded => Some("Este terminal está descarregado.".into()),
            _ => None,
        };
        if let Some(note) = note {
            painter.text(footer.right_center() - Vec2::new(TERMINAL_PAD * zoom, 0.0), egui::Align2::RIGHT_CENTER, note, small, light);
        }
    }

    /// The focused or enlarged terminal's own layer above the canvas. It holds egui's focus
    /// while typing, so the app's shortcuts stay off and Tab, arrows and Esc reach the shell,
    /// and takes the wheel for the scrollback. The canvas title strip stays draggable.
    pub(crate) fn terminal_overlay(&mut self, ctx: &Context) {
        let find = |id: &Option<String>| id.as_ref().and_then(|id| self.scene.elements.iter().find(|e| &e.id == id && e.terminal.is_some())).cloned();
        if self.elevate_animating() {
            ctx.request_repaint();
        }
        let (area, element) = match (find(&self.terminal_elevated), find(&self.terminal_focus)) {
            (Some(e), _) => (self.terminal_frame(&e).0, e),
            (None, Some(e)) => {
                let (frame, zoom) = self.terminal_frame(&e);
                let grid = Rect::from_min_max(frame.min + Vec2::new(0.0, TERMINAL_HEADER * zoom), frame.max - Vec2::new(0.0, TERMINAL_FOOTER * zoom));
                // its side bands stay canvas, so the edges resize it from the grip band
                (grid.shrink2(Vec2::new(4.0, 0.0)).intersect(self.canvas_rect), e)
            }
            (None, None) => return,
        };
        let enlarged = self.terminal_elevated.as_ref() == Some(&element.id);
        // a press elsewhere on the canvas ends typing (the floating UI handles its own clicks;
        // one that opens a text field takes the keyboard). Its title strip is still the node:
        // dragging it keeps the focus.
        let frame = self.terminal_frame(&element).0;
        // not a pan: the middle button or Space drag the view, not away from it (a right click still
        // ends it: its menu's keys are the canvas's)
        let press =
            ctx.input(|i| (i.pointer.primary_pressed() || i.pointer.secondary_pressed()).then(|| i.pointer.interact_pos()).flatten()).filter(|_| !self.panning);
        let on_canvas = press.is_some_and(|p| !frame.contains(p) && ctx.layer_id_at(p).is_none_or(|l| l.order == egui::Order::Background));
        if on_canvas && enlarged && self.dock.is_none() {
            // a click on the margin goes back to the canvas
            self.set_elevated(None);
            self.terminal_focus = None;
            return;
        }
        if on_canvas || self.palette_open || self.dialog_open() || self.composer.is_some() {
            self.terminal_focus = None;
            if !enlarged {
                return;
            }
        }
        if !area.is_positive() {
            self.terminal_focus = None;
            return;
        }
        let typing = self.terminal_focus.as_ref() == Some(&element.id);
        let (mut wheel, mut header_double, mut clicked, mut secondary) = (0.0, false, false, false);
        egui::Area::new(egui::Id::unique("terminal_overlay")).fixed_pos(area.min).order(egui::Order::Middle).show(ctx, |ui| {
            let (rect, response) = ui.allocate_exact_size(area.size(), Sense::click());
            if enlarged {
                self.paint_terminal(ui.painter(), &element, rect, self.terminal_frame(&element).1);
                let header = Rect::from_min_size(rect.min, Vec2::new(rect.width(), TERMINAL_HEADER));
                let at = response.interact_pointer_pos();
                header_double = response.double_clicked() && at.is_some_and(|p| header.contains(p))
                    || response.clicked() && at.is_some_and(|p| Self::restore_button(rect).contains(p));
            }
            if typing {
                response.request_focus();
                let all = egui::EventFilter { tab: true, horizontal_arrows: true, vertical_arrows: true, escape: true };
                ui.memory_mut(|m| m.set_focus_lock_filter(response.id, all));
            }
            if response.hovered() {
                wheel = ui.input(|i| i.smooth_scroll_delta.y);
            }
            clicked = response.clicked();
            secondary = response.secondary_clicked();
            response.on_hover_cursor(CursorIcon::Text);
        });
        if header_double {
            self.set_elevated(None);
            return;
        }
        // a right click on the window is its menu (the canvas never sees it)
        if secondary {
            if let Some(at) = ctx.input(|i| i.pointer.interact_pos()) {
                if let Some(index) = self.scene.elements.iter().position(|e| e.id == element.id) {
                    if !self.selected.contains(&index) {
                        self.selected = vec![index];
                    }
                }
                self.context = Some(ContextMenu::new(at, Target::Element(element.id.clone()), None));
            }
        }
        if clicked && enlarged {
            self.terminal_focus = Some(element.id.clone());
        }
        let Some(Ok(session)) = self.terminals.get(&element.id) else {
            return;
        };
        if typing {
            let mode = session.handle.mode();
            let mut bytes = Vec::new();
            for event in ctx.input(|i| i.events.clone()) {
                match event {
                    egui::Event::Text(text) | egui::Event::Ime(egui::ImeEvent::Commit(text)) => bytes.extend(text.into_bytes()),
                    // ⌘K clears the buffer, as in the reference app (Terminal.app does the same); off the Mac
                    // Ctrl+K is the shell's (kill to the end of the line), as in Windows Terminal and PhpStorm
                    egui::Event::Key { key: Key::K, pressed: true, modifiers, .. } if modifiers.mac_cmd => session.handle.clear(),
                    egui::Event::Key { key, pressed: true, modifiers, .. } => {
                        bytes.extend(key_bytes(key, modifiers, mode.contains(TermMode::APP_CURSOR)).unwrap_or_default());
                    }
                    egui::Event::Paste(text) => bytes.extend(paste_bytes(&text, mode.contains(TermMode::BRACKETED_PASTE))),
                    // elsewhere Ctrl+C / Ctrl+X come as copy / cut; with no selection they're the keys. Windows
                    // also turns Ctrl+Insert / Shift+Delete into them: those copy and cut nothing here
                    egui::Event::Copy if !cfg!(target_os = "macos") && !edit_key_held() => bytes.push(0x03),
                    egui::Event::Cut if !cfg!(target_os = "macos") && !edit_key_held() => bytes.push(0x18),
                    _ => {}
                }
            }
            if !bytes.is_empty() {
                session.handle.write(bytes);
            }
        }
        let zoom = if enlarged { 1.0 } else { self.zoom };
        self.terminal_scroll += wheel / (CELL.y * zoom);
        let lines = self.terminal_scroll.trunc();
        if lines != 0.0 {
            session.scroll(lines as i32);
            self.terminal_scroll -= lines;
        }
    }
}

/// Insert or Delete held down: egui-winit sends Ctrl+Insert and Shift+Delete as copy and cut on
/// Windows, with no key event to tell them from Ctrl+C and Ctrl+X.
fn edit_key_held() -> bool {
    #[cfg(windows)]
    {
        unsafe extern "system" {
            fn GetKeyState(key: i32) -> i16;
        }
        const VK_INSERT: i32 = 0x2D;
        const VK_DELETE: i32 = 0x2E;
        // the state as of the key message being handled: the top bit is "down"
        unsafe { GetKeyState(VK_INSERT) < 0 || GetKeyState(VK_DELETE) < 0 }
    }
    #[cfg(not(windows))]
    false
}
