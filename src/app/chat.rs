//! The chat screen (⌘1, back to the canvas with ⌘2): the conversation of the agent running in a
//! terminal, read from Claude Code's own files, the agents beside it and a box to write to it.
//! When the agent waits for an answer in its terminal, the terminal's last lines show in the chat.

use crate::prelude::*;
use crate::app::ui::agents::{AMBER, AgentState};
use crate::transcript::{ChatItem, Transcript, assign, claude_dir, project_folder, sessions};

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
        let (c, island, dark) = (self.colors(), self.island(), self.dark());
        // the box's text lives outside the closure, which only reads self
        let mut draft = std::mem::take(&mut self.chat_draft);
        let area = Rect::from_min_max(Pos2::new(0.0, TOP_ROW + 52.0), ctx.content_rect().max);
        let mut pick = None;
        let mut send = None;
        let mut to_terminal = false;
        egui::Area::new(egui::Id::unique("chat_screen")).fixed_pos(area.min).order(egui::Order::Middle).show(ctx, |ui| {
            ui.set_min_size(area.size());
            ui.painter().rect_filled(area, 0, if dark { Color32::from_rgb(0x12, 0x12, 0x12) } else { Color32::WHITE });
            let side = Rect::from_min_size(area.min + Vec2::new(16.0, 0.0), Vec2::new(SIDEBAR, area.height() - 16.0));
            ui.scope_builder(egui::UiBuilder::new().max_rect(side), |ui| {
                island.corner_radius(12).inner_margin(10).show(ui, |ui| {
                    ui.set_min_size(side.size() - Vec2::splat(20.0));
                    ui.label(egui::RichText::new("Agentes").size(12.0).color(c.muted));
                    ui.add_space(6.0);
                    for row in &rows {
                        let on = self.chat_agent.as_ref() == Some(&row.id);
                        let (rect, response) = ui.allocate_exact_size(Vec2::new(ui.available_width(), 40.0), Sense::click());
                        if on || response.hovered() {
                            ui.painter().rect_filled(rect, 8, if on { c.selected } else { c.surface_high });
                        }
                        let (color, filled) = row.state.dot(&c);
                        let dot = rect.left_center() + Vec2::new(12.0, -6.0);
                        if filled {
                            ui.painter().circle_filled(dot, 4.0, color);
                        } else {
                            ui.painter().circle_stroke(dot, 3.5, Stroke::new(1.2, color));
                        }
                        ui.painter().text(rect.left_top() + Vec2::new(24.0, 6.0), egui::Align2::LEFT_TOP, row.spec.label(), FontId::proportional(13.5), c.text);
                        let under = row.mission.clone().unwrap_or_else(|| row.state.label().into());
                        let under: String = under.chars().take(32).collect();
                        ui.painter().text(rect.left_top() + Vec2::new(24.0, 23.0), egui::Align2::LEFT_TOP, under, FontId::proportional(11.0), c.muted);
                        if response.on_hover_cursor(CursorIcon::PointingHand).clicked() {
                            pick = Some(row.id.clone());
                        }
                    }
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
                let width = main.width().min(820.0);
                let name = row.spec.label();
                let transcript = self.transcripts.get(&row.id);
                ui.horizontal(|ui| {
                    ui.label(egui::RichText::new(name).size(18.0).strong().color(c.text));
                    if let Some(title) = transcript.and_then(|t| t.title.clone()) {
                        ui.label(egui::RichText::new(title).size(13.0).color(c.muted));
                    }
                });
                ui.add_space(8.0);
                let composer_height = 86.0;
                egui::ScrollArea::vertical()
                    .stick_to_bottom(true)
                    .auto_shrink(false)
                    .max_height(ui.available_height() - composer_height)
                    .show(ui, |ui| {
                        ui.set_max_width(width);
                        match transcript {
                            Some(t) if !t.items.is_empty() => {
                                for item in &t.items {
                                    self.chat_item(ui, item, width, &c);
                                }
                            }
                            _ => {
                                let note = "Este terminal não tem uma conversa do Claude Code; esta é a tela dele.";
                                ui.label(egui::RichText::new(note).size(12.0).color(c.muted));
                                // waiting: the box below shows the screen already
                                if !matches!(row.state, AgentState::Waiting) {
                                    screen_box(ui, &self.screen_of(&row.id), 40, &c);
                                }
                            }
                        }
                        if matches!(row.state, AgentState::Waiting) {
                            // the "shell window": the agent stopped on a question only its terminal answers
                            ui.add_space(10.0);
                            egui::Frame::new().stroke(Stroke::new(1.0, AMBER)).corner_radius(10).inner_margin(10).show(ui, |ui| {
                                ui.label(egui::RichText::new(format!("{name} espera a sua resposta no terminal")).color(AMBER).strong());
                                screen_box(ui, &self.screen_of(&row.id), 14, &c);
                                to_terminal = ui.button("Abrir o terminal").clicked();
                            });
                        }
                    });
                ui.add_space(8.0);
                island.corner_radius(12).inner_margin(8).show(ui, |ui| {
                    ui.set_width(width);
                    let edit = egui::TextEdit::multiline(&mut draft)
                        .hint_text(format!("Mensagem para {name} — Enter envia, Shift+Enter quebra a linha"))
                        .desired_rows(2)
                        .desired_width(width - 16.0)
                        .return_key(Some(egui::KeyboardShortcut::new(Modifiers::SHIFT, Key::Enter)))
                        .frame(egui::Frame::NONE);
                    let response = ui.add(edit);
                    if response.has_focus() && ui.input(|i| i.key_pressed(Key::Enter) && !i.modifiers.shift) && !draft.trim().is_empty() {
                        send = Some(std::mem::take(&mut draft));
                    }
                });
            });
        });
        self.chat_draft = draft;
        if let Some(id) = pick {
            self.open_agent(&id);
        }
        if let (Some(text), Some(id)) = (send, self.chat_agent.clone()) {
            self.send_prompt(&id, &text);
        }
        if to_terminal {
            self.chat_open = false;
            if let Some(id) = self.chat_agent.clone() {
                self.open_agent(&id);
            }
        }
    }

    fn chat_item(&self, ui: &mut egui::Ui, item: &ChatItem, width: f32, c: &UiColors) {
        match item {
            ChatItem::User(text) => {
                ui.with_layout(egui::Layout::right_to_left(egui::Align::TOP), |ui| {
                    egui::Frame::new().fill(c.selected).corner_radius(14).inner_margin(egui::Margin::symmetric(12, 8)).show(ui, |ui| {
                        ui.set_max_width(width * 0.7);
                        ui.label(egui::RichText::new(text).size(14.0).color(c.text));
                    });
                });
            }
            ChatItem::Agent(text) => prose(ui, text, c),
            ChatItem::Tool(name, what) => {
                ui.label(egui::RichText::new(format!("• {name}  {what}")).size(11.5).monospace().color(c.muted));
            }
            ChatItem::Image { data, media, user } => {
                // by size and opening bytes: positions shift as old messages leave
                let key = (data.len(), data.chars().take(48).collect::<String>());
                let mut images = self.chat_images.borrow_mut();
                let texture = images.entry(key).or_insert_with(|| image_texture(ui.ctx(), &format!("data:{media};base64,{data}")));
                if let Some(texture) = texture {
                    let size = texture.size_vec2() * (420.0 / texture.size_vec2().x).min(1.0);
                    let side = if *user { egui::Layout::right_to_left(egui::Align::TOP) } else { egui::Layout::left_to_right(egui::Align::TOP) };
                    ui.with_layout(side, |ui| ui.image((texture.id(), size)));
                }
            }
        }
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
        // the terminals running Claude Code: by their command or the title Claude gives them
        let claude: Vec<(String, String, i64)> = self
            .scene
            .elements
            .iter()
            .filter_map(|e| {
                let session = self.terminals.get(&e.id)?.as_ref().ok().filter(|s| !s.exited())?;
                // zsh titles a terminal with the running command; Claude Code with ✳ and its topic
                // (a title that's a folder path can contain "claude" and mean nothing)
                let title = session.title().unwrap_or_default().trim().to_lowercase();
                let runs = e.terminal.as_ref()?.command.contains("claude") || title.starts_with("claude") || title.starts_with('✳');
                Some((e.id.clone(), session.cwd.as_ref()?.to_str()?.to_string(), session.started_ms)).filter(|_| runs)
            })
            .collect();
        match assign(&claude, &sessions(&dir.join("sessions"))).into_iter().find(|(t, _)| *t == id) {
            Some((_, session)) => {
                let path = dir.join("projects").join(project_folder(&session.cwd)).join(format!("{}.jsonl", session.id));
                if self.transcripts.get(&id).is_none_or(|t| t.path() != path) {
                    self.transcripts.insert(id.clone(), Transcript::new(path));
                    self.chat_images.borrow_mut().clear();
                }
            }
            None => {
                self.transcripts.remove(&id);
            }
        }
        if let Some(transcript) = self.transcripts.get_mut(&id) {
            transcript.poll();
        }
    }

    fn screen_of(&self, id: &str) -> String {
        self.terminals.get(id).and_then(|s| s.as_ref().ok()).map(|s| s.handle.screen_text()).unwrap_or_default()
    }
}

/// The agent's words: plain paragraphs, with ``` blocks as code.
fn prose(ui: &mut egui::Ui, text: &str, c: &UiColors) {
    for (k, part) in text.split("```").enumerate() {
        if k % 2 == 1 {
            // the first line of a fence names the language
            let code = part.split_once('\n').map_or(part, |(_, code)| code).trim_end();
            egui::Frame::new().fill(c.surface_low).corner_radius(8).inner_margin(10).show(ui, |ui| {
                ui.label(egui::RichText::new(code).monospace().size(12.5).color(c.text));
            });
        } else if !part.trim().is_empty() {
            // ponytail: Markdown shown as plain text, only code fences styled; a real renderer if it matters
            ui.label(egui::RichText::new(part.trim().replace("**", "")).size(14.0).color(c.text));
        }
    }
}

/// The last `lines` lines of a terminal screen, in its font.
fn screen_box(ui: &mut egui::Ui, screen: &str, lines: usize, c: &UiColors) {
    let all: Vec<&str> = screen.lines().collect();
    let tail = all[all.len().saturating_sub(lines)..].join("\n");
    egui::Frame::new().fill(TERMINAL_BG).corner_radius(8).inner_margin(10).show(ui, |ui| {
        ui.label(egui::RichText::new(tail).monospace().size(12.0).color(c.text));
    });
}
