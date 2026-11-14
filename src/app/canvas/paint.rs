//! Painting the scene, overlays and the in-place text editor; the shape being dragged out and
//! the cursor.

use crate::prelude::*;

impl DrawingApp {
    pub(crate) fn dark(&self) -> bool {
        match self.prefs.theme {
            Theme::Dark => true,
            Theme::Light => false,
            Theme::System => self.system_dark,
        }
    }

    /// The canvas's ground: the drawing's own color, and in the dark the reference app's gray
    /// while the drawing keeps the white it starts with.
    pub(crate) fn ground(&self) -> Color32 {
        let dark = self.dark();
        if dark && self.scene.background == WHITE { CANVAS_DARK } else { themed(rgba(self.scene.background), dark) }
    }

    pub(crate) fn paint_scene(&self, painter: &Painter, rect: Rect) {
        let dark = self.dark();
        painter.rect_filled(rect, 0.0, self.ground());
        // the open chat is a screen of its own: nothing of the drawing shows, under it or in the
        // strip above it, so none of it paints
        if self.chat_open {
            return;
        }
        if self.prefs.paper || self.prefs.grid {
            // graph paper as the reference app's: 20 px cells, one hairline of its --grid-line (#303030 in the dark,
            // #00000014 in the light) at half strength on a 1x screen; when zoomed far out every 5th line only
            let pixels_per_point = painter.ctx().pixels_per_point();
            // by the ground's own light, not the theme's: a scene's dark color turns light under the dark theme
            let ground = self.ground();
            let light_ground = (ground.r() as u32 + ground.g() as u32 + ground.b() as u32) > 3 * 128;
            let line = if light_ground { Color32::from_black_alpha(0x14) } else { Color32::from_rgb(0x30, 0x30, 0x30) };
            let stroke = if pixels_per_point >= 1.5 {
                Stroke::new(0.5 * 2.0 / pixels_per_point, line)
            } else {
                Stroke::new(1.0 / pixels_per_point, line.gamma_multiply(0.5))
            };
            let (top_left, bottom_right) = (self.to_world(rect.min), self.to_world(rect.max));
            let step = if GRID * self.zoom < 6.0 { GRID * 5.0 } else { GRID };
            let mut x = (top_left.x / step).floor() * step;
            while x <= bottom_right.x {
                painter.vline(self.to_screen(Pos2::new(x, 0.0)).x, rect.y_range(), stroke);
                x += step;
            }
            let mut y = (top_left.y / step).floor() * step;
            while y <= bottom_right.y {
                painter.hline(rect.x_range(), self.to_screen(Pos2::new(0.0, y)).y, stroke);
                y += step;
            }
        }
        let view = View { origin: self.origin(), zoom: self.zoom, dark, background: self.ground() };
        let mut pen = None;
        // ropes first: the part of one that crosses a window or note stays under it, so a
        // connection only ever shows leaving their edges
        // a rope lights up while one of its agents asks the other something
        let asking = self.bridge.as_ref().map(|bridge| bridge.shared.lock().unwrap().asking.clone()).unwrap_or_default();
        for element in self.scene.elements.iter().filter(|element| element.rope) {
            let end = |binding: &Option<Binding>| binding.as_ref().map(|binding| binding.element_id.clone()).unwrap_or_default();
            let (start_id, end_id) = (end(&element.ends.start), end(&element.ends.end));
            let lit = asking.iter().any(|(x, y)| (*x == start_id && *y == end_id) || (*x == end_id && *y == start_id));
            let color = if lit { PRIMARY } else { themed(with_opacity(element.style.stroke, element.style.opacity), dark) };
            let points: Vec<Pos2> = self.rope_path(element).iter().map(|point| self.to_screen(Pos2::new(point[0], point[1]))).collect();
            let stroke = Stroke::new(element.style.stroke_width * self.zoom * if lit { 1.75 } else { 1.0 }, color);
            painter.extend(Shape::dashed_line(&points, stroke, 8.0 * self.zoom, 7.0 * self.zoom));
            // a message just sent: a dot runs from the sender's end to the receiver's, and the
            // frames stop once it arrives
            for (from, to, at) in &self.rope_pulses {
                if !((*from == start_id && *to == end_id) || (*from == end_id && *to == start_id)) {
                    continue;
                }
                if let Some(dot) = pulse_at(&points, self.now - at, *from == end_id) {
                    let radius = (element.style.stroke_width * self.zoom * 1.6).clamp(3.0, 7.0);
                    painter.circle_filled(dot, radius * 2.2, PRIMARY.gamma_multiply(0.25));
                    painter.circle_filled(dot, radius, PRIMARY);
                    painter.ctx().request_repaint();
                }
            }
        }
        if let Some(draft) = self.ropes.get(CONNECT_DRAFT) {
            // "Conectar" waiting for its other end: the rope hangs from the source to the pointer
            let points: Vec<Pos2> = draft.points.iter().map(|point| self.to_screen(*point)).collect();
            painter.extend(Shape::dashed_line(&points, Stroke::new(2.0 * self.zoom, PRIMARY), 8.0 * self.zoom, 7.0 * self.zoom));
        }
        self.paint_groups(painter);
        // windows over the drawings (a portal's page is native and always on top, so its bar
        // and the other windows go there too), and the enlarged one last, over everything it covers
        let mut order: Vec<usize> = (0..self.scene.elements.len()).collect();
        order.sort_by_key(|&index| self.scene.elements[index].is_node());
        if let Some(lifted) = self.elevated_index() {
            order.retain(|index| *index != lifted);
            order.push(lifted);
        }
        for index in order {
            let element = &self.scene.elements[index];
            match self.reveal.iter().position(|id| *id == element.id) {
                Some(0) => {
                    pen = Some(paint_partial(painter, element, &view, self.reveal_progress(element)));
                    continue;
                }
                Some(_) => continue, // still to be drawn
                None => {}
            }
            if hidden_page(&self.scene.elements, element) {
                continue; // waiting behind its pile's front page
            }
            // a card nobody sees costs its whole painting (a terminal walks its grid); the frame is
            // where it shows, enlarged or on its way back included
            let card = element.portal.is_some() || element.terminal.is_some() || element.created.is_some() || element.chat.is_some();
            if card && !self.terminal_frame(element).0.expand(8.0).intersects(rect) {
                continue;
            }
            if element.portal.is_some() {
                self.paint_portal(painter, element);
                continue;
            }
            if element.rope {
                continue; // already drawn, under everything
            }
            if element.filetree.is_some() {
                let (rect, zoom) = self.terminal_frame(element);
                if rect.expand(8.0).intersects(painter.clip_rect()) {
                    self.paint_filetree(painter, element, rect, zoom);
                }
                continue;
            }
            if element.chat.is_some() {
                let (rect, zoom) = self.terminal_frame(element);
                self.paint_chat_node(painter, element, rect, zoom);
                continue;
            }
            if element.terminal.is_some() {
                // an enlarged one is drawn above the canvas instead
                if self.terminal_elevated.as_ref() != Some(&element.id) {
                    let (rect, zoom) = self.terminal_frame(element);
                    self.paint_terminal(painter, element, rect, zoom);
                }
                continue;
            }
            let faded = self.erasing.as_ref().is_some_and(|erased| erased.contains(&index));
            if note_of_label(&self.scene.elements, element).is_some() {
                continue; // a note paints its own text, or the editor types over it
            }
            if element.created.is_some() {
                let label = note_label(&self.scene.elements, index);
                let text = label.and_then(|label_index| match &self.scene.elements[label_index].kind {
                    ElementKind::Text { value, .. } => Some(value.as_str()),
                    _ => None,
                });
                // on the canvas, or lifted: the frame sets where and how big the card is
                let (frame, zoom) = self.terminal_frame(element);
                let view =
                    View { origin: frame.min.to_vec2() - local_bounds(element).min.to_vec2() * zoom, zoom, dark: view.dark, background: view.background };
                // its text, or the placeholder of a note without one (left empty, its text was dropped)
                let body = label.is_none_or(|label_index| self.editing != Some(label_index));
                paint_note(painter, &self.shown_note(element), text.unwrap_or(""), body, &view, faded);
                continue;
            }
            if is_arrow_label(&self.scene.elements, element) && self.editing != Some(index) {
                // the arrow is interrupted behind its label
                let bounds = local_bounds(element).expand(4.0);
                painter.rect_filled(Rect::from_two_pos(self.to_screen(bounds.min), self.to_screen(bounds.max)), 4, view.background);
            }
            let halo = text_needs_halo(&self.scene.elements, element);
            paint_element(painter, element, &view, faded, self.editing == Some(index), halo);
        }
        if let Some((name, last, at)) = &self.agent {
            let fade = if pen.is_some() { 1.0 } else { 1.0 - ((self.now - at) / AGENT_LINGER).clamp(0.0, 1.0) as f32 };
            if fade > 0.0 {
                // toward the pen or where it stopped, smoothly, the way a hand moves
                let target = pen.map_or(*last, |pen_point| self.to_world(pen_point));
                let at = match self.agent_drawn.get() {
                    Some((from, drawn_at)) => from + (target - from) * (1.0 - (-(self.now - drawn_at) as f32 * 18.0).exp()),
                    None => target,
                };
                self.agent_drawn.set(Some((at, self.now)));
                paint_agent_cursor(painter, self.to_screen(at), name, fade);
            } else {
                self.agent_drawn.set(None);
            }
        }
        if let (Some(start), Some(end)) = (self.marquee, self.last_pointer) {
            painter.rect(Rect::from_two_pos(self.to_screen(start), end), 0, PRIMARY.gamma_multiply(0.08), Stroke::new(1.0, PRIMARY), StrokeKind::Inside);
        }
        // the shape an arrow's tip reaches: outlined, and the moment it reaches it the outline pops
        // out once and fades (0.4 s), the sign that the arrow holds on to it
        let suggested = self.suggested_binding();
        let pulse = suggested.and_then(|index| self.scene.elements.get(index)).map(|target| match self.bind_pulse.take() {
            Some((id, at)) if id == target.id => (id, at),
            _ => (target.id.clone(), self.now),
        });
        self.bind_pulse.set(pulse.clone());
        if let Some(target) = suggested.or_else(|| self.connect_hover()).and_then(|index| self.scene.elements.get(index)) {
            // BINDING_HIGHLIGHT_RGB, width clamp(1.75, strokeWidth, 4)
            let outline =
                |grow: f32| -> Vec<Pos2> { binding_outline(target, grow).iter().map(|point| self.to_screen(Pos2::new(point[0], point[1]))).collect() };
            let width = target.style.stroke_width.clamp(1.75, 4.0);
            let color = if dark { Color32::from_rgb(104, 182, 240) } else { Color32::from_rgb(106, 189, 252) };
            painter.add(Shape::closed_line(outline(0.0), Stroke::new(width, color)));
            if let Some((_, at)) = pulse.filter(|(id, _)| *id == target.id) {
                let progress = ((self.now - at) / 0.4).clamp(0.0, 1.0) as f32;
                if progress < 1.0 {
                    let grow = 16.0 * (1.0 - (1.0 - progress).powi(2));
                    painter
                        .add(Shape::closed_line(outline(grow / self.zoom), Stroke::new(width + 2.0 * (1.0 - progress), color.gamma_multiply(1.0 - progress))));
                    painter.ctx().request_repaint();
                }
            }
        }
        if let (Some(index), Tool::Select) = (self.single_line(), self.tool) {
            // orbit bindings show their focus point, dashed from the endpoint (FOCUS_POINT_SIZE)
            let arrow = &self.scene.elements[index];
            let points = self.line_points(index);
            for (end_binding, point_index) in [(&arrow.ends.start, 0), (&arrow.ends.end, points.len() - 1)] {
                let Some(bound) = end_binding.as_ref().filter(|bound| !bound.inside) else {
                    continue;
                };
                if let Some(target) = self.scene.elements.iter().find(|element| element.id == bound.element_id) {
                    let focus = self.to_screen(focus_point(target, bound));
                    let end = self.to_screen(Pos2::new(points[point_index][0], points[point_index][1]));
                    let faint = Stroke::new(1.0, PRIMARY.gamma_multiply(0.5));
                    painter.extend(Shape::dashed_line(&[end, focus], faint, 3.0, 3.0));
                    painter.circle_stroke(focus, 10.0 / 3.0, faint);
                }
            }
        }
        if let Some(path) = &self.lasso {
            // ponytail: no 5% fill inside the trail; egui only fills convex polygons
            painter.add(Shape::line(path.iter().map(|point| self.to_screen(*point)).collect(), Stroke::new(2.0, PRIMARY)));
        }
        // laser trail: red, fading out over one second
        for pair in self.laser.windows(2) {
            let age = (self.now - pair[1].1) as f32;
            if age < 1.0 {
                let color = Color32::from_rgb(0xff, 0x3b, 0x30).gamma_multiply(1.0 - age);
                painter.line_segment([self.to_screen(pair[0].0), self.to_screen(pair[1].0)], Stroke::new(4.0 * (1.0 - age * 0.5), color));
            }
        }
        for [from, to] in &self.snap_guides {
            // object snapping lines, red like the reference app's
            let red = Stroke::new(1.0, Color32::from_rgb(0xff, 0x6b, 0x6b));
            painter.line_segment([self.to_screen(*from), self.to_screen(*to)], red);
        }
        let accent = Stroke::new(1.0, PRIMARY);
        let handle_fill = if dark { Color32::from_rgb(18, 18, 18) } else { Color32::WHITE };
        let two_point_line = self.single_line().is_some_and(|index| self.line_points(index).len() == 2);
        let selected: Vec<&Element> = self.selected.iter().filter_map(|index| self.scene.elements.get(*index)).collect();
        let card = |element: &Element| element.is_node() || element.created.is_some();
        // a note's text is grouped with it and goes along
        // and a rope picked along with its windows has no box of its own
        let with_card = |element: &&Element| {
            card(element) || element.rope || selected.iter().any(|other| card(other) && other.group_id.is_some() && other.group_id == element.group_id)
        };
        // the window in focus (a terminal being typed into, or one enlarged, the IDE open) gets no outline
        let focused = self.focused_window().cloned().or_else(|| self.note_focus_id());
        if selected.iter().any(|element| card(element)) && selected.iter().all(with_card) && self.tool == Tool::Select {
            // terminals, portals and notes: only a dashed rounded outline whose dashes crawl, stand still or
            // aren't drawn (Ajustes; the corners still resize them); drawings keep the straight box with handles
            let outline = self.prefs.window_outline;
            for window in selected.iter().filter(|window| card(window) && Some(&window.id) != focused.as_ref() && outline != WindowOutline::Hidden) {
                let (rect, zoom) = self.terminal_frame(window);
                let time = if outline == WindowOutline::Animated { self.now } else { 0.0 };
                painter.extend(crawling_outline(rect.expand(4.0), 11.0 * zoom.min(1.0) + 1.0, time));
                // 16 px/s: a step of about one pixel every 62 ms, for dashes on the screen
                if outline == WindowOutline::Animated && rect.expand(5.0).intersects(painter.clip_rect()) {
                    painter.ctx().request_repaint_after(std::time::Duration::from_millis(62));
                }
            }
        } else if self.shows_transform_handles() || (self.tool != Tool::Select && self.multi.is_none() && self.editing.is_none() && !two_point_line) {
            if let Some(corners) = self.handle_corners() {
                painter.add(Shape::closed_line(corners.to_vec(), accent));
                if self.shows_transform_handles() {
                    for corner in corners {
                        painter.rect(Rect::from_center_size(corner, Vec2::splat(8.0)), 2, handle_fill, accent, StrokeKind::Middle);
                    }
                    if let Some(handle) = self.rotation_handle() {
                        painter.circle(handle, 4.5, handle_fill, accent);
                    }
                }
            }
        }
        // radii from renderSingleLinearPoint (POINT_HANDLE_SIZE = 10), in the app's blue
        let editing_line = self.line_editor.is_some();
        let radius = if editing_line { 10.0 } else { 5.0 };
        let point_stroke = Stroke::new(1.0, PRIMARY);
        for (_, point) in self.midpoint_handles() {
            painter.circle_filled(point, 5.0, PRIMARY.gamma_multiply(0.6));
        }
        for (point_index, point) in self.point_handles() {
            let selected = editing_line && self.selected_point == Some(point_index);
            let fill = if selected { PRIMARY.gamma_multiply(0.9) } else { Color32::from_rgba_unmultiplied(255, 255, 255, 230) };
            painter.circle(point, radius, themed(fill, dark), point_stroke);
        }
        if let (Some(_), Tool::AutoShape) = (self.drawing_start, self.tool) {
            // faint trail of the stroke plus the shape it would become, at 70% opacity (lines
            // get no preview: nearly every stroke reads as a line at some early point)
            let width = (self.style.stroke_width * 0.65 * self.zoom).clamp(1.25, 4.0);
            let trail = if dark { Color32::from_white_alpha(51) } else { Color32::from_black_alpha(51) };
            painter
                .add(Shape::line(self.drawing_points.iter().map(|point| self.to_screen(Pos2::new(point[0], point[1]))).collect(), Stroke::new(width, trail)));
            if let Some(mut preview) = recognize_shape(self, &self.drawing_points).filter(|shape| !matches!(shape.kind, ElementKind::Line { arrow: false, .. }))
            {
                preview.style.opacity = 70;
                paint_element(painter, &preview, &view, false, false, false);
            }
        } else if let (Some(start), Some(pointer), Tool::Terminal | Tool::Portal | Tool::FileTree | Tool::Sticky) =
            (self.drawing_start, self.last_pointer, self.tool)
        {
            // a window (terminal, portal, tree, note) is dragged out as the same blue box, not a drawn shape
            let area = Rect::from_two_pos(self.to_screen(start), pointer);
            painter.rect(area, 8, PRIMARY.gamma_multiply(0.08), Stroke::new(1.0, PRIMARY), StrokeKind::Inside);
        } else if let (Some(start), Some(pointer)) = (self.drawing_start, self.last_pointer)
            // the chat's card is put where the press was, at its own size: nothing to drag out
            && self.tool != Tool::Chat
        {
            let temporary = self.bind_new_arrow(self.temporary_element(start, self.to_world(pointer)));
            paint_element(painter, &temporary, &view, false, false, false);
        }
    }

    pub(crate) fn text_editor(&mut self, ctx: &Context) {
        let Some(index) = self.editing else {
            return;
        };
        if let Some(note) = self.scene.elements.get(index).and_then(|label| note_of_label(&self.scene.elements, label)) {
            return self.note_editor(ctx, index, note);
        }
        let (pos, size, color, width) = match self.scene.elements.get(index) {
            Some(element @ Element { kind: ElementKind::Text { x, y, .. }, .. }) => (
                self.to_screen(Pos2::new(*x, *y)),
                element.style.font_size * self.zoom,
                themed(rgba(element.style.stroke), self.dark()),
                element_bounds(element).width() * self.zoom,
            ),
            _ => {
                self.editing = None;
                return;
            }
        };
        let mut finish = false;
        egui::Area::new(egui::Id::unique("text_editor")).fixed_pos(pos).order(egui::Order::Foreground).show(ctx, |ui| {
            if let Some(Element { kind: ElementKind::Text { value, .. }, .. }) = self.scene.elements.get_mut(index) {
                ui.spacing_mut().item_spacing = Vec2::ZERO;
                let font = FontId::new(size, egui::FontFamily::Name(HAND_FONT.into()));
                // Text auto-resizes: grow sideways, never wrap
                let mut layouter = |ui: &egui::Ui, text: &dyn egui::TextBuffer, _wrap: f32| {
                    let job = egui::text::LayoutJob::simple(text.as_str().to_owned(), font.clone(), color, f32::INFINITY);
                    ui.ctx().fonts_mut(|fonts| fonts.layout_job(job))
                };
                let response = ui.add(
                    egui::TextEdit::multiline(value)
                        .frame(egui::Frame::NONE)
                        .margin(Vec2::ZERO)
                        .text_color(color)
                        .background_color(Color32::TRANSPARENT)
                        .layouter(&mut layouter)
                        .clip_text(false)
                        .desired_rows(1)
                        .desired_width(width + size * 2.0),
                );
                // the click that creates the text releases on the next frame and counts as a click
                // elsewhere, which takes the focus back: keep asking for it while editing
                if !ui.memory(|memory| memory.has_focus(response.id)) {
                    response.request_focus();
                }
                finish = ui.input(|input| input.key_pressed(Key::Escape) || (input.key_pressed(Key::Enter) && input.modifiers.command));
            }
        });
        if finish {
            self.finish_text();
        }
    }

    /// A note's text being typed: the card's text box, the same markdown, scrolling in place.
    fn note_editor(&mut self, ctx: &Context, index: usize, note: usize) {
        let shown = self.shown_note(&self.scene.elements[note]).into_owned();
        let body = note_text_box(&shown);
        let (frame, zoom) = self.terminal_frame(&self.scene.elements[note]);
        let at = |point: Pos2| frame.min + (point - local_bounds(&shown).min) * zoom;
        let screen = Rect::from_two_pos(at(body.min), at(body.max));
        let (_, ink) = note_colors(&self.scene.elements[note], 100);
        // zoom from the note's frame: an enlarged note grows its text too
        let size = self.scene.elements[note].note.font_size * zoom;
        // a popover or the rename dialog has the keys: the editor waits for them
        let want_focus = self.rename.is_none() && self.note_color.is_none();
        let mut finish = false;
        egui::Area::new(egui::Id::unique("note_editor")).fixed_pos(screen.min).order(egui::Order::Foreground).show(ctx, |ui| {
            ui.set_width(screen.width());
            egui::ScrollArea::vertical().id_salt("note_text").max_height(screen.height()).show(ui, |ui| {
                if let Some(Element { kind: ElementKind::Text { value, .. }, .. }) = self.scene.elements.get_mut(index) {
                    let mut layouter = |ui: &egui::Ui, text: &dyn egui::TextBuffer, wrap: f32| {
                        let job = markdown::edit_job(text.as_str(), size, ink, wrap);
                        ui.ctx().fonts_mut(|fonts| fonts.layout_job(job))
                    };
                    let response = ui.add(
                        egui::TextEdit::multiline(value)
                            .id(crate::app::notes::editor_id())
                            .frame(egui::Frame::NONE)
                            .margin(Vec2::ZERO)
                            .text_color(ink)
                            .background_color(Color32::TRANSPARENT)
                            .layouter(&mut layouter)
                            .desired_width(screen.width())
                            .desired_rows(1),
                    );
                    // the resting note's placeholder, in the same letters and ink (egui's own hint_text takes the theme's
                    // weak color, near white on the paper)
                    if value.is_empty() {
                        let format = egui::TextFormat { font_id: FontId::monospace(size), color: ink.gamma_multiply(0.4), italics: true, ..Default::default() };
                        let galley = ui.fonts_mut(|fonts| fonts.layout_job(egui::text::LayoutJob::single_section("Clique para editar…".into(), format)));
                        ui.painter().galley(response.rect.min, galley, ink);
                    }
                    // the click that focuses the note releases on the next frame and counts as a
                    // click elsewhere, which takes the focus back: keep asking for it while editing
                    if want_focus && !ui.memory(|memory| memory.has_focus(response.id)) {
                        response.request_focus();
                    } else if let Some(range) = egui::TextEdit::load_state(ui.ctx(), response.id).and_then(|state| state.cursor.char_range()) {
                        let (primary, secondary) = (range.primary.index.0, range.secondary.index.0);
                        self.note_selection = Some((index, primary.min(secondary), primary.max(secondary)));
                    }
                    // the modifiers ride on the key event (scripted keys set them only there)
                    finish = ui.input(|input| {
                        input.events.iter().any(|event| {
                            matches!(event, egui::Event::Key { key: Key::Escape, pressed: true, .. })
                                || matches!(event, egui::Event::Key { key: Key::Enter, pressed: true, modifiers, .. } if modifiers.command)
                        })
                    });
                }
            });
        });
        if finish {
            // Esc or Cmd+Enter in a lifted note also puts it back on the canvas (set_elevated ends the typing)
            if self.elevated_index().is_some_and(|index| self.scene.elements[index].created.is_some()) {
                self.set_elevated(None);
            } else {
                self.finish_text();
            }
        }
    }

    /// Shift keeps squares/circles and 15° angles, Alt draws from the center (as in the reference app).
    pub(crate) fn temporary_element(&self, start: Pos2, end: Pos2) -> Element {
        let end = if self.tool == Tool::Draw { end } else { self.snap_point(end) };
        let mut delta = end - start;
        let boxed = matches!(self.tool, Tool::Rectangle | Tool::Ellipse | Tool::Diamond | Tool::Frame | Tool::Sticky | Tool::Terminal | Tool::Portal);
        if self.mods.shift && self.tool != Tool::Sticky && boxed {
            let side = delta.x.abs().max(delta.y.abs());
            delta = Vec2::new(side * delta.x.signum(), side * delta.y.signum());
        }
        if self.mods.shift && matches!(self.tool, Tool::Arrow | Tool::Line) {
            let step = std::f32::consts::PI / 12.0;
            let angle = (delta.y.atan2(delta.x) / step).round() * step;
            delta = Vec2::angled(angle) * delta.length();
        }
        let (first, second) = if self.mods.alt && boxed { (start - delta, start + delta) } else { (start, start + delta) };
        let area = Rect::from_two_pos(first, second);
        let (x, y, width, height) = (area.min.x, area.min.y, area.width().max(1.0), area.height().max(1.0));
        let kind = match self.tool {
            Tool::Draw | Tool::AutoShape => ElementKind::FreeDraw { points: self.drawing_points.clone() },
            Tool::Rectangle | Tool::Frame | Tool::Sticky | Tool::Terminal | Tool::Portal => ElementKind::Rectangle { x, y, width, height },
            Tool::Ellipse => ElementKind::Ellipse { x, y, width, height },
            Tool::Diamond => ElementKind::Diamond { x, y, width, height },
            tool => ElementKind::Line { points: vec![[first.x, first.y], [second.x, second.y]], arrow: tool == Tool::Arrow },
        };
        let mut element = self.new_element(kind);
        if self.tool == Tool::Sticky {
            element.style = sticky_style(element.style);
            element.created = Some(now_ms());
        }
        if self.tool == Tool::Frame {
            // frames: thin gray border, no fill, crisp lines; the web names them all "Frame"
            element.frame = Some("Frame".into());
            element.style = Style { stroke: [0xbb, 0xbb, 0xbb, 255], fill: None, stroke_width: 1.0, roughness: 0, round: true, ..element.style };
        }
        element
    }

    /// The hand of panning: Windows draws Grab and Grabbing as the four-way arrow, so there it is
    /// the system's hand. ponytail: one hand for both states there; a custom open/closed hand
    /// cursor through winit if that matters.
    pub(crate) fn hand_cursor(grabbing: bool) -> CursorIcon {
        match (cfg!(windows), grabbing) {
            (true, _) => CursorIcon::PointingHand,
            (false, true) => CursorIcon::Grabbing,
            (false, false) => CursorIcon::Grab,
        }
    }

    pub(crate) fn update_cursor(&self, ctx: &Context, response: &egui::Response) {
        let Some(pos) = response.hover_pos() else {
            return;
        };
        if !self.on_canvas(ctx, pos, response.layer_id) {
            return;
        }
        let icon = if self.panning {
            Self::hand_cursor(true)
        } else if self.space_pan(ctx) || self.tool == Tool::Hand {
            Self::hand_cursor(false)
        } else if self.connecting.is_some() {
            CursorIcon::Crosshair
        } else if self.moving {
            CursorIcon::Move
        } else {
            match self.tool {
                Tool::Select => self.select_cursor(pos),
                Tool::Text => CursorIcon::Text,
                // the eraser's reach (`hit`'s 10 px) is a circle on the pointer instead of the arrow
                Tool::Eraser => {
                    let layer = egui::LayerId::new(egui::Order::Tooltip, egui::Id::unique("eraser_cursor"));
                    let painter = ctx.layer_painter(layer);
                    painter.circle_stroke(pos, 10.0, Stroke::new(3.0, Color32::from_white_alpha(160)));
                    painter.circle_stroke(pos, 10.0, Stroke::new(1.5, self.colors().text));
                    CursorIcon::None
                }
                // the system has no pencil: the toolbar's, its tip on the pointer, over a halo that reads on any canvas
                Tool::Draw => {
                    let layer = egui::LayerId::new(egui::Order::Tooltip, egui::Id::unique("pencil_cursor"));
                    let painter = ctx.layer_painter(layer);
                    let at = pos + Vec2::new(6.0, -6.0);
                    let dark = ctx.theme() == egui::Theme::Dark;
                    let halo = if dark { Color32::from_black_alpha(200) } else { Color32::from_white_alpha(220) };
                    for offset in [Vec2::new(1.0, 0.0), Vec2::new(-1.0, 0.0), Vec2::new(0.0, 1.0), Vec2::new(0.0, -1.0)] {
                        paint_icon(&painter, Icon::Tool(Tool::Draw), at + offset, halo);
                    }
                    paint_icon(&painter, Icon::Tool(Tool::Draw), at, self.colors().text);
                    CursorIcon::None
                }
                _ => CursorIcon::Crosshair,
            }
        };
        ctx.set_cursor_icon(icon);
    }

    /// The selection tool's cursor says what a press there does, checked in press_select's order:
    /// Move only where the press picks something up.
    pub(crate) fn select_cursor(&self, pos: Pos2) -> CursorIcon {
        let world = self.to_world(pos);
        if self.point_handles().iter().chain(&self.midpoint_handles()).any(|(_, point)| point.distance(pos) <= 10.0) {
            return CursorIcon::PointingHand;
        }
        if self.hovered_rotation(pos) {
            return CursorIcon::Grab;
        }
        match self.hovered_handle(pos) {
            Some(0 | 2) => return CursorIcon::ResizeNwSe,
            Some(_) => return CursorIcon::ResizeNeSw,
            None => {}
        }
        if let Some((_, corner, free)) = self.window_grip(pos) {
            return match (free.x > 0.0, free.y > 0.0) {
                (true, false) => CursorIcon::ResizeHorizontal,
                (false, true) => CursorIcon::ResizeVertical,
                _ if corner % 2 == 0 => CursorIcon::ResizeNwSe,
                _ => CursorIcon::ResizeNeSw,
            };
        }
        // an enlarged note or portal: the inside is its own
        if self.elevated_index().is_some_and(|index| self.terminal_frame(&self.scene.elements[index]).0.contains(pos)) {
            return CursorIcon::Default;
        }
        match self.hit(world) {
            Some(index) if self.scene.elements[index].rope => CursorIcon::PointingHand,
            // a tree's card (header, rows) is clicked, not "moved": its title strip still drags it, with the normal cursor
            Some(index) if self.scene.elements[index].filetree.is_some() => CursorIcon::Default,
            Some(index) if self.drags(index, world, self.mods.shift) => CursorIcon::Move,
            // a body that types: the terminal and a note's text
            Some(index)
                if self.scene.elements[index].terminal.is_some()
                    || (self.scene.elements[index].created.is_some() && self.mods.is_none() && self.on_note_text(index, world)) =>
            {
                CursorIcon::Text
            }
            Some(_) => CursorIcon::Default,
            None if self.selection_bounds().is_some_and(|bounds| bounds.contains(world)) => CursorIcon::Move,
            None => CursorIcon::Default,
        }
    }
}

/// A dashed rounded outline whose dashes crawl along it: the selection of a live window.
fn crawling_outline(rect: Rect, radius: f32, time: f64) -> Vec<Shape> {
    use std::f32::consts::{FRAC_PI_2, PI};
    let round = radius.min(rect.width() / 2.0).min(rect.height() / 2.0);
    let corners = [
        (rect.right_top() + Vec2::new(-round, round), -FRAC_PI_2),
        (rect.right_bottom() + Vec2::new(-round, -round), 0.0),
        (rect.left_bottom() + Vec2::new(round, -round), FRAC_PI_2),
        (rect.left_top() + Vec2::new(round, round), PI),
    ];
    let mut path: Vec<Pos2> =
        corners.iter().flat_map(|(center, start)| (0..=6).map(move |step| *center + Vec2::angled(start + step as f32 / 6.0 * FRAC_PI_2) * round)).collect();
    path.push(path[0]);
    let (dash, gap) = (6.0, 4.0);
    let offset = (time * 16.0) as f32 % (dash + gap);
    Shape::dashed_line_with_offset(&path, Stroke::new(1.5, PRIMARY), &[dash], &[gap], offset)
}
