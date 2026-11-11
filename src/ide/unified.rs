//! Rows for the one-column diff view.

use super::git::{DiffRow, RowKind};

/// The rows of the unified viewer, each with the line it pairs with when it is half of a modified
/// row (what the changed words are found against). A modified row becomes its removed line and
/// then its added one, both still `Change`: the viewer paints them in the same blue.
pub(super) fn unified_rows(rows: &[DiffRow]) -> Vec<(DiffRow, Option<usize>)> {
    let mut out = Vec::with_capacity(rows.len());
    let mut i = 0;
    while i < rows.len() {
        let end = i + rows[i..].iter().take_while(|r| r.kind == RowKind::Change).count();
        if end == i {
            out.push((rows[i], None));
            i += 1;
            continue;
        }
        // a modified block: every removed line, then every added one
        let run = &rows[i..end];
        out.extend(run.iter().filter_map(|r| Some((DiffRow { left: Some(r.left?), right: None, kind: RowKind::Change }, r.right))));
        out.extend(run.iter().filter_map(|r| Some((DiffRow { left: None, right: Some(r.right?), kind: RowKind::Change }, r.left))));
        i = end;
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ide::git::diff_rows;

    #[test]
    fn unified_sequence_handles_added_removed_changed_and_context_blocks() {
        let rows = unified_rows(&diff_rows("a\nb\nc\nd\ne\nf\ng\n", "a\nB\nc\nD\ne\nF\ng\n"));
        let kinds = |r: &[(DiffRow, Option<usize>)]| r.iter().map(|r| r.0.kind).collect::<Vec<_>>();
        let (s, c) = (RowKind::Same, RowKind::Change);
        assert_eq!(kinds(&rows), vec![s, c, c, s, c, c, s, c, c, s]);
        // the removed half knows the added line it pairs with, and the other way round
        assert_eq!((rows[1].0.left, rows[1].1), (Some(1), Some(1)));
        assert_eq!((rows[2].0.right, rows[2].1), (Some(1), Some(1)));
        assert_eq!(unified_rows(&diff_rows("a\n", "a\nx\n"))[1].0, DiffRow { left: None, right: Some(1), kind: RowKind::Insert });
        assert_eq!(unified_rows(&diff_rows("a\nx\n", "a\n"))[1].0, DiffRow { left: Some(1), right: None, kind: RowKind::Delete });
    }
}
