//! The mascot, Lume: a small magic flame that licks and sways, with two dark eyes on its round bottom and nothing
//! else (all the expression is in the eyes and the flame); sparks fly from its tip when an agent finishes.
//! Properties chase a target with easing, a table of states gives the face, the eyes follow the pointer and blink
//! alone, particles fly, and clicks get reactions.

mod dance;
mod draw;
mod motion;
#[cfg(test)]
mod tests;

pub(crate) use dance::paint_mago_dancing;
pub(crate) use draw::LUME;
pub(crate) use motion::{MagoMotion, gaze_toward};

use crate::app::maestro::avatar::AvatarState;
use crate::app::ui::agents::AgentState;
use crate::prelude::*;
use crate::transcript::ChatItem;
use std::cell::RefCell;
use std::collections::HashMap;

/// What the wizard is doing; every agent state (and the voice's) lands on one of these.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub(crate) enum MagoState {
    #[default]
    Idle,
    /// Focused, the flame licks quicker.
    Working,
    /// The `!` badge.
    NeedsYou,
    /// The `?` badge.
    Asking,
    /// Sparks from the tip and a little hop.
    Done,
    Error,
    /// Zz, after a long time still.
    Sleeping,
    Listening,
    Speaking,
    /// Hello: smiling, one hand waving.
    Waving,
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

    /// The same in the turn its conversation tells (`items`): a step that failed in the turn at work or just over shows
    /// (as the island's cards had it), and at work, while its words are the last thing in the turn, it talks.
    pub(crate) fn in_turn(state: AgentState, items: &[ChatItem], now: f64) -> Self {
        let base = Self::from_agent(state, now);
        let turn = &items[items.iter().rposition(|item| matches!(item, ChatItem::User(_))).map_or(0, |at| at + 1)..];
        let last_step = turn.iter().rev().find_map(|item| if let ChatItem::Tool(step) = item { Some(step) } else { None });
        let error_after = turn.iter().rev().take_while(|item| !matches!(item, ChatItem::Tool(_))).any(|item| matches!(item, ChatItem::Error(_)));
        match base {
            Self::Working | Self::Done if last_step.is_some_and(|step| step.failed) || error_after => Self::Error,
            Self::Working if matches!(turn.last(), Some(ChatItem::Agent(_))) => Self::Speaking,
            _ => base,
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
    pub(crate) badge: Option<char>,
    pub(crate) tilt: f32,
    /// The flame's sway: amplitude and speed.
    pub(crate) sway: (f32, f32),
    /// The tint over the body and how much of it.
    pub(crate) tint: (Color32, f32),
}

const RED: Color32 = Color32::from_rgb(0xe8, 0x6a, 0x7e);
const NIGHT: Color32 = Color32::from_rgb(0x3a, 0x3f, 0x7a);

impl Look {
    const BASE: Look =
        Look { eye: EyeKind::Round, mouth_curve: 0.03, mouth_open: 0.0, mouth_width: 0.07, badge: None, tilt: 0.0, sway: (0.12, 1.6), tint: (RED, 0.0) };

    pub(crate) fn of(state: MagoState) -> Look {
        let base = Self::BASE;
        match state {
            MagoState::Idle => base,
            MagoState::Working => Look { eye: EyeKind::Focus, mouth_curve: 0.0, mouth_width: 0.05, badge: Some('…'), sway: (0.9, 5.0), ..base },
            MagoState::NeedsYou => {
                Look { eye: EyeKind::Wide, mouth_curve: 0.0, mouth_open: 0.0, mouth_width: 0.04, badge: Some('!'), sway: (0.35, 4.0), ..base }
            }
            MagoState::Asking => Look { eye: EyeKind::Wide, mouth_curve: -0.006, mouth_width: 0.04, badge: Some('?'), tilt: 0.12, ..base },
            MagoState::Done => Look { eye: EyeKind::Happy, mouth_curve: 0.055, mouth_width: 0.1, sway: (0.5, 6.0), ..base },
            MagoState::Error => Look { eye: EyeKind::Cross, mouth_curve: -0.035, sway: (0.0, 1.0), tint: (RED, 0.3), ..base },
            MagoState::Sleeping => {
                Look { eye: EyeKind::Sleepy, mouth_curve: 0.0, mouth_width: 0.04, tilt: 0.07, sway: (0.0, 1.0), tint: (NIGHT, 0.22), ..base }
            }
            MagoState::Listening => Look { eye: EyeKind::Wide, mouth_curve: 0.015, sway: (0.2, 2.2), ..base },
            MagoState::Speaking => Look { mouth_curve: 0.0, mouth_open: 0.03, ..base },
            MagoState::Waving => Look { eye: EyeKind::Happy, mouth_curve: 0.05, mouth_width: 0.09, sway: (0.4, 5.0), ..base },
        }
    }
}

thread_local! {
    /// One motion per mascot on screen, by the caller's id.
    static MOTIONS: RefCell<HashMap<u64, MagoMotion>> = RefCell::new(HashMap::new());
}

/// Paints the wizard `size` tall at `center` in `color` (Luna's `LUME`, an agent's own), advancing `motion` to `time`.
/// `pointer` is where the eyes look (none: around, by themselves).
pub(crate) fn paint_mago(
    painter: &Painter,
    center: Pos2,
    size: f32,
    color: Color32,
    state: MagoState,
    motion: &mut MagoMotion,
    time: f64,
    pointer: Option<Pos2>,
) {
    // a pointer that is not a number would make the gaze NaN, which the eyes never recover from
    let toward = pointer.map(|pointer| gaze_toward(center, pointer)).filter(|toward| toward.x.is_finite() && toward.y.is_finite());
    motion.advance(state, time, toward);
    draw::paint(painter, center, size, color, motion, time);
    // at rest it only sways slowly and blinks (0.12 s): 15 frames a second show it, and a wizard always on screen (the
    // island's) no longer keeps the whole app at 30 while nothing happens; a click or the pointer bring frames anyway
    let frame = if matches!(state, MagoState::Idle | MagoState::Sleeping) { 66 } else { 33 };
    painter.ctx().request_repaint_after(std::time::Duration::from_millis(frame));
}

/// What `paint` draws, squashed or stretched by `scale` around `center` (the entrance's bounces): egui scales its shapes only evenly, so they are made triangles first and those are scaled.
pub(crate) fn paint_scaled(painter: &Painter, center: Pos2, scale: Vec2, paint: impl FnOnce(&Painter)) {
    if scale == Vec2::splat(1.0) {
        return paint(painter);
    }
    let (ctx, layer) = (painter.ctx(), painter.layer_id());
    let start = ctx.graphics_mut(|graphics| graphics.entry(layer).next_idx().0);
    paint(painter);
    let end = ctx.graphics_mut(|graphics| graphics.entry(layer).next_idx().0);
    let font_size = ctx.fonts(|fonts| fonts.font_image_size());
    let mut tessellator = egui::epaint::Tessellator::new(ctx.pixels_per_point(), ctx.tessellation_options(|options| *options), font_size, Vec::new());
    ctx.graphics_mut(|graphics| {
        let list = graphics.entry(layer);
        for index in start..end {
            list.mutate_shape(egui::layers::ShapeIdx(index), |clipped| {
                let mut mesh = egui::Mesh::default();
                tessellator.tessellate_shape(std::mem::replace(&mut clipped.shape, egui::Shape::Noop), &mut mesh);
                for vertex in &mut mesh.vertices {
                    vertex.pos = center + (vertex.pos - center) * scale;
                }
                clipped.shape = egui::Shape::mesh(mesh);
            });
        }
    });
}

/// Luna, for any place that has no state of its own: the motion lives under `id`, the eyes follow the
/// pointer and a click on the wizard makes it react.
pub(crate) fn paint_mago_for(painter: &Painter, id: &str, center: Pos2, size: f32, state: MagoState, time: f64) {
    paint_mago_toward(painter, id, center, size, state, time, painter.ctx().pointer_latest_pos());
}

/// The same for an agent, in its own `color` (`DrawingApp::agent_color`), as its minis in the lists have it: its
/// mascot in its conversation, its card and its pill.
pub(crate) fn paint_agent_mago(painter: &Painter, id: &str, color: Color32, center: Pos2, size: f32, state: MagoState, time: f64) {
    paint_mago_tinted(painter, id, color, center, size, state, time, painter.ctx().pointer_latest_pos());
}

/// How many times the wizard under `id` was poked, and whether it is dizzy from it at `time`.
pub(crate) fn mago_pokes(id: &str, time: f64) -> (u32, bool) {
    MOTIONS.with(|motions| motions.borrow().get(&motion_key(id)).map_or((0, false), |motion| (motion.pokes_total(), motion.dizzy(time))))
}

/// Where the wizard under `id` was last asked to look (tests).
#[cfg(test)]
pub(crate) fn mago_gaze(id: &str) -> Option<Vec2> {
    MOTIONS.with(|motions| motions.borrow().get(&motion_key(id)).map(MagoMotion::gaze))
}

/// A mini wizard under `id`: the body alone, `body` points across, in `color`, with its two eyes glancing around by
/// themselves (the agents of the island's compact grid, of its pills and of its list).
pub(crate) fn paint_mini(painter: &Painter, id: &str, center: Pos2, body: f32, color: Color32, state: MagoState, time: f64) {
    if body < 1.0 {
        return;
    }
    let key = motion_key(id);
    MOTIONS.with(|motions| {
        let mut motions = motions.borrow_mut();
        let motion = motions.entry(key).or_insert_with(|| MagoMotion::seeded(key));
        let glance = motion.glance(time);
        motion.advance(state, time, Some(glance));
        draw::paint_mini(painter, center, body, color, motion, time);
    });
    painter.ctx().request_repaint_after(std::time::Duration::from_millis(33));
}

/// The same, still: one pose of `state`, and no frames asked for (an agent at rest in the sidebar's list leaves the
/// app idle).
pub(crate) fn paint_mini_still(painter: &Painter, id: &str, center: Pos2, body: f32, color: Color32, state: MagoState) {
    if body < 1.0 {
        return;
    }
    let mut motion = MagoMotion::seeded(motion_key(id));
    motion.advance(state, 0.0, Some(Vec2::ZERO));
    draw::paint_mini(painter, center, body, color, &motion, 0.0);
}

/// A poke on the wizard under `id` without drawing it (while it dances its motion is another): it counts towards dizziness.
pub(crate) fn mago_poke(id: &str, time: f64) {
    let key = motion_key(id);
    MOTIONS.with(|motions| motions.borrow_mut().entry(key).or_insert_with(|| MagoMotion::seeded(key)).poke(time));
}

fn motion_key(id: &str) -> u64 {
    fnv1a(id)
}

/// `paint_mago_for` with the eyes looking at `pointer` (in the painter's own coordinates; none: ahead): for a place whose
/// pointer is not the window's own, like the island following the cursor over the whole screen.
pub(crate) fn paint_mago_toward(painter: &Painter, id: &str, center: Pos2, size: f32, state: MagoState, time: f64, pointer: Option<Pos2>) {
    paint_mago_tinted(painter, id, LUME, center, size, state, time, pointer);
}

fn paint_mago_tinted(painter: &Painter, id: &str, color: Color32, center: Pos2, size: f32, state: MagoState, time: f64, pointer: Option<Pos2>) {
    let poked = pointer.is_some_and(|at| at.distance(center) < size * 0.45) && painter.ctx().input(|input| input.pointer.primary_pressed());
    let key = motion_key(id);
    MOTIONS.with(|motions| {
        let mut motions = motions.borrow_mut();
        if motions.len() > 64 {
            motions.retain(|_, motion| motion.idle_for(time) < 10.0);
        }
        let motion = motions.entry(key).or_insert_with(|| MagoMotion::seeded(key));
        if poked {
            motion.poke(time);
        }
        paint_mago(painter, center, size, color, state, motion, time, pointer);
    });
}
