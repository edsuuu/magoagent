//! The panels on the window's right, as in Claude Code: Alterações (Linha do tempo went: "remove o botão linha do tempo e o modal dele",
//! the Fichário: "não entendi muito bem para o que ele serve, remove", and Tarefas: "a aba de tarefas, ali no canvas remove por favor";
//! the tasks themselves stay, for the agents and the MCP's `tasks`). A button each in the title bar (`pane_buttons`); one open at a time, 340 wide under
//! the bar, the canvas narrower beside it (update.rs takes `pane_width` off its rect). X, Esc or
//! the same button close it; the MCP's `panel` opens and closes them for the Maestro.

mod changes;

use crate::prelude::*;

pub(crate) const PANE_WIDTH: f32 = 340.0;
/// The panel starts under the title bar (the top row's buttons on the Mac).
const PANE_TOP: f32 = 60.0;
const HEAD: f32 = 46.0;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Pane {
    Changes,
}

/// Each panel: its id (MCP), its name (screen) and its button's icon, in the buttons' order.
pub(crate) const PANES: [(Pane, &str, &str, Icon); 1] = [(Pane::Changes, "changes", "Alterações", Icon::Diff)];

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
}

impl DrawingApp {
    /// How much of the window's right the open panel takes from the canvas.
    pub(crate) fn pane_width(&self) -> f32 {
        if self.panes.open.is_some() && !self.chat_open { PANE_WIDTH } else { 0.0 }
    }

    /// Opens `pane` (closing the one open) or, when it is the open one, closes it.
    pub(crate) fn toggle_pane(&mut self, pane: Pane) {
        self.set_pane(if self.panes.open == Some(pane) { None } else { Some(pane) });
    }

    pub(crate) fn set_pane(&mut self, pane: Option<Pane>) {
        if pane.is_some() && pane != self.panes.open {
            // what it shows is read again on opening
            self.panes.changes.read_at = None;
        }
        self.panes.open = pane;
    }

    /// The title bar's buttons, right to left from `right`, each with its badge; a click
    /// opens or closes its panel. Hidden over the chat, which the panels don't cover.
    pub(crate) fn pane_buttons(&mut self, ui: &mut egui::Ui) {
        self.panes.buttons = None;
        // over the chat, which the panels don't cover, and on the welcome (asked: "eles nem devem aparecer na intro")
        if self.chat_open || self.welcome.is_some() {
            return;
        }
        let (dark, width) = (self.dark(), ui.ctx().content_rect().width());
        let ink = glass_ink(dark);
        let p = ui.painter().clone();
        let counts = [self.changed_file_count()];
        for (k, (pane, _, name, icon)) in PANES.into_iter().enumerate() {
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
    pub(crate) fn pane_rect(screen: Rect) -> Rect {
        Rect::from_min_max(Pos2::new(screen.right() - PANE_WIDTH, PANE_TOP), screen.max)
    }

    /// The open panel (and, on the Mac, whose window has no drawn bar, its buttons in the top row).
    pub(crate) fn panes_ui(&mut self, ctx: &Context) {
        if TITLE_BAR == 0.0 {
            egui::Area::new(egui::Id::unique("pane_buttons")).fixed_pos(Pos2::ZERO).order(egui::Order::Foreground).show(ctx, |ui| self.pane_buttons(ui));
        }
        let Some(pane) = self.panes.open.filter(|_| !self.chat_open) else { return };
        let rect = Self::pane_rect(ctx.content_rect());
        let (colors, ink) = (self.colors(), glass_ink(self.dark()));
        let line = ink.gamma_multiply(0.12);
        let area = egui::Area::new(egui::Id::unique("side_pane")).order(egui::Order::Foreground).fixed_pos(rect.min).constrain(false);
        area.show(ctx, |ui| {
            // the whole panel is its own: no press falls through to what is under it
            ui.allocate_rect(rect, Sense::click_and_drag());
            let p = ui.painter().clone();
            p.rect_filled(rect, 0, colors.island);
            p.vline(rect.left(), rect.y_range(), Stroke::new(1.0, line));
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
            let body = Rect::from_min_max(Pos2::new(rect.left() + 12.0, head.bottom() + 12.0), rect.max - Vec2::new(12.0, 0.0));
            ui.scope_builder(egui::UiBuilder::new().max_rect(body), |ui| {
                egui::ScrollArea::vertical().id_salt(("pane_scroll", pane.name())).auto_shrink(false).show(ui, |ui| {
                    ui.spacing_mut().item_spacing.y = 10.0;
                    match pane {
                        Pane::Changes => self.changes_pane(ui),
                    }
                    ui.add_space(12.0);
                });
            });
        });
    }

    /// The MCP's `panel`: opens `name` (changes), closes it with `open`
    /// false, toggles it with no `open`; no name with `open` false closes whichever is open.
    pub(crate) fn act_panel(&mut self, args: &Value) -> Result<String, String> {
        let name = args["name"].as_str().unwrap_or_default().trim().to_lowercase();
        let open = args["open"].as_bool();
        if name.is_empty() {
            if open == Some(false) || open.is_none() {
                self.set_pane(None);
                return Ok("Painel fechado.".into());
            }
            return Err("Diga qual painel (name): changes.".into());
        }
        let pane = PANES.iter().find(|p| p.1 == name || p.2.to_lowercase() == name).map(|p| p.0);
        let pane = pane.ok_or_else(|| format!("Painel desconhecido: {name}. Use changes."))?;
        let wanted = open.unwrap_or(self.panes.open != Some(pane));
        if wanted {
            self.chat_open = false;
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
