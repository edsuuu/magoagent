//! Luna as the island draws her, kept so the island at rest draws her again at each frame's time (`replay::live`): what
//! she dances to is read again then, and where the cursor is.

use crate::prelude::*;

use super::cursor;
use crate::app::integrations::spotify::groove::{Dance, Ear, Groove};
use crate::app::mascot::mago::{LUME, MagoState, mago_pokes, paint_mago_dancing, paint_mago_toward};

/// Luna in her `state`: while a song plays, no agent wants the person (`calm`) and she is not dizzy, she dances to it.
#[derive(Clone)]
pub(super) struct Luna {
    /// The listening and whether the song plays; None, she does not dance ("A Luna dança com a música" off, nothing
    /// loaded in Spotify).
    ear: Option<(Ear, bool)>,
    calm: bool,
    /// Her colour while she dances (`luna_colour`).
    colour: Color32,
    state: MagoState,
    /// Where her eyes look: the cursor on the screen, read at the frame's time with the screen's scale and the window's
    /// place (`eyes`); else the cursor known when the app drew her (`pointer`), or with `window_pointer`, the window's.
    eyes: Option<(f32, Pos2)>,
    pointer: Option<Pos2>,
    window_pointer: bool,
}

impl Luna {
    /// Luna in `state` at the cursor `pointer` (the window's own one too with `window_pointer`).
    pub(super) fn with(mut self, state: MagoState, pointer: Option<Pos2>, window_pointer: bool) -> Self {
        (self.state, self.pointer, self.window_pointer) = (state, pointer, window_pointer);
        self
    }

    /// Whether she dances now: the song heard, unless it went quiet.
    pub(super) fn dances(&self, time: f64) -> bool {
        self.groove(time).is_some()
    }

    /// What she dances to now, if she dances.
    fn groove(&self, time: f64) -> Option<Groove> {
        let dizzy = mago_pokes("island-main", time).1;
        let (ear, playing) = self.ear.as_ref()?;
        Some(ear.groove(*playing)).filter(|groove| self.calm && groove.dance != Dance::Nap && !dizzy)
    }

    /// Draws her `size` tall at `center` at `time`.
    pub(super) fn paint(&self, painter: &Painter, center: Pos2, size: f32, time: f64) {
        match self.groove(time) {
            Some(groove) => paint_mago_dancing(painter, "island-main-dance", center, size, self.colour, &groove, time),
            None => {
                let pointer = self.eyes.and_then(|(scale, origin)| cursor::eye_cursor().and_then(|at| cursor::eye_target(at, scale, origin))).or(self.pointer);
                let pointer = if self.window_pointer { pointer.or_else(|| painter.ctx().pointer_latest_pos()) } else { pointer };
                paint_mago_toward(painter, "island-main", center, size, self.state, time, pointer);
            }
        }
    }
}

impl DrawingApp {
    /// Luna as the island draws her now, `calm` when no agent wants the person (`dances_through`).
    pub(super) fn island_luna(&self, calm: bool) -> Luna {
        let spotify = &self.integrations.spotify;
        let ear = spotify.now.as_ref().filter(|_| self.prefs.integrations.luna_dances).map(|now| (spotify.dancer.ear(), now.playing));
        Luna { ear, calm, colour: self.luna_colour(LUME), state: MagoState::Idle, eyes: self.island.eyes, pointer: None, window_pointer: false }
    }
}
