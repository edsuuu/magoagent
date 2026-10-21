//! The IDE's frame: panels, main toolbar, the tool window stripe, main menu, toast. No status
//! bar and no right stripe, to keep it quiet on the canvas: messages show in the toolbar.

use super::*;

impl Ide {
    /// The live IDE in `ui` (sized to the node's box on screen). `focused`: it has the keyboard.
    pub(crate) fn ui(&mut self, ui: &mut Ui, focused: bool) {
        let ctx = ui.ctx().clone();
        theme::style(ui.style_mut());
        self.rect = ui.max_rect();
        ui.painter().rect_filled(self.rect, 8, t::FRAME);
        // Esc nothing else uses goes back to the canvas
        let busy = self.palette.is_some() || self.find_files.is_some() || self.find.is_some() || ctx.memory(|m| m.focused().is_some());
        if focused {
            self.shortcuts(&ctx);
        }
        self.toolbar(ui);
        self.stripe(ui);
        if let Some(tool) = self.bottom {
            egui::Panel::bottom("bottom_tool").frame(island()).resizable(true).show_separator_line(false)
                .default_size(260.0).min_size(80.0)
                .show(ui, |ui| self.tool_window(ui, tool));
        }
        if let Some(w) = self.left.filter(|_| self.tree.is_some()) {
            egui::Panel::left("left_tool").frame(island()).resizable(true).show_separator_line(false)
                .default_size(320.0).min_size(160.0)
                .show(ui, |ui| match w {
                    Left::Project => self.project(ui),
                    Left::Commit => self.commit_panel(ui),
                    Left::Database => self.db_panel(ui),
                });
        }
        egui::CentralPanel::default().frame(island()).show(ui, |ui| self.center(ui));
        self.search_everywhere(&ctx);
        self.find_in_files(&ctx);
        self.toast_ui(&ctx);
        if focused && !busy && ctx.input(|i| i.key_pressed(Key::Escape)) {
            self.wants_canvas = true;
        }
    }

    /// Only while the IDE has the keyboard (the canvas's shortcuts are off then).
    fn shortcuts(&mut self, ctx: &egui::Context) {
        let sc = |m, k| KeyboardShortcut::new(m, k);
        if ctx.input_mut(|i| i.consume_shortcut(&sc(Modifiers::CTRL, Key::Escape))) {
            self.run(Cmd::ClaudeCode);
        }
        if ctx.input_mut(|i| i.consume_shortcut(&sc(Modifiers::CTRL | Modifiers::ALT, Key::K))) {
            self.run(Cmd::MentionClaude);
        }
        let (ctrl, shift, alt) = (Modifiers::COMMAND, Modifiers::COMMAND | Modifiers::SHIFT, Modifiers::ALT);
        let table: [(KeyboardShortcut, Cmd); 23] = [
            (sc(ctrl, Key::S), Cmd::SaveAll),
            (sc(ctrl, Key::W), Cmd::CloseTab),
            (sc(ctrl, Key::F4), Cmd::CloseTab),
            (sc(ctrl, Key::F), Cmd::Find),
            (sc(ctrl, Key::G), Cmd::GotoLine),
            (sc(alt, Key::Num1), Cmd::ToggleProject),
            (sc(shift, Key::E), Cmd::ToggleProject),
            (sc(alt, Key::Num6), Cmd::Tool(ToolWindow::Problems)),
            (sc(shift, Key::M), Cmd::Tool(ToolWindow::Problems)),
            (sc(ctrl, Key::Slash), Cmd::Comment),
            (sc(ctrl, Key::D), Cmd::Duplicate),
            (sc(ctrl, Key::B), Cmd::GotoDeclaration),
            (sc(Modifiers::NONE, Key::F12), Cmd::GotoDeclaration),
            (sc(alt, Key::F7), Cmd::Usages),
            (sc(ctrl | Modifiers::ALT, Key::B), Cmd::Implementations),
            (sc(ctrl | Modifiers::ALT, Key::ArrowLeft), Cmd::Back),
            (sc(alt, Key::ArrowLeft), Cmd::Back),
            (sc(ctrl | Modifiers::ALT, Key::ArrowRight), Cmd::Forward),
            (sc(alt, Key::ArrowRight), Cmd::Forward),
            (sc(ctrl, Key::K), Cmd::Commit),
            (sc(alt, Key::Num0), Cmd::Commit),
            (sc(shift, Key::K), Cmd::Push),
            (sc(ctrl, Key::T), Cmd::Pull),
        ];
        for (s, cmd) in table {
            if ctx.input_mut(|i| i.consume_shortcut(&s)) {
                self.run(cmd);
            }
        }
        // VSCode: Ctrl+P files, Ctrl+Shift+P actions. PhpStorm: Ctrl+Shift+N files, Ctrl+Shift+A actions, Shift Shift everything.
        if ctx.input_mut(|i| i.consume_shortcut(&sc(shift, Key::P)) || i.consume_shortcut(&sc(shift, Key::A))) {
            self.palette = Some(search::Everywhere::new(search::Tab::Actions, ""));
        }
        if ctx.input_mut(|i| i.consume_shortcut(&sc(ctrl | Modifiers::ALT | Modifiers::SHIFT, Key::N))) {
            self.palette = Some(search::Everywhere::new(search::Tab::Symbols, ""));
        }
        if ctx.input_mut(|i| i.consume_shortcut(&sc(ctrl, Key::P)) || i.consume_shortcut(&sc(shift, Key::N))) {
            self.palette = Some(search::Everywhere::new(search::Tab::Files, ""));
        }
        if ctx.input_mut(|i| i.consume_shortcut(&sc(ctrl, Key::N))) {
            self.palette = Some(search::Everywhere::new(search::Tab::Classes, ""));
        }
        if ctx.input_mut(|i| i.consume_shortcut(&sc(shift, Key::F))) {
            self.open_find(None);
        }
        if ctx.input_mut(|i| i.consume_shortcut(&sc(Modifiers::CTRL, Key::Tab))) && !self.tabs.is_empty() {
            self.active = (self.active + 1) % self.tabs.len();
        }
        // PhpStorm "Search Everywhere": Shift pressed twice, nothing in between.
        let (time, events) = ctx.input(|i| (i.time, i.events.clone()));
        for e in events {
            match e {
                egui::Event::Key { key: Key::ShiftLeft | Key::ShiftRight, pressed: true, repeat: false, .. } => {
                    if self.shift_tap.is_some_and(|t0| time - t0 < 0.4) {
                        self.palette = Some(search::Everywhere::new(search::Tab::All, ""));
                        self.shift_tap = None;
                    } else {
                        self.shift_tap = Some(time);
                    }
                }
                egui::Event::Key { key: Key::ShiftLeft | Key::ShiftRight, .. } => {}
                egui::Event::Key { .. } | egui::Event::Text(_) | egui::Event::PointerButton { .. } => self.shift_tap = None,
                _ => {}
            }
        }
    }

    fn toast_ui(&mut self, ctx: &egui::Context) {
        let Some((msg, at)) = &self.toast else { return };
        if at.elapsed() > Duration::from_secs(5) {
            self.toast = None;
            return;
        }
        ctx.request_repaint_after(Duration::from_millis(500));
        egui::Area::new(self.id.with("toast")).order(Order::Foreground).pivot(Align2::RIGHT_BOTTOM)
            .fixed_pos(pos2(self.rect.right() - 16.0, self.rect.bottom() - 16.0))
            .show(ctx, |ui| {
                theme::style(ui.style_mut());
                Frame::popup(ui.style()).fill(t::POPUP).stroke(Stroke::new(1.0, t::POPUP_BORDER)).corner_radius(10).inner_margin(12)
                    .show(ui, |ui| {
                        ui.set_max_width(380.0);
                        ui.label(egui::RichText::new(msg.as_str()).color(t::TEXT));
                    });
            });
    }
}

// ---------------------------------------------------------------- chrome

fn island() -> Frame {
    Frame::NONE.fill(t::ISLAND).corner_radius(t::ISLAND_RADIUS).outer_margin(Margin::same(t::ISLAND_GAP))
}

/// Main-toolbar dropdown widget: [lead] label ⌄. Returns the response and the lead area.
fn dropdown(ui: &mut Ui, lead_w: f32, label: &str) -> (egui::Response, Rect) {
    let galley = ui.painter().layout_no_wrap(label.into(), t::ui(t::UI_SIZE), t::TEXT);
    let (rect, resp) = ui.allocate_exact_size(vec2(lead_w + galley.size().x + 34.0, 30.0), Sense::click());
    if resp.hovered() || egui::Popup::is_id_open(ui.ctx(), egui::Popup::default_response_id(&resp)) {
        ui.painter().rect_filled(rect, 6, t::HOVER);
    }
    let x = rect.left() + 8.0 + lead_w;
    ui.painter().galley(pos2(x, rect.center().y - galley.size().y / 2.0), galley, t::TEXT);
    icons::paint(ui.painter(), icons::CHEVRON_DOWN, Rect::from_center_size(pos2(rect.right() - 13.0, rect.center().y), vec2(14.0, 14.0)));
    (resp, Rect::from_min_size(pos2(rect.left() + 6.0, rect.top()), vec2(lead_w, rect.height())))
}

pub(super) fn menu_item(ui: &mut Ui, label: &str, shortcut: &str) -> bool {
    ui.add(egui::Button::new(label).shortcut_text(keys(shortcut))).clicked()
}

/// Ilha writes shortcuts with Ctrl; on the Mac the same keys take Cmd (egui's COMMAND), and the
/// canvas writes "Cmd+" too.
pub(super) fn keys(shortcut: &str) -> String {
    if cfg!(target_os = "macos") { shortcut.replace("Ctrl+", "Cmd+") } else { shortcut.into() }
}

fn project_color(name: &str) -> Color32 {
    // PhpStorm tints the project widget by project; same idea, stable hash into a few accent hues
    const C: [u32; 6] = [0x3871E1, 0x338555, 0xA56906, 0x8060DB, 0xC54E58, 0x2AACB8];
    let h = name.bytes().fold(7u32, |h, b| h.wrapping_mul(31).wrapping_add(b as u32));
    let v = C[(h % 6) as usize];
    Color32::from_rgb((v >> 16) as u8, (v >> 8) as u8, v as u8)
}

fn initials(name: &str) -> String {
    let words: Vec<&str> = name.split(|c: char| !c.is_alphanumeric()).filter(|w| !w.is_empty()).collect();
    let s: String = match words.as_slice() {
        [a, b, ..] => a.chars().take(1).chain(b.chars().take(1)).collect(),
        [a] => a.chars().take(2).collect(),
        [] => "IL".into(),
    };
    s.to_uppercase()
}

impl Ide {
    fn toolbar(&mut self, ui: &mut Ui) {
        egui::Panel::top("main_toolbar").exact_size(40.0).frame(Frame::NONE.fill(t::FRAME).inner_margin(Margin::symmetric(8, 0)))
            .show(ui, |ui| {
                // a double-click on the bar's empty part goes back to the canvas
                if ui.interact(ui.max_rect(), self.id.with("title"), Sense::click()).double_clicked() {
                    self.wants_canvas = true;
                }
                ui.horizontal_centered(|ui| {
                    ui.spacing_mut().item_spacing.x = 4.0;
                    let menu = icons::button(ui, icons::MENU, "Menu principal", false);
                    theme::menu(&menu).show(|ui| self.main_menu(ui));
                    // project widget: tinted initials badge + name
                    let name = self.root.as_ref().map_or(self.folder.clone(), |r| file_name(r));
                    let galley = ui.painter().layout_no_wrap(name.clone(), t::ui(t::UI_SIZE), t::TEXT);
                    let (rect, _) = ui.allocate_exact_size(vec2(galley.size().x + 44.0, 30.0), Sense::hover());
                    let badge = Rect::from_center_size(pos2(rect.left() + 19.0, rect.center().y), vec2(20.0, 20.0));
                    ui.painter().rect_filled(badge, 5, project_color(&name));
                    ui.painter().text(badge.center(), Align2::CENTER_CENTER, initials(&name), t::ui(9.5), Color32::WHITE);
                    ui.painter().galley(pos2(rect.left() + 36.0, rect.center().y - galley.size().y / 2.0), galley, t::TEXT);
                    if !self.branch.is_empty() {
                        let (resp, lead) = dropdown(ui, 20.0, &self.branch.clone());
                        icons::at(ui.painter(), icons::BRANCH, lead.center());
                        theme::menu(&resp).show(|ui| self.branch_menu(ui));
                    }
                    ui.with_layout(egui::Layout::right_to_left(Align::Center), |ui| {
                        let (rect, back) = ui.allocate_exact_size(vec2(30.0, 30.0), Sense::click());
                        if back.hovered() {
                            ui.painter().rect_filled(rect, 6, t::HOVER);
                        }
                        crate::app::ui::icons::paint_icon(ui.painter(), crate::app::ui::icons::Icon::Expand, rect.center(), t::TEXT);
                        if back.on_hover_text("Voltar ao canvas (Esc, ou clique duplo nesta barra)").clicked() {
                            self.wants_canvas = true;
                        }
                        if icons::button(ui, icons::SEARCH, "Pesquisar em tudo (Shift duas vezes)", false).clicked() {
                            self.palette = Some(search::Everywhere::new(search::Tab::All, ""));
                        }
                        let connected = self.claude_connected();
                        let tip = if connected {
                            "Claude Code: terminal ligado a este IDE (Ctrl+Esc) · conectado"
                        } else {
                            "Claude Code: abre um terminal ligado a este IDE (Ctrl+Esc)"
                        };
                        if icons::button(ui, icons::CLAUDE, tip, connected).clicked() {
                            self.run(Cmd::ClaudeCode);
                        }
                        // what the status bar used to say: the last message, or the language servers'
                        let msg = if self.status.is_empty() { self.insp.as_ref().map(|i| i.status.clone()).unwrap_or_default() } else { self.status.clone() };
                        ui.add_space(8.0);
                        ui.add(egui::Label::new(egui::RichText::new(msg).color(t::SECONDARY)).truncate());
                    });
                });
            });
    }

    fn stripe(&mut self, ui: &mut Ui) {
        egui::Panel::left("stripe").exact_size(40.0).frame(Frame::NONE.fill(t::FRAME).inner_margin(Margin::symmetric(5, 6)))
            .show(ui, |ui| {
                ui.spacing_mut().item_spacing.y = 4.0;
                if icons::button(ui, icons::PROJECT, "Projeto (Alt+1)", self.left == Some(Left::Project)).clicked() {
                    self.run(Cmd::ToggleProject);
                }
                if icons::button(ui, icons::COMMIT, "Commit (Alt+0)", self.left == Some(Left::Commit)).clicked() {
                    self.run(Cmd::Commit);
                }
                if icons::button(ui, icons::FT_SQL, "Banco de dados", self.left == Some(Left::Database)).clicked() {
                    self.run(Cmd::Database);
                }
                ui.with_layout(egui::Layout::bottom_up(Align::Min), |ui| {
                    if icons::button(ui, icons::PROBLEMS, "Problemas (Alt+6)", self.bottom == Some(ToolWindow::Problems)).clicked() {
                        self.run(Cmd::Tool(ToolWindow::Problems));
                    }
                });
            });
    }

    /// PhpStorm's main menu (New UI hides it behind the hamburger), with submenus.
    fn main_menu(&mut self, ui: &mut Ui) {
        ui.set_min_width(220.0);
        let mut cmd = None;
        ui.menu_button("Arquivo", |ui| {
            if menu_item(ui, "Salvar tudo", "Ctrl+S") {
                cmd = Some(Cmd::SaveAll);
            }
            if menu_item(ui, "Fechar aba", "Ctrl+W") {
                cmd = Some(Cmd::CloseTab);
            }
            ui.separator();
            if menu_item(ui, "Voltar ao canvas", "Esc") {
                self.wants_canvas = true;
                ui.close();
            }
        });
        ui.menu_button("Editar", |ui| {
            if menu_item(ui, "Localizar…", "Ctrl+F") {
                cmd = Some(Cmd::Find);
            }
            if menu_item(ui, "Localizar nos arquivos…", "Ctrl+Shift+F") {
                cmd = Some(Cmd::FindInFiles);
            }
            ui.separator();
            if menu_item(ui, "Comentar com comentário de linha", "Ctrl+/") {
                cmd = Some(Cmd::Comment);
            }
            if menu_item(ui, "Duplicar linha ou seleção", "Ctrl+D") {
                cmd = Some(Cmd::Duplicate);
            }
        });
        ui.menu_button("Exibir", |ui| {
            ui.menu_button("Janelas de ferramentas", |ui| {
                if menu_item(ui, "Projeto", "Alt+1") {
                    cmd = Some(Cmd::ToggleProject);
                }
                if menu_item(ui, "Problemas", "Alt+6") {
                    cmd = Some(Cmd::Tool(ToolWindow::Problems));
                }
                if menu_item(ui, "Commit", "Alt+0") {
                    cmd = Some(Cmd::Commit);
                }
                if menu_item(ui, "Banco de dados", "") {
                    cmd = Some(Cmd::Database);
                }
                if menu_item(ui, "Claude Code", "Ctrl+Esc") {
                    cmd = Some(Cmd::ClaudeCode);
                }
            });
            ui.separator();
            if menu_item(ui, "Aumentar fonte", "Ctrl+Roda para cima") {
                self.font_size = (self.font_size + 1.0).min(40.0);
            }
            if menu_item(ui, "Diminuir fonte", "Ctrl+Roda para baixo") {
                self.font_size = (self.font_size - 1.0).max(8.0);
            }
            if menu_item(ui, "Fonte padrão", "") {
                self.font_size = t::EDITOR_SIZE;
            }
            ui.separator();
            ui.checkbox(&mut self.code_vision, "Code Vision (usos, implementações)");
        });
        ui.menu_button("Git", |ui| {
            if menu_item(ui, "Commit…", "Ctrl+K") {
                cmd = Some(Cmd::Commit);
            }
            if menu_item(ui, "Push…", "Ctrl+Shift+K") {
                cmd = Some(Cmd::Push);
            }
            if menu_item(ui, "Atualizar projeto…", "Ctrl+T") {
                cmd = Some(Cmd::Pull);
            }
            if let Some(tab) = self.tabs.get(self.active).filter(|t| t.is_text())
                && menu_item(ui, "Mostrar diff", "")
            {
                cmd = Some(Cmd::Diff(tab.path.clone()));
            }
        });
        ui.menu_button("Ferramentas", |ui| {
            if menu_item(ui, "Claude Code", "Ctrl+Esc") {
                cmd = Some(Cmd::ClaudeCode);
            }
            if menu_item(ui, "Enviar seleção ao Claude Code", "Ctrl+Alt+K") {
                cmd = Some(Cmd::MentionClaude);
            }
        });
        ui.menu_button("Navegar", |ui| {
            if menu_item(ui, "Classe…", "Ctrl+N") {
                self.palette = Some(search::Everywhere::new(search::Tab::Classes, ""));
            }
            if menu_item(ui, "Símbolo…", "Ctrl+Alt+Shift+N") {
                self.palette = Some(search::Everywhere::new(search::Tab::Symbols, ""));
            }
            if menu_item(ui, "Pesquisar em tudo", "Shift duas vezes") {
                self.palette = Some(search::Everywhere::new(search::Tab::All, ""));
            }
            if menu_item(ui, "Arquivo…", "Ctrl+Shift+N") {
                self.palette = Some(search::Everywhere::new(search::Tab::Files, ""));
            }
            if menu_item(ui, "Ação…", "Ctrl+Shift+A") {
                self.palette = Some(search::Everywhere::new(search::Tab::Actions, ""));
            }
            if menu_item(ui, "Linha…", "Ctrl+G") {
                cmd = Some(Cmd::GotoLine);
            }
        });
        if let Some(c) = cmd {
            ui.close();
            self.run(c);
        }
    }

    fn tool_window(&mut self, ui: &mut Ui, tool: ToolWindow) {
        match tool {
            ToolWindow::Problems => self.problems(ui),
            ToolWindow::Find => self.find_window(ui),
        }
    }

    fn problems(&mut self, ui: &mut Ui) {
        tool_header(ui, "Problemas");
        let Some(insp) = &self.insp else {
            ui.label("Abra uma pasta para rodar as inspeções.");
            return;
        };
        let mut all: Vec<_> = insp.all().collect();
        all.sort_by(|a, b| (a.0, a.1.severity, a.1.line).cmp(&(b.0, b.1.severity, b.1.line)));
        let mut missing: Vec<_> = insp.missing.iter().map(|(l, h)| (*l, *h)).collect();
        missing.sort();
        let mut go = None;
        egui::ScrollArea::vertical().auto_shrink(false).show(ui, |ui| {
            ui.spacing_mut().item_spacing.y = 0.0;
            for (lang, hint) in missing {
                ui.horizontal(|ui| {
                    ui.add_space(12.0);
                    ui.label(egui::RichText::new(format!("Nenhum servidor de inspeção de {lang} encontrado. Instale com:")).color(t::MUTED));
                    ui.label(egui::RichText::new(hint).monospace().color(t::TEXT));
                    if ui.small_button("Copiar").clicked() {
                        ui.ctx().copy_text(hint.into());
                    }
                });
            }
            if all.is_empty() {
                ui.add_space(8.0);
                ui.horizontal(|ui| {
                    ui.add_space(12.0);
                    ui.label(egui::RichText::new("✔ Nenhum problema encontrado").color(t::MUTED));
                });
            }
            let mut last: Option<&PathBuf> = None;
            for (path, d) in all {
                if last != Some(path) {
                    last = Some(path);
                    let rel = self.root.as_ref().and_then(|r| path.strip_prefix(r).ok()).unwrap_or(path);
                    ui.add_space(4.0);
                    ui.horizontal(|ui| {
                        ui.add_space(12.0);
                        let (r, _) = ui.allocate_exact_size(vec2(16.0, 16.0), Sense::hover());
                        icons::file(ui.painter(), path, r.center(), 1.0);
                        ui.label(egui::RichText::new(rel.display().to_string()).color(t::TEXT));
                    });
                }
                let (resp, rect) = row(ui, false, 22.0);
                match d.severity {
                    1 => icons::at(ui.painter(), icons::ERROR, pos2(rect.left() + 40.0, rect.center().y)),
                    2 => icons::at(ui.painter(), icons::WARNING, pos2(rect.left() + 40.0, rect.center().y)),
                    _ => {}
                }
                let p = ui.painter();
                let first_line = d.message.lines().next().unwrap_or("");
                p.text(pos2(rect.left() + 54.0, rect.center().y), Align2::LEFT_CENTER,
                    format!("{first_line}   {} {}:{}", d.source, d.line + 1, d.col + 1), t::ui(t::UI_SIZE), t::TEXT);
                if resp.on_hover_text(&d.message).clicked() {
                    go = Some(Cmd::Open(path.clone(), Some((d.line, d.col))));
                }
            }
        });
        if let Some(cmd) = go {
            self.run(cmd);
        }
    }
}

// ---------------------------------------------------------------- widgets

pub(super) fn tool_header(ui: &mut Ui, title: &str) {
    ui.horizontal(|ui| {
        ui.set_height(36.0);
        ui.add_space(14.0);
        ui.label(egui::RichText::new(title).color(t::TEXT).strong());
    });
}

/// Full-width selectable row with PhpStorm hover/selection colours.
pub(super) fn row(ui: &mut Ui, selected: bool, h: f32) -> (egui::Response, Rect) {
    row_colored(ui, if selected { t::SELECTION } else { Color32::TRANSPARENT }, h)
}

pub(super) fn row_colored(ui: &mut Ui, fill: Color32, h: f32) -> (egui::Response, Rect) {
    let (rect, resp) = ui.allocate_exact_size(vec2(ui.available_width(), h), Sense::click());
    let bg = if fill != Color32::TRANSPARENT { fill } else if resp.hovered() { t::ROW_HOVER } else { Color32::TRANSPARENT };
    ui.painter().rect_filled(rect.shrink2(vec2(6.0, 0.0)), 5, bg);
    (resp, rect)
}
