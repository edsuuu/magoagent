//! Frameless see-through windows on macOS: transparent, no shadow, above the menu bar, click-through.

use crate::app::island::cursor::{cocoa_frame_pixels, cocoa_screen_at};
use objc2::MainThreadMarker;
use objc2::rc::Retained;
use objc2_app_kit::{
    NSApplication, NSColor, NSMainMenuWindowLevel, NSScreen, NSWindow, NSWindowCollectionBehavior,
};

/// This app's window called `title`.
fn find(title: &str) -> Option<Retained<NSWindow>> {
    let mtm = MainThreadMarker::new()?;
    let app = NSApplication::sharedApplication(mtm);
    app.windows().iter().find(|window| window.title().to_string() == title)
}

/// The screens as AppKit frames `[x, y, width, height]` with each backing scale, and the desktop's top (`maxY`).
fn screen_frames(mtm: MainThreadMarker) -> (Vec<([f32; 4], f32)>, f32) {
    let screens = NSScreen::screens(mtm);
    let mut frames = Vec::new();
    let mut desktop_top = 0.0f32;
    for screen in screens.iter() {
        let frame = screen.frame();
        let rect = [frame.origin.x as f32, frame.origin.y as f32, frame.size.width as f32, frame.size.height as f32];
        desktop_top = desktop_top.max(rect[1] + rect[3]);
        frames.push((rect, screen.backingScaleFactor() as f32));
    }
    (frames, desktop_top)
}

/// Clicks through the window `title` (`through`) or taken by it (`setIgnoresMouseEvents`).
pub(crate) fn click_through(title: &str, through: bool) {
    let Some(window) = find(title) else { return };
    window.setIgnoresMouseEvents(through);
}

/// The work area (the screen less the menu bar and the Dock) of the monitor at or nearest the point, in pixels:
/// left, top, right, bottom (origin top-left).
pub(crate) fn work_area(x: i32, y: i32) -> Option<[i32; 4]> {
    let mtm = MainThreadMarker::new()?;
    let (frames, desktop_top) = screen_frames(mtm);
    if frames.is_empty() {
        return None;
    }
    // find which screen holds the pixel point by comparing top-left pixel frames
    let pixel_frames: Vec<[i32; 4]> = frames.iter().map(|(rect, scale)| cocoa_frame_pixels(*rect, desktop_top, *scale)).collect();
    let index = pixel_frames
        .iter()
        .position(|&[left, top, right, bottom]| x >= left && x < right && y >= top && y < bottom)
        .or_else(|| {
            pixel_frames.iter().enumerate().min_by(|(_, a), (_, b)| {
                let dist = |[left, top, right, bottom]: [i32; 4]| {
                    let (cx, cy) = ((left + right) as f32 / 2.0, (top + bottom) as f32 / 2.0);
                    (cx - x as f32).hypot(cy - y as f32)
                };
                dist(**a).total_cmp(&dist(**b))
            }).map(|(index, _)| index)
        })?;
    let screens = NSScreen::screens(mtm);
    let screen = screens.objectAtIndex(index);
    let visible = screen.visibleFrame();
    let visible = [visible.origin.x as f32, visible.origin.y as f32, visible.size.width as f32, visible.size.height as f32];
    let scale = frames[index].1;
    Some(cocoa_frame_pixels(visible, desktop_top, scale))
}

/// True once this app's window called `title` is found and configured: transparent, no shadow, above the menu bar,
/// on every Space and over full-screen apps.
pub(crate) fn strip(title: &str) -> bool {
    let Some(window) = find(title) else { return false };
    window.setOpaque(false);
    window.setBackgroundColor(Some(&NSColor::clearColor()));
    window.setHasShadow(false);
    window.setLevel(NSMainMenuWindowLevel + 1);
    let behavior = NSWindowCollectionBehavior::CanJoinAllSpaces
        | NSWindowCollectionBehavior::FullScreenAuxiliary
        | NSWindowCollectionBehavior::Stationary
        | NSWindowCollectionBehavior::IgnoresCycle;
    window.setCollectionBehavior(behavior);
    true
}

/// The way onto the screen of a window: on macOS there is no cloak dance — once stripped it is ready (`stage` 3).
pub(crate) fn reveal(title: &str, stage: &mut u8) {
    if *stage >= 3 {
        return;
    }
    if strip(title) {
        *stage = 3;
    }
}

/// Where the notch hangs on the screen that holds the point `(x, y)` in top-left points: centre X and the physical top
/// of that screen (also top-left points). `None` when the screen has no notch.
pub(crate) fn notch_at(x: f32, y: f32) -> Option<[f32; 2]> {
    let mtm = MainThreadMarker::new()?;
    let (frames, desktop_top) = screen_frames(mtm);
    if frames.is_empty() {
        return None;
    }
    // approximate AppKit point from top-left points at scale 1 of the nearest screen; refined by the screen's own scale
    let approx = [x, desktop_top - y];
    let rects: Vec<[f32; 4]> = frames.iter().map(|(rect, _)| *rect).collect();
    let frame = cocoa_screen_at(approx, &rects)?;
    let index = frames.iter().position(|(rect, _)| *rect == frame)?;
    let scale = frames[index].1;
    let point_px = [(x * scale).round() as i32, (y * scale).round() as i32];
    let pixel_frames: Vec<[i32; 4]> = frames.iter().map(|(rect, scale)| cocoa_frame_pixels(*rect, desktop_top, *scale)).collect();
    let index = pixel_frames
        .iter()
        .position(|&[left, top, right, bottom]| point_px[0] >= left && point_px[0] < right && point_px[1] >= top && point_px[1] < bottom)
        .unwrap_or(index);
    let screens = NSScreen::screens(mtm);
    let screen = screens.objectAtIndex(index);
    let insets = screen.safeAreaInsets();
    if insets.top <= 0.0 {
        return None;
    }
    let frame = screen.frame();
    let left = screen.auxiliaryTopLeftArea();
    let right = screen.auxiliaryTopRightArea();
    let frame = [frame.origin.x as f32, frame.origin.y as f32, frame.size.width as f32, frame.size.height as f32];
    let left = [left.origin.x as f32, left.origin.y as f32, left.size.width as f32, left.size.height as f32];
    let right = [right.origin.x as f32, right.origin.y as f32, right.size.width as f32, right.size.height as f32];
    notch_anchor(frame, left, right, insets.top as f32, desktop_top)
}

/// Notch centre and physical top (top-left points) from the screen `frame` and the free areas beside the notch
/// (`left` / `right`), all AppKit bottom-left `[x, y, width, height]`. `None` when there is no notch.
pub(crate) fn notch_anchor(frame: [f32; 4], left: [f32; 4], right: [f32; 4], insets_top: f32, desktop_top: f32) -> Option<[f32; 2]> {
    if insets_top <= 0.0 {
        return None;
    }
    let notch_width = frame[2] - left[2] - right[2];
    if notch_width <= 0.0 {
        return None;
    }
    let centre_x = left[0] + left[2] + notch_width / 2.0;
    let top_y = desktop_top - (frame[1] + frame[3]);
    Some([centre_x, top_y])
}

/// A fingerprint of the current screen layout: when it changes, the island's saved anchor is recalculated.
pub(crate) fn screens_fingerprint() -> u64 {
    let Some(mtm) = MainThreadMarker::new() else { return 0 };
    let (frames, desktop_top) = screen_frames(mtm);
    let mut hash: u64 = desktop_top.to_bits() as u64;
    for (index, (frame, scale)) in frames.iter().enumerate() {
        for value in frame {
            hash = hash.wrapping_mul(16777619) ^ value.to_bits() as u64;
        }
        hash = hash.wrapping_mul(16777619) ^ scale.to_bits() as u64;
        hash = hash.wrapping_mul(16777619) ^ index as u64;
    }
    hash
}

#[cfg(test)]
mod tests {
    use super::notch_anchor;

    #[test]
    fn the_notch_anchor_sits_between_the_free_areas_at_the_physical_top() {
        // 1512-wide Liquid Retina: 38 pt safe top, free areas 300 and 300, notch in the middle
        let frame = [0.0, 0.0, 1512.0, 982.0];
        let left = [0.0, 944.0, 300.0, 38.0];
        let right = [1212.0, 944.0, 300.0, 38.0];
        assert_eq!(notch_anchor(frame, left, right, 38.0, 982.0), Some([756.0, 0.0]));
        assert_eq!(notch_anchor(frame, left, right, 0.0, 982.0), None, "no safe inset: no notch");
        assert_eq!(notch_anchor(frame, [0.0, 0.0, 756.0, 38.0], [756.0, 0.0, 756.0, 38.0], 38.0, 982.0), None, "no gap");
    }
}
