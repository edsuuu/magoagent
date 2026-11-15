//! Arrows bound to shapes: what an end binds to, the heads, and keeping them glued.

use crate::prelude::*;

impl DrawingApp {
    /// maxBindingDistance_simple: 15px, a bit more when zoomed out (in world units).
    pub(crate) fn binding_reach(&self) -> f32 {
        (15.0 / (self.zoom.min(1.0) * 1.5)).clamp(15.0, 30.0) / self.zoom
    }

    /// Bindable shape under `world` (inside it or within reach of its outline): an arrow always
    /// binds, Cmd held is the one way out.
    pub(crate) fn binding_target(&self, world: Pos2, exclude: Option<usize>) -> Option<usize> {
        if self.mods.command {
            return None;
        }
        let reach = self.binding_reach();
        let point = [world.x, world.y];
        self.scene.elements.iter().enumerate().rev().find_map(|(index, element)| {
            if Some(index) == exclude || !bindable(element) {
                return None;
            }
            let poly = binding_outline(element, 0.0);
            let count = poly.len();
            let near = (0..count).any(|side| distance_to_segment(point, poly[side], poly[(side + 1) % count]) <= reach);
            (near || polygon_contains(&poly, point)).then_some(index)
        })
    }

    /// Deep inside a shape binds "inside" (to that exact point), near its outline "orbit"
    /// (stops at the outline); Option forces "inside" — as observed in the reference app.
    pub(crate) fn bind_at(&self, world: Pos2, exclude: Option<usize>) -> Option<Binding> {
        let target = &self.scene.elements[self.binding_target(world, exclude)?];
        let poly = binding_outline(target, 0.0);
        let point = [world.x, world.y];
        let count = poly.len();
        let edge = (0..count).map(|side| distance_to_segment(point, poly[side], poly[(side + 1) % count])).fold(f32::MAX, f32::min);
        let deep = polygon_contains(&poly, point) && edge > self.binding_reach();
        let mut binding = binding_for(target, world, self.mods.alt || deep);
        if !binding.inside {
            // near the middle of a side, it locks onto that middle
            let mids = [[0.5, 0.0], [1.0, 0.5], [0.5, 1.0], [0.0, 0.5]];
            if let Some(mid) = mids.into_iter().find(|middle| {
                let probe = Binding { fixed: *middle, ..binding.clone() };
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
        if let (Some(start), Some(end)) = (&mut arrow.ends.start, &mut arrow.ends.end)
            && start.element_id == end.element_id
        {
            // both ends in one shape: the reference app keeps them where they were drawn
            (start.inside, end.inside) = (true, true);
        }
        resolve_arrow(&self.scene.elements, &mut arrow);
        arrow
    }

    pub(crate) fn set_head(&mut self, end: bool, head: Option<Head>) {
        let arrows: Vec<usize> = self
            .selected
            .iter()
            .copied()
            .filter(|index| matches!(self.scene.elements.get(*index), Some(Element { kind: ElementKind::Line { arrow: true, .. }, .. })))
            .collect();
        if !arrows.is_empty() {
            self.snapshot();
        }
        for index in arrows {
            let ends = &mut self.scene.elements[index].ends;
            *(if end { &mut ends.end_head } else { &mut ends.start_head }) = head;
        }
        *(if end { &mut self.heads.1 } else { &mut self.heads.0 }) = head;
    }

    /// Moving/rotating/resizing an arrow without its target breaks that binding. A rope never
    /// lets go: its ends follow the windows it ties.
    pub(crate) fn detach_selected_arrows(&mut self) {
        let selected_ids: Vec<String> = self.selected.iter().filter_map(|index| self.scene.elements.get(*index)).map(|element| element.id.clone()).collect();
        for index in self.selected.clone() {
            if let Some(element) = self.scene.elements.get_mut(index).filter(|element| !element.rope) {
                for end in [&mut element.ends.start, &mut element.ends.end] {
                    if end.as_ref().is_some_and(|binding| !selected_ids.contains(&binding.element_id)) {
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
            (Some(point_index), Some(index))
                if matches!(self.scene.elements[index].kind, ElementKind::Line { arrow: true, .. })
                    && (point_index == 0 || point_index + 1 == self.line_points(index).len()) =>
            {
                Some(index)
            }
            _ => None,
        };
        let drawing = self.tool == Tool::Arrow && self.drawing_start.is_some();
        if let Some(index) = self.multi.or(dragged_end) {
            return self.binding_target(pointer, Some(index));
        }
        if drawing && self.canvas_rect.contains(self.last_pointer?) {
            return self.binding_target(pointer, None);
        }
        None
    }
}
