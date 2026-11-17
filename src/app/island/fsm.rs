//! When the island opens and closes: Coucou's state machine (island/fsm.ts, MIT), by the clock instead of timers.
//! Hidden is the closed notch, petit the compact island, home the open panel, and the greeting plays once at launch.
//! The pointer coming over the notch only peeks (compact): on a PC it lands there all the time on its way elsewhere.
//! A click on the compact island opens it; open, it folds a while after the pointer left ("Fechar auto"); compact, it
//! hides after a minute. With "Abrir ao passar o mouse" on, the pointer resting on the compact island opens it, and
//! such an island folds soon after the pointer leaves, until a click inside makes it an ordinary open one.

/// How long the compact island stays after the pointer left it, in seconds, until the person sets it
/// (`set_compact_after`).
const PETIT_STAYS: f64 = 60.0;
/// The greeting over, the island folds after this long; with the pointer on it, this long after it came.
const GREETING_STAYS: f64 = 0.6;
const GREETING_HELD: f64 = 10.0;
/// Open on hover: how long the pointer rests before it opens, and how soon it folds once the pointer left.
const HOVER_OPENS: f64 = 0.5;
const HOVER_CLOSES: f64 = 0.6;

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub(crate) enum Stage {
    #[default]
    Hidden,
    Petit,
    Home,
    Greeting,
}

#[derive(Default)]
pub(crate) struct Machine {
    stage: Stage,
    /// When the stage gives way by the clock.
    due: Option<f64>,
    /// When the pointer resting on the compact island opens it.
    hover_due: Option<f64>,
    /// Open because the pointer rested on it, until a click inside.
    by_hover: bool,
    inside: bool,
    /// Something waits on the person (a question, a form half typed): the clock does not close it.
    held: bool,
    close_after: f64,
    /// How long the compact island stays (Ajustes › Geral › Encolher para o notch); None, `PETIT_STAYS`; 0, forever.
    compact_after: Option<f64>,
    open_on_hover: bool,
}

impl Machine {
    pub(crate) fn stage(&self) -> Stage {
        self.stage
    }

    /// How long the compact island stays after the pointer left it, in seconds; 0, it stays until opened.
    pub(crate) fn set_compact_after(&mut self, seconds: u16) {
        self.compact_after = Some(f64::from(seconds));
    }

    /// The app started: the greeting.
    pub(crate) fn launch(&mut self) {
        self.clear();
        self.stage = Stage::Greeting;
    }

    /// Every frame, at `time`: whether the pointer is on the island (`inside`), whether something holds it open
    /// (`held`), and what the person set: how long the open island stays after the pointer left (`close_after`,
    /// seconds) and whether resting on it opens it.
    pub(crate) fn step(&mut self, time: f64, inside: bool, held: bool, close_after: f64, open_on_hover: bool) {
        if close_after != self.close_after {
            self.close_after = close_after;
            // changed while the countdown runs: it starts again with the new wait
            if self.stage == Stage::Home && self.due.is_some() && !self.by_hover {
                self.due = Some(time + close_after);
            }
        }
        self.open_on_hover = open_on_hover;
        if held != self.held {
            self.held = held;
            if held {
                self.clear();
            } else if !self.inside {
                self.rest(time);
            }
        }
        if inside != self.inside {
            self.inside = inside;
            if inside { self.entered(time) } else { self.left(time) }
        }
        if self.hover_due.is_some_and(|due| time >= due) {
            self.hover_due = None;
            if self.stage == Stage::Petit && !self.held && self.open_on_hover {
                self.enter(Stage::Home, time);
                self.by_hover = true;
            }
        }
        if self.due.is_some_and(|due| time >= due) && !self.held {
            self.by_hover = false;
            match self.stage {
                Stage::Petit => self.enter(Stage::Hidden, time),
                Stage::Home | Stage::Greeting => self.enter(Stage::Petit, time),
                Stage::Hidden => self.due = None,
            }
        }
    }

    /// A click on the small island opens it.
    pub(crate) fn click(&mut self, time: f64) {
        if matches!(self.stage, Stage::Hidden | Stage::Petit) {
            self.by_hover = false;
            self.enter(Stage::Home, time);
        }
    }

    /// A press inside the open island: one opened by the pointer resting on it now stays like any other.
    pub(crate) fn interacted(&mut self, time: f64) {
        if std::mem::take(&mut self.by_hover) && self.stage == Stage::Home && self.due.is_some() {
            self.due = Some(time + self.close_after);
        }
    }

    /// The greeting's animation ended: it folds in a moment, unless the pointer on it already set its own wait.
    pub(crate) fn greeted(&mut self, time: f64) {
        if self.stage == Stage::Greeting && self.due.is_none() {
            self.due = Some(time + GREETING_STAYS);
        }
    }

    /// Work started or ended somewhere: the hidden island shows compact for a while.
    pub(crate) fn reveal(&mut self, time: f64) {
        if self.stage == Stage::Hidden {
            self.enter(Stage::Petit, time);
        }
    }

    /// A question, or an explicit request (an agent opened in it): open at once.
    pub(crate) fn force_home(&mut self, time: f64) {
        self.by_hover = false;
        self.enter(Stage::Home, time);
    }

    /// Its close button, Esc, a question answered: compact at once.
    pub(crate) fn force_petit(&mut self, time: f64) {
        self.by_hover = false;
        self.enter(Stage::Petit, time);
    }

    /// How much of the open island's last seconds is left before it folds, from 1 down to 0: the bar under it shows in
    /// the last 60% of the wait (10 s at the most), as Coucou's.
    pub(crate) fn countdown(&self, time: f64) -> Option<f32> {
        let due = self.due.filter(|_| self.stage == Stage::Home && !self.held)?;
        let window = (self.close_after * 0.6).min(10.0);
        let left = due - time;
        (window > 0.0 && left < window).then(|| (left / window).clamp(0.0, 1.0) as f32)
    }

    fn clear(&mut self) {
        (self.due, self.hover_due) = (None, None);
    }

    fn enter(&mut self, stage: Stage, time: f64) {
        self.clear();
        self.stage = stage;
        if !self.inside {
            self.rest(time);
        }
    }

    /// The pointer is not on it: the clock that folds this stage starts.
    fn rest(&mut self, time: f64) {
        if self.held {
            return;
        }
        match self.stage {
            Stage::Petit => {
                let stays = self.compact_after.unwrap_or(PETIT_STAYS);
                self.due = (stays > 0.0).then_some(time + stays);
            }
            Stage::Home => self.due = Some(time + if self.by_hover { HOVER_CLOSES } else { self.close_after }),
            Stage::Hidden | Stage::Greeting => {}
        }
    }

    fn entered(&mut self, time: f64) {
        match self.stage {
            Stage::Hidden => {
                self.enter(Stage::Petit, time);
                self.rest_opens(time);
            }
            Stage::Petit => {
                self.due = None;
                self.rest_opens(time);
            }
            Stage::Home => self.due = None,
            Stage::Greeting => self.due = Some(time + GREETING_HELD),
        }
    }

    fn rest_opens(&mut self, time: f64) {
        if self.open_on_hover && !self.held {
            self.hover_due = Some(time + HOVER_OPENS);
        }
    }

    fn left(&mut self, time: f64) {
        self.hover_due = None;
        if self.stage == Stage::Greeting { self.enter(Stage::Petit, time) } else { self.rest(time) }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The machine stepped every 50 ms from `from` to `to` with the pointer `inside` or not; "Fechar auto" at 15 s.
    fn run(machine: &mut Machine, from: f64, to: f64, inside: bool, hover: bool) {
        let mut time = from;
        while time <= to {
            machine.step(time, inside, false, 15.0, hover);
            time += 0.05;
        }
    }

    #[test]
    fn the_pointer_only_peeks_and_the_click_opens() {
        let mut machine = Machine::default();
        run(&mut machine, 0.0, 1.0, false, false);
        assert_eq!(machine.stage(), Stage::Hidden);
        // the pointer comes and rests for five seconds: compact, never open
        run(&mut machine, 1.0, 6.0, true, false);
        assert_eq!(machine.stage(), Stage::Petit, "the pointer over it only peeks");
        machine.click(6.0);
        assert_eq!(machine.stage(), Stage::Home, "the click opens it");
        // under the pointer it stays, however long
        run(&mut machine, 6.0, 60.0, true, false);
        assert_eq!(machine.stage(), Stage::Home);
        // the pointer leaves: 15 s later it folds to compact, and a minute after that it hides
        run(&mut machine, 60.0, 74.9, false, false);
        assert_eq!(machine.stage(), Stage::Home, "it waits 15 s");
        assert!(machine.countdown(74.0).is_some_and(|left| left > 0.0 && left < 0.2), "the bar is nearly out: {:?}", machine.countdown(74.0));
        assert_eq!(machine.countdown(61.0), None, "and shows only in the last 9 s");
        run(&mut machine, 74.9, 75.2, false, false);
        assert_eq!(machine.stage(), Stage::Petit);
        run(&mut machine, 75.2, 134.9, false, false);
        assert_eq!(machine.stage(), Stage::Petit, "compact for a minute");
        run(&mut machine, 134.9, 135.5, false, false);
        assert_eq!(machine.stage(), Stage::Hidden);
    }

    #[test]
    fn the_island_uses_the_configured_thirty_second_auto_close() {
        let mut machine = Machine::default();
        machine.click(0.0);
        machine.step(0.0, false, false, 30.0, false);
        run_with_close_after(&mut machine, 0.0, 29.9, false, 30.0);
        assert_eq!(machine.stage(), Stage::Home, "still open before thirty seconds");
        run_with_close_after(&mut machine, 29.9, 30.2, false, 30.0);
        assert_eq!(machine.stage(), Stage::Petit, "folds at thirty seconds");
    }

    /// Asked ("tem um tempo para ele ficar menor? como eu deixo configurável"): the compact island shrinks to the notch
    /// when the person set, and with 0 never.
    #[test]
    fn the_compact_island_shrinks_when_the_person_set_and_never_with_zero() {
        for (seconds, shrinks) in [(15_u16, true), (0, false)] {
            let mut machine = Machine::default();
            machine.set_compact_after(seconds);
            run(&mut machine, 0.0, 0.2, true, false);
            assert_eq!(machine.stage(), Stage::Petit, "the pointer on the notch: compact");
            run(&mut machine, 0.2, 14.9, false, false);
            assert_eq!(machine.stage(), Stage::Petit, "{seconds}: still compact");
            run(&mut machine, 14.9, 15.6, false, false);
            assert_eq!(machine.stage() == Stage::Hidden, shrinks, "{seconds}: the notch at fifteen seconds, or never");
            run(&mut machine, 15.6, 400.0, false, false);
            assert_eq!(machine.stage(), if shrinks { Stage::Hidden } else { Stage::Petit });
        }
    }

    fn run_with_close_after(machine: &mut Machine, from: f64, to: f64, inside: bool, close_after: f64) {
        let mut time = from;
        while time <= to {
            machine.step(time, inside, false, close_after, false);
            time += 0.05;
        }
    }

    #[test]
    fn the_pointer_back_in_time_stops_the_clock() {
        let mut machine = Machine::default();
        run(&mut machine, 0.0, 0.1, true, false);
        machine.click(0.1);
        run(&mut machine, 0.1, 10.0, false, false);
        run(&mut machine, 10.0, 40.0, true, false);
        assert_eq!(machine.stage(), Stage::Home, "it came back before the 15 s: open still");
        run(&mut machine, 40.0, 50.0, false, false);
        assert_eq!(machine.stage(), Stage::Home, "and the wait starts over when it leaves again");
    }

    #[test]
    fn open_on_hover_opens_after_a_rest_and_folds_soon_until_a_click_inside() {
        let mut machine = Machine::default();
        run(&mut machine, 0.0, 0.3, true, true);
        assert_eq!(machine.stage(), Stage::Petit, "not at once");
        run(&mut machine, 0.3, 0.6, true, true);
        assert_eq!(machine.stage(), Stage::Home, "half a second resting on it");
        run(&mut machine, 0.6, 1.0, false, true);
        assert_eq!(machine.stage(), Stage::Home);
        run(&mut machine, 1.0, 1.4, false, true);
        assert_eq!(machine.stage(), Stage::Petit, "0.6 s after the pointer left");
        // opened by the hover again, then a click inside: an ordinary open island, it waits the 15 s
        run(&mut machine, 2.0, 3.0, true, true);
        assert_eq!(machine.stage(), Stage::Home);
        machine.interacted(3.0);
        run(&mut machine, 3.0, 17.0, false, true);
        assert_eq!(machine.stage(), Stage::Home);
        run(&mut machine, 17.0, 18.5, false, true);
        assert_eq!(machine.stage(), Stage::Petit);
    }

    #[test]
    fn something_waiting_on_the_person_holds_it_open() {
        let mut machine = Machine::default();
        machine.force_home(0.0);
        let mut time = 0.0;
        while time < 100.0 {
            machine.step(time, false, true, 15.0, false);
            time += 0.5;
        }
        assert_eq!(machine.stage(), Stage::Home, "a question keeps it open with no pointer on it");
        // answered: the usual wait starts
        run(&mut machine, 100.0, 114.0, false, false);
        assert_eq!(machine.stage(), Stage::Home);
        run(&mut machine, 114.0, 116.0, false, false);
        assert_eq!(machine.stage(), Stage::Petit);
        // its close button: compact at once, the pointer still on it
        machine.click(116.0);
        machine.step(116.0, true, false, 15.0, false);
        machine.force_petit(116.1);
        run(&mut machine, 116.1, 120.0, true, false);
        assert_eq!(machine.stage(), Stage::Petit);
    }

    #[test]
    fn the_greeting_folds_soon_after_it_ends_later_under_the_pointer_and_at_once_when_it_leaves() {
        let mut machine = Machine::default();
        machine.launch();
        run(&mut machine, 0.0, 4.6, false, false);
        assert_eq!(machine.stage(), Stage::Greeting, "it plays to the end");
        machine.greeted(4.6);
        machine.greeted(4.7);
        run(&mut machine, 4.6, 5.1, false, false);
        assert_eq!(machine.stage(), Stage::Greeting);
        run(&mut machine, 5.1, 5.3, false, false);
        assert_eq!(machine.stage(), Stage::Petit, "0.6 s after the end");
        // the pointer on it: ten seconds from when it came
        let mut machine = Machine::default();
        machine.launch();
        run(&mut machine, 0.0, 1.0, false, false);
        run(&mut machine, 1.0, 10.9, true, false);
        machine.greeted(4.6);
        assert_eq!(machine.stage(), Stage::Greeting);
        run(&mut machine, 10.9, 11.2, true, false);
        assert_eq!(machine.stage(), Stage::Petit);
        // the pointer leaves in the middle of it: compact at once
        let mut machine = Machine::default();
        machine.launch();
        run(&mut machine, 0.0, 1.0, true, false);
        run(&mut machine, 1.0, 1.1, false, false);
        assert_eq!(machine.stage(), Stage::Petit);
    }

    #[test]
    fn work_somewhere_shows_the_compact_island_for_a_minute() {
        let mut machine = Machine::default();
        machine.step(0.0, false, false, 15.0, false);
        machine.reveal(0.0);
        assert_eq!(machine.stage(), Stage::Petit);
        run(&mut machine, 0.0, 59.0, false, false);
        assert_eq!(machine.stage(), Stage::Petit);
        run(&mut machine, 59.0, 61.0, false, false);
        assert_eq!(machine.stage(), Stage::Hidden);
    }
}
