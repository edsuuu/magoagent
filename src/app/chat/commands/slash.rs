use super::*;

impl DrawingApp {
    pub(super) fn chat_set_model(&mut self, id: &str, arg: &str) {
        let Some(chat) = self.chat_of(id) else { return };
        let all: Vec<Choice> = models(&chat.agent).into_iter().chain(more_models(&chat.agent)).collect();
        let default = agent_default(&chat.agent).0;
        let names = models(&chat.agent).iter().map(|m| m.name.clone()).collect::<Vec<_>>().join(", ");
        if arg.is_empty() {
            let now = if chat.model.is_empty() { format!("{} (padrão do agente)", label(&all, &default)) } else { label(&all, &chat.model) };
            return self.chat_note(id, format!("Modelo: {now}\nUso: /model <nome>, como {names}; /model padrão volta ao padrão."));
        }
        let wanted = resolve_model(arg, &all);
        let stored = if wanted == default { String::new() } else { wanted.clone() };
        if let Some(c) = self.chats.iter_mut().find(|c| c.id == id) {
            c.model = stored;
        }
        self.save_chats();
        let shown = if wanted.is_empty() { "padrão do agente".to_string() } else { label(&all, &wanted) };
        self.chat_note(id, format!("Modelo: {shown}"));
    }

    pub(super) fn chat_set_effort(&mut self, id: &str, arg: &str) {
        let Some(chat) = self.chat_of(id) else { return };
        let model = if chat.model.is_empty() { agent_default(&chat.agent).0 } else { chat.model.clone() };
        let list = efforts(&chat.agent, &model);
        let names = list.iter().map(|e| e.name.to_lowercase()).collect::<Vec<_>>().join(", ");
        if arg.is_empty() {
            let now = if chat.effort.is_empty() { "padrão do agente".to_string() } else { label(&list, &chat.effort) };
            return self.chat_note(id, format!("Esforço: {now}\nUso: /effort <nível>: {names}; /effort padrão volta ao padrão."));
        }
        let Some(wanted) = resolve_effort(arg, &list) else {
            return self.chat_note(id, format!("Esforço “{arg}” não existe para este modelo. Use: {names}."));
        };
        let (default, shown) = (agent_default(&chat.agent).1, if wanted.is_empty() { "padrão do agente".to_string() } else { label(&list, &wanted) });
        if let Some(c) = self.chats.iter_mut().find(|c| c.id == id) {
            c.effort = if wanted == default { String::new() } else { wanted };
        }
        self.save_chats();
        self.chat_note(id, format!("Esforço: {shown}"));
    }

    /// The custom commands of the chat's folder and the home, looked up every 5 s.
    pub(super) fn slash_custom(&mut self, chat: &Chat) -> Vec<Entry> {
        let stale = self.slash.custom.as_ref().is_none_or(|(cwd, at, _)| *cwd != chat.cwd || self.now - at > 5.0);
        if stale {
            let found = folder(&chat.cwd).map(|d| discover(&d, std::env::home_dir().as_deref())).unwrap_or_default();
            self.slash.custom = Some((chat.cwd.clone(), self.now, found));
        }
        let own = catalog(&chat.agent);
        // a custom name the chat answers itself stays the chat's
        self.slash.custom.iter().flat_map(|(.., list)| list.iter().filter(|e| !own.iter().any(|o| o.name == e.name)).cloned()).collect()
    }

    /// The menu's rows for what the box says now; None when it is closed.
    pub(crate) fn slash_entries(&mut self) -> Option<Vec<Entry>> {
        let draft = self.chat_draft.clone();
        let query = draft.strip_prefix('/')?;
        if query.contains(char::is_whitespace) || self.slash.off.as_deref() == Some(draft.as_str()) {
            return None;
        }
        let chat = self.chat_agent.as_deref().and_then(|id| self.chat_of(id))?;
        if self.slash.last != draft {
            (self.slash.last, self.slash.sel) = (draft.clone(), 0);
        }
        let custom = self.slash_custom(&chat);
        let all: Vec<Entry> = catalog(&chat.agent).into_iter().chain(custom).collect();
        Some(filter(&all, query)).filter(|l| !l.is_empty())
    }

    /// The menu's keys while the box has focus: arrows move, Tab and Enter complete, Esc closes.
    /// True when the Enter was taken (it completed, it doesn't send).
    pub(crate) fn slash_keys(&mut self, ctx: &Context, draft: &mut String, entries: &[Entry], id: egui::Id) -> bool {
        if !ctx.memory(|m| m.has_focus(id) || m.had_focus_last_frame(id)) {
            return false;
        }
        let (down, up, tab, esc, enter) = ctx.input_mut(|i| {
            (
                i.consume_key(Modifiers::NONE, Key::ArrowDown),
                i.consume_key(Modifiers::NONE, Key::ArrowUp),
                i.consume_key(Modifiers::NONE, Key::Tab),
                i.consume_key(Modifiers::NONE, Key::Escape),
                i.events.iter().any(|e| matches!(e, egui::Event::Key { key: Key::Enter, pressed: true, modifiers, .. } if !modifiers.shift)),
            )
        });
        let n = entries.len();
        let mut sel = self.slash.sel.min(n - 1);
        if down {
            sel = (sel + 1) % n;
        }
        if up {
            sel = (sel + n - 1) % n;
        }
        self.slash.sel = sel;
        if esc {
            self.slash.off = Some(draft.clone());
        }
        let complete = tab || (enter && draft.trim_start_matches('/') != entries[sel].name);
        if complete {
            complete_into(ctx, draft, &entries[sel].name, id);
        }
        enter && complete
    }

    /// The menu over the box: a row to click completes it. `anchor` is the box's top left.
    pub(crate) fn slash_menu_ui(&self, ctx: &Context, entries: &[Entry], anchor: Pos2, width: f32, look: &Look) -> Option<String> {
        let mut picked = None;
        let selected = self.slash.sel.min(entries.len() - 1);
        egui::Area::new(egui::Id::unique("slash_menu")).order(egui::Order::Foreground).pivot(egui::Align2::LEFT_BOTTOM).fixed_pos(anchor).show(ctx, |ui| {
            egui::Frame::new().fill(look.surface).stroke(Stroke::new(1.0, look.line)).corner_radius(14).inner_margin(6).show(ui, |ui| {
                ui.set_width(width - 12.0);
                egui::ScrollArea::vertical().max_height(264.0).auto_shrink([false, true]).show(ui, |ui| {
                    ui.spacing_mut().item_spacing.y = 1.0;
                    for (i, e) in entries.iter().enumerate() {
                        let (rect, response) = ui.allocate_exact_size(Vec2::new(ui.available_width(), 30.0), Sense::click());
                        if i == selected || response.hovered() {
                            ui.painter().rect_filled(rect, 8, if i == selected { look.selected } else { look.raised });
                        }
                        if i == selected && ctx.input(|i| i.key_pressed(Key::ArrowDown) || i.key_pressed(Key::ArrowUp)) {
                            ui.scroll_to_rect(rect, None);
                        }
                        let name = if e.args.is_empty() { format!("/{}", e.name) } else { format!("/{} {}", e.name, e.args) };
                        let p = ui.painter();
                        let font = FontId::proportional(13.5);
                        let galley = p.layout_no_wrap(name, font.clone(), look.text);
                        let desc_x = rect.left() + (galley.size().x + 24.0).max(190.0);
                        p.galley(Pos2::new(rect.left() + 10.0, rect.center().y - galley.size().y / 2.0), galley, look.text);
                        let room = (rect.right() - desc_x - 8.0).max(0.0);
                        let desc = p.layout(e.desc.clone(), FontId::proportional(12.0), look.muted, room);
                        if desc.rows.len() <= 1 {
                            p.galley(Pos2::new(desc_x, rect.center().y - desc.size().y / 2.0), desc, look.muted);
                        } else {
                            // too long for the row: cut at its end
                            let cut: String = e.desc.chars().take(((room / 6.5) as usize).saturating_sub(1)).collect();
                            p.text(Pos2::new(desc_x, rect.center().y), egui::Align2::LEFT_CENTER, format!("{cut}…"), FontId::proportional(12.0), look.muted);
                        }
                        if response.on_hover_cursor(CursorIcon::PointingHand).clicked() {
                            picked = Some(e.name.clone());
                        }
                    }
                });
            });
        });
        picked
    }
}
