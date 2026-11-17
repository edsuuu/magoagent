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

/// `captures` for the cursor `read` gives now (on the screen, in pixels), `scale` and `origin` taking it to the window's
/// points. Off Windows and macOS the window never passes the click through (`MousePassthrough` is the egui way there).
pub(crate) fn takes_click(read: impl Fn() -> Option<Pos2>, scale: f32, origin: Pos2, island: Rect, holding: bool) -> bool {
    !cfg!(any(windows, target_os = "macos"))
        || captures(read().map(|at| Pos2::new(at.x / scale - origin.x, at.y / scale - origin.y)), island, holding)
}

/// "x,y" as a place on the screen, in pixels.
pub(crate) fn parse_cursor(text: &str) -> Option<Pos2> {
    let (x, y) = text.split_once(',')?;
    Some(Pos2::new(x.trim().parse().ok()?, y.trim().parse().ok()?))
}

/// An AppKit point (`[x, y]`, origin bottom-left, in points) to top-left screen pixels: `desktop_top` is the highest
/// `frame.maxY` among the screens, `scale` the backing scale of the screen that holds the point.
pub(crate) fn cocoa_to_screen_pixels(point: [f32; 2], desktop_top: f32, scale: f32) -> Pos2 {
    Pos2::new(point[0] * scale, (desktop_top - point[1]) * scale)
}

/// Whether `point` (AppKit, bottom-left) lies in `frame` `[x, y, width, height]`.
pub(crate) fn cocoa_contains(point: [f32; 2], frame: [f32; 4]) -> bool {
    let [x, y, width, height] = frame;
    point[0] >= x && point[0] < x + width && point[1] >= y && point[1] < y + height
}

/// Of `frames` `[x, y, width, height]` (AppKit), the one that holds `point`, else the nearest by centre.
pub(crate) fn cocoa_screen_at(point: [f32; 2], frames: &[[f32; 4]]) -> Option<[f32; 4]> {
    frames.iter().copied().find(|frame| cocoa_contains(point, *frame)).or_else(|| {
        frames.iter().copied().min_by(|a, b| {
            let dist = |frame: [f32; 4]| {
                let centre = [frame[0] + frame[2] / 2.0, frame[1] + frame[3] / 2.0];
                (centre[0] - point[0]).hypot(centre[1] - point[1])
            };
            dist(*a).total_cmp(&dist(*b))
        })
    })
}

/// An AppKit frame `[x, y, width, height]` to a top-left `[left, top, right, bottom]` in pixels (`desktop_top` and
/// `scale` as in `cocoa_to_screen_pixels`).
pub(crate) fn cocoa_frame_pixels(frame: [f32; 4], desktop_top: f32, scale: f32) -> [i32; 4] {
    let [x, y, width, height] = frame;
    let left = (x * scale).round() as i32;
    let top = ((desktop_top - (y + height)) * scale).round() as i32;
    let right = ((x + width) * scale).round() as i32;
    let bottom = ((desktop_top - y) * scale).round() as i32;
    [left, top, right, bottom]
}

/// The cursor on the screen, in pixels (Windows and macOS; elsewhere the egui pointer is all there is). In a test scene
/// (`MAGOAGENT_SCENE`), `MAGOAGENT_FAKE_CURSOR=x,y` stands in for it: the eyes and the hover are seen without the mouse.
pub(crate) fn screen_cursor() -> Option<Pos2> {
    screen_cursor_fake().or_else(system_cursor)
}

/// The cursor on the screen for the mascot's eyes: `screen_cursor`, and on X11 the X server's (only for the eyes: under
/// XWayland it is known only over the windows of X11, so it never says whether the cursor is on the island; off them
/// there is none and the eyes look around by themselves).
pub(crate) fn eye_cursor() -> Option<Pos2> {
    #[cfg(target_os = "linux")]
    if crate::app::island::x11::active() {
        return screen_cursor_fake().or_else(crate::app::island::x11::cursor);
    }
    screen_cursor()
}

/// The cursor a test puts in the real one's place (`MAGOAGENT_FAKE_CURSOR`, in a test scene).
fn screen_cursor_fake() -> Option<Pos2> {
    static FAKE: OnceLock<Option<Pos2>> = OnceLock::new();
    *FAKE.get_or_init(|| std::env::var_os("MAGOAGENT_SCENE").and_then(|_| std::env::var("MAGOAGENT_FAKE_CURSOR").ok()).and_then(|text| parse_cursor(&text)))
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

#[cfg(target_os = "macos")]
fn system_cursor() -> Option<Pos2> {
    let mtm = objc2::MainThreadMarker::new()?;
    let point = objc2_app_kit::NSEvent::mouseLocation();
    let point = [point.x as f32, point.y as f32];
    let screens = objc2_app_kit::NSScreen::screens(mtm);
    let mut desktop_top = 0.0f32;
    let mut frames = Vec::with_capacity(screens.len());
    for screen in screens.iter() {
        let frame = screen.frame();
        let rect = [frame.origin.x as f32, frame.origin.y as f32, frame.size.width as f32, frame.size.height as f32];
        desktop_top = desktop_top.max(rect[1] + rect[3]);
        frames.push((rect, screen.backingScaleFactor() as f32));
    }
    let scale = cocoa_screen_at(point, &frames.iter().map(|(rect, _)| *rect).collect::<Vec<_>>())
        .and_then(|want| frames.iter().find(|(rect, _)| *rect == want).map(|(_, scale)| *scale))
        .or_else(|| frames.first().map(|(_, scale)| *scale))?;
    Some(cocoa_to_screen_pixels(point, desktop_top, scale))
}

#[cfg(not(any(windows, target_os = "macos")))]
fn system_cursor() -> Option<Pos2> {
    None
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
                    crate::app::voice::popup::plain::click_through(super::window::TITLE, false);
                }
                ctx.request_repaint();
            }
        }
    });
    room
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

    #[cfg(any(windows, target_os = "macos"))]
    #[test]
    fn the_click_is_asked_again_at_the_end_of_the_frame_for_the_cursor_that_came_in_meanwhile() {
        let island = Rect::from_min_size(Pos2::new(180.0, 1.0), Vec2::new(288.0, 32.0));
        let (scale, origin) = (1.0, Pos2::new(1000.0, 0.0));
        let cursor = std::cell::Cell::new(Some(Pos2::new(1900.0, 600.0)));
        let read = || cursor.get();
        assert!(!takes_click(read, scale, origin, island, false), "start of the frame: far from the island");
        // the thread saw it come in and took the click-through off; the end of the frame must not put it back
        cursor.set(Some(Pos2::new(1300.0, 20.0)));
        assert!(takes_click(read, scale, origin, island, false), "end of the frame: on the island");
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

    #[test]
    fn cocoa_points_become_top_left_pixels_on_the_screen_that_holds_them() {
        // primary 1920×1080 @2x, origin bottom-left: a point 40 from the top edge, 100 from the left
        let primary = [0.0, 0.0, 1920.0, 1080.0];
        assert_eq!(cocoa_to_screen_pixels([100.0, 1040.0], 1080.0, 2.0), Pos2::new(200.0, 80.0));
        assert_eq!(cocoa_frame_pixels(primary, 1080.0, 2.0), [0, 0, 3840, 2160]);
        // a second monitor to the right, same height
        let second = [1920.0, 0.0, 1920.0, 1080.0];
        assert_eq!(cocoa_screen_at([2000.0, 500.0], &[primary, second]), Some(second));
        assert_eq!(cocoa_frame_pixels(second, 1080.0, 1.0), [1920, 0, 3840, 1080]);
        assert_eq!(cocoa_to_screen_pixels([2000.0, 500.0], 1080.0, 1.0), Pos2::new(2000.0, 580.0));
        // off every frame: the nearest centre
        assert_eq!(cocoa_screen_at([-100.0, 500.0], &[primary, second]), Some(primary));
        assert!(!cocoa_contains([1920.0, 500.0], primary), "the right edge belongs to the next screen");
    }
}
