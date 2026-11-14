//! The wizard mascot: a soft round blob under a pointy hat, big eyes and a wand that throws sparks when
//! an agent finishes. Drawn by code on every frame with the `Painter` (no images), so it stays sharp from
//! 24 px to 160 px. The technique (properties chasing a target with easing, a table of states -> face,
//! eyes that follow the pointer and blink alone, particles, reactions to clicks) is adapted from Coucou's
//! engine (MIT); the character, shapes and palette are this app's own.

mod draw;
mod motion;
#[cfg(test)]
mod tests;

pub(crate) use motion::MagoMotion;

use crate::app::maestro::avatar::AvatarState;
use crate::app::ui::agents::AgentState;
use crate::prelude::*;
use std::cell::RefCell;
use std::collections::HashMap;

/// What the wizard is doing; every agent state (and the voice's) lands on one of these.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub(crate) enum MagoState {
    #[default]
    Idle,
    /// Focused, the hat sways.
    Working,
    /// The `!` badge.
    NeedsYou,
    /// The `?` badge.
    Asking,
    /// Sparks from the wand and a little hop.
    Done,
    Error,
    /// Zz, after a long time still.
    Sleeping,
    Listening,
    Speaking,
}

impl MagoState {
    /// An agent's state at `now`: a turn that just ended celebrates, one long over falls asleep.
    pub(crate) fn from_agent(state: AgentState, now: f64) -> Self {
        match state {
            AgentState::Working(_) => Self::Working,
            AgentState::Waiting => Self::NeedsYou,
            AgentState::Idle(Some(since)) if now - since > SLEEP_AFTER => Self::Sleeping,
            AgentState::Idle(Some(since)) if now - since < CELEBRATE_FOR => Self::Done,
            AgentState::Idle(_) | AgentState::Active(_) => Self::Idle,
            AgentState::Ended => Self::Sleeping,
        }
    }

    pub(crate) fn from_voice(state: AvatarState) -> Self {
        match state {
            AvatarState::Idle => Self::Idle,
            AvatarState::Listening => Self::Listening,
            AvatarState::Thinking => Self::Working,
            AvatarState::Speaking => Self::Speaking,
            AvatarState::Muted => Self::Sleeping,
        }
    }
}

/// Seconds still before it dozes off, and how long the end of a turn is celebrated.
pub(crate) const SLEEP_AFTER: f64 = 300.0;
pub(crate) const CELEBRATE_FOR: f64 = 6.0;

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) enum EyeKind {
    Round,
    Wide,
    /// Narrowed, concentrating.
    Focus,
    /// An arch, laughing.
    Happy,
    /// A droopy line, asleep.
    Sleepy,
    Cross,
    Spiral,
}

/// The face and posture of a state: the table the motion chases.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct Look {
    pub(crate) eye: EyeKind,
    /// The mouth's bend (+ smile, - frown), how open it is and how wide.
    pub(crate) mouth_curve: f32,
    pub(crate) mouth_open: f32,
    pub(crate) mouth_width: f32,
    pub(crate) blush: f32,
    pub(crate) badge: Option<char>,
    pub(crate) tilt: f32,
    /// The hat's sway: amplitude and speed.
    pub(crate) sway: (f32, f32),
    /// The wand's resting angle, and how much (and how fast) it waves.
    pub(crate) wand: (f32, f32, f32),
    /// The tint over the body and how much of it.
    pub(crate) tint: (Color32, f32),
}

const RED: Color32 = Color32::from_rgb(0xe8, 0x6a, 0x7e);
const NIGHT: Color32 = Color32::from_rgb(0x3a, 0x3f, 0x7a);

impl Look {
    const BASE: Look = Look {
        eye: EyeKind::Round,
        mouth_curve: 0.03,
        mouth_open: 0.0,
        mouth_width: 0.07,
        blush: 0.6,
        badge: None,
        tilt: 0.0,
        sway: (0.12, 1.6),
        wand: (0.55, 0.06, 1.5),
        tint: (RED, 0.0),
    };

    pub(crate) fn of(state: MagoState) -> Look {
        let base = Self::BASE;
        match state {
            MagoState::Idle => base,
            MagoState::Working => Look {
                eye: EyeKind::Focus,
                mouth_curve: 0.0,
                mouth_width: 0.05,
                blush: 0.2,
                badge: Some('…'),
                sway: (0.9, 5.0),
                wand: (0.7, 0.35, 7.0),
                ..base
            },
            MagoState::NeedsYou => Look {
                eye: EyeKind::Wide,
                mouth_curve: 0.0,
                mouth_open: 0.035,
                mouth_width: 0.04,
                badge: Some('!'),
                sway: (0.35, 4.0),
                wand: (0.4, 0.3, 6.0),
                ..base
            },
            MagoState::Asking => Look { eye: EyeKind::Wide, mouth_curve: -0.006, mouth_width: 0.04, badge: Some('?'), tilt: 0.12, ..base },
            MagoState::Done => Look { eye: EyeKind::Happy, mouth_curve: 0.055, mouth_width: 0.1, blush: 1.0, sway: (0.5, 6.0), wand: (0.15, 0.3, 9.0), ..base },
            MagoState::Error => {
                Look { eye: EyeKind::Cross, mouth_curve: -0.035, blush: 0.0, sway: (0.0, 1.0), wand: (1.1, 0.0, 1.0), tint: (RED, 0.3), ..base }
            }
            MagoState::Sleeping => Look {
                eye: EyeKind::Sleepy,
                mouth_curve: 0.0,
                mouth_width: 0.04,
                tilt: 0.07,
                sway: (0.0, 1.0),
                wand: (1.2, 0.0, 1.0),
                tint: (NIGHT, 0.22),
                ..base
            },
            MagoState::Listening => Look { eye: EyeKind::Wide, mouth_curve: 0.015, sway: (0.2, 2.2), ..base },
            MagoState::Speaking => Look { mouth_curve: 0.0, mouth_open: 0.03, wand: (0.3, 0.3, 6.0), ..base },
        }
    }
}

thread_local! {
    /// One motion per mascot on screen, by the caller's id.
    static MOTIONS: RefCell<HashMap<u64, MagoMotion>> = RefCell::new(HashMap::new());
}

/// Paints the wizard `size` tall at `center`, advancing `motion` to `time`. `pointer` is where the eyes look.
pub(crate) fn paint_mago(painter: &Painter, center: Pos2, size: f32, state: MagoState, motion: &mut MagoMotion, time: f64, pointer: Option<Pos2>) {
    let toward = pointer.map(|pointer| (pointer - center) / (size * 2.5));
    motion.advance(state, time, toward);
    draw::paint(painter, center, size, motion, time);
    painter.ctx().request_repaint_after(std::time::Duration::from_millis(33));
}

/// The same, for any place that has no state of its own: the motion lives under `id`, the eyes follow the
/// pointer and a click on the wizard makes it react.
pub(crate) fn paint_mago_for(painter: &Painter, id: &str, center: Pos2, size: f32, state: MagoState, time: f64) {
    let pointer = painter.ctx().pointer_latest_pos();
    let poked = pointer.is_some_and(|at| at.distance(center) < size * 0.45) && painter.ctx().input(|input| input.pointer.primary_pressed());
    let key = id.bytes().fold(0xcbf29ce484222325u64, |hash, byte| (hash ^ byte as u64).wrapping_mul(0x100000001b3));
    MOTIONS.with(|motions| {
        let mut motions = motions.borrow_mut();
        if motions.len() > 64 {
            motions.retain(|_, motion| motion.idle_for(time) < 10.0);
        }
        let motion = motions.entry(key).or_insert_with(|| MagoMotion::seeded(key));
        if poked {
            motion.poke(time);
        }
        paint_mago(painter, center, size, state, motion, time, pointer);
    });
}
