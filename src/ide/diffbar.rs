//! The diff viewer's toolbar pieces: the ignore policy and the dropdown buttons (PhpStorm's
//! "Side-by-side viewer" and "Do not ignore" menus).

use super::*;

/// What the differ leaves out when it compares two lines (DiffBundle "Ignore Differences").
#[derive(Clone, Copy, PartialEq, Eq, Default, Debug)]
pub enum Ignore {
    #[default]
    None,
    /// whitespace at the ends of a line
    Trim,
    /// all whitespace
    Whitespace,
}

impl Ignore {
    pub const ALL: [(Ignore, &'static str); 3] = [(Ignore::None, "Não ignorar"), (Ignore::Trim, "Cortar espaços"), (Ignore::Whitespace, "Ignorar espaços")];

    pub fn label(self) -> &'static str {
        Self::ALL.iter().find(|(i, _)| *i == self).map_or("", |(_, l)| l)
    }

    /// The line as the differ compares it.
    pub fn key(self, line: &str) -> String {
        match self {
            Ignore::None => line.to_string(),
            Ignore::Trim => line.trim().to_string(),
            Ignore::Whitespace => line.chars().filter(|c| !c.is_whitespace()).collect(),
        }
    }
}

/// A toolbar dropdown: the current choice and a chevron, a menu of the others under it. True when `current` changed.
pub(super) fn choice<T: Copy + PartialEq>(ui: &mut Ui, current: &mut T, options: &[(T, &str)]) -> bool {
    let label = options.iter().find(|(v, _)| v == current).map_or("", |(_, l)| *l);
    let (resp, _) = super::chrome::dropdown(ui, 0.0, label);
    let mut changed = false;
    theme::menu(&resp).show(|ui| {
        for (v, l) in options {
            if ui.selectable_label(v == current, *l).clicked() {
                (*current, changed) = (*v, *current != *v);
                ui.close();
            }
        }
    });
    changed
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ide::git::{Diff, RowKind};

    #[test]
    fn ignoring_whitespace_drops_the_blocks_that_only_move_spaces() {
        let (old, new) = ("a\nb c\nd\n", "a\n b  c \nD\n");
        let mut diff = Diff::new(old, new);
        assert_eq!(diff.blocks.len(), 1); // the two lines are one block
        diff.set_ignore(Ignore::Whitespace);
        assert_eq!(diff.blocks, vec![2]); // only "d" -> "D" is left
        assert_eq!(diff.rows[1].kind, RowKind::Same);
        diff.set_ignore(Ignore::Trim);
        assert_eq!(diff.blocks.len(), 1);
        assert_eq!(diff.rows[1].kind, RowKind::Change); // "b c" against " b  c " still differs inside
        assert_eq!(Ignore::Whitespace.key(" a\tb "), "ab");
    }
}
