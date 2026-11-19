//! The plans' use at the sidebar's foot: a line per agent whose CLI answered (Claude Code, Codex) with the bar of its
//! 5-hour session, amber from 80%; a click opens what each one read (the session, the week, the special limit and when
//! they renew) and the Maestro's ceiling, as wide as the sidebar.

use crate::app::ui::agents::AMBER;
use crate::app::ui::menu_look::{MenuColors, MenuIcon, menu_colors, paint_menu_icon_sized};
use crate::app::ui::sidebar::Tones;
use crate::app::usage::probe::{Agent, Meter, Probe, Reading, reset_when, updated_text};
use crate::prelude::*;

/// The foot's popover and the readings it shows.
#[derive(Default)]
pub(crate) struct UsageCard {
    pub(crate) open: bool,
    pub(crate) probe: Probe,
}

/// One agent's line in the foot.
const LINE: f32 = 34.0;
/// From here a bar is amber: the plan is near its end.
const HOT: f64 = 0.8;

/// How tall the foot's lines are for `count` agents (none without a reading).
pub(crate) fn usage_height(count: usize) -> f32 {
    if count == 0 { 0.0 } else { count as f32 * LINE + 8.0 }
}

/// A meter as the popover names it: the session says its 5 hours, the week is the week.
fn meter_name(meter: &Meter) -> &str {
    match meter.title.as_str() {
        "Sessão" => "Sessão 5 h",
        "Semanal" => "Semana",
        "Mensal" => "Mês",
        other => other,
    }
}

/// A bar `used` (0 to 1) full in `rect`: the accent, amber near the end.
fn paint_bar(painter: &Painter, rect: Rect, used: f64, ground: Color32, accent: Color32) {
    painter.rect_filled(rect, 2, ground);
    let fill = Rect::from_min_size(rect.min, Vec2::new(rect.width() * used.clamp(0.0, 1.0) as f32, rect.height()));

    painter.rect_filled(fill, 2, if used >= HOT { AMBER } else { accent });
}

/// "Sessão renova hoje às 22:30 · semana seg, 17/11 às 05:00", from the meters that say when.
fn renew_text(reading: &Reading, now: i64) -> Option<String> {
    let when = |title: &str| {
        let at = reading.meters.iter().find(|meter| meter.title == title)?.resets_at?;

        Some(reset_when(at, now, crate::app::chat::viewer::local_offset(at * 1000)))
    };

    // the longer window: the week, or Codex Go's month
    let long = when("Semanal").map(|at| ("semana", at)).or_else(|| when("Mensal").map(|at| ("mês", at)));

    match (when("Sessão"), long) {
        (Some(session), Some((name, at))) => Some(format!("Sessão renova {session} · {name} {at}")),
        (Some(session), None) => Some(format!("Sessão renova {session}")),
        (None, Some(("mês", at))) => Some(format!("Mês renova {at}")),
        (None, Some((_, at))) => Some(format!("Semana renova {at}")),
        (None, None) => None,
    }
}

/// What the popover's buttons asked for.
#[derive(PartialEq)]
enum CardAction {
    Refresh,
    Settings,
}

impl DrawingApp {
    /// Each frame, with the sidebar open or not: the CLIs are asked where a drawing is saved (the app proper), every 5
    /// minutes, and their answers taken.
    pub(crate) fn usage_poll(&mut self, ctx: &Context) {
        let now = ctx.input(|input| input.time);
        let probe = &mut self.usage_ui.probe;

        if self.autosave.is_some() && now >= probe.next {
            probe.refresh(now);
        }

        if probe.collect() {
            ctx.request_repaint();
        }

        ctx.request_repaint_after(std::time::Duration::from_secs_f64(if probe.busy() { 0.5 } else { (probe.next - now).clamp(0.5, 30.0) }));
    }

    /// The agents with a reading, in their order.
    pub(crate) fn usage_readings(&self) -> Vec<(Agent, Reading)> {
        self.usage_ui.probe.shown().into_iter().map(|(agent, reading)| (agent, reading.clone())).collect()
    }

    /// The lines in `area` (over Novo grupo), and the popover over them while open.
    pub(crate) fn usage_foot(&mut self, ui: &mut egui::Ui, t: &Tones, area: Rect) {
        let shown = self.usage_readings();

        if shown.is_empty() {
            self.usage_ui.open = false;

            return;
        }

        let accent = glass_accent(self.dark());
        let mut clicked = false;

        for (index, (agent, reading)) in shown.iter().enumerate() {
            let rect = Rect::from_min_size(Pos2::new(area.left() + 5.0, area.top() + 4.0 + index as f32 * LINE), Vec2::new(area.width() - 10.0, LINE - 2.0));
            let response = ui.interact(rect, egui::Id::unique(("usage_line", agent.name())), Sense::click()).on_hover_cursor(CursorIcon::PointingHand);
            let painter = ui.painter();

            if response.hovered() || self.usage_ui.open {
                painter.rect_filled(rect, 7, t.hover);
            }

            // the 5-hour session; a plan without one (Codex's Go) shows the window it has
            let Some(meter) = reading.meters.iter().find(|meter| meter.title == "Sessão").or(reading.meters.first()) else { continue };
            let session = meter.used;
            let top = rect.top() + 11.0;

            painter.text(
                Pos2::new(rect.left() + 8.0, top),
                egui::Align2::LEFT_CENTER,
                format!("{} · {}", agent.name(), meter_name(meter).to_lowercase()),
                FontId::proportional(12.0),
                t.text,
            );

            painter.text(
                Pos2::new(rect.right() - 8.0, top),
                egui::Align2::RIGHT_CENTER,
                format!("{:.0}%", session * 100.0),
                FontId::proportional(12.0),
                t.muted,
            );

            let bar = Rect::from_min_max(Pos2::new(rect.left() + 8.0, rect.top() + 21.0), Pos2::new(rect.right() - 8.0, rect.top() + 25.0));

            paint_bar(painter, bar, session, t.active, accent);
            // the card open over it already says so: the tip would fall on the next line
            let response = if self.usage_ui.open { response } else { response.tip("Uso dos agentes") };

            clicked |= response.clicked();
        }

        if clicked {
            self.usage_ui.open = !self.usage_ui.open;
        }

        if self.usage_ui.open {
            self.usage_popover(ui.ctx(), area, &shown);
        }
    }

    /// The popover: each agent's windows and when they renew, the Maestro's ceiling, Atualizar and the way to Ajustes.
    fn usage_popover(&mut self, ctx: &Context, area: Rect, shown: &[(Agent, Reading)]) {
        let colors = menu_colors(self.dark());
        let wall = now_ms() / 1000;
        let busy = self.usage_ui.probe.busy();

        let failed: Vec<Option<i64>> =
            shown.iter().map(|(agent, _)| Some(self.usage_ui.probe.slot(*agent)).filter(|slot| slot.failed).map(|slot| slot.at)).collect();

        let maestro = self.maestro_line();
        let accent = glass_accent(self.dark());
        let width = area.width() - 20.0;

        let card = egui::Area::new(egui::Id::unique("usage_card"))
            .pivot(egui::Align2::LEFT_BOTTOM)
            .fixed_pos(Pos2::new(area.left() + 10.0, area.top() - 6.0))
            .order(egui::Order::Foreground)
            .show(ctx, |ui| {
                let frame = egui::Frame::new().fill(colors.bg).corner_radius(10).stroke(Stroke::new(1.0, colors.border)).inner_margin(10);

                frame
                    .show(ui, |ui| {
                        let inner = width - 20.0;

                        ui.set_width(inner);
                        ui.spacing_mut().item_spacing = Vec2::new(0.0, 4.0);
                        let mut action = None;
                        let (head, _) = ui.allocate_exact_size(Vec2::new(inner, 22.0), Sense::hover());

                        ui.painter().text(head.left_center(), egui::Align2::LEFT_CENTER, "Uso dos agentes", FontId::proportional(13.0), colors.text);
                        let spot = Rect::from_center_size(head.right_center() - Vec2::new(11.0, 0.0), Vec2::splat(22.0));
                        let refresh = ui.interact(spot, egui::Id::unique("usage_refresh"), if busy { Sense::hover() } else { Sense::click() }).tip("Atualizar");

                        if refresh.hovered() && !busy {
                            ui.painter().rect_filled(spot, 6, colors.hover);
                        }

                        paint_menu_icon_sized(ui.painter(), MenuIcon::Refresh, spot.center(), colors.text.gamma_multiply(if busy { 0.4 } else { 1.0 }), 12.0);

                        if refresh.clicked() {
                            action = Some(CardAction::Refresh);
                        }

                        for ((agent, reading), failed) in shown.iter().zip(&failed) {
                            ui.add_space(6.0);
                            agent_block(ui, &colors, accent, inner, (*agent, reading), *failed, wall);
                        }

                        ui.add_space(6.0);
                        ui.add(egui::Label::new(egui::RichText::new(&maestro).size(12.0).color(colors.muted)).wrap());

                        let link = ui
                            .add(egui::Label::new(egui::RichText::new("Configurações…").size(12.0).color(accent)).selectable(false).sense(Sense::click()))
                            .on_hover_cursor(CursorIcon::PointingHand);

                        if link.clicked() {
                            action = Some(CardAction::Settings);
                        }

                        action
                    })
                    .inner
            });

        match card.inner {
            Some(CardAction::Refresh) => self.usage_ui.probe.refresh(ctx.input(|input| input.time)),
            Some(CardAction::Settings) => {
                self.usage_ui.open = false;
                self.open_settings_at("Agentes");
            }
            None => {}
        }

        // a popover: Esc or a press anywhere else but its lines puts it away
        let lines = Rect::from_min_size(area.min, Vec2::new(area.width(), usage_height(shown.len())));

        let outside = ctx.input(|input| input.pointer.any_pressed())
            && ctx.input(|input| input.pointer.interact_pos()).is_some_and(|point| !card.response.rect.contains(point) && !lines.contains(point));

        if outside || ctx.input(|input| input.key_pressed(Key::Escape)) {
            self.usage_ui.open = false;
        }
    }

    /// "Maestro: 3 de 6 agentes; para de recrutar a 90%."
    pub(crate) fn maestro_line(&self) -> String {
        let running = self.running_agents();

        let ceiling = match self.prefs.max_agents {
            0 => format!("Maestro: {running} agente(s), sem teto"),
            max => format!("Maestro: {running} de {max} agentes"),
        };

        match self.prefs.usage_stop {
            0 => format!("{ceiling}; não para de recrutar pelo plano."),
            stop => format!("{ceiling}; para de recrutar a {stop}%."),
        }
    }

    /// The Maestro's `usage_refresh`: reads the plans again now.
    pub(crate) fn act_usage_refresh(&mut self) -> Result<String, String> {
        self.usage_ui.probe.next = 0.0;

        // the next frame asks the CLIs (`usage_poll`); a read in course is not doubled
        Ok(format!(
            "Lendo o uso dos planos de novo (a cada {} s também); consulte o `usage` em alguns segundos.",
            crate::app::usage::probe::REFRESH_SECS as i64
        ))
    }
}

/// One agent in the popover: its name and plan, a row per meter, and when they renew (or since when it failed to read).
fn agent_block(ui: &mut egui::Ui, colors: &MenuColors, accent: Color32, width: f32, (agent, reading): (Agent, &Reading), failed: Option<i64>, now: i64) {
    let (head, _) = ui.allocate_exact_size(Vec2::new(width, 18.0), Sense::hover());

    ui.painter().text(head.left_center(), egui::Align2::LEFT_CENTER, agent.name(), FontId::proportional(13.0), colors.text);

    if let Some(plan) = &reading.plan {
        ui.painter().text(head.right_center(), egui::Align2::RIGHT_CENTER, plan, FontId::proportional(12.0), colors.muted);
    }

    for meter in &reading.meters {
        let (row, _) = ui.allocate_exact_size(Vec2::new(width, 16.0), Sense::hover());
        let painter = ui.painter();

        painter.with_clip_rect(Rect::from_min_size(row.min, Vec2::new(68.0, row.height()))).text(
            row.left_center(),
            egui::Align2::LEFT_CENTER,
            meter_name(meter),
            FontId::proportional(12.0),
            colors.text,
        );

        painter.text(row.right_center(), egui::Align2::RIGHT_CENTER, format!("{:.0}%", meter.used * 100.0), FontId::proportional(12.0), colors.muted);
        let bar = Rect::from_min_max(Pos2::new(row.left() + 72.0, row.center().y - 2.0), Pos2::new(row.right() - 42.0, row.center().y + 2.0));

        paint_bar(painter, bar, meter.used, colors.hover.gamma_multiply(2.0), accent);
    }

    let note = match failed {
        Some(at) => Some(updated_text(at, now)),
        None => renew_text(reading, now),
    };

    if let Some(note) = note {
        ui.add(egui::Label::new(egui::RichText::new(note).size(12.0).color(colors.muted)).wrap());
    }
}
