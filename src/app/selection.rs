//! Selection, handles, fill/lasso/sticky tools and locking.

use crate::prelude::*;

impl DrawingApp {
    // ---------- canvas pointer ----------

    pub(crate) fn selection_bounds(&self) -> Option<Rect> {
        self.selected
            .iter()
            .filter_map(|i| self.scene.elements.get(*i))
            .map(element_bounds)
            .reduce(|a, b| a.union(b))
    }

    /// Selection box as (center, size, angle) in world units: rotated for one rotated element.
    pub(crate) fn selection_frame(&self) -> Option<(Pos2, Vec2, f32)> {
        if let [i] = self.selected[..] {
            if let Some(e) = self.scene.elements.get(i).filter(|e| e.angle != 0.0) {
                let b = local_bounds(e);
                return Some((b.center(), b.size(), e.angle));
            }
        }
        let b = self.selection_bounds()?;
        Some((b.center(), b.size(), 0.0))
    }

    pub(crate) fn shows_transform_handles(&self) -> bool {
        // The reference app shows no bounding box for a lone 2-point line/arrow, just its points
        let two_point_line = self.single_line().is_some_and(|i| self.line_points(i).len() == 2);
        self.tool == Tool::Select
            && self.editing.is_none()
            && self.line_editor.is_none()
            && self.multi.is_none()
            && !two_point_line
    }

    pub(crate) fn handle_corners(&self) -> Option<[Pos2; 4]> {
        let (center, size, angle) = self.selection_frame()?;
        let c = self.to_screen(center);
        let h = size * self.zoom / 2.0 + Vec2::splat(5.0);
        Some([(-1.0, -1.0), (1.0, -1.0), (1.0, 1.0), (-1.0, 1.0)].map(|(sx, sy)| c + rot(Vec2::new(sx * h.x, sy * h.y), angle)))
    }

    /// Circle above the box, 16px gap like the reference app's ROTATION_RESIZE_HANDLE_GAP.
    pub(crate) fn rotation_handle(&self) -> Option<Pos2> {
        let (center, size, angle) = self.selection_frame()?;
        Some(self.to_screen(center) + rot(Vec2::new(0.0, -(size.y * self.zoom / 2.0 + 5.0 + 16.0)), angle))
    }

    pub(crate) fn hovered_handle(&self, pos: Pos2) -> Option<usize> {
        if !self.shows_transform_handles() {
            return None;
        }
        self.handle_corners()?.iter().position(|c| c.distance(pos) <= 7.0)
    }

    pub(crate) fn hovered_rotation(&self, pos: Pos2) -> bool {
        self.shows_transform_handles() && self.rotation_handle().is_some_and(|h| h.distance(pos) <= 8.0)
    }

    pub(crate) fn single_line(&self) -> Option<usize> {
        match self.selected[..] {
            [i] if matches!(self.scene.elements.get(i), Some(Element { kind: ElementKind::Line { .. }, .. })) => Some(i),
            _ => None,
        }
    }

    pub(crate) fn line_points(&self, index: usize) -> &[[f32; 2]] {
        match self.scene.elements.get(index) {
            Some(Element { kind: ElementKind::Line { points, .. }, .. }) => points,
            _ => &[],
        }
    }

    pub(crate) fn line_points_mut(&mut self, index: usize) -> Option<&mut Vec<[f32; 2]>> {
        match self.scene.elements.get_mut(index) {
            Some(Element { kind: ElementKind::Line { points, .. }, .. }) => Some(points),
            _ => None,
        }
    }

    pub(crate) fn editable_line(&self) -> Option<usize> {
        self.single_line().filter(|_| self.tool == Tool::Select && self.multi.is_none() && self.editing.is_none())
    }

    /// Every point of the selected line gets a handle (renderLinearPointHandles).
    pub(crate) fn point_handles(&self) -> Vec<(usize, Pos2)> {
        let Some(i) = self.editable_line() else {
            return Vec::new();
        };
        self.line_points(i).iter().enumerate().map(|(k, p)| (k, self.to_screen(Pos2::new(p[0], p[1])))).collect()
    }

    /// Segment midpoints (on the curve for curved lines), shown in the editor or on 2-point lines.
    pub(crate) fn midpoint_handles(&self) -> Vec<(usize, Pos2)> {
        let Some(i) = self.editable_line() else {
            return Vec::new();
        };
        let points = self.line_points(i);
        if self.line_editor != Some(i) && points.len() != 2 {
            return Vec::new();
        }
        let path = line_path(&self.scene.elements[i]);
        let curved = path.len() != points.len();
        (0..points.len() - 1)
            .map(|k| {
                let m = if curved {
                    path[k * LINE_CURVE_STEPS + LINE_CURVE_STEPS / 2]
                } else {
                    [(points[k][0] + points[k + 1][0]) / 2.0, (points[k][1] + points[k + 1][1]) / 2.0]
                };
                (k, self.to_screen(Pos2::new(m[0], m[1])))
            })
            .collect()
    }

    pub(crate) fn delete_selected_point(&mut self) -> bool {
        let (Some(i), Some(k)) = (self.line_editor, self.selected_point) else {
            return false;
        };
        if self.line_points(i).len() <= 2 {
            return true;
        }
        self.snapshot();
        if let Some(points) = self.line_points_mut(i) {
            points.remove(k.min(points.len() - 1));
        }
        self.selected_point = None;
        true
    }

    pub(crate) fn finish_multi(&mut self) {
        let Some(i) = self.multi.take() else {
            return;
        };
        let remaining = self.line_points_mut(i).map(|points| {
            points.pop(); // the point following the cursor
            points.len()
        });
        if let Some(&last) = self.line_points(i).last().filter(|_| remaining.unwrap_or(0) >= 2) {
            let binding = self.bind_at(Pos2::new(last[0], last[1]), Some(i));
            self.scene.elements[i].ends.end = binding;
        }
        if remaining.unwrap_or(0) < 2 {
            self.scene.elements.remove(i);
            self.selected.clear();
        } else {
            self.selected = vec![i];
        }
        self.dirty = true;
        if !self.tool_locked {
            self.tool = Tool::Select;
        }
    }

    /// Shift snaps a segment to 15° steps around `from`.
    pub(crate) fn snap_angle(&self, from: [f32; 2], to: Pos2) -> Pos2 {
        if !self.mods.shift {
            return to;
        }
        let d = to - Pos2::new(from[0], from[1]);
        let step = std::f32::consts::PI / 12.0;
        Pos2::new(from[0], from[1]) + Vec2::angled((d.y.atan2(d.x) / step).round() * step) * d.length()
    }

    pub(crate) fn hit(&self, world: Pos2) -> Option<usize> {
        let tolerance = 10.0 / self.zoom;
        self.scene.elements.iter().rposition(|e| !e.locked && e.contains(world, tolerance))
    }

    /// 0..1 through drawing `e`, the front of the reveal queue; a long queue plays faster.
    pub(crate) fn reveal_progress(&self, e: &Element) -> f32 {
        let speed = 1.0 + self.reveal.len() as f64 / 8.0;
        ((self.now - self.reveal_since) * speed / reveal_duration(e)).clamp(0.0, 1.0) as f32
    }

    /// Preenchimento: paints the background of the shape under the pointer.
    pub(crate) fn fill_at(&mut self, world: Pos2) {
        let target = self.scene.elements.iter().rposition(|e| {
            !e.locked && e.fillable() && e.image.is_none() && shape_polygon(e).is_some_and(|poly| {
                let p = e.to_local(world);
                polygon_contains(&poly, [p.x, p.y])
            })
        });
        if let Some(i) = target {
            self.snapshot();
            let element = &mut self.scene.elements[i];
            // a transparent pick would be invisible: the web falls back to green
            element.style.fill = Some(self.style.fill.filter(|c| c[3] > 0).unwrap_or(BACKGROUND_PICKS[2]));
            element.style.fill_style = self.style.fill_style;
            element.style.opacity = self.style.opacity;
        }
    }

    /// Selects what the freehand lasso surrounds (or touches, with "Sobreposto" box
    /// selection), whole groups at a time, live while it's drawn.
    // ponytail: tests element bounding boxes, not their exact outlines.
    pub(crate) fn select_in_lasso(&mut self, path: &[Pos2]) {
        if path.len() < 3 {
            self.selected.clear();
            return;
        }
        let poly: Vec<[f32; 2]> = path.iter().map(|p| [p.x, p.y]).collect();
        let inside = |p: Pos2| polygon_contains(&poly, [p.x, p.y]);
        let mut selected: Vec<usize> = self
            .scene
            .elements
            .iter()
            .enumerate()
            .filter(|(_, e)| !e.locked && !is_label(e))
            .filter(|(_, e)| {
                let b = element_bounds(e);
                let mut corners = [b.left_top(), b.right_top(), b.right_bottom(), b.left_bottom()].into_iter();
                if self.prefs.box_overlap {
                    corners.any(inside) || path.iter().any(|p| b.contains(*p))
                } else {
                    corners.all(inside)
                }
            })
            .map(|(i, _)| i)
            .collect();
        for i in selected.clone() {
            for j in self.group_members(i) {
                if !selected.contains(&j) {
                    selected.push(j);
                }
            }
        }
        self.selected = selected;
    }

    /// Nota adesiva: a 250px square centered on a click, or the dragged size (square unless
    /// Shift, at least 75px), with its text being edited right away.
    pub(crate) fn place_sticky(&mut self, start: Pos2, end: Pos2) {
        let d = end - start;
        let rect = if d.x.abs().max(d.y.abs()) * self.zoom < 10.0 {
            Rect::from_center_size(start, Vec2::splat(250.0))
        } else {
            let (mut w, mut h) = (d.x.abs().max(75.0), d.y.abs().max(75.0));
            if !self.mods.shift {
                w = w.max(h);
                h = w;
            }
            let x = if d.x < 0.0 { start.x - w } else { start.x };
            let y = if d.y < 0.0 { start.y - h } else { start.y };
            Rect::from_min_size(Pos2::new(x, y), Vec2::new(w, h))
        };
        let mut note = self.new_element(ElementKind::Rectangle { x: rect.min.x, y: rect.min.y, width: rect.width(), height: rect.height() });
        note.style = sticky_style(note.style);
        note.created = Some(now_ms());
        let group = uuid::Uuid::new_v4().to_string();
        note.group_id = Some(group.clone());
        let mut text = self.new_element(ElementKind::Text { x: rect.center().x, y: rect.center().y, value: String::new() });
        text.id = format!("{}-label", note.id);
        text.group_id = Some(group);
        text.style.stroke = note.style.stroke;
        text.style.font_size = 20.0;
        self.snapshot();
        self.scene.elements.push(note);
        self.scene.elements.push(text);
        let index = self.scene.elements.len() - 1;
        self.selected = vec![index];
        self.editing = Some(index);
        self.edit_focused = false;
    }

    /// The given elements plus everything lying inside any frame among them.
    pub(crate) fn with_frame_contents(&self, indices: &[usize]) -> Vec<usize> {
        let frames: Vec<Rect> = indices
            .iter()
            .filter_map(|i| self.scene.elements.get(*i))
            .filter(|e| e.frame.is_some())
            .map(|e| element_bounds(e).expand(1.0))
            .collect();
        let mut all = indices.to_vec();
        for (i, e) in self.scene.elements.iter().enumerate() {
            if !all.contains(&i) && !e.locked && frames.iter().any(|f| f.contains_rect(element_bounds(e))) {
                all.push(i);
            }
        }
        all
    }

    /// Locks the selection (Cmd+Shift+L); if all of it is locked already, unlocks it.
    /// The web's UnlockPopup: a lock button 12px above the clicked locked element's top-left
    /// corner; clicking it unlocks the whole unit and selects it.
    pub(crate) fn unlock_popup(&mut self, ctx: &Context) {
        let Some(ids) = self.active_locked.clone() else {
            return;
        };
        let unit: Vec<usize> = self.scene.elements.iter().enumerate().filter(|(_, e)| ids.contains(&e.id) && e.locked).map(|(i, _)| i).collect();
        let Some(bounds) = unit.iter().map(|i| element_bounds(&self.scene.elements[*i])).reduce(|a, b| a.union(b)) else {
            self.active_locked = None;
            return;
        };
        let c = self.colors();
        // above a card's name, not on top of it
        let name = if unit.iter().any(|i| self.scene.elements[*i].frame.is_some()) { 22.0 * self.zoom } else { 0.0 };
        let corner = self.to_screen(bounds.min) - Vec2::new(0.0, name);
        let size = 20.0 + 2.0 * 12.8;
        let response = egui::Area::new(egui::Id::unique("unlock_popup"))
            .fixed_pos(corner - Vec2::new(0.0, 12.0 + size))
            .order(egui::Order::Foreground)
            .show(ctx, |ui| {
                self.island()
                    .inner_margin(0)
                    .show(ui, |ui| {
                        let (rect, response) = ui.allocate_exact_size(Vec2::splat(size), Sense::click());
                        let color = if response.hovered() { PRIMARY } else { c.muted };
                        paint_icon(ui.painter(), Icon::Lock(true), rect.center(), color);
                        response.on_hover_cursor(CursorIcon::PointingHand).on_hover_text("Desbloquear")
                    })
                    .inner
            })
            .inner;
        if response.clicked() {
            self.selected = unit;
            self.toggle_lock_selected();
            self.active_locked = None;
        }
    }

    /// What locks and unlocks as one: the element's outermost group and, for a frame (card),
    /// everything inside it.
    pub(crate) fn lock_unit(&self, indices: &[usize]) -> Vec<usize> {
        let mut unit = indices.to_vec();
        let mut k = 0;
        while k < unit.len() {
            let e = &self.scene.elements[unit[k]];
            let frame = e.frame.is_some().then(|| element_bounds(e).expand(1.0));
            for (j, other) in self.scene.elements.iter().enumerate() {
                let grouped = e.group_id.is_some() && other.group_id == e.group_id;
                if !unit.contains(&j) && (grouped || frame.is_some_and(|f| f.contains_rect(element_bounds(other)))) {
                    unit.push(j);
                }
            }
            k += 1;
        }
        unit
    }

    pub(crate) fn toggle_lock_selected(&mut self) {
        if self.selected.is_empty() {
            return;
        }
        self.snapshot();
        let unit = self.lock_unit(&self.selected);
        let lock = unit.iter().any(|i| !self.scene.elements[*i].locked);
        for i in unit {
            self.scene.elements[i].locked = lock;
        }
        if lock {
            self.selected.clear();
        }
    }

    pub(crate) fn lock_all(&mut self) {
        if self.scene.elements.iter().all(|e| e.locked) {
            return;
        }
        self.snapshot();
        for e in &mut self.scene.elements {
            e.locked = true;
        }
        self.selected.clear();
    }

    pub(crate) fn unlock(&mut self, id: Option<&str>) {
        self.snapshot();
        for e in self.scene.elements.iter_mut().filter(|e| id.is_none_or(|id| e.id == id)) {
            e.locked = false;
        }
    }

    /// Short name for lists: the text itself, the shape's label, or the kind of element.
    pub(crate) fn describe(&self, e: &Element) -> String {
        let text_of = |e: &Element| match &e.kind {
            ElementKind::Text { value, .. } => Some(value.lines().next().unwrap_or("").to_string()),
            _ => None,
        };
        let label = self.scene.elements.iter().find(|l| l.id == format!("{}-label", e.id)).and_then(text_of);
        let kind = match &e.kind {
            ElementKind::Rectangle { .. } => "Retângulo",
            ElementKind::Ellipse { .. } => "Elipse",
            ElementKind::Diamond { .. } => "Losango",
            ElementKind::Line { arrow: true, .. } => "Flecha",
            ElementKind::Line { .. } => "Linha",
            ElementKind::FreeDraw { .. } => "Desenho",
            ElementKind::Text { .. } => "Texto",
        };
        let detail = text_of(e).or(label).filter(|t| !t.trim().is_empty());
        let mut out = match detail {
            Some(t) if matches!(e.kind, ElementKind::Text { .. }) => format!("“{t}”"),
            Some(t) => format!("{kind} · {t}"),
            None => kind.to_string(),
        };
        if out.chars().count() > 34 {
            out = out.chars().take(33).collect::<String>() + "…";
        }
        out
    }

    pub(crate) fn group_members(&self, index: usize) -> Vec<usize> {
        let group = self.scene.elements[index].group_id.clone();
        self.scene
            .elements
            .iter()
            .enumerate()
            .filter_map(|(j, e)| {
                (j == index || (!e.locked && group.as_ref().is_some_and(|g| e.group_id.as_ref() == Some(g)))).then_some(j)
            })
            .collect()
    }
}
