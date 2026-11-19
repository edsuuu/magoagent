//! How the island's size and corner get to where they are going: growing, on SwiftUI's
//! spring (response 0.5 s, damping 0.72: it overshoots a little and settles), stepped 240 times a second so that a slow
//! frame never shakes it; shrinking, along a curve in 340 ms (cubic-bezier .45, 0, .2, 1), which never overshoots.

use crate::prelude::*;

const RESPONSE: f32 = 0.5;
const DAMPING: f32 = 0.72;
const SUBSTEP: f32 = 1.0 / 240.0;
/// How long a shrink takes, in seconds.
pub(crate) const SHRINK: f64 = 0.34;

/// CSS's cubic-bezier(x1, y1, x2, y2) at `x`: how high the curve is where it is `x` across.
pub(crate) fn cubic_bezier(x1: f32, y1: f32, x2: f32, y2: f32, x: f32) -> f32 {
    let curve =
        |along: f32, first: f32, second: f32| (1.0 - along).powi(2) * 3.0 * along * first + 3.0 * (1.0 - along) * along * along * second + along.powi(3);

    let (mut low, mut high, mut along) = (0.0f32, 1.0f32, x);

    for _ in 0..12 {
        if curve(along, x1, x2) < x {
            low = along;
        } else {
            high = along;
        }

        along = (low + high) / 2.0;
    }

    curve(along, y1, y2)
}

/// The curve everything of the island shrinks along: 0 at the start, 1 at the end.
pub(crate) fn close_curve(progress: f32) -> f32 {
    if progress >= 1.0 { 1.0 } else { cubic_bezier(0.45, 0.0, 0.2, 1.0, progress.max(0.0)) }
}

/// The island's width, height and corner on their way to a target.
#[derive(Default)]
pub(crate) struct Geometry {
    value: [f32; 3],
    velocity: [f32; 3],
    target: [f32; 3],
    /// Shrinking: from where, and since when.
    curve: Option<([f32; 3], f64)>,
    moving: bool,
    placed: bool,
}

impl Geometry {
    pub(crate) fn size(&self) -> Vec2 {
        Vec2::new(self.value[0], self.value[1])
    }

    pub(crate) fn corner(&self) -> f32 {
        self.value[2]
    }

    pub(crate) fn moving(&self) -> bool {
        self.moving
    }

    /// There at once (the first time it shows, and while an edge is pulled).
    pub(crate) fn jump(&mut self, size: Vec2, corner: f32) {
        self.target = [size.x, size.y, corner];
        (self.value, self.velocity, self.curve, self.moving, self.placed) = (self.target, [0.0; 3], None, false, true);
    }

    /// A new target: one smaller than the last is shrunk to along the curve, from `time`; any other is sprung to.
    pub(crate) fn aim(&mut self, size: Vec2, corner: f32, time: f64) {
        let target = [size.x, size.y, corner];

        if !self.placed {
            return self.jump(size, corner);
        }

        if target == self.target {
            return;
        }

        let shrinking = target[0] + target[1] < self.target[0] + self.target[1];

        self.target = target;
        self.moving = true;

        if shrinking {
            (self.curve, self.velocity) = (Some((self.value, time)), [0.0; 3]);
        } else {
            self.curve = None;
        }
    }

    /// `delta` seconds on, at `time`.
    pub(crate) fn step(&mut self, delta: f32, time: f64) {
        if !self.moving {
            return;
        }

        if let Some((from, start)) = self.curve {
            let progress = ((time - start) / SHRINK) as f32;
            let eased = close_curve(progress);

            for index in 0..3 {
                self.value[index] = from[index] + (self.target[index] - from[index]) * eased;
            }

            if progress >= 1.0 {
                (self.value, self.curve, self.moving) = (self.target, None, false);
            }

            return;
        }

        let omega = std::f32::consts::TAU / RESPONSE;
        let steps = (delta / SUBSTEP).ceil().max(1.0);
        let each = delta / steps;

        for _ in 0..steps as usize {
            for index in 0..3 {
                let pull = omega * omega * (self.target[index] - self.value[index]) - 2.0 * DAMPING * omega * self.velocity[index];

                self.velocity[index] += pull * each;
                self.value[index] += self.velocity[index] * each;
            }
        }

        if (0..3).all(|index| (self.target[index] - self.value[index]).abs() < 0.02 && self.velocity[index].abs() < 0.2) {
            (self.value, self.velocity, self.moving) = (self.target, [0.0; 3], false);
        }
    }
}

/// One value on the same spring: the island's wizard, its place and size between the modes.
#[derive(Clone, Copy, Default)]
pub(crate) struct Spring {
    pub(crate) value: f32,
    pub(crate) target: f32,
    velocity: f32,
}

impl Spring {
    pub(crate) fn settled(&self) -> bool {
        (self.target - self.value).abs() < 0.01 && self.velocity.abs() < 0.05
    }

    pub(crate) fn step(&mut self, delta: f32) {
        let omega = std::f32::consts::TAU / RESPONSE;
        let steps = (delta / SUBSTEP).ceil().max(1.0);
        let each = delta / steps;

        for _ in 0..steps as usize {
            self.velocity += (omega * omega * (self.target - self.value) - 2.0 * DAMPING * omega * self.velocity) * each;
            self.value += self.velocity * each;
        }

        if self.settled() {
            (self.value, self.velocity) = (self.target, 0.0);
        }
    }
}

/// CSS's timing keywords.
pub(crate) fn ease_in(progress: f32) -> f32 {
    cubic_bezier(0.42, 0.0, 1.0, 1.0, progress)
}

pub(crate) fn ease_out(progress: f32) -> f32 {
    cubic_bezier(0.0, 0.0, 0.58, 1.0, progress)
}

pub(crate) fn ease_in_out(progress: f32) -> f32 {
    cubic_bezier(0.42, 0.0, 0.58, 1.0, progress)
}

/// One way of a CSS `transition`: how long, after how long, along which curve.
#[derive(Clone, Copy)]
pub(crate) struct Way {
    pub(crate) seconds: f64,
    pub(crate) delay: f64,
    pub(crate) ease: fn(f32) -> f32,
}

/// An opacity that follows its `on` as a CSS transition does: from wherever it is when it flips, `enter` toward 1 and
/// `leave` toward 0 (the reference's style.css, e.g. `.view.on { transition: opacity .3s ease-out .16s }`).
#[derive(Clone, Copy)]
pub(crate) struct Fade {
    on: bool,
    from: f32,
    since: f64,
    enter: Way,
    leave: Way,
}

impl Fade {
    pub(crate) const fn new(enter: Way, leave: Way) -> Self {
        Fade { on: false, from: 0.0, since: f64::NEG_INFINITY, enter, leave }
    }

    fn way(&self) -> Way {
        if self.on { self.enter } else { self.leave }
    }

    /// How far along it is, 0 to 1, at `time`.
    pub(crate) fn progress(&self, time: f64) -> f32 {
        let way = self.way();

        ((time - self.since - way.delay) / way.seconds).clamp(0.0, 1.0) as f32
    }

    pub(crate) fn value(&self, time: f64) -> f32 {
        let (to, progress) = (if self.on { 1.0 } else { 0.0 }, self.progress(time));

        if progress >= 1.0 {
            return to;
        }

        self.from + (to - self.from) * (self.way().ease)(progress)
    }

    pub(crate) fn set(&mut self, on: bool, time: f64) {
        if on != self.on {
            self.from = self.value(time);
            (self.on, self.since) = (on, time);
        }
    }

    /// There or gone at once (the entrance's page: the reference shows its canvas and takes it away, no transition).
    pub(crate) fn jump(&mut self, on: bool) {
        (self.on, self.from, self.since) = (on, 0.0, f64::NEG_INFINITY);
    }

    /// From nothing again, as a view that comes in (a page that changed).
    pub(crate) fn restart(&mut self, time: f64) {
        (self.on, self.from, self.since) = (true, 0.0, time);
    }

    pub(crate) fn moving(&self, time: f64) -> bool {
        let way = self.way();

        time < self.since + way.delay + way.seconds
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_fade_waits_its_delay_and_turns_back_from_where_it_is() {
        let way = Way { seconds: 0.3, delay: 0.16, ease: ease_out };
        let mut fade = Fade::new(way, Way { seconds: 0.16, delay: 0.0, ease: ease_in });

        assert_eq!(fade.value(0.0), 0.0);
        fade.set(true, 1.0);
        assert_eq!(fade.value(1.15), 0.0, "the delay first");
        assert!(fade.value(1.31) > 0.4 && fade.moving(1.31));
        assert_eq!((fade.value(1.5), fade.moving(1.5)), (1.0, false));
        fade.set(true, 1.6);
        assert_eq!(fade.value(1.6), 1.0, "set again, nothing changes");
        fade.set(false, 2.0);
        let middle = fade.value(2.08);

        assert!(middle > 0.0 && middle < 1.0);
        fade.set(true, 2.08);
        assert!((fade.value(2.08) - middle).abs() < 1e-6, "no jump when it turns back");
    }

    #[test]
    fn the_wizards_spring_goes_a_little_past_and_settles() {
        let mut spring = Spring { value: 0.0, target: 20.0, velocity: 0.0 };
        let mut most = 0.0f32;

        for _ in 0..120 {
            spring.step(1.0 / 60.0);
            most = most.max(spring.value);
        }

        assert!(most > 20.3 && most < 21.5, "{most}");
        assert!(spring.settled() && spring.value == 20.0);
    }

    /// The geometry after each 1/60 s from 0 to `seconds`.
    fn run(geometry: &mut Geometry, seconds: f64) -> Vec<Vec2> {
        (1..=(seconds * 60.0).round() as usize)
            .map(|frame| {
                geometry.step(1.0 / 60.0, frame as f64 / 60.0);
                geometry.size()
            })
            .collect()
    }

    #[test]
    fn growing_it_overshoots_a_little_on_the_spring_and_settles() {
        let mut geometry = Geometry::default();

        geometry.aim(Vec2::new(288.0, 32.0), 14.0, 0.0);
        assert_eq!(geometry.size(), Vec2::new(288.0, 32.0), "the first size is taken at once");
        geometry.aim(Vec2::new(640.0, 160.0), 22.0, 0.0);
        let sizes = run(&mut geometry, 2.0);
        let widest = sizes.iter().map(|size| size.x).fold(0.0, f32::max);

        // SwiftUI's spring at 0.72 of damping goes about 4% past: 352 points of travel, some 14 over
        assert!(widest > 645.0 && widest < 665.0, "it goes a little past 640 and comes back: {widest}");
        assert!(sizes[14].x > 500.0 && sizes[14].x < 640.0, "a quarter of a second in it is most of the way: {:?}", sizes[14]);
        assert_eq!((geometry.size(), geometry.corner(), geometry.moving()), (Vec2::new(640.0, 160.0), 22.0, false), "settled on the target");
    }

    #[test]
    fn shrinking_it_follows_the_curve_for_340_ms_and_never_goes_past() {
        let mut geometry = Geometry::default();

        geometry.jump(Vec2::new(640.0, 160.0), 22.0);
        geometry.aim(Vec2::new(288.0, 32.0), 14.0, 0.0);
        let sizes = run(&mut geometry, 0.5);

        assert!(sizes.windows(2).all(|pair| pair[1].x <= pair[0].x && pair[1].y <= pair[0].y), "only ever smaller");
        assert!(sizes.iter().all(|size| size.x >= 288.0 && size.y >= 32.0), "never past the target: no overshoot");
        // halfway in time (170 ms) the curve is at 0.75 of the way: slow to start, quick in the middle
        let halfway = sizes[9].x;

        assert!((halfway - (640.0 - 352.0 * close_curve(10.0 / 60.0 / 0.34))).abs() < 0.01, "{halfway}");
        assert!(sizes[19].x > 288.0, "at 333 ms it is not there yet");
        assert_eq!(sizes[20], Vec2::new(288.0, 32.0), "at 350 ms it is: 340 ms it takes");
        assert!(!geometry.moving());
    }

    #[test]
    fn the_close_curve_is_csss_cubic_bezier() {
        assert_eq!((close_curve(0.0), close_curve(1.0), close_curve(2.0)), (0.0, 1.0, 1.0));
        // cubic-bezier(.45, 0, .2, 1): values worked out from the curve itself
        assert!((close_curve(0.5) - 0.762).abs() < 0.01, "{}", close_curve(0.5));
        assert!(close_curve(0.1) < 0.03 && close_curve(0.9) > 0.985);
    }

    #[test]
    fn a_target_changed_on_the_way_is_taken_from_where_it_is() {
        let mut geometry = Geometry::default();

        geometry.jump(Vec2::new(288.0, 32.0), 14.0);
        geometry.aim(Vec2::new(640.0, 160.0), 22.0, 0.0);
        run(&mut geometry, 0.1);
        let there = geometry.size();

        // it is told to close while it opens: from where it is, down, with no jump
        geometry.aim(Vec2::new(288.0, 32.0), 14.0, 0.1);
        geometry.step(1.0 / 60.0, 0.1 + 1.0 / 60.0);
        assert!((geometry.size().x - there.x).abs() < 8.0 && geometry.size().x <= there.x, "{there:?} → {:?}", geometry.size());
    }
}
