//! The main window's shell in SwiftUI (macOS): `native_shell` publishes topic "shell" (title, tool, properties, sidebar,
//! dialogs, toast...) and `native_shell_act` answers the `ui.shell.*` verbs, which call what the egui chrome calls.
//!
//! The egui main window steps aside (`crate::step_aside_window`): see-through, deaf to the mouse, but still ordered in, so
//! eframe keeps running `ui` at the full rate and the core (bridge, agents, autosave) ticks as always. A hidden window
//! paints once in 400 ms (`logic`), which would slow all of that. The egui chrome keeps drawing unseen: its state (the
//! sidebar's slide, the toast's clock) is what this reads.

mod act;
mod model;
#[cfg(test)]
mod tests;

use crate::prelude::*;
use std::sync::atomic::{AtomicBool, Ordering};

static ASIDE: AtomicBool = AtomicBool::new(false);

// the two entry points below are called by the native layer's per-frame and verb wiring (src/app/native)
#[allow(dead_code)]
impl DrawingApp {
    /// Called every frame while the shell is on: hides the egui window once and publishes what Swift draws.
    pub(crate) fn native_shell(&mut self) {
        if !ASIDE.load(Ordering::Relaxed) {
            ASIDE.store(crate::step_aside_window(true), Ordering::Relaxed);
        }
        crate::app::native::publish("shell", &self.shell_model());
    }

    /// The `ui.shell.<verb>` Swift sent.
    pub(crate) fn native_shell_act(&mut self, verb: &str, args: &Value) -> Result<String, String> {
        self.shell_act(verb.strip_prefix("ui.shell.").unwrap_or(verb), args)
    }
}
