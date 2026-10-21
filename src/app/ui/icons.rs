//! Vector icons drawn with the painter.

use crate::prelude::*;

#[derive(Clone, Copy)]
pub(crate) enum Icon {
    Tool(Tool),
    Shapes,
    Edit,
    Connect,
    Restart,
    Expand,
    Composer,
    Agents,
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
/// `paint_icon` scaled around its center and raised `lift` points (the action bar's small
/// icons, and their hop on click).
pub(crate) fn paint_icon_scaled(p: &Painter, icon: Icon, c: Pos2, color: Color32, scale: f32, lift: f32) {
    let layer = p.layer_id();
    let next = || p.ctx().graphics_mut(|g| g.entry(layer).next_idx());
    let start = next();
    paint_icon(p, icon, c, color);
    let end = next();
    let transform = egui::emath::TSTransform::new(c.to_vec2() * (1.0 - scale) - Vec2::new(0.0, lift), scale);
    p.ctx().graphics_mut(|g| g.entry(layer).transform_range(start, end, transform));
}

/// How high a clicked icon is `t` seconds later: a jump, a smaller bounce, then rest.
pub(crate) fn hop_height(t: f64) -> f32 {
    let arc = |t: f64, len: f64, height: f64| if (0.0..len).contains(&t) { height * (std::f64::consts::PI * t / len).sin() } else { 0.0 };
    (arc(t, 0.22, 4.0) + arc(t - 0.22, 0.14, 1.5)) as f32
}

pub(crate) fn paint_icon(p: &Painter, icon: Icon, c: Pos2, color: Color32) {
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
        Icon::Tool(Tool::Portal) => {
            p.circle_stroke(at(0.0, 0.0), 7.0, s);
            line(&[(-7.0, 0.0), (7.0, 0.0)]);
            line(&[(-6.0, -3.5), (6.0, -3.5)]);
            line(&[(-6.0, 3.5), (6.0, 3.5)]);
            let meridian: Vec<(f32, f32)> = (0..=12).map(|k| {
                let a = -std::f32::consts::FRAC_PI_2 + k as f32 * std::f32::consts::PI / 12.0;
                (3.2 * a.cos(), 7.0 * a.sin())
            }).collect();
            line(&meridian);
            line(&meridian.iter().map(|(x, y)| (-x, *y)).collect::<Vec<_>>());
        }
        Icon::Tool(Tool::Ide) => {
            closed(&[(-7.0, -6.0), (7.0, -6.0), (7.0, 6.0), (-7.0, 6.0)]);
            line(&[(-2.5, -3.0), (-5.0, 0.0), (-2.5, 3.0)]);
            line(&[(2.5, -3.0), (5.0, 0.0), (2.5, 3.0)]);
            line(&[(1.0, -3.5), (-1.0, 3.5)]);
        }
        Icon::Tool(Tool::Terminal) => {
            closed(&[(-7.0, -6.0), (7.0, -6.0), (7.0, 6.0), (-7.0, 6.0)]);
            line(&[(-4.0, -2.5), (-1.5, 0.0), (-4.0, 2.5)]);
            line(&[(0.5, 2.5), (4.0, 2.5)]);
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
        Icon::Edit => {
            rect(-6.5, -5.0, 11.5, 11.5, 2);
            line(&[(-1.5, 2.0), (6.5, -6.0)]);
            line(&[(-1.5, 2.0), (-2.0, 3.0)]);
        }
        Icon::Connect => {
            line(&[(0.0, 7.0), (0.0, 0.0)]);
            line(&[(-5.5, -6.0), (0.0, 0.0), (5.5, -6.0)]);
            p.circle_filled(at(-5.5, -6.0), 1.4, color);
            p.circle_filled(at(5.5, -6.0), 1.4, color);
            p.circle_filled(at(0.0, 7.0), 1.4, color);
        }
        Icon::Restart => {
            let pts: Vec<(f32, f32)> = (0..=10).map(|k| {
                let a = -2.6 + k as f32 * 0.5;
                (6.0 * a.cos(), 6.0 * a.sin())
            }).collect();
            line(&pts);
            let (ex, ey) = pts[0];
            line(&[(ex + 3.5, ey - 1.0), (ex, ey), (ex + 0.5, ey + 3.5)]);
        }
        Icon::Expand => {
            line(&[(1.5, -6.5), (6.5, -6.5), (6.5, -1.5)]);
            line(&[(6.5, -6.5), (1.5, -1.5)]);
            line(&[(-1.5, 6.5), (-6.5, 6.5), (-6.5, 1.5)]);
            line(&[(-6.5, 6.5), (-1.5, 1.5)]);
        }
        Icon::Agents => {
            // a roster: a dot and a line per agent
            for y in [-4.5, 0.0, 4.5] {
                p.circle_filled(at(-5.0, y), 1.4, color);
                line(&[(-2.0, y), (6.0, y)]);
            }
        }
        Icon::Composer => {
            rect(-7.0, -6.0, 14.0, 9.0, 2);
            line(&[(-4.0, -2.0), (3.0, -2.0)]);
            for x in [-5.0, 0.0, 5.0] {
                p.circle_filled(at(x, 6.0), 1.2, color);
            }
        }
        Icon::Shapes => {
            rect(-7.0, -1.0, 8.0, 8.0, 1);
            p.circle_stroke(at(2.5, -2.5), 4.5, s);
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
pub(crate) fn crescent(c: Pos2) -> Vec<Pos2> {
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
