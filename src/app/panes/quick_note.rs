//! Nota rápida (asked: "uma nota rápida de fácil acesso, ao lado de Alterações … um botão a gente leva essa nota para
//! dentro do canvas … não quero que esteja em markdown"): a panel of plain text, kept with the preferences, always a
//! click away; Levar para o canvas puts it on the canvas as a note shown as typed (no markdown) and empties the panel.

use crate::prelude::*;

/// The size of the note it makes on the canvas.
const NOTE_SIZE: Vec2 = Vec2::new(280.0, 240.0);

impl DrawingApp {
    /// The panel's text, `height` tall at least: what is typed is kept as it is (no markdown).
    pub(crate) fn quick_note_pane(&mut self, ui: &mut egui::Ui, height: f32) {
        let colors = self.colors();
        let edit = egui::TextEdit::multiline(&mut self.prefs.quick_note)
            .id(egui::Id::unique("quick_note"))
            .font(FontId::monospace(13.0))
            .text_color(colors.text)
            .hint_text("Escreva aqui. Texto simples, sem markdown; fica guardado até você levar para o canvas.")
            .frame(egui::Frame::NONE)
            .margin(Vec2::ZERO)
            .desired_width(f32::INFINITY)
            .min_size(Vec2::new(ui.available_width(), height));
        let field = ui.add(edit);
        // opened: ready to type
        if std::mem::take(&mut self.panes.focus_note) {
            field.request_focus();
        }
    }

    /// The bar under the text: how long it is and Levar para o canvas.
    pub(crate) fn quick_note_bar(&mut self, ui: &mut egui::Ui) {
        let colors = self.colors();
        let text = self.prefs.quick_note.trim();
        let lines = if text.is_empty() { 0 } else { text.lines().count() };
        let mut take = false;
        ui.horizontal_centered(|ui| {
            let size = match lines {
                0 => "Vazia".to_string(),
                1 => "1 linha".to_string(),
                n => format!("{n} linhas"),
            };
            ui.label(egui::RichText::new(size).size(12.0).color(colors.muted));
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                take = ui
                    .add_enabled(lines > 0, egui::Button::new("Levar para o canvas"))
                    .on_hover_text("Vira uma nota no canvas, com o texto como está")
                    .clicked();
            });
        });
        if take && super::live(ui) {
            match self.quick_note_to_canvas() {
                Ok(said) => self.toast(said),
                Err(why) => self.toast(why),
            }
        }
    }

    /// Levar para o canvas: the text as a note on the canvas, in a free spot of the view, selected, shown as typed (no
    /// markdown); the panel empties. One Cmd+Z takes the note back. What happened.
    pub(crate) fn quick_note_to_canvas(&mut self) -> Result<String, String> {
        let text = self.prefs.quick_note.trim_end().to_string();
        if text.trim().is_empty() {
            return Err("A nota rápida está vazia.".into());
        }
        let middle = self.to_world(self.canvas_rect.center());
        let spot = self.free_spot(Rect::from_center_size(middle, NOTE_SIZE));
        let [mut note, label] = self.new_note(spot, text);
        note.note.raw = true;
        self.snapshot();
        self.scene.elements.extend([note, label]);
        self.selected = vec![self.scene.elements.len() - 2];
        self.dirty = true;
        self.prefs.quick_note.clear();
        Ok("A nota rápida foi para o canvas.".into())
    }

    /// The MCP's `quick_note`: show (opens the panel), get, set `text`, append `text` (a line under it), to_canvas.
    pub(crate) fn act_quick_note(&mut self, args: &Value) -> Result<String, String> {
        let text = args["text"].as_str().unwrap_or_default();
        match args["action"].as_str().unwrap_or("get") {
            "show" => {
                self.chat_open = false;
                self.set_pane(Some(super::Pane::QuickNote));
                Ok("Nota rápida aberta.".into())
            }
            "get" if self.prefs.quick_note.trim().is_empty() => Ok("A nota rápida está vazia.".into()),
            "get" => Ok(self.prefs.quick_note.clone()),
            "set" => {
                self.prefs.quick_note = text.to_string();
                Ok("Nota rápida trocada.".into())
            }
            "append" => {
                let gap = if self.prefs.quick_note.is_empty() || self.prefs.quick_note.ends_with('\n') { "" } else { "\n" };
                self.prefs.quick_note = format!("{}{gap}{text}", self.prefs.quick_note);
                Ok("Texto juntado à nota rápida.".into())
            }
            "to_canvas" => self.quick_note_to_canvas(),
            other => Err(format!("Ação desconhecida: {other}. Use show, get, set, append ou to_canvas.")),
        }
    }
}
