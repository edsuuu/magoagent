//! The chat screen (⌘1, back to the canvas with ⌘2): the conversation of the agent running in a
//! terminal, read from Claude Code's own files, the agents beside it and a box to write to it.
//! When the agent waits for an answer in its terminal, the terminal's last lines show in the chat.

use crate::prelude::*;
use crate::app::ui::agents::{AMBER, AgentState, RED};
use crate::app::ui::terminal_icon;
use crate::transcript::{ChatItem, Transcript, claude_dir};

const SIDEBAR: f32 = 250.0;

impl DrawingApp {
    pub(crate) fn chat_screen(&mut self, ctx: &Context) {
        if !self.chat_open {
            return;
        }
        let rows = self.agent_rows();
        if self.chat_agent.as_ref().is_none_or(|id| !rows.iter().any(|r| &r.id == id)) {
            // the Maestro by default: the first terminal running claude, else the first one
            let maestro = rows.iter().find(|r| r.spec.command.contains("claude")).or(rows.first());
            self.chat_agent = maestro.map(|r| r.id.clone());
        }
        self.refresh_transcript();
        // what other agents asked this one over the bridge: (prompt, who asked)
        let asked: Vec<(String, String)> = match (&self.bridge, &self.chat_agent) {
            (Some(bridge), Some(id)) => bridge.shared.lock().unwrap().asked.iter().filter(|(to, ..)| to == id).map(|(_, prompt, who)| (prompt.clone(), who.clone())).collect(),
            _ => Vec::new(),
        };
        let (c, island, dark) = (self.colors(), self.island(), self.dark());
        // the box's text lives outside the closure, which only reads self
        let mut draft = std::mem::take(&mut self.chat_draft);
        // a fixed id: the box keeps its focus from one frame to the next
        let draft_id = egui::Id::unique("chat_draft");
        let area = Rect::from_min_max(Pos2::new(0.0, TOP_ROW + 52.0), ctx.content_rect().max);
        let mut pick = None;
        let mut send = None;
        let mut to_terminal = false;
        let (mut toggle_terminal, mut composer) = (false, None);
        // the box grows a line at a time up to this; the conversation gives it the room
        let cap = 220f32.min(area.height() * 0.4);
        egui::Area::new(egui::Id::unique("chat_screen")).fixed_pos(area.min).order(egui::Order::Middle).show(ctx, |ui| {
            ui.set_min_size(area.size());
            ui.painter().rect_filled(area, 0, if dark { Color32::from_rgb(0x12, 0x12, 0x12) } else { Color32::WHITE });
            let side = Rect::from_min_size(area.min + Vec2::new(16.0, 0.0), Vec2::new(SIDEBAR, area.height() - 16.0));
            ui.scope_builder(egui::UiBuilder::new().max_rect(side), |ui| {
                island.corner_radius(12).inner_margin(10).show(ui, |ui| {
                    ui.set_min_size(side.size() - Vec2::splat(20.0));
                    ui.label(egui::RichText::new("Agentes").size(12.0).color(c.muted));
                    ui.add_space(6.0);
                    egui::ScrollArea::vertical().id_salt("chat_agents").auto_shrink(false).show(ui, |ui| {
                        for row in &rows {
                            let on = self.chat_agent.as_ref() == Some(&row.id);
                            let (rect, response) = ui.allocate_exact_size(Vec2::new(ui.available_width(), 44.0), Sense::click());
                            if on || response.hovered() {
                                // a subtle rounded band: a step above the island when selected
                                ui.painter().rect_filled(rect, 10, if on { c.surface_high } else { c.surface_low });
                            }
                            let p = ui.painter();
                            terminal_icon(p, &row.spec, rect.left_top() + Vec2::new(22.0, 16.0), c.text);
                            // its state at the right end; the words cut short before it
                            row.state.paint_dot(p, Pos2::new(rect.right() - 12.0, rect.top() + 15.0), &c);
                            let room = rect.width() - 38.0 - 26.0;
                            p.galley(rect.left_top() + Vec2::new(38.0, 7.0), clipped(p, row.spec.label().into(), FontId::proportional(13.5), c.text, room), c.text);
                            let what = row.mission.clone().unwrap_or_else(|| row.state.label().into());
                            let under = match &row.space {
                                Some(space) => format!("{space} · {what}"),
                                None => what,
                            };
                            p.galley(rect.left_top() + Vec2::new(38.0, 25.0), clipped(p, under, FontId::proportional(11.0), c.muted, room), c.muted);
                            if response.on_hover_cursor(CursorIcon::PointingHand).clicked() {
                                pick = Some(row.id.clone());
                            }
                        }
                    });
                });
            });
            let main = Rect::from_min_max(Pos2::new(side.right() + 24.0, area.top()), area.max - Vec2::new(24.0, 16.0));
            let Some(row) = self.chat_agent.as_ref().and_then(|id| rows.iter().find(|r| &r.id == id)) else {
                ui.scope_builder(egui::UiBuilder::new().max_rect(main), |ui| {
                    ui.label(egui::RichText::new("Nenhum agente no canvas: crie um terminal e rode o claude.").color(c.muted));
                });
                return;
            };
            ui.scope_builder(egui::UiBuilder::new().max_rect(main), |ui| {
                let width = main.width().min(760.0);
                let name = row.spec.label();
                let transcript = self.transcripts.get(&row.id);
                // with no conversation the screen shows already, in its place
                let talks = transcript.is_some_and(|t| !t.items.is_empty());
                ui.set_max_width(width);
                // the buttons first, at the right: a long title is cut before them
                ui.horizontal(|ui| {
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        to_terminal |= ui.button("Abrir no canvas").clicked();
                        if talks {
                            toggle_terminal = ui.selectable_label(self.chat_terminal, "Terminal").on_hover_text("A tela do terminal, ao vivo").clicked();
                        }
                        ui.with_layout(egui::Layout::left_to_right(egui::Align::Center), |ui| {
                            ui.label(egui::RichText::new(name).size(18.0).strong().color(c.text));
                            if let Some(title) = transcript.and_then(|t| t.title.clone()) {
                                ui.add(egui::Label::new(egui::RichText::new(title).size(13.0).color(c.muted)).truncate());
                            }
                        });
                    });
                });
                // what it is doing, since when or what for, and where
                ui.horizontal(|ui| {
                    let (dot, _) = ui.allocate_exact_size(Vec2::splat(10.0), Sense::hover());
                    row.state.paint_dot(ui.painter(), dot.center(), &c);
                    let note = self.state_note(row).map(|n| format!(" · {n}")).unwrap_or_default();
                    let space = row.space.as_ref().map(|s| format!(" · no espaço {s}")).unwrap_or_default();
                    let tool = transcript.and_then(|t| current_tool(&row.state, &t.items)).map(|t| format!(" · {t}")).unwrap_or_default();
                    // a long command is cut at the line's end
                    let line = format!("{}{note}{tool}{space}", row.state.label());
                    let label = ui.add(egui::Label::new(egui::RichText::new(&line).size(12.0).color(row.state.dot(&c).0)).truncate());
                    // whole on hover, cut or not
                    match row.state {
                        AgentState::Active(_) => label.on_hover_text(format!("{line}

Pela saída do terminal: este programa não diz se está trabalhando")),
                        _ => label.on_hover_text(line),
                    };
                });
                // waiting: the box under the conversation shows the screen already
                if self.chat_terminal && talks && !matches!(row.state, AgentState::Waiting) {
                    screen_box(ui, &self.screen_of(&row.id), 14);
                }
                ui.add_space(8.0);
                let composer_height = if self.chat_composer > 0.0 { self.chat_composer + 8.0 } else { 56.0 };
                egui::ScrollArea::vertical()
                    .stick_to_bottom(true)
                    .auto_shrink(false)
                    .max_height(ui.available_height() - composer_height)
                    .show(ui, |ui| {
                        ui.set_max_width(width);
                        // tight lines inside a bubble; the gap between items comes from chat_item
                        ui.spacing_mut().item_spacing.y = 4.0;
                        match transcript {
                            Some(t) if talks => {
                                // each question marks one message, the latest that says it, newest first
                                let mut from = std::collections::HashMap::new();
                                for (prompt, who) in asked.iter().rev() {
                                    let says = |(i, item): &(usize, &ChatItem)| !from.contains_key(i) && matches!(item, ChatItem::User(text) if prompt.split_whitespace().eq(text.split_whitespace()));
                                    if let Some((i, _)) = t.items.iter().enumerate().rev().find(says) {
                                        from.insert(i, who.as_str());
                                    }
                                }
                                for (i, item) in t.items.iter().enumerate() {
                                    self.chat_item(ui, item, width, &c, from.get(&i).copied());
                                }
                            }
                            _ => {
                                let note = "Este terminal não tem uma conversa do Claude Code; esta é a tela dele.";
                                ui.label(egui::RichText::new(note).size(12.0).color(c.muted));
                                // waiting: the box below shows the screen already
                                if !matches!(row.state, AgentState::Waiting) {
                                    screen_box(ui, &self.screen_of(&row.id), 40);
                                }
                            }
                        }
                        if matches!(row.state, AgentState::Waiting) {
                            // the "shell window": the agent stopped on a question only its terminal answers
                            ui.add_space(10.0);
                            egui::Frame::new().stroke(Stroke::new(1.0, AMBER)).corner_radius(10).inner_margin(10).show(ui, |ui| {
                                ui.label(egui::RichText::new(format!("{name} espera a sua resposta no terminal")).color(AMBER).strong());
                                screen_box(ui, &self.screen_of(&row.id), 14);
                                to_terminal |= ui.button("Abrir o terminal").clicked();
                            });
                        }
                    });
                ui.add_space(8.0);
                // the box, as the canvas composer: one line that grows with Shift+Enter up to its
                // cap, the send button on its last line
                let frame = egui::Frame::new()
                    .fill(c.surface_low)
                    .corner_radius(19)
                    .shadow(egui::epaint::Shadow { offset: [0, 4], blur: 14, spread: 0, color: Color32::from_black_alpha(36) })
                    .inner_margin(egui::Margin { left: 16, right: 6, top: 6, bottom: 6 })
                    .show(ui, |ui| {
                        ui.set_width(width - 22.0);
                        // an Area offers only last frame's size: room for the text to grow up to the cap
                        ui.set_max_height(cap);
                        // read before the box takes the key: Response::has_focus is false in the
                        // test window, and Shift lives on the key event, not the global modifiers
                        let enter = ui.memory(|m| m.has_focus(draft_id))
                            && ui.input(|i| i.events.iter().any(|e| matches!(e, egui::Event::Key { key: Key::Enter, pressed: true, modifiers, .. } if !modifiers.shift)));
                        egui::ScrollArea::vertical().id_salt("chat_draft_scroll").auto_shrink([false, true]).max_height(cap - 12.0).show(ui, |ui| {
                            let edit = egui::TextEdit::multiline(&mut draft)
                                .id(draft_id)
                                .hint_text(format!("Mensagem para {name}"))
                                .font(FontId::proportional(13.0))
                                .margin(egui::Margin::symmetric(0, 6))
                                .desired_rows(1)
                                .desired_width(width - 58.0)
                                .return_key(Some(egui::KeyboardShortcut::new(Modifiers::SHIFT, Key::Enter)))
                                .frame(egui::Frame::NONE);
                            ui.add(edit);
                        });
                        // over the right end of the last line: a row would cap the text's height
                        let inner = ui.min_rect();
                        let rect = Rect::from_center_size(Pos2::new(inner.max.x - 14.0, inner.max.y - 14.0), Vec2::splat(28.0));
                        let button = ui.interact(rect, ui.scope_id().with("chat_send"), Sense::click());
                        let ready = !draft.trim().is_empty();
                        // ready: the primary color (white in dark) with a dark arrow, else a faint disc
                        let (fill, ink) = match (ready, dark) {
                            (true, true) => (Color32::WHITE, Color32::BLACK),
                            (true, false) => (Color32::BLACK, Color32::WHITE),
                            _ => (c.text.gamma_multiply(0.1), c.muted),
                        };
                        let (p, m, stroke) = (ui.painter(), rect.center(), Stroke::new(1.6, ink));
                        p.circle_filled(m, 14.0, fill);
                        p.line_segment([m + Vec2::new(0.0, 5.0), m + Vec2::new(0.0, -5.0)], stroke);
                        p.line_segment([m + Vec2::new(-4.5, -0.5), m + Vec2::new(0.0, -5.0)], stroke);
                        p.line_segment([m + Vec2::new(4.5, -0.5), m + Vec2::new(0.0, -5.0)], stroke);
                        // the Enter may have added its newline to the draft: trim it off
                        if ready && (enter || button.on_hover_cursor(CursorIcon::PointingHand).clicked()) {
                            send = Some(std::mem::take(&mut draft).trim_end().to_string());
                        }
                    });
                composer = Some(frame.response.rect.height());
            });
        });
        self.chat_draft = draft;
        if let Some(height) = composer {
            // it grew or shrank: the conversation takes its new room next frame
            if (height - self.chat_composer).abs() > 0.5 {
                ctx.request_repaint();
            }
            self.chat_composer = height;
        }
        if toggle_terminal {
            self.chat_terminal = !self.chat_terminal;
        }
        if let Some(id) = pick {
            self.open_agent(&id);
        }
        // refused (it waits on a menu, its claude left): the words stay in the box
        if let (Some(text), Some(id)) = (send, self.chat_agent.clone()) {
            if !self.send_prompt(&id, &text) {
                self.chat_draft = text;
            }
        }
        if to_terminal {
            self.chat_open = false;
            if let Some(id) = self.chat_agent.clone() {
                self.open_agent(&id);
            }
        }
    }

    /// One message; `from` names the agent that asked it over the bridge.
    fn chat_item(&self, ui: &mut egui::Ui, item: &ChatItem, width: f32, c: &UiColors, from: Option<&str>) {
        match item {
            ChatItem::User(text) => {
                let right = egui::Layout::right_to_left(egui::Align::TOP);
                // another agent's question: said whose, in a grey bubble
                if let Some(who) = from {
                    ui.with_layout(right, |ui| ui.label(egui::RichText::new(format!("Pergunta de {who}")).size(11.0).color(c.muted)));
                }
                let (fill, ink) = if from.is_some() { (c.surface_high, c.text) } else { (bubble_fill(self.dark(), true), Color32::WHITE) };
                ui.with_layout(right, |ui| {
                    egui::Frame::new().fill(fill).corner_radius(16).inner_margin(egui::Margin::symmetric(12, 7)).show(ui, |ui| {
                        // a gutter of 48 pt on the other side
                        ui.set_max_width(width - 48.0);
                        ui.label(egui::RichText::new(text).size(14.0).color(ink));
                    });
                });
            }
            ChatItem::System(text) => {
                ui.label(egui::RichText::new(text).size(11.5).italics().color(c.muted));
            }
            ChatItem::Error(text) => {
                ui.label(egui::RichText::new(text).size(12.0).color(RED));
            }
            ChatItem::Agent(text) => prose(ui, text, c, self.dark(), width, true),
            ChatItem::Tool(name, what) => {
                ui.label(egui::RichText::new(format!("• {name}  {what}")).size(11.5).monospace().color(c.muted));
            }
            ChatItem::Image { data, user, .. } => {
                let mut images = self.chat_images.borrow_mut();
                let texture = images.entry(image_key(data)).or_insert_with(|| chat_texture(ui.ctx(), data));
                if let Some(texture) = texture {
                    // 420 pt wide at most: a small one isn't blown up
                    let size = texture.size_vec2() * (420.0_f32.min(texture.size_vec2().x) / texture.size_vec2().x);
                    let side = if *user { egui::Layout::right_to_left(egui::Align::TOP) } else { egui::Layout::left_to_right(egui::Align::TOP) };
                    ui.with_layout(side, |ui| ui.image((texture.id(), size)));
                }
            }
        }
        // with the 4 pt between parts of a message: about 12 pt between messages
        ui.add_space(8.0);
    }

    /// Finds which Claude Code conversation the chat's terminal runs and reads what's new, twice
    /// a second at most.
    fn refresh_transcript(&mut self) {
        let Some(id) = self.chat_agent.clone() else { return };
        if self.now - self.chat_polled < 0.5 {
            return;
        }
        self.chat_polled = self.now;
        let Some(dir) = claude_dir() else { return };
        // which session it runs: `check_agents` keeps that for every terminal
        match self.agent_sessions.get(&id).cloned() {
            Some(session) => {
                let path = session.transcript(&dir);
                if self.transcripts.get(&id).is_none_or(|t| t.path() != path) {
                    self.transcripts.insert(id.clone(), Transcript::new(path));
                    self.chat_images.borrow_mut().clear();
                }
            }
            None => {
                self.transcripts.remove(&id);
                self.chat_images.borrow_mut().clear();
            }
        }
        if let Some(transcript) = self.transcripts.get_mut(&id) {
            transcript.poll();
            // the textures of images no longer in the chat (old messages left, another agent's) go
            let shown: std::collections::HashSet<_> = transcript.items.iter().filter_map(|i| if let ChatItem::Image { data, .. } = i { Some(image_key(data)) } else { None }).collect();
            self.chat_images.borrow_mut().retain(|key, _| shown.contains(key));
        }
    }

    fn screen_of(&self, id: &str) -> String {
        self.terminals.get(id).and_then(|s| s.as_ref().ok()).map(|s| s.handle.screen_text()).unwrap_or_default()
    }
}

/// The tool it runs now: while it works, the last thing it did, if that's a tool call.
fn current_tool(state: &AgentState, items: &[ChatItem]) -> Option<String> {
    match (state, items.last()?) {
        (AgentState::Working(_), ChatItem::Tool(name, what)) => Some(format!("{name} {what}").trim_end().to_string()),
        _ => None,
    }
}

/// An image of the chat by size and opening bytes: positions shift as old messages leave.
fn image_key(data: &str) -> (usize, String) {
    (data.len(), data.chars().take(48).collect())
}

/// The chat's own texture of a base64 image, at most twice the 420 pt it's shown at: not in
/// `image_texture`'s cache, which keeps every one for good.
// ponytail: decoded on the UI thread, a hitch when a large one arrives; upgrade: decode in a thread
fn chat_texture(ctx: &egui::Context, data: &str) -> Option<egui::TextureHandle> {
    let image = image::load_from_memory(&crate::render::base64_decode(data)?).ok()?;
    // and no taller than the GPU's texture limit: a full-page capture would go past it
    let tallest = ctx.input(|i| i.max_texture_side) as u32;
    let image = if image.width() > 840 || image.height() > tallest { image.resize(840, tallest, image::imageops::FilterType::Triangle) } else { image };
    let rgba = image.to_rgba8();
    let pixels = egui::ColorImage::from_rgba_unmultiplied([rgba.width() as usize, rgba.height() as usize], &rgba);
    Some(ctx.load_texture("chat-image", pixels, egui::TextureOptions::LINEAR))
}

/// One line of text cut with "…" to fit `width`.
fn clipped(painter: &Painter, text: String, font: FontId, color: Color32, width: f32) -> std::sync::Arc<egui::Galley> {
    let mut job = egui::text::LayoutJob::simple_singleline(text, font, color);
    job.wrap = egui::text::TextWrapping::truncate_at_width(width);
    painter.layout_job(job)
}

fn bubble_fill(dark: bool, outgoing: bool) -> Color32 {
    match (outgoing, dark) {
        (true, true) => Color32::from_rgb(0x0a, 0x84, 0xff),
        (true, false) => Color32::from_rgb(0x00, 0x7a, 0xff),
        (false, true) => Color32::from_rgb(0x3b, 0x3b, 0x3e),
        (false, false) => Color32::from_rgb(0xe9, 0xe9, 0xeb),
    }
}

/// White at `alpha` on the dark look, black on the light one: the translucent lines of the chat.
fn tint(dark: bool, alpha: u8) -> Color32 {
    if dark { Color32::from_white_alpha(alpha) } else { Color32::from_black_alpha(alpha) }
}

/// The agent's words, in the light Markdown it writes: ``` blocks, headings, lists, quotes,
/// tables, **bold** and `code`. Each block of code is a card; the words are in `prose_blocks`.
/// `bubbles`: each run of words in a bubble of its own (the chat); off, plain text (the IDE's docs).
pub(crate) fn prose(ui: &mut egui::Ui, text: &str, c: &UiColors, dark: bool, width: f32, bubbles: bool) {
    for (k, part) in text.split("```").enumerate() {
        if k % 2 == 1 {
            // the first line of a fence names the language
            let code = part.split_once('\n').map_or(part, |(_, code)| code).trim_end();
            egui::Frame::new().fill(c.surface_low).corner_radius(8).inner_margin(10).show(ui, |ui| {
                ui.label(egui::RichText::new(code).monospace().size(12.5).color(c.text));
            });
            continue;
        }
        prose_blocks(ui, part, c, dark, width, bubbles);
    }
}

/// The blocks of one run of words: each paragraph a bubble of its own, each table a card in the
/// same column.
fn prose_blocks(ui: &mut egui::Ui, part: &str, c: &UiColors, dark: bool, width: f32, bubbles: bool) {
    let lines: Vec<&str> = part.trim_matches('\n').lines().map(str::trim_end).collect();
    for (is_table, range) in blocks(&lines) {
        let block = &lines[range];
        if is_table {
            let rows: Vec<&str> = block.iter().map(|l| l.trim_start()).collect();
            table(ui, &rows, c, dark);
        } else if !bubbles {
            md_lines(ui, block, c, dark);
        } else {
            egui::Frame::new().fill(bubble_fill(dark, false)).corner_radius(16).inner_margin(egui::Margin::symmetric(12, 7)).show(ui, |ui| {
                // the bubble hugs its words: up to 720 wide, and 48 pt short of the column's edge
                ui.set_max_width((width - 48.0).min(720.0) - 24.0);
                md_lines(ui, block, c, dark);
            });
        }
    }
}

/// Splits the lines of a run of words into blocks: (is a table, its lines). Blank lines end a
/// paragraph; a table starts at a `|` line whose next line is its |---| row.
fn blocks(lines: &[&str]) -> Vec<(bool, std::ops::Range<usize>)> {
    let table_at = |i: usize| lines[i].trim_start().starts_with('|') && lines.get(i + 1).is_some_and(|l| rule(l.trim_start()));
    let mut out = vec![];
    let mut i = 0;
    while i < lines.len() {
        if lines[i].trim().is_empty() {
            i += 1;
            continue;
        }
        let (start, table) = (i, table_at(i));
        if table {
            while i < lines.len() && lines[i].trim_start().starts_with('|') {
                i += 1;
            }
        } else {
            while i < lines.len() && !lines[i].trim().is_empty() && !table_at(i) {
                i += 1;
            }
        }
        out.push((table, start..i));
    }
    out
}

/// The lines of one paragraph block: headings, quotes, lists and plain lines, as Markdown.
fn md_lines(ui: &mut egui::Ui, lines: &[&str], c: &UiColors, dark: bool) {
    for &line in lines {
        let t = line.trim_start();
        let hashes = t.chars().take_while(|&ch| ch == '#').count();
        if (1..=6).contains(&hashes) && t[hashes..].starts_with(' ') {
            ui.add_space(4.0);
            md_line(ui, &t[hashes + 1..], [20.0, 17.0, 15.5][hashes.min(3) - 1], c.text, true, c);
            continue;
        }
        if let Some(quote) = t.strip_prefix('>') {
            egui::Frame::new().inner_margin(egui::Margin { left: 12, ..Default::default() }).show(ui, |ui| {
                let r = md_line(ui, quote.trim_start(), 14.0, c.muted, false, c);
                // the quote's bar, 3 pt and translucent so it reads on the grey bubble
                ui.painter().vline(r.left() - 6.0, r.y_range(), Stroke::new(3.0, tint(dark, 64)));
            });
            continue;
        }
        let number = t.split_once(". ").filter(|(n, _)| !n.is_empty() && n.len() < 4 && n.bytes().all(|b| b.is_ascii_digit()));
        match (t.strip_prefix("- ").or(t.strip_prefix("* ")), number) {
            (Some(rest), _) => list_item(ui, "•", rest, line.len() - t.len(), c),
            (None, Some((n, rest))) => list_item(ui, &format!("{n}."), rest, line.len() - t.len(), c),
            _ => {
                md_line(ui, t, 14.0, c.text, false, c);
            }
        }
    }
}

/// A list item; nested ones step in by the source's indentation.
fn list_item(ui: &mut egui::Ui, mark: &str, rest: &str, indent: usize, c: &UiColors) {
    ui.horizontal_top(|ui| {
        ui.add_space(4.0 + indent as f32 * 7.0);
        ui.label(egui::RichText::new(mark).size(14.0).color(c.muted));
        md_line(ui, rest, 14.0, c.text, false, c);
    });
}

/// One line of Markdown, wrapped and selectable, with its `code` and **bold** runs (`bold`: all
/// of it). The UI font has no bold weight, so bold runs are drawn a second time one pixel to
/// the right: the same layout with everything else transparent.
fn md_line(ui: &mut egui::Ui, text: &str, size: f32, color: egui::Color32, bold: bool, c: &UiColors) -> egui::Rect {
    let width = ui.available_width();
    let (mut job, mut ghost, mut any) = (egui::text::LayoutJob::default(), egui::text::LayoutJob::default(), false);
    let mut add = |run: &str, font: egui::FontId, color, background, thick: bool| {
        let format = |color, background| egui::TextFormat { font_id: font.clone(), color, background, ..Default::default() };
        job.append(run, 0.0, format(color, background));
        ghost.append(run, 0.0, format(if thick { color } else { egui::Color32::TRANSPARENT }, egui::Color32::TRANSPARENT));
        any |= thick && !run.is_empty();
    };
    // inside **…**; `code` spans don't reset it
    let mut on = false;
    for (k, piece) in text.split('`').enumerate() {
        if k % 2 == 1 {
            add(piece, egui::FontId::monospace(size - 1.5), c.text, c.surface_low, false);
            continue;
        }
        for (b, run) in piece.split("**").enumerate() {
            if b > 0 {
                on = !on;
            }
            add(run, egui::FontId::proportional(size), color, egui::Color32::TRANSPARENT, bold || on);
        }
    }
    (job.wrap.max_width, ghost.wrap.max_width) = (width, width);
    let rect = ui.add(egui::Label::new(job).wrap()).rect;
    if any {
        let ghost = ui.fonts_mut(|f| f.layout_job(ghost));
        // a whole pixel: text snaps to the pixel grid, a fraction could round back onto the original
        ui.painter().galley(rect.min + egui::vec2(1.0 / ui.pixels_per_point(), 0.0), ghost, egui::Color32::TRANSPARENT);
    }
    rect
}

/// A Markdown table as a grid: the header muted and bold, the rows striped, the columns as wide as
/// their words, all in one thin frame of radius 8.
fn table(ui: &mut egui::Ui, rows: &[&str], c: &UiColors, dark: bool) {
    let cells = table_cells(rows);
    egui::Frame::new().stroke(Stroke::new(1.0, tint(dark, 26))).corner_radius(8).show(ui, |ui| {
        egui::Grid::new(ui.next_auto_id()).striped(true).spacing([0.0, 0.0]).show(ui, |ui| {
            for (n, row) in cells.iter().enumerate() {
                for cell in row {
                    egui::Frame::new().inner_margin(egui::Margin::symmetric(10, 6)).show(ui, |ui| {
                        // ponytail: cells don't wrap, so the grid sizes columns by their words (its
                        // first-frame widths would stick and break every word); a cell wider than
                        // the line runs past the frame. Upgrade: measure the columns and wrap to them.
                        ui.set_max_width(f32::INFINITY);
                        md_line(ui, cell, 13.0, if n == 0 { tint(dark, 140) } else { c.text }, n == 0, c);
                    });
                }
                ui.end_row();
            }
        });
    });
}

/// The |---| row of a table.
fn rule(line: &str) -> bool {
    line.starts_with('|') && line.chars().all(|ch| "|-: ".contains(ch))
}

/// The cells of a table's rows, without the |---| row; their ** and ` marks stay for `md_line`.
fn table_cells<'a>(rows: &[&'a str]) -> Vec<Vec<&'a str>> {
    rows.iter().copied().filter(|r| !rule(r)).map(|r| r.trim().trim_matches('|').split('|').map(str::trim).collect()).collect()
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
    #[test]
    fn markdown_tables_split_into_cells() {
        let rows = ["| arquivo | linhas |", "|---------|:------:|", "| `maestro.rs` | 93 |", "| **ação.rs** | +5 |"];
        assert_eq!(super::table_cells(&rows), [vec!["arquivo", "linhas"], vec!["`maestro.rs`", "93"], vec!["**ação.rs**", "+5"]]);
        assert!(super::rule("|:--|---:|") && !super::rule("| a |"));
    }

    #[test]
    fn chat_text_splits_into_paragraphs_and_tables() {
        let lines = ["Oi", "tudo bem", "", "| a | b |", "|---|---|", "| 1 | 2 |", "fim"];
        assert_eq!(super::blocks(&lines), vec![(false, 0..2), (true, 3..6), (false, 6..7)]);
    }

    #[test]
    fn a_chat_image_is_kept_at_twice_the_width_it_is_shown() {
        let mut png = Vec::new();
        image::RgbaImage::new(2000, 100).write_to(&mut std::io::Cursor::new(&mut png), image::ImageFormat::Png).unwrap();
        let ctx = eframe::egui::Context::default();
        let texture = super::chat_texture(&ctx, &crate::render::base64(&png)).unwrap();
        assert_eq!(texture.size(), [840, 42]);
        let mut tall = Vec::new();
        image::RgbaImage::new(100, 9000).write_to(&mut std::io::Cursor::new(&mut tall), image::ImageFormat::Png).unwrap();
        assert_eq!(super::chat_texture(&ctx, &crate::render::base64(&tall)).unwrap().size()[1], ctx.input(|i| i.max_texture_side), "under the GPU's limit");
        assert!(super::chat_texture(&ctx, "não é base64").is_none());
    }

    #[test]
    fn the_header_names_the_tool_only_while_it_works_on_it() {
        use crate::app::ui::agents::AgentState;
        use crate::transcript::ChatItem;
        let tool = vec![ChatItem::Agent("vou rodar".into()), ChatItem::Tool("Bash".into(), "cargo test".into())];
        assert_eq!(super::current_tool(&AgentState::Working(0.0), &tool).as_deref(), Some("Bash cargo test"));
        assert_eq!(super::current_tool(&AgentState::Idle(None), &tool), None, "done: nothing runs");
        let said = vec![ChatItem::Tool("Bash".into(), "ls".into()), ChatItem::Agent("pronto".into())];
        assert_eq!(super::current_tool(&AgentState::Working(0.0), &said), None, "it talks after the tool");
    }
}
