//! Pointer gestures on the canvas: pressing, drawing, moving, releasing (arrow binding: arrows.rs).

use crate::prelude::*;
mod gesture;
mod handle;

/// Screen points a press may wander before it becomes a drag.
const DRAG_SLOP: f32 = 4.0;

impl DrawingApp {
    pub(crate) fn press_select(&mut self, pos: Pos2, world: Pos2, modifiers: Modifiers) {
        // a file tree keeps the keys only while the presses land on it
        self.card_focus = None;
        // an enlarged window: presses inside it are its own; a margin press only puts it back (a
        // docked one stays, and the canvas around it takes the press)
        if let Some(index) = self.elevated_index() {
            let inside = self.terminal_frame(&self.scene.elements[index]).0.contains(pos);
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
        if let Some((point_index, _)) = self.point_handles().into_iter().find(|(_, point)| point.distance(pos) <= 10.0) {
            self.snapshot();
            if let Some(index) = self.single_line() {
                let last = self.line_points(index).len() - 1;
                let ends = &mut self.scene.elements[index].ends;
                if point_index == 0 {
                    ends.start = None;
                } else if point_index == last {
                    ends.end = None;
                }
            }
            self.point_drag = Some(point_index);
            self.selected_point = Some(point_index);
            return;
        }
        // the point comes with the drag: a click on the handle, or a double-click that labels an arrow, adds none
        if let (Some(_), Some((point_index, _))) = (self.editable_line(), self.midpoint_handles().into_iter().find(|(_, point)| point.distance(pos) <= 8.0)) {
            self.midpoint_press = Some((point_index + 1, pos));
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
                originals: self.selected.iter().map(|index| self.scene.elements[*index].clone()).collect(),
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
                originals: self.selected.iter().map(|index| self.scene.elements[*index].clone()).collect(),
                free: Vec2::splat(1.0),
                pin: corners[corner],
            });
        } else if let Some((index, corner, free)) = self.window_grip(pos) {
            // a window's edge or corner resizes it from the band, selected or not, before its body or title
            let from = local_bounds(&self.scene.elements[index]);
            let corners = [from.left_top(), from.right_top(), from.right_bottom(), from.left_bottom()];
            self.selected = self.with_card_text(&[index]);
            self.snapshot();
            self.detach_selected_arrows();
            self.resizing = Some(Resize {
                anchor: corners[(corner + 2) % 4],
                grab: corners[corner] - world,
                from,
                center: from.center(),
                angle: 0.0,
                originals: self.selected.iter().map(|index| self.scene.elements[*index].clone()).collect(),
                free,
                pin: corners[corner],
            });
        } else if let Some(index) = self.hit(world) {
            let index = self.raise_note(index);
            if !self.drags(index, world, modifiers.shift) {
                let element = &self.scene.elements[index];
                if element.terminal.is_some() {
                    self.focus_terminal(index);
                } else if let Some(label) = note_label(&self.scene.elements, index).filter(|_| element.created.is_some()) {
                    // a note's body: a click on its text with no modifier types into it, on the
                    // release (`release`); elsewhere the press only selects it
                    self.selected = vec![index];
                    if modifiers.is_none() && self.on_note_text(index, world) {
                        self.note_press = Some(label);
                    }
                } else if element.created.is_some() {
                    // a note left empty (its text was dropped): a click under the header gives it back and types
                    self.selected = vec![index];
                    if modifiers.is_none() && self.on_note_text(index, world) {
                        self.begin_text_edit(index);
                    }
                } else {
                    // a portal's body is its page's: selected, not picked up
                    self.selected = vec![index];
                }
                return;
            }
            let members: Vec<usize> = self.group_members(index).into_iter().filter(|member| !self.is_card_text(*member)).collect();
            if modifiers.shift {
                for member in members {
                    if self.selected.contains(&member) {
                        self.selected.retain(|x| *x != member);
                    } else {
                        self.selected.push(member);
                    }
                }
            } else if !self.selected.contains(&index) {
                self.selected = members;
            }
            self.moving = true;
            self.move_snapshotted = false;
        } else if !modifiers.shift && self.selection_bounds().is_some_and(|bounds| bounds.contains(world)) {
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
    pub(crate) fn raise_note(&mut self, index: usize) -> usize {
        let element = &self.scene.elements[index];
        if element.created.is_none() || element.note.stacked {
            return index;
        }
        let mine = self.with_card_text(&[index]);
        let id = element.id.clone();
        let bounds = element_bounds(element);
        let covered = self
            .scene
            .elements
            .iter()
            .enumerate()
            .skip(index + 1)
            .any(|(position, other)| !mine.contains(&position) && element_bounds(other).intersects(bounds));
        if !covered {
            return index;
        }
        self.keeping_selection(|app| {
            let (raised, rest): (Vec<_>, Vec<_>) =
                std::mem::take(&mut app.scene.elements).into_iter().enumerate().partition(|(position, _)| mine.contains(position));
            app.scene.elements = rest.into_iter().chain(raised).map(|(_, element)| element).collect();
        });
        self.dirty = true;
        self.scene.elements.iter().position(|element| element.id == id).unwrap_or(index)
    }

    /// Whether a world point is on element `index`'s title strip (a note's header, a portal's bar).
    pub(crate) fn on_title(&self, index: usize, world: Pos2) -> bool {
        let element = &self.scene.elements[index];
        let head = if element.portal.is_some() {
            PORTAL_HEADER
        } else if element.filetree.is_some() {
            FT_BAR
        } else if element.created.is_some() {
            self.note_strip(index)
        } else {
            TERMINAL_HEADER
        };
        world.y - local_bounds(element).min.y < head
    }

    /// A note's strip that picks it up: its header, the rest of it is text ("Clique para editar…", as the reference
    /// app's); zoomed out, at least 16 screen px so it can still be caught, but at most 40% of the note.
    fn note_strip(&self, index: usize) -> f32 {
        (16.0 / self.zoom).max(NOTE_HEADER).min(0.4 * local_bounds(&self.scene.elements[index]).height())
    }

    /// Whether a press at `world` on note `index` types in it: anywhere under its header (the caret goes where
    /// the text is, or after it).
    pub(crate) fn on_note_text(&self, index: usize, world: Pos2) -> bool {
        world.y - local_bounds(&self.scene.elements[index]).min.y >= NOTE_HEADER
    }

    /// Whether a press at `world` on element `index` picks it up. A window's body is its own (the
    /// terminal's keys, the IDE's card, the page, a note's text), so only its title strip drags
    /// it; with Shift every press selects and drags.
    pub(crate) fn drags(&self, index: usize, world: Pos2, shift: bool) -> bool {
        let element = &self.scene.elements[index];
        // (a chat's card has no body of its own: it is picked up anywhere)
        let window = (element.is_node() && element.chat.is_none()) || (element.created.is_some() && !element.note.content_locked);
        // the hit's halo just outside a window is not its body
        shift || !window || self.on_title(index, world) || !local_bounds(element).contains(world)
    }

    pub(crate) fn erase_at(&mut self, world: Pos2) {
        if let (Some(index), Some(erasing)) = (self.hit(world), &self.erasing) {
            if !erasing.contains(&index) {
                let members = self.group_members(index);
                if let Some(erasing) = &mut self.erasing {
                    erasing.extend(members.into_iter().filter(|member| !erasing.contains(member)).collect::<Vec<_>>());
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
        self.scene.elements.iter().position(|element| element.id == id && !element.rope && bindable(element))
    }

    /// The node under the pointer while "Conectar" waits: highlighted, and where a click connects.
    pub(crate) fn connect_hover(&self) -> Option<usize> {
        let from = self.connecting.as_deref()?;
        self.connect_target(from, self.to_world(self.last_pointer?))
    }
}

/// The dropped paths as words to add to `draft`, quoted when they have spaces or quotes, and
/// set apart from what's typed before them.
pub(crate) fn save_pasted(image: &egui::ColorImage, directory: &std::path::Path) -> Option<PathBuf> {
    let [width, height] = image.size;
    let rgba: Vec<u8> = image.pixels.iter().flat_map(|pixel| pixel.to_srgba_unmultiplied()).collect();
    let path = directory.join(format!("magoagent-colado-{}.png", uuid::Uuid::new_v4().simple()));
    image::RgbaImage::from_raw(width as u32, height as u32, rgba)?.save(&path).ok()?;
    Some(path)
}

pub(crate) fn dropped_paths(draft: &str, paths: &[PathBuf]) -> String {
    // a name with a double quote (Linux allows it) goes in single ones, its own ' escaped
    let quote = |path: String| match (path.contains('"'), path.contains(char::is_whitespace)) {
        (true, _) => format!("'{}'", path.replace('\\', "\\\\").replace('\'', "\\'")),
        (false, true) => format!("\"{path}\""),
        _ => path,
    };
    let words: Vec<String> = paths.iter().map(|path| quote(path.display().to_string())).collect();
    let gap = if draft.is_empty() || draft.ends_with(char::is_whitespace) || words.is_empty() { "" } else { " " };
    format!("{gap}{}", words.join(" "))
}
