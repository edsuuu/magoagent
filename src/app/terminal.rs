//! Terminals on the canvas: their shells, typing into them, painting them, and the lines that
//! connect them so agents can reach each other through the bridge.

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
        let mut links: HashMap<String, Vec<String>> = HashMap::new();
        for e in &self.scene.elements {
            if let (ElementKind::Line { .. }, Some(a), Some(b)) = (&e.kind, &e.ends.start, &e.ends.end) {
                if a.element_id != b.element_id {
                    links.entry(a.element_id.clone()).or_default().push(b.element_id.clone());
                    links.entry(b.element_id.clone()).or_default().push(a.element_id.clone());
                }
            }
        }
        links
    }

    /// What agents can reach: terminals, sticky notes (their label is the text) and portals.
    pub(crate) fn agent_nodes(&self) -> Vec<Node> {
        let links = self.links();
        let label = |id: &str| {
            let label_id = format!("{id}-label");
            self.scene.elements.iter().find_map(|e| match &e.kind {
                ElementKind::Text { value, .. } if e.id == label_id => Some(value.clone()),
                _ => None,
            })
        };
        let named = |name: &str, fallback: &str| if name.trim().is_empty() { fallback.to_string() } else { name.trim().to_string() };
        self.scene
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
                } else {
                    return None;
                };
                Some(Node { id: e.id.clone(), kind, name, text, links: links.get(&e.id).cloned().unwrap_or_default() })
            })
            .collect()
    }

    fn terminal_env(&self, id: &str, name: &str) -> Vec<(String, String)> {
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
        env
    }

    /// One shell per terminal element: new ones start, removed ones end, sizes follow the box.
    /// Also tells when an agent finishes and keeps the bridge's view of the canvas current.
    pub(crate) fn sync_terminals(&mut self, ctx: &Context) {
        let elevated = self.elevated_rect();
        let live: Vec<(String, TerminalSpec, Vec2)> = self
            .scene
            .elements
            .iter()
            .filter_map(|e| {
                let size = if self.terminal_elevated.as_ref() == Some(&e.id) { elevated.size() } else { local_bounds(e).size() };
                Some((e.id.clone(), e.terminal.clone()?, size))
            })
            .collect();
        self.terminals.retain(|id, _| live.iter().any(|(live, ..)| live == id));
        for (id, spec, size) in &live {
            match self.terminals.get_mut(id) {
                Some(Ok(session)) => session.resize(grid_size(*size)),
                Some(Err(_)) => {}
                None => {
                    let env = self.terminal_env(id, &spec.name);
                    let session = TerminalSession::spawn(&spec.cwd, &spec.command, grid_size(*size), Some(ctx.clone()), env, None);
                    self.terminals.insert(id.clone(), session.map_err(|e| e.to_string()));
                }
            }
        }
        if self.leaving.as_ref().is_some_and(|(_, since)| self.now - since > 0.2) {
            self.leaving = None;
        }
        if self.terminal_focus.as_ref().is_some_and(|id| !self.terminals.contains_key(id)) {
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
            UiCommand::Recruit { from, name, command, reply } => self.recruit(&from, &name, &command, reply),
            UiCommand::CreatePortal { from, url, name, reply } => self.create_portal(&from, &url, &name, reply),
            UiCommand::Connect { a, b, reply } => self.connect_for_agent(&a, &b, reply),
            UiCommand::Notify { from, text } => self.notify(&from, &text),
        }
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

    /// What an agent drew through the MCP server hangs from its terminal: one rope to the
    /// biggest new shape, unless the terminal already holds the drawing it joins.
    pub(crate) fn tie_to_terminal(&mut self, terminal: &str, fresh: &[String]) {
        if !self.scene.elements.iter().any(|e| e.id == terminal && e.terminal.is_some()) {
            return;
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
    pub(crate) fn only_selected(&self, f: fn(&Element) -> bool) -> bool {
        !self.selected.is_empty() && self.selected.iter().all(|i| self.scene.elements.get(*i).is_some_and(f))
    }

    /// Moves every rope on: gravity pulls its links, each link keeps its length, its ends follow
    /// what they're tied to and it slides around the windows in its way. True while any moves.
    pub(crate) fn swing_ropes(&mut self, dt: f32) -> bool {
        // a rope needs both ends: whatever removed one (Delete, eraser, MCP, undo) takes the rope too
        // (not while an end is being dragged: the press unbinds it until the release binds it again)
        let exists = |b: &Option<Binding>| b.as_ref().is_some_and(|b| self.scene.elements.iter().any(|e| e.id == b.element_id));
        let loose: Vec<String> = self.scene.elements.iter().filter(|e| e.rope && !(exists(&e.ends.start) && exists(&e.ends.end))).map(|e| e.id.clone()).collect();
        if !loose.is_empty() && self.point_drag.is_none() {
            self.scene.elements.retain(|e| !loose.contains(&e.id));
            self.dirty = true;
        }
        let nodes: Vec<(String, Rect)> = self.scene.elements.iter().filter(|e| e.is_node() || e.created.is_some()).map(|e| (e.id.clone(), element_bounds(e))).collect();
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
            [i] => self.scene.elements.get(i).filter(|e| e.is_node()),
            _ => None,
        };
        self.terminal_focus.as_ref().and_then(by_id).or_else(|| self.terminal_elevated.as_ref().and_then(by_id)).or(selected)
    }

    /// A name no other terminal has: "Shell", "Shell 2"…
    pub(crate) fn unique_terminal_name(&self, base: &str, except: Option<&str>) -> String {
        let taken = |name: &str| {
            self.scene.elements.iter().any(|e| Some(e.id.as_str()) != except && e.terminal.as_ref().is_some_and(|t| t.name == name))
        };
        (1..).map(|n| if n == 1 { base.to_string() } else { format!("{base} {n}") }).find(|name| !taken(name)).unwrap_or_default()
    }

    /// Click: an 80×24 terminal centered there; drag: that box. Asks for its shell first.
    pub(crate) fn open_terminal_dialog(&mut self, start: Pos2, end: Pos2) {
        let at = if start.distance(end) * self.zoom < 10.0 {
            Rect::from_center_size(start, terminal_size(80, 24))
        } else {
            let r = Rect::from_two_pos(start, end);
            Rect::from_min_size(r.min, r.size().max(Vec2::new(200.0, 120.0)))
        };
        let folder = self.file_path.as_ref().and_then(|p| p.parent()).map(|p| p.to_string_lossy().to_string());
        let spec = TerminalSpec { name: self.unique_terminal_name("Shell", None), cwd: folder.unwrap_or_default(), command: String::new(), ..Default::default() };
        self.terminal_dialog = Some(TerminalDialog { target: None, at, spec, tab: 0 });
        if !self.tool_locked {
            self.tool = Tool::Select;
        }
    }

    /// Creates the dialog's terminal, or applies it to the one being edited (restarting it
    /// when the folder or command changed).
    pub(crate) fn confirm_terminal_dialog(&mut self) {
        let Some(TerminalDialog { target, at, mut spec, .. }) = self.terminal_dialog.take() else { return };
        spec.name = self.unique_terminal_name(if spec.name.trim().is_empty() { "Terminal" } else { spec.name.trim() }, target.as_deref());
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
        terminal.terminal = Some(spec);
        self.terminal_focus = Some(terminal.id.clone());
        self.scene.elements.push(terminal);
        self.selected = vec![self.scene.elements.len() - 1];
    }

    /// Double-click: type into it (and restart its shell if it had ended); on the title strip,
    /// enlarge it (or put it back).
    pub(crate) fn double_click_terminal(&mut self, index: usize, world: Pos2) {
        let id = self.scene.elements[index].id.clone();
        let on_header = world.y - local_bounds(&self.scene.elements[index]).min.y < TERMINAL_HEADER;
        if on_header {
            self.set_elevated(if self.terminal_elevated.as_ref() == Some(&id) { None } else { Some(id.clone()) });
        }
        if self.terminals.get(&id).is_some_and(|s| s.as_ref().map_or(true, |s| s.exited())) {
            self.terminals.remove(&id);
        }
        self.moving = false;
        self.marquee = None;
        // stays selected while typing, so its bar stays up
        self.selected = vec![index];
        self.terminal_focus = Some(id);
    }

    /// Enlarges a terminal (None puts it back), with a short zoom animation either way.
    pub(crate) fn set_elevated(&mut self, id: Option<String>) {
        if id == self.terminal_elevated {
            return;
        }
        self.ide_elevated(self.terminal_elevated.clone(), &id);
        if let Some(old) = self.terminal_elevated.take() {
            self.leaving = Some((old, self.now));
        }
        self.elevate_since = self.now;
        // a double-click on the title also makes the bar's Ampliar icon hop
        self.hop = Some(("expand", self.now));
        self.terminal_elevated = id;
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

    /// Where an enlarged terminal goes: most of the canvas under the toolbars, leaving a margin
    /// of canvas to click back to it.
    pub(crate) fn elevated_rect(&self) -> Rect {
        let margin = (self.canvas_rect.size() * 0.05).max(Vec2::splat(40.0));
        let top = (self.toolbar_bottom - self.canvas_rect.top() + 16.0).max(margin.y);
        // the bottom margin clears the zoom buttons and holds the prompt composer
        let bottom = margin.y.max(72.0);
        Rect::from_min_max(self.canvas_rect.min + Vec2::new(margin.x, top), self.canvas_rect.max - Vec2::new(margin.x, bottom))
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
        let dot = match session {
            Some(s) if s.exited() => Color32::from_rgb(0xe0, 0x31, 0x31),
            Some(s) if s.handle.busy() => PRIMARY,
            Some(_) if self.terminal_waiting.contains(&element.id) => crate::app::ui::agents::AMBER,
            Some(_) => Color32::from_rgb(0x2f, 0x9e, 0x44),
            None => muted,
        };
        painter.circle_filled(header.left_center() + Vec2::new(TERMINAL_PAD + 3.0, 0.0) * zoom, 3.5 * zoom, dot);
        if zoom >= 0.6 {
            let tint = spec.color.map_or(light, |[r, g, b]| Color32::from_rgb(r, g, b));
            terminal_icon(&painter, &spec, header.left_center() + Vec2::new(TERMINAL_PAD + 19.0, 0.0) * zoom, tint);
        }
        let name_at = header.left_center() + Vec2::new(TERMINAL_PAD + 30.0, 0.0) * zoom;
        painter.text(name_at, egui::Align2::LEFT_CENTER, spec.label(), FontId::proportional(12.0 * zoom), light);
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
            (Some(s), _) if s.exited() => Some("Processo encerrado — clique duas vezes para reiniciar".into()),
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
                (grid.intersect(self.canvas_rect), e)
            }
            (None, None) => return,
        };
        let enlarged = self.terminal_elevated.as_ref() == Some(&element.id);
        // a press elsewhere on the canvas ends typing (the floating UI handles its own clicks;
        // one that opens a text field takes the keyboard)
        let press = ctx.input(|i| i.pointer.any_pressed().then(|| i.pointer.interact_pos()).flatten());
        let on_canvas = press.is_some_and(|p| !area.contains(p) && ctx.layer_id_at(p).is_none_or(|l| l.order == egui::Order::Background));
        if on_canvas && enlarged {
            // a click on the margin goes back to the canvas
            self.set_elevated(None);
            self.terminal_focus = None;
            return;
        }
        if on_canvas || self.palette_open || self.export_open || self.dialog_open() || self.composer.is_some() {
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
        let (mut wheel, mut header_double, mut clicked) = (0.0, false, false);
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
            response.on_hover_cursor(CursorIcon::Text);
        });
        if header_double {
            self.set_elevated(None);
            return;
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
                    egui::Event::Key { key, pressed: true, modifiers, .. } => {
                        bytes.extend(key_bytes(key, modifiers, mode.contains(TermMode::APP_CURSOR)).unwrap_or_default());
                    }
                    egui::Event::Paste(text) => bytes.extend(paste_bytes(&text, mode.contains(TermMode::BRACKETED_PASTE))),
                    // elsewhere Ctrl+C / Ctrl+X come as copy / cut; with no selection they're the keys
                    egui::Event::Copy if !cfg!(target_os = "macos") => bytes.push(0x03),
                    egui::Event::Cut if !cfg!(target_os = "macos") => bytes.push(0x18),
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
