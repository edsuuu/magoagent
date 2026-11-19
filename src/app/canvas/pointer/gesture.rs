use super::*;

impl DrawingApp {
    pub(crate) fn update_gesture(&mut self, pos: Pos2) {
        let previous = self.last_pointer.unwrap_or(pos);

        self.last_pointer = Some(pos);
        let world = self.to_world(pos);

        if let Some((point_index, _)) = self.midpoint_press.filter(|(_, origin)| pos.distance(*origin) > 4.0) {
            self.midpoint_press = None;

            if let Some(line) = self.editable_line() {
                self.snapshot();

                if let Some(points) = self.line_points_mut(line) {
                    points.insert(point_index, [world.x, world.y]);
                }

                self.point_drag = Some(point_index);
                self.selected_point = Some(point_index);
            }
        }

        if let Some(index) = self.multi {
            let count = self.line_points(index).len();

            if count >= 2 {
                let snapped = self.snap_angle(self.line_points(index)[count - 2], world);

                if let Some(points) = self.line_points_mut(index) {
                    points[count - 1] = [snapped.x, snapped.y];
                }
            }
        }

        if self.panning {
            self.glide = None;
            self.pan += pos - previous;
        } else if let (Some(point_index), Some(index)) = (self.point_drag, self.single_line()) {
            let points = self.line_points(index);
            let neighbor = if point_index > 0 { points.get(point_index - 1) } else { points.get(1) }.copied();
            let target = neighbor.map_or(world, |neighbor_point| self.snap_angle(neighbor_point, world));

            if let Some(point) = self.line_points_mut(index).and_then(|points| points.get_mut(point_index)) {
                *point = [target.x, target.y];
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

            for (index, original) in self.selected.iter().zip(&rotate.originals) {
                let mut element = original.clone();

                // a note's text stays upright with its note, which centers it again
                if !self.is_card_text(*index) {
                    element.rotate_around(rotate.pivot, delta);
                }

                self.scene.elements[*index] = element;
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
                let width = to.width().max(to.height() * ratio);
                let size = Vec2::new(width, width / ratio);
                let direction = Vec2::new((corner.x - resize.anchor.x).signum(), (corner.y - resize.anchor.y).signum());

                to = Rect::from_two_pos(resize.anchor, resize.anchor + size * direction);
            }

            // a window or note alone (with its text) keeps its creation minimum and never turns inside out
            let floors: Vec<Vec2> = resize.originals.iter().filter_map(Element::min_size).collect();

            if let ([min], true) = (&floors[..], resize.originals.iter().all(|element| element.min_size().is_some() || is_label(element))) {
                // only on the axes the grip moves: an edge never pulls the other one up to its floor
                let min = *min * resize.free;
                let direction = Vec2::new((resize.pin.x - resize.anchor.x).signum(), (resize.pin.y - resize.anchor.y).signum());
                let along = |distance: f32, span: f32| if distance > 0.0 { span } else { 0.0 };

                let size = Vec2::new(
                    along((corner.x - resize.anchor.x) * direction.x, to.width()).max(min.x),
                    along((corner.y - resize.anchor.y) * direction.y, to.height()).max(min.y),
                );

                to = Rect::from_two_pos(resize.anchor, resize.anchor + size * direction);
            }

            // the resized box lives in the rotated frame: put its center back in world space
            let shift = resize.center + rot(to.center() - resize.center, resize.angle) - to.center();

            for (index, original) in self.selected.iter().zip(&resize.originals) {
                let mut element = original.clone();

                element.scale(resize.from, to);
                element.move_by(shift);
                self.scene.elements[*index] = element;
            }
        } else if self.moving {
            // a hand's tremble on a click is no drag
            if !self.move_snapshotted && pos.distance(self.to_screen(self.press_world)) > DRAG_SLOP {
                self.snapshot();
                self.detach_selected_arrows();
                self.move_snapshotted = true;
                let moving = self.with_card_text(&self.with_frame_contents(&self.selected));
                let originals = moving.iter().map(|index| self.scene.elements[*index].clone()).collect();

                self.move_origin = self.selection_bounds().map(|bounds| (moving, originals, bounds));
            }

            if let Some((moving, originals, bounds)) = self.move_origin.clone() {
                // absolute from the press point, so snapping never accumulates drift
                let mut delta = world - self.press_world;
                let (adjust, guides) = self.snap_box(bounds.translate(delta));

                delta += adjust;
                self.snap_guides = guides;

                for (index, original) in moving.into_iter().zip(originals) {
                    let mut moved = original;

                    moved.move_by(delta);
                    self.scene.elements[index] = moved;
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
            && self.hit(self.press_world).is_some_and(|index| self.scene.elements[index].rope)
        {
            self.open_rope_popover(pos, self.press_world);
        }

        if let (Some(start), Tool::Select) = (self.marquee, self.tool) {
            // a plain click on a locked element offers to unlock it (and its group / card)
            let tolerance = 10.0 / self.zoom;

            if start.distance(end) * self.zoom < 3.0 {
                let hit = |element: &Element| element.contains(end, tolerance) || (element.frame.is_some() && element_bounds(element).contains(end));

                if let Some(index) = self.scene.elements.iter().rposition(hit).filter(|index| self.scene.elements[*index].locked) {
                    // inside a locked card, the card and everything in it is the unit
                    let bounds = element_bounds(&self.scene.elements[index]);

                    let card = self
                        .scene
                        .elements
                        .iter()
                        .position(|element| element.locked && element.frame.is_some() && element_bounds(element).expand(1.0).contains_rect(bounds));

                    let unit = self.lock_unit(&[card.unwrap_or(index)]);

                    self.active_locked = Some(unit.into_iter().map(|member| self.scene.elements[member].id.clone()).collect());
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
                .filter(|(index, element)| !element.locked && !self.is_card_text(*index))
                .filter_map(|(index, element)| {
                    let bounds = element_bounds(element);

                    (if self.prefs.box_overlap { area.intersects(bounds) } else { area.contains_rect(bounds) }).then_some(index)
                })
                .collect();

            let groups: Vec<String> = selected.iter().filter_map(|index| self.scene.elements[*index].group_id.clone()).collect();

            for (index, element) in self.scene.elements.iter().enumerate() {
                if element.group_id.as_ref().is_some_and(|group| groups.contains(group)) && !selected.contains(&index) && !self.is_card_text(index) {
                    selected.push(index);
                }
            }

            for index in selected {
                if !self.selected.contains(&index) {
                    self.selected.push(index);
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

        if let Some(mut erased) = self.erasing.take()
            && !erased.is_empty()
        {
            self.snapshot();
            erased.sort_unstable();

            for index in erased.into_iter().rev() {
                self.scene.elements.remove(index);
            }

            self.selected.clear();
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

        if let (Some(point_index), Some(index)) = (self.point_drag, self.single_line()) {
            let last = self.line_points(index).len() - 1;

            if matches!(self.scene.elements[index].kind, ElementKind::Line { arrow: true, .. }) && (point_index == 0 || point_index == last) {
                let point = self.line_points(index)[point_index];
                let binding = self.bind_at(Pos2::new(point[0], point[1]), Some(index));
                let ends = &mut self.scene.elements[index].ends;

                *(if point_index == 0 { &mut ends.start } else { &mut ends.end }) = binding;
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
