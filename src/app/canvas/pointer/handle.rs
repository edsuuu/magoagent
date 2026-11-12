use super::*;

impl DrawingApp {
    pub(crate) fn handle_canvas(&mut self, response: &egui::Response, rect: Rect) {
        self.canvas_rect = rect;
        if self.design.is_some() && response.is_pointer_button_down_on() {
            self.design_exit();
        }
        let ctx = response.ctx.clone();
        if ctx.current_pass_index() > 0 {
            return;
        }
        self.mods = ctx.input(|i| i.modifiers);
        self.now = ctx.input(|i| i.time);
        self.step_glide();
        // images dragged in land where they're dropped; with the chat open, in its box
        let dropped: Vec<PathBuf> = ctx.input(|i| i.raw.dropped_files.iter().map(|f| f.path().to_path_buf()).collect());
        if self.chat_open || self.mini_takes_files(&ctx) {
            // an image pasted (Ctrl+V with no text on the clipboard) goes in as a png in the temp folder
            let pasted: Vec<_> =
                ctx.input(|i| i.events.iter().filter_map(|e| if let egui::Event::PasteImage(img) = e { Some(img.clone()) } else { None }).collect());
            let pasted = pasted.iter().filter_map(|img| save_pasted(img, &std::env::temp_dir())).collect();
            // the open chat hides the drawing: what's dropped goes into its box (pictures as
            // previews), as paths Claude Code reads
            self.chat_add_files(&ctx, dropped, false);
            self.chat_add_files(&ctx, pasted, true);
        } else {
            if !dropped.is_empty() {
                let at = ctx.input(|i| i.pointer.hover_pos()).unwrap_or(self.canvas_rect.center());
                if let Err(error) = self.drop_on_canvas(&dropped, self.to_world(at)) {
                    self.toast(error);
                }
            }
        }
        let space = self.space_pan(&ctx);
        let events = ctx.input(|i| i.events.clone());
        for event in events {
            match event {
                egui::Event::PointerButton { pos, button: button @ (egui::PointerButton::Primary | egui::PointerButton::Middle), pressed: true, modifiers } => {
                    // the open chat hides the drawing: a press in the strip above it would act unseen
                    if self.chat_open || !rect.contains(pos) || ctx.layer_id_at(pos) != Some(response.layer_id) {
                        continue;
                    }
                    self.context = None;
                    // any press on the canvas dismisses the unlock button (its own clicks never get here)
                    self.active_locked = None;
                    self.note_press = None;
                    self.mods = modifiers;
                    self.last_pointer = Some(pos);
                    // "Conectar" takes the click before anything else (the middle button still pans): a
                    // node gets the rope, a locked one says why it doesn't, anywhere else cancels
                    if button == egui::PointerButton::Primary
                        && let Some(from) = self.connecting.take()
                    {
                        let world = self.to_world(pos);
                        if let Some(i) = self.connect_target(&from, world) {
                            let to = self.scene.elements[i].id.clone();
                            self.connect(&from, &to);
                        } else if self
                            .scene
                            .elements
                            .iter()
                            .any(|e| e.locked && e.id != from && (e.is_node() || e.created.is_some()) && element_bounds(e).contains(world))
                        {
                            self.toast("Nó bloqueado: desbloqueie para conectar");
                            self.connecting = Some(from);
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
                    if cfg!(target_os = "macos") && pos.y - rect.top() < TOOLBAR_BOTTOM - 4.0 && self.hit(world).is_none() && self.tool == Tool::Select {
                        ctx.send_viewport_cmd(egui::ViewportCommand::StartDrag);
                        continue;
                    }
                    if button == egui::PointerButton::Middle || space || self.tool == Tool::Hand {
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
                                self.select_tool(Tool::Select);
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
                    if self.chat_open || !rect.contains(pos) || ctx.layer_id_at(pos) != Some(response.layer_id) {
                        continue;
                    }
                    self.last_pointer = Some(pos);
                    if self.editing.is_some() {
                        self.finish_text();
                    }
                    self.open_context(pos, self.to_world(pos));
                }
                egui::Event::PointerMoved(pos) => self.update_gesture(pos),
                egui::Event::PointerButton { pos, button: egui::PointerButton::Primary | egui::PointerButton::Middle, pressed: false, .. } => {
                    self.update_gesture(pos);
                    self.release(pos);
                }
                _ => {}
            }
        }
        // a release the canvas never got (focus lost mid-drag, an overlay took it) still ends the drag
        if self.in_gesture() && !ctx.input(|i| i.pointer.any_down()) {
            self.release(self.last_pointer.unwrap_or(rect.center()));
        }
        let double = ctx.input(|i| i.pointer.button_double_clicked(egui::PointerButton::Primary));
        // a lifted note or portal goes back on a double-click on its title, even while its text is typed
        let mut back = false;
        if let (true, Some(pos), Some(i)) = (double, ctx.input(|i| i.pointer.interact_pos()), self.elevated_index()) {
            let e = &self.scene.elements[i];
            let head = if e.portal.is_some() {
                PORTAL_HEADER
            } else if e.created.is_some() {
                NOTE_HEADER
            } else if e.filetree.is_some() {
                FT_BAR
            } else {
                0.0
            };
            let (frame, zoom) = self.terminal_frame(e);
            if head > 0.0 && frame.contains(pos) && pos.y - frame.top() < head * zoom {
                self.set_elevated(None);
                back = true;
            }
        }
        // a double-click on a group's pill opens its name
        if let (true, Some(pos)) = (double && !back, ctx.input(|i| i.pointer.interact_pos())) {
            if let Some(gid) = self.group_pill_at(pos).filter(|_| rect.contains(pos)) {
                self.group_menu_action(&gid, "rename");
                back = true;
            }
        }
        // a note's title lifts it, though the double-click's first click already put its text in focus
        if let (true, Tool::Select, Some(pos)) = (double && !back, self.tool, ctx.input(|i| i.pointer.interact_pos())) {
            if rect.contains(pos) && ctx.layer_id_at(pos) == Some(response.layer_id) {
                let world = self.to_world(pos);
                if let Some(i) = self.hit(world).filter(|&i| self.scene.elements[i].created.is_some() && self.on_title(i, world)) {
                    self.toggle_lift(i);
                    back = true;
                }
            }
        }
        if let (true, Tool::Select, None, Some(pos)) = (double && !back, self.tool, self.editing, ctx.input(|i| i.pointer.interact_pos())) {
            if rect.contains(pos) && ctx.layer_id_at(pos) == Some(response.layer_id) {
                let world = self.to_world(pos);
                // a transparent shape is hit only on its outline, but a double-click inside it labels it
                let hit = self.hit(world).or_else(|| self.containing_shape(world));
                match hit {
                    // lines: double-click; arrows: Cmd + double-click (plain double-click labels arrows in the reference app)
                    Some(i)
                        if !self.scene.elements[i].rope
                            && matches!(self.scene.elements[i].kind, ElementKind::Line { arrow, .. } if !arrow || self.mods.command) =>
                    {
                        self.moving = false;
                        self.point_drag = None;
                        self.selected = vec![i];
                        self.line_editor = Some(i);
                        self.selected_point = None;
                    }
                    Some(i) if self.scene.elements[i].terminal.is_some() => self.double_click_terminal(i, world),
                    Some(i) if self.scene.elements[i].chat.is_some() => self.open_chat_node(i),
                    Some(i) if self.scene.elements[i].ide.is_some() && self.on_panel_text(i, world) => self.ide_panel_line(i, world),
                    Some(i) if self.scene.elements[i].is_liftable() && self.on_title(i, world) => self.toggle_lift(i),
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
}
