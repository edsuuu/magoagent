//! macOS: the island's window as the system needs it (see-through, no shadow, over the menu bar where the notch is, on
//! every Space and over full-screen apps), the clicks let through all of it but the island (`setIgnoresMouseEvents`),
//! the cursor on the whole desktop and the usable part of each screen.

use crate::prelude::*;
use objc2::MainThreadMarker;
use objc2::rc::Retained;
use objc2_app_kit::{NSApplication, NSColor, NSMainMenuWindowLevel, NSScreen, NSWindow, NSWindowCollectionBehavior};
use objc2_foundation::NSObjectProtocol;
use std::ffi::c_void;

/// This app's window called `title`.
fn window(title: &str) -> Option<Retained<NSWindow>> {
    let mtm = MainThreadMarker::new()?;
    let windows = NSApplication::sharedApplication(mtm).windows();

    (0..windows.count()).map(|index| windows.objectAtIndex(index)).find(|window| window.title().to_string() == title)
}

/// Once the window `title` exists: see-through with no shadow (the island's round corners, not a square), over the menu
/// bar (the notch is in it; the floating level stays under it), on every Space and over full-screen apps. True once done.
pub(crate) fn strip(title: &str) -> bool {
    let Some(window) = window(title) else { return false };

    window.setOpaque(false);
    window.setBackgroundColor(Some(&NSColor::clearColor()));
    window.setHasShadow(false);
    window.setLevel(NSMainMenuWindowLevel + 1);

    window.setCollectionBehavior(
        NSWindowCollectionBehavior::CanJoinAllSpaces
            | NSWindowCollectionBehavior::FullScreenAuxiliary
            | NSWindowCollectionBehavior::Stationary
            | NSWindowCollectionBehavior::IgnoresCycle,
    );

    true
}

/// The clicks go `through` the window `title` to what is under it, or it takes them.
pub(crate) fn click_through(title: &str, through: bool) {
    if let Some(window) = window(title).filter(|window| window.ignoresMouseEvents() != through) {
        window.setIgnoresMouseEvents(through);
    }
}

#[repr(C)]
struct CgPoint {
    x: f64,
    y: f64,
}

#[link(name = "CoreGraphics", kind = "framework")]
unsafe extern "C" {
    fn CGEventCreate(source: *const c_void) -> *mut c_void;
    fn CGEventGetLocation(event: *const c_void) -> CgPoint;
}

#[link(name = "CoreFoundation", kind = "framework")]
unsafe extern "C" {
    fn CFRelease(object: *const c_void);
}

/// The cursor on the whole desktop, in points from the main screen's top left (Quartz's space, the one winit places the
/// windows in). Any thread, no permission asked.
pub(crate) fn cursor() -> Option<Pos2> {
    // SAFETY: an event with no source only to read where the mouse is, released right after
    unsafe {
        let event = CGEventCreate(std::ptr::null());

        if event.is_null() {
            return None;
        }

        let at = CGEventGetLocation(event);

        CFRelease(event);
        Some(Pos2::new(at.x as f32, at.y as f32))
    }
}

/// The usable part of a screen in points from the main screen's top left, from its AppKit `frame` and `visible` frame
/// (`[x, y, width, height]`, origin at the main screen's bottom left, `main_height` tall): less the Dock, with the menu
/// bar, so the island hangs from the very top (around the camera notch, where there is one).
pub(crate) fn work_of(frame: [f64; 4], visible: [f64; 4], main_height: f64) -> Rect {
    let top = main_height - (frame[1] + frame[3]);
    let bottom = main_height - visible[1];

    Rect::from_min_max(Pos2::new(visible[0] as f32, top as f32), Pos2::new((visible[0] + visible[2]) as f32, bottom as f32))
}

/// The camera notch of a screen in the same points: between the two parts of the menu bar beside it (`left` and `right`
/// wide, `height` tall), at the top of its AppKit `frame`. None on a screen without one (both parts 0 wide).
pub(crate) fn notch_of(frame: [f64; 4], left: f64, right: f64, height: f64, main_height: f64) -> Option<Rect> {
    if left <= 0.0 || right <= 0.0 || height <= 0.0 || left + right >= frame[2] {
        return None;
    }

    let top = main_height - (frame[1] + frame[3]);

    Some(Rect::from_min_max(Pos2::new((frame[0] + left) as f32, top as f32), Pos2::new((frame[0] + frame[2] - right) as f32, (top + height) as f32)))
}

/// Each screen's usable part (`work_of`) and notch (`notch_of`).
fn screens() -> Vec<(Rect, Option<Rect>)> {
    let Some(mtm) = MainThreadMarker::new() else { return Vec::new() };
    let screens = NSScreen::screens(mtm);
    let rect = |r: objc2_foundation::NSRect| [r.origin.x, r.origin.y, r.size.width, r.size.height];
    let Some(main_height) = (screens.count() > 0).then(|| screens.objectAtIndex(0).frame().size.height) else { return Vec::new() };

    (0..screens.count())
        .map(|index| screens.objectAtIndex(index))
        .map(|screen| {
            let frame = rect(screen.frame());

            // the parts of the menu bar beside the notch came with macOS 12
            let notch = screen.respondsToSelector(objc2::sel!(auxiliaryTopLeftArea)).then(|| {
                let (left, right) = (screen.auxiliaryTopLeftArea().size, screen.auxiliaryTopRightArea().size);

                notch_of(frame, left.width, right.width, left.height.min(right.height), main_height)
            });

            (work_of(frame, rect(screen.visibleFrame()), main_height), notch.flatten())
        })
        .collect()
}

/// The usable part (`work_of`) of the screen `at` is on, else of the nearest.
// ponytail: the menu bar counts as usable for the mini chat too, which may open under it; give it the visible frame if it shows
pub(crate) fn work_at(at: Pos2) -> Option<Rect> {
    let works: Vec<Rect> = screens().into_iter().map(|(work, _)| work).collect();

    crate::app::chat::mini::work_at(&works, at)
}

/// The camera notch of the screen `at` is on (else of the nearest), when it has one.
pub(crate) fn notch_at(at: Pos2) -> Option<Rect> {
    let screens = screens();
    let works: Vec<Rect> = screens.iter().map(|(work, _)| *work).collect();
    let work = crate::app::chat::mini::work_at(&works, at)?;

    screens.into_iter().find(|(each, _)| *each == work)?.1
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_screen_is_turned_to_the_top_left_with_the_menu_bar_and_without_the_dock_and_its_notch_is_found() {
        // the main screen, 1470x956 with a 33 menu bar and a 70 Dock at the bottom
        let work = work_of([0.0, 0.0, 1470.0, 956.0], [0.0, 70.0, 1470.0, 853.0], 956.0);

        assert_eq!(work, Rect::from_min_max(Pos2::new(0.0, 0.0), Pos2::new(1470.0, 886.0)));
        // a second one above it, 1920x1080 (no Dock): its top is above the main screen's
        let above = work_of([0.0, 956.0, 1920.0, 1080.0], [0.0, 956.0, 1920.0, 1055.0], 956.0);

        assert_eq!(above, Rect::from_min_max(Pos2::new(0.0, -1080.0), Pos2::new(1920.0, 0.0)));
        // this Mac's notch (a 13" Air): 179 wide in the middle, 32 tall, from the very top
        let notch = notch_of([0.0, 0.0, 1470.0, 956.0], 645.5, 645.5, 32.0, 956.0).expect("a notch");

        assert_eq!(notch, Rect::from_min_max(Pos2::new(645.5, 0.0), Pos2::new(824.5, 32.0)));
        assert_eq!(notch.center().x, 735.0);
        // the same screen above the main one; and none without the parts beside it
        assert_eq!(notch_of([0.0, 956.0, 1470.0, 956.0], 645.5, 645.5, 32.0, 956.0).map(|notch| notch.top()), Some(-956.0));
        assert_eq!(notch_of([0.0, 956.0, 1920.0, 1080.0], 0.0, 0.0, 0.0, 956.0), None);
        assert!(cursor().is_some(), "the cursor is read off the main thread, with no permission");
    }
}
