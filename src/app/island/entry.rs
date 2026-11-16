//! The entrance: when the app starts the island opens and the mascot drops into the middle of it with sparkles
//! around, bounces, slides aside to wave, comes back to the middle and sits there until the island folds to
//! compact, where the island's own wizard springs from that spot to its ear. The timing, curves and squash are Coucou's
//! (windows/src/mochi/greeting.ts, MIT; its character is not used): the same 640 by 150 space, shifted to our page.

use crate::prelude::*;
use std::sync::OnceLock;

/// When the entrance is over (`T.end`, its timer 50 ms after): the island folds 0.6 s later (`fsm.rs`).
pub(super) const END: f32 = 4.65;
const BODY: f32 = 58.0;
pub(super) const CENTER: Vec2 = Vec2::new(320.0, 90.0);
/// Coucou's space is 150 tall with the card at 36; ours has the page at 42.
pub(super) const SHIFT: f32 = 6.0;
/// Where the wizard starts: the compact island's middle height (`COMPACT.botCenterY`).
const START_Y: f32 = 16.0;
/// The wave: from the pop to the tuck (`T.pop1`, `T.tuck0`).
const WAVE: (f32, f32) = (1.45, 2.45);

fn segment(t: f32, from: f32, to: f32) -> f32 {
    ((t - from) / (to - from)).clamp(0.0, 1.0)
}

fn lerp(from: f32, to: f32, k: f32) -> f32 {
    from + (to - from) * k
}

fn ease_out(t: f32) -> f32 {
    1.0 - (1.0 - t).powi(3)
}

fn ease_in(t: f32) -> f32 {
    t * t * t
}

fn ease_in_out(t: f32) -> f32 {
    if t < 0.5 { 4.0 * t * t * t } else { 1.0 - (-2.0 * t + 2.0).powi(3) / 2.0 }
}

fn ease_back(t: f32) -> f32 {
    let (c1, c3) = (1.70158f32, 2.70158f32);
    1.0 + c3 * (t - 1.0).powi(3) + c1 * (t - 1.0).powi(2)
}

/// A bump: 0 outside `from..to`, up to 1 in its middle and back.
fn bump(t: f32, from: f32, to: f32) -> f32 {
    if t >= from && t < to { (std::f32::consts::PI * segment(t, from, to)).sin() } else { 0.0 }
}

/// Where the wizard is `t` seconds in.
pub(super) struct Pose {
    /// Where its centre is, from the island's top left, how tall it is drawn, and how squashed (x) or stretched (y).
    pub(super) at: Pos2,
    pub(super) size: f32,
    pub(super) scale: Vec2,
    pub(super) waving: bool,
    /// The card's own opacity.
    pub(super) card: f32,
}

/// The pose at `t` (greeting.ts `greetPose`).
pub(super) fn pose(t: f32) -> Pose {
    let t = t.min(END + 10.0);
    let (cx, cy) = (CENTER.x, CENTER.y);
    let size = if t < 0.2 { 0.0 } else { lerp(BODY * 0.15, BODY, ease_back(segment(t, 0.2, 0.6))) };
    let (land, peak, dip, spring, sink) = (cy + 0.12 * BODY, cy - 0.15 * BODY, cy + 0.36 * BODY, cy - 0.1 * BODY, cy + 0.3 * BODY);
    let drifts = [cx - 0.16 * BODY, cx - 0.45 * BODY, cx - 0.57 * BODY, cx - 0.85 * BODY];
    // it slides to the side, waves there and comes back
    let x = match t {
        t if t < 0.85 => cx,
        t if t < 1.2 => lerp(cx, drifts[0], ease_in_out(segment(t, 0.85, 1.2))),
        t if t < 1.3 => lerp(drifts[0], drifts[1], ease_in(segment(t, 1.2, 1.3))),
        t if t < 1.45 => lerp(drifts[1], drifts[2], ease_in_out(segment(t, 1.3, 1.45))),
        t if t < 2.4 => lerp(drifts[2], drifts[3], ease_in_out(segment(t, 1.45, 2.4))),
        t if t < 2.85 => drifts[3],
        t => lerp(drifts[3], cx, ease_in_out(segment(t, 2.85, 3.45))),
    };
    // it drops in, bounces, plunges and springs for the wave, sinks to tuck its hands and rises back
    let mut y = match t {
        t if t < 0.2 => START_Y,
        t if t < 0.6 => lerp(START_Y, land, ease_in(segment(t, 0.2, 0.6))),
        t if t < 0.73 => lerp(land, peak, ease_out(segment(t, 0.6, 0.73))),
        t if t < 0.9 => lerp(peak, cy, ease_in_out(segment(t, 0.73, 0.9))),
        t if t < 1.2 => cy,
        t if t < 1.3 => lerp(cy, dip, ease_in(segment(t, 1.2, 1.3))),
        t if t < 1.45 => lerp(dip, spring, ease_out(segment(t, 1.3, 1.45))),
        t if t < 1.6 => lerp(spring, cy, ease_in_out(segment(t, 1.45, 1.6))),
        t if t < 2.4 => cy,
        t if t < 2.7 => lerp(cy, sink, ease_in_out(segment(t, 2.4, 2.7))),
        t if t < 2.85 => sink,
        t => lerp(sink, cy, ease_in_out(segment(t, 2.85, 3.45))),
    };
    let waving = t >= WAVE.0 && t < WAVE.1;
    if waving {
        // the body bobs in step with the hand
        let wave = t - WAVE.0;
        y += (wave * std::f32::consts::TAU * 5.0).sin() * 0.02 * BODY * (wave / 0.08).min(1.0);
    }
    // squash and stretch: the landing, the bounce, the plunge and spring of the wave, the sink, a last little one
    let (land_squash, bounce) = (bump(t, 0.52, 0.68), bump(t, 0.62, 0.84));
    let (plunge, spring_up) = (bump(t, 1.18, 1.42), bump(t, 1.3, 1.46));
    let sinking = match t {
        t if (2.38..2.7).contains(&t) => ease_in_out(segment(t, 2.38, 2.7)),
        t if (2.7..2.85).contains(&t) => 1.0 - ease_in_out(segment(t, 2.7, 2.85)),
        _ => 0.0,
    };
    let micro = bump(t, 3.7, 3.82);
    let scale = Vec2::new(
        1.0 + 0.14 * land_squash - 0.1 * bounce + 0.12 * plunge - 0.18 * spring_up + 0.18 * sinking + 0.08 * micro,
        1.0 - 0.14 * land_squash + 0.18 * bounce - 0.12 * plunge + 0.25 * spring_up - 0.14 * sinking - 0.07 * micro,
    );
    Pose { at: Pos2::new(x, y + SHIFT), size: size * (62.0 / BODY), scale, waving, card: segment(t, 0.18, 0.45) }
}

struct Streak {
    across: f32,
    speed: f32,
    length: f32,
    thick: f32,
    alpha: f32,
    start: f32,
}

struct Sparkle {
    angle: f32,
    jitter: f32,
    size: f32,
    alpha: f32,
}

/// Coucou's seeded sequence (seed 7): the same streaks and sparkles every time.
fn particles() -> &'static (Vec<Streak>, Vec<Sparkle>) {
    static PARTICLES: OnceLock<(Vec<Streak>, Vec<Sparkle>)> = OnceLock::new();
    PARTICLES.get_or_init(|| {
        let mut seed = 7u32;
        let mut random = move || {
            seed = seed.wrapping_mul(1103515245).wrapping_add(12345) & 0x7fff_ffff;
            seed as f32 / 0x7fff_ffff as f32
        };
        let streaks = (0..70)
            .map(|_| Streak {
                across: random(),
                speed: 400.0 + random() * 300.0,
                length: 6.0 + random() * 16.0,
                thick: 1.0 + random() * 0.5,
                alpha: 0.25 + random() * 0.55,
                start: random() * 0.35,
            })
            .collect();
        let sparkles = (0..90)
            .map(|_| Sparkle {
                angle: random() * std::f32::consts::TAU,
                jitter: (random() - 0.5) * 0.22,
                size: 0.7 + random() * 0.9,
                alpha: 0.45 + random() * 0.55,
            })
            .collect();
        (streaks, sparkles)
    })
}

/// The white streaks of the fall and the ring of sparkles that bursts around the wizard at 0.45 s, inside `card`
/// (`origin`: the island's top left).
pub(super) fn paint_particles(painter: &Painter, origin: Pos2, card: Rect, t: f32) {
    let painter = painter.with_clip_rect(card);
    let (streaks, sparkles) = particles();
    let top = origin.y + SHIFT;
    if t < 0.55 {
        let island = lerp(184.0, 640.0, (std::f32::consts::FRAC_PI_2 * segment(t, 0.0, 0.5)).sin());
        let fade = 1.0 - segment(t, 0.4, 0.55);
        for streak in streaks.iter().filter(|streak| t >= streak.start) {
            let bottom = (t - streak.start) * streak.speed;
            let x = origin.x + 320.0 - island / 2.0 + streak.across * island;
            let from = Pos2::new(x, top + (bottom - streak.length).max(0.0));
            let to = Pos2::new(x, top + bottom.min(150.0));
            if bottom > 0.0 && to.y > from.y {
                painter.line_segment([from, to], Stroke::new(streak.thick, Color32::WHITE.gamma_multiply(streak.alpha * fade)));
            }
        }
    }
    let k = segment(t, 0.45, 1.8);
    if k > 0.0 && k < 1.0 {
        let radius = lerp(14.0, 380.0, ease_out(k));
        let fade = (1.0 - k) * if k < 0.08 { k / 0.08 } else { 1.0 };
        for sparkle in sparkles {
            let spread = 1.0 + sparkle.jitter;
            let at =
                Pos2::new(origin.x + CENTER.x + sparkle.angle.cos() * radius * spread, top + CENTER.y - SHIFT + sparkle.angle.sin() * radius * 0.34 * spread);
            painter.rect_filled(Rect::from_min_size(at, Vec2::splat(sparkle.size * 1.6)), 0, Color32::WHITE.gamma_multiply(sparkle.alpha * fade));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_wizard_drops_in_waves_aside_and_comes_back_to_the_middle() {
        assert_eq!(pose(0.1).size, 0.0, "not there yet");
        let middle = pose(0.9);
        assert!((middle.at.x - 320.0).abs() < 1.0 && middle.size > 60.0, "it appears in the middle");
        assert!(pose(2.0).waving && !pose(0.9).waving && !pose(2.6).waving);
        assert!(pose(2.0).at.x < 320.0 - 40.0, "it drifted left to wave");
        let end = pose(END);
        assert!((end.at.x - 320.0).abs() < 0.01 && (end.at.y - 96.0).abs() < 0.01, "back in the middle: {:?}", end.at);
        assert_eq!(end.scale, Vec2::splat(1.0));
        assert!(pose(0.6).scale.x > 1.05 && pose(0.6).scale.y < 0.95, "squashed as it lands");
        assert!(pose(1.38).scale.y > 1.1, "stretched as it springs up to wave");
    }

    #[test]
    fn the_sparkles_are_the_same_every_time() {
        let (streaks, sparkles) = particles();
        assert_eq!((streaks.len(), sparkles.len()), (70, 90));
        assert!(streaks.iter().all(|streak| (0.0..=1.0).contains(&streak.across)));
        assert!(std::ptr::eq(particles(), particles()));
    }
}
