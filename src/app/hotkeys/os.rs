//! The global shortcuts in the system: on Windows, RegisterHotKey on a thread of their own, which hears them and
//! tells the app. Nothing is sent to the system and the pointer is never touched: the keys are only heard. The tests
//! never register one (`listen` is not even built for them); other systems register none yet.
// shortcut: Windows only (macOS and Linux show "só no Windows" in Ajustes), add RegisterEventHotKey / XGrabKey when the island is used there

use crate::prelude::*;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::Receiver;

/// What the listener's thread tells the app.
#[cfg_attr(not(windows), allow(dead_code))]
pub(crate) enum Event {
    /// Once, as it starts: each action (its index in `ACTIONS`) and whether the system took its keys.
    Registered(Vec<(usize, bool)>),
    /// The shortcut of this action was pressed.
    Pressed(usize),
}

/// The thread that holds the shortcuts: dropped, it lets them go and ends before the drop returns, so the next one
/// finds the keys free.
pub(crate) struct Listener {
    stop: Arc<AtomicBool>,
    thread: Option<std::thread::JoinHandle<()>>,
    pub(crate) events: Receiver<Event>,
}

impl Drop for Listener {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}

impl Listener {
    /// A listener with no thread, whose events the test sends.
    #[cfg(test)]
    pub(crate) fn fake(events: Receiver<Event>) -> Listener {
        Listener { stop: Arc::default(), thread: None, events }
    }
}

/// Registers `keys` (the action's index and its combination) and hears them until dropped; `wake` is called after each
/// event, for the app to take it. None where the system registers no shortcuts.
#[cfg(not(all(windows, not(test))))]
pub(crate) fn listen(_keys: Vec<(usize, hotkeys::Combo)>, _wake: impl Fn() + Send + 'static) -> Option<Listener> {
    None
}

#[cfg(all(windows, not(test)))]
pub(crate) fn listen(keys: Vec<(usize, hotkeys::Combo)>, wake: impl Fn() + Send + 'static) -> Option<Listener> {
    use std::time::Duration;

    #[repr(C)]
    #[derive(Default)]
    struct Message {
        window: isize,
        message: u32,
        wparam: usize,
        lparam: isize,
        time: u32,
        point: [i32; 2],
        private: u32,
    }
    #[link(name = "user32")]
    unsafe extern "system" {
        fn RegisterHotKey(window: isize, id: i32, modifiers: u32, key: u32) -> i32;
        fn UnregisterHotKey(window: isize, id: i32) -> i32;
        fn PeekMessageW(message: *mut Message, window: isize, first: u32, last: u32, remove: u32) -> i32;
    }
    const MOD_ALT: u32 = 1;
    const MOD_CONTROL: u32 = 2;
    const MOD_SHIFT: u32 = 4;
    const MOD_WIN: u32 = 8;
    const MOD_NOREPEAT: u32 = 0x4000;
    const WM_HOTKEY: u32 = 0x0312;
    const PM_REMOVE: u32 = 1;

    let (sender, events) = std::sync::mpsc::channel();
    let stop = Arc::new(AtomicBool::new(false));
    let stopped = stop.clone();
    let hear = move || {
        // registered and heard here: WM_HOTKEY goes to the queue of the thread that registered
        let registered: Vec<(usize, bool)> = keys
            .iter()
            .map(|(index, combo)| {
                let modifiers = [(combo.ctrl, MOD_CONTROL), (combo.alt, MOD_ALT), (combo.shift, MOD_SHIFT), (combo.win, MOD_WIN)];
                let modifiers = modifiers.into_iter().filter(|(held, _)| *held).fold(MOD_NOREPEAT, |all, (_, flag)| all | flag);
                let key = hotkeys::keys::virtual_key(combo.key).unwrap_or_default();
                // SAFETY: no window (the thread's own queue), an id of ours and plain numbers
                (*index, key != 0 && unsafe { RegisterHotKey(0, *index as i32 + 1, modifiers, key) } != 0)
            })
            .collect();
        let _ = sender.send(Event::Registered(registered.clone()));
        wake();
        let mut message = Message::default();
        // shortcut: the queue is looked at every 15 ms instead of waiting on it, so stopping needs no message posted; GetMessageW and PostThreadMessageW if the wake-ups ever matter
        while !stopped.load(Ordering::Relaxed) {
            // SAFETY: a MSG of ours to fill in, only this thread's WM_HOTKEY taken from its queue
            while unsafe { PeekMessageW(&mut message, 0, WM_HOTKEY, WM_HOTKEY, PM_REMOVE) } != 0 {
                let _ = sender.send(Event::Pressed(message.wparam.wrapping_sub(1)));
                wake();
            }
            std::thread::sleep(Duration::from_millis(15));
        }
        for (index, _) in registered.into_iter().filter(|(_, taken)| *taken) {
            // SAFETY: an id this thread registered
            unsafe { UnregisterHotKey(0, index as i32 + 1) };
        }
    };
    let thread = std::thread::Builder::new().name("magoagent-atalhos".into()).spawn(hear).ok()?;
    Some(Listener { stop, thread: Some(thread), events })
}
