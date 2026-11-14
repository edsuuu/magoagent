//! Drawing the wizard with the painter. The figure lives in a box of ~1 unit (body ~0.7 wide, centered a
//! little low) and is mapped to the screen with the squash, tilt and lift of the moment.

use super::motion::{Particle, ParticleKind};
use super::{EyeKind, MagoMotion};
use crate::prelude::*;

const BODY_LIGHT: Color32 = Color32::from_rgb(0xa9, 0xc6, 0xff);
const BODY_MID: Color32 = Color32::from_rgb(0x7f, 0xa3, 0xf5);
const BODY_SHADE: Color32 = Color32::from_rgb(0x5a, 0x7e, 0xdc);
const HAT: Color32 = Color32::from_rgb(0x7a, 0x4b, 0xd8);
const HAT_SHADE: Color32 = Color32::from_rgb(0x56, 0x31, 0xa8);
const GOLD: Color32 = Color32::from_rgb(0xff, 0xca, 0x4a);
const INK: Color32 = Color32::from_rgb(0x1f, 0x1b, 0x3d);
const BLUSH: Color32 = Color32::from_rgb(0xff, 0x8f, 0xb1);
const WOOD: Color32 = Color32::from_rgb(0x9a, 0x62, 0x34);
const AMBER: Color32 = Color32::from_rgb(0xff, 0xb0, 0x20);
const SWEAT: Color32 = Color32::from_rgb(0x8f, 0xd3, 0xff);

/// The squash and tilt pivot: the body's bottom.
const PIVOT_Y: f32 = 0.45;
/// The figure's height is a bit less than the box, to leave room for the hat's tip and the hop.
const FIT: f32 = 0.86;
const BODY: (f32, f32, f32, f32) = (0.0, 0.12, 0.36, 0.33);
const BRIM_Y: f32 = -0.15;
const HAT_HALF: f32 = 0.27;

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

    fn ellipse(&self, center: (f32, f32), radius: (f32, f32), fill: Color32) {
        let points = (0..28)
            .map(|index| {
                let angle = index as f32 / 28.0 * std::f32::consts::TAU;
                self.at(center.0 + radius.0 * angle.cos(), center.1 + radius.1 * angle.sin())
            })
            .collect();
        self.painter.add(Shape::convex_polygon(points, fill, Stroke::NONE));
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

fn quadratic(from: (f32, f32), control: (f32, f32), to: (f32, f32), t: f32) -> (f32, f32) {
    let inverse = 1.0 - t;
    (inverse * inverse * from.0 + 2.0 * inverse * t * control.0 + t * t * to.0, inverse * inverse * from.1 + 2.0 * inverse * t * control.1 + t * t * to.1)
}

pub(super) fn paint(painter: &Painter, center: Pos2, size: f32, motion: &MagoMotion, time: f64) {
    let breath = motion.breath;
    let squash = Vec2::new(motion.squash_x.value * (1.0 - breath * 0.6), motion.squash_y.value * (1.0 + breath));
    let (sin, cos) = motion.tilt.value.sin_cos();
    let lift = motion.hop + if motion.look.mouth_open > 0.0 && motion.talk < 1.0 { 0.012 * motion.talk } else { 0.0 };
    let scale = size * FIT;
    // the figure's center sits a little low in the box, the hat's tip makes up the top
    let origin = Pos2::new(center.x, center.y + size * 0.04 - lift * scale - 0.12 * scale);
    let pen = Pen { painter, origin, scale, squash, sin, cos };
    let small = size < 34.0;

    // the shadow on the ground, smaller as it hops
    let ground = Pos2::new(center.x, origin.y + (PIVOT_Y + 0.04 + 0.12) * scale + lift * scale);
    let shade = (1.0 - lift * 3.0).clamp(0.4, 1.0);
    painter.add(Shape::ellipse_filled(ground, Vec2::new(0.3 * scale * shade, 0.04 * scale * shade), Color32::from_black_alpha(36)));

    let tint = |color: Color32| color.lerp_to_gamma(motion.tint_color, motion.tint_amount.value);
    // the body: dark base, a lighter bulb up and to the left, a shine
    pen.ellipse((BODY.0, BODY.1), (BODY.2, BODY.3), tint(BODY_SHADE));
    pen.ellipse((-0.015, 0.095), (BODY.2 * 0.93, BODY.3 * 0.9), tint(BODY_MID));
    pen.ellipse((-0.05, 0.05), (BODY.2 * 0.72, BODY.3 * 0.6), tint(BODY_LIGHT));
    if !small {
        pen.ellipse((-0.2, -0.0), (0.06, 0.035), Color32::from_white_alpha(120));
    }

    // the face
    if motion.blush.value > 0.02 && !small {
        let alpha = (motion.blush.value * 150.0) as u8;
        for side in [-1.0, 1.0] {
            pen.ellipse((side * 0.235, 0.215), (0.05, 0.03), BLUSH.gamma_multiply(alpha as f32 / 255.0));
        }
    }
    paint_eyes(&pen, motion, small);
    paint_mouth(&pen, motion);

    paint_hat(&pen, motion, small);
    paint_wand(&pen, motion);

    for particle in &motion.particles {
        paint_particle(&pen, particle, time);
    }
    if let (Some(badge), true) = (motion.badge, size >= 30.0) {
        paint_badge(&pen, badge, motion.badge_scale.value, time);
    }
}

fn paint_eyes(pen: &Pen, motion: &MagoMotion, small: bool) {
    let look = motion.look;
    let width = 0.026;
    for (index, center) in motion.eye_centers().into_iter().enumerate() {
        let (x, y) = (center.x, center.y);
        let side = if index == 0 { -1.0 } else { 1.0 };
        match look.eye {
            EyeKind::Round | EyeKind::Wide => {
                let radius = if look.eye == EyeKind::Wide { 0.1 } else { 0.087 };
                pen.ellipse((x, y), (radius, radius * motion.open.value.max(0.08)), INK);
                if motion.open.value > 0.5 {
                    // the shine moves against the gaze, a little one beside it
                    pen.ellipse((x - 0.03 - motion.look_x.value * 0.012, y - 0.034), (radius * 0.36, radius * 0.36), Color32::WHITE);
                    if !small {
                        pen.ellipse((x + 0.03, y + 0.03), (radius * 0.16, radius * 0.16), Color32::from_white_alpha(190));
                    }
                }
            }
            EyeKind::Focus => {
                pen.ellipse((x, y + 0.01), (0.085, 0.06 * motion.open.value.max(0.1)), INK);
                pen.ellipse((x - 0.022, y - 0.005), (0.026, 0.022), Color32::WHITE);
                // the brow, leaning in
                pen.line(&[(x + 0.09 * side, y - 0.1), (x - 0.09 * side, y - 0.075)], 0.026, INK);
            }
            EyeKind::Happy => {
                let arc: Vec<(f32, f32)> = (0..=10)
                    .map(|step| {
                        let t = step as f32 / 10.0;
                        (x + (t - 0.5) * 0.17, y + 0.02 - 0.06 * (1.0 - (2.0 * t - 1.0).powi(2)))
                    })
                    .collect();
                pen.line(&arc, width + 0.012, INK);
            }
            EyeKind::Sleepy => {
                let arc: Vec<(f32, f32)> = (0..=10)
                    .map(|step| {
                        let t = step as f32 / 10.0;
                        (x + (t - 0.5) * 0.15, y + 0.01 + 0.03 * (1.0 - (2.0 * t - 1.0).powi(2)))
                    })
                    .collect();
                pen.line(&arc, width + 0.008, INK);
            }
            EyeKind::Cross => {
                pen.line(&[(x - 0.05, y - 0.05), (x + 0.05, y + 0.05)], width + 0.01, INK);
                pen.line(&[(x + 0.05, y - 0.05), (x - 0.05, y + 0.05)], width + 0.01, INK);
            }
            EyeKind::Spiral => {
                let turn = motion.spin + (index as f32) * 1.5;
                let spiral: Vec<(f32, f32)> = (0..=24)
                    .map(|step| {
                        let t = step as f32 / 24.0;
                        let angle = turn + t * 3.0 * std::f32::consts::TAU;
                        (x + angle.cos() * 0.075 * t, y + angle.sin() * 0.075 * t)
                    })
                    .collect();
                pen.line(&spiral, width, INK);
            }
        }
    }
}

fn paint_mouth(pen: &Pen, motion: &MagoMotion) {
    let look = motion.look;
    let (curve, open) = (motion.mouth_curve.value, motion.mouth_open.value * motion.talk.max(0.35));
    let (x, y, half) = (0.0, 0.265, look.mouth_width);
    if open > 0.012 {
        pen.ellipse((x, y + open * 0.4), (half.max(0.035) * (0.8 + open * 4.0), open), INK);
        pen.ellipse((x, y + open * 0.9), (half.max(0.035) * 0.55, open * 0.45), BLUSH);
    } else {
        let arc: Vec<(f32, f32)> = (0..=8)
            .map(|step| {
                let t = step as f32 / 8.0;
                (x + (t - 0.5) * 2.0 * half, y + curve * (1.0 - (2.0 * t - 1.0).powi(2)) * 1.0 - curve * 0.5)
            })
            .collect();
        pen.line(&arc, 0.022, INK);
    }
}

fn paint_hat(pen: &Pen, motion: &MagoMotion, small: bool) {
    let sway = motion.sway;
    let tip = (sway * 0.17, -0.58 + sway.abs() * 0.05);
    let (left, right) = ((-HAT_HALF, BRIM_Y + 0.02), (HAT_HALF, BRIM_Y + 0.02));
    let middle = |a: (f32, f32), b: (f32, f32)| ((a.0 + b.0) / 2.0, (a.1 + b.1) / 2.0);
    let control_left = (middle(left, tip).0 - 0.025 + sway * 0.05, middle(left, tip).1);
    let control_right = (middle(right, tip).0 + 0.025 + sway * 0.05, middle(right, tip).1);
    let edge = |from, control, to, t| quadratic(from, control, to, t);
    let tint = |color: Color32| color.lerp_to_gamma(motion.tint_color, motion.tint_amount.value * 0.6);
    // the brim, then the cone over it
    pen.ellipse((0.0, BRIM_Y + 0.02), (0.37, 0.065), tint(HAT_SHADE));
    let mut cone: Vec<Pos2> = (0..=10).map(|step| edge(left, control_left, tip, step as f32 / 10.0)).map(|(x, y)| pen.at(x, y)).collect();
    cone.extend((0..=10).rev().map(|step| edge(right, control_right, tip, step as f32 / 10.0)).map(|(x, y)| pen.at(x, y)));
    pen.painter.add(Shape::convex_polygon(cone, tint(HAT), Stroke::NONE));
    // the shaded side
    let mut shade: Vec<Pos2> = vec![pen.at(0.02, BRIM_Y + 0.02)];
    shade.extend((0..=10).map(|step| edge(right, control_right, tip, step as f32 / 10.0)).map(|(x, y)| pen.at(x, y)));
    shade.push(pen.at(tip.0 - 0.01, tip.1 + 0.02));
    pen.painter.add(Shape::convex_polygon(shade, tint(HAT_SHADE).gamma_multiply(0.55), Stroke::NONE));
    // the gold band along the base
    let band: Vec<Pos2> =
        [edge(left, control_left, tip, 0.0), edge(left, control_left, tip, 0.15), edge(right, control_right, tip, 0.15), edge(right, control_right, tip, 0.0)]
            .iter()
            .map(|&(x, y)| pen.at(x, y))
            .collect();
    pen.painter.add(Shape::convex_polygon(band, GOLD, Stroke::NONE));
    // the star on it
    if !small {
        let at = edge(left, control_left, tip, 0.5);
        let at = pen.at((at.0 + edge(right, control_right, tip, 0.5).0) / 2.0, at.1);
        pen.star(at, 0.07 * pen.scale, 0.03 * pen.scale, 5, -std::f32::consts::FRAC_PI_2 + motion.tilt.value, GOLD);
    }
}

fn paint_wand(pen: &Pen, motion: &MagoMotion) {
    let hand = (0.31, 0.2);
    let tip = motion.wand_tip();
    pen.line(&[hand, (tip.x, tip.y)], 0.03, WOOD);
    pen.ellipse(hand, (0.045, 0.045), BODY_MID);
    let glow = pen.at(tip.x, tip.y);
    let pulse = 1.0 + 0.2 * (motion.wave * 8.0).sin();
    pen.painter.circle_filled(glow, 0.07 * pen.scale * pulse, GOLD.gamma_multiply(0.28));
    pen.star(glow, 0.052 * pen.scale * pulse, 0.022 * pen.scale, 4, motion.wave, GOLD);
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
            let radius = 0.03 * pen.scale;
            pen.painter.circle_filled(at, radius, SWEAT.gamma_multiply(fade));
            pen.painter.add(Shape::convex_polygon(
                vec![at + Vec2::new(-radius * 0.8, -radius * 0.4), at + Vec2::new(radius * 0.8, -radius * 0.4), at + Vec2::new(0.0, -radius * 2.2)],
                SWEAT.gamma_multiply(fade),
                Stroke::NONE,
            ));
        }
    }
}

/// The seal at the hat's side: `!`, `?` or `…`, popping in and out with the scale.
fn paint_badge(pen: &Pen, badge: char, scale: f32, time: f64) {
    if scale < 0.05 {
        return;
    }
    let radius = (0.12 * pen.scale).max(6.0) * scale * (1.0 + 0.06 * (time as f32 * 6.0).sin());
    let at = pen.at(0.4, -0.4);
    let fill = if badge == '…' { Color32::WHITE } else { AMBER };
    pen.painter.circle_filled(at + Vec2::new(0.0, radius * 0.12), radius, Color32::from_black_alpha(40));
    pen.painter.circle_filled(at, radius, fill);
    if badge == '…' {
        for dot in 0..3 {
            let rise = ((time as f32 * 5.0 - dot as f32 * 0.9).sin() * 0.5 + 0.5) * radius * 0.25;
            pen.painter.circle_filled(at + Vec2::new((dot as f32 - 1.0) * radius * 0.5, -rise), radius * 0.15, HAT_SHADE);
        }
    } else {
        pen.painter.text(at, egui::Align2::CENTER_CENTER, badge.to_string(), FontId::proportional(radius * 1.45), INK);
    }
}
