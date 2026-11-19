//! The island's conversation: one card with the indigo
//! wash, the wizard at 52 at the left, and from 84 the log (the person's words in bubbles at the right, the answers
//! plain), the model's pill and a box that grows with the words. Its ⤢ opens the whole mini chat in the island instead.

use super::log::said;
use super::paint::{AMBER, BLACK, CARD, DIM_3, EXPAND, INK, Out, View, aim, font};
use crate::app::chat::list::{Look, clipped, glyph};
use crate::app::chat::mini_agents::{KinLook, pressable};
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
/// the row over the box with whose it is, the box). The box is `bar` tall (`bar_height`: it grows with the words); the
/// log starts `head` under the top, under the row of the conversation's name (`NAME_ROW`) when there is one.
pub(super) fn chat_body(page: Rect, bar: f32, head: f32) -> (Rect, Rect, Rect) {
    let body = Rect::from_min_max(Pos2::new(page.left() + 84.0, page.top() + 12.0), Pos2::new(page.right() - 16.0, page.bottom() - 14.0));
    let bar = Rect::from_min_max(Pos2::new(body.left(), body.bottom() - bar), body.max);
    let chip_row = Rect::from_min_max(Pos2::new(body.left(), bar.top() - 6.0 - 22.0), Pos2::new(body.right() - 10.0, bar.top() - 6.0));
    let log = Rect::from_min_max(Pos2::new(body.left(), body.top() + head), Pos2::new(body.right() - 22.0, chip_row.top() - 6.0));

    (log, chip_row, bar)
}

/// The row over the conversation's log: who it is, the Maestro's agents (or the way back to it) and the ⤢, each in a
/// room of its own.
pub(super) const NAME_ROW: f32 = 28.0;

/// The name row of `page`: from the log's left to the card's right, its middle where the ⤢ always was.
pub(super) fn name_row(page: Rect) -> Rect {
    Rect::from_min_max(Pos2::new(page.left() + 84.0, page.top() + 6.0), Pos2::new(page.right() - 6.0, page.top() + 30.0))
}

/// The most lines a box grows to; past them its words scroll.
const FIELD_LINES: f32 = 5.0;

/// How tall the box `id` is (`.chat-bar`, 40 for a line): it grows with the lines its words wrapped to last frame, up to
/// `FIELD_LINES`.
pub(super) fn bar_height(ui: &egui::Ui, id: egui::Id) -> f32 {
    let words = ui.data(|data| data.get_temp::<f32>(id.with("words_height"))).unwrap_or(0.0);

    (words + 22.0).max(40.0)
}

/// A box's field in `field`: the words wrap at its width and Shift+Enter starts a line; it is as tall as they are
/// (`bar_height` sizes the box from it), in the middle while they are one line, and they scroll past `FIELD_LINES`.
/// The field's response, and whether Enter (without Shift) was pressed in it.
pub(super) fn growing_field(ui: &mut egui::Ui, field: Rect, draft: &mut String, id: egui::Id, hint: &str) -> (egui::Response, bool) {
    let row = ui.fonts_mut(|fonts| fonts.row_height(&font(13.0)));
    let words = ui.data(|data| data.get_temp::<f32>(id.with("words_height"))).unwrap_or(row).clamp(row, FIELD_LINES * row);
    let room = Rect::from_center_size(field.center(), Vec2::new(field.width(), words.min(field.height().max(row))));

    // read before the field takes the key: Shift lives on the key event, not the global modifiers
    let enter = ui.memory(|memory| memory.has_focus(id))
        && ui.input(|input| {
            input.events.iter().any(|event| matches!(event, egui::Event::Key { key: Key::Enter, pressed: true, modifiers, .. } if !modifiers.shift))
        });

    let mut child = ui.new_child(egui::UiBuilder::new().max_rect(room).layout(egui::Layout::top_down(egui::Align::Min)));
    let scroll = egui::ScrollArea::vertical().id_salt(id.with("scroll")).max_height(room.height()).auto_shrink([false, true]);

    let output = scroll
        .show(&mut child, |ui| {
            egui::TextEdit::multiline(draft)
                .id(id)
                .frame(egui::Frame::NONE)
                .margin(egui::Margin::ZERO)
                .font(font(13.0))
                .text_color(INK)
                .desired_width(room.width())
                .desired_rows(1)
                .return_key(Some(egui::KeyboardShortcut::new(Modifiers::SHIFT, Key::Enter)))
                .hint_text(egui::RichText::new(hint).font(font(13.0)).color(DIM_3))
                .show(ui)
        })
        .inner;

    let height = output.galley.size().y.clamp(row, FIELD_LINES * row);

    if (height - words).abs() > 0.5 {
        ui.data_mut(|data| data.insert_temp(id.with("words_height"), height));
        ui.ctx().request_repaint();
    }

    (output.response.response, enter)
}

/// The box's field and its round send button, `(field, button)`: the field's response, and whether Enter or the button
/// sends what is in `draft` (something besides blanks).
pub(super) fn send_field(
    ui: &mut egui::Ui,
    painter: &Painter,
    (field, button): (Rect, Rect),
    draft: &mut String,
    id: egui::Id,
    hint: &str,
    salt: &str,
) -> (egui::Response, bool) {
    let (edit, enter) = growing_field(ui, field, draft, id, hint);
    let send = pressable(ui, button, ui.scope_id().with(salt), "Enviar");
    let pressed = send.is_pointer_button_down_on();
    let ground = Rect::from_center_size(button.center(), button.size() * if pressed { 0.94 } else { 1.0 });

    painter.circle_filled(ground.center(), ground.width() / 2.0, if draft.trim().is_empty() { INK.gamma_multiply(0.5) } else { INK });
    glyph(painter, ground.center(), 12.0, 1.8, BLACK, ARROW_UP);
    let sent = (enter || send.clicked()) && !draft.trim().is_empty();

    (edit, sent)
}

impl DrawingApp {
    #[allow(clippy::too_many_arguments)]
    pub(super) fn island_prompt(&mut self, ui: &mut egui::Ui, page: Rect, row: &AgentRow, view: &View, look: &Look, time: f64, out: &mut Out) {
        self.refresh_transcript();
        let painter = ui.painter().with_clip_rect(page);

        painter.rect(page, 20, CARD, Stroke::new(1.0, Color32::from_white_alpha(9)), StrokeKind::Inside);
        // botPosition with no botY: the middle of an 84 pt card under the header, at 52 from the island's left, 44 wide
        let line = view.lines.iter().find(|line| line.id == row.id);
        let origin = view.origin.get();
        let height = page.bottom() + 10.0 - origin.y;
        let state = MagoState::from_agent(row.state, time);

        aim(view, Pos2::new(origin.x + 52.0, origin.y + 42.0 + (height - 42.0 - 84.0) / 2.0 + 42.0), 44.0, state);

        // the name row: ⤢ (the whole mini chat in the island: attachments, folder, mode, model, dictation, voice), the
        // Maestro's agents and who it is. No tip: they would cover the conversation
        let head = name_row(page);
        let cy = head.center().y;
        let expand = Rect::from_center_size(Pos2::new(head.right() - 12.0, cy), Vec2::splat(24.0));
        let response = pressable(ui, expand, ui.scope_id().with("island_prompt_full"), "Abrir o chat completo");

        glyph(&painter, expand.center(), 14.0, 1.5, if response.hovered() { INK } else { DIM_3 }, EXPAND);
        out.full = response.clicked();
        // an agent picked (or the Maestro again) is shown here, in the island: no floating window opens for it
        let kin_look = KinLook { ground: Color32::from_white_alpha(15), lit: Color32::from_white_alpha(30), ink: INK, font: font(12.0), height: 24.0 };
        let (pick, room) = self.kin_button(ui, row, expand.left() - 6.0, cy, &kin_look, time);

        if let Some(id) = pick {
            self.chat_agent = Some(id);
        }

        self.island_who(&painter, row, Rect::from_min_max(head.min, Pos2::new(expand.left() - 6.0 - room - 8.0, head.max.y)));

        let draft_id = egui::Id::unique("chat_draft");
        let (log, chip_row, bar) = chat_body(page, bar_height(ui, draft_id), NAME_ROW);
        let messages: Vec<&ChatItem> = self.transcripts.get(&row.id).map(|transcript| said(&transcript.items).collect()).unwrap_or_default();
        let working = matches!(row.state, AgentState::Working(_));

        out.picture = self.island_log(ui, log, &messages, working, Some(&row.id), time).or(out.picture.take());

        // `.model-btn`: the agent and its model; a click opens the whole chat, where they are chosen
        let chat = self.chats.iter().find(|chat| chat.id == row.id);
        let model = chat.map(|chat| if chat.model.is_empty() { "padrão".to_string() } else { chat.model.clone() }).unwrap_or_default();
        let label = format!("{} · {model}", row.spec.label());
        let name = clipped(ui.painter(), label, font(10.5), Color32::from_rgb(0x7b, 0x80, 0x89), chip_row.width() - 40.0);

        let chip = Rect::from_min_size(
            Pos2::new(chip_row.right() - name.size().x - 8.0 - 6.0 - 5.0 - 8.0, chip_row.top() + 2.0),
            Vec2::new(name.size().x + 27.0, 20.0),
        );

        let response = pressable(ui, chip, ui.scope_id().with("island_prompt_model"), "Trocar o modelo no chat completo");

        painter.rect_filled(chip, 10, Color32::from_white_alpha(if response.hovered() { 26 } else { 15 }));
        painter.circle_filled(Pos2::new(chip.left() + 11.0, chip.center().y), 3.0, line.map_or(INK, |line| line.color));
        painter.galley(Pos2::new(chip.left() + 19.0, chip.center().y - name.size().y / 2.0), name, Color32::from_rgb(0x7b, 0x80, 0x89));
        out.full |= response.clicked();

        // `.chat-bar`: 7% white, radius 12, padding 6 10; the box and the round send button
        painter.rect_filled(bar, 12, Color32::from_white_alpha(18));
        // the round button stays at the box's last line as the words grow
        let send_rect = Rect::from_center_size(Pos2::new(bar.right() - 10.0 - 14.0, bar.bottom() - 20.0), Vec2::splat(28.0));
        let field = Rect::from_min_max(Pos2::new(bar.left() + 10.0, bar.top() + 6.0), Pos2::new(send_rect.left() - 8.0, bar.bottom() - 6.0));
        let hint = if messages.is_empty() { "Pergunte qualquer coisa…" } else { "Continuar…" };
        // the / menu over the box, as the big chat's: its arrows, Tab, Enter and Esc while it is open (Esc closes it, not
        // the island)
        let typing = ui.memory(|memory| memory.has_focus(draft_id) || memory.had_focus_last_frame(draft_id));
        let typed = self.chat_draft.clone();
        let slash = if typing { self.slash_entries_of(&row.id, &typed) } else { None };
        let mut draft = std::mem::take(&mut self.chat_draft);
        let swallow = slash.as_ref().is_some_and(|entries| self.slash_keys(ui.ctx(), &mut draft, entries, draft_id));
        let (edit, sent) = send_field(ui, &painter, (field, send_rect), &mut draft, draft_id, hint, "island_prompt_send");

        if let Some(entries) = &slash
            && let Some(name) = self.slash_menu_ui(ui.ctx(), entries, bar.left_top() - Vec2::new(0.0, 6.0), bar.width(), look)
        {
            crate::app::chat::commands::complete_into(ui.ctx(), &mut draft, &name, draft_id);
        }

        if sent && !swallow {
            self.send_from_box(ui.ctx(), &row.id, std::mem::take(&mut draft).trim().to_string());
            edit.request_focus();
        }

        if self.chat_draft.is_empty() {
            self.chat_draft = draft;
        }
    }

    /// Who the conversation is with, in `room` of the name row: the Maestro with its crown as Luna, an agent by its own
    /// name; after it, dimmer, the Maestro's chat title and what it does now. The name is cut only when nothing else fits.
    fn island_who(&self, painter: &Painter, row: &AgentRow, room: Rect) {
        let chat = self.chats.iter().find(|chat| chat.id == row.id);
        let maestro = chat.is_some_and(|chat| !chat.recruit);

        let (who, about) = match chat {
            Some(_) if maestro => ("Luna".to_string(), format!("{} · {}", self.chat_short_name(row), self.list_note(row, None))),
            Some(chat) => (chat.name.clone(), self.list_note(row, None)),
            None => (self.chat_short_name(row), self.list_note(row, None)),
        };

        let crown = if maestro { 18.0 } else { 0.0 };
        let name = clipped(painter, who, font(13.5), INK, (room.width() - crown).max(0.0));
        let cy = room.center().y;

        if maestro {
            crate::app::ui::maestro_crown(painter, Pos2::new(room.left() + 7.0, cy + 0.5), 0.8, AMBER);
        }

        let left = room.left() + crown;
        let after = left + name.size().x + 8.0;

        painter.galley(Pos2::new(left, cy - name.size().y / 2.0), name, INK);

        if room.right() - after > 30.0 {
            let about = clipped(painter, about, font(11.5), DIM_3, room.right() - after);

            painter.galley(Pos2::new(after, cy - about.size().y / 2.0), about, DIM_3);
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
