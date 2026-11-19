//! What the SwiftUI file tree asks of the app: `ui.filetree.<verb>` with the tree's `id` and the verb's own arguments. Each verb calls what
//! the egui tree calls (ops.rs, pane/mod.rs, diff.rs...), so nothing about files, git or buffers is done twice.

use super::doc::byte_of;
use crate::app::filetree::editor::Body;
use crate::app::filetree::pane::PaneAct;
use crate::app::filetree::*;
use crate::editor::text::hash;
use crate::prelude::*;
use std::path::{Component, Path};

impl DrawingApp {
    pub(crate) fn native_filetree_act(&mut self, verb: &str, args: &Value) -> Result<String, String> {
        let text = |key: &str| args[key].as_str().unwrap_or_default().to_string();
        let id = text("id");
        let spec = self.ft_spec(&id).cloned().ok_or("árvore não encontrada")?;
        let path = PathBuf::from(text("path"));
        let number = |key: &str| args[key].as_u64().unwrap_or(0) as usize;
        let flag = |key: &str| args[key].as_bool();
        // what changes the disk stays inside the tree (`starts_with` does not resolve `..`)
        let inside = |target: &Path| {
            if target.starts_with(spec.root_path()) && !target.components().any(|part| matches!(part, Component::ParentDir)) {
                Ok(())
            } else {
                Err(format!("{} está fora da árvore.", target.display()))
            }
        };
        let done = |words: &str| Ok(words.to_string());
        match verb {
            // a press in the tree picks the node on the canvas, as the egui tree's `Act::Focus`
            "focus" => {
                if let Some(index) = self.scene.elements.iter().position(|e| e.id == id) {
                    (self.selected, self.card_focus, self.terminal_focus) = (vec![index], Some(id.clone()), None);
                }
                done("")
            }
            // the "Voltar" button the link left on the tree: back to where it came from, or only let it go
            "link_back" => self.link_go_back(),
            "link_dismiss" => {
                self.filetrees.link_back = None;
                done("")
            }
            "select" => {
                if let Some(view) = self.filetrees.views.get_mut(&id) {
                    (view.selected, view.kb) = (Some(path.clone()), false);
                }
                // with the editor shown, a click on a file previews it there
                if spec.editor && path.is_file() {
                    self.ft_open(&id, path, false);
                }
                done("")
            }
            "activate" => {
                self.ft_activate(&id, path);
                done("")
            }
            "toggle" => {
                self.ft_toggle(&id, &path);
                done("")
            }
            "into" => {
                self.ft_navigate(&id, path);
                done("")
            }
            "back" => {
                self.ft_back(&id);
                done("")
            }
            "forward" => {
                self.ft_forward(&id);
                done("")
            }
            "collapse_all" => {
                self.ft_collapse_all(&id);
                done("")
            }
            "filter" => {
                if let Some(view) = self.filetrees.views.get_mut(&id) {
                    (view.filter, view.scroll, view.dirty) = (text("text"), 0.0, true);
                }
                done("")
            }
            "hidden" => {
                if let Some(spec) = self.ft_spec_mut(&id) {
                    spec.hide_hidden = !spec.hide_hidden;
                }
                self.ft_refresh(&id);
                done("")
            }
            "sort" => {
                let by = SortBy::ALL.into_iter().find(|by| by.key() == text("by")).ok_or("ordem desconhecida")?;
                if let Some(spec) = self.ft_spec_mut(&id) {
                    spec.sort = by;
                }
                self.ft_refresh(&id);
                done("")
            }
            "reverse" => {
                if let Some(spec) = self.ft_spec_mut(&id) {
                    spec.reversed = !spec.reversed;
                }
                self.ft_refresh(&id);
                done("")
            }
            "copy" | "cut" => {
                inside(&path)?;
                self.filetrees.clipboard = Some((vec![path], verb == "cut"));
                done("")
            }
            "paste" => {
                let dir = PathBuf::from(text("dir"));
                inside(&dir)?;
                self.ft_paste(&id, &dir).map(|count| if count == 1 { "1 item colado.".into() } else { format!("{count} itens colados.") })
            }
            "create" => {
                let dir = PathBuf::from(text("dir"));
                inside(&dir)?;
                let folder = flag("folder").unwrap_or(false);
                self.ft_create(&id, &dir, &text("name"), folder).map(|made| made.to_string_lossy().into_owned())
            }
            "rename" => {
                inside(&path)?;
                self.ft_rename(&id, &path, &text("name")).map(|to| to.to_string_lossy().into_owned())
            }
            "trash" => {
                let paths: Vec<PathBuf> = args["paths"].as_array().into_iter().flatten().filter_map(|p| p.as_str().map(PathBuf::from)).collect();
                paths.iter().try_for_each(|p| inside(p))?;
                self.ft_trash(&id, &paths)
                    .map(|count| if count == 1 { "1 item movido para a lixeira.".into() } else { format!("{count} itens movidos para a lixeira.") })
            }
            "editor" => {
                self.ft_show_editor(&id, flag("show").unwrap_or(!spec.editor));
                done("")
            }
            "changes" => {
                self.ft_show_changes(&id, flag("show").unwrap_or(!spec.changes));
                done("")
            }
            "branches" => {
                let branches = crate::app::filetree::branch::local_branches(&spec.root_path()).unwrap_or_default();
                if let Some(view) = self.filetrees.views.get_mut(&id) {
                    view.branches = branches;
                }
                done("")
            }
            "checkout" => self.ft_checkout(&id, text("branch").trim()),
            "tab_select" => self.pane(&id, PaneAct::Select(path)),
            "tab_pin" => self.pane(&id, PaneAct::Pin(path)),
            "tab_close_others" => self.pane(&id, PaneAct::CloseOthers(path)),
            "tab_close_all" => self.pane(&id, PaneAct::CloseAll),
            "tab_close" => self.ft_editor(&id).close(&path).map(|()| String::new()),
            "save" => {
                self.filetrees.editors.get_mut(&id).ok_or("Nenhum arquivo aberto no editor desta árvore.")?.save()?;
                done("Arquivo salvo.")
            }
            "wrap" => self.pane(&id, PaneAct::Wrap),
            "reveal" => self.pane(&id, PaneAct::Reveal(path)),
            "reveal_in_tree" => self.pane(&id, PaneAct::RevealInTree(path)),
            "open_default" => self.pane(&id, PaneAct::OpenDefault(path)),
            "retry" => self.pane(&id, PaneAct::Retry(path)),
            "seam" => self.pane(&id, PaneAct::Seam(args["width"].as_f64().unwrap_or(0.0) as f32)),
            "edit" => self.native_edit(&id, &path, number("from"), number("to"), &text("text"), args["rev"].as_u64().unwrap_or(0)),
            "viewport" => {
                self.filetrees.native.trees.entry(id).or_default().viewport = (number("first"), number("last"));
                done("")
            }
            "problems_panel" => {
                self.filetrees.native.trees.entry(id).or_default().problems_open = flag("open").unwrap_or(false);
                done("")
            }
            "goto" => {
                let (line, col) = (number("line"), number("col"));
                self.pane(&id, if flag("pinned").unwrap_or(true) { PaneAct::Goto(path, line, col) } else { PaneAct::Look(path, line, col) })
            }
            // Ctrl+click on a name: its declaration (a framework's string answers now, a name once the project is searched)
            "declaration" => {
                let ctx = self.filetrees.ctx.clone().unwrap_or_default();
                let ed = self.ft_editor(&id);
                let Some(Body::Text(buf)) = ed.tabs.iter().find(|tab| tab.path == path).map(|tab| &tab.body) else {
                    return Err("O arquivo não está aberto.".into());
                };
                let byte = byte_of(&buf.text, number("offset")).ok_or("posição inválida")?;
                let chars = buf.text[..byte].chars().count();
                match ed.code.declaration(&path, buf.lang, &buf.text, chars, &ctx) {
                    Some(crate::app::filetree::code::Found::At(file, line, col)) => self.pane(&id, PaneAct::Goto(file, line, col)),
                    _ => done(""),
                }
            }
            "open_change" => {
                self.ft_open_change(&id, &path);
                done("")
            }
            "diff" => self.ft_show_diff(&id, &path).map(|count| format!("{count} mudança(s).")),
            "revert_change" => {
                inside(&path)?;
                self.ft_revert_change(&id, &path, number("index")).map(|()| String::new())
            }
            "compare" => {
                let ed = self.ft_editor(&id);
                let Some(Body::Compare(compare)) = ed.tabs.iter_mut().find(|tab| tab.path == path).map(|tab| &mut tab.body) else {
                    return Err("A aba não compara um arquivo.".into());
                };
                if let Some(unified) = flag("unified") {
                    compare.set_unified(unified);
                }
                if let Some(collapse) = flag("collapse") {
                    compare.set_collapse(collapse);
                }
                if let Some(by) = args["go"].as_i64() {
                    compare.go(by as isize);
                }
                if args["fold"].is_u64() {
                    compare.open_fold(number("fold"));
                }
                done("")
            }
            "find_open" => {
                self.ft_find_open(&id, text("query"));
                done("")
            }
            "find_set" => self.native_find_set(&id, &spec, args),
            "find_pick" => {
                let ed = self.ft_editor(&id);
                let find = ed.find.as_mut().ok_or("A busca não está aberta.")?;
                let hit = find.hits.get(number("index")).cloned().ok_or("Essa ocorrência não existe mais.")?;
                find.selected = number("index");
                let open = flag("open").unwrap_or(false);
                if open {
                    ed.find = None;
                }
                self.pane(&id, if open { PaneAct::Goto(hit.path, hit.line, hit.col) } else { PaneAct::Look(hit.path, hit.line, hit.col) })
            }
            // "Abrir na janela de busca": what it found, kept in a tab
            "find_tab" => {
                let ed = self.ft_editor(&id);
                let find = ed.find.take().ok_or("A busca não está aberta.")?;
                self.pane(&id, PaneAct::Results(find.ask(), find.hits))
            }
            "find_close" => {
                self.ft_editor(&id).find = None;
                done("")
            }
            other => Err(format!("verbo desconhecido: {other}")),
        }
    }

    fn pane(&mut self, id: &str, act: PaneAct) -> Result<String, String> {
        self.ft_pane_apply(id, act);
        Ok(String::new())
    }

    /// Swift's edit of the text of tab `path`: the UTF-16 range `from..to` of the text as Rust has it becomes `replacement`. Rust owns the buffer: the
    /// colors, the markers and the problems come back from it. An edit that does not fit the text (the two are out of step) makes Swift load it again.
    fn native_edit(&mut self, id: &str, path: &Path, from: usize, to: usize, replacement: &str, rev: u64) -> Result<String, String> {
        let ed = self.filetrees.editors.get_mut(id).ok_or("Nenhum arquivo aberto no editor desta árvore.")?;
        let Some(Body::Text(buf)) = ed.tabs.iter_mut().find(|tab| tab.path == path).map(|tab| &mut tab.body) else {
            return Err("O arquivo não está aberto.".into());
        };
        let tree = self.filetrees.native.trees.entry(id.to_string()).or_default();
        match (byte_of(&buf.text, from), byte_of(&buf.text, to)) {
            (Some(start), Some(end)) if start <= end => {
                buf.text.replace_range(start..end, replacement);
                buf.dirty = true;
                (tree.text_hash, tree.applied_rev) = (hash(&buf.text, 0.0), rev);
                Ok(String::new())
            }
            _ => {
                // the hash no longer matches: the next frame sends the text anew
                tree.text_hash = 0;
                Err("A edição não cabe no texto: o editor o carrega de novo.".into())
            }
        }
    }

    /// What was typed or ticked in "Localizar nos Arquivos"; only the fields given change.
    fn native_find_set(&mut self, id: &str, spec: &FileTreeSpec, args: &Value) -> Result<String, String> {
        let ed = self.ft_editor(id);
        let find = ed.find.as_mut().ok_or("A busca não está aberta.")?;
        if let Some(query) = args["query"].as_str() {
            find.query = query.to_string();
        }
        for (key, field) in [
            ("case", &mut find.case),
            ("words", &mut find.words),
            ("regex", &mut find.regex),
            ("multiline", &mut find.multiline),
            ("maskOn", &mut find.mask_on),
            ("inDir", &mut find.in_dir),
        ] {
            if let Some(on) = args[key].as_bool() {
                *field = on;
            }
        }
        if let Some(mask) = args["mask"].as_str() {
            find.mask = mask.to_string();
        }
        // a folder typed: from the root, or a whole path
        if let Some(typed) = args["folder"].as_str() {
            let typed = typed.trim().replace('\\', "/");
            let folder = if Path::new(&typed).is_absolute() { PathBuf::from(&typed) } else { spec.root_path().join(&typed) };
            if folder.is_dir() {
                find.folder = folder;
            }
        }
        Ok(String::new())
    }
}
