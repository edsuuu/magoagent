//! The messages queued for an agent: what the person writes while a chat still answers, or while a terminal waits on a
//! menu only it answers, waits at the conversation's end and goes when the agent is free. Editar takes it back into the
//! box; Enviar agora (and its keys, Ajustes › Atalhos) sends it without waiting: a chat's answer stops where it is and
//! the words go as the next message; a terminal gets Esc (Claude Code's "No, and tell Claude what to do differently")
//! and the words as soon as it is back at its prompt.

use crate::app::chat::list::Look;
use crate::app::chat::mini::ConvOut;
use crate::app::hotkeys::local::AppAction;
use crate::app::ui::Tip;
use crate::app::ui::agents::AgentState;
use crate::prelude::*;

impl DrawingApp {
    /// What was written to an agent while it waited on its terminal: it goes when it doesn't.
    pub(crate) fn queue_message(&mut self, id: &str, text: String) {
        match self.chat_queue.iter_mut().find(|(to, _)| to == id) {
            Some((_, queued)) => *queued = format!("{queued}\n\n{text}"),
            None => self.chat_queue.push((id.to_string(), text)),
        }
    }

    /// Sends what was queued to the agents no longer waiting on their terminal; an agent gone
    /// gives its words back to the box.
    pub(crate) fn flush_chat_queue(&mut self) {
        for (id, text) in std::mem::take(&mut self.chat_queue) {
            let ready = self.terminals.get(&id).and_then(|s| s.as_ref().ok()).is_some_and(|s| s.handle.ready().is_ok());

            match self.agent_state(&id) {
                AgentState::Waiting => self.chat_queue.push((id, text)),
                AgentState::Ended => {
                    self.toast(format!("{} fechou: a mensagem da fila voltou para a caixa.", self.terminal_name(&id)));
                    let gap = if self.chat_draft.is_empty() { "" } else { "\n" };

                    self.chat_draft = format!("{}{gap}{text}", self.chat_draft);
                }
                _ if ready => {
                    self.send_prompt(&id, &text);
                }
                _ => self.chat_queue.push((id, text)),
            }
        }
    }

    /// Whether something waits in the queue of `id`.
    pub(crate) fn has_queued(&self, id: &str) -> bool {
        self.chat_queue.iter().any(|(to, _)| to == id)
    }

    /// Editar: what was queued for `id` goes back into the box, ahead of what is there.
    pub(crate) fn unqueue(&mut self, id: &str) {
        for (_, text) in self.chat_queue.extract_if(.., |(to, _)| to == id) {
            self.chat_draft = if self.chat_draft.is_empty() { text } else { format!("{text}\n{}", self.chat_draft) };
        }
    }

    /// Enviar agora: what was queued for `id` goes without waiting for the agent. A chat's answer stops where it is
    /// (as Esc does) and the queue goes on the next frame, the chat being free (`chat_nodes_tick`); a terminal waiting on
    /// a menu gets Esc, which refuses what it asked, and the words go once it is back at its prompt (`flush_chat_queue`,
    /// at the next look at the terminals). What it did, said.
    pub(crate) fn send_queued_now(&mut self, id: &str) -> Result<String, String> {
        if !self.has_queued(id) {
            return Err(format!("Nada na fila de {}.", self.agent_label(id)));
        }

        if self.is_chat(id) {
            if !self.chat_runs.contains_key(id) {
                return Ok("O chat já está livre: a mensagem da fila vai agora.".into());
            }

            self.chat_stop(id);

            return Ok("Resposta interrompida: a mensagem da fila vai agora como a próxima.".into());
        }

        if self.agent_state(id) == AgentState::Waiting {
            let Some(Ok(session)) = self.terminals.get(id) else { return Err(format!("{} não tem terminal aberto.", self.agent_label(id))) };

            session.handle.write(vec![0x1b]);

            return Ok("Esc no terminal: o pedido foi recusado e a mensagem entra assim que ele voltar ao prompt.".into());
        }

        self.flush_chat_queue();
        Ok(if self.has_queued(id) { "A mensagem entra assim que o terminal aceitar digitação." } else { "Mensagem enviada." }.into())
    }

    /// The keys of Enviar agora (Ctrl+Shift+Enter): the words in the box that has the keyboard, or what waits in the
    /// queue, go to the agent of that conversation without waiting for its turn to end. The keys are taken only when
    /// there is something to send; else whatever has the keyboard gets them.
    pub(crate) fn send_now_key(&mut self, ctx: &Context) {
        if !self.peek_app_key(ctx, AppAction::SendNow) {
            return;
        }

        // a chat's card on the canvas has its box (and its words) of its own
        let card = self.chats.iter().map(|chat| chat.id.clone()).find(|id| ctx.memory(|m| m.has_focus(egui::Id::unique(("chat_card_box", id.as_str())))));
        let on_card = card.is_some();

        let (id, typed) = match card {
            Some(id) => {
                let key = egui::Id::unique(("chat_card_draft", id.as_str()));
                let typed = ctx.data(|d| d.get_temp::<String>(key)).unwrap_or_default();

                (id, typed)
            }
            None => {
                let in_box = ctx.memory(|m| m.has_focus(egui::Id::unique("chat_draft")));
                let shown = in_box || self.chat_open || self.mini.open;
                let Some(id) = self.chat_agent.clone().filter(|_| shown) else { return };

                (id, if in_box { self.chat_draft.clone() } else { String::new() })
            }
        };

        let typed = typed.trim_end().to_string();

        if typed.trim().is_empty() && !self.has_queued(&id) {
            return;
        }

        self.take_app_key(ctx, AppAction::SendNow);

        if !typed.trim().is_empty() {
            if on_card {
                ctx.data_mut(|d| d.insert_temp(egui::Id::unique(("chat_card_draft", id.as_str())), String::new()));
            } else {
                self.chat_draft.clear();
            }

            // busy, it joins the queue (a chat answering, a terminal on a menu); free, it simply goes
            self.send_from_box(ctx, &id, typed);
        }

        if self.has_queued(&id)
            && let Err(why) = self.send_queued_now(&id)
        {
            self.toast(why);
        }
    }

    /// The person's messages queued for `id`, at the conversation's end, each with Enviar agora (`out.send_now`) and
    /// Editar, which takes them back into the box (`out.unqueue`). The bubble keeps to the right and inside the chat,
    /// its lines at its left (bubble.rs).
    pub(crate) fn queued_bubbles(&self, ui: &mut egui::Ui, id: &str, width: f32, look: &Look, out: &mut ConvOut) {
        let chat = self.is_chat(id);

        for (_, text) in self.chat_queue.iter().filter(|(to, _)| to == id) {
            // one row as tall as a button (a right-to-left layout of the whole room would center it in its height)
            let row = Vec2::new(ui.available_width(), ui.spacing().interact_size.y);

            ui.allocate_ui_with_layout(row, egui::Layout::right_to_left(egui::Align::Center), |ui| {
                let action = |label: &str| egui::Button::new(egui::RichText::new(label).size(12.0).color(look.soft)).frame(false);

                out.unqueue |= ui.add(action("Editar")).on_hover_cursor(CursorIcon::PointingHand).clicked();
                ui.label(egui::RichText::new("·").size(12.0).color(look.muted));

                let what = if chat {
                    "Interrompe a resposta e manda esta mensagem já"
                } else {
                    "Responde Esc no terminal (recusa o pedido) e manda esta mensagem já"
                };

                out.send_now |=
                    ui.add(action("Enviar agora")).on_hover_cursor(CursorIcon::PointingHand).tip(self.tip_with_keys(what, AppAction::SendNow)).clicked();

                // what is left of the row says when it goes; a narrow chat (the mini chat, a card) keeps only the start
                let long = if chat { "Na fila · vai quando ele terminar ·" } else { "Na fila · vai quando você responder no terminal ·" };
                let fits = ui.painter().layout_no_wrap(long.into(), FontId::proportional(12.0), look.muted).size().x <= ui.available_width();

                ui.label(egui::RichText::new(if fits { long } else { "Na fila ·" }).size(12.0).color(look.muted));
            });

            let fill = crate::app::chat::bubble_fill(look.dark, true).gamma_multiply(0.55);
            let words = egui::RichText::new(text).size(14.0).color(Color32::WHITE);

            crate::app::chat::bubble::person_bubble(ui, words, width * 0.7, fill, 18, egui::Margin::symmetric(14, 7));
            ui.add_space(8.0);
        }
    }
}
