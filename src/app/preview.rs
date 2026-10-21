//! What the pointer shows before anything is made: the shape being dragged out and the cursor.

use crate::prelude::*;

impl DrawingApp {
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
            Tool::Sticky => ElementKind::Rectangle { x, y, width, height },
            Tool::Rectangle | Tool::Frame | Tool::Terminal | Tool::Portal => ElementKind::Rectangle { x, y, width, height },
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
        let space = !ctx.egui_wants_keyboard_input() && ctx.input(|i| i.key_down(Key::Space));
        let icon = if self.panning {
            CursorIcon::Grabbing
        } else if space || self.tool == Tool::Hand || self.prefs.view_mode {
            CursorIcon::Grab
        } else {
            match self.tool {
                Tool::Select if self.hovered_rotation(pos) => CursorIcon::Grab,
                Tool::Select if self.point_handles().iter().chain(&self.midpoint_handles()).any(|(_, p)| p.distance(pos) <= 10.0) => {
                    CursorIcon::PointingHand
                }
                Tool::Select => match self.hovered_handle(pos) {
                    Some(0 | 2) => CursorIcon::ResizeNwSe,
                    Some(_) => CursorIcon::ResizeNeSw,
                    None if self.hit(self.to_world(pos)).is_some()
                        || self.selection_bounds().is_some_and(|b| b.contains(self.to_world(pos))) =>
                    {
                        CursorIcon::Move
                    }
                    None => CursorIcon::Default,
                },
                Tool::Text => CursorIcon::Text,
                _ => CursorIcon::Crosshair,
            }
        };
        ctx.set_cursor_icon(icon);
    }
}
