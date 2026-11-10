use std::{
    collections::{HashMap, HashSet},
    sync::{
        Arc, Mutex, RwLock,
        atomic::{AtomicBool, Ordering},
    },
};

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
#[derive(Clone, Default)]
pub(crate) struct Monitor {
    input: Arc<Mutex<Vec<(String, u32)>>>,
    output: Arc<RwLock<Snapshot>>,
    started: Arc<AtomicBool>,
    /// When it was last asked for (seconds since 1970): unasked for a while, the thread goes.
    asked: Arc<std::sync::atomic::AtomicU64>,
}

fn now_secs() -> u64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_secs())
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

impl Monitor {
    pub(crate) fn new() -> Self {
        Self::default()
    }
    pub(crate) fn update(&self, terminals: Vec<(String, u32)>) {
        *self.input.lock().unwrap_or_else(|e| e.into_inner()) = terminals;
        self.asked.store(now_secs(), Ordering::Relaxed);
        if self.started.compare_exchange(false, true, Ordering::Relaxed, Ordering::Relaxed).is_ok() {
            let input = Arc::downgrade(&self.input);
            let output = Arc::downgrade(&self.output);
            let (started, asked) = (self.started.clone(), self.asked.clone());
            std::thread::spawn(move || {
                loop {
                    let (Some(input), Some(output)) = (input.upgrade(), output.upgrade()) else { break };
                    // nobody looks (the tab closed, no agent asks): the measuring stops until the next ask
                    if now_secs().saturating_sub(asked.load(Ordering::Relaxed)) > 10 {
                        started.store(false, Ordering::Relaxed);
                        break;
                    }
                    let terminals = input.lock().unwrap_or_else(|e| e.into_inner()).clone();
                    let parents = crate::transcript::process::parents();
                    // only the shells and what comes from them: not every process of the system
                    let roots: HashSet<u32> = terminals.iter().map(|(_, pid)| *pid).collect();
                    let mut pids = parents.keys().copied().filter(|pid| descends(&parents, *pid, &roots)).collect::<Vec<_>>();
                    pids.extend(roots.iter().copied());
                    pids.sort_unstable();
                    pids.dedup();
                    let sizes = pids.into_iter().filter_map(|pid| process_memory(pid).map(|bytes| (pid, bytes))).collect::<HashMap<_, _>>();
                    let rows = summarize(&parents, &sizes, &terminals);
                    let app_bytes = process_memory(std::process::id());
                    *output.write().unwrap_or_else(|e| e.into_inner()) = Snapshot { app_bytes, terminals: rows };
                    std::thread::sleep(std::time::Duration::from_secs(2));
                }
            });
        }
    }
    pub(crate) fn snapshot(&self) -> Snapshot {
        self.output.read().unwrap_or_else(|e| e.into_inner()).clone()
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
fn process_memory(pid: u32) -> Option<u64> {
    #[link(name = "psapi")]
    unsafe extern "system" {
        fn OpenProcess(access: u32, inherit: i32, pid: u32) -> isize;
        fn GetProcessMemoryInfo(process: isize, counters: *mut Counters, size: u32) -> i32;
        fn CloseHandle(handle: isize) -> i32;
    }
    #[repr(C)]
    struct Counters {
        cb: u32,
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
        let h = OpenProcess(QUERY | VM_READ, 0, pid);
        if h == 0 {
            return None;
        }
        let mut c = std::mem::zeroed::<Counters>();
        c.cb = std::mem::size_of::<Counters>() as u32;
        let ok = GetProcessMemoryInfo(h, &mut c, c.cb) != 0;
        CloseHandle(h);
        ok.then_some(c.working_set as u64)
    }
}
#[cfg(target_os = "linux")]
fn process_memory(pid: u32) -> Option<u64> {
    let s = std::fs::read_to_string(format!("/proc/{pid}/statm")).ok()?;
    let pages = s.split_whitespace().nth(1)?.parse::<u64>().ok()?;
    Some(pages * 4096)
}
#[cfg(target_os = "macos")]
fn process_memory(_pid: u32) -> Option<u64> {
    None
} // ponytail: add a native resident-size query when needed

pub(crate) fn format(snapshot: &Snapshot) -> String {
    let total: u64 = snapshot.terminals.iter().map(|t| t.bytes).sum();
    let mut lines = vec![format!("MagoAgent: {}", snapshot.app_bytes.map(size).unwrap_or_else(|| "indisponível".into()))];
    lines.extend(snapshot.terminals.iter().map(|t| format!("{}: {} em {} processos", t.name, size(t.bytes), t.processes)));
    lines.push(format!("Agentes: {} em {} terminais", size(total), snapshot.terminals.len()));
    lines.join("\n")
}
pub(crate) fn size(bytes: u64) -> String {
    format!("{:.1} MiB", bytes as f64 / 1048576.0)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn sums_descendants_and_two_terminals_once() {
        let p = HashMap::from([(2, 1), (3, 2), (5, 4)]);
        let m = HashMap::from([(1, 10), (2, 20), (3, 30), (4, 40), (5, 50)]);
        let rows = summarize(&p, &m, &[("A".into(), 1), ("B".into(), 4)]);
        assert_eq!((rows[0].bytes, rows[0].processes), (60, 3));
        assert_eq!((rows[1].bytes, rows[1].processes), (90, 2));
    }
    #[test]
    fn skips_unknown_pid_and_handles_cycles() {
        let p = HashMap::from([(2, 1), (3, 2), (1, 3)]);
        let m = HashMap::from([(2, 7), (3, 11)]);
        let rows = summarize(&p, &m, &[("A".into(), 1), ("missing".into(), 99)]);
        assert_eq!((rows[0].bytes, rows[0].processes), (18, 2));
        assert_eq!(rows[1].processes, 0);
    }
}
