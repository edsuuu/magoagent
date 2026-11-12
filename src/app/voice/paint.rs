use super::*;

/// The call's five bars at rest.
pub(super) const BARS: [f32; 5] = [5.0, 9.0, 12.0, 9.0, 5.0];

/// The bars' heights at `t`: a wave running through them while it speaks, the microphone's level
/// while it hears, a slow breath while it listens, one lit after the other while it starts.
pub(crate) fn call_bars(state: VoiceState, mic: f32, t: f64) -> [f32; 5] {
    std::array::from_fn(|k| {
        let wave = |speed: f64, step: f64| ((t * speed - k as f64 * step).sin() * 0.5 + 0.5) as f32;
        let base = BARS[k];
        match state {
            VoiceState::Speaking => 4.0 + (base + 2.0 - 4.0) * wave(7.0, 0.9),
            VoiceState::Hearing => 4.0 + (base + 3.0 - 4.0) * (0.25 + 0.75 * mic.clamp(0.0, 1.0) * (0.6 + 0.4 * wave(11.0, 1.3))),
            VoiceState::Preparing => 4.0 + (base - 4.0) * wave(5.0, 0.7).powi(3),
            VoiceState::Muted => 4.0,
            _ => base * (0.75 + 0.25 * wave(2.0, 0.5)),
        }
    })
}

/// Five rounded bars, 4.5 apart, centered on `c`.
pub(super) fn paint_bars(p: &Painter, c: Pos2, heights: [f32; 5], ink: Color32) {
    for (k, h) in heights.into_iter().enumerate() {
        let x = c.x + (k as f32 - 2.0) * 4.5;
        p.rect_filled(Rect::from_center_size(Pos2::new(x, c.y), Vec2::new(3.0, h.max(3.0))), 1.5, ink);
    }
}

/// A microphone: the capsule, its stand; crossed when muted.
pub(crate) fn paint_mic(p: &Painter, c: Pos2, ink: Color32, muted: bool) {
    p.rect_filled(Rect::from_center_size(c - Vec2::new(0.0, 3.0), Vec2::new(5.0, 10.0)), 2.5, ink);
    p.add(Shape::line(
        vec![c + Vec2::new(-4.5, 0.0), c + Vec2::new(-4.5, 2.0), c + Vec2::new(0.0, 6.0), c + Vec2::new(4.5, 2.0), c + Vec2::new(4.5, 0.0)],
        Stroke::new(1.5, ink),
    ));
    p.line_segment([c + Vec2::new(0.0, 6.0), c + Vec2::new(0.0, 9.0)], Stroke::new(1.5, ink));
    if muted {
        p.line_segment([c + Vec2::new(-7.0, -8.0), c + Vec2::new(7.0, 8.0)], Stroke::new(2.0, ink));
    }
}

/// The speakers: a speaker with its waves, crossed out when silenced.
pub(crate) fn paint_speaker(p: &Painter, c: Pos2, ink: Color32, off: bool) {
    let s = c - Vec2::new(2.0, 0.0);
    p.add(Shape::convex_polygon(
        vec![
            s + Vec2::new(-5.0, -2.5),
            s + Vec2::new(-2.0, -2.5),
            s + Vec2::new(2.0, -6.0),
            s + Vec2::new(2.0, 6.0),
            s + Vec2::new(-2.0, 2.5),
            s + Vec2::new(-5.0, 2.5),
        ],
        ink,
        Stroke::NONE,
    ));
    if off {
        p.line_segment([s + Vec2::new(4.5, -3.0), s + Vec2::new(10.5, 3.0)], Stroke::new(1.5, ink));
        p.line_segment([s + Vec2::new(4.5, 3.0), s + Vec2::new(10.5, -3.0)], Stroke::new(1.5, ink));
    } else {
        for r in [5.0f32, 8.5] {
            let arc: Vec<Pos2> = (0..=8).map(|k| s + Vec2::angled(-0.75 + 1.5 * k as f32 / 8.0) * r).collect();
            p.add(Shape::line(arc, Stroke::new(1.5, ink)));
        }
    }
}
