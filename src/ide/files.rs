use super::*;

impl Ide {
    pub(crate) fn open_file(&mut self, path: PathBuf, at: Option<(usize, usize)>) {
        let i = match self.tabs.iter().position(|t| t.path == path && t.in_panel()) {
            Some(i) => i,
            // a picture or a PDF: its pages instead of text
            None if media::is_media(&path) => {
                let mut tab = Tab::new(path.clone(), String::new(), false);
                tab.media = Some(Box::new(media::Media::open(&path, &self.ctx)));
                self.tabs.push(tab);
                self.tabs.len() - 1
            }
            None => {
                // refused files get a tab that says why (the editor's message, not just the status line)
                let refuse = |this: &mut Self, path: PathBuf, why: String| {
                    let mut tab = Tab::new(path, String::new(), false);
                    tab.media = Some(Box::new(media::Media::refused(why)));
                    this.tabs.push(tab);
                    this.tabs.len() - 1
                };
                let size = std::fs::metadata(&path).map_or(0, |m| m.len());
                // ponytail: the editor lays out the whole text every frame (~450 bytes of memory per byte, a frame that grows with
                // the file); a bigger file needs a windowed view of the visible lines
                let bytes = if size > MAX_TEXT {
                    Err(format!("Este arquivo tem {}, além do que o editor embutido comporta.", crate::app::chat::images::size_label(size)))
                } else {
                    std::fs::read(&path).map_err(|e| e.to_string())
                };
                let text = match bytes
                    .and_then(|b| String::from_utf8(b).map_err(|_| "Este não é um arquivo de texto, então não há nada para editar aqui.".to_string()))
                {
                    Ok(s) => s,
                    Err(why) => {
                        self.status = why.clone();
                        let i = refuse(self, path, why);
                        self.active = i;
                        return;
                    }
                };
                let crlf = text.contains("\r\n");
                let text = if crlf { text.replace("\r\n", "\n") } else { text };
                let tab = Tab::new(path, text, crlf);
                if let Some(insp) = &mut self.insp {
                    insp.open(&tab.path, tab.lang, &tab.text);
                }
                self.tabs.push(tab);
                self.tabs.len() - 1
            }
        };
        self.active = i;
        if let Some((line, col)) = at {
            let tab = &mut self.tabs[i];
            let c = char_at(&tab.text, line, col);
            tab.select = Some((c, c));
        }
    }

    /// An agent's edit (the MCP's ide_edit): `old`, found exactly once in `path`, becomes `new`,
    /// in the open tab (the file comes forward) and on disk. Back: the changed lines (first, last).
    /// A tab with the person's unsaved text is left alone; an edit that fails leaves the tab as it was.
    pub(crate) fn agent_edit(&mut self, path: &Path, old: &str, new: &str) -> Result<(usize, usize), String> {
        let (old, new) = (old.replace("\r\n", "\n"), new.replace("\r\n", "\n"));
        if old.is_empty() {
            return Err("`old` vazio: mande o trecho a trocar".into());
        }
        if self.tabs.iter().any(|t| t.path == path && t.is_text() && t.dirty) {
            return Err(format!("{} tem mudanças não salvas no IDE: a pessoa precisa salvar ou descartar antes da edição", file_name(path)));
        }
        self.open_file(path.to_path_buf(), None);
        let i = self.active;
        let tab = self.tabs.get_mut(i).filter(|t| t.path == path && t.is_text()).ok_or_else(|| self.status.clone())?;
        // a clean tab follows the disk (the agent may have written the file itself)
        tab.reload();
        let at = match tab.text.match_indices(&old).map(|(at, _)| at).collect::<Vec<_>>()[..] {
            [at] => at,
            [] => return Err(format!("o trecho não está em {}", file_name(path))),
            ref many => return Err(format!("o trecho aparece {} vezes em {}: mande mais contexto", many.len(), file_name(path))),
        };
        let before = tab.text.clone();
        tab.text.replace_range(at..at + old.len(), &new);
        let first = tab.text[..at].matches('\n').count();
        // a line break closing `new` ends its last line, it doesn't start another
        let last = first + new.trim_end_matches('\n').matches('\n').count();
        (tab.dirty, tab.lsp_pending, tab.scroll_to) = (true, Some(Instant::now()), Some(first));
        self.save(i, false);
        if self.tabs[i].dirty {
            (self.tabs[i].text, self.tabs[i].dirty) = (before, false);
            return Err(format!("não foi possível salvar: {}", self.status));
        }
        self.panel_mark = Some((path.to_path_buf(), first, last));
        Ok((first, last))
    }

    /// The conflict band's two buttons, and the MCP's ide_conflict: "reload" takes the disk's text,
    /// "keep" writes the tab's over it. `path` (absolute, or relative to the project) names the
    /// tab; empty, the open one.
    pub(crate) fn resolve_conflict(&mut self, path: &str, action: &str) -> Result<String, String> {
        let i = if path.trim().is_empty() {
            self.active.min(self.tabs.len().saturating_sub(1))
        } else {
            let named = |tab: &Tab| tab.path == Path::new(path) || self.root.as_ref().is_some_and(|root| tab.path == root.join(path));
            self.tabs.iter().position(named).ok_or_else(|| format!("{path} não está aberto no IDE."))?
        };
        let tab = self.tabs.get(i).ok_or_else(|| "Não há arquivo aberto no IDE.".to_string())?;
        let file = file_name(&tab.path);
        if !tab.conflict() {
            return Err(format!("{file} não está em conflito no IDE."));
        }
        match action {
            "reload" => {
                self.tabs[i].reload();
                Ok(format!("{file} recarregado do disco."))
            }
            "keep" => {
                self.save(i, true);
                if self.tabs[i].dirty {
                    Err(format!("Não foi possível gravar {file}: {}", self.status))
                } else {
                    Ok(format!("A versão da aba foi gravada em {file}."))
                }
            }
            other => Err(format!("ação desconhecida: {other}")),
        }
    }

    pub(super) fn save(&mut self, i: usize, force: bool) {
        let tab = &self.tabs[i];
        if !tab.dirty || !tab.is_text() {
            return;
        }
        // Claude Code, git or a formatter rewrote it meanwhile: only an explicit Ctrl+S (or the editor's
        // conflict band, `conflict_bar`) overwrites that; closing first keeps the text aside (`recover`)
        let on_disk = std::fs::metadata(&tab.path).and_then(|m| m.modified()).ok();
        if !force && tab.mtime.is_some() && on_disk.is_none() {
            return self.notify(format!("{} foi excluído do disco; {} grava de novo", file_name(&tab.path), chrome::keys("Ctrl+S")));
        }
        if !force && tab.mtime.is_some() && on_disk != tab.mtime {
            return self.notify(format!("{} mudou no disco; {} sobrescreve", file_name(&tab.path), chrome::keys("Ctrl+S")));
        }
        let tab = &mut self.tabs[i];
        let text = if tab.crlf { tab.text.replace('\n', "\r\n") } else { tab.text.clone() };
        match std::fs::write(&tab.path, text) {
            Ok(()) => {
                tab.dirty = false;
                tab.mtime = std::fs::metadata(&tab.path).and_then(|m| m.modified()).ok();
                tab.head.get_or_insert(None); // keep markers, HEAD did not move
                // the blame is of the file on disk: the next frame asks for it again
                self.blame.remove(&tab.path);
                self.blame_gen += 1;
                if self.root.is_some() && !self.excluded.contains(&tab.path) {
                    if index::indexable(&tab.path) {
                        self.index.update(&tab.path);
                        self.index_gen += 1;
                    }
                    std::sync::Arc::make_mut(&mut self.text_index).update(&tab.path);
                    self.text_gen += 1;
                }
                if let Some(insp) = &mut self.insp {
                    if tab.lsp_pending.take().is_some() {
                        insp.change(&tab.path, tab.lang, &tab.text);
                    }
                    insp.save(&tab.path, tab.lang);
                }
            }
            Err(e) => self.status = format!("Não foi possível salvar {}: {e}", tab.path.display()),
        }
    }

    /// PhpStorm-style: no "save?" prompts, everything is saved on tab close, focus loss, leaving
    /// the IDE, removing its node (`Drop`) and exit — except a file that changed on disk meanwhile,
    /// which only Ctrl+S overwrites.
    pub(crate) fn save_all(&mut self) {
        for i in 0..self.tabs.len() {
            self.save(i, false);
        }
    }

    /// `save_all` on the way out: the text of a tab that still could not be written (changed or
    /// deleted on disk, a failed write) goes to the recovery folder, named by its text so a second
    /// pass writes the same file. Back: the message saying where, also shown.
    pub(crate) fn recover(&mut self) -> Option<String> {
        self.save_all();
        let dir = self.recovery.clone()?;
        let mut kept = vec![];
        for tab in self.tabs.iter().filter(|t| t.is_text() && t.dirty) {
            let to = dir.join(format!("{:016x}-{}", hash(&tab.text, 0.0), file_name(&tab.path)));
            let text = if tab.crlf { tab.text.replace('\n', "\r\n") } else { tab.text.clone() };
            match std::fs::create_dir_all(&dir).and_then(|()| std::fs::write(&to, text)) {
                Ok(()) => kept.push(to.display().to_string()),
                Err(e) => kept.push(format!("{} NÃO foi guardado ({e})", file_name(&tab.path))),
            }
        }
        if kept.is_empty() {
            return None;
        }
        let message = format!("Mudanças não salvas que não puderam ir para o arquivo (mudou ou sumiu do disco) foram guardadas em: {}", kept.join(", "));
        self.notify(message.clone());
        Some(message)
    }

    pub(crate) fn close_tab(&mut self, i: usize) {
        if i >= self.tabs.len() {
            return;
        }
        if self.tabs[i].sheet.as_ref().is_some_and(|s| s.pending()) {
            self.notify("Envie ou reverta as pendências da tabela antes de fechar a aba.".into());
            return;
        }
        self.save(i, false);
        if self.tabs[i].dirty {
            return; // not saved (changed on disk, or the write failed): the status says why, the tab stays
        }
        // the card's panel goes to another file, at its top
        if self.card_tab() == Some(i) {
            self.panel_scroll = 0.0;
        }
        let mut tab = self.tabs.remove(i);
        if let Some((reply, _)) = tab.diff.as_mut().and_then(|d| d.claude.take()) {
            let _ = reply.send(vec!["DIFF_REJECTED".into()]);
        }
        if let Some(insp) = &mut self.insp
            && tab.is_text()
        {
            insp.close(&tab.path, tab.lang);
        }
        if self.active >= i && self.active > 0 {
            self.active -= 1;
        }
    }
}
