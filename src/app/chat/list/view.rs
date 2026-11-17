use super::running::{Entry, chip, counts, kept, space_groups};
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
    pub(crate) fn list_note(&self, row: &AgentRow, tool: Option<&str>) -> String {
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

    /// Everything running, under the chips (Todos, Trabalhando, Esperando, Prontos, with their counts) and the search,
    /// grouped by space (its dot and name); a Maestro's agents right under it.
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn chat_list(
        &self,
        ui: &mut egui::Ui,
        side: Rect,
        entries: &[Entry],
        look: &Look,
        t: f64,
        act: &mut ListAction,
        filter: &mut String,
        show: &mut Show,
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
        // the chips, wrapping when the list is narrow
        let (mut x, mut y) = (inner.left() + 2.0, head + 20.0);
        for (chip, count) in Show::ALL.into_iter().zip(counts(entries)) {
            let label = ui.painter().layout_no_wrap(chip.label().into(), FontId::proportional(12.0), look.text);
            let number = ui.painter().layout_no_wrap(count.to_string(), FontId::proportional(12.0), look.muted);
            let width = 10.0 + label.size().x + 6.0 + number.size().x + 10.0;
            if x + width > inner.right() && x > inner.left() + 2.0 {
                (x, y) = (inner.left() + 2.0, y + 32.0);
            }
            let rect = Rect::from_min_size(Pos2::new(x, y), Vec2::new(width, 26.0));
            x = rect.right() + 6.0;
            let response = ui.interact(rect, ui.scope_id().with(("chat_chip", chip.key())), Sense::click()).on_hover_cursor(CursorIcon::PointingHand);
            let on = *show == chip;
            let p = ui.painter();
            let (ground, ink, soft) = if on { (ACCENT, Color32::WHITE, Color32::from_white_alpha(180)) } else { (Color32::TRANSPARENT, look.text, look.muted) };
            let ground = if response.hovered() && !on { look.hover } else { ground };
            p.rect(rect, 13, ground, Stroke::new(1.0, if on { ACCENT } else { look.line }), StrokeKind::Inside);
            p.text(Pos2::new(rect.left() + 10.0, rect.center().y), egui::Align2::LEFT_CENTER, chip.label(), FontId::proportional(12.0), ink);
            p.text(Pos2::new(rect.right() - 10.0, rect.center().y), egui::Align2::RIGHT_CENTER, count.to_string(), FontId::proportional(12.0), soft);
            if response.clicked() {
                *show = chip;
            }
        }
        // the search over the names and the last lines
        let field = Rect::from_min_max(Pos2::new(inner.left() + 2.0, y + 34.0), Pos2::new(inner.right() - 2.0, y + 64.0));
        ui.painter().rect_filled(field, 8, look.hover);
        paint_icon(ui.painter(), Icon::Search, Pos2::new(field.left() + 15.0, field.center().y), look.muted);
        let edit = egui::TextEdit::singleline(filter)
            .id(egui::Id::unique("chat_list_filter"))
            .hint_text(egui::RichText::new("Buscar nos chats").color(look.muted))
            .font(FontId::proportional(14.0))
            .text_color(look.text)
            .margin(egui::Margin::ZERO)
            .frame(egui::Frame::NONE);
        // the frameless field keeps no margin: the room for the magnifier is left out of its rect
        ui.put(Rect::from_min_max(field.min + Vec2::new(30.0, 6.0), field.max - Vec2::new(8.0, 6.0)), edit);
        let shown = kept(entries, *show, &filter.trim().to_lowercase());
        // the agents a Maestro opened go right under it, indented
        let ids: Vec<&str> = shown.iter().map(|entry| entry.id.as_str()).collect();
        let maestro: Vec<bool> = shown.iter().map(|entry| entry.row.as_ref().is_some_and(|row| self.is_maestro_row(row))).collect();
        let parents = subchat_parents(&ids, &maestro, &self.links());
        let order: Vec<String> = self.spaces.list.iter().map(|space| space.id.clone()).collect();
        let list = Rect::from_min_max(Pos2::new(inner.left(), field.bottom() + 4.0), inner.max);
        ui.scope_builder(egui::UiBuilder::new().max_rect(list), |ui| {
            egui::ScrollArea::vertical().id_salt("chat_agents").auto_shrink(false).show(ui, |ui| {
                ui.spacing_mut().item_spacing.y = 2.0;
                if shown.is_empty() {
                    let words = if entries.is_empty() { "Nada rodando agora." } else { "Nada com esse filtro." };
                    ui.add_space(12.0);
                    ui.label(egui::RichText::new(words).size(13.0).color(look.muted));
                }
                for (space, members) in space_groups(&shown, &order) {
                    let (name, color) = self.group_look(&space);
                    let (label, _) = ui.allocate_exact_size(Vec2::new(ui.available_width(), 28.0), Sense::hover());
                    let mut job = egui::text::LayoutJob::default();
                    let format = egui::TextFormat { font_id: FontId::proportional(12.0), color: look.muted, extra_letter_spacing: 0.7, ..Default::default() };
                    job.append(&name.to_uppercase(), 0.0, format);
                    let g = ui.painter().layout_job(job);
                    let at = Pos2::new(label.left() + 20.0, label.bottom() - g.size().y - 5.0);
                    ui.painter().circle_filled(Pos2::new(label.left() + 11.0, at.y + g.size().y / 2.0), 4.0, color);
                    ui.painter().galley(at, g, look.muted);
                    for &i in &members {
                        if parents[i].is_some_and(|parent| members.contains(&parent)) {
                            continue;
                        }
                        self.list_row(ui, shown[i], 0.0, look, t, act);
                        for &j in members.iter().filter(|&&j| parents[j] == Some(i)) {
                            self.list_row(ui, shown[j], 18.0, look, t, act);
                        }
                    }
                }
            });
        });
    }

    /// A row's title: the name given with Renomear, else the conversation's own, else the agent's name.
    pub(crate) fn chat_title(&self, row: &AgentRow) -> String {
        let named = self.chats.iter().any(|c| c.id == row.id && c.named);
        let own = self.transcripts.get(&row.id).and_then(|t| t.title.clone()).filter(|_| !named);
        own.unwrap_or_else(|| row.spec.label().to_string())
    }

    /// A chat's name without its agent ("Claude Code · magoagent" → "magoagent"): the mini chat's title. A name
    /// given with Renomear stays whole; a terminal keeps its own.
    pub(crate) fn chat_short_name(&self, row: &AgentRow) -> String {
        let named = self.chats.iter().any(|chat| chat.id == row.id && chat.named);
        let label = row.spec.label();
        match label.split_once(" · ") {
            Some((_, rest)) if row.spec.chat && !named => rest.to_string(),
            _ => label.to_string(),
        }
    }

    /// Renomear's balloon under the chat's row: Salvar names it (kept in chats.json), Cancelar or Esc leaves it.
    pub(crate) fn chat_rename_ui(&mut self, ctx: &Context) {
        // the row is measured as the list draws: until then (the frame Renomear was picked) it waits
        let Some(anchor) = self.chat_listing.renaming_at.take() else { return };
        let Some((id, mut name)) = self.chat_listing.renaming.take() else { return };
        let first = ctx.data_mut(|d| !std::mem::replace(d.get_temp_mut_or_default::<bool>(egui::Id::unique("chat_rename_open")), true));
        match name_balloon(ctx, &self.colors(), anchor, "Renomear chat", "Nome do chat", &mut name, first) {
            None => self.chat_listing.renaming = Some((id, name)),
            Some(save) => {
                ctx.data_mut(|d| d.remove::<bool>(egui::Id::unique("chat_rename_open")));
                if save {
                    self.rename_chat(&id, &name);
                }
            }
        }
    }

    /// Gives chat `id` the name `name` (kept in chats.json, over the conversation's own title); a blank one changes nothing.
    pub(crate) fn rename_chat(&mut self, id: &str, name: &str) -> bool {
        let name = name.trim();
        let Some(chat) = self.chats.iter_mut().find(|c| c.id == id).filter(|_| !name.is_empty()) else { return false };
        chat.name = name.to_string();
        chat.named = true;
        self.save_chats();
        true
    }

    /// A line of the list: the mini wizard in the agent's colour, its name and state's chip, how long ago, its last
    /// line and, when it waits on the person, the buttons that answer it there; on a chat ⋯ under the pointer
    /// (Renomear, Mostrar no canvas, Remover chat), and a right click on any agent of the app.
    pub(super) fn list_row(&self, ui: &mut egui::Ui, entry: &Entry, indent: f32, look: &Look, t: f64, act: &mut ListAction) {
        let on = self.chat_agent.as_ref() == Some(&entry.id);
        let chat = self.is_chat(&entry.id);
        let width = ui.available_width();
        let x = indent + 40.0;
        // the answers' buttons, laid out first: the row is as tall as they need
        let (answers, guarded) = self.row_answers(&entry.id, t);
        let font = FontId::proportional(12.0);
        let mut buttons: Vec<(Rect, std::sync::Arc<egui::Galley>, bool, Answer)> = Vec::new();
        let (mut bx, mut by) = (x, 54.0);
        for (label, main, answer) in answers {
            let ink = if main { Color32::from_rgb(0x1d, 0x1d, 0x1f) } else { look.text };
            let g = clipped(ui.painter(), label, font.clone(), ink, (width - x - 30.0).max(20.0));
            let size = Vec2::new(g.size().x + 20.0, 24.0);
            if bx + size.x > width - 8.0 && bx > x {
                (bx, by) = (x, by + 30.0);
            }
            buttons.push((Rect::from_min_size(Pos2::new(bx, by), size), g, main, answer));
            bx += size.x + 6.0;
        }
        let height = if buttons.is_empty() { 54.0 } else { by + 32.0 };
        let (rect, response) = ui.allocate_exact_size(Vec2::new(width, height), Sense::click());
        let remove = |ui: &mut egui::Ui, act: &mut ListAction| {
            if chat && menu_row(ui, Some(Icon::Edit), "Renomear", "", None) {
                act.rename = Some(entry.id.clone());
                ui.close();
            }
            if menu_row(ui, Some(Icon::Map), "Mostrar no canvas", "", None) {
                act.on_canvas = Some(entry.id.clone());
                ui.close();
            }
            if chat && menu_row(ui, Some(Icon::Trash), "Remover chat", "", Some(crate::app::ui::UiColors::new(ui.visuals().dark_mode).danger)) {
                act.remove = Some(entry.id.clone());
                ui.close();
            }
        };
        if entry.row.is_some() {
            context_menu(&response, |ui| remove(ui, act));
        }
        if self.chat_listing.renaming.as_ref().is_some_and(|(id, _)| *id == entry.id) {
            self.chat_listing.renaming_at.set(Some(rect));
        }
        let dots = Rect::from_center_size(Pos2::new(rect.right() - 18.0, rect.top() + 17.0), Vec2::splat(24.0));
        let dots_r = ui.interact(dots, ui.scope_id().with(("chat_row_more", &entry.id)), Sense::click());
        let menu_open = egui::Popup::is_id_open(ui.ctx(), egui::Popup::default_response_id(&dots_r));
        let hovered = response.hovered() || dots_r.hovered() || menu_open;
        let p = ui.painter();
        if on || hovered {
            p.rect_filled(rect, 10, if on { look.surface } else { look.hover });
        }
        let left = rect.left();
        let mini = Pos2::new(left + indent + 19.0, rect.top() + 25.0);
        crate::app::mascot::mago::paint_mini(p, &format!("chat-list-{}", entry.id), mini, 20.0, entry.color, self.entry_mago(entry), t);
        // how long ago at the right (⋯ in its place on a chat under the pointer)
        let mut right = rect.right() - 10.0;
        if chat && hovered {
            if dots_r.hovered() {
                p.rect_filled(dots, 6, look.raised);
            }
            for dx in [-5.0, 0.0, 5.0] {
                p.circle_filled(dots.center() + Vec2::new(dx, 0.0), 1.6, look.soft);
            }
            right = dots.left() - 4.0;
        } else if let Some(seconds) = entry.ago {
            let when = p.layout_no_wrap(ago(seconds), FontId::proportional(12.0), look.muted);
            right -= when.size().x;
            p.galley(Pos2::new(right, rect.top() + 17.0 - when.size().y / 2.0), when, look.muted);
            right -= 8.0;
        }
        // the name, then the state's chip
        let (word, color) = chip(&entry.state);
        // on the light look the words go darker, to read on their pale ground
        let ink_of_tag = if look.dark { color } else { color.lerp_to_gamma(Color32::BLACK, 0.4) };
        let tag = p.layout_no_wrap(word.into(), FontId::proportional(12.0), ink_of_tag);
        let tag_w = tag.size().x + 14.0;
        let ink = if entry.state == AgentState::Ended { look.soft } else { look.strong };
        let name = clipped(p, entry.name.clone(), FontId::proportional(14.0), ink, (right - 6.0 - tag_w - (left + x)).max(0.0));
        let at = Pos2::new(left + x, rect.top() + 17.0 - name.size().y / 2.0);
        let name_right = at.x + name.size().x;
        // painted twice a pixel apart: the UI font has no bold weight
        p.galley(at + Vec2::new(1.0 / p.pixels_per_point(), 0.0), name.clone(), ink);
        p.galley(at, name, ink);
        let tag_rect = Rect::from_min_size(Pos2::new(name_right + 6.0, rect.top() + 7.0), Vec2::new(tag_w, 20.0));
        p.rect_filled(tag_rect, 10, color.gamma_multiply(0.16));
        p.galley(tag_rect.center() - tag.size() / 2.0, tag, ink_of_tag);
        // the last line
        let last = clipped(p, entry.last.clone(), FontId::proportional(12.0), look.soft, (rect.right() - 10.0 - (left + x)).max(0.0));
        p.galley(Pos2::new(left + x, rect.top() + 37.0 - last.size().y / 2.0), last, look.soft);
        for (index, (button, label, main, answer)) in buttons.into_iter().enumerate() {
            let button = button.translate(rect.min.to_vec2());
            let response =
                ui.interact(button, ui.scope_id().with(("chat_row_answer", &entry.id, index)), Sense::click()).on_hover_cursor(CursorIcon::PointingHand);
            let fade = if guarded { 0.4 } else { 1.0 };
            let ground = if main { AMBER } else { look.raised };
            let ground = if response.hovered() { ground.gamma_multiply(0.85) } else { ground };
            let p = ui.painter();
            p.rect_filled(button, 6, ground.gamma_multiply(fade));
            p.galley(button.center() - label.size() / 2.0, label, look.text);
            if response.clicked() && !guarded {
                act.answer = Some((entry.id.clone(), answer));
            }
        }
        if chat {
            dots_menu(&dots_r, |ui| remove(ui, act));
        }
        if response.on_hover_cursor(CursorIcon::PointingHand).clicked() {
            if entry.row.is_some() {
                act.pick = Some(entry.id.clone());
            } else {
                act.outside = Some(entry.id.clone());
            }
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
