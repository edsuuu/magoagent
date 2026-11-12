//! The editor: tabs, find bar, the highlighted text with gutter marks, squiggles and Code Vision.

use super::*;
mod bars;
mod body;
mod helpers;
#[cfg(test)]
mod tests;
pub(crate) use helpers::*;

pub(crate) const RIGHT_MARGIN_COLUMN: usize = 120;
const SHOW_WHITESPACES: bool = false;

impl Ide {
    fn code_comment_popup(&mut self, ui: &mut Ui) {
        let Some((path, from, to, mut comment)) = self.code_comment.take() else { return };
        let (mut send, mut keep) = (false, true);
        let has_agent = self.has_agent;
        // over the middle of the editor, above the app's own panels and in the IDE's look: a window
        // of its own on the context starts in the corner, under the sidebar, with the app's style
        egui::Window::new("Comentar com agente")
            .id(Id::new("ide_code_comment"))
            .title_bar(false)
            .resizable(false)
            .order(Order::Foreground)
            .pivot(Align2::CENTER_CENTER)
            .fixed_pos(ui.max_rect().center())
            .frame(Frame::NONE.fill(t::POPUP).stroke(Stroke::new(1.0, t::POPUP_BORDER)).corner_radius(10).inner_margin(14))
            .show(ui.ctx(), |ui| {
                t::style(ui.style_mut());
                ui.label(egui::RichText::new("Comentar com agente").strong());
                ui.label(egui::RichText::new(format!("{} · linhas {from}–{to}", file_name(&path))).color(t::MUTED));
                ui.add_space(6.0);
                let ready = has_agent && !comment.trim().is_empty();
                let field = ui.add_enabled(has_agent, TextEdit::singleline(&mut comment).hint_text("Comentário para o agente").desired_width(340.0));
                // Enter in the field sends; the field takes the keys as the window opens
                send = ready && field.lost_focus() && ui.input(|i| i.key_pressed(Key::Enter));
                if has_agent && !field.has_focus() && !field.lost_focus() && ui.memory(|m| m.focused().is_none()) {
                    field.request_focus();
                }
                if !has_agent {
                    ui.label(egui::RichText::new("Ligue um agente a este IDE para comentar.").color(t::WARNING));
                }
                ui.add_space(6.0);
                ui.horizontal(|ui| {
                    let button = egui::Button::new(egui::RichText::new("Enviar").color(Color32::WHITE)).fill(t::ACCENT).min_size(vec2(76.0, 26.0));
                    send |= ui.add_enabled(ready, button).clicked();
                    keep = !ui.add(egui::Button::new("Cancelar").min_size(vec2(76.0, 26.0))).clicked();
                });
                keep &= !ui.input(|i| i.key_pressed(Key::Escape));
            });
        if send {
            if let Some(tab) = self.tabs.iter().find(|t| t.path == path)
                && let Ok(reference) = self.code_reference(&path, &tab.text, from, to, &comment, "editor", "local")
            {
                self.to_agent.push(super::coderef::message(&reference));
                self.status = format!("Comentário sobre {} indo para o agente", file_name(&path));
            }
        } else if keep {
            self.code_comment = Some((path, from, to, comment));
        }
    }
}
