//! A sticky note's own actions: markdown on the text being edited (its bar's buttons), the text
//! size, the rename. Its text is the label element; the card holds the settings.

pub(crate) mod buttons;
mod editor;
pub(crate) mod file;
pub(crate) mod scroll;
pub(crate) mod stack;
pub(crate) mod storage;

use crate::prelude::*;
use egui::text::{CCursor, CCursorRange};
use egui::text_edit::TextEditState;

/// The editor's TextEdit id, so the bar can move its cursor and read its selection. It must stay
/// the same from frame to frame (the caret is kept under it), which `Id::unique` can't give.
#[allow(deprecated)]
pub(crate) fn editor_id() -> egui::Id {
    egui::Id::new("note_text")
}

/// Applies a markdown button to `text` between the char indices `start..end` (the caret when they
/// are equal). `operation` is the mark itself: `**`, `*`, `~~`, `` ` `` wrap the selection; `# `, `## `,
/// `### `, `- `, `- [ ] ` change the lines it touches (toggled off when the line already has it);
/// ```` ``` ```` wraps it in a code block. Returns the new text and the new selection.
pub(crate) fn format_text(text: &str, start: usize, end: usize, operation: &str) -> (String, usize, usize) {
    let chars: Vec<char> = text.chars().collect();
    let (start, end) = (start.min(end).min(chars.len()), start.max(end).min(chars.len()));
    let slice = |range: std::ops::Range<usize>| chars[range].iter().collect::<String>();
    let operation_length = operation.chars().count();
    if operation.ends_with(' ') {
        let line_start = chars[..start].iter().rposition(|character| *character == '\n').map_or(0, |index| index + 1);
        let line_end = chars[end..].iter().position(|character| *character == '\n').map_or(chars.len(), |index| end + index);
        let edited: Vec<String> = slice(line_start..line_end)
            .split('\n')
            .map(|line| {
                let known = line_prefix(line);
                let rest = &line[known.len()..];
                if known == operation { rest.to_string() } else { format!("{operation}{rest}") }
            })
            .collect();
        let block = edited.join("\n");
        let end_at = line_start + block.chars().count();
        let text = format!("{}{block}{}", slice(0..line_start), slice(line_end..chars.len()));
        return if start == end { (text, end_at, end_at) } else { (text, line_start, end_at) };
    }
    if operation == "```" {
        let (before, selection, after) = (slice(0..start), slice(start..end), slice(end..chars.len()));
        // the selected text goes inside the block (an empty caret gets an empty block)
        let text = format!("{before}```\n{selection}\n```{after}");
        let inner = start + 4 + selection.chars().count();
        return (text, start + 4, inner);
    }
    let (before, selection, after) = (slice(0..start), slice(start..end), slice(end..chars.len()));
    let marked = |characters: &[char], mark: &str| {
        characters.len() >= operation_length && characters.iter().zip(mark.chars()).all(|(actual, expected)| *actual == expected)
    };
    // already wrapped around the selection: take the marks off again (toggle)
    if start >= operation_length
        && marked(&chars[start - operation_length..start], operation)
        && marked(&chars[end..(end + operation_length).min(chars.len())], operation)
    {
        let text = format!("{}{selection}{}", slice(0..start - operation_length), slice(end + operation_length..chars.len()));
        return (text, start - operation_length, end - operation_length);
    }
    let text = format!("{before}{operation}{selection}{operation}{after}");
    if start == end { (text, start + operation_length, start + operation_length) } else { (text, start + operation_length, end + operation_length) }
}

/// The markdown prefix a line starts with, among the ones the bar sets (longest first).
fn line_prefix(line: &str) -> &str {
    ["### ", "## ", "# ", "- [ ] ", "- [x] ", "- "].into_iter().find(|prefix| line.starts_with(prefix)).unwrap_or("")
}

/// The note's first line set to `# name`: its name is the first line of its text.
pub(crate) fn rename_note_text(text: &str, name: &str) -> String {
    let heading = format!("# {}", name.trim());
    let mut lines: Vec<String> = text.lines().map(String::from).collect();
    match lines.iter().position(|line| !line.trim().is_empty()) {
        Some(index) => {
            lines[index] = heading;
            lines.join("\n")
        }
        None => heading,
    }
}

impl DrawingApp {
    /// Applies a markdown mark to the note's text. The note takes the focus first (as a click
    /// does), keeps the selection it had in the editor, and the editor takes the keyboard back.
    pub(crate) fn note_format(&mut self, ctx: &Context, note: usize, operation: &str) {
        let Some(label) = note_label(&self.scene.elements, note) else { return };
        if self.scene.elements[note].note.content_locked {
            return;
        }
        let ElementKind::Text { value, .. } = &self.scene.elements[label].kind else { return };
        let length = value.chars().count();
        // the selection the editor had while it held the keys, if it was this note's
        let (start, end) = match self.note_selection {
            Some((at, from, to)) if at == label => (from.min(length), to.min(length)),
            _ => (length, length),
        };
        self.snapshot();
        let ElementKind::Text { value, .. } = &mut self.scene.elements[label].kind else { return };
        let (text, from, to) = format_text(value, start, end, operation);
        *value = text;
        self.selected = vec![note];
        self.editing = Some(label);
        self.edit_focused = false;
        let mut state = TextEditState::load(ctx, editor_id()).unwrap_or_default();
        state.cursor.set_char_range(Some(CCursorRange::two(CCursor::new(from), CCursor::new(to))));
        state.store(ctx, editor_id());
    }

    /// The note's text size, one step at a time, between the reference app's limits.
    pub(crate) fn note_font_step(&mut self, note: usize, delta: f32) {
        self.notes_font_step(&[note], delta);
    }

    /// Steps the text size of several notes at once, one undo step for all.
    pub(crate) fn notes_font_step(&mut self, notes: &[usize], delta: f32) {
        let sizes: Vec<f32> = notes.iter().map(|note| (self.scene.elements[*note].note.font_size + delta).clamp(NOTE_FONT_MIN, NOTE_FONT_MAX)).collect();
        if notes.iter().zip(&sizes).all(|(note, size)| self.scene.elements[*note].note.font_size == *size) {
            return;
        }
        self.snapshot();
        for (note, size) in notes.iter().zip(sizes) {
            self.scene.elements[*note].note.font_size = size;
        }
    }

    /// The note whose text is being edited: it holds the focus (static outline, text cursor).
    pub(crate) fn note_focus_id(&self) -> Option<String> {
        let label = self.editing?;
        let note = note_of_label(&self.scene.elements, self.scene.elements.get(label)?)?;
        Some(self.scene.elements[note].id.clone())
    }

    /// The text a note's name is read from (its first line).
    pub(crate) fn note_text(&self, note: usize) -> String {
        note_label(&self.scene.elements, note)
            .and_then(|label| match &self.scene.elements[label].kind {
                ElementKind::Text { value, .. } => Some(value.clone()),
                _ => None,
            })
            .unwrap_or_default()
    }
}
