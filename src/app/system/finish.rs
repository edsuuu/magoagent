//! Linux on X11 (XWayland in a Wayland session): a frame goes to the screen only once the GPU has drawn all of it. The
//! swap does not wait for the screen there (main.rs), and XWayland took the window's picture while the GPU was still
//! drawing it: now and then a frame showed the canvas without the sidebar and the bar's buttons, or the whole window
//! gray. Recorded on an NVIDIA card during a zoom: 61 of 1440 frames so, none with the wait.

use crate::prelude::*;

/// An egui plugin: every frame of every window ends with a shape that waits for the GPU (glFinish), drawn after all the
/// others, before eframe swaps.
pub(crate) struct FinishFrames;

impl egui::Plugin for FinishFrames {
    fn debug_name(&self) -> &'static str {
        "finish_frames"
    }

    fn output_hook(&mut self, ctx: &Context, output: &mut egui::FullOutput) {
        output.shapes.push(finish_shape(ctx.viewport_rect()));
    }
}

/// The shape that waits for the GPU, over `rect` (the window's: egui skips a callback with nothing to draw over).
pub(crate) fn finish_shape(rect: Rect) -> egui::epaint::ClippedShape {
    let finish = eframe::egui_glow::CallbackFn::new(|_, painter| {
        use eframe::glow::HasContext;
        // SAFETY: a plain GL call on the context eframe made current to paint this frame
        unsafe { painter.gl().finish() };
    });
    let callback = egui::PaintCallback { rect, callback: std::sync::Arc::new(finish) };
    egui::epaint::ClippedShape::new(rect, egui::Shape::Callback(callback))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The wait is the frame's last shape, whatever the app drew, and it covers the window, so egui keeps it.
    #[test]
    fn every_frame_ends_waiting_for_the_gpu() {
        let ctx = Context::default();
        ctx.add_plugin(FinishFrames);
        let screen = Rect::from_min_size(Pos2::ZERO, Vec2::new(800.0, 600.0));
        let output = ctx.run_ui(egui::RawInput { screen_rect: Some(screen), ..Default::default() }, |ui| {
            ui.label("um texto");
        });
        let last = output.shapes.last().expect("a shape");
        assert!(matches!(last.shape, egui::Shape::Callback(_)), "{:?}", last.shape);
        assert_eq!(last.clip_rect, screen);
        assert!(output.shapes.len() > 1, "and the app's own shapes before it");
    }
}
