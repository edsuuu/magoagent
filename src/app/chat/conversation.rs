//! A conversation's messages, as the mini chat, its window and the chat's card on the canvas draw
//! them (the big chat has its own column).

use crate::app::chat::list::Look;
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
                        ui.label(egui::RichText::new("Nenhuma mensagem com essas palavras.").size(12.0).color(look.muted));
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
                    let button = egui::Button::new(egui::RichText::new(label).size(12.0).color(look.muted)).frame(false);
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
                    let key = egui::Id::new(("chat_block", row.id.as_str(), i, width as i32, next == t.items.len()));
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
                if let (AgentState::Idle(idle), Some(ChatItem::Agent(answer)), true) = (row.state, t.items.last(), crate::app::mascot::poses::claude(&row.spec))
                {
                    // the turn: what came after the last message sent
                    let start = t.items.iter().rposition(|i| matches!(i, ChatItem::User(_))).map_or(0, |u| u + 1);
                    let steps: Vec<(&str, &str, bool)> = t.items[start..]
                        .iter()
                        .filter_map(|i| if let ChatItem::Tool(s) = i { Some((s.name.as_str(), s.what.as_str(), s.failed)) } else { None })
                        .collect();
                    let pose = crate::app::mascot::poses::resting(idle.map_or(0.0, |s| self.now - s), t.items.len() as u64, &steps, answer);
                    let (rect, _) = ui.allocate_exact_size(Vec2::new(56.0, 46.0), Sense::hover());
                    crate::app::mascot::poses::paint_pose(ui.painter(), pose, rect.center(), 56.0, time, false, 1.0);
                }
            }
            _ if only_chat => {
                if crate::app::mascot::poses::claude(&row.spec) {
                    let (rect, _) = ui.allocate_exact_size(Vec2::new(ui.available_width(), 120.0), Sense::hover());
                    crate::app::mascot::poses::paint_pose(ui.painter(), crate::app::mascot::poses::WELCOME, rect.center(), 140.0, time, false, 1.0);
                }
                // which agent answers, chosen before the first message
                let fresh = self.chats.iter().any(|c| c.id == row.id && c.session.is_none());
                if fresh && let Some(agent) = providers(ui, &self.chat_providers(), &row.spec.command, look) {
                    out.provider = Some((row.id.clone(), agent));
                }
            }
            _ => {
                let note = "Este terminal não tem uma conversa do Claude Code; esta é a tela dele.";
                ui.label(egui::RichText::new(note).size(12.0).color(look.muted));
            }
        }
        self.queued_bubbles(ui, &row.id, width, look, &mut out.unqueue);
    }
}

/// The agents a chat can talk to, side by side under the welcome, the chat's own raised; the one
/// clicked. `list`: (name, program).
fn providers(ui: &mut egui::Ui, list: &[(String, String)], current: &str, look: &Look) -> Option<String> {
    let font = FontId::proportional(13.0);
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
