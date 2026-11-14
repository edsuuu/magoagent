//! Selection, handles, fill/lasso/sticky tools and locking.

use crate::prelude::*;

impl DrawingApp {
    pub(crate) fn selection_bounds(&self) -> Option<Rect> {
        self.selected
            .iter()
            .filter_map(|index| self.scene.elements.get(*index))
            .filter(|element| note_of_label(&self.scene.elements, element).is_none())
            .map(element_bounds)
            .reduce(|first, second| first.union(second))
    }

    /// Selection box as (center, size, angle) in world units: rotated for one rotated element.
    pub(crate) fn selection_frame(&self) -> Option<(Pos2, Vec2, f32)> {
        if let [index] = self.selected[..]
            && let Some(element) = self.scene.elements.get(index).filter(|element| element.angle != 0.0)
        {
            let bounds = local_bounds(element);
            return Some((bounds.center(), bounds.size(), element.angle));
        }
        let bounds = self.selection_bounds()?;
        Some((bounds.center(), bounds.size(), 0.0))
    }

    pub(crate) fn shows_transform_handles(&self) -> bool {
        // The reference app shows no bounding box for a lone 2-point line/arrow, just its points
        let two_point_line = self.single_line().is_some_and(|index| self.line_points(index).len() == 2);
        self.tool == Tool::Select
            && self.editing.is_none()
            && self.line_editor.is_none()
            && self.multi.is_none()
            && !two_point_line
            // a rope's shape is the windows it ties: no box to scale or turn it
            && !self.only_selected(|element| element.rope)
    }

    pub(crate) fn handle_corners(&self) -> Option<[Pos2; 4]> {
        let (center, size, angle) = self.selection_frame()?;
        let screen_center = self.to_screen(center);
        let half = size * self.zoom / 2.0 + Vec2::splat(5.0);
        Some(
            [(-1.0, -1.0), (1.0, -1.0), (1.0, 1.0), (-1.0, 1.0)]
                .map(|(sign_x, sign_y)| screen_center + rot(Vec2::new(sign_x * half.x, sign_y * half.y), angle)),
        )
    }

    /// Circle above the box, 16px gap like the reference app's ROTATION_RESIZE_HANDLE_GAP.
    pub(crate) fn rotation_handle(&self) -> Option<Pos2> {
        // windows and notes (with their text) stay upright: a turned one could no longer be resized by its edges
        if self
            .only_selected(|element| element.is_node() || element.created.is_some() || element.rope || note_of_label(&self.scene.elements, element).is_some())
        {
            return None;
        }
        let (center, size, angle) = self.selection_frame()?;
        Some(self.to_screen(center) + rot(Vec2::new(0.0, -(size.y * self.zoom / 2.0 + 5.0 + 16.0)), angle))
    }

    pub(crate) fn hovered_handle(&self, pos: Pos2) -> Option<usize> {
        if !self.shows_transform_handles() {
            return None;
        }
        self.handle_corners()?.iter().position(|corner| corner.distance(pos) <= 7.0)
    }

    pub(crate) fn hovered_rotation(&self, pos: Pos2) -> bool {
        self.shows_transform_handles() && self.rotation_handle().is_some_and(|handle| handle.distance(pos) <= 8.0)
    }

    /// A window's edge or corner under the pointer, unselected too and at any zoom: on screen, a
    /// band from 3 px inside its frame to 10 px outside (the reference app's edge handle), a corner
    /// 16 px along each side. A window over it hides it: the band never takes a press off another
    /// window's body. Gives the element, the corner it moves from (0 lt, 1 rt, 2 rb, 3 lb) and the
    /// axes that follow the pointer (an edge keeps the other one).
    pub(crate) fn window_grip(&self, pos: Pos2) -> Option<(usize, usize, Vec2)> {
        const INSIDE: f32 = 3.0;
        const OUTSIDE: f32 = 10.0;
        const CORNER: f32 = 16.0;
        for (index, element) in self.scene.elements.iter().enumerate().rev() {
            let bounds = local_bounds(element);
            let screen = Rect::from_two_pos(self.to_screen(bounds.min), self.to_screen(bounds.max));
            let window = element.is_node() || element.created.is_some();
            if !window || element.angle != 0.0 || element.locked || element.rope || self.terminal_elevated.as_ref() == Some(&element.id) {
                if window && screen.contains(pos) {
                    return None;
                }
                continue;
            }
            if screen.shrink(INSIDE).contains(pos) {
                return None;
            }
            if !screen.expand(OUTSIDE).contains(pos) {
                continue;
            }
            // how far in from each side (negative outside); a side is near within its band
            let gaps = [pos.x - screen.left(), screen.right() - pos.x, pos.y - screen.top(), screen.bottom() - pos.y];
            let [left, right, top, bottom] = gaps.map(|gap| gap < INSIDE);
            let [left_c, right_c, top_c, bottom_c] = gaps.map(|gap| gap < CORNER);
            return match (left, right, top, bottom) {
                _ if (left || top) && left_c && top_c => Some((index, 0, Vec2::splat(1.0))),
                _ if (right || top) && right_c && top_c => Some((index, 1, Vec2::splat(1.0))),
                _ if (right || bottom) && right_c && bottom_c => Some((index, 2, Vec2::splat(1.0))),
                _ if (left || bottom) && left_c && bottom_c => Some((index, 3, Vec2::splat(1.0))),
                (true, ..) => Some((index, 0, Vec2::new(1.0, 0.0))),
                (_, true, ..) => Some((index, 2, Vec2::new(1.0, 0.0))),
                (_, _, true, _) => Some((index, 0, Vec2::new(0.0, 1.0))),
                (.., true) => Some((index, 2, Vec2::new(0.0, 1.0))),
                _ => None,
            };
        }
        None
    }

    /// The one line or arrow selected, whose points can be edited (never a rope: it follows its ends).
    pub(crate) fn single_line(&self) -> Option<usize> {
        match self.selected[..] {
            [index] if matches!(self.scene.elements.get(index), Some(Element { kind: ElementKind::Line { .. }, rope: false, .. })) => Some(index),
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
        let Some(index) = self.editable_line() else {
            return Vec::new();
        };
        self.line_points(index).iter().enumerate().map(|(point_index, point)| (point_index, self.to_screen(Pos2::new(point[0], point[1])))).collect()
    }

    /// Segment midpoints (on the curve for curved lines), shown in the editor or on 2-point lines.
    pub(crate) fn midpoint_handles(&self) -> Vec<(usize, Pos2)> {
        let Some(index) = self.editable_line() else {
            return Vec::new();
        };
        let points = self.line_points(index);
        if self.line_editor != Some(index) && points.len() != 2 {
            return Vec::new();
        }
        let path = line_path(&self.scene.elements[index]);
        let curved = path.len() != points.len();
        (0..points.len() - 1)
            .map(|point_index| {
                let middle = if curved {
                    path[point_index * LINE_CURVE_STEPS + LINE_CURVE_STEPS / 2]
                } else {
                    [(points[point_index][0] + points[point_index + 1][0]) / 2.0, (points[point_index][1] + points[point_index + 1][1]) / 2.0]
                };
                (point_index, self.to_screen(Pos2::new(middle[0], middle[1])))
            })
            .collect()
    }

    pub(crate) fn delete_selected_point(&mut self) -> bool {
        let (Some(index), Some(point_index)) = (self.line_editor, self.selected_point) else {
            return false;
        };
        if self.line_points(index).len() <= 2 {
            return true;
        }
        self.snapshot();
        if let Some(points) = self.line_points_mut(index) {
            points.remove(point_index.min(points.len() - 1));
        }
        self.selected_point = None;
        true
    }

    pub(crate) fn finish_multi(&mut self) {
        let Some(index) = self.multi.take() else {
            return;
        };
        let remaining = self.line_points_mut(index).map(|points| {
            points.pop(); // the point following the cursor
            points.len()
        });
        if let Some(&last) = self.line_points(index).last().filter(|_| remaining.unwrap_or(0) >= 2) {
            let binding = self.bind_at(Pos2::new(last[0], last[1]), Some(index));
            self.scene.elements[index].ends.end = binding;
        }
        if remaining.unwrap_or(0) < 2 {
            self.scene.elements.remove(index);
            self.selected.clear();
        } else {
            self.selected = vec![index];
        }
        self.dirty = true;
        self.tool = Tool::Select;
    }

    /// Shift snaps a segment to 15° steps around `from`.
    pub(crate) fn snap_angle(&self, from: [f32; 2], to: Pos2) -> Pos2 {
        if !self.mods.shift {
            return to;
        }
        let delta = to - Pos2::new(from[0], from[1]);
        let step = std::f32::consts::PI / 12.0;
        Pos2::new(from[0], from[1]) + Vec2::angled((delta.y.atan2(delta.x) / step).round() * step) * delta.length()
    }

    pub(crate) fn hit(&self, world: Pos2) -> Option<usize> {
        let tolerance = 10.0 / self.zoom;
        // a note's text is part of its card: clicks pass over it to the card
        // windows are drawn over the drawings, so a click finds them first
        let hit = |rope: bool, node: bool| {
            self.scene.elements.iter().rposition(|element| {
                element.rope == rope
                    && (rope || element.is_node() == node)
                    && !element.locked
                    && note_of_label(&self.scene.elements, element).is_none()
                    && !hidden_page(&self.scene.elements, element)
                    && if rope { self.on_rope(element, world, tolerance) } else { element.contains(world, tolerance) }
            })
        };
        // ropes are drawn under everything, so a click finds them last too
        hit(false, true).or_else(|| hit(false, false)).or_else(|| hit(true, false))
    }

    /// Whether `world` is on the rope's curve as it is drawn now (it swings, its stored line doesn't).
    pub(crate) fn on_rope(&self, rope: &Element, world: Pos2, tolerance: f32) -> bool {
        self.rope_path(rope).windows(2).any(|window| distance_to_segment([world.x, world.y], window[0], window[1]) < tolerance)
    }

    /// 0..1 through drawing `element`, the front of the reveal queue; a long queue plays faster.
    pub(crate) fn reveal_progress(&self, element: &Element) -> f32 {
        let speed = 1.0 + self.reveal.len() as f64 / 8.0;
        ((self.now - self.reveal_since) * speed / reveal_duration(element)).clamp(0.0, 1.0) as f32
    }

    /// Preenchimento: paints the background of the shape under the pointer.
    pub(crate) fn fill_at(&mut self, world: Pos2) {
        let target = self.scene.elements.iter().rposition(|element| {
            !element.locked
                && element.fillable()
                && element.image.is_none()
                && shape_polygon(element).is_some_and(|poly| {
                    let point = element.to_local(world);
                    polygon_contains(&poly, [point.x, point.y])
                })
        });
        if let Some(index) = target {
            self.snapshot();
            let element = &mut self.scene.elements[index];
            // a transparent pick would be invisible: the web falls back to green
            element.style.fill = Some(self.style.fill.filter(|color| color[3] > 0).unwrap_or(BACKGROUND_PICKS[2]));
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
        let poly: Vec<[f32; 2]> = path.iter().map(|point| [point.x, point.y]).collect();
        let inside = |point: Pos2| polygon_contains(&poly, [point.x, point.y]);
        let mut selected: Vec<usize> = self
            .scene
            .elements
            .iter()
            .enumerate()
            .filter(|(_, element)| !element.locked && !is_label(element))
            .filter(|(_, element)| {
                let bounds = element_bounds(element);
                let mut corners = [bounds.left_top(), bounds.right_top(), bounds.right_bottom(), bounds.left_bottom()].into_iter();
                if self.prefs.box_overlap { corners.any(inside) || path.iter().any(|point| bounds.contains(*point)) } else { corners.all(inside) }
            })
            .map(|(index, _)| index)
            .collect();
        for index in selected.clone() {
            for member in self.group_members(index).into_iter().filter(|member| !self.is_card_text(*member)) {
                if !selected.contains(&member) {
                    selected.push(member);
                }
            }
        }
        self.selected = selected;
    }

    /// Nota: 260×160 centered on a click, or the dragged size (at least 250×100), with its text
    /// being edited right away.
    pub(crate) fn place_sticky(&mut self, start: Pos2, end: Pos2) {
        let delta = end - start;
        let rect = if delta.x.abs() * self.zoom < 24.0 && delta.y.abs() * self.zoom < 24.0 {
            Rect::from_center_size(start, NOTE_SIZE)
        } else {
            let (width, height) = (delta.x.abs().max(NOTE_MIN.x), delta.y.abs().max(NOTE_MIN.y));
            let x = if delta.x < 0.0 { start.x - width } else { start.x };
            let y = if delta.y < 0.0 { start.y - height } else { start.y };
            Rect::from_min_size(Pos2::new(x, y), Vec2::new(width, height))
        };
        let [note, mut text] = self.new_note(rect, String::new());
        text.style.font_size = 20.0;
        self.snapshot();
        self.scene.elements.extend([note, text]);
        let index = self.scene.elements.len() - 1;
        self.selected = vec![index];
        self.editing = Some(index);
        self.edit_focused = false;
    }

    /// The given elements plus everything lying inside any frame among them.
    pub(crate) fn with_frame_contents(&self, indices: &[usize]) -> Vec<usize> {
        let frames: Vec<Rect> = indices
            .iter()
            .filter_map(|index| self.scene.elements.get(*index))
            .filter(|element| element.frame.is_some())
            .map(|element| element_bounds(element).expand(1.0))
            .collect();
        let mut all = indices.to_vec();
        for (index, element) in self.scene.elements.iter().enumerate() {
            if !all.contains(&index) && !element.locked && frames.iter().any(|frame| frame.contains_rect(element_bounds(element))) {
                all.push(index);
            }
        }
        all
    }

    /// A lock icon right above the clicked locked element's top-left corner (the web's
    /// UnlockPopup, without the box); clicking it unlocks the whole unit and selects it.
    pub(crate) fn unlock_popup(&mut self, ctx: &Context) {
        let Some(ids) = self.active_locked.clone() else {
            return;
        };
        let unit: Vec<usize> =
            self.scene.elements.iter().enumerate().filter(|(_, element)| ids.contains(&element.id) && element.locked).map(|(index, _)| index).collect();
        let Some(bounds) = unit.iter().map(|index| element_bounds(&self.scene.elements[*index])).reduce(|first, second| first.union(second)) else {
            self.active_locked = None;
            return;
        };
        let colors = self.colors();
        // right above the corner; above a card's name when it has one
        let named = unit.iter().any(|index| self.scene.elements[*index].frame.as_ref().is_some_and(|frame_name| !frame_name.is_empty()));
        let name = if named { 4.0 * self.zoom + 13.0 * self.zoom.max(0.5) * 1.3 } else { 0.0 }; // see paint_element
        let size = 24.0;
        let at = self.to_screen(bounds.min) - Vec2::new(0.0, name + 2.0 + size);
        let response = egui::Area::new(egui::Id::unique("unlock_popup"))
            .fixed_pos(at)
            .order(egui::Order::Foreground)
            .show(ctx, |ui| {
                let (rect, response) = ui.allocate_exact_size(Vec2::splat(size), Sense::click());
                let color = if response.hovered() { PRIMARY } else { colors.text };
                paint_icon(ui.painter(), Icon::Lock(true), rect.center(), color);
                response.on_hover_cursor(CursorIcon::PointingHand).tip("Desbloquear")
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
        let mut position = 0;
        while position < unit.len() {
            let element = &self.scene.elements[unit[position]];
            let frame = element.frame.is_some().then(|| element_bounds(element).expand(1.0));
            for (other_index, other) in self.scene.elements.iter().enumerate() {
                let grouped = element.group_id.is_some() && other.group_id == element.group_id;
                if !unit.contains(&other_index) && (grouped || frame.is_some_and(|frame| frame.contains_rect(element_bounds(other)))) {
                    unit.push(other_index);
                }
            }
            position += 1;
        }
        unit
    }

    /// Locks the selection (Cmd+Shift+L); if all of it is locked already, unlocks it.
    pub(crate) fn toggle_lock_selected(&mut self) {
        if self.selected.is_empty() {
            return;
        }
        self.snapshot();
        let unit = self.lock_unit(&self.selected);
        let lock = unit.iter().any(|index| !self.scene.elements[*index].locked);
        for index in unit {
            self.scene.elements[index].locked = lock;
        }
        if lock {
            self.selected.clear();
        }
    }

    pub(crate) fn lock_all(&mut self) {
        if self.scene.elements.iter().all(|element| element.locked) {
            return;
        }
        self.snapshot();
        for element in &mut self.scene.elements {
            element.locked = true;
        }
        self.selected.clear();
    }

    pub(crate) fn unlock(&mut self, id: Option<&str>) {
        self.snapshot();
        for element in self.scene.elements.iter_mut().filter(|element| id.is_none_or(|id| element.id == id)) {
            element.locked = false;
        }
    }

    /// Whether element `index` is a sticky note's text: part of its card, never hit, selected or
    /// snapped to on its own.
    /// Runs `change` on the scene and keeps the same elements selected, by id, wherever they end
    /// up; the ones it took out leave the selection.
    pub(crate) fn keeping_selection(&mut self, change: impl FnOnce(&mut Self)) {
        let ids: Vec<String> = self.selected.iter().filter_map(|index| self.scene.elements.get(*index)).map(|element| element.id.clone()).collect();
        change(self);
        self.selected = ids.iter().filter_map(|id| self.scene.elements.iter().position(|element| &element.id == id)).collect();
    }

    pub(crate) fn is_card_text(&self, index: usize) -> bool {
        self.scene
            .elements
            .get(index)
            .is_some_and(|element| note_of_label(&self.scene.elements, element).is_some() || hidden_page(&self.scene.elements, element))
    }

    /// `indices` plus the text of the sticky notes among them: what moves, copies and deletes
    /// with its card.
    pub(crate) fn with_card_text(&self, indices: &[usize]) -> Vec<usize> {
        let mut all = indices.to_vec();
        // a pile's waiting pages go with its front page
        for page in indices
            .iter()
            .filter(|index| self.scene.elements.get(**index).is_some_and(|element| element.note.stacked && element.note.front))
            .flat_map(|index| self.stack_pages(*index))
        {
            if !all.contains(&page) {
                all.push(page);
            }
        }
        for label in all
            .clone()
            .iter()
            .filter(|index| self.scene.elements.get(**index).is_some_and(|element| element.created.is_some()))
            .filter_map(|index| note_label(&self.scene.elements, *index))
        {
            if !all.contains(&label) {
                all.push(label);
            }
        }
        all
    }

    pub(crate) fn group_members(&self, index: usize) -> Vec<usize> {
        let group = self.scene.elements[index].group_id.clone();
        self.scene
            .elements
            .iter()
            .enumerate()
            .filter_map(|(member, element)| {
                (member == index || (!element.locked && group.as_ref().is_some_and(|group_id| element.group_id.as_ref() == Some(group_id)))).then_some(member)
            })
            .collect()
    }
}
