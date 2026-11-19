//! The canvas drawn by SwiftUI on macOS (`MAGOAGENT_SWIFTUI` with "canvas"): Rust keeps the scene, the editing, the undo,
//! the bindings and the hand-drawn geometry; Swift (macos/MagoUI/Sources/MagoUI/Canvas) draws what it gets and sends the
//! pointer and the keys back as `ui.canvas.*`.
//!
//! Topics, in world coordinates:
//! - `scene`: `SceneList`, the elements in paint order with each one's revision, the ground and the grid;
//! - `scene.<id>`: `Drawing`, one element's fills, strokes and text, published only when its revision changes;
//! - `canvas`: `Overlay`, the camera, the tool and what goes over the scene (selection, handles, marquee, the shape being
//!   dragged out, the text being typed).
//!
//! The camera: Swift pans and zooms by itself and says where it is (`view`, and with every `pointer`); when Rust moves it
//! (zoom to fit, focus_canvas, a glide) `camera.serial` goes up and Swift takes Rust's.

use crate::prelude::*;
use std::{cell::RefCell, collections::HashMap};

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

/// A sticky note's body: its markdown in `area`, under the paper and header in `ops`. No `text` while it is typed.
#[derive(Serialize, Clone, Debug, PartialEq)]
pub(crate) struct Note {
    text: Option<String>,
    ink: [u8; 4],
    size: f32,
    area: [f32; 4],
    raw: bool,
    blurred: bool,
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

#[derive(Serialize, Clone, Debug, PartialEq)]
pub(crate) struct Editing {
    id: String,
    value: String,
    rect: [f32; 4],
    size: f32,
    color: [u8; 4],
    note: bool,
    center: bool,
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
}

// ponytail: the cache lives beside DrawingApp (its fields are in state.rs, shared with egui); a field of its own once egui's canvas is gone
#[derive(Default)]
pub(crate) struct Native {
    cache: HashMap<String, (Element, Look, Drawing)>,
    list: SceneList,
    next_rev: u64,
    camera: Option<(Vec2, f32)>,
    serial: u64,
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
        note_drawing(element, look, &mut out);

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

/// The card of a sticky note as `paint_note` draws it: shadowless paper with a hairline, the header strip, the tape and
/// the name; its markdown goes in `note`.
fn note_drawing(note: &Element, look: &Look, out: &mut Drawing) {
    let bounds = local_bounds(note);
    let alpha = note.style.opacity as f32 / 100.0;
    let radius = if note.style.round { 8.0_f32.min(bounds.width() / 2.0).min(bounds.height() / 2.0).min(NOTE_HEADER) } else { 0.0 };
    let (paper, ink) = note_colors(note, note.style.opacity);
    let header = Rect::from_min_size(bounds.min, Vec2::new(bounds.width(), NOTE_HEADER.min(bounds.height())));
    let strip = if ink == Color32::WHITE { Color32::from_white_alpha(26) } else { Color32::from_black_alpha(15) };
    let hairline = color(Color32::from_black_alpha(20).gamma_multiply(alpha));
    let outline = corners(bounds, radius);

    out.ops.push(Op::Fill { points: outline.clone(), color: color(paper) });
    out.ops.push(Op::Stroke { points: outline, color: hairline, width: 0.5, dash: None, closed: true });
    let header_outline = corner_outline(header, [radius, radius, 0.0, 0.0]).iter().flat_map(|point| [point.x, point.y]).collect();

    out.ops.push(Op::Fill { points: header_outline, color: color(strip.gamma_multiply(alpha)) });
    let tape: Vec<f32> = tape(bounds).iter().flat_map(|point| [point.x, point.y]).collect();

    out.ops.push(Op::Fill { points: tape.clone(), color: color(Color32::from_rgba_unmultiplied(0xe9, 0xe4, 0xd4, 205).gamma_multiply(alpha)) });
    out.ops.push(Op::Stroke { points: tape, color: color(Color32::from_black_alpha(34).gamma_multiply(alpha)), width: 0.8, dash: None, closed: true });
    let ink = ink.gamma_multiply(alpha);
    let text = look.note_text.clone().unwrap_or_default();

    if !note.note.blurred {
        let muted = ink.gamma_multiply(0.6);
        let (name, name_color) = if text.trim().is_empty() { ("Sem título".to_string(), muted.gamma_multiply(0.7)) } else { (note_name(&text), muted) };
        let size = 10.0;
        let y = bounds.min.y + (NOTE_HEADER - size * LINE_HEIGHT) / 2.0;

        out.ops.push(Op::Text { value: name, x: bounds.min.x + NOTE_PAD, y, size, color: color(name_color), hand: false, center: 0.0, halo: None });
    }

    out.note = Some(Note {
        text: (!look.hide_text).then_some(text),
        ink: color(ink),
        size: note.note.font_size,
        area: rect_of(note_text_box(note)),
        raw: note.note.raw,
        blurred: note.note.blurred,
    });
}

/// The tools by the names Swift uses: `Tool`'s own, in lower case.
fn tool_name(tool: Tool) -> String {
    format!("{tool:?}").to_lowercase()
}

fn all_tools() -> impl Iterator<Item = Tool> {
    Tool::TOOLBAR.into_iter().chain(Tool::DRAWING).chain(Tool::EXTRA).chain([Tool::Chat])
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
        // ropes under everything, the windows over the drawings (paint_scene)
        let mut order: Vec<usize> = (0..elements.len()).filter(|&index| !hidden_page(elements, &elements[index])).collect();

        order.retain(|&index| note_of_label(elements, &elements[index]).is_none());
        order.sort_by_key(|&index| (!elements[index].rope, elements[index].is_node()));
        let mut changed = Vec::new();
        let mut items = Vec::with_capacity(order.len());

        for index in order {
            let element = &elements[index];
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

            items.push(Item { id: element.id.clone(), rev });
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
        }

        let world = |screen: Pos2| {
            let point = self.to_world(screen);

            [point.x, point.y]
        };

        let dark = self.dark();

        let mut overlay = Overlay {
            camera: Camera { serial: native.serial, x: camera.0.x, y: camera.0.y, zoom: self.zoom },
            tool: tool_name(self.tool),
            dark,
            ..Default::default()
        };

        let elements = &self.scene.elements;
        let selected: Vec<&Element> = self.selected.iter().filter_map(|index| elements.get(*index)).collect();
        let card = |element: &Element| element.is_node() || element.created.is_some();

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

        overlay.editing = self.editing.and_then(|index| {
            let element = elements.get(index)?;
            let ElementKind::Text { x, y, value } = &element.kind else { return None };

            Some(match note_of_label(elements, element) {
                Some(note) => {
                    let note = &elements[note];
                    let (_, ink) = note_colors(note, 100);

                    Editing {
                        id: element.id.clone(),
                        value: value.clone(),
                        rect: rect_of(note_text_box(note)),
                        size: note.note.font_size,
                        color: color(ink),
                        note: true,
                        center: false,
                    }
                }
                None => Editing {
                    id: element.id.clone(),
                    value: value.clone(),
                    rect: [*x, *y, element_bounds(element).width(), element_bounds(element).height()],
                    size: element.style.font_size,
                    color: color(themed(rgba(element.style.stroke), dark)),
                    note: false,
                    center: is_label(element),
                },
            })
        });

        overlay
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

    /// `ui.canvas.<verb>`: pointer and keys from Swift, through the same editing code as egui's canvas.
    pub(crate) fn native_canvas_act(&mut self, verb: &str, args: &Value) -> Result<String, String> {
        let text = |key: &str| args[key].as_str().unwrap_or_default().to_string();
        let flag = |key: &str| args[key].as_bool().unwrap_or(false);
        let modifiers = Modifiers { alt: flag("alt"), ctrl: false, shift: flag("shift"), mac_cmd: flag("cmd"), command: flag("cmd") };

        match verb {
            "view" => self.native_view(args),
            "pointer" => {
                self.native_view(args);
                let pos = Pos2::new(args["px"].as_f64().unwrap_or(0.0) as f32, args["py"].as_f64().unwrap_or(0.0) as f32);

                self.mods = modifiers;

                match text("phase").as_str() {
                    "down" => {
                        self.press_canvas(pos, text("button") == "middle", false, modifiers, false);
                    }
                    "move" => self.update_gesture(pos),
                    "up" => {
                        self.update_gesture(pos);
                        self.release(pos);

                        // egui's double-click comes with the second release
                        if args["clicks"].as_u64().unwrap_or(1) >= 2 {
                            self.double_click_canvas(pos, true, true);
                        }
                    }
                    "cancel" => self.cancel_gesture(),
                    phase => return Err(format!("Fase do ponteiro desconhecida: {phase}")),
                }

                self.update_bindings();
            }
            "tool" => {
                let name = text("name");
                let tool = all_tools().find(|tool| tool_name(*tool) == name).ok_or(format!("Ferramenta desconhecida: {name}"))?;

                self.select_tool(tool);
            }
            "key" => self.native_key(&text("key"), modifiers),
            "text" => {
                if let Some(index) = self.editing
                    && let Some(Element { kind: ElementKind::Text { value, .. }, .. }) = self.scene.elements.get_mut(index)
                {
                    *value = text("value");
                    recenter_labels(&mut self.scene.elements);
                }
            }
            "text_done" => {
                if self.elevated_index().is_some_and(|index| self.scene.elements[index].created.is_some()) {
                    self.set_elevated(None);
                } else {
                    self.finish_text();
                }
            }
            "image" => {
                let id = text("id");
                let element = self.scene.elements.iter().find(|element| element.id == id).ok_or(format!("Elemento não encontrado: {id}"))?;

                return element.image.as_deref().map(str::to_string).ok_or(format!("O elemento {id} não é uma imagem"));
            }
            "font" => return Ok(base64(crate::geometry::HAND_FACE)),
            _ => return Err(format!("Ação do canvas desconhecida: {verb}")),
        }

        Ok(String::new())
    }

    /// A key on the canvas, by egui's name for it (`Key::name`): the canvas's part of handle_shortcuts.
    fn native_key(&mut self, name: &str, modifiers: Modifiers) {
        if self.in_gesture() {
            // mid-drag the keys wait; Esc cancels it
            if name == "Escape" {
                self.cancel_gesture();
            }

            return;
        }

        let Some(key) = Key::from_name(name) else { return };
        let (command, shift) = (modifiers.command, modifiers.shift);

        match key {
            Key::Z if command && shift => self.redo(),
            Key::Z if command => self.undo(),
            Key::Y if command => self.redo(),
            Key::A if command => self.select_all(),
            Key::D if command => self.duplicate(),
            Key::Delete | Key::Backspace if !command => {
                if !self.delete_selected_point() {
                    self.delete_selected();
                }
            }
            Key::Escape | Key::Enter if self.multi.is_some() => self.finish_multi(),
            Key::Escape | Key::Enter if self.line_editor.is_some() => {
                self.line_editor = None;
                self.selected_point = None;
            }
            Key::Escape => {
                self.connecting = None;
                self.set_elevated(None);

                if self.selected.is_empty() {
                    self.tool = Tool::Select;
                }

                self.selected.clear();
            }
            Key::Enter => {
                if let [index] = self.selected[..]
                    && self.scene.elements.get(index).is_some_and(|element| matches!(element.kind, ElementKind::Text { .. }) || element.created.is_some())
                {
                    self.begin_text_edit(index);
                }
            }
            Key::ArrowLeft | Key::ArrowRight | Key::ArrowUp | Key::ArrowDown if !command => {
                let delta = match key {
                    Key::ArrowLeft => Vec2::new(-1.0, 0.0),
                    Key::ArrowRight => Vec2::new(1.0, 0.0),
                    Key::ArrowUp => Vec2::new(0.0, -1.0),
                    _ => Vec2::new(0.0, 1.0),
                };

                self.nudge(if shift { delta * 5.0 } else { delta });
            }
            Key::X if shift && !command && self.drawing() => self.select_tool(Tool::AutoShape),
            _ if !command && !shift && !modifiers.alt => {
                if let Some(tool) = all_tools().find(|tool| self.tool_keys_live(*tool) && tool.keys().contains(&key)) {
                    self.select_tool(tool);
                }
            }
            _ => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pointer(app: &mut DrawingApp, phase: &str, x: f32, y: f32) {
        let args = json!({ "phase": phase, "px": x, "py": y, "x": 0.0, "y": 0.0, "zoom": 1.0, "width": 1200.0, "height": 800.0 });

        app.native_canvas_act("pointer", &args).unwrap();
    }

    #[test]
    fn a_rectangle_drawn_from_swift_lands_in_the_scene_with_its_rough_strokes() {
        let mut app = DrawingApp::default();

        app.native_canvas_act("tool", &json!({ "name": "rectangle" })).unwrap();
        pointer(&mut app, "down", 100.0, 100.0);
        pointer(&mut app, "move", 200.0, 150.0);
        // mid-drag the shape is the overlay's draft, not yet in the scene
        let mut native = Native::default();

        assert!(app.overlay(&mut native).draft.is_some_and(|draft| !draft.ops.is_empty()));
        pointer(&mut app, "move", 300.0, 220.0);
        pointer(&mut app, "up", 300.0, 220.0);
        assert_eq!(app.scene.elements.len(), 1);
        assert_eq!(element_bounds(&app.scene.elements[0]), Rect::from_min_max(Pos2::new(100.0, 100.0), Pos2::new(300.0, 220.0)));

        let changed = app.sync_scene(&mut native);

        assert_eq!(changed.len(), 1);
        let (id, drawing) = &changed[0];

        assert_eq!(*id, app.scene.elements[0].id);
        // the same strokes egui paints: rough_outlines, two passes of four edges
        let strokes = drawing.ops.iter().filter(|op| matches!(op, Op::Stroke { .. })).count();

        assert_eq!(strokes, rough_outlines(&app.scene.elements[0]).len());
        assert_eq!(native.list.items, vec![Item { id: id.clone(), rev: drawing.rev }]);
        let version = native.list.version;

        // nothing changed: nothing is sent again
        assert!(app.sync_scene(&mut native).is_empty());
        assert_eq!(native.list.version, version);

        // one element moved: only it comes back, with a new revision
        app.snapshot();
        app.scene.elements.push(app.new_element(ElementKind::Ellipse { x: 0.0, y: 0.0, width: 10.0, height: 10.0 }));
        app.sync_scene(&mut native);
        app.scene.elements[1].move_by(Vec2::new(5.0, 0.0));
        let changed = app.sync_scene(&mut native);

        assert_eq!(changed.len(), 1);
        assert_eq!(changed[0].0, app.scene.elements[1].id);
        assert!(native.list.version > version);

        // undo through the key takes the ellipse back out
        app.native_canvas_act("key", &json!({ "key": "Z", "cmd": true })).unwrap();
        app.sync_scene(&mut native);
        assert_eq!(native.list.items.len(), 1);
    }

    #[test]
    fn windows_paint_over_drawings_and_carry_their_kind() {
        let mut app = DrawingApp::default();
        let mut terminal = app.new_element(ElementKind::Rectangle { x: 0.0, y: 0.0, width: 300.0, height: 200.0 });

        terminal.terminal = Some(TerminalSpec { name: "build".into(), ..Default::default() });
        let line = app.new_element(ElementKind::Line { points: vec![[0.0, 0.0], [50.0, 50.0]], arrow: true });

        app.scene.elements = vec![terminal, line];
        let mut native = Native::default();
        let changed: HashMap<String, Drawing> = app.sync_scene(&mut native).into_iter().collect();
        let order: Vec<&str> = native.list.items.iter().map(|item| item.id.as_str()).collect();

        assert_eq!(order, [app.scene.elements[1].id.as_str(), app.scene.elements[0].id.as_str()]);
        assert_eq!(changed[&app.scene.elements[0].id].node, Some(Node { kind: "terminal", name: "build".into(), header: TERMINAL_HEADER }));
        // the arrow's head is a stroke after its line
        assert!(matches!(changed[&app.scene.elements[1].id].ops.last(), Some(Op::Stroke { closed: false, .. })));
    }

    #[test]
    fn the_camera_serial_moves_only_when_rust_moves_the_camera() {
        let mut app = DrawingApp::default();
        let mut native = Native::default();
        let first = app.overlay(&mut native).camera.serial;

        // Swift's own pan: what it sent is what it has
        app.native_view(&json!({ "x": 40.0, "y": -20.0, "zoom": 2.0, "width": 800.0, "height": 600.0 }));
        NATIVE.with_borrow(|shared| native.camera = shared.camera);
        let camera = app.overlay(&mut native).camera;

        assert_eq!((camera.serial, camera.x, camera.y, camera.zoom), (first, 40.0, -20.0, 2.0));
        app.zoom_at(1.0, Pos2::new(400.0, 300.0));
        assert_eq!(app.overlay(&mut native).camera.serial, first + 1);
    }
}
