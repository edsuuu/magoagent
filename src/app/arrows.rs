//! Arrows bound to shapes: what an end binds to, the heads, and keeping them glued.

use crate::prelude::*;

impl DrawingApp {
    /// maxBindingDistance_simple: 15px, a bit more when zoomed out (in world units).
    pub(crate) fn binding_reach(&self) -> f32 {
        (15.0 / (self.zoom.min(1.0) * 1.5)).clamp(15.0, 30.0) / self.zoom
    }

    /// Bindable shape under `world` (inside it or within reach of its outline). Cmd disables binding.
    pub(crate) fn binding_target(&self, world: Pos2, exclude: Option<usize>) -> Option<usize> {
        if self.mods.command || !self.prefs.arrow_binding {
            return None;
        }
        let reach = self.binding_reach();
        let p = [world.x, world.y];
        self.scene.elements.iter().enumerate().rev().find_map(|(i, e)| {
            if Some(i) == exclude || !bindable(e) {
                return None;
            }
            let poly = binding_outline(e, 0.0);
            let n = poly.len();
            let near = (0..n).any(|k| distance_to_segment(p, poly[k], poly[(k + 1) % n]) <= reach);
            (near || polygon_contains(&poly, p)).then_some(i)
        })
    }

    /// Deep inside a shape binds "inside" (to that exact point), near its outline "orbit"
    /// (stops at the outline); Option forces "inside" — as observed in the reference app.
    pub(crate) fn bind_at(&self, world: Pos2, exclude: Option<usize>) -> Option<Binding> {
        let target = &self.scene.elements[self.binding_target(world, exclude)?];
        let poly = binding_outline(target, 0.0);
        let p = [world.x, world.y];
        let n = poly.len();
        let edge = (0..n).map(|k| distance_to_segment(p, poly[k], poly[(k + 1) % n])).fold(f32::MAX, f32::min);
        let deep = polygon_contains(&poly, p) && edge > self.binding_reach();
        let mut binding = binding_for(target, world, self.mods.alt || deep);
        if self.prefs.midpoint_snapping && !binding.inside {
            // Encaixar em pontos intermediários: lock onto the middle of the nearest side
            let mids = [[0.5, 0.0], [1.0, 0.5], [0.5, 1.0], [0.0, 0.5]];
            if let Some(mid) = mids.into_iter().find(|m| {
                let probe = Binding { fixed: *m, ..binding.clone() };
                focus_point(target, &probe).distance(world) <= self.binding_reach()
            }) {
                binding.fixed = mid;
            }
        }
        Some(binding)
    }

    /// Binds a freshly drawn arrow to the shapes under its first and last point.
    pub(crate) fn bind_new_arrow(&self, mut arrow: Element) -> Element {
        let ElementKind::Line { points, arrow: true } = &arrow.kind else {
            return arrow;
        };
        let (first, last) = (points[0], points[points.len() - 1]);
        arrow.ends.start = self.bind_at(Pos2::new(first[0], first[1]), None);
        arrow.ends.end = self.bind_at(Pos2::new(last[0], last[1]), None);
        if let (Some(a), Some(b)) = (&mut arrow.ends.start, &mut arrow.ends.end) {
            if a.element_id == b.element_id {
                // both ends in one shape: the reference app keeps them where they were drawn
                (a.inside, b.inside) = (true, true);
            }
        }
        resolve_arrow(&self.scene.elements, &mut arrow);
        arrow
    }

    pub(crate) fn set_head(&mut self, end: bool, head: Option<Head>) {
        let arrows: Vec<usize> = self
            .selected
            .iter()
            .copied()
            .filter(|i| matches!(self.scene.elements.get(*i), Some(Element { kind: ElementKind::Line { arrow: true, .. }, .. })))
            .collect();
        if !arrows.is_empty() {
            self.snapshot();
        }
        for i in arrows {
            let ends = &mut self.scene.elements[i].ends;
            *(if end { &mut ends.end_head } else { &mut ends.start_head }) = head;
        }
        *(if end { &mut self.heads.1 } else { &mut self.heads.0 }) = head;
    }

    /// Moving/rotating/resizing an arrow without its target breaks that binding. A rope never
    /// lets go: its ends follow the windows it ties.
    pub(crate) fn detach_selected_arrows(&mut self) {
        let selected_ids: Vec<String> = self.selected.iter().filter_map(|i| self.scene.elements.get(*i)).map(|e| e.id.clone()).collect();
        for i in self.selected.clone() {
            if let Some(e) = self.scene.elements.get_mut(i).filter(|e| !e.rope) {
                for end in [&mut e.ends.start, &mut e.ends.end] {
                    if end.as_ref().is_some_and(|b| !selected_ids.contains(&b.element_id)) {
                        *end = None;
                    }
                }
            }
        }
    }

    /// Keeps every bound arrow glued to its shapes after anything moved.
    pub(crate) fn update_bindings(&mut self) {
        resolve_bindings(&mut self.scene.elements);
        recenter_labels(&mut self.scene.elements);
    }

    /// Shape that would be bound right now: highlighted like the reference app's suggestedBinding.
    pub(crate) fn suggested_binding(&self) -> Option<usize> {
        let pointer = self.to_world(self.last_pointer?);
        let dragged_end = match (self.point_drag, self.single_line()) {
            (Some(k), Some(i))
                if matches!(self.scene.elements[i].kind, ElementKind::Line { arrow: true, .. }) && (k == 0 || k + 1 == self.line_points(i).len()) =>
            {
                Some(i)
            }
            _ => None,
        };
        let drawing = self.tool == Tool::Arrow && self.drawing_start.is_some();
        if let Some(i) = self.multi.or(dragged_end) {
            return self.binding_target(pointer, Some(i));
        }
        if drawing && self.canvas_rect.contains(self.last_pointer?) {
            return self.binding_target(pointer, None);
        }
        None
    }
}
