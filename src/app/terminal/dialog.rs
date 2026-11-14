//! Novo terminal / Editar terminal: the reference app's sheet. The quick starts across the top, the
//! details under them (name, shell, command, folder and the switches). The look of every terminal is in
//! Ajustes › Terminal › Aparência.

use crate::prelude::*;
use egui::{Role, WidgetInfo};

/// The tiles of the quick starts: five to a row, 55 high, 3.25 apart (`gap-1`).
const TILE_HEIGHT: f32 = 55.0;
const TILE_GAP: f32 = 3.25;
const TILES_PER_ROW: usize = 5;

impl DrawingApp {
    pub(crate) fn terminal_dialog_ui(&mut self, ctx: &Context) {
        let Some(mut dialog) = self.terminal_dialog.clone() else { return };
        let c = self.colors();
        let editing = dialog.target.is_some();
        let title = if editing {
            "Editar Terminal"
        } else if dialog.chat {
            "Novo Chat"
        } else {
            "Novo Terminal"
        };
        // the quick starts switched on in Ajustes, 10 at most; a chat runs only the agents it speaks with
        let chat_agent = |preset: &Preset| matches!(preset.command.split_whitespace().next(), Some("claude" | "codex"));
        let presets: Vec<Preset> = self.prefs.presets.iter().filter(|preset| preset.on && (!dialog.chat || chat_agent(preset))).take(10).cloned().collect();
        let mut settings = false;
        let (confirm, cancel) = dialog_sheet(ctx, &c, title, if editing { "Salvar" } else { "Criar" }, 13.0, |ui| {
            let muted = dialog_label(&c);
            // one button makes both: a terminal or a chat card
            if !editing {
                let chat = dialog_segments(ui, "chat_or_terminal", &["Chat", "Terminal"], usize::from(!dialog.chat), &c) == 0;
                if chat && !dialog.chat && !matches!(dialog.spec.command.split_whitespace().next(), Some("claude" | "codex")) {
                    dialog.spec.command = self.prefs.presets.iter().find(|preset| chat_agent(preset)).map(|preset| preset.command.clone()).unwrap_or_default();
                }
                dialog.chat = chat;
            }
            let (head, _) = ui.allocate_exact_size(Vec2::new(DIALOG_WIDTH, 15.0), Sense::hover());
            ui.painter().text(head.left_center(), egui::Align2::LEFT_CENTER, "INÍCIO RÁPIDO", FontId::proportional(12.0), muted);
            let link = ui.painter().layout_no_wrap("Editar presets…".into(), FontId::proportional(12.0), PRIMARY);
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
                    let mark = TerminalSpec { command: preset.command.clone(), icon: preset.icon.clone(), ..Default::default() };
                    terminal_icon(ui.painter(), &mark, Pos2::new(rect.center().x, rect.top() + 6.5 + 12.0), ink);
                    let name = ui.painter().layout(preset.name.clone(), FontId::proportional(12.0), ink, tile.x - 6.5);
                    ui.painter().galley(Pos2::new(rect.center().x - name.size().x / 2.0, rect.top() + 6.5 + 24.0 + 3.25), name, ink);
                    if response.clicked() {
                        // the name follows the quick start, unless one was typed
                        let typed = dialog.spec.name.trim();
                        if typed.is_empty() || presets.iter().any(|p| p.name == typed) || typed == "Shell" {
                            dialog.spec.name = preset.name.clone();
                        }
                        dialog.spec.command = preset.command.clone();
                        // the mark and tint chosen for it in Ajustes
                        dialog.spec.icon = preset.icon.clone();
                        dialog.spec.color = preset.color;
                    }
                }
                ui.add_space(TILE_GAP);
            }
            ui.add_space(5.25);
            // what every terminal looks like is in Ajustes › Terminal › Aparência
            if dialog.chat {
                let folder = "ex. ~/projeto ou /home/usuario/projeto";
                dialog_folder(ui, "Pasta", &mut dialog.spec.cwd, folder, "Pasta do chat", &c, &mut self.prefs.folders);
            } else {
                // the name and the shell side by side, half the width each
                let half = (DIALOG_WIDTH - 12.0) / 2.0;
                ui.horizontal_top(|ui| {
                    ui.spacing_mut().item_spacing.x = 12.0;
                    ui.vertical(|ui| {
                        ui.set_width(half);
                        dialog_field(ui, "Nome do Terminal", &mut dialog.spec.name, "Nome do terminal", half, &c);
                    });
                    ui.vertical(|ui| {
                        ui.set_width(half);
                        dialog_caption(ui, "Shell", &c);
                        crate::app::terminal::prefs::shell_combo(ui, "terminal_shell", &mut dialog.spec.look.shell, &self.prefs.terminal.shell, half, &c);
                    });
                });
                dialog_field_mono(ui, "Comando", &mut dialog.spec.command, "ex: claude, codex, ou deixe vazio para shell", DIALOG_WIDTH, &c);
                let folder = "ex. ~/projeto ou /home/usuario/projeto";
                dialog_folder(ui, "Diretório de Trabalho", &mut dialog.spec.cwd, folder, "Pasta do terminal", &c, &mut self.prefs.folders);
                dialog_toggle(ui, &mut dialog.spec.maestro, "Maestro", "Promove este terminal a maestro, capaz de reger o restante do canvas.", &c);
                dialog_toggle(ui, &mut dialog.spec.chat, "Iniciar no modo de chat", "Um clique nele abre o chat em vez da tela.", &c);
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
