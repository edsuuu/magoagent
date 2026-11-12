use super::*;

impl eframe::App for DrawingApp {
    /// Closing mid-edit keeps the text being typed and anything not yet autosaved (that only runs
    /// while idle).
    fn on_exit(&mut self, _gl: Option<&eframe::glow::Context>) {
        self.save_on_exit();
    }

    /// eframe clears every viewport with this one color: the avatar's and the call popup's windows
    /// need a see-through one (else a square shows around their rounded shapes), and the app's own
    /// window paints its whole background anyway (canvas and chat).
    fn clear_color(&self, visuals: &egui::Visuals) -> [f32; 4] {
        if self.avatar.shown || self.voice.on { [0.0; 4] } else { visuals.window_fill().to_normalized_gamma_f32() }
    }

    /// Minimized (Windows) or covered (macOS), eframe stops calling `ui`, where the bridge and the
    /// agents' states run: it paints unseen every 400 ms.
    fn logic(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        const EVERY: std::time::Duration = std::time::Duration::from_millis(400);
        if ctx.input(|i| i.viewport().visible()) != Some(false) {
            self.hidden_paint = None;
            return;
        }
        // every request reaches here while hidden: only one in 400 ms paints. The input's time
        // stands still meanwhile, so the clock is the system's
        let since = self.hidden_paint.map_or(EVERY, |t| t.elapsed());
        if since >= EVERY {
            self.hidden_paint = Some(std::time::Instant::now());
            ctx.send_viewport_cmd(egui::ViewportCommand::RequestPaintWhileHidden);
            ctx.request_repaint_after(EVERY);
        } else {
            ctx.request_repaint_after(EVERY - since);
        }
    }

    fn raw_input_hook(&mut self, ctx: &egui::Context, raw: &mut egui::RawInput) {
        if let Some(script) = &mut self.script {
            script.feed(ctx, raw);
        }
        if TITLE_BAR > 0.0 {
            edge_resize(ctx, raw);
            self.caption_drag(ctx, raw);
        }
        // Tab on the bare canvas moves no focus: egui would hand the keys to the first button, and
        // the shortcuts and Space would stop until the next click
        if !self.keyboard_taken(ctx) {
            raw.events.retain(|e| !matches!(e, egui::Event::Key { key: Key::Tab, .. }));
        }
    }

    fn ui(&mut self, root: &mut egui::Ui, frame: &mut eframe::Frame) {
        let ctx = root.ctx().clone();
        // the frame's clock before anything is laid out by it: read only in `handle_canvas`, the canvas's
        // edge was a frame behind the sidebar sliding beside it, and the window's ground showed between them
        self.now = ctx.input(|i| i.time);
        if let Some(perf) = &mut self.perf {
            perf.frame(ctx.input(|i| i.time), frame.info().cpu_usage, &ctx.repaint_causes());
        }
        if let Some(theme) = ctx.system_theme() {
            self.system_dark = theme == egui::Theme::Dark;
        }
        configure_visuals(&ctx, self.dark());
        if self.prefs.ligatures != self.ligatures_shown {
            install_fonts(&ctx, self.prefs.ligatures);
            self.ligatures_shown = self.prefs.ligatures;
        }
        #[cfg(target_os = "macos")]
        crate::place_window_buttons(LIGHTS_Y as f64);
        self.screen_keys(&ctx);
        self.handle_shortcuts(&ctx);
        self.sync_filetrees(&ctx);
        self.sync_terminals(&ctx);
        self.memory.update(
            self.terminals
                .iter()
                .filter_map(|(id, session)| {
                    let session = session.as_ref().ok()?;
                    let name = self
                        .scene
                        .elements
                        .iter()
                        .find(|e| e.id == *id)
                        .and_then(|e| e.terminal.as_ref())
                        .or(self.away.get(id).map(|(_, spec)| spec))
                        .map(|t| t.name.clone())
                        .unwrap_or_else(|| id.clone());
                    Some((name, session.shell_pid))
                })
                .collect(),
        );
        self.voice_tick(&ctx);
        self.dictation_tick(&ctx);
        self.check_agents();
        self.tasks_tick();
        self.usage_tick();
        self.poll_chats();
        self.poll_cmds();
        self.poll_updates();
        self.chat_nodes_tick(&ctx);
        self.avatar_ui(&ctx);
        self.voice_popup(&ctx);
        self.chat_popup_ui(&ctx);
        self.mini_chat_ui(&ctx);
        self.maestro_note_ui(&ctx);
        self.maestro_tick(&ctx);
        self.actions_tick(&ctx);
        if self.swing_ropes(ctx.input(|i| i.stable_dt).min(0.05)) {
            ctx.request_repaint();
        }
        // every window clears see-through while a popup is up (clear_color): this one's ground stays solid
        root.painter().rect_filled(root.max_rect(), 0, ctx.global_style().visuals.window_fill());
        egui::CentralPanel::no_frame().show(root, |ui| {
            // not focusable: Tab would park the keyboard on the canvas and turn the shortcuts and Space off
            // the workspaces' sidebar keeps the left strip (its clicks and the pages stay off it)
            let rect = ui.available_rect_before_wrap();
            let rect = rect.with_min_x(rect.min.x + self.sidebar_width());
            let response = ui.allocate_rect(rect, Sense::CLICK | Sense::DRAG);
            let painter = ui.painter_at(rect);
            self.handle_canvas(&response, rect);
            // Ctrl + wheel over a window on the canvas (a file tree, an IDE, a terminal) zooms the
            // canvas too: the window's own wheel only scrolls
            let over_window = !response.hovered()
                && ctx.input(|i| i.modifiers.command && i.zoom_delta() != 1.0)
                && ctx.pointer_hover_pos().is_some_and(|p| rect.contains(p))
                && self.terminal_elevated.is_none()
                && !egui::Popup::is_any_open(&ctx)
                && self.focus(&ctx) != crate::app::canvas::shortcuts::Focus::Dialog;
            if (response.hovered() || over_window) && !self.chat_open && ctx.current_pass_index() == 0 {
                let (scroll, zoom, pointer) = ctx.input(|i| (i.smooth_scroll_delta, i.zoom_delta(), i.pointer.hover_pos()));
                let shift = ctx.input(|i| i.modifiers.shift);
                if zoom != 1.0 && ctx.input(|i| i.modifiers.command) {
                    // Ctrl + wheel: egui makes a notch a factor of e^0.2 (40 points at 1/200); the
                    // reference app adds a step per notch instead
                    self.wheel_step(zoom.ln() / 0.2, pointer.unwrap_or(rect.center()));
                } else if zoom != 1.0 {
                    // a pinch
                    self.wheel_zoom(zoom, pointer.unwrap_or(rect.center()));
                } else if scroll != Vec2::ZERO && self.prefs.mouse_wheel_zooms && !shift {
                    // Dispositivo de entrada = Mouse: the wheel zooms, a notch (40 points) a step
                    self.wheel_step(scroll.y / 40.0, pointer.unwrap_or(rect.center()));
                } else if scroll != Vec2::ZERO {
                    self.glide = None;
                    self.pan += scroll;
                }
            }
            self.update_cursor(&ctx, &response);
            self.paint_stage(&ctx, &painter, rect);
        });
        if !self.floors_ui.fx.engaged {
            self.text_editor(&ctx);
            self.note_buttons_ui(&ctx);
            self.stack_tabs_ui(&ctx);
        }
        self.floating_controls(&ctx);
        self.sidebar(&ctx);
        if TITLE_BAR > 0.0 {
            self.title_bar(&ctx);
        }
        self.note_color_ui(&ctx);
        self.links_popover_ui(&ctx);
        self.context_ui(&ctx);
        self.rename_ui(&ctx);
        let time = ctx.input(|i| i.time);
        if self.chat_was_open && !self.chat_open {
            self.chat_left_at = Some(time);
        }
        self.chat_was_open = self.chat_open;
        if let Some(left) = self.chat_left_at.filter(|_| !self.chat_open) {
            // ponytail: a plain fade of the ground, about as long as the chat's own fade in
            let k = ((time - left) / 0.2) as f32;
            if k >= 1.0 {
                self.chat_left_at = None;
            } else {
                let ground = crate::app::chat::list::Look::new(self.dark(), &self.colors()).bg;
                let veil = egui::LayerId::new(egui::Order::Foreground, egui::Id::new("chat_leave"));
                let ease = 1.0 - (1.0 - k) * (1.0 - k);
                ctx.layer_painter(veil).rect_filled(ctx.content_rect(), 0, ground.gamma_multiply(1.0 - ease));
                ctx.request_repaint();
            }
        }
        if self.chat_open {
            self.chat_screen(&ctx);
        } else if self.floors_ui.fx.engaged {
            // the floors' stage: the canvas is a card in 3D, what floats over its nodes waits
        } else {
            self.terminal_overlay(&ctx);
            self.terminal_extras(&ctx);
            self.filetree_overlay(&ctx);
            self.chat_node_overlay(&ctx);
            self.portal_chrome(&ctx);
            self.dock_grip(&ctx);
        }
        if TITLE_BAR > 0.0 {
            crate::app::ui::titlebar::edge_cursor(&ctx);
        }
        #[cfg(windows)]
        if !self.caption_hooked {
            self.caption_hooked = crate::app::ui::titlebar::native_caption::install(&*frame);
        }
        #[cfg(not(target_os = "linux"))]
        self.sync_portals(&*frame, &ctx);
        #[cfg(not(target_os = "linux"))]
        self.sync_voice_page(&*frame, &ctx);
        #[cfg(target_os = "macos")]
        if let Some(path) = self.script.as_mut().and_then(Script::take_pages) {
            self.save_page_shots(&path);
        }
        let _ = frame;
        let idle = !self.in_gesture() && self.multi.is_none() && self.editing.is_none();
        if self.prefs != self.saved_prefs {
            self.saved_prefs = self.prefs.clone();
            if let (Some(path), true) = (prefs_path(), self.autosave.is_some()) {
                let _ = serde_json::to_string_pretty(&self.prefs).map(|json| write_atomic(&path, json));
            }
        }
        let now = ctx.input(|i| i.time);
        self.laser.retain(|(_, t)| now - *t < 1.0);
        if let Some(id) = self.reveal.front() {
            match self.scene.elements.iter().find(|e| &e.id == id) {
                Some(e) if self.reveal_progress(e) < 1.0 => {}
                done => {
                    if let (Some(e), Some(agent)) = (done, &mut self.agent) {
                        (agent.1, agent.2) = (reveal_pen(e), now);
                    }
                    self.reveal.pop_front();
                    self.reveal_since = now;
                }
            }
            ctx.request_repaint();
        } else if self.agent.as_ref().is_some_and(|a| now - a.2 < AGENT_LINGER) {
            ctx.request_repaint();
        }
        if !self.laser.is_empty() {
            ctx.request_repaint();
        }
        if idle && now - self.last_poll > 0.4 {
            self.last_poll = now;
            self.sync_from_file();
        }
        // changes are written 0.4 s after the first one not yet on disk: a burst (arrow nudges,
        // typing) goes in a write every 0.4 s, not one per change, and a long one is never all lost
        if !self.dirty {
            self.dirty_since = now;
        }
        if self.dirty && idle && now - self.dirty_since >= 0.4 {
            self.write_autosave();
        }
        if self.glide.is_some() {
            ctx.request_repaint();
        }
        if std::mem::take(&mut self.attention) {
            ctx.send_viewport_cmd(egui::ViewportCommand::RequestUserAttention(egui::UserAttentionType::Informational));
        }
        ctx.request_repaint_after(std::time::Duration::from_millis(400));
    }
}
