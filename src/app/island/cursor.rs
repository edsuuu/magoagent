//! Where the cursor is on the whole screen, for the wizard's eyes: like Coucou's cursor poll (`island.rs`), they follow
//! it only while it is near the island; farther, they go back to wandering.

use crate::prelude::*;

/// How far from the island the cursor still draws the eyes (Coucou's `NEAR_ZONE`), in points.
pub(crate) const NEAR_ZONE: f32 = 420.0;

/// The cursor is near the island: within `NEAR_ZONE` of it (both in the same coordinates).
pub(crate) fn is_near(cursor: Pos2, island: Rect) -> bool {
    island.distance_to_pos(cursor) <= NEAR_ZONE
}

/// The cursor in the island window's own coordinates when it is near the island there, from the cursor on the screen
/// in pixels, the window's top left on the screen in points and the screen's scale.
pub(crate) fn eye_target(cursor: Pos2, scale: f32, window: Pos2, island: Rect) -> Option<Pos2> {
    let local = Pos2::new(cursor.x / scale - window.x, cursor.y / scale - window.y);
    (local.x.is_finite() && local.y.is_finite() && is_near(local, island)).then_some(local)
}

/// A margin around the island that still counts as on it, in points (Coucou's `HIT_MARGIN`).
const HIT_MARGIN: f32 = 14.0;

/// Whether the window takes the click: the cursor is on the island (a margin around it too), or a press is being held.
/// Anywhere else the click passes through to what is under the window.
pub(crate) fn captures(cursor: Option<Pos2>, island: Rect, holding: bool) -> bool {
    holding || cursor.is_some_and(|at| island.expand(HIT_MARGIN).contains(at))
}

/// The cursor on the screen, in pixels (Windows; elsewhere the egui pointer is all there is).
#[cfg(windows)]
pub(crate) fn screen_cursor() -> Option<Pos2> {
    #[link(name = "user32")]
    unsafe extern "system" {
        fn GetCursorPos(point: *mut [i32; 2]) -> i32;
    }
    let mut point = [0i32; 2];
    // SAFETY: a POINT (two i32) to fill in
    (unsafe { GetCursorPos(&mut point) } != 0).then(|| Pos2::new(point[0] as f32, point[1] as f32))
}

#[cfg(not(windows))]
pub(crate) fn screen_cursor() -> Option<Pos2> {
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn the_click_goes_through_outside_the_island_and_is_taken_on_it() {
        let island = Rect::from_min_size(Pos2::new(180.0, 1.0), Vec2::new(288.0, 32.0));
        assert!(captures(Some(Pos2::new(300.0, 20.0)), island, false), "on it");
        assert!(captures(Some(Pos2::new(300.0, 40.0)), island, false), "in the margin around it");
        assert!(!captures(Some(Pos2::new(300.0, 120.0)), island, false), "under it: through");
        assert!(!captures(Some(Pos2::new(40.0, 20.0)), island, false), "beside it: through");
        assert!(!captures(None, island, false), "no cursor, nothing to take");
        assert!(captures(Some(Pos2::new(300.0, 300.0)), island, true), "a press held on it is kept, wherever the cursor goes");
    }

    #[test]
    fn only_a_cursor_near_the_island_is_followed() {
        let island = Rect::from_min_size(Pos2::new(600.0, 1.0), Vec2::new(288.0, 32.0));
        assert!(is_near(Pos2::new(700.0, 20.0), island), "on it");
        assert!(is_near(Pos2::new(700.0, 400.0), island), "below it, within the zone");
        assert!(!is_near(Pos2::new(700.0, 600.0), island), "far below");
        assert!(!is_near(Pos2::new(100.0, 20.0), island), "far to the side");
        // a cursor on the screen at 1.5x, the window at 1000, 0 (points): in the window's coordinates
        assert_eq!(
            eye_target(Pos2::new(2400.0, 30.0), 1.5, Pos2::new(1000.0, 0.0), Rect::from_min_size(Pos2::new(180.0, 1.0), Vec2::new(288.0, 32.0))),
            Some(Pos2::new(600.0, 20.0))
        );
        assert_eq!(eye_target(Pos2::new(0.0, 3000.0), 1.0, Pos2::new(1000.0, 0.0), Rect::from_min_size(Pos2::new(180.0, 1.0), Vec2::new(288.0, 32.0))), None);
    }
}
