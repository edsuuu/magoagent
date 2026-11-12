//! The bar over the active terminal (or portal) and the new / edit terminal dialog.

use crate::app::ui::context::{ContextMenu, Target, item};
use crate::prelude::*;

/// A prompt that never reached terminal `id`, and why: its words come back to be sent again.
pub(crate) struct Returned {
    pub(crate) id: String,
    pub(crate) text: String,
    pub(crate) why: String,
}

/// The bar's area, one per kind of node: egui centers an area by the size it had last frame, so a
/// bar that kept another kind's width (a note's 535 under an IDE's 168) was drawn off its center
/// for a frame.
// ponytail: within a kind the bar still grows by a badge (links, annotations) and is half of it off for a frame; give the row its width up front if that shows
pub(crate) fn context_bar_id(element: &Element) -> egui::Id {
    let kind = match element {
        e if e.created.is_some() => "note",
        e if e.filetree.is_some() => "tree",
        e if e.portal.is_some() => "portal",
        e if e.ide.is_some() => "ide",
        e if e.chat.is_some() => "chat",
        _ => "terminal",
    };
    egui::Id::unique(("context_bar", kind))
}

impl DrawingApp {
    /// Two or more notes selected (or a pile and a note): the reference app's bar with the
    /// fichário button, which piles them up (Colocar / Adicionar ao Fichário).
    pub(crate) fn notes_bar(&mut self, ctx: &Context, top: f32) -> Option<Rect> {
        let loose = self.loose_selected_notes().len();
        let piles = self.selected.iter().filter(|i| self.scene.elements.get(**i).is_some_and(|e| e.note.stacked && e.note.front)).count();
        let tip = match (piles, loose) {
            (0, 2..) => "Colocar no Fichário",
            (1, 1..) => "Adicionar ao Fichário",
            _ => return None,
        };
        let dark = self.dark();
        let c = self.colors();
        let middle = self.sidebar_width() / 2.0;
        let notes = self.loose_selected_notes();
        let size = notes.first().map_or(NOTE_FONT_MIN, |n| self.scene.elements[*n].note.font_size);
        let (mut stack, mut color, mut step) = (None, None, 0.0);
        let area = egui::Area::new(egui::Id::new("notes_bar")).anchor(egui::Align2::CENTER_TOP, (middle, top)).show(ctx, |ui| {
            let size_id = egui::Id::new("notes_bar_size");
            if let Some(size) = ui.data(|d| d.get_temp::<Vec2>(size_id)) {
                paint_glass(ui.painter(), Rect::from_min_size(ui.max_rect().min, size), GLASS_HEIGHT / 2.0, dark, false);
            }
            let margin = egui::Margin::symmetric(super::GLASS_END as i8, ((GLASS_HEIGHT - GLASS_BUTTON) / 2.0) as i8);
            let shown = egui::Frame::new().inner_margin(margin).show(ui, |ui| {
                ui.horizontal(|ui| {
                    ui.spacing_mut().item_spacing.x = GLASS_GAP;
                    // as the one note's bar: color and size go to every selected note
                    let r = glass_button(ui, self.note_color.is_some(), Icon::PaintPalette, dark);
                    if r.clone().tip("Cor").clicked() {
                        color = Some(r.rect);
                    }
                    glass_divider(ui, dark);
                    if glass_button(ui, false, Icon::Minus, dark).tip("Diminuir tamanho da fonte").clicked() {
                        step = -NOTE_FONT_STEP;
                    }
                    let (rect, _) = ui.allocate_exact_size(Vec2::new(22.0, 24.0), Sense::hover());
                    ui.painter().text(rect.center(), egui::Align2::CENTER_CENTER, size.round().to_string(), FontId::proportional(12.0), c.text);
                    if glass_button(ui, false, Icon::Plus, dark).tip("Aumentar tamanho da fonte").clicked() {
                        step = NOTE_FONT_STEP;
                    }
                    glass_divider(ui, dark);
                    stack = glass_button(ui, false, Icon::Layers(false), dark).tip(tip).clicked().then_some(());
                });
            });
            ui.data_mut(|d| d.insert_temp(size_id, shown.response.rect.size()));
        });
        if let Some(anchor) = color {
            self.note_color = if self.note_color.is_some() { None } else { Some((anchor.left_bottom() + Vec2::new(0.0, 6.0), Rect::NOTHING)) };
        }
        if step != 0.0 {
            self.notes_font_step(&notes, step);
        }
        if stack.is_some() {
            self.stack_notes();
        }
        Some(area.response.rect)
    }

    /// Actions for the focused, enlarged or selected terminal or portal, under the toolbars.
    /// Returns the bar's rect so what comes below can make room.
    pub(crate) fn context_bar(&mut self, ctx: &Context, top: f32) -> Option<Rect> {
        let element = self.active_node()?.clone();
        let c = self.colors();
        let terminal = element.terminal.is_some();
        let note = element.created.is_some();
        let portal = element.portal.clone();
        let ide = element.ide.clone();
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
            let margin = egui::Margin::symmetric(super::GLASS_END as i8, ((GLASS_HEIGHT - GLASS_BUTTON) / 2.0) as i8);
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
                        button(ui, Icon::Connect, self.connecting.is_some(), "Conectar a outro terminal, chat, nota, navegador ou IDE", "connect");
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
                    if ide.is_some() {
                        button(ui, Icon::Open, false, "Mostrar na pasta", "folder");
                    }
                    if terminal || ide.is_some() {
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
                } else if let Some(spec) = ide {
                    self.ide_dialog = Some(IdeDialog { target: Some(id), at: Rect::NOTHING, spec });
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
            Some("claude") => self.claude_for_ide(&id),
            Some("folder") => {
                // the editor's root, with `~` expanded (none when the folder doesn't exist)
                if let Some(root) = self.ides.get(&id).and_then(|i| i.root.as_ref()) {
                    open_external(&root.to_string_lossy());
                } else if let Some(spec) = &element.filetree {
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

    /// Reiniciar: the shell (and its command) starts again; an unloaded one starts too.
    pub(crate) fn restart_node(&mut self, id: String) {
        if let Some(i) = self.scene.elements.iter().position(|e| e.id == id) {
            self.unload_node(i, false);
        }
        self.terminals.remove(&id);
        self.terminal_focus = Some(id);
    }

    /// A message box under the terminal, like the reference app's docked one: as wide as the
    /// terminal allows (up to 560), growing down with the text until 220 tall, then it scrolls.
    /// Enter sends it as a pasted prompt plus Enter (like `magoagent ask`, without waiting),
    /// Shift+Enter breaks the line, Esc closes.
    pub(crate) fn composer_ui(&mut self, ctx: &Context) {
        let Some((id, mut text)) = self.composer.clone() else { return };
        let Some(element) = self.scene.elements.iter().find(|e| e.id == id).cloned() else {
            self.composer = None;
            return;
        };
        // the chat screen covers the canvas: the composer waits under it, without the keys; so it
        // does under a dialog, the palette or the export, which type in fields of their own
        if self.chat_open || self.dialog_open() || self.palette_open {
            return;
        }
        let (frame, _) = self.terminal_frame(&element);
        let width = (frame.width() - 24.0).min(560.0);
        // too small on screen to fit under it: it waits, open, until the terminal is bigger
        if width < 160.0 {
            return;
        }
        // clear of the dashed outline around the terminal, and of the window's bottom; with the
        // terminal panned away it stays in view at the edge, where the keys it has go
        let canvas = self.canvas_rect;
        let top = (frame.bottom() + 14.0).min(canvas.bottom() - 64.0).max(canvas.top() + 8.0);
        let half = width / 2.0 + 8.0;
        let x = frame.center().x.min(canvas.right() - half).max(canvas.left() + half);
        let tallest = (canvas.bottom() - top - 24.0).clamp(20.0, 220.0);
        let name = element.terminal.as_ref().map_or("terminal".into(), |t| t.name.clone());
        let (mut send, mut close) = (false, false);
        egui::Area::new(egui::Id::unique("composer")).fixed_pos(Pos2::new(x, top)).pivot(egui::Align2::CENTER_TOP).order(egui::Order::Foreground).show(
            ctx,
            |ui| {
                // a pill, like a chat box; the button stays on the last line as the text grows
                // the send button sits 4 from the top, the right and the bottom; one line of text in
                // the middle of its height
                self.island().corner_radius(22).inner_margin(egui::Margin { left: 18, right: 4, top: 4, bottom: 4 }).show(ui, |ui| {
                    ui.set_width(width - 22.0);
                    // an Area offers only last frame's size; room to grow up to the cap
                    ui.set_max_height(tallest);
                    let response = egui::ScrollArea::vertical()
                        .id_salt("composer_scroll")
                        .max_height(tallest)
                        .show(ui, |ui| {
                            let edit = egui::TextEdit::multiline(&mut text)
                                .hint_text(format!("Mensagem para {name} · Esc para fechar"))
                                .desired_rows(1)
                                .margin(egui::Margin::symmetric(0, 6))
                                .min_size(Vec2::new(0.0, 32.0))
                                .align(egui::Align2::LEFT_CENTER)
                                .desired_width(width - 64.0)
                                .return_key(Some(egui::KeyboardShortcut::new(Modifiers::SHIFT, Key::Enter)))
                                .frame(egui::Frame::NONE);
                            ui.add(edit)
                        })
                        .inner;
                    if !response.has_focus() && !response.lost_focus() {
                        response.request_focus();
                    }
                    // egui's own focus, not has_focus(): that one also asks for the window's
                    let typing = ui.memory(|m| m.has_focus(response.id));
                    send = typing
                        && ui.input(|i| {
                            i.events.iter().any(|e| matches!(e, egui::Event::Key { key: Key::Enter, pressed: true, modifiers, .. } if !modifiers.shift))
                        });
                    close = ui.input(|i| i.key_pressed(Key::Escape));
                    // over the right end of the last line: a row would cap the text's height
                    let inner = ui.min_rect();
                    let rect = Rect::from_min_size(inner.max - Vec2::splat(32.0), Vec2::splat(32.0));
                    let button = ui.interact(rect, ui.scope_id().with("composer_send"), Sense::click());
                    let ready = !text.trim().is_empty();
                    let fill = match (ready, button.hovered()) {
                        (false, _) => Color32::from_gray(0x55),
                        (true, true) => PRIMARY.gamma_multiply(1.15),
                        (true, false) => PRIMARY,
                    };
                    ui.painter().circle_filled(rect.center(), 16.0, fill);
                    paint_icon(ui.painter(), Icon::Tool(Tool::Arrow), rect.center(), Color32::WHITE);
                    send |= button.on_hover_cursor(CursorIcon::PointingHand).tip("Enviar").clicked();
                });
            },
        );
        if close {
            self.composer = None;
            return;
        }
        if send && !text.trim().is_empty() && self.send_prompt(&id, &text) {
            text.clear();
        }
        self.composer = Some((id, text));
    }

    /// The bar's composer button: opens it under the terminal, with what was left in it, or closes it.
    pub(crate) fn toggle_composer(&mut self, id: String) {
        let open = self.composer.as_ref().is_some_and(|(open, _)| open == &id);
        self.composer = if open {
            None
        } else {
            let draft = self.drafts.remove(&id).unwrap_or_default();
            Some((id, draft))
        };
    }

    /// Types `text` into a terminal as a prompt, off the UI thread (the composer, the chat and
    /// the Design mode use it). False, with a toast saying why, when it would land somewhere else.
    pub(crate) fn send_prompt(&mut self, id: &str, text: &str) -> bool {
        let name = self.terminal_name(id);
        match self.terminals.get(id) {
            Some(Ok(session)) => match session.handle.ready() {
                Ok(()) => {
                    self.send_prompt_after(id, text, std::time::Duration::ZERO);
                    true
                }
                Err(why) => {
                    self.toast(format!("{name} {why}"));
                    false
                }
            },
            _ => {
                self.toast(format!("{name} não está rodando: abra o terminal e envie de novo."));
                false
            }
        }
    }

    /// `send_prompt` after a wait: the IDE's comments give Claude Code time to drop its diff
    /// prompt, so readiness is asked then, a few times. A refusal gives the words back
    /// (`Returned`).
    pub(crate) fn send_prompt_after(&self, id: &str, text: &str, delay: std::time::Duration) {
        if let Some(Ok(session)) = self.terminals.get(id) {
            let (handle, message, name, tx, id) =
                (session.handle.clone(), text.trim().to_string(), self.terminal_name(id), self.returned_prompts.0.clone(), id.to_string());
            std::thread::spawn(move || {
                std::thread::sleep(delay);
                let tries = if delay.is_zero() { 1 } else { 12 };
                let mut result = Err(String::new());
                for _ in 0..tries {
                    result = handle.type_prompt(&message);
                    if result.is_ok() {
                        return;
                    }
                    std::thread::sleep(std::time::Duration::from_millis(250));
                }
                if let Err(why) = result {
                    let _ = tx.send(Returned { id, text: message, why: format!("{name} {why}") });
                }
            });
        }
    }

    /// The prompts `send_prompt_after` could not type: into their composer when that is free or
    /// already theirs, else onto the clipboard.
    pub(crate) fn returned_prompts(&mut self, ctx: &Context) {
        let returned: Vec<Returned> = self.returned_prompts.1.try_iter().collect();
        for Returned { id, text, why } in returned {
            let on_screen = self.scene.elements.iter().any(|e| e.id == id);
            match self.composer.as_mut() {
                Some((on, draft)) if *on == id => {
                    if !draft.trim().is_empty() {
                        draft.push_str("\n\n");
                    }
                    draft.push_str(&text);
                    self.toast(format!("{why} O texto voltou para a caixa de mensagem do terminal."));
                }
                None if on_screen => {
                    self.composer = Some((id, text));
                    self.toast(format!("{why} O texto voltou para a caixa de mensagem do terminal."));
                }
                _ => {
                    ctx.copy_text(text);
                    self.toast(format!("{why} O texto foi copiado: cole e envie de novo."));
                }
            }
        }
    }

    /// A terminal's name as people see it ("O terminal" when it has none).
    pub(crate) fn terminal_name(&self, id: &str) -> String {
        let spec = self.scene.elements.iter().find(|e| e.id == id).and_then(|e| e.terminal.as_ref());
        spec.map(|t| t.name.trim().to_string()).filter(|n| !n.is_empty()).unwrap_or_else(|| "O terminal".into())
    }

    /// The note bar's color popover: the ten papers, and a custom one through the color picker
    /// (inline, ui/hsv.rs). A press outside it, Esc or a pick closes it.
    pub(crate) fn note_color_ui(&mut self, ctx: &Context) {
        let Some((at, last)) = self.note_color else { return };
        // the focused note, or every loose note of a selection of several (the notes bar)
        let id = self.active_node().filter(|e| e.created.is_some()).map(|e| e.id.clone());
        let notes = match id.and_then(|id| self.scene.elements.iter().position(|e| e.id == id)) {
            Some(i) => vec![i],
            None => self.loose_selected_notes(),
        };
        let Some(&i) = notes.first() else {
            self.note_color = None;
            return;
        };
        let pressed_outside = ctx.input(|i| i.pointer.any_pressed() && i.pointer.interact_pos().is_some_and(|p| !last.contains(p)));
        if ctx.input(|i| i.key_pressed(Key::Escape)) || (pressed_outside && last != Rect::NOTHING) {
            self.note_color = None;
            return;
        }
        let c = self.colors();
        let fill = self.scene.elements[i].style.fill.unwrap_or(STICKY_PAPER);
        let (mut picked, mut custom, dark) = (None, fill, self.dark());
        let area = egui::Area::new(egui::Id::unique("note_color")).fixed_pos(at).order(egui::Order::Foreground).show(ctx, |ui| {
            self.island().inner_margin(10).show(ui, |ui| {
                ui.set_width(160.0);
                dialog_caption(ui, "Cores predefinidas", &c);
                ui.add_space(4.0);
                ui.horizontal_wrapped(|ui| {
                    ui.spacing_mut().item_spacing = Vec2::splat(6.0);
                    for (paper, name) in NOTE_PAPERS {
                        if swatch(ui, paper, fill == paper, 22.0, dark).tip(name).clicked() {
                            picked = Some(paper);
                        }
                    }
                });
                ui.add_space(8.0);
                dialog_caption(ui, "Cor personalizada", &c);
                // inline: the egui button's popup is a window of its own, so a press in it counted as
                // "outside" and closed this one. The Hsva stays in memory (a gray would lose its hue).
                let key = egui::Id::unique("note_hsva");
                let mut hsva = ui
                    .data(|d| d.get_temp::<(egui::ecolor::Hsva, [u8; 4])>(key))
                    .filter(|(_, last)| *last == custom)
                    .map_or_else(|| egui::ecolor::Hsva::from(rgba(fill)), |(h, _)| h);
                if super::hsv::hsv_picker(ui, &mut hsva, 160.0) {
                    custom = hsva.to_srgba_unmultiplied();
                    ui.data_mut(|d| d.insert_temp(key, (hsva, custom)));
                }
            });
        });
        self.note_color = Some((at, area.response.rect));
        let chosen = picked.or((custom != fill).then_some(custom));
        if let Some(paper) = chosen {
            self.snapshot();
            for &n in &notes {
                self.scene.elements[n].style.fill = Some(paper);
            }
        }
        if picked.is_some() {
            self.note_color = None;
        }
    }
}
