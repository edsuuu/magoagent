//! IDE nodes on the canvas: the folder dialog, the card painted while one sits on the canvas, the
//! live editor (src/ide) while it is enlarged, and the Claude Code terminals roped to it.

use crate::ide::{Ide, icons, theme as t};
use crate::prelude::*;

/// The new / edit IDE dialog.
#[derive(Clone, Debug)]
pub(crate) struct IdeDialog {
    /// The IDE being edited; None creates one in `at`.
    pub(crate) target: Option<String>,
    pub(crate) at: Rect,
    pub(crate) spec: IdeSpec,
}

impl DrawingApp {
    /// Click: an IDE card centered there; drag: that box. Asks for the folder first.
    pub(crate) fn open_ide_dialog(&mut self, start: Pos2, end: Pos2) {
        let at = if start.distance(end) * self.zoom < 10.0 {
            Rect::from_center_size(start, IDE_SIZE)
        } else {
            let r = Rect::from_two_pos(start, end);
            Rect::from_min_size(r.min, r.size().max(Vec2::new(280.0, 200.0)))
        };
        let folder = self.file_path.as_ref().and_then(|p| p.parent()).map(|p| p.to_string_lossy().to_string());
        self.ide_dialog = Some(IdeDialog { target: None, at, spec: IdeSpec { name: String::new(), root: folder.unwrap_or_default() } });
        if !self.tool_locked {
            self.tool = Tool::Select;
        }
    }

    pub(crate) fn confirm_ide_dialog(&mut self) {
        let Some(IdeDialog { target, at, mut spec }) = self.ide_dialog.take() else { return };
        spec.root = spec.root.trim().to_string();
        if spec.name.trim().is_empty() {
            let folder = std::path::Path::new(&spec.root).file_name().map(|n| n.to_string_lossy().to_string());
            spec.name = folder.unwrap_or_else(|| "IDE".into());
        }
        self.snapshot();
        if let Some(id) = target {
            if let Some(e) = self.scene.elements.iter_mut().find(|e| e.id == id) {
                e.ide = Some(spec);
            }
            return;
        }
        let mut ide = self.new_element(ElementKind::Rectangle { x: at.min.x, y: at.min.y, width: at.width(), height: at.height() });
        // what exports and the AI's view show; the app paints the project card instead
        ide.style = node_style(ide.style);
        ide.ide = Some(spec);
        self.scene.elements.push(ide);
        self.selected = vec![self.scene.elements.len() - 1];
    }

    pub(crate) fn ide_dialog_ui(&mut self, ctx: &Context) {
        let Some(mut dialog) = self.ide_dialog.clone() else { return };
        let c = self.colors();
        let (mut open, mut confirm, mut cancel) = (true, false, false);
        let editing = dialog.target.is_some();
        egui::Window::new(if editing { "Editar IDE" } else { "Novo IDE" })
            .open(&mut open)
            .collapsible(false)
            .resizable(false)
            .anchor(egui::Align2::CENTER_CENTER, Vec2::ZERO)
            .show(ctx, |ui| {
                ui.set_width(400.0);
                egui::Grid::new("ide_fields").num_columns(2).spacing([12.0, 8.0]).show(ui, |ui| {
                    ui.label("Pasta");
                    ui.horizontal(|ui| {
                        ui.add(egui::TextEdit::singleline(&mut dialog.spec.root).hint_text("a pasta do projeto").desired_width(200.0));
                        if ui.button("Procurar…").clicked() {
                            if let Some(dir) = rfd::FileDialog::new().set_title("Pasta do projeto").pick_folder() {
                                dialog.spec.root = dir.to_string_lossy().into();
                            }
                        }
                    });
                    ui.end_row();
                    ui.label("Nome");
                    ui.add(egui::TextEdit::singleline(&mut dialog.spec.name).hint_text("opcional: o nome da pasta").desired_width(290.0));
                    ui.end_row();
                });
                ui.add_space(4.0);
                let note = "Clique duas vezes no IDE para abri-lo. Um terminal ligado a ele por Conectar roda o Claude Code \
                    integrado: as mudanças propostas aparecem como diff no IDE.";
                ui.label(egui::RichText::new(note).size(11.0).color(c.muted));
                ui.add_space(8.0);
                ui.horizontal(|ui| {
                    cancel = ui.button("Cancelar").clicked();
                    let label = egui::RichText::new(if editing { "Salvar" } else { "Criar" }).color(Color32::WHITE);
                    confirm = ui.add(egui::Button::new(label).fill(PRIMARY)).clicked();
                });
                confirm |= ui.input(|i| i.key_pressed(Key::Enter));
            });
        if !open || cancel {
            self.ide_dialog = None;
            return;
        }
        self.ide_dialog = Some(dialog);
        if confirm {
            self.confirm_ide_dialog();
        }
    }

    /// Folders dropped on the canvas become IDEs, side by side from where they land.
    pub(crate) fn drop_folders(&mut self, ctx: &Context, dropped: &[PathBuf]) {
        let at = ctx.input(|i| i.pointer.hover_pos()).unwrap_or(self.canvas_rect.center());
        let mut x = self.to_world(at).x;
        for dir in dropped.iter().filter(|p| p.is_dir()) {
            let spec = IdeSpec { name: String::new(), root: dir.to_string_lossy().into() };
            let at = Rect::from_min_size(Pos2::new(x, self.to_world(at).y), IDE_SIZE);
            self.ide_dialog = Some(IdeDialog { target: None, at, spec });
            self.confirm_ide_dialog();
            x += IDE_SIZE.x + 40.0;
        }
    }

    /// One editor per IDE element: new ones open, removed or re-pointed ones save and close
    /// (`Drop`: their Claude Code lock file and language servers go with them). All of them answer
    /// Claude Code every frame.
    pub(crate) fn sync_ides(&mut self, ctx: &Context) {
        let live: Vec<(String, String)> = self.scene.elements.iter().filter_map(|e| Some((e.id.clone(), e.ide.as_ref()?.root.clone()))).collect();
        self.ides.retain(|id, ide| live.iter().any(|(live, root)| live == id && *root == ide.folder));
        for (id, root) in live {
            if !self.ides.contains_key(&id) {
                let ide = Ide::new(ctx.clone(), &id, &root);
                self.ides.insert(id, ide);
            }
        }
        let mut notice = None;
        for (id, ide) in &mut self.ides {
            ide.background(ctx);
            // what an IDE off screen says (a diff Claude Code proposes) shows on the canvas
            if let Some(message) = ide.notice.take().filter(|_| self.terminal_elevated.as_ref() != Some(id)) {
                let name = self.scene.elements.iter().find(|e| &e.id == id).and_then(|e| e.ide.as_ref()).map_or(String::new(), |s| s.name.clone());
                notice = Some(format!("{name}: {message}"));
            }
        }
        if let Some(message) = notice {
            self.toast(message);
        }
        if self.ide_focus.is_some() && self.ide_focus != self.terminal_elevated {
            self.ide_focus = None;
        }
    }

    /// `set_elevated` going from `old` to `new`: an IDE leaving saves its files, one arriving
    /// takes the keyboard.
    pub(crate) fn ide_elevated(&mut self, old: Option<String>, new: &Option<String>) {
        if let Some(ide) = old.and_then(|id| self.ides.get_mut(&id)) {
            ide.save_all();
        }
        self.ide_focus = new.clone().filter(|id| self.scene.elements.iter().any(|e| &e.id == id && e.ide.is_some()));
        if self.ide_focus.is_some() {
            self.terminal_focus = None;
        }
    }

    pub(crate) fn double_click_ide(&mut self, index: usize) {
        self.moving = false;
        self.marquee = None;
        self.selected = vec![index];
        self.set_elevated(Some(self.scene.elements[index].id.clone()));
    }

    /// Claude Code in a terminal roped to an IDE connects to it. Only when the shell starts: a
    /// terminal roped afterwards needs Reiniciar (or `/ide` inside claude).
    pub(crate) fn ide_env(&self, terminal: &str) -> Vec<(String, String)> {
        let links = self.links();
        links.get(terminal).into_iter().flatten().filter_map(|id| self.ides.get(id)).flat_map(Ide::claude_env).collect()
    }

    /// The IDE's Claude Code button: the `claude` terminal roped to it, made beside it in its
    /// folder the first time (roped before it starts, so it connects), back on the canvas and typing.
    pub(crate) fn claude_for_ide(&mut self, ide: &str) {
        let Some(element) = self.scene.elements.iter().find(|e| e.id == ide).cloned() else { return };
        let is_claude = |id: &String| self.scene.elements.iter().any(|e| &e.id == id && e.terminal.as_ref().is_some_and(|t| t.command.trim_start().starts_with("claude")));
        let roped = self.links().get(ide).and_then(|l| l.iter().find(|id| is_claude(id)).cloned());
        let terminal = match roped {
            Some(id) => id,
            None => {
                let b = local_bounds(&element);
                let at = Rect::from_min_size(Pos2::new(b.max.x + 60.0, b.min.y), terminal_size(100, 30));
                let cwd = element.ide.map(|s| s.root).unwrap_or_default();
                let spec = TerminalSpec { name: "Claude Code".into(), cwd, command: "claude".into() };
                self.terminal_dialog = Some(TerminalDialog { target: None, at, spec });
                self.confirm_terminal_dialog();
                let Some(id) = self.terminal_focus.clone() else { return };
                // one Cmd+Z takes back the terminal and its rope
                self.rope_between(ide, &id, false);
                id
            }
        };
        self.set_elevated(None);
        self.selected = self.scene.elements.iter().position(|e| e.id == terminal).into_iter().collect();
        self.terminal_focus = Some(terminal);
    }

    /// The card on the canvas: title strip (folder, name, branch), the top of the project tree
    /// and the folder. The editor itself only runs enlarged.
    pub(crate) fn paint_ide(&self, painter: &Painter, element: &Element, rect: Rect, zoom: f32) {
        let spec = element.ide.clone().unwrap_or_default();
        let ide = self.ides.get(&element.id);
        let radius = (8.0 * zoom).min(255.0) as u8;
        painter.rect(rect, radius, t::ISLAND, Stroke::new(1.0, Color32::from_gray(0x3a)), StrokeKind::Inside);
        let painter = painter.with_clip_rect(rect.shrink(1.0).intersect(painter.clip_rect()));
        let header = Rect::from_min_size(rect.min, Vec2::new(rect.width(), TERMINAL_HEADER * zoom));
        painter.rect_filled(header, egui::CornerRadius { nw: radius, ne: radius, sw: 0, se: 0 }, t::FRAME);
        let folder = header.left_center() + Vec2::new(TERMINAL_PAD + 6.0, 0.0) * zoom;
        icons::paint(&painter, icons::FOLDER, Rect::from_center_size(folder, Vec2::splat(14.0 * zoom)));
        let name = if spec.name.is_empty() { "IDE" } else { &spec.name };
        let name_rect = painter.text(folder + Vec2::new(12.0 * zoom, 0.0), egui::Align2::LEFT_CENTER, name, FontId::proportional(12.0 * zoom), t::TEXT);
        if let Some(branch) = ide.map(|i| i.branch.as_str()).filter(|b| !b.is_empty()) {
            let at = name_rect.right_center() + Vec2::new(16.0 * zoom, 0.0);
            icons::paint(&painter, icons::BRANCH, Rect::from_center_size(at, Vec2::splat(13.0 * zoom)));
            painter.text(at + Vec2::new(10.0 * zoom, 0.0), egui::Align2::LEFT_CENTER, branch, FontId::proportional(11.0 * zoom), t::MUTED);
        }
        if self.terminal_elevated.as_ref() == Some(&element.id) {
            paint_icon(&painter, Icon::Expand, Self::restore_button(rect).center(), t::TEXT);
        }
        // the project's first entries, as in its tree
        let footer = Rect::from_min_max(Pos2::new(rect.left(), rect.bottom() - TERMINAL_FOOTER * zoom), rect.max);
        let row = 22.0 * zoom;
        let mut y = header.bottom() + 8.0 * zoom + row / 2.0;
        for (entry, dir) in ide.map_or(&[][..], |i| &i.preview[..]) {
            if y + row / 2.0 > footer.top() {
                break;
            }
            let icon = Pos2::new(rect.left() + 24.0 * zoom, y);
            if *dir {
                icons::paint(&painter, icons::FOLDER, Rect::from_center_size(icon, Vec2::splat(16.0 * zoom)));
            } else {
                icons::file(&painter, std::path::Path::new(entry), icon, zoom);
            }
            painter.text(icon + Vec2::new(14.0 * zoom, 0.0), egui::Align2::LEFT_CENTER, entry, t::ui(13.0 * zoom), t::TEXT);
            y += row;
        }
        painter.rect_filled(footer, egui::CornerRadius { nw: 0, ne: 0, sw: radius, se: radius }, t::FRAME);
        let small = FontId::proportional(11.0 * zoom);
        let note = if ide.is_some_and(Ide::claude_connected) { "✻ Claude Code conectado" } else { "Clique duas vezes para abrir" };
        let note = painter.text(footer.right_center() - Vec2::new(TERMINAL_PAD * zoom, 0.0), egui::Align2::RIGHT_CENTER, note, small.clone(), t::MUTED);
        // the folder gives way to the note on its right
        let room = Rect::from_min_max(footer.min, Pos2::new(note.left() - TERMINAL_PAD * zoom, footer.bottom())).intersect(painter.clip_rect());
        painter.with_clip_rect(room).text(footer.left_center() + Vec2::new(TERMINAL_PAD * zoom, 0.0), egui::Align2::LEFT_CENTER, display_folder(&spec.root), small, t::MUTED);
    }

    /// ⌘3: the IDE screen. The enlarged IDE, else the first on the canvas, opened as a double-click
    /// opens it; already open, it only takes the keyboard back (a dialog or the chat had it).
    pub(crate) fn open_ide(&mut self) {
        self.finish_text();
        let is_ide = |e: &Element| e.ide.is_some();
        let elevated = self.terminal_elevated.as_ref().and_then(|id| self.scene.elements.iter().position(|e| &e.id == id && is_ide(e)));
        let Some(index) = elevated.or_else(|| self.scene.elements.iter().position(is_ide)) else {
            self.toast("Nenhum IDE no canvas: crie um com a ferramenta IDE da barra");
            return;
        };
        self.chat_open = false;
        self.double_click_ide(index);
        self.ide_focus = Some(self.scene.elements[index].id.clone());
    }

    /// The enlarged IDE above the canvas: the card while it grows, then the live editor at 100%.
    /// It has the keyboard until a click lands elsewhere; a press on the canvas margin goes back.
    pub(crate) fn ide_overlay(&mut self, ctx: &Context) {
        let elevated = self.terminal_elevated.as_ref().and_then(|id| self.scene.elements.iter().find(|e| &e.id == id && e.ide.is_some()));
        let Some(element) = elevated.cloned() else { return };
        let id = element.id.clone();
        let (rect, zoom) = self.terminal_frame(&element);
        let press = ctx.input(|i| i.pointer.any_pressed().then(|| i.pointer.interact_pos()).flatten());
        if press.is_some_and(|p| !rect.contains(p) && ctx.layer_id_at(p).is_none_or(|l| l.order == egui::Order::Background)) {
            self.set_elevated(None);
            return;
        }
        let covered = self.palette_open || self.export_open || self.terminal_dialog.is_some() || self.portal_dialog.is_some() || self.ide_dialog.is_some();
        if covered {
            self.ide_focus = None;
        } else if press.is_some_and(|p| rect.contains(p)) {
            self.ide_focus = Some(id.clone());
        }
        let focused = self.ide_focus.as_ref() == Some(&id);
        let animating = self.elevate_animating();
        let has_agent = !self.roped_agents(&id).is_empty();
        egui::Area::new(egui::Id::unique(("ide_overlay", id.as_str()))).fixed_pos(rect.min).order(egui::Order::Middle).show(ctx, |ui| {
            ui.set_clip_rect(rect);
            if animating {
                ctx.request_repaint();
                self.paint_ide(ui.painter(), &element, rect, zoom);
                ui.allocate_rect(rect, Sense::hover());
            } else if let Some(ide) = self.ides.get_mut(&id) {
                ide.has_agent = has_agent;
                ui.scope_builder(egui::UiBuilder::new().max_rect(rect), |ui| ide.ui(ui, focused));
            }
        });
        let Some(ide) = self.ides.get_mut(&id) else { return };
        let notes = std::mem::take(&mut ide.to_agent);
        if std::mem::take(&mut ide.wants_claude) {
            self.claude_for_ide(&id);
        } else if std::mem::take(&mut ide.wants_canvas) {
            self.set_elevated(None);
        }
        // comments on changes go to the agents roped to this IDE
        if !notes.is_empty() {
            let agents = self.roped_agents(&id);
            if agents.is_empty() {
                self.toast("Ligue um agente a este IDE para mandar o comentário");
            }
            // DIFF_REJECTED travels by the WebSocket thread; if the paste's Enter reached Claude Code
            // while its diff prompt was still up, it would pick the highlighted "Yes".
            // ponytail: 1 s fixo, sem confirmação do CLI; se falhar, colar depois da resposta sair pelo socket
            for note in &notes {
                for agent in &agents {
                    self.send_prompt_after(agent, note, std::time::Duration::from_secs(1));
                }
            }
        }
    }
}
