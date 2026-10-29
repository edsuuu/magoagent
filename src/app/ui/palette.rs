//! Command palette.

use crate::app::ui::links::LinkAction;
use crate::prelude::*;

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) enum Command {
    DarkMode,
    Export,
    Save,
    SaveAs,
    Open,
    Undo,
    Redo,
    ZoomIn,
    ZoomOut,
    ZoomReset,
    ZoomFit,
    ZoomSelection,
    SelectAll,
    Clear,
    Tool(Tool),
    Lock,
    Duplicate,
    Delete,
    Group,
    Ungroup,
    Layer(Layer),
    Grid,
    LockSelection,
    LockAll,
    UnlockAll,
    SnapObjects,
    Zen,
    ViewMode,
    Stats,
    ShowHints,
    Connect,
    Disconnect,
    RemoveConnections,
}

/// What a palette line runs: a command, or the node picked in a connect / disconnect search.
#[derive(Clone, Debug)]
pub(crate) enum Run {
    Command(Command),
    Node(String),
}

/// One line of the palette.
pub(crate) struct Row {
    pub(crate) group: &'static str,
    pub(crate) icon: Icon,
    pub(crate) label: String,
    pub(crate) keys: &'static str,
    pub(crate) run: Run,
}

impl Command {
    pub(crate) const ALL: [Command; 45] = [
        Command::LockAll,
        Command::UnlockAll,
        Command::LockSelection,
        Command::Grid,
        Command::SnapObjects,
        Command::Zen,
        Command::ViewMode,
        Command::Stats,
        Command::ShowHints,
        Command::DarkMode,
        Command::Export,
        Command::Save,
        Command::SaveAs,
        Command::Open,
        Command::Undo,
        Command::Redo,
        Command::ZoomIn,
        Command::ZoomOut,
        Command::ZoomReset,
        Command::ZoomFit,
        Command::SelectAll,
        Command::Clear,
        Command::Tool(Tool::Hand),
        Command::Tool(Tool::Select),
        Command::Tool(Tool::Rectangle),
        Command::Tool(Tool::Diamond),
        Command::Tool(Tool::Ellipse),
        Command::Tool(Tool::Arrow),
        Command::Tool(Tool::Line),
        Command::Tool(Tool::Draw),
        Command::Tool(Tool::Text),
        Command::Tool(Tool::Eraser),
        Command::Lock,
        Command::ZoomSelection,
        Command::Duplicate,
        Command::Delete,
        Command::Group,
        Command::Ungroup,
        Command::Connect,
        Command::Disconnect,
        Command::RemoveConnections,
        Command::Layer(Layer::Front),
        Command::Layer(Layer::Forward),
        Command::Layer(Layer::Backward),
        Command::Layer(Layer::Back),
    ];

    pub(crate) fn group(self) -> &'static str {
        match self {
            Command::DarkMode => "App",
            Command::Export | Command::Save | Command::SaveAs | Command::Open => "Exportar",
            Command::Tool(_) | Command::Lock => "Ferramentas",
            Command::ZoomSelection
            | Command::Duplicate
            | Command::Delete
            | Command::Group
            | Command::Ungroup
            | Command::LockSelection
            | Command::Connect
            | Command::Disconnect
            | Command::RemoveConnections
            | Command::Layer(_) => "Elementos",
            _ => "Editor",
        }
    }

    pub(crate) fn label(self) -> &'static str {
        match self {
            Command::DarkMode => "Modo escuro",
            Command::Export => "Exportar imagem...",
            Command::Save => "Salvar no computador",
            Command::SaveAs => "Salvar como...",
            Command::Open => "Abrir",
            Command::Undo => "Desfazer",
            Command::Redo => "Refazer",
            Command::ZoomIn => "Aumentar zoom",
            Command::ZoomOut => "Diminuir zoom",
            Command::ZoomReset => "Redefinir zoom",
            Command::ZoomFit => "Ampliar para encaixar todos os elementos",
            Command::ZoomSelection => "Zoom para caber na tela",
            Command::SelectAll => "Selecionar tudo",
            Command::Clear => "Limpar a tela",
            Command::Tool(tool) => tool.label(),
            Command::Lock => "Manter ativa a ferramenta selecionada após desenhar",
            Command::Duplicate => "Duplicar",
            Command::Delete => "Apagar",
            Command::Group => "Agrupar seleção",
            Command::Ungroup => "Desagrupar seleção",
            Command::Layer(Layer::Front) => "Trazer para o primeiro plano",
            Command::Layer(Layer::Forward) => "Trazer para a frente",
            Command::Layer(Layer::Backward) => "Enviar para trás",
            Command::Layer(Layer::Back) => "Enviar para o fundo",
            Command::Grid => "Alternar grade",
            Command::LockSelection => "Bloquear / desbloquear seleção",
            Command::LockAll => "Bloquear tudo",
            Command::UnlockAll => "Desbloquear tudo",
            Command::SnapObjects => "Encaixar em objetos",
            Command::Zen => "Modo Zen",
            Command::ViewMode => "Modo de visualização",
            Command::Stats => "Propriedades da Tela e da Forma",
            Command::ShowHints => "Mostrar dicas",
            Command::Connect => "Conectar a…",
            Command::Disconnect => "Desconectar…",
            Command::RemoveConnections => "Remover todas as conexões",
        }
    }

    pub(crate) fn keys(self) -> &'static str {
        match self {
            Command::DarkMode => "Shift+Option+D",
            Command::Export => "Cmd+Shift+E",
            Command::Save => "Cmd+S",
            Command::SaveAs => "Cmd+Shift+S",
            Command::Open => "Cmd+O",
            Command::Undo => "Cmd+Z",
            Command::Redo => "Cmd+Shift+Z",
            Command::ZoomIn => "Cmd++",
            Command::ZoomOut => "Cmd+-",
            Command::ZoomReset => "Cmd+0",
            Command::ZoomFit => "Shift+1",
            Command::ZoomSelection => "Shift+2",
            Command::SelectAll => "Cmd+A",
            Command::Clear => "Cmd+Delete",
            Command::Tool(tool) => tool.badge(),
            Command::Lock => "Q",
            Command::Duplicate => "Cmd+D",
            Command::Delete => "Delete",
            Command::Group => "Cmd+G",
            Command::Ungroup => "Cmd+Shift+G",
            Command::Layer(Layer::Front) => "Cmd+Option+]",
            Command::Layer(Layer::Forward) => "Cmd+]",
            Command::Layer(Layer::Backward) => "Cmd+[",
            Command::Layer(Layer::Back) => "Cmd+Option+[",
            Command::Grid => "Cmd+'",
            Command::LockSelection => "Cmd+Shift+L",
            Command::LockAll | Command::UnlockAll | Command::Connect | Command::Disconnect | Command::RemoveConnections => "",
            Command::SnapObjects => "Option+S",
            Command::Zen => "Option+Z",
            Command::ViewMode => "Option+R",
            Command::Stats => "Option+/",
            Command::ShowHints => "",
        }
    }

    pub(crate) fn icon(self) -> Icon {
        match self {
            Command::DarkMode => Icon::Moon,
            Command::Export => Icon::Export,
            Command::Save | Command::SaveAs => Icon::Download,
            Command::Open => Icon::Open,
            Command::Undo => Icon::Undo,
            Command::Redo => Icon::Redo,
            Command::ZoomIn => Icon::Plus,
            Command::ZoomOut => Icon::Minus,
            Command::ZoomReset | Command::ZoomFit | Command::ZoomSelection => Icon::Search,
            Command::SelectAll => Icon::Tool(Tool::Select),
            Command::Clear | Command::Delete => Icon::Trash,
            Command::Tool(tool) => Icon::Tool(tool),
            Command::Lock => Icon::Lock(true),
            Command::Duplicate => Icon::Duplicate,
            Command::Group => Icon::Group,
            Command::Ungroup => Icon::Ungroup,
            Command::Layer(Layer::Front) => Icon::ToFront,
            Command::Layer(Layer::Forward) => Icon::Forward,
            Command::Layer(Layer::Backward) => Icon::Backward,
            Command::Layer(Layer::Back) => Icon::ToBack,
            Command::Grid => Icon::Grid,
            Command::LockSelection | Command::LockAll => Icon::Lock(true),
            Command::UnlockAll => Icon::Lock(false),
            Command::SnapObjects => Icon::Sliders,
            Command::Zen | Command::ViewMode => Icon::Eye,
            Command::Stats => Icon::Sliders,
            Command::ShowHints => Icon::Check,
            Command::Connect | Command::Disconnect => Icon::Connect,
            Command::RemoveConnections => Icon::Trash,
        }
    }
}

/// "Cmd+Shift+E" -> ["Cmd", "Shift", "E"]; keeps the "+" key in "Cmd++".
pub(crate) fn shortcut_keys(keys: &str) -> Vec<&str> {
    if keys.is_empty() {
        return Vec::new();
    }
    match keys.strip_suffix("++") {
        Some(rest) => rest.split('+').chain(["+"]).collect(),
        None => keys.split('+').collect(),
    }
}

/// Lowercase without accents, so "acao" finds "ação".
pub(crate) fn fold(text: &str) -> String {
    text.to_lowercase()
        .chars()
        .map(|ch| match ch {
            'á' | 'à' | 'â' | 'ã' | 'ä' => 'a',
            'é' | 'ê' | 'è' => 'e',
            'í' | 'ì' => 'i',
            'ó' | 'ô' | 'õ' | 'ò' => 'o',
            'ú' | 'ù' | 'ü' => 'u',
            'ç' => 'c',
            other => other,
        })
        .collect()
}

pub(crate) fn key_chip(ui: &mut egui::Ui, key: &str, c: &UiColors) {
    let galley = ui.painter().layout_no_wrap(key.to_string(), FontId::proportional(10.0), c.text);
    let (rect, _) = ui.allocate_exact_size(Vec2::new(galley.size().x + 8.0, 16.0), Sense::hover());
    ui.painter().rect_filled(rect, 4, c.selected);
    ui.painter().galley(rect.center() - galley.size() / 2.0, galley, c.text);
}

impl DrawingApp {
    pub(crate) fn open_palette(&mut self) {
        self.menu_open = false;
        self.palette_open = true;
        self.palette_links = None;
        self.palette_query.clear();
        self.palette_index = 0;
        self.palette_focused = false;
    }

    /// Commands matching the search, grouped like CommandPalette.tsx.
    pub(crate) fn palette_items(&self) -> Vec<Command> {
        let query = fold(&self.palette_query);
        let words: Vec<&str> = query.split_whitespace().collect();
        Command::ALL
            .iter()
            .copied()
            .filter(|cmd| cmd.group() != "Elementos" || !self.selected.is_empty())
            .filter(|cmd| {
                let label = fold(cmd.label());
                words.iter().all(|w| label.contains(w))
            })
            .collect()
    }

    /// The lines of the palette now: the node search of a connect / disconnect, else the commands.
    pub(crate) fn palette_rows(&self) -> Vec<Row> {
        if let Some(action) = self.palette_links {
            let Some(source) = self.active_node().map(|e| e.id.clone()) else { return Vec::new() };
            let query = fold(&self.palette_query);
            let words: Vec<&str> = query.split_whitespace().collect();
            return self
                .link_candidates(&source, action)
                .into_iter()
                .filter(|(_, _, name)| {
                    let name = fold(name);
                    words.iter().all(|w| name.contains(w))
                })
                .map(|(id, icon, label)| Row { group: "", icon, label, keys: "", run: Run::Node(id) })
                .collect();
        }
        self.palette_items()
            .into_iter()
            .map(|cmd| Row { group: cmd.group(), icon: cmd.icon(), label: cmd.label().into(), keys: cmd.keys(), run: Run::Command(cmd) })
            .collect()
    }

    pub(crate) fn run_command(&mut self, cmd: Command) {
        match cmd {
            Command::DarkMode => self.prefs.theme = if self.dark() { Theme::Light } else { Theme::Dark },
            Command::Export => self.export_open = true,
            Command::Save => self.save(),
            Command::SaveAs => self.save_as(),
            Command::Open => self.open(),
            Command::Undo => self.undo(),
            Command::Redo => self.redo(),
            Command::ZoomIn => self.zoom_step(0.1),
            Command::ZoomOut => self.zoom_step(-0.1),
            Command::ZoomReset => {
                let center = self.canvas_rect.center();
                self.zoom_at(1.0, center);
            }
            Command::ZoomFit => self.zoom_to_fit(false),
            Command::ZoomSelection => self.zoom_to_fit(true),
            Command::SelectAll => self.select_all(),
            Command::Clear => self.clear_canvas(),
            Command::Tool(tool) => self.select_tool(tool),
            Command::Lock => self.tool_locked = !self.tool_locked,
            Command::Duplicate => self.duplicate(),
            Command::Delete => self.delete_selected(),
            Command::Group => self.group_selection(),
            Command::Ungroup => self.ungroup_selection(),
            Command::Layer(layer) => self.reorder(layer),
            Command::Grid => self.prefs.grid = !self.prefs.grid,
            Command::LockSelection => self.toggle_lock_selected(),
            Command::LockAll => self.lock_all(),
            Command::UnlockAll => self.unlock(None),
            Command::SnapObjects => self.prefs.snap_objects = !self.prefs.snap_objects,
            Command::Zen => self.prefs.zen = !self.prefs.zen,
            Command::ViewMode => {
                self.prefs.view_mode = !self.prefs.view_mode;
                self.selected.clear();
            }
            Command::Stats => self.prefs.stats = !self.prefs.stats,
            Command::ShowHints => self.prefs.show_hints = !self.prefs.show_hints,
            // the node search opens in the same palette, which stays up
            Command::Connect | Command::Disconnect => {
                self.palette_links = Some(if cmd == Command::Connect { LinkAction::Connect } else { LinkAction::Disconnect });
                self.palette_query.clear();
                self.palette_index = 0;
            }
            Command::RemoveConnections => {
                let ids: Vec<String> = self.selected.iter().filter_map(|i| self.scene.elements.get(*i)).map(|e| e.id.clone()).collect();
                self.unlink_all(&ids);
            }
        }
    }

    /// Searchable command list with shortcuts (Cmd+/), styled after CommandPalette.scss.
    pub(crate) fn command_palette(&mut self, ctx: &Context) {
        let c = self.colors();
        let rows = self.palette_rows();
        self.palette_index = self.palette_index.min(rows.len().saturating_sub(1));
        let (down, up, enter, escape) = ctx.input_mut(|i| {
            (
                i.consume_key(Modifiers::NONE, Key::ArrowDown),
                i.consume_key(Modifiers::NONE, Key::ArrowUp),
                i.consume_key(Modifiers::NONE, Key::Enter),
                i.consume_key(Modifiers::NONE, Key::Escape),
            )
        });
        if down && !rows.is_empty() {
            self.palette_index = (self.palette_index + 1) % rows.len();
        }
        if up && !rows.is_empty() {
            self.palette_index = (self.palette_index + rows.len() - 1) % rows.len();
        }
        let mut run = enter.then(|| rows.get(self.palette_index).map(|r| r.run.clone())).flatten();
        let (hint, empty) = match self.palette_links {
            Some(LinkAction::Connect) => ("Buscar nós para conectar…", "Nenhum nó para conectar"),
            Some(LinkAction::Disconnect) => ("Buscar nós conectados…", "Nenhum nó conectado"),
            None => ("Procure menus, comandos e descubra gemas ocultas", "Nenhum resultado"),
        };

        let screen = ctx.content_rect();
        ctx.layer_painter(egui::LayerId::new(egui::Order::Foreground, egui::Id::unique("palette_dim"))).rect_filled(
            screen,
            0.0,
            Color32::from_rgba_unmultiplied(12, 12, 14, 89),
        );
        let selected_bg = if self.dark() { Color32::from_rgb(24, 24, 27) } else { Color32::from_rgb(0xf6, 0xf6, 0xf9) };
        let dialog =
            egui::Area::new(egui::Id::unique("command_palette")).anchor(egui::Align2::CENTER_TOP, (0.0, 47.0)).order(egui::Order::Tooltip).show(ctx, |ui| {
                egui::Frame::new()
                    .fill(c.island)
                    .corner_radius(12)
                    .stroke(Stroke::new(1.0, if self.dark() { Color32::from_rgb(60, 60, 66) } else { Color32::from_rgb(0xeb, 0xeb, 0xeb) }))
                    .inner_margin(egui::Margin { left: 24, right: 24, top: 24, bottom: 16 })
                    .show(ui, |ui| {
                        ui.set_width(672.0);
                        ui.spacing_mut().item_spacing = Vec2::new(4.0, 0.0);
                        egui::Frame::new()
                            .fill(c.surface_high)
                            .stroke(Stroke::new(1.0, PRIMARY))
                            .corner_radius(8)
                            .inner_margin(egui::Margin::symmetric(12, 8))
                            .show(ui, |ui| {
                                let input = ui.add(
                                    egui::TextEdit::singleline(&mut self.palette_query)
                                        .frame(egui::Frame::NONE)
                                        .font(FontId::proportional(16.0))
                                        .text_color(c.text)
                                        .hint_text(egui::RichText::new(hint).size(16.0).color(c.muted))
                                        .desired_width(f32::INFINITY),
                                );
                                if input.changed() {
                                    self.palette_index = 0;
                                }
                                if !self.palette_focused {
                                    input.request_focus();
                                    self.palette_focused = true;
                                }
                            });
                        ui.add_space(8.0);
                        ui.horizontal(|ui| {
                            let hints = [("↑↓", "Selecionar"), ("↵", "Confirmar"), ("Esc", "Fechar")];
                            let width = 230.0;
                            ui.add_space((ui.available_width() - width) / 2.0);
                            for (key, label) in hints {
                                key_chip(ui, key, &c);
                                ui.label(egui::RichText::new(label).size(10.0).color(c.muted));
                                ui.add_space(12.0);
                            }
                        });
                        ui.add_space(12.0);
                        let max_height = (screen.height() - 230.0).max(120.0);
                        egui::ScrollArea::vertical().max_height(max_height).auto_shrink([false, true]).show(ui, |ui| {
                            ui.set_width(ui.available_width());
                            let mut group = "";
                            if rows.is_empty() {
                                ui.add_space(8.0);
                                ui.label(egui::RichText::new(empty).size(14.0).color(c.muted));
                            }
                            for (k, row) in rows.iter().enumerate() {
                                if row.group != group {
                                    group = row.group;
                                    ui.add_space(if k == 0 { 0.0 } else { 16.0 });
                                    ui.label(egui::RichText::new(group).size(16.0).strong().color(c.text));
                                    ui.add_space(6.0);
                                }
                                let (rect, response) = ui.allocate_exact_size(Vec2::new(ui.available_width(), 40.0), Sense::click());
                                let response = response.on_hover_cursor(CursorIcon::PointingHand);
                                if response.hovered() && ui.input(|i| i.pointer.delta() != Vec2::ZERO) {
                                    self.palette_index = k;
                                }
                                let p = ui.painter();
                                if k == self.palette_index {
                                    p.rect_filled(rect, 8, selected_bg);
                                    if down || up {
                                        ui.scroll_to_rect(rect, None);
                                    }
                                }
                                paint_icon(p, row.icon, Pos2::new(rect.left() + 16.0, rect.center().y), c.text);
                                p.text(
                                    Pos2::new(rect.left() + 36.0, rect.center().y),
                                    egui::Align2::LEFT_CENTER,
                                    &row.label,
                                    FontId::proportional(16.0),
                                    c.text,
                                );
                                let mut x = rect.right() - 8.0;
                                let keys = keys_label(row.keys);
                                for key in shortcut_keys(&keys).iter().rev() {
                                    let galley = p.layout_no_wrap(key.to_string(), FontId::proportional(10.0), c.text);
                                    let chip =
                                        Rect::from_min_max(Pos2::new(x - galley.size().x - 8.0, rect.center().y - 8.0), Pos2::new(x, rect.center().y + 8.0));
                                    p.rect_filled(chip, 4, c.selected);
                                    p.galley(chip.center() - galley.size() / 2.0, galley, c.text);
                                    x = chip.left() - 4.0;
                                }
                                if response.clicked() {
                                    run = Some(row.run.clone());
                                }
                            }
                        });
                    });
            });
        let clicked_outside =
            ctx.input(|i| i.pointer.any_pressed()) && ctx.input(|i| i.pointer.interact_pos()).is_some_and(|p| !dialog.response.rect.contains(p));
        let picked = run.is_some();
        match run {
            Some(Run::Command(cmd)) => self.run_command(cmd),
            Some(Run::Node(id)) => self.pick_link_node(&id),
            None => {}
        }
        // a connect / disconnect command keeps the palette up for its node search
        if escape || clicked_outside || (picked && self.palette_links.is_none()) {
            self.palette_open = false;
            self.palette_links = None;
        }
    }
}
