use super::*;

#[derive(Clone, Copy, PartialEq, Debug)]
pub enum RowKind {
    Same,
    Insert,
    Delete,
    Change,
}

/// One aligned row of the side-by-side viewer (line indexes into old/new).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct DiffRow {
    pub left: Option<usize>,
    pub right: Option<usize>,
    pub kind: RowKind,
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
