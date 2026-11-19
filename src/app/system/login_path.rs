//! macOS: an app opened from the Finder or the Dock gets launchd's PATH (/usr/bin:/bin:/usr/sbin:/sbin),
//! without `~/.local/bin` (claude), nvm (node, codex) or Homebrew. The login shell's PATH is joined to it once at start.

/// Folders the agents' tools usually live in, for when the login shell does not answer.
const FALLBACK: [&str; 3] = ["~/.local/bin", "/opt/homebrew/bin", "/usr/local/bin"];

/// Several PATH lists as one, in order, each folder once.
fn join_paths(lists: &[&str]) -> String {
    let mut seen = std::collections::HashSet::new();
    let folders = lists.iter().flat_map(|list| std::env::split_paths(list)).filter(|p| !p.as_os_str().is_empty() && seen.insert(p.clone()));

    std::env::join_paths(folders).map(|joined| joined.to_string_lossy().into_owned()).unwrap_or_default()
}

/// The PATH of the person's login shell, None if it fails or takes more than `limit` (a shell that asks something).
fn shell_path(limit: std::time::Duration) -> Option<String> {
    let shell = std::env::var("SHELL").ok().filter(|s| !s.is_empty()).unwrap_or_else(|| "/bin/zsh".into());

    let mut child = crate::editor::command(shell)
        .args(["-lic", "printf '\\n%s' \"$PATH\""])
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::null())
        .spawn()
        .ok()?;

    let start = std::time::Instant::now();

    while child.try_wait().ok()?.is_none() {
        if start.elapsed() > limit {
            let _ = child.kill();
            let _ = child.wait();

            return None;
        }

        std::thread::sleep(std::time::Duration::from_millis(20));
    }

    let mut out = String::new();

    std::io::Read::read_to_string(&mut child.stdout.take()?, &mut out).ok()?;
    // the rc files may print something first: the PATH is the last line
    out.lines().last().map(str::to_string)
}

/// Joins the login shell's PATH to the process's. Call once, first thing in `main`, before any other thread.
pub(crate) fn fix() {
    let home = std::env::var("HOME").unwrap_or_default();
    let fallback = FALLBACK.map(|dir| dir.replacen('~', &home, 1)).join(":");
    let shell = shell_path(std::time::Duration::from_secs(2)).unwrap_or_default();
    let current = std::env::var("PATH").unwrap_or_default();

    // SAFETY: first thing in main, before any other thread starts
    unsafe { std::env::set_var("PATH", join_paths(&[&shell, &current, &fallback])) };
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn paths_join_without_repeating_folders() {
        assert_eq!(join_paths(&["/a:/b", "/b:/c::/a", ""]), "/a:/b:/c");
        assert_eq!(join_paths(&[]), "");
    }

    #[test]
    fn the_login_shell_answers_with_a_path() {
        let path = shell_path(std::time::Duration::from_secs(10)).expect("the login shell's PATH");

        assert!(path.contains("/usr/bin"), "{path}");
    }
}
