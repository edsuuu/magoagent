//! Claude Code's mascot, from its pixel-art drawings (assets/mascot/NOTICE): each square is painted
//! on its own, so the parts move. The body breathes, hops while it talks and sways while it thinks,
//! the legs tap, the eyes blink, the sparkles around it twinkle, and one expression fades into the
//! next.

// ponytail: the pixel-art mascot is kept for the island and others that still call paint_pose, unused until then; delete when none does
#![allow(dead_code)]

pub(crate) mod mago;
pub(crate) mod poses;

use crate::app::maestro::avatar::AvatarState;
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
            let attr = |name: &str| tag.split(&format!(" {name}=\"")).nth(1).and_then(|value| value.split('"').next());
            let number = |name: &str| attr(name).and_then(|value| value.parse::<f32>().ok()).unwrap_or(0.0);
            let (mut x, mut y) = (number("x"), number("y"));
            // some are turned half around a point: translate(tx,ty) rotate(-180)
            if let Some(transform) = attr("transform") {
                let numbers: Vec<f32> = transform
                    .split(|character: char| !(character.is_ascii_digit() || character == '.' || character == '-'))
                    .filter_map(|piece| piece.parse().ok())
                    .collect();
                if let [translate_x, translate_y, turn, ..] = numbers[..] {
                    (x, y) = if turn.abs() > 90.0 { (translate_x - x - SIDE, translate_y - y - SIDE) } else { (x + translate_x, y + translate_y) };
                }
            }
            let color = attr("fill").and_then(|value| Color32::from_hex(value).ok()).unwrap_or(Color32::WHITE);
            let part = match color {
                BODY => Part::Body,
                EYE => Part::Eye,
                Color32::WHITE => Part::Face,
                _ => Part::Spark,
            };
            Square { at: Pos2::new(x, y), color, part, cell: (0.0, 0.0) }
        })
        .collect();
    let (left, top) = squares.iter().fold((f32::MAX, f32::MAX), |(lowest_x, lowest_y), square| (lowest_x.min(square.at.x), lowest_y.min(square.at.y)));
    for square in &mut squares {
        square.cell = (((square.at.x - left) / STEP).round(), ((square.at.y - top) / STEP).round());
    }
    // the legs: the body's squares under its lowest row of eight or more
    let wide = squares
        .iter()
        .filter(|square| square.part == Part::Body)
        .map(|square| square.cell.1)
        .filter(|&row| squares.iter().filter(|square| square.part == Part::Body && square.cell.1 == row).count() >= 8)
        .fold(f32::MIN, f32::max);
    let mut columns: Vec<f32> = squares.iter().filter(|square| square.part == Part::Body && square.cell.1 > wide).map(|square| square.cell.0).collect();
    columns.sort_by(f32::total_cmp);
    columns.dedup();
    // side by side columns are one leg (a slanted one takes two)
    let leg_of = |col: f32| columns.windows(2).take_while(|pair| pair[1] <= col).filter(|pair| pair[1] - pair[0] > 1.0).count();
    for square in &mut squares {
        if wide > f32::MIN && square.part == Part::Body && square.cell.1 > wide {
            square.part = Part::Leg(leg_of(square.cell.0));
        }
    }
    squares
}

fn drawing(name: &str) -> &'static [Square] {
    static PARSED: OnceLock<Vec<(&'static str, Vec<Square>)>> = OnceLock::new();
    let parsed = PARSED.get_or_init(|| DRAWINGS.iter().map(|(name, svg)| (*name, parse(svg))).collect());
    parsed.iter().find(|(entry_name, _)| *entry_name == name).map_or(&[], |(_, squares)| squares)
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

fn motion(state: AvatarState, time: f64) -> Motion {
    let wave = |period: f64, phase: f64| ((time / period + phase) * std::f64::consts::TAU).sin() as f32;
    // a blink of 0.13 s every 3.4 s
    let blink = (time / 3.4).fract() < 0.13 / 3.4;
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
fn paint_drawing(painter: &Painter, squares: &[Square], center: Pos2, width: f32, movement: &Motion, time: f64, alpha: f32) {
    let scale = width / CROP.width();
    let feet = CROP.center().y + 8.0 * STEP;
    let side = SIDE * scale;
    for square in squares {
        let mut at = square.at + Vec2::splat(SIDE / 2.0);
        let mut size = Vec2::splat(side);
        let mut color = square.color;
        match square.part {
            Part::Body | Part::Eye | Part::Face => {
                // squashed toward the feet, wider; then up and to the side
                at.y = feet + (at.y - feet) * (1.0 - movement.squash);
                at.x = CROP.center().x + (at.x - CROP.center().x) * (1.0 + movement.squash);
                at += Vec2::new(movement.sway, -movement.rise) * STEP;
                size = Vec2::new(side * (1.0 + movement.squash), side * (1.0 - movement.squash));
                if square.part == Part::Eye && movement.blink {
                    size.y *= 0.25;
                }
            }
            Part::Leg(leg) => at.y -= movement.legs[leg % 2] * STEP,
            Part::Spark => {
                let beat = (time * 3.0 + (square.cell.0 * 0.9 + square.cell.1 * 1.7) as f64).sin() as f32;
                color = color.gamma_multiply(0.6 + 0.4 * beat);
                at.y -= 0.15 * STEP * ((time * 2.0) as f32 + square.cell.0).sin();
            }
        }
        let at = center + (at - CROP.center()) * scale;
        painter.rect_filled(Rect::from_center_size(at, size), 0.0, color.gamma_multiply(alpha));
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
    let movement = motion(state, now);
    let fade_progress = (((now - since) / FADE).clamp(0.0, 1.0)) as f32;
    if fade_progress < 1.0 {
        paint_drawing(painter, drawing(before), center, width, &movement, now, 1.0 - fade_progress);
    }
    paint_drawing(painter, drawing(shown), center, width, &movement, now, fade_progress);
    if state == AvatarState::Listening {
        let ring = crate::app::maestro::avatar::LISTENING.gamma_multiply(0.6 + 0.4 * (now as f32 * 3.0).sin());
        painter.circle_stroke(center, width / 2.0, Stroke::new(1.5, ring));
    }
    painter.ctx().request_repaint_after(std::time::Duration::from_millis(33));
}

/// The same mascot with no fade and no shared state: for the places that show several at once (the island's grid).
pub(crate) fn paint_mascot_still(painter: &Painter, center: Pos2, width: f32, state: AvatarState, now: f64) {
    paint_drawing(painter, drawing(expression(state, now)), center, width, &motion(state, now), now, 1.0);
    painter.ctx().request_repaint_after(std::time::Duration::from_millis(33));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_drawing_comes_apart_into_body_four_legs_eyes_and_sparkles() {
        for (name, _) in DRAWINGS {
            let squares = drawing(name);
            let legs: std::collections::BTreeSet<usize> =
                squares.iter().filter_map(|square| if let Part::Leg(leg) = square.part { Some(leg) } else { None }).collect();
            assert_eq!(legs.len(), 4, "{name}: {legs:?}");
            assert!(squares.iter().filter(|square| square.part == Part::Body).count() > 100, "{name}");
            assert!(squares.iter().any(|square| square.part == Part::Eye), "{name}");
            // turned squares land on the artboard, near the rest
            assert!(squares.iter().all(|square| CROP.expand(200.0).contains(square.at)), "{name}");
        }
        assert_eq!(drawing("calm").len(), 219);
    }

    #[test]
    fn it_breathes_hops_while_talking_blinks_and_sleeps_still_eyed() {
        assert!(motion(AvatarState::Speaking, 0.19).rise > 0.4, "up in the hop");
        assert!(motion(AvatarState::Speaking, 0.0).squash > 0.05, "squashed landing");
        let legs = |time| motion(AvatarState::Speaking, time).legs;
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
            let painter = ui.painter();
            paint_mascot(painter, Pos2::new(100.0, 100.0), 80.0, AvatarState::Idle, 10.0);
            paint_mascot(painter, Pos2::new(100.0, 100.0), 80.0, AvatarState::Speaking, 20.0);
            assert_eq!(SHOWN.with(|cell| cell.get()), ("talking", "calm", 20.0));
            paint_mascot(painter, Pos2::new(100.0, 100.0), 80.0, AvatarState::Speaking, 20.1);
            assert_eq!(SHOWN.with(|cell| cell.get()).2, 20.0, "the fade keeps its start");
        });
        out.textures_delta.clear();
    }
}
