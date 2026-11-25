//! The chat's card on the canvas, live: its conversation and the box to write in, as the mini chat
//! draws them, scaled with the canvas. Too small on screen, the card stays the picture
//! `paint_chat_node` draws (the mascot and the last message).

use crate::app::chat::composer::Hints;
use crate::app::chat::follow::{follow_scroll, talk_mark};
use crate::app::chat::list::Look;
use crate::app::chat::mini::ConvOut;
use crate::app::ui::agents::AgentState;
use crate::prelude::*;

/// The card's own size from which it shows its conversation, not the picture: room for the band and the box with a few
/// lines of conversation above them (smaller, the box took it all, or went past the card's edge).
pub(crate) const CHAT_LIVE_MIN: Vec2 = Vec2::new(220.0, 260.0);
/// A recruit's card has no box nor folder strip: its band and a few lines of its work fit in less.
pub(crate) const RECRUIT_LIVE_MIN: Vec2 = Vec2::new(220.0, 200.0);

/// Big enough to read and write in (else the picture): the card's own size, so zooming in never turns a
/// picture into the chat; too far out, it is the picture.
pub(crate) fn chat_live(rect: Rect, zoom: f32, recruit: bool) -> bool {
    let least = if recruit { RECRUIT_LIVE_MIN } else { CHAT_LIVE_MIN };

    zoom >= 0.45 && rect.width() / zoom >= least.x && rect.height() / zoom >= least.y
}

/// Where a card keeps what is pinned to its bottom edge as it was last laid out (in the card's own points).
pub(crate) fn pinned_key(id: &str) -> egui::Id {
    egui::Id::unique(("chat_card_pinned", id))
}

/// A card's inside for this frame: what scrolls on top, and under it, `gap` away, what is pinned to the bottom edge (the
/// band, the box) at the height it took last frame (`guess` the first time). Neither spills into the other: however much
/// the conversation holds, it scrolls in what is left.
pub(crate) fn split_body(ctx: &Context, key: egui::Id, inner: Rect, guess: f32, gap: f32) -> (Rect, Rect) {
    let pinned = ctx.data_mut(|d| d.get_temp::<Rect>(key)).map_or(guess, |r| r.height()).min(inner.height());
    let bottom = Rect::from_min_max(Pos2::new(inner.left(), inner.bottom() - pinned), inner.max);

    (Rect::from_min_max(inner.min, Pos2::new(inner.right(), (bottom.top() - gap).max(inner.top()))), bottom)
}

/// The pinned part as it was laid out, for the next frame (one frame more when its height changed).
pub(crate) fn keep_pinned(ctx: &Context, key: egui::Id, laid: Rect) {
    let was = ctx.data_mut(|d| d.get_temp::<Rect>(key));

    if was != Some(laid) {
        ctx.data_mut(|d| d.insert_temp(key, laid));

        if was.is_none_or(|was| (was.height() - laid.height()).abs() > 0.5) {
            ctx.request_repaint();
        }
    }
}

/// The conversation's scroll: what is left above the pinned part, never more (egui's own floor would push the
/// pinned part off the card); `follow_scroll` shows it, following the newest.
pub(crate) fn card_scroll(salt: (&str, &str), height: f32) -> egui::ScrollArea {
    egui::ScrollArea::vertical().id_salt(salt).auto_shrink(false).min_scrolled_height(0.0).max_height(height)
}

/// The part of the card under its header and over its folder strip, on screen.
pub(crate) fn chat_card_body(rect: Rect, zoom: f32) -> Rect {
    Rect::from_min_max(Pos2::new(rect.left() + 1.0, rect.top() + TERMINAL_HEADER * zoom), Pos2::new(rect.right() - 1.0, rect.bottom() - TERMINAL_FOOTER * zoom))
}

/// The card's button that opens its chat in the mini chat: in its header, left of the dot.
pub(crate) fn mini_button(rect: Rect, zoom: f32) -> Rect {
    let y = rect.top() + TERMINAL_HEADER * zoom / 2.0;

    Rect::from_center_size(Pos2::new(rect.right() - 8.125 * zoom - 6.5 * zoom - 12.0 * zoom, y), Vec2::splat(16.0 * zoom))
}

impl DrawingApp {
    /// Every frame: each chat card big enough on screen gets its conversation and its box.
    pub(crate) fn chat_node_overlay(&mut self, ctx: &Context) {
        let canvas = self.canvas_rect;

        let cards: Vec<(String, Rect, f32)> = self
            .scene
            .elements
            .iter()
            .filter(|e| e.chat.as_ref().is_some_and(|c| c.folded.is_none()) && self.is_chat(&e.id) && !self.behind_blur(&e.id))
            .map(|e| {
                let (frame, z) = self.terminal_frame(e);

                (e.id.clone(), frame, z)
            })
            .filter(|(id, frame, z)| chat_live(*frame, *z, self.is_recruit_chat(id)) && frame.intersects(canvas))
            .collect();

        for (id, frame, z) in cards {
            if self.is_recruit_chat(&id) {
                // a recruit's card has no folder strip: its work goes down to the bottom
                let body = chat_card_body(frame, z).with_max_y(frame.bottom() - 1.0);

                self.recruit_card_ui(ctx, &id, body, z, canvas);
            } else {
                self.chat_card_ui(ctx, &id, chat_card_body(frame, z), z, canvas);
            }
        }

        // on every card big enough to hit it: the card in full screen over the canvas (the terminals' Ampliar), or back
        let buttons: Vec<(String, Rect, f32)> = self
            .scene
            .elements
            .iter()
            .filter(|e| e.chat.is_some() && !self.behind_blur(&e.id))
            .map(|e| {
                let (frame, z) = self.terminal_frame(e);

                (e.id.clone(), mini_button(frame, z), z)
            })
            .filter(|(_, button, z)| *z >= 0.45 && canvas.contains_rect(*button))
            .collect();

        for (id, button, z) in buttons {
            let enlarged = self.terminal_elevated.as_deref() == Some(id.as_str());

            let clicked = egui::Area::new(egui::Id::unique(("chat_card_mini", id.as_str())))
                .fixed_pos(button.min)
                .constrain(false)
                .order(egui::Order::Middle)
                .show(ctx, |ui| {
                    let (rect, r) = ui.allocate_exact_size(button.size(), Sense::click());
                    let r = r.on_hover_cursor(CursorIcon::PointingHand).tip(if enlarged { "Voltar ao canvas" } else { "Tela cheia no canvas" });

                    if r.hovered() {
                        ui.painter().rect_filled(rect, 4.0 * z, Color32::from_white_alpha(26));
                    }

                    crate::app::ui::icons::paint_icon_scaled(
                        ui.painter(),
                        crate::app::ui::icons::Icon::Expand,
                        rect.center(),
                        Color32::from_gray(0xb0),
                        0.7 * z,
                        0.0,
                    );

                    r.clicked()
                })
                .inner;

            if clicked && let Some(index) = self.scene.elements.iter().position(|element| element.id == id) {
                self.toggle_lift(index);
            }
        }

        self.recruit_fold_buttons(ctx, canvas);
    }

    /// The chat whose card's box on the canvas has the keys: a picture pasted goes into it.
    pub(crate) fn focused_chat_card(&self, ctx: &Context) -> Option<String> {
        self.chats.iter().map(|chat| &chat.id).find(|id| ctx.memory(|m| m.has_focus(card_box_id(id)))).cloned()
    }

    /// One card: drawn at the canvas's scale by the layer's transform, laid out in the card's own size.
    fn chat_card_ui(&mut self, ctx: &Context, id: &str, body: Rect, z: f32, canvas: Rect) {
        let Some(row) = self.chat_rows().into_iter().find(|r| r.id == id) else { return };
        let (c, dark) = (self.colors(), self.dark());
        let look = Look::new(dark, &c);
        let time = ctx.input(|i| i.time);
        // the card's own box and chat for this frame: what the box and its chips act on
        let draft_key = egui::Id::unique(("chat_card_draft", id));
        let draft_id = card_box_id(id);
        let card_draft = ctx.data_mut(|d| d.get_temp::<String>(draft_key)).unwrap_or_default();
        let outer_agent = self.chat_agent.replace(id.to_string());
        let outer_draft = std::mem::replace(&mut self.chat_draft, card_draft);
        // its box has the / menu too, while it has the keys (the canvas has many cards; one box types at a time)
        let typing = ctx.memory(|m| m.has_focus(draft_id) || m.had_focus_last_frame(draft_id));
        let slash = if typing { self.slash_entries() } else { None };
        let mut draft = std::mem::take(&mut self.chat_draft);
        let swallow = slash.as_ref().is_some_and(|entries| self.slash_keys(ctx, &mut draft, entries, draft_id));
        let size = body.size() / z;
        let clip = body.intersect(canvas);
        let clip = Rect::from_min_max(((clip.min - body.min) / z).to_pos2(), ((clip.max - body.min) / z).to_pos2());
        let transcript = self.transcripts.get(id);
        let talks = transcript.is_some_and(|t| !t.items.is_empty());
        let asked = self.asked_of();
        let hint = format!("Mensagem para {}", self.maestro_name(&row.spec.name));
        let mut conv = ConvOut::default();
        let (mut send, mut interrupt, mut attach, mut footer_act) = (None, false, false, None);
        let mut box_rect = None;

        let area = egui::Area::new(egui::Id::unique(("chat_card", id))).fixed_pos(Pos2::ZERO).constrain(false).order(egui::Order::Middle).show(ctx, |ui| {
            ui.set_clip_rect(clip);
            let rect = Rect::from_min_size(Pos2::ZERO, size);

            // the whole body is the card's: a click in it never reaches the canvas
            ui.allocate_rect(rect, Sense::click());
            let inner = rect.shrink2(Vec2::new(10.0, 6.0));
            let width = inner.width() - 4.0;
            // the band and the box pinned to the bottom edge, the conversation scrolling in what is left above them
            let key = pinned_key(id);
            let (top, bottom) = split_body(ui.ctx(), key, inner, 92.0, 6.0);

            ui.scope_builder(egui::UiBuilder::new().max_rect(top), |ui| {
                ui.set_max_width(width);
                ui.spacing_mut().scroll = crate::app::ui::scroll_style();

                follow_scroll(ui, card_scroll(("chat_card_scroll", id), top.height()), id, "chat_card", talk_mark(transcript), |ui| {
                    self.conversation(ui, &row, transcript, talks, true, false, &asked, None, width, &look, &c, true, time, &mut conv);

                    // what the agent asks the person ends the conversation: however tall, it scrolls with it
                    if self.person_asked(&row.id) {
                        self.decide_card(ui, &row.id, &look, time);
                        ui.add_space(6.0);
                    }
                });
            });

            let pinned = ui.scope_builder(egui::UiBuilder::new().max_rect(bottom), |ui| {
                ui.set_max_width(width);

                if matches!(row.state, AgentState::Working(_)) {
                    interrupt |= self.work_band(ui, &row, transcript, &look, true, time);
                    ui.add_space(6.0);
                }

                let card = self.composer(ui, &mut draft, draft_id, &hint, width, 120f32.min(size.y * 0.3), swallow, &look, &c, Hints::None, time);

                (attach, footer_act, box_rect) = (card.attach, card.footer, Some(card.rect));

                if card.send {
                    send = Some(std::mem::take(&mut draft).trim_end().to_string());
                }
            });

            keep_pinned(ui.ctx(), key, pinned.response.rect);
        });

        // laid out at the card's size, shown at the canvas's
        ctx.set_transform_layer(area.response.layer_id, egui::emath::TSTransform::new(body.min.to_vec2(), z));

        // the menu over the box where the canvas shows it, at the interface's own size
        if let (Some(entries), Some(rect)) = (&slash, box_rect) {
            let anchor = body.min + rect.left_top().to_vec2() * z - Vec2::new(0.0, 6.0);

            if let Some(name) = self.slash_menu_ui(ctx, entries, anchor, (rect.width() * z).max(260.0), &look) {
                crate::app::chat::commands::complete_into(ctx, &mut draft, &name, draft_id);
            }
        }

        self.chat_draft = draft;
        // a run or a step opened or shut: the row clicked stays where it is (`conv_apply` holds the scroll)
        self.conv_apply(ctx, conv, draft_id);

        if attach {
            self.chat_attach_pick(ctx);
        }

        if interrupt {
            self.interrupt_agent();
        }

        if let Some(act) = footer_act {
            self.chat_footer_act(act);
        }

        if let Some(text) = send {
            self.send_from_box(ctx, id, text);
        }

        // back to the chat screen's own box and chat
        let card_draft = std::mem::replace(&mut self.chat_draft, outer_draft);

        self.chat_agent = outer_agent;
        ctx.data_mut(|d| d.insert_temp(draft_key, card_draft));
    }
}

/// The id of chat `id`'s box on its card.
pub(crate) fn card_box_id(id: &str) -> egui::Id {
    egui::Id::unique(("chat_card_box", id))
}
