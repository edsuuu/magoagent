//! Linux: the app runs on X11 (XWayland in a Wayland session), the only way there for a window to stay over every
//! other and be put where it is asked (Wayland leaves both to the compositor, and GNOME gives an app neither). On X11
//! the island's window keeps its size, as on Windows, and only the island takes the clicks: its input shape is the
//! island (the rest goes through to the window under it), so the window is not resized at every frame of the spring.

use crate::prelude::*;
use std::sync::{Mutex, OnceLock};
use x11rb::connection::Connection;
use x11rb::protocol::shape::{ConnectionExt as _, SK, SO};
use x11rb::protocol::xproto::{AtomEnum, ClipOrdering, ConnectionExt as _, Rectangle, Window};
use x11rb::rust_connection::RustConnection;

/// Whether the app runs on X11: when there is an X server (`DISPLAY`), XWayland included, and the libraries winit's X11
/// opens are installed (without them it would stop at the start); `MAGOAGENT_WAYLAND=1` keeps a Wayland session on
/// Wayland (the island then is a plain window the compositor places).
pub(crate) fn active() -> bool {
    static ACTIVE: OnceLock<bool> = OnceLock::new();
    *ACTIVE.get_or_init(|| chosen(std::env::var_os("DISPLAY").as_deref(), std::env::var("MAGOAGENT_WAYLAND").ok().as_deref()) && libraries())
}

/// The libraries winit opens for X11 (`dlopen`), the keyboard's included: libxkbcommon-x11 is not on every desktop that
/// has Wayland (the .deb depends on it).
fn libraries() -> bool {
    const NEEDED: [&str; 6] = ["libX11.so.6", "libX11-xcb.so.1", "libXcursor.so.1", "libXi.so.6", "libxkbcommon.so.0", "libxkbcommon-x11.so.0"];
    // SAFETY: system libraries opened and closed at once, as winit opens them; nothing of them is called
    NEEDED.iter().all(|name| unsafe { libloading::Library::new(name) }.is_ok())
}

/// `active` for these values of `DISPLAY` and `MAGOAGENT_WAYLAND`.
pub(crate) fn chosen(display: Option<&std::ffi::OsStr>, wayland: Option<&str>) -> bool {
    display.is_some_and(|display| !display.is_empty()) && wayland != Some("1")
}

/// The connection of the island's own calls, apart from winit's, and what was asked last.
struct Link {
    connection: RustConnection,
    root: Window,
    /// The island's window, once found.
    window: Option<Window>,
    /// The input shape last set, in pixels of the window (x, y, width, height).
    shape: Option<(i16, i16, u16, u16)>,
}

fn link() -> &'static Mutex<Option<Link>> {
    static LINK: OnceLock<Mutex<Option<Link>>> = OnceLock::new();
    LINK.get_or_init(|| {
        let link = x11rb::connect(None).ok().map(|(connection, screen)| {
            let root = connection.setup().roots[screen].root;
            Link { connection, root, window: None, shape: None }
        });
        Mutex::new(link)
    })
}

fn atom(connection: &RustConnection, name: &str) -> Option<u32> {
    Some(connection.intern_atom(false, name.as_bytes()).ok()?.reply().ok()?.atom)
}

/// This process's window called `title`, among the windows the window manager knows (`_NET_CLIENT_LIST`): the installed
/// app and one in development each have an island of the same name.
fn find(connection: &RustConnection, root: Window, title: &str) -> Option<Window> {
    let (list, name, utf8, pid) =
        (atom(connection, "_NET_CLIENT_LIST")?, atom(connection, "_NET_WM_NAME")?, atom(connection, "UTF8_STRING")?, atom(connection, "_NET_WM_PID")?);
    let windows: Vec<u32> = connection.get_property(false, root, list, AtomEnum::WINDOW, 0, 4096).ok()?.reply().ok()?.value32()?.collect();
    let own = std::process::id();
    windows.into_iter().rev().find(|&window| {
        let named = connection
            .get_property(false, window, name, utf8, 0, 256)
            .ok()
            .and_then(|cookie| cookie.reply().ok())
            .is_some_and(|reply| reply.value == title.as_bytes());
        let process = connection
            .get_property(false, window, pid, AtomEnum::CARDINAL, 0, 1)
            .ok()
            .and_then(|cookie| cookie.reply().ok())
            .and_then(|reply| reply.value32()?.next());
        named && process.is_none_or(|process| process == own)
    })
}

/// The part of the window `title` that takes the clicks: `area` (points, in the window) at `scale`; the rest of the
/// window lets them through. Sent only when it changes; the window is looked for until it is there.
pub(crate) fn input_shape(title: &str, area: Rect, scale: f32) {
    let mut guard = link().lock().unwrap_or_else(|poisoned| poisoned.into_inner());
    let Some(link) = guard.as_mut() else { return };
    let shape = pixels(area, scale);
    if link.window.is_none() {
        link.window = find(&link.connection, link.root, title);
        link.shape = None;
    }
    let Some(window) = link.window else { return };
    let key = (shape.x, shape.y, shape.width, shape.height);
    if link.shape == Some(key) {
        return;
    }
    let sent = link.connection.shape_rectangles(SO::SET, SK::INPUT, ClipOrdering::UNSORTED, window, 0, 0, &[shape]).ok().and_then(|cookie| cookie.check().ok());
    if sent.is_some() {
        link.shape = Some(key);
    } else {
        // the window is gone (the island was turned off and on): looked for again
        link.window = None;
    }
    let _ = link.connection.flush();
}

/// The island is gone (turned off, alerts paused): its window will be another one.
pub(crate) fn forget() {
    if let Some(link) = link().lock().unwrap_or_else(|poisoned| poisoned.into_inner()).as_mut() {
        (link.window, link.shape) = (None, None);
    }
}

/// `area` (points) in whole pixels at `scale`, rounded outwards.
pub(crate) fn pixels(area: Rect, scale: f32) -> Rectangle {
    if !area.is_positive() || !area.is_finite() {
        return Rectangle { x: 0, y: 0, width: 0, height: 0 };
    }
    let (left, top) = ((area.left() * scale).floor().max(0.0), (area.top() * scale).floor().max(0.0));
    let (right, bottom) = ((area.right() * scale).ceil().max(left), (area.bottom() * scale).ceil().max(top));
    let clamp = |value: f32| value.min(i16::MAX as f32);
    Rectangle {
        x: clamp(left) as i16,
        y: clamp(top) as i16,
        width: (right - left).min(u16::MAX as f32) as u16,
        height: (bottom - top).min(u16::MAX as f32) as u16,
    }
}

/// The cursor on the screen, in pixels, for the wizard's eyes. Under XWayland it is where the cursor last was over a
/// window of X11 (the app's own included).
pub(crate) fn cursor() -> Option<Pos2> {
    let guard = link().lock().unwrap_or_else(|poisoned| poisoned.into_inner());
    let link = guard.as_ref()?;
    let reply = link.connection.query_pointer(link.root).ok()?.reply().ok()?;
    Some(Pos2::new(reply.root_x as f32, reply.root_y as f32))
}

/// The desktop less the panels (`_NET_WORKAREA`, pixels): GNOME's top bar is outside it.
pub(crate) fn work_area() -> Option<[i32; 4]> {
    let guard = link().lock().unwrap_or_else(|poisoned| poisoned.into_inner());
    let link = guard.as_ref()?;
    let workarea = atom(&link.connection, "_NET_WORKAREA")?;
    let reply = link.connection.get_property(false, link.root, workarea, AtomEnum::CARDINAL, 0, 4).ok()?.reply().ok()?;
    let values: Vec<u32> = reply.value32()?.collect();
    let [left, top, width, height] = values[..] else { return None };
    Some([left as i32, top as i32, (left + width) as i32, (top + height) as i32])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn x11_is_chosen_where_there_is_an_x_server_unless_wayland_is_asked() {
        let display = std::ffi::OsStr::new(":0");
        assert!(chosen(Some(display), None), "an X server, XWayland included");
        assert!(chosen(Some(display), Some("0")));
        assert!(!chosen(Some(display), Some("1")), "MAGOAGENT_WAYLAND=1 stays on Wayland");
        assert!(!chosen(None, None) && !chosen(Some(std::ffi::OsStr::new("")), None), "no X server");
    }

    #[test]
    fn the_input_shape_covers_the_island_in_whole_pixels() {
        let area = Rect::from_min_max(Pos2::new(215.5, -14.0), Pos2::new(504.2, 46.0));
        let tuple = |shape: Rectangle| (shape.x, shape.y, shape.width, shape.height);
        assert_eq!(tuple(pixels(area, 1.0)), (215, 0, 290, 46), "outwards, never above the window");
        assert_eq!(tuple(pixels(area, 2.0)), (431, 0, 578, 92));
        assert_eq!(pixels(Rect::NOTHING, 1.0).width, 0, "nothing takes the clicks");
    }
}
