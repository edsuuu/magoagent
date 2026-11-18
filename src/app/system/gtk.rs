//! Linux: GTK's thread, one for the whole app. GTK takes the thread that starts it as its own and refuses every other,
//! and two parts of the app need it: the tray's AppIndicator (system/tray.rs) and the portals' pages, WebKitGTK views
//! (browser/webkit.rs). The thread starts the first time either asks, runs GTK's loop for as long as the app runs (the
//! window hidden too, when eframe stops calling the app) and runs what the app sends it, in the order sent.

use crate::prelude::*;
use std::sync::atomic::{AtomicU8, Ordering};

const STARTING: u8 = 0;
const READY: u8 = 1;
const FAILED: u8 = 2;

static STATE: AtomicU8 = AtomicU8::new(STARTING);

thread_local! {
    /// Whether this thread is GTK's: a job sent from there runs at once (waiting for it would wait forever).
    static ON_GTK: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
}

/// Starts GTK's thread, once. With the app on X11 GTK is kept to X11 too: the pages are windows inside the app's window,
/// which a Wayland GTK could not reach (and wry would stop the thread over).
fn start() {
    static STARTED: std::sync::Once = std::sync::Once::new();
    STARTED.call_once(|| {
        let thread = std::thread::Builder::new().name("gtk".into()).spawn(|| {
            ON_GTK.with(|on| on.set(true));
            // the app's number formats stay C's: GTK would take the system's locale for the whole process
            gtk::disable_setlocale();
            if island::x11::active() {
                gtk::gdk::set_allowed_backends("x11");
            }
            if gtk::init().is_err() {
                STATE.store(FAILED, Ordering::Release);
                return;
            }
            STATE.store(READY, Ordering::Release);
            gtk::main();
        });
        if thread.is_err() {
            STATE.store(FAILED, Ordering::Release);
        }
    });
}

/// Whether GTK runs (or is still starting): false once it could not start, as with no display it can open.
pub(crate) fn alive() -> bool {
    start();
    STATE.load(Ordering::Acquire) != FAILED
}

/// On GTK's thread: whether GTK's display is X11's (the X11 webviews of wry stop the thread on any other).
pub(crate) fn on_x11() -> bool {
    use gtk::prelude::ObjectExt;
    ON_GTK.with(|on| on.get()) && gtk::gdk::Display::default().is_some_and(|display| display.type_().name() == "GdkX11Display")
}

/// Runs `job` on GTK's thread after everything sent before it; false when GTK could not start (`job` is dropped then).
/// Sent before GTK is up, it waits in GTK's queue and runs once the loop does.
pub(crate) fn run(job: impl FnOnce() + Send + 'static) -> bool {
    if !alive() {
        return false;
    }
    if ON_GTK.with(|on| on.get()) {
        job();
        return true;
    }
    let mut job = Some(job);
    // the default context is GTK's loop: only GTK's thread ever runs it. At the default priority, with the windows'
    // events, so a busy page does not hold the app's asks back as an idle one would be
    gtk::glib::idle_add_full(gtk::glib::Priority::DEFAULT, move || {
        if let Some(job) = job.take() {
            // a panic must not cross GTK's loop (that aborts the app): the job is lost, the thread and the tray go on
            let _ = std::panic::catch_unwind(std::panic::AssertUnwindSafe(job));
        }
        gtk::glib::ControlFlow::Break
    });
    true
}

/// Runs `job` on GTK's thread and waits for its answer, `timeout` at most: None when GTK is not there, the job
/// panicked or it took longer.
pub(crate) fn call<T: Send + 'static>(timeout: std::time::Duration, job: impl FnOnce() -> T + Send + 'static) -> Option<T> {
    let (answer, answered) = std::sync::mpsc::sync_channel(1);
    if !run(move || {
        let _ = answer.send(job());
    }) {
        return None;
    }
    answered.recv_timeout(timeout).ok()
}
