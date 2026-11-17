//! The mascot's motion: properties chasing targets, a spring for the flame's sway, a hop, blinking, the eyes'
//! gaze, the particles and the reactions to clicks.

use super::{EyeKind, Look, MagoState};
use crate::prelude::*;

const GRAVITY: f32 = 14.0;
/// The face, the round bottom of the flame: where its middle is (figure units, y down) and its half width and height.
pub(crate) const BODY_CENTER: f32 = 0.2;
pub(crate) const BODY_HALF: (f32, f32) = (0.3, 0.27);
/// The eyes on the face, projected on the round body: each sits `EYE_SPREAD` radians to
/// its side of the middle and `EYE_PITCH` from it, and the gaze turns both (by `LOOK_YAW` and `LOOK_PITCH` at the most),
/// so they slide over a curved face and narrow as they near its edge. `LOOK_RATE` is its `1 - 0.0025^dt` a frame.
const EYE_SPREAD: f32 = 0.37;
const EYE_PITCH: f32 = -0.12;
const LOOK_YAW: f32 = 0.62;
const LOOK_PITCH: f32 = 0.5;
/// The mouth's place on the face, as the eyes' pitch: under them.
const MOUTH_PITCH: f32 = -0.42;
const LOOK_RATE: f32 = 5.99;
/// How far the pointer is (points, across and down) for the gaze to be most of the way to it.
const LOOK_FAR: (f32, f32) = (260.0, 200.0);

/// The gaze of a wizard at `center` toward `pointer` (the same coordinates, points): each axis from -1 to 1, to the
/// right and down. No zone: a pointer anywhere counts, the far ones about as much as each other.
pub(crate) fn gaze_toward(center: Pos2, pointer: Pos2) -> Vec2 {
    Vec2::new(((pointer.x - center.x) / LOOK_FAR.0).tanh(), ((pointer.y - center.y) / LOOK_FAR.1).tanh())
}

/// An eye on the face: its centre (figure units) and how much of its width and height show there.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct Eye {
    pub(crate) at: Vec2,
    pub(crate) flat: Vec2,
}
const MAX_PARTICLES: usize = 40;
/// The hand as a ball (its width, which is also the capsule width) and as a capsule (twice as long).
pub(crate) const HAND_BALL: f32 = 0.12;
const HAND_CAPSULE: f32 = 0.24;
/// The tip of the flame (figure units, y down): the sparks come out of it.
pub(crate) const FLAME_TIP: f32 = -0.4;

/// A value that eases toward its target (exponentially, `rate` per second).
#[derive(Clone, Copy, Debug)]
pub(crate) struct Tween {
    pub(crate) value: f32,
    pub(crate) target: f32,
}

impl Tween {
    pub(crate) fn new(value: f32) -> Self {
        Self { value, target: value }
    }

    pub(crate) fn step(&mut self, delta: f32, rate: f32) {
        if !self.value.is_finite() || !self.target.is_finite() {
            // never recovers by itself: start over from the target (or rest)
            self.value = if self.target.is_finite() { self.target } else { 0.0 };
            self.target = self.value;
        }
        self.value += (self.target - self.value) * (1.0 - (-rate * delta).exp());
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) enum ParticleKind {
    Spark,
    Zzz,
    Sweat,
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct Particle {
    pub(crate) at: Vec2,
    velocity: Vec2,
    pub(crate) age: f32,
    pub(crate) life: f32,
    pub(crate) kind: ParticleKind,
    pub(crate) spin: f32,
}

#[derive(Clone, Copy, Debug, PartialEq)]
enum Reaction {
    None,
    Annoyed,
    Dizzy,
}

pub(crate) struct MagoMotion {
    last_time: Option<f64>,
    state: MagoState,
    since: f64,
    pub(crate) squash_x: Tween,
    pub(crate) squash_y: Tween,
    pub(crate) tilt: Tween,
    pub(crate) look_x: Tween,
    pub(crate) look_y: Tween,
    pub(crate) open: Tween,
    pub(crate) badge_scale: Tween,
    /// The arms: 0 hanging at rest, 1 raised (waving, or up at the end of a turn).
    pub(crate) hands: Tween,
    /// The two hands (left, right), loose from the body: where each floats (a spring after its target), the
    /// near end (the pivot), the axis angle up from horizontal outward, and the length: a ball at rest, a capsule when it gestures.
    pub(crate) hand_at: [Vec2; 2],
    hand_velocity: [Vec2; 2],
    pub(crate) hand_angle: [f32; 2],
    pub(crate) hand_length: [f32; 2],
    pub(crate) tint_amount: Tween,
    pub(crate) tint_color: Color32,
    pub(crate) mouth_curve: Tween,
    pub(crate) mouth_open: Tween,
    pub(crate) sway: f32,
    sway_velocity: f32,
    pub(crate) hop: f32,
    hop_velocity: f32,
    next_hop: f64,
    blink_at: f64,
    blink_until: f64,
    shake_until: f64,
    reaction: Reaction,
    reaction_until: f64,
    pokes: u32,
    pokes_total: u32,
    last_poke: f64,
    spark_budget: f32,
    seed: u32,
    pub(crate) particles: Vec<Particle>,
    /// The face now (the state's, or a reaction's) and the breathing.
    pub(crate) look: Look,
    /// While she dances: the eyes and the mouth the song asks for (`draw::dance_expression`), on top of the state's
    /// face; the mouth eases there as it does to any face.
    pub(crate) dance_face: Option<(EyeKind, f32, f32)>,
    pub(crate) breath: f32,
    /// How much the mouth is open while it speaks (0..1).
    pub(crate) talk: f32,
    pub(crate) badge: Option<char>,
    /// The dizzy eyes' turn.
    pub(crate) spin: f32,
    /// Where a mini one glances, and until when (`glance`).
    glance: Vec2,
    glance_until: f64,
}

impl Default for MagoMotion {
    fn default() -> Self {
        let look = Look::of(MagoState::Idle);
        Self {
            last_time: None,
            state: MagoState::Idle,
            since: 0.0,
            squash_x: Tween::new(1.0),
            squash_y: Tween::new(1.0),
            tilt: Tween::new(0.0),
            look_x: Tween::new(0.0),
            look_y: Tween::new(0.0),
            open: Tween::new(1.0),
            badge_scale: Tween::new(0.0),
            hands: Tween::new(0.0),
            hand_at: [Vec2::new(-0.43, 0.28), Vec2::new(0.43, 0.28)],
            hand_velocity: [Vec2::ZERO; 2],
            hand_angle: [0.0; 2],
            hand_length: [HAND_BALL; 2],
            tint_amount: Tween::new(0.0),
            tint_color: look.tint.0,
            mouth_curve: Tween::new(look.mouth_curve),
            mouth_open: Tween::new(0.0),
            sway: 0.0,
            sway_velocity: 0.0,
            hop: 0.0,
            hop_velocity: 0.0,
            next_hop: 0.0,
            blink_at: 1.5,
            blink_until: 0.0,
            shake_until: 0.0,
            reaction: Reaction::None,
            reaction_until: 0.0,
            pokes: 0,
            pokes_total: 0,
            last_poke: f64::MIN,
            spark_budget: 0.0,
            seed: 0x9e37_79b9,
            particles: Vec::new(),
            look,
            dance_face: None,
            breath: 0.0,
            talk: 1.0,
            badge: None,
            spin: 0.0,
            glance: Vec2::ZERO,
            glance_until: 0.0,
        }
    }
}

impl MagoMotion {
    /// A motion with its own luck: wizards side by side must not blink together.
    pub(crate) fn seeded(seed: u64) -> Self {
        Self { seed: (seed as u32 ^ (seed >> 32) as u32) | 1, ..Self::default() }
    }

    pub(crate) fn state(&self) -> MagoState {
        self.state
    }

    pub(crate) fn idle_for(&self, time: f64) -> f64 {
        time - self.last_time.unwrap_or(time)
    }

    fn random(&mut self) -> f32 {
        self.seed ^= self.seed << 13;
        self.seed ^= self.seed >> 17;
        self.seed ^= self.seed << 5;
        (self.seed % 10_000) as f32 / 10_000.0
    }

    /// Where a mini one looks: it never follows the pointer, it glances around, at a new place every 0.5 to 2 s
    /// (the mini bots: across within 0.88 of the middle, from 0.55 down to 0.45 up).
    pub(crate) fn glance(&mut self, time: f64) -> Vec2 {
        if time >= self.glance_until {
            self.glance = Vec2::new(-0.88 + self.random() * 1.76, -0.45 + self.random());
            self.glance_until = time + 0.5 + self.random() as f64 * 1.5;
        }
        self.glance
    }

    fn jump(&mut self, strength: f32) {
        if self.hop <= 0.0 {
            self.hop_velocity = strength;
            self.sway_velocity += 4.0;
        }
    }

    /// A click: annoyed at first, dizzy if it insists (4 within a short while).
    pub(crate) fn poke(&mut self, time: f64) {
        if time - self.last_poke > 1.5 {
            self.pokes = 0;
        }
        self.pokes += 1;
        self.pokes_total += 1;
        self.last_poke = time;
        if self.pokes >= 4 {
            self.reaction = Reaction::Dizzy;
            self.reaction_until = time + 2.4;
            self.shake_until = time + 2.4;
        } else {
            self.reaction = Reaction::Annoyed;
            self.reaction_until = time + 0.8;
            self.shake_until = time + 0.5;
            self.jump(1.4);
        }
    }

    pub(crate) fn pokes_total(&self) -> u32 {
        self.pokes_total
    }

    /// Spinning from too many pokes at `time`.
    pub(crate) fn dizzy(&self, time: f64) -> bool {
        self.reaction == Reaction::Dizzy && time < self.reaction_until
    }

    /// Working or speaking: the flame licks quicker and taller.
    pub(crate) fn lively(&self) -> bool {
        matches!(self.state, MagoState::Working | MagoState::Speaking)
    }

    /// The flame's tip as it sways, in figure units: the sparks come out of it.
    pub(crate) fn tip(&self) -> Vec2 {
        Vec2::new(self.sway * 0.04, FLAME_TIP)
    }

    fn emit(&mut self, kind: ParticleKind) {
        if self.particles.len() >= MAX_PARTICLES {
            return;
        }
        let (random_a, random_b) = (self.random(), self.random());
        let particle = match kind {
            ParticleKind::Spark => {
                let angle = random_a * std::f32::consts::TAU;
                let speed = 0.18 + random_b * 0.3;
                Particle {
                    at: self.tip(),
                    velocity: Vec2::new(angle.cos(), angle.sin() - 0.4) * speed,
                    age: 0.0,
                    life: 0.7 + random_b * 0.4,
                    kind,
                    spin: random_a,
                }
            }
            ParticleKind::Zzz => Particle { at: Vec2::new(-0.28, -0.25), velocity: Vec2::new(0.05, -0.1), age: 0.0, life: 2.2, kind, spin: random_a },
            ParticleKind::Sweat => Particle { at: Vec2::new(0.3, 0.1), velocity: Vec2::new(0.04, 0.05), age: 0.0, life: 1.0, kind, spin: random_a },
        };
        self.particles.push(particle);
    }

    fn enter(&mut self, state: MagoState, time: f64) {
        self.state = state;
        self.since = time;
        match state {
            MagoState::Done => {
                self.jump(2.4);
                for _ in 0..14 {
                    self.emit(ParticleKind::Spark);
                }
            }
            MagoState::Error => {
                self.shake_until = time + 0.8;
                self.emit(ParticleKind::Sweat);
            }
            MagoState::NeedsYou => self.next_hop = time + 0.15,
            _ => {}
        }
    }

    /// The face of the moment: the state's, unless a click reaction is on.
    fn current_look(&self) -> Look {
        let look = Look::of(self.state);
        match self.reaction {
            Reaction::Annoyed => Look { eye: EyeKind::Focus, mouth_curve: -0.03, badge: None, ..look },
            Reaction::Dizzy => Look { eye: EyeKind::Spiral, mouth_curve: 0.0, mouth_open: 0.03, mouth_width: 0.05, badge: None, ..look },
            Reaction::None => match self.dance_face {
                Some((eye, mouth_curve, mouth_open)) => Look { eye, mouth_curve, mouth_open, ..look },
                None => look,
            },
        }
    }

    /// Moves everything to `time`; `gaze` is where the eyes look (`gaze_toward`), none for a slow wander of their own.
    pub(crate) fn advance(&mut self, state: MagoState, time: f64, gaze: Option<Vec2>) {
        let first = self.last_time.is_none();
        let delta = self.last_time.map_or(0.0, |last| (time - last).clamp(0.0, 0.1)) as f32;
        self.last_time = Some(time);
        if first {
            self.blink_at = time + 1.0 + self.random() as f64 * 3.0;
        }
        if state != self.state || first {
            self.enter(state, time);
        }
        if time >= self.reaction_until && self.reaction != Reaction::None {
            self.reaction = Reaction::None;
        }
        self.look = self.current_look();
        self.spin += delta * 9.0;
        let look = self.look;

        // the eyes: the pointer if there is one, else a slow wander; blinking alone
        let gaze = gaze
            .filter(|gaze| gaze.x.is_finite() && gaze.y.is_finite())
            .map(|gaze| gaze.clamp(Vec2::splat(-1.0), Vec2::splat(1.0)))
            .unwrap_or_else(|| Vec2::new((time * 0.45).sin() as f32 * 0.5, (time * 0.31).cos() as f32 * 0.2));
        self.look_x.target = gaze.x;
        self.look_y.target = gaze.y;
        if time >= self.blink_at {
            self.blink_until = time + 0.12;
            self.blink_at = time + 2.4 + self.random() as f64 * 3.0;
        }
        self.open.target = if time < self.blink_until { 0.08 } else { 1.0 };

        self.tilt.target = look.tilt;
        if time < self.shake_until {
            let wobble = if self.reaction == Reaction::Dizzy { (time * 6.0).sin() * 0.12 } else { (time * 38.0).sin() * 0.07 };
            self.tilt.target += wobble as f32;
        }
        self.badge = look.badge;
        let wave = if self.state == MagoState::Waving {
            let elapsed = time - self.since;
            if (0.25..0.53).contains(&elapsed) {
                (elapsed - 0.25) / 0.28
            } else if (0.53..1.55).contains(&elapsed) {
                1.0
            } else if (1.55..1.75).contains(&elapsed) {
                1.0 - (elapsed - 1.55) / 0.2
            } else {
                0.0
            }
        } else {
            0.0
        };
        self.hands.target = wave.clamp(0.0, 1.0) as f32;
        self.badge_scale.target = if look.badge.is_some() { 1.0 } else { 0.0 };
        self.mouth_curve.target = look.mouth_curve;
        self.mouth_open.target = look.mouth_open;
        self.talk = if self.state == MagoState::Speaking { 0.5 + 0.5 * (time * 11.0).sin() as f32 * (time * 3.1).cos() as f32 } else { 1.0 };
        if look.tint.1 > 0.0 {
            self.tint_color = look.tint.0;
        }
        self.tint_amount.target = look.tint.1;
        let (sway_amplitude, sway_speed) = look.sway;
        let sway_target = if matches!(self.state, MagoState::Sleeping | MagoState::Error) { 0.7 } else { sway_amplitude * (time as f32 * sway_speed).sin() };
        self.breath = 0.022 * (time as f32 * if self.state == MagoState::Working { 4.0 } else { 2.0 }).sin();

        if first {
            self.snap();
            return;
        }
        self.squash_x.target = 1.0;
        self.squash_y.target = 1.0;
        for (tween, rate) in [
            (&mut self.squash_x, 14.0),
            (&mut self.squash_y, 14.0),
            (&mut self.tilt, 12.0),
            (&mut self.look_x, LOOK_RATE),
            (&mut self.look_y, LOOK_RATE),
            (&mut self.open, 40.0),
            (&mut self.badge_scale, 14.0),
            (&mut self.hands, 12.0),
            (&mut self.tint_amount, 5.0),
            (&mut self.mouth_curve, 10.0),
            (&mut self.mouth_open, 12.0),
        ] {
            tween.step(delta, rate);
        }

        // the flame's lean: a spring after its target, and kicked by the hops
        self.sway_velocity += (-70.0 * (self.sway - sway_target) - 7.0 * self.sway_velocity) * delta;
        self.sway = (self.sway + self.sway_velocity * delta).clamp(-1.2, 1.2);

        // the hop: up with a velocity, gravity down, a squash on landing
        if self.state == MagoState::NeedsYou && time >= self.next_hop {
            self.jump(1.5);
            self.next_hop = time + 1.7;
        }
        if self.hop > 0.0 || self.hop_velocity > 0.0 {
            self.hop_velocity -= GRAVITY * delta;
            self.hop += self.hop_velocity * delta;
            if self.hop <= 0.0 {
                self.hop = 0.0;
                if self.hop_velocity < -0.5 {
                    self.squash_y.value = 0.84;
                    self.squash_x.value = 1.1;
                    self.sway_velocity -= 3.0;
                }
                self.hop_velocity = 0.0;
            }
        }

        self.float_hands(time, delta);
        self.emit_over_time(time, delta);
        self.step_particles(delta);
    }

    /// The hands float after their targets with a little lag (a spring): balls at the sides at rest, a capsule
    /// swinging about a point near the body to wave (the right one when it needs you) or both up at the end.
    fn float_hands(&mut self, time: f64, delta: f32) {
        let beat = time as f32 * 13.0;
        for (index, side) in [-1.0f32, 1.0].into_iter().enumerate() {
            let gestures = !matches!(self.state, MagoState::NeedsYou | MagoState::Waving) || side > 0.0;
            let raise = if gestures { self.hands.value } else { 0.0 };
            let mut rest = Vec2::new(side * 0.43, 0.28);
            if self.state == MagoState::Working {
                // typing: in front and low, bobbing in turns
                rest = Vec2::new(side * 0.3, 0.42 + 0.03 * (beat * 0.9 + side * 1.7).sin());
            }
            let angle =
                if matches!(self.state, MagoState::NeedsYou | MagoState::Waving) { 0.8 + 0.35 * beat.sin() } else { 0.95 + 0.12 * (beat * 0.6 + side).sin() };
            // the spring holds the near end (the pivot, by the body) and the capsule turns about it
            let target = rest + (Vec2::new(side * 0.42, 0.15) - rest) * raise;
            self.hand_velocity[index] += ((self.hand_at[index] - target) * -110.0 - self.hand_velocity[index] * 12.0) * delta;
            self.hand_at[index] += self.hand_velocity[index] * delta;
            self.hand_angle[index] = angle;
            self.hand_length[index] = HAND_BALL + (HAND_CAPSULE - HAND_BALL) * raise;
        }
    }

    fn snap(&mut self) {
        for tween in
            [&mut self.tilt, &mut self.look_x, &mut self.look_y, &mut self.badge_scale, &mut self.tint_amount, &mut self.mouth_curve, &mut self.mouth_open]
        {
            tween.value = tween.target;
        }
    }

    fn emit_over_time(&mut self, time: f64, delta: f32) {
        let (kind, rate) = match self.state {
            MagoState::Done if time - self.since < 0.9 => (ParticleKind::Spark, 20.0),
            MagoState::Working => (ParticleKind::Spark, 3.0),
            MagoState::Sleeping => (ParticleKind::Zzz, 0.8),
            MagoState::Error => (ParticleKind::Sweat, 0.5),
            _ => return,
        };
        self.spark_budget += rate * delta;
        while self.spark_budget >= 1.0 {
            self.spark_budget -= 1.0;
            self.emit(kind);
        }
    }

    fn step_particles(&mut self, delta: f32) {
        for particle in &mut self.particles {
            particle.age += delta;
            match particle.kind {
                ParticleKind::Spark => {
                    particle.velocity *= 1.0 - 1.5 * delta;
                    particle.velocity.y += 0.35 * delta;
                }
                ParticleKind::Zzz => particle.velocity.x = 0.05 * (particle.age * 3.0 + particle.spin * 6.0).sin(),
                ParticleKind::Sweat => particle.velocity.y += 0.4 * delta,
            }
            particle.at += particle.velocity * delta;
        }
        self.particles.retain(|particle| particle.age < particle.life);
    }

    /// The two eyes (left, right) as the gaze turns them; none for one turned behind the body.
    pub(crate) fn eyes(&self) -> [Option<Eye>; 2] {
        let (yaw, pitch) = (self.look_x.value * LOOK_YAW, -self.look_y.value * LOOK_PITCH);
        [-1.0, 1.0].map(|side| {
            let (eye_yaw, eye_pitch) = (side * EYE_SPREAD + yaw, EYE_PITCH + pitch);
            let (across, upright) = (eye_yaw.cos(), eye_pitch.cos());
            (across * upright > 0.04).then(|| Eye {
                at: Vec2::new(eye_yaw.sin() * upright * BODY_HALF.0, BODY_CENTER - eye_pitch.sin() * BODY_HALF.1),
                flat: Vec2::new(across.max(0.18), upright.max(0.18)),
            })
        })
    }

    /// Where the mouth is and how flat it shows: under the eyes, between them, turning with the look as they do; none
    /// when the face is turned away.
    pub(crate) fn mouth(&self) -> Option<Eye> {
        let (yaw, pitch) = (self.look_x.value * LOOK_YAW, MOUTH_PITCH - self.look_y.value * LOOK_PITCH);
        let (across, upright) = (yaw.cos(), pitch.cos());
        (across * upright > 0.04).then(|| Eye {
            at: Vec2::new(yaw.sin() * upright * BODY_HALF.0, BODY_CENTER - pitch.sin() * BODY_HALF.1),
            flat: Vec2::new(across.max(0.18), upright.max(0.18)),
        })
    }

    /// Where the eyes look, as asked last (tests).
    #[cfg(test)]
    pub(crate) fn gaze(&self) -> Vec2 {
        Vec2::new(self.look_x.target, self.look_y.target)
    }
}

impl MagoMotion {
    /// A hand's two ends on the figure (near end, far end): the capsule points out and up from the pivot.
    pub(crate) fn hand_ends(&self, index: usize) -> (Vec2, Vec2) {
        let side = if index == 0 { -1.0 } else { 1.0 };
        let angle = self.hand_angle[index];
        let axis = Vec2::new(side * angle.cos(), -angle.sin());
        (self.hand_at[index], self.hand_at[index] + axis * (self.hand_length[index] - HAND_BALL))
    }
}
