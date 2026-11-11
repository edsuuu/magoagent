//! Claude Code's mascot, from its pixel-art drawings (assets/mascot/NOTICE): each square is painted
//! on its own, so the parts move. The body breathes, hops while it talks and sways while it thinks,
//! the legs tap, the eyes blink, the sparkles around it twinkle, and one expression fades into the
//! next.

use crate::app::avatar::AvatarState;
use crate::prelude::*;
use std::sync::OnceLock;

/// The drawings' squares: 43.55 wide on a 43.2 step, on a 2000-unit artboard.
pub(crate) const STEP: f32 = 43.2;
pub(crate) const SIDE: f32 = 43.55;
/// The artboard's part with the figure in it (they come with room around).
pub(crate) const CROP: Rect = Rect { min: Pos2::new(340.0, 440.0), max: Pos2::new(1660.0, 1560.0) };
const BODY: Color32 = Color32::from_rgb(0xd9, 0x77, 0x57);
const EYE: Color32 = Color32::from_rgb(0x2f, 0x2f, 0x38);
/// One expression fades into the next over this long (seconds).
const FADE: f64 = 0.25;

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) enum Part {
    Body,
    /// One of the legs, counted from the left.
    Leg(usize),
    Eye,
    /// The mouth and the eyes' shine.
    Face,
    /// What is around it: sparkles, a bubble, a bulb.
    Spark,
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct Square {
    pub(crate) at: Pos2,
    pub(crate) color: Color32,
    pub(crate) part: Part,
    /// Its place on the drawing's grid, for the sparkles' own beat.
    pub(crate) cell: (f32, f32),
}

macro_rules! drawings {
    ($($name:literal),*) => { [$(($name, include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/assets/mascot/", $name, ".svg")))),*] };
}
const DRAWINGS: [(&str, &str); 6] = drawings!("calm", "listening", "thinking", "idea", "talking", "sleeping");

/// A drawing's squares, its parts told apart: the body's color, the eyes', white on the face,
/// the rest around it; the legs are the body's squares under its last wide row.
pub(crate) fn parse(svg: &str) -> Vec<Square> {
    let mut squares: Vec<Square> = svg
        .split("<rect")
        .skip(1)
        .map(|tag| {
            let tag = &tag[..tag.find('>').unwrap_or(tag.len())];
            let attr = |name: &str| tag.split(&format!(" {name}=\"")).nth(1).and_then(|v| v.split('"').next());
            let num = |name: &str| attr(name).and_then(|v| v.parse::<f32>().ok()).unwrap_or(0.0);
            let (mut x, mut y) = (num("x"), num("y"));
            // some are turned half around a point: translate(tx,ty) rotate(-180)
            if let Some(t) = attr("transform") {
                let n: Vec<f32> = t.split(|c: char| !(c.is_ascii_digit() || c == '.' || c == '-')).filter_map(|s| s.parse().ok()).collect();
                if let [tx, ty, turn, ..] = n[..] {
                    (x, y) = if turn.abs() > 90.0 { (tx - x - SIDE, ty - y - SIDE) } else { (x + tx, y + ty) };
                }
            }
            let color = attr("fill").and_then(|v| Color32::from_hex(v).ok()).unwrap_or(Color32::WHITE);
            let part = match color {
                BODY => Part::Body,
                EYE => Part::Eye,
                Color32::WHITE => Part::Face,
                _ => Part::Spark,
            };
            Square { at: Pos2::new(x, y), color, part, cell: (0.0, 0.0) }
        })
        .collect();
    let (left, top) = squares.iter().fold((f32::MAX, f32::MAX), |(l, t), s| (l.min(s.at.x), t.min(s.at.y)));
    for s in &mut squares {
        s.cell = (((s.at.x - left) / STEP).round(), ((s.at.y - top) / STEP).round());
    }
    // the legs: the body's squares under its lowest row of eight or more
    let wide = squares
        .iter()
        .filter(|s| s.part == Part::Body)
        .map(|s| s.cell.1)
        .filter(|&row| squares.iter().filter(|s| s.part == Part::Body && s.cell.1 == row).count() >= 8)
        .fold(f32::MIN, f32::max);
    let mut columns: Vec<f32> = squares.iter().filter(|s| s.part == Part::Body && s.cell.1 > wide).map(|s| s.cell.0).collect();
    columns.sort_by(f32::total_cmp);
    columns.dedup();
    // side by side columns are one leg (a slanted one takes two)
    let leg_of = |col: f32| columns.windows(2).take_while(|w| w[1] <= col).filter(|w| w[1] - w[0] > 1.0).count();
    for s in &mut squares {
        if wide > f32::MIN && s.part == Part::Body && s.cell.1 > wide {
            s.part = Part::Leg(leg_of(s.cell.0));
        }
    }
    squares
}

fn drawing(name: &str) -> &'static [Square] {
    static PARSED: OnceLock<Vec<(&'static str, Vec<Square>)>> = OnceLock::new();
    let parsed = PARSED.get_or_init(|| DRAWINGS.iter().map(|(name, svg)| (*name, parse(svg))).collect());
    parsed.iter().find(|(n, _)| *n == name).map_or(&[], |(_, squares)| squares)
}

/// The expression for a state at `now`: thinking goes from the dots to the lightbulb now and then.
fn expression(state: AvatarState, now: f64) -> &'static str {
    match state {
        AvatarState::Idle => "calm",
        AvatarState::Listening => "listening",
        AvatarState::Thinking if (now / 2.4).fract() < 0.75 => "thinking",
        AvatarState::Thinking => "idea",
        AvatarState::Speaking => "talking",
        AvatarState::Muted => "sleeping",
    }
}

/// How the parts move at `t`, in squares: the body's rise, sway and squash (on landing a hop),
/// the two pairs of legs' lift, and whether the eyes are shut.
#[derive(Debug, Default, PartialEq)]
struct Motion {
    rise: f32,
    sway: f32,
    squash: f32,
    legs: [f32; 2],
    blink: bool,
}

fn motion(state: AvatarState, t: f64) -> Motion {
    let wave = |period: f64, phase: f64| ((t / period + phase) * std::f64::consts::TAU).sin() as f32;
    // a blink of 0.13 s every 3.4 s
    let blink = (t / 3.4).fract() < 0.13 / 3.4;
    match state {
        AvatarState::Idle => Motion { rise: 0.12 * wave(3.2, 0.0), blink, ..Default::default() },
        AvatarState::Listening => Motion { rise: 0.1 * wave(1.6, 0.0), blink, ..Default::default() },
        AvatarState::Thinking => Motion { sway: 0.2 * wave(2.4, 0.0), legs: [0.35 * wave(0.7, 0.0).max(0.0), 0.0], blink, ..Default::default() },
        AvatarState::Speaking => {
            // hops, squashing a little as it lands; the legs take turns
            let hop = wave(0.76, 0.0).abs();
            Motion {
                rise: 0.45 * hop,
                squash: 0.07 * (1.0 - hop).powi(4),
                legs: [0.3 * wave(0.38, 0.0).max(0.0), 0.3 * wave(0.38, 0.5).max(0.0)],
                blink: false,
                sway: 0.0,
            }
        }
        AvatarState::Muted => Motion { rise: 0.1 * wave(4.5, 0.0), ..Default::default() },
    }
}

/// One drawing at `alpha`, `width` across at `center`, its parts moved by `m`.
fn paint_drawing(p: &Painter, squares: &[Square], center: Pos2, width: f32, m: &Motion, t: f64, alpha: f32) {
    let s = width / CROP.width();
    let feet = CROP.center().y + 8.0 * STEP;
    let side = SIDE * s;
    for q in squares {
        let mut at = q.at + Vec2::splat(SIDE / 2.0);
        let mut size = Vec2::splat(side);
        let mut color = q.color;
        match q.part {
            Part::Body | Part::Eye | Part::Face => {
                // squashed toward the feet, wider; then up and to the side
                at.y = feet + (at.y - feet) * (1.0 - m.squash);
                at.x = CROP.center().x + (at.x - CROP.center().x) * (1.0 + m.squash);
                at += Vec2::new(m.sway, -m.rise) * STEP;
                size = Vec2::new(side * (1.0 + m.squash), side * (1.0 - m.squash));
                if q.part == Part::Eye && m.blink {
                    size.y *= 0.25;
                }
            }
            Part::Leg(leg) => at.y -= m.legs[leg % 2] * STEP,
            Part::Spark => {
                let beat = (t * 3.0 + (q.cell.0 * 0.9 + q.cell.1 * 1.7) as f64).sin() as f32;
                color = color.gamma_multiply(0.6 + 0.4 * beat);
                at.y -= 0.15 * STEP * ((t * 2.0) as f32 + q.cell.0).sin();
            }
        }
        let at = center + (at - CROP.center()) * s;
        p.rect_filled(Rect::from_center_size(at, size), 0.0, color.gamma_multiply(alpha));
    }
}

thread_local! {
    /// The expression shown, the one before and when it changed: the fade between them.
    static SHOWN: std::cell::Cell<(&'static str, &'static str, f64)> = const { std::cell::Cell::new(("calm", "calm", f64::MIN)) };
}

/// Claude Code's mascot, `width` across (80 in the avatar's bar) at `center`, in the expression and
/// motion of the moment.
pub(crate) fn paint_mascot(painter: &Painter, center: Pos2, width: f32, state: AvatarState, now: f64) {
    let now_shown = expression(state, now);
    let (shown, before, since) = SHOWN.with(|cell| {
        let (shown, before, since) = cell.get();
        let next = if shown == now_shown { (shown, before, since) } else { (now_shown, shown, now) };
        cell.set(next);
        next
    });
    let m = motion(state, now);
    let k = (((now - since) / FADE).clamp(0.0, 1.0)) as f32;
    if k < 1.0 {
        paint_drawing(painter, drawing(before), center, width, &m, now, 1.0 - k);
    }
    paint_drawing(painter, drawing(shown), center, width, &m, now, k);
    if state == AvatarState::Listening {
        let ring = crate::app::avatar::LISTENING.gamma_multiply(0.6 + 0.4 * (now as f32 * 3.0).sin());
        painter.circle_stroke(center, width / 2.0, Stroke::new(1.5, ring));
    }
    painter.ctx().request_repaint_after(std::time::Duration::from_millis(33));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_drawing_comes_apart_into_body_four_legs_eyes_and_sparkles() {
        for (name, _) in DRAWINGS {
            let squares = drawing(name);
            let legs: std::collections::BTreeSet<usize> = squares.iter().filter_map(|s| if let Part::Leg(l) = s.part { Some(l) } else { None }).collect();
            assert_eq!(legs.len(), 4, "{name}: {legs:?}");
            assert!(squares.iter().filter(|s| s.part == Part::Body).count() > 100, "{name}");
            assert!(squares.iter().any(|s| s.part == Part::Eye), "{name}");
            // turned squares land on the artboard, near the rest
            assert!(squares.iter().all(|s| CROP.expand(200.0).contains(s.at)), "{name}");
        }
        assert_eq!(drawing("calm").len(), 219);
    }

    #[test]
    fn it_breathes_hops_while_talking_blinks_and_sleeps_still_eyed() {
        assert!(motion(AvatarState::Speaking, 0.19).rise > 0.4, "up in the hop");
        assert!(motion(AvatarState::Speaking, 0.0).squash > 0.05, "squashed landing");
        let legs = |t| motion(AvatarState::Speaking, t).legs;
        assert!(legs(0.1)[0] > 0.0 && legs(0.1)[1] == 0.0 && legs(0.3)[1] > 0.0, "the legs take turns");
        assert!(motion(AvatarState::Idle, 0.02).blink && !motion(AvatarState::Idle, 1.0).blink);
        assert!(!motion(AvatarState::Muted, 0.02).blink, "asleep, the eyes stay as drawn");
        assert_ne!(motion(AvatarState::Idle, 0.8).rise, motion(AvatarState::Idle, 2.4).rise, "it breathes");
        assert_eq!(expression(AvatarState::Thinking, 0.0), "thinking");
        assert_eq!(expression(AvatarState::Thinking, 2.0), "idea");
    }

    #[test]
    fn a_new_expression_fades_in_over_the_last() {
        let ctx = Context::default();
        let mut out = ctx.run_ui(egui::RawInput::default(), |ui| {
            let p = ui.painter();
            paint_mascot(p, Pos2::new(100.0, 100.0), 80.0, AvatarState::Idle, 10.0);
            paint_mascot(p, Pos2::new(100.0, 100.0), 80.0, AvatarState::Speaking, 20.0);
            assert_eq!(SHOWN.with(|c| c.get()), ("talking", "calm", 20.0));
            paint_mascot(p, Pos2::new(100.0, 100.0), 80.0, AvatarState::Speaking, 20.1);
            assert_eq!(SHOWN.with(|c| c.get()).2, 20.0, "the fade keeps its start");
        });
        out.textures_delta.clear();
    }
}
