//! The properties row (a glass row under the toolbars) and the export dialog.

use crate::app::ui::palette::Command;
use crate::app::ui::{color_picker, dialog_check, dialog_ui, swatch};
use crate::prelude::*;

pub(crate) fn apply_changed(target: &mut Style, before: &Style, after: &Style) {
    macro_rules! fields {
        ($($f:ident),*) => { $( if before.$f != after.$f { target.$f = after.$f; } )* };
    }
    fields!(stroke, fill, stroke_width, stroke_style, fill_style, roughness, round, opacity, font_size);
}

/// A choice of the row: its icon, tip, whether it is the current one, and the style it gives.
type Choice = (Icon, &'static str, bool, Style);

/// What a button of the row is. A `Pick` is a few values: a button each, or one button holding
/// them in a popover when the row would not fit.
enum Part {
    Stroke,
    Fill,
    Opacity,
    Name,
    Head(bool),
    Pick(&'static str, &'static str, Vec<Choice>),
    Act(Command, Icon, &'static str),
}

/// The opacity button's width (it shows "100%"), a swatch's, and a popover's text field or slider.
const OPACITY_WIDTH: f32 = 36.0;
const SWATCH: f32 = 22.0;
const POPOVER_FIELD: f32 = 150.0;

/// How far along the row's entrance is `t` seconds in: the reference app's spring (0.3 s visual
/// duration, bounce 0.4), overshooting a little before it rests, by 0.7 s.
fn pop(t: f32) -> f32 {
    let (w0, zeta) = (std::f32::consts::TAU / 0.36, 0.6_f32);
    let wd = w0 * (1.0 - zeta * zeta).sqrt();
    let t = t.max(0.0);
    if t >= 0.7 { 1.0 } else { 1.0 - (-zeta * w0 * t).exp() * ((wd * t).cos() + zeta * w0 / wd * (wd * t).sin()) }
}

/// A round button on glass showing a color (a slash through it when there is none).
fn glass_dot(ui: &mut egui::Ui, color: Option<[u8; 4]>, on: bool, dark: bool) -> egui::Response {
    let (rect, r) = ui.allocate_exact_size(Vec2::splat(GLASS_BUTTON), Sense::click());
    let ink = glass_ink(dark);
    if r.hovered() {
        ui.painter().circle_filled(rect.center(), GLASS_BUTTON / 2.0, ink.gamma_multiply(0.1));
    }
    let edge = Stroke::new(1.0, if on { glass_accent(dark) } else { ink.gamma_multiply(0.3) });
    match color {
        Some(c) => {
            ui.painter().circle(rect.center(), 8.0, themed(rgba(c), dark), edge);
        }
        None => {
            ui.painter().circle_stroke(rect.center(), 8.0, edge);
            ui.painter().line_segment([rect.center() + Vec2::new(-5.5, 5.5), rect.center() + Vec2::new(5.5, -5.5)], edge);
        }
    }
    r.on_hover_cursor(CursorIcon::PointingHand)
}

impl DrawingApp {
    pub(crate) fn shows_properties(&self) -> bool {
        // windows (terminals, portals, IDEs, notes and a note's text), images (drawn with none of it
        // but the opacity) and ropes don't take the drawing's style
        let window = |i: &usize| {
            self.scene
                .elements
                .get(*i)
                .is_some_and(|e| e.is_node() || e.rope || e.created.is_some() || e.image.is_some() || note_of_label(&self.scene.elements, e).is_some())
        };
        // nor the note and text tools, nor a text being typed: a typed text, selected, still has it
        let typing = self.editing.and_then(|i| self.scene.elements.get(i)).is_some_and(|e| matches!(e.kind, ElementKind::Text { .. }) && !is_label(e));
        !typing
            && (self.editing.is_some_and(|i| !window(&i))
                || (!self.selected.is_empty() && !self.selected.iter().all(window))
                || !matches!(
                    self.tool,
                    Tool::Select
                        | Tool::Hand
                        | Tool::Eraser
                        | Tool::Laser
                        | Tool::Lasso
                        | Tool::Terminal
                        | Tool::Portal
                        | Tool::Ide
                        | Tool::FileTree
                        | Tool::Sticky
                        | Tool::Text
                ))
    }

    /// Closes the row's popover on Esc (the key is theirs while one is open, not the canvas's deselect).
    pub(crate) fn close_properties_popover(&mut self, ctx: &Context) -> bool {
        let open = ctx.data(|d| d.get_temp::<&'static str>(egui::Id::unique("props_open"))).is_some() || self.head_picker.is_some();
        let esc = open && ctx.input(|i| i.key_pressed(Key::Escape));
        // egui's own color popup takes its Esc first
        if esc && !egui::Popup::is_any_open(ctx) {
            ctx.data_mut(|d| d.remove::<&'static str>(egui::Id::unique("props_open")));
            self.head_picker = None;
        }
        esc
    }

    /// The drawing's options as a glass row under the toolbars, centered over the canvas, with the
    /// reference app's entrance (a spring from above); what doesn't fit as buttons opens in a glass
    /// popover under its button. Gives the row's rect, `None` while there is nothing to show.
    pub(crate) fn properties_row(&mut self, ctx: &Context, top: f32) -> Option<Rect> {
        let (since_id, open_id, rects_id) = (egui::Id::unique("props_since"), egui::Id::unique("props_open"), egui::Id::unique("props_buttons"));
        if !self.shows_properties() {
            ctx.data_mut(|d| {
                d.remove::<f64>(since_id);
                d.remove::<&'static str>(open_id);
            });
            self.head_picker = None;
            return None;
        }
        let dark = self.dark();
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
        let heads_before = kinds.first().filter(|_| arrows_only).map_or(self.heads, |e| (e.ends.start_head, e.ends.end_head));
        let mut heads = heads_before;
        let has_selection = !self.selected.is_empty();
        let before = kinds.first().map(|e| e.style).unwrap_or(self.style);
        let mut s = before;
        let mut action = None;

        // the buttons in their groups
        let pick = |key, label, choices: Vec<Choice>| vec![Part::Pick(key, label, choices)];
        let mut groups: Vec<Vec<Part>> = vec![];
        groups.push(vec![Part::Stroke].into_iter().filter(|_| !frames_only && !fill_tool).chain(Some(Part::Fill).filter(|_| fillable)).collect());
        if fillable && (s.fill.is_some() || fill_tool) {
            let fills = [
                (FillStyle::Hachure, Icon::Hachure, "Hachura"),
                (FillStyle::CrossHatch, Icon::CrossHatch, "Hachura cruzada"),
                (FillStyle::Solid, Icon::SolidFill, "Sólido"),
            ];
            groups.push(pick("fill_style", "Preenchimento", fills.map(|(v, i, t)| (i, t, s.fill_style == v, Style { fill_style: v, ..s })).into()));
        }
        if stroked {
            let widths = [(1.0, "Fino"), (2.0, "Espesso"), (4.0, "Muito espesso")];
            groups.push(pick(
                "width",
                "Espessura do traço",
                widths.map(|(v, t)| (Icon::Width(v), t, s.stroke_width == v, Style { stroke_width: v, ..s })).into(),
            ));
            let dashes = [(StrokeStyle::Solid, "Sólido"), (StrokeStyle::Dashed, "Tracejado"), (StrokeStyle::Dotted, "Pontilhado")];
            groups.push(pick("dash", "Estilo do traço", dashes.map(|(v, t)| (Icon::Dash(v), t, s.stroke_style == v, Style { stroke_style: v, ..s })).into()));
            let roughs = [(0, "Arquiteto"), (1, "Artista"), (2, "Cartunista")];
            groups.push(pick(
                "rough",
                "Precisão do traço",
                roughs.map(|(v, t)| (Icon::Sloppiness(v), t, s.roughness == v, Style { roughness: v, ..s })).into(),
            ));
        }
        if arrows_only {
            let types = [(false, "Seta reta"), (true, "Seta curva")];
            groups.push(pick("arrow", "Tipo de seta", types.map(|(v, t)| (Icon::ArrowType(v), t, s.round == v, Style { round: v, ..s })).into()));
            groups.push(vec![Part::Head(false), Part::Head(true)]);
        }
        if edged {
            let edges = [(false, "Pontudo"), (true, "Arredondado")];
            groups.push(pick("edge", "Arestas", edges.map(|(v, t)| (Icon::Edge(v), t, s.round == v, Style { round: v, ..s })).into()));
        }
        if texty {
            let sizes = [(16.0, "Pequeno", "S"), (20.0, "Médio", "M"), (28.0, "Grande", "L"), (36.0, "Muito grande", "XL")];
            groups.push(pick("font", "Tamanho da fonte", sizes.map(|(v, t, l)| (Icon::Label(l), t, s.font_size == v, Style { font_size: v, ..s })).into()));
        }
        groups.push(vec![Part::Name].into_iter().filter(|_| frame_index.is_some()).chain(Some(Part::Opacity)).collect());
        // no Camadas: the order keeps its keys, the palette and the right-click menu
        if has_selection {
            let mut acts = vec![(Command::Duplicate, Icon::Duplicate, "Duplicar — Cmd+D"), (Command::Delete, Icon::Trash, "Apagar — Delete")];
            if self.selected.len() > 1 {
                acts.push((Command::Group, Icon::Group, "Agrupar seleção — Cmd+G"));
            }
            if self.selected_has_group() {
                acts.push((Command::Ungroup, Icon::Ungroup, "Desagrupar seleção — Cmd+Shift+G"));
            }
            acts.push((Command::LockSelection, Icon::Lock(true), "Bloquear — Cmd+Shift+L"));
            groups.push(acts.into_iter().map(|(c, i, t)| Part::Act(c, i, t)).collect());
        }
        groups.retain(|g| !g.is_empty());

        // as buttons while the row fits the canvas, else one button per choice group
        let width_of = |compact: bool| {
            let parts = groups.iter().enumerate().flat_map(|(g, parts)| {
                (g > 0).then_some(1.0).into_iter().chain(parts.iter().flat_map(move |p| match p {
                    Part::Pick(.., choices) if !compact => vec![GLASS_BUTTON; choices.len()],
                    Part::Opacity => vec![OPACITY_WIDTH],
                    _ => vec![GLASS_BUTTON],
                }))
            });
            glass_width(0, &parts.collect::<Vec<_>>())
        };
        let avail = ctx.content_rect().width() - self.sidebar_width() - 24.0;
        let compact = width_of(false) > avail;
        let width = width_of(compact);
        let center = (ctx.content_rect().width() + self.sidebar_width()) / 2.0;
        let pos = Pos2::new(center - width / 2.0, top);

        // the entrance: opacity in 0.15 s, and from 24 above at 90% in the reference app's spring
        let now = ctx.input(|i| i.time);
        let since = ctx.data_mut(|d| *d.get_temp_mut_or_insert_with(since_id, || now));
        let t = (now - since) as f32;
        let lift = 1.0 - pop(t);
        let scale = 1.0 - 0.1 * lift;
        let pivot = pos + Vec2::new(width / 2.0, GLASS_HEIGHT / 2.0);
        let area_id = egui::Id::unique("properties");
        ctx.set_transform_layer(
            egui::LayerId::new(egui::Order::Middle, area_id),
            egui::emath::TSTransform::new(pivot.to_vec2() * (1.0 - scale) + Vec2::new(0.0, -24.0 * lift), scale),
        );
        if t < 0.7 {
            ctx.request_repaint();
        }

        let open = ctx.data(|d| d.get_temp::<&'static str>(open_id));
        let (mut next_open, mut anchor, mut head_anchor) = (open, None, None);
        let mut rects = vec![];
        let (head_open, mut picked) = (self.head_picker, None);
        let mut head_clicked = None;
        let ink = glass_ink(dark);
        egui::Area::new(area_id).fixed_pos(pos).show(ctx, |ui| {
            ui.set_opacity(1.0 - (1.0 - (t / 0.15).clamp(0.0, 1.0)).powi(2));
            glass_row(ui, width, dark, |ui| {
                for (g, parts) in groups.iter().enumerate() {
                    if g > 0 {
                        glass_divider(ui, dark);
                    }
                    for part in parts {
                        // a button that opens a popover: `key` is it while open
                        let mut opener = |key: &'static str, r: egui::Response, tip: &str| {
                            let r = r.tip(tip);
                            if r.clicked() {
                                next_open = if open == Some(key) { None } else { Some(key) };
                            }
                            if open == Some(key) {
                                anchor = Some(r.rect);
                            }
                            rects.push(r.rect);
                        };
                        match part {
                            Part::Stroke => opener("stroke", glass_dot(ui, Some(s.stroke), open == Some("stroke"), dark), "Contorno"),
                            Part::Fill => {
                                let fill = s.fill.filter(|c| c[3] > 0);
                                opener("fill", glass_dot(ui, fill, open == Some("fill"), dark), "Fundo")
                            }
                            Part::Opacity => {
                                let (rect, r) = ui.allocate_exact_size(Vec2::new(OPACITY_WIDTH, GLASS_BUTTON), Sense::click());
                                if r.hovered() {
                                    ui.painter().rect_filled(rect, GLASS_BUTTON / 2.0, ink.gamma_multiply(0.1));
                                }
                                let color = if open == Some("opacity") { glass_accent(dark) } else { ink.gamma_multiply(0.85) };
                                ui.painter().text(rect.center(), egui::Align2::CENTER_CENTER, format!("{}%", s.opacity), FontId::proportional(11.0), color);
                                opener("opacity", r.on_hover_cursor(CursorIcon::PointingHand), "Opacidade");
                            }
                            Part::Name => opener("name", glass_button(ui, open == Some("name"), Icon::Edit, dark), "Nome do quadro"),
                            Part::Head(end) => {
                                let current = if *end { heads.1 } else { heads.0 };
                                let r = glass_button(ui, head_open == Some(*end), Icon::Head(current, !*end), dark).tip(if *end {
                                    "Ponta final"
                                } else {
                                    "Ponta inicial"
                                });
                                if r.clicked() {
                                    head_clicked = Some(*end);
                                }
                                if head_open == Some(*end) {
                                    head_anchor = Some(r.rect);
                                }
                                rects.push(r.rect);
                            }
                            Part::Pick(key, label, choices) if compact => {
                                let shown = choices.iter().find(|c| c.2).unwrap_or(&choices[0]).0;
                                opener(key, glass_button(ui, open == Some(*key), shown, dark), label);
                            }
                            Part::Pick(.., choices) => {
                                for (icon, tip, on, style) in choices {
                                    let r = glass_button(ui, *on, *icon, dark).tip(*tip);
                                    if r.clicked() {
                                        picked = Some(*style);
                                    }
                                    rects.push(r.rect);
                                }
                            }
                            Part::Act(command, icon, tip) => {
                                let r = glass_button(ui, false, *icon, dark).tip(keys_label(tip));
                                if r.clicked() {
                                    action = Some(*command);
                                }
                                rects.push(r.rect);
                            }
                        }
                    }
                }
            });
        });
        if let Some(style) = picked {
            s = style;
        }
        let (left, right) = if self.canvas_rect.is_finite() { (self.canvas_rect.left(), self.canvas_rect.right()) } else { (0.0, ctx.content_rect().right()) };
        // under its button, kept inside the canvas
        let under = |button: Rect, width: f32| {
            Pos2::new((button.center().x - width / 2.0).clamp(left + 6.0, (right - 6.0 - width).max(left + 6.0)), button.bottom() + 10.0)
        };
        let mut popover = Rect::NOTHING;
        let mut inside = vec![];
        if let (Some(key), Some(button)) = (next_open.filter(|k| Some(*k) == open), anchor) {
            let mut others = vec![];
            let mut choices = &[][..];
            match key {
                "stroke" => others = [vec![SWATCH; STROKE_PICKS.len()], vec![GLASS_BUTTON]].concat(),
                "fill" => others = [vec![SWATCH; BACKGROUND_PICKS.len() - fill_tool as usize], vec![GLASS_BUTTON]].concat(),
                "opacity" | "name" => others = vec![POPOVER_FIELD],
                _ => {
                    choices =
                        groups.iter().flatten().find_map(|p| if let Part::Pick(k, _, c) = p { (*k == key).then_some(&c[..]) } else { None }).unwrap_or(&[]);
                }
            }
            let width = glass_width(choices.len(), &others);
            let area = egui::Area::new(egui::Id::unique("props_popover")).fixed_pos(under(button, width)).order(egui::Order::Foreground).show(ctx, |ui| {
                glass_row(ui, width, dark, |ui| {
                    ui.spacing_mut().interact_size = Vec2::new(GLASS_BUTTON, 22.0);
                    match key {
                        "stroke" => {
                            for (color, name) in STROKE_PICKS.into_iter().zip(["Preto", "Vermelho", "Verde", "Azul", "Laranja"]) {
                                let r = swatch(ui, color, s.stroke == color, SWATCH, dark).tip(name);
                                if r.clicked() {
                                    s.stroke = color;
                                }
                                inside.push(r.rect);
                            }
                            color_picker(ui, &mut s.stroke);
                        }
                        "fill" => {
                            let current = s.fill.unwrap_or(TRANSPARENT);
                            // the fill tool never paints transparent
                            let names = ["Transparente", "Vermelho claro", "Verde claro", "Azul claro", "Amarelo claro"];
                            for (color, name) in BACKGROUND_PICKS.into_iter().zip(names).filter(|(c, _)| !fill_tool || c[3] > 0) {
                                let r = swatch(ui, color, current == color, SWATCH, dark).tip(name);
                                if r.clicked() {
                                    s.fill = (color[3] > 0).then_some(color);
                                }
                                inside.push(r.rect);
                            }
                            let mut chosen = current;
                            color_picker(ui, &mut chosen);
                            if chosen != current {
                                s.fill = (chosen[3] > 0).then_some(chosen);
                            }
                        }
                        "opacity" => {
                            let mut opacity = s.opacity as f32;
                            ui.spacing_mut().slider_width = POPOVER_FIELD;
                            ui.add(egui::Slider::new(&mut opacity, 0.0..=100.0).show_value(false).step_by(10.0)).tip("Opacidade");
                            s.opacity = opacity as u8;
                        }
                        "name" => {
                            if let Some(name) = frame_index.and_then(|i| self.scene.elements[i].frame.as_mut()) {
                                if ui.add(egui::TextEdit::singleline(name).desired_width(POPOVER_FIELD)).tip("Nome do quadro").changed() {
                                    self.dirty = true;
                                }
                            }
                        }
                        _ => {
                            for (icon, tip, on, style) in choices {
                                let r = glass_button(ui, *on, *icon, dark).tip(*tip);
                                if r.clicked() {
                                    s = *style;
                                    next_open = None;
                                }
                                inside.push(r.rect);
                            }
                        }
                    }
                });
            });
            popover = area.response.rect;
        }
        // the heads' popover, as the reference app's: Q W E R picks, then more
        if let (Some(end), Some(button)) = (self.head_picker.filter(|_| head_clicked.is_none()), head_anchor) {
            let current = if end { heads.1 } else { heads.0 };
            let more = self.head_picker_more;
            let (mut chosen, mut toggled) = (None, false);
            let area =
                egui::Area::new(egui::Id::unique("head_picker")).fixed_pos(under(button, glass_width(5, &[]))).order(egui::Order::Foreground).show(ctx, |ui| {
                    ui.spacing_mut().item_spacing.y = GLASS_GAP;
                    let keys = ["Q", "W", "E", "R"];
                    glass_row(ui, glass_width(5, &[]), dark, |ui| {
                        for (k, pick) in Head::PICKS[..4].iter().enumerate() {
                            let r = glass_button(ui, current == *pick, Icon::Head(*pick, !end), dark);
                            ui.painter().text(
                                r.rect.right_bottom() + Vec2::new(1.0, 2.0),
                                egui::Align2::RIGHT_BOTTOM,
                                keys[k],
                                FontId::proportional(8.0),
                                ink.gamma_multiply(0.6),
                            );
                            if r.tip(format!("{} — {}", Head::label(*pick), keys[k])).clicked() {
                                chosen = Some(*pick);
                            }
                        }
                        toggled = glass_button(ui, more, Icon::More, dark).tip(if more { "Menos opções" } else { "Mais opções" }).clicked();
                    });
                    if more {
                        glass_row(ui, glass_width(5, &[]), dark, |ui| {
                            for pick in &Head::PICKS[4..] {
                                if glass_button(ui, current == *pick, Icon::Head(*pick, !end), dark).tip(Head::label(*pick)).clicked() {
                                    chosen = Some(*pick);
                                }
                            }
                        });
                    }
                });
            popover = area.response.rect;
            if toggled {
                self.head_picker_more = !more;
            }
            if let Some(pick) = chosen {
                *(if end { &mut heads.1 } else { &mut heads.0 }) = pick;
                self.head_picker = None;
            }
        }
        if let Some(end) = head_clicked {
            self.head_picker = if self.head_picker == Some(end) { None } else { Some(end) };
            self.head_picker_more = false;
            next_open = None;
        } else if next_open != open {
            self.head_picker = None;
        }
        let outside = ctx.input(|i| i.pointer.any_pressed())
            && ctx.input(|i| i.pointer.interact_pos()).is_some_and(|p| !popover.contains(p) && !rects.iter().any(|r| r.contains(p)))
            && !egui::Popup::is_any_open(ctx);
        if outside {
            next_open = None;
            self.head_picker = None;
        }
        // a popover whose button is gone (another selection, the row fitting again) is put away
        if open.is_some() && anchor.is_none() {
            next_open = None;
        }
        ctx.data_mut(|d| {
            match next_open {
                Some(key) => {
                    d.insert_temp(open_id, key);
                }
                None => d.remove::<&'static str>(open_id),
            }
            d.insert_temp(rects_id, (rects, inside));
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
        Some(Rect::from_min_size(pos, Vec2::new(width, GLASS_HEIGHT)))
    }

    pub(crate) fn export_dialog(&mut self, ctx: &Context) {
        let c = self.colors();
        let (background, scale, svg) = (&mut self.export_background, &mut self.export_scale, &mut self.export_svg);
        let (ok, cancel) = dialog_ui(ctx, &c, "Exportar imagem", "Exportar", |ui| {
            // a row of choices under its small name, as the other dialogs' fields
            let choices = |ui: &mut egui::Ui, label: &str, picks: &[(&str, bool)]| -> Option<usize> {
                ui.add_space(6.0);
                dialog_caption(ui, label, &c);
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
