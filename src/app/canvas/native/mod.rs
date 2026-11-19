//! The canvas drawn by SwiftUI on macOS (`MAGOAGENT_SWIFTUI` with "canvas"): Rust keeps the scene, the editing, the undo,
//! the bindings and the hand-drawn geometry; Swift (macos/MagoUI/Sources/MagoUI/Canvas) draws what it gets and sends the
//! pointer and the keys back as `ui.canvas.*` (act.rs).
//!
//! Topics, in world coordinates:
//! - `scene`: `SceneList`, the elements in paint order with each one's revision, the ground and the grid: the ropes,
//!   the drawings, then the cards (windows and notes) in the drawing's order and the enlarged one last (`paint_scene`);
//! - `scene.<id>`: `Drawing`, one element's fills, strokes and text, published only when its revision changes (a note's
//!   text, buttons and tabs in note.rs);
//! - `canvas`: `Overlay`, the camera, the tool and what goes over the scene (selection, handles, marquee, the shape being
//!   dragged out, the text being typed, and pieces.rs: the enlarged window, the minimap, the link marks, "Voltar").
//!
//! The camera: Swift pans and zooms by itself and says where it is (`view`, and with every `pointer`); when Rust moves it
//! (zoom to fit, focus_canvas, a glide) `camera.serial` goes up and Swift takes Rust's. Swift's own moves don't come back.

mod act;
mod note;
mod pieces;
#[cfg(test)]
mod tests;

use crate::app::canvas::minimap::MinimapArea;
use crate::prelude::*;
use std::{cell::RefCell, collections::HashMap};

pub(crate) use note::{Mark, Note, PageLook, marks};
pub(crate) use pieces::{Chain, Lift, LinkMark, Minimap};

/// One painted step, in paint order.
#[derive(Serialize, Clone, Debug, PartialEq)]
#[serde(tag = "op", rename_all = "snake_case")]
pub(crate) enum Op {
    /// A filled polygon, points flat (x0, y0, x1, y1…).
    Fill { points: Vec<f32>, color: [u8; 4] },
    /// A polyline with round caps and joins; `dash` is [on, off].
    Stroke {
        points: Vec<f32>,
        color: [u8; 4],
        width: f32,
        #[serde(skip_serializing_if = "Option::is_none")]
        dash: Option<[f32; 2]>,
        closed: bool,
    },
    /// `value`'s lines from (x, y) down, `size` × LINE_HEIGHT apart, in the hand font or the interface's; `center` > 0
    /// centers each line in that width; `halo` rings it in the ground's color.
    Text {
        value: String,
        x: f32,
        y: f32,
        size: f32,
        color: [u8; 4],
        hand: bool,
        center: f32,
        #[serde(skip_serializing_if = "Option::is_none")]
        halo: Option<[u8; 4]>,
    },
}

/// A window on the canvas: Swift hosts the area's view in `rect` (`MagoViews`), or a card with its name.
#[derive(Serialize, Clone, Debug, PartialEq)]
pub(crate) struct Node {
    kind: &'static str,
    name: String,
    /// The title strip's height: presses there move the window (Rust's), below it the hosted view takes them.
    header: f32,
}

#[derive(Serialize, Clone, Debug, Default, PartialEq)]
pub(crate) struct Drawing {
    rev: u64,
    /// Its box before turning [x, y, width, height]: where a window, a note's body or an image goes.
    rect: [f32; 4],
    /// What it may paint [x, y, width, height]: Swift skips it when this misses the screen.
    reach: [f32; 4],
    /// Radians around the center of `rect`, for everything here (lines and freehand have theirs in their points).
    angle: f32,
    ops: Vec<Op>,
    #[serde(skip_serializing_if = "Option::is_none")]
    node: Option<Node>,
    #[serde(skip_serializing_if = "Option::is_none")]
    note: Option<Note>,
    /// A picture: Swift asks for it once (`ui.canvas.image`) and fills `rect` with it.
    image: bool,
}

#[derive(Serialize, Clone, Debug, PartialEq)]
pub(crate) struct Item {
    id: String,
    rev: u64,
    /// A window or a note: it stacks over the drawings, in the order of the list, each over the ones before.
    card: bool,
}

#[derive(Serialize, Clone, Debug, Default, PartialEq)]
pub(crate) struct SceneList {
    version: u64,
    ground: [u8; 4],
    /// The graph paper's line color, when it is on (`GRID` apart, every fifth when far out).
    #[serde(skip_serializing_if = "Option::is_none")]
    grid: Option<[u8; 4]>,
    items: Vec<Item>,
}

#[derive(Serialize, Clone, Debug, Default, PartialEq)]
pub(crate) struct Camera {
    serial: u64,
    /// Where the world's origin is in the canvas view, in points, and the scale.
    x: f32,
    y: f32,
    zoom: f32,
}

#[derive(Serialize, Clone, Debug, PartialEq)]
pub(crate) struct Selection {
    corners: [[f32; 2]; 4],
    handles: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    rotation: Option<[f32; 2]>,
}

/// The text being typed: a text, a label or a note's markdown (with its live look, `runs` and `marks`, as egui's note
/// editor shows it: only what the caret is on shows its marks).
#[derive(Serialize, Clone, Debug, PartialEq)]
pub(crate) struct Editing {
    id: String,
    value: String,
    rect: [f32; 4],
    size: f32,
    color: [u8; 4],
    /// The note it is the text of: its id (a lifted note is typed where it shows).
    note: Option<String>,
    center: bool,
    raw: bool,
    runs: Vec<markdown::LiveRun>,
    marks: Vec<Mark>,
}

#[derive(Serialize, Clone, Debug, Default, PartialEq)]
pub(crate) struct Overlay {
    camera: Camera,
    tool: String,
    dark: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    selection: Option<Selection>,
    /// Selected windows and notes: a dashed rounded outline each.
    windows: Vec<[f32; 4]>,
    points: Vec<[f32; 2]>,
    midpoints: Vec<[f32; 2]>,
    /// The line editor is open: bigger point handles, `picked` the chosen one.
    editor: bool,
    picked: Option<usize>,
    marquee: Option<[f32; 4]>,
    lasso: Vec<f32>,
    guides: Vec<[f32; 4]>,
    /// The outline of the shape an arrow's tip would hold on to.
    binding: Vec<f32>,
    draft: Option<Drawing>,
    /// A window being dragged out: a blue box.
    placing: Option<[f32; 4]>,
    editing: Option<Editing>,
    #[serde(skip_serializing_if = "Option::is_none")]
    lift: Option<Lift>,
    #[serde(skip_serializing_if = "Option::is_none")]
    minimap: Option<Minimap>,
    links: Vec<LinkMark>,
    chain: Chain,
    #[serde(skip_serializing_if = "Option::is_none")]
    back: Option<&'static str>,
}

/// What, besides the element itself, its drawing depends on.
#[derive(Clone, Debug, Default, PartialEq)]
pub(crate) struct Look {
    dark: bool,
    ground: [u8; 4],
    hide_text: bool,
    halo: bool,
    /// An arrow's label: the arrow is interrupted behind it.
    cut: bool,
    note_text: Option<String>,
    rope: Option<Vec<[f32; 2]>>,
    /// A pile's front page: its pages, for the tabs.
    pages: Vec<PageLook>,
}

// ponytail: the cache lives beside DrawingApp (its fields are in state.rs, shared with egui); a field of its own once egui's canvas is gone
#[derive(Default)]
pub(crate) struct Native {
    cache: HashMap<String, (Element, Look, Drawing)>,
    list: SceneList,
    next_rev: u64,
    /// Where the camera is (Swift's or Rust's), and the camera last moved by Rust, the one published.
    camera: Option<(Vec2, f32)>,
    serial: u64,
    published: Camera,
    /// Where the caret is in the note being typed (`ui.canvas.text`).
    caret: Option<markdown::Caret>,
    /// The minimap's area, held while it is pressed (its own: egui's minimap runs unseen beside it).
    minimap: MinimapArea,
}

thread_local! {
    static NATIVE: RefCell<Native> = RefCell::default();
}

fn color(color: Color32) -> [u8; 4] {
    color.to_srgba_unmultiplied()
}

fn flat(points: &[[f32; 2]]) -> Vec<f32> {
    points.iter().flatten().copied().collect()
}

fn rect_of(rect: Rect) -> [f32; 4] {
    [rect.min.x, rect.min.y, rect.width(), rect.height()]
}

fn corners(rect: Rect, radius: f32) -> Vec<f32> {
    corner_outline(rect, [radius; 4]).iter().flat_map(|point| [point.x, point.y]).collect()
}

/// The window's kind and name, for a terminal, chat, browser or file tree.
fn node_of(element: &Element) -> Option<Node> {
    let (kind, name, header) = if let Some(terminal) = &element.terminal {
        ("terminal", terminal.label().to_string(), TERMINAL_HEADER)
    } else if let Some(chat) = &element.chat {
        ("chat", chat.name.clone(), TERMINAL_HEADER)
    } else if let Some(portal) = &element.portal {
        ("portal", portal.name.clone(), PORTAL_HEADER)
    } else if element.filetree.is_some() {
        // the tree draws its whole card, header row included (the egui header is one row with the name in it)
        ("filetree", String::new(), 0.0)
    } else {
        return None;
    };

    Some(Node { kind, name, header })
}

/// One element as `paint_element` and `paint_note` paint it, in world coordinates.
pub(crate) fn drawing(element: &Element, look: &Look) -> Drawing {
    let bounds = local_bounds(element);
    let style = &element.style;
    let side = bounds.size().max_elem();
    // a curve swings out of its points' box; a turned box reaches its diagonal
    let mut reach = element_bounds(element).expand(40.0 + 2.0 * style.stroke_width + 0.3 * side);

    if element.angle != 0.0 {
        reach = Rect::from_center_size(reach.center(), Vec2::splat(reach.size().length()));
    }

    let mut out = Drawing { rect: rect_of(bounds), reach: rect_of(reach), angle: element.angle, ..Default::default() };
    let stroke = color(themed(with_opacity(style.stroke, style.opacity), look.dark));

    if let Some(path) = &look.rope {
        out.angle = 0.0;
        let span = path.iter().fold(Rect::NOTHING, |span, point| span.union(Rect::from_min_max((*point).into(), (*point).into())));

        out.reach = rect_of(span.expand(8.0));
        out.ops.push(Op::Stroke { points: flat(path), color: stroke, width: style.stroke_width, dash: Some([8.0, 7.0]), closed: false });

        return out;
    }

    if let Some(node) = node_of(element) {
        out.node = Some(node);

        return out;
    }

    if element.image.is_some() {
        out.image = true;

        return out;
    }

    if element.created.is_some() {
        note::note_drawing(element, look, &mut out);

        return out;
    }

    if look.cut && !look.hide_text {
        out.ops.push(Op::Fill { points: corners(bounds.expand(4.0), 4.0), color: look.ground });
    }

    if let (Some(fill), Some(polygon)) = (style.fill, shape_polygon(element)) {
        let fill = color(themed(with_opacity(fill, style.opacity), look.dark));

        if style.fill_style == FillStyle::Solid {
            out.ops.push(Op::Fill { points: flat(&polygon), color: fill });
        } else {
            for line in fill_strokes(element) {
                out.ops.push(Op::Stroke { points: flat(&line), color: fill, width: (style.stroke_width / 2.0).max(0.5), dash: None, closed: false });
            }
        }
    }

    let width = if matches!(element.kind, ElementKind::FreeDraw { .. }) { style.stroke_width * 1.5 } else { style.stroke_width };

    let dash = match style.stroke_style {
        StrokeStyle::Solid => None,
        StrokeStyle::Dashed => Some([8.0, 8.0 + style.stroke_width]),
        StrokeStyle::Dotted => Some([1.5, 6.0 + style.stroke_width]),
    };

    for outline in rough_outlines(element).into_iter().filter(|outline| outline.len() >= 2) {
        out.ops.push(Op::Stroke { points: flat(&outline), color: stroke, width, dash, closed: false });
    }

    for (shape, head) in arrowheads(element) {
        let points = flat(&shape);

        if let Some(solid) = head.fill() {
            out.ops.push(Op::Fill { points: points.clone(), color: if solid { stroke } else { look.ground } });
        }

        out.ops.push(Op::Stroke { points, color: stroke, width, dash: None, closed: head.fill().is_some() });
    }

    if let (Some(name), ElementKind::Rectangle { x, y, .. }) = (&element.frame, &element.kind)
        && !name.is_empty()
    {
        // ponytail: 13 world points; egui keeps it 13 on screen down to half zoom
        let size = 13.0;
        let gray = color(themed(Color32::from_rgb(0x99, 0x99, 0x99), look.dark));

        out.ops.push(Op::Text { value: name.clone(), x: *x, y: y - 4.0 - size * LINE_HEIGHT, size, color: gray, hand: false, center: 0.0, halo: None });
    }

    if let (ElementKind::Text { x, y, value }, false) = (&element.kind, look.hide_text) {
        let center = if is_label(element) { bounds.width() } else { 0.0 };
        let halo = look.halo.then_some(look.ground);

        out.ops.push(Op::Text { value: value.clone(), x: *x, y: *y, size: style.font_size, color: stroke, hand: true, center, halo });
    }

    out
}

impl DrawingApp {
    /// Each frame while Swift draws the canvas: the glide, the bindings, then what changed to Swift.
    pub(crate) fn native_canvas(&mut self) {
        // egui never lays the canvas out here: until Swift says its size (`view`) the camera stays finite at the corner
        if !self.canvas_rect.is_finite() {
            self.canvas_rect = Rect::ZERO;
        }

        self.step_glide();
        self.update_bindings();

        NATIVE.with_borrow_mut(|native| {
            for (id, drawing) in self.sync_scene(native) {
                crate::app::native::publish(&format!("scene.{id}"), &drawing);
            }

            crate::app::native::publish("scene", &native.list);
            crate::app::native::publish("canvas", &self.overlay(native));
        });
    }

    /// Brings `native` up to the scene: the elements whose drawing changed come back (with a new revision) and the list
    /// in paint order is renewed. Nothing changed, nothing comes back and the list keeps its version.
    pub(crate) fn sync_scene(&self, native: &mut Native) -> Vec<(String, Drawing)> {
        let elements = &self.scene.elements;
        let dark = self.dark();
        let ground = color(self.ground());
        // ropes under everything, the cards (windows and notes) over the drawings in the drawing's order, and the enlarged
        // one last (paint_scene)
        let mut order: Vec<usize> = (0..elements.len()).filter(|&index| !hidden_page(elements, &elements[index])).collect();

        order.retain(|&index| note_of_label(elements, &elements[index]).is_none());
        order.sort_by_key(|&index| (!elements[index].rope, elements[index].is_card()));

        if let Some(lifted) = self.lift_index() {
            order.retain(|&index| index != lifted);
            order.push(lifted);
        }

        let mut changed = Vec::new();
        let mut items = Vec::with_capacity(order.len());

        for index in order {
            // a lifted note is a sheet of its own size (`shown_note`)
            let element = self.shown_note(&elements[index]);
            let element = element.as_ref();
            let text = matches!(element.kind, ElementKind::Text { .. });

            let look = Look {
                dark,
                ground,
                hide_text: self.editing == Some(index)
                    || element.created.is_some() && note_label(elements, index).is_some_and(|label| self.editing == Some(label)),
                halo: text && text_needs_halo(elements, element),
                cut: text && is_arrow_label(elements, element),
                note_text: element.created.and(note_label(elements, index)).and_then(|label| match &elements[label].kind {
                    ElementKind::Text { value, .. } => Some(value.clone()),
                    _ => None,
                }),
                rope: element.rope.then(|| self.rope_path(element)),
                pages: self.native_pages(index),
            };

            let rev = match native.cache.get(&element.id) {
                Some((cached, cached_look, drawing)) if cached == element && *cached_look == look => drawing.rev,
                _ => {
                    native.next_rev += 1;
                    let drawing = Drawing { rev: native.next_rev, ..drawing(element, &look) };

                    changed.push((element.id.clone(), drawing.clone()));
                    native.cache.insert(element.id.clone(), (element.clone(), look, drawing));
                    native.next_rev
                }
            };

            items.push(Item { id: element.id.clone(), rev, card: element.is_card() });
        }

        native.cache.retain(|id, _| items.iter().any(|item| item.id == *id));

        let grid = (self.prefs.grid || self.prefs.paper).then(|| {
            let light = ground[0] as u32 + ground[1] as u32 + ground[2] as u32 > 3 * 128;

            if light { [0, 0, 0, 0x14] } else { [0x30, 0x30, 0x30, 255] }
        });

        if native.list.items != items || native.list.ground != ground || native.list.grid != grid {
            native.list = SceneList { version: native.list.version + 1, ground, grid, items };
        }

        changed
    }

    /// The camera, the tool and what paint_scene draws over the elements, as Swift draws it.
    pub(crate) fn overlay(&self, native: &mut Native) -> Overlay {
        let camera = (self.origin(), self.zoom);

        if native.camera != Some(camera) {
            native.camera = Some(camera);
            native.serial += 1;
            native.published = Camera { serial: native.serial, x: camera.0.x, y: camera.0.y, zoom: self.zoom };
        }

        let world = |screen: Pos2| {
            let point = self.to_world(screen);

            [point.x, point.y]
        };

        let dark = self.dark();
        let mut overlay = Overlay { camera: native.published.clone(), tool: tool_name(self.tool), dark, ..Default::default() };
        let elements = &self.scene.elements;
        let selected: Vec<&Element> = self.selected.iter().filter_map(|index| elements.get(*index)).collect();
        let card = |element: &Element| element.is_card();

        let with_card = |element: &&Element| {
            card(element) || element.rope || selected.iter().any(|other| card(other) && other.group_id.is_some() && other.group_id == element.group_id)
        };

        let two_point_line = self.single_line().is_some_and(|index| self.line_points(index).len() == 2);

        if selected.iter().any(|element| card(element)) && selected.iter().all(with_card) && self.tool == Tool::Select {
            if self.prefs.window_outline != WindowOutline::Hidden {
                let focused = self.unoutlined_window();

                overlay.windows =
                    selected.iter().filter(|window| card(window) && Some(&window.id) != focused.as_ref()).map(|window| rect_of(local_bounds(window))).collect();
            }
        } else if (self.shows_transform_handles() || (self.tool != Tool::Select && self.multi.is_none() && self.editing.is_none() && !two_point_line))
            && let Some(corners) = self.handle_corners()
        {
            let handles = self.shows_transform_handles();

            overlay.selection = Some(Selection { corners: corners.map(world), handles, rotation: self.rotation_handle().filter(|_| handles).map(world) });
        }

        overlay.points = self.point_handles().into_iter().map(|(_, point)| world(point)).collect();
        overlay.midpoints = self.midpoint_handles().into_iter().map(|(_, point)| world(point)).collect();
        overlay.editor = self.line_editor.is_some();
        overlay.picked = self.selected_point.filter(|_| overlay.editor);

        if let (Some(start), Some(end)) = (self.marquee, self.last_pointer) {
            overlay.marquee = Some(rect_of(Rect::from_two_pos(start, self.to_world(end))));
        }

        overlay.lasso = self.lasso.iter().flatten().flat_map(|point| [point.x, point.y]).collect();
        overlay.guides = self.snap_guides.iter().map(|[from, to]| [from.x, from.y, to.x, to.y]).collect();

        if let Some(target) = self.suggested_binding().or_else(|| self.connect_hover()).and_then(|index| elements.get(index)) {
            overlay.binding = flat(&binding_outline(target, 0.0));
        }

        let look = Look { dark, ground: color(self.ground()), ..Default::default() };

        match (self.drawing_start, self.last_pointer, self.tool) {
            (Some(_), _, Tool::AutoShape) => {
                let preview = recognize_shape(self, &self.drawing_points).filter(|shape| !matches!(shape.kind, ElementKind::Line { arrow: false, .. }));

                let mut draft = preview.map_or_else(Drawing::default, |mut shape| {
                    shape.style.opacity = 70;
                    drawing(&shape, &look)
                });

                let trail = if dark { Color32::from_white_alpha(51) } else { Color32::from_black_alpha(51) };
                let width = (self.style.stroke_width * 0.65).clamp(1.25, 4.0) / self.zoom;

                draft.ops.insert(0, Op::Stroke { points: flat(&self.drawing_points), color: color(trail), width, dash: None, closed: false });
                overlay.draft = Some(draft);
            }
            (Some(start), Some(pointer), Tool::Terminal | Tool::Portal | Tool::FileTree | Tool::Sticky) => {
                overlay.placing = Some(rect_of(Rect::from_two_pos(start, self.to_world(pointer))));
            }
            (Some(start), Some(pointer), tool) if tool != Tool::Chat => {
                overlay.draft = Some(drawing(&self.bind_new_arrow(self.temporary_element(start, self.to_world(pointer))), &look));
            }
            _ => {}
        }

        if self.editing.is_none() {
            native.caret = None;
        }

        overlay.editing = self.native_editing(native.caret);
        overlay.lift = self.native_lift();
        overlay.minimap = self.native_minimap(&mut native.minimap);
        overlay.links = self.native_links();
        overlay.chain = self.chain_colors();
        overlay.back = self.native_back();
        overlay
    }

    /// The text being typed, with a note's live look for where the caret is.
    fn native_editing(&self, caret: Option<markdown::Caret>) -> Option<Editing> {
        let elements = &self.scene.elements;
        let index = self.editing?;
        let element = elements.get(index)?;
        let ElementKind::Text { x, y, value } = &element.kind else { return None };

        Some(match note_of_label(elements, element) {
            Some(note) => {
                let note = self.shown_note(&elements[note]);
                let (_, ink) = note_colors(&note, 100);
                let raw = note.note.raw;
                let (runs, ornaments) = if raw { Default::default() } else { markdown::live_runs(value, note.note.font_size, caret) };

                Editing {
                    id: element.id.clone(),
                    value: value.clone(),
                    rect: rect_of(note_text_box(&note)),
                    size: note.note.font_size,
                    color: color(ink),
                    note: Some(note.id.clone()),
                    center: false,
                    raw,
                    runs,
                    marks: marks(&ornaments),
                }
            }
            None => Editing {
                id: element.id.clone(),
                value: value.clone(),
                rect: [*x, *y, element_bounds(element).width(), element_bounds(element).height()],
                size: element.style.font_size,
                color: color(themed(rgba(element.style.stroke), self.dark())),
                note: None,
                center: is_label(element),
                raw: false,
                runs: Vec::new(),
                marks: Vec::new(),
            },
        })
    }

    /// Where Swift's camera is: (x, y) the world's origin in the canvas view, `width` × `height` the view.
    fn native_view(&mut self, args: &Value) {
        let number = |key: &str| args[key].as_f64().map(|value| value as f32);

        if let (Some(x), Some(y), Some(zoom)) = (number("x"), number("y"), number("zoom")) {
            if let (Some(width), Some(height)) = (number("width"), number("height")) {
                self.canvas_rect = Rect::from_min_size(Pos2::ZERO, Vec2::new(width, height));
            }

            self.glide = None;
            self.zoom = zoom.clamp(0.1, 30.0);
            self.pan = Vec2::new(x, y) - self.canvas_rect.min.to_vec2();
            NATIVE.with_borrow_mut(|native| native.camera = Some((self.origin(), self.zoom)));
        }
    }
}

/// The tools by the names Swift uses: `Tool`'s own, in lower case.
fn tool_name(tool: Tool) -> String {
    format!("{tool:?}").to_lowercase()
}

fn all_tools() -> impl Iterator<Item = Tool> {
    Tool::TOOLBAR.into_iter().chain(Tool::DRAWING).chain(Tool::EXTRA).chain([Tool::Chat])
}
