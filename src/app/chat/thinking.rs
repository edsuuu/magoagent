//! While the agent thinks: Claude Code's spinner (the asterisk that grows and shrinks) and a line
//! that changes every few seconds, brighter where a shine passes over it.

use crate::prelude::*;

/// Claude's orange, the spinner of Claude Code.
pub(crate) const CLAUDE: Color32 = Color32::from_rgb(0xd9, 0x77, 0x57);

/// The first line stays this long; the next ones, `DWELL`.
const OPENING: f64 = 6.0;
const DWELL: f64 = 10.0;

const LINES: [&str; 14] = [
    "Pensando…",
    "Fazendo alguma coisa…",
    "Orquestrando…",
    "Regendo…",
    "Afinando…",
    "Buscando a nota certa…",
    "Cantarolando…",
    "Pegando a batuta…",
    "Ensaiando…",
    "Ponderando…",
    "Matutando…",
    "Cogitando…",
    "Elaborando…",
    "Refletindo…",
];

/// The line `elapsed` seconds into a turn that began at `since` (its seed, so each turn has its own
/// order): "{name} está trabalhando…" first, then the others shuffled, "{name} ainda está
/// trabalhando…" among them.
pub(crate) fn line(name: &str, since: f64, elapsed: f64) -> String {
    if elapsed < OPENING {
        return format!("{name} está trabalhando…");
    }
    let step = 1 + ((elapsed - OPENING) / DWELL) as usize;
    let mut pool: Vec<String> = LINES.iter().map(|l| l.to_string()).collect();
    pool.push(format!("{name} ainda está trabalhando…"));
    // Fisher-Yates on a tiny LCG seeded by the turn's start
    let mut seed = since.to_bits() | 1;
    for i in (1..pool.len()).rev() {
        seed = seed.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
        pool.swap(i, (seed >> 33) as usize % (i + 1));
    }
    pool[(step - 1) % pool.len()].clone()
}

/// Claude Code's spinner at `center`: `·` `✢` `✳` `✶` `✻` `✽` and back, drawn as spokes so no font
/// has to carry the glyphs.
pub(crate) fn spinner(p: &Painter, center: Pos2, size: f32, color: Color32, t: f64) {
    const FRAMES: [(usize, f32); 6] = [(0, 0.22), (4, 0.6), (8, 0.75), (6, 0.85), (8, 0.95), (8, 1.0)];
    let i = (t / 0.12) as usize % 10;
    let (spokes, reach) = FRAMES[if i < 6 { i } else { 10 - i }];
    let r = size / 2.0 * reach;
    if spokes == 0 {
        p.circle_filled(center, r.max(1.5), color);
        return;
    }
    for k in 0..spokes {
        let a = std::f32::consts::TAU * k as f32 / spokes as f32 - std::f32::consts::FRAC_PI_2;
        let d = Vec2::angled(a);
        p.line_segment([center + d * r * 0.2, center + d * r], Stroke::new((size * 0.14).max(1.4), color));
    }
}

/// `text` at `left` (centered on its y), with a shine that sweeps across it every 2 s.
pub(crate) fn shimmer(p: &Painter, left: Pos2, text: &str, font: FontId, base: Color32, bright: Color32, t: f64) -> f32 {
    let galley = p.layout_no_wrap(text.into(), font.clone(), base);
    let width = galley.size().x;
    let top = left.y - galley.size().y / 2.0;
    let sweep = ((t % 2.0) / 2.0) as f32 * (width + 80.0) - 40.0;
    let mut x = left.x;
    for ch in text.chars() {
        let g = p.layout_no_wrap(ch.to_string(), font.clone(), base);
        let mid = x - left.x + g.size().x / 2.0;
        let near = (1.0 - (mid - sweep).abs() / 40.0).clamp(0.0, 1.0);
        let w = g.size().x;
        p.galley(Pos2::new(x, top), g, base.lerp_to_gamma(bright, near));
        x += w;
    }
    width
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_line_opens_with_the_name_then_turns_and_keeps_its_order_in_a_turn() {
        assert_eq!(line("Claude", 1.0, 2.0), "Claude está trabalhando…");
        let a = line("Claude", 1.0, 7.0);
        assert_ne!(a, "Claude está trabalhando…");
        assert_eq!(line("Claude", 1.0, 15.9), a);
        assert_ne!(line("Claude", 1.0, 16.1), a);
        // all the lines come up within one round of the pool
        let seen: std::collections::HashSet<_> = (0..15).map(|k| line("Claude", 1.0, OPENING + DWELL * k as f64 + 0.1)).collect();
        assert_eq!(seen.len(), 15);
        assert!(seen.contains("Claude ainda está trabalhando…"));
    }
}
