//! The project's excluded folders, as PhpStorm's "Mark Directory as > Excluded": one list per
//! project, used by Go to File, the text index, the symbol index and Find in Files. It is kept in
//! the app's data (never inside the project) as `ide_excluidas.json`, one list per root.

use super::*;

/// What a project starts with. A name matches at any depth; `/path` only from the project's root.
pub(crate) const DEFAULT: &[&str] = &[
    "node_modules",
    "vendor",
    "target",
    ".git",
    ".idea",
    "__pycache__",
    ".next",
    ".nuxt",
    ".cache",
    "/dist",
    "/build",
    "/storage",
    "/bootstrap/cache",
    "/public/build",
];

#[derive(Clone, Debug, Default)]
pub(crate) struct Excluded {
    root: PathBuf,
    dirs: Vec<String>,
}

fn store() -> Option<PathBuf> {
    // tests never touch the app's real data
    if cfg!(test) { None } else { crate::file::data_dir().map(|d| d.join("ide_excluidas.json")) }
}

fn matches(rule: &str, path: &str, name: &str) -> bool {
    rule.strip_prefix('/').map_or(rule == name, |r| r == path)
}

type Lists = serde_json::Map<String, serde_json::Value>;

impl Excluded {
    pub(crate) fn load(root: &Path) -> Self {
        Self::load_from(store().as_deref(), root)
    }

    /// The saved list of `root` in `file`, or the defaults plus the folders its `.gitignore` names.
    pub(crate) fn load_from(file: Option<&Path>, root: &Path) -> Self {
        let saved = file
            .and_then(|f| std::fs::read_to_string(f).ok())
            .and_then(|t| serde_json::from_str::<Lists>(&t).ok())
            .and_then(|m| m.get(root.to_string_lossy().as_ref()).cloned())
            .and_then(|v| serde_json::from_value::<Vec<String>>(v).ok());
        let dirs = saved.unwrap_or_else(|| {
            let mut dirs: Vec<String> = DEFAULT.iter().map(|d| d.to_string()).collect();
            for d in gitignore_dirs(root) {
                if !dirs.contains(&d) {
                    dirs.push(d);
                }
            }
            dirs
        });
        Excluded { root: root.to_path_buf(), dirs }
    }

    /// Whether `p`, or a folder above it inside the project, is excluded.
    pub(crate) fn contains(&self, p: &Path) -> bool {
        let Ok(rel) = p.strip_prefix(&self.root) else { return false };
        let mut path = String::new();
        for c in rel.components() {
            let name = c.as_os_str().to_string_lossy();
            if !path.is_empty() {
                path.push('/');
            }
            path.push_str(&name);
            if self.dirs.iter().any(|d| matches(d, &path, &name)) {
                return true;
            }
        }
        false
    }
}

/// Folders named by the root `.gitignore`: plain names or paths (no wildcards, no `!`) that are
/// directories at the root. ponytail: nested .gitignore files and patterns are not read.
fn gitignore_dirs(root: &Path) -> Vec<String> {
    let text = std::fs::read_to_string(root.join(".gitignore")).unwrap_or_default();
    text.lines()
        .map(str::trim)
        .filter(|l| !l.is_empty() && !l.starts_with(['#', '!']) && !l.contains(['*', '?', '[', '\\']))
        .filter_map(|l| {
            let anchored = l.starts_with('/') || l.trim_end_matches('/').contains('/');
            let path = l.trim_matches('/');
            (!path.is_empty() && root.join(path).is_dir()).then(|| if anchored { format!("/{path}") } else { path.to_string() })
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn project(tag: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("mago-excl-{tag}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        for d in ["node_modules/pkg", "vendor/lib", "src/vendor", "docs", "out/gen"] {
            std::fs::create_dir_all(dir.join(d)).unwrap();
        }
        dir
    }

    #[test]
    fn defaults_and_gitignore() {
        let dir = project("a");
        std::fs::write(dir.join(".gitignore"), "# c\nout/\n*.log\n.env\ndocs/nope\n!keep\n").unwrap();
        let ex = Excluded::load_from(None, &dir);
        assert!(ex.contains(&dir.join("node_modules/pkg/index.js")) && ex.contains(&dir.join("src/vendor/x.php")));
        assert!(ex.contains(&dir.join("out/gen/a.txt")), "a gitignore folder");
        assert!(!ex.contains(&dir.join("docs/a.md")) && !ex.contains(&dir.join("src/build/a.rs")));
        assert!(ex.contains(&dir.join("build/a.rs")), "/build is the root's only");
        let _ = std::fs::remove_dir_all(&dir);
    }
}
