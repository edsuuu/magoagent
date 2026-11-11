//! What a press or key in a file tree node does: the actions of the areas (filetree_ui.rs), the
//! dialog and the questions, and the keys of the tree that has them.

use crate::app::filetree::*;
use crate::app::filetree_ui::Act;
use crate::prelude::*;
use std::path::Path;

impl DrawingApp {
    pub(super) fn ft_apply(&mut self, ctx: &Context, id: &str, act: Act) {
        let Some(spec) = self.ft_spec(id).cloned() else { return };
        match act {
            Act::Focus => {
                if let Some(index) = self.scene.elements.iter().position(|e| e.id == id) {
                    self.moving = false;
                    self.marquee = None;
                    self.selected = vec![index];
                    self.card_focus = Some(id.to_string());
                    self.terminal_focus = None;
                }
            }
            Act::Select(path) => {
                if let Some(view) = self.filetrees.views.get_mut(id) {
                    view.selected = Some(path.clone());
                }
                // with the editor shown, a click on a file previews it there
                if spec.editor && path.is_file() {
                    self.ft_open(id, path, false);
                }
            }
            Act::ToggleEditor => self.ft_show_editor(id, !spec.editor),
            Act::Toggle(path) => self.ft_toggle(id, &path),
            Act::Activate(path) => self.ft_activate(id, path),
            Act::Into(path) => self.ft_navigate(id, path),
            Act::Back => self.ft_back(id),
            Act::Forward => self.ft_forward(id),
            Act::Hidden => {
                if let Some(s) = self.ft_spec_mut(id) {
                    s.hide_hidden = !spec.hide_hidden;
                }
                self.ft_refresh(id);
            }
            Act::Sort(by) => {
                if let Some(s) = self.ft_spec_mut(id) {
                    s.sort = by;
                }
                self.ft_refresh(id);
            }
            Act::Reverse => {
                if let Some(s) = self.ft_spec_mut(id) {
                    s.reversed = !spec.reversed;
                }
                self.ft_refresh(id);
            }
            Act::CollapseAll => self.ft_collapse_all(id),
            Act::CopyPath(path, relative) => {
                let shown = if relative { path.strip_prefix(spec.root_path()).unwrap_or(&path).to_path_buf() } else { path };
                ctx.copy_text(shown.display().to_string());
                self.toast(if relative { "Caminho relativo copiado." } else { "Caminho copiado." });
            }
            Act::Copy(path) => {
                self.filetrees.clipboard = Some((vec![path], false));
                self.toast("Copiado.");
            }
            Act::Cut(path) => {
                self.filetrees.clipboard = Some((vec![path], true));
                self.toast("Pronto para mover.");
            }
            Act::Paste(dir) => match self.ft_paste(id, &dir) {
                Ok(n) => self.toast(if n == 1 { "1 item colado.".to_string() } else { format!("{n} itens colados.") }),
                Err(why) => self.toast(why),
            },
            Act::Ask(ask) => self.filetrees.ask = Some(ask),
        }
    }

    /// Whether a file tree question or dialog is open (it takes the keys).
    pub(crate) fn filetree_dialog_open(&self) -> bool {
        self.filetrees.dialog.is_some() || self.filetrees.ask.is_some()
    }

    /// The new / change root dialog, or the question a file operation asks.
    pub(crate) fn filetree_dialog_ui(&mut self, ctx: &Context) {
        let c = self.colors();
        if let Some(mut dialog) = self.filetrees.dialog.clone() {
            let editing = dialog.target.is_some();
            let (confirm, cancel) = dialog_ui(
                ctx,
                &c,
                if editing { "Alterar Diretório Raiz" } else { "Nova Árvore de Arquivos" },
                if editing { "Salvar" } else { "Criar" },
                |ui| {
                    dialog_folder(ui, "Pasta raiz", &mut dialog.root, "A pasta que esta árvore mostra", "Pasta raiz", &c, &mut self.prefs.folders);
                    dialog_note(ui, if editing { "Escolha a pasta raiz desta árvore." } else { "Escolha a pasta raiz da nova árvore." }, &c);
                },
            );
            self.filetrees.dialog = if cancel { None } else { Some(dialog) };
            if confirm && !cancel {
                self.confirm_filetree_dialog();
            }
            return;
        }
        let Some(mut ask) = self.filetrees.ask.clone() else { return };
        let (confirm, cancel) = match &mut ask {
            // a name is asked in the popover under the row (filetree_ui.rs)
            TreeAsk::Name { .. } => return,
            TreeAsk::Trash { paths, .. } => {
                let note = match paths.as_slice() {
                    [one] => format!("{} será movido para a lixeira. Você pode restaurá-lo de lá.", one.file_name().unwrap_or_default().to_string_lossy()),
                    many => format!("{} itens serão movidos para a lixeira. Você pode restaurá-los de lá.", many.len()),
                };
                dialog_ui(ctx, &c, "Mover para a Lixeira?", "Mover para a Lixeira", |ui| dialog_note(ui, &note, &c))
            }
        };
        if cancel {
            self.filetrees.ask = None;
        } else if confirm {
            self.filetrees.ask = None;
            let text = if let TreeAsk::Name { text, .. } = &ask { text.clone() } else { String::new() };
            self.ft_answer(ask, text);
        } else {
            self.filetrees.ask = Some(ask);
        }
    }

    /// The keys of the tree that has them: arrows, Home/End, Enter, and the reference's file shortcuts.
    pub(crate) fn filetree_keys(&mut self, ctx: &Context) {
        let Some(id) = self.card_focus.clone() else { return };
        // whatever took the keys since (a terminal, the open IDE, a field, a dialog) keeps them
        let taken = self.terminal_focus.is_some()
            || self.ide_focus.is_some()
            || self.terminal_elevated.is_some()
            || self.editing.is_some()
            || self.filetree_dialog_open()
            || ctx.egui_wants_keyboard_input();
        if taken {
            self.card_focus = None;
            return;
        }
        let mac = cfg!(target_os = "macos");
        let wanted = |key: Key, m: Modifiers| match (m.command, m.alt, m.shift) {
            (false, false, false) => {
                matches!(
                    key,
                    Key::ArrowUp | Key::ArrowDown | Key::ArrowLeft | Key::ArrowRight | Key::Home | Key::End | Key::Enter | Key::Escape | Key::F2 | Key::Delete
                )
            }
            (true, false, false) => matches!(key, Key::C | Key::X | Key::V | Key::N | Key::Enter | Key::Backspace),
            (true, true, false) => matches!(key, Key::C | Key::N),
            (true, true, true) => key == Key::C,
            _ => false,
        };
        let events: Vec<(Key, Modifiers)> = ctx.input_mut(|i| {
            let mut taken = vec![];
            i.events.retain(|e| match e {
                egui::Event::Key { key, pressed: true, modifiers, .. } if wanted(*key, *modifiers) => {
                    taken.push((*key, *modifiers));
                    false
                }
                _ => true,
            });
            taken
        });
        for (key, m) in events {
            let Some(spec) = self.ft_spec(&id).cloned() else { return };
            let Some(view) = self.filetrees.views.get(&id) else { return };
            let rows = view.rows.clone();
            let at = view.selected.as_ref().and_then(|s| rows.iter().position(|r| &r.path == s));
            let row = at.map(|i| rows[i].clone());
            let shown = spec.shown();
            let dir = match &row {
                Some(r) if r.dir => r.path.clone(),
                Some(r) => r.path.parent().map_or_else(|| shown.clone(), Path::to_path_buf),
                None => shown.clone(),
            };
            let go = |this: &mut Self, to: usize| {
                if let Some(r) = rows.get(to) {
                    this.ft_apply(ctx, &id, Act::Select(r.path.clone()));
                    // keep the row in view
                    if let Some(v) = this.filetrees.views.get_mut(&id) {
                        v.kb = true;
                        let (top, bottom) = (to as f32 * FT_ROW, (to + 1) as f32 * FT_ROW);
                        v.scroll = v.scroll.min(top).max(bottom - v.body_h.max(FT_ROW));
                    }
                }
            };
            let last = rows.len().saturating_sub(1);
            match (key, m.command, m.alt, m.shift) {
                (Key::Escape, ..) => {
                    // the arrows go back to nothing: with the node still selected they would push it
                    self.card_focus = None;
                    self.selected.clear();
                    return;
                }
                (Key::ArrowDown, ..) => go(self, at.map_or(0, |i| (i + 1).min(last))),
                (Key::ArrowUp, ..) => go(self, at.map_or(0, |i| i.saturating_sub(1))),
                (Key::Home, ..) => go(self, 0),
                (Key::End, ..) => go(self, last),
                (Key::ArrowRight, ..) => {
                    if let Some(r) = row.filter(|r| r.dir) {
                        if !r.open {
                            self.ft_toggle(&id, &r.path);
                        } else {
                            go(self, at.unwrap_or(0) + 1);
                        }
                    }
                }
                (Key::ArrowLeft, ..) => {
                    if let Some(r) = row {
                        if r.dir && r.open {
                            self.ft_toggle(&id, &r.path);
                        } else if let Some(parent) = rows[..at.unwrap_or(0)].iter().rposition(|p| p.depth + 1 == r.depth) {
                            go(self, parent);
                        }
                    }
                }
                (Key::Enter, false, ..) => {
                    if let Some(r) = row {
                        self.ft_activate(&id, r.path);
                    }
                }
                (Key::Enter, true, ..) if mac => self.ft_rename_ask(ctx, &id, &row),
                (Key::F2, ..) => self.ft_rename_ask(ctx, &id, &row),
                (Key::Delete, false, ..) | (Key::Backspace, true, ..) if row.is_some() && (key == Key::Delete || mac) => {
                    self.ft_apply(ctx, &id, Act::Ask(TreeAsk::Trash { tree: id.clone(), paths: vec![row.unwrap().path] }));
                }
                (Key::C, true, false, false) => {
                    if let Some(r) = row {
                        self.ft_apply(ctx, &id, Act::Copy(r.path));
                    }
                }
                (Key::X, true, ..) => {
                    if let Some(r) = row {
                        self.ft_apply(ctx, &id, Act::Cut(r.path));
                    }
                }
                (Key::V, true, ..) if self.filetrees.clipboard.is_some() => self.ft_apply(ctx, &id, Act::Paste(dir)),
                (Key::C, true, true, shift) => {
                    if let Some(r) = row {
                        self.ft_apply(ctx, &id, Act::CopyPath(r.path, shift));
                    }
                }
                (Key::N, true, alt, _) => {
                    let (kind, text) = if alt { (NameKind::NewFolder, "Nova Pasta") } else { (NameKind::NewFile, "Sem título") };
                    self.ft_apply(ctx, &id, Act::Ask(TreeAsk::Name { tree: id.clone(), at: dir, kind, text: text.into() }));
                }
                _ => {}
            }
        }
    }

    pub(crate) fn ft_rename_ask(&mut self, ctx: &Context, id: &str, row: &Option<TreeRow>) {
        if let Some(r) = row {
            self.ft_apply(
                ctx,
                id,
                Act::Ask(TreeAsk::Name {
                    tree: id.to_string(),
                    at: r.path.clone(),
                    kind: NameKind::Rename,
                    text: crate::app::filetree_menu::split_name(&r.name).0.to_string(),
                }),
            );
        }
    }
}
