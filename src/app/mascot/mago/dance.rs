//! Luna dancing (`Groove`): the preview's poses and timings, its CSS keyframes copied as they are, in the wizard's box of
//! 40 × 48 (y down, the body turning about its bottom middle, 20 × 48; an angle in degrees, clockwise). Each move takes a
//! whole number of beats near the preview's own duration (`cycle_beats`), starting on a beat, so every hop lands on one;
//! the headbang's nod falls on it too. The arms, two short tubes floating beside the body, only in Palmas, Mãos pro
//! alto, Onda and Giro, swing wider the louder it is. A new dance takes over the last pose in a quarter of a second.

use super::draw;
use super::{MOTIONS, MagoMotion, MagoState, motion_key};
use crate::app::integrations::spotify::groove::{Dance, Groove, cycle_beats, spin_beats};
use crate::prelude::*;
use std::cell::RefCell;
use std::collections::HashMap;

/// The arm: a tube 10 long and 4.4 thick (its path is `M0 -5v10`), in the box's units.
pub(crate) const ARM_LENGTH: f32 = 10.0;
pub(crate) const ARM_WIDTH: f32 = 4.4;
/// How long a new dance takes to take over.
const BLEND: f64 = 0.25;

/// A transform as the CSS writes it: moved, turned (degrees, clockwise) and scaled, in that order, about its origin.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct Place {
    pub(crate) shift: Vec2,
    pub(crate) turn: f32,
    pub(crate) scale: Vec2,
}

impl Place {
    const REST: Place = place(0.0, 0.0, 0.0, 1.0, 1.0);

    fn lerp(self, other: Place, amount: f32) -> Place {
        Place {
            shift: self.shift + (other.shift - self.shift) * amount,
            turn: self.turn + (other.turn - self.turn) * amount,
            scale: self.scale + (other.scale - self.scale) * amount,
        }
    }
}

const fn place(x: f32, y: f32, turn: f32, scale_x: f32, scale_y: f32) -> Place {
    Place { shift: Vec2::new(x, y), turn, scale: Vec2::new(scale_x, scale_y) }
}

/// An arm's place: where its middle is and its turn (it hangs straight down at 0).
const fn arm(x: f32, y: f32, turn: f32) -> Place {
    place(x, y, turn, 1.0, 1.0)
}

/// The pose of a moment: the body's place about its bottom middle and, in the dances that have them, the two arms'
/// (left, right) in the body's box, `arms_alpha` showing them.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct Pose {
    pub(crate) body: Place,
    pub(crate) arms: Option<[Place; 2]>,
    pub(crate) arms_alpha: f32,
}

impl Pose {
    fn lerp(self, other: Pose, amount: f32) -> Pose {
        let body = self.body.lerp(other.body, amount);
        let (arms, arms_alpha) = match (self.arms, other.arms) {
            (Some([left, right]), Some([to_left, to_right])) => {
                (Some([left.lerp(to_left, amount), right.lerp(to_right, amount)]), self.arms_alpha + (other.arms_alpha - self.arms_alpha) * amount)
            }
            (None, Some(arms)) => (Some(arms), other.arms_alpha * amount),
            (Some(arms), None) => (Some(arms), self.arms_alpha * (1.0 - amount)),
            (None, None) => (None, 0.0),
        };
        Pose { body, arms, arms_alpha }
    }
}

/// The CSS timing functions used: `ease-in-out` and the preview's cubic-béziers.
#[derive(Clone, Copy, Debug)]
struct Bezier(f32, f32, f32, f32);

const EASE_IN_OUT: Bezier = Bezier(0.42, 0.0, 0.58, 1.0);
const HOP_EASE: Bezier = Bezier(0.3, 0.7, 0.4, 1.0);
const BANG_EASE: Bezier = Bezier(0.5, 0.0, 0.3, 1.0);
const SPIN_EASE: Bezier = Bezier(0.6, 0.0, 0.3, 1.0);

impl Bezier {
    /// The curve's y where its x is `x` (0 to 1).
    fn at(self, x: f32) -> f32 {
        let Bezier(x1, y1, x2, y2) = self;
        let curve = |first: f32, second: f32, t: f32| 3.0 * (1.0 - t) * (1.0 - t) * t * first + 3.0 * (1.0 - t) * t * t * second + t * t * t;
        let (mut low, mut high) = (0.0f32, 1.0f32);
        for _ in 0..24 {
            let middle = (low + high) / 2.0;
            if curve(x1, x2, middle) < x {
                low = middle;
            } else {
                high = middle;
            }
        }
        curve(y1, y2, (low + high) / 2.0)
    }
}

/// A move: its keyframes (where in it, 0 to 1, and the place), the timing between each two, and about how long the
/// preview takes for it (seconds), made a whole number of beats.
struct Move {
    frames: &'static [(f32, Place)],
    ease: Bezier,
    seconds: f32,
}

impl Move {
    /// The place `progress` (0 to 1) into it.
    fn at(&self, progress: f32) -> Place {
        let progress = progress.clamp(0.0, 1.0);
        let next = self.frames.iter().position(|(at, _)| *at > progress).unwrap_or(self.frames.len() - 1).max(1);
        let ((from_at, from), (to_at, to)) = (self.frames[next - 1], self.frames[next]);
        let span = (to_at - from_at).max(1e-6);
        from.lerp(to, self.ease.at(((progress - from_at) / span).clamp(0.0, 1.0)))
    }
}

const SWAY: Move = Move {
    frames: &[(0.0, place(0.0, 0.0, -7.0, 1.0, 1.0)), (0.5, place(0.0, 0.0, 7.0, 1.0, 1.0)), (1.0, place(0.0, 0.0, -7.0, 1.0, 1.0))],
    ease: EASE_IN_OUT,
    seconds: 1.58,
};
const STEP: Move = Move {
    frames: &[
        (0.0, place(-4.0, 0.0, -4.0, 1.0, 1.0)),
        (0.25, place(0.0, -3.0, 0.0, 1.0, 1.0)),
        (0.5, place(4.0, 0.0, 4.0, 1.0, 1.0)),
        (0.75, place(0.0, -3.0, 0.0, 1.0, 1.0)),
        (1.0, place(-4.0, 0.0, -4.0, 1.0, 1.0)),
    ],
    ease: EASE_IN_OUT,
    seconds: 1.2,
};
const HOP_FRAMES: &[(f32, Place)] = &[
    (0.0, place(0.0, 0.0, 0.0, 1.05, 0.95)),
    (0.4, place(0.0, -6.0, 0.0, 0.97, 1.04)),
    (0.75, place(0.0, 0.0, 0.0, 1.02, 0.98)),
    (1.0, place(0.0, 0.0, 0.0, 1.05, 0.95)),
];
const CLAP: Move = Move { frames: HOP_FRAMES, ease: HOP_EASE, seconds: 0.556 };
const PUMP: Move = Move { frames: HOP_FRAMES, ease: HOP_EASE, seconds: 0.476 };
const WAVE: Move = Move {
    frames: &[(0.0, place(0.0, 0.0, -5.0, 1.0, 1.0)), (0.5, place(0.0, 0.0, 5.0, 1.0, 1.0)), (1.0, place(0.0, 0.0, -5.0, 1.0, 1.0))],
    ease: EASE_IN_OUT,
    seconds: 1.9,
};
/// `rotate(13deg) translateY(1px)`: the 1 px down is turned with it, (-sin 13°, cos 13°).
const BANG: Move = Move { frames: &[(0.0, Place::REST), (0.45, place(-0.225, 0.974, 13.0, 1.0, 1.0)), (1.0, Place::REST)], ease: BANG_EASE, seconds: 0.4 };
/// Where the nod is deepest: on the beat.
const BANG_NOD: f32 = 0.45;
const SPIN: Move = Move {
    frames: &[
        (0.0, Place::REST),
        (0.3, Place::REST),
        (0.45, place(0.0, -5.0, 0.0, -1.0, 1.0)),
        (0.6, place(0.0, -5.0, 0.0, 1.0, 1.0)),
        (0.75, Place::REST),
        (1.0, Place::REST),
    ],
    ease: SPIN_EASE,
    seconds: 2.4,
};
const NAP: Move = Move { frames: &[(0.0, Place::REST), (0.5, place(0.0, 0.0, 0.0, 1.03, 0.97)), (1.0, Place::REST)], ease: EASE_IN_OUT, seconds: 3.0 };

const CLAP_ARMS: [Move; 2] = [
    Move { frames: &[(0.0, arm(6.0, 3.0, -28.0)), (0.5, arm(17.6, -5.0, 0.0)), (1.0, arm(6.0, 3.0, -28.0))], ease: EASE_IN_OUT, seconds: 0.556 },
    Move { frames: &[(0.0, arm(34.0, 3.0, 28.0)), (0.5, arm(22.4, -5.0, 0.0)), (1.0, arm(34.0, 3.0, 28.0))], ease: EASE_IN_OUT, seconds: 0.556 },
];
const PUMP_ARMS: [Move; 2] = [
    Move { frames: &[(0.0, arm(0.0, 18.0, -30.0)), (0.5, arm(-1.0, 22.0, -48.0)), (1.0, arm(0.0, 18.0, -30.0))], ease: EASE_IN_OUT, seconds: 0.952 },
    Move { frames: &[(0.0, arm(41.0, 22.0, 48.0)), (0.5, arm(40.0, 18.0, 30.0)), (1.0, arm(41.0, 22.0, 48.0))], ease: EASE_IN_OUT, seconds: 0.952 },
];
const WAVE_ARMS: [Move; 2] = [
    Move { frames: &[(0.0, arm(-1.0, 18.0, -52.0)), (0.5, arm(4.0, 13.0, -8.0)), (1.0, arm(-1.0, 18.0, -52.0))], ease: EASE_IN_OUT, seconds: 1.9 },
    Move { frames: &[(0.0, arm(36.0, 13.0, 8.0)), (0.5, arm(41.0, 18.0, 52.0)), (1.0, arm(36.0, 13.0, 8.0))], ease: EASE_IN_OUT, seconds: 1.9 },
];
const SPIN_ARMS: [Move; 2] = [
    Move {
        frames: &[
            (0.0, arm(-2.0, 35.0, 22.0)),
            (0.25, arm(-2.0, 35.0, 22.0)),
            (0.4, arm(-5.0, 30.0, 90.0)),
            (0.65, arm(-5.0, 30.0, 90.0)),
            (0.8, arm(-2.0, 35.0, 22.0)),
            (1.0, arm(-2.0, 35.0, 22.0)),
        ],
        ease: EASE_IN_OUT,
        seconds: 2.4,
    },
    Move {
        frames: &[
            (0.0, arm(42.0, 35.0, -22.0)),
            (0.25, arm(42.0, 35.0, -22.0)),
            (0.4, arm(45.0, 30.0, -90.0)),
            (0.65, arm(45.0, 30.0, -90.0)),
            (0.8, arm(42.0, 35.0, -22.0)),
            (1.0, arm(42.0, 35.0, -22.0)),
        ],
        ease: EASE_IN_OUT,
        seconds: 2.4,
    },
];

/// The body's move of a dance, and its arms' (none in the four that have no arms).
fn moves(dance: Dance) -> (&'static Move, Option<&'static [Move; 2]>) {
    match dance {
        Dance::Sway => (&SWAY, None),
        Dance::Step => (&STEP, None),
        Dance::Clap => (&CLAP, Some(&CLAP_ARMS)),
        Dance::Pump => (&PUMP, Some(&PUMP_ARMS)),
        Dance::Wave => (&WAVE, Some(&WAVE_ARMS)),
        Dance::Bang => (&BANG, None),
        Dance::Spin => (&SPIN, Some(&SPIN_ARMS)),
        Dance::Nap => (&NAP, None),
    }
}

/// How far into a move of about `seconds` it is: a whole number of beats long, starting on a beat. The spin happens
/// once, from the beat it began on.
fn progress(groove: &Groove, seconds: f32) -> f32 {
    if groove.dance == Dance::Spin {
        return ((groove.beat - groove.since) / spin_beats(groove.bpm)).clamp(0.0, 1.0) as f32;
    }
    let beats = cycle_beats(seconds, groove.bpm);
    let progress = (groove.beat / beats).rem_euclid(1.0) as f32;
    if groove.dance == Dance::Bang { (progress + BANG_NOD).rem_euclid(1.0) } else { progress }
}

/// The pose of `groove` now. The arms swing wider the louder it is: from 55% of their move, quiet, to all of it (the
/// spin's are always whole).
pub(crate) fn pose(groove: &Groove) -> Pose {
    let (body, arms) = moves(groove.dance);
    let arms = arms.filter(|_| groove.dance.arms());
    let body = body.at(progress(groove, body.seconds));
    let reach = if groove.dance == Dance::Spin { 1.0 } else { 0.55 + 0.45 * groove.energy.clamp(0.0, 1.0) };
    let arms = arms.map(|moves| {
        moves.each_ref().map(|one| {
            let rest = one.frames[0].1;
            rest.lerp(one.at(progress(groove, one.seconds)), reach)
        })
    });
    Pose { body, arms, arms_alpha: if arms.is_some() { 1.0 } else { 0.0 } }
}

/// The last dance drawn under an id, the pose it left and when the new one came: the blend between them.
struct Blend {
    dance: Dance,
    from: Pose,
    changed: f64,
    last: Pose,
}

thread_local! {
    static BLENDS: RefCell<HashMap<u64, Blend>> = RefCell::new(HashMap::new());
}

/// The pose to draw under `key` at `time`: `pose`, taking over from the last dance's in `BLEND`.
pub(super) fn blended(key: u64, dance: Dance, pose: Pose, time: f64) -> Pose {
    BLENDS.with(|blends| {
        let mut blends = blends.borrow_mut();
        if blends.len() > 64 {
            blends.retain(|_, blend| (time - blend.changed).abs() < 60.0);
        }
        let blend = blends.entry(key).or_insert(Blend { dance, from: pose, changed: f64::MIN, last: pose });
        if blend.dance != dance {
            (blend.dance, blend.from, blend.changed) = (dance, blend.last, time);
        }
        let amount = ((time - blend.changed) / BLEND).clamp(0.0, 1.0) as f32;
        let amount = amount * amount * (3.0 - 2.0 * amount);
        blend.last = if amount >= 1.0 { pose } else { blend.from.lerp(pose, amount) };
        blend.last
    })
}

/// Luna dancing to `groove`: `size` tall at `center` as `paint_mago` (the moves and the arms reach about a third of it
/// beyond, above and to the sides), all in `color`, with her eyes (closed while she naps) under `id`.
pub(crate) fn paint_mago_dancing(painter: &Painter, id: &str, center: Pos2, size: f32, color: Color32, groove: &Groove, time: f64) {
    let finite = groove.beat.is_finite() && groove.since.is_finite() && groove.bpm.is_finite() && groove.energy.is_finite();
    if size < 1.0 || !size.is_finite() || !finite || !time.is_finite() {
        return;
    }
    let key = motion_key(id);
    let pose = blended(key, groove.dance, pose(groove), time);
    let state = if groove.dance == Dance::Nap { MagoState::Sleeping } else { MagoState::Idle };
    MOTIONS.with(|motions| {
        let mut motions = motions.borrow_mut();
        let motion = motions.entry(key).or_insert_with(|| MagoMotion::seeded(key));
        motion.advance(state, time, None);
        // overlay dance expressions on top of the base state
        if groove.dance != Dance::Nap {
            let (eye, mouth_curve, mouth_open) = draw::dance_expression(groove);
            motion.look.eye = eye;
            motion.mouth_curve.value = mouth_curve;
            motion.mouth_open.value = mouth_open;
        }
        draw::paint_dancer(painter, center, size, color, motion, &pose, time);
    });
    let frame = if groove.dance == Dance::Nap { std::time::Duration::from_millis(66) } else { crate::app::update::frame_spacing() };
    painter.ctx().request_repaint_after(frame.max(std::time::Duration::from_millis(16)));
}
