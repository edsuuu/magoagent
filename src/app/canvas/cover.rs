//! The windows' live parts stack as the windows do. A file tree's rows and panels and a chat card's
//! conversation are egui layers over the canvas, and egui stacks layers by when they were made or
//! last clicked, not by the drawing's order: a tree behind would show its panel over the tree in
//! front, and anything over a terminal painted on the canvas. So every frame the live layers go
//! under one anchor (`set_sublayer`), below the toolbar, in the drawing's order; and where a window
//! is in front of another's live parts, its picture is painted again over them (a cover, on an area
//! that takes the pointer from them; a press there is the canvas's, on the window in front, `on_canvas`).

use crate::prelude::*;

/// The anchor the live layers stack on, shown before the toolbar (update.rs) so they stay under it.
pub(crate) fn anchor() -> egui::LayerId {
    egui::LayerId::new(egui::Order::Middle, egui::Id::unique("node_layers"))
}

/// The layers of an element's live parts: a tree's (filetree/ui) and a chat card's (chat/node_ui.rs,
/// its mini chat button; a recruit's card and fold button).
fn live_layers(e: &Element) -> Vec<egui::LayerId> {
    let layer = |key: &str| egui::LayerId::new(egui::Order::Middle, egui::Id::unique((key, e.id.as_str())));
    if e.filetree.is_some() {
        ["ft_nav", "ft_tools", "ft_body", "ft_changes", "ft_db", "ft_pane"].into_iter().map(layer).collect()
    } else if e.chat.is_some() {
        ["chat_card", "recruit_card", "chat_card_mini", "recruit_fold"].into_iter().map(layer).collect()
    } else {
        Vec::new()
    }
}

/// What goes to the top this frame for `now` (egui's order, bottom to top) to read as `want`: egui
/// keeps the order of what it moves up, so with the first out of place found, all after it go up
/// and it stays; a place more is right each frame. Empty: in order.
fn to_raise(want: &[egui::LayerId], now: &[egui::LayerId]) -> Vec<egui::LayerId> {
    let now: Vec<egui::LayerId> = now.iter().filter(|l| want.contains(l)).copied().collect();
    // one not made yet (a cover new this frame) has no place to be out of
    let want: Vec<egui::LayerId> = want.iter().filter(|l| now.contains(l)).copied().collect();
    match want.iter().zip(&now).position(|(a, b)| a != b) {
        Some(first) => want[first + 1..].to_vec(),
        None => Vec::new(),
    }
}

impl DrawingApp {
    /// A press at `pos` belongs to the canvas: nothing over it, or only a window's cover.
    pub(crate) fn on_canvas(&self, ctx: &Context, pos: Pos2, canvas: egui::LayerId) -> bool {
        ctx.layer_id_at(pos).is_some_and(|l| l == canvas || self.covers.contains(&l))
    }

    /// Every frame, after the live parts: their layers in the drawing's order, and the windows in
    /// front of them painted again over them.
    pub(crate) fn cover_live_parts(&mut self, ctx: &Context) {
        self.covers.clear();
        if self.chat_open || self.terminal_elevated.is_some() {
            return;
        }
        let canvas = self.canvas_rect;
        // the windows as the canvas paints them: in the drawing's order, over the drawings
        let nodes: Vec<usize> = (0..self.scene.elements.len()).filter(|&i| self.scene.elements[i].is_node()).collect();
        let (mut want, mut covers) = (Vec::new(), Vec::new());
        for (k, &n) in nodes.iter().enumerate() {
            let lower = &self.scene.elements[n];
            let layers: Vec<egui::LayerId> = live_layers(lower).into_iter().filter(|l| ctx.memory(|m| m.areas().is_visible(l))).collect();
            if layers.is_empty() {
                continue;
            }
            want.extend(&layers);
            let frame = self.terminal_frame(lower).0;
            for &m in &nodes[k + 1..] {
                let front = &self.scene.elements[m];
                let (front_frame, z) = self.terminal_frame(front);
                let region = front_frame.intersect(frame).intersect(canvas);
                if front.portal.is_some() || !region.is_positive() {
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
                        let p = ui.painter();
                        if front.terminal.is_some() {
                            self.paint_terminal(p, front, front_frame, z);
                        } else if front.filetree.is_some() {
                            self.paint_filetree(p, front, front_frame, z);
                        } else if front.chat.is_some() {
                            self.paint_chat_node(p, front, front_frame, z);
                        }
                    });
                want.push(area.response.layer_id);
                covers.push(area.response.layer_id);
            }
        }
        for &layer in &want {
            ctx.set_sublayer(anchor(), layer);
        }
        let raise = to_raise(&want, &ctx.memory(|m| m.layer_ids().collect::<Vec<_>>()));
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
        let l = |n: u64| egui::LayerId::new(egui::Order::Middle, egui::Id::unique(n));
        let want = [l(1), l(2), l(3)];
        assert!(to_raise(&want, &[l(9), l(1), l(2), l(8), l(3)]).is_empty(), "others between do not count");
        assert!(to_raise(&[l(1), l(4), l(2)], &[l(1), l(2)]).is_empty(), "one not made yet is no reason to move");
        // egui's step: what is raised goes on top, in the order it had
        let step = |now: Vec<egui::LayerId>| {
            let up = to_raise(&want, &now);
            let mut next: Vec<_> = now.iter().filter(|x| !up.contains(x)).copied().collect();
            next.extend(now.iter().filter(|x| up.contains(x)));
            next
        };
        let mut now = vec![l(3), l(2), l(1)];
        for _ in 0..3 {
            now = step(now);
        }
        assert_eq!(now, want);
    }
}
