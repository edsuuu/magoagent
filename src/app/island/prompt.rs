//! The island's conversation: one card with the indigo
//! wash, the wizard at 52 at the left, and from 84 the log (the person's words in bubbles at the right, the answers
//! plain), the model's pill and a one-line box. Its ⤢ opens the whole mini chat in the island instead.

use super::log::said;
use super::paint::{BLACK, CARD, DIM_3, EXPAND, INK, Out, View, aim, font};
use crate::app::chat::list::glyph;
use crate::app::mascot::mago::MagoState;
use crate::app::ui::agents::{AgentRow, AgentState};
use crate::prelude::*;
use crate::transcript::ChatItem;

/// The island's height on the conversation (`chatPromptHeight`): 240, 40 more for each message, 300 at most.
pub(super) fn prompt_height(messages: usize) -> f32 {
    (240.0 + messages as f32 * 40.0).min(300.0)
}

const ARROW_UP: &[&[(f32, f32)]] = &[&[(12.0, 18.5), (12.0, 5.5)], &[(6.5, 11.0), (12.0, 5.5), (17.5, 11.0)]];

/// `.chat-body` of a conversation page (this one and a session's): padding 12 16 14 84, a column 6 apart; (the log,
/// the row over the box with whose it is, the box).
pub(super) fn chat_body(page: Rect) -> (Rect, Rect, Rect) {
    let body = Rect::from_min_max(Pos2::new(page.left() + 84.0, page.top() + 12.0), Pos2::new(page.right() - 16.0, page.bottom() - 14.0));
    let bar = Rect::from_min_max(Pos2::new(body.left(), body.bottom() - 40.0), body.max);
    let chip_row = Rect::from_min_max(Pos2::new(body.left(), bar.top() - 6.0 - 22.0), Pos2::new(body.right() - 10.0, bar.top() - 6.0));
    let log = Rect::from_min_max(Pos2::new(body.left(), body.top()), Pos2::new(body.right() - 22.0, chip_row.top() - 6.0));
    (log, chip_row, bar)
}

/// The box's one-line field and its round send button, `(field, button)`: the field's response, and whether Enter or
/// the button sends what is in `draft` (something besides blanks).
pub(super) fn send_field(
    ui: &mut egui::Ui,
    painter: &Painter,
    (field, button): (Rect, Rect),
    draft: &mut String,
    id: egui::Id,
    hint: &str,
    salt: &str,
) -> (egui::Response, bool) {
    let edit = ui.put(
        field,
        egui::TextEdit::singleline(draft)
            .id(id)
            .frame(egui::Frame::NONE)
            .font(font(13.0))
            .text_color(INK)
            .align(egui::Align2::LEFT_CENTER)
            .hint_text(egui::RichText::new(hint).font(font(13.0)).color(DIM_3)),
    );
    let send = ui.interact(button, ui.scope_id().with(salt), Sense::click()).on_hover_cursor(CursorIcon::PointingHand);
    let pressed = send.is_pointer_button_down_on();
    let ground = Rect::from_center_size(button.center(), button.size() * if pressed { 0.94 } else { 1.0 });
    painter.circle_filled(ground.center(), ground.width() / 2.0, if draft.trim().is_empty() { INK.gamma_multiply(0.5) } else { INK });
    glyph(painter, ground.center(), 12.0, 1.8, BLACK, ARROW_UP);
    let enter = edit.lost_focus() && ui.input(|input| input.key_pressed(egui::Key::Enter));
    let sent = (enter || send.clicked()) && !draft.trim().is_empty();
    (edit, sent)
}

impl DrawingApp {
    pub(super) fn island_prompt(&mut self, ui: &mut egui::Ui, page: Rect, row: &AgentRow, view: &View, time: f64, out: &mut Out) {
        self.refresh_transcript();
        let painter = ui.painter().with_clip_rect(page);
        painter.rect(page, 20, CARD, Stroke::new(1.0, Color32::from_white_alpha(9)), StrokeKind::Inside);
        // botPosition with no botY: the middle of an 84 pt card under the header, at 52 from the island's left, 44 wide
        let line = view.lines.iter().find(|line| line.id == row.id);
        let origin = view.origin.get();
        let height = page.bottom() + 10.0 - origin.y;
        let state = MagoState::from_agent(row.state, time);
        aim(view, Pos2::new(origin.x + 52.0, origin.y + 42.0 + (height - 42.0 - 84.0) / 2.0 + 42.0), 44.0, state);

        // ⤢: the whole mini chat in the island (attachments, folder, mode, model, dictation, voice)
        let expand = Rect::from_center_size(Pos2::new(page.right() - 18.0, page.top() + 18.0), Vec2::splat(20.0));
        let response = ui.interact(expand, ui.scope_id().with("island_prompt_full"), Sense::click()).on_hover_text("Abrir o chat completo");
        glyph(&painter, expand.center(), 14.0, 1.5, if response.hovered() { INK } else { DIM_3 }, EXPAND);
        out.full = response.clicked();

        let (log, chip_row, bar) = chat_body(page);
        let messages: Vec<&ChatItem> = self.transcripts.get(&row.id).map(|transcript| said(&transcript.items).collect()).unwrap_or_default();
        let working = matches!(row.state, AgentState::Working(_));
        out.picture = self.island_log(ui, log, &messages, working, time).or(out.picture.take());

        // `.model-btn`: the agent and its model; a click opens the whole chat, where they are chosen
        let chat = self.chats.iter().find(|chat| chat.id == row.id);
        let model = chat.map(|chat| if chat.model.is_empty() { "padrão".to_string() } else { chat.model.clone() }).unwrap_or_default();
        let label = format!("{} · {model}", row.spec.label());
        let name = crate::app::chat::list::clipped(ui.painter(), label, font(10.5), Color32::from_rgb(0x7b, 0x80, 0x89), chip_row.width() - 40.0);
        let chip = Rect::from_min_size(
            Pos2::new(chip_row.right() - name.size().x - 8.0 - 6.0 - 5.0 - 8.0, chip_row.top() + 2.0),
            Vec2::new(name.size().x + 27.0, 20.0),
        );
        let response = ui.interact(chip, ui.scope_id().with("island_prompt_model"), Sense::click()).on_hover_text("Trocar o modelo no chat completo");
        painter.rect_filled(chip, 10, Color32::from_white_alpha(if response.hovered() { 26 } else { 15 }));
        painter.circle_filled(Pos2::new(chip.left() + 11.0, chip.center().y), 3.0, line.map_or(INK, |line| line.color));
        painter.galley(Pos2::new(chip.left() + 19.0, chip.center().y - name.size().y / 2.0), name, Color32::from_rgb(0x7b, 0x80, 0x89));
        out.full |= response.clicked();

        // `.chat-bar`: 7% white, radius 12, padding 6 10; the box and the round send button
        painter.rect_filled(bar, 12, Color32::from_white_alpha(18));
        let send_rect = Rect::from_center_size(Pos2::new(bar.right() - 10.0 - 14.0, bar.center().y), Vec2::splat(28.0));
        let field = Rect::from_min_max(Pos2::new(bar.left() + 10.0, bar.top() + 6.0), Pos2::new(send_rect.left() - 8.0, bar.bottom() - 6.0));
        let hint = if messages.is_empty() { "Pergunte qualquer coisa…" } else { "Continuar…" };
        let mut draft = std::mem::take(&mut self.chat_draft);
        let (edit, sent) = send_field(ui, &painter, (field, send_rect), &mut draft, egui::Id::unique("chat_draft"), hint, "island_prompt_send");
        if sent {
            self.send_from_box(ui.ctx(), &row.id, std::mem::take(&mut draft).trim().to_string());
            edit.request_focus();
        }
        if self.chat_draft.is_empty() {
            self.chat_draft = draft;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_conversation_grows_forty_a_message_up_to_three_hundred() {
        assert_eq!(prompt_height(0), 240.0);
        assert_eq!(prompt_height(1), 280.0);
        assert_eq!(prompt_height(5), 300.0);
    }
}
