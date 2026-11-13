//! The file list behind the tree's Go to File, the index and the project's text.

use super::*;

/// All files under root, without the project's excluded folders.
pub(crate) fn list_files(root: &Path) -> Vec<PathBuf> {
    list_files_with(root, &excluded::Excluded::load(root))
}

pub(crate) fn list_files_with(root: &Path, ex: &excluded::Excluded) -> Vec<PathBuf> {
    let mut out = vec![];
    let mut stack = vec![root.to_path_buf()];
    while let Some(dir) = stack.pop() {
        for e in std::fs::read_dir(&dir).into_iter().flatten().flatten() {
            if e.file_type().is_ok_and(|t| t.is_dir()) {
                if !ex.contains(&e.path()) {
                    stack.push(e.path());
                }
            } else {
                out.push(e.path());
            }
            if out.len() >= 2_000_000 {
                return out; // ponytail: runaway guard, a project this big wants a real file watcher
            }
        }
    }
    out
}
