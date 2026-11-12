use super::*;

impl DrawingApp {
    /// A Maestro of the list: a chat of yours (not one of its recruits) or a terminal set as Maestro.
    pub(super) fn is_maestro_row(&self, row: &AgentRow) -> bool {
        row.spec.maestro || self.chats.iter().any(|c| c.id == row.id && !c.recruit)
    }

    /// How long its agent has been waiting on its terminal, when its Claude Code session says.
    pub(crate) fn waiting_secs(&self, id: &str) -> Option<f64> {
        self.agent_sessions.get(id).filter(|s| s.status == "waiting" && s.status_at > 0).map(|s| (now_ms() - s.status_at).max(0) as f64 / 1000.0)
    }

    /// Under a name in the list: its state and a word more ("Precisa de você · 2 min").
    pub(super) fn list_note(&self, row: &AgentRow, tool: Option<&str>) -> String {
        let more = match row.state {
            AgentState::Working(_) => tool.map(str::to_string).or_else(|| self.state_note(row)),
            AgentState::Waiting => self.waiting_secs(&row.id).map(|s| ago(s).trim_start_matches("há ").to_string()),
            AgentState::Ended => self.agent_events.iter().rev().find(|e| e.id == row.id).map(|e| ago(e.age())),
            _ => None,
        };
        let label = if matches!(row.state, AgentState::Idle(_)) { "Pronto" } else { row.state.label() };
        match more {
            Some(more) => format!("{label} · {more}"),
            None => label.to_string(),
        }
    }

    /// When chat `id` last changed: its session file's time (Claude Code's, or what the app keeps of a
    /// Codex chat), in Unix seconds; None before its first message. Looked up again every 5 s.
    pub(super) fn chat_when(&self, id: &str) -> Option<i64> {
        let mut dates = self.chat_listing.dates.borrow_mut();
        if let Some((_, when)) = dates.get(id).filter(|(at, _)| self.now - at < 5.0) {
            return *when;
        }
        let chat = self.chats.iter().find(|c| c.id == id)?;
        let file = match (&chat.session, chat.agent.as_str()) {
            (None, _) => None,
            (Some(_), "codex") => chat_items_path(id),
            (Some(session), _) => DrawingApp::claude_transcript(&chat.cwd, session),
        };
        let when = file.and_then(|f| fs::metadata(f).ok()?.modified().ok()?.duration_since(std::time::UNIX_EPOCH).ok()).map(|d| d.as_secs() as i64);
        dates.insert(id.to_string(), (self.now, when));
        when
    }

    /// The chats under Conversas, the newest first in groups by date, with the search field over them
    /// (`filter`); a terminal whose conversation is open goes under Terminais. `tool` is what the
    /// chosen one runs now.
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn chat_list(
        &self,
        ui: &mut egui::Ui,
        side: Rect,
        rows: &[AgentRow],
        look: &Look,
        tool: Option<&str>,
        t: f64,
        act: &mut ListAction,
        filter: &mut String,
    ) {
        ui.painter().rect_filled(side, 12, look.panel);
        // the title under the window buttons, and on macOS + and « beside them, as in the reference app
        let inner = Rect::from_min_max(Pos2::new(side.left() + 8.0, (side.top() + 10.0).max(LIGHTS_BOTTOM + 10.0)), side.max - Vec2::new(8.0, 10.0));
        let head = inner.top() + 14.0;
        let tools = if cfg!(target_os = "macos") { LIGHTS_Y } else { head };
        bold(ui.painter(), Pos2::new(inner.left() + 8.0, head - 9.0), "Conversas", 14.0, look.strong);
        let fold = Rect::from_center_size(Pos2::new(inner.right() - 14.0, tools), Vec2::splat(28.0));
        let response =
            ui.interact(fold, ui.scope_id().with("chat_fold"), Sense::click()).on_hover_cursor(CursorIcon::PointingHand).tip(keys_label("Recolher lista (⌘B)"));
        if response.hovered() {
            ui.painter().rect_filled(fold, 7, look.hover);
        }
        glyph(ui.painter(), fold.center(), 16.0, 2.0, look.soft, FOLD);
        act.toggle |= response.clicked();
        // Nova conversa: a chat with an agent installed here, in a folder
        let add = fold.translate(Vec2::new(-32.0, 0.0));
        let response = ui.interact(add, ui.scope_id().with("chat_new"), Sense::click()).on_hover_cursor(CursorIcon::PointingHand).tip("Nova conversa");
        if response.hovered() {
            ui.painter().rect_filled(add, 7, look.hover);
        }
        glyph(ui.painter(), add.center(), 16.0, 2.0, look.soft, &[&[(12.0, 5.0), (12.0, 19.0)], &[(5.0, 12.0), (19.0, 12.0)]]);
        act.new_chat |= response.clicked();
        // the search over the titles and names
        let field = Rect::from_min_max(Pos2::new(inner.left() + 2.0, head + 20.0), Pos2::new(inner.right() - 2.0, head + 50.0));
        ui.painter().rect_filled(field, 8, look.hover);
        paint_icon(ui.painter(), Icon::Search, Pos2::new(field.left() + 15.0, field.center().y), look.muted);
        let edit = egui::TextEdit::singleline(filter)
            .id(egui::Id::unique("chat_list_filter"))
            .hint_text(egui::RichText::new("Buscar conversas").color(look.muted))
            .font(FontId::proportional(13.0))
            .text_color(look.text)
            .margin(egui::Margin::ZERO)
            .frame(egui::Frame::NONE);
        // the frameless field keeps no margin: the room for the magnifier is left out of its rect
        ui.put(Rect::from_min_max(field.min + Vec2::new(30.0, 6.0), field.max - Vec2::new(8.0, 6.0)), edit);
        let words = filter.trim().to_lowercase();
        let title = |row: &AgentRow| self.transcripts.get(&row.id).and_then(|t| t.title.clone()).unwrap_or_else(|| row.spec.label().to_string());
        let says = |row: &AgentRow| words.is_empty() || title(row).to_lowercase().contains(&words) || row.spec.label().to_lowercase().contains(&words);
        let all: Vec<&AgentRow> = rows.iter().filter(|r| says(r)).collect();
        // the agents a Maestro opened go under it (its subchats), not in the date groups
        let ids: Vec<&str> = all.iter().map(|r| r.id.as_str()).collect();
        let maestro: Vec<bool> = all.iter().map(|r| self.is_maestro_row(r)).collect();
        let parents = subchat_parents(&ids, &maestro, &self.links());
        let shown: Vec<&AgentRow> = all.iter().zip(&parents).filter(|(_, p)| p.is_none()).map(|(r, _)| *r).collect();
        let children = |id: &str| -> Vec<&AgentRow> {
            let Some(at) = ids.iter().position(|i| *i == id) else { return vec![] };
            all.iter().zip(&parents).filter(|(_, p)| **p == Some(at)).map(|(r, _)| *r).collect()
        };
        let now = now_ms();
        let dates: Vec<(Option<i64>, bool)> = shown.iter().map(|r| (self.chat_when(&r.id), self.is_chat(&r.id))).collect();
        let groups = date_groups(&dates, now / 1000, crate::app::chat::viewer::local_offset(now));
        let list = Rect::from_min_max(Pos2::new(inner.left(), field.bottom() + 4.0), inner.max);
        ui.scope_builder(egui::UiBuilder::new().max_rect(list), |ui| {
            egui::ScrollArea::vertical().id_salt("chat_agents").auto_shrink(false).show(ui, |ui| {
                ui.spacing_mut().item_spacing.y = 2.0;
                for (group, members) in groups {
                    let (label, _) = ui.allocate_exact_size(Vec2::new(ui.available_width(), 28.0), Sense::hover());
                    let mut job = egui::text::LayoutJob::default();
                    let format = egui::TextFormat { font_id: FontId::proportional(11.0), color: look.faint, extra_letter_spacing: 0.6, ..Default::default() };
                    job.append(&group.to_uppercase(), 0.0, format);
                    let g = ui.painter().layout_job(job);
                    ui.painter().galley(Pos2::new(label.left() + 10.0, label.bottom() - g.size().y - 4.0), g, look.faint);
                    for i in members {
                        self.list_row(ui, shown[i], &title(shown[i]), look, tool, t, act);
                        let subs = children(&shown[i].id);
                        if subs.is_empty() {
                            continue;
                        }
                        // its subchats, indented under it on a guide line
                        let top = ui.cursor().top();
                        ui.horizontal(|ui| {
                            ui.add_space(22.0);
                            ui.vertical(|ui| {
                                for sub in &subs {
                                    self.list_row(ui, sub, &title(sub), look, tool, t, act);
                                }
                            });
                        });
                        let x = ui.min_rect().left() + 20.0;
                        ui.painter().vline(x, top..=ui.cursor().top() - 8.0, Stroke::new(1.0, look.line));
                    }
                }
            });
        });
    }

    /// A conversation of the list: the agent's mark, the title, the folder and state under it after
    /// the state's dot, and ⋯ (Remover chat) under the pointer.
    #[allow(clippy::too_many_arguments)]
    pub(super) fn list_row(&self, ui: &mut egui::Ui, row: &AgentRow, title: &str, look: &Look, tool: Option<&str>, t: f64, act: &mut ListAction) {
        let on = self.chat_agent.as_ref() == Some(&row.id);
        let chat = self.is_chat(&row.id);
        let (rect, response) = ui.allocate_exact_size(Vec2::new(ui.available_width(), 52.0), Sense::click());
        let remove = |ui: &mut egui::Ui, act: &mut ListAction| {
            if menu_row(ui, Some(Icon::Map), "Mostrar no canvas", "", None) {
                act.on_canvas = Some(row.id.clone());
                ui.close();
            }
            if chat && menu_row(ui, Some(Icon::Trash), "Remover chat", "", Some(crate::app::ui::UiColors::new(ui.visuals().dark_mode).danger)) {
                act.remove = Some(row.id.clone());
                ui.close();
            }
        };
        context_menu(&response, |ui| remove(ui, act));
        let dots = Rect::from_center_size(Pos2::new(rect.right() - 18.0, rect.center().y), Vec2::splat(24.0));
        let dots_r = ui.interact(dots, ui.scope_id().with(("chat_row_more", &row.id)), Sense::click());
        let menu_open = egui::Popup::is_id_open(ui.ctx(), egui::Popup::default_response_id(&dots_r));
        let hovered = response.hovered() || dots_r.hovered() || menu_open;
        let p = ui.painter();
        if on || hovered {
            p.rect_filled(rect, 10, if on { look.surface } else { look.hover });
        }
        let claude = crate::app::chat::header::program(&row.spec, self.agent_sessions.contains_key(&row.id)) == Some("Claude Code");
        let working = matches!(row.state, AgentState::Working(_));
        crate::app::chat::header::agent_mark(p, &row.spec, claude, Pos2::new(rect.left() + 20.0, rect.center().y), 14.0, look.soft, working, t);
        let room = rect.width() - 40.0 - if chat && hovered { 34.0 } else { 10.0 };
        let ink = if row.state == AgentState::Ended { look.soft } else { look.text };
        p.galley(Pos2::new(rect.left() + 40.0, rect.top() + 8.0), clipped(p, title.to_string(), FontId::proportional(13.5), ink, room), ink);
        let note = self.list_note(row, if on { tool } else { None });
        let line = format!("{} · {note}", crate::app::chat::direct::folder_label(&row.spec.cwd));
        let y = rect.top() + 35.0;
        p.circle_filled(Pos2::new(rect.left() + 43.0, y), 3.0, crate::app::chat::header::state_color(&row.state, look));
        let g = clipped(p, line, FontId::proportional(11.5), look.muted, room - 12.0);
        p.galley(Pos2::new(rect.left() + 51.0, y - g.size().y / 2.0), g, look.muted);
        if chat && hovered {
            if dots_r.hovered() {
                p.rect_filled(dots, 6, look.raised);
            }
            for dx in [-5.0, 0.0, 5.0] {
                p.circle_filled(dots.center() + Vec2::new(dx, 0.0), 1.6, look.soft);
            }
        }
        if chat {
            click_menu(&dots_r, |ui| remove(ui, act));
        }
        if response.on_hover_cursor(CursorIcon::PointingHand).clicked() {
            act.pick = Some(row.id.clone());
        }
    }

    /// Folded or narrow: a column of initials, each with its state's badge, under the » button.
    pub(crate) fn chat_column(&self, ui: &mut egui::Ui, side: Rect, rows: &[AgentRow], look: &Look, t: f64, act: &mut ListAction) {
        // folded, the column starts under the window buttons, which stay on the bare ground
        let side = side.with_min_y(side.top().max(LIGHTS_BOTTOM + 4.0));
        ui.painter().rect_filled(side, 12, look.panel);
        let unfold = Rect::from_center_size(Pos2::new(side.center().x, side.top() + 24.0), Vec2::new(36.0, 32.0));
        let response = ui
            .interact(unfold, ui.scope_id().with("chat_unfold"), Sense::click())
            .on_hover_cursor(CursorIcon::PointingHand)
            .tip(keys_label("Expandir lista (⌘B)"));
        if response.hovered() {
            ui.painter().rect_filled(unfold, 8, look.surface);
        }
        glyph(ui.painter(), unfold.center(), 16.0, 2.0, look.soft, UNFOLD);
        act.toggle |= response.clicked();
        ui.painter().hline(Rect::from_center_size(unfold.center(), Vec2::new(28.0, 0.0)).x_range(), unfold.bottom() + 8.0, Stroke::new(1.0, look.line));
        let list = Rect::from_min_max(Pos2::new(side.left(), unfold.bottom() + 15.0), side.max - Vec2::new(0.0, 8.0));
        ui.scope_builder(egui::UiBuilder::new().max_rect(list), |ui| {
            egui::ScrollArea::vertical().id_salt("chat_initials").auto_shrink(false).show(ui, |ui| {
                ui.spacing_mut().item_spacing.y = 6.0;
                for row in rows {
                    let on = self.chat_agent.as_ref() == Some(&row.id);
                    let (strip, _) = ui.allocate_exact_size(Vec2::new(ui.available_width(), 38.0), Sense::hover());
                    let rect = Rect::from_center_size(strip.center(), Vec2::splat(38.0));
                    let tip = format!("{} · {}", row.spec.label(), row.state.label());
                    let response =
                        ui.interact(rect, ui.scope_id().with(("chat_initial", &row.id)), Sense::click()).on_hover_cursor(CursorIcon::PointingHand).tip(tip);
                    let ground = if on { look.selected } else { look.panel };
                    let p = ui.painter();
                    if on || response.hovered() {
                        p.rect_filled(rect, 10, if on { look.selected } else { look.surface.gamma_multiply(0.6) });
                    }
                    let initial: String = row.spec.label().chars().next().map(|c| c.to_uppercase().collect()).unwrap_or_default();
                    let ink = if row.state == AgentState::Ended {
                        look.muted
                    } else if on {
                        look.text
                    } else {
                        look.soft
                    };
                    let g = p.layout_no_wrap(initial.clone(), FontId::proportional(14.0), ink);
                    bold(p, rect.center() - g.size() / 2.0, &initial, 14.0, ink);
                    // the badge, ringed in the ground it sits on
                    let badge = rect.right_bottom() - Vec2::splat(6.5);
                    p.circle_filled(badge, 5.5, ground);
                    state_mark(p, badge, 7.0, &row.state, look, t);
                    if response.clicked() {
                        act.pick = Some(row.id.clone());
                    }
                }
            });
        });
    }
}
