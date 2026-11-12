use super::*;

impl DrawingApp {
    /// The address bar over each portal's header: back, forward and reload (× while loading),
    /// and the address, which turns into a field on a click.
    pub(crate) fn portal_chrome(&mut self, ctx: &Context) {
        if self.portals_covered() {
            return;
        }
        let portals: Vec<Element> = self
            .scene
            .elements
            .iter()
            .filter(|e| e.portal.as_ref().is_some_and(|p| !p.chrome_hidden) && self.chrome_live(e) && !self.lifted_other(e))
            .cloned()
            .collect();
        for element in portals {
            let (frame, zoom) = self.terminal_frame(&element);
            let area = Rect::from_min_size(frame.min + Vec2::new(0.0, PORTAL_HEADER * zoom), Vec2::new(frame.width(), PORTAL_BAR * zoom));
            if !area.intersects(self.canvas_rect) {
                continue;
            }
            let spec = element.portal.clone().unwrap_or_default();
            let loading = self.tab_loading(&element.id);
            let editing = self.portal_url_edit.as_ref().filter(|(id, ..)| *id == element.id).cloned();
            let mut address = editing.as_ref().map_or(spec.url.clone(), |(_, text, _)| text.clone());
            let mut action = None;
            // None: no change; Some(None): stop editing; Some(Some(..)): keep editing that text
            let mut next_edit: Option<Option<(String, String, bool)>> = None;
            // not kept on screen by egui: moved down, the bar would lie over its own page
            egui::Area::new(egui::Id::unique(("portal_chrome", element.id.as_str()))).fixed_pos(area.min).constrain(false).order(egui::Order::Middle).show(
                ctx,
                |ui| {
                    ui.set_clip_rect(area.intersect(self.canvas_rect));
                    let (rect, _) = ui.allocate_exact_size(area.size(), Sense::hover());
                    let p = ui.painter().clone();
                    let (light, muted) = (Color32::from_gray(0xd0), Color32::from_gray(0x8a));
                    // back, forward and reload in one pill: 4 pt inside, 24 pt buttons 2 pt apart
                    let nav = Rect::from_min_size(Pos2::new(rect.left() + 8.0 * zoom, rect.center().y - 14.0 * zoom), Vec2::new(84.0 * zoom, 28.0 * zoom));
                    p.rect_filled(nav, 14.0 * zoom, Color32::from_white_alpha(15));
                    let slot = |i: f32| Rect::from_center_size(Pos2::new(nav.left() + (16.0 + 26.0 * i) * zoom, nav.center().y), Vec2::splat(24.0 * zoom));
                    for (i, (icon, chrome, tip)) in
                        [(Icon::Arrow(false), Chrome::Back, "Voltar"), (Icon::Arrow(true), Chrome::Forward, "Avançar")].into_iter().enumerate()
                    {
                        let r = ui.interact(slot(i as f32), ui.scope_id().with(("nav", i)), Sense::click());
                        if r.hovered() {
                            p.circle_filled(r.rect.center(), 12.0 * zoom, Color32::from_white_alpha(26));
                        }
                        paint_icon_scaled(&p, icon, r.rect.center(), light, 0.875 * zoom, 0.0);
                        if r.tip(tip).clicked() {
                            action = Some(chrome);
                        }
                    }
                    let reload = ui.interact(slot(2.0), ui.scope_id().with("reload"), Sense::click());
                    if reload.hovered() {
                        p.circle_filled(reload.rect.center(), 12.0 * zoom, Color32::from_white_alpha(26));
                    }
                    if loading {
                        p.text(reload.rect.center(), egui::Align2::CENTER_CENTER, "×", FontId::proportional(14.0 * zoom), light);
                    } else {
                        paint_icon_scaled(&p, Icon::Restart, reload.rect.center(), light, 0.75 * zoom, 0.0);
                    }
                    if reload.tip(if loading { "Parar" } else { "Recarregar" }).clicked() {
                        action = Some(if loading { Chrome::Stop } else { Chrome::Reload });
                    }
                    // the address: a pill with a lens; a click turns it into a field
                    let field = Rect::from_min_max(Pos2::new(nav.right() + 6.0 * zoom, nav.top()), Pos2::new(rect.right() - 8.0 * zoom, nav.bottom()));
                    p.rect_filled(field, 14.0 * zoom, Color32::from_white_alpha(15));
                    paint_icon_scaled(&p, Icon::Search, Pos2::new(field.left() + 16.0 * zoom, field.center().y), muted, 0.75 * zoom, 0.0);
                    let text_room =
                        Rect::from_min_max(Pos2::new(field.left() + 28.0 * zoom, field.top()), Pos2::new(field.right() - 10.0 * zoom, field.bottom()));
                    let font = FontId::monospace(12.0 * zoom);
                    if let Some((_, _, focus)) = editing.as_ref() {
                        // no margin and centered: the text stays where the bar showed it
                        let edit = egui::TextEdit::singleline(&mut address)
                            .frame(egui::Frame::NONE)
                            .margin(egui::Margin::ZERO)
                            .align(egui::Align2::LEFT_CENTER)
                            .font(font)
                            .text_color(light)
                            .hint_text("Digite uma URL…")
                            .desired_width(text_room.width());
                        let response = ui.put(text_room, edit);
                        // a new edit starts with the whole address selected, as the reference app's does
                        let mut selected = false;
                        if *focus {
                            response.request_focus();
                            if let Some(mut state) = egui::TextEdit::load_state(ui.ctx(), response.id) {
                                let end = egui::text::CCursor::new(address.chars().count());
                                state.cursor.set_char_range(Some(egui::text_selection::CCursorRange::two(egui::text::CCursor::new(0), end)));
                                state.store(ui.ctx(), response.id);
                                selected = true;
                            }
                        }
                        if response.has_focus() && ui.input(|i| i.key_pressed(Key::Escape)) {
                            next_edit = Some(None);
                        } else if response.lost_focus() {
                            if ui.input(|i| i.key_pressed(Key::Enter)) {
                                action = Some(Chrome::Navigate(address.clone()));
                            }
                            next_edit = Some(None);
                        } else {
                            // still owed the selection while the field has no state yet (its first frame)
                            next_edit = Some(Some((element.id.clone(), address.clone(), *focus && !selected)));
                        }
                    } else {
                        let click = ui.interact(field, ui.scope_id().with("address"), Sense::click());
                        let (text, color) =
                            if spec.url.is_empty() { ("Digite uma URL…".to_string(), muted) } else { (display_url(&spec.url).to_string(), light) };
                        p.with_clip_rect(text_room.intersect(p.clip_rect())).text(text_room.left_center(), egui::Align2::LEFT_CENTER, text, font, color);
                        if click.on_hover_cursor(CursorIcon::Text).clicked() {
                            next_edit = Some(Some((element.id.clone(), spec.url.clone(), true)));
                        }
                    }
                },
            );
            match next_edit {
                Some(Some(state)) => self.portal_url_edit = Some(state),
                Some(None) => self.portal_url_edit = None,
                None => {}
            }
            if let Some(action) = action {
                self.apply_chrome(&element.id, action);
            }
        }
    }

    pub(super) fn apply_chrome(&mut self, id: &str, action: Chrome) {
        let script = match action {
            Chrome::Back => "history.back()",
            Chrome::Forward => "history.forward()",
            Chrome::Reload => return self.portal_reload(id),
            Chrome::Stop => {
                // a stopped load may not report its finish, so the bar goes back to ⟳ now
                self.set_loading(id, false);
                "window.stop()"
            }
            Chrome::Navigate(url) => return self.portal_navigate(id, &url),
        };
        self.portal_script(id, script);
    }

    pub(crate) fn portal_history(&mut self, id: &str, action: &str) {
        match action {
            "back" => self.apply_chrome(id, Chrome::Back),
            "forward" => self.apply_chrome(id, Chrome::Forward),
            "reload" => self.apply_chrome(id, Chrome::Reload),
            _ => {}
        }
    }

    #[cfg(not(target_os = "linux"))]
    pub(super) fn set_loading(&mut self, id: &str, loading: bool) {
        let active = self.scene.elements.iter().find(|e| e.id == id).and_then(|e| Some(e.portal.as_ref()?.active_tab()));
        if let Some(tab) = active.and_then(|a| self.portals.get_mut(id)?.tabs.get_mut(a)) {
            tab.loading = loading;
        }
    }

    #[cfg(target_os = "linux")]
    pub(super) fn set_loading(&mut self, id: &str, loading: bool) {
        let _ = (id, loading);
    }
}
