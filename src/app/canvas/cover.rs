//! The cards' live parts stack as the cards do. A file tree's rows and panels, a chat card's
//! conversation, a portal's bar and the grid of the terminal with the keyboard are egui layers over
//! the canvas, and egui stacks layers by when they were made or last clicked, not by the drawing's
//! order: a tree behind would show its panel over the tree in front, and a portal's bar over a note
//! or a terminal in front of it. So every frame the live layers go under one anchor
//! (`set_sublayer`), below the toolbar, in the drawing's order; and where a card (a window or a
//! note) is in front of another's live parts, its picture is painted again over them (a cover, on an
//! area that takes the pointer from them; a press there is the canvas's, on the card in front,
//! `on_canvas`). A portal's page, native over everything, leaves a hole where a card is in front of
//! it (browser/sync.rs).

use crate::prelude::*;

/// The anchor the live layers stack on, shown before the toolbar (update.rs) so they stay under it.
pub(crate) fn anchor() -> egui::LayerId {
    egui::LayerId::new(egui::Order::Middle, egui::Id::unique("node_layers"))
}

/// What goes to the top this frame for `now` (egui's order, bottom to top) to read as `want`: egui
/// keeps the order of what it moves up, so with the first out of place found, all after it go up
/// and it stays; a place more is right each frame. Empty: in order.
fn to_raise(want: &[egui::LayerId], now: &[egui::LayerId]) -> Vec<egui::LayerId> {
    let now: Vec<egui::LayerId> = now.iter().filter(|layer| want.contains(layer)).copied().collect();
    // one not made yet (a cover new this frame) has no place to be out of
    let want: Vec<egui::LayerId> = want.iter().filter(|layer| now.contains(layer)).copied().collect();
    match want.iter().zip(&now).position(|(wanted, present)| wanted != present) {
        Some(first) => want[first + 1..].to_vec(),
        None => Vec::new(),
    }
}

impl DrawingApp {
    /// The layers of a card's live parts: a tree's (filetree/ui), a chat card's (chat/node_ui.rs, its mini chat button;
    /// a recruit's card and fold button), a portal's bar (browser/chrome.rs) and the grid of the terminal that has the
    /// keyboard (terminal/overlay.rs).
    pub(crate) fn live_layers(&self, element: &Element) -> Vec<egui::LayerId> {
        let layer = |key: &str| egui::LayerId::new(egui::Order::Middle, egui::Id::unique((key, element.id.as_str())));
        if element.filetree.is_some() {
            ["ft_nav", "ft_tools", "ft_body", "ft_changes", "ft_db", "ft_pane"].into_iter().map(layer).collect()
        } else if element.chat.is_some() {
            ["chat_card", "recruit_card", "chat_card_mini", "recruit_fold"].into_iter().map(layer).collect()
        } else if element.portal.is_some() {
            vec![layer("portal_chrome")]
        } else if element.terminal.is_some() && self.terminal_focus.as_ref() == Some(&element.id) {
            vec![egui::LayerId::new(egui::Order::Middle, egui::Id::unique("terminal_overlay"))]
        } else {
            Vec::new()
        }
    }

    /// A press on a card's live parts brings the card to the front, as a press on the card itself does on the canvas
    /// (`raise_card`): what is pressed is the card on top there.
    fn raise_pressed_card(&mut self, ctx: &Context) {
        let Some(layer) = ctx.input(|i| i.pointer.primary_pressed().then(|| i.pointer.interact_pos()).flatten()).and_then(|at| ctx.layer_id_at(at)) else {
            return;
        };
        if let Some(index) = (0..self.scene.elements.len()).find(|&index| self.live_layers(&self.scene.elements[index]).contains(&layer)) {
            self.raise_card(index);
        }
    }

    /// A press at `pos` belongs to the canvas: nothing over it, or only a window's cover.
    pub(crate) fn on_canvas(&self, ctx: &Context, pos: Pos2, canvas: egui::LayerId) -> bool {
        ctx.layer_id_at(pos).is_some_and(|layer| layer == canvas || self.covers.contains(&layer))
    }

    /// Every frame, after the live parts: their layers in the drawing's order, and the cards in
    /// front of them painted again over them.
    pub(crate) fn cover_live_parts(&mut self, ctx: &Context) {
        self.covers_before = std::mem::take(&mut self.covers);
        if self.chat_open || self.terminal_elevated.is_some() {
            return;
        }
        self.raise_pressed_card(ctx);
        let canvas = self.canvas_rect;
        // the cards as the canvas paints them: in the drawing's order, over the drawings (a pile's waiting pages and a
        // note's text paint with their card)
        let elements = &self.scene.elements;
        let cards: Vec<usize> = (0..elements.len()).filter(|&index| elements[index].is_card() && !hidden_page(elements, &elements[index])).collect();
        let (mut want, mut covers) = (Vec::new(), Vec::new());
        for (position, &card) in cards.iter().enumerate() {
            let lower = &self.scene.elements[card];
            let layers: Vec<egui::LayerId> =
                self.live_layers(lower).into_iter().filter(|layer| ctx.memory(|memory| memory.areas().is_visible(layer))).collect();
            if layers.is_empty() {
                continue;
            }
            want.extend(&layers);
            let frame = self.terminal_frame(lower).0;
            for &front_card in &cards[position + 1..] {
                let front = &self.scene.elements[front_card];
                let (front_frame, depth) = self.terminal_frame(front);
                let region = front_frame.intersect(frame).intersect(canvas);
                if !region.is_positive() {
                    continue;
                }
                let area = egui::Area::new(egui::Id::unique(("node_cover", lower.id.as_str(), front.id.as_str())))
                    .fixed_pos(region.min)
                    .constrain(false)
                    .order(egui::Order::Middle)
                    .show(ctx, |ui| {
                        ui.set_clip_rect(region);
                        // takes the pointer from the live parts under it
                        ui.allocate_exact_size(region.size(), Sense::hover());
                        let painter = ui.painter();
                        if front.terminal.is_some() {
                            self.paint_terminal(painter, front, front_frame, depth);
                        } else if front.filetree.is_some() {
                            self.paint_filetree(painter, front, front_frame, depth);
                        } else if front.chat.is_some() {
                            self.paint_chat_node(painter, front, front_frame, depth);
                        } else if front.portal.is_some() {
                            self.paint_portal(painter, front);
                        } else {
                            self.paint_note_card(painter, front_card);
                        }
                    });
                want.push(area.response.layer_id);
                covers.push(area.response.layer_id);
            }
        }
        for &layer in &want {
            ctx.set_sublayer(anchor(), layer);
        }
        let raise = to_raise(&want, &ctx.memory(|memory| memory.layer_ids().collect::<Vec<_>>()));
        for &layer in &raise {
            ctx.move_to_top(layer);
        }
        if !raise.is_empty() {
            ctx.request_repaint();
        }
        self.covers = covers;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn layers_out_of_the_drawings_order_are_raised_until_in_it() {
        let layer = |number: u64| egui::LayerId::new(egui::Order::Middle, egui::Id::unique(number));
        let want = [layer(1), layer(2), layer(3)];
        assert!(to_raise(&want, &[layer(9), layer(1), layer(2), layer(8), layer(3)]).is_empty(), "others between do not count");
        assert!(to_raise(&[layer(1), layer(4), layer(2)], &[layer(1), layer(2)]).is_empty(), "one not made yet is no reason to move");
        // egui's step: what is raised goes on top, in the order it had
        let step = |now: Vec<egui::LayerId>| {
            let up = to_raise(&want, &now);
            let mut next: Vec<_> = now.iter().filter(|other| !up.contains(other)).copied().collect();
            next.extend(now.iter().filter(|other| up.contains(other)));
            next
        };
        let mut now = vec![layer(3), layer(2), layer(1)];
        for _ in 0..3 {
            now = step(now);
        }
        assert_eq!(now, want);
    }
}
