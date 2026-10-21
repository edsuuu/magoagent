//! Toolbar, footer, stats panel and hints.

use crate::prelude::*;

impl DrawingApp {
    pub(crate) fn colors(&self) -> UiColors {
        UiColors::new(self.dark())
    }

    pub(crate) fn island(&self) -> egui::Frame {
        let c = self.colors();
        egui::Frame::new()
            .fill(c.island)
            .corner_radius(8)
            .inner_margin(4)
            .stroke(Stroke::new(1.0, Color32::from_black_alpha(if self.dark() { 60 } else { 14 })))
            .shadow(egui::epaint::Shadow { offset: [0, 6], blur: 14, spread: 0, color: Color32::from_black_alpha(14) })
    }

    fn tool_button(&mut self, ui: &mut egui::Ui, tool: Tool, c: &UiColors) {
        let fg = if self.tool == tool && !self.dark() { Color32::from_rgb(0x03, 0x03, 0x5c) } else { c.text };
        let r = icon_button(ui, 36.0, self.tool == tool, Color32::TRANSPARENT, Icon::Tool(tool), fg, c);
        ui.painter().text(r.rect.right_bottom() - Vec2::new(4.0, 3.0), egui::Align2::RIGHT_BOTTOM, tool.badge(), FontId::proportional(9.0), c.muted);
        let tip = match tool.keys() {
            [] => tool.label().to_string(),
            keys => format!("{} — {}", tool.label(), keys.iter().map(|k| k.name()).collect::<Vec<_>>().join(" ou ")),
        };
        if r.on_hover_text(tip).clicked() {
            self.select_tool(tool);
        }
    }

    /// A card that takes the keyboard: the node dialogs or Ajustes.
    pub(crate) fn dialog_open(&self) -> bool {
        self.terminal_dialog.is_some() || self.portal_dialog.is_some() || self.ide_dialog.is_some() || self.settings_open
    }

    pub(crate) fn floating_controls(&mut self, ctx: &Context) {
        let c = self.colors();
        // main menu
        let menu_button = egui::Area::new(egui::Id::unique("main_menu"))
            .fixed_pos((MENU_LEFT, TOP_ROW + 4.0))
            .show(ctx, |ui| icon_button(ui, 36.0, self.menu_open, c.surface_low, Icon::Menu, c.text, &c).on_hover_text("Menu"))
            .inner;
        if menu_button.clicked() {
            self.menu_open = !self.menu_open;
            // every opening starts with both submenus closed
            (self.prefs_open, self.locked_open) = (false, false);
        }
        if self.menu_open {
            self.main_menu(ctx, menu_button.rect);
        }
        self.agents_panel(ctx, &c);

        if self.prefs.view_mode || self.prefs.zen {
            // only a way back, like the reference app's exit buttons
            let (label, view) = if self.prefs.view_mode { ("Sair do modo de visualização", true) } else { ("Sair do modo zen", false) };
            egui::Area::new(egui::Id::unique("exit_mode"))
                .anchor(egui::Align2::CENTER_BOTTOM, (0.0, -16.0))
                .show(ctx, |ui| {
                    let button = egui::Button::new(egui::RichText::new(label).size(14.0).color(c.text)).fill(c.surface_low).corner_radius(8).min_size(Vec2::new(0.0, 36.0));
                    if ui.add(button).clicked() {
                        if view {
                            self.prefs.view_mode = false;
                        } else {
                            self.prefs.zen = false;
                        }
                    }
                });
        }
        if self.prefs.stats {
            self.stats_panel(ctx);
        }
        if self.prefs.view_mode {
            self.footer(ctx);
            if self.palette_open {
                self.command_palette(ctx);
            }
            return;
        }

        // toolbar: the main tools, and the drawing tools in a second row under the last button
        let drawing = self.draw_open || Tool::DRAWING.contains(&self.tool) || Tool::EXTRA.contains(&self.tool);
        let mut extra_anchor: Option<Rect> = None;
        // a window being typed into or enlarged keeps the top to its own bar
        let window_focus = self.focused_window().is_some();
        let mut toolbar = if window_focus {
            Rect::from_min_size(Pos2::new(0.0, TOP_ROW - 8.0), Vec2::ZERO)
        } else {
            egui::Area::new(egui::Id::unique("tools"))
            .anchor(egui::Align2::CENTER_TOP, (0.0, TOP_ROW))
            .show(ctx, |ui| {
                self.island().show(ui, |ui| {
                    ui.horizontal(|ui| {
                        ui.spacing_mut().item_spacing.x = 4.0;
                        // with a selection the lock locks it; otherwise it keeps the tool active
                        let locks_selection = self.locks_selection();
                        let on = self.tool_locked && !locks_selection;
                        let lock = icon_button(ui, 36.0, on, Color32::TRANSPARENT, Icon::Lock(on), c.text, &c);
                        let tip = if locks_selection {
                            "Bloquear seleção — Cmd+Shift+L"
                        } else {
                            "Manter ativa a ferramenta selecionada após desenhar — Q"
                        };
                        if lock.on_hover_text(tip).clicked() {
                            self.toolbar_lock_clicked();
                        }
                        separator(ui, &c);
                        for tool in Tool::TOOLBAR {
                            self.tool_button(ui, tool, &c);
                        }
                        let image = icon_button(ui, 36.0, false, Color32::TRANSPARENT, Icon::Image, c.text, &c);
                        ui.painter().text(image.rect.right_bottom() - Vec2::new(4.0, 3.0), egui::Align2::RIGHT_BOTTOM, "9", FontId::proportional(9.0), c.muted);
                        if image.on_hover_text("Inserir imagem — 9").clicked() {
                            self.insert_image();
                        }
                        separator(ui, &c);
                        if icon_button(ui, 36.0, drawing, Color32::TRANSPARENT, Icon::Shapes, c.text, &c).on_hover_text("Ferramentas de desenho").clicked() {
                            // closing the row puts a drawing tool down
                            if drawing && !Tool::TOOLBAR.contains(&self.tool) {
                                self.select_tool(Tool::Select);
                            }
                            self.draw_open = !drawing;
                            self.extra_open = false;
                        }
                    });
                });
            })
            .response
            .rect
        };
        if drawing && !window_focus {
            toolbar = egui::Area::new(egui::Id::unique("drawing_tools"))
                .anchor(egui::Align2::CENTER_TOP, (0.0, toolbar.bottom() + 8.0))
                .show(ctx, |ui| {
                    self.island().show(ui, |ui| {
                        ui.horizontal(|ui| {
                            ui.spacing_mut().item_spacing.x = 4.0;
                            for tool in Tool::DRAWING {
                                self.tool_button(ui, tool, &c);
                            }
                            separator(ui, &c);
                            let icon = if Tool::EXTRA.contains(&self.tool) { Icon::Tool(self.tool) } else { Icon::More };
                            let more = icon_button(ui, 36.0, Tool::EXTRA.contains(&self.tool) || self.extra_open, Color32::TRANSPARENT, icon, c.text, &c);
                            extra_anchor = Some(more.rect);
                            if more.on_hover_text("Mais ferramentas").clicked() {
                                self.extra_open = !self.extra_open;
                            }
                        });
                    });
                })
                .response
                .rect;
        }
        if let Some(bar) = self.context_bar(ctx, toolbar.bottom() + 8.0).filter(|_| !self.chat_open) {
            toolbar = bar;
        }
        self.toolbar_bottom = toolbar.bottom();
        if let (true, Some(anchor)) = (self.extra_open, extra_anchor) {
            let menu = egui::Area::new(egui::Id::unique("extra_tools"))
                .anchor(egui::Align2::RIGHT_TOP, (anchor.right() - ctx.content_rect().right(), anchor.bottom() + 8.0))
                .order(egui::Order::Foreground)
                .show(ctx, |ui| {
                    self.island().inner_margin(8).show(ui, |ui| {
                        ui.set_width(260.0);
                        ui.spacing_mut().item_spacing.y = 1.0;
                        for tool in Tool::EXTRA {
                            if menu_item(ui, Icon::Tool(tool), tool.label(), tool.badge(), if self.tool == tool { PRIMARY } else { c.text }, &c) {
                                self.select_tool(tool);
                                self.extra_open = false;
                            }
                        }
                    });
                });
            let outside = ctx.input(|i| i.pointer.any_pressed())
                && ctx.input(|i| i.pointer.interact_pos()).is_some_and(|p| !menu.response.rect.contains(p) && !anchor.contains(p));
            if outside {
                self.extra_open = false;
            }
        }

        // hint under toolbar
        if let Some(hint) = self.hint() {
            egui::Area::new(egui::Id::unique("hint"))
                .anchor(egui::Align2::CENTER_TOP, (0.0, toolbar.bottom() + 10.0))
                .interactable(false)
                .show(ctx, |ui| {
                    ui.add(egui::Label::new(egui::RichText::new(hint).size(12.0).color(c.muted)).wrap_mode(egui::TextWrapMode::Extend));
                });
        }

        if self.shows_properties() && !self.prefs.zen {
            self.properties_panel(ctx);
        }
        self.unlock_popup(ctx);

        self.footer(ctx);

        // minimap + its toggle, bottom right
        // separate fixed-size areas: an anchored area that changes size jumps for a frame
        if self.minimap_open && !self.prefs.zen && self.terminal_elevated.is_none() {
            egui::Area::new(egui::Id::unique("minimap"))
                .anchor(egui::Align2::RIGHT_BOTTOM, (-16.0, -16.0 - 36.0 - 8.0))
                .show(ctx, |ui| {
                    self.island().inner_margin(6).show(ui, |ui| self.minimap(ui));
                });
        }
        egui::Area::new(egui::Id::unique("minimap_toggle"))
            .anchor(egui::Align2::RIGHT_BOTTOM, (-16.0, -16.0))
            .enabled(!self.prefs.zen)
            .show(ctx, |ui| {
                group_frame(&c).show(ui, |ui| {
                    let tip = if self.minimap_open { "Ocultar minimapa" } else { "Mostrar minimapa" };
                    if icon_button(ui, 36.0, self.minimap_open, Color32::TRANSPARENT, Icon::Map, c.text, &c).on_hover_text(tip).clicked() {
                        self.minimap_open = !self.minimap_open;
                        ctx.request_repaint();
                    }
                });
            });

        if self.export_open {
            self.export_dialog(ctx);
        }
        // one card at a time: Ajustes (Editar presets…) hides the node dialog under it, so its
        // Esc and Enter reach only the card on top; the dialog comes back when Ajustes closes
        if self.settings_open {
            self.settings_ui(ctx);
        } else if self.terminal_dialog.is_some() {
            self.terminal_dialog_ui(ctx);
        } else if self.portal_dialog.is_some() {
            self.portal_dialog_ui(ctx);
        } else if self.ide_dialog.is_some() {
            self.ide_dialog_ui(ctx);
        }
        self.composer_ui(ctx);
        if self.palette_open {
            self.command_palette(ctx);
        }
        self.status_toast(ctx);
    }

    fn status_toast(&mut self, ctx: &Context) {
        let c = self.colors();
        let now = ctx.input(|i| i.time);
        if self.status_time < 0.0 {
            self.status_time = now;
        }
        if !self.status.is_empty() && now - self.status_time < 3.0 {
            egui::Area::new(egui::Id::unique("toast"))
                .anchor(egui::Align2::CENTER_BOTTOM, (0.0, -24.0))
                .interactable(false)
                .show(ctx, |ui| {
                    self.island().inner_margin(egui::Margin::symmetric(14, 8)).show(ui, |ui| {
                        ui.label(egui::RichText::new(&self.status).color(c.text));
                    });
                });
            ctx.request_repaint_after(std::time::Duration::from_millis(250));
        }
    }

    /// Zoom and undo / redo, bottom left; view mode keeps only the zoom, without the hints
    /// (Cmd+0 doesn't work there).
    fn footer(&mut self, ctx: &Context) {
        let c = self.colors();
        let view = self.prefs.view_mode;
        let tip = |r: egui::Response, text: &str| if view { r } else { r.on_hover_text(text) };
        egui::Area::new(egui::Id::unique("footer"))
            .anchor(egui::Align2::LEFT_BOTTOM, (16.0, -16.0))
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    ui.spacing_mut().item_spacing.x = 8.0;
                    group_frame(&c).show(ui, |ui| {
                        ui.spacing_mut().item_spacing.x = 0.0;
                        if tip(icon_button(ui, 36.0, false, Color32::TRANSPARENT, Icon::Minus, c.text, &c), "Diminuir zoom — Cmd+-").clicked() {
                            self.zoom_step(-0.1);
                        }
                        let (rect, reset) = ui.allocate_exact_size(Vec2::new(58.0, 36.0), Sense::click());
                        let reset = reset.on_hover_cursor(CursorIcon::PointingHand);
                        if reset.hovered() && !view {
                            ui.painter().rect_filled(rect, 8, c.surface_high);
                        }
                        ui.painter().text(
                            rect.center(),
                            egui::Align2::CENTER_CENTER,
                            format!("{}%", (self.zoom * 100.0).round()),
                            FontId::proportional(13.0),
                            c.text,
                        );
                        if tip(reset, "Redefinir zoom — Cmd+0").clicked() {
                            let center = self.canvas_rect.center();
                            self.zoom_at(1.0, center);
                        }
                        if tip(icon_button(ui, 36.0, false, Color32::TRANSPARENT, Icon::Plus, c.text, &c), "Aumentar zoom — Cmd++").clicked() {
                            self.zoom_step(0.1);
                        }
                    });
                    if view {
                        return;
                    }
                    group_frame(&c).show(ui, |ui| {
                        ui.spacing_mut().item_spacing.x = 0.0;
                        for (icon, enabled, text) in [
                            (Icon::Undo, !self.undo.is_empty(), "Desfazer — Cmd+Z"),
                            (Icon::Redo, !self.redo.is_empty(), "Refazer — Cmd+Shift+Z"),
                        ] {
                            let fg = if enabled { c.text } else { c.muted };
                            if icon_button(ui, 36.0, false, Color32::TRANSPARENT, icon, fg, &c).on_hover_text(text).clicked() && enabled {
                                if matches!(icon, Icon::Undo) {
                                    self.undo();
                                } else {
                                    self.redo();
                                }
                            }
                        }
                    });
                });
            });
    }

    /// "Propriedades da Tela e da Forma" (Option+/): scene totals, and editable x/y/size/angle
    /// for a single selected shape.
    pub(crate) fn stats_panel(&mut self, ctx: &Context) {
        let c = self.colors();
        egui::Area::new(egui::Id::unique("stats"))
            .anchor(egui::Align2::RIGHT_TOP, (-16.0, 16.0))
            .show(ctx, |ui| {
                self.island().inner_margin(12).show(ui, |ui| {
                    ui.set_width(200.0);
                    ui.horizontal(|ui| {
                        ui.label(egui::RichText::new("Propriedades").size(16.0).strong().color(c.text));
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            if ui.add(egui::Button::new("×").frame(false)).on_hover_text("Fechar — Option+/").clicked() {
                                self.prefs.stats = false;
                            }
                        });
                    });
                    ui.add_space(6.0);
                    let row = |ui: &mut egui::Ui, label: &str, value: String| {
                        ui.horizontal(|ui| {
                            ui.label(egui::RichText::new(label).size(13.0).color(c.muted));
                            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                ui.label(egui::RichText::new(value).size(13.0).color(c.text));
                            });
                        });
                    };
                    ui.label(egui::RichText::new("Geral").size(13.0).strong().color(c.text));
                    let all = self.scene.elements.iter().map(element_bounds).reduce(|a, b| a.union(b));
                    row(ui, "Formas", self.scene.elements.len().to_string());
                    if let Some(b) = all {
                        row(ui, "Largura", format!("{:.0}", b.width()));
                        row(ui, "Altura", format!("{:.0}", b.height()));
                    }
                    if self.selected.is_empty() {
                        return;
                    }
                    ui.add_space(8.0);
                    ui.label(egui::RichText::new("Propriedades da forma").size(13.0).strong().color(c.text));
                    if let [i] = self.selected[..] {
                        let e = self.scene.elements[i].clone();
                        let b = if e.rotatable() { local_bounds(&e) } else { element_bounds(&e) };
                        let (mut x, mut y, mut w, mut h) = (b.min.x, b.min.y, b.width(), b.height());
                        let mut degrees = e.angle.to_degrees();
                        let mut changed = false;
                        egui::Grid::new("stats_fields").num_columns(4).spacing([6.0, 6.0]).show(ui, |ui| {
                            for (label, value) in [("X", &mut x), ("Y", &mut y)] {
                                ui.label(egui::RichText::new(label).size(13.0).color(c.muted));
                                changed |= ui.add(egui::DragValue::new(value).speed(1.0).max_decimals(0)).changed();
                            }
                            ui.end_row();
                            for (label, value) in [("L", &mut w), ("A", &mut h)] {
                                ui.label(egui::RichText::new(label).size(13.0).color(c.muted));
                                changed |= ui.add(egui::DragValue::new(value).speed(1.0).range(1.0..=100_000.0).max_decimals(0)).changed();
                            }
                            ui.end_row();
                            if e.rotatable() {
                                ui.label(egui::RichText::new("Ângulo").size(13.0).color(c.muted));
                                changed |= ui.add(egui::DragValue::new(&mut degrees).speed(1.0).range(0.0..=360.0).suffix("°").max_decimals(0)).changed();
                                ui.end_row();
                            }
                        });
                        if changed {
                            if !self.style_edit_active {
                                self.snapshot();
                                self.style_edit_active = true;
                            }
                            let mut edited = e.clone();
                            edited.scale(b, Rect::from_min_size(Pos2::new(x, y), Vec2::new(w, h)));
                            if edited.rotatable() {
                                edited.angle = degrees.to_radians().rem_euclid(std::f32::consts::TAU);
                            }
                            self.scene.elements[i] = edited;
                            self.dirty = true;
                        }
                    } else if let Some(b) = self.selection_bounds() {
                        row(ui, "Selecionado", self.selected.len().to_string());
                        row(ui, "Largura", format!("{:.0}", b.width()));
                        row(ui, "Altura", format!("{:.0}", b.height()));
                    }
                });
            });
    }

    pub(crate) fn hint(&self) -> Option<&'static str> {
        if !self.prefs.show_hints || self.prefs.view_mode || self.prefs.zen {
            return None;
        }
        if self.editing.is_some() {
            return Some("Pressione Esc ou Cmd+Enter para salvar as edições");
        }
        if self.terminal_elevated.is_some() {
            return None;
        }
        if self.multi.is_some() {
            return Some("Clique no último ponto ou pressione Esc ou Enter para finalizar");
        }
        if self.rotating.is_some() {
            return Some("Você pode restringir ângulos segurando Shift enquanto rotaciona");
        }
        if self.line_editor.is_some() && self.selected_point.is_some() {
            return Some("Pressione Delete para remover o ponto, ou arraste para mover");
        }
        if self.line_editor.is_some() {
            return Some("Selecione um ponto para editar, ou segure Option e clique para adicionar novos pontos");
        }
        if let (Some(i), Tool::Select) = (self.single_line(), self.tool) {
            return Some(if matches!(self.scene.elements[i].kind, ElementKind::Line { arrow: true, .. }) {
                "Segure Cmd e clique duas vezes ou pressione Cmd+Enter para editar pontos"
            } else {
                "Clique duas vezes ou pressione Cmd+Enter para editar pontos"
            });
        }
        match self.tool {
            Tool::Hand => None,
            Tool::Select if self.scene.elements.is_empty() => {
                Some("Para mover a tela, segure Roda do mouse ou Barra de espaço enquanto arrasta ou use a ferramenta de mão")
            }
            _ if self.connecting.is_some() => Some("Clique no terminal, nota ou portal para conectar — Esc cancela"),
            // already being typed into or open: nothing to teach
            Tool::Select if self.focused_window().is_some() => None,
            Tool::Select if self.only_selected(|e| e.terminal.is_some()) => Some("Clique para digitar; clique duas vezes no título para ampliar"),
            Tool::Select if self.active_node().is_some_and(|e| e.ide.is_some()) => Some("Clique para abrir o projeto"),
            Tool::Select if self.only_selected(Element::is_node) => Some("Arraste pelo título para mover; a página recebe os cliques"),
            Tool::Select => match self.selected[..] {
                [i] if self.scene.elements.get(i).is_some_and(|e| matches!(e.kind, ElementKind::Text { .. }) || e.created.is_some()) => {
                    Some("Clique duas vezes ou pressione Enter para editar texto")
                }
                [_] => Some("Você pode restringir proporções segurando Shift enquanto redimensiona"),
                _ => None,
            },
            Tool::Arrow => Some("Clique para colocar vários pontos, arraste para uma linha única"),
            Tool::Line => Some("Clique para iniciar vários pontos, arraste para uma única linha"),
            Tool::Draw => Some("Toque e arraste, solte quando terminar"),
            Tool::AutoShape => Some("Solte para converter em forma"),
            Tool::Sticky => Some("Clique para inserir uma nota ou arraste para definir o tamanho"),
            Tool::Terminal => Some("Clique para abrir um terminal ou arraste para definir o tamanho"),
            Tool::Portal => Some("Clique para abrir uma página ou arraste para definir o tamanho"),
            Tool::Ide => Some("Clique para abrir um projeto ou arraste para definir o tamanho (ou solte uma pasta no canvas)"),
            Tool::Text => Some("Dica: você também pode adicionar texto clicando duas vezes em qualquer lugar com a ferramenta de seleção"),
            Tool::Eraser => None,
            _ => Some("Segure Shift para proporções iguais e Alt para desenhar a partir do centro"),
        }
    }
}
