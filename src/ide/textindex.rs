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
    /// Text held in memory, in bytes (the benchmark below).
    #[cfg(test)]
    pub(crate) fn bytes(&self) -> usize {
        self.entries.iter().map(|e| e.text.as_ref().map_or(0, |t| t.len())).sum()
    }

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

    /// (Re)reads one file after it changed; a file gone or no longer text leaves the index.
    pub(crate) fn update(&mut self, p: &Path) {
        let entry = stamp(p).and_then(|stamp| read_text(p).map(|t| Entry { path: p.to_path_buf(), stamp, text: Some(t.into()) }));
        match (self.entries.iter().position(|e| e.path == p), entry) {
            (Some(i), Some(e)) => self.entries[i] = e,
            (Some(i), None) => _ = self.entries.swap_remove(i),
            (None, Some(e)) => self.entries.push(e),
            (None, None) => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::AtomicUsize;

    fn write(root: &Path, rel: &str, text: &str) -> PathBuf {
        let p = root.join(rel);
        std::fs::create_dir_all(p.parent().unwrap()).unwrap();
        std::fs::write(&p, text).unwrap();
        p
    }

    #[test]
    fn text_search_finds_json_and_skips_excluded_folders() {
        let dir = std::env::temp_dir().join(format!("mago-textindex-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let config = write(&dir, "config/app.json", "{\n  \"Needle\": 1\n}\n");
        write(&dir, "src/main.rs", "fn main() {}\n");
        write(&dir, "node_modules/pkg/index.js", "needle in a dependency\n");
        write(&dir, "vendor/lib/a.php", "needle again\n");
        std::fs::write(dir.join("logo.png"), [0x89, b'P', 0, 0, b'n', b'e', b'e', b'd', b'l', b'e']).unwrap();
        let mut ex = excluded::Excluded::load_from(None, &dir);
        let files = tree::list_files_with(&dir, &ex);
        assert!(files.iter().all(|f| !f.to_string_lossy().contains("node_modules") && !f.to_string_lossy().contains("vendor")));
        let mut ix = TextIndex::build(&files, &TextIndex::default());
        assert_eq!(ix.entries.len(), 2, "the binary file is left out");
        let hits = find::search_index(&ix, "needle", false, false, &AtomicUsize::new(0), 0, 100);
        assert_eq!((hits.len(), &hits[0].path, hits[0].line), (1, &config, 1));
        // a saved file is read again; a removed folder's exclusion brings its files back
        std::fs::write(&config, "{ \"other\": 2 }\n").unwrap();
        ix.update(&config);
        assert!(find::search_index(&ix, "needle", false, false, &AtomicUsize::new(0), 0, 100).is_empty());
        ex.set(&dir.join("vendor"), false).unwrap();
        let files = tree::list_files_with(&dir, &ex);
        let ix = TextIndex::build(&files, &ix);
        let hits = find::search_index(&ix, "NEEDLE", false, false, &AtomicUsize::new(0), 0, 100);
        assert_eq!(hits.len(), 1);
        assert!(hits[0].path.ends_with("a.php"));
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// `cargo test -- --ignored --nocapture timing`: a project with 150k files in the folders kept
    /// and 300k in excluded ones.
    #[test]
    #[ignore]
    fn timing() {
        let dir = std::env::temp_dir().join("mago-textindex-timing");
        let _ = std::fs::remove_dir_all(&dir);
        let body = "line of code with some words\n".repeat(40);
        for (top, dirs, per) in [("src", 1500, 100), ("node_modules", 2000, 100), ("vendor", 1000, 100)] {
            for d in 0..dirs {
                for f in 0..per {
                    write(&dir, &format!("{top}/d{d}/f{f}.json"), &format!("{body}marker{}\n", d * per + f));
                }
            }
        }
        let ex = excluded::Excluded::load_from(None, &dir);
        let t = std::time::Instant::now();
        let files = tree::list_files_with(&dir, &ex);
        let listed = t.elapsed();
        let t = std::time::Instant::now();
        let ix = TextIndex::build(&files, &TextIndex::default());
        let built = t.elapsed();
        let t = std::time::Instant::now();
        let again = TextIndex::build(&files, &ix);
        let reused = t.elapsed();
        let t = std::time::Instant::now();
        let hits = find::search_index(&again, "marker149999", false, false, &AtomicUsize::new(0), 0, 2000);
        println!(
            "{} files, list {listed:?}, build {built:?}, rebuild {reused:?}, {} MB, search {:?} ({} hits)",
            files.len(),
            ix.bytes() >> 20,
            t.elapsed(),
            hits.len()
        );
        let _ = std::fs::remove_dir_all(&dir);
    }
}
