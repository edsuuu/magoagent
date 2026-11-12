use super::*;

impl Ide {
    pub(in super::super) fn center(&mut self, ui: &mut Ui) {
        if self.tabs.is_empty() {
            return self.empty_editor(ui);
        }
        self.active = self.active.min(self.tabs.len() - 1);
        self.tab_bar(ui);
        if self.tabs[self.active].diff.is_some() {
            return self.diff_view(ui);
        }
        if self.tabs[self.active].sheet.is_some() {
            return self.sheet_view(ui);
        }
        if let Some(media) = &self.tabs[self.active].media {
            return media.view(ui);
        }
        if self.tabs[self.active].conflict() {
            self.conflict_bar(ui);
        }
        if self.find.is_some() {
            self.find_bar(ui);
        }
        self.inspection_widget(ui);
        self.editor(ui);
        self.code_comment_popup(ui);
    }

    /// The active tab has unsaved text and its file changed on disk: the person keeps one of the two.
    pub(super) fn conflict_bar(&mut self, ui: &mut Ui) {
        let i = self.active;
        let (mut reload, mut keep) = (false, false);
        Frame::NONE.fill(t::INLINE).inner_margin(Margin::symmetric(12, 6)).show(ui, |ui| {
            ui.horizontal(|ui| {
                let what = format!("{} mudou no disco e tem mudanças suas não salvas.", file_name(&self.tabs[i].path));
                ui.label(egui::RichText::new(what).color(t::WARNING));
                reload = ui.button("Recarregar do disco").on_hover_text("Descarta as suas mudanças").clicked();
                keep = ui.button("Manter a minha versão").on_hover_text("Grava a sua por cima do disco").clicked();
            });
        });
        if reload {
            let _ = self.resolve_conflict("", "reload");
        } else if keep {
            let _ = self.resolve_conflict("", "keep");
        }
    }

    pub(super) fn empty_editor(&mut self, ui: &mut Ui) {
        if self.root.is_none() {
            ui.vertical_centered(|ui| {
                ui.add_space(ui.available_height() * 0.35);
                ui.label(egui::RichText::new(&self.status).size(16.0).color(t::TEXT));
                ui.add_space(8.0);
                ui.label(egui::RichText::new("Escolha outra pasta em Editar, na barra do IDE.").color(t::MUTED));
            });
            return;
        }
        let c = ui.max_rect().center();
        let hints = [
            ("Pesquisar em tudo", "Shift duas vezes"),
            ("Ir para arquivo", "Ctrl+Shift+N  ·  Ctrl+P"),
            ("Localizar ação", "Ctrl+Shift+A  ·  Ctrl+Shift+P"),
            ("Projeto", "Alt+1"),
            ("Problemas", "Alt+6"),
        ];
        for (i, (what, keys)) in hints.iter().enumerate() {
            let y = c.y - 60.0 + i as f32 * 28.0;
            ui.painter().text(pos2(c.x - 8.0, y), Align2::RIGHT_CENTER, what, t::ui(14.0), t::MUTED);
            ui.painter().text(pos2(c.x + 8.0, y), Align2::LEFT_CENTER, super::chrome::keys(keys), t::ui(14.0), t::LINK);
        }
    }

    pub(super) fn tab_bar(&mut self, ui: &mut Ui) {
        let mut close = None;
        // the active tab comes into view when it changes: a new tab opens past the right edge
        let shown = (self.active, self.tabs.len());
        let id = ui.make_persistent_id("tabs_shown");
        let reveal = ui.data_mut(|d| std::mem::replace(d.get_temp_mut_or(id, (usize::MAX, 0)), shown)) != shown;
        // the plain mouse wheel scrolls the tabs sideways (a horizontal area takes only Shift+wheel)
        let wheel = std::mem::replace(&mut ui.style_mut().always_scroll_the_only_direction, true);
        egui::ScrollArea::horizontal().id_salt("tabs").max_height(40.0).show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.add_space(6.0);
                ui.set_height(40.0);
                for (i, tab) in self.tabs.iter().enumerate() {
                    let name = match (tab.diff.as_ref(), tab.sheet.as_ref()) {
                        (Some(d), _) => d.title.clone().unwrap_or_else(|| file_name(&tab.path) + " (diff)"),
                        // the source in brackets, as the reference does, so loja.clientes of two servers tell apart
                        (_, Some(sheet)) => {
                            let source = file_name(&tab.path);
                            let source = source.split('@').next().unwrap_or_default();
                            sheet.table.as_ref().map_or_else(|| format!("Console · {source}"), |t| format!("{t} [{source}]"))
                        }
                        _ => file_name(&tab.path),
                    };
                    let galley = ui.painter().layout_no_wrap(name, t::ui(t::UI_SIZE), t::TEXT);
                    let w = galley.size().x + 56.0;
                    let (rect, resp) = ui.allocate_exact_size(vec2(w, 30.0), Sense::click());
                    let selected = i == self.active;
                    let p = ui.painter();
                    if selected {
                        p.rect(rect, 6, t::TAB_SELECTED, Stroke::new(1.0, t::TAB_BORDER), egui::StrokeKind::Inside);
                    } else if resp.hovered() {
                        p.rect_filled(rect, 6, t::HOVER);
                    }
                    let at = pos2(rect.left() + 16.0, rect.center().y);
                    // a table's tab is about the data, not the database file's git status
                    let base = if tab.sheet.is_some() {
                        icons::at(ui.painter(), icons::FT_SQL, at);
                        t::TEXT
                    } else {
                        icons::file(ui.painter(), &tab.path, at, 1.0);
                        // its file is gone from the disk: saving writes it back
                        if tab.gone { t::ERROR } else { self.file_status(&tab.path).map_or(t::TEXT, |s| s.color()) }
                    };
                    let color = if selected { base } else { base.gamma_multiply(0.75) }; // EditorTabs.unselectedAlpha
                    p.galley_with_override_text_color(pos2(rect.left() + 28.0, rect.center().y - galley.size().y / 2.0), galley, color);
                    let x = Rect::from_center_size(pos2(rect.right() - 14.0, rect.center().y), vec2(16.0, 16.0));
                    let over_x = ui.rect_contains_pointer(x);
                    if tab.dirty && !over_x {
                        p.circle_filled(x.center(), 3.5, t::MUTED);
                    } else if resp.hovered() || selected {
                        icons::paint(ui.painter(), if over_x { icons::CLOSE_SMALL_HOVER } else { icons::CLOSE_SMALL }, x);
                    }
                    if resp.clicked() {
                        if over_x { close = Some(i) } else { self.active = i }
                    }
                    if resp.middle_clicked() {
                        close = Some(i);
                    }
                    if reveal && selected {
                        resp.scroll_to_me(None);
                    }
                    resp.on_hover_text(if tab.gone { format!("{} (excluído do disco)", tab.path.display()) } else { tab.path.display().to_string() });
                }
            });
        });
        ui.style_mut().always_scroll_the_only_direction = wheel;
        if let Some(i) = close {
            self.close_tab(i);
        }
    }

    pub(super) fn find_bar(&mut self, ui: &mut Ui) {
        let focus = std::mem::take(&mut self.find_focus);
        let Some(tab) = self.tabs.get_mut(self.active) else { return };
        let query = self.find.as_mut().unwrap();
        let mut close = false;
        ui.horizontal(|ui| {
            ui.add_space(8.0);
            let (ir, _) = ui.allocate_exact_size(vec2(16.0, 24.0), Sense::hover());
            icons::at(ui.painter(), icons::SEARCH, ir.center());
            let r = ui.add(TextEdit::singleline(query).hint_text("Localizar").desired_width(280.0));
            if focus || (!r.has_focus() && ui.memory(|m| m.focused().is_none())) {
                r.request_focus();
            }
            let hay = tab.text.to_ascii_lowercase();
            let needle = query.to_ascii_lowercase();
            let hits: Vec<usize> = if needle.is_empty() { vec![] } else { hay.match_indices(&needle).map(|(b, _)| b).collect() };
            let cur_byte = byte_at(&tab.text, tab.sel.1);
            let enter = r.lost_focus() && ui.input(|i| i.key_pressed(Key::Enter));
            let back = ui.input(|i| i.modifiers.shift);
            let mut jump = |forward: bool| {
                let hit = if forward {
                    hits.iter().find(|&&b| b >= cur_byte).or(hits.first())
                } else {
                    let start = byte_at(&tab.text, tab.sel.0);
                    hits.iter().rev().find(|&&b| b < start).or(hits.last())
                };
                if let Some(&b) = hit {
                    let a = tab.text[..b].chars().count();
                    tab.select = Some((a, a + query.chars().count()));
                }
            };
            ui.label(
                egui::RichText::new(if needle.is_empty() {
                    String::new()
                } else if hits.is_empty() {
                    "0 resultados".into()
                } else {
                    format!("{} resultados", hits.len())
                })
                .color(t::MUTED),
            );
            if ui.small_button("↑").clicked() || (enter && back) {
                jump(false);
            }
            if ui.small_button("↓").clicked() || (enter && !back) {
                jump(true);
            }
            if enter {
                r.request_focus();
            }
            if ui.small_button("×").clicked() || ui.input(|i| i.key_pressed(Key::Escape)) {
                close = true;
            }
        });
        if close {
            self.find = None;
        }
    }
}
