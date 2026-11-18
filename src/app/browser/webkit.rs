//! Linux: a portal's pages are WebKitGTK views (wry), each in an X11 window of its own inside the app's window. GTK
//! only runs on its thread (system/gtk.rs): the pages and their profiles live there, by number, and the app holds a
//! handle to each that sends GTK what it asks for, in order. The handle has the calls of wry's page that the app makes,
//! so the rest of the browser reads the same on every system.

use super::*;
use crate::app::system::gtk as toolkit;
use std::cell::RefCell;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use wry::raw_window_handle::{HandleError, HasWindowHandle, RawWindowHandle, WindowHandle, XlibWindowHandle};

thread_local! {
    /// The live pages and the profiles they use, on GTK's thread.
    static PAGES: RefCell<HashMap<u64, wry::WebView>> = RefCell::new(HashMap::new());
    static PROFILES: RefCell<HashMap<u64, wry::WebContext>> = RefCell::new(HashMap::new());
}

/// How long the app waits for GTK to make a page (it takes a few milliseconds; the first one also waits for GTK).
const MAKING: std::time::Duration = std::time::Duration::from_secs(10);

/// Why a page does not open, on the card and for the agents.
pub(crate) const PAGES_UNAVAILABLE: &str = "No Linux o navegador abre só no X11 (sem MAGOAGENT_WAYLAND=1)";

fn next_number() -> u64 {
    static NEXT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(1);
    NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
}

/// Whether pages open here: the app on X11 (its window holds them) and GTK up. ponytail: on Wayland
/// (`MAGOAGENT_WAYLAND=1`) a portal shows only its address; wry's `build_gtk` in a GTK window of its own over the card
/// would be the way there, which the compositor places where it likes.
pub(crate) fn pages_open() -> bool {
    crate::app::island::x11::active() && toolkit::alive()
}

/// The app's window, as wry asks for the parent of a page.
struct Parent(std::ffi::c_ulong);

impl HasWindowHandle for Parent {
    fn window_handle(&self) -> Result<WindowHandle<'_>, HandleError> {
        let raw = RawWindowHandle::Xlib(XlibWindowHandle::new(self.0));
        // SAFETY: an X11 window id; X keeps it valid while the app's window lives, and a page's window goes with it
        Ok(unsafe { WindowHandle::borrow_raw(raw) })
    }
}

/// A portal's profile (cookies, logins), on GTK's thread.
pub(crate) struct GtkProfile {
    number: u64,
}

impl GtkProfile {
    pub(crate) fn new(folder: Option<PathBuf>) -> Self {
        let number = next_number();
        toolkit::run(move || {
            PROFILES.with(|profiles| profiles.borrow_mut().insert(number, wry::WebContext::new(folder)));
        });
        Self { number }
    }
}

impl Drop for GtkProfile {
    fn drop(&mut self) {
        let number = self.number;
        toolkit::run(move || {
            let profile = PROFILES.with(|profiles| profiles.borrow_mut().remove(&number));
            drop(profile);
        });
    }
}

/// A page on GTK's thread, as the app holds it.
pub(crate) struct GtkPage {
    number: u64,
    /// The address the page is at, kept by GTK as it changes (a single-page app's too).
    url: Arc<Mutex<String>>,
    /// The X11 window wry made to hold the page (a child of the app's), where the holes are cut.
    holder: u32,
    /// The app's window, which takes the keyboard back from the page.
    parent: u32,
}

impl Drop for GtkPage {
    fn drop(&mut self) {
        let number = self.number;
        toolkit::run(move || {
            let page = PAGES.with(|pages| pages.borrow_mut().remove(&number));
            drop(page);
        });
    }
}

impl GtkPage {
    /// Runs `job` with the page on GTK's thread, after what was asked before (nothing if the page is gone).
    fn with(&self, job: impl FnOnce(&wry::WebView) + Send + 'static) -> wry::Result<()> {
        let number = self.number;
        toolkit::run(move || {
            PAGES.with(|pages| {
                if let Some(page) = pages.borrow().get(&number) {
                    job(page);
                }
            })
        });
        Ok(())
    }

    /// Puts the page over `page` (window points at `scale` pixels a point), its zoom the card's.
    pub(crate) fn place(&self, page: Rect, zoom: f32, scale: f32) -> wry::Result<()> {
        let [x, y, width, height] = page_pixels(page, scale);
        let bounds =
            wry::Rect { position: wry::dpi::PhysicalPosition::new(x, y).into(), size: wry::dpi::PhysicalSize::new(width as u32, height as u32).into() };
        self.with(move |page| {
            use gtk::prelude::WidgetExt;
            use wry::WebViewExtUnix;
            let _ = page.set_bounds(bounds);
            let _ = page.zoom(page_zoom(zoom, scale, page.webview().scale_factor()));
        })
    }

    pub(crate) fn set_visible(&self, visible: bool) -> wry::Result<()> {
        self.with(move |page| {
            let _ = page.set_visible(visible);
        })
    }

    pub(crate) fn load_url(&self, url: &str) -> wry::Result<()> {
        let url = url.to_string();
        self.with(move |page| {
            let _ = page.load_url(&url);
        })
    }

    pub(crate) fn reload(&self) -> wry::Result<()> {
        self.with(|page| {
            let _ = page.reload();
        })
    }

    pub(crate) fn evaluate_script(&self, script: &str) -> wry::Result<()> {
        let script = script.to_string();
        self.with(move |page| {
            let _ = page.evaluate_script(&script);
        })
    }

    /// The answer comes on GTK's thread. ponytail: wry holds the scripts sent before the page's first load and runs
    /// them then without an answer; the callback goes unanswered (dropped) there, as with a page that is gone.
    pub(crate) fn evaluate_script_with_callback(&self, script: &str, callback: impl Fn(String) + Send + 'static) -> wry::Result<()> {
        let script = script.to_string();
        self.with(move |page| {
            let _ = page.evaluate_script_with_callback(&script, callback);
        })
    }

    /// The keyboard back to the app's window: a page took it when clicked (`watch`).
    pub(crate) fn focus_parent(&self) -> wry::Result<()> {
        release_keys(Some(self.parent));
        Ok(())
    }

    pub(crate) fn url(&self) -> wry::Result<String> {
        Ok(self.url.lock().map(|url| url.clone()).unwrap_or_default())
    }

    /// The page's picture as GTK shows it now (opaque RGBA) and the pixels a CSS px takes, to `done` on GTK's thread
    /// once WebKit has drawn it; None when it could not (a page not on screen).
    pub(crate) fn picture(&self, done: impl FnOnce(Option<(u32, u32, Vec<u8>, f64)>) + Send + 'static) {
        let _ = self.with(move |page| {
            use gtk::prelude::WidgetExt;
            use webkit2gtk::WebViewExt;
            use wry::WebViewExtUnix;
            let view = page.webview();
            let scale = view.zoom_level() * f64::from(view.scale_factor().max(1));
            let cancel: Option<&gtk::gio::Cancellable> = None;
            view.snapshot(webkit2gtk::SnapshotRegion::Visible, webkit2gtk::SnapshotOptions::NONE, cancel, move |surface| {
                done(surface.ok().and_then(surface_rgba).map(|(width, height, rgba)| (width, height, rgba, scale)));
            });
        });
    }

    /// Where the page's X11 window is inside the app's (x, y, width, height in pixels), the size WebKit lays the page
    /// out at and its zoom, to `done` on GTK's thread: the roteiro's `shot` checks it against the card.
    pub(crate) fn frame(&self, done: impl FnOnce([i32; 4], [i32; 2], f64) + Send + 'static) {
        let _ = self.with(move |page| {
            use gtk::prelude::WidgetExt;
            use webkit2gtk::WebViewExt;
            use wry::WebViewExtUnix;
            let view = page.webview();
            let (x, y, width, height) = view.parent_window().map_or((0, 0, 0, 0), |holder| holder.geometry());
            done([x, y, width, height], [view.allocated_width(), view.allocated_height()], view.zoom_level());
        });
    }
}

/// A PNG of `rect` (x, y, width, height in the page's CSS px, of what shows) of the page; `done` runs on GTK's thread
/// when WebKit has it (None if it failed), as the Mac's `design::snapshot` does on its main thread.
pub(crate) fn snapshot(page: &GtkPage, rect: [f64; 4], done: impl FnOnce(Option<Vec<u8>>) + Send + 'static) {
    page.picture(move |picture| {
        let cut = picture.and_then(|(width, height, rgba, scale)| crop(width, height, &rgba, rect, scale));
        done(cut.and_then(|(width, height, rgba)| crate::app::browser::shot::png(width, height, rgba).ok()));
    });
}

/// A tab's live page, made on GTK's thread inside `window` (the app's, an X11 window).
pub(super) fn new_tab_view(
    window: &impl HasWindowHandle,
    portal: &str,
    url: &str,
    events: &std::sync::mpsc::Sender<PortalEvent>,
    ctx: &Context,
    web_context: &mut GtkProfile,
    extras: &crate::app::browser::extras::TabExtras,
) -> Result<TabView, String> {
    let parent = match window.window_handle().map(|handle| handle.as_raw()) {
        Ok(RawWindowHandle::Xlib(handle)) => handle.window,
        Ok(RawWindowHandle::Xcb(handle)) => handle.window.get() as std::ffi::c_ulong,
        _ => return Err(PAGES_UNAVAILABLE.into()),
    };
    let uid = next_number();
    let seen = Arc::new(Mutex::new(url.to_string()));
    let (portal, address, events, ctx, extras, profile, kept) =
        (portal.to_string(), url.to_string(), events.clone(), ctx.clone(), extras.clone(), web_context.number, seen.clone());
    let made = toolkit::call(MAKING, move || -> Result<u32, String> {
        if !toolkit::on_x11() {
            return Err(PAGES_UNAVAILABLE.into());
        }
        let page = PROFILES.with(|profiles| {
            let mut profiles = profiles.borrow_mut();
            let context = profiles.get_mut(&profile).ok_or("o perfil do navegador não abriu")?;
            page_builder(context, &portal, uid, &address, &events, &ctx, &extras).build_as_child(&Parent(parent)).map_err(|error| error.to_string())
        })?;
        let holder = watch(&page, kept);
        // the page's windows exist for the app's own X11 calls (the holes) once GTK has sent them
        if let Some(display) = gtk::gdk::Display::default() {
            display.flush();
        }
        PAGES.with(|pages| pages.borrow_mut().insert(uid, page));
        Ok(holder)
    });
    match made {
        Some(Ok(holder)) => {
            let webview = GtkPage { number: uid, url: seen, holder, parent: parent as u32 };
            Ok(TabView { webview, uid, url: url.into(), loading: true, secret: None })
        }
        Some(Err(error)) => Err(error),
        None => {
            // still being made: it goes once it is (after the making, in GTK's order)
            drop(GtkPage { number: uid, url: seen, holder: 0, parent: 0 });
            Err("o navegador não respondeu".into())
        }
    }
}

/// Keeps the page's address in `seen` as it changes, gives the page the keyboard when it is clicked and gives the X11
/// window that holds it.
fn watch(page: &wry::WebView, seen: Arc<Mutex<String>>) -> u32 {
    use gtk::prelude::{ObjectExt, WidgetExt};
    use webkit2gtk::WebViewExt;
    use wry::WebViewExtUnix;
    let view = page.webview();
    view.connect_notify_local(Some("uri"), move |view, _| {
        if let (Some(uri), Ok(mut seen)) = (view.uri(), seen.lock()) {
            *seen = uri.to_string();
        }
    });
    // X gives the keys to the window that has the keyboard (the app's, which the window manager gave it), or to a window
    // inside it under the pointer that asks for keys: a click on the page takes the keyboard into the page's window, as
    // WebView2 and WKWebView take it, and the page asks for keys only then, until the app has the keyboard again
    // (`release_keys`); else, merely under the pointer, it would take what is typed in the app
    view.connect_button_press_event(|view, _| {
        take_keys(view, true);
        activate(view, true);
        if let Some(window) = view.window().and_then(xid) {
            crate::app::island::x11::give_keyboard(window);
        }
        gtk::glib::Propagation::Proceed
    });
    // the page's own X11 window (`xid` makes it one), with GDK's mask, keys included: none until a click
    take_keys(&view, false);
    // the page's parent window is the one wry made inside the app's
    view.parent_window().and_then(xid).unwrap_or(0)
}

/// On GTK's thread: whether a page's window asks X for the keys.
fn take_keys(view: &webkit2gtk::WebView, on: bool) {
    use gtk::prelude::WidgetExt;
    let keys = gtk::gdk::EventMask::KEY_PRESS_MASK | gtk::gdk::EventMask::KEY_RELEASE_MASK;
    if let Some(window) = view.window() {
        window.set_events(if on { window.events() | keys } else { window.events() - keys });
    }
}

/// On GTK's thread: tells a page's GTK window it has the keyboard (or not), as the window manager tells a window of its
/// own, which GDK can't hear for a window inside another program's: WebKit then shows the caret and the page has the
/// focus (`document.hasFocus()`, focus and blur), as GTK's docs do for `gtk_widget_send_focus_change`.
fn activate(view: &webkit2gtk::WebView, on: bool) {
    use gtk::glib::object::Cast;
    use gtk::glib::translate::{ToGlibPtr, ToGlibPtrMut};
    use gtk::prelude::{GtkWindowExt, WidgetExt};
    if on {
        view.grab_focus();
    }
    let Some(toplevel) = view.toplevel().and_then(|toplevel| toplevel.downcast::<gtk::Window>().ok()) else { return };
    if toplevel.is_active() == on {
        return;
    }
    let mut event = gtk::gdk::Event::new(gtk::gdk::EventType::FocusChange);
    let raw: *mut gtk::gdk::ffi::GdkEvent = event.to_glib_none_mut().0;
    // SAFETY: a FocusChange event is a GdkEventFocus; freeing the event lets go of its window, so it takes a reference
    unsafe {
        (*raw).focus_change.in_ = i16::from(on);
        if let Some(window) = toplevel.window() {
            (*raw).focus_change.window = window.to_glib_full();
        }
    }
    toplevel.send_focus_change(&event);
}

/// The pages stop taking keys (a click on one gives it them again) and, with `parent`, the app's window takes the
/// keyboard: a click on the app after one on a page, or the app given the keyboard again by the window manager.
/// ponytail: back from another program, the page that had the keyboard needs a click again (WebView2 keeps it); upgrade:
/// remember that page and give it the keys back when the window manager gives the app's window the keyboard.
pub(crate) fn release_keys(parent: Option<u32>) {
    toolkit::run(move || {
        use wry::WebViewExtUnix;
        PAGES.with(|pages| {
            for page in pages.borrow().values() {
                take_keys(&page.webview(), false);
                activate(&page.webview(), false);
            }
        });
        if let Some(parent) = parent {
            crate::app::island::x11::give_keyboard(parent);
        }
    });
}

/// A GDK window's X11 id (made a real X11 window if it was only GDK's).
fn xid(window: gtk::gdk::Window) -> Option<u32> {
    use gtk::glib::object::Cast;
    window.downcast::<gdkx11::X11Window>().ok().map(|window| window.xid() as u32)
}

/// Leaves `holes` (window points) out of a page's X11 window, as Windows does with a window region, so the app's
/// floating UI over it shows through while the page keeps its size; `origin` is where the page's box starts.
pub(super) fn cut_page(page: &GtkPage, origin: Pos2, holes: &[Rect], pixels_per_point: f32) {
    let holes: Vec<_> = holes.iter().map(|hole| crate::app::island::x11::pixels(hole.translate(-origin.to_vec2()), pixels_per_point)).collect();
    if page.holder != 0 {
        crate::app::island::x11::cut(page.holder, &holes);
    }
}

/// The page's box in whole pixels of the window (x, y, width, height): whole points inside the card's box, as wry
/// rounds them elsewhere, at `scale`.
pub(crate) fn page_pixels(page: Rect, scale: f32) -> [i32; 4] {
    let (min, max) = (page.min.ceil(), page.max.floor());
    let pixels = |points: f32| (points * scale).round() as i32;
    [pixels(min.x), pixels(min.y), pixels(max.x - min.x).max(1), pixels(max.y - min.y).max(1)]
}

/// WebKit's zoom for a card shown at `zoom`: the box is given in pixels and GTK draws `gdk_scale` pixels a CSS px, so
/// the page lays out as wide in CSS px as the card's box is in points over its zoom, as on the other systems.
pub(crate) fn page_zoom(zoom: f32, scale: f32, gdk_scale: i32) -> f64 {
    f64::from(zoom) * f64::from(scale) / f64::from(gdk_scale.max(1))
}

/// `rect` (CSS px of what shows) of a picture taken at `scale` pixels a CSS px: None when nothing of it is inside.
pub(crate) fn crop(width: u32, height: u32, rgba: &[u8], [x, y, w, h]: [f64; 4], scale: f64) -> Option<(u32, u32, Vec<u8>)> {
    let clamp = |value: f64, most: u32| value.clamp(0.0, f64::from(most)) as u32;
    let (left, top) = (clamp((x * scale).floor(), width), clamp((y * scale).floor(), height));
    let (right, bottom) = (clamp(((x + w) * scale).ceil(), width), clamp(((y + h) * scale).ceil(), height));
    if right <= left || bottom <= top || rgba.len() < (width * height * 4) as usize {
        return None;
    }
    let mut out = Vec::with_capacity(((right - left) * (bottom - top) * 4) as usize);
    for row in top..bottom {
        let start = ((row * width + left) * 4) as usize;
        out.extend_from_slice(&rgba[start..start + ((right - left) * 4) as usize]);
    }
    Some((right - left, bottom - top, out))
}

/// Cairo's picture (BGRA, rows `stride` bytes apart) as opaque RGBA.
fn surface_rgba(surface: gtk::cairo::Surface) -> Option<(u32, u32, Vec<u8>)> {
    let image = gtk::cairo::ImageSurface::try_from(surface).ok()?;
    image.flush();
    let (width, height, stride) = (image.width().max(0) as usize, image.height().max(0) as usize, image.stride().max(0) as usize);
    let mut pixels = None;
    image.with_data(|data| pixels = rows(data, width, height, stride)).ok()?;
    Some((width as u32, height as u32, crate::app::browser::shot::opaque_rgba(pixels?)))
}

/// The pixels of `height` rows of `width` 4-byte pixels, `stride` bytes apart, without the padding at their ends.
pub(crate) fn rows(data: &[u8], width: usize, height: usize, stride: usize) -> Option<Vec<u8>> {
    if width == 0 || height == 0 || stride < width * 4 || data.len() < stride * (height - 1) + width * 4 {
        return None;
    }
    Some((0..height).flat_map(|row| &data[row * stride..row * stride + width * 4]).copied().collect())
}
