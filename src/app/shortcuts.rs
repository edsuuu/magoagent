//! Keyboard shortcuts.

use crate::prelude::*;

impl DrawingApp {
    /// Whether a window or a field has the keys (a focused terminal, the IDE, the chat, a dialog,
    /// a text box): then the canvas's shortcuts stay off and the keys go to it.
    pub(crate) fn keyboard_taken(&self, ctx: &Context) -> bool {
        ctx.egui_wants_keyboard_input()
            || self.editing.is_some()
            || self.ide_focus.is_some()
            || self.card_focus.is_some()
            || self.terminal_focus.is_some()
            || self.composer.is_some()
            || self.portal_url_edit.is_some()
            || self.chat_open
            || self.agents_open
            || self.palette_open
            || self.export_open
            || self.prefs_open
            || self.locked_open
            || self.dialog_open()
            || self.ides.values().any(|ide| ide.file_ask.is_some())
    }

    /// Space held pans like the hand, unless the keys are someone's (a focused terminal types it).
    pub(crate) fn space_pan(&self, ctx: &Context) -> bool {
        !self.keyboard_taken(ctx) && ctx.input(|i| i.key_down(Key::Space))
    }

    pub(crate) fn handle_shortcuts(&mut self, ctx: &Context) {
        if self.design.is_some() && ctx.input(|i| i.key_pressed(Key::Escape)) {
            self.design_exit();
        }
        // ⌘, opens and closes Ajustes whoever has the keys (read from the event: scripted keys set the modifiers only there)
        let settings_key = ctx.input(|i| {
            i.events.iter().any(|e| matches!(e, egui::Event::Key { key: Key::Comma, pressed: true, modifiers, .. } if modifiers.command))
        });
        if ctx.current_pass_index() == 0 && self.card_focus.is_some() {
            self.ide_card_keys(ctx);
        }
        // mid-drag the keys wait (Delete or Ctrl+Z would pull the scene from under the drag); Esc cancels it
        if ctx.current_pass_index() == 0 && self.in_gesture() {
            if ctx.input(|i| i.key_pressed(Key::Escape)) {
                self.cancel_gesture();
            }
            return;
        }
        if ctx.current_pass_index() > 0 || (self.keyboard_taken(ctx) && !settings_key) {
            return;
        }
        let events = ctx.input(|i| i.events.clone());
        let hit = |key: Key, command: bool, shift: bool, alt: bool| {
            events.iter().any(|e| {
                matches!(e, egui::Event::Key { key: k, pressed: true, modifiers, .. }
                    if *k == key && modifiers.command == command && modifiers.shift == shift && modifiers.alt == alt)
            })
        };
        let plain = |key: Key| hit(key, false, false, false);
        let cmd = |key: Key| hit(key, true, false, false);
        let cmd_shift = |key: Key| hit(key, true, true, false);

        let editable = !self.prefs.view_mode;
        for event in events.iter().filter(|_| editable) {
            match event {
                egui::Event::Copy => self.copy(ctx),
                egui::Event::Cut => {
                    self.copy(ctx);
                    self.delete_selected();
                }
                egui::Event::Paste(text) => self.paste(text),
                _ => {}
            }
        }
        // preferences: Option+S snap, Cmd+' grid, Option+Z zen, Option+R view mode, Option+/ stats
        let alt = |key: Key| hit(key, false, false, true);
        if alt(Key::S) {
            self.prefs.snap_objects = !self.prefs.snap_objects;
        }
        if cmd(Key::Comma) {
            self.settings_open = !self.settings_open;
        }
        if cmd(Key::Quote) {
            self.prefs.grid = !self.prefs.grid;
        }
        if alt(Key::Z) {
            self.prefs.zen = !self.prefs.zen;
        }
        if alt(Key::R) {
            self.prefs.view_mode = !self.prefs.view_mode;
            self.selected.clear();
        }
        if alt(Key::Slash) {
            self.prefs.stats = !self.prefs.stats;
        }
        if self.prefs.view_mode {
            // view mode: look around only
            if cmd(Key::Equals) || cmd(Key::Plus) {
                self.zoom_step(0.1);
            }
            if cmd(Key::Minus) {
                self.zoom_step(-0.1);
            }
            if cmd(Key::Slash) {
                self.open_palette();
            }
            if plain(Key::Escape) {
                self.prefs.view_mode = false;
            }
            return;
        }
        // a dialog's Esc and Enter are its own, not deselect / type into the terminal (after the
        // mode toggles: view mode draws no card, so Esc must still leave it)
        if self.dialog_open() {
            return;
        }
        if let Some(end) = self.head_picker {
            for (key, pick) in [(Key::Q, None), (Key::W, Some(Head::Arrow)), (Key::E, Some(Head::Triangle)), (Key::R, Some(Head::TriangleOutline))] {
                if plain(key) {
                    self.set_head(end, pick);
                    return;
                }
            }
        }
        if self.tool == Tool::Fill && plain(Key::B) {
            // B again cycles the fill color (red, green, blue, yellow)
            let picks = &BACKGROUND_PICKS[1..];
            let at = picks.iter().position(|c| Some(*c) == self.style.fill).map_or(1, |i| (i + 1) % picks.len());
            self.style.fill = Some(picks[at]);
        } else {
            for tool in Tool::TOOLBAR.into_iter().chain(Tool::DRAWING).chain(Tool::EXTRA) {
                if tool.keys().iter().any(|k| plain(*k)) {
                    self.select_tool(tool);
                }
            }
        }
        if hit(Key::X, false, true, false) {
            self.select_tool(Tool::AutoShape);
        }
        if plain(Key::Num9) {
            self.insert_image();
        }
        if plain(Key::Q) {
            self.tool_locked = !self.tool_locked;
        }
        let finish_keys = plain(Key::Escape) || plain(Key::Enter);
        if finish_keys && self.multi.is_some() {
            self.finish_multi();
        } else if finish_keys && self.line_editor.is_some() {
            self.line_editor = None;
            self.selected_point = None;
        } else if cmd(Key::Enter) && self.single_line().is_some() {
            self.line_editor = self.single_line();
            self.selected_point = None;
        } else if plain(Key::Escape) {
            self.menu_open = false;
            self.connecting = None;
            self.set_elevated(None);
            if self.selected.is_empty() {
                self.tool = Tool::Select;
            }
            self.selected.clear();
        } else if plain(Key::Enter) {
            if let [i] = self.selected[..] {
                if self.scene.elements.get(i).is_some_and(|e| e.terminal.is_some()) {
                    // Enter on a selected terminal types into it
                    self.terminal_focus = Some(self.scene.elements[i].id.clone());
                } else if self.scene.elements.get(i).is_some_and(|e| matches!(e.kind, ElementKind::Text { .. }) || e.created.is_some()) {
                    self.begin_text_edit(i);
                }
            }
        }
        if plain(Key::Delete) || plain(Key::Backspace) {
            if !self.delete_selected_point() {
                self.delete_selected();
            }
        }
        if cmd(Key::Delete) || cmd(Key::Backspace) {
            self.clear_canvas();
        }
        if cmd(Key::Z) {
            self.undo();
        }
        if cmd_shift(Key::Z) || cmd(Key::Y) {
            self.redo();
        }
        if cmd(Key::S) {
            self.save();
        }
        if cmd_shift(Key::S) {
            self.save_as();
        }
        if cmd(Key::O) {
            self.open();
        }
        if cmd_shift(Key::E) {
            self.export_open = true;
        }
        if cmd(Key::G) {
            self.group_selection();
        }
        if cmd_shift(Key::G) {
            self.ungroup_selection();
        }
        if cmd_shift(Key::L) {
            self.toggle_lock_selected();
        }
        if cmd(Key::A) {
            self.select_all();
        }
        if cmd(Key::D) {
            self.duplicate();
        }
        if cmd(Key::Equals) || cmd(Key::Plus) || cmd_shift(Key::Equals) || cmd_shift(Key::Plus) {
            self.zoom_step(0.1);
        }
        if cmd(Key::Minus) {
            self.zoom_step(-0.1);
        }
        if cmd(Key::Num0) {
            let center = self.canvas_rect.center();
            self.zoom_at(1.0, center);
        }
        if hit(Key::Num1, false, true, false) || plain(Key::Exclamationmark) {
            self.zoom_to_fit(false);
        }
        if hit(Key::Num2, false, true, false) {
            self.zoom_to_fit(true);
        }
        for (key, delta) in [
            (Key::ArrowLeft, Vec2::new(-1.0, 0.0)),
            (Key::ArrowRight, Vec2::new(1.0, 0.0)),
            (Key::ArrowUp, Vec2::new(0.0, -1.0)),
            (Key::ArrowDown, Vec2::new(0.0, 1.0)),
        ] {
            if plain(key) {
                self.nudge(delta);
            }
            if hit(key, false, true, false) {
                self.nudge(delta * 5.0);
            }
        }
        if cmd(Key::CloseBracket) {
            self.reorder(Layer::Forward);
        }
        if cmd(Key::OpenBracket) {
            self.reorder(Layer::Backward);
        }
        if cmd_shift(Key::CloseBracket) || hit(Key::CloseBracket, true, false, true) {
            self.reorder(Layer::Front);
        }
        if cmd_shift(Key::OpenBracket) || hit(Key::OpenBracket, true, false, true) {
            self.reorder(Layer::Back);
        }
        if hit(Key::D, false, true, true) {
            self.prefs.theme = if self.dark() { Theme::Light } else { Theme::Dark };
        }
        if cmd(Key::Slash) || plain(Key::Questionmark) || hit(Key::Questionmark, false, true, false) || hit(Key::Slash, false, true, false) {
            self.open_palette();
        }
    }
}
