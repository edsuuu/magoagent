//! The text behind Search Everywhere's Text tab and Find in Files: every text file of the project
//! (excluded folders left out) read once in the background, so typing never waits on the disk.

use super::*;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

/// Bigger files are not text worth searching (dumps, bundles).
pub(crate) const MAX_FILE: u64 = 5 << 20;
/// Text kept in memory; past it a file is read from the disk when searched.
const BUDGET: usize = 600 << 20;

#[derive(Clone)]
pub(crate) struct Entry {
    pub(crate) path: PathBuf,
    stamp: (u64, Option<SystemTime>),
    /// None: over the memory budget, read from the disk on each search
    pub(crate) text: Option<Arc<str>>,
}

#[derive(Clone, Default)]
pub(crate) struct TextIndex {
    pub(crate) entries: Vec<Entry>,
}

/// The file as text, or None for a binary or non-UTF-8 file.
pub(crate) fn read_text(p: &Path) -> Option<String> {
    let bytes = std::fs::read(p).ok()?;
    if bytes[..bytes.len().min(8192)].contains(&0) {
        return None;
    }
    String::from_utf8(bytes).ok()
}

fn stamp(p: &Path) -> Option<(u64, Option<SystemTime>)> {
    let m = std::fs::metadata(p).ok().filter(|m| m.len() <= MAX_FILE)?;
    Some((m.len(), m.modified().ok()))
}

impl TextIndex {
    /// Reads `files` on all cores; what `old` already holds with the same size and time is kept.
    pub(crate) fn build(files: &[PathBuf], old: &TextIndex) -> TextIndex {
        let reuse: std::collections::HashMap<&Path, &Entry> = old.entries.iter().map(|e| (e.path.as_path(), e)).collect();
        let used = AtomicUsize::new(0);
        let one = |p: &PathBuf| -> Option<Entry> {
            let stamp = stamp(p)?;
            if let Some(e) = reuse.get(p.as_path()).filter(|e| e.stamp == stamp) {
                used.fetch_add(e.text.as_ref().map_or(0, |t| t.len()), Ordering::Relaxed);
                return Some((*e).clone());
            }
            let text = read_text(p)?;
            let keep = used.fetch_add(text.len(), Ordering::Relaxed) + text.len() <= BUDGET;
            Some(Entry { path: p.clone(), stamp, text: keep.then(|| text.into()) })
        };
        let n = std::thread::available_parallelism().map_or(4, |n| n.get()).min(16);
        let chunk = files.len().div_ceil(n).max(1);
        let parts: Vec<Vec<Entry>> = std::thread::scope(|s| {
            let jobs: Vec<_> = files.chunks(chunk).map(|c| s.spawn(|| c.iter().filter_map(&one).collect::<Vec<_>>())).collect();
            jobs.into_iter().map(|j| j.join().unwrap_or_default()).collect()
        });
        TextIndex { entries: parts.concat() }
    }
}
