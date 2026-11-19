//! `ui.canvas.<verb>`: what Swift's canvas sends, through the same code as egui's canvas.

use super::*;

impl DrawingApp {
    /// `ui.canvas.<verb>`: pointer and keys from Swift, through the same editing code as egui's canvas.
    pub(crate) fn native_canvas_act(&mut self, verb: &str, args: &Value) -> Result<String, String> {
        let text = |key: &str| args[key].as_str().unwrap_or_default().to_string();
        let flag = |key: &str| args[key].as_bool().unwrap_or(false);
        let number = |key: &str| args[key].as_f64().unwrap_or(0.0) as f32;
        let modifiers = Modifiers { alt: flag("alt"), ctrl: false, shift: flag("shift"), mac_cmd: flag("cmd"), command: flag("cmd") };

        match verb {
            "view" => self.native_view(args),
            "pointer" => {
                self.native_view(args);
                let pos = Pos2::new(number("px"), number("py"));

                self.mods = modifiers;

                match text("phase").as_str() {
                    "down" => {
                        self.press_canvas(pos, text("button") == "middle", false, modifiers, false);
                    }
                    "move" => self.update_gesture(pos),
                    "up" => {
                        self.update_gesture(pos);
                        self.release(pos);

                        // egui's double-click comes with the second release
                        if args["clicks"].as_u64().unwrap_or(1) >= 2 {
                            self.double_click_canvas(pos, true, true);
                        }
                    }
                    "cancel" => self.cancel_gesture(),
                    phase => return Err(format!("Fase do ponteiro desconhecida: {phase}")),
                }

                self.update_bindings();
            }
            "tool" => {
                let name = text("name");
                let tool = all_tools().find(|tool| tool_name(*tool) == name).ok_or(format!("Ferramenta desconhecida: {name}"))?;

                self.select_tool(tool);
            }
            "key" => self.native_key(&text("key"), modifiers),
            "text" => {
                if let Some(index) = self.editing
                    && let Some(Element { kind: ElementKind::Text { value, .. }, .. }) = self.scene.elements.get_mut(index)
                {
                    *value = text("value");
                    recenter_labels(&mut self.scene.elements);

                    // where the caret is: a note shows the marks of what it is on (and the bar's marks go there)
                    let caret =
                        args["head"].as_u64().map(|head| markdown::Caret { head: head as usize, anchor: args["anchor"].as_u64().unwrap_or(head) as usize });

                    if let Some(caret) = caret {
                        self.note_selection = Some((index, caret.head.min(caret.anchor), caret.head.max(caret.anchor)));
                    }

                    NATIVE.with_borrow_mut(|native| native.caret = caret);
                }
            }
            "text_done" => {
                if self.elevated_index().is_some_and(|index| self.scene.elements[index].created.is_some()) {
                    self.set_elevated(None);
                } else {
                    self.finish_text();
                }
            }
            "image" => {
                let id = text("id");
                let element = self.scene.elements.iter().find(|element| element.id == id).ok_or(format!("Elemento não encontrado: {id}"))?;

                return element.image.as_deref().map(str::to_string).ok_or(format!("O elemento {id} não é uma imagem"));
            }
            "font" => return Ok(base64(crate::geometry::HAND_FACE)),
            // a note's button: run (notes/buttons.rs) or, from its menu, removed
            "note_button" => {
                let id = text("id");
                let note = self.scene.elements.iter().position(|element| element.id == id).ok_or(format!("Nota não encontrada: {id}"))?;
                let button = args["index"].as_u64().unwrap_or(0) as usize;

                if flag("remove") { self.remove_note_button(note, button) } else { self.run_note_button(note, button) }
            }
            // a drawing's link mark (filetree/link.rs)
            "link" => {
                let id = text("id");
                let link = self.scene.elements.iter().find(|element| element.id == id).and_then(|element| element.link.clone()).ok_or("Sem link.")?;

                self.open_element_link(&id, &link);
            }
            // "Voltar"'s ×: it goes without going back
            "link_dismiss" => self.filetrees.link_back = None,
            "minimap" => {
                let at = Pos2::new(number("x"), number("y"));

                NATIVE.with_borrow_mut(|native| self.native_minimap_press(&mut native.minimap, text("phase") != "up", at));
            }
            "dock" => self.native_dock(&text("phase"), Pos2::new(number("x"), number("y")))?,
            // the pinch or Cmd + wheel over the enlarged tree, by `factor`: its own zoom, a tenth per e^0.1 (canvas/wheel.rs)
            "lifted_zoom" => self.lifted_zoom_by(number("factor").max(0.01).ln() / 0.1),
            _ => return Err(format!("Ação do canvas desconhecida: {verb}")),
        }

        Ok(String::new())
    }

    /// A key on the canvas, by egui's name for it (`Key::name`): the canvas's part of handle_shortcuts.
    fn native_key(&mut self, name: &str, modifiers: Modifiers) {
        if self.in_gesture() {
            // mid-drag the keys wait; Esc cancels it
            if name == "Escape" {
                self.cancel_gesture();
            }

            return;
        }

        let Some(key) = Key::from_name(name) else { return };
        let (command, shift) = (modifiers.command, modifiers.shift);

        match key {
            Key::Z if command && shift => self.redo(),
            Key::Z if command => self.undo(),
            Key::Y if command => self.redo(),
            Key::A if command => self.select_all(),
            Key::D if command => self.duplicate(),
            Key::Delete | Key::Backspace if !command => {
                if !self.delete_selected_point() {
                    self.delete_selected();
                }
            }
            Key::Escape | Key::Enter if self.multi.is_some() => self.finish_multi(),
            Key::Escape | Key::Enter if self.line_editor.is_some() => {
                self.line_editor = None;
                self.selected_point = None;
            }
            Key::Escape => {
                self.connecting = None;
                self.set_elevated(None);

                if self.selected.is_empty() {
                    self.tool = Tool::Select;
                }

                self.selected.clear();
            }
            Key::Enter => {
                if let [index] = self.selected[..]
                    && self.scene.elements.get(index).is_some_and(|element| matches!(element.kind, ElementKind::Text { .. }) || element.created.is_some())
                {
                    self.begin_text_edit(index);
                }
            }
            Key::ArrowLeft | Key::ArrowRight | Key::ArrowUp | Key::ArrowDown if !command => {
                let delta = match key {
                    Key::ArrowLeft => Vec2::new(-1.0, 0.0),
                    Key::ArrowRight => Vec2::new(1.0, 0.0),
                    Key::ArrowUp => Vec2::new(0.0, -1.0),
                    _ => Vec2::new(0.0, 1.0),
                };

                self.nudge(if shift { delta * 5.0 } else { delta });
            }
            Key::X if shift && !command && self.drawing() => self.select_tool(Tool::AutoShape),
            _ if !command && !shift && !modifiers.alt => {
                if let Some(tool) = all_tools().find(|tool| self.tool_keys_live(*tool) && tool.keys().contains(&key)) {
                    self.select_tool(tool);
                }
            }
            _ => {}
        }
    }
}
