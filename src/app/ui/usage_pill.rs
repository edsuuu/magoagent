//! The agents' use of their plans, in the canvas's bottom right row: a pill left of the
//! minimap's button, and what a click on it opens.

use crate::app::usage_probe::{Agent, Probe, REFRESH_SECS, Reading, Slot, reset_text, updated_text};
use crate::prelude::*;

/// What the pill and its card keep between frames.
#[derive(Default)]
pub(crate) struct UsagePill {
    pub(crate) open: bool,
    pub(crate) probe: Probe,
}

/// A group of rings on the pill (the reference app's): 20 across, 2.4 thick, 0.9 apart, 6 between groups.
const PILL_RING: f32 = 20.0;
const PILL_LINE: f32 = 2.4;
const PILL_SPACING: f32 = 0.9;
const PILL_GROUP_GAP: f32 = 6.0;
/// The card: 340 wide, 16 of padding, rings of 72 and 8 thick.
const CARD_WIDTH: f32 = 340.0;
const CARD_PAD: f32 = 16.0;

/// The diameters of a group's rings, outside in: each a step of 2 * (line + spacing) inside the one before,
/// while it still is wider than 1.5 lines.
pub(crate) fn ring_diameters(size: f32, line: f32, spacing: f32, count: usize) -> Vec<f32> {
    (0..count).map(|i| size - i as f32 * 2.0 * (line + spacing)).take_while(|d| *d > line * 1.5).collect()
}

/// The ring at `position` (0 outside): the agent's color, then the same hue less saturated and brighter.
pub(crate) fn ring_color(base: Color32, position: usize) -> Color32 {
    if position == 0 {
        return base;
    }
    let mut hsva = egui::ecolor::Hsva::from_srgba_unmultiplied([base.r(), base.g(), base.b(), 255]);
    let step = position.min(2) as f32;
    hsva.s = (hsva.s - 0.12 * step).max(0.35);
    hsva.v = (hsva.v + 0.1 * step).min(1.0);
    let [r, g, b] = hsva.to_srgb();
    Color32::from_rgb(r, g, b)
}

/// A group of rings centered at `center`: the track at 20% and, over it, the used part from the top,
/// clockwise, with round ends.
// ponytail: flat color; the reference fades the arc's start and shades its head, add a gradient
// mesh if the difference is missed
fn paint_rings(p: &Painter, center: Pos2, size: f32, line: f32, spacing: f32, base: Color32, used: &[f64]) {
    for (position, d) in ring_diameters(size, line, spacing, used.len()).into_iter().enumerate() {
        let color = ring_color(base, position);
        let radius = (d - line) / 2.0;
        p.circle_stroke(center, radius, Stroke::new(line, color.gamma_multiply(0.2)));
        let fraction = used[position].clamp(0.0, 1.0) as f32;
        if fraction >= 1.0 {
            p.circle_stroke(center, radius, Stroke::new(line, color));
        } else if fraction > 0.0 {
            let at = |t: f32| center + Vec2::angled(-std::f32::consts::FRAC_PI_2 + t * std::f32::consts::TAU) * radius;
            let steps = ((fraction * 90.0).ceil() as usize).max(2);
            let points: Vec<Pos2> = (0..=steps).map(|i| at(fraction * i as f32 / steps as f32)).collect();
            p.add(Shape::line(points.clone(), Stroke::new(line, color)));
            for end in [points[0], points[steps]] {
                p.circle_filled(end, line / 2.0, color);
            }
        }
    }
}

/// What the card's buttons asked for.
#[derive(PartialEq)]
enum CardAction {
    Refresh,
    Settings,
}

/// The agents the pill shows, each with what the card needs.
struct Shown {
    agent: Agent,
    reading: Reading,
    slot: Slot,
}

/// A refresh arrow: three quarters of a circle and an arrowhead.
fn paint_refresh(p: &Painter, center: Pos2, color: Color32) {
    let at = |deg: f32| center + Vec2::angled(deg.to_radians()) * 5.0;
    p.add(Shape::line((0..=20).map(|i| at(-40.0 + 280.0 * i as f32 / 20.0)).collect(), Stroke::new(1.4, color)));
    let tip = at(-40.0);
    p.add(Shape::convex_polygon(vec![tip + Vec2::new(-3.0, -3.5), tip + Vec2::new(3.5, -2.5), tip + Vec2::new(0.5, 3.0)], color, Stroke::NONE));
}

/// The card: a title with the refresh button, each agent's rings, plan and windows, and the way to Ajustes.
fn usage_card(ui: &mut egui::Ui, c: &UiColors, accent: Color32, shown: &[Shown], busy: bool, now: i64) -> Option<CardAction> {
    let mut action = None;
    let line = Stroke::new(1.0, c.muted.gamma_multiply(0.25));
    ui.spacing_mut().item_spacing = Vec2::ZERO;
    ui.set_width(CARD_WIDTH);
    let (head, _) = ui.allocate_exact_size(Vec2::new(CARD_WIDTH, 48.0), Sense::hover());
    let title = egui::UiBuilder::new().max_rect(Rect::from_min_size(head.left_top() + Vec2::new(CARD_PAD, 0.0), Vec2::new(240.0, 48.0)));
    ui.scope_builder(title.layout(egui::Layout::left_to_right(egui::Align::Center)), |ui| {
        ui.label(egui::RichText::new("Uso dos Agentes").size(13.0).strong().color(c.text))
    });
    let button = Rect::from_center_size(Pos2::new(head.right() - 10.0 - 14.0, head.center().y), Vec2::splat(28.0));
    let refresh = ui.interact(button, egui::Id::unique("usage_refresh"), if busy { Sense::hover() } else { Sense::click() }).tip("Atualizar");
    if refresh.hovered() && !busy {
        ui.painter().rect_filled(button, 6, c.text.gamma_multiply(0.07));
    }
    paint_refresh(ui.painter(), button.center(), c.text.gamma_multiply(if busy { 0.4 } else { 1.0 }));
    if refresh.clicked() {
        action = Some(CardAction::Refresh);
    }
    ui.painter().hline(ui.min_rect().x_range(), ui.cursor().top(), line);
    // as tall as the reference's (480), then it scrolls
    egui::ScrollArea::vertical().max_height(480.0).auto_shrink([false, true]).show(ui, |ui| {
        egui::Frame::new().inner_margin(CARD_PAD as i8).show(ui, |ui| {
            ui.spacing_mut().item_spacing = Vec2::ZERO;
            for (i, s) in shown.iter().enumerate() {
                if i > 0 {
                    ui.add_space(20.0);
                }
                ui.horizontal_top(|ui| {
                    let (ring, _) = ui.allocate_exact_size(Vec2::splat(72.0), Sense::hover());
                    let used: Vec<f64> = s.reading.meters.iter().map(|m| m.used).collect();
                    paint_rings(ui.painter(), ring.center(), 72.0, 8.0, 2.0, s.agent.color(), &used);
                    ui.add_space(14.0);
                    ui.vertical(|ui| {
                        ui.label(egui::RichText::new(s.agent.name()).size(13.0).strong().color(c.text));
                        ui.add_space(3.0);
                        if let Some(plan) = &s.reading.plan {
                            ui.label(egui::RichText::new(plan).size(11.0).color(c.muted));
                        }
                        if s.slot.failed {
                            ui.label(egui::RichText::new(updated_text(s.slot.at, now)).size(11.0).color(c.muted));
                        }
                    });
                });
                for (position, m) in s.reading.meters.iter().enumerate() {
                    ui.add_space(14.0);
                    ui.horizontal(|ui| {
                        ui.label(egui::RichText::new(&m.title).size(12.0).color(c.text));
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            ui.label(egui::RichText::new(format!("{:.0}%", m.used * 100.0)).size(12.0).color(c.text));
                        });
                    });
                    ui.add_space(5.0);
                    segment_bar(ui, m.used, ring_color(s.agent.color(), position));
                    if let Some(at) = m.resets_at {
                        ui.add_space(5.0);
                        ui.label(egui::RichText::new(reset_text(at, now)).size(10.0).color(c.muted));
                    }
                }
            }
        });
    });
    ui.painter().hline(ui.min_rect().x_range(), ui.cursor().top(), line);
    ui.allocate_ui_with_layout(Vec2::new(CARD_WIDTH, 34.0), egui::Layout::right_to_left(egui::Align::Center), |ui| {
        ui.add_space(CARD_PAD);
        let link = ui
            .add(egui::Label::new(egui::RichText::new("Configurações…").size(11.0).color(accent)).sense(Sense::click()))
            .on_hover_cursor(CursorIcon::PointingHand);
        if link.hovered() {
            ui.painter().hline(link.rect.x_range(), link.rect.bottom(), Stroke::new(1.0, accent));
        }
        if link.clicked() {
            action = Some(CardAction::Settings);
        }
    });
    action
}

/// 36 segments across, the used share lit (at least one while anything is used).
// ponytail: flat colors; the reference shades each segment top to bottom
fn segment_bar(ui: &mut egui::Ui, used: f64, color: Color32) {
    const SEGMENTS: usize = 36;
    let (rect, _) = ui.allocate_exact_size(Vec2::new(ui.available_width(), 26.0), Sense::hover());
    let gap = (rect.width() / 100.0).floor().max(1.0);
    let width = (rect.width() - gap * (SEGMENTS - 1) as f32) / SEGMENTS as f32;
    let lit = if used > 0.0 { ((used.min(1.0) * SEGMENTS as f64).round() as usize).max(1) } else { 0 };
    for i in 0..SEGMENTS {
        let cell = Rect::from_min_size(rect.min + Vec2::new(i as f32 * (width + gap), 0.0), Vec2::new(width, rect.height()));
        ui.painter().rect_filled(cell, 99, if i < lit { color } else { color.gamma_multiply(0.2) });
    }
}

impl DrawingApp {
    /// The pill, its right end `right` from the window's right edge (negative, as an anchor's
    /// offset). Gives how wide it drew: none while there is nothing to show.
    pub(crate) fn usage_pill(&mut self, ctx: &Context, right: f32) -> f32 {
        let now = ctx.input(|i| i.time);
        let probe = &mut self.usage_ui.probe;
        // the CLIs are asked where a drawing is saved (the app proper), every 5 minutes
        if self.autosave.is_some() && now >= probe.next {
            probe.refresh(now);
        }
        if probe.collect() {
            ctx.request_repaint();
        }
        ctx.request_repaint_after(std::time::Duration::from_secs_f64(if probe.busy() { 0.5 } else { (probe.next - now).clamp(0.5, 30.0) }));
        let shown: Vec<Shown> = Agent::ALL
            .into_iter()
            .filter_map(|agent| Some(Shown { agent, reading: probe.slot(agent).reading.clone()?, slot: probe.slot(agent).clone() }))
            .collect();
        if shown.is_empty() {
            self.usage_ui.open = false;
            return 0.0;
        }
        let (dark, c) = (self.dark(), self.colors());
        let n = shown.len() as f32;
        let width = n * PILL_RING + (n - 1.0) * PILL_GROUP_GAP + 17.0;
        let pill = egui::Area::new(egui::Id::unique("usage_pill")).anchor(egui::Align2::RIGHT_BOTTOM, (right, -CORNER_GAP)).show(ctx, |ui| {
            let (rect, response) = ui.allocate_exact_size(Vec2::new(width, GLASS_HEIGHT), Sense::click());
            paint_glass(ui.painter(), rect, GLASS_HEIGHT / 2.0, dark, false);
            let mut clicked = response.clone().on_hover_cursor(CursorIcon::PointingHand).tip("Uso dos Agentes").clicked();
            for (i, s) in shown.iter().enumerate() {
                let center = Pos2::new(rect.left() + 8.5 + PILL_RING / 2.0 + i as f32 * (PILL_RING + PILL_GROUP_GAP), rect.center().y);
                let used: Vec<f64> = s.reading.meters.iter().map(|m| m.used).collect();
                paint_rings(ui.painter(), center, PILL_RING, PILL_LINE, PILL_SPACING, s.agent.color(), &used);
                if s.slot.failed {
                    ui.painter().circle(center + Vec2::new(8.0, 8.0), 4.0, c.warning, Stroke::new(1.0, Color32::WHITE));
                }
                let group = ui.interact(Rect::from_center_size(center, Vec2::new(21.0, 25.0)), response.id.with(i), Sense::click());
                clicked |= group.on_hover_cursor(CursorIcon::PointingHand).tip(s.agent.name()).clicked();
            }
            (rect, clicked)
        });
        let (pill_rect, clicked) = pill.inner;
        if clicked {
            self.usage_ui.open = !self.usage_ui.open;
        }
        if self.usage_ui.open {
            let (busy, wall) = (self.usage_ui.probe.busy(), now_ms() / 1000);
            let accent = glass_accent(dark);
            let card_anchor = (right, -(CORNER_GAP + GLASS_HEIGHT + 8.0));
            let frame = self.island().inner_margin(0);
            let card = egui::Area::new(egui::Id::unique("usage_card"))
                .anchor(egui::Align2::RIGHT_BOTTOM, card_anchor)
                .show(ctx, |ui| frame.show(ui, |ui| usage_card(ui, &c, accent, &shown, busy, wall)).inner);
            let outside = ctx.input(|i| i.pointer.any_pressed())
                && ctx.input(|i| i.pointer.interact_pos()).is_some_and(|p| !card.response.rect.contains(p) && !pill_rect.contains(p));
            match card.inner {
                Some(CardAction::Refresh) => self.usage_ui.probe.refresh(now),
                Some(CardAction::Settings) => {
                    self.usage_ui.open = false;
                    self.settings_open = true;
                    self.settings_tab = crate::app::ui::settings::SETTINGS_TABS.iter().position(|t| *t == "Desempenho").unwrap_or(0);
                }
                None => {}
            }
            if outside || ctx.input(|i| i.key_pressed(Key::Escape)) {
                self.usage_ui.open = false;
            }
        }
        width
    }

    /// The Maestro's `usage_refresh`: reads the plans again now.
    pub(crate) fn act_usage_refresh(&mut self) -> Result<String, String> {
        self.usage_ui.probe.next = 0.0;
        // the next frame asks the CLIs (`usage_pill`); a read in course is not doubled
        Ok(format!("Lendo o uso dos planos de novo (a cada {} s também); consulte o `usage` em alguns segundos.", REFRESH_SECS as i64))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::usage_probe::Meter;

    #[test]
    fn rings_step_inward_and_keep_the_agents_hue() {
        let near = |a: Vec<f32>, b: &[f32]| a.len() == b.len() && a.iter().zip(b).all(|(x, y)| (x - y).abs() < 1e-4);
        assert!(near(ring_diameters(20.0, 2.4, 0.9, 3), &[20.0, 13.4, 6.8]));
        assert!(near(ring_diameters(72.0, 8.0, 2.0, 3), &[72.0, 52.0, 32.0]));
        assert!(near(ring_diameters(20.0, 2.4, 0.9, 2), &[20.0, 13.4]), "only the rings there are");
        assert!(ring_diameters(20.0, 2.4, 0.9, 5).len() <= 3, "the fourth would be thinner than its stroke");
        assert_eq!(ring_color(Color32::from_rgb(0xD9, 0x77, 0x57), 0), Color32::from_rgb(0xD9, 0x77, 0x57));
        let inner = ring_color(Color32::from_rgb(0xD9, 0x77, 0x57), 1);
        assert!(inner.r() >= inner.g() && inner.g() >= inner.b() && inner != Color32::from_rgb(0xD9, 0x77, 0x57), "{inner:?}");
    }

    fn run(app: &mut DrawingApp, ctx: &Context, events: Vec<egui::Event>) -> (f32, Vec<String>) {
        let mut width = 0.0;
        let mut output =
            ctx.run_ui(egui::RawInput { screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(800.0, 900.0))), events, ..Default::default() }, |ui| {
                width = app.usage_pill(ui.ctx(), -300.0);
            });
        output.textures_delta.clear();
        fn text_of(shape: &Shape, out: &mut Vec<String>) {
            match shape {
                Shape::Text(t) => out.push(t.galley.text().to_string()),
                Shape::Vec(v) => v.iter().for_each(|s| text_of(s, out)),
                _ => {}
            }
        }
        let mut texts = vec![];
        output.shapes.iter().for_each(|s| text_of(&s.shape, &mut texts));
        (width, texts)
    }

    #[test]
    fn the_pill_shows_who_has_a_reading_and_a_click_opens_the_card() {
        let ctx = Context::default();
        let mut app = DrawingApp::default();
        assert_eq!(run(&mut app, &ctx, vec![]).0, 0.0, "nothing read, no pill");
        let meter = |title: &str, used: f64| Meter { title: title.into(), used, resets_at: Some(now_ms() / 1000 + 3 * 86400) };
        let reading = Reading { plan: Some("Max".into()), meters: vec![meter("Sessão", 0.98), meter("Semanal", 0.43), meter("Fable", 0.79)] };
        app.usage_ui.probe.tx.send((Agent::Claude, Ok(reading.clone()))).unwrap();
        let (one, _) = run(&mut app, &ctx, vec![]);
        app.usage_ui.probe.tx.send((Agent::Codex, Ok(reading))).unwrap();
        let (two, _) = run(&mut app, &ctx, vec![]);
        assert_eq!((one, two), (37.0, 63.0), "63 across for both, as in the reference app");
        // a click on the pill: right end at 800 - 300
        let at = Pos2::new(500.0 - 20.0, 900.0 - CORNER_GAP - GLASS_HEIGHT / 2.0);
        let press = |pressed| egui::Event::PointerButton { pos: at, button: egui::PointerButton::Primary, pressed, modifiers: Default::default() };
        run(&mut app, &ctx, vec![egui::Event::PointerMoved(at), press(true)]);
        run(&mut app, &ctx, vec![press(false)]);
        assert!(app.usage_ui.open);
        // an area shows from its second frame
        run(&mut app, &ctx, vec![]);
        let (_, texts) = run(&mut app, &ctx, vec![]);
        for want in ["Uso dos Agentes", "Claude Code", "Codex", "Max", "Sessão", "Semanal", "Fable", "98%", "Redefine em 3 dias", "Configurações…"] {
            assert!(texts.iter().any(|t| t == want), "{want}: {texts:?}");
        }
        // Esc puts it away
        run(&mut app, &ctx, vec![egui::Event::Key { key: Key::Escape, physical_key: None, pressed: true, repeat: false, modifiers: Default::default() }]);
        assert!(!app.usage_ui.open);
        // the Maestro reads the plans, and asks for them again
        app.usage_ui.probe.next = 1e9;
        app.act("usage_refresh", &json!({})).unwrap();
        assert_eq!(app.usage_ui.probe.next, 0.0);
        let said = app.act("usage", &json!({})).unwrap();
        assert!(said.contains("Plano lido pela CLI, Claude Code (Max): Sessão 98% (redefine em 3 dias)"), "{said}");
    }
}
