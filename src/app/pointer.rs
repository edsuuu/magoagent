//! Pointer gestures on the canvas: drawing, moving, binding, releasing.

use crate::prelude::*;

impl DrawingApp {
    pub(crate) fn press_select(&mut self, pos: Pos2, world: Pos2, modifiers: Modifiers) {
        if let Some((k, _)) = self.point_handles().into_iter().find(|(_, p)| p.distance(pos) <= 10.0) {
            self.snapshot();
            if let Some(i) = self.single_line() {
                let last = self.line_points(i).len() - 1;
                let ends = &mut self.scene.elements[i].ends;
                if k == 0 {
                    ends.start = None;
                } else if k == last {
                    ends.end = None;
                }
            }
            self.point_drag = Some(k);
            self.selected_point = Some(k);
            return;
        }
        if let (Some(line), Some((k, _))) =
            (self.editable_line(), self.midpoint_handles().into_iter().find(|(_, p)| p.distance(pos) <= 8.0))
        {
            self.snapshot();
            if let Some(points) = self.line_points_mut(line) {
                points.insert(k + 1, [world.x, world.y]);
            }
            self.point_drag = Some(k + 1);
            self.selected_point = Some(k + 1);
            return;
        }
        if let Some(line) = self.line_editor {
            if modifiers.alt {
                self.snapshot();
                if let Some(points) = self.line_points_mut(line) {
                    points.push([world.x, world.y]);
                    self.selected_point = Some(points.len() - 1);
                }
                return;
            }
            self.line_editor = None;
            self.selected_point = None;
            if self.hit(world) == Some(line) {
                self.moving = true;
                self.move_snapshotted = false;
                return;
            }
        }
        if let (true, Some((center, _, _))) = (self.hovered_rotation(pos), self.selection_frame()) {
            self.snapshot();
            self.detach_selected_arrows();
            self.rotating = Some(Rotate {
                pivot: center,
                start: (world - center).angle(),
                originals: self.selected.iter().map(|i| self.scene.elements[*i].clone()).collect(),
            });
        } else if let (Some(corner), Some((center, size, angle))) = (self.hovered_handle(pos), self.selection_frame()) {
            let from = Rect::from_center_size(center, size);
            let corners = [from.left_top(), from.right_top(), from.right_bottom(), from.left_bottom()];
            let local = center + rot(world - center, -angle);
            self.snapshot();
            self.detach_selected_arrows();
            self.resizing = Some(Resize {
                anchor: corners[(corner + 2) % 4],
                grab: corners[corner] - local,
                from,
                center,
                angle,
                originals: self.selected.iter().map(|i| self.scene.elements[*i].clone()).collect(),
            });
        } else if let Some(i) = self.hit(world) {
            let members = self.group_members(i);
            if modifiers.shift {
                for j in members {
                    if self.selected.contains(&j) {
                        self.selected.retain(|x| *x != j);
                    } else {
                        self.selected.push(j);
                    }
                }
            } else if !self.selected.contains(&i) {
                self.selected = members;
            }
            self.moving = true;
            self.move_snapshotted = false;
        } else if !modifiers.shift && self.selection_bounds().is_some_and(|b| b.contains(world)) {
            self.moving = true;
            self.move_snapshotted = false;
        } else {
            if !modifiers.shift {
                self.selected.clear();
            }
            self.marquee = Some(world);
        }
    }

    pub(crate) fn erase_at(&mut self, world: Pos2) {
        if let (Some(i), Some(erasing)) = (self.hit(world), &self.erasing) {
            if !erasing.contains(&i) {
                let members = self.group_members(i);
                if let Some(erasing) = &mut self.erasing {
                    erasing.extend(members.into_iter().filter(|j| !erasing.contains(j)).collect::<Vec<_>>());
                }
            }
        }
    }

    pub(crate) fn handle_canvas(&mut self, response: &egui::Response, rect: Rect) {
        self.canvas_rect = rect;
        let ctx = response.ctx.clone();
        if ctx.current_pass_index() > 0 {
            return;
        }
        self.mods = ctx.input(|i| i.modifiers);
        self.now = ctx.input(|i| i.time);
        // images dragged in from Finder land where they're dropped
        let dropped: Vec<PathBuf> = ctx.input(|i| i.raw.dropped_files.iter().map(|f| f.path().to_path_buf()).collect());
        self.drop_folders(&ctx, &dropped);
        let images: Vec<PathBuf> = dropped
            .into_iter()
            .filter(|p| p.extension().and_then(|e| e.to_str()).is_some_and(|e| IMAGE_EXTENSIONS.contains(&e.to_lowercase().as_str())))
            .collect();
        if !images.is_empty() {
            let at = ctx.input(|i| i.pointer.hover_pos()).unwrap_or(self.canvas_rect.center());
            self.insert_image_files(images, self.to_world(at));
        }
        let space = !ctx.egui_wants_keyboard_input() && ctx.input(|i| i.key_down(Key::Space));
        let events = ctx.input(|i| i.events.clone());
        for event in events {
            match event {
                egui::Event::PointerButton {
                    pos,
                    button: button @ (egui::PointerButton::Primary | egui::PointerButton::Middle),
                    pressed: true,
                    modifiers,
                } => {
                    if !rect.contains(pos) || ctx.layer_id_at(pos) != Some(response.layer_id) {
                        continue;
                    }
                    self.menu_open = false;
                    // any press on the canvas dismisses the unlock button (its own clicks never get here)
                    self.active_locked = None;
                    self.mods = modifiers;
                    self.last_pointer = Some(pos);
                    if self.editing.is_some() {
                        self.finish_text();
                        continue;
                    }
                    let world = self.to_world(pos);
                    if let Some(i) = self.multi {
                        // click on the last placed point finishes, any other click adds a point
                        let points = self.line_points(i);
                        let last = points.len().checked_sub(2).map(|k| points[k]);
                        if last.is_some_and(|p| self.to_screen(Pos2::new(p[0], p[1])).distance(pos) <= 10.0) {
                            self.finish_multi();
                        } else if let Some(points) = self.line_points_mut(i) {
                            let floating = *points.last().unwrap_or(&[world.x, world.y]);
                            points.push(floating);
                        }
                        continue;
                    }
                    self.press_world = world;
                    // macOS: the empty strip along the window buttons moves the window
                    if cfg!(target_os = "macos") && pos.y - rect.top() < TOP_ROW + 44.0 && self.hit(world).is_none() && self.tool == Tool::Select {
                        ctx.send_viewport_cmd(egui::ViewportCommand::StartDrag);
                        continue;
                    }
                    if let Some(from) = self.connecting.take() {
                        // "Conectar": the next element clicked (a note's label counts as its note)
                        if let Some(i) = self.hit(world) {
                            let to = self.scene.elements[i].id.trim_end_matches("-label").to_string();
                            self.connect(&from, &to);
                        }
                        continue;
                    }
                    if button == egui::PointerButton::Middle || space || self.tool == Tool::Hand || self.prefs.view_mode {
                        self.panning = true;
                        continue;
                    }
                    match self.tool {
                        Tool::Select => self.press_select(pos, world, modifiers),
                        Tool::Text => self.start_text(world),
                        Tool::Eraser => {
                            self.erasing = Some(Vec::new());
                            self.erase_at(world);
                        }
                        Tool::Laser => {
                            self.laser_down = true;
                            self.laser.push((world, self.now));
                        }
                        Tool::Fill => self.fill_at(world),
                        Tool::Lasso => self.lasso = Some(vec![world]),
                        _ => {
                            let start = if matches!(self.tool, Tool::Draw | Tool::AutoShape) { world } else { self.snap_point(world) };
                            self.drawing_start = Some(start);
                            self.drawing_points = vec![[world.x, world.y]];
                        }
                    }
                }
                egui::Event::PointerMoved(pos) => self.update_gesture(pos),
                egui::Event::PointerButton {
                    pos,
                    button: egui::PointerButton::Primary | egui::PointerButton::Middle,
                    pressed: false,
                    ..
                } => {
                    self.update_gesture(pos);
                    self.release(pos);
                }
                _ => {}
            }
        }
        let double = ctx.input(|i| i.pointer.button_double_clicked(egui::PointerButton::Primary));
        if let (true, Tool::Select, None, false, Some(pos)) =
            (double, self.tool, self.editing, self.prefs.view_mode, ctx.input(|i| i.pointer.interact_pos()))
        {
            if rect.contains(pos) && ctx.layer_id_at(pos) == Some(response.layer_id) {
                let world = self.to_world(pos);
                match self.hit(world) {
                    // lines: double-click; arrows: Cmd + double-click (plain double-click labels arrows in the reference app)
                    Some(i) if matches!(self.scene.elements[i].kind, ElementKind::Line { arrow, .. } if !arrow || self.mods.command) => {
                        self.moving = false;
                        self.point_drag = None;
                        self.selected = vec![i];
                        self.line_editor = Some(i);
                        self.selected_point = None;
                    }
                    Some(i) if self.scene.elements[i].terminal.is_some() => self.double_click_terminal(i, world),
                    Some(i) if self.scene.elements[i].ide.is_some() => self.double_click_ide(i),
                    Some(i) if matches!(self.scene.elements[i].kind, ElementKind::Text { .. }) => {
                        self.snapshot();
                        self.moving = false;
                        self.selected = vec![i];
                        self.editing = Some(i);
                        self.edit_focused = false;
                    }
                    None => {
                        self.marquee = None;
                        self.start_text(world);
                    }
                    _ => {}
                }
            }
        }
        self.update_bindings();
    }

    /// maxBindingDistance_simple: 15px, a bit more when zoomed out (in world units).
    pub(crate) fn binding_reach(&self) -> f32 {
        (15.0 / (self.zoom.min(1.0) * 1.5)).clamp(15.0, 30.0) / self.zoom
    }

    /// Bindable shape under `world` (inside it or within reach of its outline). Cmd disables binding.
    pub(crate) fn binding_target(&self, world: Pos2, exclude: Option<usize>) -> Option<usize> {
        if self.mods.command || !self.prefs.arrow_binding {
            return None;
        }
        let reach = self.binding_reach();
        let p = [world.x, world.y];
        self.scene.elements.iter().enumerate().rev().find_map(|(i, e)| {
            if Some(i) == exclude || !bindable(e) {
                return None;
            }
            let poly = binding_outline(e, 0.0);
            let n = poly.len();
            let near = (0..n).any(|k| distance_to_segment(p, poly[k], poly[(k + 1) % n]) <= reach);
            (near || polygon_contains(&poly, p)).then_some(i)
        })
    }

    /// Deep inside a shape binds "inside" (to that exact point), near its outline "orbit"
    /// (stops at the outline); Option forces "inside" — as observed in the reference app.
    pub(crate) fn bind_at(&self, world: Pos2, exclude: Option<usize>) -> Option<Binding> {
        let target = &self.scene.elements[self.binding_target(world, exclude)?];
        let poly = binding_outline(target, 0.0);
        let p = [world.x, world.y];
        let n = poly.len();
        let edge = (0..n).map(|k| distance_to_segment(p, poly[k], poly[(k + 1) % n])).fold(f32::MAX, f32::min);
        let deep = polygon_contains(&poly, p) && edge > self.binding_reach();
        let mut binding = binding_for(target, world, self.mods.alt || deep);
        if self.prefs.midpoint_snapping && !binding.inside {
            // Encaixar em pontos intermediários: lock onto the middle of the nearest side
            let mids = [[0.5, 0.0], [1.0, 0.5], [0.5, 1.0], [0.0, 0.5]];
            if let Some(mid) = mids.into_iter().find(|m| {
                let probe = Binding { fixed: *m, ..binding.clone() };
                focus_point(target, &probe).distance(world) <= self.binding_reach()
            }) {
                binding.fixed = mid;
            }
        }
        Some(binding)
    }

    /// Binds a freshly drawn arrow to the shapes under its first and last point.
    pub(crate) fn bind_new_arrow(&self, mut arrow: Element) -> Element {
        let ElementKind::Line { points, arrow: true } = &arrow.kind else {
            return arrow;
        };
        let (first, last) = (points[0], points[points.len() - 1]);
        arrow.ends.start = self.bind_at(Pos2::new(first[0], first[1]), None);
        arrow.ends.end = self.bind_at(Pos2::new(last[0], last[1]), None);
        if let (Some(a), Some(b)) = (&mut arrow.ends.start, &mut arrow.ends.end) {
            if a.element_id == b.element_id {
                // both ends in one shape: the reference app keeps them where they were drawn
                (a.inside, b.inside) = (true, true);
            }
        }
        resolve_arrow(&self.scene.elements, &mut arrow);
        arrow
    }

    pub(crate) fn set_head(&mut self, end: bool, head: Option<Head>) {
        let arrows: Vec<usize> = self
            .selected
            .iter()
            .copied()
            .filter(|i| matches!(self.scene.elements.get(*i), Some(Element { kind: ElementKind::Line { arrow: true, .. }, .. })))
            .collect();
        if !arrows.is_empty() {
            self.snapshot();
        }
        for i in arrows {
            let ends = &mut self.scene.elements[i].ends;
            *(if end { &mut ends.end_head } else { &mut ends.start_head }) = head;
        }
        *(if end { &mut self.heads.1 } else { &mut self.heads.0 }) = head;
    }

    /// Moving/rotating/resizing an arrow without its target breaks that binding.
    pub(crate) fn detach_selected_arrows(&mut self) {
        let selected_ids: Vec<String> =
            self.selected.iter().filter_map(|i| self.scene.elements.get(*i)).map(|e| e.id.clone()).collect();
        for i in self.selected.clone() {
            if let Some(e) = self.scene.elements.get_mut(i) {
                for end in [&mut e.ends.start, &mut e.ends.end] {
                    if end.as_ref().is_some_and(|b| !selected_ids.contains(&b.element_id)) {
                        *end = None;
                    }
                }
            }
        }
    }

    /// Keeps every bound arrow glued to its shapes after anything moved.
    pub(crate) fn update_bindings(&mut self) {
        resolve_bindings(&mut self.scene.elements);
        recenter_labels(&mut self.scene.elements);
    }

    /// Shape that would be bound right now: highlighted like the reference app's suggestedBinding.
    pub(crate) fn suggested_binding(&self) -> Option<usize> {
        let pointer = self.to_world(self.last_pointer?);
        let dragged_end = match (self.point_drag, self.single_line()) {
            (Some(k), Some(i)) if matches!(self.scene.elements[i].kind, ElementKind::Line { arrow: true, .. })
                && (k == 0 || k + 1 == self.line_points(i).len()) => Some(i),
            _ => None,
        };
        let drawing = self.tool == Tool::Arrow && self.drawing_start.is_some();
        if let Some(i) = self.multi.or(dragged_end) {
            return self.binding_target(pointer, Some(i));
        }
        if drawing && self.canvas_rect.contains(self.last_pointer?) {
            return self.binding_target(pointer, None);
        }
        None
    }

    pub(crate) fn update_gesture(&mut self, pos: Pos2) {
        let previous = self.last_pointer.unwrap_or(pos);
        self.last_pointer = Some(pos);
        let world = self.to_world(pos);
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
                element.rotate_around(rotate.pivot, delta);
                self.scene.elements[*i] = element;
            }
        } else if let Some(resize) = &self.resizing {
            let local = resize.center + rot(world - resize.center, -resize.angle);
            let corner = local + resize.grab;
            let mut to = Rect::from_two_pos(resize.anchor, corner);
            if self.mods.shift && resize.from.height() > 0.0 {
                // keep aspect ratio
                let ratio = resize.from.width() / resize.from.height();
                let w = to.width().max(to.height() * ratio);
                let size = Vec2::new(w, w / ratio);
                let dir = Vec2::new((corner.x - resize.anchor.x).signum(), (corner.y - resize.anchor.y).signum());
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
            if pos != previous && !self.move_snapshotted {
                self.snapshot();
                self.detach_selected_arrows();
                self.move_snapshotted = true;
                let moving = self.with_frame_contents(&self.selected);
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
                .filter(|(_, e)| !e.locked)
                .filter_map(|(i, e)| {
                    let b = element_bounds(e);
                    (if self.prefs.box_overlap { area.intersects(b) } else { area.contains_rect(b) }).then_some(i)
                })
                .collect();
            let groups: Vec<String> =
                selected.iter().filter_map(|i| self.scene.elements[*i].group_id.clone()).collect();
            for (i, e) in self.scene.elements.iter().enumerate() {
                if e.group_id.as_ref().is_some_and(|g| groups.contains(g)) && !selected.contains(&i) {
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
        if let (Some(start), Tool::Ide) = (self.drawing_start, self.tool) {
            self.drawing_start = None;
            self.open_ide_dialog(start, end);
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
                if !self.tool_locked && self.tool != Tool::Draw {
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
        self.move_origin = None;
        self.snap_guides.clear();
    }
}
