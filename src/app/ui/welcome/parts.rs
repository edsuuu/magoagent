//! The pieces Luna's scenes are drawn with, in the app's look: the mini canvas with its dotted grid, a terminal's card,
//! a chat's, a file tree's, a sticky note, a rope with a message on it, a little screen with the island's notch and
//! its agents, and Luna's speech bubble. Each takes its place on screen, `zoom` (points per design unit) and how much
//! of it shows (`alpha`).

use super::super::settings::kit::{K, semibold};
use crate::app::mascot::mago::paint_mini;
use crate::prelude::*;

/// The island's notch: near black in both themes, its text and its amber, as the island draws them.
pub(super) const NOTCH: Color32 = Color32::from_rgb(0x0a, 0x0a, 0x0c);
const NOTCH_ROW: Color32 = Color32::from_rgb(0x1a, 0x1b, 0x1f);
const NOTCH_INK: Color32 = Color32::from_rgb(0xf5, 0xf6, 0xf8);
const NOTCH_DIM: Color32 = Color32::from_rgb(0x8e, 0x93, 0x9c);
pub(super) const AMBER: Color32 = Color32::from_rgb(0xf5, 0xa5, 0x24);
pub(super) const GREEN: Color32 = Color32::from_rgb(0x2f, 0x9e, 0x44);
const GOLD: Color32 = Color32::from_rgb(0xff, 0xca, 0x4a);

/// Luna's sparkles: her gold, deeper on a light ground so they still show.
pub(super) fn gold(k: &K) -> Color32 {
    if k.dark { GOLD } else { Color32::from_rgb(0xf0, 0xa4, 0x18) }
}
/// A message coming in on a terminal (`> ` lines), in the blue of the ropes.
const INCOMING: Color32 = Color32::from_rgb(0x7c, 0xc4, 0xff);
/// The mini canvas's grid step, in design units.
const GRID: f32 = 18.0;

/// The soft shadow under a card.
fn shadow(painter: &Painter, rect: Rect, radius: f32, alpha: f32, dark: bool) {
    let color = Color32::from_black_alpha(if dark { 90 } else { 26 }).gamma_multiply(alpha);

    painter.add(egui::epaint::Shadow { offset: [0, 3], blur: 12, spread: 0, color }.as_shape(rect, radius));
}

/// A card's ground in the theme: the app's floating panels (#242424 in the dark, white in the light).
pub(super) fn surface(k: &K) -> Color32 {
    if k.dark { Color32::from_rgb(0x24, 0x24, 0x24) } else { Color32::WHITE }
}

/// The mini canvas: the app's ground and its dotted grid moved by `pan` (points) in `rect`. What goes on it is drawn
/// with the painter it returns, clipped to it.
pub(super) fn board(painter: &Painter, rect: Rect, zoom: f32, pan: Vec2, k: &K) -> Painter {
    let radius = 14.0 * zoom;

    shadow(painter, rect, radius, 1.0, k.dark);
    let ground = if k.dark { CANVAS_DARK } else { Color32::WHITE };

    painter.rect(rect, radius, ground, Stroke::new(1.0, k.border), StrokeKind::Inside);
    let step = GRID * zoom;
    let dot = if k.dark { Color32::from_white_alpha(34) } else { Color32::from_black_alpha(40) };

    // only the dots inside the round corners
    let inside = |at: Pos2| {
        let corner = Pos2::new(at.x.clamp(rect.left() + radius, rect.right() - radius), at.y.clamp(rect.top() + radius, rect.bottom() - radius));

        at.distance(corner) < radius - 3.0
    };

    let mut y = rect.top() + pan.y.rem_euclid(step);

    while y < rect.bottom() {
        let mut x = rect.left() + pan.x.rem_euclid(step);

        while x < rect.right() {
            let at = Pos2::new(x, y);

            if inside(at) {
                painter.circle_filled(at, (0.9 * zoom).max(0.8), dot);
            }

            x += step;
        }

        y += step;
    }

    painter.with_clip_rect(rect.shrink(1.0))
}

/// The first `shown` of `lines` (a whole line each, the one after them cut by the fraction), for text coming in.
fn coming_in<'a>(lines: &[&'a str], shown: f32) -> impl Iterator<Item = &'a str> {
    let whole = shown.max(0.0).floor() as usize;
    let part = shown.fract();

    lines.iter().enumerate().take_while(move |(index, _)| *index < whole || (*index == whole && part > 0.0)).map(move |(index, line)| {
        if index < whole {
            *line
        } else {
            let cut = (line.chars().count() as f32 * part).round() as usize;

            line.char_indices().nth(cut).map_or(*line, |(at, _)| &line[..at])
        }
    })
}

/// A terminal's card as the canvas draws one, dark in both themes: the title strip with its mark, its name and its state
/// dot, and the first `shown` of its `lines` in monospace (`$ ` a command, `> ` a message from another agent).
#[allow(clippy::too_many_arguments)]
pub(super) fn terminal(painter: &Painter, rect: Rect, zoom: f32, alpha: f32, name: &str, lines: &[&str], shown: f32, dot: Color32, dark: bool) {
    let radius = 6.0 * zoom;

    shadow(painter, rect, radius, alpha, dark);
    let fade = |color: Color32| color.gamma_multiply(alpha);

    painter.rect(rect, radius, fade(TERMINAL_BG), Stroke::new(1.0, fade(Color32::from_gray(0x3a))), StrokeKind::Inside);
    let painter = painter.with_clip_rect(rect.shrink(1.0));
    let header = Rect::from_min_size(rect.min, Vec2::new(rect.width(), 16.0 * zoom));
    let strip = Color32::from_rgb(0x24, 0x24, 0x24);
    let round = radius.min(255.0) as u8;

    painter.rect_filled(header, egui::CornerRadius { nw: round, ne: round, sw: 0, se: 0 }, fade(strip));
    painter.hline(header.x_range(), header.bottom() - 0.5, Stroke::new(1.0, fade(TERMINAL_FG.gamma_multiply(0.14))));
    let middle = header.center().y;
    let mark = Rect::from_center_size(Pos2::new(header.left() + 11.0 * zoom, middle), Vec2::new(9.0, 7.0) * zoom);

    painter.rect_stroke(mark, 1.5 * zoom, Stroke::new((0.8 * zoom).max(0.6), fade(TERMINAL_FG.gamma_multiply(0.8))), StrokeKind::Inside);
    let chevron = [mark.left_top() + Vec2::new(2.0, 2.0) * zoom, mark.left_top() + Vec2::new(3.8, 3.5) * zoom, mark.left_top() + Vec2::new(2.0, 5.0) * zoom];

    painter.add(Shape::line(chevron.to_vec(), Stroke::new((0.8 * zoom).max(0.6), fade(TERMINAL_FG))));
    painter.text(Pos2::new(mark.right() + 4.0 * zoom, middle), egui::Align2::LEFT_CENTER, name, FontId::proportional(7.5 * zoom), fade(TERMINAL_FG));
    painter.circle_filled(Pos2::new(header.right() - 9.0 * zoom, middle), 2.6 * zoom, fade(dot));
    let font = FontId::monospace(6.6 * zoom);

    for (index, line) in coming_in(lines, shown).enumerate() {
        let color = match line.chars().next() {
            Some('$') => TERMINAL_FG,
            Some('>') => INCOMING,
            _ => TERMINAL_FG.gamma_multiply(0.62),
        };

        let at = Pos2::new(rect.left() + 7.0 * zoom, header.bottom() + (6.0 + index as f32 * 9.5) * zoom);

        painter.text(at, egui::Align2::LEFT_TOP, line, font.clone(), fade(color));
    }
}

/// A chat's card: its title, a question in blue, the agent's answer coming in with `progress` and the composer.
pub(super) fn chat(painter: &Painter, rect: Rect, zoom: f32, alpha: f32, progress: f32, k: &K) {
    let fade = |color: Color32| color.gamma_multiply(alpha);
    let radius = 8.0 * zoom;

    shadow(painter, rect, radius, alpha, k.dark);
    painter.rect(rect, radius, fade(surface(k)), Stroke::new(1.0, fade(k.border)), StrokeKind::Inside);
    let painter = painter.with_clip_rect(rect.shrink(1.0));
    let header = Rect::from_min_size(rect.min, Vec2::new(rect.width(), 16.0 * zoom));

    painter.text(Pos2::new(header.left() + 8.0 * zoom, header.center().y), egui::Align2::LEFT_CENTER, "Novo chat", semibold(7.5 * zoom), fade(k.fg));
    painter.hline(header.x_range(), header.bottom() - 0.5, Stroke::new(1.0, fade(k.hairline)));
    // the question, at the right in blue
    let question = painter.layout_no_wrap("Faça a tela de login".into(), FontId::proportional(7.0 * zoom), Color32::WHITE);

    let ask = Rect::from_min_size(
        Pos2::new(rect.right() - 8.0 * zoom - question.size().x - 12.0 * zoom, header.bottom() + 9.0 * zoom),
        question.size() + Vec2::new(12.0, 8.0) * zoom,
    );

    painter.rect_filled(ask, 6.0 * zoom, fade(PRIMARY));
    painter.galley(ask.min + Vec2::new(6.0, 4.0) * zoom, question, fade(Color32::WHITE));
    // the answer: two lines of words, then lines of its work
    let mut y = ask.bottom() + 10.0 * zoom;

    for line in coming_in(&["Claro! Vou criar a tela", "e os testes dela."], progress * 4.0) {
        painter.text(Pos2::new(rect.left() + 8.0 * zoom, y), egui::Align2::LEFT_TOP, line, FontId::proportional(7.0 * zoom), fade(k.fg));
        y += 10.0 * zoom;
    }

    y += 4.0 * zoom;
    let inner = rect.width() - 16.0 * zoom;

    for (index, width) in [0.86, 0.64, 0.78, 0.5, 0.7].into_iter().enumerate() {
        let grow = (progress * 6.0 - 2.0 - index as f32 * 0.6).clamp(0.0, 1.0);

        if grow > 0.0 {
            let bar = Rect::from_min_size(Pos2::new(rect.left() + 8.0 * zoom, y), Vec2::new(inner * width * grow, 4.5 * zoom));

            painter.rect_filled(bar, 2.25 * zoom, fade(k.muted_fg.gamma_multiply(0.35)));
        }

        y += 9.0 * zoom;
    }

    // the composer
    let composer = Rect::from_min_max(Pos2::new(rect.left() + 6.0 * zoom, rect.bottom() - 24.0 * zoom), rect.max - Vec2::splat(6.0 * zoom));

    painter.rect(composer, 9.0 * zoom, fade(k.well), Stroke::new(1.0, fade(k.well_line)), StrokeKind::Inside);
    let middle = composer.center().y;

    painter.text(
        Pos2::new(composer.left() + 8.0 * zoom, middle),
        egui::Align2::LEFT_CENTER,
        "Pergunte algo…",
        FontId::proportional(6.5 * zoom),
        fade(k.muted_fg),
    );

    let send = Pos2::new(composer.right() - 9.0 * zoom, middle);

    painter.circle_filled(send, 6.0 * zoom, fade(PRIMARY));
    let arrow = Stroke::new((1.1 * zoom).max(0.8), fade(Color32::WHITE));

    painter.line_segment([send + Vec2::new(0.0, 3.0) * zoom, send - Vec2::new(0.0, 3.0) * zoom], arrow);
    painter.add(Shape::line(vec![send + Vec2::new(-2.5, -0.5) * zoom, send - Vec2::new(0.0, 3.0) * zoom, send + Vec2::new(2.5, -0.5) * zoom], arrow));
}

/// A file tree's card: the folder's name, its first rows coming in with `progress`, and the search field.
pub(super) fn tree(painter: &Painter, rect: Rect, zoom: f32, alpha: f32, progress: f32, k: &K) {
    const ROWS: [(&str, Option<Color32>); 8] = [
        (".claude", None),
        ("assets", None),
        ("docs", None),
        ("src", None),
        ("AGENTS.md", Some(Color32::from_rgb(0x4d, 0x9b, 0xf8))),
        ("Cargo.toml", Some(Color32::from_rgb(0xe8, 0x8a, 0x3c))),
        ("Cargo.lock", Some(Color32::from_rgb(0x9a, 0x9a, 0xa2))),
        ("README.md", Some(Color32::from_rgb(0x4d, 0x9b, 0xf8))),
    ];

    let fade = |color: Color32| color.gamma_multiply(alpha);
    let radius = 8.0 * zoom;

    shadow(painter, rect, radius, alpha, k.dark);
    painter.rect(rect, radius, fade(surface(k)), Stroke::new(1.0, fade(k.border)), StrokeKind::Inside);
    let painter = painter.with_clip_rect(rect.shrink(1.0));
    let header = Rect::from_min_size(rect.min, Vec2::new(rect.width(), 16.0 * zoom));

    painter.text(
        Pos2::new(header.left() + 8.0 * zoom, header.center().y),
        egui::Align2::LEFT_CENTER,
        "‹ ›",
        FontId::proportional(7.5 * zoom),
        fade(k.muted_fg),
    );

    painter.text(Pos2::new(header.left() + 22.0 * zoom, header.center().y), egui::Align2::LEFT_CENTER, "magoagent", semibold(7.5 * zoom), fade(k.fg));
    painter.hline(header.x_range(), header.bottom() - 0.5, Stroke::new(1.0, fade(k.hairline)));
    let thin = Stroke::new((0.8 * zoom).max(0.6), fade(k.muted_fg));
    let rows = (progress * ROWS.len() as f32).ceil() as usize;

    for (index, (name, file)) in ROWS.into_iter().take(rows).enumerate() {
        let middle = header.bottom() + (10.0 + index as f32 * 13.0) * zoom;
        let icon = Pos2::new(rect.left() + 13.0 * zoom, middle);

        match file {
            None => {
                painter.text(icon - Vec2::new(7.0 * zoom, 0.0), egui::Align2::CENTER_CENTER, "›", FontId::proportional(6.5 * zoom), fade(k.muted_fg));
                let folder = Rect::from_center_size(icon + Vec2::new(0.0, 0.5 * zoom), Vec2::new(8.0, 6.0) * zoom);

                painter.rect_stroke(folder, 1.2 * zoom, thin, StrokeKind::Inside);
                painter.line_segment([folder.left_top() + Vec2::new(0.5, -1.0) * zoom, folder.left_top() + Vec2::new(3.5, -1.0) * zoom], thin);
            }
            Some(color) => {
                painter.rect_filled(Rect::from_center_size(icon, Vec2::new(6.0, 7.5) * zoom), 1.2 * zoom, fade(color));
            }
        }

        painter.text(Pos2::new(icon.x + 8.0 * zoom, middle), egui::Align2::LEFT_CENTER, name, FontId::proportional(7.0 * zoom), fade(k.fg));
    }

    let search = Rect::from_min_max(Pos2::new(rect.left() + 6.0 * zoom, rect.bottom() - 20.0 * zoom), rect.max - Vec2::splat(6.0 * zoom));

    painter.rect(search, 4.0 * zoom, fade(k.well), Stroke::new(1.0, fade(k.well_line)), StrokeKind::Inside);
    painter.text(search.left_center() + Vec2::new(7.0 * zoom, 0.0), egui::Align2::LEFT_CENTER, "Buscar", FontId::proportional(6.5 * zoom), fade(k.muted_fg));
}

/// A line of a note: its text, and whether it is a box to tick (and ticked).
pub(super) type NoteLine = (&'static str, Option<bool>);

/// A sticky note in one of the app's papers (`render::NOTE_PAPERS`): the header strip with its name, the heading and
/// its lines in the paper's ink, and a strip of tape holding it when it has just stuck.
#[allow(clippy::too_many_arguments)]
pub(super) fn note(painter: &Painter, rect: Rect, zoom: f32, alpha: f32, paper: usize, title: &str, lines: &[NoteLine], tape: f32, dark: bool) {
    let [r, g, b, _] = NOTE_PAPERS[paper].0;
    let [ink_r, ink_g, ink_b] = NOTE_INK[paper];
    let fade = |color: Color32| color.gamma_multiply(alpha);
    let ink = Color32::from_rgb(ink_r, ink_g, ink_b);
    let radius = 6.0 * zoom;

    shadow(painter, rect, radius, alpha, dark);
    painter.rect(rect, radius, fade(Color32::from_rgb(r, g, b)), Stroke::new(0.5, fade(Color32::from_black_alpha(20))), StrokeKind::Inside);
    let header = Rect::from_min_size(rect.min, Vec2::new(rect.width(), 11.0 * zoom));
    let round = radius.min(255.0) as u8;

    painter.rect_filled(header, egui::CornerRadius { nw: round, ne: round, sw: 0, se: 0 }, fade(Color32::from_black_alpha(15)));

    painter.text(
        header.left_center() + Vec2::new(6.0 * zoom, 0.0),
        egui::Align2::LEFT_CENTER,
        title,
        FontId::proportional(5.5 * zoom),
        fade(ink.gamma_multiply(0.6)),
    );

    let left = rect.left() + 7.0 * zoom;

    painter.text(Pos2::new(left, header.bottom() + 5.0 * zoom), egui::Align2::LEFT_TOP, title, semibold(9.5 * zoom), fade(ink));

    for (index, (text, tick)) in lines.iter().enumerate() {
        let middle = header.bottom() + (25.0 + index as f32 * 11.0) * zoom;
        let mut x = left;

        if let Some(done) = tick {
            let square = Rect::from_center_size(Pos2::new(x + 3.0 * zoom, middle), Vec2::splat(5.5 * zoom));

            if *done {
                painter.rect_filled(square, 1.0 * zoom, fade(ink.gamma_multiply(0.55)));
            } else {
                painter.rect_stroke(square, 1.0 * zoom, Stroke::new((0.7 * zoom).max(0.6), fade(ink.gamma_multiply(0.7))), StrokeKind::Inside);
            }

            x += 9.0 * zoom;
        }

        let color = if *tick == Some(true) { ink.gamma_multiply(0.55) } else { ink };
        let written = painter.text(Pos2::new(x, middle), egui::Align2::LEFT_CENTER, text, FontId::proportional(7.5 * zoom), fade(color));

        if *tick == Some(true) {
            painter.hline(written.x_range(), middle, Stroke::new((0.7 * zoom).max(0.6), fade(color)));
        }
    }

    if tape > 0.0 {
        let middle = Pos2::new(rect.center().x, rect.top());
        let (half, tall, turn) = (14.0 * zoom, 4.5 * zoom, -0.08_f32);
        let (sin, cos) = turn.sin_cos();
        let corner = |x: f32, y: f32| middle + Vec2::new(x * cos - y * sin, x * sin + y * cos);
        let points = vec![corner(-half, -tall), corner(half, -tall), corner(half, tall), corner(-half, tall)];
        let edge = Stroke::new(0.6, Color32::from_black_alpha(22).gamma_multiply(alpha * tape));

        painter.add(Shape::convex_polygon(points, Color32::from_white_alpha(160).gamma_multiply(alpha * tape), edge));
    }
}

/// A rope's path from `from` to `to`, hanging `sag` points at its middle.
pub(super) fn rope_path(from: Pos2, to: Pos2, sag: f32) -> Vec<Pos2> {
    let control = from.lerp(to, 0.5) + Vec2::new(0.0, 2.0 * sag);

    (0..=24)
        .map(|step| {
            let t = step as f32 / 24.0;
            let rest = 1.0 - t;

            (from.to_vec2() * rest * rest + control.to_vec2() * 2.0 * rest * t + to.to_vec2() * t * t).to_pos2()
        })
        .collect()
}

/// The point `t` of the way along `points`, by length.
pub(super) fn along(points: &[Pos2], t: f32) -> Pos2 {
    let total: f32 = points.windows(2).map(|pair| pair[0].distance(pair[1])).sum();
    let mut left = total * t.clamp(0.0, 1.0);

    for pair in points.windows(2) {
        let length = pair[0].distance(pair[1]);

        if left <= length && length > 0.0 {
            return pair[0].lerp(pair[1], left / length);
        }

        left -= length;
    }

    points.last().copied().unwrap_or(Pos2::ZERO)
}

/// The first `reach` of a rope (0 to 1), dashed as the canvas draws one; blue and thicker while it carries a message.
pub(super) fn rope(painter: &Painter, points: &[Pos2], reach: f32, zoom: f32, alpha: f32, lit: bool, k: &K) {
    if reach <= 0.0 || points.len() < 2 {
        return;
    }

    let total: f32 = points.windows(2).map(|pair| pair[0].distance(pair[1])).sum();
    let mut left = total * reach.min(1.0);
    let mut drawn = vec![points[0]];

    for pair in points.windows(2) {
        let length = pair[0].distance(pair[1]);

        if left <= length {
            drawn.push(pair[0].lerp(pair[1], if length > 0.0 { left / length } else { 1.0 }));

            break;
        }

        drawn.push(pair[1]);
        left -= length;
    }

    let color = if lit { PRIMARY } else { k.fg.gamma_multiply(0.5) };
    let stroke = Stroke::new(1.5 * zoom * if lit { 1.6 } else { 1.0 }, color.gamma_multiply(alpha));

    painter.extend(Shape::dashed_line(&drawn, stroke, 6.0 * zoom, 5.0 * zoom));
}

/// A message on its way along a rope: the blue dot with its halo.
pub(super) fn pulse(painter: &Painter, at: Pos2, zoom: f32, alpha: f32) {
    let radius = (3.2 * zoom).max(2.5);

    painter.circle_filled(at, radius * 2.2, PRIMARY.gamma_multiply(0.25 * alpha));
    painter.circle_filled(at, radius, PRIMARY.gamma_multiply(alpha));
}

/// The words of her handwriting from `start` (the first line's baseline), written up to `pen` (0 to 1): each word a pen
/// stroke of cursive loops, the tall ones for the letters that rise, two lines of them.
pub(super) fn handwriting(start: Pos2, zoom: f32, pen: f32) -> Vec<Vec<Pos2>> {
    /// Loops per word, by line.
    const LINES: [&[usize]; 2] = [&[4, 6, 2, 5], &[3, 6, 3]];

    /// How far a loop goes, how far back its stroke swings, how much the letters lean, the space between words and the
    /// points a loop is drawn with.
    const ADVANCE: f32 = 5.0;

    const SWING: f32 = 1.6;
    const SLANT: f32 = 0.35;
    const GAP: f32 = 7.0;
    const STEPS: usize = 10;
    let total: usize = LINES.iter().flat_map(|words| words.iter()).sum();
    let mut left = pen.clamp(0.0, 1.0) * total as f32;
    let mut count = 0;
    let mut words = Vec::new();

    for (row, line) in LINES.iter().enumerate() {
        let mut x = 0.0;

        for &loops in line.iter() {
            let reach = left.min(loops as f32);

            if reach <= 0.0 {
                return words;
            }

            let points: Vec<Pos2> = (0..=(reach * STEPS as f32) as usize)
                .map(|step| {
                    let turns = step as f32 / STEPS as f32;
                    let angle = turns * std::f32::consts::TAU;
                    // the letters that rise: every few, by a fixed beat
                    let tall = (count + turns as usize) % 5 == 1 || (count + turns as usize) % 7 == 3;
                    let rise = if tall { 9.0 } else { 4.0 } * (0.5 - 0.5 * angle.cos());

                    start + Vec2::new(x + turns * ADVANCE - SWING * angle.sin() + SLANT * rise, row as f32 * 22.0 - rise) * zoom
                })
                .collect();

            words.push(points);
            left -= loops as f32;
            count += loops;
            x += loops as f32 * ADVANCE + GAP;
        }
    }

    words
}

/// A four-pointed star: Luna's sparkle, and the tip of her pen.
pub(super) fn star(painter: &Painter, center: Pos2, radius: f32, turn: f32, color: Color32) {
    let mut mesh = egui::epaint::Mesh::default();

    mesh.colored_vertex(center, color);

    for index in 0..8 {
        let angle = turn + index as f32 / 8.0 * std::f32::consts::TAU;
        let reach = if index % 2 == 0 { radius } else { radius * 0.36 };

        mesh.colored_vertex(center + Vec2::angled(angle) * reach, color);
    }

    for index in 0..8u32 {
        mesh.add_triangle(0, 1 + index, 1 + (index + 1) % 8);
    }

    painter.add(Shape::mesh(mesh));
}

/// A little screen: its wallpaper, the menu bar on top, and a browser and a terminal open on it, for the island to hang
/// over. Returns the painter clipped to it.
pub(super) fn screen(painter: &Painter, rect: Rect, zoom: f32, k: &K) -> Painter {
    let radius = 12.0 * zoom;

    shadow(painter, rect, radius, 1.0, k.dark);
    let wallpaper = if k.dark { Color32::from_rgb(0x12, 0x14, 0x1a) } else { Color32::from_rgb(0xdc, 0xe3, 0xee) };

    painter.rect(rect, radius, wallpaper, Stroke::new(1.0, k.border), StrokeKind::Inside);
    let inside = painter.with_clip_rect(rect.shrink(1.0));
    // a browser
    let browser = Rect::from_min_size(rect.min + Vec2::new(26.0, 40.0) * zoom, Vec2::new(200.0, 140.0) * zoom);

    shadow(&inside, browser, 7.0 * zoom, 1.0, k.dark);
    inside.rect(browser, 7.0 * zoom, surface(k), Stroke::new(1.0, k.border), StrokeKind::Inside);

    for (index, color) in [(0xff, 0x5f, 0x57), (0xfe, 0xbc, 0x2e), (0x28, 0xc8, 0x40)].into_iter().enumerate() {
        let (r, g, b) = color;

        inside.circle_filled(browser.left_top() + Vec2::new(9.0 + index as f32 * 8.0, 8.0) * zoom, 2.4 * zoom, Color32::from_rgb(r, g, b));
    }

    let address = Rect::from_min_size(browser.left_top() + Vec2::new(40.0, 3.5) * zoom, Vec2::new(120.0, 9.0) * zoom);

    inside.rect_filled(address, 4.5 * zoom, k.well);
    inside.text(address.left_center() + Vec2::new(6.0 * zoom, 0.0), egui::Align2::LEFT_CENTER, "localhost:3000", FontId::proportional(5.5 * zoom), k.muted_fg);

    for (index, width) in [0.7, 0.9, 0.55, 0.8].into_iter().enumerate() {
        let bar = Rect::from_min_size(browser.left_top() + Vec2::new(12.0, 30.0 + index as f32 * 14.0) * zoom, Vec2::new(176.0 * width, 6.0) * zoom);

        inside.rect_filled(bar, 3.0 * zoom, k.muted_fg.gamma_multiply(0.22));
    }

    // a terminal
    let shell = Rect::from_min_size(rect.min + Vec2::new(250.0, 92.0) * zoom, Vec2::new(206.0, 150.0) * zoom);

    terminal(&inside, shell, zoom, 1.0, "zsh", &["$ npm run dev", "pronto em 412 ms", "$ git status"], 3.0, GREEN, k.dark);
    // the menu bar, under the notch
    let bar = Rect::from_min_size(rect.min, Vec2::new(rect.width(), 15.0 * zoom));
    let round = radius.min(255.0) as u8;

    inside.rect_filled(bar, egui::CornerRadius { nw: round, ne: round, sw: 0, se: 0 }, surface(k).gamma_multiply(0.75));
    let middle = bar.center().y;

    inside.text(Pos2::new(bar.left() + 14.0 * zoom, middle), egui::Align2::LEFT_CENTER, "MagoAgent", semibold(6.5 * zoom), k.fg);
    inside.text(Pos2::new(bar.left() + 58.0 * zoom, middle), egui::Align2::LEFT_CENTER, "Arquivo   Editar   Ver", FontId::proportional(6.5 * zoom), k.muted_fg);
    inside.text(Pos2::new(bar.right() - 14.0 * zoom, middle), egui::Align2::RIGHT_CENTER, "9:41", semibold(6.5 * zoom), k.fg);
    inside
}

/// What an agent on the island is doing.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Status {
    Working,
    /// The `!`: it waits on you.
    NeedsYou,
    /// The ✓: its turn is over.
    Done,
}

/// The island's notch in `rect`, hanging from its top edge (square at the top, round at the bottom).
pub(super) fn notch(painter: &Painter, rect: Rect, radius: f32) {
    let round = radius.clamp(0.0, 255.0) as u8;

    painter.add(egui::epaint::Shadow { offset: [0, 4], blur: 14, spread: 0, color: Color32::from_black_alpha(60) }.as_shape(rect, radius));
    painter.rect_filled(rect, egui::CornerRadius { nw: 0, ne: 0, sw: round, se: round }, NOTCH);
}

/// The island's header when open: how many agents there are and what waits on you.
pub(super) fn island_header(painter: &Painter, left: Pos2, right: Pos2, zoom: f32, alpha: f32, waiting: bool) {
    painter.text(left, egui::Align2::LEFT_CENTER, "3 agentes", semibold(8.5 * zoom), NOTCH_INK.gamma_multiply(alpha));
    let (text, color) = if waiting { ("1 precisa de você", AMBER) } else { ("tudo em dia", NOTCH_DIM) };

    painter.text(right, egui::Align2::RIGHT_CENTER, text, FontId::proportional(7.0 * zoom), color.gamma_multiply(alpha));
}

/// One agent's pill on the open island: its mini wizard in its colour, its name and what it does, and its state at the
/// right. `grow` (0 to 1) pops it in.
#[allow(clippy::too_many_arguments)]
pub(super) fn island_row(painter: &Painter, rect: Rect, zoom: f32, grow: f32, id: &str, (name, task, color): (&str, &str, Color32), status: Status, time: f64) {
    if grow <= 0.0 {
        return;
    }

    let rect = Rect::from_center_size(rect.center(), rect.size() * (0.9 + 0.1 * grow));
    let alpha = grow.min(1.0);

    painter.rect_filled(rect, rect.height() / 2.0, NOTCH_ROW.gamma_multiply(alpha));
    let mini = Pos2::new(rect.left() + rect.height() / 2.0 + 2.0 * zoom, rect.center().y);

    let state = match status {
        Status::Working => MagoState::Working,
        Status::NeedsYou => MagoState::NeedsYou,
        Status::Done => MagoState::Done,
    };

    paint_mini(painter, id, mini, 15.0 * zoom * grow.min(1.0), color, state, time);
    let x = mini.x + 13.0 * zoom;

    painter.text(Pos2::new(x, rect.center().y - 4.5 * zoom), egui::Align2::LEFT_CENTER, name, semibold(7.5 * zoom), NOTCH_INK.gamma_multiply(alpha));

    painter.text(
        Pos2::new(x, rect.center().y + 5.0 * zoom),
        egui::Align2::LEFT_CENTER,
        task,
        FontId::proportional(6.5 * zoom),
        NOTCH_DIM.gamma_multiply(alpha),
    );

    let mark = Pos2::new(rect.right() - rect.height() / 2.0 - 2.0 * zoom, rect.center().y);
    let radius = 6.5 * zoom;

    match status {
        Status::Working => {
            for dot in 0..3 {
                let rise = ((time as f32 * 5.0 - dot as f32 * 0.9).sin() * 0.5 + 0.5) * 2.5 * zoom;

                painter.circle_filled(mark + Vec2::new((dot as f32 - 1.0) * 4.5 * zoom, -rise + 1.0 * zoom), 1.4 * zoom, NOTCH_DIM.gamma_multiply(alpha));
            }
        }
        Status::NeedsYou => {
            let beat = 1.0 + 0.08 * (time as f32 * 6.0).sin();

            painter.circle_filled(mark, radius * beat, AMBER.gamma_multiply(alpha));
            painter.text(mark, egui::Align2::CENTER_CENTER, "!", semibold(9.0 * zoom), NOTCH.gamma_multiply(alpha));
        }
        Status::Done => {
            painter.circle_filled(mark, radius, GREEN.gamma_multiply(alpha));
            let tick = vec![mark + Vec2::new(-2.8, 0.2) * zoom, mark + Vec2::new(-0.8, 2.2) * zoom, mark + Vec2::new(3.0, -2.2) * zoom];

            painter.add(Shape::line(tick, Stroke::new((1.4 * zoom).max(1.0), Color32::WHITE.gamma_multiply(alpha))));
        }
    }
}

/// Luna's speech bubble around `rect` with its tail pointing at `toward` (her), from the side that faces her.
pub(super) fn bubble(painter: &Painter, rect: Rect, toward: Pos2, zoom: f32, alpha: f32, k: &K) {
    let radius = 12.0 * zoom;
    let fill = if k.dark { Color32::from_rgb(0x2a, 0x2c, 0x32) } else { Color32::WHITE };
    let line = if k.dark { Color32::from_white_alpha(30) } else { Color32::from_black_alpha(30) };
    let (fill, line) = (fill.gamma_multiply(alpha), line.gamma_multiply(alpha));

    shadow(painter, rect, radius, alpha, k.dark);
    painter.rect(rect, radius, fill, Stroke::new(1.0, line), StrokeKind::Inside);
    // the tail: on the side facing her, its base along that side, its tip toward her
    let (half, reach) = (7.0 * zoom, 11.0 * zoom);

    let (base, normal, along) = if toward.x < rect.left() {
        (Pos2::new(rect.left(), toward.y.clamp(rect.top() + radius + half, rect.bottom() - radius - half)), Vec2::new(-1.0, 0.0), Vec2::Y)
    } else if toward.x > rect.right() {
        (Pos2::new(rect.right(), toward.y.clamp(rect.top() + radius + half, rect.bottom() - radius - half)), Vec2::new(1.0, 0.0), Vec2::Y)
    } else if toward.y > rect.bottom() {
        (Pos2::new(toward.x.clamp(rect.left() + radius + half, rect.right() - radius - half), rect.bottom()), Vec2::new(0.0, 1.0), Vec2::X)
    } else {
        (Pos2::new(toward.x.clamp(rect.left() + radius + half, rect.right() - radius - half), rect.top()), Vec2::new(0.0, -1.0), Vec2::X)
    };

    let lean = (toward - base).dot(along).clamp(-reach * 0.6, reach * 0.6);
    let tip = base + normal * reach + along * lean;
    let (first, second) = (base - along * half, base + along * half);

    // the fill reaches a little inside, over the bubble's own edge
    painter.add(Shape::convex_polygon(vec![first - normal * 1.5, tip, second - normal * 1.5], fill, Stroke::NONE));
    painter.add(Shape::line(vec![first, tip, second], Stroke::new(1.0, line)));
}
