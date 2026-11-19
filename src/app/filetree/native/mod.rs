//! The file tree for the SwiftUI UI on macOS (`FileTreeView(id:)`, macos/MagoUI/Sources/MagoUI/FileTree). Rust keeps owning the folders,
//! git, the open files' text and the saving; each frame `native_filetree` does what the egui pane does between frames (the disk's news,
//! the project's index, the problems) and publishes, per tree:
//!   `filetree:<id>`       the rows with their marks, the git state, the panels' lists and the editor's tabs (snapshot.rs)
//!   `filetree:<id>:doc`   the active tab's body; the text of a text tab goes along once per `loadRev` (doc.rs)
//!   `filetree:<id>:live`  what the text's edits change: the colors (spans), the change markers, the problems (doc.rs)
//!   `filetree:<id>:find`  "Localizar nos Arquivos" (null while closed)
//! and, once for all trees, `filetree:icons` (the embedded SVGs) and `filetree:theme` (the editor's colors). What Swift does goes to
//! `native_filetree_act` as `ui.filetree.<verb>` (verbs.rs).

mod doc;
mod snapshot;
#[cfg(test)]
mod tests;
mod verbs;

use crate::app::filetree::code::Found;
use crate::app::filetree::editor::Body;
use crate::app::filetree::pane::PaneAct;
use crate::app::filetree::problems::propagate;
use crate::app::native::publish;
use crate::editor::highlight::St;
use crate::editor::text::hash;
use crate::prelude::*;
use doc::*;
use snapshot::*;
use std::collections::HashMap;
use std::path::Path;

/// What the SwiftUI trees keep between frames.
#[derive(Default)]
pub(crate) struct Native {
    trees: HashMap<String, TreeNative>,
    /// When the icons and the theme were last published (they are sent again now and then, in case Swift started late).
    statics_at: Option<f64>,
}

#[derive(Default)]
struct TreeNative {
    checked: f64,
    problems_at: f64,
    problems: ProblemsOut,
    problems_open: bool,
    /// The first and last line Swift shows (a big file is colored around them).
    viewport: (usize, usize),
    /// The tab the text bookkeeping below is of, the hash of its text as Rust and Swift last agreed on it, and the revisions:
    /// `load_rev` moves when Rust changes the text (Swift loads it again), `applied_rev` is Swift's last edit applied.
    active: Option<PathBuf>,
    text_hash: u64,
    load_rev: u64,
    applied_rev: u64,
    tokens: Option<(u64, Vec<(usize, usize, St)>)>,
    jump: Option<Jump>,
    jump_seq: u64,
    focus_seq: u64,
    find_seq: u64,
    doc_key: String,
    live_key: String,
    find_key: String,
}

impl DrawingApp {
    /// Every frame, while the SwiftUI file tree is on: the trees' news in, their topics out.
    pub(crate) fn native_filetree(&mut self) {
        let Some(ctx) = self.filetrees.ctx.clone() else { return };
        let now = ctx.input(|input| input.time);
        let native = &mut self.filetrees.native;

        if native.statics_at.is_none_or(|at| now - at > 5.0) {
            publish("filetree:icons", &icon_table());
            publish("filetree:theme", &theme());
            native.statics_at = Some(now);
        }

        let ids: Vec<String> = self.scene.elements.iter().filter(|e| e.filetree.is_some()).map(|e| e.id.clone()).collect();

        self.filetrees.native.trees.retain(|id, _| ids.contains(id));

        for id in ids {
            self.native_tree(&id, &ctx, now);
        }
    }

    fn native_tree(&mut self, id: &str, ctx: &Context, now: f64) {
        let Some(spec) = self.ft_spec(id).cloned() else { return };

        if !self.filetrees.views.contains_key(id) {
            return;
        }

        let root = spec.root_path();
        let mut found = None;

        // what the pane does before it draws: the disk's news once a second, the project's index, the compare tabs, the problems
        if spec.editor
            && let Some(ed) = self.filetrees.editors.get_mut(id)
        {
            let tree = self.filetrees.native.trees.entry(id.to_string()).or_default();

            if ed.active.as_ref().is_some_and(|active| ed.preview.as_ref() == Some(active)) && ed.dirty() {
                ed.preview = None;
            }

            if now - tree.checked > 1.0 {
                tree.checked = now;

                for tab in &mut ed.tabs {
                    if let Body::Text(buf) = &mut tab.body {
                        buf.check_disk(&tab.path);
                    }
                }
            }

            found = ed.code.poll();
            ed.sync_compares();

            ed.problems.sync(
                ed.code.root(),
                ed.tabs.iter().filter_map(|tab| match &tab.body {
                    Body::Text(buf) => Some((tab.path.as_path(), buf.text.as_str(), buf.lang)),
                    _ => None,
                }),
            );

            if let Some(find) = ed.find.as_mut() {
                find.tick(&mut ed.texts, ctx);
            }

            if now - tree.problems_at > 0.5 {
                tree.problems_at = now;
                tree.problems = ProblemsOut::read(ed, &root);

                if let Some(view) = self.filetrees.views.get_mut(id) {
                    view.problems = propagate(&ed.problems.error_files(), &root);
                }
            }
        }

        match found {
            Some(Found::At(path, line, col)) => self.ft_pane_apply(id, PaneAct::Goto(path, line, col)),
            Some(Found::Nowhere(word)) => self.ft_pane_apply(id, PaneAct::Missing(word)),
            // ponytail: a Ctrl+click on the declaration itself lists its usages in egui; the SwiftUI editor has no such popup yet
            Some(Found::Own(..)) | None => {}
        }

        let tree = self.filetrees.native.trees.entry(id.to_string()).or_default();
        let (problems_open, problems) = (tree.problems_open, std::mem::take(&mut tree.problems));
        let state = self.native_tree_state(id, problems_open, &problems);

        self.filetrees.native.trees.entry(id.to_string()).or_default().problems = problems;

        if let Some(state) = state {
            publish(&format!("filetree:{id}"), &state);
        }

        self.native_doc(id, &root);
        self.native_find(id, &root);
    }

    /// The active tab's `:doc` and `:live` topics.
    fn native_doc(&mut self, id: &str, root: &Path) {
        let editor_open = self.ft_spec(id).is_some_and(|spec| spec.editor);
        let tree = self.filetrees.native.trees.entry(id.to_string()).or_default();
        let ed = self.filetrees.editors.get_mut(id).filter(|_| editor_open);

        let Some(path) = ed.as_ref().and_then(|ed| ed.active.clone()) else {
            if tree.doc_key != "none" {
                tree.doc_key = "none".into();
                publish(&format!("filetree:{id}:doc"), &Doc { kind: "none", ..Default::default() });
            }

            return;
        };

        let ed = ed.expect("a tab is active");
        let diags = ed.problems.for_file(&path);
        let Some(tab) = ed.tabs.iter_mut().find(|tab| tab.path == path) else { return };

        let Body::Text(buf) = &mut tab.body else {
            let key = format!("{}:{}", path.display(), body_key(&tab.body));

            if tree.doc_key != key {
                tree.doc_key = key;
                publish(&format!("filetree:{id}:doc"), &doc_of(&tab.body, &path, root));
            }

            // coming back to a text tab loads it again
            tree.active = Some(path);

            return;
        };

        let text_hash = hash(&buf.text, 0.0);

        if tree.active.as_ref() != Some(&path) || tree.text_hash != text_hash {
            (tree.active, tree.text_hash, tree.applied_rev, tree.tokens) = (Some(path.clone()), text_hash, 0, None);
            tree.load_rev += 1;
        }

        crate::app::filetree::diff::refresh(buf, text_hash);

        if let Some((line, col)) = buf.jump.take() {
            tree.jump_seq += 1;
            tree.jump = Some(Jump { line, col, seq: tree.jump_seq, focus: !buf.quiet });
        }

        if std::mem::take(&mut buf.focus) {
            tree.focus_seq += 1;
        }

        let name = format!("filetree:{id}:doc");
        let doc_key = format!("{}:{}", path.display(), tree.load_rev);

        if tree.doc_key != doc_key {
            tree.doc_key = doc_key;

            let doc = Doc {
                kind: "text",
                path: path.to_string_lossy().into_owned(),
                load_rev: tree.load_rev,
                text: Some(buf.text.clone()),
                lang: Some(buf.lang.id.to_string()),
                ..Default::default()
            };

            publish(&name, &doc);
        }

        let big = buf.text.len() > 120_000;
        let hunks = hunks_out(buf);
        let diags = diags_out(diags);
        let rest = serde_json::to_string(&(&hunks, &diags)).unwrap_or_default();

        let key = format!(
            "{text_hash}|{}|{}|{}|{}|{}|{}|{}|{}|{rest}",
            tree.load_rev,
            tree.applied_rev,
            buf.dirty,
            buf.external,
            tree.jump_seq,
            tree.focus_seq,
            if big { format!("{:?}", tree.viewport) } else { String::new() },
            buf.head.is_some()
        );

        if tree.live_key != key {
            tree.live_key = key;

            if tree.tokens.as_ref().is_none_or(|(known, _)| *known != text_hash) {
                tree.tokens = Some((text_hash, tokens_of(&buf.text, &path)));
            }

            let tokens = tree.tokens.as_ref().map(|(_, tokens)| tokens.as_slice()).unwrap_or_default();
            let (spans, span_range) = spans(&buf.text, tokens, tree.viewport);

            publish(
                &format!("filetree:{id}:live"),
                &Live {
                    path: path.to_string_lossy().into_owned(),
                    load_rev: tree.load_rev,
                    applied_rev: tree.applied_rev,
                    dirty: buf.dirty,
                    external: buf.external,
                    has_head: buf.head.is_some(),
                    hunks,
                    diags,
                    jump: tree.jump,
                    focus_seq: tree.focus_seq,
                    span_range,
                    spans,
                },
            );
        }
    }

    /// "Localizar nos Arquivos" as a topic, published when what it shows changes.
    fn native_find(&mut self, id: &str, root: &Path) {
        let tree = self.filetrees.native.trees.entry(id.to_string()).or_default();
        let find = self.filetrees.editors.get_mut(id).and_then(|ed| ed.find.as_mut());

        let (key, out) = match find {
            Some(find) => {
                if std::mem::take(&mut find.focus) {
                    tree.find_seq += 1;
                }

                let key = format!(
                    "{:?}|{}|{}|{}|{}|{}|{}|{}|{}|{}|{}|{}",
                    find.ask(),
                    find.hits.len(),
                    find.selected,
                    find.searching(),
                    find.query.len(),
                    find.in_dir,
                    find.folder.display(),
                    find.mask_on,
                    tree.find_seq,
                    find.hits.first().map_or(0, |hit| hit.line),
                    find.hits.last().map_or(0, |hit| hit.line),
                    find.hits.last().map_or(String::new(), |hit| hit.path.display().to_string())
                );

                (key, Some(find_out(find, root, tree.find_seq)))
            }
            None => ("closed".to_string(), None),
        };

        if tree.find_key != key {
            tree.find_key = key;
            publish(&format!("filetree:{id}:find"), &out);
        }
    }
}
