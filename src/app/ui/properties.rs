//! Properties panel and export dialog.

use crate::prelude::*;
use crate::app::ui::palette::Command;
use crate::app::ui::{dialog_check, dialog_label, dialog_ui};

pub(crate) fn apply_changed(target: &mut Style, before: &Style, after: &Style) {
    macro_rules! fields {
        ($($f:ident),*) => { $( if before.$f != after.$f { target.$f = after.$f; } )* };
    }
    fields!(stroke, fill, stroke_width, stroke_style, fill_style, roughness, round, opacity, font_size);
}

impl DrawingApp {
    pub(crate) fn shows_properties(&self) -> bool {
        // windows (terminals, portals, IDEs, notes and a note's text), images (drawn with none of it
        // but the opacity) and ropes don't take the drawing's style
        let window = |i: &usize| {
            self.scene.elements.get(*i).is_some_and(|e| e.is_node() || e.rope || e.created.is_some() || e.image.is_some() || note_of_label(&self.scene.elements, e).is_some())
        };
        // nor the note and text tools, nor a text being typed: a typed text, selected, still has it
        let typing = self.editing.and_then(|i| self.scene.elements.get(i)).is_some_and(|e| matches!(e.kind, ElementKind::Text { .. }) && !is_label(e));
        !typing
            && (self.editing.is_some_and(|i| !window(&i))
                || (!self.selected.is_empty() && !self.selected.iter().all(window))
                || !matches!(self.tool, Tool::Select | Tool::Hand | Tool::Eraser | Tool::Laser | Tool::Lasso | Tool::Terminal | Tool::Portal | Tool::Ide | Tool::Sticky | Tool::Text))
    }

    pub(crate) fn properties_panel(&mut self, ctx: &Context) {
        let c = self.colors();
        let kinds: Vec<Element> = if self.selected.is_empty() {
            let probe = match self.tool {
                Tool::Rectangle | Tool::Fill | Tool::Frame => ElementKind::Rectangle { x: 0.0, y: 0.0, width: 1.0, height: 1.0 },
                Tool::Diamond => ElementKind::Diamond { x: 0.0, y: 0.0, width: 1.0, height: 1.0 },
                Tool::Ellipse => ElementKind::Ellipse { x: 0.0, y: 0.0, width: 1.0, height: 1.0 },
                tool => ElementKind::Line { points: vec![], arrow: tool == Tool::Arrow },
            };
            vec![self.new_element(probe)]
        } else {
            self.selected.iter().filter_map(|i| self.scene.elements.get(*i).cloned()).collect()
        };
        // like the web: frames only take opacity and layers, the fill tool only its fill
        let frames_only = if self.selected.is_empty() { self.tool == Tool::Frame } else { kinds.iter().all(|e| e.frame.is_some()) };
        let fill_tool = self.selected.is_empty() && self.tool == Tool::Fill;
        let fillable = !frames_only && kinds.iter().any(Element::fillable);
        let stroked = !frames_only && !fill_tool && kinds.iter().any(|e| !matches!(e.kind, ElementKind::Text { .. }));
        let edged = !frames_only && !fill_tool && kinds.iter().any(|e| matches!(e.kind, ElementKind::Rectangle { .. } | ElementKind::Diamond { .. }));
        let texty = kinds.iter().any(|e| matches!(e.kind, ElementKind::Text { .. }));
        let frame_index = match self.selected[..] {
            [i] if self.scene.elements.get(i).is_some_and(|e| e.frame.is_some()) => Some(i),
            _ => None,
        };
        let arrows_only = kinds.iter().all(|e| matches!(e.kind, ElementKind::Line { arrow: true, .. }));
        let heads_before = kinds
            .first()
            .filter(|_| arrows_only)
            .map_or(self.heads, |e| (e.ends.start_head, e.ends.end_head));
        let mut heads = heads_before;
        let mut picker_anchor = None;
        let has_selection = !self.selected.is_empty();
        let before = kinds.first().map(|e| e.style).unwrap_or(self.style);
        let mut s = before;
        let mut action = None;
        let dark = self.dark();

        egui::Area::new(egui::Id::unique("properties"))
            .fixed_pos((16.0, 76.0))
            .show(ctx, |ui| {
                self.island().inner_margin(12).show(ui, |ui| {
                    ui.set_width(178.0);
                    ui.spacing_mut().item_spacing = Vec2::new(4.0, 4.0);
                    if !frames_only && !fill_tool {
                        section(ui, "Contorno", &c);
                        ui.horizontal(|ui| {
                            for color in STROKE_PICKS {
                                if swatch(ui, color, s.stroke == color, 22.0, dark).clicked() {
                                    s.stroke = color;
                                }
                            }
                            ui.add_space(4.0);
                            color_picker(ui, &mut s.stroke);
                        });
                    }
                    if fillable {
                        section(ui, "Fundo", &c);
                        ui.horizontal(|ui| {
                            let current = s.fill.unwrap_or(TRANSPARENT);
                            // the fill tool never paints transparent
                            for color in BACKGROUND_PICKS.into_iter().filter(|c| !fill_tool || c[3] > 0) {
                                if swatch(ui, color, current == color, 22.0, dark).clicked() {
                                    s.fill = (color[3] > 0).then_some(color);
                                }
                            }
                            ui.add_space(4.0);
                            let mut picked = current;
                            color_picker(ui, &mut picked);
                            if picked != current {
                                s.fill = (picked[3] > 0).then_some(picked);
                            }
                        });
                        if s.fill.is_some() || fill_tool {
                            section(ui, "Preenchimento", &c);
                            ui.horizontal(|ui| {
                                for (value, icon, tip) in [
                                    (FillStyle::Hachure, Icon::Hachure, "Hachura"),
                                    (FillStyle::CrossHatch, Icon::CrossHatch, "Hachura cruzada"),
                                    (FillStyle::Solid, Icon::SolidFill, "Sólido"),
                                ] {
                                    if prop_button(ui, s.fill_style == value, icon, tip, &c) {
                                        s.fill_style = value;
                                    }
                                }
                            });
                        }
                    }
                    if stroked {
                        section(ui, "Espessura do traço", &c);
                        ui.horizontal(|ui| {
                            for (value, tip) in [(1.0, "Fino"), (2.0, "Espesso"), (4.0, "Muito espesso")] {
                                if prop_button(ui, s.stroke_width == value, Icon::Width(value), tip, &c) {
                                    s.stroke_width = value;
                                }
                            }
                        });
                        section(ui, "Estilo do traço", &c);
                        ui.horizontal(|ui| {
                            for (value, tip) in [
                                (StrokeStyle::Solid, "Sólido"),
                                (StrokeStyle::Dashed, "Tracejado"),
                                (StrokeStyle::Dotted, "Pontilhado"),
                            ] {
                                if prop_button(ui, s.stroke_style == value, Icon::Dash(value), tip, &c) {
                                    s.stroke_style = value;
                                }
                            }
                        });
                        section(ui, "Precisão do traço", &c);
                        ui.horizontal(|ui| {
                            for (value, tip) in [(0, "Arquiteto"), (1, "Artista"), (2, "Cartunista")] {
                                if prop_button(ui, s.roughness == value, Icon::Sloppiness(value), tip, &c) {
                                    s.roughness = value;
                                }
                            }
                        });
                    }
                    if arrows_only {
                        section(ui, "Tipo de seta", &c);
                        ui.horizontal(|ui| {
                            for (value, tip) in [(false, "Seta reta"), (true, "Seta curva")] {
                                if prop_button(ui, s.round == value, Icon::ArrowType(value), tip, &c) {
                                    s.round = value;
                                }
                            }
                        });
                        section(ui, "Pontas", &c);
                        ui.horizontal(|ui| {
                            for (end, tip) in [(false, "Ponta inicial"), (true, "Ponta final")] {
                                let current = if end { heads.1 } else { heads.0 };
                                let response = icon_button(ui, 32.0, self.head_picker == Some(end), c.surface_low, Icon::Head(current, !end), c.text, &c);
                                if response.clicked() {
                                    self.head_picker = if self.head_picker == Some(end) { None } else { Some(end) };
                                    self.head_picker_more = false;
                                }
                                if self.head_picker == Some(end) {
                                    picker_anchor = Some(response.rect.left_bottom());
                                }
                                response.on_hover_text(tip);
                            }
                        });
                    }
                    if edged {
                        section(ui, "Arestas", &c);
                        ui.horizontal(|ui| {
                            for (value, tip) in [(false, "Pontudo"), (true, "Arredondado")] {
                                if prop_button(ui, s.round == value, Icon::Edge(value), tip, &c) {
                                    s.round = value;
                                }
                            }
                        });
                    }
                    if let Some(i) = frame_index {
                        section(ui, "Nome do quadro", &c);
                        if let Some(name) = self.scene.elements[i].frame.as_mut() {
                            if ui.add(egui::TextEdit::singleline(name).desired_width(178.0)).changed() {
                                self.dirty = true;
                            }
                        }
                    }
                    if texty {
                        section(ui, "Tamanho da fonte", &c);
                        ui.horizontal(|ui| {
                            for (value, tip, label) in
                                [(16.0, "Pequeno", "S"), (20.0, "Médio", "M"), (28.0, "Grande", "L"), (36.0, "Muito grande", "XL")]
                            {
                                if prop_button(ui, s.font_size == value, Icon::Label(label), tip, &c) {
                                    s.font_size = value;
                                }
                            }
                        });
                    }
                    section(ui, "Opacidade", &c);
                    let mut opacity = s.opacity as f32;
                    ui.spacing_mut().slider_width = 178.0;
                    ui.add(egui::Slider::new(&mut opacity, 0.0..=100.0).show_value(false).step_by(10.0));
                    s.opacity = opacity as u8;
                    ui.horizontal(|ui| {
                        ui.label(egui::RichText::new("0").size(11.0).color(c.text));
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            ui.label(egui::RichText::new("100").size(11.0).color(c.text));
                        });
                    });
                    if has_selection {
                        section(ui, "Camadas", &c);
                        ui.horizontal(|ui| {
                            for (value, icon, tip) in [
                                (Layer::Back, Icon::ToBack, "Enviar para o fundo — Cmd+Option+["),
                                (Layer::Backward, Icon::Backward, "Enviar para trás — Cmd+["),
                                (Layer::Forward, Icon::Forward, "Trazer para a frente — Cmd+]"),
                                (Layer::Front, Icon::ToFront, "Trazer para o primeiro plano — Cmd+Option+]"),
                            ] {
                                if prop_button(ui, false, icon, tip, &c) {
                                    action = Some(Command::Layer(value));
                                }
                            }
                        });
                        section(ui, "Ações", &c);
                        ui.horizontal(|ui| {
                            if prop_button(ui, false, Icon::Duplicate, "Duplicar — Cmd+D", &c) {
                                action = Some(Command::Duplicate);
                            }
                            if prop_button(ui, false, Icon::Trash, "Apagar — Delete", &c) {
                                action = Some(Command::Delete);
                            }
                            if self.selected.len() > 1 && prop_button(ui, false, Icon::Group, "Agrupar seleção — Cmd+G", &c) {
                                action = Some(Command::Group);
                            }
                            if self.selected_has_group() && prop_button(ui, false, Icon::Ungroup, "Desagrupar seleção — Cmd+Shift+G", &c) {
                                action = Some(Command::Ungroup);
                            }
                            if prop_button(ui, false, Icon::Lock(true), "Bloquear — Cmd+Shift+L", &c) {
                                action = Some(Command::LockSelection);
                            }
                        });
                    }
                });
            });

        if s != before {
            if has_selection {
                if !self.style_edit_active {
                    self.snapshot();
                    self.style_edit_active = true;
                }
                for i in self.selected.clone() {
                    if let Some(e) = self.scene.elements.get_mut(i) {
                        apply_changed(&mut e.style, &before, &s);
                    }
                }
                self.dirty = true;
            }
            let current = self.style;
            apply_changed(&mut self.style, &current, &s);
            if arrows_only {
                // arrow type is remembered separately from the box "edges" setting
                self.arrow_curved = s.round;
                self.style.round = current.round;
            }
        }
        if let (Some(end), Some(anchor)) = (self.head_picker, picker_anchor) {
            // popover like the reference app's: Q W E R picks, then "Mais opções"
            let current = if end { heads.1 } else { heads.0 };
            let mut picked = None;
            let popup = egui::Area::new(egui::Id::unique("head_picker"))
                .fixed_pos(anchor + Vec2::new(8.0, 4.0))
                .order(egui::Order::Foreground)
                .show(ctx, |ui| {
                    self.island().inner_margin(6).show(ui, |ui| {
                        ui.spacing_mut().item_spacing = Vec2::new(4.0, 4.0);
                        let mut row = |ui: &mut egui::Ui, picks: &[Option<Head>], keys: &[&str]| {
                            ui.horizontal(|ui| {
                                for (k, &pick) in picks.iter().enumerate() {
                                    let r = icon_button(ui, 32.0, current == pick, c.surface_low, Icon::Head(pick, !end), c.text, &c);
                                    if let Some(key) = keys.get(k) {
                                        ui.painter().text(r.rect.right_bottom() - Vec2::new(3.0, 2.0), egui::Align2::RIGHT_BOTTOM, *key, FontId::proportional(8.0), c.muted);
                                    }
                                    let tip = keys.get(k).map_or(Head::label(pick).to_string(), |key| format!("{} — {key}", Head::label(pick)));
                                    if r.on_hover_text(tip).clicked() {
                                        picked = Some(pick);
                                    }
                                }
                            });
                        };
                        row(ui, &Head::PICKS[..4], &["Q", "W", "E", "R"]);
                        let label = if self.head_picker_more { "Mais opções  ⌃" } else { "Mais opções  ⌄" };
                        if ui.add(egui::Label::new(egui::RichText::new(label).size(12.0).color(c.text)).sense(Sense::click())).clicked() {
                            self.head_picker_more = !self.head_picker_more;
                        }
                        if self.head_picker_more {
                            row(ui, &Head::PICKS[4..], &[]);
                        }
                    });
                });
            if let Some(pick) = picked {
                *(if end { &mut heads.1 } else { &mut heads.0 }) = pick;
                self.head_picker = None;
            } else if ctx.input(|i| i.pointer.any_pressed())
                // ponytail: x > 230 skips the properties panel (16px margin + 202px island); use its rect if it ever moves
                && ctx.input(|i| i.pointer.interact_pos()).is_some_and(|p| !popup.response.rect.contains(p) && p.x > 230.0)
            {
                self.head_picker = None;
            }
        }
        if heads != heads_before {
            if has_selection {
                self.snapshot();
                for i in self.selected.clone() {
                    if let Some(e) = self.scene.elements.get_mut(i) {
                        (e.ends.start_head, e.ends.end_head) = heads;
                    }
                }
            }
            self.heads = heads;
        }
        if !arrows_only {
            self.head_picker = None;
        }
        if !ctx.input(|i| i.pointer.primary_down()) {
            self.style_edit_active = false;
        }
        if let Some(cmd) = action {
            self.run_command(cmd);
        }
    }

    pub(crate) fn export_dialog(&mut self, ctx: &Context) {
        let c = self.colors();
        let (background, scale, svg) = (&mut self.export_background, &mut self.export_scale, &mut self.export_svg);
        let (ok, cancel) = dialog_ui(ctx, &c, "Exportar imagem", "Exportar", |ui| {
            // a row of choices under its small name, as the other dialogs' fields
            let choices = |ui: &mut egui::Ui, label: &str, picks: &[(&str, bool)]| -> Option<usize> {
                ui.add_space(6.0);
                ui.label(egui::RichText::new(label).size(11.5).color(dialog_label(&c)));
                let mut picked = None;
                ui.horizontal(|ui| {
                    for (i, (text, on)) in picks.iter().enumerate() {
                        if ui.selectable_label(*on, *text).clicked() {
                            picked = Some(i);
                        }
                    }
                });
                picked
            };
            if let Some(i) = choices(ui, "Formato", &[("PNG", !*svg), ("SVG", *svg)]) {
                *svg = i == 1;
            }
            let scales = [1.0, 2.0, 3.0];
            if let Some(i) = choices(ui, "Escala", &scales.map(|s| (["1×", "2×", "3×"][s as usize - 1], *scale == s))) {
                *scale = scales[i];
            }
            ui.add_space(8.0);
            dialog_check(ui, background, "Fundo", &c);
        });
        if ok {
            self.export_open = false;
            self.export(!self.export_svg);
        } else if cancel {
            self.export_open = false;
        }
    }
}
