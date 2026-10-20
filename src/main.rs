use std::{fs, path::PathBuf, time::SystemTime};

mod mcp;

use eframe::egui::{
    self, Color32, Context, CursorIcon, FontId, Key, Modifiers, Painter, Pos2, Rect, Sense, Shape,
    Stroke, StrokeKind, Vec2,
};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

const APP_TITLE: &str = "MagoAgent";
const PRIMARY: Color32 = Color32::from_rgb(0x69, 0x65, 0xdb);
const BLACK: [u8; 4] = [0x1e, 0x1e, 0x1e, 255];
const WHITE: [u8; 4] = [255, 255, 255, 255];
const TRANSPARENT: [u8; 4] = [0, 0, 0, 0];
const LINE_HEIGHT: f32 = 1.25;
/// The hand-drawn font family (OFL, see assets/hand-font-license.txt).
const HAND_FONT: &str = "MagoHand";
const LINE_CURVE_STEPS: usize = 12;
// ponytail: average hand-font advance; measure glyphs if text bounds must be pixel exact
const CHAR_WIDTH: f32 = 0.55;

// Values from packages/common/src/colors.ts
const STROKE_PICKS: [[u8; 4]; 5] = [
    BLACK,
    [0xe0, 0x31, 0x31, 255],
    [0x2f, 0x9e, 0x44, 255],
    [0x19, 0x71, 0xc2, 255],
    [0xf0, 0x8c, 0x00, 255],
];
const BACKGROUND_PICKS: [[u8; 4]; 5] = [
    TRANSPARENT,
    [0xff, 0xc9, 0xc9, 255],
    [0xb2, 0xf2, 0xbb, 255],
    [0xa5, 0xd8, 0xff, 255],
    [0xff, 0xec, 0x99, 255],
];
const CANVAS_PICKS: [[u8; 4]; 5] = [
    WHITE,
    [0xf8, 0xf9, 0xfa, 255],
    [0xf5, 0xfa, 0xff, 255],
    [0xff, 0xfc, 0xe8, 255],
    [0xfd, 0xf8, 0xf6, 255],
];

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
enum Tool {
    Hand,
    #[default]
    Select,
    Rectangle,
    Diamond,
    Ellipse,
    Arrow,
    Line,
    Draw,
    Text,
    Eraser,
    Frame,
    Sticky,
    AutoShape,
    Laser,
    Fill,
    Lasso,
}

impl Tool {
    /// Tools in the "⋮ Mais ferramentas" menu.
    const EXTRA: [Tool; 5] = [Tool::Frame, Tool::AutoShape, Tool::Laser, Tool::Fill, Tool::Lasso];

    const TOOLBAR: [Tool; 11] = [
        Tool::Hand,
        Tool::Select,
        Tool::Rectangle,
        Tool::Diamond,
        Tool::Ellipse,
        Tool::Arrow,
        Tool::Line,
        Tool::Draw,
        Tool::Text,
        Tool::Sticky,
        Tool::Eraser,
    ];

    fn label(self) -> &'static str {
        match self {
            Tool::Hand => "Mão (ferramenta de rolagem)",
            Tool::Select => "Seleção",
            Tool::Rectangle => "Retângulo",
            Tool::Diamond => "Losango",
            Tool::Ellipse => "Elipse",
            Tool::Arrow => "Flecha",
            Tool::Line => "Linha",
            Tool::Draw => "Desenhar",
            Tool::Text => "Texto",
            Tool::Eraser => "Borracha",
            Tool::Frame => "Ferramenta de quadro",
            Tool::Sticky => "Nota adesiva",
            Tool::AutoShape => "Desenhar para criar forma",
            Tool::Laser => "Ponteiro de laser",
            Tool::Fill => "Preenchimento",
            Tool::Lasso => "Seleção de laço",
        }
    }

    /// Letter shortcut first, numeric second (HelpDialog.tsx).
    fn keys(self) -> &'static [Key] {
        match self {
            Tool::Hand => &[Key::H],
            Tool::Select => &[Key::V, Key::Num1],
            Tool::Rectangle => &[Key::R, Key::Num2],
            Tool::Diamond => &[Key::D, Key::Num3],
            Tool::Ellipse => &[Key::O, Key::Num4],
            Tool::Arrow => &[Key::A, Key::Num5],
            Tool::Line => &[Key::L, Key::Num6],
            Tool::Draw => &[Key::P, Key::Num7],
            Tool::Text => &[Key::T, Key::Num8],
            Tool::Eraser => &[Key::E, Key::Num0],
            Tool::Frame => &[Key::F],
            Tool::Sticky => &[Key::N],
            Tool::AutoShape => &[], // Shift+X, handled with the shortcuts
            Tool::Laser => &[Key::K],
            Tool::Fill => &[Key::B],
            Tool::Lasso => &[],
        }
    }

    fn badge(self) -> &'static str {
        match self {
            Tool::Hand => "",
            Tool::Select => "V",
            Tool::Rectangle => "R",
            Tool::Diamond => "D",
            Tool::Ellipse => "O",
            Tool::Arrow => "A",
            Tool::Line => "L",
            Tool::Draw => "P",
            Tool::Text => "T",
            Tool::Eraser => "E",
            Tool::Frame => "F",
            Tool::Sticky => "N",
            Tool::AutoShape => "Shift+X",
            Tool::Laser => "K",
            Tool::Fill => "B",
            Tool::Lasso => "",
        }
    }
}

#[derive(Clone, Copy, Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
enum StrokeStyle {
    #[default]
    Solid,
    Dashed,
    Dotted,
}

#[derive(Clone, Copy, Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
enum FillStyle {
    Hachure,
    CrossHatch,
    #[default]
    Solid,
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq)]
#[serde(default)]
struct Style {
    stroke: [u8; 4],
    fill: Option<[u8; 4]>,
    stroke_width: f32,
    stroke_style: StrokeStyle,
    fill_style: FillStyle,
    roughness: u8,
    round: bool,
    opacity: u8,
    font_size: f32,
}

impl Default for Style {
    fn default() -> Self {
        Self {
            stroke: BLACK,
            fill: None,
            stroke_width: 2.0,
            stroke_style: StrokeStyle::Solid,
            fill_style: FillStyle::Solid,
            roughness: 1,
            round: true,
            opacity: 100,
            font_size: 20.0,
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(tag = "type", rename_all = "snake_case")]
enum ElementKind {
    Rectangle { x: f32, y: f32, width: f32, height: f32 },
    Ellipse { x: f32, y: f32, width: f32, height: f32 },
    Diamond { x: f32, y: f32, width: f32, height: f32 },
    Line { points: Vec<[f32; 2]>, arrow: bool },
    FreeDraw { points: Vec<[f32; 2]> },
    /// `x`, `y` is the top-left corner of the first line.
    Text { x: f32, y: f32, value: String },
}

/// Arrowheads from the reference app's "Pontas" picker (the cardinality set is not ported).
#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
enum Head {
    Arrow,
    Bar,
    #[serde(alias = "dot")]
    Circle,
    CircleOutline,
    Triangle,
    TriangleOutline,
    Diamond,
    DiamondOutline,
}

impl Head {
    /// Picker order from actionProperties.tsx getArrowheadOptions.
    const PICKS: [Option<Head>; 9] = [
        None,
        Some(Head::Arrow),
        Some(Head::Triangle),
        Some(Head::TriangleOutline),
        Some(Head::Circle),
        Some(Head::CircleOutline),
        Some(Head::Diamond),
        Some(Head::DiamondOutline),
        Some(Head::Bar),
    ];

    fn label(head: Option<Head>) -> &'static str {
        match head {
            None => "Nenhuma",
            Some(Head::Arrow) => "Flecha",
            Some(Head::Bar) => "Barra",
            Some(Head::Circle) => "Círculo",
            Some(Head::CircleOutline) => "Círculo (contorno)",
            Some(Head::Triangle) => "Triângulo",
            Some(Head::TriangleOutline) => "Triângulo (contorno)",
            Some(Head::Diamond) => "Diamante",
            Some(Head::DiamondOutline) => "Diamante (contorno)",
        }
    }

    /// Some(true) filled with the stroke color, Some(false) filled with the canvas, None open.
    fn fill(self) -> Option<bool> {
        match self {
            Head::Arrow | Head::Bar => None,
            Head::Circle | Head::Triangle | Head::Diamond => Some(true),
            Head::CircleOutline | Head::TriangleOutline | Head::DiamondOutline => Some(false),
        }
    }
}

/// the reference app's FixedPointBinding: `fixed` is the anchor as a ratio of the target's
/// unrotated box; "orbit" stops on the outline, `inside` goes to the anchor itself.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
struct Binding {
    element_id: String,
    fixed: [f32; 2],
    #[serde(default)]
    inside: bool,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(default)]
struct ArrowEnds {
    start_head: Option<Head>,
    end_head: Option<Head>,
    start: Option<Binding>,
    end: Option<Binding>,
}

impl Default for ArrowEnds {
    fn default() -> Self {
        Self { start_head: None, end_head: Some(Head::Arrow), start: None, end: None }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
struct Element {
    id: String,
    #[serde(default)]
    group_id: Option<String>,
    kind: ElementKind,
    /// Radians around the center; lines and freehand bake rotation into their points.
    #[serde(default)]
    angle: f32,
    /// Arrowheads and shape bindings; only meaningful for arrows.
    #[serde(default)]
    ends: ArrowEnds,
    /// Locked elements can't be selected, moved or erased until unlocked.
    #[serde(default)]
    locked: bool,
    /// Some(name) makes a rectangle a frame: moving it carries everything inside.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    frame: Option<String>,
    /// Some(data URL) makes a rectangle an image; interop files keep it in `files` instead.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    image: Option<String>,
    /// Some(creation time, ms since 1970; 0 when unknown) makes a rectangle a sticky note.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    created: Option<i64>,
    #[serde(flatten)]
    style: Style,
}

fn rot(v: Vec2, angle: f32) -> Vec2 {
    let (sin, cos) = angle.sin_cos();
    Vec2::new(v.x * cos - v.y * sin, v.x * sin + v.y * cos)
}

impl Element {
    fn rotatable(&self) -> bool {
        !matches!(self.kind, ElementKind::Line { .. } | ElementKind::FreeDraw { .. })
    }

    fn center(&self) -> Pos2 {
        local_bounds(self).center()
    }

    /// World point -> element's unrotated frame.
    fn to_local(&self, point: Pos2) -> Pos2 {
        if self.angle == 0.0 { point } else { self.center() + rot(point - self.center(), -self.angle) }
    }

    fn rotate_around(&mut self, pivot: Pos2, delta: f32) {
        if self.rotatable() {
            let center = self.center();
            self.move_by(pivot + rot(center - pivot, delta) - center);
            self.angle = (self.angle + delta).rem_euclid(std::f32::consts::TAU);
        } else if let ElementKind::Line { points, .. } | ElementKind::FreeDraw { points } = &mut self.kind {
            for point in points {
                let p = pivot + rot(Pos2::new(point[0], point[1]) - pivot, delta);
                *point = [p.x, p.y];
            }
        }
    }

    fn move_by(&mut self, delta: Vec2) {
        match &mut self.kind {
            ElementKind::Rectangle { x, y, .. }
            | ElementKind::Ellipse { x, y, .. }
            | ElementKind::Diamond { x, y, .. }
            | ElementKind::Text { x, y, .. } => {
                *x += delta.x;
                *y += delta.y;
            }
            ElementKind::Line { points, .. } | ElementKind::FreeDraw { points } => {
                for point in points {
                    point[0] += delta.x;
                    point[1] += delta.y;
                }
            }
        }
    }

    fn scale(&mut self, from: Rect, to: Rect) {
        let sx = if from.width() > 0.0 { to.width() / from.width() } else { 1.0 };
        let sy = if from.height() > 0.0 { to.height() / from.height() } else { 1.0 };
        let map = |px: f32, py: f32| {
            [
                to.min.x + (px - from.min.x) * sx,
                to.min.y + (py - from.min.y) * sy,
            ]
        };
        match &mut self.kind {
            ElementKind::Rectangle { x, y, width, height }
            | ElementKind::Ellipse { x, y, width, height }
            | ElementKind::Diamond { x, y, width, height } => {
                [*x, *y] = map(*x, *y);
                *width = (*width * sx).max(1.0);
                *height = (*height * sy).max(1.0);
            }
            ElementKind::Text { x, y, .. } => {
                [*x, *y] = map(*x, *y);
                self.style.font_size = (self.style.font_size * sy).clamp(4.0, 400.0);
            }
            ElementKind::Line { points, .. } | ElementKind::FreeDraw { points } => {
                for point in points {
                    *point = map(point[0], point[1]);
                }
            }
        }
    }

    fn fillable(&self) -> bool {
        matches!(
            self.kind,
            ElementKind::Rectangle { .. } | ElementKind::Ellipse { .. } | ElementKind::Diamond { .. }
        )
    }

    /// Transparent shapes are only hit near their outline, like in the reference app.
    fn contains(&self, point: Pos2, tolerance: f32) -> bool {
        let point = self.to_local(point);
        let p = [point.x, point.y];
        let near = |poly: &[[f32; 2]], closed: bool| {
            let n = poly.len();
            let edges = if closed { n } else { n.saturating_sub(1) };
            (0..edges).any(|i| distance_to_segment(p, poly[i], poly[(i + 1) % n]) < tolerance)
        };
        match &self.kind {
            ElementKind::Text { .. } => local_bounds(self).expand(tolerance / 2.0).contains(point),
            ElementKind::Line { .. } => near(&line_path(self), false),
            ElementKind::FreeDraw { points } => near(points, false),
            _ => {
                let poly = shape_polygon(self).unwrap_or_default();
                if let Some(name) = &self.frame {
                    let b = local_bounds(self);
                    let tag = Rect::from_min_size(b.min - Vec2::new(0.0, 22.0), Vec2::new((name.chars().count() as f32 * 8.0).max(40.0), 22.0));
                    return near(&poly, true) || tag.contains(point);
                }
                near(&poly, true) || ((self.style.fill.is_some() || self.image.is_some()) && polygon_contains(&poly, p))
            }
        }
    }
}

fn default_background() -> [u8; 4] {
    WHITE
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
struct Scene {
    version: u32,
    elements: Vec<Element>,
    #[serde(default = "default_background")]
    background: [u8; 4],
    /// The AI client that wrote this file through the MCP server, shown while the app
    /// replays what it drew.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    agent: Option<String>,
}

impl Default for Scene {
    fn default() -> Self {
        Self { version: 1, elements: Vec::new(), background: WHITE, agent: None }
    }
}

/// Main menu → Preferências (DefaultItems.tsx Preferences), remembered between launches.
#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq)]
#[serde(default)]
struct Prefs {
    box_overlap: bool,
    mouse_wheel_zooms: bool,
    tool_locked: bool,
    snap_objects: bool,
    grid: bool,
    zen: bool,
    view_mode: bool,
    stats: bool,
    arrow_binding: bool,
    midpoint_snapping: bool,
    show_hints: bool,
    theme: Theme,
}

impl Default for Prefs {
    fn default() -> Self {
        Self {
            box_overlap: false,
            mouse_wheel_zooms: false,
            tool_locked: false,
            snap_objects: false,
            grid: false,
            zen: false,
            view_mode: false,
            stats: false,
            arrow_binding: true,
            midpoint_snapping: true,
            show_hints: true,
            theme: Theme::Light,
        }
    }
}

const GRID: f32 = 20.0;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
enum Theme {
    #[default]
    Light,
    Dark,
    System,
}

/// Resize gesture in the selection's own frame (rotated for a single rotated element):
/// the opposite corner stays fixed, elements are rescaled from their originals.
struct Resize {
    anchor: Pos2,
    grab: Vec2,
    from: Rect,
    center: Pos2,
    angle: f32,
    originals: Vec<Element>,
}

struct Rotate {
    pivot: Pos2,
    start: f32,
    originals: Vec<Element>,
}

#[derive(Clone, Copy, Debug, PartialEq)]
enum Layer {
    Backward,
    Forward,
    Back,
    Front,
}

struct DrawingApp {
    scene: Scene,
    undo: Vec<Scene>,
    redo: Vec<Scene>,
    tool: Tool,
    tool_locked: bool,
    selected: Vec<usize>,
    style: Style,
    drawing_start: Option<Pos2>,
    drawing_points: Vec<[f32; 2]>,
    moving: bool,
    move_snapshotted: bool,
    last_pointer: Option<Pos2>,
    mods: Modifiers,
    theme: Theme,
    file_path: Option<PathBuf>,
    status: String,
    status_time: f64,
    zoom: f32,
    pan: Vec2,
    canvas_rect: Rect,
    marquee: Option<Pos2>,
    resizing: Option<Resize>,
    rotating: Option<Rotate>,
    line_editor: Option<usize>,
    point_drag: Option<usize>,
    selected_point: Option<usize>,
    multi: Option<usize>,
    arrow_curved: bool,
    heads: (Option<Head>, Option<Head>),
    head_picker: Option<bool>,
    head_picker_more: bool,
    panning: bool,
    erasing: Option<Vec<usize>>,
    editing: Option<usize>,
    edit_focused: bool,
    clipboard: Vec<Element>,
    menu_open: bool,
    export_open: bool,
    export_background: bool,
    export_scale: f32,
    system_dark: bool,
    palette_open: bool,
    palette_query: String,
    palette_index: usize,
    palette_focused: bool,
    minimap_open: bool,
    extra_open: bool,
    now: f64,
    laser: Vec<(Pos2, f64)>,
    laser_down: bool,
    lasso: Option<Vec<Pos2>>,
    /// Ids of the locked element (or whole group / card) last clicked: shows the unlock button.
    active_locked: Option<Vec<String>>,
    /// Elements an AI just added, drawn one after another (front one since `reveal_since`).
    reveal: std::collections::VecDeque<String>,
    reveal_since: f64,
    /// Who is drawing, and where its cursor last stopped (world) and when.
    agent: Option<(String, Pos2, f64)>,
    prefs: Prefs,
    saved_prefs: Prefs,
    prefs_open: bool,
    locked_open: bool,
    press_world: Pos2,
    move_origin: Option<(Vec<usize>, Vec<Element>, Rect)>,
    snap_guides: Vec<[Pos2; 2]>,
    style_edit_active: bool,
    dirty: bool,
    autosave: Option<PathBuf>,
    /// Scene as last read from / written to the shared file, for merging outside edits.
    synced: Scene,
    synced_stamp: Option<SystemTime>,
    last_poll: f64,
}

impl Default for DrawingApp {
    fn default() -> Self {
        Self {
            scene: Scene::default(),
            undo: Vec::new(),
            redo: Vec::new(),
            tool: Tool::Select,
            tool_locked: false,
            selected: Vec::new(),
            style: Style::default(),
            drawing_start: None,
            drawing_points: Vec::new(),
            moving: false,
            move_snapshotted: false,
            last_pointer: None,
            mods: Modifiers::NONE,
            theme: Theme::Light,
            file_path: None,
            status: String::new(),
            status_time: 0.0,
            zoom: 1.0,
            pan: Vec2::ZERO,
            canvas_rect: Rect::NOTHING,
            marquee: None,
            resizing: None,
            rotating: None,
            line_editor: None,
            point_drag: None,
            selected_point: None,
            multi: None,
            arrow_curved: true,
            heads: (None, Some(Head::Arrow)),
            head_picker: None,
            head_picker_more: false,
            panning: false,
            erasing: None,
            editing: None,
            edit_focused: false,
            clipboard: Vec::new(),
            menu_open: false,
            export_open: false,
            export_background: true,
            export_scale: 2.0,
            system_dark: false,
            palette_open: false,
            palette_query: String::new(),
            palette_index: 0,
            palette_focused: false,
            minimap_open: true,
            extra_open: false,
            now: 0.0,
            laser: Vec::new(),
            laser_down: false,
            lasso: None,
            active_locked: None,
            reveal: Default::default(),
            reveal_since: 0.0,
            agent: None,
            prefs: Prefs::default(),
            saved_prefs: Prefs::default(),
            prefs_open: false,
            locked_open: false,
            press_world: Pos2::ZERO,
            move_origin: None,
            snap_guides: Vec::new(),
            style_edit_active: false,
            dirty: false,
            autosave: None,
            synced: Scene::default(),
            synced_stamp: None,
            last_poll: 0.0,
        }
    }
}

impl DrawingApp {
    fn toast(&mut self, message: impl Into<String>) {
        self.status = message.into();
        self.status_time = -1.0;
    }

    fn snapshot(&mut self) {
        self.undo.push(self.scene.clone());
        if self.undo.len() > 100 {
            self.undo.remove(0);
        }
        self.redo.clear();
        self.dirty = true;
    }

    fn undo(&mut self) {
        if let Some(previous) = self.undo.pop() {
            self.redo.push(std::mem::replace(&mut self.scene, previous));
            self.selected.clear();
            self.dirty = true;
        }
    }

    fn redo(&mut self) {
        if let Some(next) = self.redo.pop() {
            self.undo.push(std::mem::replace(&mut self.scene, next));
            self.selected.clear();
            self.dirty = true;
        }
    }

    fn new_element(&self, kind: ElementKind) -> Element {
        let mut style = self.style;
        if matches!(kind, ElementKind::Line { .. } | ElementKind::FreeDraw { .. }) {
            // arrows default to curved, lines and freehand to sharp (reference defaults)
            style.round = matches!(kind, ElementKind::Line { arrow: true, .. }) && self.arrow_curved;
            style.fill = None;
        }
        Element {
            id: uuid::Uuid::new_v4().to_string(),
            group_id: None,
            kind,
            angle: 0.0,
            ends: ArrowEnds { start_head: self.heads.0, end_head: self.heads.1, ..Default::default() },
            locked: false,
            frame: None,
            image: None,
            created: None,
            style,
        }
    }

    fn origin(&self) -> Vec2 {
        self.canvas_rect.min.to_vec2() + self.pan
    }

    fn to_world(&self, screen: Pos2) -> Pos2 {
        (screen - self.origin()) / self.zoom
    }

    fn to_screen(&self, world: Pos2) -> Pos2 {
        world * self.zoom + self.origin()
    }

    // ---------- files ----------

    fn save_as(&mut self) {
        let path = rfd::FileDialog::new()
            .set_title("Salvar como")
            .add_filter("Desenho MagoAgent (.rea)", &["rea"])
            .add_filter("Outros quadros brancos", &[concat!("excali", "draw")])
            .set_file_name("desenho.rea")
            .save_file();
        if let Some(path) = path {
            self.save_to(path);
        }
    }

    fn save(&mut self) {
        match self.file_path.clone() {
            Some(path) => self.save_to(path),
            None => self.save_as(),
        }
    }

    fn save_to(&mut self, path: PathBuf) {
        let data = if path.extension().is_some_and(|e| e == "rea") {
            serde_json::to_string_pretty(&self.scene)
        } else {
            serde_json::to_string_pretty(&to_interop(&self.scene))
        };
        match data
            .map_err(|e| e.to_string())
            .and_then(|json| fs::write(&path, json).map_err(|e| e.to_string()))
        {
            Ok(()) => {
                let name = path.file_name().unwrap_or_default().to_string_lossy().to_string();
                self.file_path = Some(path);
                self.toast(format!("Salvo em {name}"));
            }
            Err(error) => self.toast(format!("Falha ao salvar: {error}")),
        }
    }

    fn open(&mut self) {
        let Some(path) = rfd::FileDialog::new()
            .set_title("Abrir")
            .add_filter("Desenhos", &["rea", concat!("excali", "draw"), "json"])
            .pick_file()
        else {
            return;
        };
        match fs::read_to_string(&path).map_err(|e| e.to_string()).and_then(|d| parse_scene(&d)) {
            Ok(scene) => {
                self.snapshot();
                self.scene = scene;
                self.file_path = Some(path);
                self.selected.clear();
                self.zoom_to_fit(false);
            }
            Err(error) => self.toast(format!("Não foi possível abrir: {error}")),
        }
    }

    /// Inserir imagem: PNG/JPEG becomes a rectangle carrying the picture, at most 600px wide,
    /// in the middle of the view.
    fn insert_image(&mut self) {
        if let Some(paths) = rfd::FileDialog::new().set_title("Inserir imagem").add_filter("Imagens", &IMAGE_EXTENSIONS).pick_files() {
            self.insert_image_files(paths, self.to_world(self.canvas_rect.center()));
        }
    }

    /// Images go side by side around `at`, as tall as the natural size allows: at most half
    /// the view and never more than the view minus 120px (initializeImage on the web).
    fn insert_image_files(&mut self, paths: Vec<PathBuf>, at: Pos2) {
        let max_height = (self.canvas_rect.height() - 120.0).max(160.0).min((self.canvas_rect.height() * 0.5).floor() / self.zoom);
        let mut elements = Vec::new();
        for path in paths {
            let loaded = fs::read(&path).map_err(|e| e.to_string()).and_then(|bytes| {
                let format = image::guess_format(&bytes).map_err(|e| e.to_string())?;
                let size = image::load_from_memory(&bytes).map_err(|e| e.to_string())?;
                Ok((format!("data:{};base64,{}", format.to_mime_type(), mcp::base64(&bytes)), Vec2::new(size.width() as f32, size.height() as f32)))
            });
            match loaded {
                Ok((url, natural)) => {
                    let height = natural.y.min(max_height);
                    let size = Vec2::new(height * natural.x / natural.y, height);
                    let mut element = self.new_element(ElementKind::Rectangle { x: 0.0, y: 0.0, width: size.x, height: size.y });
                    element.image = Some(url);
                    element.style.fill = None;
                    elements.push(element);
                }
                Err(error) => self.toast(format!("Não foi possível inserir a imagem: {error}")),
            }
        }
        // one row, 50px apart on screen, centered on `at`
        let gap = 50.0 / self.zoom;
        let row = elements.iter().map(|e| element_bounds(e).width()).sum::<f32>() + gap * elements.len().saturating_sub(1) as f32;
        let mut x = at.x - row / 2.0;
        for element in &mut elements {
            let b = element_bounds(element);
            element.move_by(Vec2::new(x, at.y - b.height() / 2.0));
            x += b.width() + gap;
        }
        self.insert_elements(elements);
    }

    fn export(&mut self, png: bool) {
        let (ext, title) = if png { ("png", "Imagem PNG") } else { ("svg", "Imagem SVG") };
        let Some(path) = rfd::FileDialog::new()
            .set_title("Exportar imagem")
            .add_filter(title, &[ext])
            .set_file_name(format!("desenho.{ext}"))
            .save_file()
        else {
            return;
        };
        let background = self.export_background.then_some(self.scene.background);
        let svg = scene_svg(&self.scene, background);
        let bytes = if png { render_svg_png(&svg, self.export_scale) } else { Ok(svg.into_bytes()) };
        match bytes.and_then(|b| fs::write(&path, b).map_err(|e| e.to_string())) {
            Ok(()) => {
                self.export_open = false;
                self.toast("Imagem exportada");
            }
            Err(error) => self.toast(format!("Falha ao exportar: {error}")),
        }
    }

    fn write_autosave(&mut self) {
        let Some(path) = self.autosave.clone() else {
            self.dirty = false;
            return;
        };
        if modified(&path) != self.synced_stamp && self.synced_stamp.is_some() {
            // an AI (MCP) wrote meanwhile: merge first, write on a later frame
            self.sync_from_file();
            return;
        }
        if let Ok(stamp) = write_scene_file(&path, &self.scene) {
            self.synced = self.scene.clone();
            self.synced_stamp = stamp;
            self.dirty = false;
        }
    }

    /// Picks up drawings made through the MCP server while the app is open.
    fn sync_from_file(&mut self) {
        let Some(path) = self.autosave.clone() else {
            return;
        };
        let stamp = modified(&path);
        if stamp == self.synced_stamp {
            return;
        }
        let Some((external, stamp)) = read_scene_file(&path) else {
            return;
        };
        // ponytail: an MCP write landing inside our own read-modify-write window (ms) can still lose one side
        let merged = merge_scenes(&self.synced, &self.scene, &external);
        if let Some(name) = &external.agent {
            let known: std::collections::HashSet<&str> =
                self.synced.elements.iter().chain(&self.scene.elements).map(|e| e.id.as_str()).collect();
            let fresh: Vec<String> =
                external.elements.iter().filter(|e| !known.contains(e.id.as_str())).map(|e| e.id.clone()).collect();
            if !fresh.is_empty() {
                if self.reveal.is_empty() {
                    self.reveal_since = self.now;
                }
                self.reveal.extend(fresh);
                let pos = self.agent.as_ref().map_or(self.to_world(self.canvas_rect.center()), |a| a.1);
                self.agent = Some((name.clone(), pos, self.now));
            }
        }
        if merged != self.scene {
            self.snapshot(); // Cmd+Z undoes what the AI did
            self.scene = merged;
            self.selected.retain(|i| *i < self.scene.elements.len());
            self.editing = None;
            self.line_editor = None;
            self.multi = None;
        }
        self.dirty = self.scene != external;
        self.synced = external;
        self.synced_stamp = stamp;
    }

    // ---------- editing actions ----------

    fn select_tool(&mut self, tool: Tool) {
        self.finish_text();
        self.finish_multi();
        self.line_editor = None;
        self.tool = tool;
        if !matches!(tool, Tool::Select | Tool::Hand) {
            self.selected.clear();
        }
    }

    fn delete_selected(&mut self) {
        if self.selected.is_empty() {
            return;
        }
        self.snapshot();
        self.selected.sort_unstable();
        self.selected.dedup();
        for index in self.selected.drain(..).rev() {
            if index < self.scene.elements.len() {
                self.scene.elements.remove(index);
            }
        }
    }

    fn clear_canvas(&mut self) {
        self.snapshot();
        self.scene.elements.clear();
        self.selected.clear();
        self.pan = Vec2::ZERO;
        self.zoom = 1.0;
    }

    fn group_selection(&mut self) {
        if self.selected.len() < 2 {
            return;
        }
        self.snapshot();
        let group_id = uuid::Uuid::new_v4().to_string();
        for index in self.selected.iter().copied() {
            if let Some(element) = self.scene.elements.get_mut(index) {
                element.group_id = Some(group_id.clone());
            }
        }
    }

    fn ungroup_selection(&mut self) {
        if !self.selected_has_group() {
            return;
        }
        self.snapshot();
        for index in self.selected.iter().copied() {
            if let Some(element) = self.scene.elements.get_mut(index) {
                element.group_id = None;
            }
        }
    }

    fn selected_has_group(&self) -> bool {
        self.selected
            .iter()
            .any(|i| self.scene.elements.get(*i).is_some_and(|e| e.group_id.is_some()))
    }

    fn select_all(&mut self) {
        self.tool = Tool::Select;
        self.selected = (0..self.scene.elements.len()).filter(|i| !self.scene.elements[*i].locked).collect();
    }

    /// Clones elements with fresh ids and group ids, so pasted groups stay separate groups.
    fn fresh_copies(elements: &[Element], offset: Vec2) -> Vec<Element> {
        let mut groups = std::collections::HashMap::new();
        let ids: std::collections::HashMap<&str, String> =
            elements.iter().map(|e| (e.id.as_str(), uuid::Uuid::new_v4().to_string())).collect();
        // a binding survives only when its target was copied too
        let remap = |b: &Option<Binding>| {
            b.as_ref().and_then(|b| ids.get(b.element_id.as_str()).map(|id| Binding { element_id: id.clone(), ..b.clone() }))
        };
        elements
            .iter()
            .map(|e| {
                let mut copy = e.clone();
                copy.id = ids[e.id.as_str()].clone();
                copy.ends.start = remap(&e.ends.start);
                copy.ends.end = remap(&e.ends.end);
                copy.group_id = e.group_id.as_ref().map(|g| {
                    groups.entry(g.clone()).or_insert_with(|| uuid::Uuid::new_v4().to_string()).clone()
                });
                copy.move_by(offset);
                copy
            })
            .collect()
    }

    fn insert_elements(&mut self, elements: Vec<Element>) {
        if elements.is_empty() {
            return;
        }
        self.snapshot();
        let start = self.scene.elements.len();
        self.scene.elements.extend(elements);
        self.selected = (start..self.scene.elements.len()).collect();
        self.tool = Tool::Select;
    }

    fn duplicate(&mut self) {
        let chosen: Vec<Element> =
            self.selected.iter().filter_map(|i| self.scene.elements.get(*i).cloned()).collect();
        self.insert_elements(Self::fresh_copies(&chosen, Vec2::splat(10.0)));
    }

    fn copy(&mut self, ctx: &Context) {
        self.clipboard =
            self.selected.iter().filter_map(|i| self.scene.elements.get(*i).cloned()).collect();
        if !self.clipboard.is_empty() {
            ctx.copy_text(
                json!({ "type": "magoagent/clipboard", "elements": interop_elements(&self.clipboard) })
                    .to_string(),
            );
        }
    }

    fn paste(&mut self, text: &str) {
        let parsed = serde_json::from_str::<Value>(text)
            .ok()
            .filter(|v| v["type"] == "magoagent/clipboard" || v["type"] == concat!("excali", "draw/clipboard"))
            .map(|v| from_interop_elements(&v["elements"]));
        let elements = match parsed {
            Some(elements) => elements,
            None if !text.trim().is_empty() => {
                vec![self.new_element(ElementKind::Text { x: 0.0, y: 0.0, value: text.trim_end().into() })]
            }
            None if !self.clipboard.is_empty() => self.clipboard.clone(),
            None => return,
        };
        // Paste centered under the cursor, like the reference app.
        let bounds = elements.iter().map(element_bounds).reduce(|a, b| a.union(b)).unwrap_or(Rect::ZERO);
        let target = self
            .last_pointer
            .filter(|p| self.canvas_rect.contains(*p))
            .map(|p| self.to_world(p))
            .unwrap_or(bounds.center() + Vec2::splat(10.0));
        self.insert_elements(Self::fresh_copies(&elements, target - bounds.center()));
    }

    fn nudge(&mut self, delta: Vec2) {
        if self.selected.is_empty() {
            return;
        }
        self.snapshot();
        self.detach_selected_arrows();
        for i in &self.selected {
            if let Some(e) = self.scene.elements.get_mut(*i) {
                e.move_by(delta);
            }
        }
    }

    fn reorder(&mut self, layer: Layer) {
        if self.selected.is_empty() {
            return;
        }
        self.snapshot();
        let mut items: Vec<(bool, Element)> = std::mem::take(&mut self.scene.elements)
            .into_iter()
            .enumerate()
            .map(|(i, e)| (self.selected.contains(&i), e))
            .collect();
        let n = items.len();
        match layer {
            Layer::Forward => {
                for i in (0..n.saturating_sub(1)).rev() {
                    if items[i].0 && !items[i + 1].0 {
                        items.swap(i, i + 1);
                    }
                }
            }
            Layer::Backward => {
                for i in 1..n {
                    if items[i].0 && !items[i - 1].0 {
                        items.swap(i, i - 1);
                    }
                }
            }
            Layer::Front => items.sort_by_key(|(s, _)| *s),
            Layer::Back => items.sort_by_key(|(s, _)| !*s),
        }
        self.selected = items.iter().enumerate().filter_map(|(i, (s, _))| s.then_some(i)).collect();
        self.scene.elements = items.into_iter().map(|(_, e)| e).collect();
    }

    fn zoom_at(&mut self, zoom: f32, anchor: Pos2) {
        let world = self.to_world(anchor);
        self.zoom = zoom.clamp(0.1, 30.0);
        self.pan = anchor - self.canvas_rect.min - world.to_vec2() * self.zoom;
    }

    fn zoom_step(&mut self, delta: f32) {
        let center = self.canvas_rect.center();
        let next = ((self.zoom + delta) * 10.0).round() / 10.0;
        self.zoom_at(next, center);
    }

    /// Grid snap for points (drawing), when the grid is on.
    fn snap_point(&self, p: Pos2) -> Pos2 {
        if self.prefs.grid && !self.mods.command {
            Pos2::new((p.x / GRID).round() * GRID, (p.y / GRID).round() * GRID)
        } else {
            p
        }
    }

    /// Offset that snaps a moved box to other elements' edges/centers (Encaixar em objetos)
    /// or to the grid, plus the red guide lines to show. Cmd disables snapping.
    fn snap_box(&self, moved: Rect) -> (Vec2, Vec<[Pos2; 2]>) {
        if self.mods.command {
            return (Vec2::ZERO, Vec::new());
        }
        if self.prefs.snap_objects {
            let threshold = 8.0 / self.zoom;
            let others: Vec<Rect> = self
                .scene
                .elements
                .iter()
                .enumerate()
                .filter(|(i, _)| !self.selected.contains(i))
                .map(|(_, e)| element_bounds(e))
                .collect();
            let mut adjust = Vec2::ZERO;
            let mut guides = Vec::new();
            for axis in 0..2 {
                let lines = |r: &Rect| if axis == 0 { [r.min.x, r.center().x, r.max.x] } else { [r.min.y, r.center().y, r.max.y] };
                let best = others
                    .iter()
                    .flat_map(|o| lines(o).into_iter().map(move |v| (v, *o)))
                    .flat_map(|(v, o)| lines(&moved).into_iter().map(move |m| (v - m, v, o)))
                    .filter(|(d, _, _)| d.abs() <= threshold)
                    .min_by(|a, b| a.0.abs().total_cmp(&b.0.abs()));
                if let Some((d, v, o)) = best {
                    adjust[axis] = d;
                    let span = moved.translate(adjust).union(o);
                    guides.push(if axis == 0 {
                        [Pos2::new(v, span.min.y), Pos2::new(v, span.max.y)]
                    } else {
                        [Pos2::new(span.min.x, v), Pos2::new(span.max.x, v)]
                    });
                }
            }
            if adjust != Vec2::ZERO {
                return (adjust, guides);
            }
        }
        if self.prefs.grid {
            return (self.snap_point(moved.min) - moved.min, Vec::new());
        }
        (Vec2::ZERO, Vec::new())
    }

    /// Centers the canvas on a world point (used by the minimap).
    fn pan_to(&mut self, world: Pos2) {
        self.pan = self.canvas_rect.size() / 2.0 - world.to_vec2() * self.zoom;
    }

    /// World area the minimap covers: every element plus what is on screen, with a margin.
    fn minimap_world(&self) -> Rect {
        let view = Rect::from_two_pos(self.to_world(self.canvas_rect.min), self.to_world(self.canvas_rect.max));
        let all = self.scene.elements.iter().map(element_bounds).fold(view, |a, b| a.union(b));
        all.expand(all.size().max_elem() * 0.05 + 20.0)
    }

    fn zoom_to_fit(&mut self, selection: bool) {
        let bounds = if selection {
            self.selection_bounds()
        } else {
            self.scene.elements.iter().map(element_bounds).reduce(|a, b| a.union(b))
        };
        let (Some(bounds), true) = (bounds, self.canvas_rect.is_positive()) else {
            return;
        };
        let view = self.canvas_rect.size() - Vec2::splat(120.0);
        let zoom = (view.x / bounds.width().max(1.0)).min(view.y / bounds.height().max(1.0));
        self.zoom = if selection { zoom.clamp(0.1, 30.0) } else { zoom.clamp(0.1, 1.0) };
        self.pan = self.canvas_rect.size() / 2.0 - bounds.center().to_vec2() * self.zoom;
    }

    fn start_text(&mut self, world: Pos2) {
        self.snapshot();
        let size = self.style.font_size;
        self.scene.elements.push(self.new_element(ElementKind::Text {
            x: world.x,
            y: world.y - size * LINE_HEIGHT / 2.0,
            value: String::new(),
        }));
        let index = self.scene.elements.len() - 1;
        self.selected = vec![index];
        self.editing = Some(index);
        self.edit_focused = false;
    }

    fn finish_text(&mut self) {
        let Some(index) = self.editing.take() else {
            return;
        };
        self.dirty = true;
        let empty = matches!(
            self.scene.elements.get(index),
            Some(Element { kind: ElementKind::Text { value, .. }, .. }) if value.trim().is_empty()
        );
        if empty {
            self.scene.elements.remove(index);
            self.selected.clear();
        } else {
            self.selected = vec![index];
        }
        if !self.tool_locked {
            self.tool = Tool::Select;
        }
    }

    // ---------- keyboard ----------

    fn handle_shortcuts(&mut self, ctx: &Context) {
        if ctx.egui_wants_keyboard_input() || self.editing.is_some() || ctx.current_pass_index() > 0 {
            return;
        }
        let events = ctx.input(|i| i.events.clone());
        let hit = |key: Key, command: bool, shift: bool, alt: bool| {
            events.iter().any(|e| {
                matches!(e, egui::Event::Key { key: k, pressed: true, modifiers, .. }
                    if *k == key && modifiers.command == command && modifiers.shift == shift && modifiers.alt == alt)
            })
        };
        let plain = |key: Key| hit(key, false, false, false);
        let cmd = |key: Key| hit(key, true, false, false);
        let cmd_shift = |key: Key| hit(key, true, true, false);

        let editable = !self.prefs.view_mode;
        for event in events.iter().filter(|_| editable) {
            match event {
                egui::Event::Copy => self.copy(ctx),
                egui::Event::Cut => {
                    self.copy(ctx);
                    self.delete_selected();
                }
                egui::Event::Paste(text) => self.paste(text),
                _ => {}
            }
        }
        // preferences: Option+S snap, Cmd+' grid, Option+Z zen, Option+R view mode, Option+/ stats
        let alt = |key: Key| hit(key, false, false, true);
        if alt(Key::S) {
            self.prefs.snap_objects = !self.prefs.snap_objects;
        }
        if cmd(Key::Quote) {
            self.prefs.grid = !self.prefs.grid;
        }
        if alt(Key::Z) {
            self.prefs.zen = !self.prefs.zen;
        }
        if alt(Key::R) {
            self.prefs.view_mode = !self.prefs.view_mode;
            self.selected.clear();
        }
        if alt(Key::Slash) {
            self.prefs.stats = !self.prefs.stats;
        }
        if self.prefs.view_mode {
            // view mode: look around only
            if cmd(Key::Equals) || cmd(Key::Plus) {
                self.zoom_step(0.1);
            }
            if cmd(Key::Minus) {
                self.zoom_step(-0.1);
            }
            if cmd(Key::Slash) {
                self.open_palette();
            }
            if plain(Key::Escape) {
                self.prefs.view_mode = false;
            }
            return;
        }
        if let Some(end) = self.head_picker {
            for (key, pick) in [(Key::Q, None), (Key::W, Some(Head::Arrow)), (Key::E, Some(Head::Triangle)), (Key::R, Some(Head::TriangleOutline))] {
                if plain(key) {
                    self.set_head(end, pick);
                    return;
                }
            }
        }
        if self.tool == Tool::Fill && plain(Key::B) {
            // B again cycles the fill color (red, green, blue, yellow)
            let picks = &BACKGROUND_PICKS[1..];
            let at = picks.iter().position(|c| Some(*c) == self.style.fill).map_or(1, |i| (i + 1) % picks.len());
            self.style.fill = Some(picks[at]);
        } else {
            for tool in Tool::TOOLBAR.into_iter().chain(Tool::EXTRA) {
                if tool.keys().iter().any(|k| plain(*k)) {
                    self.select_tool(tool);
                }
            }
        }
        if hit(Key::X, false, true, false) {
            self.select_tool(Tool::AutoShape);
        }
        if plain(Key::Num9) {
            self.insert_image();
        }
        if plain(Key::Q) {
            self.tool_locked = !self.tool_locked;
        }
        let finish_keys = plain(Key::Escape) || plain(Key::Enter);
        if finish_keys && self.multi.is_some() {
            self.finish_multi();
        } else if finish_keys && self.line_editor.is_some() {
            self.line_editor = None;
            self.selected_point = None;
        } else if cmd(Key::Enter) && self.single_line().is_some() {
            self.line_editor = self.single_line();
            self.selected_point = None;
        } else if plain(Key::Escape) {
            self.menu_open = false;
            if self.selected.is_empty() {
                self.tool = Tool::Select;
            }
            self.selected.clear();
        } else if plain(Key::Enter) {
            if let [i] = self.selected[..] {
                if matches!(self.scene.elements.get(i), Some(Element { kind: ElementKind::Text { .. }, .. })) {
                    self.snapshot();
                    self.editing = Some(i);
                    self.edit_focused = false;
                }
            }
        }
        if plain(Key::Delete) || plain(Key::Backspace) {
            if !self.delete_selected_point() {
                self.delete_selected();
            }
        }
        if cmd(Key::Delete) || cmd(Key::Backspace) {
            self.clear_canvas();
        }
        if cmd(Key::Z) {
            self.undo();
        }
        if cmd_shift(Key::Z) || cmd(Key::Y) {
            self.redo();
        }
        if cmd(Key::S) {
            self.save();
        }
        if cmd_shift(Key::S) {
            self.save_as();
        }
        if cmd(Key::O) {
            self.open();
        }
        if cmd_shift(Key::E) {
            self.export_open = true;
        }
        if cmd(Key::G) {
            self.group_selection();
        }
        if cmd_shift(Key::G) {
            self.ungroup_selection();
        }
        if cmd_shift(Key::L) {
            self.toggle_lock_selected();
        }
        if cmd(Key::A) {
            self.select_all();
        }
        if cmd(Key::D) {
            self.duplicate();
        }
        if cmd(Key::Equals) || cmd(Key::Plus) || cmd_shift(Key::Equals) || cmd_shift(Key::Plus) {
            self.zoom_step(0.1);
        }
        if cmd(Key::Minus) {
            self.zoom_step(-0.1);
        }
        if cmd(Key::Num0) {
            let center = self.canvas_rect.center();
            self.zoom_at(1.0, center);
        }
        if hit(Key::Num1, false, true, false) || plain(Key::Exclamationmark) {
            self.zoom_to_fit(false);
        }
        if hit(Key::Num2, false, true, false) {
            self.zoom_to_fit(true);
        }
        for (key, delta) in [
            (Key::ArrowLeft, Vec2::new(-1.0, 0.0)),
            (Key::ArrowRight, Vec2::new(1.0, 0.0)),
            (Key::ArrowUp, Vec2::new(0.0, -1.0)),
            (Key::ArrowDown, Vec2::new(0.0, 1.0)),
        ] {
            if plain(key) {
                self.nudge(delta);
            }
            if hit(key, false, true, false) {
                self.nudge(delta * 5.0);
            }
        }
        if cmd(Key::CloseBracket) {
            self.reorder(Layer::Forward);
        }
        if cmd(Key::OpenBracket) {
            self.reorder(Layer::Backward);
        }
        if cmd_shift(Key::CloseBracket) || hit(Key::CloseBracket, true, false, true) {
            self.reorder(Layer::Front);
        }
        if cmd_shift(Key::OpenBracket) || hit(Key::OpenBracket, true, false, true) {
            self.reorder(Layer::Back);
        }
        if hit(Key::D, false, true, true) {
            self.theme = if self.dark() { Theme::Light } else { Theme::Dark };
        }
        if cmd(Key::Slash) || plain(Key::Questionmark) || hit(Key::Questionmark, false, true, false) || hit(Key::Slash, false, true, false) {
            self.open_palette();
        }
    }

    // ---------- canvas pointer ----------

    fn selection_bounds(&self) -> Option<Rect> {
        self.selected
            .iter()
            .filter_map(|i| self.scene.elements.get(*i))
            .map(element_bounds)
            .reduce(|a, b| a.union(b))
    }

    /// Selection box as (center, size, angle) in world units: rotated for one rotated element.
    fn selection_frame(&self) -> Option<(Pos2, Vec2, f32)> {
        if let [i] = self.selected[..] {
            if let Some(e) = self.scene.elements.get(i).filter(|e| e.angle != 0.0) {
                let b = local_bounds(e);
                return Some((b.center(), b.size(), e.angle));
            }
        }
        let b = self.selection_bounds()?;
        Some((b.center(), b.size(), 0.0))
    }

    fn shows_transform_handles(&self) -> bool {
        // The reference app shows no bounding box for a lone 2-point line/arrow, just its points
        let two_point_line = self.single_line().is_some_and(|i| self.line_points(i).len() == 2);
        self.tool == Tool::Select
            && self.editing.is_none()
            && self.line_editor.is_none()
            && self.multi.is_none()
            && !two_point_line
    }

    fn handle_corners(&self) -> Option<[Pos2; 4]> {
        let (center, size, angle) = self.selection_frame()?;
        let c = self.to_screen(center);
        let h = size * self.zoom / 2.0 + Vec2::splat(5.0);
        Some([(-1.0, -1.0), (1.0, -1.0), (1.0, 1.0), (-1.0, 1.0)].map(|(sx, sy)| c + rot(Vec2::new(sx * h.x, sy * h.y), angle)))
    }

    /// Circle above the box, 16px gap like the reference app's ROTATION_RESIZE_HANDLE_GAP.
    fn rotation_handle(&self) -> Option<Pos2> {
        let (center, size, angle) = self.selection_frame()?;
        Some(self.to_screen(center) + rot(Vec2::new(0.0, -(size.y * self.zoom / 2.0 + 5.0 + 16.0)), angle))
    }

    fn hovered_handle(&self, pos: Pos2) -> Option<usize> {
        if !self.shows_transform_handles() {
            return None;
        }
        self.handle_corners()?.iter().position(|c| c.distance(pos) <= 7.0)
    }

    fn hovered_rotation(&self, pos: Pos2) -> bool {
        self.shows_transform_handles() && self.rotation_handle().is_some_and(|h| h.distance(pos) <= 8.0)
    }

    fn single_line(&self) -> Option<usize> {
        match self.selected[..] {
            [i] if matches!(self.scene.elements.get(i), Some(Element { kind: ElementKind::Line { .. }, .. })) => Some(i),
            _ => None,
        }
    }

    fn line_points(&self, index: usize) -> &[[f32; 2]] {
        match self.scene.elements.get(index) {
            Some(Element { kind: ElementKind::Line { points, .. }, .. }) => points,
            _ => &[],
        }
    }

    fn line_points_mut(&mut self, index: usize) -> Option<&mut Vec<[f32; 2]>> {
        match self.scene.elements.get_mut(index) {
            Some(Element { kind: ElementKind::Line { points, .. }, .. }) => Some(points),
            _ => None,
        }
    }

    fn editable_line(&self) -> Option<usize> {
        self.single_line().filter(|_| self.tool == Tool::Select && self.multi.is_none() && self.editing.is_none())
    }

    /// Every point of the selected line gets a handle (renderLinearPointHandles).
    fn point_handles(&self) -> Vec<(usize, Pos2)> {
        let Some(i) = self.editable_line() else {
            return Vec::new();
        };
        self.line_points(i).iter().enumerate().map(|(k, p)| (k, self.to_screen(Pos2::new(p[0], p[1])))).collect()
    }

    /// Segment midpoints (on the curve for curved lines), shown in the editor or on 2-point lines.
    fn midpoint_handles(&self) -> Vec<(usize, Pos2)> {
        let Some(i) = self.editable_line() else {
            return Vec::new();
        };
        let points = self.line_points(i);
        if self.line_editor != Some(i) && points.len() != 2 {
            return Vec::new();
        }
        let path = line_path(&self.scene.elements[i]);
        let curved = path.len() != points.len();
        (0..points.len() - 1)
            .map(|k| {
                let m = if curved {
                    path[k * LINE_CURVE_STEPS + LINE_CURVE_STEPS / 2]
                } else {
                    [(points[k][0] + points[k + 1][0]) / 2.0, (points[k][1] + points[k + 1][1]) / 2.0]
                };
                (k, self.to_screen(Pos2::new(m[0], m[1])))
            })
            .collect()
    }

    fn delete_selected_point(&mut self) -> bool {
        let (Some(i), Some(k)) = (self.line_editor, self.selected_point) else {
            return false;
        };
        if self.line_points(i).len() <= 2 {
            return true;
        }
        self.snapshot();
        if let Some(points) = self.line_points_mut(i) {
            points.remove(k.min(points.len() - 1));
        }
        self.selected_point = None;
        true
    }

    fn finish_multi(&mut self) {
        let Some(i) = self.multi.take() else {
            return;
        };
        let remaining = self.line_points_mut(i).map(|points| {
            points.pop(); // the point following the cursor
            points.len()
        });
        if let Some(&last) = self.line_points(i).last().filter(|_| remaining.unwrap_or(0) >= 2) {
            let binding = self.bind_at(Pos2::new(last[0], last[1]), Some(i));
            self.scene.elements[i].ends.end = binding;
        }
        if remaining.unwrap_or(0) < 2 {
            self.scene.elements.remove(i);
            self.selected.clear();
        } else {
            self.selected = vec![i];
        }
        self.dirty = true;
        if !self.tool_locked {
            self.tool = Tool::Select;
        }
    }

    /// Shift snaps a segment to 15° steps around `from`.
    fn snap_angle(&self, from: [f32; 2], to: Pos2) -> Pos2 {
        if !self.mods.shift {
            return to;
        }
        let d = to - Pos2::new(from[0], from[1]);
        let step = std::f32::consts::PI / 12.0;
        Pos2::new(from[0], from[1]) + Vec2::angled((d.y.atan2(d.x) / step).round() * step) * d.length()
    }

    fn hit(&self, world: Pos2) -> Option<usize> {
        let tolerance = 10.0 / self.zoom;
        self.scene.elements.iter().rposition(|e| !e.locked && e.contains(world, tolerance))
    }

    /// 0..1 through drawing `e`, the front of the reveal queue; a long queue plays faster.
    fn reveal_progress(&self, e: &Element) -> f32 {
        let speed = 1.0 + self.reveal.len() as f64 / 8.0;
        ((self.now - self.reveal_since) * speed / reveal_duration(e)).clamp(0.0, 1.0) as f32
    }

    /// Preenchimento: paints the background of the shape under the pointer.
    fn fill_at(&mut self, world: Pos2) {
        let target = self.scene.elements.iter().rposition(|e| {
            !e.locked && e.fillable() && e.image.is_none() && shape_polygon(e).is_some_and(|poly| {
                let p = e.to_local(world);
                polygon_contains(&poly, [p.x, p.y])
            })
        });
        if let Some(i) = target {
            self.snapshot();
            let element = &mut self.scene.elements[i];
            // a transparent pick would be invisible: the web falls back to green
            element.style.fill = Some(self.style.fill.filter(|c| c[3] > 0).unwrap_or(BACKGROUND_PICKS[2]));
            element.style.fill_style = self.style.fill_style;
            element.style.opacity = self.style.opacity;
        }
    }

    /// Selects what the freehand lasso surrounds (or touches, with "Sobreposto" box
    /// selection), whole groups at a time, live while it's drawn.
    // ponytail: tests element bounding boxes, not their exact outlines.
    fn select_in_lasso(&mut self, path: &[Pos2]) {
        if path.len() < 3 {
            self.selected.clear();
            return;
        }
        let poly: Vec<[f32; 2]> = path.iter().map(|p| [p.x, p.y]).collect();
        let inside = |p: Pos2| polygon_contains(&poly, [p.x, p.y]);
        let mut selected: Vec<usize> = self
            .scene
            .elements
            .iter()
            .enumerate()
            .filter(|(_, e)| !e.locked && !is_label(e))
            .filter(|(_, e)| {
                let b = element_bounds(e);
                let mut corners = [b.left_top(), b.right_top(), b.right_bottom(), b.left_bottom()].into_iter();
                if self.prefs.box_overlap {
                    corners.any(inside) || path.iter().any(|p| b.contains(*p))
                } else {
                    corners.all(inside)
                }
            })
            .map(|(i, _)| i)
            .collect();
        for i in selected.clone() {
            for j in self.group_members(i) {
                if !selected.contains(&j) {
                    selected.push(j);
                }
            }
        }
        self.selected = selected;
    }

    /// Nota adesiva: a 250px square centered on a click, or the dragged size (square unless
    /// Shift, at least 75px), with its text being edited right away.
    fn place_sticky(&mut self, start: Pos2, end: Pos2) {
        let d = end - start;
        let rect = if d.x.abs().max(d.y.abs()) * self.zoom < 10.0 {
            Rect::from_center_size(start, Vec2::splat(250.0))
        } else {
            let (mut w, mut h) = (d.x.abs().max(75.0), d.y.abs().max(75.0));
            if !self.mods.shift {
                w = w.max(h);
                h = w;
            }
            let x = if d.x < 0.0 { start.x - w } else { start.x };
            let y = if d.y < 0.0 { start.y - h } else { start.y };
            Rect::from_min_size(Pos2::new(x, y), Vec2::new(w, h))
        };
        let mut note = self.new_element(ElementKind::Rectangle { x: rect.min.x, y: rect.min.y, width: rect.width(), height: rect.height() });
        note.style = sticky_style(note.style);
        note.created = Some(now_ms());
        let group = uuid::Uuid::new_v4().to_string();
        note.group_id = Some(group.clone());
        let mut text = self.new_element(ElementKind::Text { x: rect.center().x, y: rect.center().y, value: String::new() });
        text.id = format!("{}-label", note.id);
        text.group_id = Some(group);
        text.style.stroke = note.style.stroke;
        text.style.font_size = 20.0;
        self.snapshot();
        self.scene.elements.push(note);
        self.scene.elements.push(text);
        let index = self.scene.elements.len() - 1;
        self.selected = vec![index];
        self.editing = Some(index);
        self.edit_focused = false;
    }

    /// The given elements plus everything lying inside any frame among them.
    fn with_frame_contents(&self, indices: &[usize]) -> Vec<usize> {
        let frames: Vec<Rect> = indices
            .iter()
            .filter_map(|i| self.scene.elements.get(*i))
            .filter(|e| e.frame.is_some())
            .map(|e| element_bounds(e).expand(1.0))
            .collect();
        let mut all = indices.to_vec();
        for (i, e) in self.scene.elements.iter().enumerate() {
            if !all.contains(&i) && !e.locked && frames.iter().any(|f| f.contains_rect(element_bounds(e))) {
                all.push(i);
            }
        }
        all
    }

    /// Locks the selection (Cmd+Shift+L); if all of it is locked already, unlocks it.
    /// The web's UnlockPopup: a lock button 12px above the clicked locked element's top-left
    /// corner; clicking it unlocks the whole unit and selects it.
    fn unlock_popup(&mut self, ctx: &Context) {
        let Some(ids) = self.active_locked.clone() else {
            return;
        };
        let unit: Vec<usize> = self.scene.elements.iter().enumerate().filter(|(_, e)| ids.contains(&e.id) && e.locked).map(|(i, _)| i).collect();
        let Some(bounds) = unit.iter().map(|i| element_bounds(&self.scene.elements[*i])).reduce(|a, b| a.union(b)) else {
            self.active_locked = None;
            return;
        };
        let c = self.colors();
        // above a card's name, not on top of it
        let name = if unit.iter().any(|i| self.scene.elements[*i].frame.is_some()) { 22.0 * self.zoom } else { 0.0 };
        let corner = self.to_screen(bounds.min) - Vec2::new(0.0, name);
        let size = 20.0 + 2.0 * 12.8;
        let response = egui::Area::new(egui::Id::unique("unlock_popup"))
            .fixed_pos(corner - Vec2::new(0.0, 12.0 + size))
            .order(egui::Order::Foreground)
            .show(ctx, |ui| {
                self.island()
                    .inner_margin(0)
                    .show(ui, |ui| {
                        let (rect, response) = ui.allocate_exact_size(Vec2::splat(size), Sense::click());
                        let color = if response.hovered() { PRIMARY } else { c.muted };
                        paint_icon(ui.painter(), Icon::Lock(true), rect.center(), color);
                        response.on_hover_cursor(CursorIcon::PointingHand).on_hover_text("Desbloquear")
                    })
                    .inner
            })
            .inner;
        if response.clicked() {
            self.selected = unit;
            self.toggle_lock_selected();
            self.active_locked = None;
        }
    }

    /// What locks and unlocks as one: the element's outermost group and, for a frame (card),
    /// everything inside it.
    fn lock_unit(&self, indices: &[usize]) -> Vec<usize> {
        let mut unit = indices.to_vec();
        let mut k = 0;
        while k < unit.len() {
            let e = &self.scene.elements[unit[k]];
            let frame = e.frame.is_some().then(|| element_bounds(e).expand(1.0));
            for (j, other) in self.scene.elements.iter().enumerate() {
                let grouped = e.group_id.is_some() && other.group_id == e.group_id;
                if !unit.contains(&j) && (grouped || frame.is_some_and(|f| f.contains_rect(element_bounds(other)))) {
                    unit.push(j);
                }
            }
            k += 1;
        }
        unit
    }

    fn toggle_lock_selected(&mut self) {
        if self.selected.is_empty() {
            return;
        }
        self.snapshot();
        let unit = self.lock_unit(&self.selected);
        let lock = unit.iter().any(|i| !self.scene.elements[*i].locked);
        for i in unit {
            self.scene.elements[i].locked = lock;
        }
        if lock {
            self.selected.clear();
        }
    }

    fn lock_all(&mut self) {
        if self.scene.elements.iter().all(|e| e.locked) {
            return;
        }
        self.snapshot();
        for e in &mut self.scene.elements {
            e.locked = true;
        }
        self.selected.clear();
    }

    fn unlock(&mut self, id: Option<&str>) {
        self.snapshot();
        for e in self.scene.elements.iter_mut().filter(|e| id.is_none_or(|id| e.id == id)) {
            e.locked = false;
        }
    }

    /// Short name for lists: the text itself, the shape's label, or the kind of element.
    fn describe(&self, e: &Element) -> String {
        let text_of = |e: &Element| match &e.kind {
            ElementKind::Text { value, .. } => Some(value.lines().next().unwrap_or("").to_string()),
            _ => None,
        };
        let label = self.scene.elements.iter().find(|l| l.id == format!("{}-label", e.id)).and_then(text_of);
        let kind = match &e.kind {
            ElementKind::Rectangle { .. } => "Retângulo",
            ElementKind::Ellipse { .. } => "Elipse",
            ElementKind::Diamond { .. } => "Losango",
            ElementKind::Line { arrow: true, .. } => "Flecha",
            ElementKind::Line { .. } => "Linha",
            ElementKind::FreeDraw { .. } => "Desenho",
            ElementKind::Text { .. } => "Texto",
        };
        let detail = text_of(e).or(label).filter(|t| !t.trim().is_empty());
        let mut out = match detail {
            Some(t) if matches!(e.kind, ElementKind::Text { .. }) => format!("“{t}”"),
            Some(t) => format!("{kind} · {t}"),
            None => kind.to_string(),
        };
        if out.chars().count() > 34 {
            out = out.chars().take(33).collect::<String>() + "…";
        }
        out
    }

    fn group_members(&self, index: usize) -> Vec<usize> {
        let group = self.scene.elements[index].group_id.clone();
        self.scene
            .elements
            .iter()
            .enumerate()
            .filter_map(|(j, e)| {
                (j == index || (!e.locked && group.as_ref().is_some_and(|g| e.group_id.as_ref() == Some(g)))).then_some(j)
            })
            .collect()
    }

    fn press_select(&mut self, pos: Pos2, world: Pos2, modifiers: Modifiers) {
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
        if let (Some(line), Some((k, _))) =
            (self.editable_line(), self.midpoint_handles().into_iter().find(|(_, p)| p.distance(pos) <= 8.0))
        {
            self.snapshot();
            if let Some(points) = self.line_points_mut(line) {
                points.insert(k + 1, [world.x, world.y]);
            }
            self.point_drag = Some(k + 1);
            self.selected_point = Some(k + 1);
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
            });
        } else if let Some(i) = self.hit(world) {
            let members = self.group_members(i);
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

    fn erase_at(&mut self, world: Pos2) {
        if let (Some(i), Some(erasing)) = (self.hit(world), &self.erasing) {
            if !erasing.contains(&i) {
                let members = self.group_members(i);
                if let Some(erasing) = &mut self.erasing {
                    erasing.extend(members.into_iter().filter(|j| !erasing.contains(j)).collect::<Vec<_>>());
                }
            }
        }
    }

    fn handle_canvas(&mut self, response: &egui::Response, rect: Rect) {
        self.canvas_rect = rect;
        let ctx = response.ctx.clone();
        if ctx.current_pass_index() > 0 {
            return;
        }
        self.mods = ctx.input(|i| i.modifiers);
        self.now = ctx.input(|i| i.time);
        // images dragged in from Finder land where they're dropped
        let dropped: Vec<PathBuf> = ctx.input(|i| i.raw.dropped_files.iter().map(|f| f.path().to_path_buf()).collect());
        let images: Vec<PathBuf> = dropped
            .into_iter()
            .filter(|p| p.extension().and_then(|e| e.to_str()).is_some_and(|e| IMAGE_EXTENSIONS.contains(&e.to_lowercase().as_str())))
            .collect();
        if !images.is_empty() {
            let at = ctx.input(|i| i.pointer.hover_pos()).unwrap_or(self.canvas_rect.center());
            self.insert_image_files(images, self.to_world(at));
        }
        let space = !ctx.egui_wants_keyboard_input() && ctx.input(|i| i.key_down(Key::Space));
        let events = ctx.input(|i| i.events.clone());
        for event in events {
            match event {
                egui::Event::PointerButton {
                    pos,
                    button: button @ (egui::PointerButton::Primary | egui::PointerButton::Middle),
                    pressed: true,
                    modifiers,
                } => {
                    if !rect.contains(pos) || ctx.layer_id_at(pos) != Some(response.layer_id) {
                        continue;
                    }
                    self.menu_open = false;
                    // any press on the canvas dismisses the unlock button (its own clicks never get here)
                    self.active_locked = None;
                    self.mods = modifiers;
                    self.last_pointer = Some(pos);
                    if self.editing.is_some() {
                        self.finish_text();
                        continue;
                    }
                    let world = self.to_world(pos);
                    if let Some(i) = self.multi {
                        // click on the last placed point finishes, any other click adds a point
                        let points = self.line_points(i);
                        let last = points.len().checked_sub(2).map(|k| points[k]);
                        if last.is_some_and(|p| self.to_screen(Pos2::new(p[0], p[1])).distance(pos) <= 10.0) {
                            self.finish_multi();
                        } else if let Some(points) = self.line_points_mut(i) {
                            let floating = *points.last().unwrap_or(&[world.x, world.y]);
                            points.push(floating);
                        }
                        continue;
                    }
                    self.press_world = world;
                    if button == egui::PointerButton::Middle || space || self.tool == Tool::Hand || self.prefs.view_mode {
                        self.panning = true;
                        continue;
                    }
                    match self.tool {
                        Tool::Select => self.press_select(pos, world, modifiers),
                        Tool::Text => self.start_text(world),
                        Tool::Eraser => {
                            self.erasing = Some(Vec::new());
                            self.erase_at(world);
                        }
                        Tool::Laser => {
                            self.laser_down = true;
                            self.laser.push((world, self.now));
                        }
                        Tool::Fill => self.fill_at(world),
                        Tool::Lasso => self.lasso = Some(vec![world]),
                        _ => {
                            let start = if matches!(self.tool, Tool::Draw | Tool::AutoShape) { world } else { self.snap_point(world) };
                            self.drawing_start = Some(start);
                            self.drawing_points = vec![[world.x, world.y]];
                        }
                    }
                }
                egui::Event::PointerMoved(pos) => self.update_gesture(pos),
                egui::Event::PointerButton {
                    pos,
                    button: egui::PointerButton::Primary | egui::PointerButton::Middle,
                    pressed: false,
                    ..
                } => {
                    self.update_gesture(pos);
                    self.release(pos);
                }
                _ => {}
            }
        }
        let double = ctx.input(|i| i.pointer.button_double_clicked(egui::PointerButton::Primary));
        if let (true, Tool::Select, None, false, Some(pos)) =
            (double, self.tool, self.editing, self.prefs.view_mode, ctx.input(|i| i.pointer.interact_pos()))
        {
            if rect.contains(pos) && ctx.layer_id_at(pos) == Some(response.layer_id) {
                let world = self.to_world(pos);
                match self.hit(world) {
                    // lines: double-click; arrows: Cmd + double-click (plain double-click labels arrows in the reference app)
                    Some(i) if matches!(self.scene.elements[i].kind, ElementKind::Line { arrow, .. } if !arrow || self.mods.command) => {
                        self.moving = false;
                        self.point_drag = None;
                        self.selected = vec![i];
                        self.line_editor = Some(i);
                        self.selected_point = None;
                    }
                    Some(i) if matches!(self.scene.elements[i].kind, ElementKind::Text { .. }) => {
                        self.snapshot();
                        self.moving = false;
                        self.selected = vec![i];
                        self.editing = Some(i);
                        self.edit_focused = false;
                    }
                    None => {
                        self.marquee = None;
                        self.start_text(world);
                    }
                    _ => {}
                }
            }
        }
        self.update_bindings();
    }

    /// maxBindingDistance_simple: 15px, a bit more when zoomed out (in world units).
    fn binding_reach(&self) -> f32 {
        (15.0 / (self.zoom.min(1.0) * 1.5)).clamp(15.0, 30.0) / self.zoom
    }

    /// Bindable shape under `world` (inside it or within reach of its outline). Cmd disables binding.
    fn binding_target(&self, world: Pos2, exclude: Option<usize>) -> Option<usize> {
        if self.mods.command || !self.prefs.arrow_binding {
            return None;
        }
        let reach = self.binding_reach();
        let p = [world.x, world.y];
        self.scene.elements.iter().enumerate().rev().find_map(|(i, e)| {
            if Some(i) == exclude || !bindable(e) {
                return None;
            }
            let poly = binding_outline(e, 0.0);
            let n = poly.len();
            let near = (0..n).any(|k| distance_to_segment(p, poly[k], poly[(k + 1) % n]) <= reach);
            (near || polygon_contains(&poly, p)).then_some(i)
        })
    }

    /// Deep inside a shape binds "inside" (to that exact point), near its outline "orbit"
    /// (stops at the outline); Option forces "inside" — as observed in the reference app.
    fn bind_at(&self, world: Pos2, exclude: Option<usize>) -> Option<Binding> {
        let target = &self.scene.elements[self.binding_target(world, exclude)?];
        let poly = binding_outline(target, 0.0);
        let p = [world.x, world.y];
        let n = poly.len();
        let edge = (0..n).map(|k| distance_to_segment(p, poly[k], poly[(k + 1) % n])).fold(f32::MAX, f32::min);
        let deep = polygon_contains(&poly, p) && edge > self.binding_reach();
        let mut binding = binding_for(target, world, self.mods.alt || deep);
        if self.prefs.midpoint_snapping && !binding.inside {
            // Encaixar em pontos intermediários: lock onto the middle of the nearest side
            let mids = [[0.5, 0.0], [1.0, 0.5], [0.5, 1.0], [0.0, 0.5]];
            if let Some(mid) = mids.into_iter().find(|m| {
                let probe = Binding { fixed: *m, ..binding.clone() };
                focus_point(target, &probe).distance(world) <= self.binding_reach()
            }) {
                binding.fixed = mid;
            }
        }
        Some(binding)
    }

    /// Binds a freshly drawn arrow to the shapes under its first and last point.
    fn bind_new_arrow(&self, mut arrow: Element) -> Element {
        let ElementKind::Line { points, arrow: true } = &arrow.kind else {
            return arrow;
        };
        let (first, last) = (points[0], points[points.len() - 1]);
        arrow.ends.start = self.bind_at(Pos2::new(first[0], first[1]), None);
        arrow.ends.end = self.bind_at(Pos2::new(last[0], last[1]), None);
        if let (Some(a), Some(b)) = (&mut arrow.ends.start, &mut arrow.ends.end) {
            if a.element_id == b.element_id {
                // both ends in one shape: the reference app keeps them where they were drawn
                (a.inside, b.inside) = (true, true);
            }
        }
        resolve_arrow(&self.scene.elements, &mut arrow);
        arrow
    }

    fn set_head(&mut self, end: bool, head: Option<Head>) {
        let arrows: Vec<usize> = self
            .selected
            .iter()
            .copied()
            .filter(|i| matches!(self.scene.elements.get(*i), Some(Element { kind: ElementKind::Line { arrow: true, .. }, .. })))
            .collect();
        if !arrows.is_empty() {
            self.snapshot();
        }
        for i in arrows {
            let ends = &mut self.scene.elements[i].ends;
            *(if end { &mut ends.end_head } else { &mut ends.start_head }) = head;
        }
        *(if end { &mut self.heads.1 } else { &mut self.heads.0 }) = head;
    }

    /// Moving/rotating/resizing an arrow without its target breaks that binding.
    fn detach_selected_arrows(&mut self) {
        let selected_ids: Vec<String> =
            self.selected.iter().filter_map(|i| self.scene.elements.get(*i)).map(|e| e.id.clone()).collect();
        for i in self.selected.clone() {
            if let Some(e) = self.scene.elements.get_mut(i) {
                for end in [&mut e.ends.start, &mut e.ends.end] {
                    if end.as_ref().is_some_and(|b| !selected_ids.contains(&b.element_id)) {
                        *end = None;
                    }
                }
            }
        }
    }

    /// Keeps every bound arrow glued to its shapes after anything moved.
    fn update_bindings(&mut self) {
        resolve_bindings(&mut self.scene.elements);
        recenter_labels(&mut self.scene.elements);
    }

    /// Shape that would be bound right now: highlighted like the reference app's suggestedBinding.
    fn suggested_binding(&self) -> Option<usize> {
        let pointer = self.to_world(self.last_pointer?);
        let dragged_end = match (self.point_drag, self.single_line()) {
            (Some(k), Some(i)) if matches!(self.scene.elements[i].kind, ElementKind::Line { arrow: true, .. })
                && (k == 0 || k + 1 == self.line_points(i).len()) => Some(i),
            _ => None,
        };
        let drawing = self.tool == Tool::Arrow && self.drawing_start.is_some();
        if let Some(i) = self.multi.or(dragged_end) {
            return self.binding_target(pointer, Some(i));
        }
        if drawing && self.canvas_rect.contains(self.last_pointer?) {
            return self.binding_target(pointer, None);
        }
        None
    }

    fn update_gesture(&mut self, pos: Pos2) {
        let previous = self.last_pointer.unwrap_or(pos);
        self.last_pointer = Some(pos);
        let world = self.to_world(pos);
        if let Some(i) = self.multi {
            let len = self.line_points(i).len();
            if len >= 2 {
                let snapped = self.snap_angle(self.line_points(i)[len - 2], world);
                if let Some(points) = self.line_points_mut(i) {
                    points[len - 1] = [snapped.x, snapped.y];
                }
            }
        }
        if self.panning {
            self.pan += pos - previous;
        } else if let (Some(k), Some(i)) = (self.point_drag, self.single_line()) {
            let points = self.line_points(i);
            let neighbor = if k > 0 { points.get(k - 1) } else { points.get(1) }.copied();
            let target = neighbor.map_or(world, |n| self.snap_angle(n, world));
            if let Some(p) = self.line_points_mut(i).and_then(|points| points.get_mut(k)) {
                *p = [target.x, target.y];
            }
        } else if let Some(rotate) = &self.rotating {
            let mut delta = (world - rotate.pivot).angle() - rotate.start;
            if self.mods.shift {
                let step = std::f32::consts::PI / 12.0;
                let base = match &rotate.originals[..] {
                    [one] => one.angle,
                    _ => 0.0,
                };
                delta = ((base + delta) / step).round() * step - base;
            }
            for (i, original) in self.selected.iter().zip(&rotate.originals) {
                let mut element = original.clone();
                element.rotate_around(rotate.pivot, delta);
                self.scene.elements[*i] = element;
            }
        } else if let Some(resize) = &self.resizing {
            let local = resize.center + rot(world - resize.center, -resize.angle);
            let corner = local + resize.grab;
            let mut to = Rect::from_two_pos(resize.anchor, corner);
            if self.mods.shift && resize.from.height() > 0.0 {
                // keep aspect ratio
                let ratio = resize.from.width() / resize.from.height();
                let w = to.width().max(to.height() * ratio);
                let size = Vec2::new(w, w / ratio);
                let dir = Vec2::new((corner.x - resize.anchor.x).signum(), (corner.y - resize.anchor.y).signum());
                to = Rect::from_two_pos(resize.anchor, resize.anchor + size * dir);
            }
            // the resized box lives in the rotated frame: put its center back in world space
            let shift = resize.center + rot(to.center() - resize.center, resize.angle) - to.center();
            for (i, original) in self.selected.iter().zip(&resize.originals) {
                let mut element = original.clone();
                element.scale(resize.from, to);
                element.move_by(shift);
                self.scene.elements[*i] = element;
            }
        } else if self.moving {
            if pos != previous && !self.move_snapshotted {
                self.snapshot();
                self.detach_selected_arrows();
                self.move_snapshotted = true;
                let moving = self.with_frame_contents(&self.selected);
                let originals = moving.iter().map(|i| self.scene.elements[*i].clone()).collect();
                self.move_origin = self.selection_bounds().map(|b| (moving, originals, b));
            }
            if let Some((moving, originals, bounds)) = self.move_origin.clone() {
                // absolute from the press point, so snapping never accumulates drift
                let mut delta = world - self.press_world;
                let (adjust, guides) = self.snap_box(bounds.translate(delta));
                delta += adjust;
                self.snap_guides = guides;
                for (i, original) in moving.into_iter().zip(originals) {
                    let mut moved = original;
                    moved.move_by(delta);
                    self.scene.elements[i] = moved;
                }
            }
        } else if self.drawing_start.is_some() && matches!(self.tool, Tool::Draw | Tool::AutoShape) {
            self.drawing_points.push([world.x, world.y]);
        } else if self.laser_down {
            self.laser.push((world, self.now));
        } else if let Some(lasso) = &mut self.lasso {
            lasso.push(world);
            let path = lasso.clone();
            self.select_in_lasso(&path);
        } else if self.erasing.is_some() {
            // sample the swept segment so fast strokes don't skip elements
            let from = self.to_world(previous);
            let steps = (from.distance(world) * self.zoom / 4.0).ceil().max(1.0) as usize;
            for step in 1..=steps {
                self.erase_at(from.lerp(world, step as f32 / steps as f32));
            }
        }
    }

    fn release(&mut self, pos: Pos2) {
        let end = self.to_world(pos);
        self.panning = false;
        if let (Some(start), Tool::Select) = (self.marquee, self.tool) {
            // a plain click on a locked element offers to unlock it (and its group / card)
            let tolerance = 10.0 / self.zoom;
            if start.distance(end) * self.zoom < 3.0 {
                let hit = |e: &Element| e.contains(end, tolerance) || (e.frame.is_some() && element_bounds(e).contains(end));
                if let Some(i) = self.scene.elements.iter().rposition(hit).filter(|i| self.scene.elements[*i].locked) {
                    // inside a locked card, the card and everything in it is the unit
                    let b = element_bounds(&self.scene.elements[i]);
                    let card = self.scene.elements.iter().position(|e| e.locked && e.frame.is_some() && element_bounds(e).expand(1.0).contains_rect(b));
                    let unit = self.lock_unit(&[card.unwrap_or(i)]);
                    self.active_locked = Some(unit.into_iter().map(|j| self.scene.elements[j].id.clone()).collect());
                }
            }
        }
        if let Some(start) = self.marquee.take() {
            let area = Rect::from_two_pos(start, end);
            let mut selected: Vec<usize> = self
                .scene
                .elements
                .iter()
                .enumerate()
                .filter(|(_, e)| !e.locked)
                .filter_map(|(i, e)| {
                    let b = element_bounds(e);
                    (if self.prefs.box_overlap { area.intersects(b) } else { area.contains_rect(b) }).then_some(i)
                })
                .collect();
            let groups: Vec<String> =
                selected.iter().filter_map(|i| self.scene.elements[*i].group_id.clone()).collect();
            for (i, e) in self.scene.elements.iter().enumerate() {
                if e.group_id.as_ref().is_some_and(|g| groups.contains(g)) && !selected.contains(&i) {
                    selected.push(i);
                }
            }
            for i in selected {
                if !self.selected.contains(&i) {
                    self.selected.push(i);
                }
            }
        }
        self.laser_down = false;
        if let Some(path) = self.lasso.take() {
            self.select_in_lasso(&path);
        }
        if let (Some(start), Tool::Sticky) = (self.drawing_start, self.tool) {
            self.drawing_start = None;
            self.place_sticky(start, end);
        }
        if let (Some(_), Tool::AutoShape) = (self.drawing_start, self.tool) {
            self.drawing_start = None;
            if let Some(element) = recognize_shape(self, &self.drawing_points.clone()) {
                self.snapshot();
                self.scene.elements.push(element);
                self.selected = vec![self.scene.elements.len() - 1];
            }
            self.drawing_points.clear();
        }
        if let Some(start) = self.drawing_start.take() {
            let click = start.distance(end) * self.zoom < 3.0;
            if click && matches!(self.tool, Tool::Line | Tool::Arrow) {
                // click-click mode: second point follows the cursor until the next click
                let mut element = self.temporary_element(start, start);
                if let ElementKind::Line { points, .. } = &mut element.kind {
                    points.truncate(1);
                    points.push([end.x, end.y]);
                }
                element.ends.start = self.bind_at(start, None);
                self.snapshot();
                self.scene.elements.push(element);
                self.multi = Some(self.scene.elements.len() - 1);
                self.selected = vec![self.scene.elements.len() - 1];
            } else if start.distance(end) > 1.0 / self.zoom {
                let element = self.bind_new_arrow(self.temporary_element(start, end));
                self.snapshot();
                self.scene.elements.push(element);
                self.selected = vec![self.scene.elements.len() - 1];
                if !self.tool_locked && self.tool != Tool::Draw {
                    self.tool = Tool::Select;
                }
            }
            self.drawing_points.clear();
        }
        if let Some(mut erased) = self.erasing.take() {
            if !erased.is_empty() {
                self.snapshot();
                erased.sort_unstable();
                for i in erased.into_iter().rev() {
                    self.scene.elements.remove(i);
                }
                self.selected.clear();
            }
        }
        self.moving = false;
        self.move_snapshotted = false;
        if let (Some(k), Some(i)) = (self.point_drag, self.single_line()) {
            let last = self.line_points(i).len() - 1;
            if matches!(self.scene.elements[i].kind, ElementKind::Line { arrow: true, .. }) && (k == 0 || k == last) {
                let p = self.line_points(i)[k];
                let binding = self.bind_at(Pos2::new(p[0], p[1]), Some(i));
                let ends = &mut self.scene.elements[i].ends;
                *(if k == 0 { &mut ends.start } else { &mut ends.end }) = binding;
            }
        }
        self.resizing = None;
        self.rotating = None;
        self.point_drag = None;
        self.move_origin = None;
        self.snap_guides.clear();
    }

    /// Shift keeps squares/circles and 15° angles, Alt draws from the center (as in the reference app).
    fn temporary_element(&self, start: Pos2, end: Pos2) -> Element {
        let end = if self.tool == Tool::Draw { end } else { self.snap_point(end) };
        let mut d = end - start;
        let boxed = matches!(self.tool, Tool::Rectangle | Tool::Ellipse | Tool::Diamond | Tool::Frame | Tool::Sticky);
        // sticky notes are square by default and Shift frees them, like images on the web
        if (self.mods.shift != (self.tool == Tool::Sticky)) && boxed {
            let side = d.x.abs().max(d.y.abs());
            d = Vec2::new(side * d.x.signum(), side * d.y.signum());
        }
        if self.mods.shift && matches!(self.tool, Tool::Arrow | Tool::Line) {
            let step = std::f32::consts::PI / 12.0;
            let angle = (d.y.atan2(d.x) / step).round() * step;
            d = Vec2::angled(angle) * d.length();
        }
        let (a, b) = if self.mods.alt && boxed { (start - d, start + d) } else { (start, start + d) };
        let r = Rect::from_two_pos(a, b);
        let (x, y, width, height) = (r.min.x, r.min.y, r.width().max(1.0), r.height().max(1.0));
        let kind = match self.tool {
            Tool::Draw | Tool::AutoShape => ElementKind::FreeDraw { points: self.drawing_points.clone() },
            Tool::Sticky => ElementKind::Rectangle { x, y, width, height },
            Tool::Rectangle | Tool::Frame => ElementKind::Rectangle { x, y, width, height },
            Tool::Ellipse => ElementKind::Ellipse { x, y, width, height },
            Tool::Diamond => ElementKind::Diamond { x, y, width, height },
            tool => ElementKind::Line { points: vec![[a.x, a.y], [b.x, b.y]], arrow: tool == Tool::Arrow },
        };
        let mut element = self.new_element(kind);
        if self.tool == Tool::Sticky {
            element.style = sticky_style(element.style);
            element.created = Some(now_ms());
        }
        if self.tool == Tool::Frame {
            // frames: thin gray border, no fill, crisp lines; the web names them all "Frame"
            element.frame = Some("Frame".into());
            element.style = Style { stroke: [0xbb, 0xbb, 0xbb, 255], fill: None, stroke_width: 1.0, roughness: 0, round: true, ..element.style };
        }
        element
    }

    fn update_cursor(&self, ctx: &Context, response: &egui::Response) {
        let Some(pos) = response.hover_pos() else {
            return;
        };
        if ctx.layer_id_at(pos) != Some(response.layer_id) {
            return;
        }
        let space = !ctx.egui_wants_keyboard_input() && ctx.input(|i| i.key_down(Key::Space));
        let icon = if self.panning {
            CursorIcon::Grabbing
        } else if space || self.tool == Tool::Hand || self.prefs.view_mode {
            CursorIcon::Grab
        } else {
            match self.tool {
                Tool::Select if self.hovered_rotation(pos) => CursorIcon::Grab,
                Tool::Select if self.point_handles().iter().chain(&self.midpoint_handles()).any(|(_, p)| p.distance(pos) <= 10.0) => {
                    CursorIcon::PointingHand
                }
                Tool::Select => match self.hovered_handle(pos) {
                    Some(0 | 2) => CursorIcon::ResizeNwSe,
                    Some(_) => CursorIcon::ResizeNeSw,
                    None if self.hit(self.to_world(pos)).is_some()
                        || self.selection_bounds().is_some_and(|b| b.contains(self.to_world(pos))) =>
                    {
                        CursorIcon::Move
                    }
                    None => CursorIcon::Default,
                },
                Tool::Text => CursorIcon::Text,
                _ => CursorIcon::Crosshair,
            }
        };
        ctx.set_cursor_icon(icon);
    }

    // ---------- painting ----------

    fn dark(&self) -> bool {
        match self.theme {
            Theme::Dark => true,
            Theme::Light => false,
            Theme::System => self.system_dark,
        }
    }

    fn paint_scene(&self, painter: &Painter, rect: Rect) {
        let dark = self.dark();
        painter.rect_filled(rect, 0.0, themed(rgba(self.scene.background), dark));
        if self.prefs.grid {
            // 20px grid, every 5th line stronger; minor lines vanish when zoomed far out
            let ink = if dark { Color32::WHITE } else { Color32::BLACK };
            let (minor, major) = (Stroke::new(1.0, ink.gamma_multiply(0.05)), Stroke::new(1.0, ink.gamma_multiply(0.1)));
            let (top_left, bottom_right) = (self.to_world(rect.min), self.to_world(rect.max));
            let step = if GRID * self.zoom < 6.0 { GRID * 5.0 } else { GRID };
            let mut x = (top_left.x / step).floor() * step;
            while x <= bottom_right.x {
                let sx = self.to_screen(Pos2::new(x, 0.0)).x;
                let s = if (x / GRID).round() as i64 % 5 == 0 { major } else { minor };
                painter.vline(sx, rect.y_range(), s);
                x += step;
            }
            let mut y = (top_left.y / step).floor() * step;
            while y <= bottom_right.y {
                let sy = self.to_screen(Pos2::new(0.0, y)).y;
                let s = if (y / GRID).round() as i64 % 5 == 0 { major } else { minor };
                painter.hline(rect.x_range(), sy, s);
                y += step;
            }
        }
        let view = View { origin: self.origin(), zoom: self.zoom, dark, background: themed(rgba(self.scene.background), dark) };
        let mut pen = None;
        for (i, element) in self.scene.elements.iter().enumerate() {
            match self.reveal.iter().position(|id| *id == element.id) {
                Some(0) => {
                    pen = Some(paint_partial(painter, element, &view, self.reveal_progress(element)));
                    continue;
                }
                Some(_) => continue, // still to be drawn
                None => {}
            }
            let faded = self.erasing.as_ref().is_some_and(|e| e.contains(&i));
            if is_arrow_label(&self.scene.elements, element) && self.editing != Some(i) {
                // the arrow is interrupted behind its label
                let b = local_bounds(element).expand(4.0);
                painter.rect_filled(Rect::from_two_pos(self.to_screen(b.min), self.to_screen(b.max)), 4, view.background);
            }
            let halo = text_needs_halo(&self.scene.elements, element);
            paint_element(painter, element, &view, faded, self.editing == Some(i), halo);
        }
        if let Some((name, last, at)) = &self.agent {
            let fade = if pen.is_some() { 1.0 } else { 1.0 - ((self.now - at) / AGENT_LINGER).clamp(0.0, 1.0) as f32 };
            if fade > 0.0 {
                paint_agent_cursor(painter, pen.unwrap_or(self.to_screen(*last)), name, fade);
            }
        }
        if let (Some(start), Some(end)) = (self.marquee, self.last_pointer) {
            painter.rect(
                Rect::from_two_pos(self.to_screen(start), end),
                0,
                PRIMARY.gamma_multiply(0.08),
                Stroke::new(1.0, PRIMARY),
                StrokeKind::Inside,
            );
        }
        if let Some(target) = self.suggested_binding().and_then(|i| self.scene.elements.get(i)) {
            // BINDING_HIGHLIGHT_RGB, width clamp(1.75, strokeWidth, 4)
            let outline = binding_outline(target, 0.0).iter().map(|p| self.to_screen(Pos2::new(p[0], p[1]))).collect();
            let width = target.style.stroke_width.clamp(1.75, 4.0);
            let color = if dark { Color32::from_rgb(104, 182, 240) } else { Color32::from_rgb(106, 189, 252) };
            painter.add(Shape::closed_line(outline, Stroke::new(width, color)));
        }
        if let (Some(i), Tool::Select) = (self.single_line(), self.tool) {
            // orbit bindings show their focus point, dashed from the endpoint (FOCUS_POINT_SIZE)
            let arrow = &self.scene.elements[i];
            let points = self.line_points(i);
            for (binding, k) in [(&arrow.ends.start, 0), (&arrow.ends.end, points.len() - 1)] {
                let Some(b) = binding.as_ref().filter(|b| !b.inside) else {
                    continue;
                };
                if let Some(target) = self.scene.elements.iter().find(|e| e.id == b.element_id) {
                    let focus = self.to_screen(focus_point(target, b));
                    let end = self.to_screen(Pos2::new(points[k][0], points[k][1]));
                    let faint = Stroke::new(1.0, PRIMARY.gamma_multiply(0.5));
                    painter.extend(Shape::dashed_line(&[end, focus], faint, 3.0, 3.0));
                    painter.circle_stroke(focus, 10.0 / 3.0, faint);
                }
            }
        }
        if let Some(path) = &self.lasso {
            // ponytail: no 5% fill inside the trail; egui only fills convex polygons
            painter.add(Shape::line(path.iter().map(|p| self.to_screen(*p)).collect(), Stroke::new(2.0, PRIMARY)));
        }
        // laser trail: red, fading out over one second
        for pair in self.laser.windows(2) {
            let age = (self.now - pair[1].1) as f32;
            if age < 1.0 {
                let color = Color32::from_rgb(0xff, 0x3b, 0x30).gamma_multiply(1.0 - age);
                painter.line_segment([self.to_screen(pair[0].0), self.to_screen(pair[1].0)], Stroke::new(4.0 * (1.0 - age * 0.5), color));
            }
        }
        for [a, b] in &self.snap_guides {
            // object snapping lines, red like the reference app's
            let red = Stroke::new(1.0, Color32::from_rgb(0xff, 0x6b, 0x6b));
            painter.line_segment([self.to_screen(*a), self.to_screen(*b)], red);
        }
        let accent = Stroke::new(1.0, PRIMARY);
        let handle_fill = if dark { Color32::from_rgb(18, 18, 18) } else { Color32::WHITE };
        let two_point_line = self.single_line().is_some_and(|i| self.line_points(i).len() == 2);
        if self.shows_transform_handles() || (self.tool != Tool::Select && self.multi.is_none() && self.editing.is_none() && !two_point_line) {
            if let Some(corners) = self.handle_corners() {
                painter.add(Shape::closed_line(corners.to_vec(), accent));
                if self.shows_transform_handles() {
                    for corner in corners {
                        painter.rect(Rect::from_center_size(corner, Vec2::splat(8.0)), 2, handle_fill, accent, StrokeKind::Middle);
                    }
                    if let Some(handle) = self.rotation_handle() {
                        painter.circle(handle, 4.5, handle_fill, accent);
                    }
                }
            }
        }
        // colors and radii from renderSingleLinearPoint (POINT_HANDLE_SIZE = 10)
        let editing_line = self.line_editor.is_some();
        let radius = if editing_line { 10.0 } else { 5.0 };
        let point_stroke = Stroke::new(1.0, themed(Color32::from_rgb(0x5e, 0x5a, 0xd8), dark));
        for (_, p) in self.midpoint_handles() {
            painter.circle_filled(p, 5.0, themed(Color32::from_rgba_unmultiplied(177, 151, 252, 178), dark));
        }
        for (k, p) in self.point_handles() {
            let selected = editing_line && self.selected_point == Some(k);
            let fill = if selected {
                Color32::from_rgba_unmultiplied(134, 131, 226, 230)
            } else {
                Color32::from_rgba_unmultiplied(255, 255, 255, 230)
            };
            painter.circle(p, radius, themed(fill, dark), point_stroke);
        }
        if let (Some(_), Tool::AutoShape) = (self.drawing_start, self.tool) {
            // faint trail of the stroke plus the shape it would become, at 70% opacity (lines
            // get no preview: nearly every stroke reads as a line at some early point)
            let width = (self.style.stroke_width * 0.65 * self.zoom).clamp(1.25, 4.0);
            let trail = if dark { Color32::from_white_alpha(51) } else { Color32::from_black_alpha(51) };
            painter.add(Shape::line(self.drawing_points.iter().map(|p| self.to_screen(Pos2::new(p[0], p[1]))).collect(), Stroke::new(width, trail)));
            if let Some(mut preview) = recognize_shape(self, &self.drawing_points).filter(|e| !matches!(e.kind, ElementKind::Line { arrow: false, .. })) {
                preview.style.opacity = 70;
                paint_element(painter, &preview, &view, false, false, false);
            }
        } else if let (Some(start), Some(pointer)) = (self.drawing_start, self.last_pointer) {
            let temporary = self.bind_new_arrow(self.temporary_element(start, self.to_world(pointer)));
            paint_element(painter, &temporary, &view, false, false, false);
        }
    }

    fn text_editor(&mut self, ctx: &Context) {
        let Some(index) = self.editing else {
            return;
        };
        let (pos, size, color, width) = match self.scene.elements.get(index) {
            Some(e @ Element { kind: ElementKind::Text { x, y, .. }, .. }) => (
                self.to_screen(Pos2::new(*x, *y)),
                e.style.font_size * self.zoom,
                themed(rgba(e.style.stroke), self.dark()),
                element_bounds(e).width() * self.zoom,
            ),
            _ => {
                self.editing = None;
                return;
            }
        };
        let mut finish = false;
        egui::Area::new(egui::Id::unique("text_editor"))
            .fixed_pos(pos)
            .order(egui::Order::Foreground)
            .show(ctx, |ui| {
                if let Some(Element { kind: ElementKind::Text { value, .. }, .. }) =
                    self.scene.elements.get_mut(index)
                {
                    ui.spacing_mut().item_spacing = Vec2::ZERO;
                    let font = FontId::new(size, egui::FontFamily::Name(HAND_FONT.into()));
                    // Text auto-resizes: grow sideways, never wrap
                    let mut layouter = |ui: &egui::Ui, text: &dyn egui::TextBuffer, _wrap: f32| {
                        let job = egui::text::LayoutJob::simple(text.as_str().to_owned(), font.clone(), color, f32::INFINITY);
                        ui.ctx().fonts_mut(|f| f.layout_job(job))
                    };
                    let response = ui.add(
                        egui::TextEdit::multiline(value)
                            .frame(egui::Frame::NONE)
                            .margin(Vec2::ZERO)
                            .text_color(color)
                            .background_color(Color32::TRANSPARENT)
                            .layouter(&mut layouter)
                            .clip_text(false)
                            .desired_rows(1)
                            .desired_width(width + size * 2.0),
                    );
                    if !self.edit_focused {
                        response.request_focus();
                        self.edit_focused = true;
                    }
                    finish = ui.input(|i| {
                        i.key_pressed(Key::Escape) || (i.key_pressed(Key::Enter) && i.modifiers.command)
                    });
                }
            });
        if finish {
            self.finish_text();
        }
    }

    // ---------- floating UI ----------

    fn colors(&self) -> UiColors {
        UiColors::new(self.dark())
    }

    fn island(&self) -> egui::Frame {
        let c = self.colors();
        egui::Frame::new()
            .fill(c.island)
            .corner_radius(8)
            .inner_margin(4)
            .stroke(Stroke::new(1.0, Color32::from_black_alpha(if self.dark() { 60 } else { 14 })))
            .shadow(egui::epaint::Shadow { offset: [0, 6], blur: 14, spread: 0, color: Color32::from_black_alpha(14) })
    }

    fn floating_controls(&mut self, ctx: &Context) {
        let c = self.colors();
        // main menu
        let menu_button = egui::Area::new(egui::Id::unique("main_menu"))
            .fixed_pos((16.0, 16.0))
            .show(ctx, |ui| {
                let response = square_button(ui, 36.0, self.menu_open, c.surface_low, &c);
                paint_icon(ui.painter(), Icon::Menu, response.rect.center(), c.text);
                response.on_hover_text("Menu")
            })
            .inner;
        if menu_button.clicked() {
            self.menu_open = !self.menu_open;
        }
        if self.menu_open {
            self.main_menu(ctx, menu_button.rect);
        }

        if self.prefs.view_mode || self.prefs.zen {
            // only a way back, like the reference app's exit buttons
            let (label, view) = if self.prefs.view_mode { ("Sair do modo de visualização", true) } else { ("Sair do modo zen", false) };
            egui::Area::new(egui::Id::unique("exit_mode"))
                .anchor(egui::Align2::CENTER_BOTTOM, (0.0, -16.0))
                .show(ctx, |ui| {
                    let button = egui::Button::new(egui::RichText::new(label).size(14.0).color(c.text)).fill(c.surface_low).corner_radius(8).min_size(Vec2::new(0.0, 36.0));
                    if ui.add(button).clicked() {
                        if view {
                            self.prefs.view_mode = false;
                        } else {
                            self.prefs.zen = false;
                        }
                    }
                });
        }
        if self.prefs.stats {
            self.stats_panel(ctx);
        }
        if self.prefs.view_mode {
            self.floating_footer_zoom_only(ctx);
            if self.palette_open {
                self.command_palette(ctx);
            }
            return;
        }

        // toolbar
        let mut extra_anchor: Option<Rect> = None;
        egui::Area::new(egui::Id::unique("tools"))
            .anchor(egui::Align2::CENTER_TOP, (0.0, 16.0))
            .show(ctx, |ui| {
                self.island().show(ui, |ui| {
                    ui.horizontal(|ui| {
                        ui.spacing_mut().item_spacing.x = 4.0;
                        let lock = square_button(ui, 36.0, self.tool_locked, Color32::TRANSPARENT, &c);
                        paint_icon(ui.painter(), Icon::Lock(self.tool_locked), lock.rect.center(), c.text);
                        if lock.on_hover_text("Manter ativa a ferramenta selecionada após desenhar — Q").clicked() {
                            self.tool_locked = !self.tool_locked;
                        }
                        separator(ui, &c);
                        for tool in Tool::TOOLBAR {
                            if tool == Tool::Eraser {
                                ui.add_space(0.0);
                            }
                            let r = square_button(ui, 36.0, self.tool == tool, Color32::TRANSPARENT, &c);
                            let fg = if self.tool == tool && !self.dark() { Color32::from_rgb(0x03, 0x03, 0x5c) } else { c.text };
                            paint_icon(ui.painter(), Icon::Tool(tool), r.rect.center(), fg);
                            ui.painter().text(
                                r.rect.right_bottom() - Vec2::new(4.0, 3.0),
                                egui::Align2::RIGHT_BOTTOM,
                                tool.badge(),
                                FontId::proportional(9.0),
                                c.muted,
                            );
                            let shortcut = tool.keys().iter().map(|k| k.name()).collect::<Vec<_>>().join(" ou ");
                            if r.on_hover_text(format!("{} — {shortcut}", tool.label())).clicked() {
                                self.select_tool(tool);
                            }
                        }
                        separator(ui, &c);
                        let more = square_button(ui, 36.0, Tool::EXTRA.contains(&self.tool) || self.extra_open, Color32::TRANSPARENT, &c);
                        let icon = if Tool::EXTRA.contains(&self.tool) { Icon::Tool(self.tool) } else { Icon::More };
                        paint_icon(ui.painter(), icon, more.rect.center(), c.text);
                        extra_anchor = Some(more.rect);
                        if more.on_hover_text("Mais ferramentas").clicked() {
                            self.extra_open = !self.extra_open;
                        }
                    });
                });
            });
        if let (true, Some(anchor)) = (self.extra_open, extra_anchor) {
            let menu = egui::Area::new(egui::Id::unique("extra_tools"))
                .anchor(egui::Align2::RIGHT_TOP, (anchor.right() - ctx.content_rect().right(), anchor.bottom() + 8.0))
                .order(egui::Order::Foreground)
                .show(ctx, |ui| {
                    self.island().inner_margin(8).show(ui, |ui| {
                        ui.set_width(260.0);
                        ui.spacing_mut().item_spacing.y = 1.0;
                        if menu_item(ui, Icon::Image, "Inserir imagem", "9", c.text, &c) {
                            self.extra_open = false;
                            self.insert_image();
                        }
                        for tool in Tool::EXTRA {
                            if menu_item(ui, Icon::Tool(tool), tool.label(), tool.badge(), if self.tool == tool { PRIMARY } else { c.text }, &c) {
                                self.select_tool(tool);
                                self.extra_open = false;
                            }
                        }
                    });
                });
            let outside = ctx.input(|i| i.pointer.any_pressed())
                && ctx.input(|i| i.pointer.interact_pos()).is_some_and(|p| !menu.response.rect.contains(p) && !anchor.contains(p));
            if outside {
                self.extra_open = false;
            }
        }

        // hint under toolbar
        if let Some(hint) = self.hint() {
            egui::Area::new(egui::Id::unique("hint"))
                .anchor(egui::Align2::CENTER_TOP, (0.0, 70.0))
                .interactable(false)
                .show(ctx, |ui| {
                    ui.add(egui::Label::new(egui::RichText::new(hint).size(12.0).color(c.muted)).wrap_mode(egui::TextWrapMode::Extend));
                });
        }

        if self.shows_properties() && !self.prefs.zen {
            self.properties_panel(ctx);
        }
        self.unlock_popup(ctx);

        // footer: zoom + undo/redo
        egui::Area::new(egui::Id::unique("footer"))
            .anchor(egui::Align2::LEFT_BOTTOM, (16.0, -16.0))
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    ui.spacing_mut().item_spacing.x = 8.0;
                    group_frame(&c).show(ui, |ui| {
                        ui.spacing_mut().item_spacing.x = 0.0;
                        let minus = square_button(ui, 36.0, false, Color32::TRANSPARENT, &c);
                        paint_icon(ui.painter(), Icon::Minus, minus.rect.center(), c.text);
                        if minus.on_hover_text("Diminuir zoom — Cmd+-").clicked() {
                            self.zoom_step(-0.1);
                        }
                        let (rect, reset) = ui.allocate_exact_size(Vec2::new(58.0, 36.0), Sense::click());
                        let reset = reset.on_hover_cursor(CursorIcon::PointingHand);
                        if reset.hovered() {
                            ui.painter().rect_filled(rect, 8, c.surface_high);
                        }
                        ui.painter().text(
                            rect.center(),
                            egui::Align2::CENTER_CENTER,
                            format!("{}%", (self.zoom * 100.0).round()),
                            FontId::proportional(13.0),
                            c.text,
                        );
                        if reset.on_hover_text("Redefinir zoom — Cmd+0").clicked() {
                            let center = self.canvas_rect.center();
                            self.zoom_at(1.0, center);
                        }
                        let plus = square_button(ui, 36.0, false, Color32::TRANSPARENT, &c);
                        paint_icon(ui.painter(), Icon::Plus, plus.rect.center(), c.text);
                        if plus.on_hover_text("Aumentar zoom — Cmd++").clicked() {
                            self.zoom_step(0.1);
                        }
                    });
                    group_frame(&c).show(ui, |ui| {
                        ui.spacing_mut().item_spacing.x = 0.0;
                        for (icon, enabled, tip) in [
                            (Icon::Undo, !self.undo.is_empty(), "Desfazer — Cmd+Z"),
                            (Icon::Redo, !self.redo.is_empty(), "Refazer — Cmd+Shift+Z"),
                        ] {
                            let r = square_button(ui, 36.0, false, Color32::TRANSPARENT, &c);
                            paint_icon(ui.painter(), icon, r.rect.center(), if enabled { c.text } else { c.muted });
                            if r.on_hover_text(tip).clicked() && enabled {
                                if matches!(icon, Icon::Undo) {
                                    self.undo();
                                } else {
                                    self.redo();
                                }
                            }
                        }
                    });
                });
            });

        // minimap + its toggle, bottom right
        // separate fixed-size areas: an anchored area that changes size jumps for a frame
        if self.minimap_open && !self.prefs.zen {
            egui::Area::new(egui::Id::unique("minimap"))
                .anchor(egui::Align2::RIGHT_BOTTOM, (-16.0, -16.0 - 36.0 - 8.0))
                .show(ctx, |ui| {
                    self.island().inner_margin(6).show(ui, |ui| self.minimap(ui));
                });
        }
        egui::Area::new(egui::Id::unique("minimap_toggle"))
            .anchor(egui::Align2::RIGHT_BOTTOM, (-16.0, -16.0))
            .enabled(!self.prefs.zen)
            .show(ctx, |ui| {
                group_frame(&c).show(ui, |ui| {
                    let r = square_button(ui, 36.0, self.minimap_open, Color32::TRANSPARENT, &c);
                    paint_icon(ui.painter(), Icon::Map, r.rect.center(), c.text);
                    let tip = if self.minimap_open { "Ocultar minimapa" } else { "Mostrar minimapa" };
                    if r.on_hover_text(tip).clicked() {
                        self.minimap_open = !self.minimap_open;
                        ctx.request_repaint();
                    }
                });
            });

        if self.export_open {
            self.export_dialog(ctx);
        }
        if self.palette_open {
            self.command_palette(ctx);
        }
        let now = ctx.input(|i| i.time);
        if self.status_time < 0.0 {
            self.status_time = now;
        }
        if !self.status.is_empty() && now - self.status_time < 3.0 {
            egui::Area::new(egui::Id::unique("toast"))
                .anchor(egui::Align2::CENTER_BOTTOM, (0.0, -24.0))
                .interactable(false)
                .show(ctx, |ui| {
                    self.island().inner_margin(egui::Margin::symmetric(14, 8)).show(ui, |ui| {
                        ui.label(egui::RichText::new(&self.status).color(c.text));
                    });
                });
            ctx.request_repaint_after(std::time::Duration::from_millis(250));
        }
    }

    /// View mode keeps the menu, zoom and exit button only.
    fn floating_footer_zoom_only(&mut self, ctx: &Context) {
        let c = self.colors();
        egui::Area::new(egui::Id::unique("footer"))
            .anchor(egui::Align2::LEFT_BOTTOM, (16.0, -16.0))
            .show(ctx, |ui| {
                group_frame(&c).show(ui, |ui| {
                    ui.horizontal(|ui| {
                        ui.spacing_mut().item_spacing.x = 0.0;
                        let minus = square_button(ui, 36.0, false, Color32::TRANSPARENT, &c);
                        paint_icon(ui.painter(), Icon::Minus, minus.rect.center(), c.text);
                        if minus.clicked() {
                            self.zoom_step(-0.1);
                        }
                        let (rect, reset) = ui.allocate_exact_size(Vec2::new(58.0, 36.0), Sense::click());
                        ui.painter().text(rect.center(), egui::Align2::CENTER_CENTER, format!("{}%", (self.zoom * 100.0).round()), FontId::proportional(13.0), c.text);
                        if reset.on_hover_cursor(CursorIcon::PointingHand).clicked() {
                            let center = self.canvas_rect.center();
                            self.zoom_at(1.0, center);
                        }
                        let plus = square_button(ui, 36.0, false, Color32::TRANSPARENT, &c);
                        paint_icon(ui.painter(), Icon::Plus, plus.rect.center(), c.text);
                        if plus.clicked() {
                            self.zoom_step(0.1);
                        }
                    });
                });
            });
    }

    /// "Propriedades da Tela e da Forma" (Option+/): scene totals, and editable x/y/size/angle
    /// for a single selected shape.
    fn stats_panel(&mut self, ctx: &Context) {
        let c = self.colors();
        egui::Area::new(egui::Id::unique("stats"))
            .anchor(egui::Align2::RIGHT_TOP, (-16.0, 16.0))
            .show(ctx, |ui| {
                self.island().inner_margin(12).show(ui, |ui| {
                    ui.set_width(200.0);
                    ui.horizontal(|ui| {
                        ui.label(egui::RichText::new("Propriedades").size(16.0).strong().color(c.text));
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            if ui.add(egui::Button::new("×").frame(false)).on_hover_text("Fechar — Option+/").clicked() {
                                self.prefs.stats = false;
                            }
                        });
                    });
                    ui.add_space(6.0);
                    let row = |ui: &mut egui::Ui, label: &str, value: String| {
                        ui.horizontal(|ui| {
                            ui.label(egui::RichText::new(label).size(13.0).color(c.muted));
                            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                ui.label(egui::RichText::new(value).size(13.0).color(c.text));
                            });
                        });
                    };
                    ui.label(egui::RichText::new("Geral").size(13.0).strong().color(c.text));
                    let all = self.scene.elements.iter().map(element_bounds).reduce(|a, b| a.union(b));
                    row(ui, "Formas", self.scene.elements.len().to_string());
                    if let Some(b) = all {
                        row(ui, "Largura", format!("{:.0}", b.width()));
                        row(ui, "Altura", format!("{:.0}", b.height()));
                    }
                    if self.selected.is_empty() {
                        return;
                    }
                    ui.add_space(8.0);
                    ui.label(egui::RichText::new("Propriedades da forma").size(13.0).strong().color(c.text));
                    if let [i] = self.selected[..] {
                        let e = self.scene.elements[i].clone();
                        let b = if e.rotatable() { local_bounds(&e) } else { element_bounds(&e) };
                        let (mut x, mut y, mut w, mut h) = (b.min.x, b.min.y, b.width(), b.height());
                        let mut degrees = e.angle.to_degrees();
                        let mut changed = false;
                        egui::Grid::new("stats_fields").num_columns(4).spacing([6.0, 6.0]).show(ui, |ui| {
                            for (label, value) in [("X", &mut x), ("Y", &mut y)] {
                                ui.label(egui::RichText::new(label).size(13.0).color(c.muted));
                                changed |= ui.add(egui::DragValue::new(value).speed(1.0).max_decimals(0)).changed();
                            }
                            ui.end_row();
                            for (label, value) in [("L", &mut w), ("A", &mut h)] {
                                ui.label(egui::RichText::new(label).size(13.0).color(c.muted));
                                changed |= ui.add(egui::DragValue::new(value).speed(1.0).range(1.0..=100_000.0).max_decimals(0)).changed();
                            }
                            ui.end_row();
                            if e.rotatable() {
                                ui.label(egui::RichText::new("Ângulo").size(13.0).color(c.muted));
                                changed |= ui.add(egui::DragValue::new(&mut degrees).speed(1.0).range(0.0..=360.0).suffix("°").max_decimals(0)).changed();
                                ui.end_row();
                            }
                        });
                        if changed {
                            if !self.style_edit_active {
                                self.snapshot();
                                self.style_edit_active = true;
                            }
                            let mut edited = e.clone();
                            edited.scale(b, Rect::from_min_size(Pos2::new(x, y), Vec2::new(w, h)));
                            if edited.rotatable() {
                                edited.angle = degrees.to_radians().rem_euclid(std::f32::consts::TAU);
                            }
                            self.scene.elements[i] = edited;
                            self.dirty = true;
                        }
                    } else if let Some(b) = self.selection_bounds() {
                        row(ui, "Selecionado", self.selected.len().to_string());
                        row(ui, "Largura", format!("{:.0}", b.width()));
                        row(ui, "Altura", format!("{:.0}", b.height()));
                    }
                });
            });
    }

    fn hint(&self) -> Option<&'static str> {
        if !self.prefs.show_hints || self.prefs.view_mode || self.prefs.zen {
            return None;
        }
        if self.editing.is_some() {
            return Some("Pressione Esc ou Cmd+Enter para salvar as edições");
        }
        if self.multi.is_some() {
            return Some("Clique no último ponto ou pressione Esc ou Enter para finalizar");
        }
        if self.rotating.is_some() {
            return Some("Você pode restringir ângulos segurando Shift enquanto rotaciona");
        }
        if self.line_editor.is_some() && self.selected_point.is_some() {
            return Some("Pressione Delete para remover o ponto, ou arraste para mover");
        }
        if self.line_editor.is_some() {
            return Some("Selecione um ponto para editar, ou segure Option e clique para adicionar novos pontos");
        }
        if let (Some(i), Tool::Select) = (self.single_line(), self.tool) {
            return Some(if matches!(self.scene.elements[i].kind, ElementKind::Line { arrow: true, .. }) {
                "Segure Cmd e clique duas vezes ou pressione Cmd+Enter para editar pontos"
            } else {
                "Clique duas vezes ou pressione Cmd+Enter para editar pontos"
            });
        }
        match self.tool {
            Tool::Hand => None,
            Tool::Select if self.scene.elements.is_empty() => {
                Some("Para mover a tela, segure Roda do mouse ou Barra de espaço enquanto arrasta ou use a ferramenta de mão")
            }
            Tool::Select => match self.selected[..] {
                [i] if matches!(self.scene.elements.get(i), Some(Element { kind: ElementKind::Text { .. }, .. })) => {
                    Some("Clique duas vezes ou pressione Enter para editar texto")
                }
                [_] => Some("Você pode restringir proporções segurando Shift enquanto redimensiona"),
                _ => None,
            },
            Tool::Arrow => Some("Clique para colocar vários pontos, arraste para uma linha única"),
            Tool::Line => Some("Clique para iniciar vários pontos, arraste para uma única linha"),
            Tool::Draw => Some("Toque e arraste, solte quando terminar"),
            Tool::AutoShape => Some("Solte para converter em forma"),
            Tool::Sticky => Some("Clique para inserir uma nota ou arraste para definir o tamanho"),
            Tool::Text => Some("Dica: você também pode adicionar texto clicando duas vezes em qualquer lugar com a ferramenta de seleção"),
            Tool::Eraser => None,
            _ => Some("Segure Shift para proporções iguais e Alt para desenhar a partir do centro"),
        }
    }

    fn main_menu(&mut self, ctx: &Context, button: Rect) {
        let c = self.colors();
        let mut prefs_anchor: Option<Rect> = None;
        let mut locked_anchor: Option<Rect> = None;
        // MainMenu.tsx items; .dropdown-menu-container: 8px padding, 32px items, 1px gap
        let area = egui::Area::new(egui::Id::unique("main_menu_popup"))
            .fixed_pos(button.left_bottom() + Vec2::new(0.0, 8.0))
            .order(egui::Order::Foreground)
            .show(ctx, |ui| {
                self.island().inner_margin(8).show(ui, |ui| {
                    ui.set_width(237.0);
                    ui.spacing_mut().item_spacing.y = 1.0;
                    if menu_item(ui, Icon::Open, "Abrir", "Cmd+O", c.text, &c) {
                        self.menu_open = false;
                        self.open();
                    }
                    if menu_item(ui, Icon::Download, "Salvar como...", "", c.text, &c) {
                        self.menu_open = false;
                        self.save_as();
                    }
                    if menu_item(ui, Icon::Export, "Exportar imagem...", "Cmd+Shift+E", c.text, &c) {
                        self.menu_open = false;
                        self.export_open = true;
                    }
                    if menu_item(ui, Icon::Palette, "Paleta de comandos", "Cmd+/", PRIMARY, &c) {
                        self.menu_open = false;
                        self.open_palette();
                    }
                    if menu_item(ui, Icon::Trash, "Limpar a tela", "", c.text, &c) {
                        self.menu_open = false;
                        self.clear_canvas();
                    }
                    let (line, _) = ui.allocate_exact_size(Vec2::new(ui.available_width(), 9.0), Sense::hover());
                    ui.painter().hline(line.x_range(), line.center().y, Stroke::new(1.0, c.surface_low));
                    let locked_count = self.scene.elements.iter().filter(|e| e.locked).count();
                    let before = ui.cursor().min;
                    let locked_label = format!("Elementos bloqueados ({locked_count})");
                    if menu_item(ui, Icon::Lock(true), &locked_label, "›", c.text, &c) {
                        self.locked_open = !self.locked_open;
                    }
                    let row = Rect::from_min_size(before, Vec2::new(ui.available_width(), 32.0));
                    if ui.rect_contains_pointer(row) {
                        self.locked_open = true;
                        self.prefs_open = false;
                    }
                    locked_anchor = Some(row);
                    let before = ui.cursor().min;
                    if menu_item(ui, Icon::Sliders, "Preferências", "›", c.text, &c) {
                        self.prefs_open = !self.prefs_open;
                    }
                    let row = Rect::from_min_size(before, Vec2::new(ui.available_width(), 32.0));
                    if ui.rect_contains_pointer(row) {
                        self.prefs_open = true;
                        self.locked_open = false;
                    }
                    prefs_anchor = Some(row);
                    ui.horizontal(|ui| {
                        ui.set_height(32.0);
                        ui.add_space(8.0);
                        ui.label(egui::RichText::new("Tema").size(14.0).color(c.text));
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            group_frame(&c).inner_margin(2).show(ui, |ui| {
                                ui.spacing_mut().item_spacing.x = 2.0;
                                // right-to-left: shows light, dark, system
                                for (theme, icon, tip) in [
                                    (Theme::System, Icon::Monitor, "Sistema"),
                                    (Theme::Dark, Icon::Moon, "Escuro"),
                                    (Theme::Light, Icon::Sun, "Claro"),
                                ] {
                                    let on = self.theme == theme;
                                    let r = square_button(ui, 26.0, false, Color32::TRANSPARENT, &c);
                                    if on {
                                        ui.painter().rect_filled(r.rect, 6, PRIMARY);
                                    }
                                    paint_icon(ui.painter(), icon, r.rect.center(), if on { Color32::WHITE } else { c.text });
                                    if r.on_hover_text(tip).clicked() {
                                        self.theme = theme;
                                    }
                                }
                            });
                        });
                    });
                    ui.add_space(6.0);
                    ui.horizontal(|ui| {
                        ui.add_space(8.0);
                        ui.label(egui::RichText::new("Fundo da tela").size(14.0).color(c.text));
                    });
                    ui.add_space(4.0);
                    ui.horizontal(|ui| {
                        ui.add_space(8.0);
                        ui.spacing_mut().item_spacing.x = 8.0;
                        for color in CANVAS_PICKS {
                            if swatch(ui, color, self.scene.background == color, 28.0, self.dark()).clicked() {
                                self.snapshot();
                                self.scene.background = color;
                            }
                        }
                    });
                    ui.add_space(4.0);
                });
            });
        let mut submenu = Rect::NOTHING;
        if let (true, Some(row)) = (self.prefs_open, prefs_anchor) {
            submenu = self.preferences_menu(ctx, Pos2::new(area.response.rect.right() + 8.0, row.top() - 8.0));
        }
        if let (true, Some(row)) = (self.locked_open, locked_anchor) {
            submenu = self.locked_menu(ctx, Pos2::new(area.response.rect.right() + 8.0, row.top() - 8.0));
        }
        let clicked_outside = ctx.input(|i| i.pointer.any_pressed())
            && ctx
                .input(|i| i.pointer.interact_pos())
                .is_some_and(|p| !area.response.rect.contains(p) && !button.contains(p) && !submenu.contains(p));
        if clicked_outside {
            self.menu_open = false;
            self.prefs_open = false;
            self.locked_open = false;
        }
    }

    /// Scaled-down drawing with the visible area; click or drag to move there.
    fn minimap(&mut self, ui: &mut egui::Ui) {
        let c = self.colors();
        let (rect, response) = ui.allocate_exact_size(Vec2::new(200.0, 130.0), Sense::click_and_drag());
        let world = self.minimap_world();
        let scale = (rect.width() / world.width()).min(rect.height() / world.height());
        let to_map = |p: Pos2| rect.center() + (p - world.center()) * scale;
        let p = ui.painter_at(rect);
        p.rect_filled(rect, 6, themed(rgba(self.scene.background), self.dark()));
        let ink = c.text.gamma_multiply(0.55);
        for e in &self.scene.elements {
            let pts: Vec<[f32; 2]> = match &e.kind {
                ElementKind::Line { .. } => line_path(e),
                ElementKind::FreeDraw { points } => points.iter().step_by(4).copied().collect(),
                _ => binding_outline(e, 0.0),
            };
            let mapped: Vec<Pos2> = pts.iter().map(|q| to_map(Pos2::new(q[0], q[1]))).collect();
            match &e.kind {
                ElementKind::Text { .. } => {
                    // text reads as a thin bar at this size
                    let b = Rect::from_two_pos(to_map(element_bounds(e).min), to_map(element_bounds(e).max));
                    p.rect_filled(b.shrink2(Vec2::new(0.0, b.height() * 0.3)), 1, ink);
                }
                ElementKind::Line { .. } | ElementKind::FreeDraw { .. } => {
                    p.add(Shape::line(mapped, Stroke::new(1.0, ink)));
                }
                _ => {
                    if let Some(fill) = e.style.fill {
                        p.add(Shape::convex_polygon(mapped.clone(), themed(rgba(fill), self.dark()).gamma_multiply(0.7), Stroke::NONE));
                    }
                    p.add(Shape::closed_line(mapped, Stroke::new(1.0, ink)));
                }
            }
        }
        let view = Rect::from_two_pos(
            to_map(self.to_world(self.canvas_rect.min)),
            to_map(self.to_world(self.canvas_rect.max)),
        );
        p.rect(view, 2, PRIMARY.gamma_multiply(0.12), Stroke::new(1.5, PRIMARY), StrokeKind::Middle);
        if let Some(pos) = response.interact_pointer_pos().filter(|_| response.clicked() || response.dragged()) {
            self.pan_to(world.center() + (pos - rect.center()) / scale);
        }
        response.on_hover_cursor(CursorIcon::PointingHand).on_hover_text("Clique ou arraste para navegar");
    }

    /// Every locked element, with "show" (click the name) and "unlock" per row.
    fn locked_menu(&mut self, ctx: &Context, at: Pos2) -> Rect {
        let c = self.colors();
        let locked: Vec<(String, String, Rect)> = self
            .scene
            .elements
            .iter()
            .filter(|e| e.locked)
            .map(|e| (e.id.clone(), self.describe(e), element_bounds(e)))
            .collect();
        let mut unlock: Option<Option<String>> = None;
        let mut show: Option<Rect> = None;
        let mut lock_everything = false;
        let rect = egui::Area::new(egui::Id::unique("locked_menu"))
            .fixed_pos(at)
            .order(egui::Order::Foreground)
            .show(ctx, |ui| {
                self.island().inner_margin(8).show(ui, |ui| {
                    ui.set_width(300.0);
                    ui.spacing_mut().item_spacing.y = 1.0;
                    if locked.is_empty() {
                        ui.add_space(6.0);
                        ui.horizontal(|ui| {
                            ui.add_space(8.0);
                            ui.label(egui::RichText::new("Nenhum elemento bloqueado").size(14.0).color(c.muted));
                        });
                        ui.add_space(6.0);
                    }
                    egui::ScrollArea::vertical().max_height(320.0).show(ui, |ui| {
                        for (id, name, bounds) in &locked {
                            let (row, response) = ui.allocate_exact_size(Vec2::new(ui.available_width(), 32.0), Sense::click());
                            let response = response.on_hover_cursor(CursorIcon::PointingHand).on_hover_text("Mostrar na tela");
                            let p = ui.painter();
                            if response.hovered() {
                                p.rect_filled(row, 6, c.surface_high);
                            }
                            paint_icon(p, Icon::Lock(true), Pos2::new(row.left() + 16.0, row.center().y), c.text);
                            p.text(Pos2::new(row.left() + 34.0, row.center().y), egui::Align2::LEFT_CENTER, name, FontId::proportional(14.0), c.text);
                            let button = Rect::from_center_size(Pos2::new(row.right() - 18.0, row.center().y), Vec2::splat(28.0));
                            let open = ui.interact(button, ui.scope_id().with(("unlock", id)), Sense::click()).on_hover_cursor(CursorIcon::PointingHand);
                            ui.painter().rect_filled(button, 6, if open.hovered() { c.selected } else { c.surface_low });
                            paint_icon(ui.painter(), Icon::Lock(false), button.center(), c.text);
                            if open.on_hover_text("Desbloquear").clicked() {
                                unlock = Some(Some(id.clone()));
                            } else if response.clicked() {
                                show = Some(*bounds);
                            }
                        }
                    });
                    let (line, _) = ui.allocate_exact_size(Vec2::new(ui.available_width(), 9.0), Sense::hover());
                    ui.painter().hline(line.x_range(), line.center().y, Stroke::new(1.0, c.surface_low));
                    if !locked.is_empty() && menu_item(ui, Icon::Lock(false), "Desbloquear tudo", "", c.text, &c) {
                        unlock = Some(None);
                    }
                    if self.scene.elements.iter().any(|e| !e.locked) && menu_item(ui, Icon::Lock(true), "Bloquear tudo", "", c.text, &c) {
                        lock_everything = true;
                    }
                });
            })
            .response
            .rect;
        if let Some(target) = unlock {
            self.unlock(target.as_deref());
        }
        if lock_everything {
            self.lock_all();
        }
        if let Some(bounds) = show {
            self.pan_to(bounds.center());
        }
        rect
    }

    /// The Preferências submenu: ✓ toggles and two segmented choices, as in the web app.
    fn preferences_menu(&mut self, ctx: &Context, at: Pos2) -> Rect {
        let c = self.colors();
        egui::Area::new(egui::Id::unique("preferences_menu"))
            .fixed_pos(at)
            .order(egui::Order::Foreground)
            .show(ctx, |ui| {
                self.island().inner_margin(8).show(ui, |ui| {
                    ui.set_width(300.0);
                    ui.spacing_mut().item_spacing.y = 1.0;
                    let choice = |ui: &mut egui::Ui, label: &str, options: [&str; 2], second: &mut bool| {
                        let (rect, _) = ui.allocate_exact_size(Vec2::new(ui.available_width(), 32.0), Sense::hover());
                        // segmented control on the right, inside a rounded frame like the web one
                        let font = FontId::proportional(13.0);
                        let widths: Vec<f32> = options
                            .iter()
                            .map(|o| ui.painter().layout_no_wrap(o.to_string(), font.clone(), c.text).size().x + 16.0)
                            .collect();
                        let frame_right = rect.right() - 8.0;
                        let frame_left = frame_right - widths.iter().sum::<f32>() - 2.0 * 2.0 - 2.0;
                        let frame = Rect::from_min_max(Pos2::new(frame_left, rect.top() + 2.0), Pos2::new(frame_right, rect.bottom() - 2.0));
                        ui.painter().rect(frame, 8, Color32::TRANSPARENT, Stroke::new(1.0, c.surface_low), StrokeKind::Inside);
                        let mut x = frame.left() + 2.0;
                        let mut clicked = None;
                        for (k, option) in options.iter().enumerate() {
                            let pill = Rect::from_min_size(Pos2::new(x, frame.top() + 2.0), Vec2::new(widths[k], frame.height() - 4.0));
                            let on = (k == 1) == *second;
                            let response = ui.interact(pill, ui.scope_id().with((label, k)), Sense::click()).on_hover_cursor(CursorIcon::PointingHand);
                            ui.painter().rect_filled(pill, 6, if on { PRIMARY } else if response.hovered() { c.surface_high } else { Color32::TRANSPARENT });
                            ui.painter().text(pill.center(), egui::Align2::CENTER_CENTER, *option, font.clone(), if on { Color32::WHITE } else { PRIMARY });
                            if response.clicked() {
                                clicked = Some(k == 1);
                            }
                            x = pill.right() + 2.0;
                        }
                        // the label gets what is left and ends in "…" when it doesn't fit
                        let room = frame.left() - 12.0 - (rect.left() + 34.0);
                        let label_font = FontId::proportional(14.0);
                        let fits = |t: &str| ui.painter().layout_no_wrap(t.to_string(), label_font.clone(), c.text).size().x <= room;
                        let mut shown = label.to_string();
                        if !fits(&shown) {
                            let chars: Vec<char> = label.chars().collect();
                            let mut n = chars.len();
                            while n > 1 && !fits(&format!("{}…", chars[..n].iter().collect::<String>().trim_end())) {
                                n -= 1;
                            }
                            shown = format!("{}…", chars[..n].iter().collect::<String>().trim_end());
                        }
                        let label_response = ui.interact(
                            Rect::from_min_max(Pos2::new(rect.left() + 34.0, rect.top()), Pos2::new(frame.left() - 12.0, rect.bottom())),
                            ui.scope_id().with((label, "label")),
                            Sense::hover(),
                        );
                        if shown != label {
                            label_response.on_hover_text(label);
                        }
                        ui.painter().text(Pos2::new(rect.left() + 34.0, rect.center().y), egui::Align2::LEFT_CENTER, shown, label_font, c.text);
                        if let Some(v) = clicked {
                            *second = v;
                        }
                    };
                    choice(ui, "Modo de seleção por caixa", ["Contido", "Sobreposto"], &mut self.prefs.box_overlap);
                    choice(ui, "Dispositivo de entrada", ["Trackpad", "Mouse"], &mut self.prefs.mouse_wheel_zooms);
                    let check = |ui: &mut egui::Ui, label: &str, shortcut: &str, value: &mut bool| {
                        let icon = if *value { Icon::Check } else { Icon::Blank };
                        if menu_item(ui, icon, label, shortcut, c.text, &c) {
                            *value = !*value;
                        }
                    };
                    check(ui, "Bloqueio de ferramenta", "Q", &mut self.tool_locked);
                    check(ui, "Encaixar em objetos", "Option+S", &mut self.prefs.snap_objects);
                    check(ui, "Alternar grade", "Cmd+'", &mut self.prefs.grid);
                    check(ui, "Modo Zen", "Option+Z", &mut self.prefs.zen);
                    check(ui, "Modo de visualização", "Option+R", &mut self.prefs.view_mode);
                    check(ui, "Propriedades da Tela e da Forma", "Option+/", &mut self.prefs.stats);
                    check(ui, "Atalho de flecha", "", &mut self.prefs.arrow_binding);
                    check(ui, "Encaixar em pontos intermediários", "", &mut self.prefs.midpoint_snapping);
                    check(ui, "Mostrar dicas", "", &mut self.prefs.show_hints);
                });
            })
            .response
            .rect
    }

    fn open_palette(&mut self) {
        self.menu_open = false;
        self.palette_open = true;
        self.palette_query.clear();
        self.palette_index = 0;
        self.palette_focused = false;
    }

    /// Commands matching the search, grouped like CommandPalette.tsx.
    fn palette_items(&self) -> Vec<Command> {
        let query = fold(&self.palette_query);
        let words: Vec<&str> = query.split_whitespace().collect();
        Command::ALL
            .iter()
            .copied()
            .filter(|cmd| cmd.group() != "Elementos" || !self.selected.is_empty())
            .filter(|cmd| {
                let label = fold(cmd.label());
                words.iter().all(|w| label.contains(w))
            })
            .collect()
    }

    fn run_command(&mut self, cmd: Command) {
        match cmd {
            Command::DarkMode => self.theme = if self.dark() { Theme::Light } else { Theme::Dark },
            Command::Export => self.export_open = true,
            Command::Save => self.save(),
            Command::SaveAs => self.save_as(),
            Command::Open => self.open(),
            Command::Undo => self.undo(),
            Command::Redo => self.redo(),
            Command::ZoomIn => self.zoom_step(0.1),
            Command::ZoomOut => self.zoom_step(-0.1),
            Command::ZoomReset => {
                let center = self.canvas_rect.center();
                self.zoom_at(1.0, center);
            }
            Command::ZoomFit => self.zoom_to_fit(false),
            Command::ZoomSelection => self.zoom_to_fit(true),
            Command::SelectAll => self.select_all(),
            Command::Clear => self.clear_canvas(),
            Command::Tool(tool) => self.select_tool(tool),
            Command::Lock => self.tool_locked = !self.tool_locked,
            Command::Duplicate => self.duplicate(),
            Command::Delete => self.delete_selected(),
            Command::Group => self.group_selection(),
            Command::Ungroup => self.ungroup_selection(),
            Command::Layer(layer) => self.reorder(layer),
            Command::Grid => self.prefs.grid = !self.prefs.grid,
            Command::LockSelection => self.toggle_lock_selected(),
            Command::LockAll => self.lock_all(),
            Command::UnlockAll => self.unlock(None),
            Command::SnapObjects => self.prefs.snap_objects = !self.prefs.snap_objects,
            Command::Zen => self.prefs.zen = !self.prefs.zen,
            Command::ViewMode => {
                self.prefs.view_mode = !self.prefs.view_mode;
                self.selected.clear();
            }
            Command::Stats => self.prefs.stats = !self.prefs.stats,
            Command::ShowHints => self.prefs.show_hints = !self.prefs.show_hints,
        }
    }

    /// Searchable command list with shortcuts (Cmd+/), styled after CommandPalette.scss.
    fn command_palette(&mut self, ctx: &Context) {
        let c = self.colors();
        let items = self.palette_items();
        self.palette_index = self.palette_index.min(items.len().saturating_sub(1));
        let (down, up, enter, escape) = ctx.input_mut(|i| {
            (
                i.consume_key(Modifiers::NONE, Key::ArrowDown),
                i.consume_key(Modifiers::NONE, Key::ArrowUp),
                i.consume_key(Modifiers::NONE, Key::Enter),
                i.consume_key(Modifiers::NONE, Key::Escape),
            )
        });
        if down && !items.is_empty() {
            self.palette_index = (self.palette_index + 1) % items.len();
        }
        if up && !items.is_empty() {
            self.palette_index = (self.palette_index + items.len() - 1) % items.len();
        }
        let mut run = enter.then(|| items.get(self.palette_index).copied()).flatten();

        let screen = ctx.content_rect();
        ctx.layer_painter(egui::LayerId::new(egui::Order::Foreground, egui::Id::unique("palette_dim")))
            .rect_filled(screen, 0.0, Color32::from_rgba_unmultiplied(12, 12, 14, 89));
        let selected_bg = if self.dark() { Color32::from_rgb(24, 24, 27) } else { Color32::from_rgb(0xf6, 0xf6, 0xf9) };
        let dialog = egui::Area::new(egui::Id::unique("command_palette"))
            .anchor(egui::Align2::CENTER_TOP, (0.0, 47.0))
            .order(egui::Order::Tooltip)
            .show(ctx, |ui| {
                egui::Frame::new()
                    .fill(c.island)
                    .corner_radius(12)
                    .stroke(Stroke::new(1.0, if self.dark() { Color32::from_rgb(60, 60, 66) } else { Color32::from_rgb(0xeb, 0xeb, 0xeb) }))
                    .inner_margin(egui::Margin { left: 24, right: 24, top: 24, bottom: 16 })
                    .show(ui, |ui| {
                        ui.set_width(672.0);
                        ui.spacing_mut().item_spacing = Vec2::new(4.0, 0.0);
                        egui::Frame::new()
                            .fill(c.surface_high)
                            .stroke(Stroke::new(1.0, PRIMARY))
                            .corner_radius(8)
                            .inner_margin(egui::Margin::symmetric(12, 8))
                            .show(ui, |ui| {
                                let input = ui.add(
                                    egui::TextEdit::singleline(&mut self.palette_query)
                                        .frame(egui::Frame::NONE)
                                        .font(FontId::proportional(16.0))
                                        .text_color(c.text)
                                        .hint_text(
                                            egui::RichText::new("Procure menus, comandos e descubra gemas ocultas")
                                                .size(16.0)
                                                .color(c.muted),
                                        )
                                        .desired_width(f32::INFINITY),
                                );
                                if input.changed() {
                                    self.palette_index = 0;
                                }
                                if !self.palette_focused {
                                    input.request_focus();
                                    self.palette_focused = true;
                                }
                            });
                        ui.add_space(8.0);
                        ui.horizontal(|ui| {
                            let hints = [("↑↓", "Selecionar"), ("↵", "Confirmar"), ("Esc", "Fechar")];
                            let width = 230.0;
                            ui.add_space((ui.available_width() - width) / 2.0);
                            for (key, label) in hints {
                                key_chip(ui, key, &c);
                                ui.label(egui::RichText::new(label).size(10.0).color(c.muted));
                                ui.add_space(12.0);
                            }
                        });
                        ui.add_space(12.0);
                        let max_height = (screen.height() - 230.0).max(120.0);
                        egui::ScrollArea::vertical().max_height(max_height).auto_shrink([false, true]).show(ui, |ui| {
                            ui.set_width(ui.available_width());
                            let mut group = "";
                            if items.is_empty() {
                                ui.add_space(8.0);
                                ui.label(egui::RichText::new("Nenhum resultado").size(14.0).color(c.muted));
                            }
                            for (k, cmd) in items.iter().copied().enumerate() {
                                if cmd.group() != group {
                                    group = cmd.group();
                                    ui.add_space(if k == 0 { 0.0 } else { 16.0 });
                                    ui.label(egui::RichText::new(group).size(16.0).strong().color(c.text));
                                    ui.add_space(6.0);
                                }
                                let (rect, response) =
                                    ui.allocate_exact_size(Vec2::new(ui.available_width(), 40.0), Sense::click());
                                let response = response.on_hover_cursor(CursorIcon::PointingHand);
                                if response.hovered() && ui.input(|i| i.pointer.delta() != Vec2::ZERO) {
                                    self.palette_index = k;
                                }
                                let p = ui.painter();
                                if k == self.palette_index {
                                    p.rect_filled(rect, 8, selected_bg);
                                    if down || up {
                                        ui.scroll_to_rect(rect, None);
                                    }
                                }
                                paint_icon(p, cmd.icon(), Pos2::new(rect.left() + 16.0, rect.center().y), c.text);
                                p.text(Pos2::new(rect.left() + 36.0, rect.center().y), egui::Align2::LEFT_CENTER, cmd.label(), FontId::proportional(16.0), c.text);
                                let mut x = rect.right() - 8.0;
                                for key in shortcut_keys(cmd.keys()).iter().rev() {
                                    let galley = p.layout_no_wrap(key.to_string(), FontId::proportional(10.0), c.text);
                                    let chip = Rect::from_min_max(
                                        Pos2::new(x - galley.size().x - 8.0, rect.center().y - 8.0),
                                        Pos2::new(x, rect.center().y + 8.0),
                                    );
                                    p.rect_filled(chip, 4, c.selected);
                                    p.galley(chip.center() - galley.size() / 2.0, galley, c.text);
                                    x = chip.left() - 4.0;
                                }
                                if response.clicked() {
                                    run = Some(cmd);
                                }
                            }
                        });
                    });
            });
        let clicked_outside = ctx.input(|i| i.pointer.any_pressed())
            && ctx.input(|i| i.pointer.interact_pos()).is_some_and(|p| !dialog.response.rect.contains(p));
        if escape || clicked_outside || run.is_some() {
            self.palette_open = false;
        }
        if let Some(cmd) = run {
            self.run_command(cmd);
        }
    }

    fn shows_properties(&self) -> bool {
        self.editing.is_some()
            || !self.selected.is_empty()
            || !matches!(self.tool, Tool::Select | Tool::Hand | Tool::Eraser | Tool::Laser | Tool::Lasso)
    }

    fn properties_panel(&mut self, ctx: &Context) {
        let c = self.colors();
        let kinds: Vec<Element> = if self.selected.is_empty() {
            let probe = match self.tool {
                Tool::Rectangle | Tool::Fill | Tool::Sticky | Tool::Frame => ElementKind::Rectangle { x: 0.0, y: 0.0, width: 1.0, height: 1.0 },
                Tool::Diamond => ElementKind::Diamond { x: 0.0, y: 0.0, width: 1.0, height: 1.0 },
                Tool::Ellipse => ElementKind::Ellipse { x: 0.0, y: 0.0, width: 1.0, height: 1.0 },
                Tool::Text => ElementKind::Text { x: 0.0, y: 0.0, value: String::new() },
                tool => ElementKind::Line { points: vec![], arrow: tool == Tool::Arrow },
            };
            vec![self.new_element(probe)]
        } else {
            self.selected.iter().filter_map(|i| self.scene.elements.get(*i).cloned()).collect()
        };
        // like the web: frames only take opacity and layers, the fill tool only its fill
        let frames_only = if self.selected.is_empty() { self.tool == Tool::Frame } else { kinds.iter().all(|e| e.frame.is_some()) };
        let fill_tool = self.selected.is_empty() && self.tool == Tool::Fill;
        let fillable = !frames_only && kinds.iter().any(Element::fillable);
        let stroked = !frames_only && !fill_tool && kinds.iter().any(|e| !matches!(e.kind, ElementKind::Text { .. }));
        let edged = !frames_only && !fill_tool && kinds.iter().any(|e| matches!(e.kind, ElementKind::Rectangle { .. } | ElementKind::Diamond { .. }));
        let texty = kinds.iter().any(|e| matches!(e.kind, ElementKind::Text { .. }));
        let frame_index = match self.selected[..] {
            [i] if self.scene.elements.get(i).is_some_and(|e| e.frame.is_some()) => Some(i),
            _ => None,
        };
        let arrows_only = kinds.iter().all(|e| matches!(e.kind, ElementKind::Line { arrow: true, .. }));
        let heads_before = kinds
            .first()
            .filter(|_| arrows_only)
            .map_or(self.heads, |e| (e.ends.start_head, e.ends.end_head));
        let mut heads = heads_before;
        let mut picker_anchor = None;
        let has_selection = !self.selected.is_empty();
        let before = kinds.first().map(|e| e.style).unwrap_or(self.style);
        let mut s = before;
        let mut layer = None;
        let mut action = None;
        let dark = self.dark();

        egui::Area::new(egui::Id::unique("properties"))
            .fixed_pos((16.0, 76.0))
            .show(ctx, |ui| {
                self.island().inner_margin(12).show(ui, |ui| {
                    ui.set_width(178.0);
                    ui.spacing_mut().item_spacing = Vec2::new(4.0, 4.0);
                    if !frames_only && !fill_tool {
                        section(ui, "Contorno", &c);
                        ui.horizontal(|ui| {
                            for color in STROKE_PICKS {
                                if swatch(ui, color, s.stroke == color, 22.0, dark).clicked() {
                                    s.stroke = color;
                                }
                            }
                            ui.add_space(4.0);
                            color_picker(ui, &mut s.stroke);
                        });
                    }
                    if fillable {
                        section(ui, "Fundo", &c);
                        ui.horizontal(|ui| {
                            let current = s.fill.unwrap_or(TRANSPARENT);
                            // the fill tool never paints transparent
                            for color in BACKGROUND_PICKS.into_iter().filter(|c| !fill_tool || c[3] > 0) {
                                if swatch(ui, color, current == color, 22.0, dark).clicked() {
                                    s.fill = (color[3] > 0).then_some(color);
                                }
                            }
                            ui.add_space(4.0);
                            let mut picked = current;
                            color_picker(ui, &mut picked);
                            if picked != current {
                                s.fill = (picked[3] > 0).then_some(picked);
                            }
                        });
                        if s.fill.is_some() || fill_tool {
                            section(ui, "Preenchimento", &c);
                            ui.horizontal(|ui| {
                                for (value, icon, tip) in [
                                    (FillStyle::Hachure, Icon::Hachure, "Hachura"),
                                    (FillStyle::CrossHatch, Icon::CrossHatch, "Hachura cruzada"),
                                    (FillStyle::Solid, Icon::SolidFill, "Sólido"),
                                ] {
                                    if prop_button(ui, s.fill_style == value, icon, tip, &c) {
                                        s.fill_style = value;
                                    }
                                }
                            });
                        }
                    }
                    if stroked {
                        section(ui, "Espessura do traço", &c);
                        ui.horizontal(|ui| {
                            for (value, tip) in [(1.0, "Fino"), (2.0, "Espesso"), (4.0, "Muito espesso")] {
                                if prop_button(ui, s.stroke_width == value, Icon::Width(value), tip, &c) {
                                    s.stroke_width = value;
                                }
                            }
                        });
                        section(ui, "Estilo do traço", &c);
                        ui.horizontal(|ui| {
                            for (value, tip) in [
                                (StrokeStyle::Solid, "Sólido"),
                                (StrokeStyle::Dashed, "Tracejado"),
                                (StrokeStyle::Dotted, "Pontilhado"),
                            ] {
                                if prop_button(ui, s.stroke_style == value, Icon::Dash(value), tip, &c) {
                                    s.stroke_style = value;
                                }
                            }
                        });
                        section(ui, "Precisão do traço", &c);
                        ui.horizontal(|ui| {
                            for (value, tip) in [(0, "Arquiteto"), (1, "Artista"), (2, "Cartunista")] {
                                if prop_button(ui, s.roughness == value, Icon::Sloppiness(value), tip, &c) {
                                    s.roughness = value;
                                }
                            }
                        });
                    }
                    if arrows_only {
                        section(ui, "Tipo de seta", &c);
                        ui.horizontal(|ui| {
                            for (value, tip) in [(false, "Seta reta"), (true, "Seta curva")] {
                                if prop_button(ui, s.round == value, Icon::ArrowType(value), tip, &c) {
                                    s.round = value;
                                }
                            }
                        });
                        section(ui, "Pontas", &c);
                        ui.horizontal(|ui| {
                            for (end, tip) in [(false, "Ponta inicial"), (true, "Ponta final")] {
                                let current = if end { heads.1 } else { heads.0 };
                                let response = square_button(ui, 32.0, self.head_picker == Some(end), c.surface_low, &c);
                                paint_icon(ui.painter(), Icon::Head(current, !end), response.rect.center(), c.text);
                                if response.clicked() {
                                    self.head_picker = if self.head_picker == Some(end) { None } else { Some(end) };
                                    self.head_picker_more = false;
                                }
                                if self.head_picker == Some(end) {
                                    picker_anchor = Some(response.rect.left_bottom());
                                }
                                response.on_hover_text(tip);
                            }
                        });
                    }
                    if edged {
                        section(ui, "Arestas", &c);
                        ui.horizontal(|ui| {
                            for (value, tip) in [(false, "Pontudo"), (true, "Arredondado")] {
                                if prop_button(ui, s.round == value, Icon::Edge(value), tip, &c) {
                                    s.round = value;
                                }
                            }
                        });
                    }
                    if let Some(i) = frame_index {
                        section(ui, "Nome do quadro", &c);
                        if let Some(name) = self.scene.elements[i].frame.as_mut() {
                            if ui.add(egui::TextEdit::singleline(name).desired_width(178.0)).changed() {
                                self.dirty = true;
                            }
                        }
                    }
                    if texty {
                        section(ui, "Tamanho da fonte", &c);
                        ui.horizontal(|ui| {
                            for (value, tip, label) in
                                [(16.0, "Pequeno", "S"), (20.0, "Médio", "M"), (28.0, "Grande", "L"), (36.0, "Muito grande", "XL")]
                            {
                                if prop_button(ui, s.font_size == value, Icon::Label(label), tip, &c) {
                                    s.font_size = value;
                                }
                            }
                        });
                    }
                    section(ui, "Opacidade", &c);
                    let mut opacity = s.opacity as f32;
                    ui.spacing_mut().slider_width = 178.0;
                    ui.add(egui::Slider::new(&mut opacity, 0.0..=100.0).show_value(false).step_by(10.0));
                    s.opacity = opacity as u8;
                    ui.horizontal(|ui| {
                        ui.label(egui::RichText::new("0").size(11.0).color(c.text));
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            ui.label(egui::RichText::new("100").size(11.0).color(c.text));
                        });
                    });
                    if has_selection {
                        section(ui, "Camadas", &c);
                        ui.horizontal(|ui| {
                            for (value, icon, tip) in [
                                (Layer::Back, Icon::ToBack, "Enviar para o fundo — Cmd+Option+["),
                                (Layer::Backward, Icon::Backward, "Enviar para trás — Cmd+["),
                                (Layer::Forward, Icon::Forward, "Trazer para a frente — Cmd+]"),
                                (Layer::Front, Icon::ToFront, "Trazer para o primeiro plano — Cmd+Option+]"),
                            ] {
                                if prop_button(ui, false, icon, tip, &c) {
                                    layer = Some(value);
                                }
                            }
                        });
                        section(ui, "Ações", &c);
                        ui.horizontal(|ui| {
                            if prop_button(ui, false, Icon::Duplicate, "Duplicar — Cmd+D", &c) {
                                action = Some("duplicate");
                            }
                            if prop_button(ui, false, Icon::Trash, "Apagar — Delete", &c) {
                                action = Some("delete");
                            }
                            if self.selected.len() > 1 && prop_button(ui, false, Icon::Group, "Agrupar seleção — Cmd+G", &c) {
                                action = Some("group");
                            }
                            if self.selected_has_group() && prop_button(ui, false, Icon::Ungroup, "Desagrupar seleção — Cmd+Shift+G", &c) {
                                action = Some("ungroup");
                            }
                            if prop_button(ui, false, Icon::Lock(true), "Bloquear — Cmd+Shift+L", &c) {
                                action = Some("lock");
                            }
                        });
                    }
                });
            });

        if s != before {
            if has_selection {
                if !self.style_edit_active {
                    self.snapshot();
                    self.style_edit_active = true;
                }
                for i in self.selected.clone() {
                    if let Some(e) = self.scene.elements.get_mut(i) {
                        apply_changed(&mut e.style, &before, &s);
                    }
                }
                self.dirty = true;
            }
            let current = self.style;
            apply_changed(&mut self.style, &current, &s);
            if arrows_only {
                // arrow type is remembered separately from the box "edges" setting
                self.arrow_curved = s.round;
                self.style.round = current.round;
            }
        }
        if let (Some(end), Some(anchor)) = (self.head_picker, picker_anchor) {
            // popover like the reference app's: Q W E R picks, then "Mais opções"
            let current = if end { heads.1 } else { heads.0 };
            let mut picked = None;
            let popup = egui::Area::new(egui::Id::unique("head_picker"))
                .fixed_pos(anchor + Vec2::new(8.0, 4.0))
                .order(egui::Order::Foreground)
                .show(ctx, |ui| {
                    self.island().inner_margin(6).show(ui, |ui| {
                        ui.spacing_mut().item_spacing = Vec2::new(4.0, 4.0);
                        let mut row = |ui: &mut egui::Ui, picks: &[Option<Head>], keys: &[&str]| {
                            ui.horizontal(|ui| {
                                for (k, &pick) in picks.iter().enumerate() {
                                    let r = square_button(ui, 32.0, current == pick, c.surface_low, &c);
                                    paint_icon(ui.painter(), Icon::Head(pick, !end), r.rect.center(), c.text);
                                    if let Some(key) = keys.get(k) {
                                        ui.painter().text(r.rect.right_bottom() - Vec2::new(3.0, 2.0), egui::Align2::RIGHT_BOTTOM, *key, FontId::proportional(8.0), c.muted);
                                    }
                                    let tip = keys.get(k).map_or(Head::label(pick).to_string(), |key| format!("{} — {key}", Head::label(pick)));
                                    if r.on_hover_text(tip).clicked() {
                                        picked = Some(pick);
                                    }
                                }
                            });
                        };
                        row(ui, &Head::PICKS[..4], &["Q", "W", "E", "R"]);
                        let label = if self.head_picker_more { "Mais opções  ⌃" } else { "Mais opções  ⌄" };
                        if ui.add(egui::Label::new(egui::RichText::new(label).size(12.0).color(c.text)).sense(Sense::click())).clicked() {
                            self.head_picker_more = !self.head_picker_more;
                        }
                        if self.head_picker_more {
                            row(ui, &Head::PICKS[4..], &[]);
                        }
                    });
                });
            if let Some(pick) = picked {
                *(if end { &mut heads.1 } else { &mut heads.0 }) = pick;
                self.head_picker = None;
            } else if ctx.input(|i| i.pointer.any_pressed())
                // ponytail: x > 230 skips the properties panel (16px margin + 202px island); use its rect if it ever moves
                && ctx.input(|i| i.pointer.interact_pos()).is_some_and(|p| !popup.response.rect.contains(p) && p.x > 230.0)
            {
                self.head_picker = None;
            }
        }
        if heads != heads_before {
            if has_selection {
                self.snapshot();
                for i in self.selected.clone() {
                    if let Some(e) = self.scene.elements.get_mut(i) {
                        (e.ends.start_head, e.ends.end_head) = heads;
                    }
                }
            }
            self.heads = heads;
        }
        if !arrows_only {
            self.head_picker = None;
        }
        if !ctx.input(|i| i.pointer.primary_down()) {
            self.style_edit_active = false;
        }
        if let Some(layer) = layer {
            self.reorder(layer);
        }
        match action {
            Some("duplicate") => self.duplicate(),
            Some("delete") => self.delete_selected(),
            Some("group") => self.group_selection(),
            Some("ungroup") => self.ungroup_selection(),
            Some("lock") => self.toggle_lock_selected(),
            _ => {}
        }
    }

    fn export_dialog(&mut self, ctx: &Context) {
        let mut open = true;
        let mut export = None;
        egui::Window::new("Exportar imagem")
            .open(&mut open)
            .collapsible(false)
            .resizable(false)
            .anchor(egui::Align2::CENTER_CENTER, Vec2::ZERO)
            .show(ctx, |ui| {
                ui.set_width(280.0);
                ui.checkbox(&mut self.export_background, "Fundo");
                ui.horizontal(|ui| {
                    ui.label("Escala");
                    for scale in [1.0, 2.0, 3.0] {
                        if ui.selectable_label(self.export_scale == scale, format!("{scale}×")).clicked() {
                            self.export_scale = scale;
                        }
                    }
                });
                ui.add_space(8.0);
                ui.horizontal(|ui| {
                    if ui.add(egui::Button::new(egui::RichText::new("PNG").color(Color32::WHITE)).fill(PRIMARY)).clicked() {
                        export = Some(true);
                    }
                    if ui.add(egui::Button::new(egui::RichText::new("SVG").color(Color32::WHITE)).fill(PRIMARY)).clicked() {
                        export = Some(false);
                    }
                });
            });
        self.export_open &= open;
        if let Some(png) = export {
            self.export(png);
        }
    }

}

#[derive(Clone, Copy, Debug, PartialEq)]
enum Command {
    DarkMode,
    Export,
    Save,
    SaveAs,
    Open,
    Undo,
    Redo,
    ZoomIn,
    ZoomOut,
    ZoomReset,
    ZoomFit,
    ZoomSelection,
    SelectAll,
    Clear,
    Tool(Tool),
    Lock,
    Duplicate,
    Delete,
    Group,
    Ungroup,
    Layer(Layer),
    Grid,
    LockSelection,
    LockAll,
    UnlockAll,
    SnapObjects,
    Zen,
    ViewMode,
    Stats,
    ShowHints,
}

impl Command {
    const ALL: [Command; 42] = [
        Command::LockAll,
        Command::UnlockAll,
        Command::LockSelection,
        Command::Grid,
        Command::SnapObjects,
        Command::Zen,
        Command::ViewMode,
        Command::Stats,
        Command::ShowHints,
        Command::DarkMode,
        Command::Export,
        Command::Save,
        Command::SaveAs,
        Command::Open,
        Command::Undo,
        Command::Redo,
        Command::ZoomIn,
        Command::ZoomOut,
        Command::ZoomReset,
        Command::ZoomFit,
        Command::SelectAll,
        Command::Clear,
        Command::Tool(Tool::Hand),
        Command::Tool(Tool::Select),
        Command::Tool(Tool::Rectangle),
        Command::Tool(Tool::Diamond),
        Command::Tool(Tool::Ellipse),
        Command::Tool(Tool::Arrow),
        Command::Tool(Tool::Line),
        Command::Tool(Tool::Draw),
        Command::Tool(Tool::Text),
        Command::Tool(Tool::Eraser),
        Command::Lock,
        Command::ZoomSelection,
        Command::Duplicate,
        Command::Delete,
        Command::Group,
        Command::Ungroup,
        Command::Layer(Layer::Front),
        Command::Layer(Layer::Forward),
        Command::Layer(Layer::Backward),
        Command::Layer(Layer::Back),
    ];

    fn group(self) -> &'static str {
        match self {
            Command::DarkMode => "App",
            Command::Export | Command::Save | Command::SaveAs | Command::Open => "Exportar",
            Command::Tool(_) | Command::Lock => "Ferramentas",
            Command::ZoomSelection
            | Command::Duplicate
            | Command::Delete
            | Command::Group
            | Command::Ungroup
            | Command::LockSelection
            | Command::Layer(_) => "Elementos",
            _ => "Editor",
        }
    }

    fn label(self) -> &'static str {
        match self {
            Command::DarkMode => "Modo escuro",
            Command::Export => "Exportar imagem...",
            Command::Save => "Salvar no computador",
            Command::SaveAs => "Salvar como...",
            Command::Open => "Abrir",
            Command::Undo => "Desfazer",
            Command::Redo => "Refazer",
            Command::ZoomIn => "Aumentar zoom",
            Command::ZoomOut => "Diminuir zoom",
            Command::ZoomReset => "Redefinir zoom",
            Command::ZoomFit => "Ampliar para encaixar todos os elementos",
            Command::ZoomSelection => "Zoom para caber na tela",
            Command::SelectAll => "Selecionar tudo",
            Command::Clear => "Limpar a tela",
            Command::Tool(tool) => tool.label(),
            Command::Lock => "Manter ativa a ferramenta selecionada após desenhar",
            Command::Duplicate => "Duplicar",
            Command::Delete => "Apagar",
            Command::Group => "Agrupar seleção",
            Command::Ungroup => "Desagrupar seleção",
            Command::Layer(Layer::Front) => "Trazer para o primeiro plano",
            Command::Layer(Layer::Forward) => "Trazer para a frente",
            Command::Layer(Layer::Backward) => "Enviar para trás",
            Command::Layer(Layer::Back) => "Enviar para o fundo",
            Command::Grid => "Alternar grade",
            Command::LockSelection => "Bloquear / desbloquear seleção",
            Command::LockAll => "Bloquear tudo",
            Command::UnlockAll => "Desbloquear tudo",
            Command::SnapObjects => "Encaixar em objetos",
            Command::Zen => "Modo Zen",
            Command::ViewMode => "Modo de visualização",
            Command::Stats => "Propriedades da Tela e da Forma",
            Command::ShowHints => "Mostrar dicas",
        }
    }

    fn keys(self) -> &'static str {
        match self {
            Command::DarkMode => "Shift+Option+D",
            Command::Export => "Cmd+Shift+E",
            Command::Save => "Cmd+S",
            Command::SaveAs => "Cmd+Shift+S",
            Command::Open => "Cmd+O",
            Command::Undo => "Cmd+Z",
            Command::Redo => "Cmd+Shift+Z",
            Command::ZoomIn => "Cmd++",
            Command::ZoomOut => "Cmd+-",
            Command::ZoomReset => "Cmd+0",
            Command::ZoomFit => "Shift+1",
            Command::ZoomSelection => "Shift+2",
            Command::SelectAll => "Cmd+A",
            Command::Clear => "Cmd+Delete",
            Command::Tool(tool) => tool.badge(),
            Command::Lock => "Q",
            Command::Duplicate => "Cmd+D",
            Command::Delete => "Delete",
            Command::Group => "Cmd+G",
            Command::Ungroup => "Cmd+Shift+G",
            Command::Layer(Layer::Front) => "Cmd+Option+]",
            Command::Layer(Layer::Forward) => "Cmd+]",
            Command::Layer(Layer::Backward) => "Cmd+[",
            Command::Layer(Layer::Back) => "Cmd+Option+[",
            Command::Grid => "Cmd+'",
            Command::LockSelection => "Cmd+Shift+L",
            Command::LockAll | Command::UnlockAll => "",
            Command::SnapObjects => "Option+S",
            Command::Zen => "Option+Z",
            Command::ViewMode => "Option+R",
            Command::Stats => "Option+/",
            Command::ShowHints => "",
        }
    }

    fn icon(self) -> Icon {
        match self {
            Command::DarkMode => Icon::Moon,
            Command::Export => Icon::Export,
            Command::Save | Command::SaveAs => Icon::Download,
            Command::Open => Icon::Open,
            Command::Undo => Icon::Undo,
            Command::Redo => Icon::Redo,
            Command::ZoomIn => Icon::Plus,
            Command::ZoomOut => Icon::Minus,
            Command::ZoomReset | Command::ZoomFit | Command::ZoomSelection => Icon::Search,
            Command::SelectAll => Icon::Tool(Tool::Select),
            Command::Clear | Command::Delete => Icon::Trash,
            Command::Tool(tool) => Icon::Tool(tool),
            Command::Lock => Icon::Lock(true),
            Command::Duplicate => Icon::Duplicate,
            Command::Group => Icon::Group,
            Command::Ungroup => Icon::Ungroup,
            Command::Layer(Layer::Front) => Icon::ToFront,
            Command::Layer(Layer::Forward) => Icon::Forward,
            Command::Layer(Layer::Backward) => Icon::Backward,
            Command::Layer(Layer::Back) => Icon::ToBack,
            Command::Grid => Icon::Grid,
            Command::LockSelection | Command::LockAll => Icon::Lock(true),
            Command::UnlockAll => Icon::Lock(false),
            Command::SnapObjects => Icon::Sliders,
            Command::Zen | Command::ViewMode => Icon::Eye,
            Command::Stats => Icon::Sliders,
            Command::ShowHints => Icon::Check,
        }
    }
}

/// "Cmd+Shift+E" -> ["Cmd", "Shift", "E"]; keeps the "+" key in "Cmd++".
fn shortcut_keys(keys: &str) -> Vec<&str> {
    if keys.is_empty() {
        return Vec::new();
    }
    match keys.strip_suffix("++") {
        Some(rest) => rest.split('+').chain(["+"]).collect(),
        None => keys.split('+').collect(),
    }
}

/// Lowercase without accents, so "acao" finds "ação".
fn fold(text: &str) -> String {
    text.to_lowercase()
        .chars()
        .map(|ch| match ch {
            'á' | 'à' | 'â' | 'ã' | 'ä' => 'a',
            'é' | 'ê' | 'è' => 'e',
            'í' | 'ì' => 'i',
            'ó' | 'ô' | 'õ' | 'ò' => 'o',
            'ú' | 'ù' | 'ü' => 'u',
            'ç' => 'c',
            other => other,
        })
        .collect()
}

fn key_chip(ui: &mut egui::Ui, key: &str, c: &UiColors) {
    let galley = ui.painter().layout_no_wrap(key.to_string(), FontId::proportional(10.0), c.text);
    let (rect, _) = ui.allocate_exact_size(Vec2::new(galley.size().x + 8.0, 16.0), Sense::hover());
    ui.painter().rect_filled(rect, 4, c.selected);
    ui.painter().galley(rect.center() - galley.size() / 2.0, galley, c.text);
}

fn apply_changed(target: &mut Style, before: &Style, after: &Style) {
    macro_rules! fields {
        ($($f:ident),*) => { $( if before.$f != after.$f { target.$f = after.$f; } )* };
    }
    fields!(stroke, fill, stroke_width, stroke_style, fill_style, roughness, round, opacity, font_size);
}

impl eframe::App for DrawingApp {
    fn ui(&mut self, root: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let ctx = root.ctx().clone();
        if let Some(theme) = ctx.system_theme() {
            self.system_dark = theme == egui::Theme::Dark;
        }
        configure_visuals(&ctx, self.dark());
        self.handle_shortcuts(&ctx);
        egui::CentralPanel::no_frame().show(root, |ui| {
            let (response, painter) = ui.allocate_painter(ui.available_size(), Sense::click_and_drag());
            let rect = response.rect;
            self.handle_canvas(&response, rect);
            if response.hovered() && ctx.current_pass_index() == 0 {
                let (scroll, zoom, pointer) = ctx.input(|i| (i.smooth_scroll_delta, i.zoom_delta(), i.pointer.hover_pos()));
                let shift = ctx.input(|i| i.modifiers.shift);
                if zoom != 1.0 {
                    self.zoom_at(self.zoom * zoom, pointer.unwrap_or(rect.center()));
                } else if scroll != Vec2::ZERO && self.prefs.mouse_wheel_zooms && !shift {
                    // Dispositivo de entrada = Mouse: the wheel zooms
                    self.zoom_at(self.zoom * (1.0 + scroll.y * 0.002), pointer.unwrap_or(rect.center()));
                } else if scroll != Vec2::ZERO {
                    self.pan += scroll;
                }
            }
            self.update_cursor(&ctx, &response);
            self.paint_scene(&painter, rect);
        });
        self.text_editor(&ctx);
        self.floating_controls(&ctx);
        let idle = !self.moving
            && self.resizing.is_none()
            && self.rotating.is_none()
            && self.point_drag.is_none()
            && self.drawing_start.is_none()
            && self.multi.is_none()
            && self.editing.is_none();
        self.prefs.tool_locked = self.tool_locked;
        self.prefs.theme = self.theme;
        if self.prefs != self.saved_prefs {
            self.saved_prefs = self.prefs;
            if let (Some(path), true) = (prefs_path(), self.autosave.is_some()) {
                let _ = serde_json::to_string_pretty(&self.prefs).map(|json| fs::write(path, json));
            }
        }
        let now = ctx.input(|i| i.time);
        self.laser.retain(|(_, t)| now - *t < 1.0);
        if let Some(id) = self.reveal.front() {
            match self.scene.elements.iter().find(|e| &e.id == id) {
                Some(e) if self.reveal_progress(e) < 1.0 => {}
                done => {
                    if let (Some(e), Some(agent)) = (done, &mut self.agent) {
                        (agent.1, agent.2) = (reveal_pen(e), now);
                    }
                    self.reveal.pop_front();
                    self.reveal_since = now;
                }
            }
            ctx.request_repaint();
        } else if self.agent.as_ref().is_some_and(|a| now - a.2 < AGENT_LINGER) {
            ctx.request_repaint();
        }
        if !self.laser.is_empty() {
            ctx.request_repaint();
        }
        if idle && now - self.last_poll > 0.4 {
            self.last_poll = now;
            self.sync_from_file();
        }
        if self.dirty && idle {
            self.write_autosave();
        }
        ctx.request_repaint_after(std::time::Duration::from_millis(400));
    }
}

// ---------- UI helpers ----------

struct UiColors {
    island: Color32,
    surface_low: Color32,
    surface_high: Color32,
    selected: Color32,
    text: Color32,
    muted: Color32,
}

impl UiColors {
    // css/theme.scss
    fn new(dark: bool) -> Self {
        if dark {
            Self {
                island: Color32::from_rgb(0x23, 0x23, 0x29),
                // lifted above the island (#232329) so buttons and toggles stay visible
                surface_low: Color32::from_rgb(0x31, 0x30, 0x3c),
                surface_high: Color32::from_rgb(0x3d, 0x3c, 0x4b),
                selected: Color32::from_rgb(0x40, 0x3e, 0x6a),
                text: Color32::from_rgb(0xe3, 0xe3, 0xe8),
                muted: Color32::from_rgb(0x7a, 0x7a, 0x7a),
            }
        } else {
            Self {
                island: Color32::WHITE,
                surface_low: Color32::from_rgb(0xec, 0xec, 0xf4),
                surface_high: Color32::from_rgb(0xf1, 0xf0, 0xff),
                selected: Color32::from_rgb(0xe0, 0xdf, 0xff),
                text: Color32::from_rgb(0x1b, 0x1b, 0x1f),
                muted: Color32::from_rgb(0x99, 0x99, 0x99),
            }
        }
    }
}

fn group_frame(c: &UiColors) -> egui::Frame {
    egui::Frame::new().fill(c.surface_low).corner_radius(8)
}

fn square_button(ui: &mut egui::Ui, size: f32, selected: bool, idle: Color32, c: &UiColors) -> egui::Response {
    let (rect, response) = ui.allocate_exact_size(Vec2::splat(size), Sense::click());
    let fill = if selected {
        c.selected
    } else if response.hovered() {
        c.surface_high
    } else {
        idle
    };
    ui.painter().rect_filled(rect, 8, fill);
    response.on_hover_cursor(CursorIcon::PointingHand)
}

fn separator(ui: &mut egui::Ui, c: &UiColors) {
    let (rect, _) = ui.allocate_exact_size(Vec2::new(1.0, 36.0), Sense::hover());
    ui.painter().line_segment(
        [rect.center_top() + Vec2::new(0.0, 8.0), rect.center_bottom() - Vec2::new(0.0, 8.0)],
        Stroke::new(1.0, c.surface_low),
    );
}

fn section(ui: &mut egui::Ui, title: &str, c: &UiColors) {
    ui.add_space(6.0);
    ui.label(egui::RichText::new(title).size(12.0).color(c.text));
    ui.add_space(2.0);
}

fn prop_button(ui: &mut egui::Ui, selected: bool, icon: Icon, tip: &str, c: &UiColors) -> bool {
    let response = square_button(ui, 32.0, selected, c.surface_low, c);
    let fg = if selected && !ui.visuals().dark_mode { Color32::from_rgb(0x03, 0x03, 0x5c) } else { c.text };
    paint_icon(ui.painter(), icon, response.rect.center(), fg);
    response.on_hover_text(tip).clicked()
}

/// .dropdown-menu-item: 32px row, 8px padding, 16px icon, 10px gap, 14px text, shortcut at 50%.
fn menu_item(ui: &mut egui::Ui, icon: Icon, label: &str, shortcut: &str, color: Color32, c: &UiColors) -> bool {
    let (rect, response) = ui.allocate_exact_size(Vec2::new(ui.available_width(), 32.0), Sense::click());
    let response = response.on_hover_cursor(CursorIcon::PointingHand);
    let p = ui.painter();
    if response.hovered() {
        p.rect_filled(rect, 6, c.surface_high);
    }
    paint_icon(p, icon, Pos2::new(rect.left() + 16.0, rect.center().y), color);
    p.text(Pos2::new(rect.left() + 34.0, rect.center().y), egui::Align2::LEFT_CENTER, label, FontId::proportional(14.0), color);
    p.text(rect.right_center() - Vec2::new(8.0, 0.0), egui::Align2::RIGHT_CENTER, shortcut, FontId::proportional(14.0), color.gamma_multiply(0.5));
    response.clicked()
}

fn swatch(ui: &mut egui::Ui, color: [u8; 4], selected: bool, size: f32, dark: bool) -> egui::Response {
    let (rect, response) = ui.allocate_exact_size(Vec2::splat(size), Sense::click());
    let response = response.on_hover_cursor(CursorIcon::PointingHand);
    let p = ui.painter();
    let inner = rect.shrink(1.0);
    if color[3] == 0 {
        // checkerboard for "transparent"
        p.rect_filled(inner, 4, Color32::WHITE);
        let cell = inner.width() / 4.0;
        for row in 0..4 {
            for col in 0..4 {
                if (row + col) % 2 == 0 {
                    p.rect_filled(
                        Rect::from_min_size(inner.min + Vec2::new(col as f32, row as f32) * cell, Vec2::splat(cell)),
                        0,
                        Color32::from_gray(225),
                    );
                }
            }
        }
    } else {
        p.rect_filled(inner, 4, themed(rgba(color), dark));
    }
    p.rect_stroke(inner, 4, Stroke::new(1.0, Color32::from_black_alpha(25)), StrokeKind::Inside);
    if selected {
        p.rect_stroke(rect.expand(1.5), 5, Stroke::new(1.0, PRIMARY), StrokeKind::Outside);
    }
    response
}

fn color_picker(ui: &mut egui::Ui, rgba_value: &mut [u8; 4]) {
    let mut color = rgba(*rgba_value);
    if ui.color_edit_button_srgba(&mut color).on_hover_text("Mais cores").changed() {
        *rgba_value = [color.r(), color.g(), color.b(), color.a()];
    }
}

#[derive(Clone, Copy)]
enum Icon {
    Tool(Tool),
    Image,
    Lock(bool),
    Menu,
    Minus,
    Plus,
    Undo,
    Redo,
    Open,
    Download,
    Export,
    Palette,
    Search,
    Monitor,
    Map,
    Sliders,
    More,
    Check,
    Blank,
    Grid,
    Eye,
    Trash,
    Sun,
    Moon,
    Width(f32),
    Dash(StrokeStyle),
    Sloppiness(u8),
    Edge(bool),
    Hachure,
    CrossHatch,
    SolidFill,
    Label(&'static str),
    ToBack,
    Backward,
    Forward,
    ToFront,
    Duplicate,
    Group,
    Ungroup,
    ArrowType(bool),
    Head(Option<Head>, bool),
}

/// 16px icons drawn at 1.25px, matching the reference app's tabler-style set.
fn paint_icon(p: &Painter, icon: Icon, c: Pos2, color: Color32) {
    let s = Stroke::new(1.25, color);
    let at = |x: f32, y: f32| c + Vec2::new(x, y);
    let line = |pts: &[(f32, f32)]| {
        p.add(Shape::line(pts.iter().map(|(x, y)| at(*x, *y)).collect(), s));
    };
    let closed = |pts: &[(f32, f32)]| {
        p.add(Shape::closed_line(pts.iter().map(|(x, y)| at(*x, *y)).collect(), s));
    };
    let rect = |x: f32, y: f32, w: f32, h: f32, r: u8| {
        p.rect_stroke(Rect::from_min_size(at(x, y), Vec2::new(w, h)), r, s, StrokeKind::Middle);
    };
    let arrow_v = |up: bool| {
        let d = if up { -1.0 } else { 1.0 };
        line(&[(0.0, -5.0 * d), (0.0, 5.0 * d)]);
        line(&[(-3.5, 1.5 * d), (0.0, 5.0 * d), (3.5, 1.5 * d)]);
    };
    match icon {
        Icon::Tool(Tool::Hand) => {
            line(&[(-3.0, 1.0), (-3.0, -5.0)]);
            line(&[(0.0, 0.0), (0.0, -6.5)]);
            line(&[(3.0, 0.0), (3.0, -5.0)]);
            line(&[(5.5, 1.0), (5.5, -2.5)]);
            line(&[(-3.0, 0.0), (-5.5, -2.0), (-6.5, 0.0), (-3.5, 5.0), (0.0, 7.0), (3.0, 7.0), (5.5, 4.0), (5.5, 1.0)]);
        }
        Icon::Tool(Tool::Select) => {
            closed(&[(-4.5, -6.5), (5.5, 1.0), (1.0, 2.0), (-1.5, 6.5)]);
            line(&[(1.0, 2.0), (4.0, 6.5)]);
        }
        Icon::Tool(Tool::Rectangle) => rect(-6.0, -6.0, 12.0, 12.0, 2),
        Icon::Tool(Tool::Diamond) => closed(&[(0.0, -7.0), (7.0, 0.0), (0.0, 7.0), (-7.0, 0.0)]),
        Icon::Tool(Tool::Ellipse) => {
            p.circle_stroke(c, 6.5, s);
        }
        Icon::Tool(Tool::Arrow) => {
            line(&[(-7.0, 0.0), (7.0, 0.0)]);
            line(&[(3.0, -4.0), (7.0, 0.0), (3.0, 4.0)]);
        }
        Icon::Tool(Tool::Line) => line(&[(-7.0, 0.0), (7.0, 0.0)]),
        Icon::Tool(Tool::Draw) => {
            closed(&[(-6.0, 6.0), (-6.0, 3.0), (3.5, -6.5), (6.5, -3.5), (-3.0, 6.0)]);
            line(&[(1.5, -4.5), (4.5, -1.5)]);
        }
        Icon::Tool(Tool::Text) => {
            line(&[(-6.0, 6.5), (0.0, -6.5), (6.0, 6.5)]);
            line(&[(-3.5, 2.0), (3.5, 2.0)]);
            line(&[(-2.0, -6.5), (2.0, -6.5)]);
        }
        Icon::Tool(Tool::Sticky) => {
            closed(&[(-6.5, -6.5), (6.5, -6.5), (6.5, 2.0), (2.0, 6.5), (-6.5, 6.5)]);
            line(&[(6.5, 2.0), (2.0, 2.0), (2.0, 6.5)]);
            line(&[(-3.5, -2.5), (3.0, -2.5)]);
            line(&[(-3.5, 0.5), (0.0, 0.5)]);
        }
        Icon::Tool(Tool::AutoShape) => {
            p.circle_stroke(at(-1.5, -1.5), 4.5, s);
            rect(-0.5, -0.5, 7.0, 7.0, 2);
        }
        Icon::Tool(Tool::Laser) => {
            line(&[(-6.5, 6.5), (2.0, -2.0)]);
            line(&[(-5.0, 7.0), (3.5, -1.5)]);
            for (x, y) in [(4.5, -6.5), (6.5, -4.5), (6.0, -7.0), (2.5, -7.0), (7.0, -2.0)] {
                p.circle_filled(at(x, y), 0.8, color);
            }
        }
        Icon::Tool(Tool::Fill) => {
            closed(&[(-4.0, -3.0), (1.0, -7.0), (6.0, -2.0), (1.0, 3.0)]);
            line(&[(-4.0, -3.0), (-5.5, 1.0)]);
            p.circle_filled(at(-5.5, 4.5), 1.6, color);
        }
        Icon::Tool(Tool::Lasso) => {
            line(&[(-1.0, 3.0), (-5.0, 1.0), (-6.5, -2.5), (-4.0, -6.0), (1.5, -6.5), (6.0, -4.0), (6.5, -0.5), (3.0, 2.5), (-1.0, 3.0)]);
            line(&[(-1.0, 3.0), (-2.5, 6.5)]);
        }
        Icon::Tool(Tool::Frame) => {
            line(&[(-3.0, -7.0), (-3.0, 7.0)]);
            line(&[(3.0, -7.0), (3.0, 7.0)]);
            line(&[(-7.0, -3.0), (7.0, -3.0)]);
            line(&[(-7.0, 3.0), (7.0, 3.0)]);
        }
        Icon::Tool(Tool::Eraser) => {
            closed(&[(-6.5, 2.5), (1.5, -5.5), (6.5, -0.5), (0.5, 5.5), (-3.5, 5.5)]);
            line(&[(-2.5, -1.5), (2.5, 3.5)]);
            line(&[(0.5, 5.5), (6.5, 5.5)]);
        }
        Icon::Lock(locked) => {
            rect(-5.0, -1.0, 10.0, 8.0, 2);
            if locked {
                line(&[(-3.0, -1.0), (-3.0, -4.0), (-2.0, -6.0), (0.0, -6.8), (2.0, -6.0), (3.0, -4.0), (3.0, -1.0)]);
            } else {
                line(&[(-3.0, -1.0), (-3.0, -4.0), (-2.0, -6.0), (0.0, -6.8), (2.0, -6.0), (3.0, -4.0)]);
            }
            p.circle_filled(at(0.0, 3.0), 1.0, color);
        }
        Icon::Menu => {
            for y in [-4.0, 0.0, 4.0] {
                line(&[(-6.0, y), (6.0, y)]);
            }
        }
        Icon::Minus => line(&[(-6.0, 0.0), (6.0, 0.0)]),
        Icon::Plus => {
            line(&[(-6.0, 0.0), (6.0, 0.0)]);
            line(&[(0.0, -6.0), (0.0, 6.0)]);
        }
        Icon::Undo | Icon::Redo => {
            let m = if matches!(icon, Icon::Undo) { 1.0 } else { -1.0 };
            line(&[(-3.0 * m, -5.0), (-6.0 * m, -2.0), (-3.0 * m, 1.0)]);
            line(&[(-6.0 * m, -2.0), (2.0 * m, -2.0), (5.0 * m, 0.0), (6.0 * m, 3.0), (5.0 * m, 5.0), (2.0 * m, 6.0), (-1.0 * m, 6.0)]);
        }
        Icon::Open => {
            closed(&[(-7.0, -5.0), (-2.0, -5.0), (0.0, -3.0), (7.0, -3.0), (7.0, 5.5), (-7.0, 5.5)]);
        }
        Icon::Download => {
            line(&[(-6.0, 3.0), (-6.0, 6.0), (6.0, 6.0), (6.0, 3.0)]);
            line(&[(0.0, -6.0), (0.0, 2.5)]);
            line(&[(-3.5, -1.0), (0.0, 2.5), (3.5, -1.0)]);
        }
        Icon::Image => {
            rect(-7.0, -6.0, 14.0, 12.0, 2);
            line(&[(-7.0, 4.0), (-2.0, -1.0), (2.0, 3.0), (4.0, 1.0), (7.0, 4.0)]);
            p.circle_stroke(at(3.0, -2.5), 1.2, s);
        }
        Icon::Export => {
            rect(-6.5, -5.5, 13.0, 11.0, 2);
            line(&[(-6.5, 3.0), (-2.5, -1.0), (1.0, 2.5), (3.0, 0.5), (6.5, 4.0)]);
            p.circle_stroke(at(2.5, -2.5), 1.2, s);
        }
        Icon::Palette => closed(&[(1.5, -7.0), (-5.0, 1.0), (0.0, 1.0), (-1.5, 7.0), (5.0, -1.0), (0.0, -1.0)]),
        Icon::Search => {
            p.circle_stroke(at(-1.0, -1.0), 5.0, s);
            line(&[(2.5, 2.5), (6.5, 6.5)]);
        }
        Icon::Sliders => {
            line(&[(-7.0, -4.0), (7.0, -4.0)]);
            line(&[(-7.0, 4.0), (7.0, 4.0)]);
            p.circle(at(-2.5, -4.0), 2.0, Color32::TRANSPARENT, s);
            p.circle(at(3.0, 4.0), 2.0, Color32::TRANSPARENT, s);
        }
        Icon::Check => line(&[(-5.5, 0.5), (-1.5, 4.5), (6.0, -4.5)]),
        Icon::More => {
            for y in [-5.0, 0.0, 5.0] {
                p.circle_filled(at(0.0, y), 1.3, color);
            }
        }
        Icon::Blank => {}
        Icon::Grid => {
            rect(-6.5, -6.5, 13.0, 13.0, 2);
            line(&[(-2.2, -6.5), (-2.2, 6.5)]);
            line(&[(2.2, -6.5), (2.2, 6.5)]);
            line(&[(-6.5, -2.2), (6.5, -2.2)]);
            line(&[(-6.5, 2.2), (6.5, 2.2)]);
        }
        Icon::Eye => {
            line(&[(-7.0, 0.0), (-4.0, -3.5), (0.0, -5.0), (4.0, -3.5), (7.0, 0.0), (4.0, 3.5), (0.0, 5.0), (-4.0, 3.5), (-7.0, 0.0)]);
            p.circle_stroke(c, 2.2, s);
        }
        Icon::Map => {
            closed(&[(-7.0, -5.0), (-2.5, -7.0), (2.5, -5.0), (7.0, -7.0), (7.0, 5.0), (2.5, 7.0), (-2.5, 5.0), (-7.0, 7.0)]);
            line(&[(-2.5, -7.0), (-2.5, 5.0)]);
            line(&[(2.5, -5.0), (2.5, 7.0)]);
        }
        Icon::Monitor => {
            rect(-7.0, -5.5, 14.0, 9.5, 2);
            line(&[(-3.0, 6.5), (3.0, 6.5)]);
            line(&[(0.0, 4.0), (0.0, 6.5)]);
        }
        Icon::Trash => {
            line(&[(-6.5, -4.0), (6.5, -4.0)]);
            line(&[(-2.0, -4.0), (-2.0, -6.5), (2.0, -6.5), (2.0, -4.0)]);
            line(&[(-5.0, -4.0), (-4.0, 6.5), (4.0, 6.5), (5.0, -4.0)]);
            line(&[(-1.5, -1.0), (-1.5, 4.0)]);
            line(&[(1.5, -1.0), (1.5, 4.0)]);
        }
        Icon::Sun => {
            p.circle_stroke(c, 3.0, s);
            for i in 0..8 {
                let d = Vec2::angled(i as f32 * std::f32::consts::FRAC_PI_4);
                p.line_segment([c + d * 5.0, c + d * 7.0], s);
            }
        }
        Icon::Moon => {
            p.add(Shape::closed_line(crescent(c), s));
        }
        Icon::Width(width) => {
            p.line_segment([at(-6.5, 0.0), at(6.5, 0.0)], Stroke::new(width.max(1.25) * 0.9 + 0.4, color));
        }
        Icon::Dash(style) => {
            let pts = [at(-7.0, 0.0), at(7.0, 0.0)];
            let shapes = match style {
                StrokeStyle::Solid => vec![Shape::line_segment(pts, Stroke::new(2.0, color))],
                StrokeStyle::Dashed => Shape::dashed_line(&pts, Stroke::new(2.0, color), 3.5, 2.5),
                StrokeStyle::Dotted => Shape::dashed_line(&pts, Stroke::new(2.0, color), 1.5, 2.5),
            };
            p.extend(shapes);
        }
        Icon::Sloppiness(level) => match level {
            0 => line(&[(-7.0, 3.0), (-3.0, -1.5), (3.0, -1.5), (7.0, 3.0)]),
            1 => {
                line(&[(-7.0, 3.0), (-3.5, -1.0), (0.0, 1.5), (3.5, -2.0), (7.0, 2.5)]);
                line(&[(-6.5, 1.5), (-3.0, -2.5), (0.5, 0.0), (4.0, -3.0), (7.0, 1.0)]);
            }
            _ => {
                line(&[(-7.0, 4.0), (-4.0, -3.0), (-1.0, 3.0), (2.0, -3.5), (5.0, 2.5), (7.0, -1.0)]);
            }
        },
        Icon::Edge(round) => {
            if round {
                line(&[(-6.0, 6.0), (-6.0, -1.0), (-5.0, -4.5), (-1.0, -6.0), (6.0, -6.0)]);
            } else {
                line(&[(-6.0, 6.0), (-6.0, -6.0), (6.0, -6.0)]);
            }
            p.add(Shape::dashed_line(&[at(-6.0, 6.0), at(6.0, 6.0), at(6.0, -6.0)], Stroke::new(1.0, color), 1.5, 2.0));
        }
        Icon::Hachure | Icon::CrossHatch | Icon::SolidFill => {
            let r = Rect::from_center_size(c, Vec2::splat(13.0));
            if matches!(icon, Icon::SolidFill) {
                p.rect_filled(r, 2, color);
            } else {
                p.rect_stroke(r, 2, s, StrokeKind::Middle);
                for i in -2..=2 {
                    let o = i as f32 * 3.5;
                    p.line_segment([at(o - 4.0, 5.0), at(o + 4.0, -5.0)].map(|q| q.clamp(r.min, r.max)), s);
                    if matches!(icon, Icon::CrossHatch) {
                        p.line_segment([at(o - 4.0, -5.0), at(o + 4.0, 5.0)].map(|q| q.clamp(r.min, r.max)), s);
                    }
                }
            }
        }
        Icon::Label(text) => {
            p.text(c, egui::Align2::CENTER_CENTER, text, FontId::proportional(13.0), color);
        }
        Icon::ToBack | Icon::ToFront => {
            let up = matches!(icon, Icon::ToFront);
            let y = if up { -6.5 } else { 6.5 };
            line(&[(-6.0, y), (6.0, y)]);
            let d = if up { 1.0 } else { -1.0 };
            line(&[(0.0, -3.5 * d), (0.0, 6.0 * d)]);
            line(&[(-3.5, -0.5 * d), (0.0, -3.5 * d), (3.5, -0.5 * d)]);
        }
        Icon::Backward => arrow_v(false),
        Icon::Forward => arrow_v(true),
        Icon::Duplicate => {
            rect(-6.0, -6.0, 9.0, 9.0, 2);
            rect(-3.0, -3.0, 9.0, 9.0, 2);
        }
        Icon::Head(head, flip) => {
            // shaft plus the head at +x (mirrored for the start end)
            let m = if flip { -1.0 } else { 1.0 };
            let tip = at(6.5 * m, 0.0);
            line(&[(-6.5 * m, 0.0), (6.5 * m, 0.0)]);
            let back = |dx: f32, dy: f32| at((6.5 - dx) * m, dy);
            match head {
                None => {}
                Some(Head::Arrow) => line(&[(2.0 * m, -3.5), (6.5 * m, 0.0), (2.0 * m, 3.5)]),
                Some(Head::Bar) => line(&[(6.5 * m, -4.0), (6.5 * m, 4.0)]),
                Some(h @ (Head::Triangle | Head::TriangleOutline)) => {
                    let pts = vec![tip, back(5.0, -3.0), back(5.0, 3.0)];
                    p.add(Shape::convex_polygon(pts, if h == Head::Triangle { color } else { Color32::TRANSPARENT }, s));
                }
                Some(h @ (Head::Diamond | Head::DiamondOutline)) => {
                    let pts = vec![tip, back(3.0, -2.5), back(6.0, 0.0), back(3.0, 2.5)];
                    p.add(Shape::convex_polygon(pts, if h == Head::Diamond { color } else { Color32::TRANSPARENT }, s));
                }
                Some(h @ (Head::Circle | Head::CircleOutline)) => {
                    let center = back(2.5, 0.0);
                    if h == Head::Circle {
                        p.circle_filled(center, 2.5, color);
                    }
                    p.circle_stroke(center, 2.5, s);
                }
            }
        }
        Icon::ArrowType(curved) => {
            if curved {
                line(&[(-6.0, 6.0), (-5.0, 0.0), (-2.0, -4.0), (3.0, -6.0), (6.5, -6.0)]);
            } else {
                line(&[(-6.0, 6.0), (6.0, -6.0)]);
            }
            line(&[(2.0, -6.5), (6.5, -6.0), (6.0, -1.5)]);
        }
        Icon::Group | Icon::Ungroup => {
            rect(-6.5, -6.5, 8.0, 8.0, 1);
            rect(-1.5, -1.5, 8.0, 8.0, 1);
            if matches!(icon, Icon::Group) {
                p.add(Shape::dashed_line(
                    &[at(-7.5, -7.5), at(7.5, -7.5), at(7.5, 7.5), at(-7.5, 7.5), at(-7.5, -7.5)],
                    Stroke::new(1.0, color),
                    1.5,
                    1.5,
                ));
            }
        }
    }
}

/// Crescent outline (outer circle minus an offset circle) centered on `c`.
fn crescent(c: Pos2) -> Vec<Pos2> {
    let (outer, r1) = (c, 6.5);
    let (inner, r2) = (c + Vec2::new(3.6, -3.0), 6.3);
    // contiguous arc of `ring` where `keep` holds, in increasing angle
    let arc = |center: Pos2, r: f32, keep: &dyn Fn(Pos2) -> bool| -> Vec<Pos2> {
        let n = 120;
        let pts: Vec<Pos2> =
            (0..n).map(|k| center + Vec2::angled(k as f32 / n as f32 * std::f32::consts::TAU) * r).collect();
        let start = (0..n).find(|&k| !keep(pts[k]) && keep(pts[(k + 1) % n])).map_or(0, |k| k + 1);
        (0..n).map(|k| pts[(start + k) % n]).take_while(|p| keep(*p)).collect()
    };
    let mut path = arc(outer, r1, &|p| p.distance(inner) > r2);
    let mut bite = arc(inner, r2, &|p| p.distance(outer) < r1);
    // the bite must start where the outer arc ended
    if let (Some(end), Some(first), Some(last)) = (path.last(), bite.first(), bite.last()) {
        if first.distance(*end) > last.distance(*end) {
            bite.reverse();
        }
    }
    path.extend(bite);
    path
}

fn configure_visuals(ctx: &Context, dark: bool) {
    let c = UiColors::new(dark);
    let mut v = if dark { egui::Visuals::dark() } else { egui::Visuals::light() };
    v.selection.bg_fill = c.selected;
    v.selection.stroke = Stroke::new(1.0, PRIMARY);
    v.widgets.inactive.weak_bg_fill = c.surface_low;
    v.widgets.inactive.bg_fill = c.surface_low;
    v.widgets.inactive.bg_stroke = Stroke::NONE;
    v.widgets.hovered.bg_fill = c.surface_high;
    v.widgets.hovered.weak_bg_fill = c.surface_high;
    v.widgets.hovered.bg_stroke = Stroke::NONE;
    v.widgets.active.bg_fill = PRIMARY;
    v.widgets.inactive.corner_radius = 6.into();
    v.widgets.hovered.corner_radius = 6.into();
    v.window_fill = c.island;
    v.panel_fill = c.island;
    v.window_corner_radius = 12.into();
    v.window_stroke = Stroke::new(1.0, Color32::from_black_alpha(20));
    v.override_text_color = Some(c.text);
    v.slider_trailing_fill = true;
    v.interact_cursor = Some(CursorIcon::PointingHand);
    ctx.set_visuals(v);
}

fn install_fonts(ctx: &Context) {
    let mut fonts = egui::FontDefinitions::default();
    fonts
        .font_data
        .insert("Assistant".into(), egui::FontData::from_static(include_bytes!("../assets/Assistant.ttf")).into());
    fonts.families.get_mut(&egui::FontFamily::Proportional).unwrap().insert(0, "Assistant".into());
    let sources: [&[u8]; 7] = [
        include_bytes!("../assets/hand-1.ttf"),
        include_bytes!("../assets/hand-2.ttf"),
        include_bytes!("../assets/hand-3.ttf"),
        include_bytes!("../assets/hand-4.ttf"),
        include_bytes!("../assets/hand-5.ttf"),
        include_bytes!("../assets/hand-6.ttf"),
        include_bytes!("../assets/hand-7.ttf"),
    ];
    let mut names = vec![];
    for (i, data) in sources.into_iter().enumerate() {
        let name = format!("hand{i}");
        fonts.font_data.insert(name.clone(), egui::FontData::from_static(data).into());
        names.push(name);
    }
    names.push("Assistant".into());
    // symbols the bundled fonts lack (→ ✓ ★ …) come from macOS, then egui's emoji font
    for (i, path) in SYMBOL_FONTS.iter().enumerate() {
        if let Ok(data) = fs::read(path) {
            let name = format!("symbols{i}");
            fonts.font_data.insert(name.clone(), egui::FontData::from_owned(data).into());
            names.push(name.clone());
            fonts.families.get_mut(&egui::FontFamily::Proportional).unwrap().push(name);
        }
    }
    let egui_fallbacks: Vec<String> = fonts.families[&egui::FontFamily::Proportional]
        .iter()
        .filter(|n| !names.contains(n))
        .cloned()
        .collect();
    names.extend(egui_fallbacks);
    fonts.families.insert(egui::FontFamily::Name(HAND_FONT.into()), names);
    ctx.set_fonts(fonts);
    ctx.all_styles_mut(|s| {
        s.text_styles.insert(egui::TextStyle::Body, FontId::proportional(14.0));
        s.text_styles.insert(egui::TextStyle::Button, FontId::proportional(14.0));
        s.text_styles.insert(egui::TextStyle::Heading, FontId::proportional(18.0));
        s.spacing.button_padding = Vec2::new(12.0, 6.0);
        s.spacing.interact_size.y = 28.0;
    });
    ctx.options_mut(|o| o.zoom_with_keyboard = false);
}

// ---------- geometry & rendering ----------

struct View {
    origin: Vec2,
    zoom: f32,
    dark: bool,
    background: Color32,
}

fn rgba(c: [u8; 4]) -> Color32 {
    Color32::from_rgba_unmultiplied(c[0], c[1], c[2], c[3])
}

/// the reference app's dark mode: `invert(93%) hue-rotate(180deg)`, i.e. flip lightness, keep hue.
/// The web's dark-mode filter, `invert(93%) hue-rotate(180deg)` (applyDarkModeFilter).
fn themed(color: Color32, dark: bool) -> Color32 {
    if !dark {
        return color;
    }
    let [r, g, b, a] = color.to_srgba_unmultiplied();
    let [r, g, b] = [r, g, b].map(|v| (v as f32 * 0.07 + (255.0 - v as f32) * 0.93).round() / 255.0);
    let out = [
        -0.574 * r + 1.43 * g + 0.144 * b,
        0.426 * r + 0.43 * g + 0.144 * b,
        0.426 * r + 1.43 * g - 0.856 * b,
    ]
    .map(|v| (v.clamp(0.0, 1.0) * 255.0).round() as u8);
    Color32::from_rgba_unmultiplied(out[0], out[1], out[2], a)
}

fn with_opacity(color: [u8; 4], opacity: u8) -> Color32 {
    let a = (color[3] as f32 * opacity as f32 / 100.0).round() as u8;
    Color32::from_rgba_unmultiplied(color[0], color[1], color[2], a)
}

fn text_lines(value: &str) -> Vec<&str> {
    let lines: Vec<&str> = value.split('\n').collect();
    if lines.is_empty() { vec![""] } else { lines }
}

fn paint_element(painter: &Painter, element: &Element, view: &View, faded: bool, hide_text: bool, halo: bool) {
    let opacity = if faded { element.style.opacity / 3 } else { element.style.opacity };
    let stroke_color = themed(with_opacity(element.style.stroke, opacity), view.dark);
    let (center, angle) = (element.center(), element.angle);
    let to = |p: &[f32; 2]| {
        let p = if angle == 0.0 { Pos2::new(p[0], p[1]) } else { center + rot(Pos2::new(p[0], p[1]) - center, angle) };
        Pos2::new(view.origin.x + p.x * view.zoom, view.origin.y + p.y * view.zoom)
    };
    if let Some(url) = &element.image {
        let b = local_bounds(element);
        let corners = [b.left_top(), b.right_top(), b.right_bottom(), b.left_bottom()].map(|c| to(&[c.x, c.y]));
        match image_texture(painter.ctx(), url) {
            Some(texture) => {
                let mut mesh = egui::Mesh::with_texture(texture.id());
                let tint = Color32::WHITE.gamma_multiply(opacity as f32 / 100.0);
                for (pos, uv) in corners.into_iter().zip([Pos2::new(0.0, 0.0), Pos2::new(1.0, 0.0), Pos2::new(1.0, 1.0), Pos2::new(0.0, 1.0)]) {
                    mesh.vertices.push(egui::epaint::Vertex { pos, uv, color: tint });
                }
                mesh.indices.extend([0, 1, 2, 0, 2, 3]);
                painter.add(mesh);
            }
            None => {
                painter.add(Shape::closed_line(corners.to_vec(), Stroke::new(1.0, stroke_color)));
            }
        }
        return;
    }
    if let Some(created) = element.created {
        // sticky note: soft drop shadow, paper, faint inner edge and the creation date
        let b = local_bounds(element);
        let paper: Vec<Pos2> = sticky_outline(b, element.style.round, 0.0).iter().map(to).collect();
        let shadow = sticky_outline(b, element.style.round, 3.0).iter().map(to).collect();
        let alpha = opacity as f32 / 100.0;
        painter.add(Shape::convex_polygon(shadow, Color32::from_black_alpha((0.16 * alpha * 255.0) as u8), Stroke::NONE));
        let paper_color = themed(with_opacity(element.style.fill.unwrap_or([0xff, 0xdf, 0x6b, 255]), opacity), view.dark);
        painter.add(Shape::convex_polygon(paper.clone(), paper_color, Stroke::new(0.5 * view.zoom, Color32::from_black_alpha((0.08 * alpha * 255.0) as u8))));
        if b.width() >= 75.0 && b.height() >= 75.0 {
            if let Some(date) = sticky_date(created, b.width() - 32.0 < 80.0) {
                let at = to(&[b.max.x - 16.0, b.max.y - 11.0]);
                painter.text(at, egui::Align2::RIGHT_BOTTOM, date, egui::FontId::proportional(12.0 * view.zoom), stroke_color);
            }
        }
        return;
    }
    if let (Some(fill), Some(poly)) = (element.style.fill, shape_polygon(element)) {
        let fill = themed(with_opacity(fill, opacity), view.dark);
        if element.style.fill_style == FillStyle::Solid {
            painter.add(Shape::convex_polygon(poly.iter().map(to).collect(), fill, Stroke::NONE));
        } else {
            let s = Stroke::new((element.style.stroke_width / 2.0).max(0.5) * view.zoom, fill);
            for line in fill_strokes(element) {
                painter.add(Shape::line(line.iter().map(to).collect(), s));
            }
        }
    }
    let width = if matches!(element.kind, ElementKind::FreeDraw { .. }) {
        element.style.stroke_width * 1.5
    } else {
        element.style.stroke_width
    };
    let stroke = Stroke::new(width * view.zoom, stroke_color);
    for outline in rough_outlines(element) {
        if outline.len() < 2 {
            continue;
        }
        let pts: Vec<Pos2> = outline.iter().map(to).collect();
        let sw = element.style.stroke_width;
        match element.style.stroke_style {
            StrokeStyle::Solid => {
                painter.add(Shape::line(pts, stroke));
            }
            StrokeStyle::Dashed => painter.extend(Shape::dashed_line(&pts, stroke, 8.0 * view.zoom, (8.0 + sw) * view.zoom)),
            StrokeStyle::Dotted => painter.extend(Shape::dashed_line(&pts, stroke, 1.5 * view.zoom, (6.0 + sw) * view.zoom)),
        }
    }
    for (shape, head) in arrowheads(element) {
        let pts: Vec<Pos2> = shape.iter().map(to).collect();
        match head.fill() {
            Some(solid) => {
                let fill = if solid { stroke_color } else { view.background };
                painter.add(Shape::convex_polygon(pts, fill, stroke));
            }
            None => {
                painter.add(Shape::line(pts, stroke));
            }
        }
    }
    if let (Some(name), ElementKind::Rectangle { x, y, .. }) = (&element.frame, &element.kind) {
        if !name.is_empty() {
            let color = themed(Color32::from_rgb(0x99, 0x99, 0x99), view.dark);
            painter.text(to(&[*x, y - 4.0]), egui::Align2::LEFT_BOTTOM, name, FontId::proportional(13.0 * view.zoom.max(0.5)), color);
        }
    }
    if let (ElementKind::Text { x, y, value }, false) = (&element.kind, hide_text) {
        let size = element.style.font_size;
        let font = FontId::new(size * view.zoom, egui::FontFamily::Name(HAND_FONT.into()));
        let width = local_bounds(element).width() * view.zoom;
        for (i, line) in text_lines(value).into_iter().enumerate() {
            let galley = painter.layout_no_wrap(line.to_string(), font.clone(), stroke_color);
            // labels are centered like the reference app's container text
            let indent = if is_label(element) { (width - galley.size().x).max(0.0) / 2.0 / view.zoom } else { 0.0 };
            let pos = to(&[*x + indent, y + i as f32 * size * LINE_HEIGHT]);
            if halo {
                // background-colored outline keeps text readable over lines
                let ring = (size * view.zoom * 0.08).clamp(1.0, 3.0);
                for k in 0..12 {
                    let offset = Vec2::angled(k as f32 / 12.0 * std::f32::consts::TAU) * ring;
                    let shadow = painter.layout_no_wrap(line.to_string(), font.clone(), view.background);
                    painter.add(egui::epaint::TextShape::new(pos + offset, shadow, view.background).with_angle(angle));
                }
            }
            painter.add(egui::epaint::TextShape::new(pos, galley, stroke_color).with_angle(angle));
        }
    }
}

fn distance_to_segment(point: [f32; 2], start: [f32; 2], end: [f32; 2]) -> f32 {
    let v = [end[0] - start[0], end[1] - start[1]];
    let w = [point[0] - start[0], point[1] - start[1]];
    let len_sq = v[0] * v[0] + v[1] * v[1];
    if len_sq == 0.0 {
        return ((point[0] - start[0]).powi(2) + (point[1] - start[1]).powi(2)).sqrt();
    }
    let t = ((w[0] * v[0] + w[1] * v[1]) / len_sq).clamp(0.0, 1.0);
    ((point[0] - (start[0] + t * v[0])).powi(2) + (point[1] - (start[1] + t * v[1])).powi(2)).sqrt()
}

fn polygon_contains(poly: &[[f32; 2]], p: [f32; 2]) -> bool {
    let mut inside = false;
    let n = poly.len();
    for i in 0..n {
        let (a, b) = (poly[i], poly[(i + n - 1) % n]);
        if (a[1] > p[1]) != (b[1] > p[1]) && p[0] < (b[0] - a[0]) * (p[1] - a[1]) / (b[1] - a[1]) + a[0] {
            inside = !inside;
        }
    }
    inside
}

/// the reference app's adaptive corner radius (packages/common/src/constants.ts ROUNDNESS).
fn corner_radius(size: f32) -> f32 {
    if size <= 128.0 { size * 0.25 } else { 32.0 }
}

fn box_corners(element: &Element) -> Option<(Vec<[f32; 2]>, f32)> {
    match &element.kind {
        ElementKind::Rectangle { x, y, width: w, height: h } => Some((
            vec![[*x, *y], [x + w, *y], [x + w, y + h], [*x, y + h]],
            // frames always have the web's fixed 8px corner
            if element.frame.is_some() { 8.0 } else if element.style.round { corner_radius(w.min(*h)) } else { 0.0 },
        )),
        ElementKind::Diamond { x, y, width: w, height: h } => Some((
            vec![[x + w / 2.0, *y], [x + w, y + h / 2.0], [x + w / 2.0, y + h], [*x, y + h / 2.0]],
            if element.style.round { corner_radius(w.min(*h)) * 0.5 } else { 0.0 },
        )),
        _ => None,
    }
}

/// Points a corner with radius `r` is cut at: (toward previous, toward next).
fn corner_cut(prev: [f32; 2], c: [f32; 2], next: [f32; 2], r: f32) -> ([f32; 2], [f32; 2]) {
    let toward = |o: [f32; 2]| {
        let (dx, dy) = (o[0] - c[0], o[1] - c[1]);
        let len = (dx * dx + dy * dy).sqrt().max(0.001);
        let r = r.min(len / 2.0);
        [c[0] + dx / len * r, c[1] + dy / len * r]
    };
    (toward(prev), toward(next))
}

fn quad(a: [f32; 2], c: [f32; 2], b: [f32; 2], steps: usize) -> Vec<[f32; 2]> {
    (0..=steps)
        .map(|i| {
            let t = i as f32 / steps as f32;
            let u = 1.0 - t;
            [u * u * a[0] + 2.0 * u * t * c[0] + t * t * b[0], u * u * a[1] + 2.0 * u * t * c[1] + t * t * b[1]]
        })
        .collect()
}

/// Closed outline (rounded where needed) used for fills and hit testing.
fn shape_polygon(element: &Element) -> Option<Vec<[f32; 2]>> {
    if let ElementKind::Ellipse { x, y, width: w, height: h } = &element.kind {
        let (cx, cy) = (x + w / 2.0, y + h / 2.0);
        return Some(
            (0..48)
                .map(|i| {
                    let t = i as f32 / 48.0 * std::f32::consts::TAU;
                    [cx + w / 2.0 * t.cos(), cy + h / 2.0 * t.sin()]
                })
                .collect(),
        );
    }
    let (corners, r) = box_corners(element)?;
    if r <= 0.0 {
        return Some(corners);
    }
    let n = corners.len();
    let mut out = Vec::new();
    for i in 0..n {
        let (a, b) = corner_cut(corners[(i + n - 1) % n], corners[i], corners[(i + 1) % n], r);
        out.extend(quad(a, corners[i], b, 6));
    }
    Some(out)
}

/// Deterministic jitter seeded by element id, so the sketch doesn't wobble between frames.
struct Rng(u64);

impl Rng {
    fn new(seed: &str, pass: u64) -> Self {
        let hash = seed.bytes().fold(0xcbf29ce484222325_u64, |h, b| (h ^ b as u64).wrapping_mul(0x100000001b3));
        Self((hash ^ pass.wrapping_mul(0x9e3779b97f4a7c15)) | 1)
    }

    /// Uniform in [-1, 1].
    fn next(&mut self) -> f32 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        (self.0 >> 40) as f32 / (1u64 << 23) as f32 - 1.0
    }
}

fn catmull_rom(points: &[[f32; 2]], steps: usize) -> Vec<[f32; 2]> {
    let mut out = vec![points[1]];
    for i in 1..points.len() - 2 {
        let (p0, p1, p2, p3) = (points[i - 1], points[i], points[i + 1], points[i + 2]);
        for step in 1..=steps {
            let t = step as f32 / steps as f32;
            let (t2, t3) = (t * t, t * t * t);
            let f = |a: f32, b: f32, c: f32, d: f32| {
                0.5 * (2.0 * b + (c - a) * t + (2.0 * a - 5.0 * b + 4.0 * c - d) * t2 + (3.0 * b - a - 3.0 * c + d) * t3)
            };
            out.push([f(p0[0], p1[0], p2[0], p3[0]), f(p0[1], p1[1], p2[1], p3[1])]);
        }
    }
    out
}

/// One rough.js-style stroke: jittered endpoints and a slight bow, scaled by roughness.
fn rough_line(a: [f32; 2], b: [f32; 2], rng: &mut Rng, roughness: f32) -> Vec<[f32; 2]> {
    let (dx, dy) = (b[0] - a[0], b[1] - a[1]);
    let len = (dx * dx + dy * dy).sqrt().max(0.001);
    let off = (len / 10.0).min(1.5) * roughness;
    let bow = (len / 100.0).min(4.0) * roughness;
    let (nx, ny) = (-dy / len, dx / len);
    let mut at = |t: f32, bend: f32| {
        let k = bend * rng.next();
        [a[0] + dx * t + nx * k + off * rng.next(), a[1] + dy * t + ny * k + off * rng.next()]
    };
    let (p0, m1, m2, p3) = (at(0.0, 0.0), at(0.5, bow), at(0.75, bow), at(1.0, 0.0));
    catmull_rom(&[p0, p0, m1, m2, p3, p3], 6)
}

/// Hand-drawn outline in world coordinates, shared by the canvas and the SVG/PNG export.
/// How long the agent's cursor stays after it finishes drawing (seconds).
const AGENT_LINGER: f64 = 1.5;
const AGENT_COLOR: Color32 = Color32::from_rgb(0xe8, 0x59, 0x0c);

/// Seconds to replay one AI-drawn element: strokes by their length, text by its letters.
fn reveal_duration(e: &Element) -> f64 {
    match &e.kind {
        ElementKind::Text { value, .. } => (value.chars().count() as f64 * 0.035).clamp(0.25, 1.0),
        _ if e.image.is_some() || e.created.is_some() || e.frame.is_some() => 0.35,
        _ => {
            let length: f32 = rough_outlines(e).iter().flat_map(|o| o.windows(2).map(|w| dist(w[0], w[1]))).sum();
            (length as f64 / 1400.0).clamp(0.3, 1.0)
        }
    }
}

/// Where the cursor rests once `e` is drawn (world).
fn reveal_pen(e: &Element) -> Pos2 {
    match &e.kind {
        ElementKind::Text { .. } => local_bounds(e).right_center(),
        ElementKind::Line { points, .. } | ElementKind::FreeDraw { points } if !points.is_empty() => {
            let last = points[points.len() - 1];
            Pos2::new(last[0], last[1])
        }
        _ => element_bounds(e).right_bottom(),
    }
}

/// Draws `e` partway, the way a hand would: strokes along their outline, then fill and
/// arrowheads fade in; text is typed; notes, images and frames fade in. Returns the pen
/// position on screen.
fn paint_partial(painter: &Painter, e: &Element, view: &View, progress: f32) -> Pos2 {
    let (center, angle) = (e.center(), e.angle);
    let to = |p: [f32; 2]| {
        let p = if angle == 0.0 { Pos2::new(p[0], p[1]) } else { center + rot(Pos2::new(p[0], p[1]) - center, angle) };
        Pos2::new(view.origin.x + p.x * view.zoom, view.origin.y + p.y * view.zoom)
    };
    let mut part = e.clone();
    match &mut part.kind {
        ElementKind::Text { value, .. } => {
            let n = (value.chars().count() as f32 * progress).ceil() as usize;
            *value = value.chars().take(n).collect();
            paint_element(painter, &part, view, false, false, false);
            let b = local_bounds(&part);
            return to([b.max.x, b.center().y]);
        }
        _ if e.image.is_some() || e.created.is_some() || e.frame.is_some() => {
            part.style.opacity = (e.style.opacity as f32 * progress) as u8;
            paint_element(painter, &part, view, false, false, false);
            let b = local_bounds(e);
            return to([b.max.x, b.max.y]);
        }
        _ => {}
    }
    let outlines = rough_outlines(e);
    let total: f32 = outlines.iter().flat_map(|o| o.windows(2).map(|w| dist(w[0], w[1]))).sum();
    let mut budget = total * (progress / 0.8).min(1.0);
    let width = if matches!(e.kind, ElementKind::FreeDraw { .. }) { e.style.stroke_width * 1.5 } else { e.style.stroke_width };
    let stroke = Stroke::new(width * view.zoom, themed(with_opacity(e.style.stroke, e.style.opacity), view.dark));
    let mut pen = to(outlines.first().and_then(|o| o.first()).copied().unwrap_or([center.x, center.y]));
    'outer: for outline in &outlines {
        for w in outline.windows(2) {
            if budget <= 0.0 {
                break 'outer;
            }
            let seg = dist(w[0], w[1]);
            let t = if seg > 0.0 { (budget / seg).min(1.0) } else { 1.0 };
            let end = [w[0][0] + (w[1][0] - w[0][0]) * t, w[0][1] + (w[1][1] - w[0][1]) * t];
            painter.line_segment([to(w[0]), to(end)], stroke);
            pen = to(end);
            budget -= seg;
        }
    }
    if progress > 0.8 {
        part.style.opacity = (e.style.opacity as f32 * (progress - 0.8) / 0.2) as u8;
        paint_element(painter, &part, view, false, false, false);
    }
    pen
}

/// A collaborator-style pointer with the agent's name in a pill beside it.
fn paint_agent_cursor(painter: &Painter, at: Pos2, name: &str, alpha: f32) {
    let color = AGENT_COLOR.gamma_multiply(alpha);
    let arrow = [at, at + Vec2::new(0.0, 16.0), at + Vec2::new(4.5, 12.0), at + Vec2::new(11.0, 11.5)];
    painter.add(Shape::convex_polygon(arrow.to_vec(), color, Stroke::new(1.5, Color32::WHITE.gamma_multiply(alpha))));
    let galley = painter.layout_no_wrap(name.to_string(), egui::FontId::proportional(12.0), Color32::WHITE.gamma_multiply(alpha));
    let pill = Rect::from_min_size(at + Vec2::new(12.0, 16.0), galley.size() + Vec2::new(12.0, 6.0));
    painter.rect_filled(pill, 6, color);
    painter.galley(pill.min + Vec2::new(6.0, 3.0), galley, Color32::WHITE);
}

/// Decoded image textures, keyed by a hash of the data URL so the same picture uploads once.
const IMAGE_EXTENSIONS: [&str; 7] = ["png", "jpg", "jpeg", "jfif", "gif", "webp", "bmp"];

fn image_texture(ctx: &Context, url: &str) -> Option<egui::TextureHandle> {
    use std::hash::{Hash, Hasher};
    thread_local! {
        static CACHE: std::cell::RefCell<std::collections::HashMap<u64, Option<egui::TextureHandle>>> = Default::default();
    }
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    url.hash(&mut hasher);
    let key = hasher.finish();
    CACHE.with_borrow_mut(|cache| {
        cache
            .entry(key)
            .or_insert_with(|| {
                let bytes = base64_decode(url.split_once(";base64,")?.1)?;
                let rgba = image::load_from_memory(&bytes).ok()?.to_rgba8();
                let image = egui::ColorImage::from_rgba_unmultiplied([rgba.width() as usize, rgba.height() as usize], &rgba);
                Some(ctx.load_texture(format!("image-{key}"), image, egui::TextureOptions::LINEAR))
            })
            .clone()
    })
}

fn base64_decode(text: &str) -> Option<Vec<u8>> {
    let value = |c: u8| match c {
        b'A'..=b'Z' => Some(c - b'A'),
        b'a'..=b'z' => Some(c - b'a' + 26),
        b'0'..=b'9' => Some(c - b'0' + 52),
        b'+' | b'-' => Some(62),
        b'/' | b'_' => Some(63),
        _ => None,
    };
    let digits: Vec<u8> = text.bytes().filter(|c| !c.is_ascii_whitespace() && *c != b'=').map(value).collect::<Option<_>>()?;
    let mut out = Vec::with_capacity(digits.len() * 3 / 4);
    for chunk in digits.chunks(4) {
        let n = chunk.iter().enumerate().fold(0u32, |n, (i, d)| n | (*d as u32) << (18 - 6 * i));
        out.extend(n.to_be_bytes()[1..chunk.len()].iter());
    }
    Some(out)
}

fn rough_outlines(element: &Element) -> Vec<Vec<[f32; 2]>> {
    let roughness = element.style.roughness as f32;
    let passes = if roughness == 0.0 || element.style.stroke_style != StrokeStyle::Solid { 1 } else { 2 };
    let mut out = Vec::new();
    if let Some((corners, r)) = box_corners(element) {
        let n = corners.len();
        for pass in 0..passes {
            let mut rng = Rng::new(&element.id, pass);
            let cuts: Vec<_> = (0..n)
                .map(|i| corner_cut(corners[(i + n - 1) % n], corners[i], corners[(i + 1) % n], r))
                .collect();
            if r <= 0.0 {
                // sharp corners: separate overshooting strokes, like rough.js
                for i in 0..n {
                    out.push(rough_line(corners[i], corners[(i + 1) % n], &mut rng, roughness));
                }
                continue;
            }
            // rounded: one continuous path per pass (arc, edge, arc, edge...)
            let mut path = Vec::new();
            for i in 0..n {
                let j = (i + 1) % n;
                let wobble = 0.6 * roughness;
                let c = [corners[i][0] + wobble * rng.next(), corners[i][1] + wobble * rng.next()];
                path.extend(quad(cuts[i].0, c, cuts[i].1, 6));
                let mut edge = rough_line(cuts[i].1, cuts[j].0, &mut rng, roughness);
                let last = edge.len() - 1;
                (edge[0], edge[last]) = (cuts[i].1, cuts[j].0);
                path.extend(&edge[1..last]);
            }
            path.push(path[0]);
            out.push(path);
        }
        return out;
    }
    match &element.kind {
        ElementKind::Ellipse { x, y, width: w, height: h } => {
            let (cx, cy) = (x + w / 2.0, y + h / 2.0);
            for pass in 0..passes {
                let mut rng = Rng::new(&element.id, pass);
                let n = 10;
                let start = rng.next() * std::f32::consts::PI;
                let ctl: Vec<[f32; 2]> = (0..n)
                    .map(|i| {
                        let t = start + i as f32 / n as f32 * std::f32::consts::TAU;
                        let k = 1.0 + 0.025 * roughness * rng.next();
                        [
                            cx + w / 2.0 * k * t.cos() + roughness * rng.next(),
                            cy + h / 2.0 * k * t.sin() + roughness * rng.next(),
                        ]
                    })
                    .collect();
                // ponytail: closes with a fixed overlap; rough.js also varies overlap per pass
                let mut ring = vec![ctl[n - 1]];
                ring.extend(&ctl);
                ring.extend([ctl[0], ctl[1], if roughness > 0.0 { ctl[2] } else { ctl[1] }]);
                out.push(catmull_rom(&ring, 8));
            }
        }
        ElementKind::Line { points, .. } => {
            let curved = element.style.round && points.len() > 2;
            let path = line_path(element);
            for pass in 0..passes {
                let mut rng = Rng::new(&element.id, pass);
                if curved {
                    // one wobbly pass along the smooth curve instead of per-segment strokes
                    let wobble = roughness * 0.8;
                    let (dx, dy) = (wobble * rng.next(), wobble * rng.next());
                    out.push(path.iter().map(|p| [p[0] + dx, p[1] + dy]).collect());
                } else {
                    for pair in points.windows(2) {
                        out.push(rough_line(pair[0], pair[1], &mut rng, roughness));
                    }
                }
            }
        }
        ElementKind::FreeDraw { points } => out.push(points.clone()),
        _ => {}
    }
    out
}

/// Arrowhead outlines at both ends (getArrowheadPoints: size 25/15/12, angle 20°/25°/90°).
fn arrowheads(element: &Element) -> Vec<(Vec<[f32; 2]>, Head)> {
    let ElementKind::Line { points, arrow: true } = &element.kind else {
        return Vec::new();
    };
    if points.len() < 2 {
        return Vec::new();
    }
    let path = line_path(element);
    let step = if path.len() != points.len() { 3 } else { 1 }; // a little back along a curve
    let n = points.len();
    let v = |p: [f32; 2]| Pos2::new(p[0], p[1]);
    let mut out = Vec::new();
    for (head, at_end) in [(element.ends.start_head, false), (element.ends.end_head, true)] {
        let Some(head) = head else {
            continue;
        };
        let (tip, from, segment) = if at_end {
            (v(path[path.len() - 1]), v(path[path.len() - 1 - step]), v(points[n - 1]) - v(points[n - 2]))
        } else {
            (v(path[0]), v(path[step]), v(points[0]) - v(points[1]))
        };
        let dir = (tip - from).normalized();
        let diamond = matches!(head, Head::Diamond | Head::DiamondOutline);
        let size = match head {
            Head::Arrow => 25.0,
            _ if diamond => 12.0,
            _ => 15.0,
        };
        let size = f32::min(size, segment.length() * if diamond { 0.25 } else { 0.5 });
        let angle = match head {
            Head::Arrow => 20.0_f32,
            Head::Bar => 90.0,
            _ => 25.0,
        }
        .to_radians();
        let back = tip - dir * size;
        let side = |a: f32| tip + rot(back - tip, a);
        let pts: Vec<Pos2> = match head {
            Head::Arrow => vec![side(-angle), tip, side(angle)],
            Head::Bar => vec![side(-angle), side(angle)],
            Head::Triangle | Head::TriangleOutline => vec![tip, side(-angle), side(angle)],
            Head::Diamond | Head::DiamondOutline => vec![tip, side(-angle), tip - dir * size * 2.0, side(angle)],
            Head::Circle | Head::CircleOutline => {
                let r = (size + element.style.stroke_width - 2.0).max(2.0) / 2.0;
                let c = tip - dir * r;
                (0..20).map(|k| c + Vec2::angled(k as f32 / 20.0 * std::f32::consts::TAU) * r).collect()
            }
        };
        out.push((pts.iter().map(|p| [p.x, p.y]).collect(), head));
    }
    out
}

fn sticky_style(style: Style) -> Style {
    Style { stroke: BLACK, fill: Some([0xff, 0xdf, 0x6b, 255]), fill_style: FillStyle::Solid, roughness: 1, round: true, ..style }
}

fn now_ms() -> i64 {
    SystemTime::now().duration_since(SystemTime::UNIX_EPOCH).map_or(0, |d| d.as_millis() as i64)
}

/// (year, month 1-12, day) of a UTC day count, Howard Hinnant's civil_from_days.
fn civil_date(ms: i64) -> (i64, usize, i64) {
    let z = ms.div_euclid(86_400_000) + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let month = if mp < 10 { mp + 3 } else { mp - 9 };
    (yoe + era * 400 + i64::from(month <= 2), month as usize, doy - (153 * mp + 2) / 5 + 1)
}

/// The note's creation-date footer: "20 Oct", plus the year when it isn't this one.
// ponytail: UTC day, not the local time zone; add a tz crate if notes made near midnight matter.
fn sticky_date(created: i64, short: bool) -> Option<String> {
    const MONTHS: [&str; 12] = ["Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec"];
    if created <= 0 {
        return None;
    }
    let (year, month, day) = civil_date(created);
    let label = format!("{day} {}", MONTHS[month - 1]);
    Some(if short || year == civil_date(now_ms()).0 { label } else { format!("{label} {year}") })
}

/// Rounded-rectangle outline, corner radius 4% of the short side up to 16 (sticky notes).
fn sticky_outline(b: Rect, round: bool, shift: f32) -> Vec<[f32; 2]> {
    let r = if round { (b.width().min(b.height()) * 0.04).min(16.0) } else { 0.0 };
    let b = b.translate(Vec2::splat(shift));
    let corners = [(b.right_bottom(), 0.0), (b.left_bottom(), 0.25), (b.left_top(), 0.5), (b.right_top(), 0.75)];
    corners
        .into_iter()
        .flat_map(|(c, turn)| {
            let center = c + Vec2::new(if turn == 0.0 || turn == 0.75 { -r } else { r }, if turn < 0.5 { -r } else { r });
            (0..=6).map(move |k| {
                let a = (turn + k as f32 / 24.0) * std::f32::consts::TAU;
                [center.x + r * a.cos(), center.y + r * a.sin()]
            })
        })
        .collect()
}

#[derive(Clone, Copy, Debug, PartialEq)]
enum Recognized {
    Rectangle,
    Diamond,
    Ellipse,
    Line,
    Arrow,
}

/// The web's moment-based shape recognizer (convertToShape.ts): closed strokes are matched
/// against rectangle/diamond/ellipse prototypes, open straight ones become lines, or arrows
/// when the arrowhead makes them lopsided. None means "not a shape".
fn classify_stroke(points: &[[f32; 2]], zoom: f32) -> Option<Recognized> {
    let (min, max) = points.iter().fold(([f32::MAX; 2], [f32::MIN; 2]), |(lo, hi), p| {
        ([lo[0].min(p[0]), lo[1].min(p[1])], [hi[0].max(p[0]), hi[1].max(p[1])])
    });
    if points.len() < 3 || (max[0] - min[0]).max(max[1] - min[1]) * zoom < 25.0 {
        return None;
    }
    let pts = resample(points, 64);
    let path: f32 = pts.windows(2).map(|w| dist(w[0], w[1])).sum();
    let gap_ratio = if path > 0.0 { dist(pts[0], pts[63]) / path } else { 0.0 };
    let moments = |values: &[f32], order: i32| {
        let n = values.len() as f32;
        let mean = values.iter().sum::<f32>() / n;
        let var = values.iter().map(|v| (v - mean).powi(2)).sum::<f32>() / n;
        let m = values.iter().map(|v| (v - mean).powi(order)).sum::<f32>() / n;
        if var.sqrt() > f32::EPSILON { m / var.sqrt().powi(order) } else { 0.0 }
    };
    if gap_ratio > 0.15 {
        // open: principal axes of the point cloud
        let n = pts.len() as f32;
        let c = [pts.iter().map(|p| p[0]).sum::<f32>() / n, pts.iter().map(|p| p[1]).sum::<f32>() / n];
        let (mut m20, mut m02, mut m11) = (0.0, 0.0, 0.0);
        for p in &pts {
            let (dx, dy) = (p[0] - c[0], p[1] - c[1]);
            (m20, m02, m11) = (m20 + dx * dx / n, m02 + dy * dy / n, m11 + dx * dy / n);
        }
        let diff = (m20 - m02).hypot(2.0 * m11);
        let (major_var, minor_var) = ((m20 + m02 + diff) / 2.0, (m20 + m02 - diff) / 2.0);
        let major = if m11.abs() > f32::EPSILON { Vec2::new(major_var - m02, m11).normalized() } else if m20 >= m02 { Vec2::X } else { Vec2::Y };
        let elongation = if major_var > 0.0 { minor_var / major_var } else { 1.0 };
        let u: Vec<f32> = pts.iter().map(|p| (p[0] - c[0]) * major.x + (p[1] - c[1]) * major.y).collect();
        // the shaft runs from the start to the farthest point; the arrowhead zone around it is exempt
        let tip = pts.iter().copied().max_by(|a, b| dist(pts[0], *a).total_cmp(&dist(pts[0], *b)))?;
        let tip_distance = dist(pts[0], tip);
        let shaft = pts
            .iter()
            .filter(|p| dist(**p, tip) > 0.5 * tip_distance)
            .map(|p| distance_to_segment(*p, pts[0], tip))
            .fold(0.0, f32::max);
        if elongation > 0.25 || (tip_distance > 0.0 && shaft / tip_distance > 0.15) {
            return None;
        }
        return Some(if moments(&u, 3).abs() >= 0.3 { Recognized::Arrow } else { Recognized::Line });
    }
    // closed: hull fill, corner sharpness and the kurtosis product against each prototype
    let box_area = (max[0] - min[0]) * (max[1] - min[1]);
    let hull = convex_hull(&pts);
    let hull_area = (0..hull.len()).map(|i| {
        let (a, b) = (hull[i], hull[(i + 1) % hull.len()]);
        a[0] * b[1] - b[0] * a[1]
    }).sum::<f32>().abs() / 2.0;
    let hull_fill = if box_area > 0.0 { hull_area / box_area } else { 0.0 };
    let turns: Vec<f32> = (3..pts.len() - 3)
        .map(|i| {
            let (a, b, c) = (pts[i - 3], pts[i], pts[i + 3]);
            let (v1, v2) = ([b[0] - a[0], b[1] - a[1]], [c[0] - b[0], c[1] - b[1]]);
            (v1[0] * v2[1] - v1[1] * v2[0]).atan2(v1[0] * v2[0] + v1[1] * v2[1]).abs()
        })
        .collect();
    let total: f32 = turns.iter().sum();
    let mut taken = vec![false; turns.len()];
    let mut top4 = 0.0;
    for _ in 0..4 {
        let Some(peak) = (0..turns.len()).filter(|i| !taken[*i] && turns[*i] > 0.0).max_by(|a, b| turns[*a].total_cmp(&turns[*b])) else {
            break;
        };
        for i in peak.saturating_sub(3)..=(peak + 3).min(turns.len() - 1) {
            if !taken[i] {
                top4 += turns[i];
                taken[i] = true;
            }
        }
    }
    let corner_share = if total > 0.0 { top4 / total } else { 0.0 };
    let xs: Vec<f32> = pts.iter().map(|p| p[0]).collect();
    let ys: Vec<f32> = pts.iter().map(|p| p[1]).collect();
    let kurtosis = moments(&xs, 4) * moments(&ys, 4);
    [
        (Recognized::Rectangle, 1.0, 0.95, 1.83),
        (Recognized::Diamond, 0.5, 0.95, 3.24),
        (Recognized::Ellipse, std::f32::consts::FRAC_PI_4, 0.55, 2.25),
    ]
    .into_iter()
    .map(|(shape, fill, corners, kurt)| {
        let d = ((hull_fill - fill) / 0.2).hypot((corner_share - corners) / 0.2).hypot((kurtosis - kurt) / 0.7);
        (shape, d)
    })
    .filter(|(_, d)| *d < 1.5)
    .min_by(|a, b| a.1.total_cmp(&b.1))
    .map(|(shape, _)| shape)
}

fn dist(a: [f32; 2], b: [f32; 2]) -> f32 {
    (a[0] - b[0]).hypot(a[1] - b[1])
}

/// `n` points evenly spaced along the stroke.
fn resample(pts: &[[f32; 2]], n: usize) -> Vec<[f32; 2]> {
    let total: f32 = pts.windows(2).map(|w| dist(w[0], w[1])).sum();
    let step = total / (n - 1) as f32;
    let mut out = vec![pts[0]];
    let (mut prev, mut carried) = (pts[0], 0.0);
    for &p in &pts[1..] {
        let mut seg = dist(prev, p);
        while step > 0.0 && carried + seg >= step && out.len() < n {
            let t = (step - carried) / seg;
            prev = [prev[0] + t * (p[0] - prev[0]), prev[1] + t * (p[1] - prev[1])];
            out.push(prev);
            seg = dist(prev, p);
            carried = 0.0;
        }
        carried += seg;
        prev = p;
    }
    out.resize(n, pts[pts.len() - 1]);
    out
}

/// Andrew's monotone chain.
fn convex_hull(points: &[[f32; 2]]) -> Vec<[f32; 2]> {
    let mut sorted = points.to_vec();
    sorted.sort_by(|a, b| a[0].total_cmp(&b[0]).then(a[1].total_cmp(&b[1])));
    let cross = |o: [f32; 2], a: [f32; 2], b: [f32; 2]| (a[0] - o[0]) * (b[1] - o[1]) - (a[1] - o[1]) * (b[0] - o[0]);
    let half = |pts: &mut dyn Iterator<Item = [f32; 2]>| {
        let mut chain: Vec<[f32; 2]> = Vec::new();
        for p in pts {
            while chain.len() >= 2 && cross(chain[chain.len() - 2], chain[chain.len() - 1], p) <= 0.0 {
                chain.pop();
            }
            chain.push(p);
        }
        chain.pop();
        chain
    };
    let mut hull = half(&mut sorted.iter().copied());
    hull.extend(half(&mut sorted.iter().rev().copied()));
    if hull.len() >= 3 { hull } else { points.to_vec() }
}

/// Desenhar para criar forma: the element a stroke becomes, or None (the stroke just fades).
/// A line from or to a shape is a connector, so it becomes a bound arrow.
fn recognize_shape(app: &DrawingApp, points: &[[f32; 2]]) -> Option<Element> {
    let shape = classify_stroke(points, app.zoom)?;
    let (min, max) = points.iter().fold(([f32::MAX; 2], [f32::MIN; 2]), |(lo, hi), p| {
        ([lo[0].min(p[0]), lo[1].min(p[1])], [hi[0].max(p[0]), hi[1].max(p[1])])
    });
    let (x, y, width, height) = (min[0], min[1], max[0] - min[0], max[1] - min[1]);
    let start = points[0];
    let kind = match shape {
        Recognized::Rectangle => ElementKind::Rectangle { x, y, width, height },
        Recognized::Diamond => ElementKind::Diamond { x, y, width, height },
        Recognized::Ellipse => ElementKind::Ellipse { x, y, width, height },
        Recognized::Line => ElementKind::Line { points: vec![start, points[points.len() - 1]], arrow: false },
        Recognized::Arrow => {
            // the tip is the input point nearest the bounding-box spot farthest from the start
            let spots = [[x, y], [x + width / 2.0, y], [x + width, y], [x + width, y + height / 2.0], [x + width, y + height], [x + width / 2.0, y + height], [x, y + height], [x, y + height / 2.0]];
            let ideal = spots.into_iter().max_by(|a, b| dist(*a, start).total_cmp(&dist(*b, start)))?;
            let tip = points.iter().copied().min_by(|a, b| dist(*a, ideal).total_cmp(&dist(*b, ideal)))?;
            ElementKind::Line { points: vec![start, tip], arrow: dist(start, tip) >= 60.0 }
        }
    };
    let mut element = app.new_element(kind);
    if let ElementKind::Line { points, arrow } = &mut element.kind {
        let ends = [Pos2::new(points[0][0], points[0][1]), Pos2::new(points[1][0], points[1][1])];
        let targets = ends.map(|p| app.binding_target(p, None));
        if !*arrow && targets.iter().any(Option::is_some) && targets[0] != targets[1] {
            *arrow = true;
            element.ends.start_head = None;
        }
        if *arrow {
            return Some(app.bind_new_arrow(element));
        }
    }
    Some(element)
}

fn bindable(e: &Element) -> bool {
    !matches!(e.kind, ElementKind::Line { .. } | ElementKind::FreeDraw { .. })
}

/// getBindingGap: BASE_BINDING_GAP (5) + half the target's stroke.
fn binding_gap(e: &Element) -> f32 {
    5.0 + e.style.stroke_width / 2.0
}

/// World-space outline of a bindable element, grown by `grow`.
fn binding_outline(e: &Element, grow: f32) -> Vec<[f32; 2]> {
    let b = local_bounds(e).expand(grow);
    let (x, y, width, height) = (b.min.x, b.min.y, b.width(), b.height());
    let mut shape = e.clone();
    shape.kind = match e.kind {
        ElementKind::Ellipse { .. } => ElementKind::Ellipse { x, y, width, height },
        ElementKind::Diamond { .. } => ElementKind::Diamond { x, y, width, height },
        _ => ElementKind::Rectangle { x, y, width, height },
    };
    if matches!(e.kind, ElementKind::Text { .. }) {
        shape.style.round = false;
    }
    let c = b.center();
    shape_polygon(&shape)
        .unwrap_or_default()
        .into_iter()
        .map(|p| {
            let q = c + rot(Pos2::new(p[0], p[1]) - c, e.angle);
            [q.x, q.y]
        })
        .collect()
}

fn binding_for(target: &Element, world: Pos2, inside: bool) -> Binding {
    let b = local_bounds(target);
    let local = target.to_local(world) - b.min;
    let ratio = |v: f32, size: f32| if size > 0.0 { (v / size).clamp(0.0, 1.0) } else { 0.5 };
    Binding { element_id: target.id.clone(), fixed: [ratio(local.x, b.width()), ratio(local.y, b.height())], inside }
}

fn focus_point(target: &Element, binding: &Binding) -> Pos2 {
    let b = local_bounds(target);
    let p = b.min + Vec2::new(binding.fixed[0] * b.width(), binding.fixed[1] * b.height());
    b.center() + rot(p - b.center(), target.angle)
}

fn segment_intersection(a: Pos2, b: Pos2, c: [f32; 2], d: [f32; 2]) -> Option<Pos2> {
    let (c, d) = (Pos2::new(c[0], c[1]), Pos2::new(d[0], d[1]));
    let (r, s) = (b - a, d - c);
    let denom = r.x * s.y - r.y * s.x;
    if denom.abs() < 1e-6 {
        return None;
    }
    let t = ((c - a).x * s.y - (c - a).y * s.x) / denom;
    let u = ((c - a).x * r.y - (c - a).y * r.x) / denom;
    ((0.0..=1.0).contains(&t) && (0.0..=1.0).contains(&u)).then(|| a + r * t)
}

/// updateBoundPoint: "orbit" stops on the gap-grown outline along the line toward `other`.
fn bound_endpoint(target: &Element, binding: &Binding, other: Pos2) -> Pos2 {
    let focus = focus_point(target, binding);
    if binding.inside {
        return focus;
    }
    let poly = binding_outline(target, binding_gap(target));
    let n = poly.len();
    (0..n)
        .filter_map(|k| segment_intersection(focus, other, poly[k], poly[(k + 1) % n]))
        .min_by(|a, b| a.distance(other).total_cmp(&b.distance(other)))
        .unwrap_or(focus)
}

/// Glues every bound arrow to its shapes (also used by the MCP server before saving).
fn resolve_bindings(elements: &mut [Element]) {
    for i in 0..elements.len() {
        if elements[i].ends.start.is_none() && elements[i].ends.end.is_none() {
            continue;
        }
        let mut arrow = elements[i].clone();
        if resolve_arrow(elements, &mut arrow) {
            elements[i] = arrow;
        }
    }
}

/// Text that sits on a line or arrow gets a background-colored outline, so it keeps its
/// contrast (the MCP diagrams put labels on arrows all the time).
fn text_needs_halo(elements: &[Element], text: &Element) -> bool {
    if !matches!(text.kind, ElementKind::Text { .. }) {
        return false;
    }
    if is_arrow_label(elements, text) {
        return true;
    }
    let area = local_bounds(text).expand(2.0);
    let edges = [
        (area.left_top(), area.right_top()),
        (area.right_top(), area.right_bottom()),
        (area.right_bottom(), area.left_bottom()),
        (area.left_bottom(), area.left_top()),
    ];
    elements.iter().filter(|e| e.id != text.id).any(|e| {
        let path = match &e.kind {
            ElementKind::Line { .. } => line_path(e),
            ElementKind::FreeDraw { points } => points.clone(),
            _ => return false,
        };
        path.windows(2).any(|w| {
            let (a, b) = (Pos2::new(w[0][0], w[0][1]), Pos2::new(w[1][0], w[1][1]));
            area.contains(a)
                || area.contains(b)
                || edges.iter().any(|(c, d)| segment_intersection(a, b, [c.x, c.y], [d.x, d.y]).is_some())
        })
    })
}

fn is_label(e: &Element) -> bool {
    matches!(e.kind, ElementKind::Text { .. }) && e.id.ends_with("-label")
}

fn is_arrow_label(elements: &[Element], e: &Element) -> bool {
    is_label(e)
        && e.id.strip_suffix("-label").is_some_and(|owner| {
            elements.iter().any(|o| o.id == owner && matches!(o.kind, ElementKind::Line { .. }))
        })
}

/// Labels (`<owner>-label` text, made by the MCP `label` shortcut) stay centered on their
/// shape, or on the middle of their arrow, whatever moved.
fn recenter_labels(elements: &mut [Element]) {
    for i in 0..elements.len() {
        let Some(owner) = elements[i].id.strip_suffix("-label") else {
            continue;
        };
        let Some(owner) = elements.iter().find(|e| e.id == owner && !matches!(e.kind, ElementKind::Text { .. })) else {
            continue;
        };
        let target = match &owner.kind {
            ElementKind::Line { .. } => {
                let path = line_path(owner);
                let mid = path[path.len() / 2];
                // 2-point paths have no middle sample: average the ends
                if path.len() == 2 {
                    Pos2::new((path[0][0] + path[1][0]) / 2.0, (path[0][1] + path[1][1]) / 2.0)
                } else {
                    Pos2::new(mid[0], mid[1])
                }
            }
            _ => element_bounds(owner).center(),
        };
        let delta = target - local_bounds(&elements[i]).center();
        if delta.length_sq() > 0.01 {
            elements[i].move_by(delta);
        }
    }
}

/// Recomputes a bound arrow's endpoints from its targets; drops bindings to missing shapes.
fn resolve_arrow(elements: &[Element], arrow: &mut Element) -> bool {
    let before = arrow.clone();
    let find = |b: &Option<Binding>| {
        b.as_ref().and_then(|b| elements.iter().find(|e| e.id == b.element_id && e.id != arrow.id).map(|e| (e, b.clone())))
    };
    let (start, end) = (find(&arrow.ends.start), find(&arrow.ends.end));
    if start.is_none() {
        arrow.ends.start = None;
    }
    if end.is_none() {
        arrow.ends.end = None;
    }
    if let ElementKind::Line { points, .. } = &mut arrow.kind {
        let n = points.len();
        if n >= 2 {
            let at = |k: usize| Pos2::new(points[k][0], points[k][1]);
            // a 2-point arrow aims at the other shape's anchor, longer ones at their neighbor point
            let start_aim = match (&end, n) {
                (Some((e, b)), 2) => focus_point(e, b),
                _ => at(1),
            };
            let end_aim = match (&start, n) {
                (Some((e, b)), 2) => focus_point(e, b),
                _ => at(n - 2),
            };
            if let Some((e, b)) = &start {
                let p = bound_endpoint(e, b, start_aim);
                points[0] = [p.x, p.y];
            }
            if let Some((e, b)) = &end {
                let p = bound_endpoint(e, b, end_aim);
                points[n - 1] = [p.x, p.y];
            }
        }
    }
    *arrow != before
}

/// Hachure / cross-hatch lines clipped to the shape (rough.js: gap = 4 × stroke, angle −41°).
fn fill_strokes(element: &Element) -> Vec<Vec<[f32; 2]>> {
    let (Some(poly), true) = (shape_polygon(element), element.style.fill_style != FillStyle::Solid) else {
        return Vec::new();
    };
    let gap = (element.style.stroke_width * 4.0).max(4.0);
    let mut angles = vec![-41.0_f32];
    if element.style.fill_style == FillStyle::CrossHatch {
        angles.push(49.0);
    }
    let mut rng = Rng::new(&element.id, 7);
    let mut out = Vec::new();
    for angle in angles {
        let (sin, cos) = angle.to_radians().sin_cos();
        let rot = |p: [f32; 2], s: f32| [p[0] * cos + p[1] * s * sin, -p[0] * s * sin + p[1] * cos];
        let rotated: Vec<[f32; 2]> = poly.iter().map(|p| rot(*p, 1.0)).collect();
        let (min_y, max_y) = rotated.iter().fold((f32::MAX, f32::MIN), |(lo, hi), p| (lo.min(p[1]), hi.max(p[1])));
        let mut y = min_y + gap / 2.0;
        while y < max_y {
            let n = rotated.len();
            let mut xs: Vec<f32> = (0..n)
                .filter_map(|i| {
                    let (a, b) = (rotated[i], rotated[(i + 1) % n]);
                    ((a[1] <= y) != (b[1] <= y)).then(|| a[0] + (y - a[1]) / (b[1] - a[1]) * (b[0] - a[0]))
                })
                .collect();
            xs.sort_by(f32::total_cmp);
            for pair in xs.chunks_exact(2) {
                let a = rot([pair[0], y], -1.0);
                let b = rot([pair[1], y], -1.0);
                out.push(rough_line(a, b, &mut rng, element.style.roughness as f32 * 0.5));
            }
            y += gap;
        }
    }
    out
}

/// Axis-aligned bounds in world space (rotation included).
fn element_bounds(e: &Element) -> Rect {
    let local = local_bounds(e);
    if e.angle == 0.0 {
        return local;
    }
    let c = local.center();
    [local.left_top(), local.right_top(), local.right_bottom(), local.left_bottom()]
        .map(|p| c + rot(p - c, e.angle))
        .into_iter()
        .fold(Rect::NOTHING, |r, p| r.union(Rect::from_min_max(p, p)))
}

/// Smooth curve through a line's points when it is curved (arrow type "curved").
fn line_path(e: &Element) -> Vec<[f32; 2]> {
    match &e.kind {
        ElementKind::Line { points, .. } if e.style.round && points.len() > 2 => {
            let mut ctl = vec![points[0]];
            ctl.extend(points);
            ctl.push(points[points.len() - 1]);
            catmull_rom(&ctl, LINE_CURVE_STEPS)
        }
        ElementKind::Line { points, .. } => points.clone(),
        _ => Vec::new(),
    }
}

/// Unrotated bounds of the element in its own frame.
fn local_bounds(e: &Element) -> Rect {
    match &e.kind {
        ElementKind::Rectangle { x, y, width, height }
        | ElementKind::Ellipse { x, y, width, height }
        | ElementKind::Diamond { x, y, width, height } => Rect::from_min_size(Pos2::new(*x, *y), Vec2::new(*width, *height)),
        ElementKind::Text { x, y, value } => {
            let size = e.style.font_size;
            let lines = text_lines(value);
            let longest = lines.iter().map(|l| l.chars().count()).max().unwrap_or(0) as f32;
            Rect::from_min_size(
                Pos2::new(*x, *y),
                Vec2::new((longest * size * CHAR_WIDTH).max(size * 0.5), lines.len() as f32 * size * LINE_HEIGHT),
            )
        }
        ElementKind::Line { points, .. } | ElementKind::FreeDraw { points } => points
            .iter()
            .map(|p| Rect::from_min_max(Pos2::new(p[0], p[1]), Pos2::new(p[0], p[1])))
            .reduce(|a, b| a.union(b))
            .unwrap_or(Rect::ZERO),
    }
}

// ---------- export ----------

fn hex(c: [u8; 4]) -> String {
    format!("#{:02x}{:02x}{:02x}", c[0], c[1], c[2])
}

fn svg_points(points: &[[f32; 2]], offset: [f32; 2]) -> String {
    points
        .iter()
        .map(|p| format!("{:.2},{:.2}", p[0] + offset[0], p[1] + offset[1]))
        .collect::<Vec<_>>()
        .join(" ")
}

fn scene_svg(scene: &Scene, background: Option<[u8; 4]>) -> String {
    let bounds = scene
        .elements
        .iter()
        // a frame's name sits above it
        .map(|e| if e.frame.is_some() { element_bounds(e).union(element_bounds(e).translate(Vec2::new(0.0, -22.0))) } else { element_bounds(e) })
        .reduce(|a, b| a.union(b))
        .unwrap_or(Rect::from_min_size(Pos2::ZERO, Vec2::new(160.0, 120.0)));
    let width = bounds.width().max(1.0) + 20.0;
    let height = bounds.height().max(1.0) + 20.0;
    let offset = [10.0 - bounds.min.x, 10.0 - bounds.min.y];
    let mut svg = format!(
        "<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"{width}\" height=\"{height}\" viewBox=\"0 0 {width} {height}\">"
    );
    svg.push_str("<defs><style>");
    svg.push_str(include_str!("../assets/hand-font-svg.css"));
    svg.push_str("</style></defs>");
    if let Some(bg) = background {
        svg.push_str(&format!("<rect width=\"100%\" height=\"100%\" fill=\"{}\"/>", hex(bg)));
    }
    for element in &scene.elements {
        let st = &element.style;
        let stroke = hex(st.stroke);
        let opacity = st.opacity as f32 / 100.0;
        let transform = if element.angle == 0.0 {
            String::new()
        } else {
            let c = element.center();
            format!(" transform=\"rotate({} {} {})\"", element.angle.to_degrees(), c.x + offset[0], c.y + offset[1])
        };
        svg.push_str(&format!("<g opacity=\"{opacity}\"{transform}>"));
        if let Some(url) = &element.image {
            let b = local_bounds(element);
            svg.push_str(&format!(
                "<image href=\"{}\" x=\"{}\" y=\"{}\" width=\"{}\" height=\"{}\" preserveAspectRatio=\"none\"/></g>",
                xml_escape(url), b.min.x + offset[0], b.min.y + offset[1], b.width(), b.height()
            ));
            continue;
        }
        if let Some(created) = element.created {
            let b = local_bounds(element);
            svg.push_str(&format!(
                "<polygon points=\"{}\" fill=\"#000000\" fill-opacity=\"0.16\"/><polygon points=\"{}\" fill=\"{}\" stroke=\"#000000\" stroke-opacity=\"0.08\" stroke-width=\"0.5\"/>",
                svg_points(&sticky_outline(b, st.round, 3.0), offset),
                svg_points(&sticky_outline(b, st.round, 0.0), offset),
                hex(st.fill.unwrap_or([0xff, 0xdf, 0x6b, 255]))
            ));
            if let Some(date) = sticky_date(created, b.width() - 32.0 < 80.0).filter(|_| b.width() >= 75.0 && b.height() >= 75.0) {
                svg.push_str(&format!(
                    "<text x=\"{}\" y=\"{}\" text-anchor=\"end\" font-family=\"Helvetica, Arial, sans-serif\" font-size=\"12\" fill=\"{stroke}\">{date}</text>",
                    b.max.x - 16.0 + offset[0],
                    b.max.y - 14.0 + offset[1]
                ));
            }
            svg.push_str("</g>");
            continue;
        }
        if let (Some(fill), Some(poly)) = (st.fill, shape_polygon(element)) {
            if st.fill_style == FillStyle::Solid {
                svg.push_str(&format!("<polygon points=\"{}\" fill=\"{}\"/>", svg_points(&poly, offset), hex(fill)));
            }
            for line in fill_strokes(element) {
                svg.push_str(&format!(
                    "<polyline points=\"{}\" fill=\"none\" stroke=\"{}\" stroke-width=\"{}\"/>",
                    svg_points(&line, offset),
                    hex(fill),
                    st.stroke_width / 2.0
                ));
            }
        }
        if let (Some(name), ElementKind::Rectangle { x, y, .. }) = (&element.frame, &element.kind) {
            if !name.is_empty() {
                svg.push_str(&format!(
                    "<text x=\"{}\" y=\"{}\" font-family=\"Assistant, sans-serif\" font-size=\"13\" fill=\"#999999\">{}</text>",
                    x + offset[0],
                    y + offset[1] - 5.0,
                    xml_escape(name)
                ));
            }
        }
        if let ElementKind::Text { x, y, value } = &element.kind {
            let b = local_bounds(element);
            if is_arrow_label(&scene.elements, element) {
                let m = b.expand(4.0);
                svg.push_str(&format!(
                    "<rect x=\"{}\" y=\"{}\" width=\"{}\" height=\"{}\" rx=\"4\" fill=\"{}\"/>",
                    m.min.x + offset[0], m.min.y + offset[1], m.width(), m.height(), hex(background.unwrap_or(scene.background))
                ));
            }
            let (anchor, tx) = if is_label(element) { ("middle", b.center().x) } else { ("start", *x) };
            for (i, line) in text_lines(value).into_iter().enumerate() {
                svg.push_str(&svg_text_line(
                    tx + offset[0],
                    y + offset[1] + st.font_size * (0.95 + i as f32 * LINE_HEIGHT),
                    anchor == "middle",
                    st.font_size,
                    &stroke,
                    text_needs_halo(&scene.elements, element).then(|| hex(background.unwrap_or(scene.background))).as_deref(),
                    line,
                ));
            }
        }
        let dash = match st.stroke_style {
            StrokeStyle::Solid => String::new(),
            StrokeStyle::Dashed => format!(" stroke-dasharray=\"8 {}\"", 8.0 + st.stroke_width),
            StrokeStyle::Dotted => format!(" stroke-dasharray=\"1.5 {}\"", 6.0 + st.stroke_width),
        };
        for outline in rough_outlines(element) {
            svg.push_str(&format!(
                "<polyline points=\"{}\" fill=\"none\" stroke=\"{stroke}\" stroke-width=\"{}\" stroke-linecap=\"round\" stroke-linejoin=\"round\"{dash}/>",
                svg_points(&outline, offset),
                st.stroke_width
            ));
        }
        for (shape, head) in arrowheads(element) {
            let fill = match head.fill() {
                Some(true) => stroke.clone(),
                Some(false) => hex(background.unwrap_or(scene.background)),
                None => "none".into(),
            };
            let tag = if head.fill().is_some() { "polygon" } else { "polyline" };
            svg.push_str(&format!(
                "<{tag} points=\"{}\" fill=\"{fill}\" stroke=\"{stroke}\" stroke-width=\"{}\" stroke-linecap=\"round\" stroke-linejoin=\"round\"/>",
                svg_points(&shape, offset),
                st.stroke_width
            ));
        }
        svg.push_str("</g>");
    }
    svg.push_str("</svg>");
    svg
}

fn xml_escape(text: &str) -> String {
    text.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;").replace('"', "&quot;").replace('\'', "&apos;")
}

/// macOS fonts used as fallback for arrows, check marks and other symbols.
const SYMBOL_FONTS: [&str; 2] = ["/System/Library/Fonts/Apple Symbols.ttf", "/System/Library/Fonts/Supplemental/Arial Unicode.ttf"];

/// Built once: bundled fonts first, then every system font as fallback for missing glyphs.
fn export_fonts() -> std::sync::Arc<resvg::usvg::fontdb::Database> {
    static FONTS: std::sync::OnceLock<std::sync::Arc<resvg::usvg::fontdb::Database>> = std::sync::OnceLock::new();
    FONTS
        .get_or_init(|| {
            let mut db = resvg::usvg::fontdb::Database::new();
            // the hand font ships as 7 unicode-range subsets; the basic-Latin one (5) must be found first
            for data in [
                &include_bytes!("../assets/hand-5.ttf")[..],
                include_bytes!("../assets/hand-1.ttf"),
                include_bytes!("../assets/hand-2.ttf"),
                include_bytes!("../assets/hand-3.ttf"),
                include_bytes!("../assets/hand-4.ttf"),
                include_bytes!("../assets/hand-6.ttf"),
                include_bytes!("../assets/hand-7.ttf"),
                include_bytes!("../assets/Assistant.ttf"),
            ] {
                db.load_font_data(data.to_vec());
            }
            for path in SYMBOL_FONTS {
                let _ = db.load_font_file(path);
            }
            // SVGs ask for "MagoHand, cursive"; cursive resolves to the hand font's own family name
            let family = db.faces().next().map(|f| f.families[0].0.clone());
            if let Some(family) = family {
                db.set_cursive_family(family);
            }
            db.load_system_fonts(); // emoji and anything else, memory-mapped
            db.set_sans_serif_family("Assistant");
            std::sync::Arc::new(db)
        })
        .clone()
}

/// One line of SVG text. Characters the hand font lacks (→ ✓ ★ …) become their own `<text>`
/// in a font that has them, placed by real glyph advances; resvg would otherwise reshape
/// the whole line in the fallback font.
fn svg_text_line(x: f32, y: f32, centered: bool, size: f32, fill: &str, halo: Option<&str>, line: &str) -> String {
    let db = export_fonts();
    // the 7 bundled hand-font subsets are the first faces loaded
    let hand_faces: Vec<_> = db.faces().take(7).map(|f| f.id).collect();
    let face_of = |ch: char| -> Option<(resvg::usvg::fontdb::ID, bool)> {
        let mut hand = None;
        let mut other = None;
        for face in db.faces() {
            let covers = db
                .with_face_data(face.id, |data, index| {
                    ttf_parser::Face::parse(data, index).ok().and_then(|f| f.glyph_index(ch)).is_some()
                })
                .unwrap_or(false);
            if covers {
                if hand_faces.contains(&face.id) {
                    hand = Some(face.id);
                    break;
                }
                other = other.or(Some(face.id));
            }
        }
        hand.map(|id| (id, true)).or(other.map(|id| (id, false)))
    };
    let advance = |id, ch: char| {
        db.with_face_data(id, |data, index| {
            let face = ttf_parser::Face::parse(data, index).ok()?;
            let glyph = face.glyph_index(ch)?;
            Some(face.glyph_hor_advance(glyph)? as f32 / face.units_per_em() as f32 * size)
        })
        .flatten()
        .unwrap_or(size * CHAR_WIDTH)
    };
    // runs of (text, fallback family or None for the hand font, width)
    let mut runs: Vec<(String, Option<String>, f32)> = Vec::new();
    for ch in line.chars() {
        let (family, width) = match face_of(ch) {
            Some((id, true)) => (None, advance(id, ch)),
            Some((id, false)) => (db.face(id).map(|f| f.families[0].0.clone()), advance(id, ch)),
            None => (None, size * CHAR_WIDTH),
        };
        match runs.last_mut() {
            Some((text, f, w)) if *f == family || ch.is_whitespace() => {
                text.push(ch);
                *w += width;
            }
            _ => runs.push((ch.to_string(), family, width)),
        }
    }
    let total: f32 = runs.iter().map(|r| r.2).sum();
    let mut cursor = if centered { x - total / 2.0 } else { x };
    let halo_attr = halo.map_or(String::new(), |color| {
        format!(" stroke=\"{color}\" stroke-width=\"{:.1}\" stroke-linejoin=\"round\" paint-order=\"stroke\"", (size * 0.16).clamp(2.0, 6.0))
    });
    let mut out = String::new();
    for (text, family, width) in runs {
        out.push_str(&format!(
            "<text x=\"{cursor}\" y=\"{y}\" font-family=\"{}\" font-size=\"{size}\" fill=\"{fill}\"{halo_attr} xml:space=\"preserve\">{}</text>",
            xml_escape(family.as_deref().unwrap_or("MagoHand, cursive")),
            xml_escape(&text)
        ));
        cursor += width;
    }
    out
}

fn render_svg_png(svg: &str, scale: f32) -> Result<Vec<u8>, String> {
    let mut options = resvg::usvg::Options::default();
    options.fontdb = export_fonts();
    options.font_family = "Assistant".into();
    let tree = resvg::usvg::Tree::from_str(svg, &options).map_err(|e| e.to_string())?;
    let size = tree.size().to_int_size().scale_by(scale).ok_or("Tamanho de imagem inválido")?;
    let mut pixmap = resvg::tiny_skia::Pixmap::new(size.width(), size.height()).ok_or("Tamanho de imagem inválido")?;
    resvg::render(&tree, resvg::tiny_skia::Transform::from_scale(scale, scale), &mut pixmap.as_mut());
    pixmap.encode_png().map_err(|e| e.to_string())
}

// ---------- interop JSON format (open/save files from other whiteboards) ----------

fn parse_hex(value: &Value) -> Option<[u8; 4]> {
    let s = value.as_str()?.trim_start_matches('#');
    let s: String = if s.len() == 3 { s.chars().flat_map(|c| [c, c]).collect() } else { s.into() };
    if s.len() < 6 {
        return None; // "transparent" and friends
    }
    let byte = |i: usize| u8::from_str_radix(s.get(i..i + 2)?, 16).ok();
    Some([byte(0)?, byte(2)?, byte(4)?, if s.len() >= 8 { byte(6)? } else { 255 }])
}

fn interop_elements(elements: &[Element]) -> Vec<Value> {
    // targets list the arrows bound to them, so other whiteboards move those arrows too
    let mut bound: std::collections::HashMap<&str, Vec<Value>> = std::collections::HashMap::new();
    for e in elements {
        for b in [&e.ends.start, &e.ends.end].into_iter().flatten() {
            bound.entry(b.element_id.as_str()).or_default().push(json!({ "id": e.id, "type": "arrow" }));
        }
    }
    let binding = |b: &Option<Binding>| match b {
        Some(b) => json!({ "elementId": b.element_id, "fixedPoint": b.fixed, "mode": if b.inside { "inside" } else { "orbit" } }),
        None => Value::Null,
    };
    let head = |h: Option<Head>| serde_json::to_value(h).unwrap_or(Value::Null);
    elements
        .iter()
        .map(|e| {
            let b = local_bounds(e);
            let st = &e.style;
            let mut v = json!({
                "id": e.id,
                "x": b.min.x, "y": b.min.y, "width": b.width(), "height": b.height(),
                "angle": e.angle,
                "strokeColor": hex(st.stroke),
                "backgroundColor": st.fill.map(hex).unwrap_or_else(|| "transparent".into()),
                "fillStyle": match st.fill_style { FillStyle::Hachure => "hachure", FillStyle::CrossHatch => "cross-hatch", FillStyle::Solid => "solid" },
                "strokeWidth": st.stroke_width,
                "strokeStyle": match st.stroke_style { StrokeStyle::Solid => "solid", StrokeStyle::Dashed => "dashed", StrokeStyle::Dotted => "dotted" },
                "roughness": st.roughness,
                "opacity": st.opacity,
                "groupIds": e.group_id.iter().collect::<Vec<_>>(),
                "frameId": null,
                "roundness": null,
                "seed": Rng::new(&e.id, 0).0 as u32 >> 1,
                "version": 1,
                "versionNonce": 0,
                "isDeleted": false,
                "boundElements": bound.get(e.id.as_str()).cloned().map(Value::Array).unwrap_or(Value::Null),
                "updated": 0,
                "link": null,
                "locked": e.locked,
            });
            let relative = |points: &[[f32; 2]]| -> Value {
                let first = points.first().copied().unwrap_or([0.0, 0.0]);
                points.iter().map(|p| json!([p[0] - first[0], p[1] - first[1]])).collect()
            };
            if let Some(name) = &e.frame {
                v["name"] = name.clone().into();
            }
            match &e.kind {
                ElementKind::Rectangle { .. } | ElementKind::Diamond { .. } | ElementKind::Ellipse { .. } => {
                    let kind = match e.kind {
                        ElementKind::Rectangle { .. } if e.image.is_some() => "image",
                        ElementKind::Rectangle { .. } if e.created.is_some() => "stickynote",
                        ElementKind::Rectangle { .. } if e.frame.is_some() => "frame",
                        ElementKind::Rectangle { .. } => "rectangle",
                        ElementKind::Diamond { .. } => "diamond",
                        _ => "ellipse",
                    };
                    v["type"] = kind.into();
                    if kind == "image" {
                        v["fileId"] = e.id.clone().into();
                        v["status"] = "saved".into();
                        v["scale"] = json!([1, 1]);
                    }
                    if let Some(created) = e.created {
                        v["created"] = created.into();
                    }
                    if st.round && kind != "ellipse" && kind != "image" {
                        v["roundness"] = json!({ "type": 3 });
                    }
                }
                ElementKind::Line { points, arrow } => {
                    v["type"] = if *arrow { "arrow" } else { "line" }.into();
                    v["x"] = points.first().map_or(0.0, |p| p[0]).into();
                    v["y"] = points.first().map_or(0.0, |p| p[1]).into();
                    v["points"] = relative(points);
                    v["lastCommittedPoint"] = Value::Null;
                    v["startBinding"] = binding(&e.ends.start);
                    v["endBinding"] = binding(&e.ends.end);
                    v["startArrowhead"] = if *arrow { head(e.ends.start_head) } else { Value::Null };
                    v["endArrowhead"] = if *arrow { head(e.ends.end_head) } else { Value::Null };
                    if st.round {
                        v["roundness"] = json!({ "type": 2 });
                    }
                }
                ElementKind::FreeDraw { points } => {
                    v["type"] = "freedraw".into();
                    v["x"] = points.first().map_or(0.0, |p| p[0]).into();
                    v["y"] = points.first().map_or(0.0, |p| p[1]).into();
                    v["points"] = relative(points);
                    v["pressures"] = json!([]);
                    v["simulatePressure"] = true.into();
                    v["lastCommittedPoint"] = Value::Null;
                }
                ElementKind::Text { value, .. } => {
                    v["type"] = "text".into();
                    v["text"] = value.clone().into();
                    v["originalText"] = value.clone().into();
                    v["fontSize"] = st.font_size.into();
                    v["fontFamily"] = 5.into();
                    v["textAlign"] = "left".into();
                    v["verticalAlign"] = "top".into();
                    v["containerId"] = Value::Null;
                    v["lineHeight"] = LINE_HEIGHT.into();
                    v["autoResize"] = true.into();
                }
            }
            v
        })
        .collect()
}

fn to_interop(scene: &Scene) -> Value {
    json!({
        "type": "excalidraw",
        "version": 2,
        "source": "magoagent",
        "elements": interop_elements(&scene.elements),
        "appState": { "viewBackgroundColor": hex(scene.background), "gridSize": 20 },
        "files": scene
            .elements
            .iter()
            .filter_map(|e| {
                let url = e.image.as_ref()?;
                let mime = url.strip_prefix("data:")?.split(';').next()?;
                Some((e.id.clone(), json!({ "id": e.id, "mimeType": mime, "dataURL": url, "created": 0 })))
            })
            .collect::<serde_json::Map<_, _>>(),
    })
}

fn from_interop_elements(elements: &Value) -> Vec<Element> {
    let f = |v: &Value, k: &str| v[k].as_f64().unwrap_or(0.0) as f32;
    elements
        .as_array()
        .into_iter()
        .flatten()
        .filter(|v| v["isDeleted"] != true)
        .filter_map(|v| {
            let (x, y, width, height) = (f(v, "x"), f(v, "y"), f(v, "width"), f(v, "height"));
            let points = || -> Vec<[f32; 2]> {
                v["points"]
                    .as_array()
                    .into_iter()
                    .flatten()
                    .map(|p| [x + p[0].as_f64().unwrap_or(0.0) as f32, y + p[1].as_f64().unwrap_or(0.0) as f32])
                    .collect()
            };
            let kind = match v["type"].as_str()? {
                "rectangle" | "frame" | "image" | "stickynote" => ElementKind::Rectangle { x, y, width, height },
                "diamond" => ElementKind::Diamond { x, y, width, height },
                "ellipse" => ElementKind::Ellipse { x, y, width, height },
                "line" => ElementKind::Line { points: points(), arrow: false },
                "arrow" => ElementKind::Line { points: points(), arrow: true },
                "freedraw" => ElementKind::FreeDraw { points: points() },
                "text" => ElementKind::Text { x, y, value: v["text"].as_str().unwrap_or_default().into() },
                _ => return None,
            };
            let style = Style {
                stroke: parse_hex(&v["strokeColor"]).unwrap_or(BLACK),
                fill: parse_hex(&v["backgroundColor"]),
                stroke_width: v["strokeWidth"].as_f64().unwrap_or(2.0) as f32,
                stroke_style: match v["strokeStyle"].as_str() {
                    Some("dashed") => StrokeStyle::Dashed,
                    Some("dotted") => StrokeStyle::Dotted,
                    _ => StrokeStyle::Solid,
                },
                fill_style: match v["fillStyle"].as_str() {
                    Some("hachure") => FillStyle::Hachure,
                    Some("cross-hatch") => FillStyle::CrossHatch,
                    _ => FillStyle::Solid,
                },
                roughness: v["roughness"].as_u64().unwrap_or(1).min(2) as u8,
                round: !v["roundness"].is_null(),
                opacity: v["opacity"].as_u64().unwrap_or(100).min(100) as u8,
                font_size: v["fontSize"].as_f64().unwrap_or(20.0) as f32,
            };
            let mut element = Element {
                id: v["id"].as_str().map(String::from).unwrap_or_else(|| uuid::Uuid::new_v4().to_string()),
                // outermost group, so nested groups still move together
                group_id: v["groupIds"].as_array().and_then(|g| g.last()).and_then(|g| g.as_str()).map(String::from),
                kind,
                angle: 0.0,
                locked: v["locked"].as_bool().unwrap_or(false),
                frame: (v["type"] == "frame").then(|| v["name"].as_str().unwrap_or_default().to_string()),
                // the file id for now; parse_scene swaps in the data URL from `files`
                image: (v["type"] == "image").then(|| v["fileId"].as_str().unwrap_or_default().to_string()),
                created: (v["type"] == "stickynote").then(|| v["created"].as_i64().unwrap_or(0)),
                ends: ArrowEnds {
                    start_head: serde_json::from_value(v["startArrowhead"].clone()).ok().flatten(),
                    end_head: match v.get("endArrowhead") {
                        Some(h) => serde_json::from_value(h.clone()).ok().flatten(),
                        None => Some(Head::Arrow),
                    },
                    start: parse_binding(&v["startBinding"]),
                    end: parse_binding(&v["endBinding"]),
                },
                style,
            };
            let angle = v["angle"].as_f64().unwrap_or(0.0) as f32;
            if angle != 0.0 {
                // Rotation is around the element center; lines get the rotation baked in
                let center = element.center();
                element.rotate_around(center, angle);
            }
            Some(element)
        })
        .collect()
}

fn parse_binding(v: &Value) -> Option<Binding> {
    let fixed = v["fixedPoint"].as_array().map_or([0.5, 0.5], |p| {
        [p.first().and_then(Value::as_f64).unwrap_or(0.5) as f32, p.get(1).and_then(Value::as_f64).unwrap_or(0.5) as f32]
    });
    Some(Binding { element_id: v["elementId"].as_str()?.into(), fixed, inside: v["mode"] == "inside" })
}

/// Accepts `.excalidraw` files and this app's own `.rea` format.
fn parse_scene(data: &str) -> Result<Scene, String> {
    let value: Value = serde_json::from_str(data).map_err(|e| e.to_string())?;
    if value["type"] == "excalidraw" {
        let mut elements = from_interop_elements(&value["elements"]);
        for e in &mut elements {
            if let Some(image) = e.image.as_mut() {
                if let Some(url) = value["files"][image.as_str()]["dataURL"].as_str() {
                    *image = url.to_string();
                }
            }
        }
        return Ok(Scene {
            version: 1,
            elements,
            background: parse_hex(&value["appState"]["viewBackgroundColor"]).unwrap_or(WHITE),
            agent: None,
        });
    }
    serde_json::from_value(value).map_err(|e| e.to_string())
}

/// The live drawing shared by the app and the MCP server; MAGOAGENT_SCENE points elsewhere.
fn autosave_path() -> Option<PathBuf> {
    if let Some(path) = std::env::var_os("MAGOAGENT_SCENE") {
        return Some(PathBuf::from(path));
    }
    std::env::var_os("HOME")
        .map(|home| PathBuf::from(home).join("Library/Application Support/MagoAgent/autosave.rea"))
}

fn prefs_path() -> Option<PathBuf> {
    autosave_path().map(|p| p.with_file_name("preferences.json"))
}

fn modified(path: &std::path::Path) -> Option<SystemTime> {
    fs::metadata(path).and_then(|m| m.modified()).ok()
}

fn read_scene_file(path: &std::path::Path) -> Option<(Scene, Option<SystemTime>)> {
    let stamp = modified(path);
    let scene = parse_scene(&fs::read_to_string(path).ok()?).ok()?;
    Some((scene, stamp))
}

/// Write-then-rename, so the other process never reads half a file.
fn write_scene_file(path: &std::path::Path, scene: &Scene) -> std::io::Result<Option<SystemTime>> {
    if let Some(dir) = path.parent() {
        fs::create_dir_all(dir)?;
    }
    let tmp = path.with_extension(format!("tmp-{}", std::process::id()));
    fs::write(&tmp, serde_json::to_string(scene).map_err(std::io::Error::other)?)?;
    fs::rename(&tmp, path)?;
    Ok(modified(path))
}

/// Three-way merge by element id: whatever changed in `external` since `base` wins,
/// local edits to untouched elements and locally new elements survive.
fn merge_scenes(base: &Scene, local: &Scene, external: &Scene) -> Scene {
    let find = |scene: &Scene, id: &str| scene.elements.iter().find(|e| e.id == id).cloned();
    let mut elements: Vec<Element> = external
        .elements
        .iter()
        .filter_map(|ext| match find(base, &ext.id) {
            // untouched outside: keep the local version (or the local deletion)
            Some(old) if old == *ext => find(local, &ext.id),
            _ => Some(ext.clone()),
        })
        .collect();
    elements.extend(
        local
            .elements
            .iter()
            .filter(|e| find(base, &e.id).is_none() && find(external, &e.id).is_none())
            .cloned(),
    );
    let background = if external.background != base.background { external.background } else { local.background };
    Scene { version: external.version, elements, background, agent: None }
}

fn main() -> eframe::Result {
    if std::env::args().nth(1).as_deref() == Some("mcp") {
        mcp::run();
        return Ok(());
    }
    let native_options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title(APP_TITLE)
            .with_inner_size([1440.0, 920.0])
            .with_min_inner_size([920.0, 640.0]),
        ..Default::default()
    };
    eframe::run_native(
        APP_TITLE,
        native_options,
        Box::new(|cc| {
            install_fonts(&cc.egui_ctx);
            let mut app = DrawingApp { autosave: autosave_path(), ..Default::default() };
            if let Some(prefs) = prefs_path()
                .and_then(|p| fs::read_to_string(p).ok())
                .and_then(|d| serde_json::from_str::<Prefs>(&d).ok())
            {
                app.prefs = prefs;
                app.saved_prefs = prefs;
                app.tool_locked = prefs.tool_locked;
                app.theme = prefs.theme;
            }
            if let Some((scene, stamp)) = app.autosave.as_deref().and_then(read_scene_file) {
                app.scene = scene.clone();
                app.synced = scene;
                app.synced_stamp = stamp;
            }
            Ok(Box::new(app))
        }),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rect_element(x: f32, y: f32, width: f32, height: f32, fill: Option<[u8; 4]>) -> Element {
        let mut e = DrawingApp::default().new_element(ElementKind::Rectangle { x, y, width, height });
        e.style.fill = fill;
        e
    }

    #[test]
    fn save_format_round_trips_scene() {
        let scene = Scene { elements: vec![rect_element(5.0, 8.0, 40.0, 30.0, None)], ..Default::default() };
        let decoded: Scene = serde_json::from_str(&serde_json::to_string(&scene).unwrap()).unwrap();
        assert_eq!(decoded, scene);
    }

    #[test]
    fn legacy_rea_files_still_load() {
        let old = r#"{"version":1,"elements":[{"id":"a","kind":{"type":"rectangle","x":1,"y":2,"width":3,"height":4},"stroke":[1,2,3,255],"fill":null,"stroke_width":4}]}"#;
        let scene = parse_scene(old).unwrap();
        assert_eq!(scene.elements[0].style.stroke_width, 4.0);
        assert_eq!(scene.elements[0].style.roughness, 1);
    }

    #[test]
    fn interop_format_round_trips() {
        let mut app = DrawingApp::default();
        app.scene.elements = vec![
            rect_element(10.0, 20.0, 50.0, 40.0, Some([255, 201, 201, 255])),
            app.new_element(ElementKind::Line { points: vec![[0.0, 0.0], [30.0, 40.0]], arrow: true }),
            app.new_element(ElementKind::Text { x: 5.0, y: 6.0, value: "Olá\nmundo".into() }),
        ];
        app.scene.elements[0].style.fill_style = FillStyle::Hachure;
        app.scene.elements[0].angle = 0.5;
        app.scene.elements[1].style.stroke_style = StrokeStyle::Dashed;
        let back = parse_scene(&to_interop(&app.scene).to_string()).unwrap();
        assert_eq!(back.elements.len(), 3);
        assert_eq!(back.elements[0].kind, app.scene.elements[0].kind);
        assert_eq!(back.elements[0].style.fill_style, FillStyle::Hachure);
        assert!((back.elements[0].angle - 0.5).abs() < 1e-6);
        assert_eq!(back.elements[1].kind, app.scene.elements[1].kind);
        assert_eq!(back.elements[1].style.stroke_style, StrokeStyle::Dashed);
        assert_eq!(back.elements[2].kind, app.scene.elements[2].kind);
    }

    #[test]
    fn svg_export_contains_drawn_shape() {
        let scene = Scene { elements: vec![rect_element(10.0, 20.0, 50.0, 40.0, Some([240, 240, 250, 255]))], ..Default::default() };
        let svg = scene_svg(&scene, Some(WHITE));
        assert!(svg.contains("<polygon") && svg.contains("<polyline"));
        assert!(svg.contains("width=\"70\""));
    }

    #[test]
    fn png_export_is_a_valid_image() {
        let png = render_svg_png(&scene_svg(&Scene::default(), Some(WHITE)), 2.0).unwrap();
        let image = image::load_from_memory(&png).unwrap();
        assert_eq!(image.width(), 360);
    }

    #[test]
    fn grouping_and_ungrouping_keeps_elements_together() {
        let mut app = DrawingApp::default();
        app.scene.elements = vec![rect_element(0.0, 0.0, 10.0, 10.0, None), rect_element(20.0, 0.0, 10.0, 10.0, None)];
        app.selected = vec![0, 1];
        app.group_selection();
        assert_eq!(app.scene.elements[0].group_id, app.scene.elements[1].group_id);
        app.ungroup_selection();
        assert!(app.scene.elements.iter().all(|e| e.group_id.is_none()));
    }

    #[test]
    fn layering_moves_selection() {
        let mut app = DrawingApp::default();
        app.scene.elements = (0..4).map(|i| rect_element(i as f32, 0.0, 1.0, 1.0, None)).collect();
        let ids: Vec<String> = app.scene.elements.iter().map(|e| e.id.clone()).collect();
        app.selected = vec![1];
        app.reorder(Layer::Front);
        assert_eq!(app.scene.elements[3].id, ids[1]);
        assert_eq!(app.selected, vec![3]);
        app.reorder(Layer::Backward);
        assert_eq!(app.scene.elements[2].id, ids[1]);
        app.reorder(Layer::Back);
        assert_eq!(app.scene.elements[0].id, ids[1]);
    }

    #[test]
    fn duplicate_and_paste_create_fresh_elements() {
        let mut app = DrawingApp::default();
        app.scene.elements = vec![rect_element(0.0, 0.0, 10.0, 10.0, None)];
        app.selected = vec![0];
        app.duplicate();
        assert_eq!(app.scene.elements.len(), 2);
        assert_ne!(app.scene.elements[0].id, app.scene.elements[1].id);
        assert_eq!(element_bounds(&app.scene.elements[1]).min, Pos2::new(10.0, 10.0));
        let clip = json!({ "type": "magoagent/clipboard", "elements": interop_elements(&app.scene.elements[..1]) });
        app.paste(&clip.to_string());
        assert_eq!(app.scene.elements.len(), 3);
        app.paste("texto colado");
        assert!(matches!(&app.scene.elements[3].kind, ElementKind::Text { value, .. } if value == "texto colado"));
    }

    #[test]
    fn bindings_and_heads_round_trip_through_interop_format() {
        let mut app = DrawingApp::default();
        app.scene.elements = vec![rect_element(0.0, 0.0, 100.0, 100.0, None), rect_element(300.0, 0.0, 100.0, 100.0, None)];
        app.tool = Tool::Arrow;
        app.heads = (Some(Head::Circle), Some(Head::TriangleOutline));
        let arrow = app.bind_new_arrow(app.new_element(ElementKind::Line { points: vec![[95.0, 50.0], [305.0, 50.0]], arrow: true }));
        app.scene.elements.push(arrow);
        let file = to_interop(&app.scene);
        assert_eq!(file["elements"][0]["boundElements"][0]["id"], app.scene.elements[2].id);
        assert_eq!(file["elements"][2]["startArrowhead"], "circle");
        assert_eq!(file["elements"][2]["endBinding"]["mode"], "orbit");
        let back = parse_scene(&file.to_string()).unwrap();
        assert_eq!(back.elements[2].ends, app.scene.elements[2].ends);
        // legacy .rea arrows without "ends" keep their end arrowhead
        let mut legacy = serde_json::to_value(&app.scene.elements[2]).unwrap();
        legacy.as_object_mut().unwrap().remove("ends");
        let old: Element = serde_json::from_value(legacy).unwrap();
        assert_eq!((old.ends.start_head, old.ends.end_head), (None, Some(Head::Arrow)));
    }

    #[test]
    fn arrowhead_shapes_match_reference_sizes() {
        let app = DrawingApp::default();
        let mut e = app.new_element(ElementKind::Line { points: vec![[0.0, 0.0], [200.0, 0.0]], arrow: true });
        let heads = arrowheads(&e);
        assert_eq!(heads.len(), 1);
        let (pts, head) = &heads[0];
        assert_eq!(*head, Head::Arrow);
        // size 25 at 20°: the barbs end 25 * cos(20°) behind the tip
        assert!((pts[0][0] - (200.0 - 25.0 * 20f32.to_radians().cos())).abs() < 0.01);
        e.ends = ArrowEnds { start_head: Some(Head::Bar), end_head: None, ..Default::default() };
        let heads = arrowheads(&e);
        assert_eq!(heads.len(), 1);
        assert!((heads[0].0[0][0] - 0.0).abs() < 0.01, "bar is perpendicular at the start tip");
    }

    #[test]
    fn palette_search_filters_without_accents() {
        let mut app = DrawingApp::default();
        app.palette_query = "zoom".into();
        let found: Vec<&str> = app.palette_items().iter().map(|c| c.label()).collect();
        assert_eq!(found, ["Aumentar zoom", "Diminuir zoom", "Redefinir zoom"], "selection-only commands stay hidden");
        app.palette_query = "retangulo".into();
        assert_eq!(app.palette_items(), [Command::Tool(Tool::Rectangle)]);
        app.palette_query = "salvar comp".into();
        assert_eq!(app.palette_items(), [Command::Save]);
        app.selected = vec![0];
        app.palette_query = "zoom".into();
        assert_eq!(app.palette_items().len(), 4);
        assert_eq!(shortcut_keys("Cmd++"), ["Cmd", "+"]);
        assert_eq!(shortcut_keys("Cmd+Shift+E"), ["Cmd", "Shift", "E"]);
        app.run_command(Command::Tool(Tool::Ellipse));
        assert_eq!(app.tool, Tool::Ellipse);
    }

    #[test]
    fn merge_keeps_both_sides_changes() {
        let a = rect_element(0.0, 0.0, 10.0, 10.0, None);
        let b = rect_element(50.0, 0.0, 10.0, 10.0, None);
        let base = Scene { elements: vec![a.clone(), b.clone()], ..Default::default() };
        // user moved `a` and drew `c`; the AI recolored `b` and added `d`
        let mut local = base.clone();
        local.elements[0].move_by(Vec2::new(5.0, 0.0));
        let c = rect_element(100.0, 0.0, 10.0, 10.0, None);
        local.elements.push(c.clone());
        let mut external = base.clone();
        external.elements[1].style.stroke = [255, 0, 0, 255];
        let d = rect_element(200.0, 0.0, 10.0, 10.0, None);
        external.elements.push(d.clone());
        let merged = merge_scenes(&base, &local, &external);
        let ids: Vec<&str> = merged.elements.iter().map(|e| e.id.as_str()).collect();
        assert_eq!(ids, [a.id.as_str(), b.id.as_str(), d.id.as_str(), c.id.as_str()]);
        assert_eq!(merged.elements[0], local.elements[0]);
        assert_eq!(merged.elements[1].style.stroke, [255, 0, 0, 255]);
        // the AI deleting an element the user didn't touch removes it
        let external = Scene { elements: vec![a.clone()], ..Default::default() };
        let merged = merge_scenes(&base, &base, &external);
        assert_eq!(merged.elements.len(), 1);
    }

    #[test]
    fn minimap_covers_drawing_and_centers_on_click() {
        let mut app = DrawingApp::default();
        app.canvas_rect = Rect::from_min_size(Pos2::ZERO, Vec2::new(800.0, 600.0));
        app.scene.elements = vec![rect_element(3000.0, 2000.0, 100.0, 100.0, None)];
        let world = app.minimap_world();
        assert!(world.contains_rect(element_bounds(&app.scene.elements[0])));
        assert!(world.contains(Pos2::ZERO), "the visible area is part of the map");
        app.zoom = 2.0;
        app.pan_to(Pos2::new(3050.0, 2050.0));
        assert_eq!(app.to_screen(Pos2::new(3050.0, 2050.0)), app.canvas_rect.center());
    }

    #[test]
    fn symbols_render_in_png_instead_of_boxes() {
        if !std::path::Path::new(SYMBOL_FONTS[0]).exists() {
            return; // not macOS
        }
        let line = |text: &str| {
            format!("<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"60\" height=\"40\">{}</svg>", svg_text_line(5.0, 30.0, false, 28.0, "#000", None, text))
        };
        let missing = render_svg_png(&line("\u{E123}"), 1.0).unwrap();
        for symbol in ["→", "✓", "★", "⇒"] {
            assert_ne!(render_svg_png(&line(symbol), 1.0).unwrap(), missing, "{symbol} fell back to a box");
        }
        // only the symbol leaves the hand font, the words around it stay hand-drawn
        let svg = svg_text_line(0.0, 0.0, false, 20.0, "#000", None, "Login → Painel");
        assert_eq!(svg.matches("font-family=\"MagoHand, cursive\"").count(), 2, "{svg}");
        assert!(svg.contains(">Login <") && svg.contains(">Painel<"), "{svg}");
    }

    #[test]
    fn text_over_lines_gets_an_outline() {
        let app = DrawingApp::default();
        let line = app.new_element(ElementKind::Line { points: vec![[0.0, 50.0], [300.0, 50.0]], arrow: true });
        let over = app.new_element(ElementKind::Text { x: 100.0, y: 40.0, value: "lê · grava".into() });
        let away = app.new_element(ElementKind::Text { x: 100.0, y: 200.0, value: "longe".into() });
        let elements = vec![line, over.clone(), away.clone()];
        assert!(text_needs_halo(&elements, &over));
        assert!(!text_needs_halo(&elements, &away));
        assert!(svg_text_line(0.0, 0.0, false, 20.0, "#000", Some("#ffffff"), "x").contains("paint-order=\"stroke\""));
    }

    #[test]
    fn hachure_stays_inside_shape() {
        let mut e = rect_element(0.0, 0.0, 100.0, 60.0, Some([255, 0, 0, 255]));
        e.style.fill_style = FillStyle::CrossHatch;
        e.style.roughness = 0;
        let lines = fill_strokes(&e);
        assert!(lines.len() > 10);
        let area = element_bounds(&e).expand(1.0);
        assert!(lines.iter().flatten().all(|p| area.contains(Pos2::new(p[0], p[1]))));
    }

    #[test]
    fn dark_mode_flips_lightness() {
        let black = themed(rgba(BLACK), true);
        let white = themed(Color32::WHITE, true);
        assert!(black.r() > 200 && white.r() < 30);
        let red = themed(rgba([0xe0, 0x31, 0x31, 255]), true);
        assert!(red.r() > red.g() && red.r() > red.b());
    }
}

#[cfg(test)]
mod interaction_tests {
    use super::*;

    fn frame(app: &mut DrawingApp, ctx: &Context, events: Vec<egui::Event>) {
        let mut output = ctx.run_ui(
            egui::RawInput {
                screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(800.0, 600.0))),
                events,
                ..Default::default()
            },
            |ui| {
                egui::CentralPanel::no_frame().show(ui, |ui| {
                    let (response, _) = ui.allocate_painter(ui.available_size(), Sense::click_and_drag());
                    app.handle_canvas(&response, response.rect);
                });
                app.floating_controls(ctx);
            },
        );
        output.textures_delta.clear();
    }

    fn button(pos: Pos2, pressed: bool) -> egui::Event {
        egui::Event::PointerButton { pos, button: egui::PointerButton::Primary, pressed, modifiers: Default::default() }
    }

    fn drag(app: &mut DrawingApp, ctx: &Context, start: Pos2, end: Pos2) {
        frame(app, ctx, vec![egui::Event::PointerMoved(start), button(start, true)]);
        frame(app, ctx, vec![egui::Event::PointerMoved(start + Vec2::new(12.0, 12.0))]);
        frame(app, ctx, vec![egui::Event::PointerMoved(end)]);
        frame(app, ctx, vec![button(end, false)]);
    }

    #[test]
    fn drawing_uses_press_origin_and_drag_moves_selection() {
        let ctx = Context::default();
        let mut app = DrawingApp::default();
        app.tool = Tool::Rectangle;
        frame(&mut app, &ctx, vec![]);
        drag(&mut app, &ctx, Pos2::new(300.0, 200.0), Pos2::new(500.0, 350.0));
        assert_eq!(app.scene.elements.len(), 1);
        assert_eq!(app.tool, Tool::Select, "returns to selection after drawing, like the reference app");
        let before = element_bounds(&app.scene.elements[0]);
        assert_eq!(before.size(), Vec2::new(200.0, 150.0));
        drag(&mut app, &ctx, Pos2::new(350.0, 250.0), Pos2::new(420.0, 320.0));
        let after = element_bounds(&app.scene.elements[0]);
        assert!(after.min.x > before.min.x + 40.0);
        app.undo();
        assert_eq!(element_bounds(&app.scene.elements[0]), before);
    }

    #[test]
    fn rapid_gesture_preserves_start_and_end() {
        let ctx = Context::default();
        let mut app = DrawingApp::default();
        app.tool = Tool::Rectangle;
        frame(&mut app, &ctx, vec![]);
        let (start, end) = (Pos2::new(300.0, 200.0), Pos2::new(500.0, 350.0));
        frame(&mut app, &ctx, vec![egui::Event::PointerMoved(start), button(start, true)]);
        frame(&mut app, &ctx, vec![egui::Event::PointerMoved(end), button(end, false)]);
        assert_eq!(element_bounds(&app.scene.elements[0]), Rect::from_two_pos(start, end));
    }

    #[test]
    fn corner_handle_resizes_selection() {
        let ctx = Context::default();
        let mut app = DrawingApp::default();
        app.tool = Tool::Rectangle;
        frame(&mut app, &ctx, vec![]);
        drag(&mut app, &ctx, Pos2::new(300.0, 200.0), Pos2::new(500.0, 350.0));
        // bottom-right handle sits 5px outside the element bounds
        drag(&mut app, &ctx, Pos2::new(505.0, 355.0), Pos2::new(705.0, 505.0));
        assert_eq!(
            element_bounds(&app.scene.elements[0]),
            Rect::from_min_max(Pos2::new(300.0, 200.0), Pos2::new(700.0, 500.0))
        );
        app.undo();
        assert_eq!(element_bounds(&app.scene.elements[0]).size(), Vec2::new(200.0, 150.0));
    }

    #[test]
    fn eraser_removes_touched_elements() {
        let ctx = Context::default();
        let mut app = DrawingApp::default();
        app.scene.elements = vec![
            app.new_element(ElementKind::Line { points: vec![[300.0, 100.0], [300.0, 500.0]], arrow: false }),
            app.new_element(ElementKind::Line { points: vec![[600.0, 100.0], [600.0, 500.0]], arrow: false }),
        ];
        app.tool = Tool::Eraser;
        frame(&mut app, &ctx, vec![]);
        drag(&mut app, &ctx, Pos2::new(250.0, 300.0), Pos2::new(350.0, 300.0));
        assert_eq!(app.scene.elements.len(), 1);
        app.undo();
        assert_eq!(app.scene.elements.len(), 2);
    }

    #[test]
    fn hand_tool_pans_without_editing() {
        let ctx = Context::default();
        let mut app = DrawingApp::default();
        app.tool = Tool::Hand;
        frame(&mut app, &ctx, vec![]);
        drag(&mut app, &ctx, Pos2::new(300.0, 300.0), Pos2::new(400.0, 350.0));
        assert_eq!(app.pan, Vec2::new(100.0, 50.0));
        assert!(app.undo.is_empty());
    }

    #[test]
    fn rough_outline_is_stable_and_irregular() {
        let app = DrawingApp::default();
        let e = app.new_element(ElementKind::Ellipse { x: 0.0, y: 0.0, width: 200.0, height: 100.0 });
        let a = rough_outlines(&e);
        assert_eq!(a, rough_outlines(&e));
        assert_eq!(a.len(), 2);
        let mut smooth = e.clone();
        smooth.style.roughness = 0;
        assert_eq!(rough_outlines(&smooth).len(), 1);
    }

    fn click(app: &mut DrawingApp, ctx: &Context, pos: Pos2) {
        frame(app, ctx, vec![egui::Event::PointerMoved(pos), button(pos, true)]);
        frame(app, ctx, vec![button(pos, false)]);
    }

    #[test]
    fn rotation_handle_rotates_and_hit_test_follows() {
        let ctx = Context::default();
        let mut app = DrawingApp::default();
        app.tool = Tool::Rectangle;
        frame(&mut app, &ctx, vec![]);
        drag(&mut app, &ctx, Pos2::new(300.0, 200.0), Pos2::new(500.0, 350.0));
        // handle sits 5 + 16px above the box, centered
        drag(&mut app, &ctx, Pos2::new(400.0, 179.0), Pos2::new(550.0, 275.0));
        let e = &app.scene.elements[0];
        assert!((e.angle - std::f32::consts::FRAC_PI_2).abs() < 0.01, "angle {}", e.angle);
        let b = element_bounds(e);
        assert!((b.width() - 150.0).abs() < 0.5 && (b.height() - 200.0).abs() < 0.5);
        assert!(e.contains(Pos2::new(325.0, 275.0), 5.0), "rotated left edge");
        assert!(!e.contains(Pos2::new(450.0, 200.0), 5.0), "old top edge is empty now");
        app.undo();
        assert_eq!(app.scene.elements[0].angle, 0.0);
    }

    #[test]
    fn resizing_rotated_element_keeps_opposite_corner() {
        let mut app = DrawingApp::default();
        let mut e = app.new_element(ElementKind::Rectangle { x: 0.0, y: 0.0, width: 100.0, height: 50.0 });
        e.angle = 0.7;
        app.scene.elements = vec![e];
        app.selected = vec![0];
        app.canvas_rect = Rect::from_min_size(Pos2::ZERO, Vec2::new(800.0, 600.0));
        let corners = app.handle_corners().unwrap();
        let ctx = Context::default();
        frame(&mut app, &ctx, vec![]);
        drag(&mut app, &ctx, corners[2], corners[2] + Vec2::new(40.0, 30.0));
        let after = app.handle_corners().unwrap();
        assert!(after[0].distance(corners[0]) < 0.5, "{:?} vs {:?}", after[0], corners[0]);
        assert!((app.scene.elements[0].angle - 0.7).abs() < 1e-5);
    }

    #[test]
    fn click_click_creates_multi_point_arrow() {
        let ctx = Context::default();
        let mut app = DrawingApp::default();
        app.tool = Tool::Arrow;
        frame(&mut app, &ctx, vec![]);
        click(&mut app, &ctx, Pos2::new(400.0, 100.0));
        assert!(app.multi.is_some());
        frame(&mut app, &ctx, vec![egui::Event::PointerMoved(Pos2::new(500.0, 100.0))]);
        click(&mut app, &ctx, Pos2::new(500.0, 100.0));
        frame(&mut app, &ctx, vec![egui::Event::PointerMoved(Pos2::new(500.0, 200.0))]);
        click(&mut app, &ctx, Pos2::new(500.0, 200.0));
        app.finish_multi();
        assert_eq!(app.line_points(0), &[[400.0, 100.0], [500.0, 100.0], [500.0, 200.0]]);
        assert!(app.scene.elements[0].style.round, "arrows are curved by default");
        assert_eq!(app.tool, Tool::Select);
    }

    #[test]
    fn line_points_drag_insert_and_delete() {
        let ctx = Context::default();
        let mut app = DrawingApp::default();
        app.scene.elements = vec![app.new_element(ElementKind::Line { points: vec![[400.0, 300.0], [600.0, 300.0]], arrow: false })];
        app.selected = vec![0];
        frame(&mut app, &ctx, vec![]);
        drag(&mut app, &ctx, Pos2::new(600.0, 300.0), Pos2::new(600.0, 400.0));
        assert_eq!(app.line_points(0), &[[400.0, 300.0], [600.0, 400.0]]);
        app.line_editor = Some(0);
        drag(&mut app, &ctx, Pos2::new(500.0, 350.0), Pos2::new(500.0, 450.0));
        assert_eq!(app.line_points(0), &[[400.0, 300.0], [500.0, 450.0], [600.0, 400.0]]);
        assert_eq!(app.selected_point, Some(1));
        assert!(app.delete_selected_point());
        assert_eq!(app.line_points(0).len(), 2);
    }

    fn two_boxes(app: &mut DrawingApp) {
        app.scene.elements = vec![
            app.new_element(ElementKind::Rectangle { x: 300.0, y: 200.0, width: 100.0, height: 100.0 }),
            app.new_element(ElementKind::Rectangle { x: 600.0, y: 200.0, width: 100.0, height: 100.0 }),
        ];
    }

    #[test]
    fn arrow_binds_to_shapes_and_follows_them() {
        let ctx = Context::default();
        let mut app = DrawingApp::default();
        two_boxes(&mut app);
        app.tool = Tool::Arrow;
        frame(&mut app, &ctx, vec![]);
        // released near the edges -> orbit
        drag(&mut app, &ctx, Pos2::new(395.0, 250.0), Pos2::new(605.0, 250.0));
        let arrow = &app.scene.elements[2];
        assert!(!arrow.ends.start.as_ref().unwrap().inside && !arrow.ends.end.as_ref().unwrap().inside);
        assert_eq!(arrow.ends.start.as_ref().map(|b| &b.element_id), Some(&app.scene.elements[0].id));
        assert_eq!(arrow.ends.end.as_ref().map(|b| &b.element_id), Some(&app.scene.elements[1].id));
        // orbit: endpoints stop on the outline + gap (5 + stroke 2 / 2)
        let pts = app.line_points(2).to_vec();
        assert!((pts[0][0] - 406.0).abs() < 0.5 && (pts[1][0] - 594.0).abs() < 0.5, "{pts:?}");
        // move the right box down: the arrow end follows it
        app.selected = vec![1];
        drag(&mut app, &ctx, Pos2::new(650.0, 250.0), Pos2::new(650.0, 350.0));
        let end = app.line_points(2)[1];
        assert!(end[1] > 280.0 && end[0] < 600.0, "{end:?}");
        assert!(app.scene.elements[2].ends.end.is_some(), "moving the shape keeps the binding");
        // moving the arrow alone detaches it
        app.selected = vec![2];
        app.nudge(Vec2::new(0.0, 5.0));
        assert!(app.scene.elements[2].ends.start.is_none() && app.scene.elements[2].ends.end.is_none());
    }

    #[test]
    fn deep_inside_binds_to_the_exact_point() {
        let mut app = DrawingApp::default();
        two_boxes(&mut app);
        app.tool = Tool::Arrow;
        let arrow = app.bind_new_arrow(app.new_element(ElementKind::Line { points: vec![[350.0, 250.0], [650.0, 250.0]], arrow: true }));
        assert!(arrow.ends.start.as_ref().unwrap().inside && arrow.ends.end.as_ref().unwrap().inside);
        assert_eq!(arrow.kind, ElementKind::Line { points: vec![[350.0, 250.0], [650.0, 250.0]], arrow: true });
    }

    #[test]
    fn command_disables_binding_and_deleting_target_unbinds() {
        let ctx = Context::default();
        let mut app = DrawingApp::default();
        two_boxes(&mut app);
        app.tool = Tool::Arrow;
        app.mods = Modifiers::COMMAND;
        let arrow = app.bind_new_arrow(app.new_element(ElementKind::Line { points: vec![[350.0, 250.0], [650.0, 250.0]], arrow: true }));
        assert!(arrow.ends.start.is_none() && arrow.ends.end.is_none());
        app.mods = Modifiers::NONE;
        frame(&mut app, &ctx, vec![]);
        drag(&mut app, &ctx, Pos2::new(350.0, 250.0), Pos2::new(650.0, 250.0));
        app.selected = vec![1];
        app.delete_selected();
        app.update_bindings();
        assert!(app.scene.elements[1].ends.end.is_none());
    }

    #[test]
    fn copies_keep_bindings_only_with_their_targets() {
        let mut app = DrawingApp::default();
        two_boxes(&mut app);
        app.tool = Tool::Arrow;
        let arrow = app.bind_new_arrow(app.new_element(ElementKind::Line { points: vec![[350.0, 250.0], [650.0, 250.0]], arrow: true }));
        app.scene.elements.push(arrow);
        let both = DrawingApp::fresh_copies(&app.scene.elements, Vec2::ZERO);
        assert_eq!(both[2].ends.start.as_ref().unwrap().element_id, both[0].id);
        let alone = DrawingApp::fresh_copies(&app.scene.elements[2..], Vec2::ZERO);
        assert!(alone[0].ends.start.is_none());
    }

    #[test]
    fn box_selection_contain_vs_overlap() {
        let ctx = Context::default();
        let mut app = DrawingApp::default();
        two_boxes(&mut app); // 300..400 and 600..700, y 200..300
        frame(&mut app, &ctx, vec![]);
        // marquee over the whole first box and half of the second
        drag(&mut app, &ctx, Pos2::new(280.0, 180.0), Pos2::new(650.0, 320.0));
        assert_eq!(app.selected, vec![0], "Contido: only fully enclosed shapes");
        app.selected.clear();
        app.prefs.box_overlap = true;
        drag(&mut app, &ctx, Pos2::new(280.0, 180.0), Pos2::new(650.0, 320.0));
        assert_eq!(app.selected, vec![0, 1], "Sobreposto: touching is enough");
    }

    #[test]
    fn snapping_to_objects_and_grid() {
        let ctx = Context::default();
        let mut app = DrawingApp::default();
        two_boxes(&mut app);
        frame(&mut app, &ctx, vec![]);
        app.prefs.snap_objects = true;
        app.selected = vec![1];
        // move the right box so its top lands 5px below the other's top: it snaps to y = 200
        drag(&mut app, &ctx, Pos2::new(650.0, 250.0), Pos2::new(650.0, 305.0));
        let moved = element_bounds(&app.scene.elements[1]);
        assert_eq!(moved.min.y, 250.0, "snaps 5px to the other box's center line");
        app.prefs.snap_objects = false;
        app.prefs.grid = true;
        app.tool = Tool::Rectangle;
        drag(&mut app, &ctx, Pos2::new(452.0, 407.0), Pos2::new(538.0, 489.0));
        let drawn = element_bounds(app.scene.elements.last().unwrap());
        assert_eq!((drawn.min, drawn.max), (Pos2::new(460.0, 400.0), Pos2::new(540.0, 480.0)), "grid snaps to 20px");
    }

    #[test]
    fn binding_and_midpoint_preferences() {
        let mut app = DrawingApp::default();
        two_boxes(&mut app);
        app.tool = Tool::Arrow;
        // released near the right side's middle: snaps the anchor to [1, 0.5]
        let arrow = app.bind_new_arrow(app.new_element(ElementKind::Line { points: vec![[395.0, 262.0], [605.0, 250.0]], arrow: true }));
        assert_eq!(arrow.ends.start.as_ref().unwrap().fixed, [1.0, 0.5]);
        app.prefs.arrow_binding = false;
        let free = app.bind_new_arrow(app.new_element(ElementKind::Line { points: vec![[395.0, 262.0], [605.0, 250.0]], arrow: true }));
        assert!(free.ends.start.is_none() && free.ends.end.is_none());
    }

    #[test]
    fn locked_elements_ignore_the_pointer() {
        let ctx = Context::default();
        let mut app = DrawingApp::default();
        two_boxes(&mut app);
        frame(&mut app, &ctx, vec![]);
        app.selected = vec![0];
        app.toggle_lock_selected();
        assert!(app.scene.elements[0].locked && app.selected.is_empty());
        // clicking its outline or a marquee around it no longer selects it
        drag(&mut app, &ctx, Pos2::new(300.0, 250.0), Pos2::new(300.0, 250.0));
        assert!(app.selected.is_empty());
        drag(&mut app, &ctx, Pos2::new(280.0, 180.0), Pos2::new(720.0, 320.0));
        assert_eq!(app.selected, vec![1]);
        app.select_all();
        assert_eq!(app.selected, vec![1]);
        app.lock_all();
        assert!(app.scene.elements.iter().all(|e| e.locked));
        let id = app.scene.elements[1].id.clone();
        app.unlock(Some(&id));
        assert!(app.scene.elements[0].locked && !app.scene.elements[1].locked);
        app.unlock(None);
        assert!(app.scene.elements.iter().all(|e| !e.locked));
        assert_eq!(app.describe(&app.scene.elements[0]), "Retângulo");
    }

    #[test]
    fn frame_carries_its_contents() {
        let ctx = Context::default();
        let mut app = DrawingApp::default();
        app.tool = Tool::Frame;
        frame(&mut app, &ctx, vec![]);
        drag(&mut app, &ctx, Pos2::new(250.0, 150.0), Pos2::new(750.0, 400.0));
        assert_eq!(app.scene.elements[0].frame.as_deref(), Some("Frame"));
        app.scene.elements.push(app.new_element(ElementKind::Rectangle { x: 300.0, y: 200.0, width: 100.0, height: 100.0 }));
        app.scene.elements.push(app.new_element(ElementKind::Rectangle { x: 900.0, y: 200.0, width: 50.0, height: 50.0 }));
        // a click inside the frame doesn't grab it
        assert_eq!(app.hit(Pos2::new(600.0, 350.0)), None);
        // dragging its border moves the frame and what's inside, not what's outside
        app.tool = Tool::Select;
        app.selected = vec![0];
        drag(&mut app, &ctx, Pos2::new(250.0, 300.0), Pos2::new(250.0, 350.0));
        assert_eq!(element_bounds(&app.scene.elements[1]).min, Pos2::new(300.0, 250.0));
        assert_eq!(element_bounds(&app.scene.elements[2]).min, Pos2::new(900.0, 200.0));
        let back = parse_scene(&to_interop(&app.scene).to_string()).unwrap();
        assert_eq!(back.elements[0].frame.as_deref(), Some("Frame"));
    }

    #[test]
    fn extra_tools() {
        let ctx = Context::default();
        let mut app = DrawingApp::default();
        frame(&mut app, &ctx, vec![]);
        // nota adesiva: a click drops a 250px note, already editing its label; a drag is square
        app.tool = Tool::Sticky;
        drag(&mut app, &ctx, Pos2::new(400.0, 300.0), Pos2::new(401.0, 300.0));
        assert_eq!(element_bounds(&app.scene.elements[0]).size(), Vec2::splat(250.0));
        assert!(app.scene.elements[0].created.is_some());
        assert_eq!(app.editing, Some(1));
        app.finish_text();
        app.tool = Tool::Sticky;
        drag(&mut app, &ctx, Pos2::new(700.0, 300.0), Pos2::new(800.0, 340.0));
        assert_eq!(element_bounds(app.scene.elements.iter().rev().find(|e| e.created.is_some()).unwrap()).size(), Vec2::splat(100.0));
        app.finish_text();
        let back = parse_scene(&to_interop(&app.scene).to_string()).unwrap();
        assert_eq!(back.elements[0].created, app.scene.elements[0].created);
        assert_eq!(sticky_date(1_792_497_600_000, true).as_deref(), Some("20 Oct"));
        // dark mode uses the web's invert(93%) hue-rotate(180deg): the note turns brown
        assert_eq!(themed(Color32::from_rgb(0xff, 0xdf, 0x6b), true), Color32::from_rgb(75, 48, 0));
        assert_eq!(sticky_date(1_700_000_000_000, false).as_deref(), Some("14 Nov 2023"));
        // preenchimento paints the shape under the pointer
        app.scene.elements = vec![app.new_element(ElementKind::Rectangle { x: 100.0, y: 100.0, width: 100.0, height: 100.0 })];
        app.scene.elements[0].style.fill = None;
        app.fill_at(Pos2::new(150.0, 150.0));
        assert!(app.scene.elements[0].style.fill.is_some());
        // laço selects only what it surrounds
        app.scene.elements.push(app.new_element(ElementKind::Rectangle { x: 500.0, y: 100.0, width: 50.0, height: 50.0 }));
        app.select_in_lasso(&[Pos2::new(50.0, 50.0), Pos2::new(250.0, 50.0), Pos2::new(250.0, 250.0), Pos2::new(50.0, 250.0)]);
        assert_eq!(app.selected, vec![0]);
        // desenhar para criar forma: a round scribble becomes an ellipse, a straight one a line
        let circle: Vec<[f32; 2]> = (0..=40).map(|i| { let a = i as f32 / 40.0 * std::f32::consts::TAU; [100.0 + 50.0 * a.cos(), 100.0 + 50.0 * a.sin()] }).collect();
        assert!(matches!(recognize_shape(&app, &circle).unwrap().kind, ElementKind::Ellipse { .. }));
        let square = [[0.0, 0.0], [50.0, 0.0], [100.0, 0.0], [100.0, 50.0], [100.0, 100.0], [50.0, 100.0], [0.0, 100.0], [0.0, 50.0], [0.0, 2.0]];
        assert!(matches!(recognize_shape(&app, &square).unwrap().kind, ElementKind::Rectangle { .. }));
        let diamond = [[50.0, 0.0], [100.0, 50.0], [50.0, 100.0], [0.0, 50.0], [49.0, 1.0]];
        assert!(matches!(recognize_shape(&app, &diamond).unwrap().kind, ElementKind::Diamond { .. }));
        app.scene.elements.clear();
        let straight: Vec<[f32; 2]> = (0..10).map(|i| [i as f32 * 10.0, i as f32 * 5.0]).collect();
        assert!(matches!(recognize_shape(&app, &straight).unwrap().kind, ElementKind::Line { arrow: false, .. }));
        // a shaft with a head scribbled at its tip is an arrow
        let mut arrow: Vec<[f32; 2]> = (0..=20).map(|i| [i as f32 * 10.0, 0.0]).collect();
        arrow.extend([[185.0, -15.0], [200.0, 0.0], [185.0, 15.0], [200.0, 0.0], [185.0, -15.0], [200.0, 0.0], [185.0, 15.0]]);
        assert!(matches!(recognize_shape(&app, &arrow).unwrap().kind, ElementKind::Line { arrow: true, .. }));
        // a zigzag is no shape at all
        let zigzag: Vec<[f32; 2]> = (0..12).map(|i| [i as f32 * 20.0, if i % 2 == 0 { 0.0 } else { 80.0 }]).collect();
        assert!(recognize_shape(&app, &zigzag).is_none());
    }

    #[test]
    fn ai_drawings_replay_with_the_agent_name() {
        let path = std::env::temp_dir().join(format!("magoagent-replay-{}.rea", std::process::id()));
        let mut app = DrawingApp::default();
        app.autosave = Some(path.clone());
        let mut external = app.scene.clone();
        external.agent = Some("Claude Code".into());
        external.elements.push(app.new_element(ElementKind::Rectangle { x: 0.0, y: 0.0, width: 100.0, height: 50.0 }));
        external.elements.push(app.new_element(ElementKind::Text { x: 10.0, y: 10.0, value: "oi".into() }));
        write_scene_file(&path, &external).unwrap();
        app.sync_from_file();
        assert_eq!(app.reveal.len(), 2);
        assert_eq!(app.agent.as_ref().map(|a| a.0.as_str()), Some("Claude Code"));
        // the front element is drawn over time, then the next one starts
        let first = app.scene.elements[0].clone();
        assert_eq!(app.reveal_progress(&first), 0.0);
        app.now += 10.0;
        assert_eq!(app.reveal_progress(&first), 1.0);
        // the file the app writes back never claims to be the AI's
        assert_eq!(app.scene.agent, None);
        let _ = fs::remove_file(path);
    }

    #[test]
    fn locked_group_shows_one_unlock_button() {
        let ctx = Context::default();
        let mut app = DrawingApp::default();
        frame(&mut app, &ctx, vec![]);
        let mut a = app.new_element(ElementKind::Rectangle { x: 100.0, y: 100.0, width: 100.0, height: 100.0 });
        let mut b = app.new_element(ElementKind::Rectangle { x: 300.0, y: 100.0, width: 100.0, height: 100.0 });
        (a.group_id, b.group_id) = (Some("g".into()), Some("g".into()));
        let mut card = app.new_element(ElementKind::Rectangle { x: 100.0, y: 400.0, width: 300.0, height: 200.0 });
        card.frame = Some("Frame".into());
        let inside = app.new_element(ElementKind::Rectangle { x: 150.0, y: 450.0, width: 50.0, height: 50.0 });
        app.scene.elements = vec![a, b, card, inside];
        // locking one member of a group locks the group; locking a card locks what's in it
        app.selected = vec![0];
        app.toggle_lock_selected();
        app.selected = vec![2];
        app.toggle_lock_selected();
        assert!(app.scene.elements.iter().all(|e| e.locked));
        // clicking a locked element offers the unlock button for its whole unit
        app.tool = Tool::Select;
        drag(&mut app, &ctx, Pos2::new(100.0, 150.0), Pos2::new(100.0, 150.0));
        assert_eq!(app.active_locked.as_ref().map(Vec::len), Some(2));
        drag(&mut app, &ctx, Pos2::new(600.0, 300.0), Pos2::new(600.0, 300.0));
        assert_eq!(app.active_locked, None);
        // anywhere in a locked card, even on what's inside it, means the whole card
        for p in [Pos2::new(100.0, 500.0), Pos2::new(300.0, 550.0), Pos2::new(175.0, 475.0)] {
            drag(&mut app, &ctx, p, p);
            assert_eq!(app.active_locked.as_ref().map(Vec::len), Some(2));
        }
        // the unlock button frees the whole unit at once
        app.selected = app.lock_unit(&[2]);
        app.toggle_lock_selected();
        assert!(!app.scene.elements[2].locked && !app.scene.elements[3].locked && app.scene.elements[0].locked);
    }

    #[test]
    fn image_round_trip() {
        let mut png = Vec::new();
        image::RgbaImage::from_pixel(4, 2, image::Rgba([255, 0, 0, 255]))
            .write_to(&mut std::io::Cursor::new(&mut png), image::ImageFormat::Png)
            .unwrap();
        assert_eq!(base64_decode(&mcp::base64(&png)).unwrap(), png);
        let url = format!("data:image/png;base64,{}", mcp::base64(&png));
        let app = DrawingApp::default();
        let mut element = app.new_element(ElementKind::Rectangle { x: 0.0, y: 0.0, width: 40.0, height: 20.0 });
        element.image = Some(url.clone());
        let scene = Scene { elements: vec![element], ..Scene::default() };
        let interop = to_interop(&scene);
        assert_eq!(interop["elements"][0]["type"], "image");
        let back = parse_scene(&interop.to_string()).unwrap();
        assert_eq!(back.elements[0].image.as_deref(), Some(url.as_str()));
        // images are grabbed anywhere inside, and come out in the PNG export
        assert!(back.elements[0].contains(Pos2::new(20.0, 10.0), 1.0));
        let out = image::load_from_memory(&render_svg_png(&scene_svg(&scene, None), 1.0).unwrap()).unwrap().to_rgba8();
        assert_eq!(out.get_pixel(30, 20).0, [255, 0, 0, 255]);
    }

    #[test]
    fn view_mode_only_pans() {
        let ctx = Context::default();
        let mut app = DrawingApp::default();
        app.prefs.view_mode = true;
        app.tool = Tool::Rectangle;
        frame(&mut app, &ctx, vec![]);
        drag(&mut app, &ctx, Pos2::new(300.0, 300.0), Pos2::new(400.0, 350.0));
        assert!(app.scene.elements.is_empty());
        assert_eq!(app.pan, Vec2::new(100.0, 50.0));
    }

    #[test]
    fn text_png_contains_visible_pixels() {
        let mut app = DrawingApp::default();
        app.scene.elements.push(app.new_element(ElementKind::Text { x: 20.0, y: 40.0, value: "Planejamento".into() }));
        let png = render_svg_png(&scene_svg(&app.scene, Some(WHITE)), 1.0).unwrap();
        let img = image::load_from_memory(&png).unwrap().into_rgba8();
        assert!(img.pixels().filter(|p| p[0] < 100 && p[1] < 100 && p[2] < 100).count() > 30);
    }
}







