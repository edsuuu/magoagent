//! Recent and favorite folders: the menu beside a folder field and on the chat's folder chip.

use crate::prelude::*;

/// How many recent folders are kept.
const RECENT: usize = 8;

/// The path without the separator at its end (a root keeps it).
fn clean(path: &str) -> &str {
    let path = path.trim();
    let cut = path.trim_end_matches(['/', '\\']);
    if cut.is_empty() || cut.ends_with(':') { path } else { cut }
}

/// What makes two spellings the same folder: on Windows, neither the case nor the slash tells them apart.
fn key(path: &str) -> String {
    let path = clean(path);
    if cfg!(windows) { path.to_lowercase().replace('/', "\\") } else { path.to_string() }
}

impl Folders {
    pub(crate) fn is_empty(&self) -> bool {
        self.recent.is_empty() && self.favorite.is_empty()
    }

    pub(crate) fn is_favorite(&self, path: &str) -> bool {
        self.favorite.iter().any(|f| key(f) == key(path))
    }

    /// A folder that was just used: the first of the recent ones, which keep the last `RECENT`.
    pub(crate) fn remember(&mut self, path: &str) {
        let path = clean(path);
        if path.is_empty() || path == "~" {
            return;
        }
        self.recent.retain(|p| key(p) != key(path));
        self.recent.insert(0, path.to_string());
        self.recent.truncate(RECENT);
    }

    /// Stars the folder, or takes its star off; whether it is a favorite now.
    pub(crate) fn toggle_favorite(&mut self, path: &str) -> bool {
        let path = clean(path);
        if path.is_empty() {
            return false;
        }
        let before = self.favorite.len();
        self.favorite.retain(|p| key(p) != key(path));
        if self.favorite.len() == before {
            self.favorite.push(path.to_string());
        }
        self.favorite.len() > before
    }

    /// The menu's rows: the favorites, then the recent ones that are not favorites.
    pub(crate) fn choices(&self) -> Vec<(String, bool)> {
        let recent = self.recent.iter().filter(|p| !self.is_favorite(p)).map(|p| (p.clone(), false));
        self.favorite.iter().map(|p| (p.clone(), true)).chain(recent).collect()
    }
}

/// What a click on the menu asked for.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum FolderPick {
    Use(String),
    Star(String),
}

/// The menu's rows: the folder's name over its path and, at the right, the word that stars it.
/// A folder that is gone stays listed (its star can still come off) but cannot be picked.
pub(crate) fn folder_rows(ui: &mut egui::Ui, folders: &Folders, c: &UiColors) -> Option<FolderPick> {
    let mut pick = None;
    for (path, favorite) in folders.choices() {
        // ponytail: one stat per row per frame while the menu is open; cache it if a network folder makes the menu stutter
        let there = std::path::Path::new(&path).is_dir();
        let (rect, response) = ui.allocate_exact_size(Vec2::new(ui.available_width(), 44.0), Sense::click());
        let star = Rect::from_min_max(Pos2::new(rect.right() - 80.0, rect.top()), rect.max);
        let star_response = ui.interact(star, ui.scope_id().with(("folder_star", &path)), Sense::click()).on_hover_cursor(CursorIcon::PointingHand);
        let over = star_response.hovered();
        if response.hovered() || over {
            ui.painter().rect_filled(rect, 6, c.surface_high);
        }
        let left = rect.left() + 12.0;
        let note = if there { path.as_str() } else { "a pasta não existe mais" };
        // a long name or path ends in "…" before the star's word
        let line = |text: &str, size: f32, color: Color32, y: f32| {
            let galley = crate::app::chat_list::clipped(ui.painter(), text.to_string(), FontId::proportional(size), color, star.left() - 6.0 - left);
            ui.painter().galley(Pos2::new(left, rect.top() + y), galley, color);
        };
        line(&crate::app::chat_direct::folder_label(&path), 14.0, if there { c.text } else { c.muted }, 8.0);
        line(note, 11.5, c.muted, 26.0);
        let (word, color) = if favorite { ("Favorita", c.text) } else { ("Favoritar", if over { c.text } else { c.muted }) };
        ui.painter().text(Pos2::new(rect.right() - 12.0, rect.center().y), egui::Align2::RIGHT_CENTER, word, FontId::proportional(11.5), color);
        if star_response.tip(if favorite { "Tirar das favoritas" } else { "Deixar sempre no topo da lista" }).clicked() {
            pick = Some(FolderPick::Star(path));
        } else if there && response.on_hover_cursor(CursorIcon::PointingHand).clicked() {
            pick = Some(FolderPick::Use(path));
        }
    }
    pick
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_used_folder_goes_first_once_and_only_the_last_ones_stay() {
        let mut f = Folders::default();
        for path in ["/a", "/b/", "  ", "~", "/a"] {
            f.remember(path);
        }
        assert_eq!(f.recent, ["/a", "/b"]); // again goes to the front, the separator and blanks do not count
        for n in 0..20 {
            f.remember(&format!("/p{n}"));
        }
        assert_eq!(f.recent.len(), RECENT);
        assert_eq!(f.recent[0], "/p19");
        f.remember("C:\\");
        assert_eq!(f.recent[0], "C:\\"); // a root keeps its separator
        if cfg!(windows) {
            f.remember("c:/Projetos/App");
            f.remember("C:\\projetos\\app\\");
            assert_eq!(f.recent.iter().filter(|p| p.to_lowercase().contains("projetos")).count(), 1);
        }
    }

    #[test]
    fn favorites_come_first_and_a_second_star_takes_it_off() {
        let mut f = Folders::default();
        f.remember("/a");
        f.remember("/b");
        assert!(f.toggle_favorite("/a/"));
        assert!(f.toggle_favorite("/c"));
        // the favorites in the order they were starred, then what is only recent
        assert_eq!(f.choices(), [("/a".to_string(), true), ("/c".to_string(), true), ("/b".to_string(), false)]);
        assert!(!f.toggle_favorite("/a"));
        assert_eq!(f.choices(), [("/c".to_string(), true), ("/b".to_string(), false), ("/a".to_string(), false)]);
        assert!(!f.toggle_favorite("  "));
        assert!(!Folders::default().toggle_favorite(""));
        assert!(Folders::default().is_empty() && !f.is_empty());
    }
}
