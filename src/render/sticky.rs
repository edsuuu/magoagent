//! Sticky notes: style, outline and creation-date footer.

use crate::prelude::*;

pub(crate) fn sticky_style(style: Style) -> Style {
    Style { stroke: BLACK, fill: Some([0xff, 0xdf, 0x6b, 255]), fill_style: FillStyle::Solid, roughness: 1, round: true, ..style }
}

pub(crate) fn now_ms() -> i64 {
    SystemTime::now().duration_since(SystemTime::UNIX_EPOCH).map_or(0, |d| d.as_millis() as i64)
}

/// (year, month 1-12, day) of a UTC day count, Howard Hinnant's civil_from_days.
pub(crate) fn civil_date(ms: i64) -> (i64, usize, i64) {
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
pub(crate) fn sticky_date(created: i64, short: bool) -> Option<String> {
    const MONTHS: [&str; 12] = ["Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec"];
    if created <= 0 {
        return None;
    }
    let (year, month, day) = civil_date(created);
    let label = format!("{day} {}", MONTHS[month - 1]);
    Some(if short || year == civil_date(now_ms()).0 { label } else { format!("{label} {year}") })
}

/// Rounded-rectangle outline, corner radius 4% of the short side up to 16 (sticky notes).
pub(crate) fn sticky_outline(b: Rect, round: bool, shift: f32) -> Vec<[f32; 2]> {
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
