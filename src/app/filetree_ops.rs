//! What the person does to the files of a file tree (new file or folder, rename, trash, copy, cut,
//! paste) and the MCP's `file_tree` action, which does all of it by name.

use crate::app::chat_direct::folder;
use crate::app::filetree::*;
use crate::ide::fileops::{name_problem, trash};
use crate::prelude::*;
use std::path::Path;

/// A name for a copy of `name` in `dir` that nothing has: "a.txt", then "a cópia.txt", "a cópia 2.txt"…
fn free_name(dir: &Path, name: &str) -> PathBuf {
    let first = dir.join(name);
    if !first.exists() {
        return first;
    }
    let path = Path::new(name);
    let stem = path.file_stem().map_or(String::new(), |s| s.to_string_lossy().into_owned());
    let ext = path.extension().map_or(String::new(), |e| format!(".{}", e.to_string_lossy()));
    (1..).map(|n| dir.join(if n == 1 { format!("{stem} cópia{ext}") } else { format!("{stem} cópia {n}{ext}") })).find(|p| !p.exists()).unwrap()
}

fn copy_tree(from: &Path, to: &Path) -> std::io::Result<()> {
    if from.is_dir() {
        fs::create_dir(to)?;
        for e in fs::read_dir(from)? {
            let e = e?;
            copy_tree(&e.path(), &to.join(e.file_name()))?;
        }
        Ok(())
    } else {
        fs::copy(from, to).map(drop)
    }
}

/// A name that can be made: not empty, with nothing the system refuses in it.
fn check_name(name: &str) -> Result<&str, String> {
    let name = name.trim();
    if name.is_empty() {
        return Err("Nada foi criado.".into());
    }
    name_problem(name, std::env::consts::OS).map_or(Ok(name), |why| Err(why.into()))
}

impl DrawingApp {
    /// Makes a file or folder `name` in `dir`.
    pub(crate) fn ft_create(&mut self, id: &str, dir: &Path, name: &str, is_folder: bool) -> Result<PathBuf, String> {
        let path = dir.join(check_name(name)?);
        let made = if is_folder { fs::create_dir(&path) } else { fs::File::create_new(&path).map(drop) };
        made.map_err(|e| format!("Não foi possível criar isso: {e}"))?;
        self.ft_reveal(id, &path);
        Ok(path)
    }

    /// Renames `path` in its folder; another file of that name is never replaced.
    pub(crate) fn ft_rename(&mut self, id: &str, path: &Path, name: &str) -> Result<PathBuf, String> {
        let name = name.trim();
        if path.file_name() == Some(std::ffi::OsStr::new(name)) {
            return Err("Nada foi renomeado.".into());
        }
        let to = path.with_file_name(check_name(name).map_err(|_| "Nada foi renomeado.".to_string())?);
        // a change of case only is the same file on Windows and macOS
        if to.exists() && fs::canonicalize(&to).ok() != fs::canonicalize(path).ok() {
            return Err(format!("Já existe {name} nesta pasta."));
        }
        fs::rename(path, &to).map_err(|e| format!("Não foi possível renomear isso: {e}"))?;
        self.ft_reveal(id, &to);
        Ok(to)
    }

    /// Sends `paths` to the system's trash; nothing is deleted for good when the trash refuses.
    pub(crate) fn ft_trash(&mut self, id: &str, paths: &[PathBuf]) -> Result<usize, String> {
        for path in paths {
            trash(path).map_err(|e| format!("Nada foi excluído: {e}. O MagoAgent não apaga um arquivo de vez quando a lixeira o recusa."))?;
        }
        self.ft_refresh(id);
        Ok(paths.len())
    }

    /// Puts what was copied or cut into `dir`; a name taken gets "cópia" after it.
    // ponytail: copies on the UI thread, a big folder holds the frame; move it to a worker if that bites
    pub(crate) fn ft_paste(&mut self, id: &str, dir: &Path) -> Result<usize, String> {
        let (paths, cut) = self.filetrees.clipboard.clone().ok_or("Nada foi colado.")?;
        let mut done = 0;
        for from in paths {
            let Some(name) = from.file_name().map(|n| n.to_string_lossy().into_owned()) else { continue };
            if dir.starts_with(&from) {
                return Err("Não é possível colar uma pasta dentro dela mesma.".into());
            }
            if cut && from.parent() == Some(dir) {
                continue; // already there
            }
            let to = free_name(dir, &name);
            let result = if cut {
                fs::rename(&from, &to)
                    .or_else(|_| copy_tree(&from, &to).and_then(|_| if from.is_dir() { fs::remove_dir_all(&from) } else { fs::remove_file(&from) }))
            } else {
                copy_tree(&from, &to)
            };
            result.map_err(|e| format!("Não foi possível colar isso: {e}"))?;
            done += 1;
        }
        if cut {
            self.filetrees.clipboard = None;
        }
        self.ft_refresh(id);
        Ok(done)
    }

    /// Shows a path just made: its folders open and its row is the selected one.
    pub(crate) fn ft_reveal(&mut self, id: &str, path: &Path) {
        if let Some(view) = self.filetrees.views.get_mut(id) {
            let mut up = path.parent();
            while let Some(dir) = up {
                if view.listings.contains_key(dir) {
                    view.expanded.insert(dir.to_path_buf());
                }
                up = dir.parent();
            }
            // the new name has to be listed before it can be a row
            if let Some(dir) = path.parent() {
                view.read_now(dir);
            }
            view.selected = Some(path.to_path_buf());
            view.reveal = Some((path.to_path_buf(), 30));
            view.stale = true;
        }
    }

    /// The answer of a file operation: its words in the toast.
    fn ft_done(&mut self, result: Result<String, String>) {
        self.toast(result.unwrap_or_else(|e| e));
    }

    /// Runs the question the person was asked, once answered.
    pub(crate) fn ft_answer(&mut self, ask: TreeAsk, text: String) {
        match ask {
            TreeAsk::Name { tree, at, kind, .. } => {
                let result = match kind {
                    NameKind::NewFile => self.ft_create(&tree, &at, &text, false).map(|_| "Arquivo criado.".to_string()),
                    NameKind::NewFolder => self.ft_create(&tree, &at, &text, true).map(|_| "Pasta criada.".to_string()),
                    NameKind::Rename => self.ft_rename(&tree, &at, &text).map(|_| "Renomeado.".to_string()),
                };
                self.ft_done(result);
            }
            TreeAsk::Trash { tree, paths } => {
                let result = self
                    .ft_trash(&tree, &paths)
                    .map(|n| if n == 1 { "1 item movido para a lixeira.".into() } else { format!("{n} itens movidos para a lixeira.") });
                self.ft_done(result);
            }
        }
    }

    /// The MCP's `file_tree`: what the person does with a file tree node, by name.
    pub(crate) fn act_file_tree(&mut self, args: &Value) -> Result<String, String> {
        let text = |key: &str| args[key].as_str().unwrap_or_default().trim().to_string();
        let action = text("action");
        if action == "create" {
            return self.act_tree_create(&text("path"));
        }
        let id = self.tree_named(&text("tree"))?;
        self.filetrees.views.entry(id.clone()).or_insert_with(TreeView::new);
        let spec = self.ft_spec(&id).cloned().unwrap_or_default();
        // a path as given, or inside the folder shown
        let at = |key: &str| {
            let p = text(key);
            let path = folder(&p).unwrap_or_default();
            if p.is_empty() {
                spec.shown()
            } else if path.is_absolute() {
                path
            } else {
                spec.shown().join(p)
            }
        };
        let inside = |p: &Path| if p.starts_with(spec.root_path()) { Ok(()) } else { Err(format!("{} está fora da árvore.", p.display())) };
        match action.as_str() {
            "list" => Ok(self.describe_tree(&id)),
            "expand" | "collapse" => {
                let path = at("path");
                inside(&path)?;
                let open = self.filetrees.views[&id].expanded.contains(&path);
                if open == (action == "collapse") {
                    self.ft_toggle(&id, &path);
                }
                Ok(self.describe_tree(&id))
            }
            "collapse_all" => {
                self.ft_collapse_all(&id);
                Ok("Tudo recolhido.".into())
            }
            "navigate" => {
                let path = at("path");
                inside(&path)?;
                self.ft_navigate(&id, path);
                Ok(self.describe_tree(&id))
            }
            "back" => {
                self.ft_back(&id);
                Ok(self.describe_tree(&id))
            }
            "forward" => {
                self.ft_forward(&id);
                Ok(self.describe_tree(&id))
            }
            "refresh" => {
                self.ft_refresh(&id);
                Ok(self.describe_tree(&id))
            }
            "open" => {
                let path = at("path");
                inside(&path)?;
                self.ft_activate(&id, path.clone());
                Ok(format!("{} aberto.", path.display()))
            }
            "root" => {
                let root = text("path");
                self.check_root_for_act(&root)?;
                self.snapshot();
                self.ft_set_root(&id, root);
                Ok(self.describe_tree(&id))
            }
            "hidden" => {
                let show = args["show"].as_bool().unwrap_or(spec.hide_hidden);
                self.snapshot();
                if let Some(s) = self.ft_spec_mut(&id) {
                    s.hide_hidden = !show;
                }
                self.ft_refresh(&id);
                Ok(if show { "Arquivos ocultos à vista.".into() } else { "Arquivos ocultos escondidos.".to_string() })
            }
            "sort" => {
                let by = SortBy::ALL.into_iter().find(|s| s.key() == text("by")).ok_or("ordene por name, kind, modified, created ou size")?;
                self.snapshot();
                if let Some(s) = self.ft_spec_mut(&id) {
                    (s.sort, s.reversed) = (by, args["reverse"].as_bool().unwrap_or(false));
                }
                self.ft_refresh(&id);
                Ok(self.describe_tree(&id))
            }
            "new_file" | "new_folder" => {
                let dir = at("path");
                inside(&dir)?;
                let made = self.ft_create(&id, &dir, &text("name"), action == "new_folder")?;
                Ok(format!("{} criado.", made.display()))
            }
            "rename" => {
                let path = at("path");
                inside(&path)?;
                let to = self.ft_rename(&id, &path, &text("name"))?;
                Ok(format!("Renomeado para {}.", to.display()))
            }
            "delete" => {
                let path = at("path");
                inside(&path)?;
                self.ft_trash(&id, std::slice::from_ref(&path))?;
                Ok(format!("{} foi para a lixeira.", path.display()))
            }
            "copy" | "cut" => {
                let path = at("path");
                inside(&path)?;
                self.filetrees.clipboard = Some((vec![path], action == "cut"));
                Ok(if action == "cut" { "Pronto para mover." } else { "Copiado." }.into())
            }
            "paste" => {
                let dir = at("path");
                inside(&dir)?;
                let n = self.ft_paste(&id, &dir)?;
                Ok(format!("{n} item(ns) colado(s)."))
            }
            "search" => {
                if let Some(view) = self.filetrees.views.get_mut(&id) {
                    view.filter = text("query");
                    view.scroll = 0.0;
                    view.read_index_now(&spec.shown());
                }
                Ok(self.describe_tree(&id))
            }
            "editor" => {
                // the pane shows (`show` true), hides (false) or flips
                self.ft_show_editor(&id, args["show"].as_bool().unwrap_or(!spec.editor));
                Ok(self.describe_tree(&id))
            }
            "save" => {
                let ed = self.filetrees.editors.get_mut(&id).ok_or("Nenhum arquivo aberto no editor desta árvore.")?;
                ed.save()?;
                Ok("Arquivo salvo.".into())
            }
            "close_file" => {
                let path = at("path");
                let ed = self.filetrees.editors.get_mut(&id).ok_or("Nenhum arquivo aberto no editor desta árvore.")?;
                ed.close(&path)?;
                Ok(self.describe_tree(&id))
            }
            "close" => {
                let index = self.scene.elements.iter().position(|e| e.id == id).ok_or("a árvore sumiu")?;
                self.selected = vec![index];
                self.delete_selected();
                Ok("Árvore de arquivos fechada (dá para desfazer).".into())
            }
            other => Err(format!("ação desconhecida: {other}")),
        }
    }

    fn check_root_for_act(&self, root: &str) -> Result<(), String> {
        match folder(root) {
            Some(p) if p.is_dir() => Ok(()),
            _ => Err(format!("A pasta {root} não existe.")),
        }
    }

    /// A tree on `path`, in the middle of the view, and its rows.
    fn act_tree_create(&mut self, path: &str) -> Result<String, String> {
        self.check_root_for_act(path)?;
        let at = self.free_spot(Rect::from_center_size(self.to_world(self.canvas_rect.center()), FILETREE_SIZE));
        let mut node = self.new_element(ElementKind::Rectangle { x: at.min.x, y: at.min.y, width: at.width(), height: at.height() });
        node.style = node_style(node.style);
        node.filetree = Some(FileTreeSpec { root: path.trim().to_string(), ..Default::default() });
        let id = node.id.clone();
        self.snapshot();
        self.scene.elements.push(node);
        self.filetrees.views.insert(id.clone(), TreeView::new());
        Ok(self.describe_tree(&id))
    }

    /// The tree's header and rows as text, for an agent: the folder shown, the branch and what is open.
    pub(crate) fn describe_tree(&mut self, id: &str) -> String {
        let Some(spec) = self.ft_spec(id).cloned() else { return String::new() };
        // the editor pane: its tabs (> forward, * unsaved, ~ preview)
        let editor = match self.filetrees.editors.get(id).filter(|_| spec.editor) {
            Some(ed) if !ed.tabs.is_empty() => {
                let tab = |t: &crate::app::filetree_pane::PaneTab| {
                    let name = t.path.file_name().map_or(String::new(), |n| n.to_string_lossy().into_owned());
                    let forward = if ed.active.as_ref() == Some(&t.path) { ">" } else { "" };
                    let mark = if t.dirty() {
                        "*"
                    } else if ed.preview.as_ref() == Some(&t.path) {
                        "~"
                    } else {
                        ""
                    };
                    format!("{forward}{mark}{name}")
                };
                format!("Editor: aberto, abas {}\n", ed.tabs.iter().map(tab).collect::<Vec<_>>().join(", "))
            }
            _ if spec.editor => "Editor: aberto, sem arquivos\n".to_string(),
            _ => String::new(),
        };
        let Some(view) = self.filetrees.views.get_mut(id) else { return String::new() };
        let shown = spec.shown();
        // an agent asks for what is on the disk now
        for dir in std::iter::once(shown.clone()).chain(view.expanded.iter().cloned().filter(|d| d.starts_with(&shown)).collect::<Vec<_>>()) {
            view.read_now(&dir);
        }
        view.git = TreeGit::read(&spec.root_path());
        view.rebuild(&spec);
        let mut out = format!("Árvore de arquivos {} ({})", spec.name(), shown.display());
        if view.git.repo {
            out += &format!(", ramo {}, {} alterado(s)", view.git.branch, view.git.changes);
        }
        out.push('\n');
        out += &editor;
        if !view.filter.trim().is_empty() {
            out +=
                &format!("Busca \"{}\": {}; a árvore tem {} linhas\n", view.filter.trim(), crate::app::filetree_ui::hits_note(view.rows.len()), view.tree_rows);
        }
        if let Some(note) = view.empty_note(&spec) {
            out += &format!("{note}\n");
        }
        for row in &view.rows {
            let mark = view.git.of(&row.path, row.dir).map_or(' ', |(s, _)| crate::app::filetree_ui::badge(s));
            let fold = if !row.dir {
                " "
            } else if row.open {
                "v"
            } else {
                ">"
            };
            let hint = if row.hint.is_empty() { String::new() } else { format!(" ({})", row.hint) };
            out += &format!("{}{fold} {}{hint} {mark}\n", "  ".repeat(row.depth), row.name);
        }
        if view.dropped > 0 {
            out += &format!("Exibindo um número limitado de itens. Mais {} não estão listados.\n", view.dropped);
        }
        out
    }
}
