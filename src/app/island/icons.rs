//! The island's icons, drawn in a 24 grid: filled, or a line of
//! the stroke it names. Rasterized by resvg as the editor's (`editor::icons`) and tinted with the ink.

use crate::editor::icons::{Svg, paint_tint};
use crate::prelude::*;

macro_rules! filled {
    ($name:ident, $path:literal) => {
        pub(super) const $name: Svg = (
            concat!("island_", stringify!($name)),
            concat!(r#"<svg xmlns="http://www.w3.org/2000/svg" width="24" height="24" viewBox="0 0 24 24"><path fill="white" d=""#, $path, r#""/></svg>"#),
        );
    };
}

macro_rules! line {
    ($name:ident, $stroke:literal, $path:literal) => {
        pub(super) const $name: Svg = (
            concat!("island_", stringify!($name)),
            concat!(
                r#"<svg xmlns="http://www.w3.org/2000/svg" width="24" height="24" viewBox="0 0 24 24"><path fill="none" stroke="white" stroke-width=""#,
                $stroke,
                r#"" stroke-linecap="round" stroke-linejoin="round" d=""#,
                $path,
                r#""/></svg>"#
            ),
        );
    };
}

filled!(HOUSE, "M12 3.2 2.8 10.6V21h6.6v-5.4h5.2V21h6.6V10.6L12 3.2z");
filled!(BUBBLE, "M8 4.2h8a5 5 0 0 1 5 5v3a5 5 0 0 1-5 5h-5.3l-4.4 3.2c-.5.4-1.2 0-1.1-.6l.5-2.9A5 5 0 0 1 3 12.2v-3a5 5 0 0 1 5-5z");
filled!(PLUS, "M11 4h2v7h7v2h-7v7h-2v-7H4v-2h7V4z");
// the new tabs: a pull request and a note
line!(
    PULL,
    "1.9",
    "M3.8 5.5a2.2 2.2 0 1 0 4.4 0a2.2 2.2 0 1 0-4.4 0M3.8 18.5a2.2 2.2 0 1 0 4.4 0a2.2 2.2 0 1 0-4.4 0M6 7.7v8.6M15.8 18.5a2.2 2.2 0 1 0 4.4 0a2.2 2.2 0 1 0-4.4 0M18 16.3V10a3.5 3.5 0 0 0-3.5-3.5H11M13.4 4.1 11 6.5l2.4 2.4"
);
line!(
    PULL_DRAFT,
    "1.9",
    "M3.8 5.5a2.2 2.2 0 1 0 4.4 0a2.2 2.2 0 1 0-4.4 0M3.8 18.5a2.2 2.2 0 1 0 4.4 0a2.2 2.2 0 1 0-4.4 0M6 7.7v8.6M15.8 18.5a2.2 2.2 0 1 0 4.4 0a2.2 2.2 0 1 0-4.4 0M18 13.6v.1M18 9.6v.1M18 5.6v.1"
);
line!(
    PULL_MERGED,
    "1.9",
    "M3.8 5.5a2.2 2.2 0 1 0 4.4 0a2.2 2.2 0 1 0-4.4 0M3.8 18.5a2.2 2.2 0 1 0 4.4 0a2.2 2.2 0 1 0-4.4 0M6 7.7v8.6M15.8 12a2.2 2.2 0 1 0 4.4 0a2.2 2.2 0 1 0-4.4 0M6 7.7c0 3 2.6 4.3 5 4.3h4.8"
);
line!(
    PULL_CLOSED,
    "1.9",
    "M3.8 5.5a2.2 2.2 0 1 0 4.4 0a2.2 2.2 0 1 0-4.4 0M3.8 18.5a2.2 2.2 0 1 0 4.4 0a2.2 2.2 0 1 0-4.4 0M6 7.7v8.6M15.8 18.5a2.2 2.2 0 1 0 4.4 0a2.2 2.2 0 1 0-4.4 0M18 16.3v-5.5M15.6 4.2l4.8 4.8M20.4 4.2l-4.8 4.8"
);
line!(
    SHUFFLE,
    "1.8",
    "M3 7h3.5c2.5 0 4 1.3 5.5 4.5S15 16 17.5 16H21M17.5 13 21 16l-3.5 3M3 16h3.5c1.3 0 2.3-.4 3.1-1.2M13.4 8.2c.8-.8 2-1.2 4.1-1.2H21M17.5 4 21 7l-3.5 3"
);
line!(REPEAT, "1.8", "M4 11V9.5A3.5 3.5 0 0 1 7.5 6H19M16 3l3 3-3 3M20 13v1.5a3.5 3.5 0 0 1-3.5 3.5H5M8 21l-3-3 3-3");
filled!(PREVIOUS, "M6 5h2v14H6zM20 5v14L9.5 12z");
filled!(NEXT, "M16 5h2v14h-2zM4 5v14l10.5-7z");
filled!(PLAY, "M7 4.6v14.8L19.4 12z");
filled!(PAUSE, "M7 5h3.6v14H7zM13.4 5H17v14h-3.6z");
filled!(HEART, "M12 20.3 4.6 13a4.8 4.8 0 0 1 6.8-6.8l.6.6.6-.6a4.8 4.8 0 1 1 6.8 6.8z");
line!(HEART_LINE, "1.8", "M12 20.3 4.6 13a4.8 4.8 0 0 1 6.8-6.8l.6.6.6-.6a4.8 4.8 0 1 1 6.8 6.8z");
line!(FROM, "2", "M4 6h16M4 12h10M4 18h7M17 14v6M17 20a2 2 0 1 1-2-2");
line!(SEARCH, "1.9", "M4 10.5a6.5 6.5 0 1 0 13 0a6.5 6.5 0 1 0-13 0M15.3 15.3 20 20");
filled!(NOTE, "M8 5.6 20 3.4v2.4L10 7.6v10.9H8zM18 5.4h2v11.1h-2zM5 18.5a2.5 2.5 0 1 0 5 0a2.5 2.5 0 1 0-5 0zM15 16.5a2.5 2.5 0 1 0 5 0a2.5 2.5 0 1 0-5 0z");
filled!(XMARK, "M6.4 5 12 10.6 17.6 5 19 6.4 13.4 12 19 17.6 17.6 19 12 13.4 6.4 19 5 17.6 10.6 12 5 6.4 6.4 5z");
filled!(
    GEAR_FILL,
    "M10.9 2h2.2l.35 2.1c.6.17 1.16.4 1.67.71l1.9-1 1.55 1.55-1 1.9c.3.5.54 1.07.7 1.67l2.13.35v2.2l-2.12.35c-.17.6-.4 1.16-.71 1.67l1 1.9-1.55 1.55-1.9-1c-.5.3-1.07.54-1.67.7L13.1 22h-2.2l-.35-2.12c-.6-.17-1.16-.4-1.67-.71l-1.9 1L5.43 18.6l1-1.9c-.3-.5-.54-1.07-.7-1.67L3.6 14.7v-2.2l2.12-.35c.17-.6.4-1.16.71-1.67l-1-1.9 1.55-1.55 1.9 1c.5-.3 1.07-.54 1.67-.7L10.9 2zM12 8.2a3.8 3.8 0 1 0 0 7.6 3.8 3.8 0 0 0 0-7.6z"
);
filled!(
    TIMER,
    "M12 4.2a7.8 7.8 0 1 0 0 15.6 7.8 7.8 0 0 0 0-15.6zm0 1.9a5.9 5.9 0 1 1 0 11.8 5.9 5.9 0 0 1 0-11.8zm-.95 2.3v4.2l3.3 2 .95-1.55-2.4-1.45V8.4h-1.85zM9.2 2h5.6v1.7H9.2V2z"
);
line!(
    GEAR_LINE,
    "1.6",
    "M18.93 9.39 L21.41 10.09 L21.41 13.91 L18.93 14.61 L18.74 15.05 L20.00 17.30 L17.30 20.00 L15.05 18.74 L14.61 18.93 L13.91 21.41 L10.09 21.41 L9.39 18.93 L8.95 18.74 L6.70 20.00 L4.00 17.30 L5.26 15.05 L5.07 14.61 L2.59 13.91 L2.59 10.09 L5.07 9.39 L5.26 8.95 L4.00 6.70 L6.70 4.00 L8.95 5.26 L9.39 5.07 L10.09 2.59 L13.91 2.59 L14.61 5.07 L15.05 5.26 L17.30 4.00 L20.00 6.70 L18.74 8.95Z M15.2 12a3.2 3.2 0 1 1-6.4 0 3.2 3.2 0 0 1 6.4 0z"
);
line!(SPEAKER, "1.6", "M3.8 9.4h3.4l4.6-3.9v13l-4.6-3.9H3.8zM15.4 9.1a4.2 4.2 0 0 1 0 5.8M18.3 6.4a8 8 0 0 1 0 11.2");
line!(SPEAKER_SLASH, "1.6", "M3.8 9.4h3.4l4.6-3.9v13l-4.6-3.9H3.8zM15.5 9.5l5 5M20.5 9.5l-5 5");
// not in the reference: the microphone of the voice chat and the list of the conversations, in its manner
line!(MIC, "1.6", "M12 3.6a2.9 2.9 0 0 1 2.9 2.9v5a2.9 2.9 0 0 1-5.8 0v-5A2.9 2.9 0 0 1 12 3.6zM6.6 11.2a5.4 5.4 0 0 0 10.8 0M12 16.6v3.6");
filled!(
    LIST,
    "M5 5.4a1.6 1.6 0 1 1 0 3.2 1.6 1.6 0 0 1 0-3.2zm0 5a1.6 1.6 0 1 1 0 3.2 1.6 1.6 0 0 1 0-3.2zm0 5a1.6 1.6 0 1 1 0 3.2 1.6 1.6 0 0 1 0-3.2zM9 6h11v2H9V6zm0 5h11v2H9v-2zm0 5h11v2H9v-2z"
);

/// `icon` at `size` points, centred on `center`, in `ink`.
pub(super) fn paint(painter: &Painter, icon: Svg, center: Pos2, size: f32, ink: Color32) {
    paint_tint(painter, icon, Rect::from_center_size(center, Vec2::splat(size)), ink);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_icon_is_an_svg_resvg_reads() {
        for (name, svg) in [HOUSE, BUBBLE, PLUS, XMARK, GEAR_FILL, TIMER, GEAR_LINE, SPEAKER, SPEAKER_SLASH, MIC, LIST] {
            let tree = resvg::usvg::Tree::from_str(svg, &resvg::usvg::Options::default()).unwrap_or_else(|error| panic!("{name}: {error}"));

            assert!(tree.root().has_children(), "{name} draws something");
        }
    }
}
