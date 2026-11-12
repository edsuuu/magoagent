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
            .shadow(egui::epaint::Shadow::NONE)
    }

    fn tool_button(&mut self, ui: &mut egui::Ui, tool: Tool, c: &UiColors) {
        let r = glass_button(ui, self.tool == tool, Icon::Tool(tool), self.dark());
        // its keys are shown only while they answer (shortcuts.rs)
        let live = self.tool_keys_live(tool);
        if live {
            ui.painter().text(r.rect.right_bottom() + Vec2::new(1.0, 2.0), egui::Align2::RIGHT_BOTTOM, tool.badge(), FontId::proportional(8.0), c.muted);
        }
        let tip = match tool.keys() {
            keys if live && !keys.is_empty() => format!("{} — {}", tool.label(), keys.iter().map(|k| k.name()).collect::<Vec<_>>().join(" ou ")),
            _ => tool.label().to_string(),
        };
        if r.tip(tip).clicked() {
            self.select_tool(tool);
        }
    }

    /// A card that takes the keyboard: the node dialogs, Exportar or Ajustes.
    pub(crate) fn dialog_open(&self) -> bool {
        self.terminal_dialog.is_some()
            || self.portal_dialog.is_some()
            || self.ide_dialog.is_some()
            || self.filetree_dialog_open()
            || self.design_confirm.is_some()
            || self.space_dialog.is_some()
            || self.floors_ui.is_open()
            || self.settings_open
            || self.export_open
    }

    pub(crate) fn floating_controls(&mut self, ctx: &Context) {
        let c = self.colors();
        // ☰, the chat's button and the space in use: beside the window buttons on macOS; elsewhere
        // they live in the drawn title bar
        if TITLE_BAR == 0.0 {
            let first = self.top_button(0);
            egui::Area::new(egui::Id::unique("top_buttons")).fixed_pos(first.min).show(ctx, |ui| {
                let end = self.top_buttons(ui);
                ui.allocate_rect(Rect::from_min_max(first.min, Pos2::new(end, first.bottom())), Sense::hover());
            });
        }
        self.agents_panel(ctx, &c);
        self.returned_prompts(ctx);

        // toolbar: the main tools, and the drawing tools in a second row under the last button
        let drawing = self.drawing();
        let mut extra_anchor: Option<Rect> = None;
        // an enlarged window keeps the top to its own bar, and the chat has none; one clicked or typed
        // into on the canvas keeps the toolbar, with its bar under it
        let window_focus = (self.terminal_elevated.is_some() && self.dock.is_none()) || self.chat_open;
        // the toolbars are centered over the canvas, which starts past the sidebar
        let (dark, middle) = (self.dark(), self.sidebar_width() / 2.0);
        let mut toolbar = if window_focus {
            // the window's bar goes up in the toolbar's place, in the title bar
            Rect::from_min_size(Pos2::new(0.0, TOP_ROW - 8.0), Vec2::ZERO)
        } else {
            egui::Area::new(egui::Id::unique("tools"))
                .anchor(egui::Align2::CENTER_TOP, (middle, TOP_ROW))
                .show(ctx, |ui| {
                    // the tools, a hair, the drawing row's button
                    glass_row(ui, glass_width(Tool::TOOLBAR.len() + 1, &[1.0]), dark, |ui| {
                        for tool in Tool::TOOLBAR {
                            self.tool_button(ui, tool, &c);
                        }
                        glass_divider(ui, dark);
                        if glass_button(ui, drawing, Icon::Shapes, dark).tip("Ferramentas de desenho").clicked() {
                            // closing the row puts a drawing tool down
                            if drawing && !Tool::TOOLBAR.contains(&self.tool) {
                                self.select_tool(Tool::Select);
                            }
                            self.draw_open = !drawing;
                            self.extra_open = false;
                        }
                    });
                })
                .response
                .rect
        };
        self.toolbar_span = toolbar;
        if drawing && !window_focus {
            toolbar = egui::Area::new(egui::Id::unique("drawing_tools"))
                .anchor(egui::Align2::CENTER_TOP, (middle, toolbar.bottom() + 8.0))
                .show(ctx, |ui| {
                    glass_row(ui, glass_width(Tool::DRAWING.len() + 1, &[1.0]), dark, |ui| {
                        for tool in Tool::DRAWING {
                            self.tool_button(ui, tool, &c);
                        }
                        glass_divider(ui, dark);
                        let icon = if Tool::EXTRA.contains(&self.tool) { Icon::Tool(self.tool) } else { Icon::More };
                        let more = glass_button(ui, Tool::EXTRA.contains(&self.tool) || self.extra_open, icon, dark);
                        extra_anchor = Some(more.rect);
                        if more.tip("Mais ferramentas").clicked() {
                            self.extra_open = !self.extra_open;
                        }
                    });
                })
                .response
                .rect;
        }
        // the drawing's options, under the toolbars
        if let Some(row) = if window_focus { None } else { self.properties_row(ctx, toolbar.bottom() + 8.0) } {
            toolbar = row;
        }
        // not even drawn under the chat
        if let Some(bar) = if self.chat_open { None } else { self.context_bar(ctx, toolbar.bottom() + 8.0) } {
            toolbar = bar;
            // in the title bar its clicks are its own, not the window's drag
            if window_focus {
                self.toolbar_span = bar;
            }
        }
        self.toolbar_bottom = toolbar.bottom();
        if let (true, Some(anchor)) = (self.extra_open, extra_anchor) {
            let menu = egui::Area::new(egui::Id::unique("extra_tools"))
                .anchor(egui::Align2::RIGHT_TOP, (anchor.right() - ctx.content_rect().right(), anchor.bottom() + 8.0))
                .order(egui::Order::Foreground)
                .show(ctx, |ui| {
                    menu_frame(self.dark()).show(ui, |ui| {
                        ui.set_width(280.0);
                        ui.spacing_mut().item_spacing.y = 1.0;
                        for tool in Tool::EXTRA {
                            if extra_row(ui, tool, self.tool == tool, &c) {
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
        if let Some(hint) = self.hint().filter(|_| !self.chat_open) {
            egui::Area::new(egui::Id::unique("hint")).anchor(egui::Align2::CENTER_TOP, (middle, toolbar.bottom() + 10.0)).interactable(false).show(ctx, |ui| {
                ui.add(egui::Label::new(egui::RichText::new(keys_label(hint)).size(12.0).color(c.muted)).wrap_mode(egui::TextWrapMode::Extend));
            });
        }

        self.unlock_popup(ctx);

        // the corners are the canvas's: a focused window has them out of its way until it lets go, and over the chat
        // there is no canvas to zoom or map
        if self.focused_window().is_none() && !self.chat_open {
            self.footer(ctx);

            // the minimap's toggle, left of the zoom, and the minimap over the row while open
            // separate fixed-size areas: an anchored area that changes size jumps for a frame
            let right = -CORNER_GAP - self.corner_inset().1;
            let toggle = egui::Area::new(egui::Id::unique("minimap_toggle"))
                .anchor(egui::Align2::RIGHT_BOTTOM, (right - ZOOM_WIDTH - PILL_GAP, -CORNER_GAP))
                .show(ctx, |ui| glass_row(ui, glass_width(1, &[]), dark, |ui| glass_button(ui, self.prefs.minimap, Icon::Map, dark).tip("Minimapa")))
                .inner;
            if toggle.clicked() {
                self.prefs.minimap = !self.prefs.minimap;
                ctx.request_repaint();
            } else if self.prefs.minimap {
                let panel = egui::Area::new(egui::Id::unique("minimap")).anchor(egui::Align2::RIGHT_BOTTOM, (right, -60.0)).show(ctx, |ui| self.minimap(ui));
                // a popover, as in the reference app: Esc or a press anywhere else puts it away
                let outside = ctx.input(|i| i.pointer.any_pressed())
                    && ctx.input(|i| i.pointer.interact_pos()).is_some_and(|p| !panel.response.rect.contains(p) && !toggle.rect.contains(p));
                if panel.inner || outside || ctx.input(|i| i.key_pressed(Key::Escape)) {
                    self.prefs.minimap = false;
                }
            }
            // left of the toggle, right to left: the plans' use, then the floors (each gives how wide it drew)
            let mut right = right - ZOOM_WIDTH - PILL_GAP - glass_width(1, &[]) - PILL_GAP;
            let usage = self.usage_pill(ctx, right);
            if usage > 0.0 {
                right -= usage + PILL_GAP;
            }
            self.floors_pill(ctx, right);
        }

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
        } else if self.filetree_dialog_open() {
            self.filetree_dialog_ui(ctx);
        } else if self.design_confirm.is_some() {
            self.design_confirm_ui(ctx);
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
        if !self.status.is_empty() && now - self.status_time < toast_seconds(&self.status) {
            egui::Area::new(egui::Id::unique("toast")).anchor(egui::Align2::CENTER_BOTTOM, (0.0, -24.0)).interactable(false).show(ctx, |ui| {
                self.island().inner_margin(egui::Margin::symmetric(14, 8)).show(ui, |ui| {
                    ui.label(egui::RichText::new(&self.status).color(c.text));
                });
            });
            ctx.request_repaint_after(std::time::Duration::from_millis(250));
        }
    }

    /// The zoom, last in the bottom right row (the floors, the plan's use, the minimap's toggle,
    /// the zoom), each in its glass.
    fn footer(&mut self, ctx: &Context) {
        let dark = self.dark();
        let ink = glass_ink(dark);
        let tip = |r: egui::Response, text: &str| r.tip(keys_label(text));
        let anchor = (-CORNER_GAP - self.corner_inset().1, -CORNER_GAP);
        egui::Area::new(egui::Id::unique("footer")).anchor(egui::Align2::RIGHT_BOTTOM, anchor).show(ctx, |ui| {
            glass_row(ui, ZOOM_WIDTH, dark, |ui| {
                if tip(glass_button(ui, false, Icon::Minus, dark), "Diminuir zoom — Cmd+-").clicked() {
                    self.zoom_step(-0.1);
                }
                let (rect, reset) = ui.allocate_exact_size(Vec2::new(ZOOM_TEXT, 20.0), Sense::click_and_drag());
                let reset = reset.on_hover_cursor(CursorIcon::ResizeHorizontal);
                // dragging sideways scrubs the zoom, 4 points to a 5% step, as in the reference app
                if let (true, Some(pos)) = (reset.dragged(), reset.interact_pointer_pos()) {
                    let key = reset.id.with("start");
                    if reset.drag_started() {
                        ui.data_mut(|d| d.insert_temp(key, (pos.x, self.zoom)));
                    }
                    if let Some((x, zoom)) = ui.data(|d| d.get_temp::<(f32, f32)>(key)) {
                        let next = ((zoom + (pos.x - x) / 4.0 * 0.05) / 0.05).round() * 0.05;
                        let center = self.canvas_rect.center();
                        self.zoom_at(next.max(0.1), center);
                    }
                }
                if reset.hovered() {
                    ui.painter().rect_filled(rect, 10, ink.gamma_multiply(0.1));
                }
                let percent = format!("{}%", (self.zoom * 100.0).round());
                ui.painter().text(rect.center(), egui::Align2::CENTER_CENTER, percent, FontId::monospace(12.0), ink.gamma_multiply(0.85));
                if tip(reset, "Redefinir zoom — Cmd+0").clicked() {
                    let center = self.canvas_rect.center();
                    self.zoom_at(1.0, center);
                }
                if tip(glass_button(ui, false, Icon::Plus, dark), "Aumentar zoom — Cmd++").clicked() {
                    self.zoom_step(0.1);
                }
            });
        });
    }

    pub(crate) fn hint(&self) -> Option<&'static str> {
        if !self.prefs.show_hints {
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
        if self.floors_ui.overview {
            return None;
        }
        match self.tool {
            Tool::Hand => None,
            Tool::Select if self.scene.elements.is_empty() => {
                Some("Para mover a tela, segure Roda do mouse ou Barra de espaço enquanto arrasta ou use a ferramenta de mão")
            }
            _ if self.connecting.is_some() => Some("Clique no terminal, chat, IDE, nota ou navegador para conectar — Esc cancela"),
            // already being typed into or open: nothing to teach
            Tool::Select if self.focused_window().is_some() => None,
            Tool::Select if self.only_selected(|e| e.terminal.is_some()) => Some("Clique para digitar; clique duas vezes no título para ampliar"),
            Tool::Select if self.active_node().is_some_and(|e| e.ide.is_some()) => Some("Duplo clique no título para ampliar o projeto"),
            // the reference app has no hint over its file tree
            Tool::Select if self.only_selected(|e| e.filetree.is_some()) => None,
            Tool::Select if self.only_selected(|e| e.chat.is_some()) => Some("Clique duas vezes para abrir o chat"),
            Tool::Select if self.only_selected(Element::is_node) => Some("Arraste pelo título para mover; a página recebe os cliques"),
            Tool::Select => match self.selected[..] {
                [i] if self.scene.elements.get(i).is_some_and(|e| matches!(e.kind, ElementKind::Text { .. }) || e.created.is_some()) => {
                    Some("Clique duas vezes ou pressione Enter para editar texto")
                }
                // a rope isn't resized
                [i] if self.scene.elements.get(i).is_some_and(|e| e.rope) => None,
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
            Tool::FileTree => Some("Clique para mostrar uma pasta ou arraste para definir o tamanho"),
            Tool::Ide => Some("Clique para abrir um projeto ou arraste para definir o tamanho (ou solte uma pasta no canvas)"),
            Tool::Text => Some("Dica: você também pode adicionar texto clicando duas vezes em qualquer lugar com a ferramenta de seleção"),
            Tool::Eraser => None,
            _ => Some("Segure Shift para proporções iguais e Alt para desenhar a partir do centro"),
        }
    }
}

/// How long a toast stays: 3 s, and longer the more there is to read (a warning with where a file
/// was kept takes more than a glance), up to 12 s.
fn toast_seconds(text: &str) -> f64 {
    (text.chars().count() as f64 / 18.0 + 1.5).clamp(3.0, 12.0)
}

#[cfg(test)]
mod tests {
    #[test]
    fn a_longer_toast_stays_longer() {
        let seconds = super::toast_seconds;
        assert_eq!(seconds("Copiado"), 3.0);
        assert!((4.5..5.5).contains(&seconds(&"a".repeat(60))), "about five seconds for a line");
        assert_eq!(seconds(&"a".repeat(400)), 12.0);
    }
}

/// A row of the "Mais ferramentas" menu: `menu_look::menu_row`'s look with the item and its shortcut 2 px larger.
fn extra_row(ui: &mut egui::Ui, tool: Tool, current: bool, c: &UiColors) -> bool {
    use super::menu_look::{ICON_SIZE, ITEM_RADIUS, MENU_TEXT_SIZE, ROW_GAP, ROW_HEIGHT, ROW_PAD, menu_colors};
    let colors = menu_colors(ui.visuals().dark_mode);
    let (rect, response) = ui.allocate_exact_size(Vec2::new(ui.available_width(), ROW_HEIGHT), Sense::click());
    let response = response.on_hover_cursor(CursorIcon::PointingHand);
    let p = ui.painter();
    if response.hovered() {
        p.rect_filled(rect, ITEM_RADIUS, colors.hover);
    }
    let ink = if current { PRIMARY } else { c.text };
    let size = MENU_TEXT_SIZE + 2.0;
    paint_icon_scaled(p, Icon::Tool(tool), Pos2::new(rect.left() + ROW_PAD + ICON_SIZE / 2.0, rect.center().y), ink, ICON_SIZE / 16.0, 0.0);
    let x = rect.left() + ROW_PAD + ICON_SIZE + ROW_GAP;
    p.text(Pos2::new(x, rect.center().y), egui::Align2::LEFT_CENTER, tool.label(), FontId::proportional(size), ink);
    p.text(Pos2::new(rect.right() - ROW_PAD, rect.center().y), egui::Align2::RIGHT_CENTER, keys_label(tool.badge()), FontId::proportional(size), colors.muted);
    response.clicked()
}
