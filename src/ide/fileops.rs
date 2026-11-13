//! File operations from the project tree: name checks, and sending to the system's trash.

use super::*;
use std::io;

/// Why `name` can't name a file or folder on `os` (as `std::env::consts::OS` says it), in the
/// words the tree shows; None when it can. An empty name is the caller's to refuse.
pub(crate) fn name_problem(name: &str, os: &str) -> Option<&'static str> {
    if os == "linux" {
        if name.contains('/') {
            return Some("Um nome não pode conter /");
        }
    } else if name.contains(['\\', '/', ':', '<', '>', '"', '|', '?', '*']) {
        return Some("Um nome não pode conter \\ / : < > \" | ? *");
    }
    if !name.is_empty() && name.chars().all(|c| c == '.') {
        return Some("Um nome não pode ter só pontos.");
    }
    // CON, NUL, COM1… are devices on Windows, with any extension
    let stem = name.split('.').next().unwrap_or_default().trim_end().to_ascii_uppercase();
    let device = ["CON", "PRN", "AUX", "NUL"].contains(&stem.as_str())
        || (stem.len() == 4 && (stem.starts_with("COM") || stem.starts_with("LPT")) && matches!(stem.as_bytes()[3], b'1'..=b'9'));
    (os == "windows" && device).then_some("O Windows reserva esse nome para um dispositivo.")
}

/// Sends `path` (a file or a whole folder) to the system's trash. Nothing is deleted for good
/// when the trash refuses it: the error comes back instead.
pub(crate) fn trash(path: &Path) -> io::Result<()> {
    #[cfg(windows)]
    {
        #[repr(C)]
        struct FileOp {
            hwnd: isize,
            func: u32,
            from: *const u16,
            to: *const u16,
            flags: u16,
            aborted: i32,
            mappings: *mut std::ffi::c_void,
            title: *const u16,
        }
        #[link(name = "shell32")]
        unsafe extern "system" {
            fn SHFileOperationW(op: *mut FileOp) -> i32;
        }
        use std::os::windows::ffi::OsStrExt;
        // a list of paths: each one ends in a NUL, the list in another
        let from: Vec<u16> = path.as_os_str().encode_wide().chain([0, 0]).collect();
        const FO_DELETE: u32 = 3;
        // FOF_SILENT | FOF_NOCONFIRMATION | FOF_ALLOWUNDO (the Recycle Bin) | FOF_NOERRORUI |
        // FOF_WANTNUKEWARNING: what the bin can't take is asked about, never deleted unasked
        const FLAGS: u16 = 0x4 | 0x10 | 0x40 | 0x400 | 0x4000;
        let mut op = FileOp {
            hwnd: 0,
            func: FO_DELETE,
            from: from.as_ptr(),
            to: std::ptr::null(),
            flags: FLAGS,
            aborted: 0,
            mappings: std::ptr::null_mut(),
            title: std::ptr::null(),
        };
        // SAFETY: `from` is double-NUL-terminated and outlives the call; the rest is null or plain
        match unsafe { SHFileOperationW(&mut op) } {
            0 if op.aborted == 0 => Ok(()),
            0 => Err(io::Error::new(io::ErrorKind::Interrupted, "cancelado")),
            code => Err(io::Error::other(format!("a Lixeira recusou (código {code:#x})"))),
        }
    }
    #[cfg(target_os = "macos")]
    {
        use objc2_foundation::{NSFileManager, NSString, NSURL};
        let url = NSURL::fileURLWithPath(&NSString::from_str(&path.to_string_lossy()));
        NSFileManager::defaultManager().trashItemAtURL_resultingItemURL_error(&url, None).map_err(|e| io::Error::other(e.localizedDescription().to_string()))
    }
    #[cfg(not(any(windows, target_os = "macos")))]
    {
        freedesktop_trash(path)
    }
}

/// The freedesktop.org trash in the home folder: the item moves to `Trash/files`, with a
/// `.trashinfo` beside it in `Trash/info` saying where it came from. Something on another disk
/// can't be moved there and is refused.
// ponytail: only the home trash (no `$topdir/.Trash-$uid` per disk), and the date is UTC where the
// spec asks for local time; add both if a desktop ever misplaces what it restores
#[cfg(not(any(windows, target_os = "macos")))]
fn freedesktop_trash(path: &Path) -> io::Result<()> {
    use std::io::Write;
    let var = |name: &str| std::env::var_os(name).filter(|v| !v.is_empty()).map(std::path::PathBuf::from);
    let data = var("XDG_DATA_HOME").or_else(|| var("HOME").map(|h| h.join(".local/share"))).ok_or_else(|| io::Error::other("sem pasta pessoal"))?;
    let (files, info) = (data.join("Trash/files"), data.join("Trash/info"));
    std::fs::create_dir_all(&files)?;
    std::fs::create_dir_all(&info)?;
    let path = std::path::absolute(path)?;
    let name = path.file_name().ok_or_else(|| io::Error::other("sem nome"))?.to_string_lossy().into_owned();
    // the first free name: a.txt, a.txt.2, a.txt.3…; the info file is claimed first, as the spec says
    let (kept, mut file) = (1..)
        .map(|n| if n == 1 { name.clone() } else { format!("{name}.{n}") })
        .find_map(|n| std::fs::File::create_new(info.join(format!("{n}.trashinfo"))).ok().map(|f| (n, f)))
        .expect("an endless range");
    let encoded: String = path
        .to_string_lossy()
        .bytes()
        .map(|b| if b.is_ascii_alphanumeric() || b"/-_.~".contains(&b) { (b as char).to_string() } else { format!("%{b:02X}") })
        .collect();
    let secs = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap_or_default().as_secs() as i64;
    let (days, rest) = (secs.div_euclid(86_400), secs.rem_euclid(86_400));
    // days since 1970 to a calendar date (Howard Hinnant's civil_from_days)
    let z = days + 719_468;
    let (era, doe) = (z.div_euclid(146_097), z.rem_euclid(146_097));
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let (day, month) = (doy - (153 * mp + 2) / 5 + 1, if mp < 10 { mp + 3 } else { mp - 9 });
    let year = yoe + era * 400 + i64::from(month <= 2);
    let date = format!("{year:04}-{month:02}-{day:02}T{:02}:{:02}:{:02}", rest / 3600, rest / 60 % 60, rest % 60);
    let written = write!(file, "[Trash Info]\nPath={encoded}\nDeletionDate={date}\n");
    let moved = written.and_then(|_| std::fs::rename(&path, files.join(&kept)));
    if moved.is_err() {
        let _ = std::fs::remove_file(info.join(format!("{kept}.trashinfo")));
    }
    moved
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names_follow_each_systems_rules() {
        assert_eq!(name_problem("main.rs", "windows"), None);
        assert_eq!(name_problem("a:b", "macos"), Some("Um nome não pode conter \\ / : < > \" | ? *"));
        assert_eq!(name_problem("a:b", "linux"), None);
        assert_eq!(name_problem("a/b", "linux"), Some("Um nome não pode conter /"));
        assert_eq!(name_problem("..", "linux"), Some("Um nome não pode ter só pontos."));
        for device in ["con", "NUL.txt", "com1", "Lpt9.log"] {
            assert_eq!(name_problem(device, "windows"), Some("O Windows reserva esse nome para um dispositivo."), "{device}");
            assert_eq!(name_problem(device, "linux"), None);
        }
        assert_eq!(name_problem("com10", "windows"), None);
        assert_eq!(name_problem("console.txt", "windows"), None);
    }
}
