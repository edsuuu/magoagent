use super::*;

impl DrawingApp {
    pub(crate) fn update_gesture(&mut self, pos: Pos2) {
        let previous = self.last_pointer.unwrap_or(pos);
        self.last_pointer = Some(pos);
        let world = self.to_world(pos);
        if let Some((k, _)) = self.midpoint_press.filter(|(_, origin)| pos.distance(*origin) > 4.0) {
            self.midpoint_press = None;
            if let Some(line) = self.editable_line() {
                self.snapshot();
                if let Some(points) = self.line_points_mut(line) {
                    points.insert(k, [world.x, world.y]);
                }
                self.point_drag = Some(k);
                self.selected_point = Some(k);
            }
        }
        if let Some(i) = self.multi {
            let len = self.line_points(i).len();
            if len >= 2 {
                let snapped = self.snap_angle(self.line_points(i)[len - 2], world);
                if let Some(points) = self.line_points_mut(i) {
                    points[len - 1] = [snapped.x, snapped.y];
                }
            }
        }
        if self.panning {
            self.glide = None;
            self.pan += pos - previous;
        } else if let (Some(k), Some(i)) = (self.point_drag, self.single_line()) {
            let points = self.line_points(i);
            let neighbor = if k > 0 { points.get(k - 1) } else { points.get(1) }.copied();
            let target = neighbor.map_or(world, |n| self.snap_angle(n, world));
            if let Some(p) = self.line_points_mut(i).and_then(|points| points.get_mut(k)) {
                *p = [target.x, target.y];
            }
        } else if let Some(rotate) = &self.rotating {
            let mut delta = (world - rotate.pivot).angle() - rotate.start;
            if self.mods.shift {
                let step = std::f32::consts::PI / 12.0;
                let base = match &rotate.originals[..] {
                    [one] => one.angle,
                    _ => 0.0,
                };
                delta = ((base + delta) / step).round() * step - base;
            }
            for (i, original) in self.selected.iter().zip(&rotate.originals) {
                let mut element = original.clone();
                // a note's text stays upright with its note, which centers it again
                if !self.is_card_text(*i) {
                    element.rotate_around(rotate.pivot, delta);
                }
                self.scene.elements[*i] = element;
            }
        } else if let Some(resize) = &self.resizing {
            let local = resize.center + rot(world - resize.center, -resize.angle);
            let mut corner = local + resize.grab;
            // an edge grip moves one axis only: the grabbed corner keeps its other coordinate
            if resize.free.x == 0.0 {
                corner.x = resize.pin.x;
            }
            if resize.free.y == 0.0 {
                corner.y = resize.pin.y;
            }
            let mut to = Rect::from_two_pos(resize.anchor, corner);
            if self.mods.shift && resize.free == Vec2::splat(1.0) && resize.from.height() > 0.0 {
                // keep aspect ratio
                let ratio = resize.from.width() / resize.from.height();
                let w = to.width().max(to.height() * ratio);
                let size = Vec2::new(w, w / ratio);
                let dir = Vec2::new((corner.x - resize.anchor.x).signum(), (corner.y - resize.anchor.y).signum());
                to = Rect::from_two_pos(resize.anchor, resize.anchor + size * dir);
            }
            // a window or note alone (with its text) keeps its creation minimum and never turns inside out
            let floors: Vec<Vec2> = resize.originals.iter().filter_map(Element::min_size).collect();
            if let ([min], true) = (&floors[..], resize.originals.iter().all(|e| e.min_size().is_some() || is_label(e))) {
                // only on the axes the grip moves: an edge never pulls the other one up to its floor
                let min = *min * resize.free;
                let dir = Vec2::new((resize.pin.x - resize.anchor.x).signum(), (resize.pin.y - resize.anchor.y).signum());
                let along = |d: f32, span: f32| if d > 0.0 { span } else { 0.0 };
                let size = Vec2::new(
                    along((corner.x - resize.anchor.x) * dir.x, to.width()).max(min.x),
                    along((corner.y - resize.anchor.y) * dir.y, to.height()).max(min.y),
                );
                to = Rect::from_two_pos(resize.anchor, resize.anchor + size * dir);
            }
            // the resized box lives in the rotated frame: put its center back in world space
            let shift = resize.center + rot(to.center() - resize.center, resize.angle) - to.center();
            for (i, original) in self.selected.iter().zip(&resize.originals) {
                let mut element = original.clone();
                element.scale(resize.from, to);
                element.move_by(shift);
                self.scene.elements[*i] = element;
            }
        } else if self.moving {
            // a hand's tremble on a click is no drag
            if !self.move_snapshotted && pos.distance(self.to_screen(self.press_world)) > DRAG_SLOP {
                self.snapshot();
                self.detach_selected_arrows();
                self.move_snapshotted = true;
                let moving = self.with_card_text(&self.with_frame_contents(&self.selected));
                let originals = moving.iter().map(|i| self.scene.elements[*i].clone()).collect();
                self.move_origin = self.selection_bounds().map(|b| (moving, originals, b));
            }
            if let Some((moving, originals, bounds)) = self.move_origin.clone() {
                // absolute from the press point, so snapping never accumulates drift
                let mut delta = world - self.press_world;
                let (adjust, guides) = self.snap_box(bounds.translate(delta));
                delta += adjust;
                self.snap_guides = guides;
                for (i, original) in moving.into_iter().zip(originals) {
                    let mut moved = original;
                    moved.move_by(delta);
                    self.scene.elements[i] = moved;
                }
            }
        } else if self.drawing_start.is_some() && matches!(self.tool, Tool::Draw | Tool::AutoShape) {
            self.drawing_points.push([world.x, world.y]);
        } else if self.laser_down {
            self.laser.push((world, self.now));
        } else if let Some(lasso) = &mut self.lasso {
            lasso.push(world);
            let path = lasso.clone();
            self.select_in_lasso(&path);
        } else if self.erasing.is_some() {
            // sample the swept segment so fast strokes don't skip elements
            let from = self.to_world(previous);
            let steps = (from.distance(world) * self.zoom / 4.0).ceil().max(1.0) as usize;
            for step in 1..=steps {
                self.erase_at(from.lerp(world, step as f32 / steps as f32));
            }
        }
    }

    pub(crate) fn release(&mut self, pos: Pos2) {
        let end = self.to_world(pos);
        self.panning = false;
        // a plain click on a rope opens its popover (ui/links.rs)
        if self.tool == Tool::Select
            && self.press_world.distance(end) * self.zoom < 3.0
            && self.hit(self.press_world).is_some_and(|i| self.scene.elements[i].rope)
        {
            self.open_rope_popover(pos, self.press_world);
        }
        if let (Some(start), Tool::Select) = (self.marquee, self.tool) {
            // a plain click on a locked element offers to unlock it (and its group / card)
            let tolerance = 10.0 / self.zoom;
            if start.distance(end) * self.zoom < 3.0 {
                let hit = |e: &Element| e.contains(end, tolerance) || (e.frame.is_some() && element_bounds(e).contains(end));
                if let Some(i) = self.scene.elements.iter().rposition(hit).filter(|i| self.scene.elements[*i].locked) {
                    // inside a locked card, the card and everything in it is the unit
                    let b = element_bounds(&self.scene.elements[i]);
                    let card = self.scene.elements.iter().position(|e| e.locked && e.frame.is_some() && element_bounds(e).expand(1.0).contains_rect(b));
                    let unit = self.lock_unit(&[card.unwrap_or(i)]);
                    self.active_locked = Some(unit.into_iter().map(|j| self.scene.elements[j].id.clone()).collect());
                }
            }
        }
        if let Some(start) = self.marquee.take() {
            let area = Rect::from_two_pos(start, end);
            let mut selected: Vec<usize> = self
                .scene
                .elements
                .iter()
                .enumerate()
                .filter(|(i, e)| !e.locked && !self.is_card_text(*i))
                .filter_map(|(i, e)| {
                    let b = element_bounds(e);
                    (if self.prefs.box_overlap { area.intersects(b) } else { area.contains_rect(b) }).then_some(i)
                })
                .collect();
            let groups: Vec<String> = selected.iter().filter_map(|i| self.scene.elements[*i].group_id.clone()).collect();
            for (i, e) in self.scene.elements.iter().enumerate() {
                if e.group_id.as_ref().is_some_and(|g| groups.contains(g)) && !selected.contains(&i) && !self.is_card_text(i) {
                    selected.push(i);
                }
            }
            for i in selected {
                if !self.selected.contains(&i) {
                    self.selected.push(i);
                }
            }
        }
        self.laser_down = false;
        if let Some(path) = self.lasso.take() {
            self.select_in_lasso(&path);
        }
        if let (Some(start), Tool::Sticky) = (self.drawing_start, self.tool) {
            self.drawing_start = None;
            self.place_sticky(start, end);
        }
        if let (Some(start), Tool::Terminal) = (self.drawing_start, self.tool) {
            self.drawing_start = None;
            self.open_terminal_dialog(start, end);
        }
        if let (Some(start), Tool::Portal) = (self.drawing_start, self.tool) {
            self.drawing_start = None;
            self.open_portal_dialog(start, end);
        }
        if let (Some(start), Tool::Chat) = (self.drawing_start, self.tool) {
            self.drawing_start = None;
            self.place_chat(start);
        }
        if let (Some(start), Tool::FileTree) = (self.drawing_start, self.tool) {
            self.drawing_start = None;
            self.open_filetree_dialog(start, end);
        }
        if let (Some(_), Tool::AutoShape) = (self.drawing_start, self.tool) {
            self.drawing_start = None;
            if let Some(element) = recognize_shape(self, &self.drawing_points.clone()) {
                self.snapshot();
                self.scene.elements.push(element);
                self.selected = vec![self.scene.elements.len() - 1];
            }
            self.drawing_points.clear();
        }
        if let Some(start) = self.drawing_start.take() {
            let click = start.distance(end) * self.zoom < 3.0;
            if click && matches!(self.tool, Tool::Line | Tool::Arrow) {
                // click-click mode: second point follows the cursor until the next click
                let mut element = self.temporary_element(start, start);
                if let ElementKind::Line { points, .. } = &mut element.kind {
                    points.truncate(1);
                    points.push([end.x, end.y]);
                }
                element.ends.start = self.bind_at(start, None);
                self.snapshot();
                self.scene.elements.push(element);
                self.multi = Some(self.scene.elements.len() - 1);
                self.selected = vec![self.scene.elements.len() - 1];
            } else if start.distance(end) > 1.0 / self.zoom {
                let element = self.bind_new_arrow(self.temporary_element(start, end));
                self.snapshot();
                self.scene.elements.push(element);
                self.selected = vec![self.scene.elements.len() - 1];
                if self.tool != Tool::Draw {
                    self.tool = Tool::Select;
                }
            }
            self.drawing_points.clear();
        }
        if let Some(mut erased) = self.erasing.take() {
            if !erased.is_empty() {
                self.snapshot();
                erased.sort_unstable();
                for i in erased.into_iter().rev() {
                    self.scene.elements.remove(i);
                }
                self.selected.clear();
            }
        }
        // a click on a note's text (at most 4 px of movement) takes its focus: the editor types
        // into it from here on. Its header and the empty part below the text only selected it
        if let Some(label) = self.note_press.take()
            && self.tool == Tool::Select
            && pos.distance(self.to_screen(self.press_world)) <= DRAG_SLOP
        {
            self.editing = Some(label);
            self.edit_focused = false;
        }
        if self.moving && self.move_snapshotted {
            self.drop_into_stack();
        }
        self.moving = false;
        self.move_snapshotted = false;
        if let (Some(k), Some(i)) = (self.point_drag, self.single_line()) {
            let last = self.line_points(i).len() - 1;
            if matches!(self.scene.elements[i].kind, ElementKind::Line { arrow: true, .. }) && (k == 0 || k == last) {
                let p = self.line_points(i)[k];
                let binding = self.bind_at(Pos2::new(p[0], p[1]), Some(i));
                let ends = &mut self.scene.elements[i].ends;
                *(if k == 0 { &mut ends.start } else { &mut ends.end }) = binding;
            }
        }
        self.resizing = None;
        self.rotating = None;
        self.point_drag = None;
        self.midpoint_press = None;
        self.move_origin = None;
        self.snap_guides.clear();
    }
}
