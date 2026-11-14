//! The entrance: when the island first opens the wizard drops into the middle of the panel with a glow behind it and
//! sparkles around, waves, sinks, and slides to the left where Home keeps it. The timing and curves are Coucou's
//! (windows/src/mochi/greeting.ts, MIT; its character is not used): the same 640 by 150 space, shifted to our page.

use crate::prelude::*;
use std::sync::OnceLock;

/// When the wizard is home, and when the entrance is over (the Home content fades in from `CONTENT_FROM`).
pub(super) const HOME_AT: f32 = 3.45;
pub(super) const END: f32 = 3.6;
const CONTENT_FROM: f32 = 3.0;
const BODY: f32 = 58.0;
const CENTER: Vec2 = Vec2::new(320.0, 90.0);
/// Coucou's space is 150 tall with the card at 36; ours has the page at 42.
const SHIFT: f32 = 6.0;
/// The wave: from the pop to the tuck (greeting.ts `T.pop1`, `T.tuck0`).
const WAVE: (f32, f32) = (1.45, 2.4);

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

/// Where the wizard is `t` seconds in.
pub(super) struct Pose {
    /// Where its centre is, from the island's top left, and how tall it is drawn.
    pub(super) at: Pos2,
    pub(super) size: f32,
    pub(super) waving: bool,
    /// The glow: how strong, and how blue (it goes from gold to blue).
    pub(super) halo: f32,
    pub(super) blue: f32,
    /// How much of Home's content shows.
    pub(super) content: f32,
    /// The card's own opacity.
    pub(super) card: f32,
}

/// The pose at `t`; `home` is where Home keeps the wizard (from the island's top left).
pub(super) fn pose(t: f32, home: Pos2) -> Pose {
    let (cx, cy) = (CENTER.x, CENTER.y);
    let size = if t < 0.2 { 0.0 } else { lerp(BODY * 0.15, BODY, ease_back(segment(t, 0.2, 0.6))) };
    let (land, peak, dip, spring, sink) = (cy + 0.12 * BODY, cy - 0.15 * BODY, cy + 0.36 * BODY, cy - 0.1 * BODY, cy + 0.3 * BODY);
    let drifts = [cx - 0.16 * BODY, cx - 0.45 * BODY, cx - 0.57 * BODY, cx - 0.85 * BODY];
    let x = match t {
        t if t < 0.85 => cx,
        t if t < 1.2 => lerp(cx, drifts[0], ease_in_out(segment(t, 0.85, 1.2))),
        t if t < 1.3 => lerp(drifts[0], drifts[1], ease_in(segment(t, 1.2, 1.3))),
        t if t < 1.45 => lerp(drifts[1], drifts[2], ease_in_out(segment(t, 1.3, 1.45))),
        t if t < 2.4 => lerp(drifts[2], drifts[3], ease_in_out(segment(t, 1.45, 2.4))),
        t if t < 2.85 => drifts[3],
        t => lerp(drifts[3], home.x, ease_in_out(segment(t, 2.85, HOME_AT))),
    };
    let mut y = match t {
        t if t < 0.2 => 16.0,
        t if t < 0.6 => lerp(16.0, land, ease_in(segment(t, 0.2, 0.6))),
        t if t < 0.73 => lerp(land, peak, ease_out(segment(t, 0.6, 0.73))),
        t if t < 0.9 => lerp(peak, cy, ease_in_out(segment(t, 0.73, 0.9))),
        t if t < 1.2 => cy,
        t if t < 1.3 => lerp(cy, dip, ease_in(segment(t, 1.2, 1.3))),
        t if t < 1.45 => lerp(dip, spring, ease_out(segment(t, 1.3, 1.45))),
        t if t < 1.6 => lerp(spring, cy, ease_in_out(segment(t, 1.45, 1.6))),
        t if t < 2.4 => cy,
        t if t < 2.7 => lerp(cy, sink, ease_in_out(segment(t, 2.4, 2.7))),
        t if t < 2.85 => sink,
        t => lerp(sink, home.y - SHIFT, ease_in_out(segment(t, 2.85, HOME_AT))),
    };
    let waving = t >= WAVE.0 && t < WAVE.1;
    if waving {
        // the body bobs in step with the hand
        let wave = t - WAVE.0;
        y += (wave * std::f32::consts::TAU * 5.0).sin() * 0.02 * BODY * (wave / 0.08).min(1.0);
    }
    Pose {
        at: Pos2::new(x, y + SHIFT),
        size: size * (62.0 / BODY),
        waving,
        halo: ease_out(segment(t, 0.3, 0.7)),
        blue: segment(t, 2.5, 3.2),
        content: segment(t, CONTENT_FROM, END),
        card: segment(t, 0.18, 0.45),
    }
}

/// The glow's colour: gold, turning blue.
pub(super) fn halo_color(blue: f32) -> Color32 {
    let channel = |gold: f32, sky: f32| lerp(gold, sky, blue).round() as u8;
    Color32::from_rgb(channel(232.0, 59.0), channel(195.0, 158.0), channel(154.0, 255.0))
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
    fn the_wizard_drops_in_waves_and_ends_up_at_home() {
        let home = Pos2::new(68.0, 96.0);
        assert_eq!(pose(0.1, home).size, 0.0, "not there yet");
        let middle = pose(0.9, home);
        assert!((middle.at.x - 320.0).abs() < 1.0 && middle.size > 60.0, "it appears in the middle");
        assert!(pose(2.0, home).waving && !pose(0.9, home).waving && !pose(2.6, home).waving);
        assert!(pose(2.0, home).at.x < 320.0 - 40.0, "it drifted left to wave");
        let end = pose(END, home);
        assert!((end.at.x - 68.0).abs() < 0.01 && (end.at.y - 96.0).abs() < 0.01, "home: {:?}", end.at);
        assert_eq!((end.content, end.halo), (1.0, 1.0));
        assert!(pose(1.0, home).content == 0.0, "Home's content waits");
    }

    #[test]
    fn the_sparkles_are_the_same_every_time() {
        let (streaks, sparkles) = particles();
        assert_eq!((streaks.len(), sparkles.len()), (70, 90));
        assert!(streaks.iter().all(|streak| (0.0..=1.0).contains(&streak.across)));
        assert!(std::ptr::eq(particles(), particles()));
    }
}
