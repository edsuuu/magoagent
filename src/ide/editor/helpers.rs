use super::*;

/// Opens a gap of `h` points above each line in `gaps` (line, h): that row and every row after it move
/// down. Rows are moved, not made taller, because egui centres the text of a custom line height.
/// Restyles the job's bytes `r`, cutting its sections at both ends first.
pub(crate) fn restyle(job: &mut egui::text::LayoutJob, r: std::ops::Range<usize>, f: impl Fn(&mut egui::text::TextFormat)) {
    hints::split_at(job, r.start);
    hints::split_at(job, r.end);
    for s in job.sections.iter_mut().filter(|s| r.start <= s.byte_range.start.0 && s.byte_range.end.0 <= r.end) {
        f(&mut s.format);
    }
}

pub(super) fn open_gaps(galley: &mut egui::Galley, gaps: &[(usize, f32)]) {
    let mut shift = 0.0;
    for (i, row) in galley.rows.iter_mut().enumerate() {
        if let Some(&(_, h)) = gaps.iter().find(|g| g.0 == i) {
            shift += h;
        }
        row.pos.y += shift;
    }
    galley.rect.max.y += shift;
    galley.mesh_bounds.max.y += shift;
}

/// A person glyph (head and shoulders) centred on `c`, drawn with strokes, at scale `z` (the file tree's pane zooms).
pub(crate) fn person_icon(p: &egui::Painter, c: egui::Pos2, color: Color32, z: f32) {
    let stroke = Stroke::new(1.2 * z, color);
    p.circle_stroke(c - egui::vec2(0.0, 2.6 * z), 2.4 * z, stroke);
    let shoulders = (0..=8)
        .map(|i| {
            let a = std::f32::consts::PI * i as f32 / 8.0;
            pos2(c.x - 4.2 * z * a.cos(), c.y + (4.2 - 3.4 * a.sin()) * z)
        })
        .collect();
    p.add(Shape::line(shoulders, stroke));
}

/// The lines of the declaration on line `d`: down to the last non-blank line still indented past it,
/// its braces included, as a half-open range.
// ponytail: by indentation, not by the parser; a declaration split over several lines at its own indent ends early
pub(crate) fn decl_span(lines: &[&str], d: usize) -> (usize, usize) {
    let base = indent_of(lines[d]).unwrap_or(0);
    let mut end = d + 1;
    for (l, text) in lines.iter().enumerate().skip(d + 1) {
        match indent_of(text) {
            None => {}
            Some(c) if c > base || (c == base && text.trim_start().starts_with(['{', '}'])) => end = l + 1,
            Some(_) => break,
        }
    }
    (d, end)
}

/// Who wrote most of the lines in `span`: "nome", or "nome +N" when N other people wrote the rest.
pub(crate) fn decl_author(blame: &[Option<String>], (from, to): (usize, usize)) -> Option<String> {
    let mut count = std::collections::BTreeMap::new();
    for name in blame.get(from..to.min(blame.len())).unwrap_or_default().iter().flatten() {
        *count.entry(name.as_str()).or_insert(0usize) += 1;
    }
    let (top, _) = count.iter().max_by_key(|(_, n)| **n)?;
    let others = count.len() - 1;
    Some(if others == 0 { top.to_string() } else { format!("{top} +{others}") })
}

/// Columns of leading whitespace (a tab is 4), None for a blank line.
pub(crate) fn indent_of(line: &str) -> Option<usize> {
    if line.trim().is_empty() {
        return None;
    }
    Some(line.chars().take_while(|c| c.is_whitespace()).map(|c| if c == '\t' { 4 } else { 1 }).sum())
}

/// The indent guides of `text`, as the reference IDE draws them: (column, opening line, end line) of each block. A block
/// opens where the indent rises, and its guide runs at the opening line's column from under that line down to the line
/// before `end` (the first line back at that column, or the text's length). None at column 0. A blank line stays in the
/// block of the smaller indent around it, or of the line above it when a closing bracket comes next.
pub(crate) fn indent_guides(text: &str) -> Vec<(usize, usize, usize)> {
    let lines: Vec<&str> = text.split('\n').collect();
    let own: Vec<Option<usize>> = lines.iter().map(|l| indent_of(l)).collect();
    let (mut cols, mut above) = (Vec::with_capacity(lines.len()), 0);
    while cols.len() < lines.len() {
        if let Some(c) = own[cols.len()] {
            above = c;
            cols.push(c);
            continue;
        }
        let next = (cols.len()..lines.len()).find(|&l| own[l].is_some());
        let below = next.and_then(|l| own[l]).unwrap_or(above);
        let closes = next.is_some_and(|l| lines[l].trim_start().starts_with(['}', ')', ']']));
        cols.resize(next.unwrap_or(lines.len()), if closes { above } else { above.min(below) });
    }
    let (mut out, mut open): (Vec<(usize, usize, usize)>, Vec<(usize, usize)>) = (vec![], vec![]);
    for l in 1..cols.len() {
        while let Some(&(col, start)) = open.last().filter(|o| cols[l] <= o.0) {
            open.pop();
            out.push((col, start, l));
        }
        if cols[l] > cols[l - 1] + 1 {
            open.push((cols[l - 1], l - 1));
        }
    }
    out.extend(open.into_iter().map(|(col, start)| (col, start, cols.len())));
    out.retain(|g| g.0 > 0);
    out
}

/// The guide of the block the caret's `line` is in (the innermost one), its opening and closing lines included.
pub(crate) fn caret_guide(guides: &[(usize, usize, usize)], line: usize) -> Option<(usize, usize, usize)> {
    guides.iter().copied().filter(|g| g.1 <= line && line <= g.2).max_by_key(|g| g.0)
}
