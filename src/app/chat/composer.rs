//! The box to write in, as a card: the words on top and, under them inside it, Anexar, the chat's
//! folder and permission, its model and effort, Ditar, the call and send. Under the card, the keys it
//! takes (or where the call or the dictation stands). The chat screen and the mini chat share it.

use crate::app::chat::footer::FooterAct;
use crate::app::chat::list::{ACCENT, Look, glyph};
use crate::prelude::*;

/// The line under the card: the whole one, a short one (a narrow chat) or none (the mini chat).
#[derive(Clone, Copy, PartialEq)]
pub(crate) enum Hints {
    Full,
    Short,
    None,
}

/// What the card's buttons and keys asked for, applied after the frame.
pub(crate) struct BoxOut {
    /// Enter or the send button, with something to send.
    pub(crate) send: bool,
    pub(crate) attach: bool,
    pub(crate) dictate: bool,
    /// The call's buttons: ligar, mudo, alto-falantes.
    pub(crate) voice: (bool, bool, bool),
    pub(crate) footer: Option<FooterAct>,
    /// The card itself.
    pub(crate) rect: Rect,
}

/// The send arrow, the + of Anexar.
const ARROW: &[&[(f32, f32)]] = &[&[(12.0, 19.0), (12.0, 5.0)], &[(5.0, 12.0), (12.0, 5.0), (19.0, 12.0)]];
const PLUS: &[&[(f32, f32)]] = &[&[(12.0, 5.0), (12.0, 19.0)], &[(5.0, 12.0), (19.0, 12.0)]];

/// A round button of the card, 32 across, filled under the pointer.
fn round(ui: &egui::Ui, center: Pos2, salt: &str, look: &Look) -> egui::Response {
    let rect = Rect::from_center_size(center, Vec2::splat(32.0));
    let response = ui.interact(rect, ui.scope_id().with(salt), Sense::click()).on_hover_cursor(CursorIcon::PointingHand);
    if response.hovered() {
        ui.painter().circle_filled(center, 16.0, look.hover);
    }
    response
}

impl DrawingApp {
    /// The card `width` wide, its text growing up to `cap`; `swallow`: the slash menu took the Enter.
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn composer(
        &self,
        ui: &mut egui::Ui,
        draft: &mut String,
        draft_id: egui::Id,
        hint: &str,
        width: f32,
        cap: f32,
        swallow: bool,
        look: &Look,
        c: &UiColors,
        hints: Hints,
        t: f64,
    ) -> BoxOut {
        let mut out = BoxOut { send: false, attach: false, dictate: false, voice: (false, false, false), footer: None, rect: Rect::NOTHING };
        let mut clicked_send = false;
        let ready = !draft.trim().is_empty() || self.has_attached();
        // 16 + 12 of margins and the hairline's 2
        let inner = width - 30.0;
        let frame = egui::Frame::new()
            .fill(look.panel)
            .stroke(Stroke::new(1.0, look.line))
            .corner_radius(18)
            .inner_margin(egui::Margin { left: 16, right: 12, top: 12, bottom: 10 })
            .show(ui, |ui| {
                ui.set_width(inner);
                ui.spacing_mut().item_spacing.y = 0.0;
                // read before the box takes the key: Response::has_focus is false in the test window,
                // and Shift lives on the key event, not the global modifiers
                let enter = ui.memory(|m| m.has_focus(draft_id))
                    && ui.input(|i| {
                        i.events.iter().any(|e| matches!(e, egui::Event::Key { key: Key::Enter, pressed: true, modifiers, .. } if !modifiers.shift))
                    });
                // the pictures pasted or dropped, above the words
                self.attached_row(ui, c);
                egui::ScrollArea::vertical().id_salt("chat_draft_scroll").auto_shrink([false, true]).max_height(cap).show(ui, |ui| {
                    let edit = egui::TextEdit::multiline(draft)
                        .id(draft_id)
                        .hint_text(egui::RichText::new(hint).color(look.muted))
                        .font(FontId::proportional(14.0))
                        .desired_rows(if hints == Hints::None { 1 } else { 2 })
                        .desired_width(inner)
                        .return_key(Some(egui::KeyboardShortcut::new(Modifiers::SHIFT, Key::Enter)))
                        // Tab never moves focus off the box (it would land on Anexar, and Enter would press it)
                        .lock_focus(true)
                        .frame(egui::Frame::NONE);
                    ui.add(edit);
                });
                ui.add_space(10.0);
                let (row, _) = ui.allocate_exact_size(Vec2::new(inner, 32.0), Sense::hover());
                let y = row.center().y;
                // Anexar
                let plus = round(ui, Pos2::new(row.left() + 16.0, y), "chat_attach", look).tip("Anexar imagem");
                glyph(ui.painter(), Pos2::new(row.left() + 16.0, y), 16.0, 2.0, if plus.hovered() { look.text } else { look.soft }, PLUS);
                out.attach = plus.clicked();
                // send, at the right: the primary color with the ground's arrow, grey with nothing to send
                let at = Pos2::new(row.right() - 16.0, y);
                let send = ui.interact(Rect::from_center_size(at, Vec2::splat(32.0)), ui.scope_id().with("chat_send"), Sense::click());
                let (fill, ink) = if ready { (look.text, look.bg) } else { (look.raised, look.muted) };
                ui.painter().circle_filled(at, 16.0, fill);
                glyph(ui.painter(), at, 16.0, 2.4, ink, ARROW);
                clicked_send = ready && send.on_hover_cursor(CursorIcon::PointingHand).clicked();
                // the call (with its microphone and speakers while it is on), then Ditar
                // the call on hold (voice::CALL_ON): Ditar takes its place
                let call = at - Vec2::new(if crate::app::voice::CALL_ON { 36.0 } else { 0.0 }, 0.0);
                if crate::app::voice::CALL_ON {
                    out.voice = self.voice_buttons(ui, call, look, t);
                }
                let mut left_of = call.x - 16.0 - if self.voice.on { 72.0 } else { 0.0 };
                if !self.voice.on {
                    let mic = call - Vec2::new(36.0, 0.0);
                    let on = self.dictation.is_some();
                    // grey while it starts, blue once it listens
                    let ready = self.dictation.as_ref().is_some_and(|d| d.ready);
                    let r = round(ui, mic, "chat_dictate", look);
                    let r = match (on, ready) {
                        (false, _) => r.tip("Ditar"),
                        (true, true) => r.tip("Só falar agora"),
                        (true, false) => r,
                    };
                    let ink = if on {
                        ui.painter().circle_filled(mic, 16.0, if ready { ACCENT } else { look.raised });
                        if ready { Color32::WHITE } else { look.text }
                    } else if r.hovered() {
                        look.text
                    } else {
                        look.soft
                    };
                    crate::app::voice::paint_mic(ui.painter(), mic, ink, false);
                    out.dictate = r.clicked();
                    left_of = mic.x - 16.0;
                }
                // a chat's own chips between Anexar and the voice
                if let Some(chat) = self.chat_agent.as_ref().and_then(|id| self.chats.iter().find(|c| &c.id == id)) {
                    let chips = Rect::from_min_max(Pos2::new(row.left() + 38.0, y - 14.0), Pos2::new(left_of - 6.0, y + 14.0));
                    if chips.width() > 40.0 {
                        out.footer = self.chat_footer(ui, chips, chat, look);
                    }
                }
                enter && !swallow
            });
        out.rect = frame.response.rect;
        out.send = ready && (frame.inner || clicked_send);
        // under the card only what the voice is doing (dictation, the call); the keys' hint is gone
        // (the owner asked: the box says enough)
        let voice = &self.voice;
        let line = if let Some(dictation) = &self.dictation {
            dictation.line().map(|l| (l, look.soft))
        } else if voice.on || !voice.status.is_empty() {
            let state = voice.state.label();
            Some((
                if voice.status.is_empty() { state.to_string() } else { format!("{state} · {}", voice.status) },
                if voice.on { look.soft } else { look.faint },
            ))
        } else {
            None
        };
        if let (true, Some((text, color))) = (hints != Hints::None, line) {
            ui.add_space(8.0);
            let (line, _) = ui.allocate_exact_size(Vec2::new(width, 14.0), Sense::hover());
            let g = crate::app::chat::list::clipped(ui.painter(), text, FontId::proportional(11.0), color, width);
            ui.painter().galley(Pos2::new(line.center().x - g.size().x / 2.0, line.center().y - g.size().y / 2.0), g, color);
        }
        out
    }
}
