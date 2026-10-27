//! Main menu, minimap, locked elements and preferences.

use crate::prelude::*;

impl DrawingApp {
    pub(crate) fn main_menu(&mut self, ctx: &Context, button: Rect) {
        let c = self.colors();
        let mut prefs_anchor: Option<Rect> = None;
        let mut locked_anchor: Option<Rect> = None;
        // MainMenu.tsx items; .dropdown-menu-container: 8px padding, 32px items, 1px gap
        let area = egui::Area::new(egui::Id::unique("main_menu_popup"))
            .fixed_pos(button.left_bottom() + Vec2::new(0.0, 8.0))
            .order(egui::Order::Foreground)
            .show(ctx, |ui| {
                self.island().inner_margin(8).show(ui, |ui| {
                    ui.set_width(237.0);
                    ui.spacing_mut().item_spacing.y = 1.0;
                    if menu_item(ui, Icon::Open, "Abrir", "Cmd+O", c.text, &c) {
                        self.menu_open = false;
                        self.open();
                    }
                    if menu_item(ui, Icon::Download, "Salvar como...", "", c.text, &c) {
                        self.menu_open = false;
                        self.save_as();
                    }
                    if menu_item(ui, Icon::Export, "Exportar imagem...", "Cmd+Shift+E", c.text, &c) {
                        self.menu_open = false;
                        self.export_open = true;
                    }
                    if menu_item(ui, Icon::Palette, "Paleta de comandos", "Cmd+/", PRIMARY, &c) {
                        self.menu_open = false;
                        self.open_palette();
                    }
                    if menu_item(ui, Icon::Trash, "Limpar a tela", "", c.text, &c) {
                        self.menu_open = false;
                        self.clear_canvas();
                    }
                    if menu_item(ui, Icon::Sliders, "Ajustes…", "Cmd+,", c.text, &c) {
                        self.menu_open = false;
                        self.settings_open = true;
                    }
                    let (line, _) = ui.allocate_exact_size(Vec2::new(ui.available_width(), 9.0), Sense::hover());
                    ui.painter().hline(line.x_range(), line.center().y, Stroke::new(1.0, c.surface_low));
                    let locked_count = self.scene.elements.iter().filter(|e| e.locked).count();
                    let before = ui.cursor().min;
                    let locked_label = format!("Elementos bloqueados ({locked_count})");
                    if menu_item(ui, Icon::Lock(true), &locked_label, "›", c.text, &c) {
                        (self.locked_open, self.prefs_open) = (!self.locked_open, false);
                    }
                    let row = Rect::from_min_size(before, Vec2::new(ui.available_width(), 32.0));
                    if ui.rect_contains_pointer(row) {
                        self.locked_open = true;
                        self.prefs_open = false;
                    }
                    locked_anchor = Some(row);
                    let before = ui.cursor().min;
                    if menu_item(ui, Icon::Sliders, "Preferências", "›", c.text, &c) {
                        (self.prefs_open, self.locked_open) = (!self.prefs_open, false);
                    }
                    let row = Rect::from_min_size(before, Vec2::new(ui.available_width(), 32.0));
                    if ui.rect_contains_pointer(row) {
                        self.prefs_open = true;
                        self.locked_open = false;
                    }
                    prefs_anchor = Some(row);
                    ui.horizontal(|ui| {
                        ui.set_height(32.0);
                        ui.add_space(8.0);
                        ui.label(egui::RichText::new("Tema").size(14.0).color(c.text));
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            group_frame(&c).inner_margin(2).show(ui, |ui| {
                                ui.spacing_mut().item_spacing.x = 2.0;
                                // right-to-left: shows light, dark, system
                                for (theme, icon, tip) in [
                                    (Theme::System, Icon::Monitor, "Sistema"),
                                    (Theme::Dark, Icon::Moon, "Escuro"),
                                    (Theme::Light, Icon::Sun, "Claro"),
                                ] {
                                    let on = self.prefs.theme == theme;
                                    let r = square_button(ui, 26.0, false, Color32::TRANSPARENT, &c);
                                    if on {
                                        ui.painter().rect_filled(r.rect, 6, PRIMARY);
                                    }
                                    paint_icon(ui.painter(), icon, r.rect.center(), if on { Color32::WHITE } else { c.text });
                                    if r.on_hover_text(tip).clicked() {
                                        self.prefs.theme = theme;
                                    }
                                }
                            });
                        });
                    });
                    ui.add_space(6.0);
                    ui.horizontal(|ui| {
                        ui.add_space(8.0);
                        ui.label(egui::RichText::new("Fundo da tela").size(14.0).color(c.text));
                    });
                    ui.add_space(4.0);
                    ui.horizontal(|ui| {
                        ui.add_space(8.0);
                        ui.spacing_mut().item_spacing.x = 8.0;
                        for color in CANVAS_PICKS {
                            if swatch(ui, color, self.scene.background == color, 28.0, self.dark()).clicked() {
                                self.snapshot();
                                self.scene.background = color;
                            }
                        }
                    });
                    ui.add_space(4.0);
                });
            });
        let mut submenu = Rect::NOTHING;
        if let (true, Some(row)) = (self.prefs_open, prefs_anchor) {
            submenu = self.preferences_menu(ctx, Pos2::new(area.response.rect.right() + 8.0, row.top() - 8.0));
        }
        if let (true, Some(row)) = (self.locked_open, locked_anchor) {
            submenu = self.locked_menu(ctx, Pos2::new(area.response.rect.right() + 8.0, row.top() - 8.0));
        }
        let clicked_outside = ctx.input(|i| i.pointer.any_pressed())
            && ctx
                .input(|i| i.pointer.interact_pos())
                .is_some_and(|p| !area.response.rect.contains(p) && !button.contains(p) && !submenu.contains(p));
        if clicked_outside {
            self.menu_open = false;
            self.prefs_open = false;
            self.locked_open = false;
        }
    }

    /// Scaled-down drawing with the visible area; click or drag to move there.
    pub(crate) fn minimap(&mut self, ui: &mut egui::Ui) {
        let c = self.colors();
        let (rect, response) = ui.allocate_exact_size(Vec2::new(200.0, 130.0), Sense::click_and_drag());
        let world = self.minimap_world();
        let scale = (rect.width() / world.width()).min(rect.height() / world.height());
        let to_map = |p: Pos2| rect.center() + (p - world.center()) * scale;
        let p = ui.painter_at(rect);
        p.rect_filled(rect, 6, themed(rgba(self.scene.background), self.dark()));
        let ink = c.text.gamma_multiply(0.55);
        for e in &self.scene.elements {
            let pts: Vec<[f32; 2]> = match &e.kind {
                ElementKind::Line { .. } => line_path(e),
                ElementKind::FreeDraw { points } => points.iter().step_by(4).copied().collect(),
                _ => binding_outline(e, 0.0),
            };
            let mapped: Vec<Pos2> = pts.iter().map(|q| to_map(Pos2::new(q[0], q[1]))).collect();
            match &e.kind {
                ElementKind::Text { .. } => {
                    // text reads as a thin bar at this size
                    let b = Rect::from_two_pos(to_map(element_bounds(e).min), to_map(element_bounds(e).max));
                    p.rect_filled(b.shrink2(Vec2::new(0.0, b.height() * 0.3)), 1, ink);
                }
                ElementKind::Line { .. } | ElementKind::FreeDraw { .. } => {
                    p.add(Shape::line(mapped, Stroke::new(1.0, ink)));
                }
                _ => {
                    if let Some(fill) = e.style.fill {
                        p.add(Shape::convex_polygon(mapped.clone(), themed(rgba(fill), self.dark()).gamma_multiply(0.7), Stroke::NONE));
                    }
                    p.add(Shape::closed_line(mapped, Stroke::new(1.0, ink)));
                }
            }
        }
        let view = Rect::from_two_pos(
            to_map(self.to_world(self.canvas_rect.min)),
            to_map(self.to_world(self.canvas_rect.max)),
        );
        p.rect(view, 2, PRIMARY.gamma_multiply(0.12), Stroke::new(1.5, PRIMARY), StrokeKind::Middle);
        if let Some(pos) = response.interact_pointer_pos().filter(|_| response.clicked() || response.dragged()) {
            self.pan_to(world.center() + (pos - rect.center()) / scale);
        }
        response.on_hover_cursor(CursorIcon::PointingHand).on_hover_text("Clique ou arraste para navegar");
    }

    /// Every locked element, with "show" (click the name) and "unlock" per row.
    pub(crate) fn locked_menu(&mut self, ctx: &Context, at: Pos2) -> Rect {
        let c = self.colors();
        let locked: Vec<(String, String, Rect)> = self
            .scene
            .elements
            .iter()
            .filter(|e| e.locked)
            .map(|e| (e.id.clone(), self.describe(e), element_bounds(e)))
            .collect();
        let mut unlock: Option<Option<String>> = None;
        let mut show: Option<Rect> = None;
        let mut lock_everything = false;
        let rect = egui::Area::new(egui::Id::unique("locked_menu"))
            .fixed_pos(at)
            .order(egui::Order::Foreground)
            .show(ctx, |ui| {
                self.island().inner_margin(8).show(ui, |ui| {
                    ui.set_width(300.0);
                    ui.spacing_mut().item_spacing.y = 1.0;
                    if locked.is_empty() {
                        ui.add_space(6.0);
                        ui.horizontal(|ui| {
                            ui.add_space(8.0);
                            ui.label(egui::RichText::new("Nenhum elemento bloqueado").size(14.0).color(c.muted));
                        });
                        ui.add_space(6.0);
                    }
                    egui::ScrollArea::vertical().max_height(320.0).show(ui, |ui| {
                        for (id, name, bounds) in &locked {
                            let (row, response) = ui.allocate_exact_size(Vec2::new(ui.available_width(), 32.0), Sense::click());
                            let response = response.on_hover_cursor(CursorIcon::PointingHand).on_hover_text("Mostrar na tela");
                            let p = ui.painter();
                            if response.hovered() {
                                p.rect_filled(row, 6, c.surface_high);
                            }
                            paint_icon(p, Icon::Lock(true), Pos2::new(row.left() + 16.0, row.center().y), c.text);
                            p.text(Pos2::new(row.left() + 34.0, row.center().y), egui::Align2::LEFT_CENTER, name, FontId::proportional(14.0), c.text);
                            let button = Rect::from_center_size(Pos2::new(row.right() - 18.0, row.center().y), Vec2::splat(28.0));
                            let open = ui.interact(button, ui.scope_id().with(("unlock", id)), Sense::click()).on_hover_cursor(CursorIcon::PointingHand);
                            ui.painter().rect_filled(button, 6, if open.hovered() { c.selected } else { c.surface_low });
                            paint_icon(ui.painter(), Icon::Lock(false), button.center(), c.text);
                            if open.on_hover_text("Desbloquear").clicked() {
                                unlock = Some(Some(id.clone()));
                            } else if response.clicked() {
                                show = Some(*bounds);
                            }
                        }
                    });
                    let (line, _) = ui.allocate_exact_size(Vec2::new(ui.available_width(), 9.0), Sense::hover());
                    ui.painter().hline(line.x_range(), line.center().y, Stroke::new(1.0, c.surface_low));
                    if !locked.is_empty() && menu_item(ui, Icon::Lock(false), "Desbloquear tudo", "", c.text, &c) {
                        unlock = Some(None);
                    }
                    if self.scene.elements.iter().any(|e| !e.locked) && menu_item(ui, Icon::Lock(true), "Bloquear tudo", "", c.text, &c) {
                        lock_everything = true;
                    }
                });
            })
            .response
            .rect;
        if let Some(target) = unlock {
            self.unlock(target.as_deref());
        }
        if lock_everything {
            self.lock_all();
        }
        if let Some(bounds) = show {
            self.pan_to(bounds.center());
        }
        rect
    }

    /// The Preferências submenu: ✓ toggles and two segmented choices, as in the web app.
    pub(crate) fn preferences_menu(&mut self, ctx: &Context, at: Pos2) -> Rect {
        let c = self.colors();
        egui::Area::new(egui::Id::unique("preferences_menu"))
            .fixed_pos(at)
            .order(egui::Order::Foreground)
            .show(ctx, |ui| {
                self.island().inner_margin(8).show(ui, |ui| {
                    // wide enough for "Modo de seleção por caixa" next to its two options
                    ui.set_width(400.0);
                    ui.spacing_mut().item_spacing.y = 1.0;
                    let choice = |ui: &mut egui::Ui, label: &str, options: [&str; 2], second: &mut bool| {
                        let (rect, _) = ui.allocate_exact_size(Vec2::new(ui.available_width(), 32.0), Sense::hover());
                        // segmented control on the right, inside a rounded frame like the web one
                        let font = FontId::proportional(13.0);
                        let widths: Vec<f32> = options
                            .iter()
                            .map(|o| ui.painter().layout_no_wrap(o.to_string(), font.clone(), c.text).size().x + 16.0)
                            .collect();
                        let frame_right = rect.right() - 8.0;
                        let frame_left = frame_right - widths.iter().sum::<f32>() - 2.0 * 2.0 - 2.0;
                        let frame = Rect::from_min_max(Pos2::new(frame_left, rect.top() + 2.0), Pos2::new(frame_right, rect.bottom() - 2.0));
                        ui.painter().rect(frame, 8, Color32::TRANSPARENT, Stroke::new(1.0, c.surface_low), StrokeKind::Inside);
                        let mut x = frame.left() + 2.0;
                        let mut clicked = None;
                        for (k, option) in options.iter().enumerate() {
                            let pill = Rect::from_min_size(Pos2::new(x, frame.top() + 2.0), Vec2::new(widths[k], frame.height() - 4.0));
                            let on = (k == 1) == *second;
                            let response = ui.interact(pill, ui.scope_id().with((label, k)), Sense::click()).on_hover_cursor(CursorIcon::PointingHand);
                            ui.painter().rect_filled(pill, 6, if on { PRIMARY } else if response.hovered() { c.surface_high } else { Color32::TRANSPARENT });
                            ui.painter().text(pill.center(), egui::Align2::CENTER_CENTER, *option, font.clone(), if on { Color32::WHITE } else { PRIMARY });
                            if response.clicked() {
                                clicked = Some(k == 1);
                            }
                            x = pill.right() + 2.0;
                        }
                        // the label gets what is left; "…" (full text on hover) if a longer one ever stops fitting
                        let room = Rect::from_min_max(Pos2::new(rect.left() + 34.0, rect.top()), Pos2::new(frame.left() - 12.0, rect.bottom()));
                        let text = egui::Label::new(egui::RichText::new(label).size(14.0).color(c.text)).truncate().selectable(false);
                        ui.new_child(egui::UiBuilder::new().max_rect(room).layout(egui::Layout::left_to_right(egui::Align::Center))).add(text);
                        if let Some(v) = clicked {
                            *second = v;
                        }
                    };
                    choice(ui, "Modo de seleção por caixa", ["Contido", "Sobreposto"], &mut self.prefs.box_overlap);
                    choice(ui, "Dispositivo de entrada", ["Trackpad", "Mouse"], &mut self.prefs.mouse_wheel_zooms);
                    let check = |ui: &mut egui::Ui, label: &str, shortcut: &str, value: &mut bool| {
                        let icon = if *value { Icon::Check } else { Icon::Blank };
                        if menu_item(ui, icon, label, shortcut, c.text, &c) {
                            *value = !*value;
                        }
                    };
                    check(ui, "Bloqueio de ferramenta", "Q", &mut self.tool_locked);
                    check(ui, "Encaixar em objetos", "Option+S", &mut self.prefs.snap_objects);
                    check(ui, "Alternar grade", "Cmd+'", &mut self.prefs.grid);
                    check(ui, "Modo Zen", "Option+Z", &mut self.prefs.zen);
                    check(ui, "Modo de visualização", "Option+R", &mut self.prefs.view_mode);
                    check(ui, "Propriedades da Tela e da Forma", "Option+/", &mut self.prefs.stats);
                    check(ui, "Atalho de flecha", "", &mut self.prefs.arrow_binding);
                    check(ui, "Encaixar em pontos intermediários", "", &mut self.prefs.midpoint_snapping);
                    check(ui, "Mostrar dicas", "", &mut self.prefs.show_hints);
                    check(ui, "Confirmar antes de excluir arquivos", "", &mut self.prefs.confirm_trash);
                });
            })
            .response
            .rect
    }
}
