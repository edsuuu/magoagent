//! A conversation's messages, as the mini chat, its window and the chat's card on the canvas draw
//! them (the big chat has its own column).

use crate::app::chat::list::{GREEN, Look, clipped, elapsed, pulse_dot};
use crate::app::chat::mini::ConvOut;
use crate::app::chat::window_start;
use crate::app::ui::agents::{AgentRow, AgentState};
use crate::prelude::*;
use crate::transcript::{ChatItem, Step, Transcript};

impl DrawingApp {
    /// The messages of a conversation, in the column the caller scrolls: `narrow` is the mini chat's
    /// (and a narrow window's) tighter look. `query` shows only the messages that say it.
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn conversation(
        &self,
        ui: &mut egui::Ui,
        row: &AgentRow,
        transcript: Option<&Transcript>,
        talks: bool,
        only_chat: bool,
        waiting: bool,
        asked: &[(String, String)],
        query: Option<&str>,
        width: f32,
        look: &Look,
        c: &UiColors,
        narrow: bool,
        time: f64,
        out: &mut ConvOut,
    ) {
        // room between the messages and the scroll bar at the right edge
        ui.set_max_width(width - 14.0);
        ui.add_space(16.0);
        // tight lines inside a bubble; the gap between items comes from chat_item
        ui.spacing_mut().item_spacing.y = 4.0;
        match transcript {
            Some(t) if talks => {
                // each question marks one message, the latest that says it, newest first
                let mut from = std::collections::HashMap::new();
                for (prompt, who) in asked.iter().rev() {
                    let says = |(i, item): &(usize, &ChatItem)| {
                        !from.contains_key(i) && matches!(item, ChatItem::User(text) if prompt.split_whitespace().eq(text.split_whitespace()))
                    };
                    if let Some((i, _)) = t.items.iter().enumerate().rev().find(says) {
                        from.insert(i, who.as_str());
                    }
                }
                if let Some(query) = query {
                    let found = crate::app::chat::list::found(&t.items, query);
                    if found.is_empty() {
                        ui.label(egui::RichText::new("Nenhuma mensagem com essas palavras.").size(13.0).color(look.muted));
                    }
                    for i in found {
                        self.chat_item(ui, &t.items[i], width, c, from.get(&i).copied(), &mut out.cite);
                    }
                }
                // only the end of a long conversation is laid out; the rest comes a page at a time
                let pages = if self.chat_earlier.0 == row.id { self.chat_earlier.1 } else { 0 };
                let first = window_start(t.items.len(), pages);
                if query.is_none() && first > 0 {
                    let label = format!("Mostrar mensagens anteriores ({first} acima)");
                    let button = egui::Button::new(egui::RichText::new(label).size(13.0).color(look.muted)).frame(false);
                    out.earlier |= ui.add(button).on_hover_cursor(CursorIcon::PointingHand).clicked();
                    ui.add_space(8.0);
                }
                // searching, only what says it (above)
                let mut i = if query.is_some() { t.items.len() } else { first };
                // only what is on screen is laid out: the rest takes the room it took when last
                // shown (a long chat laid out whole each frame made the scroll stutter)
                let visible = ui.clip_rect().expand(600.0);
                while i < t.items.len() {
                    // tool calls one after another go as one run of steps
                    let steps: Vec<&Step> = t.items[i..].iter().map_while(|item| if let ChatItem::Tool(s) = item { Some(s) } else { None }).collect();
                    let next = i + steps.len().max(1);
                    let top = ui.cursor().top();
                    let key = egui::Id::unique(("chat_block", row.id.as_str(), i, width as i32, next == t.items.len()));
                    if next < t.items.len()
                        && let Some(height) = ui.data(|d| d.get_temp::<f32>(key))
                        && !visible.y_range().intersects(egui::Rangef::new(top, top + height))
                    {
                        ui.add_space(height);
                        i = next;
                        continue;
                    }
                    if !steps.is_empty() {
                        let trailing = next == t.items.len();
                        self.tool_run(ui, &steps, trailing, &t.running, waiting, look, narrow, time, &mut out.fold);
                    } else {
                        self.chat_item(ui, &t.items[i], width, c, from.get(&i).copied(), &mut out.cite);
                    }
                    let height = ui.cursor().top() - top;
                    ui.data_mut(|d| d.insert_temp(key, height));
                    i = next;
                }
                self.login_buttons(ui, &row.id, &t.items, look);
                // the turn over, Claude's mascot signs it: done, asleep after a while
                if let (AgentState::Idle(_), Some(ChatItem::Agent(_)), true) = (row.state, t.items.last(), crate::app::mascot::claude(&row.spec)) {
                    // the turn: what came after the last message sent
                    let start = t.items.iter().rposition(|i| matches!(i, ChatItem::User(_))).map_or(0, |u| u + 1);
                    let steps: Vec<(&str, &str, bool)> = t.items[start..]
                        .iter()
                        .filter_map(|i| if let ChatItem::Tool(s) = i { Some((s.name.as_str(), s.what.as_str(), s.failed)) } else { None })
                        .collect();
                    let state = if steps.last().is_some_and(|(.., failed)| *failed) { MagoState::Error } else { MagoState::from_agent(row.state, self.now) };
                    let (rect, _) = ui.allocate_exact_size(Vec2::new(56.0, 52.0), Sense::hover());
                    paint_agent_mago(ui.painter(), &format!("{}-end", row.id), self.agent_color(&row.id), rect.center(), 52.0, state, time);
                }
            }
            // a session outside the app before its first message, or one whose file the app does not read
            _ if row.id.starts_with(crate::app::maestro::outside::PILL_PREFIX) => {
                ui.label(egui::RichText::new(self.outside_unread(&row.id)).size(13.0).color(look.muted));
            }
            _ if only_chat => {
                if crate::app::mascot::claude(&row.spec) {
                    let (rect, _) = ui.allocate_exact_size(Vec2::new(ui.available_width(), 130.0), Sense::hover());
                    paint_agent_mago(ui.painter(), &format!("{}-welcome", row.id), self.agent_color(&row.id), rect.center(), 130.0, MagoState::Idle, time);
                }
                // which agent answers, chosen before the first message
                let fresh = self.chats.iter().any(|c| c.id == row.id && c.session.is_none());
                if fresh && let Some(agent) = providers(ui, &self.chat_providers(), &row.spec.command, look) {
                    out.provider = Some((row.id.clone(), agent));
                }
            }
            _ => {
                let note = "Este terminal não tem uma conversa do Claude Code; esta é a tela dele.";
                ui.label(egui::RichText::new(note).size(13.0).color(look.muted));
            }
        }
        self.queued_bubbles(ui, &row.id, width, look, out);
        self.team_rows(ui, &row.id, look, time);
    }

    /// The agents `id` recruited (`boss_of`) at work now, a line each at the end of its conversation: who, for how long
    /// and the step it is on, as it happens; what each says at the end comes in as its answer (ask.rs; asked: "quando
    /// recebe uma mensagem dos agentes, não aparece na hora").
    pub(crate) fn team_rows(&self, ui: &mut egui::Ui, id: &str, look: &Look, time: f64) {
        let working: Vec<(String, f64)> = self
            .chats
            .iter()
            .filter(|chat| chat.recruit && self.boss_of(&chat.id).as_deref() == Some(id))
            .filter_map(|chat| match self.chat_state(&chat.id) {
                Some(AgentState::Working(since)) => Some((chat.id.clone(), since)),
                _ => None,
            })
            .collect();
        for (recruit, since) in working {
            let step = self.transcripts.get(&recruit).and_then(|transcript| {
                transcript.items.iter().rev().find_map(|item| match item {
                    ChatItem::Tool(step) => Some(format!("{} {}", crate::app::chat::steps::short_tool(&step.name), step.what)),
                    _ => None,
                })
            });
            let line = match step {
                Some(step) => format!("{} trabalhando · {} · {step}", self.speaker(&recruit), elapsed(self.now - since)),
                None => format!("{} trabalhando · {}", self.speaker(&recruit), elapsed(self.now - since)),
            };
            let (rect, _) = ui.allocate_exact_size(Vec2::new(ui.available_width(), 20.0), Sense::hover());
            let p = ui.painter();
            pulse_dot(p, Pos2::new(rect.left() + 6.0, rect.center().y), 3.0, GREEN, 1.6, time);
            let text = clipped(p, line, FontId::proportional(12.5), look.muted, rect.width() - 18.0);
            p.galley(Pos2::new(rect.left() + 16.0, rect.center().y - text.size().y / 2.0), text, look.muted);
            ui.ctx().request_repaint_after(std::time::Duration::from_millis(500));
        }
    }
}

/// The agents a chat can talk to, side by side under the welcome, the chat's own raised; the one
/// clicked. `list`: (name, program).
fn providers(ui: &mut egui::Ui, list: &[(String, String)], current: &str, look: &Look) -> Option<String> {
    let font = FontId::proportional(14.0);
    let widths: Vec<f32> = list.iter().map(|(name, _)| ui.painter().layout_no_wrap(name.clone(), font.clone(), look.text).size().x + 28.0).collect();
    let total = widths.iter().sum::<f32>() + 6.0 * (list.len().saturating_sub(1)) as f32;
    let (row, _) = ui.allocate_exact_size(Vec2::new(ui.available_width(), 34.0), Sense::hover());
    let mut x = row.center().x - total / 2.0;
    let mut picked = None;
    for ((name, program), w) in list.iter().zip(widths) {
        let chip = Rect::from_min_size(Pos2::new(x, row.top()), Vec2::new(w, 32.0));
        x += w + 6.0;
        let on = program == current;
        let r = ui.interact(chip, ui.scope_id().with(("chat_provider", program.as_str())), Sense::click()).on_hover_cursor(CursorIcon::PointingHand);
        let p = ui.painter();
        if on {
            p.rect_filled(chip, 16, look.raised);
        } else if r.hovered() {
            p.rect_filled(chip, 16, look.hover);
        }
        p.rect_stroke(chip, 16, Stroke::new(1.0, look.line), StrokeKind::Inside);
        p.text(chip.center(), egui::Align2::CENTER_CENTER, name, font.clone(), if on { look.text } else { look.soft });
        if r.clicked() && !on {
            picked = Some(program.clone());
        }
    }
    picked
}
