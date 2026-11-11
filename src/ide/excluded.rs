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

    pub(crate) fn save(&self) {
        self.save_to(store().as_deref());
    }

    pub(crate) fn save_to(&self, file: Option<&Path>) {
        let Some(file) = file else { return };
        let mut all: Lists = std::fs::read_to_string(file).ok().and_then(|t| serde_json::from_str(&t).ok()).unwrap_or_default();
        all.insert(self.root.to_string_lossy().into_owned(), serde_json::json!(self.dirs));
        if let Ok(text) = serde_json::to_string_pretty(&all) {
            let _ = std::fs::write(file, text);
        }
    }

    pub(crate) fn list(&self) -> &[String] {
        &self.dirs
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

    /// Marks the folder `dir` excluded (`on`) or takes the exclusion off it; says what happened.
    pub(crate) fn set(&mut self, dir: &Path, on: bool) -> Result<String, String> {
        let rel = dir.strip_prefix(&self.root).ok().filter(|r| !r.as_os_str().is_empty()).ok_or("A pasta precisa estar dentro do projeto.")?;
        if rel.components().any(|c| matches!(c, std::path::Component::ParentDir)) || !dir.is_dir() {
            return Err("Escolha uma pasta do projeto.".into());
        }
        let path = rel.to_string_lossy().replace('\\', "/");
        let name = path.rsplit('/').next().unwrap_or(&path).to_string();
        let own = self.dirs.iter().position(|d| matches(d, &path, &name));
        match (on, own) {
            (true, Some(_)) => Ok(format!("{path} já está excluída.")),
            (true, None) if self.contains(dir) => Ok(format!("{path} já está dentro de uma pasta excluída.")),
            (true, None) => {
                self.dirs.push(format!("/{path}"));
                Ok(format!("{path} excluída: fora da busca e do índice."))
            }
            (false, Some(i)) => {
                self.dirs.remove(i);
                Ok(format!("{path} deixou de ser excluída."))
            }
            (false, None) if self.contains(dir) => Err(format!("{path} está dentro de uma pasta excluída; cancele a exclusão da pasta de cima.")),
            (false, None) => Ok(format!("{path} não estava excluída.")),
        }
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

impl Ide {
    /// The folder `path` (relative to the project, or a full one) excluded or not, as the tree's
    /// menu does it: the list is saved and the files and indexes are made again without it.
    pub(crate) fn set_excluded(&mut self, path: &Path, on: bool) -> Result<String, String> {
        let root = self.root.clone().ok_or("O IDE não tem pasta aberta.")?;
        let done = self.excluded.set(&root.join(path), on)?;
        self.excluded.save();
        self.reload_files(&root);
        self.status = done.clone();
        Ok(done)
    }

    /// The `ide_exclude` tool: `list`, `exclude` or `include` (cancel the exclusion of) a folder.
    pub(crate) fn agent_exclude(&mut self, action: &str, path: &str) -> Result<String, String> {
        match action {
            "list" | "" => Ok(format!("Pastas excluídas (fora da busca e do índice):\n{}", self.excluded.list().join("\n"))),
            "exclude" | "include" if path.trim().is_empty() => Err("diga a pasta".into()),
            "exclude" | "include" => self.set_excluded(Path::new(path.trim()), action == "exclude"),
            other => Err(format!("ação desconhecida: {other}")),
        }
    }
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
    fn defaults_gitignore_mark_and_persist() {
        let dir = project("a");
        std::fs::write(dir.join(".gitignore"), "# c\nout/\n*.log\n.env\ndocs/nope\n!keep\n").unwrap();
        let mut ex = Excluded::load_from(None, &dir);
        assert!(ex.contains(&dir.join("node_modules/pkg/index.js")) && ex.contains(&dir.join("src/vendor/x.php")));
        assert!(ex.contains(&dir.join("out/gen/a.txt")), "a gitignore folder");
        assert!(!ex.contains(&dir.join("docs/a.md")) && !ex.contains(&dir.join("src/build/a.rs")) && ex.list().len() == DEFAULT.len() + 1);
        assert!(ex.contains(&dir.join("build/a.rs")), "/build is the root's only");
        // mark, repeat, inside one, and cancel
        assert!(ex.set(&dir.join("docs"), true).unwrap().contains("excluída"));
        assert!(ex.contains(&dir.join("docs/a.md")));
        assert!(ex.set(&dir.join("docs"), true).unwrap().contains("já está"));
        assert!(ex.set(&dir.join("node_modules/pkg"), false).is_err(), "only the folder above can be cancelled");
        assert!(ex.set(&dir, true).is_err() && ex.set(&dir.join("nada"), true).is_err());
        // saved per root, apart from the project
        let file = std::env::temp_dir().join(format!("mago-excl-store-{}.json", std::process::id()));
        ex.save_to(Some(&file));
        let other = project("b");
        Excluded::load_from(None, &other).save_to(Some(&file));
        let mut back = Excluded::load_from(Some(&file), &dir);
        assert!(back.contains(&dir.join("docs/a.md")));
        assert!(!Excluded::load_from(Some(&file), &other).contains(&other.join("docs/a.md")));
        back.set(&dir.join("node_modules"), false).unwrap();
        back.save_to(Some(&file));
        assert!(!Excluded::load_from(Some(&file), &dir).contains(&dir.join("node_modules/pkg/a.js")));
        for d in [&dir, &other] {
            let _ = std::fs::remove_dir_all(d);
        }
        let _ = std::fs::remove_file(file);
    }
}
