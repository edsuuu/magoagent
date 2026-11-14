//! The usage rings and bars as the reference app draws them: a ring sweeps in from empty the first
//! time it shows (after 120 ms, 1.1 s on cubic-bezier(0.2, 0.9, 0.25, 1)) and springs to each new value
//! after; its arc goes from the color at 70% to the full color, with round ends, and past 100% it laps
//! itself with a shadow under its head. A bar's segments are shaded top to bottom.

use crate::app::floors::fx::Spring;
use crate::prelude::*;
use egui::epaint::Mesh;
use std::f32::consts::{FRAC_PI_2, PI, TAU};

const DELAY: f64 = 0.12;
const SWEEP: f64 = 1.1;
/// A later change: Motion's spring of response 0.6 and damping 0.85.
const UPDATE: (f32, f32) = (0.6, 0.85);

/// CSS's cubic-bezier(x1, y1, x2, y2) at `x`.
pub(crate) fn cubic_bezier(x1: f32, y1: f32, x2: f32, y2: f32, x: f32) -> f32 {
    let at = |start_control: f32, end_control: f32, progress: f32| {
        3.0 * (1.0 - progress) * (1.0 - progress) * progress * start_control
            + 3.0 * (1.0 - progress) * progress * progress * end_control
            + progress * progress * progress
    };
    if x <= 0.0 || x >= 1.0 {
        return x.clamp(0.0, 1.0);
    }
    let (mut lowest, mut highest) = (0.0, 1.0);
    for _ in 0..30 {
        let mid = (lowest + highest) / 2.0;
        if at(x1, x2, mid) < x {
            lowest = mid;
        } else {
            highest = mid;
        }
    }
    at(y1, y2, (lowest + highest) / 2.0)
}

/// One ring's fill as it moves.
pub(crate) struct Sweep {
    born: f64,
    target: f32,
    spring: Option<Spring>,
}

impl Sweep {
    pub(crate) fn new(target: f32, now: f64) -> Self {
        Self { born: now, target, spring: None }
    }

    pub(crate) fn value(&self, now: f64) -> f32 {
        match &self.spring {
            Some(spring) => spring.value(now),
            None => self.target * cubic_bezier(0.2, 0.9, 0.25, 1.0, ((now - self.born - DELAY) / SWEEP).clamp(0.0, 1.0) as f32),
        }
    }

    /// Goes to `target` on the spring, from wherever it is.
    pub(crate) fn aim(&mut self, target: f32, now: f64) {
        if (target - self.target).abs() < 1e-6 {
            return;
        }
        let at = self.value(now);
        self.spring.get_or_insert(Spring::new(at, UPDATE)).go(target, now, 0.0);
        self.target = target;
    }

    pub(crate) fn moving(&self, now: f64) -> bool {
        match &self.spring {
            Some(spring) => !spring.done(now),
            None => now < self.born + DELAY + SWEEP,
        }
    }
}

/// `a` to `b` by `t`, premultiplied, channel by channel.
fn mix(first: Color32, second: Color32, amount: f32) -> Color32 {
    let channel = |x: u8, y: u8| (x as f32 + (y as f32 - x as f32) * amount).round() as u8;
    Color32::from_rgba_premultiplied(
        channel(first.r(), second.r()),
        channel(first.g(), second.g()),
        channel(first.b(), second.b()),
        channel(first.a(), second.a()),
    )
}

/// The arc of the ring of `radius` from angle `from` over `span` (radians, clockwise on screen), `line`
/// thick, `c0` to `c1` along it; its sides fade over a pixel, as egui's own strokes do.
fn arc_mesh(center: Pos2, radius: f32, line: f32, from: f32, span: f32, (start_color, end_color): (Color32, Color32), feather: f32) -> Mesh {
    let steps = ((span * radius / 1.5).ceil() as usize).clamp(4, 512);
    let (inner, outer) = (radius - line / 2.0, radius + line / 2.0);
    let mut mesh = Mesh::default();
    for index in 0..=steps {
        let progress = index as f32 / steps as f32;
        let direction = Vec2::angled(from + span * progress);
        let color = mix(start_color, end_color, progress);
        for (offset_radius, vertex_color) in [
            (inner - feather / 2.0, Color32::TRANSPARENT),
            (inner + feather / 2.0, color),
            (outer - feather / 2.0, color),
            (outer + feather / 2.0, Color32::TRANSPARENT),
        ] {
            mesh.colored_vertex(center + direction * offset_radius, vertex_color);
        }
        if index > 0 {
            let base = (index as u32 - 1) * 4;
            for corner in 0..3 {
                mesh.add_triangle(base + corner, base + corner + 1, base + 4 + corner);
                mesh.add_triangle(base + corner + 1, base + 5 + corner, base + 4 + corner);
            }
        }
    }
    mesh
}

/// A ring `size` across, `line` thick, filled to `value` (1 is full; past it the arc laps itself), all of
/// it at `opacity`: the track at 20%, the arc, the start's cap at 70% and the head's. Over a known `ground`
/// the faded ring is mixed into it, opaque, as the reference fades the ring as one: see-through, its caps
/// over the arc would show as lighter dots.
pub(crate) fn paint_ring(painter: &Painter, center: Pos2, size: f32, line: f32, color: Color32, value: f32, opacity: f32, ground: Option<Color32>) {
    let shade = |share: f32| match ground {
        Some(ground) => ground.lerp_to_gamma(color, share * opacity),
        None => color.gamma_multiply(share * opacity),
    };
    let radius = (size - line) / 2.0;
    let feather = 1.0 / painter.pixels_per_point();
    // the track on the same mesh as the arc: egui's circle stroke sits outside the radius, not on it
    let track = shade(0.2);
    painter.add(arc_mesh(center, radius, line, 0.0, TAU, (track, track), feather));
    if value <= 0.0 {
        return;
    }
    let start = shade(0.7);
    let color = shade(1.0);
    let turn = if value >= 1.0 { value.fract() } else { 0.0 };
    let from = -FRAC_PI_2 + turn * TAU;
    painter.add(arc_mesh(center, radius, line, from, value.min(1.0) * TAU, (start, color), feather));
    let on_ring = |angle: f32| center + Vec2::angled(angle) * radius;
    if value < 1.0 {
        painter.circle_filled(on_ring(from), line / 2.0, start);
    }
    let head = -FRAC_PI_2 + value.fract() * TAU;
    if value >= 1.0 {
        // the head over its own tail: a shadow a fifth of the line ahead of it
        let ahead = Vec2::new(-head.sin(), head.cos()) * line * 0.2;
        painter.circle_filled(on_ring(head) + ahead, line / 2.0 + line * 0.125, Color32::from_black_alpha((0.3 * 0.6 * 255.0 * opacity) as u8));
    }
    painter.circle_filled(on_ring(head), line / 2.0, color);
}

/// 36 segments across `rect`, the used share lit (at least one while anything is used): lit ones from
/// the color mixed 70% with white at the top to the color, the others from 26% to 14% of it.
pub(crate) fn segment_bar(painter: &Painter, rect: Rect, used: f64, color: Color32) {
    const SEGMENTS: usize = 36;
    let gap = (rect.width() / 100.0).floor().max(1.0);
    let width = (rect.width() - gap * (SEGMENTS - 1) as f32) / SEGMENTS as f32;
    let lit = if used > 0.0 { ((used.min(1.0) * SEGMENTS as f64).round() as usize).max(1) } else { 0 };
    let light = mix(color, Color32::WHITE, 0.3);
    for index in 0..SEGMENTS {
        let cell = Rect::from_min_size(rect.min + Vec2::new(index as f32 * (width + gap), 0.0), Vec2::new(width, rect.height()));
        // the bottom's color with egui's soft edge, and over it, a hair inside, the top's fading down
        let (bottom, top) = if index < lit { (color, light) } else { (color.gamma_multiply(0.14), color.gamma_multiply(0.14)) };
        painter.rect_filled(cell, width / 2.0, bottom);
        painter.add(fade_down(cell.shrink(0.5), top));
    }
}

/// A stadium standing in `rect`, `top` at its top and nothing at its bottom.
fn fade_down(rect: Rect, top: Color32) -> Mesh {
    let radius = rect.width() / 2.0;
    let (up, down) = (Pos2::new(rect.center().x, rect.top() + radius), Pos2::new(rect.center().x, rect.bottom() - radius));
    let arc = |arc_center: Pos2, from: f32| (0..=8).map(move |step| arc_center + Vec2::angled(from + PI * step as f32 / 8.0) * radius);
    let points: Vec<Pos2> = arc(up, PI).chain(arc(down, 0.0)).collect();
    let shade = |point: Pos2| mix(top, Color32::TRANSPARENT, ((point.y - rect.top()) / rect.height()).clamp(0.0, 1.0));
    let mut mesh = Mesh::default();
    mesh.colored_vertex(rect.center(), shade(rect.center()));
    for point in &points {
        mesh.colored_vertex(*point, shade(*point));
    }
    let count = points.len() as u32;
    for index in 0..count {
        mesh.add_triangle(0, 1 + index, 1 + (index + 1) % count);
    }
    mesh
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_ring_sweeps_in_then_springs() {
        assert!((cubic_bezier(0.2, 0.9, 0.25, 1.0, 0.0)).abs() < 1e-3 && (cubic_bezier(0.2, 0.9, 0.25, 1.0, 1.0) - 1.0).abs() < 1e-3);
        assert!(cubic_bezier(0.2, 0.9, 0.25, 1.0, 0.3) > 0.7, "fast at first");
        let mut sweep = Sweep::new(0.5, 10.0);
        assert_eq!(sweep.value(10.1), 0.0, "it waits 120 ms");
        assert!(sweep.value(10.5) > 0.2 && sweep.value(10.5) < 0.5 && sweep.moving(10.5));
        assert!((sweep.value(11.3) - 0.5).abs() < 1e-6 && !sweep.moving(11.3));
        sweep.aim(0.8, 11.3);
        assert!(sweep.moving(11.4) && sweep.value(11.4) > 0.5 && sweep.value(11.4) < 0.8);
        assert!((sweep.value(14.0) - 0.8).abs() < 0.01);
    }
}
