//! Editing actions: groups, clipboard, layers, zoom and text.

use crate::prelude::*;

impl DrawingApp {
    pub(crate) fn select_tool(&mut self, tool: Tool) {
        self.finish_text();
        self.finish_multi();
        self.line_editor = None;
        self.terminal_focus = None;
        self.ide_focus = None;
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
        self.pan = Vec2::ZERO;
        self.zoom = 1.0;
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
        self.selected
            .iter()
            .any(|i| self.scene.elements.get(*i).is_some_and(|e| e.group_id.is_some()))
    }

    pub(crate) fn select_all(&mut self) {
        self.tool = Tool::Select;
        self.selected = (0..self.scene.elements.len()).filter(|i| !self.scene.elements[*i].locked).collect();
    }

    /// Clones elements with fresh ids and group ids, so pasted groups stay separate groups.
    pub(crate) fn fresh_copies(elements: &[Element], offset: Vec2) -> Vec<Element> {
        let mut groups = std::collections::HashMap::new();
        let ids: std::collections::HashMap<&str, String> =
            elements.iter().map(|e| (e.id.as_str(), new_id())).collect();
        // a binding survives only when its target was copied too
        let remap = |b: &Option<Binding>| {
            b.as_ref().and_then(|b| ids.get(b.element_id.as_str()).map(|id| Binding { element_id: id.clone(), ..b.clone() }))
        };
        elements
            .iter()
            .map(|e| {
                let mut copy = e.clone();
                copy.id = ids[e.id.as_str()].clone();
                copy.ends.start = remap(&e.ends.start);
                copy.ends.end = remap(&e.ends.end);
                copy.group_id = e.group_id.as_ref().map(|g| {
                    groups.entry(g.clone()).or_insert_with(new_id).clone()
                });
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
        self.scene.elements.extend(elements);
        self.selected = (start..self.scene.elements.len()).collect();
        self.tool = Tool::Select;
    }

    pub(crate) fn duplicate(&mut self) {
        let chosen: Vec<Element> =
            self.selected.iter().filter_map(|i| self.scene.elements.get(*i).cloned()).collect();
        self.insert_elements(Self::fresh_copies(&chosen, Vec2::splat(10.0)));
    }

    pub(crate) fn copy(&mut self, ctx: &Context) {
        self.clipboard =
            self.selected.iter().filter_map(|i| self.scene.elements.get(*i).cloned()).collect();
        if !self.clipboard.is_empty() {
            ctx.copy_text(
                json!({ "type": "magoagent/clipboard", "elements": interop_elements(&self.clipboard) })
                    .to_string(),
            );
        }
    }

    pub(crate) fn paste(&mut self, text: &str) {
        let parsed = serde_json::from_str::<Value>(text)
            .ok()
            .filter(|v| v["type"] == "magoagent/clipboard")
            .map(|v| from_interop_elements(&v["elements"]));
        let elements = match parsed {
            Some(elements) => elements,
            None if !text.trim().is_empty() => {
                vec![self.new_element(ElementKind::Text { x: 0.0, y: 0.0, value: text.trim_end().into() })]
            }
            None if !self.clipboard.is_empty() => self.clipboard.clone(),
            None => return,
        };
        // Paste centered under the cursor, like the reference app.
        let bounds = elements.iter().map(element_bounds).reduce(|a, b| a.union(b)).unwrap_or(Rect::ZERO);
        let target = self
            .last_pointer
            .filter(|p| self.canvas_rect.contains(*p))
            .map(|p| self.to_world(p))
            .unwrap_or(bounds.center() + Vec2::splat(10.0));
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
        let mut items: Vec<(bool, Element)> = std::mem::take(&mut self.scene.elements)
            .into_iter()
            .enumerate()
            .map(|(i, e)| (self.selected.contains(&i), e))
            .collect();
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
        self.zoom = zoom.clamp(0.1, 30.0);
        self.pan = anchor - self.canvas_rect.min - world.to_vec2() * self.zoom;
    }

    pub(crate) fn zoom_step(&mut self, delta: f32) {
        let center = self.canvas_rect.center();
        let next = ((self.zoom + delta) * 10.0).round() / 10.0;
        self.zoom_at(next, center);
    }

    /// Grid snap for points (drawing), when the grid is on.
    pub(crate) fn snap_point(&self, p: Pos2) -> Pos2 {
        if self.prefs.grid && !self.mods.command {
            Pos2::new((p.x / GRID).round() * GRID, (p.y / GRID).round() * GRID)
        } else {
            p
        }
    }

    /// Offset that snaps a moved box to other elements' edges/centers (Encaixar em objetos)
    /// or to the grid, plus the red guide lines to show. Cmd disables snapping.
    pub(crate) fn snap_box(&self, moved: Rect) -> (Vec2, Vec<[Pos2; 2]>) {
        if self.mods.command {
            return (Vec2::ZERO, Vec::new());
        }
        if self.prefs.snap_objects {
            let threshold = 8.0 / self.zoom;
            let others: Vec<Rect> = self
                .scene
                .elements
                .iter()
                .enumerate()
                .filter(|(i, _)| !self.selected.contains(i))
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

    /// Centers the canvas on a world point (used by the minimap).
    pub(crate) fn pan_to(&mut self, world: Pos2) {
        self.pan = self.canvas_rect.size() / 2.0 - world.to_vec2() * self.zoom;
    }

    /// World area the minimap covers: every element plus what is on screen, with a margin.
    pub(crate) fn minimap_world(&self) -> Rect {
        let view = Rect::from_two_pos(self.to_world(self.canvas_rect.min), self.to_world(self.canvas_rect.max));
        let all = self.scene.elements.iter().map(element_bounds).fold(view, |a, b| a.union(b));
        all.expand(all.size().max_elem() * 0.05 + 20.0)
    }

    pub(crate) fn zoom_to_fit(&mut self, selection: bool) {
        let bounds = if selection {
            self.selection_bounds()
        } else {
            self.scene.elements.iter().map(element_bounds).reduce(|a, b| a.union(b))
        };
        let (Some(bounds), true) = (bounds, self.canvas_rect.is_positive()) else {
            return;
        };
        let view = self.canvas_rect.size() - Vec2::splat(120.0);
        let zoom = (view.x / bounds.width().max(1.0)).min(view.y / bounds.height().max(1.0));
        self.zoom = if selection { zoom.clamp(0.1, 30.0) } else { zoom.clamp(0.1, 1.0) };
        self.pan = self.canvas_rect.size() / 2.0 - bounds.center().to_vec2() * self.zoom;
    }

    /// Starts typing into a text element; a sticky note's text is its label.
    pub(crate) fn begin_text_edit(&mut self, index: usize) {
        let index = match self.scene.elements.get(index) {
            Some(e) if e.created.is_some() => match note_label(&self.scene.elements, index) {
                Some(label) => label,
                None => return,
            },
            _ => index,
        };
        self.snapshot();
        self.selected = vec![index];
        self.editing = Some(index);
        self.edit_focused = false;
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
        self.scene.elements.push(self.new_element(ElementKind::Text {
            x: world.x,
            y: world.y - size * LINE_HEIGHT / 2.0,
            value: String::new(),
        }));
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
        if empty {
            self.scene.elements.remove(index);
            self.selected.clear();
        } else {
            self.selected = vec![index];
        }
        if !self.tool_locked {
            self.tool = Tool::Select;
        }
    }
}
