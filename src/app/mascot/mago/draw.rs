//! Drawing the wizard with the painter: a smooth one-color blob (wider at the bottom), two black dots for
//! eyes and a small crooked hat. The figure lives in a box of ~1 unit and is mapped to the screen with the
//! squash, tilt and lift of the moment.

use super::motion::{Particle, ParticleKind};
use super::{EyeKind, MagoMotion};
use crate::prelude::*;

const BODY_BASE: Color32 = Color32::from_rgb(0xd6, 0xcc, 0xf5);
const BODY_LIGHT: Color32 = Color32::from_rgb(0xe9, 0xe4, 0xfc);
const HAT: Color32 = Color32::from_rgb(0x6d, 0x44, 0xc9);
const HAT_EDGE: Color32 = Color32::from_rgb(0x56, 0x34, 0xa6);
const GOLD: Color32 = Color32::from_rgb(0xff, 0xca, 0x4a);
const INK: Color32 = Color32::from_rgb(0x17, 0x14, 0x2b);
const AMBER: Color32 = Color32::from_rgb(0xff, 0xb0, 0x20);
const SWEAT: Color32 = Color32::from_rgb(0x8f, 0xd3, 0xff);

/// The squash and tilt pivot: the body bottom.
const PIVOT_Y: f32 = 0.42;
const FIT: f32 = 1.05;
const BODY_CENTER: f32 = 0.12;
const BODY_RADIUS: f32 = 0.3;
/// Where the hat sits: the middle of its base, and how crooked it rests (radians).
const HAT_BASE: (f32, f32) = (0.02, -0.14);
const HAT_CROOK: f32 = 0.2;

/// Figure units to the screen.
struct Pen<'a> {
    painter: &'a Painter,
    origin: Pos2,
    scale: f32,
    squash: Vec2,
    sin: f32,
    cos: f32,
}

impl Pen<'_> {
    fn at(&self, x: f32, y: f32) -> Pos2 {
        let (x, dy) = (x * self.squash.x, (y - PIVOT_Y) * self.squash.y);
        Pos2::new(self.origin.x + (x * self.cos - dy * self.sin) * self.scale, self.origin.y + (x * self.sin + dy * self.cos + PIVOT_Y) * self.scale)
    }

    fn polygon(&self, points: impl Iterator<Item = (f32, f32)>, fill: Color32) {
        self.painter.add(Shape::convex_polygon(points.map(|(x, y)| self.at(x, y)).collect(), fill, Stroke::NONE));
    }

    fn ellipse(&self, center: (f32, f32), radius: (f32, f32), fill: Color32) {
        self.polygon(
            (0..28).map(|index| {
                let angle = index as f32 / 28.0 * std::f32::consts::TAU;
                (center.0 + radius.0 * angle.cos(), center.1 + radius.1 * angle.sin())
            }),
            fill,
        );
    }

    /// The soft drop: a circle whose bottom is wider than its top, `grow` times the size, moved by `shift`.
    fn blob(&self, shift: (f32, f32), grow: f32, fill: Color32) {
        self.polygon(
            (0..40).map(|index| {
                let angle = index as f32 / 40.0 * std::f32::consts::TAU;
                let wide = 1.0 + 0.16 * angle.sin();
                (shift.0 + BODY_RADIUS * grow * wide * angle.cos(), BODY_CENTER + shift.1 + BODY_RADIUS * grow * angle.sin())
            }),
            fill,
        );
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
    let origin = Pos2::new(center.x, center.y - lift * scale + 0.02 * scale);
    let pen = Pen { painter, origin, scale, squash, sin, cos };

    // the shadow on the ground, smaller as it hops
    let ground = Pos2::new(center.x, origin.y + (PIVOT_Y + 0.03) * scale + lift * scale);
    let shade = (1.0 - lift * 3.0).clamp(0.4, 1.0);
    painter.add(Shape::ellipse_filled(ground, Vec2::new(0.26 * scale * shade, 0.03 * scale * shade), Color32::from_black_alpha(28)));

    let tint = |color: Color32| color.lerp_to_gamma(motion.tint_color, motion.tint_amount.value);
    // one color, with only a hint of light on top
    pen.blob((0.0, 0.0), 1.0, tint(BODY_BASE));
    pen.blob((-0.02, -0.03), 0.8, tint(BODY_LIGHT).gamma_multiply(0.7));

    paint_eyes(&pen, motion);
    paint_mouth(&pen, motion);
    paint_hat(&pen, motion);

    for particle in &motion.particles {
        paint_particle(&pen, particle, time);
    }
    if let (Some(badge), true) = (motion.badge, size >= 26.0) {
        paint_badge(&pen, badge, motion.badge_scale.value, time);
    }
}

fn paint_eyes(pen: &Pen, motion: &MagoMotion) {
    let look = motion.look;
    for (index, center) in motion.eye_centers().into_iter().enumerate() {
        let (x, y) = (center.x, center.y);
        // the dots squash when blinking; a bit larger when wide awake
        let open = motion.open.value.max(0.1);
        match look.eye {
            EyeKind::Round => pen.ellipse((x, y), (0.034, 0.05 * open), INK),
            EyeKind::Wide => pen.ellipse((x, y), (0.042, 0.064 * open), INK),
            EyeKind::Focus => pen.ellipse((x, y), (0.036, 0.03 * open), INK),
            EyeKind::Happy => {
                let arc: Vec<(f32, f32)> = (0..=8)
                    .map(|step| {
                        let t = step as f32 / 8.0;
                        (x + (t - 0.5) * 0.1, y + 0.02 - 0.05 * (1.0 - (2.0 * t - 1.0).powi(2)))
                    })
                    .collect();
                pen.line(&arc, 0.026, INK);
            }
            EyeKind::Sleepy => pen.line(&[(x - 0.04, y + 0.01), (x + 0.04, y + 0.01)], 0.026, INK),
            EyeKind::Cross => {
                pen.line(&[(x - 0.035, y - 0.035), (x + 0.035, y + 0.035)], 0.026, INK);
                pen.line(&[(x + 0.035, y - 0.035), (x - 0.035, y + 0.035)], 0.026, INK);
            }
            EyeKind::Spiral => {
                let spiral: Vec<(f32, f32)> = (0..=20)
                    .map(|step| {
                        let t = step as f32 / 20.0;
                        let angle = motion.spin + index as f32 * 1.5 + t * 2.5 * std::f32::consts::TAU;
                        (x + angle.cos() * 0.055 * t, y + angle.sin() * 0.055 * t)
                    })
                    .collect();
                pen.line(&spiral, 0.02, INK);
            }
        }
    }
}

/// No mouth at rest: only a small dash while it speaks.
fn paint_mouth(pen: &Pen, motion: &MagoMotion) {
    if motion.mouth_open.value > 0.01 {
        let half = 0.02 + 0.03 * motion.talk;
        pen.line(&[(0.02 - half, 0.27), (0.02 + half, 0.27)], 0.022, INK);
    }
}

fn paint_hat(pen: &Pen, motion: &MagoMotion) {
    // local coordinates from the base middle, turned by the crook and the spring sway
    let turn = HAT_CROOK + motion.sway * 0.14;
    let (sin, cos) = turn.sin_cos();
    let local = |x: f32, y: f32| (HAT_BASE.0 + x * cos - y * sin, HAT_BASE.1 + x * sin + y * cos);
    let bend = motion.sway.abs() * 0.04;
    // a short brim, then the cone (its tip folding a little to the side)
    pen.polygon(
        (0..20).map(|index| {
            let angle = index as f32 / 20.0 * std::f32::consts::TAU;
            local(0.2 * angle.cos(), 0.025 * angle.sin())
        }),
        HAT_EDGE,
    );
    pen.polygon(
        [local(-0.16, 0.0), local(-0.055, -0.15 + bend), local(0.05 + bend * 2.0, -0.31 + bend), local(0.06, -0.16), local(0.16, 0.0)].into_iter(),
        HAT,
    );
    // a single dot
    let (x, y) = local(0.0, -0.1);
    pen.painter.circle_filled(pen.at(x, y), (0.016 * pen.scale).max(0.8), GOLD);
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

/// The small seal at the hat side: `!`, `?` or `…`, popping in and out with the scale.
fn paint_badge(pen: &Pen, badge: char, scale: f32, time: f64) {
    if scale < 0.05 {
        return;
    }
    let radius = (0.085 * pen.scale).max(4.5) * scale * (1.0 + 0.05 * (time as f32 * 6.0).sin());
    let at = pen.at(0.3, -0.3);
    let fill = if badge == '…' { Color32::from_gray(235) } else { AMBER };
    pen.painter.circle_filled(at, radius, fill);
    if badge == '…' {
        for dot in 0..3 {
            let rise = ((time as f32 * 5.0 - dot as f32 * 0.9).sin() * 0.5 + 0.5) * radius * 0.25;
            pen.painter.circle_filled(at + Vec2::new((dot as f32 - 1.0) * radius * 0.5, -rise), (radius * 0.16).max(0.7), HAT);
        }
    } else {
        pen.painter.text(at, egui::Align2::CENTER_CENTER, badge.to_string(), FontId::proportional(radius * 1.4), INK);
    }
}
