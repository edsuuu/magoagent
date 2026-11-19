//! A resting note whose text is longer than its card: the wheel over its text scrolls it, as a window's own wheel does
//! (Ctrl + wheel still zooms the canvas), and a thin bar at its right edge says where it is (render/mod.rs
//! `paint_note`). Kept for the session, by note id; typing, the editor scrolls instead.

use crate::prelude::*;

/// A note's text scrolled: how far down (`offset`) and how much of it the card hid when last painted (`overflow`),
/// both in the note's points.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub(crate) struct NoteScroll {
    pub(crate) offset: f32,
    pub(crate) overflow: f32,
}

impl DrawingApp {
    /// How far note `id`'s text is scrolled for this frame's paint: never past what the card hid last frame.
    pub(crate) fn note_scroll_offset(&self, id: &str) -> f32 {
        self.note_scroll.borrow().get(id).map_or(0.0, |scroll| scroll.offset.min(scroll.overflow).max(0.0))
    }

    /// A note's text painted, ending at `end` (its points, unscrolled): what the card hides now. A note whose text fits
    /// is forgotten.
    pub(crate) fn note_painted(&self, note: &Element, end: f32) {
        let overflow = end - note_text_box(note).max.y;
        let mut scrolls = self.note_scroll.borrow_mut();

        if overflow > 0.5 {
            let scroll = scrolls.entry(note.id.clone()).or_default();

            (scroll.overflow, scroll.offset) = (overflow, scroll.offset.min(overflow));
        } else {
            scrolls.remove(&note.id);
        }
    }

    /// The wheel over the text of a resting note the card cuts: the text scrolls, and true (the canvas leaves the wheel
    /// alone). Over its header, with Ctrl, or over a note whose text fits, false: the canvas's.
    pub(crate) fn note_wheel(&mut self, ctx: &Context) -> bool {
        let (delta, command, pointer) = ctx.input(|input| (input.smooth_scroll_delta.y, input.modifiers.command, input.pointer.hover_pos()));
        let Some(pointer) = pointer.filter(|_| delta != 0.0 && !command) else { return false };

        // over the window enlarged in the middle, nothing of the canvas behind it scrolls
        if self.elevated_index().is_some_and(|index| self.terminal_frame(&self.scene.elements[index]).0.contains(pointer)) {
            return false;
        }

        let world = self.to_world(pointer);
        let Some(index) = self.hit(world).filter(|index| self.scene.elements[*index].created.is_some()) else { return false };

        if !self.on_note_text(index, world) {
            return false;
        }

        let (id, zoom) = (self.scene.elements[index].id.clone(), self.terminal_frame(&self.scene.elements[index]).1);
        let mut scrolls = self.note_scroll.borrow_mut();
        let Some(scroll) = scrolls.get_mut(&id).filter(|scroll| scroll.overflow > 0.0) else { return false };

        scroll.offset = (scroll.offset - delta / zoom).clamp(0.0, scroll.overflow);
        true
    }
}
