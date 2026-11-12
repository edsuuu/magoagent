//! A recruit's card on the canvas (a chat agent the Maestro called, chat/node.rs): no chat to write
//! in but a window on its work, the task it was given over what it does now. Folded, its name and
//! task only; and while its work is out of sight (folded, off the view, too small), what it starts
//! and when it ends pop at the top of the window, under the toolbar.

use crate::app::chat::list::{DOWN, Look, RED, glyph};
use crate::app::chat::node_ui::{chat_live, mini_button};
use crate::app::chat::steps::{Live, short_tool, step_row};
use crate::app::ui::agents::AgentState;
use crate::prelude::*;
use crate::transcript::ChatItem;

/// A folded card: its header and one row for the task.
pub(crate) const FOLDED_HEIGHT: f32 = TERMINAL_HEADER + 28.0;
/// How long a popup stays, the last part fading.
const POPUP_SECONDS: f64 = 5.0;
/// What the card lists of its work: the newest.
const ACTIVITY: usize = 40;
const UP: &[&[(f32, f32)]] = &[&[(6.0, 14.5), (12.0, 8.5), (18.0, 14.5)]];

/// What a recruit did, popped at the top: whose, what, when.
pub(crate) struct RecruitPopup {
    pub(crate) chat: String,
    pub(crate) who: String,
    pub(crate) text: String,
    pub(crate) born: f64,
}

/// The fold button: in the header, left of the mini chat's.
pub(crate) fn fold_button(rect: Rect, zoom: f32) -> Rect {
    mini_button(rect, zoom).translate(Vec2::new(-20.0 * zoom, 0.0))
}

/// The message's text, without the "asked by" the bridge put before another agent's question.
fn said(text: &str) -> &str {
    crate::app::chat::ask::asked_by(text).map_or(text, |(_, t)| t)
}

impl DrawingApp {
    /// What the recruit was asked: its first message (sent, or still waiting to be).
    pub(crate) fn recruit_task(&self, id: &str) -> Option<String> {
        let sent = self.transcripts.get(id).and_then(|t| t.items.iter().find_map(|i| if let ChatItem::User(text) = i { Some(said(text)) } else { None }));
        let task = sent.or_else(|| self.chat_starts.iter().find(|(c, _)| c == id).map(|(_, p)| p.as_str()))?;
        Some(task.split_whitespace().collect::<Vec<_>>().join(" "))
    }

    /// The card folded to its header and task, or open again at the height it had.
    pub(crate) fn fold_chat_card(&mut self, id: &str) -> Option<bool> {
        let i = self.chat_node(id)?;
        self.snapshot();
        let e = &mut self.scene.elements[i];
        let (ElementKind::Rectangle { height, .. }, Some(spec)) = (&mut e.kind, e.chat.as_mut()) else { return None };
        Some(match spec.folded.take() {
            Some(was) => {
                *height = was;
                false
            }
            None => {
                spec.folded = Some(*height);
                *height = FOLDED_HEIGHT;
                true
            }
        })
    }

    /// The task on the card's picture (folded, or too small to show its work).
    pub(crate) fn paint_recruit_task(&self, painter: &Painter, id: &str, body: Rect, zoom: f32, light: Color32, muted: Color32) {
        if body.width() < 24.0 * zoom {
            return;
        }
        let rows = ((body.height() / (12.5 * zoom)).floor() as usize).max(1);
        let (text, ink) = match self.recruit_task(id) {
            Some(task) => (task, light.gamma_multiply(0.85)),
            None => ("Sem tarefa ainda.".into(), muted),
        };
        let mut job = egui::text::LayoutJob::simple(text, FontId::proportional(10.5 * zoom), ink, body.width());
        job.wrap.max_rows = rows;
        job.wrap.overflow_character = Some('…');
        let g = painter.layout_job(job);
        // one row: in the middle of what is left under the header
        let y = if rows == 1 { body.center().y - g.size().y / 2.0 } else { body.top() };
        painter.galley(Pos2::new(body.left(), y), g, ink);
    }

    /// The live card: its task, what it did and does (tools, what it said), the band while it works.
    /// Laid out at the card's size and shown at the canvas's, like `chat_card_ui`.
    pub(crate) fn recruit_card_ui(&mut self, ctx: &Context, id: &str, body: Rect, z: f32, canvas: Rect) {
        let Some(row) = self.chat_rows().into_iter().find(|r| r.id == id) else { return };
        let look = Look::new(self.dark(), &self.colors());
        let time = ctx.input(|i| i.time);
        let size = body.size() / z;
        let clip = body.intersect(canvas);
        let clip = Rect::from_min_max(((clip.min - body.min) / z).to_pos2(), ((clip.max - body.min) / z).to_pos2());
        let task = self.recruit_task(id);
        let transcript = self.transcripts.get(id);
        let working = matches!(row.state, AgentState::Working(_));
        let (mut interrupt, mut open) = (false, false);
        let area = egui::Area::new(egui::Id::unique(("recruit_card", id))).fixed_pos(Pos2::ZERO).constrain(false).order(egui::Order::Middle).show(ctx, |ui| {
            ui.set_clip_rect(clip);
            let rect = Rect::from_min_size(Pos2::ZERO, size);
            // the whole body is the card's: a click never reaches the canvas, a double-click opens the chat
            open = ui.allocate_rect(rect, Sense::click()).double_clicked();
            let inner = rect.shrink2(Vec2::new(10.0, 8.0));
            ui.scope_builder(egui::UiBuilder::new().max_rect(inner), |ui| {
                ui.set_max_width(inner.width());
                ui.spacing_mut().item_spacing.y = 4.0;
                ui.label(egui::RichText::new("Tarefa").size(11.0).color(look.muted));
                let text = task.as_deref().unwrap_or("Sem tarefa ainda.");
                let mut job = egui::text::LayoutJob::simple(text.into(), FontId::proportional(13.0), look.text, inner.width());
                job.wrap.max_rows = 3;
                job.wrap.overflow_character = Some('…');
                ui.label(job);
                ui.add_space(2.0);
                let (line, _) = ui.allocate_exact_size(Vec2::new(inner.width(), 1.0), Sense::hover());
                ui.painter().hline(line.x_range(), line.center().y, Stroke::new(1.0, look.line));
                let band_key = egui::Id::unique(("recruit_band", id));
                let band = if working { ui.ctx().data_mut(|d| d.get_temp::<f32>(band_key)).unwrap_or(60.0) } else { 22.0 };
                ui.spacing_mut().scroll = egui::style::ScrollStyle::floating();
                egui::ScrollArea::vertical()
                    .id_salt(("recruit_scroll", id))
                    .stick_to_bottom(true)
                    .auto_shrink(false)
                    .max_height((ui.available_height() - band).max(0.0))
                    .show(ui, |ui| {
                        let Some(t) = transcript else { return };
                        // what came after the task: its tools, what it said, what went wrong, what it was asked since
                        let first = t.items.iter().position(|i| matches!(i, ChatItem::User(_))).map_or(0, |i| i + 1);
                        let items: Vec<&ChatItem> = t.items[first..].iter().filter(|i| !matches!(i, ChatItem::Image { .. } | ChatItem::System(_))).collect();
                        if items.is_empty() {
                            ui.label(egui::RichText::new(if working { "Começando…" } else { "Nada feito ainda." }).size(12.0).color(look.muted));
                        }
                        for item in &items[items.len().saturating_sub(ACTIVITY)..] {
                            match item {
                                ChatItem::Tool(step) => {
                                    let live = if t.running.iter().any(|(s, _)| *s == step.id) { Live::Running } else { Live::Done };
                                    open |= step_row(ui, step, live, &look, true, time);
                                }
                                ChatItem::Agent(text) => {
                                    let mut job =
                                        egui::text::LayoutJob::simple(text.trim().into(), FontId::proportional(12.0), look.soft, ui.available_width());
                                    job.wrap.max_rows = 3;
                                    job.wrap.overflow_character = Some('…');
                                    ui.label(job);
                                }
                                ChatItem::Error(text) => {
                                    ui.label(egui::RichText::new(text.trim()).size(12.0).color(RED));
                                }
                                ChatItem::User(text) => {
                                    ui.label(egui::RichText::new(format!("→ {}", said(text).trim())).size(12.0).color(look.muted));
                                }
                                _ => {}
                            }
                        }
                    });
                let top = ui.cursor().top();
                ui.add_space(4.0);
                if working {
                    interrupt = self.work_band(ui, &row, transcript, &look, true, time);
                    let height = ui.cursor().top() - top;
                    if (height - band).abs() > 0.5 {
                        ui.ctx().data_mut(|d| d.insert_temp(band_key, height));
                        ui.ctx().request_repaint();
                    }
                } else {
                    let done = if transcript.is_some_and(|t| t.items.iter().any(|i| matches!(i, ChatItem::Agent(_)))) { "Pronto" } else { "Esperando" };
                    ui.label(egui::RichText::new(done).size(12.0).color(look.muted));
                }
            });
        });
        ctx.set_transform_layer(area.response.layer_id, egui::emath::TSTransform::new(body.min.to_vec2(), z));
        if interrupt {
            let outer = self.chat_agent.replace(id.to_string());
            self.interrupt_agent();
            self.chat_agent = outer;
        }
        if open {
            self.open_mini_chat(id);
        }
    }

    /// The fold button on each recruit's card big enough to hit it.
    pub(crate) fn recruit_fold_buttons(&mut self, ctx: &Context, canvas: Rect) {
        let buttons: Vec<(String, Rect, f32, bool)> = self
            .scene
            .elements
            .iter()
            .filter(|e| e.chat.is_some() && self.is_recruit_chat(&e.id))
            .map(|e| {
                let (frame, z) = self.terminal_frame(e);
                (e.id.clone(), fold_button(frame, z), z, e.chat.as_ref().is_some_and(|c| c.folded.is_some()))
            })
            .filter(|(_, button, z, _)| *z >= 0.45 && canvas.contains_rect(*button))
            .collect();
        for (id, button, z, folded) in buttons {
            let clicked = egui::Area::new(egui::Id::unique(("recruit_fold", id.as_str())))
                .fixed_pos(button.min)
                .constrain(false)
                .order(egui::Order::Middle)
                .show(ctx, |ui| {
                    let (rect, r) = ui.allocate_exact_size(button.size(), Sense::click());
                    let r = r.on_hover_cursor(CursorIcon::PointingHand).tip(if folded { "Mostrar o que ele faz" } else { "Ocultar: só o nome e a tarefa" });
                    if r.hovered() {
                        ui.painter().rect_filled(rect, 4.0 * z, Color32::from_white_alpha(26));
                    }
                    glyph(ui.painter(), rect.center(), 16.0 * z, 2.2, Color32::from_gray(0xb0), if folded { DOWN } else { UP });
                    r.clicked()
                })
                .inner;
            if clicked {
                self.fold_chat_card(&id);
            }
        }
    }

    /// Each frame: what each recruit started (a tool) or ended since the last look, popped when its
    /// card doesn't show it.
    pub(crate) fn recruit_popups_tick(&mut self) {
        let ids: Vec<String> = self.chats.iter().filter(|c| c.recruit).map(|c| c.id.clone()).collect();
        for id in ids {
            let Some(t) = self.transcripts.get(&id) else { continue };
            let steps = t.items.iter().filter(|i| matches!(i, ChatItem::Tool(_))).count();
            let working = matches!(self.chat_state(&id), Some(AgentState::Working(_)));
            let Some((was_steps, was_working)) = self.recruit_seen.insert(id.clone(), (steps, working)) else { continue };
            let shown = self.chat_open
                || self.chat_node(&id).is_some_and(|i| {
                    let e = &self.scene.elements[i];
                    let (frame, z) = self.terminal_frame(e);
                    e.chat.as_ref().is_some_and(|c| c.folded.is_none()) && chat_live(frame, z) && frame.intersects(self.canvas_rect)
                });
            if shown {
                continue;
            }
            let t = &self.transcripts[&id];
            let text = if was_working && !working {
                match t.items.iter().rev().find_map(|i| match i {
                    ChatItem::Agent(text) => Some(Ok(text)),
                    ChatItem::Error(text) => Some(Err(text)),
                    _ => None,
                }) {
                    Some(Ok(text)) => format!("terminou: {}", text.lines().find(|l| !l.trim().is_empty()).unwrap_or("").trim()),
                    Some(Err(text)) => format!("parou: {}", text.trim()),
                    None => "terminou".into(),
                }
            } else if steps > was_steps
                && let Some(step) = t.items.iter().rev().find_map(|i| if let ChatItem::Tool(s) = i { Some(s) } else { None })
            {
                format!("{} {}", short_tool(&step.name), step.what).trim().to_string()
            } else {
                continue;
            };
            let who = self.maestro_name(&id);
            self.recruit_popups.push(RecruitPopup { chat: id, who, text, born: self.now });
        }
        let now = self.now;
        self.recruit_popups.retain(|p| now - p.born < POPUP_SECONDS);
        // the newest three
        let extra = self.recruit_popups.len().saturating_sub(3);
        self.recruit_popups.drain(..extra);
    }

    /// The popups, under the toolbar; a click goes to the card.
    pub(crate) fn recruit_popups(&mut self, ctx: &Context) {
        if self.recruit_popups.is_empty() || self.chat_open {
            return;
        }
        let look = Look::new(self.dark(), &self.colors());
        let mut go = None;
        egui::Area::new(egui::Id::unique("recruit_popups")).anchor(egui::Align2::CENTER_TOP, (0.0, TOOLBAR_BOTTOM + 10.0)).order(egui::Order::Foreground).show(
            ctx,
            |ui| {
                for p in &self.recruit_popups {
                    let fade = ((POPUP_SECONDS - (self.now - p.born)) / 0.6).clamp(0.0, 1.0) as f32;
                    let r = self
                        .island()
                        .multiply_with_opacity(fade)
                        .inner_margin(egui::Margin::symmetric(12, 7))
                        .show(ui, |ui| {
                            ui.set_max_width(440.0);
                            ui.horizontal(|ui| {
                                ui.label(egui::RichText::new(&p.who).strong().size(12.5).color(look.text.gamma_multiply(fade)));
                                ui.add(egui::Label::new(egui::RichText::new(&p.text).size(12.5).color(look.soft.gamma_multiply(fade))).truncate());
                            });
                        })
                        .response
                        .interact(Sense::click())
                        .on_hover_cursor(CursorIcon::PointingHand);
                    if r.clicked() {
                        go = Some(p.chat.clone());
                    }
                    ui.add_space(4.0);
                }
            },
        );
        ctx.request_repaint_after(std::time::Duration::from_millis(100));
        if let Some(id) = go {
            self.recruit_popups.retain(|p| p.chat != id);
            let _ = self.show_chat_on_canvas(&id);
        }
    }
}
