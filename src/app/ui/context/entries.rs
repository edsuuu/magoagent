use super::*;

impl DrawingApp {
    /// A right click at `pos` (world `world`) opens the menu for what is under it. Its element
    /// joins the selection unless it is in it already; on empty canvas the selection goes.
    pub(crate) fn open_context(&mut self, pos: Pos2, world: Pos2) {
        if let Some(gid) = self.group_pill_at(pos) {
            self.context = Some(ContextMenu::new(pos, Target::Group(gid), None));
            return;
        }
        // a rope's right click is its connections popover (ui/links.rs)
        if self.hit(world).is_some_and(|i| self.scene.elements[i].rope) {
            self.open_rope_popover(pos, world);
            return;
        }
        let mut items = None;
        let target = match self.hit(world) {
            Some(i) => {
                if !self.selected.contains(&i) {
                    self.selected = self.group_members(i).into_iter().filter(|j| !self.is_card_text(*j)).collect();
                }
                let target = Target::Element(self.scene.elements[i].id.clone());
                // on an IDE card's tree: the row's file comes first, then the card's own rows
                if let Some((path, root)) = self.card_path_at(i, world) {
                    items = Some(file_entries(&path, root).into_iter().chain(self.context_entries(&target)).collect());
                }
                target
            }
            None => {
                self.selected.clear();
                Target::Canvas(world)
            }
        };
        self.context = Some(ContextMenu::new(pos, target, items));
    }

    /// Draws the open menu, runs the row picked in it, and closes it on Esc or on a press outside
    /// its panels (the press that opened it has no panel yet).
    pub(crate) fn context_ui(&mut self, ctx: &Context) {
        let Some(mut menu) = self.context.take() else { return };
        // no tips over an open menu (Tip reads this). The frame is read first: asking the context
        // for it inside `data_mut` takes its lock twice and the app stops for good
        let frame = ctx.cumulative_frame_nr();
        ctx.data_mut(|d| d.insert_temp(egui::Id::unique("menu_frame"), frame));
        let outside = ctx.input(|i| i.pointer.any_pressed() && i.pointer.interact_pos().is_some_and(|p| !menu.rects.iter().any(|r| r.contains(p))));
        if ctx.input(|i| i.key_pressed(Key::Escape)) || (outside && !menu.rects.is_empty()) {
            return;
        }
        let entries = menu.items.clone().unwrap_or_else(|| self.context_entries(&menu.target));
        let mut rects = Vec::new();
        match panel(ctx, "context_menu", menu.at, &entries, &mut menu.path, 0, &mut rects) {
            Some(id) => {
                let target = menu.target.clone();
                self.context_action(ctx, &target, &id);
            }
            None => {
                menu.rects = rects;
                self.context = Some(menu);
            }
        }
    }

    /// The rows for a target: the canvas's, or the element's (by its kind), or the selection's.
    pub(crate) fn context_entries(&self, target: &Target) -> Vec<Entry> {
        match target {
            Target::Canvas(_) => {
                let presets: Vec<Entry> =
                    self.prefs.presets.iter().enumerate().filter(|(_, p)| p.on).take(10).map(|(k, p)| item(&format!("preset:{k}"), &p.name)).collect();
                // as the reference app: a plain row without presets, a submenu with "Novo Terminal…" above them
                let terminal = if presets.is_empty() {
                    item("add_terminal", "Terminal")
                } else {
                    sub("Terminal", [vec![item("add_terminal", "Novo Terminal…"), Entry::Sep], presets].concat())
                };
                vec![
                    sub(
                        "Adicionar",
                        vec![
                            terminal,
                            self.chat_add_entry(),
                            item("add_note", "Nota"),
                            item("add_file", "Anexar"),
                            item("add_filetree", "Árvore de Arquivos"),
                            item("add_portal", "Navegador"),
                            item("add_text", "Texto"),
                        ],
                    ),
                    Entry::Sep,
                    item("paste", "Colar"),
                    item("select_all", "Selecionar Tudo"),
                ]
            }
            Target::Element(id) => self.element_entries(id),
            Target::Group(gid) => self.group_entries(gid),
        }
    }

    pub(super) fn element_entries(&self, id: &str) -> Vec<Entry> {
        let Some(i) = self.scene.elements.iter().position(|e| e.id == id) else { return Vec::new() };
        // several elements, not a repeated index or a note's own text: a lone image gets no Agrupar
        let mut chosen: Vec<usize> = self.selected.iter().copied().filter(|j| !self.is_card_text(*j)).collect();
        chosen.sort_unstable();
        chosen.dedup();
        if chosen.len() >= 2 && chosen.contains(&i) {
            let mut rows = vec![sub(
                "Alinhar",
                ALIGNS
                    .iter()
                    .enumerate()
                    .flat_map(|(k, label)| {
                        let sep = if k == 3 || k == 6 { Some(Entry::Sep) } else { None };
                        sep.into_iter().chain([item(&format!("align:{k}"), label)])
                    })
                    .collect(),
            )];
            rows.push(keyed("tidy", "Organizar", "⇧⌘T"));
            rows.push(Entry::Sep);
            let (loose, piles) = (self.loose_selected_notes().len(), chosen.iter().filter(|j| self.scene.elements[**j].note.stacked).count());
            if piles == 0 && loose >= 2 {
                rows.push(item("stack", "Colocar no Fichário"));
            } else if piles == 1 && loose >= 1 {
                rows.push(item("stack", "Adicionar ao Fichário"));
            }
            rows.push(item("group", "Grupo"));
            if self.selected_has_group() {
                rows.push(item("ungroup", "Desagrupar"));
            }
            rows.extend([Entry::Sep, item("front", "Trazer para Frente"), item("back", "Enviar para Trás")]);
            return rows;
        }
        if self.scene.elements[i].terminal.is_some() {
            return self.terminal_entries(i);
        }
        if self.scene.elements[i].chat.is_some() {
            return self.chat_node_entries(id);
        }
        let e = &self.scene.elements[i];
        let lock = if e.locked { "Desbloquear" } else { "Bloquear" };
        // the common rows of every element: copy, duplicate, lock, then apagar
        let common = || {
            vec![
                item("copy_el", "Copiar elemento"),
                item("dup", "Duplicar"),
                item("lock", lock),
                item("front", "Trazer para Frente"),
                item("back", "Enviar para Trás"),
                Entry::Sep,
                red("delete", "Apagar"),
            ]
        };
        let mut rows = if let Some(t) = &e.terminal {
            // the reference app's order: name and focus, the buffer, monitoring
            vec![
                item("rename", "Renomear"),
                item("edit", "Editar…"),
                entry("unfocus", "Desfocar", None, false, self.terminal_focus.as_deref() == Some(id)),
                Entry::Sep,
                // off the Mac Ctrl+K stays the shell's, so no shortcut
                keyed("clear", "Limpar buffer", if cfg!(target_os = "macos") { "⌘K" } else { "" }),
                item("reload", "Recarregar"),
                entry("unload", "Descarregar", None, false, !t.unloaded),
                Entry::Sep,
                item("monitor", if t.quiet { "Ativar monitoramento" } else { "Desativar monitoramento" }),
                Entry::Sep,
            ]
        } else if e.created.is_some() {
            // a pile's page: the pile's own menu (each page's is on its tab)
            return if e.note.stacked { self.fichario_rows(i) } else { self.note_rows(i, id) };
        } else if let Some(p) = &e.portal {
            vec![
                item("rename", "Renomear"),
                item("edit", "Editar…"),
                item("reload", "Recarregar"),
                entry("unload", "Descarregar", None, false, !p.unloaded),
                item("open_browser", "Abrir no navegador"),
                item("chrome", if p.chrome_hidden { "Mostrar Barra" } else { "Ocultar Barra" }),
                sub(
                    "Mostrar Tamanho",
                    SizeReadout::ALL.iter().map(|s| tick(&format!("portal:size:{}", s.key()), s.label(), self.prefs.portal_size == *s)).collect(),
                ),
                entry("portal:shot", "Copiar Captura de Tela", None, false, cfg!(windows) && !p.unloaded),
                entry("portal:mute", if p.muted { "Ativar som" } else { "Silenciar" }, None, false, !p.unloaded),
                sub("Armazenamento", StorageScope::ALL.iter().map(|s| tick(&format!("portal:storage:{}", s.key()), s.label(), p.storage == *s)).collect()),
                Entry::Sep,
            ]
        } else if e.filetree.is_some() {
            vec![item("show_folder", "Mostrar na Pasta"), item("edit", "Alterar Diretório Raiz"), Entry::Sep]
        } else if e.ide.is_some() {
            vec![item("show_folder", "Mostrar na pasta"), Entry::Sep]
        } else {
            vec![]
        };
        rows.extend(self.disconnect_rows(id).unwrap_or_default());
        rows.extend(common());
        rows
    }

    /// A sticky note's menu, row for row the reference app's; a pile's page (its tab's menu) has the pile's rows instead of
    /// copy, duplicate and lock.
    pub(crate) fn note_rows(&self, i: usize, id: &str) -> Vec<Entry> {
        let e = &self.scene.elements[i];
        let paper = e.style.fill;
        let colors = NOTE_PAPERS.iter().enumerate().map(|(k, (c, name))| tick(&format!("color:{k}"), name, paper == Some(*c))).collect();
        let has_text = !self.note_text(i).trim().is_empty();
        let head = vec![
            item("rename", "Renomear"),
            item("lock_content", if e.note.content_locked { "Desbloquear Conteúdo" } else { "Bloquear Conteúdo" }),
            item("note_blur", if e.note.blurred { "Remover Desfoque" } else { "Desfocar" }),
            Entry::Sep,
            entry("copy_content", "Copiar Conteúdo", None, false, has_text),
            entry("show_folder", "Mostrar na Pasta", None, false, self.autosave.is_some()),
            entry("note_move", if e.note.file.is_some() { "Salvar no MagoAgent" } else { "Mover Arquivo para…" }, None, false, self.autosave.is_some()),
        ];
        if e.note.stacked {
            return [head, vec![sub("Cor", colors), Entry::Sep], self.stack_rows(i)].concat();
        }
        let lock = if e.locked { "Desbloquear" } else { "Bloquear" };
        let mut rows = [
            head,
            vec![
                item("copy_el", "Copiar Elemento"),
                item("dup", "Duplicar"),
                sub("Cor", colors),
                Entry::Sep,
                item("lock", lock),
                Entry::Sep,
                red("delete", "Apagar"),
            ],
        ]
        .concat();
        if let Some(cut) = self.disconnect_rows(id) {
            rows.splice(11..11, cut);
        }
        rows
    }

    /// The nodes `id` is joined to, one row each to cut it (ui/links.rs), and a separator.
    pub(crate) fn disconnect_rows(&self, id: &str) -> Option<Vec<Entry>> {
        let joined = self.links().remove(id).filter(|j| !j.is_empty())?;
        let nodes = self.agent_nodes();
        let cuts = joined
            .iter()
            .map(|other| item(&format!("disconnect:{other}"), &nodes.iter().find(|n| &n.id == other).map(|n| n.name.clone()).unwrap_or_default()))
            .collect();
        Some(vec![sub("Desconectar", cuts), Entry::Sep])
    }
}
