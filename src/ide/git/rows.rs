use super::*;

#[derive(Clone, Copy, PartialEq, Debug)]
pub enum MarkKind {
    Added,
    Modified,
    Deleted,
}

/// Gutter change markers: [start, end) line ranges of the new text.
pub type Mark = (usize, usize, MarkKind);

pub fn line_marks(old: &str, new: &str) -> Vec<Mark> {
    TextDiff::from_lines(old, new)
        .ops()
        .iter()
        .filter_map(|op| {
            let (tag, _, n) = op.as_tag_tuple();
            match tag {
                DiffTag::Equal => None,
                DiffTag::Insert => Some((n.start, n.end, MarkKind::Added)),
                DiffTag::Delete => Some((n.start, n.start, MarkKind::Deleted)),
                DiffTag::Replace => Some((n.start, n.end, MarkKind::Modified)),
            }
        })
        .collect()
}

#[derive(Clone, Copy, PartialEq, Debug)]
pub enum RowKind {
    Same,
    Insert,
    Delete,
    Change,
}

/// An edit made in a diff's right side.
pub enum DiffEdit {
    /// » on a block (true: Ctrl, keep yours and add HEAD's after)
    Revert(usize, bool),
    /// a line typed over
    Line(usize, String),
    Undo,
    Redo,
}

/// One aligned row of the side-by-side viewer (line indexes into old/new).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct DiffRow {
    pub left: Option<usize>,
    pub right: Option<usize>,
    pub kind: RowKind,
}

/// `new` with its `block`-th change (counted like `Diff::blocks`) put back as it is in `old`:
/// PhpStorm's » between the two sides of a diff. `append` (Ctrl+») keeps the new lines and puts
/// the old ones after them.
pub fn revert(old: &str, new: &str, block: usize, append: bool) -> String {
    // the differ's own lines (a lone \r ends one too), so the ops index the right slices
    let diff = TextDiff::from_lines(old, new);
    let (o, n) = (diff.old_slices(), diff.new_slices());
    let (mut out, mut seen, mut in_change) = (String::new(), 0, false);
    let mut tail = String::new(); // the old lines of an appended block, after its new ones
    for op in diff.ops() {
        let (tag, a, b) = op.as_tag_tuple();
        if tag == DiffTag::Equal {
            seen += in_change as usize;
            in_change = false;
            out.push_str(&tail);
            tail.clear();
            out.extend(n[b].iter().copied());
        } else {
            in_change = true;
            let target = seen == block;
            if target && append {
                let old_lines = &o[a];
                out.extend(n[b].iter().copied());
                if !old_lines.is_empty() && !out.ends_with('\n') {
                    out.push('\n');
                }
                tail.extend(old_lines.iter().copied());
            } else {
                out.extend(if target { &o[a] } else { &n[b] }.iter().copied());
            }
        }
    }
    out.push_str(&tail);
    out
}

/// Byte ranges of the words that differ between two paired lines: (old side, new side).
pub fn word_marks(old: &str, new: &str) -> (Vec<Range<usize>>, Vec<Range<usize>>) {
    let (mut o, mut n, mut at_o, mut at_n) = (vec![], vec![], 0, 0);
    for c in TextDiff::from_words(old, new).iter_all_changes() {
        let len = c.value().len();
        match c.tag() {
            ChangeTag::Equal => {
                at_o += len;
                at_n += len;
            }
            ChangeTag::Delete => {
                o.push(at_o..at_o + len);
                at_o += len;
            }
            ChangeTag::Insert => {
                n.push(at_n..at_n + len);
                at_n += len;
            }
        }
    }
    (o, n)
}

/// The rows with the lines compared as `ignore` says (the lines themselves stay as they are).
pub fn diff_rows_ignoring(old: &str, new: &str, ignore: Ignore) -> Vec<DiffRow> {
    if ignore == Ignore::None {
        return diff_rows(old, new);
    }
    let keyed = |s: &str| {
        s.lines()
            .map(|l| {
                ignore.key(l)
                    + "
"
            })
            .collect::<String>()
    };
    diff_rows(&keyed(old), &keyed(new))
}

/// The first row of every changed block.
pub(super) fn blocks_of(rows: &[DiffRow]) -> Vec<usize> {
    (0..rows.len()).filter(|&i| rows[i].kind != RowKind::Same && (i == 0 || rows[i - 1].kind == RowKind::Same)).collect()
}

pub fn diff_rows(old: &str, new: &str) -> Vec<DiffRow> {
    let mut out = vec![];
    for op in TextDiff::from_lines(old, new).ops() {
        let (tag, o, n) = op.as_tag_tuple();
        let kind = match tag {
            DiffTag::Equal => RowKind::Same,
            DiffTag::Insert => RowKind::Insert,
            DiffTag::Delete => RowKind::Delete,
            DiffTag::Replace => RowKind::Change,
        };
        for i in 0..o.len().max(n.len()) {
            out.push(DiffRow { left: (i < o.len()).then(|| o.start + i), right: (i < n.len()).then(|| n.start + i), kind });
        }
    }
    out
}

pub struct Diff {
    pub old: Vec<String>,
    pub new: Vec<String>,
    pub rows: Vec<DiffRow>,
    /// first row of every changed block, for "next/previous difference"
    pub blocks: Vec<usize>,
    pub current: usize,
    pub scroll_to: Option<usize>,
    /// tab title given by Claude Code's openDiff
    pub title: Option<String>,
    /// pending Claude Code openDiff: reply channel + proposed text
    pub claude: Option<(std::sync::mpsc::Sender<Vec<String>>, String)>,
    /// what the person wants to say about this change, sent to the agent
    pub comment: String,
    /// HEAD's text this diff is against: the buffer's diff is recomputed from it on every edit
    pub base: String,
    /// the buffer's text this diff was last computed from (a hash), so an edit elsewhere shows here
    pub seen: u64,
    /// the buffer before each » or line edit made here, newest last, for ⌘Z; ⌘⇧Z walks `redo` back
    pub undo: Vec<String>,
    pub redo: Vec<String>,
    /// the right-side line being typed into: its index and its text so far
    pub editing: Option<(usize, String)>,
    /// a commit's diff (history.rs): the names of its two sides, both read-only
    pub sides: Option<(String, String)>,
    /// what the differ leaves out when it compares lines (the toolbar's "Não ignorar" menu)
    pub ignore: Ignore,
}

impl Diff {
    /// The current block's lines (1-based, inclusive) and their text: what a comment about "this
    /// change" points at. The new side, or the old side when the block only removes lines
    /// (`removed`).
    pub fn current_lines(&self) -> Option<(usize, usize, Vec<&str>, bool)> {
        let start = *self.blocks.get(self.current)?;
        let block: Vec<&DiffRow> = self.rows[start..].iter().take_while(|r| r.kind != RowKind::Same).collect();
        let (lines, text, removed): (Vec<usize>, &[String], bool) = match block.iter().filter_map(|r| r.right).collect::<Vec<_>>() {
            l if !l.is_empty() => (l, &self.new, false),
            _ => (block.iter().filter_map(|r| r.left).collect(), &self.old, true),
        };
        let (first, last) = (*lines.first()?, *lines.last()?);
        Some((first + 1, last + 1, lines.iter().filter_map(|&l| text.get(l).map(String::as_str)).collect(), removed))
    }

    /// The rows of block `k`: from its first row up to the next unchanged one.
    pub fn block_rows(&self, k: usize) -> Range<usize> {
        let start = self.blocks[k];
        let end = self.rows[start..].iter().position(|r| r.kind == RowKind::Same).map_or(self.rows.len(), |p| start + p);
        start..end
    }

    pub fn new(old: &str, new: &str) -> Self {
        Self::build(old, new, Ignore::None)
    }

    fn build(old: &str, new: &str, ignore: Ignore) -> Self {
        let rows = diff_rows_ignoring(old, new, ignore);
        let blocks = blocks_of(&rows);
        let scroll_to = blocks.first().copied();
        Diff {
            old: old.lines().map(String::from).collect(),
            new: new.lines().map(String::from).collect(),
            rows,
            blocks,
            current: 0,
            scroll_to,
            title: None,
            claude: None,
            comment: String::new(),
            base: old.to_string(),
            seen: 0,
            undo: Vec::new(),
            redo: Vec::new(),
            editing: None,
            sides: None,
            ignore,
        }
    }

    /// The working copy's diff against its base: the buffer is its right side and can be edited.
    pub fn is_plain(&self) -> bool {
        self.claude.is_none() && self.sides.is_none()
    }

    /// The rows and blocks again after the buffer became `new`; the current block stays if it exists.
    /// What the buffer `text` becomes after `edit` made in the diff, keeping the ⌘Z / ⌘⇧Z history;
    /// None when nothing changes.
    pub fn apply(&mut self, text: &str, edit: DiffEdit) -> Option<String> {
        let next = match edit {
            DiffEdit::Revert(k, append) => revert(&self.base, text, k, append),
            DiffEdit::Line(n, line) => {
                let mut lines: Vec<&str> = text.split('\n').collect();
                if lines.get(n).is_none_or(|l| *l == line) {
                    return None;
                }
                lines[n] = &line;
                lines.join("\n")
            }
            DiffEdit::Undo => return self.undo.pop().inspect(|_| self.redo.push(text.to_string())),
            DiffEdit::Redo => return self.redo.pop().inspect(|_| self.undo.push(text.to_string())),
        };
        if next == text {
            return None;
        }
        self.undo.push(text.to_string());
        self.redo.clear();
        Some(next)
    }

    pub fn recompute(&mut self, new: &str) {
        let fresh = Diff::build(&self.base, new, self.ignore);
        self.current = self.current.min(fresh.blocks.len().saturating_sub(1));
        (self.old, self.new, self.rows, self.blocks) = (fresh.old, fresh.new, fresh.rows, fresh.blocks);
    }
}

impl Diff {
    /// The rows again under another ignore policy, from the lines on show.
    pub fn set_ignore(&mut self, ignore: Ignore) {
        self.ignore = ignore;
        let fresh = Diff::build(
            &self.old.join(
                "
",
            ),
            &self.new.join(
                "
",
            ),
            ignore,
        );
        self.current = self.current.min(fresh.blocks.len().saturating_sub(1));
        (self.rows, self.blocks) = (fresh.rows, fresh.blocks);
        self.scroll_to = self.blocks.get(self.current).copied();
    }
}
