//! The panels on the window's right, as in Claude Code: Alterações (Linha do tempo went: "remove o botão linha do tempo e o modal dele",
//! the Fichário: "não entendi muito bem para o que ele serve, remove", and Tarefas: "a aba de tarefas, ali no canvas remove por favor";
//! the tasks themselves stay, for the agents and the MCP's `tasks`). A button each in the title bar (`pane_buttons`); one open at a time, 340 wide under
//! the bar, the canvas narrower beside it (update.rs takes `pane_width` off its rect). Its left edge pulls it wider or
//! narrower ("precisa ter uma opção só para expandir a largura manual"), kept in the prefs; a double-click there gives
//! back the 340. X, Esc or the same button close it; the MCP's `panel` opens, closes and widens them for the Maestro.

pub(crate) mod changes;
mod doubts;
mod quick_note;

use crate::prelude::*;

pub(crate) const PANE_WIDTH: f32 = 340.0;
/// The narrowest the edge pulls the panel to, and the canvas the widest one still leaves beside it.
pub(crate) const PANE_MIN: f32 = 300.0;
pub(crate) const CANVAS_MIN: f32 = 360.0;
/// How far each side of the panel's left line takes a press to pull it.
const EDGE_GRAB: f32 = 4.0;
/// The panel starts under the title bar (the top row's buttons on the Mac).
const PANE_TOP: f32 = 60.0;
const HEAD: f32 = 46.0;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Pane {
    Changes,
    QuickNote,
    /// The agents' questions (app/doubts/): also over the chat.
    Doubts,
}

/// Each panel: its id (MCP), its name (screen) and its button's icon, in the buttons' order. Dúvidas is the last: over
/// the chat it is the only one, at the right end of the bar.
pub(crate) const PANES: [(Pane, &str, &str, Icon); 3] = [
    (Pane::Changes, "changes", "Alterações", Icon::Diff),
    (Pane::QuickNote, "note", "Nota rápida", Icon::Tool(Tool::Sticky)),
    (Pane::Doubts, "doubts", "Dúvidas", Icon::Question),
];

impl Pane {
    /// Shows over the chat too (the others are the canvas's).
    pub(crate) fn over_chat(self) -> bool {
        self == Pane::Doubts
    }
}

impl Pane {
    pub(crate) fn name(self) -> &'static str {
        PANES.iter().find(|p| p.0 == self).map_or("", |p| p.2)
    }
}

/// What the panels keep between frames.
#[derive(Default)]
pub(crate) struct Panes {
    pub(crate) open: Option<Pane>,
    /// Where the title bar's buttons are (none while hidden): the caption leaves them alone.
    pub(crate) buttons: Option<Rect>,
    pub(crate) changes: changes::ChangesPane,
    /// Nota rápida just opened: its text takes the keys.
    pub(crate) focus_note: bool,
    /// The width while the edge is pulled: the prefs take it when it is let go (not at every frame of the pull).
    pub(crate) pulled: Option<f32>,
    /// The window's width beside the sidebar, last frame: what the panel and the canvas share.
    pub(crate) room: f32,
}

/// The panel's width for `wanted` where it and the canvas share `room` (0: not known yet): from `PANE_MIN` to what
/// still leaves the canvas `CANVAS_MIN`.
pub(crate) fn clamp_pane_width(wanted: f32, room: f32) -> f32 {
    let widest = if room > 0.0 { (room - CANVAS_MIN).max(PANE_MIN) } else { f32::INFINITY };

    wanted.clamp(PANE_MIN, widest).round()
}

impl DrawingApp {
    /// How much of the window's right the open panel takes from the canvas.
    pub(crate) fn pane_width(&self) -> f32 {
        if self.panes.open.is_some() && !self.chat_open { self.panel_width() } else { 0.0 }
    }

    /// The panel's width, open or not: the one being pulled, else the one kept, else `PANE_WIDTH`.
    pub(crate) fn panel_width(&self) -> f32 {
        clamp_pane_width(self.panes.pulled.or(self.prefs.pane_width).unwrap_or(PANE_WIDTH), self.panes.room)
    }

    /// Opens `pane` (closing the one open) or, when it is the open one, closes it.
    pub(crate) fn toggle_pane(&mut self, pane: Pane) {
        self.set_pane(if self.panes.open == Some(pane) { None } else { Some(pane) });
    }

    pub(crate) fn set_pane(&mut self, pane: Option<Pane>) {
        if pane.is_some() && pane != self.panes.open {
            // what it shows is read again on opening
            self.panes.changes.read_at = None;
            self.panes.focus_note = pane == Some(Pane::QuickNote);
        }

        self.panes.open = pane;
    }

    /// The title bar's buttons, right to left from `right`, each with its badge; a click
    /// opens or closes its panel. Over the chat, only Dúvidas'.
    pub(crate) fn pane_buttons(&mut self, ui: &mut egui::Ui) {
        self.panes.buttons = None;

        // not on the welcome; over the chat only the panels that show there (`Pane::over_chat`)
        if self.welcome.is_some() {
            return;
        }

        let (dark, width) = (self.dark(), ui.ctx().content_rect().width());
        let ink = glass_ink(dark);
        let p = ui.painter().clone();

        let counts = PANES.map(|(pane, ..)| match pane {
            Pane::Changes => self.changed_file_count(),
            Pane::Doubts => self.doubts.unanswered(),
            Pane::QuickNote => 0,
        });

        for (k, (pane, _, name, icon)) in PANES.into_iter().enumerate() {
            // over the chat only Dúvidas
            if self.chat_open && !pane.over_chat() {
                continue;
            }

            let rect = self.pane_button(width, k);

            self.panes.buttons = Some(self.panes.buttons.map_or(rect, |span| span.union(rect)));
            let button = ui.interact(rect, egui::Id::unique(("pane_button", k)), Sense::click()).on_hover_cursor(CursorIcon::PointingHand).tip(name);
            let on = self.panes.open == Some(pane);

            paint_glass(&p, rect, rect.width() / 2.0, dark, false);

            if button.hovered() || on {
                p.circle_filled(rect.center(), GLASS_BUTTON / 2.0, ink.gamma_multiply(0.1));
            }

            paint_icon_scaled(&p, icon, rect.center(), if on { glass_accent(dark) } else { ink.gamma_multiply(0.85) }, GLASS_ICON, 0.0);

            if counts[k] > 0 {
                let text = if counts[k] > 99 { "99+".to_string() } else { counts[k].to_string() };
                let galley = p.layout_no_wrap(text, FontId::proportional(9.5), Color32::WHITE);
                let pill = Rect::from_center_size(rect.right_top() + Vec2::new(-4.0, 5.0), Vec2::new((galley.size().x + 7.0).max(14.0), 14.0));

                p.rect_filled(pill, 7, glass_accent(dark));
                p.galley(pill.center() - galley.size() / 2.0, galley, Color32::WHITE);
            }

            if button.clicked() && live(ui) {
                self.toggle_pane(pane);
            }
        }
    }

    /// Where the open panel sits in a window of `screen`.
    pub(crate) fn pane_rect(&self, screen: Rect) -> Rect {
        Rect::from_min_max(Pos2::new(screen.right() - self.panel_width(), PANE_TOP), screen.max)
    }

    /// The panel's left edge: pulled, it widens or narrows the panel (the canvas gives or takes the difference);
    /// double-clicked, it goes back to `PANE_WIDTH`.
    fn pane_edge(&mut self, ui: &mut egui::Ui, rect: Rect) {
        let edge = Rect::from_min_max(Pos2::new(rect.left() - EDGE_GRAB, rect.top()), Pos2::new(rect.left() + EDGE_GRAB, rect.bottom()));
        let pull = ui.interact(edge, egui::Id::unique("pane_edge"), Sense::click_and_drag()).on_hover_cursor(CursorIcon::ResizeHorizontal);

        if pull.dragged()
            && let Some(at) = pull.interact_pointer_pos()
        {
            self.panes.pulled = Some(clamp_pane_width(rect.right() - at.x, self.panes.room));
            ui.ctx().set_cursor_icon(CursorIcon::ResizeHorizontal);
        } else if let Some(width) = self.panes.pulled.take() {
            self.prefs.pane_width = Some(width);
        }

        if pull.double_clicked() && live(ui) {
            (self.prefs.pane_width, self.panes.pulled) = (None, None);
        }

        if pull.hovered() || pull.dragged() {
            ui.painter().vline(rect.left(), rect.y_range(), Stroke::new(2.0, glass_accent(self.dark())));
        }
    }

    /// The open panel (and, on the Mac, whose window has no drawn bar, its buttons in the top row).
    pub(crate) fn panes_ui(&mut self, ctx: &Context) {
        if TITLE_BAR == 0.0 {
            egui::Area::new(egui::Id::unique("pane_buttons")).fixed_pos(Pos2::ZERO).order(egui::Order::Foreground).show(ctx, |ui| self.pane_buttons(ui));
        }

        self.panes.room = ctx.content_rect().width() - self.sidebar_width();
        let Some(pane) = self.panes.open.filter(|pane| !self.chat_open || pane.over_chat()) else { return };
        let rect = self.pane_rect(ctx.content_rect());
        let (colors, ink) = (self.colors(), glass_ink(self.dark()));
        let line = ink.gamma_multiply(0.12);
        let area = egui::Area::new(egui::Id::unique("side_pane")).order(egui::Order::Foreground).fixed_pos(rect.min).constrain(false);

        area.show(ctx, |ui| {
            // the whole panel is its own: no press falls through to what is under it
            ui.allocate_rect(rect, Sense::click_and_drag());
            let p = ui.painter().clone();

            p.rect_filled(rect, 0, colors.island);
            p.vline(rect.left(), rect.y_range(), Stroke::new(1.0, line));
            self.pane_edge(ui, rect);
            let head = Rect::from_min_size(rect.min, Vec2::new(rect.width(), HEAD));

            p.text(head.left_center() + Vec2::new(14.0, 0.0), egui::Align2::LEFT_CENTER, pane.name(), FontId::proportional(14.0), colors.text);
            p.hline(rect.x_range(), head.bottom(), Stroke::new(1.0, line));
            let close = Rect::from_center_size(head.right_center() - Vec2::new(22.0, 0.0), Vec2::splat(28.0));
            let button = ui.interact(close, egui::Id::unique("pane_close"), Sense::click()).on_hover_cursor(CursorIcon::PointingHand).tip("Fechar");

            if button.hovered() {
                p.rect_filled(close, 6, ink.gamma_multiply(0.1));
            }

            for (from, to) in [(Vec2::new(-4.5, -4.5), Vec2::new(4.5, 4.5)), (Vec2::new(-4.5, 4.5), Vec2::new(4.5, -4.5))] {
                p.line_segment([close.center() + from, close.center() + to], Stroke::new(1.25, ink.gamma_multiply(0.85)));
            }

            if button.clicked() && live(ui) {
                self.set_pane(None);

                return;
            }

            // Alterações' comment on the lines picked, Nota rápida's Levar para o canvas: a bar pinned to the bottom, the
            // rest scrolling above it
            let bar = match pane {
                Pane::Changes => self.comment_bar_height(),
                Pane::QuickNote => 48.0,
                Pane::Doubts => 0.0,
            };

            let body = Rect::from_min_max(Pos2::new(rect.left() + 12.0, head.bottom() + 12.0), rect.max - Vec2::new(12.0, bar));

            if bar > 0.0 {
                let strip = Rect::from_min_max(Pos2::new(rect.left(), rect.bottom() - bar), rect.max);

                p.hline(strip.x_range(), strip.top(), Stroke::new(1.0, line));

                ui.scope_builder(egui::UiBuilder::new().max_rect(strip.shrink2(Vec2::new(12.0, 8.0))), |ui| match pane {
                    Pane::Changes => self.review_comment_bar(ui),
                    Pane::QuickNote => self.quick_note_bar(ui),
                    Pane::Doubts => {}
                });
            }

            ui.scope_builder(egui::UiBuilder::new().max_rect(body), |ui| {
                egui::ScrollArea::vertical().id_salt(("pane_scroll", pane.name())).auto_shrink(false).show(ui, |ui| {
                    ui.spacing_mut().item_spacing.y = 10.0;

                    match pane {
                        Pane::Changes => self.changes_pane(ui),
                        Pane::QuickNote => self.quick_note_pane(ui, body.height() - 12.0),
                        Pane::Doubts => self.doubts_pane(ui),
                    }

                    ui.add_space(12.0);
                });
            });
        });
    }

    /// The MCP's `panel`: opens `name` (changes, note, doubts), closes it with `open`
    /// false, toggles it with no `open`; no name with `open` false closes whichever is open. `width` pulls the
    /// panel's edge (0: back to the default); `worktree` shows Alterações on that worktree ("todos": all the trees).
    pub(crate) fn act_panel(&mut self, args: &Value) -> Result<String, String> {
        let mut name = args["name"].as_str().unwrap_or_default().trim().to_lowercase();
        let mut open = args["open"].as_bool();
        let mut said = Vec::new();

        if let Some(width) = args["width"].as_f64() {
            self.prefs.pane_width = (width > 0.0).then(|| clamp_pane_width(width as f32, self.panes.room));
            said.push(format!("Largura do painel: {} px.", self.panel_width()));
        }

        if let Some(wanted) = args["worktree"].as_str().map(str::trim).filter(|w| !w.is_empty()) {
            said.push(self.choose_worktree(wanted)?);

            // the worktree is Alterações': it shows there
            if name.is_empty() && open != Some(false) {
                (name, open) = ("changes".into(), Some(true));
            }
        }

        if name.is_empty() && open.is_none() && !said.is_empty() {
            return Ok(said.join(" "));
        }

        let done = self.act_panel_open(&name, open)?;

        Ok([done].into_iter().chain(said).collect::<Vec<_>>().join(" "))
    }

    /// `act_panel`'s opening and closing.
    fn act_panel_open(&mut self, name: &str, open: Option<bool>) -> Result<String, String> {
        if name.is_empty() {
            if open == Some(false) || open.is_none() {
                self.set_pane(None);

                return Ok("Painel fechado.".into());
            }

            return Err("Diga qual painel (name): changes, note ou doubts.".into());
        }

        let pane = PANES.iter().find(|p| p.1 == name || p.2.to_lowercase() == name).map(|p| p.0);
        let pane = pane.ok_or_else(|| format!("Painel desconhecido: {name}. Use changes, note ou doubts."))?;
        let wanted = open.unwrap_or(self.panes.open != Some(pane));

        if wanted {
            // the canvas's panels leave the chat; Dúvidas opens over it
            self.chat_open &= pane.over_chat();
            self.set_pane(Some(pane));
            Ok(format!("Painel {} aberto.", pane.name()))
        } else {
            if self.panes.open == Some(pane) {
                self.set_pane(None);
            }

            Ok(format!("Painel {} fechado.", pane.name()))
        }
    }
}

/// The frame's first pass: a pass egui runs again (a new area sizes itself) sees the same
/// clicks, so what a click does happens only in the first.
fn live(ui: &egui::Ui) -> bool {
    ui.ctx().current_pass_index() == 0
}

/// A box with the panels' look around `add`.
fn card<R>(ui: &mut egui::Ui, dark: bool, add: impl FnOnce(&mut egui::Ui) -> R) -> R {
    let ink = glass_ink(dark);
    let frame = egui::Frame::new().fill(ink.gamma_multiply(0.04)).stroke(Stroke::new(1.0, ink.gamma_multiply(0.12))).corner_radius(8).inner_margin(8);

    frame
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            add(ui)
        })
        .inner
}
