//! Linha do tempo: the agents' events (maestro/timeline.rs), newest first, with a chip per agent to
//! show only its own; a click takes the canvas to that agent's card.

use super::{chip, live};
use crate::prelude::*;

/// How many events the panel reads.
const SHOWN: usize = 200;

/// The events as last read, and what they were read at: the database is read again only when the
/// app's own list moved.
#[derive(Default)]
pub(crate) struct TimelineCache {
    events: Vec<AgentEvent>,
    read: Option<(usize, i64)>,
}

/// The events of `agent` (what it did, sent or got), newest first.
pub(crate) fn newest_first<'a>(events: &'a [AgentEvent], agent: Option<&str>) -> Vec<&'a AgentEvent> {
    events.iter().rev().filter(|e| agent.is_none_or(|id| e.id == id || e.to == id)).collect()
}

impl DrawingApp {
    pub(crate) fn timeline_pane(&mut self, ui: &mut egui::Ui) {
        let colors = self.colors();
        let key = (self.agent_events.len(), self.agent_events.back().map_or(0, |e| e.at));
        if self.panes.timeline.read != Some(key) {
            self.panes.timeline = TimelineCache { events: self.timeline_events(None, None, None, SHOWN), read: Some(key) };
        }
        let events = std::mem::take(&mut self.panes.timeline.events);
        // a chip per agent in the list, the one that did something last first
        let mut agents: Vec<(&str, &str)> = Vec::new();
        for event in events.iter().rev().filter(|e| !e.id.is_empty()) {
            if !agents.iter().any(|(id, _)| *id == event.id) {
                agents.push((&event.id, &event.name));
            }
        }
        ui.horizontal_wrapped(|ui| {
            if chip(ui, self.panes.timeline_agent.is_none(), "Todos") {
                self.panes.timeline_agent = None;
            }
            for (id, name) in &agents {
                let on = self.panes.timeline_agent.as_deref() == Some(*id);
                if chip(ui, on, *name) {
                    self.panes.timeline_agent = Some(id.to_string());
                }
            }
        });
        let shown = newest_first(&events, self.panes.timeline_agent.as_deref());
        if shown.is_empty() {
            ui.label(egui::RichText::new("Nada aconteceu ainda.").color(colors.muted));
        }
        let mut go = None;
        for (index, event) in shown.into_iter().enumerate() {
            let row = ui.horizontal_top(|ui| {
                let clock = crate::app::chat::viewer::local_clock(event.at).unwrap_or_else(|| "--:--".into());
                ui.add_sized(Vec2::new(40.0, 20.0), egui::Label::new(egui::RichText::new(clock).monospace().size(12.0).color(colors.muted)));
                let (dot, _) = ui.allocate_exact_size(Vec2::new(10.0, 20.0), Sense::hover());
                ui.painter().circle_filled(dot.center(), 4.5, crate::app::island::agent_color("", &event.id));
                ui.vertical(|ui| {
                    let name = match event.to.as_str() {
                        "" => event.name.clone(),
                        to => format!("{} → {}", event.name, self.agent_label(to)),
                    };
                    ui.label(egui::RichText::new(name).strong());
                    ui.add(egui::Label::new(egui::RichText::new(&event.what).size(12.0).color(colors.muted)).wrap());
                });
            });
            let rect = row.response.rect.expand2(Vec2::new(4.0, 3.0));
            let click = ui.interact(rect, ui.scope_id().with(("timeline_row", index)), Sense::click()).on_hover_cursor(CursorIcon::PointingHand);
            if click.hovered() {
                ui.painter().rect_filled(rect, 7, glass_ink(self.dark()).gamma_multiply(0.06));
            }
            if click.clicked() {
                go = Some(event.id.clone());
            }
        }
        self.panes.timeline.events = events;
        if let Some(id) = go.filter(|_| live(ui)) {
            self.timeline_go(&id);
        }
    }

    /// A row clicked: its agent's card on the canvas, if it is still there.
    pub(crate) fn timeline_go(&mut self, id: &str) {
        if self.agent_rows().iter().any(|r| r.id == id) {
            self.open_agent(id);
        } else {
            self.toast("Esse agente não está mais no canvas.");
        }
    }
}
