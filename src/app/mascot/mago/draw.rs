//! Drawing the wizard with the painter: a smooth one-color squircle (a bit wider than tall), two black dots for
//! eyes and the app icon hat, resting crooked on top. The figure lives in a box of ~1 unit and is mapped to the screen with the
//! squash, tilt and lift of the moment.

use super::motion::{HAND_BALL, HAT_BASE_Y, HAT_UNIT, Particle, ParticleKind, hat_point};
use super::{EyeKind, MagoMotion};
use crate::prelude::*;

const BODY_BASE: Color32 = Color32::from_rgb(0xcb, 0xc0, 0xf3);
const BODY_LIGHT: Color32 = Color32::from_rgb(0xe6, 0xe0, 0xfb);
/// The app icon colors: the hat and its four-pointed star.
const HAT: Color32 = Color32::from_rgb(0xe4, 0xe4, 0xe8);
const HAT_LINE: Color32 = Color32::from_rgb(0xb4, 0xb2, 0xc6);
const STAR: Color32 = Color32::from_rgb(0xf0, 0xa0, 0x20);
const GOLD: Color32 = Color32::from_rgb(0xff, 0xca, 0x4a);
const INK: Color32 = Color32::from_rgb(0x17, 0x14, 0x2b);
const AMBER: Color32 = Color32::from_rgb(0xff, 0xb0, 0x20);
const SWEAT: Color32 = Color32::from_rgb(0x8f, 0xd3, 0xff);

/// The squash and tilt pivot: the body bottom.
const PIVOT_Y: f32 = 0.47;
const FIT: f32 = 0.96;
const BODY_CENTER: f32 = 0.2;
/// The squircle half width and height.
const BODY_HALF: (f32, f32) = (0.34, 0.27);

/// The hat outline as drawn in assets/icon.png (512 px, y down): the wide A with its tip folded into a
/// hook, ending in the notch under the middle. Traced clockwise as one simple polygon.
const HAT_OUTLINE: [(f32, f32); 23] = [
    (125.0, 345.0),
    (150.0, 300.0),
    (190.0, 225.0),
    (222.0, 160.0),
    (236.0, 132.0),
    (250.0, 117.0),
    (268.0, 111.0),
    (288.0, 115.0),
    (306.0, 126.0),
    (320.0, 138.0),
    (300.0, 141.0),
    (291.0, 148.0),
    (293.0, 162.0),
    (310.0, 205.0),
    (345.0, 280.0),
    (382.0, 345.0),
    (300.0, 335.0),
    (272.0, 262.0),
    (264.0, 246.0),
    (256.0, 241.0),
    (248.0, 243.0),
    (240.0, 255.0),
    (210.0, 335.0),
];

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

    /// The squircle body, `grow` times the size, moved by `shift`.
    fn squircle(&self, shift: (f32, f32), grow: f32, fill: Color32) {
        self.polygon(
            (0..48).map(|index| {
                let angle = index as f32 / 48.0 * std::f32::consts::TAU;
                let (sin, cos) = angle.sin_cos();
                // a superellipse of power 4
                let round = |value: f32| value.signum() * value.abs().sqrt();
                (shift.0 + BODY_HALF.0 * grow * round(cos), BODY_CENTER + shift.1 + BODY_HALF.1 * grow * round(sin))
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
    painter.add(Shape::ellipse_filled(ground, Vec2::new(0.3 * scale * shade, 0.03 * scale * shade), Color32::from_black_alpha(28)));

    let tint = |color: Color32| color.lerp_to_gamma(motion.tint_color, motion.tint_amount.value);
    // one color, with only a hint of light on top
    paint_hands(&pen, motion, &tint);
    pen.squircle((0.0, 0.0), 1.0, tint(BODY_BASE));
    for grow in [0.92, 0.8, 0.66] {
        pen.squircle((-0.015, -0.025), grow, tint(BODY_LIGHT).gamma_multiply(0.25));
    }

    paint_eyes(&pen, motion);
    paint_mouth(&pen, motion);
    paint_hat(&pen, motion, time);

    for particle in &motion.particles {
        paint_particle(&pen, particle, time);
    }
    if let (Some(badge), true) = (motion.badge, size >= 26.0) {
        paint_badge(&pen, badge, motion.badge_scale.value, time);
    }
}

/// The loose hands, behind the body: a ball at rest, a tilted capsule when they gesture (technique adapted
/// from Coucou, MIT), in the body color with the same hint of light.
fn paint_hands(pen: &Pen, motion: &MagoMotion, tint: &dyn Fn(Color32) -> Color32) {
    for index in 0..2 {
        let (near, far) = motion.hand_ends(index);
        let half = (far - near).length() / 2.0;
        let axis = if half > 0.0 { (far - near).normalized() } else { Vec2::X };
        let ends = [near, far];
        for (grow, shift, color) in [(1.0, 0.0, tint(BODY_BASE)), (0.62, -0.012, tint(BODY_LIGHT).gamma_multiply(0.55))] {
            let width = HAND_BALL * grow;
            let ends = ends.map(|end| (end.x, end.y + shift));
            if half > 0.001 {
                // the capsule: a quad between the two round ends
                let normal = Vec2::new(-axis.y, axis.x) * (width / 2.0);
                pen.polygon(
                    [
                        (ends[0].0 + normal.x, ends[0].1 + normal.y),
                        (ends[1].0 + normal.x, ends[1].1 + normal.y),
                        (ends[1].0 - normal.x, ends[1].1 - normal.y),
                        (ends[0].0 - normal.x, ends[0].1 - normal.y),
                    ]
                    .into_iter(),
                    color,
                );
            }
            for end in ends {
                pen.ellipse(end, (width / 2.0, width / 2.0), color);
            }
        }
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

fn paint_hat(pen: &Pen, motion: &MagoMotion, time: f64) {
    let sway = motion.sway;
    // the outline points, the tip folding a bit more with the spring
    let points: Vec<(f32, f32)> = HAT_OUTLINE
        .iter()
        .map(|&(x, y)| {
            let height = ((HAT_BASE_Y - y) / 265.0).clamp(0.0, 1.2);
            hat_point(sway, ((x - 252.0) * HAT_UNIT + sway * 0.06 * height * height, (y - HAT_BASE_Y) * HAT_UNIT))
        })
        .collect();
    // the brim: a wide flat ellipse
    let brim: Vec<Pos2> = (0..28)
        .map(|index| {
            let angle = index as f32 / 28.0 * std::f32::consts::TAU;
            let (x, y) = hat_point(sway, (182.0 * HAT_UNIT * angle.cos(), 7.0 * HAT_UNIT + 28.0 * HAT_UNIT * angle.sin()));
            pen.at(x, y)
        })
        .collect();
    pen.painter.add(Shape::convex_polygon(brim, HAT, Stroke::new(1.0, HAT_LINE)));
    let screen: Vec<Pos2> = points.iter().map(|&(x, y)| pen.at(x, y)).collect();
    let mut mesh = egui::epaint::Mesh::default();
    for &point in &screen {
        mesh.colored_vertex(point, HAT);
    }
    for [a, b, c] in triangulate(&points) {
        mesh.add_triangle(a, b, c);
    }
    pen.painter.add(Shape::mesh(mesh));
    pen.painter.add(Shape::closed_line(screen, Stroke::new(1.0, HAT_LINE)));
    // the star beside the tip, twinkling; the sparks come out of it
    let star = motion.star();
    let twinkle = 1.0 + 0.14 * (time as f32 * 4.0).sin();
    pen.star(pen.at(star.x, star.y), 0.1 * pen.scale * twinkle, 0.027 * pen.scale * twinkle, 4, -std::f32::consts::FRAC_PI_2, STAR);
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

/// The small seal at the hat side: `!`, `?` or `…`, popping in and out with the scale.
fn paint_badge(pen: &Pen, badge: char, scale: f32, time: f64) {
    if scale < 0.05 {
        return;
    }
    let radius = (0.085 * pen.scale).max(4.5) * scale * (1.0 + 0.05 * (time as f32 * 6.0).sin());
    let at = pen.at(-0.38, -0.15);
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
