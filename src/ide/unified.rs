//! Rows for the one-column diff view.

use super::git::{DiffRow, RowKind};

pub(super) fn unified_rows(rows: &[DiffRow]) -> Vec<DiffRow> {
    let mut out = Vec::with_capacity(rows.len());
    for row in rows {
        if row.kind == RowKind::Change {
            if let Some(left) = row.left {
                out.push(DiffRow { left: Some(left), right: None, kind: RowKind::Delete });
            }
            if let Some(right) = row.right {
                out.push(DiffRow { left: None, right: Some(right), kind: RowKind::Insert });
            }
        } else {
            out.push(*row);
        }
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
        assert_eq!(
            rows.iter().map(|r| r.kind).collect::<Vec<_>>(),
            vec![
                RowKind::Same,
                RowKind::Delete,
                RowKind::Insert,
                RowKind::Same,
                RowKind::Delete,
                RowKind::Insert,
                RowKind::Same,
                RowKind::Delete,
                RowKind::Insert,
                RowKind::Same,
            ]
        );
        assert_eq!(unified_rows(&diff_rows("a\n", "a\nx\n"))[1], DiffRow { left: None, right: Some(1), kind: RowKind::Insert });
        assert_eq!(unified_rows(&diff_rows("a\nx\n", "a\n"))[1], DiffRow { left: Some(1), right: None, kind: RowKind::Delete });
    }
}
