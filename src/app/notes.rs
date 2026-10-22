//! A sticky note's own actions: markdown on the text being edited (its bar's buttons), the text
//! size, the rename. Its text is the label element; the card holds the settings.

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
/// are equal). `op` is the mark itself: `**`, `*`, `~~`, `` ` `` wrap the selection; `# `, `## `,
/// `### `, `- `, `- [ ] ` change the lines it touches (toggled off when the line already has it);
/// ```` ``` ```` wraps it in a code block. Returns the new text and the new selection.
pub(crate) fn format_text(text: &str, start: usize, end: usize, op: &str) -> (String, usize, usize) {
    let chars: Vec<char> = text.chars().collect();
    let (start, end) = (start.min(end).min(chars.len()), start.max(end).min(chars.len()));
    let slice = |r: std::ops::Range<usize>| chars[r].iter().collect::<String>();
    let op_len = op.chars().count();
    if op.ends_with(' ') {
        let line_start = chars[..start].iter().rposition(|c| *c == '\n').map_or(0, |i| i + 1);
        let line_end = chars[end..].iter().position(|c| *c == '\n').map_or(chars.len(), |i| end + i);
        let edited: Vec<String> = slice(line_start..line_end)
            .split('\n')
            .map(|line| {
                let known = line_prefix(line);
                let rest = &line[known.len()..];
                if known == op { rest.to_string() } else { format!("{op}{rest}") }
            })
            .collect();
        let block = edited.join("\n");
        let end_at = line_start + block.chars().count();
        let text = format!("{}{block}{}", slice(0..line_start), slice(line_end..chars.len()));
        return if start == end { (text, end_at, end_at) } else { (text, line_start, end_at) };
    }
    if op == "```" {
        let (before, sel, after) = (slice(0..start), slice(start..end), slice(end..chars.len()));
        // the selected text goes inside the block (an empty caret gets an empty block)
        let text = format!("{before}```\n{sel}\n```{after}");
        let inner = start + 4 + sel.chars().count();
        return (text, start + 4, inner);
    }
    let (before, sel, after) = (slice(0..start), slice(start..end), slice(end..chars.len()));
    let marked = |ch: &[char], mark: &str| ch.len() >= op_len && ch.iter().zip(mark.chars()).all(|(a, b)| *a == b);
    // already wrapped around the selection: take the marks off again (toggle)
    if start >= op_len && marked(&chars[start - op_len..start], op) && marked(&chars[end..(end + op_len).min(chars.len())], op) {
        let text = format!("{}{sel}{}", slice(0..start - op_len), slice(end + op_len..chars.len()));
        return (text, start - op_len, end - op_len);
    }
    let text = format!("{before}{op}{sel}{op}{after}");
    if start == end { (text, start + op_len, start + op_len) } else { (text, start + op_len, end + op_len) }
}

/// The markdown prefix a line starts with, among the ones the bar sets (longest first).
fn line_prefix(line: &str) -> &str {
    ["### ", "## ", "# ", "- [ ] ", "- [x] ", "- "].into_iter().find(|p| line.starts_with(p)).unwrap_or("")
}

/// The note's first line set to `# name`: its name is the first line of its text.
pub(crate) fn rename_note_text(text: &str, name: &str) -> String {
    let heading = format!("# {}", name.trim());
    let mut lines: Vec<String> = text.lines().map(String::from).collect();
    match lines.iter().position(|l| !l.trim().is_empty()) {
        Some(i) => {
            lines[i] = heading;
            lines.join("\n")
        }
        None => heading,
    }
}

impl DrawingApp {
    /// Applies a markdown mark to the note's text. The note takes the focus first (as a click
    /// does), keeps the selection it had in the editor, and the editor takes the keyboard back.
    pub(crate) fn note_format(&mut self, ctx: &Context, note: usize, op: &str) {
        let Some(label) = note_label(&self.scene.elements, note) else { return };
        if self.scene.elements[note].note.content_locked {
            return;
        }
        let ElementKind::Text { value, .. } = &self.scene.elements[label].kind else { return };
        let len = value.chars().count();
        // the selection the editor had while it held the keys, if it was this note's
        let (start, end) = match self.note_selection {
            Some((at, from, to)) if at == label => (from.min(len), to.min(len)),
            _ => (len, len),
        };
        self.snapshot();
        let ElementKind::Text { value, .. } = &mut self.scene.elements[label].kind else { return };
        let (text, from, to) = format_text(value, start, end, op);
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
        let size = (self.scene.elements[note].note.font_size + delta).clamp(NOTE_FONT_MIN, NOTE_FONT_MAX);
        if size != self.scene.elements[note].note.font_size {
            self.snapshot();
            self.scene.elements[note].note.font_size = size;
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
            .and_then(|l| match &self.scene.elements[l].kind {
                ElementKind::Text { value, .. } => Some(value.clone()),
                _ => None,
            })
            .unwrap_or_default()
    }
}
