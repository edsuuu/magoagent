//! Editing actions: groups, clipboard, layers, zoom and text.

use crate::prelude::*;
mod align;
mod viewport;
pub(crate) use align::*;

/// The zoom steps of − / +: the reference's 16 up to 300%, then ours up to the 3000% limit.
const ZOOM_LEVELS: [f32; 26] =
    [0.1, 0.15, 0.25, 0.33, 0.5, 0.67, 0.75, 0.9, 1.0, 1.1, 1.25, 1.5, 1.75, 2.0, 2.5, 3.0, 4.0, 5.0, 6.0, 8.0, 10.0, 12.0, 15.0, 20.0, 25.0, 30.0];

/// How long after its last change the zoom counts as at rest, its text laid out again at its own size (`text_zoom`).
pub(crate) const ZOOM_SETTLES: f64 = 0.25;

impl DrawingApp {
    pub(crate) fn select_tool(&mut self, tool: Tool) {
        self.finish_text();
        self.finish_multi();
        self.line_editor = None;
        self.terminal_focus = None;
        self.card_focus = None;
        self.connecting = None;
        self.tool = tool;
        if !matches!(tool, Tool::Select | Tool::Hand) {
            self.selected.clear();
        }
        // a window to make (terminal, chat, note…) puts the drawing row away
        if Tool::TOOLBAR.contains(&tool) && !matches!(tool, Tool::Select | Tool::Hand) {
            self.draw_open = false;
            self.extra_open = false;
        }
    }

    /// The drawing row is open: by its button, or with a drawing tool in hand.
    pub(crate) fn drawing(&self) -> bool {
        self.draw_open || Tool::DRAWING.contains(&self.tool) || Tool::EXTRA.contains(&self.tool)
    }

    /// Whether `tool`'s keys answer now. Mão and Seleção make nothing, so theirs always do; the
    /// rest only while drawing: on the bare canvas a stray letter (the keys of a window that lost
    /// them) would arm a tool, and the next click would draw.
    pub(crate) fn tool_keys_live(&self, tool: Tool) -> bool {
        Tool::TOOLBAR.contains(&tool) || self.drawing()
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
        self.scene.groups.clear();
        self.selected.clear();
        (self.pan, self.zoom, self.glide) = (Vec2::ZERO, 1.0, None);
    }

    pub(crate) fn select_all(&mut self) {
        self.tool = Tool::Select;
        self.selected = (0..self.scene.elements.len()).filter(|index| !self.scene.elements[*index].locked && !self.is_card_text(*index)).collect();
    }

    /// Clones elements with fresh ids and group ids, so pasted groups stay separate groups. A chat's
    /// card is left out: its id is the chat's, and a copy would be a second card of one conversation.
    /// It carries the canvas groups too: the copies of the members of one of `known`
    /// (two or more, a note's text not counting) form a new group of their own with its name and color.
    pub(crate) fn fresh_copies_in(elements: &[Element], offset: Vec2, known: &[FrameGroup]) -> (Vec<Element>, Vec<FrameGroup>) {
        let mut groups = std::collections::HashMap::new();
        let elements: Vec<&Element> = elements.iter().filter(|element| element.chat.is_none()).collect();
        let ids: std::collections::HashMap<&str, String> = elements.iter().map(|element| (element.id.as_str(), new_id())).collect();
        // a note's text keeps the "-label" id of its card's copy, so it stays that card's text
        let text_id = |element: &Element| match element.id.strip_suffix("-label").and_then(|owner| ids.get(owner)) {
            Some(owner) if matches!(element.kind, ElementKind::Text { .. }) => format!("{owner}-label"),
            _ => ids[element.id.as_str()].clone(),
        };
        // a binding survives only when its target was copied too
        let remap = |binding: &Option<Binding>| {
            binding.as_ref().and_then(|binding| ids.get(binding.element_id.as_str()).map(|id| Binding { element_id: id.clone(), ..binding.clone() }))
        };
        let is_text = |element: &Element| {
            matches!(element.kind, ElementKind::Text { .. }) && element.id.strip_suffix("-label").is_some_and(|owner| ids.contains_key(owner))
        };
        let mut new_groups: Vec<FrameGroup> = Vec::new();
        let mut frame_ids = std::collections::HashMap::new();
        for group in known {
            if elements.iter().filter(|element| element.frame_group.as_deref() == Some(group.id.as_str()) && !is_text(element)).count() >= 2 {
                let id = new_id();
                frame_ids.insert(group.id.clone(), id.clone());
                new_groups.push(FrameGroup { id, ..group.clone() });
            }
        }
        let copies = elements
            .iter()
            .map(|&element| {
                let mut copy = element.clone();
                copy.id = text_id(element);
                copy.ends.start = remap(&element.ends.start);
                copy.ends.end = remap(&element.ends.end);
                copy.group_id = element.group_id.as_ref().map(|group_id| groups.entry(group_id.clone()).or_insert_with(new_id).clone());
                copy.frame_group = element.frame_group.as_ref().and_then(|frame_group| frame_ids.get(frame_group).cloned());
                copy.note.file = None;
                copy.move_by(offset);
                // a copied terminal starts its own conversation, not a second one of the original's
                if let Some(terminal) = copy.terminal.as_mut() {
                    (terminal.claude_session, terminal.claude_folder) = Default::default();
                }
                copy
            })
            .collect();
        (copies, new_groups)
    }

    /// Pastes `fresh_copies_in` of `elements`: the new groups join the scene.
    fn insert_copies(&mut self, elements: &[Element], offset: Vec2, known: &[FrameGroup]) {
        let (copies, groups) = Self::fresh_copies_in(elements, offset, known);
        if copies.is_empty() {
            return;
        }
        self.insert_elements(copies);
        self.scene.groups.extend(groups);
    }

    pub(crate) fn insert_elements(&mut self, elements: Vec<Element>) {
        if elements.is_empty() {
            return;
        }
        self.snapshot();
        let start = self.scene.elements.len();
        for mut element in elements {
            // a copied terminal gets a name of its own, numbered ("Shell 2"): the bridge calls terminals by name
            if let Some(terminal) = element.terminal.as_mut().filter(|terminal| {
                self.scene.elements.iter().any(|other| other.terminal.as_ref().is_some_and(|other_terminal| other_terminal.name == terminal.name))
            }) {
                let base = terminal.name.trim_end_matches(|character: char| character.is_ascii_digit()).trim_end();
                terminal.name = self.unique_terminal_name(if base.is_empty() { &terminal.name } else { base }, None);
            }
            self.scene.elements.push(element);
        }
        self.selected = (start..self.scene.elements.len()).filter(|index| !self.is_card_text(*index)).collect();
        self.tool = Tool::Select;
    }

    pub(crate) fn duplicate(&mut self) {
        let chosen: Vec<Element> = self.with_card_text(&self.selected).iter().filter_map(|index| self.scene.elements.get(*index).cloned()).collect();
        let known = self.scene.groups.clone();
        self.insert_copies(&chosen, Vec2::splat(10.0), &known);
    }

    pub(crate) fn copy(&mut self, ctx: &Context) {
        self.clipboard = self.with_card_text(&self.selected).iter().filter_map(|index| self.scene.elements.get(*index).cloned()).collect();
        if !self.clipboard.is_empty() {
            // the groups ride along: each with the ids of its members that were copied
            let groups: Vec<Value> = self
                .scene
                .groups
                .iter()
                .map(|group| {
                    let members: Vec<&str> = self
                        .clipboard
                        .iter()
                        .filter(|element| element.frame_group.as_deref() == Some(group.id.as_str()))
                        .map(|element| element.id.as_str())
                        .collect();
                    json!({ "id": group.id, "name": group.name, "color": group.color, "members": members })
                })
                .filter(|group| group["members"].as_array().is_some_and(|members| !members.is_empty()))
                .collect();
            ctx.copy_text(json!({ "type": "magoagent/clipboard", "elements": interop_elements(&self.clipboard), "frameGroups": groups }).to_string());
        }
    }

    pub(crate) fn paste(&mut self, text: &str) {
        let json = serde_json::from_str::<Value>(text).ok().filter(|value| value["type"] == "magoagent/clipboard");
        let mut known = self.scene.groups.clone();
        let parsed = json.as_ref().map(|value| {
            let mut elements = from_interop_elements(&value["elements"]);
            known.clear();
            for group in value["frameGroups"].as_array().into_iter().flatten() {
                let id = group["id"].as_str().unwrap_or_default().to_string();
                known.push(FrameGroup {
                    id: id.clone(),
                    name: group["name"].as_str().unwrap_or(groups::DEFAULT_NAME).into(),
                    color: serde_json::from_value(group["color"].clone()).ok().flatten(),
                });
                for member in group["members"].as_array().into_iter().flatten().filter_map(|member| member.as_str()) {
                    if let Some(element) = elements.iter_mut().find(|element| element.id == member) {
                        element.frame_group = Some(id.clone());
                    }
                }
            }
            elements
        });
        let elements = match parsed {
            Some(elements) => elements,
            None if !text.trim().is_empty() => {
                vec![self.new_element(ElementKind::Text { x: 0.0, y: 0.0, value: text.trim_end().into() })]
            }
            None if !self.clipboard.is_empty() => self.clipboard.clone(),
            None => return,
        };
        // Paste centered under the cursor, like the reference app.
        let bounds = elements
            .iter()
            .filter(|element| note_of_label(&elements, element).is_none())
            .map(element_bounds)
            .reduce(|first, second| first.union(second))
            .unwrap_or(Rect::ZERO);
        let target = self
            .last_pointer
            .filter(|pointer| self.canvas_rect.contains(*pointer))
            .map(|pointer| self.to_world(pointer))
            .unwrap_or(bounds.center() + Vec2::splat(10.0));
        self.insert_copies(&elements, target - bounds.center(), &known);
    }

    pub(crate) fn nudge(&mut self, delta: Vec2) {
        if self.selected.is_empty() {
            return;
        }
        self.snapshot();
        self.detach_selected_arrows();
        for index in &self.selected {
            if let Some(element) = self.scene.elements.get_mut(*index) {
                element.move_by(delta);
            }
        }
    }

    pub(crate) fn reorder(&mut self, layer: Layer) {
        if self.selected.is_empty() {
            return;
        }
        self.snapshot();
        let mut items: Vec<(bool, Element)> =
            std::mem::take(&mut self.scene.elements).into_iter().enumerate().map(|(index, element)| (self.selected.contains(&index), element)).collect();
        let count = items.len();
        match layer {
            Layer::Forward => {
                for index in (0..count.saturating_sub(1)).rev() {
                    if items[index].0 && !items[index + 1].0 {
                        items.swap(index, index + 1);
                    }
                }
            }
            Layer::Backward => {
                for index in 1..count {
                    if items[index].0 && !items[index - 1].0 {
                        items.swap(index, index - 1);
                    }
                }
            }
            Layer::Front => items.sort_by_key(|(selected, _)| *selected),
            Layer::Back => items.sort_by_key(|(selected, _)| !*selected),
        }
        self.selected = items.iter().enumerate().filter_map(|(index, (selected, _))| selected.then_some(index)).collect();
        self.scene.elements = items.into_iter().map(|(_, element)| element).collect();
    }

    /// Starts typing into a text element; a sticky note's text is its label.
    pub(crate) fn begin_text_edit(&mut self, index: usize) {
        // a note's text is typed in place: the card stays the selection
        let (card, text) = match self.scene.elements.get(index) {
            Some(element) if element.created.is_some() && element.note.content_locked => return,
            Some(element) if element.created.is_some() => match note_label(&self.scene.elements, index) {
                Some(label) => (index, label),
                // leaving an empty note drops its text (finish_text): give it a new one to type into
                None => {
                    let note = &self.scene.elements[index];
                    let [_, mut label] = self.new_note(element_bounds(note), String::new());
                    label.id = format!("{}-label", note.id);
                    label.group_id = note.group_id.clone();
                    label.style.font_size = 20.0;
                    self.scene.elements.push(label);
                    (index, self.scene.elements.len() - 1)
                }
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
        self.scene.elements.iter().rposition(|element| {
            takes_label(element) && !matches!(element.kind, ElementKind::Line { .. }) && local_bounds(element).contains(element.to_local(world))
        })
    }

    /// A terminal, portal, IDE or sticky note under `world`: the text tool and double-clicks leave
    /// those to the window instead of writing over them.
    pub(crate) fn window_at(&self, world: Pos2) -> bool {
        self.scene.elements.iter().any(|element| (element.is_node() || element.created.is_some()) && element_bounds(element).contains(world))
    }

    /// The label of a shape or arrow (`<id>-label`, the MCP `label` makes the same), created empty
    /// on its first double-click and kept centered on its owner.
    pub(crate) fn owner_label(&mut self, owner: usize) -> usize {
        let id = format!("{}-label", self.scene.elements[owner].id);
        let index = match self.scene.elements.iter().position(|element| element.id == id) {
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
        let card = self.scene.elements.get(index).and_then(|element| note_of_label(&self.scene.elements, element));
        if empty {
            self.scene.elements.remove(index);
        }
        self.selected = match card {
            Some(card) => vec![if empty && card > index { card - 1 } else { card }],
            None if empty => Vec::new(),
            None => vec![index],
        };
        self.tool = Tool::Select;
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
        let ids: Vec<String> = self.selected.iter().filter_map(|&index| self.scene.elements.get(index)).map(|element| element.id.clone()).collect();
        let layout = match &self.tidy_last {
            Some((selection, last)) if *selection == ids => (last + 1) % 3,
            _ => 0,
        };
        let Some(deltas) = tidy_deltas(&boxes, layout) else { return };
        self.move_units(&units, deltas);
        self.tidy_last = Some((ids, layout));
    }

    /// Organizar o canvas: every window, note, group and drawing of the canvas (or of `only`) in rows WINDOW_GAP apart, those
    /// tied by a rope or an arrow next to each other, as one undo step, then the whole in view. Strokes and shapes near each
    /// other or tied by an arrow are one drawing and keep their shape; a canvas group moves whole. Ropes and bound arrows
    /// follow their ends. How many it moved.
    // ponytail: locked elements stay where they are and the rows may cover them; leaving room around them is the next step
    pub(crate) fn arrange_canvas(&mut self, only: Option<&[usize]>) -> usize {
        let elements = &self.scene.elements;
        let tied = |element: &Element| element.rope || element.ends.start.is_some() || element.ends.end.is_some();
        let candidates: Vec<usize> = only
            .map_or_else(|| (0..elements.len()).collect(), <[usize]>::to_vec)
            .into_iter()
            .filter(|&index| elements.get(index).is_some_and(|element| !tied(element) && !element.locked) && !self.is_card_text(index))
            .collect();
        let (units, boxes) = self.units_of(&candidates);
        let unit_of = |index: usize| units.iter().position(|unit| unit.contains(&index));
        let ties: Vec<(usize, usize)> = elements
            .iter()
            .filter(|element| tied(element))
            .filter_map(|element| {
                let end = |binding: &Option<Binding>| binding.as_ref().and_then(|binding| elements.iter().position(|other| other.id == binding.element_id));
                Some((unit_of(end(&element.ends.start)?)?, unit_of(end(&element.ends.end)?)?))
            })
            .collect();
        // a drawing: no window, note, frame or canvas group in it (a shape and its label share a `group_id`: still a drawing)
        let drawing: Vec<bool> = units
            .iter()
            .map(|unit| {
                unit.iter().all(|&member| {
                    let element = &elements[member];
                    !element.is_node() && element.created.is_none() && element.frame.is_none() && element.frame_group.is_none()
                })
            })
            .collect();
        let canvas_group: Vec<Option<&str>> = units.iter().map(|unit| unit.iter().find_map(|&member| elements[member].frame_group.as_deref())).collect();
        let mut parent: Vec<usize> = (0..units.len()).collect();
        fn root(parent: &mut [usize], mut index: usize) -> usize {
            while parent[index] != index {
                parent[index] = parent[parent[index]];
                index = parent[index];
            }
            index
        }
        for first in 0..units.len() {
            for second in first + 1..units.len() {
                let near = drawing[first] && drawing[second] && boxes[first].expand(20.0).intersects(boxes[second]);
                let arrow = drawing[first] && drawing[second] && ties.iter().any(|&tie| tie == (first, second) || tie == (second, first));
                // the members of a canvas group (Grupo) move as one, its frame around them
                let grouped = canvas_group[first].is_some() && canvas_group[first] == canvas_group[second];
                if near || arrow || grouped {
                    let (first_root, second_root) = (root(&mut parent, first), root(&mut parent, second));
                    parent[second_root] = first_root;
                }
            }
        }
        // the drawings joined: one unit each
        let (mut merged, mut merged_boxes): (Vec<Vec<usize>>, Vec<Rect>) = (Vec::new(), Vec::new());
        let mut slot_of = vec![usize::MAX; units.len()];
        for index in 0..units.len() {
            let top = root(&mut parent, index);
            if slot_of[top] == usize::MAX {
                slot_of[top] = merged.len();
                merged.push(Vec::new());
                merged_boxes.push(Rect::NOTHING);
            }
            let slot = slot_of[top];
            merged[slot].extend(&units[index]);
            merged_boxes[slot] = merged_boxes[slot].union(boxes[index]);
        }
        if merged.len() < 2 {
            return 0;
        }
        // what a rope or an arrow ties goes side by side: each tied set where its first one is, in reading order
        let mut cluster: Vec<usize> = (0..merged.len()).collect();
        for &(first, second) in &ties {
            let (first, second) = (root(&mut cluster, slot_of[root(&mut parent, first)]), root(&mut cluster, slot_of[root(&mut parent, second)]));
            cluster[second] = first;
        }
        let reading = |first: &usize, second: &usize| {
            let (first, second) = (merged_boxes[*first], merged_boxes[*second]);
            first.min.y.total_cmp(&second.min.y).then(first.min.x.total_cmp(&second.min.x))
        };
        let mut order: Vec<usize> = (0..merged.len()).collect();
        order.sort_by(reading);
        let tops: Vec<usize> = (0..merged.len()).map(|index| root(&mut cluster, index)).collect();
        let (mut lead, mut read_at) = (vec![usize::MAX; merged.len()], vec![0; merged.len()]);
        for (position, &index) in order.iter().enumerate() {
            lead[tops[index]] = lead[tops[index]].min(position);
            read_at[index] = position;
        }
        order.sort_by_key(|&index| (lead[tops[index]], read_at[index]));
        let deltas = arrange_deltas(&merged_boxes, &order, WINDOW_GAP);
        self.move_units(&merged, deltas);
        self.update_bindings();
        self.zoom_to_fit(false);
        merged.len()
    }

    /// The selection as units that move whole, each with its box.
    fn selected_units(&self) -> (Vec<Vec<usize>>, Vec<Rect>) {
        self.units_of(&self.selected.clone())
    }

    /// `indices` as units that move whole, each with its box.
    fn units_of(&self, indices: &[usize]) -> (Vec<Vec<usize>>, Vec<Rect>) {
        let mut units: Vec<Vec<usize>> = Vec::new();
        for &index in indices {
            let Some(element) = self.scene.elements.get(index) else { continue };
            if element.rope || self.is_card_text(index) || units.iter().any(|unit| unit.contains(&index)) {
                continue;
            }
            let unit: Vec<usize> =
                self.lock_unit(&[index]).into_iter().filter(|member| !self.scene.elements[*member].rope && !self.scene.elements[*member].locked).collect();
            // a unit all locked stays put and doesn't count
            if unit.is_empty() {
                continue;
            }
            // one picked earlier that is inside this one (a shape in a selected frame) goes with it
            units.retain(|other| !other.iter().all(|member| unit.contains(member)));
            units.push(unit);
        }
        let boxes: Vec<Rect> = units
            .iter()
            .map(|unit| {
                unit.iter()
                    .filter(|member| !self.is_card_text(**member))
                    .map(|member| element_bounds(&self.scene.elements[*member]))
                    .reduce(|first, second| first.union(second))
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
            for member in self.with_card_text(unit) {
                if moved.insert(member) {
                    self.scene.elements[member].move_by(delta);
                }
            }
        }
    }
}
