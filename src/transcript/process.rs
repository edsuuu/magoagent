//! The processes behind Claude Code's session files: whether one still runs, and whose child it is.

use std::collections::HashMap;

/// Whether process `pid` still runs: a claude that crashed leaves its file behind, saying busy.
/// 0 (a file without one) counts as running, and so does a process that isn't ours to look at.
// ponytail: a dead claude's pid reused by another program reads as running; the process's start
// time against `startedAt` would tell them apart
pub(super) fn alive(pid: u32) -> bool {
    if pid == 0 {
        return true;
    }
    #[cfg(windows)]
    {
        unsafe extern "system" {
            fn OpenProcess(access: u32, inherit: i32, pid: u32) -> isize;
            fn GetExitCodeProcess(process: isize, code: *mut u32) -> i32;
            fn CloseHandle(handle: isize) -> i32;
        }
        const PROCESS_QUERY_LIMITED_INFORMATION: u32 = 0x1000;
        const STILL_ACTIVE: u32 = 259;
        const ERROR_ACCESS_DENIED: i32 = 5;
        // SAFETY: the handle is checked before use and closed once
        unsafe {
            let process = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, 0, pid);
            if process == 0 {
                return std::io::Error::last_os_error().raw_os_error() == Some(ERROR_ACCESS_DENIED);
            }
            let mut code = 0;
            let read = GetExitCodeProcess(process, &mut code) != 0;
            CloseHandle(process);
            !read || code == STILL_ACTIVE
        }
    }
    #[cfg(unix)]
    {
        unsafe extern "C" {
            fn kill(pid: i32, signal: i32) -> i32;
        }
        const EPERM: i32 = 1;
        // SAFETY: signal 0 only asks whether the process exists
        let found = unsafe { kill(pid as i32, 0) } == 0;
        found || std::io::Error::last_os_error().raw_os_error() == Some(EPERM)
    }
}

/// Each running process's parent (pid → parent pid).
pub(crate) fn parents() -> HashMap<u32, u32> {
    let mut map = HashMap::new();
    #[cfg(windows)]
    {
        #[repr(C)]
        struct Entry {
            size: u32,
            usage: u32,
            pid: u32,
            heap: usize,
            module: u32,
            threads: u32,
            parent: u32,
            priority: i32,
            flags: u32,
            exe: [u16; 260],
        }
        unsafe extern "system" {
            fn CreateToolhelp32Snapshot(flags: u32, pid: u32) -> isize;
            fn Process32FirstW(snapshot: isize, entry: *mut Entry) -> i32;
            fn Process32NextW(snapshot: isize, entry: *mut Entry) -> i32;
            fn CloseHandle(handle: isize) -> i32;
        }
        const TH32CS_SNAPPROCESS: u32 = 2;
        // SAFETY: the entry carries its size as the API asks; the snapshot is checked and closed once
        unsafe {
            let snapshot = CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0);
            if snapshot == -1 {
                return map;
            }
            let mut entry: Entry = std::mem::zeroed();
            entry.size = std::mem::size_of::<Entry>() as u32;
            let mut more = Process32FirstW(snapshot, &mut entry) != 0;
            while more {
                map.insert(entry.pid, entry.parent);
                more = Process32NextW(snapshot, &mut entry) != 0;
            }
            CloseHandle(snapshot);
        }
    }
    #[cfg(target_os = "linux")]
    for entry in std::fs::read_dir("/proc").into_iter().flatten().flatten() {
        let Ok(pid) = entry.file_name().to_string_lossy().parse::<u32>() else { continue };
        // after the name in parentheses: the state, then the parent
        let stat = std::fs::read_to_string(entry.path().join("stat")).unwrap_or_default();
        if let Some(parent) = stat.rsplit(')').next().and_then(|rest| rest.split_whitespace().nth(1)).and_then(|p| p.parse().ok()) {
            map.insert(pid, parent);
        }
    }
    #[cfg(target_os = "macos")]
    if let Ok(out) = std::process::Command::new("ps").args(["-A", "-o", "pid=,ppid="]).output() {
        for line in String::from_utf8_lossy(&out.stdout).lines() {
            let mut n = line.split_whitespace().filter_map(|p| p.parse::<u32>().ok());
            if let (Some(pid), Some(parent)) = (n.next(), n.next()) {
                map.insert(pid, parent);
            }
        }
    }
    map
}
