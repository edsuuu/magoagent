//! What egui draws over the canvas besides the scene, for SwiftUI: the enlarged window (its frame, the blur behind it,
//! its skeleton in its place, the grip that docks it), the minimap, the drawings' link marks and "Voltar".

use super::*;
use crate::app::canvas::dock::DOCK_MARGIN;
use crate::app::canvas::lifted::{SKELETON_LINES, skeleton_name};
use crate::app::canvas::minimap::{MAP_SIZE, MapDot, MinimapArea, dot, to_map, to_world};

/// The window enlarged, or shrinking back (canvas/lifted.rs, canvas/dock.rs).
#[derive(Serialize, Clone, Debug, PartialEq)]
pub(crate) struct Lift {
    pub(super) id: String,
    /// Where it shows in the canvas view [x, y, width, height] and at what scale (`terminal_frame`).
    frame: [f32; 4],
    pub(super) zoom: f32,
    /// How much the canvas behind it is blurred: 1 in the middle, 0 docked at a side.
    blur: f32,
    #[serde(skip_serializing_if = "Option::is_none")]
    skeleton: Option<Skeleton>,
    /// Its title as a handle (screen): dragged it docks, a double click puts it back.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) grip: Option<[f32; 4]>,
    /// Docked, the inner edge that sets its width (screen).
    #[serde(skip_serializing_if = "Option::is_none")]
    edge: Option<[f32; 4]>,
    /// Where it would dock, while its title is dragged (screen).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) zone: Option<[f32; 4]>,
}

/// The window's place on the canvas while it is enlarged (`paint_skeleton`): its box in the world, the title strip's
/// height, its name, the words in the middle and the share of the width of each grey bar.
#[derive(Serialize, Clone, Debug, PartialEq)]
pub(crate) struct Skeleton {
    rect: [f32; 4],
    head: f32,
    name: String,
    words: &'static str,
    lines: [f32; 6],
}

/// The minimap (ui/menus.rs `minimap`): the panel's size, the surface's inset in it, and in the surface's points the
/// drawings' dots and the view.
#[derive(Serialize, Clone, Debug, PartialEq)]
pub(crate) struct Minimap {
    size: [f32; 2],
    inset: f32,
    /// From the canvas's bottom right corner to the panel's.
    right: f32,
    bottom: f32,
    pub(super) nodes: Vec<MapNode>,
    view: [f32; 4],
    accent: [u8; 4],
}

#[derive(Serialize, Clone, Debug, PartialEq)]
pub(crate) struct MapNode {
    pub(super) rect: [f32; 4],
    #[serde(skip_serializing_if = "Option::is_none")]
    fill: Option<[u8; 4]>,
    #[serde(skip_serializing_if = "Option::is_none")]
    stroke: Option<[u8; 4]>,
}

/// A drawing's link mark (filetree/link.rs `element_links_ui`): it sits at the corner of `bounds` (world).
#[derive(Serialize, Clone, Debug, PartialEq)]
pub(crate) struct LinkMark {
    pub(super) id: String,
    link: String,
    bounds: [f32; 4],
}

/// The marks' colors: the island, the island under the pointer, the chain.
#[derive(Serialize, Clone, Debug, Default, PartialEq)]
pub(crate) struct Chain {
    fill: [u8; 4],
    hover: [u8; 4],
    ink: [u8; 4],
}

/// The surface's inset in the minimap's panel (its 6 points of padding).
const MAP_INSET: f32 = 4.875;

impl DrawingApp {
    pub(crate) fn native_lift(&self) -> Option<Lift> {
        let index = self.lift_index()?;
        let element = &self.scene.elements[index];
        let (frame, zoom) = self.terminal_frame(element);
        let blur = self.blurred_for().filter(|(id, _)| *id == element.id).map_or(0.0, |(_, shown)| shown);
        let enlarged = self.terminal_elevated.as_ref() == Some(&element.id);

        let skeleton = enlarged.then(|| Skeleton {
            rect: rect_of(local_bounds(element)),
            head: TERMINAL_HEADER,
            name: skeleton_name(element),
            words: if self.dock.is_some() { "Aberto ao lado" } else { "Em tela cheia" },
            lines: SKELETON_LINES,
        });

        // (height, right) of the strip, as dock_grip's
        let (head, right) = match () {
            _ if element.portal.as_ref().is_some_and(|portal| portal.chrome_hidden) => (0.0, 48.0),
            _ if element.portal.is_some() => (PORTAL_HEADER * zoom, 48.0),
            _ if element.created.is_some() => (NOTE_HEADER * zoom, 0.0),
            _ if element.filetree.is_some() => (FT_BAR * zoom, 0.0),
            _ => (TERMINAL_HEADER * zoom, 48.0),
        };

        let grip = Rect::from_min_max(frame.min, Pos2::new(frame.right() - right, frame.top() + head));
        let grip = (enlarged && !self.elevate_animating() && grip.is_positive()).then(|| rect_of(grip));
        let zone = self.dock_drag.and_then(|(_, pointer)| self.dock_side(pointer.x, element)).map(|(side, width)| rect_of(self.dock_rect(side, width)));

        let edge = self.dock.filter(|_| enlarged && self.dock_drag.is_none()).map(|(side, _)| {
            let edge = if side { frame.left() } else { frame.right() };

            rect_of(Rect::from_center_size(Pos2::new(edge, frame.center().y), Vec2::new(8.0, frame.height())))
        });

        Some(Lift { id: element.id.clone(), frame: rect_of(frame), zoom, blur, skeleton, grip, edge, zone })
    }

    /// The minimap while it is open over the canvas (toolbar.rs), its area held by `area` while it is pressed.
    pub(crate) fn native_minimap(&self, area: &mut MinimapArea) -> Option<Minimap> {
        if !self.prefs.minimap || self.focused_window().is_some() || self.chat_open {
            return None;
        }

        let dark = self.dark();
        let accent = glass_accent(dark);
        let shown = area.area(self.now, self.minimap_world());

        if area.settling() {
            crate::app::native::repaint();
        }

        let map = |rect: Rect| dot(to_map(shown, Pos2::ZERO, rect.min), to_map(shown, Pos2::ZERO, rect.max));

        let nodes = self
            .minimap_nodes()
            .into_iter()
            .map(|(rect, kind)| {
                let (fill, stroke) = match kind {
                    MapDot::Window => (Some(accent), None),
                    MapDot::Paper(paper) => (Some(rgba(paper).gamma_multiply(0.8)), None),
                    MapDot::Shape => (None, Some(glass_ink(dark).gamma_multiply(0.55))),
                };

                MapNode { rect: rect_of(map(rect)), fill: fill.map(color), stroke: stroke.map(color) }
            })
            .collect();

        let view = self.visible_world();
        let view = Rect::from_two_pos(to_map(shown, Pos2::ZERO, view.min), to_map(shown, Pos2::ZERO, view.max));

        Some(Minimap {
            size: [MAP_SIZE.x, MAP_SIZE.y],
            inset: MAP_INSET,
            right: CORNER_GAP + self.corner_inset().1,
            bottom: 60.0,
            nodes,
            view: rect_of(view),
            accent: color(accent),
        })
    }

    /// A press on the minimap's surface at `at` (its points): the view goes there, the map held still while it is down.
    pub(crate) fn native_minimap_press(&mut self, area: &mut MinimapArea, down: bool, at: Pos2) {
        let shown = area.area(self.now, self.minimap_world());

        area.press(down, self.now, shown);

        if down {
            let surface = MAP_SIZE - Vec2::splat(2.0 * MAP_INSET);

            self.minimap_go(to_world(shown, Pos2::ZERO, at.clamp(Pos2::ZERO, surface.to_pos2())));
        }
    }

    /// The drawings with a link, while nothing is enlarged or typed (`element_links_ui`).
    pub(crate) fn native_links(&self) -> Vec<LinkMark> {
        if self.terminal_elevated.is_some() || self.editing.is_some() {
            return Vec::new();
        }

        self.scene
            .elements
            .iter()
            .filter(|element| !element.is_node() && !hidden_page(&self.scene.elements, element))
            .filter_map(|element| Some(LinkMark { id: element.id.clone(), link: element.link.clone()?, bounds: rect_of(element_bounds(element)) }))
            .collect()
    }

    /// "Voltar" (or "Voltar ao chat") while a link's tree is on screen (`link_back_ui`).
    pub(crate) fn native_back(&self) -> Option<&'static str> {
        let back = self.filetrees.link_back.as_ref()?;

        (!self.chat_open && self.scene.elements.iter().any(|element| element.id == back.tree)).then_some(if back.chat_open {
            "Voltar ao chat"
        } else {
            "Voltar"
        })
    }

    /// The enlarged window's title dragged (`phase` start, move, end), double-clicked (restore), or its docked edge
    /// dragged (edge), the pointer at `pointer`: what `dock_grip` does with egui's pointer.
    pub(crate) fn native_dock(&mut self, phase: &str, pointer: Pos2) -> Result<(), String> {
        let index = self.elevated_index().ok_or("Nada ampliado.")?;
        let element = self.scene.elements[index].clone();

        match phase {
            "start" => self.dock_drag = Some((pointer - self.terminal_frame(&element).0.min, pointer)),
            "move" => self.dock_drag = self.dock_drag.map(|(grab, _)| (grab, pointer)),
            "end" => {
                if let Some((_, pointer)) = self.dock_drag.take() {
                    self.dock = self.dock_side(pointer.x, &element);
                }
            }
            "restore" => self.set_elevated(None),
            "edge" => {
                let side = self.dock.ok_or("Não está no canto.")?.0;
                let canvas = self.canvas_rect;
                let width = if side { canvas.right() - DOCK_MARGIN - pointer.x } else { pointer.x - canvas.left() - DOCK_MARGIN };

                self.dock = Some((side, self.dock_resized(width, &element)));
            }
            phase => return Err(format!("Fase desconhecida: {phase}")),
        }

        Ok(())
    }

    pub(crate) fn chain_colors(&self) -> Chain {
        let colors = self.colors();

        Chain { fill: color(colors.island), hover: color(colors.surface_high), ink: color(colors.code) }
    }
}
