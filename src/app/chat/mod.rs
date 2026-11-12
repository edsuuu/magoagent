//! The chat screen (⌘1, back to the canvas with ⌘2): the conversation of the agent running in a
//! terminal, read from Claude Code's own files, the agents beside it and the header over it
//! (chat/list.rs, chat/header.rs), and the box to write to it (chat/composer.rs). Over the box, what the agent runs now or, when it
//! waits on its terminal, that terminal's last lines (chat/steps.rs).

pub(crate) mod ask;
pub(crate) mod commands;
pub(crate) mod composer;
pub(crate) mod context;
pub(crate) mod context_ui;
pub(crate) mod context_view;
pub(crate) mod conversation;
pub(crate) mod direct;
pub(crate) mod footer;
pub(crate) mod header;
pub(crate) mod images;
pub(crate) mod list;
pub(crate) mod markdown;
pub(crate) mod media;
pub(crate) mod mini;
pub(crate) mod mini_window;
pub(crate) mod node;
pub(crate) mod node_ui;
pub(crate) mod search;
pub(crate) mod segments;
pub(crate) mod steps;
pub(crate) mod thinking;
pub(crate) mod tools;
pub(crate) mod viewer;

use crate::app::chat::composer::Hints;
use crate::app::chat::list::{CHAT_SEARCH, COLUMN, LIST, ListAction, Look, NARROW, message_menu};
use crate::app::chat::mini::ConvOut;
use crate::app::chat::steps::{CardClick, running_step};
use crate::app::ui::agents::AgentState;
use crate::prelude::*;
use crate::transcript::{ChatItem, Transcript, claude_dir};

impl DrawingApp {
    pub(crate) fn chat_screen(&mut self, ctx: &Context) {
        if !self.chat_open {
            return;
        }
        // the providers the empty chat offers (looked up again every 10 s)
        self.installed_agents();
        let mut rows = self.agent_rows();
        if self.chat_agent.as_ref().is_none_or(|id| !rows.iter().any(|r| &r.id == id)) {
            // nothing to talk to: a chat with the agent and the leave Ajustes preset, no form
            if self.chats.is_empty() && self.start_chat(true) {
                rows = self.agent_rows();
            }
            // a chat by default, Claude's first; an agent's conversation is opened from its terminal
            // (the bar's Chat button) or from the agents panel
            let chats = self.chat_rows();
            self.chat_agent = chats.iter().find(|r| r.spec.command.contains("claude")).or(chats.first()).map(|r| r.id.clone());
        }
        self.refresh_transcript();
        // a picture open covers the whole chat: the conversation isn't drawn behind it
        let agent = self.chat_agent.as_ref().and_then(|id| rows.iter().find(|r| &r.id == id)).map_or(String::new(), |r| r.spec.label().to_string());
        if self.image_viewer(ctx, &agent) {
            return;
        }
        let asked = self.asked_of();
        let (c, dark) = (self.colors(), self.dark());
        let look = Look::new(dark, &c);
        let time = ctx.input(|i| i.time);
        // the pulses and the dots move only while someone works or needs you
        if rows.iter().any(|r| matches!(r.state, AgentState::Working(_) | AgentState::Waiting)) && ctx.input(|i| i.viewport().visible()) != Some(false) {
            ctx.request_repaint_after(std::time::Duration::from_millis(33));
        }
        if ctx.input_mut(|i| i.consume_shortcut(&egui::KeyboardShortcut::new(Modifiers::COMMAND, Key::B))) {
            self.chat_list_toggled = !self.chat_list_toggled;
        }
        // ⌘F: the search field in the header
        if ctx.input_mut(|i| i.consume_shortcut(&egui::KeyboardShortcut::new(Modifiers::COMMAND, Key::F))) {
            self.chat_search.get_or_insert_default();
            ctx.memory_mut(|m| m.request_focus(egui::Id::unique(CHAT_SEARCH)));
        }
        // the slash menu over the box (chat_commands.rs); with it open Esc and the arrows are its
        let slash = self.slash_entries();
        let menu_keys = slash.is_some()
            && ctx.memory(|m| {
                let id = egui::Id::unique("chat_draft");
                m.has_focus(id) || m.had_focus_last_frame(id)
            });
        let chosen = self.chat_agent.as_ref().and_then(|id| rows.iter().find(|r| &r.id == id));
        // Esc closes the search, else stops it as in its terminal, else (no menu or dialog over the
        // chat) goes back to the canvas
        let panel_esc = self.context_esc(ctx);
        let esc = !panel_esc
            && !menu_keys
            && !egui::Popup::is_any_open(ctx)
            && !self.dialog_open()
            && !self.agents_open
            && ctx.input_mut(|i| i.consume_key(Modifiers::NONE, Key::Escape));
        let working = chosen.is_some_and(|r| matches!(r.state, AgentState::Working(_)));
        // out of self while the header edits it
        let mut search = self.chat_search.take();
        let (mut interrupt, leave) = if esc && search.take().is_none() { (working, !working) } else { (false, false) };
        // the box's text lives outside the closure, which only reads self
        let mut draft = std::mem::take(&mut self.chat_draft);
        // a fixed id: the box keeps its focus from one frame to the next
        let draft_id = egui::Id::unique("chat_draft");
        // under the title bar off the Mac, which stays over it
        let area = Rect::from_min_max(Pos2::new(0.0, TOP_ROW.max(TITLE_BAR)), ctx.content_rect().max);
        // narrow, the list starts closed and Ctrl+B opens it over the conversation
        let narrow = area.width() < NARROW;
        let full = narrow == self.chat_list_toggled;
        let pad = if narrow { 10.0 } else { 16.0 };
        let side = Rect::from_min_size(area.min + Vec2::new(pad, 0.0), Vec2::new(if full && !narrow { LIST } else { COLUMN }, area.height() - pad));
        let mut main = Rect::from_min_max(Pos2::new(side.right() + pad, area.top()), area.max - Vec2::splat(pad));
        // o painel Contexto: ao lado da conversa, ou por cima quando a janela é estreita
        let panel_id = self.chat_agent.clone().filter(|id| self.context_wanted(id));
        if let Some(id) = &panel_id {
            let open = self.prefs.chat_panel;
            self.context_tick(ctx, id, open);
        }
        let panel = panel_id.as_ref().filter(|_| self.prefs.chat_panel).map(|_| self.context_rect(area, pad, narrow));
        if let (Some(rect), false) = (panel, narrow) {
            main.max.x = (rect.left() - pad).max(main.min.x + 300.0);
        }
        let swallow = slash.as_ref().is_some_and(|entries| self.slash_keys(ctx, &mut draft, entries, draft_id));
        // with no menu, Tab does nothing in the box (lock_focus would type a tab)
        if slash.is_none() && ctx.memory(|m| m.has_focus(draft_id)) {
            ctx.input_mut(|i| i.consume_key(Modifiers::NONE, Key::Tab));
        }
        let mut slash_pick = None;
        let mut act = ListAction { canvas: leave, ..Default::default() };
        let (mut send, mut attach, mut dictate, mut pick) = (None, false, false, None);
        let mut conv = ConvOut::default();
        let mut footer_act = None;
        let (mut composer, mut footer) = (None, None);
        let (mut voice_click, mut mute_click, mut speakers_click, mut launch_click) = (false, false, false, None);
        // the box grows a line at a time up to this; the conversation gives it the room
        let cap = 200f32.min(area.height() * 0.35);
        let tool = chosen.and_then(|r| self.transcripts.get(&r.id)).and_then(running_step).map(|(s, _)| s.name);
        // the chats, and under Terminais the terminal whose conversation is open
        let mut listed = self.chat_rows();
        listed.extend(chosen.filter(|r| !self.is_chat(&r.id)).cloned());
        let mut filter = std::mem::take(&mut self.chat_listing.filter);
        egui::Area::new(egui::Id::unique("chat_screen")).fixed_pos(area.min).order(egui::Order::Middle).show(ctx, |ui| {
            ui.set_min_size(area.size());
            // the bar's strip too: the chat and the bar are one piece
            ui.painter().rect_filled(area.with_min_y(0.0), 0, look.bg);
            if full && !narrow {
                self.chat_list(ui, side, &listed, &look, tool.as_deref(), time, &mut act, &mut filter);
            } else {
                self.chat_column(ui, side, &self.chat_rows(), &look, time, &mut act);
            }
            let Some(row) = chosen else { return };
            let transcript = self.transcripts.get(&row.id);
            // with no conversation the screen shows already, in its place
            let talks = transcript.is_some_and(|t| !t.items.is_empty());
            let head = self.chat_header(ui, main, row, &look, narrow, talks, time, &mut act, &mut search);
            let query = search.as_deref().map(|q| q.trim().to_lowercase()).filter(|q| !q.is_empty());
            // the conversation takes the screen, a gutter of 32 at each side, up to 1100 wide
            let width = (main.width() - 64.0).clamp(main.width().min(360.0), 1100.0);
            let column =
                Rect::from_min_max(Pos2::new(main.center().x - width / 2.0, main.top() + head), Pos2::new(main.center().x + width / 2.0, main.bottom()));
            let name = row.spec.label();
            let waiting = row.state == AgentState::Waiting;
            // waiting, what is sent waits in the queue (`flush_chat_queue`)
            let hint = match (waiting, narrow) {
                (true, true) => format!("Mensagem para {name} (fica na fila)"),
                (true, false) => format!("Mensagem para {name} (fica na fila até você responder no terminal)"),
                _ => format!("Mensagem para {name}"),
            };
            ui.scope_builder(egui::UiBuilder::new().max_rect(column), |ui| {
                ui.set_max_width(width);
                // waiting: the box under the conversation shows the screen already
                // a chat (direct, or a terminal in chat mode) never shows the screen here
                let only_chat = self.is_chat(&row.id) || row.spec.chat;
                if self.chat_terminal && talks && !waiting && !only_chat {
                    ui.add_space(12.0);
                    screen_box(ui, &self.screen_of(&row.id), 14);
                }
                // the footer (band, box, hints) as tall as last frame
                let footer_height = if self.chat_footer > 0.0 { self.chat_footer } else { 76.0 };
                // the scroll takes the whole width, its bar at the window's right edge (not a post
                // beside the text); the conversation stays a centered column in it; a thin bar that
                // shows while scrolling or under the pointer
                let height = ui.available_height() - footer_height;
                let strip = Rect::from_min_max(Pos2::new(main.left(), ui.cursor().top()), Pos2::new(main.right(), ui.cursor().top() + height));
                // a child that doesn't widen the column (the band and the box under it stay centered)
                ui.add_space(height);
                {
                    let ui = &mut ui.new_child(egui::UiBuilder::new().max_rect(strip));
                    ui.spacing_mut().scroll = egui::style::ScrollStyle::floating();
                    egui::ScrollArea::vertical().stick_to_bottom(true).auto_shrink(false).max_height(height).show(ui, |ui| {
                        let left = ui.max_rect().left() + ((ui.available_width() - width) / 2.0).max(0.0);
                        let inner = Rect::from_min_max(Pos2::new(left, ui.cursor().top()), Pos2::new(left + width, ui.max_rect().bottom()));
                        ui.scope_builder(egui::UiBuilder::new().max_rect(inner), |ui| {
                            ui.set_width(width);
                            self.conversation(
                                ui,
                                row,
                                transcript,
                                talks,
                                only_chat,
                                waiting,
                                &asked,
                                query.as_deref(),
                                width,
                                &look,
                                &c,
                                narrow,
                                time,
                                &mut conv,
                            );
                        });
                    });
                }
                let top = ui.cursor().top();
                ui.add_space(8.0);
                if matches!(row.state, AgentState::Working(_)) {
                    interrupt |= self.work_band(ui, row, transcript, &look, narrow, time);
                    ui.add_space(8.0);
                }
                // the agent stopped on a question only its terminal answers
                if waiting {
                    match self.waiting_card(ui, row, &look, narrow, time) {
                        Some(CardClick::Terminal) => act.canvas = true,
                        Some(CardClick::Pick(from, to)) => pick = Some((from, to)),
                        None => {}
                    }
                    ui.add_space(8.0);
                }
                // the box, a card: the words, and under them its buttons and the chat's chips
                let hints = if narrow { Hints::Short } else { Hints::Full };
                let card = self.composer(ui, &mut draft, draft_id, &hint, width, cap, swallow, &look, &c, hints, time);
                let r = card.rect;
                if let Some(entries) = &slash {
                    slash_pick = self.slash_menu_ui(ctx, entries, Pos2::new(r.left(), r.top() - 6.0), r.width(), &look);
                }
                composer = Some(r.height());
                // a call that couldn't start says why over the conversation; one on is in its popup
                let cover = Rect::from_min_max(Pos2::new(main.left(), column.top()), Pos2::new(main.right(), r.top() - 4.0));
                launch_click = self.voice_stage(ui, cover, &look);
                (voice_click, mute_click, speakers_click) = card.voice;
                (attach, dictate, footer_act) = (card.attach, card.dictate, card.footer);
                if card.send {
                    // the Enter may have added its newline to the draft: trim it off
                    send = Some(std::mem::take(&mut draft).trim_end().to_string());
                }
                footer = Some(ui.cursor().top() - top);
            });
        });
        if full && narrow {
            let over = Rect::from_min_size(side.min, Vec2::new(LIST, side.height()));
            egui::Area::new(egui::Id::unique("chat_list_over")).fixed_pos(over.min).order(egui::Order::Foreground).show(ctx, |ui| {
                self.chat_list(ui, over, &listed, &look, tool.as_deref(), time, &mut act, &mut filter);
            });
            // picking one closes it
            if act.pick.is_some() {
                self.chat_list_toggled = false;
            }
        }
        if let (Some(rect), Some(id)) = (panel, &panel_id) {
            self.context_panel(ctx, rect, &look, id);
        }
        if act.context {
            self.prefs.chat_panel = !self.prefs.chat_panel;
        }
        if voice_click {
            self.voice_toggle();
        }
        if dictate {
            self.dictate_toggle();
        }
        if mute_click {
            self.voice_mute();
        }
        if speakers_click {
            self.voice_speakers();
        }
        if let Some(click) = launch_click {
            self.voice_launch_click(click);
        }
        self.chat_draft = draft;
        self.chat_search = search;
        self.chat_listing.filter = filter;
        // Abrir em miniatura: the chat closes and its panel opens over the canvas
        if act.mini {
            self.chat_open = false;
            self.mini_set(true);
        }
        if act.new_chat {
            self.start_chat(false);
        }
        if let Some(id) = act.remove.take() {
            self.remove_chat(&id);
        }
        // a chat's card (made now if this canvas has none); a terminal's row, its terminal
        if let Some(id) = act.on_canvas.take() {
            if self.is_chat(&id) {
                if let Err(why) = self.show_chat_on_canvas(&id) {
                    self.toast(why);
                }
            } else {
                self.show_canvas();
                self.open_agent(&id);
            }
        }
        self.conv_apply(ctx, conv, draft_id);
        if let Some(name) = slash_pick {
            crate::app::chat::commands::complete_into(ctx, &mut self.chat_draft, &name, draft_id);
        }
        // the login error's Entrar button
        if crate::app::chat::commands::take_login_ask(ctx)
            && let Some(id) = self.chat_agent.clone().filter(|id| self.is_chat(id))
        {
            self.chat_submit(ctx, &id, "/login", "/login");
        }
        if crate::app::chat::commands::take_retry_ask(ctx)
            && let Some(id) = self.chat_agent.clone().filter(|id| self.is_chat(id))
        {
            self.chat_retry(ctx, &id);
        }
        if let (Some(height), Some(total)) = (composer, footer) {
            // it grew or shrank: the conversation takes its new room next frame
            if (total - self.chat_footer).abs() > 0.5 {
                ctx.request_repaint();
            }
            (self.chat_composer, self.chat_footer) = (height, total);
        }
        if act.terminal {
            self.chat_terminal = !self.chat_terminal;
        }
        if act.toggle {
            self.chat_list_toggled = !self.chat_list_toggled;
        }
        if let Some(id) = act.pick {
            self.open_agent(&id);
        }
        // the same way a dropped file goes in (`handle_canvas`)
        if let Some(act) = footer_act {
            self.chat_footer_act(act);
        }
        if attach && let Some(paths) = rfd::FileDialog::new().set_title("Anexar imagem").add_filter("Imagens", &IMAGE_EXTENSIONS).pick_files() {
            self.chat_add_files(ctx, paths, false);
        }
        // a choice of the menu it waits on: the keys go to its terminal, as if typed there
        if let (Some((from, to)), Some(Ok(session))) = (pick, self.chat_agent.as_ref().and_then(|id| self.terminals.get(id))) {
            let handle = session.handle.clone();
            std::thread::spawn(move || handle.pick(from, to));
        }
        if interrupt {
            self.interrupt_agent();
        }
        // waiting on its terminal it goes in the queue; refused (its claude left): the words stay
        if let (Some(text), Some(id)) = (send, self.chat_agent.clone()) {
            self.send_from_box(ctx, &id, text);
        }
        if act.canvas {
            self.chat_open = false;
            if let Some(id) = self.chat_agent.clone() {
                self.open_agent(&id);
            }
        }
    }

    /// One message; `from` names the agent that asked it over the bridge. Its right-click menu's
    /// Citar leaves the words in `cite`.
    pub(crate) fn chat_item(&self, ui: &mut egui::Ui, item: &ChatItem, width: f32, c: &UiColors, from: Option<&str>, cite: &mut Option<String>) {
        match item {
            ChatItem::User(text) => {
                let right = egui::Layout::right_to_left(egui::Align::TOP);
                // another agent's question: said whose, in a grey bubble
                if let Some(who) = from {
                    ui.with_layout(right, |ui| ui.label(egui::RichText::new(format!("Pergunta de {who}")).size(11.0).color(c.muted)));
                }
                let (fill, ink) = if from.is_some() { (c.surface_high, c.text) } else { (bubble_fill(self.dark(), true), Color32::WHITE) };
                let bubble = ui.with_layout(right, |ui| {
                    egui::Frame::new()
                        .fill(fill)
                        .corner_radius(16)
                        .inner_margin(egui::Margin::symmetric(12, 7))
                        .show(ui, |ui| {
                            // a gutter of 48 pt on the other side
                            ui.set_max_width(width - 48.0);
                            // the right-to-left row would neither wrap it nor start its lines at the left
                            ui.add(egui::Label::new(egui::RichText::new(text).size(14.0).color(ink)).wrap().halign(egui::Align::LEFT));
                        })
                        .response
                });
                message_menu(&bubble.inner, text, cite);
            }
            // what a terminal said before it restarted, above; what comes now, below
            ChatItem::System(text) if text == EARLIER => {
                let (rect, _) = ui.allocate_exact_size(Vec2::new(ui.available_width(), 20.0), Sense::hover());
                let p = ui.painter();
                let label = p.layout_no_wrap(EARLIER.into(), FontId::proportional(11.5), c.muted);
                let (half, y, line) = (label.size().x / 2.0 + 10.0, rect.center().y, Stroke::new(1.0, tint(self.dark(), 30)));
                p.hline(rect.left()..=rect.center().x - half, y, line);
                p.hline(rect.center().x + half..=rect.right(), y, line);
                p.galley(rect.center() - label.size() / 2.0, label, c.muted);
            }
            // a command's output: several lines, in a block
            ChatItem::System(text) if text.contains('\n') => {
                egui::Frame::new().fill(c.surface_high).corner_radius(8).inner_margin(10).show(ui, |ui| {
                    ui.set_max_width(width - 20.0);
                    ui.add(egui::Label::new(egui::RichText::new(text.trim_end()).monospace().size(12.0).color(c.text)).wrap().selectable(true));
                });
            }
            ChatItem::System(text) => {
                ui.label(egui::RichText::new(text).size(11.5).italics().color(c.muted));
            }
            // the mascot's bad day at its left (the plug pulled when it is the login)
            // (a terminal's conversation is Claude Code's; a direct chat says its agent)
            ChatItem::Error(text) if !self.chats.iter().any(|ch| Some(&ch.id) == self.chat_agent.as_ref() && !ch.agent.trim_start().starts_with("claude")) => {
                ui.horizontal_top(|ui| {
                    let (rect, _) = ui.allocate_exact_size(Vec2::new(52.0, 42.0), Sense::hover());
                    let pose = crate::app::mascot::poses::failing(text);
                    crate::app::mascot::poses::paint_pose(ui.painter(), pose, rect.center(), 52.0, ui.input(|i| i.time), false, 1.0);
                    ui.vertical(|ui| {
                        ui.label(egui::RichText::new(text).size(12.0).color(c.danger));
                        if let Some(hint) = error_hint(text) {
                            ui.label(egui::RichText::new(hint).size(12.0).color(c.muted));
                        }
                    });
                });
            }
            ChatItem::Error(text) => {
                ui.label(egui::RichText::new(text).size(12.0).color(c.danger));
                if let Some(hint) = error_hint(text) {
                    ui.label(egui::RichText::new(hint).size(12.0).color(c.muted));
                }
            }
            ChatItem::Agent(text) => {
                let words = ui.scope(|ui| self.agent_words(ui, text, width, c)).response;
                message_menu(&words, text, cite);
            }
            // in runs of steps (`tool_run`)
            ChatItem::Tool(_) => return,
            ChatItem::Image { .. } => self.chat_thumb(ui, item, c),
        }
        // with the 4 pt between parts of a message: about 12 pt between messages
        ui.add_space(8.0);
    }

    /// Finds which Claude Code conversation the chat's terminal runs and reads what's new, twice
    /// a second at most.
    pub(crate) fn refresh_transcript(&mut self) {
        let Some(id) = self.chat_agent.clone() else { return };
        if self.now - self.chat_polled < 0.5 {
            return;
        }
        self.chat_polled = self.now;
        if self.is_chat(&id) {
            self.chat_refresh(&id);
            return;
        }
        let Some(dir) = claude_dir() else { return };
        self.follow_transcript(&id, &dir);
        if let Some(transcript) = self.transcripts.get_mut(&id) {
            transcript.poll();
            // the textures of images no longer in the chat (old messages left, another agent's) go
            let shown: std::collections::HashSet<_> =
                transcript.items.iter().filter_map(|i| if let ChatItem::Image { data, .. } = i { Some(image_key(data)) } else { None }).collect();
            // (0, path): a picture the agent's words name, kept (chat/media.rs)
            self.chat_images.borrow_mut().retain(|key, _| key.0 == 0 || shown.contains(key));
        }
    }

    /// Which conversation terminal `id`'s chat reads from `dir` (Claude Code's folder): the session
    /// it runs (`check_agents` keeps that for every terminal), else the one it remembers while its
    /// file is there, read once and closed by the "Conversa anterior" divider; a resume writes on
    /// in the same file, under it.
    pub(crate) fn follow_transcript(&mut self, id: &str, dir: &std::path::Path) {
        let remembered = || {
            let t = self.scene.elements.iter().find(|e| e.id == id)?.terminal.as_ref()?;
            let path = crate::transcript::transcript_path(dir, &t.claude_folder, &t.claude_session);
            path.is_file().then_some(path).filter(|_| !t.claude_session.is_empty())
        };
        let (path, earlier) = match self.agent_sessions.get(id) {
            Some(session) => (Some(session.transcript(dir)), false),
            None => (remembered(), true),
        };
        let Some(path) = path else {
            self.transcripts.remove(id);
            self.chat_images.borrow_mut().clear();
            return;
        };
        if self.transcripts.get(id).is_some_and(|t| t.path() == path) {
            return;
        }
        let mut transcript = Transcript::new(path);
        if earlier {
            transcript.poll();
            // whatever was left running there stopped with it
            transcript.running.clear();
            transcript.items.push(ChatItem::System(EARLIER.into()));
        }
        self.transcripts.insert(id.to_string(), transcript);
        self.chat_images.borrow_mut().clear();
    }

    fn screen_of(&self, id: &str) -> String {
        self.terminals.get(id).and_then(|s| s.as_ref().ok()).map(|s| s.handle.screen_text()).unwrap_or_default()
    }
}

/// The divider under a conversation a terminal remembers from before it restarted.
pub(crate) const EARLIER: &str = "Conversa anterior";

/// How many of a conversation's last items are laid out, and how many more each "Mostrar mensagens
/// anteriores" brings: a frame's work stays the same however long the conversation gets.
const CHAT_PAGE: usize = 200;

/// The first item laid out of a conversation of `len`, with `pages` earlier pages asked for.
pub(crate) fn window_start(len: usize, pages: usize) -> usize {
    len.saturating_sub(CHAT_PAGE.saturating_mul(pages.saturating_add(1)))
}

/// An image of the chat by size and opening bytes: positions shift as old messages leave.
pub(crate) fn image_key(data: &str) -> (usize, String) {
    (data.len(), data.chars().take(48).collect())
}

pub(crate) fn bubble_fill(dark: bool, outgoing: bool) -> Color32 {
    match (outgoing, dark) {
        (true, true) => Color32::from_rgb(0x0a, 0x84, 0xff),
        (true, false) => Color32::from_rgb(0x00, 0x7a, 0xff),
        (false, true) => Color32::from_rgb(0x3b, 0x3b, 0x3e),
        (false, false) => Color32::from_rgb(0xe9, 0xe9, 0xeb),
    }
}

/// White at `alpha` on the dark look, black on the light one: the translucent lines of the chat.
pub(crate) fn tint(dark: bool, alpha: u8) -> Color32 {
    if dark { Color32::from_white_alpha(alpha) } else { Color32::from_black_alpha(alpha) }
}

/// The last `lines` lines of a terminal screen, in its font.
/// The terminal's own colours, whatever the theme: its dark ground stays.
fn screen_box(ui: &mut egui::Ui, screen: &str, lines: usize) {
    let all: Vec<&str> = screen.lines().collect();
    let tail = all[all.len().saturating_sub(lines)..].join("\n");
    egui::Frame::new().fill(TERMINAL_BG).corner_radius(8).inner_margin(10).show(ui, |ui| {
        ui.label(egui::RichText::new(tail).monospace().size(12.0).color(TERMINAL_FG));
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_long_conversation_shows_its_end_and_earlier_pages_on_request() {
        assert_eq!(window_start(50, 0), 0);
        assert_eq!(window_start(200, 0), 0);
        assert_eq!(window_start(10_000, 0), 9_800);
        assert_eq!(window_start(10_000, 1), 9_600);
        assert_eq!(window_start(10_000, 49), 0);
        assert_eq!(window_start(10_000, usize::MAX), 0);
    }
}

/// What to do about an agent's error the person can fix: an expired login says how to sign in again.
pub(crate) fn error_hint(error: &str) -> Option<&'static str> {
    let login = ["Failed to authenticate", "OAuth", "Invalid API key", "Please run /login", "not logged in"];
    login.iter().any(|w| error.contains(w)).then_some("O login do agente venceu. Clique em Entrar ou digite /login aqui.")
}

#[cfg(test)]
mod error_hint_tests {
    #[test]
    fn an_expired_login_says_how_to_sign_in_again() {
        assert!(super::error_hint("Failed to authenticate: OAuth session expired and could not be refreshed").is_some_and(|h| h.contains("/login")));
        assert!(super::error_hint("Erro da API: 529 Overloaded").is_none());
    }
}
