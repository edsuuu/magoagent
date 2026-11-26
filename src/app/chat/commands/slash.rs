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
            let known = self.slash_known();
            let found = folder(&chat.cwd).map(|d| discover(&d, std::env::home_dir().as_deref(), &known)).unwrap_or_default();

            self.slash.custom = Some((chat.cwd.clone(), self.now, found));
        }

        let own = catalog(&chat.agent);

        // a custom name the chat answers itself stays the chat's
        self.slash.custom.iter().flat_map(|(.., list)| list.iter().filter(|e| !own.iter().any(|o| o.name == e.name)).cloned()).collect()
    }

    /// Where what Claude Code said it has is kept, beside the drawing.
    fn known_path(&self) -> Option<PathBuf> {
        self.autosave.as_ref().map(|path| path.with_file_name("skills-claude.json"))
    }

    /// What Claude Code said it has, read from its file the first time.
    fn slash_known(&mut self) -> Known {
        if self.slash.known.is_none() {
            self.slash.known = Some(self.known_path().and_then(|path| read_json::<Known>(&path)).unwrap_or_default());
        }

        self.slash.known.clone().unwrap_or_default()
    }

    /// A line of a Claude chat's answer: its `init` says the skills Claude Code has, kept for the menu (and for the next
    /// chats, which have none yet).
    pub(crate) fn slash_heard(&mut self, event: &Value) {
        let Some(known) = Known::of_init(event) else { return };

        if self.slash.known.as_ref() == Some(&known) {
            return;
        }

        if let Some(path) = self.known_path()
            && let Ok(text) = serde_json::to_string(&known)
        {
            std::thread::spawn(move || _ = fs::write(path, text));
        }

        self.slash.known = Some(known);
        self.slash.custom = None;
    }

    /// The menu's rows for what the box says now; None when it is closed.
    pub(crate) fn slash_entries(&mut self) -> Option<Vec<Entry>> {
        let (id, draft) = (self.chat_agent.clone()?, self.chat_draft.clone());

        self.slash_entries_of(&id, &draft)
    }

    /// `slash_entries` for the box of chat `id` holding `draft`: the island's own box and a card's on the canvas have
    /// theirs.
    pub(crate) fn slash_entries_of(&mut self, id: &str, draft: &str) -> Option<Vec<Entry>> {
        let query = draft.strip_prefix('/')?;

        if self.slash.off.as_deref() == Some(draft) {
            return None;
        }

        let chat = self.chat_of(id)?;

        if self.slash.last != draft {
            (self.slash.last, self.slash.sel) = (draft.to_string(), 0);
            self.slash.moved.set(true);
        }

        // "/model op": the command's own list (pickers.rs), by what follows; a space after it closes the menu
        if let Some((word, rest)) = query.split_once(char::is_whitespace) {
            if rest.ends_with(char::is_whitespace) {
                return None;
            }

            let rest = rest.to_lowercase();
            let rows = self.slash_args(&chat, word)?;
            let value = |row: &Entry| row.name.split_once(' ').map_or(String::new(), |(_, value)| value.to_lowercase());
            let kept: Vec<Entry> = rows.into_iter().filter(|row| value(row).contains(&rest) || row.desc.to_lowercase().contains(&rest)).collect();

            return Some(kept).filter(|kept| !kept.is_empty());
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

        if down || up {
            self.slash.moved.set(true);
        }

        if esc {
            self.slash.off = Some(draft.clone());
        }

        // a row of a command's list: Enter takes it and sends ("/model opus"), but a subagent's fills the box
        if enter && let Some((word, _)) = entries[sel].name.split_once(' ') {
            if word == "agents" {
                complete_into(ctx, draft, &entries[sel].name, id);

                return true;
            }

            *draft = format!("/{}", entries[sel].name);

            return false;
        }

        let complete = tab || (enter && draft.trim_start_matches('/') != entries[sel].name);

        if complete {
            complete_into(ctx, draft, &entries[sel].name, id);
        }

        enter && complete
    }

    /// The menu over the box: a row to click completes it. `anchor` is the box's top left. It is never taller than the
    /// room above the box in its window (the island's is short).
    pub(crate) fn slash_menu_ui(&self, ctx: &Context, entries: &[Entry], anchor: Pos2, width: f32, look: &Look) -> Option<String> {
        let mut picked = None;
        let selected = self.slash.sel.min(entries.len() - 1);
        let room = (anchor.y - ctx.content_rect().top() - 16.0).clamp(60.0, 264.0);

        egui::Area::new(egui::Id::unique("slash_menu")).order(egui::Order::Foreground).pivot(egui::Align2::LEFT_BOTTOM).fixed_pos(anchor).show(ctx, |ui| {
            egui::Frame::new().fill(look.surface).stroke(Stroke::new(1.0, look.line)).corner_radius(14).inner_margin(6).show(ui, |ui| {
                ui.set_width(width - 12.0);

                egui::ScrollArea::vertical().max_height(room).auto_shrink([false, true]).show(ui, |ui| {
                    ui.spacing_mut().item_spacing.y = 1.0;

                    for (i, e) in entries.iter().enumerate() {
                        let (rect, response) = ui.allocate_exact_size(Vec2::new(ui.available_width(), 30.0), Sense::click());

                        if i == selected || response.hovered() {
                            ui.painter().rect_filled(rect, 8, if i == selected { look.selected } else { look.raised });
                        }

                        // the arrows were taken by `slash_keys` before the menu is drawn: it says so in `moved`
                        if i == selected && self.slash.moved.take() {
                            ui.scroll_to_rect(rect, None);
                        }

                        let name = if e.args.is_empty() { format!("/{}", e.name) } else { format!("/{} {}", e.name, e.args) };
                        let p = ui.painter();
                        let font = FontId::proportional(14.0);
                        let galley = p.layout_no_wrap(name, font.clone(), look.text);
                        let desc_x = rect.left() + (galley.size().x + 24.0).max(190.0);

                        p.galley(Pos2::new(rect.left() + 10.0, rect.center().y - galley.size().y / 2.0), galley, look.text);
                        let room = (rect.right() - desc_x - 8.0).max(0.0);
                        let desc = p.layout(e.desc.clone(), FontId::proportional(13.0), look.muted, room);

                        if desc.rows.len() <= 1 {
                            p.galley(Pos2::new(desc_x, rect.center().y - desc.size().y / 2.0), desc, look.muted);
                        } else {
                            // too long for the row: cut at its end
                            let cut: String = e.desc.chars().take(((room / 6.5) as usize).saturating_sub(1)).collect();

                            p.text(Pos2::new(desc_x, rect.center().y), egui::Align2::LEFT_CENTER, format!("{cut}…"), FontId::proportional(13.0), look.muted);
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
