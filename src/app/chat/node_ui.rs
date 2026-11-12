//! The chat's card on the canvas, live: its conversation and the box to write in, as the mini chat
//! draws them, scaled with the canvas. Too small on screen, the card stays the picture
//! `paint_chat_node` draws (the mascot and the last message).

use crate::app::chat::composer::Hints;
use crate::app::chat::list::Look;
use crate::app::chat::mini::ConvOut;
use crate::app::ui::agents::AgentState;
use crate::prelude::*;

/// Big enough on screen to read and write in (else the picture).
pub(crate) fn chat_live(rect: Rect, zoom: f32) -> bool {
    zoom >= 0.45 && rect.width() >= 220.0 && rect.height() >= 160.0
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
            .filter(|e| e.chat.is_some() && self.is_chat(&e.id))
            .map(|e| {
                let (frame, z) = self.terminal_frame(e);
                (e.id.clone(), frame, z)
            })
            .filter(|(_, frame, z)| chat_live(*frame, *z) && frame.intersects(canvas))
            .collect();
        for (id, frame, z) in cards {
            self.chat_card_ui(ctx, &id, chat_card_body(frame, z), z, canvas);
        }
        // on every card big enough to hit it: its chat in the mini chat
        let buttons: Vec<(String, Rect, f32)> = self
            .scene
            .elements
            .iter()
            .filter(|e| e.chat.is_some())
            .map(|e| {
                let (frame, z) = self.terminal_frame(e);
                (e.id.clone(), mini_button(frame, z), z)
            })
            .filter(|(_, button, z)| *z >= 0.45 && canvas.contains_rect(*button))
            .collect();
        for (id, button, z) in buttons {
            let clicked = egui::Area::new(egui::Id::new(("chat_card_mini", id.as_str())))
                .fixed_pos(button.min)
                .constrain(false)
                .order(egui::Order::Middle)
                .show(ctx, |ui| {
                    let (rect, r) = ui.allocate_exact_size(button.size(), Sense::click());
                    let r = r.on_hover_cursor(CursorIcon::PointingHand).tip("Abrir no mini chat");
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
            if clicked {
                self.open_mini_chat(&id);
            }
        }
    }

    /// One card: drawn at the canvas's scale by the layer's transform, laid out in the card's own size.
    fn chat_card_ui(&mut self, ctx: &Context, id: &str, body: Rect, z: f32, canvas: Rect) {
        let Some(row) = self.chat_rows().into_iter().find(|r| r.id == id) else { return };
        let (c, dark) = (self.colors(), self.dark());
        let look = Look::new(dark, &c);
        let time = ctx.input(|i| i.time);
        // the card's own box and chat for this frame: what the box and its chips act on
        let draft_key = egui::Id::new(("chat_card_draft", id));
        let draft_id = egui::Id::new(("chat_card_box", id));
        let card_draft = ctx.data_mut(|d| d.get_temp::<String>(draft_key)).unwrap_or_default();
        let outer_agent = self.chat_agent.replace(id.to_string());
        let outer_draft = std::mem::replace(&mut self.chat_draft, card_draft);
        let size = body.size() / z;
        let clip = body.intersect(canvas);
        let clip = Rect::from_min_max(((clip.min - body.min) / z).to_pos2(), ((clip.max - body.min) / z).to_pos2());
        let transcript = self.transcripts.get(id);
        let talks = transcript.is_some_and(|t| !t.items.is_empty());
        let asked = self.asked_of();
        let hint = format!("Mensagem para {}", self.maestro_name(&row.spec.name));
        let mut draft = std::mem::take(&mut self.chat_draft);
        let mut conv = ConvOut::default();
        let (mut send, mut interrupt, mut attach, mut footer_act) = (None, false, false, None);
        let area = egui::Area::new(egui::Id::new(("chat_card", id))).fixed_pos(Pos2::ZERO).constrain(false).order(egui::Order::Middle).show(ctx, |ui| {
            ui.set_clip_rect(clip);
            let rect = Rect::from_min_size(Pos2::ZERO, size);
            // the whole body is the card's: a click in it never reaches the canvas
            ui.allocate_rect(rect, Sense::click());
            let inner = rect.shrink2(Vec2::new(10.0, 6.0));
            let width = inner.width() - 4.0;
            ui.scope_builder(egui::UiBuilder::new().max_rect(inner), |ui| {
                ui.set_max_width(width);
                let footer_key = egui::Id::new(("chat_card_footer", id));
                let footer_height = ui.ctx().data_mut(|d| d.get_temp::<f32>(footer_key)).unwrap_or(70.0);
                ui.spacing_mut().scroll = egui::style::ScrollStyle::floating();
                egui::ScrollArea::vertical()
                    .id_salt(("chat_card_scroll", id))
                    .stick_to_bottom(true)
                    .auto_shrink(false)
                    .max_height((ui.available_height() - footer_height).max(0.0))
                    .show(ui, |ui| self.conversation(ui, &row, transcript, talks, true, false, &asked, None, width, &look, &c, true, time, &mut conv));
                let top = ui.cursor().top();
                ui.add_space(6.0);
                if matches!(row.state, AgentState::Working(_)) {
                    interrupt |= self.work_band(ui, &row, transcript, &look, true, time);
                    ui.add_space(6.0);
                }
                let card = self.composer(ui, &mut draft, draft_id, &hint, width, 120f32.min(size.y * 0.3), false, &look, &c, Hints::None, time);
                (attach, footer_act) = (card.attach, card.footer);
                if card.send {
                    send = Some(std::mem::take(&mut draft).trim_end().to_string());
                }
                let footer = ui.cursor().top() - top;
                if (footer - footer_height).abs() > 0.5 {
                    ui.ctx().data_mut(|d| d.insert_temp(footer_key, footer));
                    ui.ctx().request_repaint();
                }
            });
        });
        // laid out at the card's size, shown at the canvas's
        ctx.set_transform_layer(area.response.layer_id, egui::emath::TSTransform::new(body.min.to_vec2(), z));
        self.chat_draft = draft;
        self.conv_apply(ctx, conv, draft_id);
        if attach && let Some(paths) = rfd::FileDialog::new().set_title("Anexar imagem").add_filter("Imagens", &IMAGE_EXTENSIONS).pick_files() {
            self.chat_add_files(ctx, paths, false);
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
