//! Pointer gestures on the canvas: pressing, drawing, moving, releasing (arrow binding: arrows.rs).

use crate::prelude::*;
mod gesture;
mod handle;

/// Screen points a press may wander before it becomes a drag.
const DRAG_SLOP: f32 = 4.0;

impl DrawingApp {
    pub(crate) fn press_select(&mut self, pos: Pos2, world: Pos2, modifiers: Modifiers) {
        // an IDE card's tree keeps the keys only while the presses land on it (ide_card_press)
        self.card_focus = None;
        // an enlarged window: presses inside it are its own; a margin press only puts it back (a
        // docked one stays, and the canvas around it takes the press)
        if let Some(i) = self.elevated_index() {
            let inside = self.terminal_frame(&self.scene.elements[i]).0.contains(pos);
            if inside || self.dock.is_none() {
                if !inside {
                    self.set_elevated(None);
                    self.terminal_focus = None;
                }
                // nor does its release open what lies on the canvas (a rope's popover)
                self.press_world = Pos2::new(f32::NAN, f32::NAN);
                return;
            }
        }
        // a group's pill picks all its members up
        if let Some(gid) = self.group_pill_at(pos) {
            self.press_group_pill(&gid);
            return;
        }
        if let Some((k, _)) = self.point_handles().into_iter().find(|(_, p)| p.distance(pos) <= 10.0) {
            self.snapshot();
            if let Some(i) = self.single_line() {
                let last = self.line_points(i).len() - 1;
                let ends = &mut self.scene.elements[i].ends;
                if k == 0 {
                    ends.start = None;
                } else if k == last {
                    ends.end = None;
                }
            }
            self.point_drag = Some(k);
            self.selected_point = Some(k);
            return;
        }
        // the point comes with the drag: a click on the handle, or a double-click that labels an arrow, adds none
        if let (Some(_), Some((k, _))) = (self.editable_line(), self.midpoint_handles().into_iter().find(|(_, p)| p.distance(pos) <= 8.0)) {
            self.midpoint_press = Some((k + 1, pos));
            return;
        }
        if let Some(line) = self.line_editor {
            if modifiers.alt {
                self.snapshot();
                if let Some(points) = self.line_points_mut(line) {
                    points.push([world.x, world.y]);
                    self.selected_point = Some(points.len() - 1);
                }
                return;
            }
            self.line_editor = None;
            self.selected_point = None;
            if self.hit(world) == Some(line) {
                self.moving = true;
                self.move_snapshotted = false;
                return;
            }
        }
        if let (true, Some((center, _, _))) = (self.hovered_rotation(pos), self.selection_frame()) {
            self.snapshot();
            self.detach_selected_arrows();
            self.rotating = Some(Rotate {
                pivot: center,
                start: (world - center).angle(),
                originals: self.selected.iter().map(|i| self.scene.elements[*i].clone()).collect(),
            });
        } else if let (Some(corner), Some((center, size, angle))) = (self.hovered_handle(pos), self.selection_frame()) {
            let from = Rect::from_center_size(center, size);
            let corners = [from.left_top(), from.right_top(), from.right_bottom(), from.left_bottom()];
            let local = center + rot(world - center, -angle);
            self.snapshot();
            self.detach_selected_arrows();
            self.resizing = Some(Resize {
                anchor: corners[(corner + 2) % 4],
                grab: corners[corner] - local,
                from,
                center,
                angle,
                originals: self.selected.iter().map(|i| self.scene.elements[*i].clone()).collect(),
                free: Vec2::splat(1.0),
                pin: corners[corner],
            });
        } else if let Some((i, corner, free)) = self.window_grip(pos) {
            // a window's edge or corner resizes it from the band, selected or not, before its body or title
            let from = local_bounds(&self.scene.elements[i]);
            let corners = [from.left_top(), from.right_top(), from.right_bottom(), from.left_bottom()];
            self.selected = self.with_card_text(&[i]);
            self.snapshot();
            self.detach_selected_arrows();
            self.resizing = Some(Resize {
                anchor: corners[(corner + 2) % 4],
                grab: corners[corner] - world,
                from,
                center: from.center(),
                angle: 0.0,
                originals: self.selected.iter().map(|i| self.scene.elements[*i].clone()).collect(),
                free,
                pin: corners[corner],
            });
        } else if let Some(i) = self.hit(world) {
            let i = self.raise_note(i);
            if !self.drags(i, world, modifiers.shift) {
                let e = &self.scene.elements[i];
                if e.terminal.is_some() {
                    self.focus_terminal(i);
                } else if e.ide.is_some() {
                    self.ide_card_press(i, world);
                } else if let Some(label) = note_label(&self.scene.elements, i).filter(|_| e.created.is_some()) {
                    // a note's body: a click on its text with no modifier types into it, on the
                    // release (`release`); elsewhere the press only selects it
                    self.selected = vec![i];
                    if modifiers.is_none() && self.on_note_text(i, world) {
                        self.note_press = Some(label);
                    }
                } else {
                    // a portal's body is its page's: selected, not picked up
                    self.selected = vec![i];
                }
                return;
            }
            let members: Vec<usize> = self.group_members(i).into_iter().filter(|j| !self.is_card_text(*j)).collect();
            if modifiers.shift {
                for j in members {
                    if self.selected.contains(&j) {
                        self.selected.retain(|x| *x != j);
                    } else {
                        self.selected.push(j);
                    }
                }
            } else if !self.selected.contains(&i) {
                self.selected = members;
            }
            self.moving = true;
            self.move_snapshotted = false;
        } else if !modifiers.shift && self.selection_bounds().is_some_and(|b| b.contains(world)) {
            self.moving = true;
            self.move_snapshotted = false;
        } else {
            if !modifiers.shift {
                self.selected.clear();
            }
            self.marquee = Some(world);
        }
    }

    /// A pressed sticky note comes to the front with its text, as a node does in the reference app
    /// (a fichário's pages keep their order); the note's new index.
    pub(crate) fn raise_note(&mut self, i: usize) -> usize {
        let e = &self.scene.elements[i];
        if e.created.is_none() || e.note.stacked {
            return i;
        }
        let mine = self.with_card_text(&[i]);
        let id = e.id.clone();
        let bounds = element_bounds(e);
        let covered = self.scene.elements.iter().enumerate().skip(i + 1).any(|(j, o)| !mine.contains(&j) && element_bounds(o).intersects(bounds));
        if !covered {
            return i;
        }
        self.keeping_selection(|app| {
            let (raised, rest): (Vec<_>, Vec<_>) = std::mem::take(&mut app.scene.elements).into_iter().enumerate().partition(|(j, _)| mine.contains(j));
            app.scene.elements = rest.into_iter().chain(raised).map(|(_, e)| e).collect();
        });
        self.dirty = true;
        self.scene.elements.iter().position(|e| e.id == id).unwrap_or(i)
    }

    /// Whether a world point is on element `i`'s title strip (a note's header, a portal's or IDE's bar).
    pub(crate) fn on_title(&self, i: usize, world: Pos2) -> bool {
        let e = &self.scene.elements[i];
        let head = if e.portal.is_some() {
            PORTAL_HEADER
        } else if e.filetree.is_some() {
            FT_BAR
        } else if e.created.is_some() {
            self.note_strip(i)
        } else {
            TERMINAL_HEADER
        };
        world.y - local_bounds(e).min.y < head
    }

    /// A note's strip that picks it up: its header, the rest of it is text ("Clique para editar…", as the reference
    /// app's); zoomed out, at least 16 screen px so it can still be caught, but at most 40% of the note.
    fn note_strip(&self, i: usize) -> f32 {
        (16.0 / self.zoom).max(NOTE_HEADER).min(0.4 * local_bounds(&self.scene.elements[i]).height())
    }

    /// Whether `world` is on note `i`'s text, under its header: an empty note is all text. Below the
    /// last line painted (`note_text_bottom`) or beside the column is not; an unpainted note is.
    pub(crate) fn on_note_text(&self, i: usize, world: Pos2) -> bool {
        let e = &self.scene.elements[i];
        let text = note_label(&self.scene.elements, i).and_then(|j| match &self.scene.elements[j].kind {
            ElementKind::Text { value, .. } => Some(value.trim().is_empty()),
            _ => None,
        });
        let bottom = self.note_text_bottom.borrow().get(&e.id).copied().unwrap_or(f32::INFINITY);
        let column = note_text_box(e).x_range();
        world.y - local_bounds(e).min.y >= NOTE_HEADER && (text.unwrap_or(true) || (column.contains(world.x) && world.y <= bottom))
    }

    /// Whether a press at `world` on element `i` picks it up. A window's body is its own (the
    /// terminal's keys, the IDE's card, the page, a note's text), so only its title strip drags
    /// it; with Shift every press selects and drags.
    pub(crate) fn drags(&self, i: usize, world: Pos2, shift: bool) -> bool {
        let e = &self.scene.elements[i];
        // (a chat's card has no body of its own: it is picked up anywhere)
        let window = (e.is_node() && e.chat.is_none()) || (e.created.is_some() && !e.note.content_locked);
        // the hit's halo just outside a window is not its body
        shift || !window || self.on_title(i, world) || !local_bounds(e).contains(world)
    }

    pub(crate) fn erase_at(&mut self, world: Pos2) {
        if let (Some(i), Some(erasing)) = (self.hit(world), &self.erasing) {
            if !erasing.contains(&i) {
                let members = self.group_members(i);
                if let Some(erasing) = &mut self.erasing {
                    erasing.extend(members.into_iter().filter(|j| !erasing.contains(j)).collect::<Vec<_>>());
                }
            }
        }
    }

    /// The element a rope from `from` reaches when it lands at `world`: a note's label counts as the
    /// note; the source itself, ropes, arrows and strokes reach nothing.
    pub(crate) fn connect_target(&self, from: &str, world: Pos2) -> Option<usize> {
        let hit = self.hit(world)?;
        let id = self.scene.elements[hit].id.trim_end_matches("-label");
        if id == from {
            return None;
        }
        self.scene.elements.iter().position(|e| e.id == id && !e.rope && bindable(e))
    }

    /// The node under the pointer while "Conectar" waits: highlighted, and where a click connects.
    pub(crate) fn connect_hover(&self) -> Option<usize> {
        let from = self.connecting.as_deref()?;
        self.connect_target(from, self.to_world(self.last_pointer?))
    }
}

/// The dropped paths as words to add to `draft`, quoted when they have spaces or quotes, and
/// set apart from what's typed before them.
pub(crate) fn save_pasted(img: &egui::ColorImage, dir: &std::path::Path) -> Option<PathBuf> {
    let [w, h] = img.size;
    let rgba: Vec<u8> = img.pixels.iter().flat_map(|c| c.to_srgba_unmultiplied()).collect();
    let path = dir.join(format!("magoagent-colado-{}.png", uuid::Uuid::new_v4().simple()));
    image::RgbaImage::from_raw(w as u32, h as u32, rgba)?.save(&path).ok()?;
    Some(path)
}

pub(crate) fn dropped_paths(draft: &str, paths: &[PathBuf]) -> String {
    // a name with a double quote (Linux allows it) goes in single ones, its own ' escaped
    let quote = |p: String| match (p.contains('"'), p.contains(char::is_whitespace)) {
        (true, _) => format!("'{}'", p.replace('\\', "\\\\").replace('\'', "\\'")),
        (false, true) => format!("\"{p}\""),
        _ => p,
    };
    let words: Vec<String> = paths.iter().map(|p| quote(p.display().to_string())).collect();
    let gap = if draft.is_empty() || draft.ends_with(char::is_whitespace) || words.is_empty() { "" } else { " " };
    format!("{gap}{}", words.join(" "))
}
