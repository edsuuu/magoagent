//! The floors' overview in 3D, as the reference app does it: the canvas tips back 55 degrees, shrinks to 0.55
//! and drops 130 points under a dark scrim, with the other floors as cards stacked above and below it, all
//! seen through a perspective of twice the canvas's height. Everything moves on springs (`Spring`, in closed
//! form like `ui/sidebar.rs::slide`): the stage's (response 0.4, no bounce) takes the canvas in and out and
//! fades the cards, the stack's (0.5, damping 0.75) slides the cards when the floor on view changes, and the
//! chips' (0.25) pop in. egui has no 3D, so the canvas is painted as it always is, tessellated and each vertex
//! projected (`project`); a floor off view is a card of its last picture (the shapes taken when the overview
//! opened) or, without one, its name and what it holds.

use crate::prelude::*;
use egui::epaint::{ClippedPrimitive, ClippedShape, Mesh, Primitive};
use egui::layers::ShapeIdx;
use std::collections::HashMap;

const TILT: f32 = 55.0;
const SCALE: f32 = 0.55;
const SPACING: f32 = 130.0;
const LIVE_OFFSET: f32 = 130.0;
const GAP: f32 = 30.0;
const PERSPECTIVE: f32 = 0.5;
const ABOVE: f32 = 0.35;
const BELOW: f32 = 0.7;
const RADIUS: f32 = 12.0;
const CELL: f32 = 20.0;
/// (response in seconds, damping fraction) of the reference app's `springFromResponse`.
pub(crate) const STAGE: (f32, f32) = (0.4, 1.0);
const STACK: (f32, f32) = (0.5, 0.75);
pub(crate) const ITEM: (f32, f32) = (0.25, 1.0);
/// What a chip waits per place from the bottom before it pops in.
pub(crate) const STAGGER: f64 = 0.03;

/// Where a spring of `response` and damping fraction `zeta` is `t` seconds after it left `d0` (from its
/// target) at speed `v0`: its displacement from the target and its speed. Stiffness (2π/response)², damping
/// 4πζ/response, mass 1: the same numbers Motion's `spring` gets.
pub(crate) fn spring_at(d0: f32, v0: f32, t: f32, (response, zeta): (f32, f32)) -> (f32, f32) {
    let t = t.max(0.0);
    let w = std::f32::consts::TAU / response;
    if zeta >= 1.0 {
        let b = v0 + w * d0;
        let e = (-w * t).exp();
        return ((d0 + b * t) * e, (b - w * (d0 + b * t)) * e);
    }
    let a = zeta * w;
    let wd = w * (1.0 - zeta * zeta).sqrt();
    let b = (v0 + a * d0) / wd;
    let (s, c) = (wd * t).sin_cos();
    let e = (-a * t).exp();
    (e * (d0 * c + b * s), e * ((b * wd - a * d0) * c - (a * b + d0 * wd) * s))
}

/// A value that follows its target on a spring, and keeps its speed when the target moves.
#[derive(Clone, Copy, Debug)]
pub(crate) struct Spring {
    from: f32,
    to: f32,
    speed: f32,
    since: f64,
    kind: (f32, f32),
}

impl Spring {
    pub(crate) fn new(at: f32, kind: (f32, f32)) -> Self {
        Self { from: at, to: at, speed: 0.0, since: 0.0, kind }
    }

    fn state(&self, now: f64) -> (f32, f32) {
        let (d, v) = spring_at(self.from - self.to, self.speed, (now - self.since) as f32, self.kind);
        (self.to + d, v)
    }

    pub(crate) fn value(&self, now: f64) -> f32 {
        self.state(now).0
    }

    /// Aims at `to`, `delay` seconds from `now`: a spring already going there is left alone.
    pub(crate) fn go(&mut self, to: f32, now: f64, delay: f64) {
        if (to - self.to).abs() < 1e-6 {
            return;
        }
        let (from, speed) = self.state(now);
        *self = Self { from, to, speed, since: now + delay, kind: self.kind };
    }

    /// Motion's rest: within 0.01 of the target and slower than 0.1 a second.
    pub(crate) fn done(&self, now: f64) -> bool {
        let (x, v) = self.state(now);
        now >= self.since && (x - self.to).abs() < 0.01 && v.abs() < 0.1
    }
}

/// How the canvas stands: tipped back `tilt` degrees, scaled, and moved down `y` points.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct Pose {
    pub(crate) tilt: f32,
    pub(crate) scale: f32,
    pub(crate) y: f32,
}

impl Pose {
    /// The canvas on view at stage progress `p` (0 at rest, 1 in the overview).
    pub(crate) fn live(p: f32) -> Self {
        Self { tilt: TILT * p, scale: 1.0 + (SCALE - 1.0) * p, y: LIVE_OFFSET * p }
    }

    pub(crate) fn card(y: f32) -> Self {
        Self { tilt: TILT, scale: SCALE, y }
    }
}

/// CSS `perspective: height / 0.5` over `translateY(y) scale(s) rotateX(tilt)`, about the stage's center.
pub(crate) fn project(stage: Rect, pose: Pose, at: Pos2) -> Pos2 {
    let (center, d) = (stage.center(), stage.height() / PERSPECTIVE);
    let q = at - center;
    let (sin, cos) = pose.tilt.to_radians().sin_cos();
    let z = q.y * sin;
    center + Vec2::new(q.x * pose.scale, q.y * cos * pose.scale + pose.y) * (d / (d - z))
}

/// How far a card sits from the stage's middle: `rel` floors above (+) or below (-) the one on view.
pub(crate) fn card_y(rel: i32) -> f32 {
    -rel as f32 * SPACING + LIVE_OFFSET - GAP * (rel as f32).clamp(-1.0, 1.0)
}

/// What a card fades to: none for the floor on view, 0.35 above it and 0.7 below, on its own layer only.
pub(crate) fn card_opacity(rel: i32, above_layer: bool) -> f32 {
    if rel == 0 || (rel > 0) != above_layer {
        0.0
    } else if rel > 0 {
        ABOVE
    } else {
        BELOW
    }
}

#[derive(Clone, Copy)]
struct Ghost {
    y: Spring,
    below: Spring,
    above: Spring,
}

/// A floor's picture: what the canvas painted the moment the overview opened.
pub(crate) struct Snap {
    rect: Rect,
    prims: Vec<(Rect, Mesh)>,
}

/// The overview's stage: whether it is on, how far along, and the cards.
pub(crate) struct FloorFx {
    pub(crate) engaged: bool,
    was_open: bool,
    capture: bool,
    progress: Spring,
    ghosts: HashMap<String, Ghost>,
    snaps: HashMap<String, Snap>,
}

impl Default for FloorFx {
    fn default() -> Self {
        Self { engaged: false, was_open: false, capture: false, progress: Spring::new(0.0, STAGE), ghosts: HashMap::new(), snaps: HashMap::new() }
    }
}

impl FloorFx {
    /// One frame: starts the stage when the overview opens or closes, and moves the cards to where `active`
    /// (an index into `stops`, the floors' ids from the ground up) puts them. Gives whether it is still moving.
    pub(crate) fn tick(&mut self, now: f64, open: bool, stops: &[String], active: usize) -> bool {
        if open && !self.was_open {
            // a stage that mounts: the cards start unseen and fade in, the canvas starts at rest
            (self.engaged, self.capture) = (true, true);
            self.progress = Spring::new(0.0, STAGE);
            self.ghosts.clear();
        }
        self.was_open = open;
        if !self.engaged {
            return false;
        }
        self.progress.go(if open { 1.0 } else { 0.0 }, now, 0.0);
        self.ghosts.retain(|id, _| stops.contains(id));
        let mut moving = !self.progress.done(now);
        for (i, id) in stops.iter().enumerate() {
            let rel = i as i32 - active as i32;
            let y = card_y(rel);
            let ghost = self.ghosts.entry(id.clone()).or_insert_with(|| Ghost {
                y: Spring::new(y, STACK),
                below: Spring::new(0.0, STAGE),
                above: Spring::new(0.0, STAGE),
            });
            ghost.y.go(y, now, 0.0);
            ghost.below.go(if open { card_opacity(rel, false) } else { 0.0 }, now, 0.0);
            ghost.above.go(if open { card_opacity(rel, true) } else { 0.0 }, now, 0.0);
            moving |= !(ghost.y.done(now) && ghost.below.done(now) && ghost.above.done(now));
        }
        // the stage lets go when the canvas is back (Motion's onAnimationComplete)
        if !open && self.progress.done(now) {
            self.engaged = false;
            self.ghosts.clear();
            return false;
        }
        moving
    }

    pub(crate) fn progress(&self, now: f64) -> f32 {
        self.progress.value(now).clamp(0.0, 1.0)
    }

    /// Forgets the pictures (a new space or theme makes them stale).
    pub(crate) fn forget(&mut self) {
        self.snaps.clear();
    }
}

fn project_mesh(mesh: &Mesh, stage: Rect, pose: Pose, opacity: f32) -> Mesh {
    let mut out = mesh.clone();
    for v in &mut out.vertices {
        v.pos = project(stage, pose, v.pos);
        if opacity < 1.0 {
            v.color = v.color.gamma_multiply(opacity);
        }
    }
    out
}

fn meshes(prims: Vec<ClippedPrimitive>) -> Vec<(Rect, Mesh)> {
    prims.into_iter().filter_map(|p| if let Primitive::Mesh(m) = p.primitive { Some((p.clip_rect, m)) } else { None }).collect()
}

/// What a floor off view needs to be drawn as a card.
pub(crate) struct CardView {
    pub(crate) id: String,
    pub(crate) name: String,
    pub(crate) detail: String,
    pub(crate) color: Option<[u8; 3]>,
}

impl DrawingApp {
    /// The canvas, and over it (while the overview is on or settling) the stage of floors in 3D.
    pub(crate) fn paint_stage(&mut self, ctx: &Context, painter: &Painter, rect: Rect) {
        let now = ctx.input(|i| i.time);
        let space = self.current_space().cloned();
        let ids: Vec<String> = std::iter::once(String::new()).chain(space.iter().flat_map(|w| w.floors.iter().map(|f| f.id.clone()))).collect();
        let active_id = space.as_ref().map_or(String::new(), |w| w.floor.clone());
        let active = ids.iter().position(|id| *id == active_id).unwrap_or(0);
        let open = self.floors_ui.overview;
        if self.floors_ui.stale != (self.spaces.current.clone(), self.dark()) {
            self.floors_ui.stale = (self.spaces.current.clone(), self.dark());
            self.floors_ui.fx.forget();
        }
        let moving = self.floors_ui.fx.tick(now, open, &ids, active);
        if !self.floors_ui.fx.engaged {
            self.paint_scene(painter, rect);
            return;
        }
        // the overview opens on a canvas with nothing selected (the picture would carry its handles)
        if self.floors_ui.fx.capture {
            self.selected.clear();
        }
        // the canvas as shapes: painted as ever, taken back out of the layer, tessellated
        let (layer, ppp) = (painter.layer_id(), ctx.pixels_per_point());
        let len = || ctx.graphics(|g| g.get(layer).map_or(0, |l| l.all_entries().len()));
        let start = len();
        self.paint_scene(painter, rect);
        let taken: Vec<ClippedShape> = ctx.graphics_mut(|g| {
            let list = g.entry(layer);
            let shapes: Vec<ClippedShape> = list.all_entries().skip(start).cloned().collect();
            (start..start + shapes.len()).for_each(|i| list.reset_shape(ShapeIdx(i)));
            shapes
        });
        let live = meshes(ctx.tessellate(taken, ppp));
        let dark = self.dark();
        let (ground, ink) = (self.ground(), glass_ink(dark));
        let p = self.floors_ui.fx.progress(now);
        if self.floors_ui.fx.capture {
            self.floors_ui.fx.capture = false;
            let prims = live.iter().map(|(c, m)| (*c, m.clone())).collect();
            self.floors_ui.fx.snaps.insert(active_id.clone(), Snap { rect, prims });
        }
        // the scrim over what is behind the canvas
        painter.rect_filled(rect, 0, ground);
        painter.rect_filled(rect, 0, Color32::from_black_alpha((if dark { 140.0 } else { 77.0 } * p) as u8));
        let views: Vec<CardView> = ids
            .iter()
            .enumerate()
            .map(|(i, id)| {
                let (name, color) = match space.as_ref().and_then(|w| w.floors.iter().find(|f| f.id == *id)) {
                    Some(f) => (f.name.clone(), Some(f.color)),
                    None => (crate::app::floors::GROUND.to_string(), None),
                };
                let items = self.floors_ui.counts.get(id).copied().unwrap_or_default();
                CardView { id: id.clone(), name, detail: crate::app::floors::pill::items_text(items), color }
            })
            .collect();
        let fx = &self.floors_ui.fx;
        for above in [false, true] {
            if above {
                for (rect_clip, mesh) in &live {
                    let _ = rect_clip;
                    painter.add(Shape::mesh(project_mesh(mesh, rect, Pose::live(p), 1.0)));
                }
            }
            for view in &views {
                let Some(g) = fx.ghosts.get(&view.id) else { continue };
                let opacity = if above { g.above } else { g.below }.value(now).clamp(0.0, 1.0);
                if opacity > 0.002 {
                    paint_card(ctx, painter, rect, Pose::card(g.y.value(now)), opacity, view, fx.snaps.get(&view.id), ground, ink, dark);
                }
            }
        }
        if moving {
            ctx.request_repaint();
        }
    }
}

/// A floor off view: its picture, or its name and what it holds, on a grid, with its edge.
#[allow(clippy::too_many_arguments)]
fn paint_card(
    ctx: &Context,
    painter: &Painter,
    stage: Rect,
    pose: Pose,
    opacity: f32,
    view: &CardView,
    snap: Option<&Snap>,
    ground: Color32,
    ink: Color32,
    dark: bool,
) {
    let mut out: Vec<Mesh> = Vec::new();
    match snap.filter(|s| s.rect.size() == stage.size()) {
        Some(snap) => out.extend(snap.prims.iter().map(|(_, m)| m.clone())),
        None => {
            let line = if dark { Color32::from_rgb(0x30, 0x30, 0x30) } else { Color32::from_black_alpha(0x14) }.gamma_multiply(0.65);
            let mut shapes = vec![Shape::rect_filled(stage, RADIUS, ground)];
            let mut x = stage.left() + CELL;
            while x < stage.right() {
                shapes.push(Shape::vline(x, stage.y_range(), Stroke::new(1.0, line)));
                x += CELL;
            }
            let mut y = stage.top() + CELL;
            while y < stage.bottom() {
                shapes.push(Shape::hline(stage.x_range(), y, Stroke::new(1.0, line)));
                y += CELL;
            }
            let (name, detail) = (
                painter.layout_no_wrap(view.name.clone(), FontId::proportional(24.0), ink.gamma_multiply(0.62)),
                painter.layout_no_wrap(view.detail.clone(), FontId::proportional(13.0), ink.gamma_multiply(0.42)),
            );
            let c = stage.center();
            shapes.push(Shape::galley(Pos2::new(c.x - name.size().x / 2.0, c.y - 25.0), name, ink));
            shapes.push(Shape::galley(Pos2::new(c.x - detail.size().x / 2.0, c.y + 9.0), detail, ink));
            out.extend(
                meshes(ctx.tessellate(shapes.into_iter().map(|s| ClippedShape::new(Rect::EVERYTHING, s)).collect(), ctx.pixels_per_point()))
                    .into_iter()
                    .map(|(_, m)| m),
            );
        }
    }
    // its edge: the floor's color, or the hairline
    let edge = match view.color {
        Some([r, g, b]) => Color32::from_rgb(r, g, b).gamma_multiply(0.55),
        None if dark => Color32::from_white_alpha(0x24),
        None => Color32::from_black_alpha(0x2e),
    };
    let border = Shape::rect_stroke(stage, RADIUS, Stroke::new(1.0, edge), StrokeKind::Inside);
    out.extend(meshes(ctx.tessellate(vec![ClippedShape::new(Rect::EVERYTHING, border)], ctx.pixels_per_point())).into_iter().map(|(_, m)| m));
    for mesh in out {
        painter.add(Shape::mesh(project_mesh(&mesh, stage, pose, opacity)));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_spring_starts_at_rest_and_ends_on_target() {
        for kind in [STAGE, STACK, ITEM] {
            let (start, _) = spring_at(1.0, 0.0, 0.0, kind);
            assert!((start - 1.0).abs() < 1e-6);
            let (mid, v) = spring_at(1.0, 0.0, kind.0 / 2.0, kind);
            assert!(mid < 0.9 && mid > -0.2, "{kind:?} {mid}");
            assert!(v < 0.0);
            let (end, _) = spring_at(1.0, 0.0, 3.0, kind);
            assert!(end.abs() < 1e-3);
        }
        // the stage's spring never passes its target; the stack's does, a little
        assert!((0..100).all(|i| spring_at(1.0, 0.0, i as f32 * 0.02, STAGE).0 >= 0.0));
        assert!((0..100).any(|i| spring_at(1.0, 0.0, i as f32 * 0.02, STACK).0 < 0.0));
    }

    #[test]
    fn the_canvas_tips_back_like_the_reference() {
        let stage = Rect::from_min_size(Pos2::new(260.0, 0.0), Vec2::new(940.0, 800.0));
        assert_eq!(project(stage, Pose::live(0.0), stage.left_top()), stage.left_top());
        let pose = Pose::live(1.0);
        let (top, bottom) = (project(stage, pose, stage.center_top()), project(stage, pose, stage.center_bottom()));
        // measured in the reference app: the card's top edge near y 403 and its foot near y 720
        assert!((top.y - 403.0).abs() < 2.0, "{top:?}");
        assert!((bottom.y - 722.0).abs() < 3.0, "{bottom:?}");
        let left = project(stage, pose, stage.left_bottom());
        assert!((left.x - 405.0).abs() < 4.0, "{left:?}");
    }

    #[test]
    fn the_cards_stack_above_and_below() {
        assert_eq!(card_y(0), 130.0);
        assert_eq!(card_y(1), -30.0);
        assert_eq!(card_y(-1), 290.0);
        assert_eq!((card_opacity(0, true), card_opacity(1, true), card_opacity(1, false), card_opacity(-1, false)), (0.0, 0.35, 0.0, 0.7));
    }

    #[test]
    fn the_overview_opens_moves_on_a_switch_and_closes() {
        let stops: Vec<String> = ["", "a", "b"].iter().map(|s| s.to_string()).collect();
        let mut fx = FloorFx::default();
        assert!(!fx.tick(0.0, false, &stops, 0) && !fx.engaged);
        // opens: engaged at rest, then the stage runs to 1
        assert!(fx.tick(1.0, true, &stops, 0) && fx.engaged);
        assert!(fx.progress(1.0) < 0.01 && fx.progress(1.2) > 0.1 && fx.progress(1.2) < 0.9);
        assert!(fx.progress(3.0) > 0.999);
        assert!(!fx.tick(3.0, true, &stops, 0));
        // a switch moves the cards (the stack's spring) and settles
        assert!(fx.tick(3.1, true, &stops, 1));
        assert!((fx.ghosts["b"].y.value(3.1) - card_y(2)).abs() < 1.0 && fx.ghosts["b"].y.value(3.3) > card_y(2) + 60.0);
        assert!(!fx.tick(6.0, true, &stops, 1));
        // closes: still engaged while the canvas comes back, then lets go
        assert!(fx.tick(6.1, false, &stops, 1) && fx.engaged);
        assert!(!fx.tick(8.0, false, &stops, 1) && !fx.engaged);
    }
}
