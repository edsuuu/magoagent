use super::*;

impl DrawingApp {
    /// Actions for the focused, enlarged or selected terminal or portal, under the toolbars.
    /// Returns the bar's rect so what comes below can make room.
    pub(crate) fn context_bar(&mut self, ctx: &Context, top: f32) -> Option<Rect> {
        let element = self.active_node()?.clone();
        let c = self.colors();
        let terminal = element.terminal.is_some();
        let note = element.created.is_some();
        let portal = element.portal.clone();
        let filetree = element.filetree.is_some();
        let chat = element.chat.is_some();
        let links = self.links().get(&element.id).map_or(0, Vec::len);
        let annotations = self.annotations.get(&element.id).map_or(0, Vec::len);
        let enlarged = self.terminal_elevated.as_ref() == Some(&element.id);
        let unread = self.terminal_unread.contains(&element.id);
        let (hopping, now) = (self.hop, self.now);
        let mut action = None;
        // the closures below can't write `action` while `button` holds it: they set these instead
        let (mut annotations_clicked, mut trash_clicked) = (false, false);
        let mut anchor = Rect::NOTHING;
        let mut badge = None;
        let size = element.note.font_size;
        let raw = element.note.raw;
        let index = self.scene.elements.iter().position(|e| e.id == element.id);
        // the toolbar's glass pill and buttons (it takes the toolbar's place over a focused window);
        // the glass goes under the row at last frame's size, as the area's own anchoring does
        let dark = self.dark();
        // centered over the canvas, as the toolbar is: the sidebar takes its left
        let middle = self.sidebar_width() / 2.0;
        let bar_id = context_bar_id(&element);
        let area = egui::Area::new(bar_id).anchor(egui::Align2::CENTER_TOP, (middle, top)).show(ctx, |ui| {
            let size_id = bar_id.with("size");
            let last = ui.data(|d| d.get_temp::<Vec2>(size_id));
            if let Some(size) = last {
                paint_glass(ui.painter(), Rect::from_min_size(ui.max_rect().min, size), GLASS_HEIGHT / 2.0, dark, false);
            }
            let margin = egui::Margin::symmetric(super::super::GLASS_END as i8, ((GLASS_HEIGHT - GLASS_BUTTON) / 2.0) as i8);
            let shown = egui::Frame::new().inner_margin(margin).show(ui, |ui| {
                ui.horizontal(|ui| {
                    ui.spacing_mut().item_spacing.x = GLASS_GAP;
                    let mut button = |ui: &mut egui::Ui, icon: Icon, on: bool, tip: &str, id: &'static str| {
                        let r = glass_button(ui, on, icon, dark);
                        let rect = r.rect;
                        if r.tip(tip).clicked() {
                            action = Some(id);
                            anchor = rect;
                        }
                        rect
                    };
                    if note {
                        // the note's bar: color, size, markdown, lists, raw view, more (ui/notes.rs)
                        button(ui, Icon::PaintPalette, self.note_color.is_some(), "Cor", "note_color");
                        glass_divider(ui, dark);
                        button(ui, Icon::Minus, false, "Diminuir tamanho da fonte", "font_down");
                        let (rect, _) = ui.allocate_exact_size(Vec2::new(22.0, 24.0), Sense::hover());
                        ui.painter().text(rect.center(), egui::Align2::CENTER_CENTER, size.round().to_string(), FontId::proportional(12.0), c.text);
                        button(ui, Icon::Plus, false, "Aumentar tamanho da fonte", "font_up");
                        glass_divider(ui, dark);
                        button(ui, Icon::Bold, false, "Negrito", "fmt:**");
                        button(ui, Icon::Italic, false, "Itálico", "fmt:*");
                        button(ui, Icon::Strike, false, "Tachado", "fmt:~~");
                        button(ui, Icon::Code, false, "Código", "fmt:`");
                        button(ui, Icon::Heading, false, "Título", "note_heading");
                        glass_divider(ui, dark);
                        button(ui, Icon::Todo, false, "Inserir tarefa", "fmt:- [ ] ");
                        button(ui, Icon::Bullet, false, "Inserir marcador", "fmt:- ");
                        button(ui, Icon::CodeBlock, false, "Bloco de código", "fmt:```");
                        glass_divider(ui, dark);
                        button(ui, Icon::Markdown, raw, if raw { "Formatado" } else { "Bruto" }, "note_raw");
                        button(ui, Icon::More, false, "Mais", "note_more");
                        glass_divider(ui, dark);
                    }
                    if filetree {
                        // the reference app's "Ferramentas da árvore": the folder, its root
                        button(ui, Icon::Open, false, "Mostrar na Pasta", "folder");
                        button(ui, Icon::Edit, false, "Alterar Diretório Raiz", "edit");
                        // rope it to a terminal or a chat: that agent then sees the tree
                        button(ui, Icon::Connect, links > 0, "Vincular a um terminal ou chat", "link");
                        button(ui, Icon::Expand, enlarged, if enlarged { "Sair da tela cheia" } else { "Tela cheia (clique duplo no título)" }, "expand");
                    } else if portal.is_some() {
                        button(ui, Icon::Restart, false, "Recarregar", "reload");
                    } else if chat {
                        // the card opens the conversation; what a chat is set to is chosen there
                        button(ui, Icon::Agents, false, "Abrir chat (clique duplo)", "chat");
                    } else if !note {
                        button(ui, Icon::Edit, false, "Editar", "edit");
                    }
                    if terminal {
                        glass_divider(ui, dark);
                        let composing = self.composer.as_ref().is_some_and(|(id, _)| id == &element.id);
                        let chat = button(ui, Icon::Agents, false, "Chat", "chat");
                        if unread {
                            // it finished while you looked elsewhere: the agents table's unread dot
                            ui.painter().circle(chat.right_top() + Vec2::new(-4.0, 4.0), 4.0, PRIMARY, Stroke::new(1.5, c.island));
                        }
                        button(ui, Icon::Composer, composing, "Compositor de prompts", "composer");
                        let searching = self.term_ui.find.as_ref().is_some_and(|f| f.id == element.id);
                        button(ui, Icon::Search, searching, &format!("Buscar no terminal ({})", crate::app::terminal::find::FIND_KEYS), "find");
                        glass_divider(ui, dark);
                    }
                    if !filetree {
                        button(ui, Icon::Connect, self.connecting.is_some(), "Conectar a outro terminal, chat, nota, navegador ou árvore", "connect");
                    }
                    if links > 0 {
                        // how many it reaches, a blue dot beside the button: its popover lists them
                        let (rect, r) = ui.allocate_exact_size(Vec2::new(15.0, 24.0), Sense::click());
                        ui.painter().circle_filled(rect.center(), 7.5, PRIMARY);
                        ui.painter().text(rect.center(), egui::Align2::CENTER_CENTER, links.to_string(), FontId::proportional(10.0), Color32::WHITE);
                        if r.on_hover_cursor(CursorIcon::PointingHand).tip("Gerenciar conexões").clicked() {
                            badge = Some(rect);
                        }
                    }
                    if terminal {
                        glass_divider(ui, dark);
                        button(ui, Icon::Restart, false, "Reiniciar", "restart");
                    }
                    if terminal {
                        let tip = if enlarged {
                            "Voltar ao canvas"
                        } else if terminal {
                            "Ampliar (clique duplo no título)"
                        } else {
                            "Abrir (clique duplo)"
                        };
                        button(ui, Icon::Expand, enlarged, tip, "expand");
                    }
                    if let Some(spec) = &portal {
                        // the reference app's order: reload, connect | design, annotations, edit, hide bar | close
                        glass_divider(ui, dark);
                        let armed = self.design.as_ref().is_some_and(|d| d.portal == element.id);
                        button(ui, Icon::Brush, armed, if armed { "Sair do Design" } else { "Design" }, "design");
                        if annotations > 0 {
                            // the count, as the reference app's badge
                            let (rect, r) = ui.allocate_exact_size(Vec2::new(16.0, 24.0), Sense::click());
                            ui.painter().circle_filled(rect.center(), 8.0, PRIMARY);
                            ui.painter().text(rect.center(), egui::Align2::CENTER_CENTER, annotations.to_string(), FontId::proportional(10.0), Color32::WHITE);
                            if r.on_hover_cursor(CursorIcon::PointingHand).tip("Anotações").clicked() {
                                annotations_clicked = true;
                            }
                        }
                        button(ui, Icon::Edit, false, "Editar", "edit");
                        button(ui, Icon::Panel, spec.chrome_hidden, if spec.chrome_hidden { "Mostrar barra" } else { "Ocultar barra" }, "chrome");
                    }
                    glass_divider(ui, dark);
                    if portal.is_some() {
                        // the reference app's destructive button: red, and it closes the portal
                        let (rect, r) = ui.allocate_exact_size(Vec2::splat(GLASS_BUTTON), Sense::click());
                        paint_icon(ui.painter(), Icon::Trash, rect.center(), Color32::from_rgb(0xf8, 0x51, 0x49));
                        if r.on_hover_cursor(CursorIcon::PointingHand).tip("Fechar navegador").clicked() {
                            trash_clicked = true;
                        }
                    } else {
                        let tip = match (filetree, chat) {
                            (true, _) => "Excluir árvore de arquivos",
                            (_, true) => "Tirar do canvas (a conversa continua na lista de chats)",
                            _ => "Apagar",
                        };
                        button(ui, Icon::Trash, false, tip, "delete");
                    }
                });
            });
            let size = shown.response.rect.size();
            if last != Some(size) {
                ui.data_mut(|d| d.insert_temp(size_id, size));
                ui.ctx().request_repaint();
            }
        });
        if annotations_clicked {
            action = Some("annotations");
        }
        if trash_clicked {
            action = Some("delete");
        }
        let id = element.id.clone();
        if let Some(rect) = badge {
            self.toggle_node_links(&id, rect);
        }
        if let Some(hop) = action {
            self.hop = Some((hop, self.now));
        }
        if hopping.is_some_and(|(_, at)| now - at < 0.4) {
            ctx.request_repaint();
        }
        match action {
            Some("edit") => {
                if let Some(spec) = element.terminal.clone() {
                    self.terminal_dialog = Some(TerminalDialog { target: Some(id), at: Rect::NOTHING, spec, tab: 0 });
                } else if let Some(spec) = portal {
                    self.portal_dialog = Some(PortalDialog { target: Some(id), at: Rect::NOTHING, spec });
                } else if let Some(spec) = element.filetree.clone() {
                    self.filetrees.dialog = Some(FileTreeDialog { target: Some(id), at: Rect::NOTHING, root: spec.root });
                }
            }
            Some("design") => self.toggle_design(&id),
            Some("annotations") => {
                if self.design.as_ref().is_some_and(|d| d.portal == id) {
                    self.portal_script(&id, "window.__magoDesign&&window.__magoDesign.panel()");
                } else {
                    self.arm_design(&id, true);
                }
            }
            Some("reload") => self.portal_reload(&id),
            Some("chrome") => {
                if let Some(i) = self.scene.elements.iter().position(|e| e.id == id) {
                    self.snapshot();
                    if let Some(spec) = self.scene.elements[i].portal.as_mut() {
                        spec.chrome_hidden = !spec.chrome_hidden;
                    }
                }
            }
            Some("claude") => self.claude_for_tree(&id),
            Some("folder") => {
                // the editor's root, with `~` expanded (none when the folder doesn't exist)
                if let Some(spec) = &element.filetree {
                    open_external(&spec.shown().to_string_lossy());
                }
            }
            Some("note_color") => {
                self.note_color = if self.note_color.is_some() { None } else { Some((anchor.left_bottom() + Vec2::new(0.0, 6.0), Rect::NOTHING)) };
            }
            Some("note_heading") | Some("note_more") => {
                let (items, at) = if action == Some("note_heading") {
                    (vec![item("fmt:# ", "#  Título 1"), item("fmt:## ", "##  Título 2"), item("fmt:### ", "###  Título 3")], anchor)
                } else {
                    let locked = element.note.content_locked;
                    (
                        vec![
                            item("rename", "Renomear nota"),
                            item("copy_content", "Copiar Conteúdo"),
                            item("lock_content", if locked { "Desbloquear Conteúdo" } else { "Bloquear Conteúdo" }),
                        ],
                        anchor,
                    )
                };
                self.context = Some(ContextMenu::new(at.left_bottom() + Vec2::new(0.0, 6.0), Target::Element(id.clone()), Some(items)));
            }
            Some("note_raw") => {
                if let Some(i) = index {
                    self.snapshot();
                    self.scene.elements[i].note.raw = !raw;
                }
            }
            Some("font_down") | Some("font_up") => {
                if let Some(i) = index {
                    self.note_font_step(i, if action == Some("font_up") { NOTE_FONT_STEP } else { -NOTE_FONT_STEP });
                }
            }
            Some(op) if op.starts_with("fmt:") => {
                if let Some(i) = index {
                    self.note_format(ctx, i, &op[4..]);
                }
            }
            Some("connect") => {
                if self.connecting.is_some() {
                    self.connecting = None;
                } else {
                    self.start_connect(id);
                }
            }
            Some("link") => {
                let rows = self.link_rows(&id);
                if rows.is_empty() {
                    self.toast("Não há terminal nem chat no canvas para vincular.");
                } else {
                    self.context = Some(ContextMenu::new(anchor.left_bottom() + Vec2::new(0.0, 6.0), Target::Element(id), Some(rows)));
                }
            }
            Some("composer") => self.toggle_composer(id),
            Some("find") => {
                let seed = match self.terminals.get(&id) {
                    Some(Ok(session)) => session.handle.selected_text(),
                    _ => String::new(),
                };
                self.open_find(&id, Some(seed));
            }
            Some("chat") => self.chat_on(&id),
            Some("restart") => self.restart_node(id),
            Some("expand") => self.set_elevated(if enlarged { None } else { Some(id) }),
            Some("delete") => {
                if let Some(i) = self.scene.elements.iter().position(|e| e.id == id) {
                    self.selected = vec![i];
                    self.delete_selected();
                }
            }
            _ => {}
        }
        Some(area.response.rect)
    }
}
