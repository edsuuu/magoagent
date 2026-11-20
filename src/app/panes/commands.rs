//! Comandos: the terminals that run a command to watch (tests, `php artisan serve`), kept off the canvas so they do not
//! crowd it. Each is a row that opens like an accordion: its name, command, folder and state; open, its screen live with
//! Parar, Reiniciar and Levar para o canvas. The terminal is the same element and the same shell (`TerminalSpec::docked`),
//! only shown here: the agents' `create_terminal` puts theirs here, and the terminal's menu (Mover para Comandos) or the
//! MCP's `terminal_menu` dock moves one either way.

use crate::app::ui::links::rope_ends;
use crate::prelude::*;
use std::collections::{HashMap, HashSet};

/// The rows an open screen shows.
const ROWS: usize = 16;
/// How often the processes under each shell are read again for the state.
const REFRESH_SECONDS: f64 = 3.0;
/// What a screen's width loses to the panel's margins, the row's box and the scroll bar.
const INSET: f32 = 48.0;

/// What Comandos keeps between frames.
#[derive(Default)]
pub(crate) struct CommandsPane {
    /// The rows open, by terminal id.
    pub(crate) open: HashSet<String>,
    /// Per terminal, whether a program runs under its shell, read every `REFRESH_SECONDS` while the panel shows.
    running: HashMap<String, bool>,
    read_at: Option<f64>,
}

/// A row's state.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum CommandState {
    /// A program runs under its shell (or it printed just now).
    Running,
    /// Back at the shell's prompt.
    Done,
    /// Its shell is gone (or never opened).
    Ended,
    /// Descarregado, or its shell still opening.
    Waiting,
}

impl CommandState {
    pub(crate) fn label(self) -> &'static str {
        match self {
            CommandState::Running => "Rodando",
            CommandState::Done => "Terminou",
            CommandState::Ended => "Encerrado",
            CommandState::Waiting => "Parado",
        }
    }

    fn color(self) -> Color32 {
        match self {
            CommandState::Running => PRIMARY,
            CommandState::Done => Color32::from_rgb(0x2f, 0x9e, 0x44),
            CommandState::Ended => Color32::from_rgb(0xe0, 0x31, 0x31),
            CommandState::Waiting => Color32::from_gray(0x8a),
        }
    }
}

/// What a row's buttons asked for.
enum CommandAction {
    Toggle(String),
    Stop(String),
    Restart(String),
    ToCanvas(String),
}

/// A terminal in Comandos, or a rope tied to one: neither is on the canvas (`hidden_page`).
pub(crate) fn off_canvas(elements: &[Element], element: &Element) -> bool {
    element.docked()
        || element.rope && rope_ends(element).is_some_and(|(start, end)| elements.iter().any(|other| other.docked() && (other.id == start || other.id == end)))
}

/// The grid a terminal in Comandos gets in a panel `width` wide, at font size `font`.
pub(crate) fn docked_grid(width: f32, font: f32) -> (usize, usize) {
    (((width - INSET - 2.0 * TERMINAL_PAD) / cell(font).x).floor().max(20.0) as usize, ROWS)
}

impl DrawingApp {
    /// The terminals in Comandos, in the drawing's order.
    pub(crate) fn docked_ids(&self) -> Vec<String> {
        self.scene.elements.iter().filter(|element| element.docked()).map(|element| element.id.clone()).collect()
    }

    /// The ones whose screen shows now: Comandos open beside the canvas, their row open.
    pub(crate) fn docked_shown(&self) -> Vec<String> {
        if self.panes.open != Some(super::Pane::Commands) || self.chat_open {
            return Vec::new();
        }

        self.docked_ids().into_iter().filter(|id| self.panes.commands.open.contains(id)).collect()
    }

    pub(crate) fn command_state(&self, id: &str) -> CommandState {
        match self.terminals.get(id) {
            Some(Ok(session)) if session.exited() => CommandState::Ended,
            Some(Ok(session)) if session.handle.busy() || self.panes.commands.running.get(id) == Some(&true) => CommandState::Running,
            Some(Ok(_)) => CommandState::Done,
            Some(Err(_)) => CommandState::Ended,
            None => CommandState::Waiting,
        }
    }

    /// Puts terminal `id` in Comandos (`on`) or back on the canvas, as one undo step. Back on the canvas it takes a free
    /// spot beside the one it is tied to, else in the middle of the view, and the view goes to it.
    pub(crate) fn set_docked(&mut self, id: &str, on: bool) -> Result<String, String> {
        let index = self.terminal_index(id).ok_or("Esse terminal não está mais no canvas.")?;
        let name = self.scene.elements[index].terminal.as_ref().map_or("Terminal", |terminal| terminal.label()).to_string();

        if self.scene.elements[index].docked() == on {
            return Ok(format!("{name} já estava {}.", if on { "em Comandos" } else { "no canvas" }));
        }

        self.snapshot();

        if on {
            self.panes.commands.open.insert(id.to_string());
            self.selected.retain(|selected| *selected != index);

            if self.terminal_focus.as_deref() == Some(id) {
                self.terminal_focus = None;
            }

            if self.terminal_elevated.as_deref() == Some(id) {
                self.terminal_elevated = None;
            }
        } else {
            let size = local_bounds(&self.scene.elements[index]).size();
            let partner = self.links().get(id).and_then(|linked| linked.first().cloned());
            let around = partner.and_then(|partner| self.scene.elements.iter().find(|element| element.id == partner && !element.docked())).map(local_bounds);
            let middle = Rect::from_center_size(self.to_world(self.canvas_rect.center()), size);
            let want = around.map_or(middle, |around| Rect::from_min_size(Pos2::new(around.max.x + 60.0, around.min.y), size));
            let at = self.free_spot_near(want, around.unwrap_or(middle));
            let from = local_bounds(&self.scene.elements[index]).min;

            self.scene.elements[index].move_by(at.min - from);
            self.selected = vec![index];
            self.glide_to(at.center());
        }

        if let Some(terminal) = self.scene.elements[index].terminal.as_mut() {
            terminal.docked = on;
        }

        self.update_bindings();
        self.publish_nodes();

        Ok(format!("{name} {}.", if on { "foi para Comandos" } else { "voltou para o canvas" }))
    }

    /// A terminal in Comandos shows there (the panel opens on its row open) instead of on the canvas; false for any other.
    pub(crate) fn show_docked(&mut self, id: &str) -> bool {
        if !self.scene.elements.iter().any(|element| element.id == id && element.docked()) {
            return false;
        }

        self.chat_open = false;
        self.panes.commands.open.insert(id.to_string());
        self.set_pane(Some(super::Pane::Commands));
        true
    }

    /// Reads which shells have a program under them, every `REFRESH_SECONDS` while the panel shows.
    fn read_command_processes(&mut self, ctx: &Context) {
        ctx.request_repaint_after(std::time::Duration::from_secs_f64(REFRESH_SECONDS));

        if self.panes.commands.read_at.is_some_and(|at| self.now - at < REFRESH_SECONDS) {
            return;
        }

        let shells: Vec<(String, u32)> = self
            .docked_ids()
            .into_iter()
            .filter_map(|id| {
                let pid = self.terminals.get(&id)?.as_ref().ok().filter(|session| !session.exited())?.shell_pid;

                Some((id, pid))
            })
            .collect();

        // ponytail: one process table a refresh on the UI thread, as the processes popover reads it; move it to a thread if it shows
        let table = if shells.is_empty() { HashMap::new() } else { crate::transcript::process::table() };

        self.panes.commands.running = shells.into_iter().map(|(id, pid)| (id, !crate::app::terminal::procs::descendants(&table, pid).is_empty())).collect();
        self.panes.commands.read_at = Some(self.now);
    }

    /// The panel's body.
    pub(crate) fn commands_pane(&mut self, ui: &mut egui::Ui) {
        let (colors, dark) = (self.colors(), self.dark());
        let ids = self.docked_ids();

        if ids.is_empty() {
            ui.add(
                egui::Label::new(
                    egui::RichText::new(
                        "Nenhum comando agora. Os terminais que os agentes abrem para rodar testes ou servidores ficam aqui, sem ocupar o canvas; \
                         no menu de um terminal do canvas, Mover para Comandos traz ele para cá.",
                    )
                    .size(13.0)
                    .color(colors.muted),
                )
                .wrap(),
            );

            return;
        }

        self.read_command_processes(ui.ctx());
        let mut action = None;

        for id in &ids {
            let Some(spec) = self.scene.elements.iter().find(|element| &element.id == id).and_then(|element| element.terminal.clone()) else { continue };
            let (state, open) = (self.command_state(id), self.panes.commands.open.contains(id));

            ui.push_id(id, |ui| {
                super::card(ui, dark, |ui| {
                    ui.spacing_mut().item_spacing.y = 6.0;
                    let (head, response) = ui.allocate_exact_size(Vec2::new(ui.available_width(), 20.0), Sense::click());
                    let response = response.on_hover_cursor(CursorIcon::PointingHand).tip(if open { "Recolher" } else { "Mostrar a saída" });
                    let painter = ui.painter();
                    // the chevron: right when closed, down when open
                    let tip = head.left_center() + Vec2::new(5.0, 0.0);

                    let points = if open {
                        vec![tip + Vec2::new(-4.0, -2.0), tip + Vec2::new(4.0, -2.0), tip + Vec2::new(0.0, 3.0)]
                    } else {
                        vec![tip + Vec2::new(-2.0, -4.0), tip + Vec2::new(3.0, 0.0), tip + Vec2::new(-2.0, 4.0)]
                    };

                    painter.add(egui::Shape::convex_polygon(points, colors.muted, Stroke::NONE));
                    let status = painter.layout_no_wrap(state.label().into(), FontId::proportional(11.5), colors.muted);
                    let status_at = Pos2::new(head.right() - status.size().x, head.center().y - status.size().y / 2.0);

                    painter.circle_filled(Pos2::new(status_at.x - 8.0, head.center().y), 3.5, state.color());
                    painter.galley(status_at, status, colors.muted);
                    let words = Rect::from_min_max(Pos2::new(head.left() + 16.0, head.top()), Pos2::new(status_at.x - 18.0, head.bottom()));

                    painter.with_clip_rect(words.intersect(painter.clip_rect())).text(
                        words.left_center(),
                        egui::Align2::LEFT_CENTER,
                        spec.label(),
                        FontId::new(13.5, egui::FontFamily::Name(SYSTEM_SEMIBOLD.into())),
                        colors.text,
                    );

                    if response.clicked() {
                        action = Some(CommandAction::Toggle(id.clone()));
                    }

                    let command = if spec.command.trim().is_empty() { "(só o shell)" } else { spec.command.trim() };

                    ui.add(egui::Label::new(egui::RichText::new(command).monospace().size(11.5).color(colors.text)).truncate());
                    ui.add(egui::Label::new(egui::RichText::new(display_folder(&spec.cwd)).size(11.5).color(colors.muted)).truncate());

                    if !open {
                        return;
                    }

                    let (look, font) = self.terminal_look(&spec);
                    let height = ROWS as f32 * cell(font).y + 2.0 * TERMINAL_PAD;
                    let (screen, _) = ui.allocate_exact_size(Vec2::new(ui.available_width(), height), Sense::hover());
                    let painter = ui.painter_at(screen);

                    painter.rect_filled(screen, 6, look.bg);

                    match self.terminals.get(id) {
                        Some(Ok(session)) => {
                            let view = GridView { look: &look, font, marks: &[], text_zoom: 1.0 };

                            session.paint(&painter, screen.min + Vec2::splat(TERMINAL_PAD), 1.0, false, &view);
                        }
                        other => {
                            let note = match other {
                                Some(Err(error)) => format!("Não foi possível abrir o shell: {error}"),
                                _ if spec.unloaded => "Este terminal está descarregado.".into(),
                                _ => "Abrindo…".into(),
                            };

                            painter.text(screen.center(), egui::Align2::CENTER_CENTER, note, FontId::proportional(12.0), look.muted());
                        }
                    }

                    ui.horizontal_wrapped(|ui| {
                        let stop = ui.add_enabled(state == CommandState::Running, egui::Button::new("Parar")).on_hover_text("Manda Ctrl+C ao comando");

                        if stop.clicked() {
                            action = Some(CommandAction::Stop(id.clone()));
                        }

                        if ui.button("Reiniciar").on_hover_text("O shell e o comando começam de novo").clicked() {
                            action = Some(CommandAction::Restart(id.clone()));
                        }

                        if ui.button("Levar para o canvas").on_hover_text("Vira um cartão de terminal no canvas, o mesmo shell").clicked() {
                            action = Some(CommandAction::ToCanvas(id.clone()));
                        }
                    });
                });
            });
        }

        match action.filter(|_| super::live(ui)) {
            Some(CommandAction::Toggle(id)) => {
                if !self.panes.commands.open.remove(&id) {
                    self.panes.commands.open.insert(id);
                }
            }
            Some(CommandAction::Stop(id)) => {
                if let Some(Ok(session)) = self.terminals.get(&id) {
                    session.handle.write(vec![0x03]);
                }
            }
            Some(CommandAction::Restart(id)) => {
                self.restart_node(id);
                self.terminal_focus = None;
                self.panes.commands.read_at = None;
            }
            Some(CommandAction::ToCanvas(id)) => {
                if let Err(why) = self.set_docked(&id, false) {
                    self.toast(why);
                }
            }
            None => {}
        }
    }
}
