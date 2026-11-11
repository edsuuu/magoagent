//! Novo terminal / Editar terminal: the reference app's sheet. The quick starts across the top, the
//! tabs under them, then Detalhes (name, command, shell, folder and the switches) or Aparência (icon,
//! colour, theme).

use crate::app::terminal_prefs::{terminal_look_ui, terminal_prefs_ui};
use crate::prelude::*;
use egui::{Role, WidgetInfo};

/// The tiles of the quick starts: five to a row, 55 high, 3.25 apart (`gap-1`).
const TILE_HEIGHT: f32 = 55.0;
const TILE_GAP: f32 = 3.25;
const TILES_PER_ROW: usize = 5;
/// What follows a section of the form (`gap-5`).
const GAP_AFTER: f32 = 16.25;

impl DrawingApp {
    pub(crate) fn terminal_dialog_ui(&mut self, ctx: &Context) {
        let Some(mut dialog) = self.terminal_dialog.clone() else { return };
        let c = self.colors();
        let editing = dialog.target.is_some();
        let title = if editing { "Editar Terminal" } else { "Novo Terminal" };
        // the quick starts switched on in Ajustes, 10 at most
        let presets: Vec<Preset> = self.prefs.presets.iter().filter(|p| p.on).take(10).cloned().collect();
        let mut settings = false;
        let (confirm, cancel) = dialog_sheet(ctx, &c, title, if editing { "Salvar" } else { "Criar" }, 13.0, |ui| {
            let muted = dialog_label(&c);
            let (head, _) = ui.allocate_exact_size(Vec2::new(DIALOG_WIDTH, 15.0), Sense::hover());
            ui.painter().text(head.left_center(), egui::Align2::LEFT_CENTER, "INÍCIO RÁPIDO", FontId::proportional(11.0), muted);
            let link = ui.painter().layout_no_wrap("Editar presets…".into(), FontId::proportional(11.0), PRIMARY);
            let spot = Rect::from_min_size(Pos2::new(head.right() - link.size().x, head.top()), Vec2::new(link.size().x, 15.0));
            let response = ui.interact(spot, ui.scope_id().with("presets_link"), Sense::click()).on_hover_cursor(CursorIcon::PointingHand);
            response.widget_info(|| WidgetInfo::labeled(Role::Button, true, "Editar presets…"));
            ui.painter().galley(spot.min, link, PRIMARY);
            settings = response.clicked();
            ui.add_space(4.875);
            let tile = Vec2::new((DIALOG_WIDTH - (TILES_PER_ROW as f32 - 1.0) * TILE_GAP) / TILES_PER_ROW as f32, TILE_HEIGHT);
            for row in presets.chunks(TILES_PER_ROW) {
                let (line, _) = ui.allocate_exact_size(Vec2::new(DIALOG_WIDTH, TILE_HEIGHT), Sense::hover());
                for (i, preset) in row.iter().enumerate() {
                    let rect = Rect::from_min_size(line.min + Vec2::new(i as f32 * (tile.x + TILE_GAP), 0.0), tile);
                    let response = ui.interact(rect, ui.scope_id().with(("preset", i, &preset.name)), Sense::click()).on_hover_cursor(CursorIcon::PointingHand);
                    response.widget_info(|| WidgetInfo::labeled(Role::Button, true, preset.name.as_str()));
                    let on = dialog.spec.command == preset.command;
                    if on {
                        ui.painter().rect_filled(rect, 11.4, PRIMARY.gamma_multiply(0.1));
                    }
                    let ink = if on || response.hovered() { c.text } else { muted };
                    let mark = TerminalSpec { command: preset.command.clone(), ..Default::default() };
                    terminal_icon(ui.painter(), &mark, Pos2::new(rect.center().x, rect.top() + 6.5 + 12.0), ink);
                    let name = ui.painter().layout(preset.name.clone(), FontId::proportional(11.0), ink, tile.x - 6.5);
                    ui.painter().galley(Pos2::new(rect.center().x - name.size().x / 2.0, rect.top() + 6.5 + 24.0 + 3.25), name, ink);
                    if response.clicked() {
                        // the name follows the quick start, unless one was typed
                        let typed = dialog.spec.name.trim();
                        if typed.is_empty() || presets.iter().any(|p| p.name == typed) || typed == "Shell" {
                            dialog.spec.name = preset.name.clone();
                        }
                        dialog.spec.command = preset.command.clone();
                    }
                }
                ui.add_space(TILE_GAP);
            }
            ui.add_space(5.25);
            dialog_tabs(ui, &mut dialog.tab, &["Detalhes", "Aparência"], &c);
            if dialog.tab == 0 {
                dialog_field(ui, "Nome do Terminal", &mut dialog.spec.name, "Nome do terminal", DIALOG_WIDTH, &c);
                dialog_field_mono(ui, "Comando", &mut dialog.spec.command, "ex: claude, codex, ou deixe vazio para shell", DIALOG_WIDTH, &c);
                terminal_look_ui(ui, &mut dialog.spec.look, &self.prefs.terminal, &c, false);
                let folder = "ex. ~/projeto ou /home/usuario/projeto";
                dialog_folder(ui, "Diretório de Trabalho", &mut dialog.spec.cwd, folder, "Pasta do terminal", &c, &mut self.prefs.folders);
                let mut monitor = !dialog.spec.quiet;
                dialog_toggle(ui, &mut monitor, "Monitorar Atividade", "Detecta a saída do terminal e avisa quando o trabalho termina.", &c);
                dialog.spec.quiet = !monitor;
                dialog_toggle(ui, &mut dialog.spec.maestro, "Maestro", "Promove este terminal a maestro, capaz de reger o restante do canvas.", &c);
                dialog_toggle(ui, &mut dialog.spec.chat, "Iniciar no modo de chat", "Um clique nele abre o chat em vez da tela.", &c);
                if dialog.spec.command.trim() == "claude" {
                    let hint = "Ao reiniciar ou reabrir o app, o claude continua a conversa de antes (claude --resume).";
                    dialog_toggle(ui, &mut dialog.spec.resume, "Retomar a conversa ao reiniciar", hint, &c);
                }
                dialog_note(ui, "Agentes ligados a ele pelo botão Conectar conversam entre si com magoagent ask.", &c);
            } else {
                appearance_ui(ui, &mut dialog.spec, &c);
                terminal_look_ui(ui, &mut dialog.spec.look, &self.prefs.terminal, &c, true);
                egui::CollapsingHeader::new("Padrões de todos os terminais").show(ui, |ui| {
                    terminal_prefs_ui(ui, &mut self.prefs.terminal, &c);
                });
            }
        });
        if settings {
            self.settings_open = true;
        }
        if cancel {
            self.terminal_dialog = None;
            return;
        }
        self.terminal_dialog = Some(dialog);
        if confirm {
            self.confirm_terminal_dialog();
        }
    }
}

/// Ícone and Cor: the marks in a box (27 square, eight to a row), then the tints as 20 px dots.
fn appearance_ui(ui: &mut egui::Ui, spec: &mut TerminalSpec, c: &UiColors) {
    let tint = spec.color.map_or(c.text, |[r, g, b]| Color32::from_rgb(r, g, b));
    dialog_caption(ui, "Ícone", c);
    let rows = TERMINAL_ICONS.len().div_ceil(8);
    let (grid, _) = ui.allocate_exact_size(Vec2::new(DIALOG_WIDTH, rows as f32 * 31.0 + 13.0), Sense::hover());
    ui.painter().rect_stroke(grid, 8.125, Stroke::new(1.0, dialog_label(c).gamma_multiply(0.25)), StrokeKind::Inside);
    for (i, key) in TERMINAL_ICONS.iter().enumerate() {
        let cell = (DIALOG_WIDTH - 14.0) / 8.0;
        let center = grid.min + Vec2::new(7.0 + cell * ((i % 8) as f32 + 0.5), 6.5 + 31.0 * (i / 8) as f32 + 13.5);
        let rect = Rect::from_center_size(center, Vec2::splat(27.0));
        let response = ui.interact(rect, ui.scope_id().with(("icon", i)), Sense::click()).on_hover_cursor(CursorIcon::PointingHand);
        let name = TERMINAL_ICON_NAMES[i];
        response.widget_info(|| WidgetInfo::selected(Role::Switch, true, spec.icon == *key, name));
        if response.hovered() {
            ui.painter().rect_filled(rect, 8.125, c.text.gamma_multiply(0.08));
        }
        if spec.icon == *key {
            ui.painter().rect_stroke(rect, 8.125, Stroke::new(2.0, PRIMARY), StrokeKind::Inside);
        }
        if key.is_empty() {
            ui.painter().text(rect.center(), egui::Align2::CENTER_CENTER, "auto", FontId::proportional(10.0), dialog_label(c));
        } else {
            terminal_icon(ui.painter(), &TerminalSpec { icon: (*key).into(), ..Default::default() }, rect.center(), tint);
        }
        if response.tip(name).clicked() {
            spec.icon = (*key).into();
        }
    }
    ui.add_space(GAP_AFTER);
    dialog_caption(ui, "Cor", c);
    let (line, _) = ui.allocate_exact_size(Vec2::new(DIALOG_WIDTH, 24.0), Sense::hover());
    for (i, color) in TERMINAL_COLORS.iter().enumerate() {
        let rect = Rect::from_min_size(line.min + Vec2::new(26.0 * i as f32 + 2.0, 2.0), Vec2::splat(20.0));
        let response = ui.interact(rect, ui.scope_id().with(("tint", i)), Sense::click()).on_hover_cursor(CursorIcon::PointingHand);
        let name = if i == 0 { "Padrão".to_string() } else { format!("Cor {i}") };
        response.widget_info(|| WidgetInfo::selected(Role::Switch, true, spec.color == *color, name.clone()));
        match color {
            Some([r, g, b]) => ui.painter().circle_filled(rect.center(), 10.0, Color32::from_rgb(*r, *g, *b)),
            None => ui.painter().circle_stroke(rect.center(), 9.5, Stroke::new(1.0, dialog_label(c))),
        };
        if spec.color == *color {
            ui.painter().circle_stroke(rect.center(), 12.0, Stroke::new(2.0, PRIMARY));
        }
        if response.clicked() {
            spec.color = *color;
        }
    }
    ui.add_space(GAP_AFTER);
}
