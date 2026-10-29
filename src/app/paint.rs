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

    pub(crate) fn paint_scene(&self, painter: &Painter, rect: Rect) {
        let dark = self.dark();
        painter.rect_filled(rect, 0.0, themed(rgba(self.scene.background), dark));
        // the open chat is a screen of its own: nothing of the drawing shows, under it or in the
        // strip above it, so none of it paints
        if self.chat_open {
            return;
        }
        if self.prefs.grid {
            // 20px grid, every 5th line stronger; minor lines vanish when zoomed far out
            let ink = if dark { Color32::WHITE } else { Color32::BLACK };
            let (minor, major) = (Stroke::new(1.0, ink.gamma_multiply(0.05)), Stroke::new(1.0, ink.gamma_multiply(0.1)));
            let (top_left, bottom_right) = (self.to_world(rect.min), self.to_world(rect.max));
            let step = if GRID * self.zoom < 6.0 { GRID * 5.0 } else { GRID };
            let mut x = (top_left.x / step).floor() * step;
            while x <= bottom_right.x {
                let sx = self.to_screen(Pos2::new(x, 0.0)).x;
                let s = if (x / GRID).round() as i64 % 5 == 0 { major } else { minor };
                painter.vline(sx, rect.y_range(), s);
                x += step;
            }
            let mut y = (top_left.y / step).floor() * step;
            while y <= bottom_right.y {
                let sy = self.to_screen(Pos2::new(0.0, y)).y;
                let s = if (y / GRID).round() as i64 % 5 == 0 { major } else { minor };
                painter.hline(rect.x_range(), sy, s);
                y += step;
            }
        }
        let view = View { origin: self.origin(), zoom: self.zoom, dark, background: themed(rgba(self.scene.background), dark) };
        let mut pen = None;
        // ropes first: the part of one that crosses a window or note stays under it, so a
        // connection only ever shows leaving their edges
        // a rope lights up while one of its agents asks the other something
        let asking = self.bridge.as_ref().map(|b| b.shared.lock().unwrap().asking.clone()).unwrap_or_default();
        for element in self.scene.elements.iter().filter(|e| e.rope) {
            let end = |b: &Option<Binding>| b.as_ref().map(|b| b.element_id.clone()).unwrap_or_default();
            let (a, b) = (end(&element.ends.start), end(&element.ends.end));
            let lit = asking.iter().any(|(x, y)| (*x == a && *y == b) || (*x == b && *y == a));
            let color = if lit { PRIMARY } else { themed(with_opacity(element.style.stroke, element.style.opacity), dark) };
            let points: Vec<Pos2> = self.rope_path(element).iter().map(|p| self.to_screen(Pos2::new(p[0], p[1]))).collect();
            let stroke = Stroke::new(element.style.stroke_width * self.zoom * if lit { 1.75 } else { 1.0 }, color);
            painter.extend(Shape::dashed_line(&points, stroke, 8.0 * self.zoom, 7.0 * self.zoom));
        }
        if let Some(draft) = self.ropes.get(CONNECT_DRAFT) {
            // "Conectar" waiting for its other end: the rope hangs from the source to the pointer
            let points: Vec<Pos2> = draft.points.iter().map(|p| self.to_screen(*p)).collect();
            painter.extend(Shape::dashed_line(&points, Stroke::new(2.0 * self.zoom, PRIMARY), 8.0 * self.zoom, 7.0 * self.zoom));
        }
        // the enlarged window goes last, over everything it covers
        let mut order: Vec<usize> = (0..self.scene.elements.len()).collect();
        if let Some(lifted) = self.elevated_index() {
            order.retain(|j| *j != lifted);
            order.push(lifted);
        }
        for i in order {
            let element = &self.scene.elements[i];
            match self.reveal.iter().position(|id| *id == element.id) {
                Some(0) => {
                    pen = Some(paint_partial(painter, element, &view, self.reveal_progress(element)));
                    continue;
                }
                Some(_) => continue, // still to be drawn
                None => {}
            }
            // a card nobody sees costs its whole painting (a terminal walks its grid); the frame is
            // where it shows, enlarged or on its way back included
            let card = element.portal.is_some() || element.ide.is_some() || element.terminal.is_some() || element.created.is_some();
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
            if element.ide.is_some() {
                // the card; enlarged, the live editor is drawn above the canvas instead
                if self.terminal_elevated.as_ref() != Some(&element.id) {
                    let (rect, zoom) = self.terminal_frame(element);
                    self.paint_ide(painter, element, rect, zoom);
                }
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
            let faded = self.erasing.as_ref().is_some_and(|e| e.contains(&i));
            if note_of_label(&self.scene.elements, element).is_some() {
                continue; // a note paints its own text, or the editor types over it
            }
            if element.created.is_some() {
                let label = note_label(&self.scene.elements, i);
                let text = label.and_then(|j| match &self.scene.elements[j].kind {
                    ElementKind::Text { value, .. } => Some(value.as_str()),
                    _ => None,
                });
                // on the canvas, or lifted: the frame sets where and how big the card is
                let (frame, zoom) = self.terminal_frame(element);
                let view =
                    View { origin: frame.min.to_vec2() - local_bounds(element).min.to_vec2() * zoom, zoom, dark: view.dark, background: view.background };
                let body = label.is_some_and(|j| self.editing != Some(j));
                let bottom = paint_note(painter, element, text.unwrap_or(""), body, &view, faded);
                if body {
                    self.note_text_bottom.borrow_mut().insert(element.id.clone(), bottom);
                }
                continue;
            }
            if is_arrow_label(&self.scene.elements, element) && self.editing != Some(i) {
                // the arrow is interrupted behind its label
                let b = local_bounds(element).expand(4.0);
                painter.rect_filled(Rect::from_two_pos(self.to_screen(b.min), self.to_screen(b.max)), 4, view.background);
            }
            let halo = text_needs_halo(&self.scene.elements, element);
            paint_element(painter, element, &view, faded, self.editing == Some(i), halo);
        }
        if let Some((name, last, at)) = &self.agent {
            let fade = if pen.is_some() { 1.0 } else { 1.0 - ((self.now - at) / AGENT_LINGER).clamp(0.0, 1.0) as f32 };
            if fade > 0.0 {
                // toward the pen or where it stopped, smoothly, the way a hand moves
                let target = pen.map_or(*last, |p| self.to_world(p));
                let at = match self.agent_drawn.get() {
                    Some((from, t)) => from + (target - from) * (1.0 - (-(self.now - t) as f32 * 18.0).exp()),
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
        if let Some(target) = self.suggested_binding().or_else(|| self.connect_hover()).and_then(|i| self.scene.elements.get(i)) {
            // BINDING_HIGHLIGHT_RGB, width clamp(1.75, strokeWidth, 4)
            let outline = binding_outline(target, 0.0).iter().map(|p| self.to_screen(Pos2::new(p[0], p[1]))).collect();
            let width = target.style.stroke_width.clamp(1.75, 4.0);
            let color = if dark { Color32::from_rgb(104, 182, 240) } else { Color32::from_rgb(106, 189, 252) };
            painter.add(Shape::closed_line(outline, Stroke::new(width, color)));
        }
        if let (Some(i), Tool::Select) = (self.single_line(), self.tool) {
            // orbit bindings show their focus point, dashed from the endpoint (FOCUS_POINT_SIZE)
            let arrow = &self.scene.elements[i];
            let points = self.line_points(i);
            for (binding, k) in [(&arrow.ends.start, 0), (&arrow.ends.end, points.len() - 1)] {
                let Some(b) = binding.as_ref().filter(|b| !b.inside) else {
                    continue;
                };
                if let Some(target) = self.scene.elements.iter().find(|e| e.id == b.element_id) {
                    let focus = self.to_screen(focus_point(target, b));
                    let end = self.to_screen(Pos2::new(points[k][0], points[k][1]));
                    let faint = Stroke::new(1.0, PRIMARY.gamma_multiply(0.5));
                    painter.extend(Shape::dashed_line(&[end, focus], faint, 3.0, 3.0));
                    painter.circle_stroke(focus, 10.0 / 3.0, faint);
                }
            }
        }
        if let Some(path) = &self.lasso {
            // ponytail: no 5% fill inside the trail; egui only fills convex polygons
            painter.add(Shape::line(path.iter().map(|p| self.to_screen(*p)).collect(), Stroke::new(2.0, PRIMARY)));
        }
        // laser trail: red, fading out over one second
        for pair in self.laser.windows(2) {
            let age = (self.now - pair[1].1) as f32;
            if age < 1.0 {
                let color = Color32::from_rgb(0xff, 0x3b, 0x30).gamma_multiply(1.0 - age);
                painter.line_segment([self.to_screen(pair[0].0), self.to_screen(pair[1].0)], Stroke::new(4.0 * (1.0 - age * 0.5), color));
            }
        }
        for [a, b] in &self.snap_guides {
            // object snapping lines, red like the reference app's
            let red = Stroke::new(1.0, Color32::from_rgb(0xff, 0x6b, 0x6b));
            painter.line_segment([self.to_screen(*a), self.to_screen(*b)], red);
        }
        let accent = Stroke::new(1.0, PRIMARY);
        let handle_fill = if dark { Color32::from_rgb(18, 18, 18) } else { Color32::WHITE };
        let two_point_line = self.single_line().is_some_and(|i| self.line_points(i).len() == 2);
        let selected: Vec<&Element> = self.selected.iter().filter_map(|i| self.scene.elements.get(*i)).collect();
        let card = |e: &Element| e.is_node() || e.created.is_some();
        // a note's text is grouped with it and goes along
        // and a rope picked along with its windows has no box of its own
        let with_card = |e: &&Element| card(e) || e.rope || selected.iter().any(|c| card(c) && c.group_id.is_some() && c.group_id == e.group_id);
        // the window in focus (a terminal being typed into, or one enlarged, the IDE open) gets no outline
        let focused = self.focused_window().cloned().or_else(|| self.note_focus_id());
        if selected.iter().any(|e| card(e)) && selected.iter().all(with_card) && self.tool == Tool::Select {
            // terminals, portals and notes: only a dashed rounded outline whose dashes crawl
            // (the corners still resize them); drawings keep the straight box with handles
            for e in selected.iter().filter(|e| card(e) && Some(&e.id) != focused.as_ref()) {
                let (rect, zoom) = self.terminal_frame(e);
                painter.extend(crawling_outline(rect.expand(4.0), 11.0 * zoom.min(1.0) + 1.0, self.now));
                // 16 px/s: a step of about one pixel every 62 ms, for dashes on the screen
                if rect.expand(5.0).intersects(painter.clip_rect()) {
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
        // colors and radii from renderSingleLinearPoint (POINT_HANDLE_SIZE = 10)
        let editing_line = self.line_editor.is_some();
        let radius = if editing_line { 10.0 } else { 5.0 };
        let point_stroke = Stroke::new(1.0, themed(Color32::from_rgb(0x5e, 0x5a, 0xd8), dark));
        for (_, p) in self.midpoint_handles() {
            painter.circle_filled(p, 5.0, themed(Color32::from_rgba_unmultiplied(177, 151, 252, 178), dark));
        }
        for (k, p) in self.point_handles() {
            let selected = editing_line && self.selected_point == Some(k);
            let fill = if selected { Color32::from_rgba_unmultiplied(134, 131, 226, 230) } else { Color32::from_rgba_unmultiplied(255, 255, 255, 230) };
            painter.circle(p, radius, themed(fill, dark), point_stroke);
        }
        if let (Some(_), Tool::AutoShape) = (self.drawing_start, self.tool) {
            // faint trail of the stroke plus the shape it would become, at 70% opacity (lines
            // get no preview: nearly every stroke reads as a line at some early point)
            let width = (self.style.stroke_width * 0.65 * self.zoom).clamp(1.25, 4.0);
            let trail = if dark { Color32::from_white_alpha(51) } else { Color32::from_black_alpha(51) };
            painter.add(Shape::line(self.drawing_points.iter().map(|p| self.to_screen(Pos2::new(p[0], p[1]))).collect(), Stroke::new(width, trail)));
            if let Some(mut preview) = recognize_shape(self, &self.drawing_points).filter(|e| !matches!(e.kind, ElementKind::Line { arrow: false, .. })) {
                preview.style.opacity = 70;
                paint_element(painter, &preview, &view, false, false, false);
            }
        } else if let (Some(start), Some(pointer), Tool::Terminal | Tool::Portal | Tool::Ide) = (self.drawing_start, self.last_pointer, self.tool) {
            // a terminal (or portal) is dragged out as a selection-like box, not a drawn shape
            let area = Rect::from_two_pos(self.to_screen(start), pointer);
            painter.rect(area, 8, PRIMARY.gamma_multiply(0.08), Stroke::new(1.0, PRIMARY), StrokeKind::Inside);
        } else if let (Some(start), Some(pointer)) = (self.drawing_start, self.last_pointer) {
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
            Some(e @ Element { kind: ElementKind::Text { x, y, .. }, .. }) => (
                self.to_screen(Pos2::new(*x, *y)),
                e.style.font_size * self.zoom,
                themed(rgba(e.style.stroke), self.dark()),
                element_bounds(e).width() * self.zoom,
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
                    ui.ctx().fonts_mut(|f| f.layout_job(job))
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
                if !ui.memory(|m| m.has_focus(response.id)) {
                    response.request_focus();
                }
                finish = ui.input(|i| i.key_pressed(Key::Escape) || (i.key_pressed(Key::Enter) && i.modifiers.command));
            }
        });
        if finish {
            self.finish_text();
        }
    }

    /// A note's text being typed: the card's text box, the same markdown, scrolling in place.
    fn note_editor(&mut self, ctx: &Context, index: usize, note: usize) {
        let body = note_text_box(&self.scene.elements[note]);
        let (frame, zoom) = self.terminal_frame(&self.scene.elements[note]);
        let at = |p: Pos2| frame.min + (p - local_bounds(&self.scene.elements[note]).min) * zoom;
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
                        ui.ctx().fonts_mut(|f| f.layout_job(job))
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
                    // the click that focuses the note releases on the next frame and counts as a
                    // click elsewhere, which takes the focus back: keep asking for it while editing
                    if want_focus && !ui.memory(|m| m.has_focus(response.id)) {
                        response.request_focus();
                    } else if let Some(r) = egui::TextEdit::load_state(ui.ctx(), response.id).and_then(|s| s.cursor.char_range()) {
                        let (a, b) = (r.primary.index.0, r.secondary.index.0);
                        self.note_selection = Some((index, a.min(b), a.max(b)));
                    }
                    // the modifiers ride on the key event (scripted keys set them only there)
                    finish = ui.input(|i| {
                        i.events.iter().any(|e| {
                            matches!(e, egui::Event::Key { key: Key::Escape, pressed: true, .. })
                                || matches!(e, egui::Event::Key { key: Key::Enter, pressed: true, modifiers, .. } if modifiers.command)
                        })
                    });
                }
            });
        });
        if finish {
            // Esc or Cmd+Enter in a lifted note also puts it back on the canvas (set_elevated ends the typing)
            if self.elevated_index().is_some_and(|i| self.scene.elements[i].created.is_some()) {
                self.set_elevated(None);
            } else {
                self.finish_text();
            }
        }
    }

    /// Shift keeps squares/circles and 15° angles, Alt draws from the center (as in the reference app).
    pub(crate) fn temporary_element(&self, start: Pos2, end: Pos2) -> Element {
        let end = if self.tool == Tool::Draw { end } else { self.snap_point(end) };
        let mut d = end - start;
        let boxed = matches!(self.tool, Tool::Rectangle | Tool::Ellipse | Tool::Diamond | Tool::Frame | Tool::Sticky | Tool::Terminal | Tool::Portal);
        // sticky notes are square by default and Shift frees them, like images on the web
        if (self.mods.shift != (self.tool == Tool::Sticky)) && boxed {
            let side = d.x.abs().max(d.y.abs());
            d = Vec2::new(side * d.x.signum(), side * d.y.signum());
        }
        if self.mods.shift && matches!(self.tool, Tool::Arrow | Tool::Line) {
            let step = std::f32::consts::PI / 12.0;
            let angle = (d.y.atan2(d.x) / step).round() * step;
            d = Vec2::angled(angle) * d.length();
        }
        let (a, b) = if self.mods.alt && boxed { (start - d, start + d) } else { (start, start + d) };
        let r = Rect::from_two_pos(a, b);
        let (x, y, width, height) = (r.min.x, r.min.y, r.width().max(1.0), r.height().max(1.0));
        let kind = match self.tool {
            Tool::Draw | Tool::AutoShape => ElementKind::FreeDraw { points: self.drawing_points.clone() },
            Tool::Rectangle | Tool::Frame | Tool::Sticky | Tool::Terminal | Tool::Portal => ElementKind::Rectangle { x, y, width, height },
            Tool::Ellipse => ElementKind::Ellipse { x, y, width, height },
            Tool::Diamond => ElementKind::Diamond { x, y, width, height },
            tool => ElementKind::Line { points: vec![[a.x, a.y], [b.x, b.y]], arrow: tool == Tool::Arrow },
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

    pub(crate) fn update_cursor(&self, ctx: &Context, response: &egui::Response) {
        let Some(pos) = response.hover_pos() else {
            return;
        };
        if ctx.layer_id_at(pos) != Some(response.layer_id) {
            return;
        }
        let icon = if self.panning {
            CursorIcon::Grabbing
        } else if self.space_pan(ctx) || self.tool == Tool::Hand || self.prefs.view_mode {
            CursorIcon::Grab
        } else if self.connecting.is_some() {
            CursorIcon::Crosshair
        } else if self.moving {
            CursorIcon::Move
        } else {
            match self.tool {
                Tool::Select => self.select_cursor(pos),
                Tool::Text => CursorIcon::Text,
                _ => CursorIcon::Crosshair,
            }
        };
        ctx.set_cursor_icon(icon);
    }

    /// The selection tool's cursor says what a press there does, checked in press_select's order:
    /// Move only where the press picks something up.
    pub(crate) fn select_cursor(&self, pos: Pos2) -> CursorIcon {
        let world = self.to_world(pos);
        if self.point_handles().iter().chain(&self.midpoint_handles()).any(|(_, p)| p.distance(pos) <= 10.0) {
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
        if self.elevated_index().is_some_and(|i| self.terminal_frame(&self.scene.elements[i]).0.contains(pos)) {
            return CursorIcon::Default;
        }
        match self.hit(world) {
            Some(i) if self.scene.elements[i].rope => CursorIcon::PointingHand,
            Some(i) if self.drags(i, world, self.mods.shift) => CursorIcon::Move,
            // a body that types: the terminal and a note's text
            Some(i)
                if self.scene.elements[i].terminal.is_some()
                    || (self.scene.elements[i].created.is_some() && self.mods.is_none() && self.on_note_text(i, world)) =>
            {
                CursorIcon::Text
            }
            Some(_) => CursorIcon::Default,
            None if self.selection_bounds().is_some_and(|b| b.contains(world)) => CursorIcon::Move,
            None => CursorIcon::Default,
        }
    }
}

/// A dashed rounded outline whose dashes crawl along it: the selection of a live window.
fn crawling_outline(rect: Rect, radius: f32, time: f64) -> Vec<Shape> {
    use std::f32::consts::{FRAC_PI_2, PI};
    let r = radius.min(rect.width() / 2.0).min(rect.height() / 2.0);
    let corners = [
        (rect.right_top() + Vec2::new(-r, r), -FRAC_PI_2),
        (rect.right_bottom() + Vec2::new(-r, -r), 0.0),
        (rect.left_bottom() + Vec2::new(r, -r), FRAC_PI_2),
        (rect.left_top() + Vec2::new(r, r), PI),
    ];
    let mut path: Vec<Pos2> = corners.iter().flat_map(|(c, start)| (0..=6).map(move |k| *c + Vec2::angled(start + k as f32 / 6.0 * FRAC_PI_2) * r)).collect();
    path.push(path[0]);
    let (dash, gap) = (6.0, 4.0);
    let offset = (time * 16.0) as f32 % (dash + gap);
    Shape::dashed_line_with_offset(&path, Stroke::new(1.5, PRIMARY), &[dash], &[gap], offset)
}
