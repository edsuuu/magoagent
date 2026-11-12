use super::*;

impl DrawingApp {
    pub(crate) fn zoom_at(&mut self, zoom: f32, anchor: Pos2) {
        let world = self.to_world(anchor);
        self.glide = None;
        self.zoom = zoom.clamp(0.1, 30.0);
        self.pan = anchor - self.canvas_rect.min - world.to_vec2() * self.zoom;
    }

    /// − / + and their shortcuts: the next step down or up (only the sign of `delta` counts),
    /// from the middle of the screen.
    pub(crate) fn zoom_step(&mut self, delta: f32) {
        let z = self.zoom;
        let next = if delta > 0.0 { ZOOM_LEVELS.iter().find(|l| **l > z + 1e-3) } else { ZOOM_LEVELS.iter().rev().find(|l| **l < z - 1e-3) };
        self.zoom_at(next.copied().unwrap_or(z), self.canvas_rect.center());
    }

    /// The wheel or a pinch asks for `factor`; one frame moves at most 0.4 (up to 300%, then the
    /// same share of the zoom), so a fast spin doesn't jump.
    pub(crate) fn wheel_zoom(&mut self, factor: f32, anchor: Pos2) {
        let cap = 0.4 * (self.zoom / 3.0).max(1.0);
        self.zoom_at(self.zoom + (self.zoom * factor - self.zoom).clamp(-cap, cap), anchor);
    }

    /// The wheel under Ctrl, as in the reference app: each notch adds a step (`Prefs.zoom_step`, 10 points)
    /// at the pointer (100% → 110% → 120%), the same share of the zoom past 300%, and one frame
    /// moves at most four steps.
    pub(crate) fn wheel_step(&mut self, notches: f32, anchor: Pos2) {
        let share = (self.zoom / 3.0).max(1.0);
        self.zoom_at(self.zoom + notches.clamp(-4.0, 4.0) * f32::from(self.prefs.zoom_step) / 100.0 * share, anchor);
    }

    /// Grid snap for points (drawing), when the grid is on.
    pub(crate) fn snap_point(&self, p: Pos2) -> Pos2 {
        if self.prefs.grid && !self.mods.command { Pos2::new((p.x / GRID).round() * GRID, (p.y / GRID).round() * GRID) } else { p }
    }

    /// Offset that snaps a moved box to the grid, plus the guide lines to show. Cmd disables
    /// snapping; on a window it tiles it against the others instead (`tile_box`).
    pub(crate) fn snap_box(&self, moved: Rect) -> (Vec2, Vec<[Pos2; 2]>) {
        if self.mods.command {
            let moving = self.move_origin.as_ref().map_or(&self.selected, |(moving, ..)| moving);
            let window = |i: &usize| self.scene.elements.get(*i).is_some_and(|e| e.is_node() || e.created.is_some());
            return if moving.iter().any(window) { self.tile_box(moved) } else { (Vec2::ZERO, Vec::new()) };
        }
        if self.prefs.grid {
            return (self.snap_point(moved.min) - moved.min, Vec::new());
        }
        (Vec2::ZERO, Vec::new())
    }

    /// Magnetic tiles (Cmd/Ctrl while a window is dragged): on each axis the box goes to the nearest
    /// place among the windows and notes beside it, within 48 px of screen: one grid step after or
    /// before one, its walls in line with one's, or centered in a gap between two. No fixed grid.
    pub(crate) fn tile_box(&self, moved: Rect) -> (Vec2, Vec<[Pos2; 2]>) {
        let reach = 48.0 / self.zoom;
        let moving = self.move_origin.as_ref().map_or(&self.selected, |(moving, ..)| moving);
        let others: Vec<Rect> = (self.scene.elements.iter().enumerate())
            .filter(|(i, e)| !moving.contains(i) && !e.rope && (e.is_node() || e.created.is_some()))
            .map(|(_, e)| element_bounds(e))
            .collect();
        let (mut adjust, mut guides) = (Vec2::ZERO, Vec::new());
        for axis in 0..2 {
            let size = moved.size()[axis];
            // the ones beside it on this axis: overlapping it across, give or take the reach
            let across = 1 - axis;
            let beside: Vec<&Rect> = others.iter().filter(|o| o.min[across] < moved.max[across] + reach && o.max[across] > moved.min[across] - reach).collect();
            // (where its min goes, the wall it lines up with, the one that wall belongs to)
            let mut places: Vec<(f32, f32, &Rect)> = Vec::new();
            for o in &beside {
                places.extend([
                    (o.max[axis] + GRID, o.max[axis], *o),
                    (o.min[axis] - GRID - size, o.min[axis], *o),
                    (o.min[axis], o.min[axis], *o),
                    (o.max[axis] - size, o.max[axis], *o),
                ]);
                for b in &beside {
                    let room = b.min[axis] - o.max[axis];
                    if room >= size && (o.max[axis]..b.min[axis]).contains(&moved.center()[axis]) {
                        places.push((o.max[axis] + (room - size) / 2.0, o.max[axis], *o));
                    }
                }
            }
            let best = places
                .into_iter()
                .filter(|(at, ..)| (at - moved.min[axis]).abs() <= reach)
                .min_by(|a, b| (a.0 - moved.min[axis]).abs().total_cmp(&(b.0 - moved.min[axis]).abs()));
            if let Some((at, wall, o)) = best {
                adjust[axis] = at - moved.min[axis];
                let span = moved.translate(adjust).union(*o);
                guides.push(if axis == 0 {
                    [Pos2::new(wall, span.min.y), Pos2::new(wall, span.max.y)]
                } else {
                    [Pos2::new(span.min.x, wall), Pos2::new(span.max.x, wall)]
                });
            }
        }
        (adjust, guides)
    }

    /// Centers the canvas on a world point at once (the minimap, which follows the pointer).
    pub(crate) fn pan_to(&mut self, world: Pos2) {
        self.glide = None;
        self.pan = self.canvas_rect.size() / 2.0 - world.to_vec2() * self.zoom;
    }

    /// Centers the canvas on a world point, gliding there in 0.35 s. A pan or a zoom on the way
    /// stops it where it is.
    pub(crate) fn glide_to(&mut self, world: Pos2) {
        let from = self.pan;
        self.pan_to(world);
        if from != self.pan {
            self.glide = Some((self.now, from, self.pan));
            self.pan = from;
        }
    }

    // ponytail: an ease-out cubic in place of the reference's spring without bounce
    pub(crate) fn step_glide(&mut self) {
        let Some((since, from, to)) = self.glide else { return };
        let t = ((self.now - since) / 0.35).min(1.0) as f32;
        self.pan = from + (to - from) * (1.0 - (1.0 - t).powi(3));
        if t >= 1.0 {
            self.glide = None;
        }
    }

    pub(crate) fn zoom_to_fit(&mut self, selection: bool) {
        let bounds = if selection {
            self.selection_bounds()
        } else {
            self.scene.elements.iter().filter(|e| note_of_label(&self.scene.elements, e).is_none()).map(element_bounds).reduce(|a, b| a.union(b))
        };
        let Some(bounds) = bounds else { return };
        self.fit_view(bounds, None);
        if selection {
            // a selection may zoom past 100%
            let view = self.canvas_rect.size() - Vec2::splat(120.0);
            self.fit_view(bounds, Some((view.x / bounds.width().max(1.0)).min(view.y / bounds.height().max(1.0)).clamp(0.1, 30.0)));
        }
    }

    /// The canvas centered on `bounds` at `zoom`, or zoomed to fit it (100% at most).
    pub(crate) fn fit_view(&mut self, bounds: Rect, zoom: Option<f32>) {
        if !self.canvas_rect.is_positive() {
            return;
        }
        let view = self.canvas_rect.size() - Vec2::splat(120.0);
        self.zoom = zoom.unwrap_or_else(|| (view.x / bounds.width().max(1.0)).min(view.y / bounds.height().max(1.0)).clamp(0.1, 1.0));
        self.glide = None;
        self.pan = self.canvas_rect.size() / 2.0 - bounds.center().to_vec2() * self.zoom;
    }
}
