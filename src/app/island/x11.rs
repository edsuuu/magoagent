//! Linux: the app runs on X11 (XWayland in a Wayland session), the only way there for a window to stay over every
//! other and be put where it is asked (Wayland leaves both to the compositor, and GNOME gives an app neither). On X11
//! the island's window keeps its size, as on Windows, and only the island takes the clicks: its input shape is the
//! island (the rest goes through to the window under it), so the window is not resized at every frame of the spring.

use crate::prelude::*;
use std::sync::{Mutex, OnceLock};
use x11rb::connection::Connection;
use x11rb::protocol::Event;
use x11rb::protocol::shape::{ConnectionExt as _, SK, SO};
use x11rb::protocol::sync::{ConnectionExt as _, Int64};
use x11rb::protocol::xproto::{
    AtomEnum, ClientMessageEvent, ClipOrdering, ConnectionExt as _, CreateWindowAux, EventMask, PropMode, Rectangle, Window, WindowClass,
};
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
    /// The screen's number, and a window of ours never shown: what the X server's time is read on and what sends the
    /// startup notifications (`startup_token`).
    screen: usize,
    helper: Option<Window>,
    /// The app's window and its resize counter (`frame_drawn`); None when the server has no sync extension.
    resize: Option<(Window, Option<ResizeSync>)>,
}

/// The counter the window manager waits on while it resizes the app's window, and the value last set.
struct ResizeSync {
    counter: u32,
    value: i64,
}

fn link() -> &'static Mutex<Option<Link>> {
    static LINK: OnceLock<Mutex<Option<Link>>> = OnceLock::new();
    LINK.get_or_init(|| {
        let link = x11rb::connect(None).ok().map(|(connection, screen)| {
            let root = connection.setup().roots[screen].root;
            Link { connection, root, window: None, shape: None, screen, helper: None, resize: None }
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

/// The app's window `window` takes part in the window manager's resize (`_NET_WM_SYNC_REQUEST`, EWMH): before each
/// size of a drag on the edge the manager asks, and shows the window again only when its counter says the app drew a
/// frame after it. Without it GNOME (Mutter, the app on XWayland) showed each new size before the app had drawn it, and
/// the window flickered black. winit has no part in it: the counter is ours, set after every frame drawn at the
/// window's size (`DrawingApp::resize_sync`).
pub(crate) fn frame_drawn(window: u32) {
    let mut guard = link().lock().unwrap_or_else(|poisoned| poisoned.into_inner());
    let Some(link) = guard.as_mut() else { return };
    if link.resize.as_ref().is_none_or(|(known, _)| *known != window) {
        link.resize = Some((window, resize_sync(&link.connection, window)));
    }
    let Some((_, Some(sync))) = link.resize.as_mut() else { return };
    sync.value = next_drawn(sync.value);
    if link.connection.sync_set_counter(sync.counter, int64(sync.value)).is_ok() {
        let _ = link.connection.flush();
    }
}

/// The counter's value after a frame: past the 240 the window manager adds to the last value it saw when it asks.
fn next_drawn(value: i64) -> i64 {
    value + 1000
}

fn int64(value: i64) -> Int64 {
    Int64 { hi: (value >> 32) as i32, lo: value as u32 }
}

/// The counter of `window`, named on it (`_NET_WM_SYNC_REQUEST_COUNTER`) and its protocol added to the ones winit put
/// (`WM_PROTOCOLS`).
fn resize_sync(connection: &RustConnection, window: Window) -> Option<ResizeSync> {
    connection.sync_initialize(3, 1).ok()?.reply().ok()?;
    let counter = connection.generate_id().ok()?;
    connection.sync_create_counter(counter, int64(0)).ok()?.check().ok()?;
    let (protocols, request, property) =
        (atom(connection, "WM_PROTOCOLS")?, atom(connection, "_NET_WM_SYNC_REQUEST")?, atom(connection, "_NET_WM_SYNC_REQUEST_COUNTER")?);
    connection.change_property(PropMode::REPLACE, window, property, AtomEnum::CARDINAL, 32, 1, &counter.to_ne_bytes()).ok()?.check().ok()?;
    connection.change_property(PropMode::APPEND, window, protocols, AtomEnum::ATOM, 32, 1, &request.to_ne_bytes()).ok()?.check().ok()?;
    Some(ResizeSync { counter, value: 0 })
}

/// This process's window called `title`, once the window manager has it (mapped and managed).
pub(crate) fn own_window(title: &str) -> Option<u32> {
    let guard = link().lock().unwrap_or_else(|poisoned| poisoned.into_inner());
    let link = guard.as_ref()?;
    find(&link.connection, link.root, title)
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

/// The windows the window manager knows (`_NET_CLIENT_LIST`): (window, its process, its title). Under XWayland only the
/// programs on X11 are there; the ones on Wayland are not.
pub(crate) fn client_windows() -> Vec<(u32, u32, String)> {
    let guard = link().lock().unwrap_or_else(|poisoned| poisoned.into_inner());
    let Some(link) = guard.as_ref() else { return Vec::new() };
    let connection = &link.connection;
    let (Some(list), Some(name), Some(utf8), Some(pid)) =
        (atom(connection, "_NET_CLIENT_LIST"), atom(connection, "_NET_WM_NAME"), atom(connection, "UTF8_STRING"), atom(connection, "_NET_WM_PID"))
    else {
        return Vec::new();
    };
    let reply = connection.get_property(false, link.root, list, AtomEnum::WINDOW, 0, 4096).ok().and_then(|cookie| cookie.reply().ok());
    let windows: Vec<u32> = reply.and_then(|reply| Some(reply.value32()?.collect())).unwrap_or_default();
    windows
        .into_iter()
        .filter_map(|window| {
            let process = connection.get_property(false, window, pid, AtomEnum::CARDINAL, 0, 1).ok()?.reply().ok()?.value32()?.next()?;
            let title = connection.get_property(false, window, name, utf8, 0, 256).ok()?.reply().ok()?.value;
            Some((window, process, String::from_utf8_lossy(&title).into_owned()))
        })
        .collect()
}

/// Brings `window` to the front with the keyboard, as a taskbar does: `_NET_ACTIVE_WINDOW` from a pager, with no time
/// (mutter then takes its own, and a pager's ask is not held back as a program stealing the focus). Only after a click of
/// the person in the app asking for it.
pub(crate) fn activate(window: u32) -> bool {
    // the tests never bring a real window (the person's) to the front
    if cfg!(test) {
        return false;
    }
    let guard = link().lock().unwrap_or_else(|poisoned| poisoned.into_inner());
    let Some(link) = guard.as_ref() else { return false };
    let Some(active) = atom(&link.connection, "_NET_ACTIVE_WINDOW") else { return false };
    // source 2: a pager; time 0: now
    let event = ClientMessageEvent::new(32, window, active, [2u32, 0, 0, 0, 0]);
    let mask = EventMask::SUBSTRUCTURE_REDIRECT | EventMask::SUBSTRUCTURE_NOTIFY;
    let sent = link.connection.send_event(false, link.root, mask, event).ok().and_then(|cookie| cookie.check().ok()).is_some();
    let _ = link.connection.flush();
    sent
}

/// The window of ours that is never shown, made the first time.
fn helper(link: &mut Link) -> Option<Window> {
    if link.helper.is_none() {
        let window = link.connection.generate_id().ok()?;
        let aux = CreateWindowAux::new().event_mask(EventMask::PROPERTY_CHANGE);
        link.connection.create_window(0, window, link.root, -10, -10, 1, 1, 0, WindowClass::INPUT_ONLY, 0, &aux).ok()?.check().ok()?;
        link.helper = Some(window);
    }
    link.helper
}

/// The X server's time now, as ICCCM reads it: a property of our helper window changed (by nothing), and the time of its
/// notify. Waits a moment at most.
fn server_time(link: &mut Link) -> Option<u32> {
    let window = helper(link)?;
    let mark = atom(&link.connection, "_MAGOAGENT_TIME")?;
    link.connection.change_property(PropMode::APPEND, window, mark, AtomEnum::STRING, 8, 0, &[]).ok()?;
    link.connection.flush().ok()?;
    for _ in 0..100 {
        match link.connection.poll_for_event().ok()? {
            Some(Event::PropertyNotify(notify)) if notify.window == window => return Some(notify.time),
            Some(_) => {}
            None => std::thread::sleep(std::time::Duration::from_millis(2)),
        }
    }
    None
}

/// The startup notification's message (the startup-notification spec): what starts, on which screen.
pub(crate) fn startup_message(id: &str, screen: usize) -> String {
    format!("new: ID={id} NAME=MagoAgent SCREEN={screen}")
}

/// Sends a startup notification's message, in its pieces, from our helper window.
fn startup_send(link: &mut Link, message: &str) -> Option<()> {
    let window = helper(link)?;
    let (begin, more) = (atom(&link.connection, "_NET_STARTUP_INFO_BEGIN")?, atom(&link.connection, "_NET_STARTUP_INFO")?);
    for (index, piece) in startup_pieces(message).into_iter().enumerate() {
        let event = ClientMessageEvent::new(8, window, if index == 0 { begin } else { more }, piece);
        link.connection.send_event(false, link.root, EventMask::PROPERTY_CHANGE, event).ok()?;
    }
    link.connection.flush().ok()
}

/// A message in the pieces of 20 bytes the client messages carry, ended by a zero.
pub(crate) fn startup_pieces(message: &str) -> Vec<[u8; 20]> {
    let mut bytes = message.as_bytes().to_vec();
    bytes.push(0);
    bytes
        .chunks(20)
        .map(|chunk| {
            let mut piece = [0u8; 20];
            piece[..chunk.len()].copy_from_slice(chunk);
            piece
        })
        .collect()
}

/// A token for a program on Wayland to come to the front with (xdg-activation, as a launcher hands one): a startup
/// notification of ours on X11 (`_NET_STARTUP_INFO`), stamped with the X server's time now, whose ID mutter takes as a
/// token when the program shows it, and honours when nothing the person did came after it.
pub(crate) fn startup_token() -> Option<String> {
    static COUNT: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);
    let mut guard = link().lock().unwrap_or_else(|poisoned| poisoned.into_inner());
    let link = guard.as_mut()?;
    let time = server_time(link)?;
    let count = COUNT.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let id = format!("magoagent-{}-{count}_TIME{time}", std::process::id());
    let message = startup_message(&id, link.screen);
    startup_send(link, &message)?;
    Some(id)
}

/// The startup notification `id` is over (mutter ends it when the token is used; this one is for when it was not).
pub(crate) fn startup_done(id: &str) {
    let mut guard = link().lock().unwrap_or_else(|poisoned| poisoned.into_inner());
    if let Some(link) = guard.as_mut() {
        startup_send(link, &format!("remove: ID={id}"));
    }
}

/// The cursor on the screen, in pixels, for the wizard's eyes. Under XWayland the X server knows it only over a window
/// of X11 (the app's own included); off them GNOME's extension tells it (`gnome::cursor`), when installed.
pub(crate) fn cursor() -> Option<Pos2> {
    let guard = link().lock().unwrap_or_else(|poisoned| poisoned.into_inner());
    let link = guard.as_ref()?;
    let reply = link.connection.query_pointer(link.root).ok()?.reply().ok()?;
    static XWAYLAND: OnceLock<bool> = OnceLock::new();
    let xwayland = *XWAYLAND.get_or_init(|| std::env::var_os("WAYLAND_DISPLAY").is_some_and(|display| !display.is_empty()));
    let known = known_cursor(xwayland, reply.child, reply.root_x, reply.root_y);
    drop(guard);
    known.or_else(super::gnome::cursor)
}

/// The cursor the X server gave, when it can be trusted: under XWayland it only follows the cursor over a window of
/// X11 (`over` is none off them) and stays where it last was over the others, which the eyes would stare at.
fn known_cursor(xwayland: bool, over: u32, x: i16, y: i16) -> Option<Pos2> {
    (!xwayland || over != x11rb::NONE).then(|| Pos2::new(x as f32, y as f32))
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
    fn under_xwayland_the_cursor_off_its_windows_is_unknown_and_on_x11_it_is_always_known() {
        assert_eq!(known_cursor(true, 0x0120_0003, 640, 12), Some(Pos2::new(640.0, 12.0)), "over a window of X11 (the island)");
        assert_eq!(known_cursor(true, x11rb::NONE, 640, 12), None, "over a Wayland window: where it last was, not where it is");
        assert_eq!(known_cursor(false, x11rb::NONE, 640, 12), Some(Pos2::new(640.0, 12.0)), "X11 alone: over the desktop too");
    }

    #[test]
    fn x11_is_chosen_where_there_is_an_x_server_unless_wayland_is_asked() {
        let display = std::ffi::OsStr::new(":0");
        assert!(chosen(Some(display), None), "an X server, XWayland included");
        assert!(chosen(Some(display), Some("0")));
        assert!(!chosen(Some(display), Some("1")), "MAGOAGENT_WAYLAND=1 stays on Wayland");
        assert!(!chosen(None, None) && !chosen(Some(std::ffi::OsStr::new("")), None), "no X server");
    }

    #[test]
    fn a_startup_notification_goes_in_pieces_of_twenty_bytes_ended_by_a_zero() {
        let message = startup_message("magoagent-7-0_TIME1234", 0);
        assert_eq!(message, "new: ID=magoagent-7-0_TIME1234 NAME=MagoAgent SCREEN=0");
        let pieces = startup_pieces(&message);
        assert_eq!(pieces.len(), message.len() / 20 + 1);
        let joined: Vec<u8> = pieces.concat();
        assert_eq!(&joined[..message.len()], message.as_bytes());
        assert_eq!(joined[message.len()], 0, "the zero that ends it");
        assert_eq!(startup_pieces(&"x".repeat(20)).len(), 2, "a message of exactly 20 bytes takes another piece for its zero");
    }

    #[test]
    fn the_input_shape_covers_the_island_in_whole_pixels() {
        let area = Rect::from_min_max(Pos2::new(215.5, -14.0), Pos2::new(504.2, 46.0));
        let tuple = |shape: Rectangle| (shape.x, shape.y, shape.width, shape.height);
        assert_eq!(tuple(pixels(area, 1.0)), (215, 0, 290, 46), "outwards, never above the window");
        assert_eq!(tuple(pixels(area, 2.0)), (431, 0, 578, 92));
        assert_eq!(pixels(Rect::NOTHING, 1.0).width, 0, "nothing takes the clicks");
    }

    #[test]
    fn the_resize_counter_stays_ahead_of_what_the_window_manager_waits_for() {
        // Mutter asks for its last value + 1 + 240 and waits until the counter gets there
        let mut value = 0;
        for _ in 0..3 {
            let waits_for = value + 1 + 240;
            value = next_drawn(value);
            assert!(value >= waits_for);
        }
        let halves = int64(5_000_000_000);
        assert_eq!((halves.hi, halves.lo), (1, 705_032_704), "split in two halves for the protocol");
    }
}
