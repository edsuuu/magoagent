//! The wheel and the pinch over the canvas: they pan and zoom it, a long note's text scrolls under them, and the
//! windows on it (a tree, a chat card, a focused terminal) scroll their own content. A gesture that started on the
//! canvas stays the canvas's while it lasts, over whatever window passes under the pointer, as a browser latches a
//! wheel to what it started on (asked: "se eu estou scrollando pelo canvas e sem querer deixo o mouse em cima de algum
//! item, chat ou sei lá, ele foca no elemento e não consigo scrollar mais"); the window takes the wheel once the gesture
//! pauses or after a click. Ctrl + wheel and the pinch zoom the canvas whoever has the keys, over a window too ("se eu
//! não clicar no canvas para pegar o foco, eu não consigo segurar Control e tirar o zoom"), and an enlarged file tree
//! zooms itself ("se eu estou com o foco nela e tento dar zoom no código, não funciona").

use crate::prelude::*;

/// How long after the last wheel the canvas took a gesture is still its own (a browser's latching, about the same).
pub(crate) const WHEEL_LATCH: f64 = 0.4;

/// The zoom an enlarged tree goes between, by steps of a tenth.
const LIFTED_ZOOM: std::ops::RangeInclusive<f32> = 0.5..=3.0;

impl DrawingApp {
    /// This frame's wheel and pinch over the canvas (`rect`, its `response`); what it takes, the windows drawn after it
    /// no longer see.
    pub(crate) fn canvas_wheel(&mut self, ctx: &Context, response: &egui::Response, rect: Rect) {
        if self.chat_open || ctx.current_pass_index() > 0 {
            return;
        }
        let now = ctx.input(|input| input.time);
        // the zoom at rest again: its text is laid out at its size once more (`text_zoom`)
        let settles = crate::app::canvas::editing::ZOOM_SETTLES - (now - self.zoomed_at);
        if settles > 0.0 {
            ctx.request_repaint_after(std::time::Duration::from_secs_f64(settles + 0.01));
        }
        let (scroll, zoom, pointer, pressed, pinch, shift) = ctx.input(|input| {
            let pinch = input.multi_touch().is_some() || input.events.iter().any(|event| matches!(event, egui::Event::Zoom(_)));
            (input.smooth_scroll_delta, input.zoom_delta(), input.pointer.hover_pos(), input.pointer.any_pressed(), pinch, input.modifiers.shift)
        });
        // a click ends the canvas's gesture: the window under it takes the next wheel
        if pressed {
            self.wheel_latch = None;
        }
        let Some(pointer) = pointer.filter(|pointer| rect.contains(*pointer)) else { return };
        if scroll == Vec2::ZERO && zoom == 1.0 {
            return;
        }
        if let Some(mouse) = ctx.input(|input| crate::app::update::wheel_from_mouse(&input.raw.events)) {
            self.wheel_is_mouse = mouse;
        }
        // Ctrl + wheel or the pinch over an enlarged tree: its own zoom (the canvas is behind it)
        if zoom != 1.0 && self.lifted_tree_at(pointer) {
            self.lifted_zoom_by(if pinch { zoom.ln() / 0.1 } else { zoom.ln() / 0.2 });
            return;
        }
        // nothing over the canvas that has the wheel for itself: an enlarged window, a menu, a dialog
        let free = self.terminal_elevated.is_none() && !egui::Popup::is_any_open(ctx) && self.focus(ctx) != crate::app::canvas::shortcuts::Focus::Dialog;
        let latched = free && self.wheel_latch.is_some_and(|at| now - at < WHEEL_LATCH);
        let over_window = free && !response.hovered() && zoom != 1.0;
        // the wheel over a long note's text scrolls it, not the canvas (notes/scroll.rs), unless the canvas is moving
        if !(response.hovered() || over_window || latched) || (response.hovered() && !latched && self.note_wheel(ctx)) {
            return;
        }
        let wheel_zooms = if self.prefs.wheel_auto { self.wheel_is_mouse } else { self.prefs.mouse_wheel_zooms };
        if zoom != 1.0 && !pinch {
            // Ctrl + wheel: egui makes a notch a factor of e^0.2 (40 points at 1/200); the reference app adds a step per
            // notch instead. The notch's last frames still zoom after Ctrl is let go (egui keeps the gesture's keys)
            self.wheel_step(zoom.ln() / 0.2, pointer);
        } else if zoom != 1.0 {
            self.wheel_zoom(zoom, pointer);
        } else if wheel_zooms && !shift {
            // a mouse: the wheel zooms, a notch (40 points) a step; the side wheel (tilt) pans sideways
            self.glide = None;
            self.pan.x += scroll.x;
            if scroll.y != 0.0 {
                self.wheel_step(scroll.y / 40.0, pointer);
            }
        } else {
            self.glide = None;
            self.pan += scroll;
        }
        self.wheel_latch = Some(now);
        ctx.input_mut(|input| input.smooth_scroll_delta = Vec2::ZERO);
    }

    /// An enlarged or docked file tree under `pointer` (screen).
    fn lifted_tree_at(&self, pointer: Pos2) -> bool {
        self.elevated_index()
            .map(|index| &self.scene.elements[index])
            .is_some_and(|element| element.filetree.is_some() && self.terminal_frame(element).0.contains(pointer))
    }

    /// The enlarged tree's zoom, `notches` tenths up (or down), between half and three times.
    pub(crate) fn lifted_zoom_by(&mut self, notches: f32) {
        self.set_lifted_zoom(self.lifted_zoom + notches.clamp(-4.0, 4.0) * 0.1);
    }

    /// The enlarged tree's zoom at `zoom`, kept between half and three times (focus_canvas sets it too).
    pub(crate) fn set_lifted_zoom(&mut self, zoom: f32) {
        self.lifted_zoom = zoom.clamp(*LIFTED_ZOOM.start(), *LIFTED_ZOOM.end());
    }

    /// Aproximar, Afastar and Voltar a 100% (Ctrl + =, Ctrl + -, Ctrl + 0): an enlarged tree's own zoom, else the
    /// canvas's. They reach the canvas from a tree or a text field too (which have no use for them), not from a
    /// terminal (whose shell reads them), a dialog or the open chat.
    pub(crate) fn zoom_shortcuts(&mut self, ctx: &Context, zoom_in: bool, zoom_out: bool, reset: bool) {
        use crate::app::canvas::shortcuts::Focus;
        if !(zoom_in || zoom_out || reset) || !matches!(self.focus(ctx), Focus::Canvas | Focus::Tree | Focus::EguiField | Focus::CanvasText) {
            return;
        }
        if self.dialog_open() || self.palette_open || self.chat_open || self.terminal_focus.is_some() {
            return;
        }
        let tree = self.elevated_index().is_some_and(|index| self.scene.elements[index].filetree.is_some());
        match (tree, zoom_in, zoom_out) {
            (true, true, _) => self.lifted_zoom_by(1.0),
            (true, _, true) => self.lifted_zoom_by(-1.0),
            (true, ..) => self.lifted_zoom = 1.0,
            (false, true, _) => self.zoom_step(0.1),
            (false, _, true) => self.zoom_step(-0.1),
            (false, ..) => {
                let center = self.canvas_rect.center();
                self.zoom_at(1.0, center);
            }
        }
    }
}

/// Linux on X11: winit hands the keyboard's modifiers only to the window that has the keyboard, so over the app's
/// window while another had it (the island's, another app's) a Ctrl + wheel scrolled instead of zooming. The X server
/// knows them: each wheel there carries the ones `held` gives. Windows and macOS read them from the wheel's own event
/// (winit), so there nothing changes. ponytail: under XWayland the X server knows the keys only while an X11 window
/// has the keyboard (the island does; a Wayland app does not).
pub(crate) fn wheel_modifiers(events: &mut Vec<egui::Event>, focused: bool, current: Modifiers, held: impl FnOnce() -> Option<Modifiers>) {
    if focused {
        return;
    }
    let Some(first) = events.iter().position(|event| matches!(event, egui::Event::MouseWheel { .. })) else { return };
    let Some(held) = held() else { return };
    for event in events.iter_mut() {
        if let egui::Event::MouseWheel { modifiers, .. } = event {
            *modifiers = held;
        }
    }
    if held != current {
        events.insert(first, egui::Event::ModifiersChanged(held));
    }
}

/// The modifiers the X server says are down (Linux on X11; None without a server).
#[cfg(target_os = "linux")]
pub(crate) fn system_modifiers() -> Option<Modifiers> {
    use std::sync::{Mutex, OnceLock};
    use x11rb::connection::Connection as _;
    use x11rb::protocol::xproto::{ConnectionExt as _, KeyButMask};
    static LINK: OnceLock<Mutex<Option<(x11rb::rust_connection::RustConnection, u32)>>> = OnceLock::new();
    if !crate::app::island::x11::active() {
        return None;
    }
    let link = LINK.get_or_init(|| {
        Mutex::new(x11rb::connect(None).ok().map(|(connection, screen)| {
            let root = connection.setup().roots[screen].root;
            (connection, root)
        }))
    });
    let guard = link.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
    let (connection, root) = guard.as_ref()?;
    let mask = connection.query_pointer(*root).ok()?.reply().ok()?.mask;
    let (ctrl, shift, alt) = (mask.contains(KeyButMask::CONTROL), mask.contains(KeyButMask::SHIFT), mask.contains(KeyButMask::MOD1));
    Some(Modifiers { alt, ctrl, shift, mac_cmd: false, command: ctrl })
}

/// Windows and macOS: the wheel's own event already says the keys held (winit).
#[cfg(not(target_os = "linux"))]
pub(crate) fn system_modifiers() -> Option<Modifiers> {
    None
}
