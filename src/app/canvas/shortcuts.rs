//! Keyboard shortcuts.

use crate::prelude::*;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Focus {
    Canvas,
    EguiField,
    CanvasText,
    Tree,
    Terminal,
    Composer,
    PortalAddress,
    Chat,
    Palette,
    Dialog,
}

impl DrawingApp {
    /// Who has the keys now, in the order they are given away. A surface that takes the keyboard
    /// gets its case here: one left out would let its keys fall through to the canvas.
    pub(crate) fn focus(&self, ctx: &Context) -> Focus {
        if ctx.egui_wants_keyboard_input() {
            Focus::EguiField
        } else if self.editing.is_some() {
            Focus::CanvasText
        } else if self.card_focus.is_some() {
            Focus::Tree
        } else if self.terminal_focus.is_some() {
            Focus::Terminal
        } else if self.composer.is_some() {
            Focus::Composer
        } else if self.portal_url_edit.is_some() {
            Focus::PortalAddress
        } else if self.chat_open {
            Focus::Chat
        } else if self.palette_open {
            Focus::Palette
        } else if self.dialog_open() {
            Focus::Dialog
        } else {
            Focus::Canvas
        }
    }

    /// Whether a window or a field has the keys: then the canvas's shortcuts stay off and the keys go to it.
    pub(crate) fn keyboard_taken(&self, ctx: &Context) -> bool {
        self.focus(ctx) != Focus::Canvas
    }

    /// Space held pans like the hand, unless the keys are someone's (a focused terminal types it).
    pub(crate) fn space_pan(&self, ctx: &Context) -> bool {
        !self.keyboard_taken(ctx) && ctx.input(|input| input.key_down(Key::Space))
    }

    pub(crate) fn handle_shortcuts(&mut self, ctx: &Context) {
        if self.design.is_some() && ctx.input(|input| input.key_pressed(Key::Escape)) {
            self.design_exit();
        }
        // ⌘, opens and closes Ajustes whoever has the keys (read from the event: scripted keys set the modifiers only there)
        let settings_key = ctx.input(|input| {
            input.events.iter().any(|event| matches!(event, egui::Event::Key { key: Key::Comma, pressed: true, modifiers, .. } if modifiers.command))
        });
        self.filetree_find_key(ctx);
        if ctx.current_pass_index() == 0 && self.card_focus.is_some() {
            self.filetree_keys(ctx);
        }
        // mid-drag the keys wait (Delete or Ctrl+Z would pull the scene from under the drag); Esc cancels it
        if ctx.current_pass_index() == 0 && self.in_gesture() {
            if ctx.input(|input| input.key_pressed(Key::Escape)) {
                self.cancel_gesture();
            }
            return;
        }
        // the floors' overview (Ctrl+Shift+\; with Shift the key may come as the pipe): it opens over the
        // canvas and, open, holds the keys as a dialog, so the same keys close it
        let floors_key = ctx.input(|input| {
            input.events.iter().any(
                |event| matches!(event, egui::Event::Key { key: Key::Backslash | Key::Pipe, pressed: true, modifiers, .. } if modifiers.command && modifiers.shift),
            )
        });
        if floors_key && ctx.current_pass_index() == 0 && (self.focus(ctx) == Focus::Canvas || (self.floors_ui.overview && self.focus(ctx) == Focus::Dialog)) {
            self.floors_ui.overview = !self.floors_ui.overview;
        }
        if ctx.current_pass_index() > 0 || (self.keyboard_taken(ctx) && !settings_key) {
            return;
        }
        let events = ctx.input(|input| input.events.clone());
        let hit = |key: Key, command: bool, shift: bool, alt: bool| {
            events.iter().any(|event| {
                matches!(event, egui::Event::Key { key: pressed_key, pressed: true, modifiers, .. }
                    if *pressed_key == key && modifiers.command == command && modifiers.shift == shift && modifiers.alt == alt)
            })
        };
        let plain = |key: Key| hit(key, false, false, false);
        let command_pressed = |key: Key| hit(key, true, false, false);
        let command_shift_pressed = |key: Key| hit(key, true, true, false);

        for event in events.iter() {
            match event {
                egui::Event::Copy => self.copy(ctx),
                egui::Event::Cut => {
                    self.copy(ctx);
                    self.delete_selected();
                }
                egui::Event::Paste(text) => self.paste(text),
                egui::Event::PasteImage(image) => self.paste_image(image),
                _ => {}
            }
        }
        // preferences: Cmd+, Ajustes, Cmd+' grid
        if command_pressed(Key::Comma) {
            self.settings_open = !self.settings_open;
        }
        if command_pressed(Key::Quote) {
            self.prefs.grid = !self.prefs.grid;
        }
        // a dialog's Esc and Enter are its own, not deselect / type into the terminal
        if self.dialog_open() {
            return;
        }
        if self.close_properties_popover(ctx) {
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
            let at = picks.iter().position(|pick| Some(*pick) == self.style.fill).map_or(1, |index| (index + 1) % picks.len());
            self.style.fill = Some(picks[at]);
        } else {
            let tools = Tool::TOOLBAR.into_iter().chain(Tool::DRAWING).chain(Tool::EXTRA);
            let picked: Vec<Tool> = tools.filter(|tool| self.tool_keys_live(*tool) && tool.keys().iter().any(|key| plain(*key))).collect();
            for tool in picked {
                self.select_tool(tool);
            }
        }
        // Shift+X and 9 make something too: only while drawing
        let drawing = self.drawing();
        if drawing && hit(Key::X, false, true, false) {
            self.select_tool(Tool::AutoShape);
        }
        if drawing && plain(Key::Num9) {
            self.insert_image();
        }
        let finish_keys = plain(Key::Escape) || plain(Key::Enter);
        if finish_keys && self.multi.is_some() {
            self.finish_multi();
        } else if finish_keys && self.line_editor.is_some() {
            self.line_editor = None;
            self.selected_point = None;
        } else if command_pressed(Key::Enter) && self.single_line().is_some() {
            self.line_editor = self.single_line();
            self.selected_point = None;
        } else if plain(Key::Escape) {
            self.connecting = None;
            self.set_elevated(None);
            if self.selected.is_empty() {
                self.tool = Tool::Select;
            }
            self.selected.clear();
        } else if plain(Key::Enter)
            && let [index] = self.selected[..]
        {
            if self.scene.elements.get(index).is_some_and(|element| element.terminal.is_some()) {
                // Enter on a selected terminal types into it
                self.terminal_focus = Some(self.scene.elements[index].id.clone());
            } else if self.scene.elements.get(index).is_some_and(|element| matches!(element.kind, ElementKind::Text { .. }) || element.created.is_some()) {
                self.begin_text_edit(index);
            }
        }
        if (plain(Key::Delete) || plain(Key::Backspace)) && !self.delete_selected_point() {
            self.delete_selected();
        }
        if command_pressed(Key::Delete) || command_pressed(Key::Backspace) {
            self.clear_canvas();
        }
        if command_pressed(Key::Z) {
            self.undo();
        }
        if command_shift_pressed(Key::Z) || command_pressed(Key::Y) {
            self.redo();
        }
        if command_pressed(Key::S) {
            self.save();
        }
        if command_shift_pressed(Key::S) {
            self.save_as();
        }
        if command_pressed(Key::O) {
            self.open();
        }
        if command_shift_pressed(Key::E) {
            self.export_open = true;
        }
        if command_pressed(Key::G) {
            self.group_selection();
        }
        if command_shift_pressed(Key::G) {
            self.ungroup_selection();
        }
        if command_shift_pressed(Key::T) {
            self.tidy_selection();
        }
        if command_shift_pressed(Key::L) {
            self.toggle_lock_selected();
        }
        if command_pressed(Key::A) {
            self.select_all();
        }
        if command_pressed(Key::D) {
            self.duplicate();
        }
        if command_pressed(Key::Equals) || command_pressed(Key::Plus) || command_shift_pressed(Key::Equals) || command_shift_pressed(Key::Plus) {
            self.zoom_step(0.1);
        }
        if command_pressed(Key::Minus) {
            self.zoom_step(-0.1);
        }
        if command_pressed(Key::Num0) {
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
            if self.minimap_area.focused {
                break;
            }
            if plain(key) {
                self.nudge(delta);
            }
            if hit(key, false, true, false) {
                self.nudge(delta * 5.0);
            }
        }
        if command_pressed(Key::CloseBracket) {
            self.reorder(Layer::Forward);
        }
        if command_pressed(Key::OpenBracket) {
            self.reorder(Layer::Backward);
        }
        if command_shift_pressed(Key::CloseBracket) || hit(Key::CloseBracket, true, false, true) {
            self.reorder(Layer::Front);
        }
        if command_shift_pressed(Key::OpenBracket) || hit(Key::OpenBracket, true, false, true) {
            self.reorder(Layer::Back);
        }
        if hit(Key::D, false, true, true) {
            self.prefs.theme = if self.dark() { Theme::Light } else { Theme::Dark };
        }
        if command_pressed(Key::Slash) || plain(Key::Questionmark) || hit(Key::Questionmark, false, true, false) || hit(Key::Slash, false, true, false) {
            self.open_palette();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn check(app: DrawingApp, expected: Focus) {
        let ctx = Context::default();
        assert_eq!(app.focus(&ctx), expected);
        assert_eq!(app.keyboard_taken(&ctx), expected != Focus::Canvas);
    }

    #[test]
    fn focus_names_each_keyboard_owner_and_only_canvas_leaves_it_free() {
        check(DrawingApp::default(), Focus::Canvas);
        let mut app = DrawingApp::default();
        app.editing = Some(0);
        check(app, Focus::CanvasText);
        let mut app = DrawingApp::default();
        app.card_focus = Some("i".into());
        check(app, Focus::Tree);
        let mut app = DrawingApp::default();
        app.terminal_focus = Some("t".into());
        check(app, Focus::Terminal);
        let mut app = DrawingApp::default();
        app.composer = Some(("i".into(), String::new()));
        check(app, Focus::Composer);
        let mut app = DrawingApp::default();
        app.portal_url_edit = Some(("p".into(), String::new(), false));
        check(app, Focus::PortalAddress);
        let mut app = DrawingApp::default();
        app.chat_open = true;
        check(app, Focus::Chat);
        let mut app = DrawingApp::default();
        app.palette_open = true;
        check(app, Focus::Palette);
        let mut app = DrawingApp::default();
        app.settings_open = true;
        check(app, Focus::Dialog);
        let ctx = Context::default();
        let field = egui::Id::unique("field");
        ctx.memory_mut(|memory| memory.request_focus(field));
        let mut text = String::new();
        let mut output = ctx.run_ui(egui::RawInput::default(), |ui| {
            ui.add(egui::TextEdit::singleline(&mut text).id(field));
        });
        output.textures_delta.clear();
        let app = DrawingApp::default();
        assert_eq!(app.focus(&ctx), Focus::EguiField);
        assert!(app.keyboard_taken(&ctx));
    }
}
