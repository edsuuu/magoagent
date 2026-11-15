//! Where the cursor is on the whole screen, for the wizard's eyes: as Coucou's cursor poll (`island.rs`), they follow
//! it wherever it is, with no zone around the island; and whether the island's window takes the click.

use crate::prelude::*;
use std::sync::OnceLock;

/// The cursor in the island window's own coordinates, from the cursor on the screen in pixels, the window's top left
/// on the screen in points and the screen's scale: where the eyes look, however far it is.
pub(crate) fn eye_target(cursor: Pos2, scale: f32, window: Pos2) -> Option<Pos2> {
    let local = Pos2::new(cursor.x / scale - window.x, cursor.y / scale - window.y);
    (local.x.is_finite() && local.y.is_finite()).then_some(local)
}

/// A margin around the island that still counts as on it, in points (Coucou's `HIT_MARGIN`).
pub(crate) const HIT_MARGIN: f32 = 14.0;

/// Whether the window takes the click: the cursor is on the island (a margin around it too), or a press is being held.
/// Anywhere else the click passes through to what is under the window.
pub(crate) fn captures(cursor: Option<Pos2>, island: Rect, holding: bool) -> bool {
    holding || cursor.is_some_and(|at| island.expand(HIT_MARGIN).contains(at))
}

/// "x,y" as a place on the screen, in pixels.
pub(crate) fn parse_cursor(text: &str) -> Option<Pos2> {
    let (x, y) = text.split_once(',')?;
    Some(Pos2::new(x.trim().parse().ok()?, y.trim().parse().ok()?))
}

/// The cursor on the screen, in pixels (Windows; elsewhere the egui pointer is all there is). In a test scene
/// (`MAGOAGENT_SCENE`), `MAGOAGENT_FAKE_CURSOR=x,y` stands in for it: the eyes and the hover are seen without the mouse.
pub(crate) fn screen_cursor() -> Option<Pos2> {
    static FAKE: OnceLock<Option<Pos2>> = OnceLock::new();
    let fake =
        FAKE.get_or_init(|| std::env::var_os("MAGOAGENT_SCENE").and_then(|_| std::env::var("MAGOAGENT_FAKE_CURSOR").ok()).and_then(|text| parse_cursor(&text)));
    fake.or_else(system_cursor)
}

#[cfg(windows)]
fn system_cursor() -> Option<Pos2> {
    #[link(name = "user32")]
    unsafe extern "system" {
        fn GetCursorPos(point: *mut [i32; 2]) -> i32;
    }
    let mut point = [0i32; 2];
    // SAFETY: a POINT (two i32) to fill in
    (unsafe { GetCursorPos(&mut point) } != 0).then(|| Pos2::new(point[0] as f32, point[1] as f32))
}

#[cfg(not(windows))]
fn system_cursor() -> Option<Pos2> {
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
    fn the_eyes_have_the_cursor_wherever_it_is_on_the_screen() {
        // a cursor on the screen at 1.5x, the window at 1000, 0 (points): in the window's coordinates
        assert_eq!(eye_target(Pos2::new(2400.0, 30.0), 1.5, Pos2::new(1000.0, 0.0)), Some(Pos2::new(600.0, 20.0)));
        // far from the island, on another monitor to the left: followed all the same
        assert_eq!(eye_target(Pos2::new(-1900.0, 3000.0), 1.0, Pos2::new(1000.0, 0.0)), Some(Pos2::new(-2900.0, 3000.0)));
        assert_eq!(eye_target(Pos2::new(f32::NAN, 0.0), 1.0, Pos2::ZERO), None);
        // the cursor a test puts in the real one's place
        assert_eq!(parse_cursor("1200, 300"), Some(Pos2::new(1200.0, 300.0)));
        assert_eq!(parse_cursor("-40,8.5"), Some(Pos2::new(-40.0, 8.5)));
        assert_eq!(parse_cursor("aqui"), None);
    }
}

/// Windows: a thread that reads the cursor every 16 ms, as the reference's poll (src-tauri `island.rs`), and does
/// nothing else until the cursor crosses the island's room on the screen (pixels, `room`): then the window takes the
/// click at once and the island is drawn again. The frames alone come at 250 ms while the cursor is away, and a press
/// that came sooner went through the island to the window under it.
#[cfg(windows)]
pub(crate) fn watch(ctx: &Context) -> std::sync::Arc<std::sync::Mutex<Option<Rect>>> {
    let room: std::sync::Arc<std::sync::Mutex<Option<Rect>>> = Default::default();
    let (shared, ctx) = (room.clone(), ctx.clone());
    std::thread::spawn(move || {
        let mut inside = false;
        loop {
            std::thread::sleep(std::time::Duration::from_millis(16));
            let Some(room) = *shared.lock().unwrap_or_else(|poisoned| poisoned.into_inner()) else { continue };
            let now = system_cursor().is_some_and(|at| room.contains(at));
            if now != inside {
                inside = now;
                if now {
                    crate::app::voice::popup::plain::click_through("Ilha do Maestro", false);
                }
                ctx.request_repaint();
            }
        }
    });
    room
}
