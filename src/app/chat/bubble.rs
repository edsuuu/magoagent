//! The person's words in a bubble at the conversation's right: a message sent (chat/mod.rs `chat_item`) or queued
//! (chat/queue.rs), in every view of a chat (the chat screen, the mini chat, the card on the canvas, the island).

use crate::prelude::*;

/// `text` in a bubble at the right of what is left of the row, `fill`ed, never wider than `widest` (its margins in) nor
/// than that room, its lines starting at its left. The text is laid out here at that width: a label in a right-to-left
/// row neither wraps nor starts its lines at the left, and the row's `set_max_width` grows to the left, past the chat's
/// edge, when asked more than it has (the mini chat cut a queued message off at its left).
pub(crate) fn person_bubble(ui: &mut egui::Ui, text: egui::RichText, widest: f32, fill: Color32, corner: u8, margin: egui::Margin) -> egui::Response {
    let inner = (widest.min(ui.available_width()) - margin.sum().x).max(1.0);
    let galley = egui::WidgetText::from(text).into_galley(ui, Some(egui::TextWrapMode::Wrap), inner, egui::FontSelection::Default);

    ui.with_layout(egui::Layout::right_to_left(egui::Align::TOP), |ui| {
        egui::Frame::new().fill(fill).corner_radius(corner).inner_margin(margin).show(ui, |ui| ui.add(egui::Label::new(galley))).response
    })
    .inner
}
