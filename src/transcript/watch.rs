//! Which Claude Code session each terminal runs, found off the interface's thread: a folder of
//! files to read and, at times, the list of the system's processes, twice a second.

use super::{Session, assign, parents, sessions};
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

/// A terminal that may run Claude Code: its id, folder, start (ms since 1970) and its shell's pid.
pub(crate) type Terminal = (String, String, i64, u32);

/// The watcher: the interface says which terminals to look at and takes what was found; the looking
/// is a thread's, which goes away once nobody asks for a while.
#[derive(Clone, Default)]
pub(crate) struct SessionWatch {
    input: Arc<Mutex<(Vec<Terminal>, Option<PathBuf>)>>,
    found: Arc<Mutex<Option<Vec<(String, Session)>>>>,
    running: Arc<AtomicBool>,
    asked: Arc<AtomicU64>,
}

fn now_secs() -> u64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map_or(0, |d| d.as_secs())
}

impl SessionWatch {
    /// The terminals to look at now and Claude Code's folder; starts the thread when it isn't there.
    pub(crate) fn ask(&self, terminals: Vec<Terminal>, dir: Option<PathBuf>) {
        *self.input.lock().unwrap_or_else(|e| e.into_inner()) = (terminals, dir);
        self.asked.store(now_secs(), Ordering::Relaxed);
        if self.running.swap(true, Ordering::Relaxed) {
            return;
        }
        let (input, found, running, asked) = (self.input.clone(), self.found.clone(), self.running.clone(), self.asked.clone());
        std::thread::spawn(move || {
            // nobody asked for ten seconds (the app closed, no agent left): the thread ends, and the next ask starts another
            while now_secs().saturating_sub(asked.load(Ordering::Relaxed)) <= 10 {
                let (terminals, dir) = input.lock().unwrap_or_else(|e| e.into_inner()).clone();
                let result = match &dir {
                    Some(dir) if !terminals.is_empty() => assign(&terminals, &sessions(&dir.join("sessions")), parents),
                    _ => Vec::new(),
                };
                *found.lock().unwrap_or_else(|e| e.into_inner()) = Some(result);
                std::thread::sleep(Duration::from_millis(500));
            }
            running.store(false, Ordering::Relaxed);
        });
    }

    /// What the thread found since the last take, if it found anything new.
    pub(crate) fn take(&self) -> Option<Vec<(String, Session)>> {
        self.found.lock().unwrap_or_else(|e| e.into_inner()).take()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_sessions_are_looked_for_off_the_callers_thread() {
        let dir = std::env::temp_dir().join(format!("mago-watch-{}", std::process::id()));
        std::fs::create_dir_all(dir.join("sessions")).unwrap();
        let watch = SessionWatch::default();
        assert_eq!(watch.take(), None, "nothing asked, nothing found");
        watch.ask(vec![("t1".into(), dir.to_string_lossy().into_owned(), 0, 1)], Some(dir.clone()));
        let mut found = None;
        for _ in 0..100 {
            found = watch.take();
            if found.is_some() {
                break;
            }
            std::thread::sleep(Duration::from_millis(30));
        }
        // an empty folder of sessions: an answer, with nothing in it
        assert_eq!(found, Some(Vec::new()));
        let _ = std::fs::remove_dir_all(dir);
    }
}
