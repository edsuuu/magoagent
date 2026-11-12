use super::*;

impl DrawingApp {
    /// 0 → 1 over the enlarge animation that started at `since`, eased.
    pub(super) fn elevate_progress(&self, since: f64) -> f32 {
        let t = ((self.now - since) / 0.2).clamp(0.0, 1.0) as f32;
        t * t * (3.0 - 2.0 * t)
    }

    pub(crate) fn elevate_animating(&self) -> bool {
        let running = |since: f64| self.now - since < 0.2;
        (self.terminal_elevated.is_some() && running(self.elevate_since)) || self.leaving.as_ref().is_some_and(|(_, since)| running(*since))
    }

    /// Where an enlarged terminal goes: the canvas under its bar, leaving a strip of canvas to
    /// click back to it (the corner buttons hide while it has the focus).
    pub(crate) fn elevated_rect(&self) -> Rect {
        let rect = match self.dock {
            Some((right, width)) => self.dock_rect(right, width),
            None => self.centered_rect(),
        };
        // its title being dragged: the panel follows the pointer
        match self.dock_drag {
            Some((grab, at)) => Rect::from_min_size(at - grab, rect.size()),
            None => rect,
        }
    }

    pub(super) fn centered_rect(&self) -> Rect {
        let margin = 16.0;
        let top = (self.toolbar_bottom - self.canvas_rect.top() + 12.0).max(margin);
        // an open prompt composer goes under it
        let composing = self.composer.as_ref().is_some_and(|(id, _)| self.terminal_elevated.as_ref() == Some(id));
        let bottom = if composing { 72.0 } else { margin };
        Rect::from_min_max(self.canvas_rect.min + Vec2::new(margin, top), self.canvas_rect.max - Vec2::new(margin, bottom))
    }

    /// The enlarged terminal's "back to the canvas" button, in its title strip.
    pub(crate) fn restore_button(rect: Rect) -> Rect {
        Rect::from_center_size(Pos2::new(rect.right() - 18.0, rect.top() + TERMINAL_HEADER / 2.0), Vec2::splat(22.0))
    }

    /// The terminal's box on screen and its scale: on the canvas, or enlarged at 100%, and in
    /// between while it grows or shrinks back.
    pub(crate) fn terminal_frame(&self, element: &Element) -> (Rect, f32) {
        let b = local_bounds(element);
        let on_canvas = Rect::from_min_max(self.to_screen(b.min), self.to_screen(b.max));
        // a note lifted fits its box in the enlarged frame, keeping its proportions; no animation
        if element.created.is_some() {
            if self.terminal_elevated.as_ref() == Some(&element.id) {
                let target = self.elevated_rect();
                let zoom = (target.width() / b.width()).min(target.height() / b.height());
                return (Rect::from_center_size(target.center(), b.size() * zoom), zoom);
            }
            return (on_canvas, self.zoom);
        }
        let t = if self.terminal_elevated.as_ref() == Some(&element.id) {
            self.elevate_progress(self.elevate_since)
        } else {
            match &self.leaving {
                Some((id, since)) if *id == element.id => 1.0 - self.elevate_progress(*since),
                _ => 0.0,
            }
        };
        if t <= 0.0 {
            return (on_canvas, self.zoom);
        }
        let target = self.elevated_rect();
        (Rect::from_min_max(on_canvas.min.lerp(target.min, t), on_canvas.max.lerp(target.max, t)), self.zoom + (1.0 - self.zoom) * t)
    }

    /// The focused or enlarged terminal's own layer above the canvas. It holds egui's focus
    /// while typing, so the app's shortcuts stay off and Tab, arrows and Esc reach the shell,
    /// and takes the wheel for the scrollback. The canvas title strip stays draggable.
    pub(crate) fn terminal_overlay(&mut self, ctx: &Context) {
        let find = |id: &Option<String>| id.as_ref().and_then(|id| self.scene.elements.iter().find(|e| &e.id == id && e.terminal.is_some())).cloned();
        if self.elevate_animating() {
            ctx.request_repaint();
        }
        let (area, element) = match (find(&self.terminal_elevated), find(&self.terminal_focus)) {
            (Some(e), _) => (self.terminal_frame(&e).0, e),
            (None, Some(e)) => {
                let (frame, zoom) = self.terminal_frame(&e);
                let grid = Rect::from_min_max(frame.min + Vec2::new(0.0, TERMINAL_HEADER * zoom), frame.max - Vec2::new(0.0, TERMINAL_FOOTER * zoom));
                // its side bands stay canvas, so the edges resize it from the grip band
                (grid.shrink2(Vec2::new(4.0, 0.0)).intersect(self.canvas_rect), e)
            }
            (None, None) => return,
        };
        let enlarged = self.terminal_elevated.as_ref() == Some(&element.id);
        // a press elsewhere on the canvas ends typing (the floating UI handles its own clicks;
        // one that opens a text field takes the keyboard). Its title strip is still the node:
        // dragging it keeps the focus.
        let frame = self.terminal_frame(&element).0;
        // not a pan: the middle button or Space drag the view, not away from it (a right click still
        // ends it: its menu's keys are the canvas's)
        let press =
            ctx.input(|i| (i.pointer.primary_pressed() || i.pointer.secondary_pressed()).then(|| i.pointer.interact_pos()).flatten()).filter(|_| !self.panning);
        let on_canvas = press.is_some_and(|p| !frame.contains(p) && ctx.layer_id_at(p).is_none_or(|l| l.order == egui::Order::Background));
        if on_canvas && enlarged && self.dock.is_none() {
            // a click on the margin goes back to the canvas
            self.set_elevated(None);
            self.terminal_focus = None;
            return;
        }
        if on_canvas || self.palette_open || self.dialog_open() || self.composer.is_some() {
            self.terminal_focus = None;
            if !enlarged {
                return;
            }
        }
        if !area.is_positive() {
            self.terminal_focus = None;
            return;
        }
        // the search bar's field has the keys while it is open, and a blurred terminal has none
        let typing =
            self.terminal_focus.as_ref() == Some(&element.id) && self.term_ui.find.is_none() && !element.terminal.as_ref().is_some_and(|t| t.look.blurred);
        let (mut wheel, mut header_double, mut clicked, mut secondary) = (0.0, false, false, false);
        let mut gesture = crate::app::terminal::menu::PointerGesture::default();
        egui::Area::new(egui::Id::unique("terminal_overlay")).fixed_pos(area.min).order(egui::Order::Middle).show(ctx, |ui| {
            let (rect, response) = ui.allocate_exact_size(area.size(), Sense::click_and_drag());
            if enlarged {
                self.paint_terminal(ui.painter(), &element, rect, self.terminal_frame(&element).1);
                let header = Rect::from_min_size(rect.min, Vec2::new(rect.width(), TERMINAL_HEADER));
                let at = response.interact_pointer_pos();
                header_double = response.double_clicked() && at.is_some_and(|p| header.contains(p))
                    || response.clicked() && at.is_some_and(|p| Self::restore_button(rect).contains(p));
            }
            if typing {
                response.request_focus();
                let all = egui::EventFilter { tab: true, horizontal_arrows: true, vertical_arrows: true, escape: true };
                ui.memory_mut(|m| m.set_focus_lock_filter(response.id, all));
            }
            if response.hovered() {
                wheel = ui.input(|i| i.smooth_scroll_delta.y);
            }
            clicked = response.clicked();
            secondary = response.secondary_clicked();
            gesture = crate::app::terminal::menu::PointerGesture {
                pos: response.interact_pointer_pos().or(response.hover_pos()),
                clicked,
                double: response.double_clicked(),
                triple: response.triple_clicked(),
                drag_started: response.drag_started(),
                dragging: response.dragged(),
                drag_stopped: response.drag_stopped(),
            };
            response.on_hover_cursor(CursorIcon::Text);
        });
        if header_double {
            self.set_elevated(None);
            return;
        }
        // a right click on the window is its menu (the canvas never sees it)
        if secondary && let Some(at) = ctx.input(|i| i.pointer.interact_pos()) {
            if let Some(index) = self.scene.elements.iter().position(|e| e.id == element.id)
                && !self.selected.contains(&index)
            {
                self.selected = vec![index];
            }
            self.context = Some(ContextMenu::new(at, Target::Element(element.id.clone()), None));
        }
        if clicked && enlarged {
            self.terminal_focus = Some(element.id.clone());
        }
        // the grid's own presses and drags select, copy on a double click, clear on a click
        let (frame, card_zoom) = self.terminal_frame(&element);
        let (_, font) = self.terminal_look(&element.terminal.clone().unwrap_or_default());
        let cell_size = cell(font) * card_zoom;
        self.terminal_pointer(ctx, &element.id, frame.min + Vec2::new(TERMINAL_PAD, TERMINAL_HEADER + 4.0) * card_zoom, cell_size, &gesture);
        let Some(Ok(session)) = self.terminals.get(&element.id) else {
            return;
        };
        let (mut find, mut copied) = (None, None);
        if typing {
            let mode = session.handle.mode();
            let mut bytes = Vec::new();
            for event in ctx.input(|i| i.events.clone()) {
                match event {
                    egui::Event::Text(text) | egui::Event::Ime(egui::ImeEvent::Commit(text)) => bytes.extend(text.into_bytes()),
                    // ⌘K clears the buffer, as in the reference app (Terminal.app does the same); off the Mac
                    // Ctrl+K is the shell's (kill to the end of the line), as in Windows Terminal and PhpStorm
                    egui::Event::Key { key: Key::K, pressed: true, modifiers, .. } if modifiers.mac_cmd => session.handle.clear(),
                    // Buscar no Terminal: ⌘F, Ctrl+Shift+F elsewhere, with the selected word in it
                    egui::Event::Key { key: Key::F, pressed: true, modifiers, .. }
                        if if cfg!(target_os = "macos") { modifiers.mac_cmd && !modifiers.shift } else { modifiers.ctrl && modifiers.shift } =>
                    {
                        find = Some(session.handle.selected_text());
                    }
                    // Ctrl+Shift+C copies what is selected, as the terminals of Windows and Linux do
                    egui::Event::Key { key: Key::C, pressed: true, modifiers, .. } if !cfg!(target_os = "macos") && modifiers.ctrl && modifiers.shift => {
                        copied = Some(session.handle.selected_text());
                    }
                    egui::Event::Key { key, pressed: true, modifiers, .. } => {
                        bytes.extend(key_bytes(key, modifiers, mode.contains(TermMode::APP_CURSOR)).unwrap_or_default());
                    }
                    // text, or a picture as the path of a png (self.pasted_bytes)
                    egui::Event::Paste(_) | egui::Event::PasteImage(_) => bytes.extend(self.pasted_bytes(&element.id, &event).unwrap_or_default()),
                    // ⌘C / Ctrl+C copy what is selected; elsewhere, with nothing selected, Ctrl+C / Ctrl+X are the
                    // keys. Windows also turns Ctrl+Insert / Shift+Delete into them: those copy and cut nothing here
                    egui::Event::Copy if session.handle.has_selection() => copied = Some(session.handle.selected_text()),
                    egui::Event::Copy if !cfg!(target_os = "macos") && !edit_key_held() => bytes.push(0x03),
                    egui::Event::Cut if !cfg!(target_os = "macos") && !edit_key_held() => bytes.push(0x18),
                    _ => {}
                }
            }
            if !bytes.is_empty() {
                // typing over a selection ends it
                session.handle.clear_selection();
                session.handle.write(bytes);
            }
        }
        if let Some(text) = copied.filter(|t| !t.is_empty()) {
            ctx.copy_text(text);
        }
        self.terminal_scroll += wheel / cell_size.y;
        let lines = self.terminal_scroll.trunc();
        if lines != 0.0 {
            session.scroll(lines as i32);
            self.terminal_scroll -= lines;
        }
        if let Some(seed) = find {
            self.open_find(&element.id, Some(seed));
        }
    }
}
