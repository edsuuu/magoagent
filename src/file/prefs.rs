//! The preferences written off the interface's thread, one write at a time and in order: the keychain (for the API
//! keys) and the disk's fsync can take a while, and a window or a slider dragged changes them every frame.

use crate::prelude::*;
use std::sync::OnceLock;
use std::sync::mpsc::{Sender, channel};

/// One write (none: only waiting for the ones before it), and who waits for it.
struct Write {
    prefs: Option<(PathBuf, Prefs, Prefs)>,
    done: Option<Sender<Result<(), String>>>,
}

/// The writer's thread, started at its first write.
fn writer() -> &'static Sender<Write> {
    static WRITER: OnceLock<Sender<Write>> = OnceLock::new();

    WRITER.get_or_init(|| {
        let (sender, writes) = channel::<Write>();

        std::thread::spawn(move || {
            for write in writes {
                let written = write.prefs.map_or(Ok(()), |(path, prefs, before)| {
                    let json = serde_json::to_string_pretty(&prefs_for_file(&prefs, &before)).map_err(|error| error.to_string())?;

                    write_atomic(&path, json).map_err(|error| error.to_string())
                });

                if let Some(done) = write.done {
                    let _ = done.send(written);
                }
            }
        });

        sender
    })
}

/// Writes `prefs` to `path` on the writer's thread (`before`: as they were last written, for the keychain's keys);
/// with `wait`, returns once they are on disk (closing the app, before an update). Without, at once.
pub(crate) fn write_prefs(path: PathBuf, prefs: Prefs, before: Prefs, wait: bool) -> Result<(), String> {
    send(Write { prefs: Some((path, prefs, before)), done: None }, wait)
}

/// Returns once every write asked before is on disk.
pub(crate) fn prefs_written() {
    let _ = send(Write { prefs: None, done: None }, true);
}

fn send(mut write: Write, wait: bool) -> Result<(), String> {
    let (done, written) = channel();

    if wait {
        write.done = Some(done);
    }

    writer().send(write).map_err(|_| "o gravador das preferências parou".to_string())?;

    if !wait {
        return Ok(());
    }

    written.recv_timeout(std::time::Duration::from_secs(10)).map_err(|_| "as preferências demoraram a gravar".to_string())?
}
