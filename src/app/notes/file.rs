//! A text file opened in a note on the canvas (asked of a chat's file card: "abrir no canvas em uma nota q já é um
//! editor"; the MCP's `open_as_note`): its words in a note sized to read them, beside the chat's card or in the middle
//! of the view, and the canvas shown on it. The note holds a copy, as a dropped file's does: it is the person's to edit.

use crate::prelude::*;
use std::path::Path;

/// The note a file opens in: wide enough for a line of prose (about 64 letters of the note's font), as tall as its
/// text up to most of the view.
const READ_WIDTH: f32 = 560.0;
const READ_MAX_HEIGHT: f32 = 720.0;
/// The share of the view's height the note takes at most.
const READ_VIEW_SHARE: f32 = 0.8;
/// Between the chat's card and the note beside it.
const BESIDE: f32 = 40.0;

/// The text files a note opens: markdown (formatted) and plain text (as typed).
pub(crate) fn opens_as_note(path: &Path) -> bool {
    path.extension().is_some_and(|extension| matches!(extension.to_string_lossy().to_lowercase().as_str(), "md" | "markdown" | "txt"))
}

/// How tall `text` lays out in a note `width` wide at the note's font, in its points: each line wrapped at the letters
/// that fit (the font is monospace), a heading taller.
// ponytail: an estimate without the fonts (the MCP has none at hand); the note scrolls what it misses, and resizes
pub(crate) fn note_text_height(text: &str, width: f32, font_size: f32) -> f32 {
    let letters = ((width - 2.0 * NOTE_PAD) / (font_size * 0.6)).floor().max(1.0);
    let line = font_size * crate::render::markdown::NOTE_LINE;
    let lines: f32 = text
        .lines()
        .map(|row| {
            let (size, rest) = match row.trim_start().split_once(' ') {
                Some((marks, rest)) if !marks.is_empty() && marks.chars().all(|c| c == '#') => (1.4, rest),
                _ => (1.0, row),
            };
            size * (rest.chars().count() as f32 * size / letters).ceil().max(1.0)
        })
        .sum();
    NOTE_HEADER + 2.0 * NOTE_PAD + lines * line
}

impl DrawingApp {
    /// Opens the text file `path` in a note on the canvas: beside the card of `chat` when it has one here, else in the
    /// middle of the view; the same file opened again goes to the note it is already in. The note is selected and the
    /// canvas glides to it (the chat screen gives way to the canvas). What it did, said.
    pub(crate) fn open_file_as_note(&mut self, path: &Path, chat: Option<&str>) -> Result<String, String> {
        let name = path.file_name().map_or_else(|| path.display().to_string(), |file_name| file_name.to_string_lossy().into_owned());
        if !path.is_file() {
            return Err(format!("{} não existe.", path.display()));
        }
        if !opens_as_note(path) {
            return Err(format!("{name} não é um arquivo de texto (.md ou .txt) para abrir numa nota."));
        }
        let text = crate::app::canvas::drops::note_text(path).ok_or_else(|| format!("{name} é grande demais ou não é texto para uma nota."))?;
        let same = (0..self.scene.elements.len()).find(|index| self.scene.elements[*index].created.is_some() && self.note_text(*index) == text);
        let (index, opened) = match same {
            Some(index) => (index, false),
            None => {
                let size = self.reading_size(&text);
                let beside = chat.and_then(|id| self.chat_node(id)).map(|node| local_bounds(&self.scene.elements[node]));
                let want = match beside {
                    Some(card) => Rect::from_min_size(Pos2::new(card.right() + BESIDE, card.top()), size),
                    None => {
                        let middle = if self.canvas_rect.is_positive() { self.to_world(self.canvas_rect.center()) } else { Pos2::ZERO };
                        Rect::from_center_size(middle, size)
                    }
                };
                let [mut note, label] = self.new_note(self.free_spot(want), text);
                // only markdown is formatted; a .txt shows as typed
                note.note.raw = path.extension().is_some_and(|extension| extension.eq_ignore_ascii_case("txt"));
                self.insert_elements(vec![note, label]);
                (self.scene.elements.len() - 2, true)
            }
        };
        self.selected = vec![index];
        self.show_canvas();
        // a window enlarged in the middle would hide it
        if self.terminal_elevated.is_some() && self.dock.is_none() {
            self.set_elevated(None);
        }
        self.glide_to(local_bounds(&self.scene.elements[index]).center());
        Ok(if opened { format!("{name} aberto numa nota no canvas.") } else { format!("{name} já estava numa nota: o canvas foi até ela.") })
    }

    /// The note's size for `text`: `READ_WIDTH` wide, as tall as the text up to most of the view (at its zoom).
    fn reading_size(&self, text: &str) -> Vec2 {
        let view = if self.canvas_rect.is_positive() { self.canvas_rect.height() * READ_VIEW_SHARE / self.zoom } else { READ_MAX_HEIGHT };
        let tallest = READ_MAX_HEIGHT.min(view).max(NOTE_MIN.y);
        let height = note_text_height(text, READ_WIDTH, crate::render::markdown::NOTE_FONT).clamp(NOTE_MIN.y, tallest);
        Vec2::new(READ_WIDTH, height)
    }
}
