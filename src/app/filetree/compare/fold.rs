use std::collections::HashSet;

/// Lines of context kept above and below each change when the unchanged stretches are collapsed (the IDE's own).
pub(crate) const CONTEXT: usize = 4;
/// A stretch hides only when it spares at least this many lines (a marker for one line would be longer than the line).
const LEAST: usize = 2;

/// What the viewer lists for each row of the diff: the row itself or one marker for the unchanged rows it hides.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) enum Plan {
    Show(usize),
    /// `count` rows from `first` hidden; `key` (the old file's line the stretch starts at) names the stretch.
    Fold {
        key: usize,
        first: usize,
        count: usize,
    },
}

/// Which rows show. `keys` has, for each row, `Some(its line in the old file)` when it is unchanged and `None` when it is a
/// change. Each long unchanged stretch keeps `CONTEXT` rows next to the changes and hides the rest, unless `opened` has its
/// key; with no change at all, or `collapse` off, everything shows.
pub(crate) fn visible_plan(keys: &[Option<usize>], opened: &HashSet<usize>, collapse: bool) -> Vec<Plan> {
    let mut plan = vec![];
    let mut index = 0;
    while index < keys.len() {
        let Some(key) = keys[index] else {
            plan.push(Plan::Show(index));
            index += 1;
            continue;
        };
        let end = index + keys[index..].iter().take_while(|key| key.is_some()).count();
        let head = if index > 0 { CONTEXT } else { 0 };
        let tail = if end < keys.len() { CONTEXT } else { 0 };
        let hidden = (end - index).saturating_sub(head + tail);
        let whole = index == 0 && end == keys.len();
        if collapse && !whole && hidden >= LEAST && !opened.contains(&key) {
            plan.extend((index..index + head).map(Plan::Show));
            plan.push(Plan::Fold { key, first: index + head, count: hidden });
            plan.extend((end - tail..end).map(Plan::Show));
        } else {
            plan.extend((index..end).map(Plan::Show));
        }
        index = end;
    }
    plan
}
