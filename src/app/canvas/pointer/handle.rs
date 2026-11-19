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

        self.mods = ctx.input(|input| input.modifiers);
        self.now = ctx.input(|input| input.time);
        self.step_glide();
        // images dragged in land where they're dropped; with the chat open, in its box
        let dropped: Vec<PathBuf> = ctx.input(|input| input.raw.dropped_files.iter().map(|file| file.path().to_path_buf()).collect());

        if self.chat_open || self.mini_takes_files(&ctx) {
            // an image pasted (Ctrl+V with no text on the clipboard) goes in as a png in the temp folder
            let pasted: Vec<_> = ctx.input(|input| {
                input.events.iter().filter_map(|event| if let egui::Event::PasteImage(image) = event { Some(image.clone()) } else { None }).collect()
            });

            let pasted = pasted.iter().filter_map(|image| save_pasted(image, &std::env::temp_dir())).collect();

            // the open chat hides the drawing: what's dropped goes into its box (pictures as
            // previews), as paths Claude Code reads
            self.chat_add_files(&ctx, dropped, false);
            self.chat_add_files(&ctx, pasted, true);
        } else {
            if !dropped.is_empty() {
                let at = ctx.input(|input| input.pointer.hover_pos()).unwrap_or(self.canvas_rect.center());

                if let Err(error) = self.drop_on_canvas(&dropped, self.to_world(at)) {
                    self.toast(error);
                }
            }
        }

        let space = self.space_pan(&ctx);
        let events = ctx.input(|input| input.events.clone());

        for event in events {
            match event {
                egui::Event::PointerButton { pos, button: button @ (egui::PointerButton::Primary | egui::PointerButton::Middle), pressed: true, modifiers } => {
                    // the open chat hides the drawing: a press in the strip above it would act unseen
                    // while "Conectar" waits, a click on a window drawn over the canvas (a file tree, a chat card) is the canvas's
                    let to_window = self.connecting.is_some() && ctx.layer_id_at(pos).is_none_or(|layer| layer.order <= egui::Order::Middle);

                    if self.chat_open || !rect.contains(pos) || !(to_window || self.on_canvas(&ctx, pos, response.layer_id)) {
                        continue;
                    }

                    // the press that brought the window back from another program, wherever the pointer was: the note
                    // being typed (or the window in focus) stays as the person left it
                    if self.coming_back() && (self.editing.is_some() || self.terminal_elevated.is_some()) {
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

                        if let Some(index) = self.connect_target(&from, world) {
                            let to = self.scene.elements[index].id.clone();

                            self.connect(&from, &to);
                        } else if self.scene.elements.iter().any(|element| {
                            element.locked && element.id != from && (element.is_node() || element.created.is_some()) && element_bounds(element).contains(world)
                        }) {
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

                    if let Some(index) = self.multi {
                        // click on the last placed point finishes, any other click adds a point
                        let points = self.line_points(index);
                        let last = points.len().checked_sub(2).map(|point_index| points[point_index]);

                        if last.is_some_and(|point| self.to_screen(Pos2::new(point[0], point[1])).distance(pos) <= 10.0) {
                            self.finish_multi();
                        } else if let Some(points) = self.line_points_mut(index) {
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

                    // an enlarged window: a press on the canvas around it puts it back, whatever the drawing tool (the hand and the middle button pan) (press_select)
                    if let Some(index) = self.elevated_index().filter(|_| self.dock.is_none())
                        && !self.terminal_frame(&self.scene.elements[index]).0.contains(pos)
                    {
                        self.press_select(pos, world, modifiers);

                        continue;
                    }

                    match self.tool {
                        Tool::Select => self.press_select(pos, world, modifiers),
                        // the text tool labels a shape under it, leaves a window to the click, and writes on empty canvas
                        Tool::Text => match self.hit(world).or_else(|| self.containing_shape(world)) {
                            Some(index) if takes_label(&self.scene.elements[index]) => {
                                let label = self.owner_label(index);

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
                        // a window is made on the empty canvas: a press on one already there picks it, as Seleção would
                        Tool::Terminal | Tool::Portal | Tool::FileTree | Tool::Chat | Tool::Sticky if self.window_at(world) => {
                            self.select_tool(Tool::Select);
                            self.press_select(pos, world, modifiers);
                        }
                        _ => {
                            let start = if matches!(self.tool, Tool::Draw | Tool::AutoShape) { world } else { self.snap_point(world) };

                            self.drawing_start = Some(start);
                            self.drawing_points = vec![[world.x, world.y]];
                        }
                    }
                }
                // right click: the menu of what is under the pointer (ui/context.rs)
                egui::Event::PointerButton { pos, button: egui::PointerButton::Secondary, pressed: true, .. } => {
                    if self.chat_open || !rect.contains(pos) || !self.on_canvas(&ctx, pos, response.layer_id) {
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
        if self.in_gesture() && !ctx.input(|input| input.pointer.any_down()) {
            self.release(self.last_pointer.unwrap_or(rect.center()));
        }

        let double = ctx.input(|input| input.pointer.button_double_clicked(egui::PointerButton::Primary));
        // a lifted note or portal goes back on a double-click on its title, even while its text is typed
        let mut back = false;

        if let (true, Some(pos), Some(index)) = (double, ctx.input(|input| input.pointer.interact_pos()), self.elevated_index()) {
            let element = &self.scene.elements[index];

            let head = if element.portal.is_some() {
                PORTAL_HEADER
            } else if element.created.is_some() {
                NOTE_HEADER
            } else if element.filetree.is_some() {
                FT_BAR
            } else {
                0.0
            };

            let (frame, zoom) = self.terminal_frame(element);

            if head > 0.0 && frame.contains(pos) && pos.y - frame.top() < head * zoom {
                self.set_elevated(None);
                back = true;
            }
        }

        // a double-click on a group's pill opens its name
        if let (true, Some(pos)) = (double && !back, ctx.input(|input| input.pointer.interact_pos()))
            && let Some(gid) = self.group_pill_at(pos).filter(|_| rect.contains(pos))
        {
            self.group_menu_action(&gid, "rename");
            back = true;
        }

        // a note's title lifts it, though the double-click's first click already put its text in focus
        if let (true, Tool::Select, Some(pos)) = (double && !back, self.tool, ctx.input(|input| input.pointer.interact_pos()))
            && rect.contains(pos)
            && self.on_canvas(&ctx, pos, response.layer_id)
        {
            let world = self.to_world(pos);

            if let Some(index) = self.hit(world).filter(|&index| self.scene.elements[index].created.is_some() && self.on_title(index, world)) {
                self.toggle_lift(index);
                back = true;
            }
        }

        if let (true, Tool::Select, None, Some(pos)) = (double && !back, self.tool, self.editing, ctx.input(|input| input.pointer.interact_pos()))
            && rect.contains(pos)
            && self.on_canvas(&ctx, pos, response.layer_id)
        {
            let world = self.to_world(pos);
            // a transparent shape is hit only on its outline, but a double-click inside it labels it
            let hit = self.hit(world).or_else(|| self.containing_shape(world));

            match hit {
                // lines: double-click; arrows: Cmd + double-click (plain double-click labels arrows in the reference app)
                Some(index)
                    if !self.scene.elements[index].rope
                        && matches!(self.scene.elements[index].kind, ElementKind::Line { arrow, .. } if !arrow || self.mods.command) =>
                {
                    self.moving = false;
                    self.point_drag = None;
                    self.selected = vec![index];
                    self.line_editor = Some(index);
                    self.selected_point = None;
                }
                Some(index) if self.scene.elements[index].terminal.is_some() => self.double_click_terminal(index, world),
                Some(index) if self.scene.elements[index].chat.is_some() => self.open_chat_node(index),
                Some(index) if self.scene.elements[index].is_liftable() && self.on_title(index, world) => self.toggle_lift(index),
                Some(index) if matches!(self.scene.elements[index].kind, ElementKind::Text { .. }) || self.scene.elements[index].created.is_some() => {
                    self.moving = false;
                    self.begin_text_edit(index);
                }
                Some(index) if takes_label(&self.scene.elements[index]) => {
                    self.moving = false;
                    let label = self.owner_label(index);

                    self.begin_text_edit(label);
                }
                None if !self.window_at(world) => {
                    self.marquee = None;
                    self.start_text(world);
                }
                _ => {}
            }
        }

        self.update_bindings();
    }
}
