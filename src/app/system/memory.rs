//! The memory of the app and of each terminal's processes, for the MCP's `memory`: measured when it is asked, on the
//! action's own thread (`act_later`), never on a timer the whole session long.

use crate::prelude::*;
use std::collections::{HashMap, HashSet};

#[derive(Clone, Debug, Default)]
pub(crate) struct TerminalMemory {
    pub name: String,
    pub bytes: u64,
    pub processes: usize,
}
#[derive(Clone, Debug, Default)]
pub(crate) struct Snapshot {
    pub app_bytes: Option<u64>,
    pub terminals: Vec<TerminalMemory>,
}

/// Whether `pid` is one of `roots` or comes from one (its parent, its parent's parent…).
fn descends(parents: &HashMap<u32, u32>, mut pid: u32, roots: &HashSet<u32>) -> bool {
    for _ in 0..64 {
        if roots.contains(&pid) {
            return true;
        }
        match parents.get(&pid) {
            Some(&up) if up != 0 && up != pid => pid = up,
            _ => return false,
        }
    }
    false
}

/// The app's memory and each terminal's (its name, its shell's pid), read now: the process table and the size of the
/// shells and of what comes from them, not of every process of the system. It reads `/proc` (Linux) or runs `ps`
/// (macOS): never on the interface's thread.
pub(crate) fn measure(terminals: &[(String, u32)]) -> Snapshot {
    let parents = crate::transcript::process::parents();
    let roots: HashSet<u32> = terminals.iter().map(|(_, pid)| *pid).collect();
    let mut pids = parents.keys().copied().filter(|pid| descends(&parents, *pid, &roots)).collect::<Vec<_>>();
    pids.extend(roots.iter().copied());
    pids.sort_unstable();
    pids.dedup();
    let sizes = pids.into_iter().filter_map(|pid| process_memory(pid).map(|bytes| (pid, bytes))).collect::<HashMap<_, _>>();
    Snapshot { app_bytes: process_memory(std::process::id()), terminals: summarize(&parents, &sizes, terminals) }
}

impl DrawingApp {
    /// The terminals whose shell runs, by name, with the shell's pid: what `measure` is asked about.
    pub(crate) fn memory_terminals(&self) -> Vec<(String, u32)> {
        self.terminals
            .iter()
            .filter_map(|(id, session)| {
                let session = session.as_ref().ok()?;
                let spec = self.scene.elements.iter().find(|element| element.id == *id).and_then(|element| element.terminal.as_ref());
                let name = spec.or(self.away.get(id).map(|(_, spec)| spec)).map_or_else(|| id.clone(), |spec| spec.name.clone());
                Some((name, session.shell_pid))
            })
            .collect()
    }

    /// The MCP's `memory`, measured on the action's thread: the app goes on drawing meanwhile.
    pub(crate) fn memory_job(&self) -> Box<dyn FnOnce() -> Result<String, String> + Send> {
        let terminals = self.memory_terminals();
        Box::new(move || Ok(format(&measure(&terminals))))
    }
}

pub(crate) fn summarize(parents: &HashMap<u32, u32>, sizes: &HashMap<u32, u64>, terminals: &[(String, u32)]) -> Vec<TerminalMemory> {
    let mut claimed = HashSet::new();
    terminals
        .iter()
        .map(|(name, root)| {
            let mut todo = vec![*root];
            let mut bytes = 0;
            let mut processes = 0;
            while let Some(pid) = todo.pop() {
                if !claimed.insert(pid) {
                    continue;
                }
                if let Some(size) = sizes.get(&pid) {
                    bytes += size;
                    processes += 1;
                }
                todo.extend(parents.iter().filter_map(|(&child, &parent)| (parent == pid && child != pid).then_some(child)));
            }
            TerminalMemory { name: name.clone(), bytes, processes }
        })
        .collect()
}

#[cfg(windows)]
pub(crate) fn process_memory(pid: u32) -> Option<u64> {
    #[link(name = "psapi")]
    unsafe extern "system" {
        fn OpenProcess(access: u32, inherit: i32, pid: u32) -> isize;
        fn GetProcessMemoryInfo(process: isize, counters: *mut Counters, size: u32) -> i32;
        fn CloseHandle(handle: isize) -> i32;
    }
    #[repr(C)]
    struct Counters {
        size_in_bytes: u32,
        page_faults: u32,
        peak_working_set: usize,
        working_set: usize,
        quota_peak_paged: usize,
        quota_paged: usize,
        quota_peak_nonpaged: usize,
        quota_nonpaged: usize,
        pagefile: usize,
        peak_pagefile: usize,
    }
    const QUERY: u32 = 0x0400;
    const VM_READ: u32 = 0x0010;
    unsafe {
        let handle = OpenProcess(QUERY | VM_READ, 0, pid);
        if handle == 0 {
            return None;
        }
        let mut counters = std::mem::zeroed::<Counters>();
        counters.size_in_bytes = std::mem::size_of::<Counters>() as u32;
        let ok = GetProcessMemoryInfo(handle, &mut counters, counters.size_in_bytes) != 0;
        CloseHandle(handle);
        ok.then_some(counters.working_set as u64)
    }
}
#[cfg(target_os = "linux")]
pub(crate) fn process_memory(pid: u32) -> Option<u64> {
    let statm = std::fs::read_to_string(format!("/proc/{pid}/statm")).ok()?;
    let pages = statm.split_whitespace().nth(1)?.parse::<u64>().ok()?;
    Some(pages * 4096)
}
#[cfg(target_os = "macos")]
pub(crate) fn process_memory(pid: u32) -> Option<u64> {
    // ponytail: one `ps` per process, every 2 s; `proc_pidinfo` if the terminals' trees grow
    let out = crate::editor::command("ps").args(["-o", "rss=", "-p", &pid.to_string()]).output().ok()?;
    rss_bytes(&String::from_utf8_lossy(&out.stdout))
}
/// `ps -o rss=` prints the resident size in KiB.
#[cfg(target_os = "macos")]
fn rss_bytes(output: &str) -> Option<u64> {
    output.trim().parse::<u64>().ok().map(|kib| kib * 1024)
}

pub(crate) fn format(snapshot: &Snapshot) -> String {
    let total: u64 = snapshot.terminals.iter().map(|terminal| terminal.bytes).sum();
    let mut lines = vec![format!("MagoAgent: {}", snapshot.app_bytes.map(size).unwrap_or_else(|| "indisponível".into()))];
    lines.extend(snapshot.terminals.iter().map(|terminal| format!("{}: {} em {} processos", terminal.name, size(terminal.bytes), terminal.processes)));
    lines.push(format!("Agentes: {} em {} terminais", size(total), snapshot.terminals.len()));
    lines.join("\n")
}
pub(crate) fn size(bytes: u64) -> String {
    format!("{:.1} MiB", bytes as f64 / 1048576.0).replace('.', ",")
}

#[cfg(test)]
mod tests {
    use super::*;
    #[cfg(target_os = "macos")]
    #[test]
    fn reads_ps_rss_in_kib() {
        assert_eq!(rss_bytes("  2048\n"), Some(2 * 1024 * 1024));
        assert_eq!(rss_bytes(""), None);
        assert!(process_memory(std::process::id()).is_some_and(|bytes| bytes > 0));
    }
    #[test]
    fn sums_descendants_and_two_terminals_once() {
        let parents = HashMap::from([(2, 1), (3, 2), (5, 4)]);
        let sizes = HashMap::from([(1, 10), (2, 20), (3, 30), (4, 40), (5, 50)]);
        let rows = summarize(&parents, &sizes, &[("A".into(), 1), ("B".into(), 4)]);
        assert_eq!((rows[0].bytes, rows[0].processes), (60, 3));
        assert_eq!((rows[1].bytes, rows[1].processes), (90, 2));
    }
    #[test]
    fn skips_unknown_pid_and_handles_cycles() {
        let parents = HashMap::from([(2, 1), (3, 2), (1, 3)]);
        let sizes = HashMap::from([(2, 7), (3, 11)]);
        let rows = summarize(&parents, &sizes, &[("A".into(), 1), ("missing".into(), 99)]);
        assert_eq!((rows[0].bytes, rows[0].processes), (18, 2));
        assert_eq!(rows[1].processes, 0);
    }
}
