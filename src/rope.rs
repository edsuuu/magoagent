//! Ropes: the connection lines hang and swing like a real rope under gravity.

use crate::prelude::*;

const ROPE_SEGMENTS: usize = 20;
const ROPE_GRAVITY: f32 = 1600.0;
const ROPE_STEP: f32 = 1.0 / 60.0;
const ROPE_DAMPING: f32 = 0.965;
/// The rope is this much longer than the gap between its ends, so it always hangs a little.
const ROPE_SLACK: f32 = 1.05;
const ROPE_ITERATIONS: usize = 8;
const ROPE_CLEARANCE: f32 = 20.0;
/// With its ends at rest a rope sleeps after this many seconds, settled or not.
const ROPE_MAX_AWAKE: f32 = 10.0;

/// A rope as a chain of points moved by Verlet integration (position minus last position is
/// the velocity), the same technique the reference app's ropes use.
pub(crate) struct RopeSim {
    pub(crate) points: Vec<Pos2>,
    previous: Vec<Pos2>,
    ends: (Pos2, Pos2),
    pending: f32,
    still: bool,
    awake: f32,
    /// The windows and notes it went around last time: a new one, or one that moved, wakes it.
    around: Vec<Rect>,
}

impl RopeSim {
    /// Already at rest under its own weight.
    pub(crate) fn hanging(a: Pos2, b: Pos2) -> Self {
        let points: Vec<Pos2> = resample(&rope_curve([a.x, a.y], [b.x, b.y]), ROPE_SEGMENTS + 1).into_iter().map(Pos2::from).collect();
        Self { previous: points.clone(), points, ends: (a, b), pending: 0.0, still: true, awake: 0.0, around: Vec::new() }
    }

    /// Fixed 1/60 s steps (at most four a frame); returns whether it is still moving. A still
    /// rope whose ends and obstacles didn't move sleeps, and so does one awake for `ROPE_MAX_AWAKE`.
    // ponytail: any obstacle that moves wakes every rope for a step, not only those within 80 of it
    pub(crate) fn advance(&mut self, a: Pos2, b: Pos2, dt: f32, obstacles: &[Rect]) -> bool {
        if (a, b) != self.ends || obstacles != self.around.as_slice() {
            self.ends = (a, b);
            self.around = obstacles.to_vec();
            self.still = false;
            self.awake = 0.0;
        }
        if self.still {
            return false;
        }
        self.pending = (self.pending + dt).min(4.0 * ROPE_STEP);
        while self.pending >= ROPE_STEP {
            self.pending -= ROPE_STEP;
            self.awake += ROPE_STEP;
            self.still = self.step(a, b, obstacles) < 0.5 || self.awake >= ROPE_MAX_AWAKE;
        }
        !self.still
    }

    fn step(&mut self, a: Pos2, b: Pos2, obstacles: &[Rect]) -> f32 {
        let last = self.points.len() - 1;
        let fall = Vec2::new(0.0, ROPE_GRAVITY * ROPE_STEP * ROPE_STEP);
        for i in 1..last {
            let velocity = (self.points[i] - self.previous[i]) * ROPE_DAMPING;
            self.previous[i] = self.points[i];
            self.points[i] += velocity + fall;
        }
        let link = (a.distance(b) * ROPE_SLACK).max(40.0) / last as f32;
        for _ in 0..ROPE_ITERATIONS {
            self.points[0] = a;
            self.points[last] = b;
            for i in 0..last {
                let delta = self.points[i + 1] - self.points[i];
                let length = delta.length();
                if length < 1e-3 {
                    continue;
                }
                let fix = delta * ((length - link) / length * 0.5);
                if i != 0 {
                    self.points[i] += fix;
                }
                if i + 1 != last {
                    self.points[i + 1] -= fix;
                }
            }
            for point in &mut self.points[1..last] {
                for rect in obstacles {
                    let around = rect.expand(ROPE_CLEARANCE);
                    if around.contains(*point) {
                        // out through the nearest side, eased so it slides instead of jumping
                        let exits = [
                            (point.x - around.left(), Vec2::new(around.left() - point.x, 0.0)),
                            (around.right() - point.x, Vec2::new(around.right() - point.x, 0.0)),
                            (point.y - around.top(), Vec2::new(0.0, around.top() - point.y)),
                            (around.bottom() - point.y, Vec2::new(0.0, around.bottom() - point.y)),
                        ];
                        let (_, push) = exits.into_iter().min_by(|x, y| x.0.total_cmp(&y.0)).unwrap();
                        *point += push * 0.4;
                    }
                }
            }
        }
        self.points[0] = a;
        self.points[last] = b;
        (1..last).map(|i| (self.points[i] - self.previous[i]).length()).sum()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_rope_sleeps_after_ten_seconds_awake_until_an_end_moves() {
        let (a, b) = (Pos2::new(0.0, 0.0), Pos2::new(300.0, 200.0));
        let mut rope = RopeSim::hanging(a, Pos2::new(300.0, 0.0));
        // a jolt wakes it with its clock at zero
        assert!(rope.advance(a, b, ROPE_STEP, &[]));
        assert_eq!(rope.awake, ROPE_STEP);
        // still swinging at the limit: the next step puts it to sleep, and it stays asleep
        rope.awake = ROPE_MAX_AWAKE - ROPE_STEP / 2.0;
        assert!(!rope.advance(a, b, ROPE_STEP, &[]));
        assert!(!rope.advance(a, b, ROPE_STEP, &[]));
        assert!(rope.advance(a, Pos2::new(300.0, 250.0), ROPE_STEP, &[]), "an end moving wakes it");
    }
}
