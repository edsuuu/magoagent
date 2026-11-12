use super::*;

/// The filled parts of a block element (U+2580–U+259F) as fractions of the cell, and the
/// shade for ░▒▓; None for any other character.
pub(crate) fn block_parts(c: char) -> Option<(Vec<[f32; 4]>, f32)> {
    let eighth = |n: u32| n as f32 / 8.0;
    let (ul, ur, ll, lr) = ([0.0, 0.0, 0.5, 0.5], [0.5, 0.0, 1.0, 0.5], [0.0, 0.5, 0.5, 1.0], [0.5, 0.5, 1.0, 1.0]);
    let parts = match c as u32 {
        0x2580 => vec![[0.0, 0.0, 1.0, 0.5]],
        n @ 0x2581..=0x2588 => vec![[0.0, 1.0 - eighth(n - 0x2580), 1.0, 1.0]],
        n @ 0x2589..=0x258F => vec![[0.0, 0.0, eighth(0x2590 - n), 1.0]],
        0x2590 => vec![[0.5, 0.0, 1.0, 1.0]],
        0x2591 => return Some((vec![[0.0, 0.0, 1.0, 1.0]], 0.25)),
        0x2592 => return Some((vec![[0.0, 0.0, 1.0, 1.0]], 0.5)),
        0x2593 => return Some((vec![[0.0, 0.0, 1.0, 1.0]], 0.75)),
        0x2594 => vec![[0.0, 0.0, 1.0, 0.125]],
        0x2595 => vec![[0.875, 0.0, 1.0, 1.0]],
        0x2596 => vec![ll],
        0x2597 => vec![lr],
        0x2598 => vec![ul],
        0x2599 => vec![ul, ll, lr],
        0x259A => vec![ul, lr],
        0x259B => vec![ul, ur, ll],
        0x259C => vec![ul, ur, lr],
        0x259D => vec![ur],
        0x259E => vec![ur, ll],
        0x259F => vec![ur, ll, lr],
        _ => return None,
    };
    Some((parts, 1.0))
}

pub(super) fn indexed(i: usize, look: &Look) -> Color32 {
    match i {
        0..16 => look.ansi[i],
        16..232 => {
            let level = |n: usize| if n == 0 { 0 } else { (55 + n * 40) as u8 };
            let i = i - 16;
            Color32::from_rgb(level(i / 36), level(i / 6 % 6), level(i % 6))
        }
        232..256 => Color32::from_gray((8 + (i - 232) * 10) as u8),
        _ => look.fg,
    }
}

pub(super) fn resolve(color: Color, colors: &Colors, look: &Look) -> Color32 {
    let rgb = |c: Rgb| Color32::from_rgb(c.r, c.g, c.b);
    match color {
        Color::Spec(c) => rgb(c),
        Color::Indexed(i) => colors[i as usize].map(rgb).unwrap_or_else(|| indexed(i as usize, look)),
        Color::Named(name) => colors[name].map(rgb).unwrap_or_else(|| match name {
            NamedColor::Background => look.bg,
            NamedColor::DimForeground => look.fg.gamma_multiply(0.66),
            NamedColor::Foreground | NamedColor::BrightForeground | NamedColor::Cursor => look.fg,
            name if (name as usize) < 16 => indexed(name as usize, look),
            dim => indexed(dim as usize - NamedColor::DimBlack as usize, look).gamma_multiply(0.66),
        }),
    }
}

/// `top` laid over `under` as its alpha says.
pub(super) fn over(under: Color32, top: Color32) -> Color32 {
    let a = top.a() as f32 / 255.0;
    let mix = |u: u8, t: u8| (u as f32 * (1.0 - a) + t as f32 * a).round() as u8;
    Color32::from_rgb(mix(under.r(), top.r()), mix(under.g(), top.g()), mix(under.b(), top.b()))
}
