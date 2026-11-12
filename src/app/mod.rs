//! The window: `DrawingApp` state and the per-frame loop.

use crate::prelude::*;

pub(crate) mod browser;
pub(crate) mod canvas;
pub(crate) mod chat;
pub(crate) mod filetree;
pub(crate) mod floors;
pub(crate) mod groups;
mod init;
mod maestro;
pub(crate) mod mascot;
mod mcp_actions;
pub(crate) mod notes;
mod state;
pub(crate) mod system;
mod tasks;
pub(crate) mod terminal;
#[cfg(test)]
mod tests;
pub(crate) mod ui;
mod update;
pub(crate) mod usage;
pub(crate) mod voice;
pub(crate) mod workspaces;
pub(crate) use state::*;

impl DrawingApp {
    pub(crate) fn toast(&mut self, message: impl Into<String>) {
        self.status = message.into();
        self.status_time = -1.0;
    }

    pub(crate) fn snapshot(&mut self) {
        self.undo.push(self.scene.clone());
        if self.undo.len() > 100 {
            self.undo.remove(0);
        }
        self.redo.clear();
        self.dirty = true;
    }

    /// A press still held on the canvas: the scene and the selection's indices are mid-change, so
    /// the file merge, the loose-rope cleanup and the shortcuts wait for the release.
    pub(crate) fn in_gesture(&self) -> bool {
        self.moving
            || self.panning
            || self.resizing.is_some()
            || self.rotating.is_some()
            || self.point_drag.is_some()
            || self.midpoint_press.is_some()
            || self.drawing_start.is_some()
            || self.erasing.is_some()
            || self.marquee.is_some()
            || self.lasso.is_some()
            || self.note_press.is_some()
    }

    /// Esc mid-drag: what the drag changed goes back to before the press, and the drag ends.
    pub(crate) fn cancel_gesture(&mut self) {
        // these took a snapshot when they began changing the scene
        let changed = self.move_snapshotted || self.resizing.is_some() || self.rotating.is_some() || self.point_drag.is_some();
        // only then: a marquee or a pan took none, and the last step must stay
        if let Some(previous) = changed.then(|| self.undo.pop()).flatten() {
            self.scene = previous;
            self.dirty = true;
        }
        self.moving = false;
        self.move_snapshotted = false;
        self.move_origin = None;
        self.panning = false;
        self.resizing = None;
        self.rotating = None;
        self.point_drag = None;
        self.midpoint_press = None;
        self.drawing_start = None;
        self.drawing_points.clear();
        self.erasing = None;
        self.marquee = None;
        self.lasso = None;
        self.snap_guides.clear();
        self.note_press = None;
    }

    pub(crate) fn undo(&mut self) {
        if let Some(previous) = self.undo.pop() {
            self.redo.push(std::mem::replace(&mut self.scene, previous));
            self.selected.clear();
            self.dirty = true;
        }
    }

    pub(crate) fn redo(&mut self) {
        if let Some(next) = self.redo.pop() {
            self.undo.push(std::mem::replace(&mut self.scene, next));
            self.selected.clear();
            self.dirty = true;
        }
    }

    pub(crate) fn new_element(&self, kind: ElementKind) -> Element {
        let mut style = self.style;
        if matches!(kind, ElementKind::Line { .. } | ElementKind::FreeDraw { .. }) {
            // arrows default to curved, lines and freehand to sharp (reference defaults)
            style.round = matches!(kind, ElementKind::Line { arrow: true, .. }) && self.arrow_curved;
            style.fill = None;
        }
        Element {
            id: new_id(),
            group_id: None,
            frame_group: None,
            kind,
            angle: 0.0,
            ends: ArrowEnds { start_head: self.heads.0, end_head: self.heads.1, ..Default::default() },
            locked: false,
            frame: None,
            image: None,
            created: None,
            terminal: None,
            portal: None,
            legacy_ide: None,
            filetree: None,
            chat: None,
            note: NoteSpec::default(),
            rope: false,
            style,
        }
    }

    /// A sticky note over `rect` and its label `<id>-label`, grouped, the label at its center.
    pub(crate) fn new_note(&self, rect: Rect, text: String) -> [Element; 2] {
        let mut note = self.new_element(ElementKind::Rectangle { x: rect.min.x, y: rect.min.y, width: rect.width(), height: rect.height() });
        note.style = sticky_style(note.style);
        note.created = Some(now_ms());
        note.group_id = Some(new_id());
        let mut label = self.new_element(ElementKind::Text { x: rect.center().x, y: rect.center().y, value: text });
        label.id = format!("{}-label", note.id);
        label.group_id = note.group_id.clone();
        label.style.stroke = note.style.stroke;
        [note, label]
    }

    pub(crate) fn origin(&self) -> Vec2 {
        self.canvas_rect.min.to_vec2() + self.pan
    }

    pub(crate) fn to_world(&self, screen: Pos2) -> Pos2 {
        (screen - self.origin()) / self.zoom
    }

    pub(crate) fn to_screen(&self, world: Pos2) -> Pos2 {
        world * self.zoom + self.origin()
    }
}
