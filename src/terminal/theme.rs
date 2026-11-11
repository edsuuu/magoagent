//! What a terminal's grid is painted with: the colors of a theme (the reference app's built-in
//! ones, per terminal) and the size of its font.

use super::ANSI;
use crate::prelude::*;

pub(super) struct Theme {
    pub(super) id: &'static str,
    pub(super) name: &'static str,
    pub(super) bg: [u8; 3],
    pub(super) fg: [u8; 3],
    pub(super) cursor: [u8; 3],
    pub(super) selection: [u8; 4],
    pub(super) ansi: [[u8; 3]; 16],
}

include!("themes_table.rs");

/// The theme a terminal has when none was picked: the app's own look (Alacritty's palette).
pub(crate) const THEME_DEFAULT: &str = "";
/// Follows the app: Mago Dark in the dark theme, Mago Light in the light one.
pub(crate) const THEME_SYSTEM: &str = "system";
/// How a grid is painted: its colors, its font size and, by visible row, the matches of a search
/// (first column, last column, whether it is the current one).
pub(crate) struct GridView<'a> {
    pub(crate) look: &'a Look,
    pub(crate) font: f32,
    pub(crate) marks: &'a [Vec<(usize, usize, bool)>],
}

/// What a search's matches are painted with: a yellow, and an orange for the current one.
pub(super) const MATCH: Color32 = Color32::from_rgba_premultiplied(0x66, 0x56, 0x00, 0x66);
pub(super) const MATCH_CURRENT: Color32 = Color32::from_rgba_premultiplied(0xb2, 0x6f, 0x07, 0xb2);

/// A terminal's own appearance and shell, over the defaults (`TermPrefs`): what Editar sets.
#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub(crate) struct TermLook {
    /// A theme (`theme_choices`); none: the default's.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) theme: Option<String>,
    /// The font's size in pt (the family is always Hack); none: the default's.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) font_size: Option<f32>,
    /// The shell's program or path; empty: the default's.
    #[serde(skip_serializing_if = "String::is_empty")]
    pub(crate) shell: String,
    /// "Desfocar": its screen hidden behind a veil until "Remover Desfoque".
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub(crate) blurred: bool,
}

impl TermLook {
    pub(crate) fn is_default(&self) -> bool {
        *self == Self::default()
    }
}

/// The defaults for every terminal (Ajustes › Terminal, and "Padrões para Novos Elementos").
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub(crate) struct TermPrefs {
    pub(crate) theme: String,
    pub(crate) font_size: f32,
    /// The shell's program or path; empty: the login shell.
    pub(crate) shell: String,
    /// What a new terminal measures (world units); none: 80 by 24 cells.
    pub(crate) size: Option<[f32; 2]>,
    /// A selection is copied the moment it is made.
    pub(crate) copy_on_select: bool,
}

impl Default for TermPrefs {
    fn default() -> Self {
        Self { theme: THEME_DEFAULT.into(), font_size: TERMINAL_FONT, shell: String::new(), size: None, copy_on_select: false }
    }
}

impl TermPrefs {
    /// The theme, font size and shell a terminal with this look really has.
    pub(crate) fn resolve<'a>(&'a self, look: &'a TermLook) -> (&'a str, f32, &'a str) {
        let shell = if look.shell.trim().is_empty() { &self.shell } else { &look.shell };
        (look.theme.as_deref().unwrap_or(&self.theme), font_size(look.font_size, self.font_size), shell)
    }
}

/// The font sizes a terminal can have (pt), and the one it has by default.
pub(crate) const FONT_SIZES: std::ops::RangeInclusive<f32> = 8.0..=32.0;

/// Every theme that can be picked: its id and its name.
pub(crate) fn theme_choices() -> Vec<(&'static str, &'static str)> {
    [(THEME_DEFAULT, "Padrão"), (THEME_SYSTEM, "Sistema")].into_iter().chain(THEMES.iter().map(|t| (t.id, t.name))).collect()
}

/// The colors a grid is painted with.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct Look {
    pub(crate) bg: Color32,
    pub(crate) fg: Color32,
    pub(crate) cursor: Color32,
    pub(crate) selection: Color32,
    pub(crate) ansi: [Color32; 16],
}

impl Look {
    /// The theme `id` names (`dark`: the app's theme, for "system"); unknown ids are the default.
    pub(crate) fn of(id: &str, dark: bool) -> Self {
        let id = if id == THEME_SYSTEM { if dark { "mago-dark" } else { "mago-light" } } else { id };
        match THEMES.iter().find(|t| t.id == id) {
            Some(t) => {
                let c = |[r, g, b]: [u8; 3]| Color32::from_rgb(r, g, b);
                let [r, g, b, a] = t.selection;
                Self { bg: c(t.bg), fg: c(t.fg), cursor: c(t.cursor), selection: Color32::from_rgba_unmultiplied(r, g, b, a), ansi: t.ansi.map(c) }
            }
            None => Self::default(),
        }
    }

    /// The strips over and under the grid: the background with the text color's 6% over it.
    pub(crate) fn strip(&self) -> Color32 {
        let mix = |a: u8, b: u8| (a as f32 * 0.94 + b as f32 * 0.06).round() as u8;
        Color32::from_rgb(mix(self.bg.r(), self.fg.r()), mix(self.bg.g(), self.fg.g()), mix(self.bg.b(), self.fg.b()))
    }

    /// The rule between a strip and the grid, and the card's outline: the text color's 14%.
    pub(crate) fn rule(&self) -> Color32 {
        self.fg.gamma_multiply(0.14)
    }

    pub(crate) fn muted(&self) -> Color32 {
        self.fg.gamma_multiply(0.6)
    }
}

impl Default for Look {
    fn default() -> Self {
        Self {
            bg: TERMINAL_BG,
            fg: TERMINAL_FG,
            cursor: TERMINAL_FG.gamma_multiply(0.7),
            selection: Color32::from_rgba_unmultiplied(0x0a, 0x84, 0xff, 0x66),
            ansi: ANSI.map(|[r, g, b]| Color32::from_rgb(r, g, b)),
        }
    }
}

/// The font size a terminal's spec asks for, else the default one (never outside `FONT_SIZES`).
pub(crate) fn font_size(spec: Option<f32>, default: f32) -> f32 {
    spec.unwrap_or(default).clamp(*FONT_SIZES.start(), *FONT_SIZES.end())
}

/// One grid cell in world units at this font size.
pub(crate) fn cell(font: f32) -> Vec2 {
    CELL * (font / TERMINAL_FONT)
}

/// Columns and rows that fit a terminal of this size (world units) at this font size.
pub(crate) fn grid_size_at(size: Vec2, font: f32) -> (usize, usize) {
    let c = cell(font);
    let cols = ((size.x - 2.0 * TERMINAL_PAD) / c.x).floor().max(2.0) as usize;
    let rows = ((size.y - TERMINAL_HEADER - TERMINAL_FOOTER - TERMINAL_PAD) / c.y).floor().max(1.0) as usize;
    (cols, rows)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_theme_reads_and_the_default_keeps_the_apps_colors() {
        assert_eq!(Look::of("", true), Look::default());
        assert_eq!(Look::of("no-such-theme", true), Look::default());
        assert_ne!(Look::of("dracula", true).bg, Look::default().bg);
        assert_ne!(Look::of(THEME_SYSTEM, false), Look::of(THEME_SYSTEM, true));
        assert_eq!(Look::of(THEME_SYSTEM, false).bg, Color32::WHITE);
        assert_eq!(theme_choices().len(), THEMES.len() + 2);
        for t in THEMES {
            assert!(Look::of(t.id, true).fg != Look::of(t.id, true).bg, "{}", t.id);
        }
    }

    #[test]
    fn a_terminal_has_its_own_look_or_the_defaults() {
        let prefs = TermPrefs { theme: "nord".into(), font_size: 12.0, shell: "zsh".into(), ..Default::default() };
        assert_eq!(prefs.resolve(&TermLook::default()), ("nord", 12.0, "zsh"));
        let own = TermLook { theme: Some(String::new()), font_size: Some(20.0), shell: "fish".into(), blurred: true };
        assert_eq!(prefs.resolve(&own), ("", 20.0, "fish"));
        assert!(TermLook::default().is_default() && !own.is_default());
        let json = serde_json::to_string(&TermLook::default()).unwrap();
        assert_eq!(json, "{}", "a terminal that kept the defaults saves nothing");
    }

    #[test]
    fn the_cell_grows_with_the_font() {
        assert_eq!(cell(TERMINAL_FONT), CELL);
        assert_eq!(cell(28.0), CELL * 2.0);
        assert_eq!(font_size(Some(100.0), 14.0), 32.0);
        assert_eq!(font_size(None, 14.0), 14.0);
        // a bigger font fits fewer columns in the same box, and the default one fits what it always did
        assert_eq!(grid_size_at(terminal_size(80, 24), TERMINAL_FONT), (80, 24));
        let big = grid_size_at(terminal_size(80, 24), 28.0);
        assert!(big.0 < 50 && big.1 < 16, "{big:?}");
    }
}
