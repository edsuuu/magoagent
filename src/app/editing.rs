//! Editing actions: groups, clipboard, layers, zoom and text.

use crate::prelude::*;

/// The zoom steps of − / +: the reference's 16 up to 300%, then ours up to the 3000% limit.
const ZOOM_LEVELS: [f32; 26] =
    [0.1, 0.15, 0.25, 0.33, 0.5, 0.67, 0.75, 0.9, 1.0, 1.1, 1.25, 1.5, 1.75, 2.0, 2.5, 3.0, 4.0, 5.0, 6.0, 8.0, 10.0, 12.0, 15.0, 20.0, 25.0, 30.0];

impl DrawingApp {
    pub(crate) fn select_tool(&mut self, tool: Tool) {
        self.finish_text();
        self.finish_multi();
        self.line_editor = None;
        self.terminal_focus = None;
        self.ide_focus = None;
        self.card_focus = None;
        self.connecting = None;
        self.tool = tool;
        if !matches!(tool, Tool::Select | Tool::Hand) {
            self.selected.clear();
        }
    }

    pub(crate) fn delete_selected(&mut self) {
        if self.selected.is_empty() {
            return;
        }
        self.snapshot();
        self.selected = self.with_card_text(&self.selected);
        self.selected.sort_unstable();
        self.selected.dedup();
        for index in self.selected.drain(..).rev() {
            if index < self.scene.elements.len() {
                self.scene.elements.remove(index);
            }
        }
    }

    pub(crate) fn clear_canvas(&mut self) {
        self.snapshot();
        self.scene.elements.clear();
        self.selected.clear();
        (self.pan, self.zoom, self.glide) = (Vec2::ZERO, 1.0, None);
    }

    pub(crate) fn group_selection(&mut self) {
        if self.selected.len() < 2 {
            return;
        }
        self.snapshot();
        let group_id = new_id();
        for index in self.selected.iter().copied() {
            if let Some(element) = self.scene.elements.get_mut(index) {
                element.group_id = Some(group_id.clone());
            }
        }
    }

    pub(crate) fn ungroup_selection(&mut self) {
        if !self.selected_has_group() {
            return;
        }
        self.snapshot();
        for index in self.selected.iter().copied() {
            if let Some(element) = self.scene.elements.get_mut(index) {
                element.group_id = None;
            }
        }
    }

    pub(crate) fn selected_has_group(&self) -> bool {
        self.selected.iter().any(|i| self.scene.elements.get(*i).is_some_and(|e| e.group_id.is_some()))
    }

    pub(crate) fn select_all(&mut self) {
        self.tool = Tool::Select;
        self.selected = (0..self.scene.elements.len()).filter(|i| !self.scene.elements[*i].locked && !self.is_card_text(*i)).collect();
    }

    /// Clones elements with fresh ids and group ids, so pasted groups stay separate groups.
    pub(crate) fn fresh_copies(elements: &[Element], offset: Vec2) -> Vec<Element> {
        let mut groups = std::collections::HashMap::new();
        let ids: std::collections::HashMap<&str, String> = elements.iter().map(|e| (e.id.as_str(), new_id())).collect();
        // a note's text keeps the "-label" id of its card's copy, so it stays that card's text
        let text_id = |e: &Element| match e.id.strip_suffix("-label").and_then(|owner| ids.get(owner)) {
            Some(owner) if matches!(e.kind, ElementKind::Text { .. }) => format!("{owner}-label"),
            _ => ids[e.id.as_str()].clone(),
        };
        // a binding survives only when its target was copied too
        let remap = |b: &Option<Binding>| b.as_ref().and_then(|b| ids.get(b.element_id.as_str()).map(|id| Binding { element_id: id.clone(), ..b.clone() }));
        elements
            .iter()
            .map(|e| {
                let mut copy = e.clone();
                copy.id = text_id(e);
                copy.ends.start = remap(&e.ends.start);
                copy.ends.end = remap(&e.ends.end);
                copy.group_id = e.group_id.as_ref().map(|g| groups.entry(g.clone()).or_insert_with(new_id).clone());
                copy.move_by(offset);
                copy
            })
            .collect()
    }

    pub(crate) fn insert_elements(&mut self, elements: Vec<Element>) {
        if elements.is_empty() {
            return;
        }
        self.snapshot();
        let start = self.scene.elements.len();
        for mut e in elements {
            // a copied terminal is a new agent with a name of its own: the bridge calls terminals by name
            if let Some(t) = e.terminal.as_mut().filter(|t| self.scene.elements.iter().any(|o| o.terminal.as_ref().is_some_and(|o| o.name == t.name))) {
                t.name = self.agent_name();
            }
            self.scene.elements.push(e);
        }
        self.selected = (start..self.scene.elements.len()).filter(|i| !self.is_card_text(*i)).collect();
        self.tool = Tool::Select;
    }

    pub(crate) fn duplicate(&mut self) {
        let chosen: Vec<Element> = self.with_card_text(&self.selected).iter().filter_map(|i| self.scene.elements.get(*i).cloned()).collect();
        self.insert_elements(Self::fresh_copies(&chosen, Vec2::splat(10.0)));
    }

    pub(crate) fn copy(&mut self, ctx: &Context) {
        self.clipboard = self.with_card_text(&self.selected).iter().filter_map(|i| self.scene.elements.get(*i).cloned()).collect();
        if !self.clipboard.is_empty() {
            ctx.copy_text(json!({ "type": "magoagent/clipboard", "elements": interop_elements(&self.clipboard) }).to_string());
        }
    }

    pub(crate) fn paste(&mut self, text: &str) {
        let parsed = serde_json::from_str::<Value>(text).ok().filter(|v| v["type"] == "magoagent/clipboard").map(|v| from_interop_elements(&v["elements"]));
        let elements = match parsed {
            Some(elements) => elements,
            None if !text.trim().is_empty() => {
                vec![self.new_element(ElementKind::Text { x: 0.0, y: 0.0, value: text.trim_end().into() })]
            }
            None if !self.clipboard.is_empty() => self.clipboard.clone(),
            None => return,
        };
        // Paste centered under the cursor, like the reference app.
        let bounds = elements.iter().filter(|e| note_of_label(&elements, e).is_none()).map(element_bounds).reduce(|a, b| a.union(b)).unwrap_or(Rect::ZERO);
        let target = self.last_pointer.filter(|p| self.canvas_rect.contains(*p)).map(|p| self.to_world(p)).unwrap_or(bounds.center() + Vec2::splat(10.0));
        self.insert_elements(Self::fresh_copies(&elements, target - bounds.center()));
    }

    pub(crate) fn nudge(&mut self, delta: Vec2) {
        if self.selected.is_empty() {
            return;
        }
        self.snapshot();
        self.detach_selected_arrows();
        for i in &self.selected {
            if let Some(e) = self.scene.elements.get_mut(*i) {
                e.move_by(delta);
            }
        }
    }

    pub(crate) fn reorder(&mut self, layer: Layer) {
        if self.selected.is_empty() {
            return;
        }
        self.snapshot();
        let mut items: Vec<(bool, Element)> =
            std::mem::take(&mut self.scene.elements).into_iter().enumerate().map(|(i, e)| (self.selected.contains(&i), e)).collect();
        let n = items.len();
        match layer {
            Layer::Forward => {
                for i in (0..n.saturating_sub(1)).rev() {
                    if items[i].0 && !items[i + 1].0 {
                        items.swap(i, i + 1);
                    }
                }
            }
            Layer::Backward => {
                for i in 1..n {
                    if items[i].0 && !items[i - 1].0 {
                        items.swap(i, i - 1);
                    }
                }
            }
            Layer::Front => items.sort_by_key(|(s, _)| *s),
            Layer::Back => items.sort_by_key(|(s, _)| !*s),
        }
        self.selected = items.iter().enumerate().filter_map(|(i, (s, _))| s.then_some(i)).collect();
        self.scene.elements = items.into_iter().map(|(_, e)| e).collect();
    }

    pub(crate) fn zoom_at(&mut self, zoom: f32, anchor: Pos2) {
        let world = self.to_world(anchor);
        self.glide = None;
        self.zoom = zoom.clamp(0.1, 30.0);
        self.pan = anchor - self.canvas_rect.min - world.to_vec2() * self.zoom;
    }

    /// − / + and their shortcuts: the next step down or up (only the sign of `delta` counts),
    /// from the middle of the screen.
    pub(crate) fn zoom_step(&mut self, delta: f32) {
        let z = self.zoom;
        let next = if delta > 0.0 { ZOOM_LEVELS.iter().find(|l| **l > z + 1e-3) } else { ZOOM_LEVELS.iter().rev().find(|l| **l < z - 1e-3) };
        self.zoom_at(next.copied().unwrap_or(z), self.canvas_rect.center());
    }

    /// The wheel or a pinch asks for `factor`; one frame moves at most 0.4 (up to 300%, then the
    /// same share of the zoom), so a fast spin doesn't jump.
    pub(crate) fn wheel_zoom(&mut self, factor: f32, anchor: Pos2) {
        let cap = 0.4 * (self.zoom / 3.0).max(1.0);
        self.zoom_at(self.zoom + (self.zoom * factor - self.zoom).clamp(-cap, cap), anchor);
    }

    /// Grid snap for points (drawing), when the grid is on.
    pub(crate) fn snap_point(&self, p: Pos2) -> Pos2 {
        if self.prefs.grid && !self.mods.command { Pos2::new((p.x / GRID).round() * GRID, (p.y / GRID).round() * GRID) } else { p }
    }

    /// Offset that snaps a moved box to other elements' edges/centers (Encaixar em objetos)
    /// or to the grid, plus the red guide lines to show. Cmd disables snapping; on a window it
    /// tiles it against the others instead (`tile_box`).
    pub(crate) fn snap_box(&self, moved: Rect) -> (Vec2, Vec<[Pos2; 2]>) {
        if self.mods.command {
            let moving = self.move_origin.as_ref().map_or(&self.selected, |(moving, ..)| moving);
            let window = |i: &usize| self.scene.elements.get(*i).is_some_and(|e| e.is_node() || e.created.is_some());
            return if moving.iter().any(window) { self.tile_box(moved) } else { (Vec2::ZERO, Vec::new()) };
        }
        if self.prefs.snap_objects {
            let threshold = 8.0 / self.zoom;
            // nothing that goes along can be a target: what the drag moves (a frame's contents, a
            // note's text) and the ropes and arrows tied to it
            let moving = self.move_origin.as_ref().map_or(&self.selected, |(moving, ..)| moving);
            let ids: std::collections::HashSet<&str> = moving.iter().filter_map(|i| self.scene.elements.get(*i)).map(|e| e.id.as_str()).collect();
            let tied = |e: &Element| [&e.ends.start, &e.ends.end].into_iter().flatten().any(|b| ids.contains(b.element_id.as_str()));
            // and the labels centered on any of those
            let along: std::collections::HashSet<&str> =
                self.scene.elements.iter().filter(|e| ids.contains(e.id.as_str()) || tied(e)).map(|e| e.id.as_str()).collect();
            let label_along = |e: &Element| is_label(e) && e.id.strip_suffix("-label").is_some_and(|owner| along.contains(owner));
            let others: Vec<Rect> = self
                .scene
                .elements
                .iter()
                .enumerate()
                .filter(|(i, e)| !moving.contains(i) && !self.is_card_text(*i) && !tied(e) && !label_along(e))
                .map(|(_, e)| element_bounds(e))
                .collect();
            let mut adjust = Vec2::ZERO;
            let mut guides = Vec::new();
            for axis in 0..2 {
                let lines = |r: &Rect| if axis == 0 { [r.min.x, r.center().x, r.max.x] } else { [r.min.y, r.center().y, r.max.y] };
                let best = others
                    .iter()
                    .flat_map(|o| lines(o).into_iter().map(move |v| (v, *o)))
                    .flat_map(|(v, o)| lines(&moved).into_iter().map(move |m| (v - m, v, o)))
                    .filter(|(d, _, _)| d.abs() <= threshold)
                    .min_by(|a, b| a.0.abs().total_cmp(&b.0.abs()));
                if let Some((d, v, o)) = best {
                    adjust[axis] = d;
                    let span = moved.translate(adjust).union(o);
                    guides.push(if axis == 0 {
                        [Pos2::new(v, span.min.y), Pos2::new(v, span.max.y)]
                    } else {
                        [Pos2::new(span.min.x, v), Pos2::new(span.max.x, v)]
                    });
                }
            }
            if adjust != Vec2::ZERO {
                return (adjust, guides);
            }
        }
        if self.prefs.grid {
            return (self.snap_point(moved.min) - moved.min, Vec::new());
        }
        (Vec2::ZERO, Vec::new())
    }

    /// Magnetic tiles (Cmd/Ctrl while a window is dragged): on each axis the box goes to the nearest
    /// place among the windows and notes beside it, within 48 px of screen: one grid step after or
    /// before one, its walls in line with one's, or centered in a gap between two. No fixed grid.
    pub(crate) fn tile_box(&self, moved: Rect) -> (Vec2, Vec<[Pos2; 2]>) {
        let reach = 48.0 / self.zoom;
        let moving = self.move_origin.as_ref().map_or(&self.selected, |(moving, ..)| moving);
        let others: Vec<Rect> = (self.scene.elements.iter().enumerate())
            .filter(|(i, e)| !moving.contains(i) && !e.rope && (e.is_node() || e.created.is_some()))
            .map(|(_, e)| element_bounds(e))
            .collect();
        let (mut adjust, mut guides) = (Vec2::ZERO, Vec::new());
        for axis in 0..2 {
            let size = moved.size()[axis];
            // the ones beside it on this axis: overlapping it across, give or take the reach
            let across = 1 - axis;
            let beside: Vec<&Rect> = others.iter().filter(|o| o.min[across] < moved.max[across] + reach && o.max[across] > moved.min[across] - reach).collect();
            // (where its min goes, the wall it lines up with, the one that wall belongs to)
            let mut places: Vec<(f32, f32, &Rect)> = Vec::new();
            for o in &beside {
                places.extend([
                    (o.max[axis] + GRID, o.max[axis], *o),
                    (o.min[axis] - GRID - size, o.min[axis], *o),
                    (o.min[axis], o.min[axis], *o),
                    (o.max[axis] - size, o.max[axis], *o),
                ]);
                for b in &beside {
                    let room = b.min[axis] - o.max[axis];
                    if room >= size && (o.max[axis]..b.min[axis]).contains(&moved.center()[axis]) {
                        places.push((o.max[axis] + (room - size) / 2.0, o.max[axis], *o));
                    }
                }
            }
            let best = places
                .into_iter()
                .filter(|(at, ..)| (at - moved.min[axis]).abs() <= reach)
                .min_by(|a, b| (a.0 - moved.min[axis]).abs().total_cmp(&(b.0 - moved.min[axis]).abs()));
            if let Some((at, wall, o)) = best {
                adjust[axis] = at - moved.min[axis];
                let span = moved.translate(adjust).union(*o);
                guides.push(if axis == 0 {
                    [Pos2::new(wall, span.min.y), Pos2::new(wall, span.max.y)]
                } else {
                    [Pos2::new(span.min.x, wall), Pos2::new(span.max.x, wall)]
                });
            }
        }
        (adjust, guides)
    }

    /// Centers the canvas on a world point at once (the minimap, which follows the pointer).
    pub(crate) fn pan_to(&mut self, world: Pos2) {
        self.glide = None;
        self.pan = self.canvas_rect.size() / 2.0 - world.to_vec2() * self.zoom;
    }

    /// Centers the canvas on a world point, gliding there in 0.35 s unless Menu → Preferências
    /// says Instantâneo. A pan or a zoom on the way stops it where it is.
    pub(crate) fn glide_to(&mut self, world: Pos2) {
        let from = self.pan;
        self.pan_to(world);
        if !self.prefs.instant_camera && from != self.pan {
            self.glide = Some((self.now, from, self.pan));
            self.pan = from;
        }
    }

    // ponytail: an ease-out cubic in place of the reference's spring without bounce
    pub(crate) fn step_glide(&mut self) {
        let Some((since, from, to)) = self.glide else { return };
        let t = ((self.now - since) / 0.35).min(1.0) as f32;
        self.pan = from + (to - from) * (1.0 - (1.0 - t).powi(3));
        if t >= 1.0 {
            self.glide = None;
        }
    }

    /// World area the minimap covers: every element plus what is on screen, with a margin.
    pub(crate) fn minimap_world(&self) -> Rect {
        let view = Rect::from_two_pos(self.to_world(self.canvas_rect.min), self.to_world(self.canvas_rect.max));
        let all = self.scene.elements.iter().filter(|e| note_of_label(&self.scene.elements, e).is_none()).map(element_bounds).fold(view, |a, b| a.union(b));
        all.expand(all.size().max_elem() * 0.05 + 20.0)
    }

    pub(crate) fn zoom_to_fit(&mut self, selection: bool) {
        let bounds = if selection {
            self.selection_bounds()
        } else {
            self.scene.elements.iter().filter(|e| note_of_label(&self.scene.elements, e).is_none()).map(element_bounds).reduce(|a, b| a.union(b))
        };
        let (Some(bounds), true) = (bounds, self.canvas_rect.is_positive()) else {
            return;
        };
        let view = self.canvas_rect.size() - Vec2::splat(120.0);
        let zoom = (view.x / bounds.width().max(1.0)).min(view.y / bounds.height().max(1.0));
        self.zoom = if selection { zoom.clamp(0.1, 30.0) } else { zoom.clamp(0.1, 1.0) };
        self.glide = None;
        self.pan = self.canvas_rect.size() / 2.0 - bounds.center().to_vec2() * self.zoom;
    }

    /// Starts typing into a text element; a sticky note's text is its label.
    pub(crate) fn begin_text_edit(&mut self, index: usize) {
        // a note's text is typed in place: the card stays the selection
        let (card, text) = match self.scene.elements.get(index) {
            Some(e) if e.created.is_some() && e.note.content_locked => return,
            Some(e) if e.created.is_some() => match note_label(&self.scene.elements, index) {
                Some(label) => (index, label),
                None => return,
            },
            _ => (index, index),
        };
        self.snapshot();
        self.selected = vec![card];
        self.editing = Some(text);
        self.edit_focused = false;
    }

    /// The shape or arrow a click inside it falls on, if it takes a label. A transparent shape is
    /// hit only on its outline, so the inside counts here too.
    pub(crate) fn containing_shape(&self, world: Pos2) -> Option<usize> {
        self.scene.elements.iter().rposition(|e| takes_label(e) && !matches!(e.kind, ElementKind::Line { .. }) && local_bounds(e).contains(e.to_local(world)))
    }

    /// A terminal, portal, IDE or sticky note under `world`: the text tool and double-clicks leave
    /// those to the window instead of writing over them.
    pub(crate) fn window_at(&self, world: Pos2) -> bool {
        self.scene.elements.iter().any(|e| (e.is_node() || e.created.is_some()) && element_bounds(e).contains(world))
    }

    /// The label of a shape or arrow (`<id>-label`, the MCP `label` makes the same), created empty
    /// on its first double-click and kept centered on its owner.
    pub(crate) fn owner_label(&mut self, owner: usize) -> usize {
        let id = format!("{}-label", self.scene.elements[owner].id);
        let index = match self.scene.elements.iter().position(|e| e.id == id) {
            Some(index) => index,
            None => {
                let mut label = self.new_element(ElementKind::Text { x: 0.0, y: 0.0, value: String::new() });
                label.id = id;
                self.scene.elements.push(label);
                self.scene.elements.len() - 1
            }
        };
        recenter_labels(&mut self.scene.elements);
        index
    }

    pub(crate) fn start_text(&mut self, world: Pos2) {
        self.snapshot();
        let size = self.style.font_size;
        self.scene.elements.push(self.new_element(ElementKind::Text { x: world.x, y: world.y - size * LINE_HEIGHT / 2.0, value: String::new() }));
        let index = self.scene.elements.len() - 1;
        self.selected = vec![index];
        self.editing = Some(index);
        self.edit_focused = false;
    }

    pub(crate) fn finish_text(&mut self) {
        let Some(index) = self.editing.take() else {
            return;
        };
        self.dirty = true;
        let empty = matches!(
            self.scene.elements.get(index),
            Some(Element { kind: ElementKind::Text { value, .. }, .. }) if value.trim().is_empty()
        );
        // a note's text belongs to its card, which stays selected (the card sits before its text)
        let card = self.scene.elements.get(index).and_then(|e| note_of_label(&self.scene.elements, e));
        if empty {
            self.scene.elements.remove(index);
        }
        self.selected = match card {
            Some(card) => vec![if empty && card > index { card - 1 } else { card }],
            None if empty => Vec::new(),
            None => vec![index],
        };
        if !self.tool_locked {
            self.tool = Tool::Select;
        }
    }

    /// Alinhar: moves the selected units to a common edge or center (0..=5), or spreads them out
    /// evenly (6 across, 7 down). A unit is a group, or a frame with what is in it, or one card
    /// with its text: it moves as one box. Ropes follow their windows; locked elements stay.
    pub(crate) fn align_selection(&mut self, how: usize) {
        let (units, boxes) = self.selected_units();
        let Some(deltas) = align_deltas(&boxes, how) else { return };
        self.move_units(&units, deltas);
    }

    /// Organizar (⇧Ctrl T): the selected units in an even grid; again on the same selection, in a
    /// row, then a column, then the grid again. Units as in Alinhar.
    pub(crate) fn tidy_selection(&mut self) {
        let (units, boxes) = self.selected_units();
        let ids: Vec<String> = self.selected.iter().filter_map(|&i| self.scene.elements.get(i)).map(|e| e.id.clone()).collect();
        let layout = match &self.tidy_last {
            Some((selection, last)) if *selection == ids => (last + 1) % 3,
            _ => 0,
        };
        let Some(deltas) = tidy_deltas(&boxes, layout) else { return };
        self.move_units(&units, deltas);
        self.tidy_last = Some((ids, layout));
    }

    /// The selection as units that move whole, each with its box.
    fn selected_units(&self) -> (Vec<Vec<usize>>, Vec<Rect>) {
        let mut units: Vec<Vec<usize>> = Vec::new();
        for &i in &self.selected {
            let Some(e) = self.scene.elements.get(i) else { continue };
            if e.rope || self.is_card_text(i) || units.iter().any(|u| u.contains(&i)) {
                continue;
            }
            let unit: Vec<usize> = self.lock_unit(&[i]).into_iter().filter(|j| !self.scene.elements[*j].rope && !self.scene.elements[*j].locked).collect();
            // a unit all locked stays put and doesn't count
            if unit.is_empty() {
                continue;
            }
            // one picked earlier that is inside this one (a shape in a selected frame) goes with it
            units.retain(|u| !u.iter().all(|j| unit.contains(j)));
            units.push(unit);
        }
        let boxes: Vec<Rect> = units
            .iter()
            .map(|u| {
                u.iter()
                    .filter(|j| !self.is_card_text(**j))
                    .map(|j| element_bounds(&self.scene.elements[*j]))
                    .reduce(|a, b| a.union(b))
                    .unwrap_or(Rect::NOTHING)
            })
            .collect();
        (units, boxes)
    }

    /// Each unit moved by its delta (a card's text along), as one undo step.
    fn move_units(&mut self, units: &[Vec<usize>], deltas: Vec<Vec2>) {
        self.snapshot();
        let mut moved = std::collections::HashSet::new();
        for (unit, delta) in units.iter().zip(deltas) {
            for j in self.with_card_text(unit) {
                if moved.insert(j) {
                    self.scene.elements[j].move_by(delta);
                }
            }
        }
    }
}

/// Organizar's move for each box: into cells as big as the biggest box, GRID apart, in reading
/// order (top to bottom, then left to right) from the boxes' top left corner. Layout 0 is a grid
/// of ⌈√n⌉ columns, 1 a row, 2 a column. None under two boxes.
// ponytail: the reference doesn't say which layouts it cycles through; grid, row and column are ours
pub(crate) fn tidy_deltas(boxes: &[Rect], layout: usize) -> Option<Vec<Vec2>> {
    if boxes.len() < 2 {
        return None;
    }
    let union = boxes.iter().copied().reduce(|a, b| a.union(b))?;
    let cell = boxes.iter().fold(Vec2::ZERO, |m, b| m.max(b.size())) + Vec2::splat(GRID);
    let columns = match layout {
        1 => boxes.len(),
        2 => 1,
        _ => (boxes.len() as f32).sqrt().ceil() as usize,
    };
    let mut order: Vec<usize> = (0..boxes.len()).collect();
    order.sort_by(|&a, &b| boxes[a].min.y.total_cmp(&boxes[b].min.y).then(boxes[a].min.x.total_cmp(&boxes[b].min.x)));
    let mut deltas = vec![Vec2::ZERO; boxes.len()];
    for (k, &i) in order.iter().enumerate() {
        let at = union.min + Vec2::new((k % columns) as f32 * cell.x, (k / columns) as f32 * cell.y);
        deltas[i] = at - boxes[i].min;
    }
    Some(deltas)
}

/// Alinhar's move for each box: to the boxes' common edge or center (0 left, 1 center, 2 right,
/// 3 top, 4 middle, 5 bottom) or spread with even gaps (6 across, 7 down). None under two boxes.
pub(crate) fn align_deltas(boxes: &[Rect], how: usize) -> Option<Vec<Vec2>> {
    if boxes.len() < 2 {
        return None;
    }
    let union = boxes.iter().copied().reduce(|a, b| a.union(b))?;
    match how {
        0..=5 => Some(
            boxes
                .iter()
                .map(|b| match how {
                    0 => Vec2::new(union.min.x - b.min.x, 0.0),
                    1 => Vec2::new(union.center().x - b.center().x, 0.0),
                    2 => Vec2::new(union.max.x - b.max.x, 0.0),
                    3 => Vec2::new(0.0, union.min.y - b.min.y),
                    4 => Vec2::new(0.0, union.center().y - b.center().y),
                    _ => Vec2::new(0.0, union.max.y - b.max.y),
                })
                .collect(),
        ),
        6 | 7 => {
            let horizontal = how == 6;
            let start = |b: &Rect| if horizontal { b.min.x } else { b.min.y };
            let span = |b: &Rect| if horizontal { b.width() } else { b.height() };
            let mut order: Vec<usize> = (0..boxes.len()).collect();
            order.sort_by(|a, b| start(&boxes[*a]).total_cmp(&start(&boxes[*b])));
            let total: f32 = boxes.iter().map(span).sum();
            let gap = (span(&union) - total) / (boxes.len() - 1) as f32;
            let mut deltas = vec![Vec2::ZERO; boxes.len()];
            let mut at = start(&union);
            for k in order {
                let d = at - start(&boxes[k]);
                deltas[k] = if horizontal { Vec2::new(d, 0.0) } else { Vec2::new(0.0, d) };
                at += span(&boxes[k]) + gap;
            }
            Some(deltas)
        }
        _ => None,
    }
}
