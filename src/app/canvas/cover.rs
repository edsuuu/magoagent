//! A window in front stays in front of the live parts too. A file tree's rows and panels and a chat
//! card's conversation are egui layers over the canvas, so a terminal (or a chat card's picture)
//! painted on the canvas in front of them would show them through. Where one is in front, its
//! picture is painted again right over each of those layers (`set_sublayer`), on an area that takes
//! the pointer from them; a press there is the canvas's, on the window in front (`on_canvas`).

use crate::app::chat::node_ui::chat_live;
use crate::prelude::*;

/// The layers of an element's live parts: a tree's (filetree/ui) and a chat card's (chat/node_ui.rs).
fn live_layers(e: &Element, recruit: bool) -> Vec<egui::LayerId> {
    let layer = |key: &str| egui::LayerId::new(egui::Order::Middle, egui::Id::unique((key, e.id.as_str())));
    if e.filetree.is_some() {
        ["ft_nav", "ft_tools", "ft_body", "ft_changes", "ft_db", "ft_pane"].into_iter().map(layer).collect()
    } else if e.chat.as_ref().is_some_and(|c| c.folded.is_none()) {
        vec![layer(if recruit { "recruit_card" } else { "chat_card" })]
    } else {
        Vec::new()
    }
}

impl DrawingApp {
    /// A press at `pos` belongs to the canvas: nothing over it, or only a window's cover.
    pub(crate) fn on_canvas(&self, ctx: &Context, pos: Pos2, canvas: egui::LayerId) -> bool {
        ctx.layer_id_at(pos).is_some_and(|l| l == canvas || self.covers.contains(&l))
    }

    /// Every frame, after the live parts: the windows in front of them, painted again over them.
    pub(crate) fn cover_live_parts(&mut self, ctx: &Context) {
        self.covers.clear();
        if self.chat_open || self.terminal_elevated.is_some() {
            return;
        }
        let canvas = self.canvas_rect;
        // the windows as the canvas paints them: in the drawing's order, over the drawings
        let nodes: Vec<usize> = (0..self.scene.elements.len()).filter(|&i| self.scene.elements[i].is_node()).collect();
        let mut covers = Vec::new();
        for (k, &n) in nodes.iter().enumerate() {
            let lower = &self.scene.elements[n];
            let layers: Vec<egui::LayerId> =
                live_layers(lower, self.is_recruit_chat(&lower.id)).into_iter().filter(|l| ctx.memory(|m| m.areas().is_visible(l))).collect();
            if layers.is_empty() {
                continue;
            }
            let frame = self.terminal_frame(lower).0;
            for &m in &nodes[k + 1..] {
                let front = &self.scene.elements[m];
                let (front_frame, z) = self.terminal_frame(front);
                // only what the canvas paints: a tree or a live card in front has live parts of its own
                let painted = front.terminal.is_some() || front.chat.as_ref().is_some_and(|c| c.folded.is_some() || !chat_live(front_frame, z));
                let region = front_frame.intersect(frame).intersect(canvas);
                if !painted || !region.is_positive() {
                    continue;
                }
                for &layer in &layers {
                    let area = egui::Area::new(egui::Id::unique(("node_cover", layer.id, front.id.as_str())))
                        .fixed_pos(region.min)
                        .constrain(false)
                        .order(egui::Order::Middle)
                        .show(ctx, |ui| {
                            ui.set_clip_rect(region);
                            // takes the pointer from the live parts under it
                            ui.allocate_exact_size(region.size(), Sense::hover());
                            if front.terminal.is_some() {
                                self.paint_terminal(ui.painter(), front, front_frame, z);
                            } else {
                                self.paint_chat_node(ui.painter(), front, front_frame, z);
                            }
                        });
                    ctx.set_sublayer(layer, area.response.layer_id);
                    covers.push(area.response.layer_id);
                }
            }
        }
        self.covers = covers;
    }
}
