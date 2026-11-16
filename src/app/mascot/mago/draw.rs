//! Drawing Lume with the painter: a magic flame in one colour that licks and sways, with two dark eyes on its round
//! bottom. The figure lives in a box of ~1 unit and is mapped to the
//! screen with the squash, tilt and lift of the moment.

use super::dance::{ARM_LENGTH, ARM_WIDTH, Pose};
use super::motion::{BODY_HALF, FLAME_TIP, Particle, ParticleKind};
use super::{EyeKind, MagoMotion};
use crate::prelude::*;

/// Lume's blue.
pub(crate) const LUME: Color32 = Color32::from_rgb(0x38, 0xbd, 0xf8);
const GOLD: Color32 = Color32::from_rgb(0xff, 0xca, 0x4a);
const INK: Color32 = Color32::from_rgb(0x17, 0x14, 0x2b);
/// The eyes of a mini one, on its agent's colour.
const MINI_INK: Color32 = Color32::from_rgb(0x10, 0x13, 0x1a);
/// The eye, a pill: its width and height in face radii; a mini one's are this much bigger and this much farther apart.
const EYE_SIZE: (f32, f32) = (0.3, 0.34);
const MINI_EYES: f32 = 1.8;
const MINI_SPREAD: f32 = 1.3;
const AMBER: Color32 = Color32::from_rgb(0xff, 0xb0, 0x20);
const SWEAT: Color32 = Color32::from_rgb(0x8f, 0xd3, 0xff);

/// The squash and tilt pivot: the flame's bottom.
const PIVOT_Y: f32 = 0.47;
/// The figure's middle (from the tip to the bottom) and its height in `size`, with room for the sparks; a mini one fills
/// its box.
const MIDDLE: f32 = (FLAME_TIP + PIVOT_Y) / 2.0;
const FIT: f32 = 1.05;
const MINI_FIT: f32 = 1.08;

/// The flame as drawn in the design, 100 units across and y down, its bottom middle at (50, 93): from the tip down the
/// right side, round the bottom, up the left to the small tongue, into the notch and back up to the tip.
const FLAME_START: (f32, f32) = (50.0, 6.0);
const FLAME: [[(f32, f32); 3]; 6] = [
    [(60.0, 24.0), (80.0, 38.0), (80.0, 62.0)],
    [(80.0, 80.0), (67.0, 93.0), (50.0, 93.0)],
    [(33.0, 93.0), (20.0, 80.0), (20.0, 62.0)],
    [(20.0, 46.0), (31.0, 36.0), (38.0, 24.0)],
    [(41.0, 33.0), (45.0, 38.0), (50.0, 40.0)],
    [(49.0, 29.0), (47.0, 17.0), (50.0, 6.0)],
];
/// Points per curve.
const STEPS: usize = 8;

/// A design point in figure units: its bottom middle lands on the pivot.
fn figure((x, y): (f32, f32)) -> (f32, f32) {
    ((x - 50.0) / 100.0, (y - 93.0) / 100.0 + PIVOT_Y)
}

fn cubic(from: (f32, f32), [first, second, to]: [(f32, f32); 3], t: f32) -> (f32, f32) {
    let rest = 1.0 - t;
    let (a, b, c, d) = (rest * rest * rest, 3.0 * rest * rest * t, 3.0 * rest * t * t, t * t * t);
    (a * from.0 + b * first.0 + c * second.0 + d * to.0, a * from.1 + b * first.1 + c * second.1 + d * to.1)
}

/// The points along `curves` from `start`, in figure units; the last one is left out when it closes on the first.
fn trace(start: (f32, f32), curves: &[[(f32, f32); 3]]) -> Vec<(f32, f32)> {
    let mut points = vec![figure(start)];
    let mut from = start;
    for curve in curves {
        points.extend((1..=STEPS).map(|step| figure(cubic(from, *curve, step as f32 / STEPS as f32))));
        from = curve[2];
    }
    if from == start {
        points.pop();
    }
    points
}

/// The flame at `time`: the tip and the small tongue wander, the sides breathe and the bottom stays; `lively` makes it
/// quicker and taller (working, speaking) and `sway` leans the tip.
fn flame(time: f32, lively: bool, sway: f32) -> Vec<(f32, f32)> {
    let (speed, reach) = if lively { (1.8, 1.4) } else { (1.0, 1.0) };
    let wave = |rate: f32, phase: f32| (time * rate * speed + phase).sin() * reach;
    let tip = (wave(3.1, 0.0) * 1.6 + sway * 4.0, wave(4.3, 1.0) * 2.4 - if lively { 2.0 } else { 0.0 });
    let tongue = (wave(3.7, 1.0) * 1.4, wave(5.1, 2.0) * 2.0);
    let (right, left) = (wave(2.3, 0.0) * 1.2, wave(2.3, 0.8) * 1.2);
    let mut curves = FLAME;
    let mut start = FLAME_START;
    let nudge = |point: &mut (f32, f32), (x, y): (f32, f32)| {
        point.0 += x;
        point.1 += y;
    };
    nudge(&mut curves[0][0], (wave(3.1, 0.6) * 1.2 + tip.0 * 0.5, 0.0));
    for (curve, at) in [(0, 1), (0, 2), (1, 0)] {
        curves[curve][at].0 += right;
    }
    for (curve, at) in [(2, 1), (2, 2), (3, 0)] {
        curves[curve][at].0 -= left;
    }
    nudge(&mut curves[3][2], tongue);
    nudge(&mut curves[4][0], (tongue.0 * 0.5, tongue.1 * 0.5));
    nudge(&mut curves[5][1], (tip.0 * 0.5, tip.1 * 0.5));
    nudge(&mut curves[5][2], tip);
    nudge(&mut start, tip);
    trace(start, &curves)
}

/// The flame's triangles: found once on the flame at rest, they hold for its small licks (the same points, moved a
/// little).
fn flame_triangles() -> &'static [[u32; 3]] {
    static TRIANGLES: std::sync::OnceLock<Vec<[u32; 3]>> = std::sync::OnceLock::new();
    TRIANGLES.get_or_init(|| triangulate(&trace(FLAME_START, &FLAME)))
}

/// Figure units to the screen: squashed, turned about the pivot and moved by `shift`.
struct Pen<'a> {
    painter: &'a Painter,
    origin: Pos2,
    scale: f32,
    squash: Vec2,
    sin: f32,
    cos: f32,
    shift: Vec2,
}

impl Pen<'_> {
    fn at(&self, x: f32, y: f32) -> Pos2 {
        let (x, dy) = (x * self.squash.x, (y - PIVOT_Y) * self.squash.y);
        Pos2::new(
            self.origin.x + (x * self.cos - dy * self.sin + self.shift.x) * self.scale,
            self.origin.y + (x * self.sin + dy * self.cos + PIVOT_Y + self.shift.y) * self.scale,
        )
    }

    fn polygon(&self, points: impl Iterator<Item = (f32, f32)>, fill: Color32) {
        self.painter.add(Shape::convex_polygon(points.map(|(x, y)| self.at(x, y)).collect(), fill, Stroke::NONE));
    }

    /// A pill (a rectangle with wholly round ends) of half sizes `half` at `center`, then flattened by `flat` (an eye
    /// near the edge of the face).
    fn pill(&self, center: (f32, f32), half: (f32, f32), flat: Vec2, fill: Color32) {
        let round = half.0.min(half.1);
        self.polygon(
            (0..32).map(|index| {
                // between the axes, so no corner is asked which side it is on
                let (sin, cos) = ((index as f32 + 0.5) / 32.0 * std::f32::consts::TAU).sin_cos();
                let (x, y) = ((half.0 - round) * cos.signum() + round * cos, (half.1 - round) * sin.signum() + round * sin);
                (center.0 + x * flat.x, center.1 + y * flat.y)
            }),
            fill,
        );
    }

    /// The flame (not convex): its triangles filled, and its edge drawn over them so it is smooth.
    fn flame(&self, points: &[(f32, f32)], fill: Color32) {
        let screen: Vec<Pos2> = points.iter().map(|&(x, y)| self.at(x, y)).collect();
        let mut mesh = egui::epaint::Mesh::default();
        for &point in &screen {
            mesh.colored_vertex(point, fill);
        }
        for &[a, b, c] in flame_triangles() {
            mesh.add_triangle(a, b, c);
        }
        self.painter.add(Shape::mesh(mesh));
        self.painter.add(Shape::closed_line(screen, Stroke::new(1.0, fill)));
    }

    fn line(&self, points: &[(f32, f32)], width: f32, color: Color32) {
        let points: Vec<Pos2> = points.iter().map(|&(x, y)| self.at(x, y)).collect();
        self.painter.add(Shape::line(points, Stroke::new((width * self.scale).max(1.0), color)));
    }

    /// A star of `points` tips, as a fan of triangles (it is not convex).
    fn star(&self, center: Pos2, outer: f32, inner: f32, points: usize, turn: f32, color: Color32) {
        let mut mesh = egui::epaint::Mesh::default();
        mesh.colored_vertex(center, color);
        for index in 0..points * 2 {
            let angle = turn + index as f32 / (points * 2) as f32 * std::f32::consts::TAU;
            let radius = if index % 2 == 0 { outer } else { inner };
            mesh.colored_vertex(center + Vec2::new(angle.cos(), angle.sin()) * radius, color);
        }
        let count = (points * 2) as u32;
        for index in 0..count {
            mesh.add_triangle(0, 1 + index, 1 + (index + 1) % count);
        }
        self.painter.add(Shape::mesh(mesh));
    }
}

pub(super) fn paint(painter: &Painter, center: Pos2, size: f32, motion: &MagoMotion, time: f64) {
    let breath = motion.breath;
    let squash = Vec2::new(motion.squash_x.value * (1.0 - breath * 0.6), motion.squash_y.value * (1.0 + breath));
    let (sin, cos) = motion.tilt.value.sin_cos();
    let lift = motion.hop;
    let scale = size * FIT;
    // the figure in the middle of `size`
    let origin = Pos2::new(center.x, center.y - lift * scale - MIDDLE * scale);
    let pen = Pen { painter, origin, scale, squash, sin, cos, shift: Vec2::ZERO };

    // the shadow on the ground, smaller as it hops
    let ground = Pos2::new(center.x, origin.y + (PIVOT_Y + 0.03) * scale + lift * scale);
    let shade = (1.0 - lift * 3.0).clamp(0.4, 1.0);
    painter.add(Shape::ellipse_filled(ground, Vec2::new(0.24 * scale * shade, 0.03 * scale * shade), Color32::from_black_alpha(28)));

    let tint = |color: Color32| color.lerp_to_gamma(motion.tint_color, motion.tint_amount.value);
    let lively = motion.lively();
    pen.flame(&flame(time as f32, lively, motion.sway), tint(LUME));

    paint_eyes(&pen, motion, 1.0, 1.0, INK);
    paint_mouth(&pen, motion);

    for particle in &motion.particles {
        paint_particle(&pen, particle, time);
    }
    if let (Some(badge), true) = (motion.badge, size >= 26.0) {
        paint_badge(&pen, badge, motion.badge_scale.value, time);
    }
}

/// The eyes, `grow` times their size and `spread` times as far apart, in `ink`: pills where the gaze puts them on the
/// curved face (`MagoMotion::eyes`), blinking by their height; the other faces are drawn there too.
fn paint_eyes(pen: &Pen, motion: &MagoMotion, grow: f32, spread: f32, ink: Color32) {
    let look = motion.look;
    let radius = (BODY_HALF.0 * BODY_HALF.1).sqrt();
    let (width, height) = (radius * EYE_SIZE.0 * grow, radius * EYE_SIZE.1 * grow);
    for (index, eye) in motion.eyes().into_iter().enumerate() {
        let Some(eye) = eye else { continue };
        let (x, y) = (eye.at.x * spread, eye.at.y);
        // a blink squashes it, never thinner than a line
        let pill = |wide: f32, tall: f32| {
            let (wide, tall) = (width * wide, (height * tall * motion.open.value).max(width * wide * 0.3));
            pen.pill((x, y), (wide / 2.0, tall / 2.0), eye.flat, ink);
        };
        let line = 0.026 * grow.sqrt();
        match look.eye {
            EyeKind::Round => pill(1.0, 1.0),
            EyeKind::Wide => pill(1.16, 1.12),
            EyeKind::Focus => pill(1.0, 0.6),
            EyeKind::Happy => {
                let arc: Vec<(f32, f32)> = (0..=8)
                    .map(|step| {
                        let t = step as f32 / 8.0;
                        (x + (t - 0.5) * 0.1 * grow, y + (0.02 - 0.05 * (1.0 - (2.0 * t - 1.0).powi(2))) * grow)
                    })
                    .collect();
                pen.line(&arc, line, ink);
            }
            EyeKind::Sleepy => pen.line(&[(x - 0.04 * grow, y + 0.01), (x + 0.04 * grow, y + 0.01)], line, ink),
            EyeKind::Cross => {
                let arm = 0.035 * grow;
                pen.line(&[(x - arm, y - arm), (x + arm, y + arm)], line, ink);
                pen.line(&[(x + arm, y - arm), (x - arm, y + arm)], line, ink);
            }
            EyeKind::Spiral => {
                let spiral: Vec<(f32, f32)> = (0..=20)
                    .map(|step| {
                        let t = step as f32 / 20.0;
                        let angle = motion.spin + index as f32 * 1.5 + t * 2.5 * std::f32::consts::TAU;
                        (x + angle.cos() * 0.055 * grow * t, y + angle.sin() * 0.055 * grow * t)
                    })
                    .collect();
                pen.line(&spiral, 0.02 * grow.sqrt(), ink);
            }
        }
    }
}

/// A mini one: only the flame, filling `body` points, in its agent's `color`, with the two eyes, bigger and farther
/// apart; it breathes a little (taller as it gets narrower).
pub(super) fn paint_mini(painter: &Painter, center: Pos2, body: f32, color: Color32, motion: &MagoMotion, time: f64) {
    let breath = (time as f32 * 2.2).sin();
    let scale = body * MINI_FIT;
    let origin = Pos2::new(center.x, center.y - MIDDLE * scale);
    let pen = Pen { painter, origin, scale, squash: Vec2::new(1.0 - breath * 0.02, 1.0 + breath * 0.04), sin: 0.0, cos: 1.0, shift: Vec2::ZERO };
    // each one licks at its own time
    pen.flame(&flame(time as f32 + center.x * 0.37, motion.lively(), motion.sway), color);
    paint_eyes(&pen, motion, MINI_EYES, MINI_SPREAD, MINI_INK);
}

/// The dance's box (40 × 48, the preview's) in figure units: its 47 from the tip to the bottom are the flame's height,
/// its bottom middle (20, 48) is the pivot.
const BOX: f32 = (PIVOT_Y - FLAME_TIP) / 47.0;

fn from_box(point: Vec2) -> (f32, f32) {
    ((point.x - 20.0) * BOX, (point.y - 48.0) * BOX + PIVOT_Y)
}

/// Luna dancing: the flame in `color`, its body placed by `pose` about its bottom, the eyes as `paint` draws them, and
/// the arms, tubes floating beside it in the same colour, never thinner than 1.3 points nor shorter than 2.4 widths (so
/// they show on the island's small one).
pub(super) fn paint_dancer(painter: &Painter, center: Pos2, size: f32, color: Color32, motion: &MagoMotion, pose: &Pose, time: f64) {
    let scale = size * FIT;
    let origin = Pos2::new(center.x, center.y - MIDDLE * scale);
    let body = pose.body;
    let (sin, cos) = body.turn.to_radians().sin_cos();
    let pen = Pen { painter, origin, scale, squash: body.scale, sin, cos, shift: body.shift * BOX };
    pen.flame(&flame(time as f32, false, 0.0), color);
    paint_eyes(&pen, motion, 1.0, 1.0, MINI_INK);
    let Some(arms) = pose.arms.filter(|_| pose.arms_alpha > 0.01) else { return };
    let width = (ARM_WIDTH * BOX * scale).max(1.3);
    let fill = color.gamma_multiply(pose.arms_alpha.clamp(0.0, 1.0));
    for arm in arms {
        let (sin, cos) = arm.turn.to_radians().sin_cos();
        // its path is M0 -5v10: straight down at no turn
        let end = |side: f32| {
            let (x, y) = from_box(arm.shift + Vec2::new(-sin, cos) * side * ARM_LENGTH / 2.0);
            pen.at(x, y)
        };
        let (start, finish) = (end(-1.0), end(1.0));
        let middle = start + (finish - start) / 2.0;
        let along = (finish - start).normalized();
        let half = ((finish - start).length() / 2.0).max(width * 1.2);
        tube(painter, middle - along * half, middle + along * half, width, fill);
    }
}

/// A tube from `start` to `finish`, `width` thick with round ends, as one shape (a fading one shows no seams).
fn tube(painter: &Painter, start: Pos2, finish: Pos2, width: f32, fill: Color32) {
    let along = (finish - start).normalized();
    let across = along.rot90();
    let radius = width / 2.0;
    // half a circle round each end, from one side over the end to the other: the two make the outline
    let cap = |end: Pos2, side: Vec2, outward: Vec2| {
        (0..=8).map(move |step| {
            let angle = step as f32 / 8.0 * std::f32::consts::PI;
            end + (side * angle.cos() + outward * angle.sin()) * radius
        })
    };
    let points: Vec<Pos2> = cap(finish, across, along).chain(cap(start, -across, -along)).collect();
    painter.add(Shape::convex_polygon(points, fill, Stroke::NONE));
}

/// No mouth at rest: only a small dash while it speaks.
fn paint_mouth(pen: &Pen, motion: &MagoMotion) {
    if motion.mouth_open.value > 0.01 {
        let half = 0.02 + 0.03 * motion.talk;
        pen.line(&[(0.01 - half, 0.31), (0.01 + half, 0.31)], 0.022, INK);
    }
}

/// Ear clipping of a simple polygon: its triangles as indices.
fn triangulate(points: &[(f32, f32)]) -> Vec<[u32; 3]> {
    let cross = |a: (f32, f32), b: (f32, f32), c: (f32, f32)| (b.0 - a.0) * (c.1 - a.1) - (b.1 - a.1) * (c.0 - a.0);
    let area: f32 = (0..points.len()).map(|index| cross((0.0, 0.0), points[index], points[(index + 1) % points.len()])).sum();
    let sign = area.signum();
    let mut left: Vec<usize> = (0..points.len()).collect();
    let mut triangles = Vec::new();
    while left.len() > 3 {
        let count = left.len();
        let ear = (0..count).find(|&at| {
            let (a, b, c) = (left[(at + count - 1) % count], left[at], left[(at + 1) % count]);
            if cross(points[a], points[b], points[c]) * sign <= 0.0 {
                return false;
            }
            left.iter().filter(|&&other| other != a && other != b && other != c).all(|&other| {
                let p = points[other];
                let sides = [cross(points[a], points[b], p), cross(points[b], points[c], p), cross(points[c], points[a], p)];
                !sides.iter().all(|side| side * sign >= 0.0)
            })
        });
        // a degenerate leftover: stop rather than loop
        let Some(at) = ear else { break };
        triangles.push([left[(at + count - 1) % count] as u32, left[at] as u32, left[(at + 1) % count] as u32]);
        left.remove(at);
    }
    if left.len() == 3 {
        triangles.push([left[0] as u32, left[1] as u32, left[2] as u32]);
    }
    triangles
}

fn paint_particle(pen: &Pen, particle: &Particle, time: f64) {
    let fade = (1.0 - particle.age / particle.life).clamp(0.0, 1.0);
    let at = pen.at(particle.at.x, particle.at.y);
    match particle.kind {
        ParticleKind::Spark => {
            let twinkle = 0.6 + 0.4 * (time as f32 * 20.0 + particle.spin * 9.0).sin();
            let radius = 0.045 * pen.scale * (0.4 + fade) * twinkle;
            pen.star(at, radius, radius * 0.38, 4, particle.spin * 3.0 + particle.age * 4.0, GOLD.gamma_multiply(fade.sqrt()));
        }
        ParticleKind::Zzz => {
            let alpha = (fade * 2.0).min(1.0) * (particle.age * 3.0).min(1.0);
            let grow = 0.7 + particle.age * 0.35;
            pen.painter.text(
                at,
                egui::Align2::CENTER_CENTER,
                "z",
                FontId::proportional(0.13 * pen.scale * grow),
                Color32::from_rgb(0xc9, 0xd6, 0xff).gamma_multiply(alpha),
            );
        }
        ParticleKind::Sweat => {
            let radius = 0.025 * pen.scale;
            pen.painter.circle_filled(at, radius, SWEAT.gamma_multiply(fade));
            pen.painter.add(Shape::convex_polygon(
                vec![at + Vec2::new(-radius * 0.8, -radius * 0.4), at + Vec2::new(radius * 0.8, -radius * 0.4), at + Vec2::new(0.0, -radius * 2.2)],
                SWEAT.gamma_multiply(fade),
                Stroke::NONE,
            ));
        }
    }
}

/// The small seal at the flame's side: `!`, `?` or `…`, popping in and out with the scale.
fn paint_badge(pen: &Pen, badge: char, scale: f32, time: f64) {
    if scale < 0.05 {
        return;
    }
    let radius = (0.085 * pen.scale).max(4.5) * scale * (1.0 + 0.05 * (time as f32 * 6.0).sin());
    let at = pen.at(-0.34, -0.12);
    let fill = if badge == '…' { Color32::from_gray(235) } else { AMBER };
    pen.painter.circle_filled(at, radius, fill);
    if badge == '…' {
        for dot in 0..3 {
            let rise = ((time as f32 * 5.0 - dot as f32 * 0.9).sin() * 0.5 + 0.5) * radius * 0.25;
            pen.painter.circle_filled(at + Vec2::new((dot as f32 - 1.0) * radius * 0.5, -rise), (radius * 0.16).max(0.7), Color32::from_gray(90));
        }
    } else {
        pen.painter.text(at, egui::Align2::CENTER_CENTER, badge.to_string(), FontId::proportional(radius * 1.4), INK);
    }
}
