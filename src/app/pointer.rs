//! Pointer gestures on the canvas: pressing, drawing, moving, releasing (arrow binding: arrows.rs).

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
        // the point comes with the drag: a click on the handle, or a double-click that labels an arrow, adds none
        if let (Some(_), Some((k, _))) =
            (self.editable_line(), self.midpoint_handles().into_iter().find(|(_, p)| p.distance(pos) <= 8.0))
        {
            self.midpoint_press = Some((k + 1, pos));
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
            // a terminal or IDE takes a press on its body (focus, or enlarge); only its title strip drags
            let e = &self.scene.elements[i];
            if !modifiers.shift && world.y - local_bounds(e).min.y >= TERMINAL_HEADER {
                if e.terminal.is_some() {
                    self.focus_terminal(i);
                    return;
                }
                if e.ide.is_some() {
                    self.enlarge_ide(i);
                    return;
                }
            }
            let members: Vec<usize> = self.group_members(i).into_iter().filter(|j| !self.is_card_text(*j)).collect();
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

    /// The element a rope from `from` reaches when it lands at `world`: a note's label counts as the
    /// note; the source itself and ropes reach nothing.
    pub(crate) fn connect_target(&self, from: &str, world: Pos2) -> Option<usize> {
        let hit = self.hit(world)?;
        let id = self.scene.elements[hit].id.trim_end_matches("-label");
        if id == from {
            return None;
        }
        self.scene.elements.iter().position(|e| e.id == id && !e.rope)
    }

    /// The node under the pointer while "Conectar" waits: highlighted, and where a click connects.
    pub(crate) fn connect_hover(&self) -> Option<usize> {
        let from = self.connecting.as_deref()?;
        self.connect_target(from, self.to_world(self.last_pointer?))
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
                    self.context = None;
                    // any press on the canvas dismisses the unlock button (its own clicks never get here)
                    self.active_locked = None;
                    self.mods = modifiers;
                    self.last_pointer = Some(pos);
                    // "Conectar" takes the press before anything else: a node gets the rope, anywhere else cancels
                    if let Some(from) = self.connecting.take() {
                        if let Some(i) = self.connect_target(&from, self.to_world(pos)) {
                            let to = self.scene.elements[i].id.clone();
                            self.connect(&from, &to);
                        }
                        continue;
                    }
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
                    if button == egui::PointerButton::Middle || space || self.tool == Tool::Hand || self.prefs.view_mode {
                        self.panning = true;
                        continue;
                    }
                    match self.tool {
                        Tool::Select => self.press_select(pos, world, modifiers),
                        // the text tool labels a shape under it, leaves a window to the click, and writes on empty canvas
                        Tool::Text => match self.hit(world).or_else(|| self.containing_shape(world)) {
                            Some(i) if takes_label(&self.scene.elements[i]) => {
                                let label = self.owner_label(i);
                                self.begin_text_edit(label);
                            }
                            _ if self.window_at(world) => {
                                if !self.tool_locked {
                                    self.select_tool(Tool::Select);
                                }
                                self.press_select(pos, world, modifiers);
                            }
                            _ => self.start_text(world),
                        },
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
                // right click: the menu of what is under the pointer (ui/context.rs)
                egui::Event::PointerButton { pos, button: egui::PointerButton::Secondary, pressed: true, .. } => {
                    if !rect.contains(pos) || ctx.layer_id_at(pos) != Some(response.layer_id) {
                        continue;
                    }
                    self.menu_open = false;
                    self.last_pointer = Some(pos);
                    if self.editing.is_some() {
                        self.finish_text();
                    }
                    self.open_context(pos, self.to_world(pos));
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
                // a transparent shape is hit only on its outline, but a double-click inside it labels it
                let hit = self.hit(world).or_else(|| self.containing_shape(world));
                match hit {
                    // lines: double-click; arrows: Cmd + double-click (plain double-click labels arrows in the reference app)
                    Some(i) if matches!(self.scene.elements[i].kind, ElementKind::Line { arrow, .. } if !arrow || self.mods.command) => {
                        self.moving = false;
                        self.point_drag = None;
                        self.selected = vec![i];
                        self.line_editor = Some(i);
                        self.selected_point = None;
                    }
                    Some(i) if self.scene.elements[i].terminal.is_some() => self.double_click_terminal(i, world),
                    Some(i) if self.scene.elements[i].ide.is_some() => self.enlarge_ide(i),
                    Some(i) if matches!(self.scene.elements[i].kind, ElementKind::Text { .. }) || self.scene.elements[i].created.is_some() => {
                        self.moving = false;
                        self.begin_text_edit(i);
                    }
                    Some(i) if takes_label(&self.scene.elements[i]) => {
                        self.moving = false;
                        let label = self.owner_label(i);
                        self.begin_text_edit(label);
                    }
                    None if !self.window_at(world) => {
                        self.marquee = None;
                        self.start_text(world);
                    }
                    _ => {}
                }
            }
        }
        self.update_bindings();
    }

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
        if self.tool == Tool::Select && self.press_world.distance(end) * self.zoom < 3.0 && self.hit(self.press_world).is_some_and(|i| self.scene.elements[i].rope) {
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
            let groups: Vec<String> =
                selected.iter().filter_map(|i| self.scene.elements[*i].group_id.clone()).collect();
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
        // a click on a note (no drag) takes its focus: the editor types into it from here on
        if self.moving && !self.move_snapshotted && self.tool == Tool::Select {
            if let [card] = self.selected[..] {
                let note = self.scene.elements.get(card).filter(|e| e.created.is_some() && !e.note.content_locked);
                if let (Some(_), Some(label)) = (note, note_label(&self.scene.elements, card)) {
                    self.editing = Some(label);
                    self.edit_focused = false;
                }
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
        self.midpoint_press = None;
        self.move_origin = None;
        self.snap_guides.clear();
    }
}
