//! The mini chat: leaving the chat with a conversation open leaves a pill over the canvas (the
//! agent, what it does); a click opens the conversation in a small floating panel, 340 × 600,
//! dragged by its header and kept inside the window, like the reference's palette panels. The
//! messages, steps and box are the big chat's own (`conversation`, `work_band`, `send_from_box`).

use crate::app::chat::composer::Hints;
use crate::app::chat::list::{AMBER, CROSS, Look, clipped, glyph, quote_into, state_mark};
use crate::app::ui::agents::{AgentRow, AgentState};
use crate::prelude::*;

/// The panel's size at most and the gap it keeps from the window.
pub(crate) const WIDTH: f32 = 340.0;
pub(crate) const HEIGHT: f32 = 600.0;
const HEAD: f32 = 50.0;

/// What the conversation's widgets ask for; applied after the frame (`conv_apply`).
#[derive(Default)]
pub(crate) struct ConvOut {
    pub(crate) fold: Option<String>,
    pub(crate) cite: Option<String>,
    pub(crate) earlier: bool,
    pub(crate) unqueue: bool,
    /// The provider picked before the first message: the chat and the agent's program.
    pub(crate) provider: Option<(String, String)>,
}

#[derive(Default)]
pub(crate) struct Mini {
    /// The pill is on: a conversation was left, and it stays until dismissed.
    pub(crate) on: bool,
    /// The panel is open (else the pill).
    pub(crate) open: bool,
    was_chat: bool,
    /// How many messages the person has seen; more than that is news.
    seen: usize,
    /// While the header is dragged: the panel's top left from the area's.
    drag: Option<Vec2>,
    /// While the pill is dragged: how far it went.
    pill_drag: Vec2,
    /// The panel as painted last frame, for the keys and the drops.
    pub(crate) rect: Option<Rect>,
    footer: f32,
    focus: bool,
    /// The panel is being drawn in its own window (chat/mini_window.rs), filling it.
    pub(crate) window: bool,
}

/// Where the pill's top right goes for `at`: inside the canvas, room for the pill left of it.
pub(crate) fn pill_place(area: Rect, at: Pos2) -> Pos2 {
    Pos2::new(at.x.clamp(area.left() + 160.0, area.right() - 4.0), at.y.clamp(area.top() + 4.0, area.bottom() - 48.0))
}

/// The panel's size in `area`: 340 × 600 at most, 24 in from the sides and 68 from the top and bottom.
pub(crate) fn mini_size(area: Rect) -> Vec2 {
    Vec2::new(WIDTH.min(area.width() - 24.0), HEIGHT.min(area.height() - 68.0)).max(Vec2::splat(0.0))
}

/// The panel in `area`: at `at` from its top left, else top right; never out of the area.
pub(crate) fn mini_place(area: Rect, size: Vec2, at: Option<Vec2>) -> Rect {
    let min = match at {
        Some(at) => area.min + at,
        None => Pos2::new(area.right() - 12.0 - size.x, area.top() + 56.0),
    };
    let most = (area.max - size).max(area.min);
    Rect::from_min_size(Pos2::new(min.x.clamp(area.min.x, most.x), min.y.clamp(area.min.y, most.y)), size)
}

fn status(state: &AgentState, unread: bool) -> &'static str {
    match state {
        AgentState::Working(_) => "Trabalhando…",
        AgentState::Waiting => "Aguardando você",
        _ if unread => "Nova resposta",
        AgentState::Ended => "Encerrado",
        _ => "Pronto",
    }
}

impl DrawingApp {
    /// What other agents asked this one over the bridge: (prompt, who asked).
    pub(crate) fn asked_of(&self) -> Vec<(String, String)> {
        match (&self.bridge, &self.chat_agent) {
            (Some(bridge), Some(id)) => {
                bridge.shared.lock().unwrap().asked.iter().filter(|(to, ..)| to == id).map(|(_, prompt, who)| (prompt.clone(), who.clone())).collect()
            }
            _ => Vec::new(),
        }
    }

    fn talk_len(&self, id: &str) -> usize {
        self.transcripts.get(id).map_or(0, |t| t.items.len())
    }

    /// The window below the title bar and beside the sidebar: where the mini chat lives.
    fn mini_area(&self, ctx: &Context) -> Rect {
        let r = ctx.content_rect();
        Rect::from_min_max(Pos2::new(r.min.x + self.sidebar_width(), r.min.y + TITLE_BAR), r.max)
    }

    /// The pill's click, the header's expand and the MCP: open or fold the panel.
    pub(crate) fn mini_set(&mut self, open: bool) {
        self.mini.open = open;
        self.mini.focus = open;
        self.mini.on |= open;
        if open {
            self.mini.seen = self.chat_agent.as_deref().map_or(0, |id| self.talk_len(id));
        }
    }

    /// Whether the mini chat takes dropped files and pasted pictures: open, with the pointer over it
    /// or its box focused.
    pub(crate) fn mini_takes_files(&self, ctx: &Context) -> bool {
        let Some(rect) = self.mini.rect.filter(|_| self.mini.open && !self.chat_open) else { return false };
        ctx.input(|i| i.pointer.hover_pos()).is_some_and(|p| rect.contains(p)) || ctx.memory(|m| m.has_focus(egui::Id::unique("chat_draft")))
    }

    /// Esc, Interromper: the agent stops what it does.
    pub(crate) fn interrupt_agent(&mut self) {
        if let Some(id) = self.chat_agent.clone().filter(|id| self.is_chat(id)) {
            self.chat_stop(&id);
        } else if let Some(Ok(session)) = self.chat_agent.as_ref().and_then(|id| self.terminals.get(id)) {
            session.handle.write(vec![0x1b]);
        }
    }

    /// The words of the box go to the agent `id`: a chat runs them (a local command or the agent),
    /// a terminal waiting on a menu queues them. Refused (its claude left): they stay in the box.
    pub(crate) fn send_from_box(&mut self, ctx: &Context, id: &str, text: String) {
        // the pictures above the box go first, as the paths Claude Code reads
        let prompt = self.attached_prompt(&text);
        if self.is_chat(id) {
            if self.chat_submit(ctx, id, &text, &prompt) {
                self.chat_pics.get_mut().attached.clear();
            } else {
                self.chat_draft = text;
            }
        } else if self.agent_state(id) == AgentState::Waiting {
            self.queue_message(id, prompt);
            self.chat_pics.get_mut().attached.clear();
        } else if self.send_prompt(id, &prompt) {
            self.chat_pics.get_mut().attached.clear();
        } else {
            self.chat_draft = text;
        }
    }

    /// Citar, the folded runs, Mostrar mensagens anteriores and Editar of a conversation drawn.
    pub(crate) fn conv_apply(&mut self, ctx: &Context, out: ConvOut, draft_id: egui::Id) {
        // Citar: the words go into the box as a quote, the box takes the keys
        if let Some(text) = out.cite {
            quote_into(&mut self.chat_draft, &text);
            ctx.memory_mut(|m| m.request_focus(draft_id));
        }
        if let Some(key) = out.fold
            && !self.chat_unfolded.remove(&key)
        {
            self.chat_unfolded.insert(key);
        }
        // one more page of this conversation
        if out.earlier
            && let Some(id) = self.chat_agent.clone()
        {
            let pages = if self.chat_earlier.0 == id { self.chat_earlier.1 + 1 } else { 1 };
            self.chat_earlier = (id, pages);
        }
        if let Some((id, agent)) = out.provider
            && let Err(e) = self.chat_switch_agent(&id, &agent)
        {
            self.toast(e);
        }
        // Editar: what was queued goes back into the box
        if out.unqueue
            && let Some(id) = &self.chat_agent
        {
            for (_, text) in self.chat_queue.extract_if(.., |(to, _)| to == id) {
                self.chat_draft = if self.chat_draft.is_empty() { text } else { format!("{text}\n{}", self.chat_draft) };
            }
        }
    }

    /// Every frame: the pill (or the panel) over the canvas while a conversation was left open.
    pub(crate) fn mini_chat_ui(&mut self, ctx: &Context) {
        let left = self.mini.was_chat && !self.chat_open;
        self.mini.was_chat = self.chat_open;
        if self.chat_open {
            self.mini.open = false;
            self.mini.rect = None;
            return;
        }
        let Some(id) = self.chat_agent.clone() else { return };
        if left && self.talk_len(&id) > 0 {
            self.mini.on = true;
            self.mini.seen = self.talk_len(&id);
        }
        let rows = self.agent_rows();
        let row = rows.iter().find(|r| r.id == id);
        let Some(row) = row.filter(|_| self.mini.on && !self.floors_ui.fx.engaged) else {
            self.mini.rect = None;
            return;
        };
        self.refresh_transcript();
        // a picture open covers the screen, whatever is under it
        if self.image_viewer(ctx, row.spec.label()) {
            return;
        }
        let (c, dark) = (self.colors(), self.dark());
        let look = Look::new(dark, &c);
        let time = ctx.input(|i| i.time);
        let working = matches!(row.state, AgentState::Working(_));
        if (working || row.state == AgentState::Waiting) && ctx.input(|i| i.viewport().visible()) != Some(false) {
            ctx.request_repaint_after(std::time::Duration::from_millis(33));
        }
        let unread = !self.mini.open && !working && self.talk_len(&id) > self.mini.seen;
        let area = self.mini_area(ctx);
        if self.mini.open && ctx.embed_viewports() {
            // no windows of its own here (tests, a backend without them): over the canvas
            self.mini_panel(ctx, row, area, &look, &c, time, unread);
        } else if self.mini.open {
            self.mini_window(ctx, row, &look, &c, time, unread);
        } else {
            self.mini.rect = None;
            self.mini_pill(ctx, row, area, &look, time, unread);
        }
    }

    /// The pill: the agent, what it does (the spinner, or a dot when news came), a × to dismiss.
    fn mini_pill(&mut self, ctx: &Context, row: &AgentRow, area: Rect, look: &Look, time: f64, unread: bool) {
        let (mut open, mut dismiss, mut moved, mut stopped) = (false, false, Vec2::ZERO, false);
        // where it was left (dragged anywhere over the canvas), kept inside the canvas
        let saved = self.prefs.mini_pill_at.map(|[x, y]| area.min + Vec2::new(x, y) + self.mini.pill_drag);
        let anchor = pill_place(area, saved.unwrap_or(Pos2::new(area.right() - 12.0, area.top() + 56.0) + self.mini.pill_drag));
        egui::Area::new(egui::Id::unique("mini_pill")).order(egui::Order::Foreground).pivot(egui::Align2::RIGHT_TOP).fixed_pos(anchor).show(ctx, |ui| {
            let p = ui.painter().clone();
            let name = clipped(&p, row.spec.label().to_string(), FontId::proportional(13.5), look.text, 130.0);
            let note = p.layout_no_wrap(status(&row.state, unread).to_string(), FontId::proportional(11.5), look.muted);
            let claude = crate::app::mascot::poses::claude(&row.spec);
            let icon_w = if claude { 34.0 } else { 22.0 };
            let size = Vec2::new(12.0 + icon_w + 8.0 + name.size().x.max(note.size().x) + 8.0 + 22.0 + 4.0, if claude { 44.0 } else { 40.0 });
            let (rect, response) = ui.allocate_exact_size(size, Sense::click_and_drag());
            let cursor = if response.dragged() { CursorIcon::Grabbing } else { CursorIcon::PointingHand };
            let response = response.on_hover_cursor(cursor).tip("Abrir o chat em miniatura · arraste para mudar de lugar");
            if response.dragged() {
                moved = response.drag_delta();
            }
            stopped = response.drag_stopped();
            ui.painter().add(egui::epaint::Shadow { offset: [0, 4], blur: 16, spread: 0, color: Color32::from_black_alpha(70) }.as_shape(rect, 20));
            ui.painter().rect_filled(rect, 20, if response.hovered() { look.surface } else { look.bg });
            ui.painter().rect_stroke(rect, 20, Stroke::new(1.0, look.line), StrokeKind::Inside);
            let icon = Pos2::new(rect.left() + 12.0 + icon_w / 2.0, rect.center().y);
            if claude {
                // Claude's mascot: thinking while it works, happy with news, waiting with its sign
                let pose = match row.state {
                    AgentState::Working(since) => crate::app::mascot::poses::working(None, "", time - since.min(time), since.to_bits()),
                    AgentState::Waiting => "it-depends-sign",
                    _ if unread => "happy-pixel-celebrating",
                    _ => "artboard-24cla",
                };
                crate::app::mascot::poses::paint_pose(ui.painter(), pose, icon, 44.0, time, matches!(row.state, AgentState::Working(_)), 1.0);
            } else if matches!(row.state, AgentState::Working(_)) {
                crate::app::chat::thinking::spinner(ui.painter(), icon, 18.0, crate::app::chat::thinking::CLAUDE, time);
            } else {
                ui.painter().circle_filled(icon, 11.0, look.raised);
                let initial = row.spec.label().chars().next().unwrap_or('?').to_uppercase().to_string();
                ui.painter().text(icon, egui::Align2::CENTER_CENTER, initial, FontId::proportional(12.5), look.text);
            }
            if unread {
                ui.painter().circle(icon + Vec2::new(8.0, -8.0), 5.0, AMBER, Stroke::new(2.0, look.bg));
            }
            let x = rect.left() + 12.0 + icon_w + 8.0;
            ui.painter().galley(Pos2::new(x, rect.center().y - 15.0), name, look.text);
            ui.painter().galley(Pos2::new(x, rect.center().y + 1.0), note, look.muted);
            let close = Rect::from_center_size(Pos2::new(rect.right() - 20.0, rect.center().y), Vec2::splat(22.0));
            let hit = ui.interact(close, ui.scope_id().with("mini_dismiss"), Sense::click()).on_hover_cursor(CursorIcon::PointingHand).tip("Dispensar");
            glyph(ui.painter(), close.center(), 14.0, 2.0, if hit.hovered() { look.text } else { look.muted }, CROSS);
            dismiss = hit.clicked();
            open = response.clicked() && !dismiss;
        });
        self.mini.pill_drag += moved;
        if stopped {
            let at = anchor + moved - area.min;
            self.prefs.mini_pill_at = Some([at.x, at.y]);
            self.mini.pill_drag = Vec2::ZERO;
        }
        if dismiss {
            self.mini.on = false;
        } else if open {
            self.mini_set(true);
        }
    }

    /// The panel: header (close, agent and state, expand), the conversation, the band, the box.
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn mini_panel(&mut self, ctx: &Context, row: &AgentRow, area: Rect, look: &Look, c: &UiColors, time: f64, unread: bool) {
        let id = row.id.clone();
        self.mini.seen = self.talk_len(&id);
        let window = self.mini.window;
        let size = if window { area.size() } else { mini_size(area) };
        let saved = self.prefs.mini_chat_at.map(|[x, y]| Vec2::new(x, y));
        let rect = if window { area } else { mini_place(area, size, self.mini.drag.or(saved)) };
        // its own window has the system's corners and shadow
        let radius = if window { 0 } else { 18 };
        self.mini.rect = Some(rect);
        let draft_id = egui::Id::unique("chat_draft");
        // the box's menu of / commands; with it open, Esc and the arrows are its
        let slash = self.slash_entries();
        let focused = ctx.memory(|m| m.has_focus(draft_id));
        // egui drops the focus of the box at Esc before this frame runs: it had it a frame ago
        let had = ctx.memory(|m| m.had_focus_last_frame(draft_id));
        let menu_keys = slash.is_some() && (focused || had);
        let over = ctx.input(|i| i.pointer.hover_pos()).is_some_and(|p| rect.contains(p));
        let esc = !menu_keys
            && !egui::Popup::is_any_open(ctx)
            && !self.dialog_open()
            && (focused || had || over)
            && ctx.input_mut(|i| i.consume_key(Modifiers::NONE, Key::Escape));
        let mut draft = std::mem::take(&mut self.chat_draft);
        let swallow = slash.as_ref().is_some_and(|entries| self.slash_keys(ctx, &mut draft, entries, draft_id));
        // with no menu, Tab does nothing in the box (lock_focus would type a tab)
        if slash.is_none() && focused {
            ctx.input_mut(|i| i.consume_key(Modifiers::NONE, Key::Tab));
        }
        // the area is hidden on its first frame: the request is repeated until the box has the focus
        if self.mini.focus {
            self.mini.focus = !focused;
            ctx.memory_mut(|m| m.request_focus(draft_id));
        }
        let transcript = self.transcripts.get(&id);
        let talks = transcript.is_some_and(|t| !t.items.is_empty());
        let (waiting, only_chat) = (row.state == AgentState::Waiting, self.is_chat(&id) || row.spec.chat);
        let asked = self.asked_of();
        let cap = 140f32.min(rect.height() * 0.3);
        let hint = format!("Mensagem para {}", row.spec.label());
        let (mut close, mut expand, mut interrupt, mut attach, mut send) = (esc, false, false, false, None);
        let mut moved = None;
        let mut stopped = false;
        let mut conv = ConvOut::default();
        let mut slash_pick = None;
        let mut footer = self.mini.footer;
        let (mut dictate, mut voice, mut footer_act) = (false, (false, false, false), None);
        egui::Area::new(egui::Id::unique("mini_chat")).order(egui::Order::Foreground).fixed_pos(rect.min).constrain(false).show(ctx, |ui| {
            if !window {
                ui.painter().add(egui::epaint::Shadow { offset: [0, 10], blur: 32, spread: 0, color: Color32::from_black_alpha(90) }.as_shape(rect, 18));
            }
            ui.painter().rect_filled(rect, radius, look.bg);
            ui.painter().rect_stroke(rect, radius, Stroke::new(1.0, look.line), StrokeKind::Inside);
            // the whole panel is the panel's: a click in it never reaches the canvas
            ui.allocate_rect(rect, Sense::click_and_drag());
            let head = Rect::from_min_size(rect.min, Vec2::new(rect.width(), HEAD));
            let cy = head.center().y;
            let grip = ui.interact(head, ui.scope_id().with("mini_grip"), Sense::drag()).on_hover_cursor(CursorIcon::Grab);
            if window && grip.drag_started() {
                // its window goes where the system takes it
                ui.ctx().send_viewport_cmd(egui::ViewportCommand::StartDrag);
            } else if !window && grip.dragged() {
                moved = Some(mini_place(area, size, Some(rect.min - area.min + grip.drag_delta())).min - area.min);
            }
            stopped = grip.drag_stopped();
            // close, at the left like the reference's panels
            let x = Rect::from_center_size(Pos2::new(rect.left() + 24.0, cy), Vec2::splat(22.0));
            let hit = ui.interact(x, ui.scope_id().with("mini_close"), Sense::click()).on_hover_cursor(CursorIcon::PointingHand).tip("Recolher (Esc)");
            ui.painter().circle_filled(x.center(), 10.0, if hit.hovered() { look.selected } else { look.raised });
            glyph(ui.painter(), x.center(), 13.0, 2.2, look.text, CROSS);
            close |= hit.clicked();
            // expand, at the right
            let big = Rect::from_center_size(Pos2::new(rect.right() - 24.0, cy), Vec2::splat(28.0));
            let hit =
                ui.interact(big, ui.scope_id().with("mini_expand"), Sense::click()).on_hover_cursor(CursorIcon::PointingHand).tip("Abrir em tela cheia (⌘1)");
            if hit.hovered() {
                ui.painter().rect_filled(big, 8, look.raised);
            }
            crate::app::ui::icons::paint_icon(ui.painter(), crate::app::ui::icons::Icon::Expand, big.center(), look.text);
            expand = hit.clicked();
            // who and how
            let left = rect.left() + 46.0;
            let name = clipped(ui.painter(), row.spec.label().to_string(), FontId::proportional(14.0), look.text, big.left() - left - 8.0);
            let top = cy - name.size().y - 1.0;
            ui.painter().galley(Pos2::new(left + 1.0 / ui.painter().pixels_per_point(), top), name.clone(), look.text);
            ui.painter().galley(Pos2::new(left, top), name, look.text);
            state_mark(ui.painter(), Pos2::new(left + 4.0, cy + 9.0), 7.0, &row.state, look, time);
            ui.painter().text(Pos2::new(left + 14.0, cy + 9.0), egui::Align2::LEFT_CENTER, status(&row.state, unread), FontId::proportional(11.5), look.muted);
            ui.painter().hline(rect.left() + 12.0..=rect.right() - 12.0, head.bottom(), Stroke::new(1.0, look.line));
            // the conversation and the box
            let body = Rect::from_min_max(Pos2::new(rect.left() + 10.0, head.bottom() + 2.0), Pos2::new(rect.right() - 10.0, rect.bottom() - 10.0));
            let width = body.width() - 4.0;
            ui.scope_builder(egui::UiBuilder::new().max_rect(body), |ui| {
                ui.set_max_width(width);
                let footer_height = if self.mini.footer > 0.0 { self.mini.footer } else { 70.0 };
                ui.spacing_mut().scroll = egui::style::ScrollStyle::floating();
                egui::ScrollArea::vertical()
                    .id_salt("mini_scroll")
                    .stick_to_bottom(true)
                    .auto_shrink(false)
                    .max_height(ui.available_height() - footer_height)
                    .show(ui, |ui| self.conversation(ui, row, transcript, talks, only_chat, waiting, &asked, None, width, look, c, true, time, &mut conv));
                let top = ui.cursor().top();
                ui.add_space(6.0);
                if working_now(row) {
                    interrupt |= self.work_band(ui, row, transcript, look, true, time);
                    ui.add_space(6.0);
                }
                // the big chat's card, without the line of keys under it
                let card = self.composer(ui, &mut draft, draft_id, &hint, width, cap, swallow, look, c, Hints::None, time);
                let r = card.rect;
                if let Some(entries) = &slash {
                    slash_pick = self.slash_menu_ui(ctx, entries, Pos2::new(r.left(), r.top() - 6.0), r.width(), look);
                }
                (attach, dictate, voice, footer_act) = (card.attach, card.dictate, card.voice, card.footer);
                if card.send {
                    send = Some(std::mem::take(&mut draft).trim_end().to_string());
                }
                footer = ui.cursor().top() - top;
            });
            if window {
                crate::app::chat::mini_window::resize_edges(ui, rect);
            }
        });
        self.chat_draft = draft;
        if let Some(at) = moved {
            self.mini.drag = Some(at);
        }
        if stopped && let Some(at) = self.mini.drag.take() {
            self.prefs.mini_chat_at = Some([at.x, at.y]);
        }
        if (footer - self.mini.footer).abs() > 0.5 {
            ctx.request_repaint();
        }
        self.mini.footer = footer;
        self.conv_apply(ctx, conv, draft_id);
        if let Some(name) = slash_pick {
            crate::app::chat::commands::complete_into(ctx, &mut self.chat_draft, &name, draft_id);
        }
        if crate::app::chat::commands::take_login_ask(ctx) && self.is_chat(&id) {
            self.chat_submit(ctx, &id, "/login", "/login");
        }
        if crate::app::chat::commands::take_retry_ask(ctx) && self.is_chat(&id) {
            self.chat_retry(ctx, &id);
        }
        if attach && let Some(paths) = rfd::FileDialog::new().set_title("Anexar imagem").add_filter("Imagens", &IMAGE_EXTENSIONS).pick_files() {
            self.chat_add_files(ctx, paths, false);
        }
        if interrupt {
            self.interrupt_agent();
        }
        if dictate {
            self.dictate_toggle();
        }
        if voice.0 {
            self.voice_toggle();
        }
        if voice.1 {
            self.voice_mute();
        }
        if voice.2 {
            self.voice_speakers();
        }
        if let Some(act) = footer_act {
            self.chat_footer_act(act);
        }
        if let Some(text) = send {
            self.send_from_box(ctx, &id, text);
        }
        if expand {
            self.mini.open = false;
            self.chat_agent = Some(id);
            self.show_chat();
        } else if close {
            self.mini.open = false;
        }
    }
}

fn working_now(row: &AgentRow) -> bool {
    matches!(row.state, AgentState::Working(_))
}
