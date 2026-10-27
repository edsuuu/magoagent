//! Connections: the number on the context bar opens the node's list of ropes (each with an ×), a
//! click or right click on a rope opens the ropes under it, and "Conectar" mode gives the canvas
//! the next click alone. The palette's "Conectar a…" / "Desconectar…" searches live here too.

use crate::prelude::*;

const DANGER: Color32 = Color32::from_rgb(0xff, 0x45, 0x3a);

/// What the connections popover lists: a node's ropes (its badge), or the ropes under a click.
#[derive(Clone, Debug, PartialEq)]
pub(crate) enum LinksOf {
    Node(String),
    Ropes(Vec<String>),
}

/// The connections popover open now, where it opened, its panel and what it hangs from (last
/// frame's rects: a press there doesn't close it).
#[derive(Clone, Debug)]
pub(crate) struct LinksPopover {
    pub(crate) of: LinksOf,
    pub(crate) at: Pos2,
    pub(crate) rect: Rect,
    pub(crate) anchor: Rect,
}

/// What the palette's node search does with the node picked.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) enum LinkAction {
    Connect,
    Disconnect,
}

/// A row the popover's presses asked for.
enum LinksAct {
    Focus(String),
    Remove(String),
    RemoveAll(String),
}

/// The two ends of a rope, by element id.
pub(crate) fn rope_ends(e: &Element) -> Option<(&str, &str)> {
    let (start, end) = (e.ends.start.as_ref()?, e.ends.end.as_ref()?);
    Some((start.element_id.as_str(), end.element_id.as_str()))
}

/// A node's icon and name in the lists: the name agents see (bridge.rs), or an IDE's own.
fn node_row(e: &Element, nodes: &[Node]) -> (Icon, String) {
    let tool = if e.terminal.is_some() {
        Tool::Terminal
    } else if e.portal.is_some() {
        Tool::Portal
    } else if e.ide.is_some() {
        Tool::Ide
    } else {
        Tool::Sticky
    };
    let name = nodes.iter().find(|n| n.id == e.id).map(|n| n.name.clone()).or_else(|| e.ide.as_ref().map(|i| i.name.clone()));
    (Icon::Tool(tool), name.unwrap_or_default())
}

/// A row of the popover: the node's icon and name, clickable, `width` wide.
fn chip(ui: &mut egui::Ui, icon: Icon, name: &str, width: f32, c: &UiColors) -> egui::Response {
    let (rect, response) = ui.allocate_exact_size(Vec2::new(width, 26.0), Sense::click());
    if response.hovered() {
        ui.painter().rect_filled(rect, 6, c.surface_high);
    }
    let p = ui.painter();
    paint_icon(p, icon, Pos2::new(rect.left() + 12.0, rect.center().y), c.text);
    p.text(Pos2::new(rect.left() + 26.0, rect.center().y), egui::Align2::LEFT_CENTER, name, FontId::proportional(12.5), c.text);
    response.on_hover_cursor(CursorIcon::PointingHand)
}

/// The × that removes a rope, red when pointed at.
fn remove_button(ui: &mut egui::Ui, tip: &str, c: &UiColors) -> egui::Response {
    let (rect, response) = ui.allocate_exact_size(Vec2::splat(22.0), Sense::click());
    let ink = if response.hovered() { DANGER } else { c.muted };
    let (center, arm) = (rect.center(), 4.0);
    let stroke = Stroke::new(1.5, ink);
    ui.painter().line_segment([center - Vec2::splat(arm), center + Vec2::splat(arm)], stroke);
    ui.painter().line_segment([center + Vec2::new(arm, -arm), center + Vec2::new(-arm, arm)], stroke);
    response.on_hover_text(tip).on_hover_cursor(CursorIcon::PointingHand)
}

impl DrawingApp {
    /// "Conectar": the source gives up the keyboard and an enlarged window goes back to the canvas,
    /// so the next press is the canvas's alone (pointer.rs).
    pub(crate) fn start_connect(&mut self, id: String) {
        self.finish_text();
        self.terminal_focus = None;
        self.set_elevated(None);
        self.connecting = Some(id);
    }

    /// Takes out the ropes `gone` picks, as one undo step.
    fn drop_ropes(&mut self, gone: impl Fn(&Element) -> bool) {
        let is_gone = |e: &Element| e.rope && gone(e);
        if self.scene.elements.iter().any(|e| is_gone(e)) {
            self.snapshot();
            self.keeping_selection(|app| app.scene.elements.retain(|e| !is_gone(e)));
        }
    }

    /// Cuts the rope between `a` and `b`.
    pub(crate) fn unlink(&mut self, a: &str, b: &str) {
        self.drop_ropes(|e| rope_ends(e).is_some_and(|(x, y)| (x == a && y == b) || (x == b && y == a)));
    }

    /// Cuts every rope that touches one of `ids`.
    pub(crate) fn unlink_all(&mut self, ids: &[String]) {
        self.drop_ropes(|e| rope_ends(e).is_some_and(|(x, y)| ids.iter().any(|id| id == x || id == y)));
    }

    /// A click on a row: a terminal takes the keyboard, anything else is selected; either way it
    /// moves to the middle of the view.
    pub(crate) fn focus_node(&mut self, id: &str) {
        let Some(i) = self.scene.elements.iter().position(|e| e.id == id) else { return };
        if self.scene.elements[i].terminal.is_some() {
            self.focus_terminal(i);
        } else {
            self.selected = vec![i];
        }
        self.pan_to(self.scene.elements[i].center());
    }

    /// The badge's click: its popover opens below it, or closes if it is open.
    pub(crate) fn toggle_node_links(&mut self, id: &str, anchor: Rect) {
        if self.links_popover.as_ref().is_some_and(|p| p.of == LinksOf::Node(id.to_string())) {
            self.links_popover = None;
            return;
        }
        let at = Pos2::new(anchor.center().x - 120.0, anchor.bottom() + 8.0);
        self.links_popover = Some(LinksPopover { of: LinksOf::Node(id.to_string()), at, rect: Rect::NOTHING, anchor });
    }

    /// A click or right click on ropes: the popover of every rope under `world`, at `at`.
    pub(crate) fn open_rope_popover(&mut self, at: Pos2, world: Pos2) {
        let tolerance = 10.0 / self.zoom;
        let ids: Vec<String> = self.scene.elements.iter().filter(|e| e.rope && !e.locked && self.on_rope(e, world, tolerance)).map(|e| e.id.clone()).collect();
        if !ids.is_empty() {
            self.links_popover = Some(LinksPopover { of: LinksOf::Ropes(ids), at, rect: Rect::NOTHING, anchor: Rect::NOTHING });
        }
    }

    /// The nodes the palette's search offers `source`: those it is not joined to (Connect), or
    /// those it is joined to (Disconnect). Each as (id, icon, name).
    pub(crate) fn link_candidates(&self, source: &str, action: LinkAction) -> Vec<(String, Icon, String)> {
        let mut links = self.links();
        let joined = links.remove(source).unwrap_or_default();
        let nodes = self.agent_nodes();
        self.scene
            .elements
            .iter()
            .filter(|e| !e.rope && e.id != source && (e.is_node() || e.created.is_some()))
            .filter(|e| joined.contains(&e.id) == (action == LinkAction::Disconnect))
            .map(|e| {
                let (icon, name) = node_row(e, &nodes);
                (e.id.clone(), icon, name)
            })
            .collect()
    }

    /// The palette's pick: joins or cuts the node picked to the active node.
    pub(crate) fn pick_link_node(&mut self, node: &str) {
        let action = self.palette_links.take();
        let source = self.active_node().map(|e| e.id.clone());
        if let (Some(action), Some(source)) = (action, source) {
            match action {
                LinkAction::Connect => self.connect(&source, node),
                LinkAction::Disconnect => self.unlink(&source, node),
            }
        }
    }

    /// Draws the connections popover and runs the press it got. It closes on Esc, on a press
    /// outside it (and its anchor), or when no rope is left to list.
    pub(crate) fn links_popover_ui(&mut self, ctx: &Context) {
        let Some(mut popover) = self.links_popover.take() else { return };
        let pressed_outside = ctx.input(|i| {
            i.pointer.any_pressed() && i.pointer.interact_pos().is_some_and(|p| !popover.rect.contains(p) && !popover.anchor.contains(p))
        });
        if ctx.input(|i| i.key_pressed(Key::Escape)) || pressed_outside {
            return;
        }
        let nodes = self.agent_nodes();
        // (rope id, the ends to list): a node lists its other end, a click lists both
        let mut rows: Vec<(String, Vec<String>)> = Vec::new();
        for e in self.scene.elements.iter().filter(|e| e.rope) {
            let Some((a, b)) = rope_ends(e) else { continue };
            match &popover.of {
                LinksOf::Node(id) if a == id || b == id => rows.push((e.id.clone(), vec![if a == id { b } else { a }.to_string()])),
                LinksOf::Ropes(ids) if ids.contains(&e.id) => rows.push((e.id.clone(), vec![a.to_string(), b.to_string()])),
                _ => {}
            }
        }
        if rows.is_empty() {
            return;
        }
        let node_of = matches!(popover.of, LinksOf::Node(_));
        let source = match &popover.of {
            LinksOf::Node(id) => Some(id.clone()),
            LinksOf::Ropes(_) => None,
        };
        let c = self.colors();
        let mut act = None;
        let area = egui::Area::new(egui::Id::unique("links_popover")).fixed_pos(popover.at).order(egui::Order::Foreground).show(ctx, |ui| {
            self.island().corner_radius(12).inner_margin(egui::Margin::symmetric(8, 8)).show(ui, |ui| {
                ui.set_width(240.0);
                ui.spacing_mut().item_spacing = Vec2::new(4.0, 2.0);
                if let (true, Some(id)) = (node_of, &source) {
                    ui.horizontal(|ui| {
                        ui.label(egui::RichText::new("Conexões").size(13.0).strong().color(c.text));
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            if ui.add(egui::Button::new(egui::RichText::new("Remover tudo").size(11.5).color(c.muted)).frame(false)).on_hover_cursor(CursorIcon::PointingHand).clicked() {
                                act = Some(LinksAct::RemoveAll(id.clone()));
                            }
                        });
                    });
                }
                for (rope, ends) in &rows {
                    ui.horizontal(|ui| {
                        let room = ui.available_width() - 30.0;
                        let mut names = Vec::new();
                        for (k, id) in ends.iter().enumerate() {
                            let (icon, name) = self.scene.elements.iter().find(|e| &e.id == id).map_or((Icon::Tool(Tool::Sticky), String::new()), |e| node_row(e, &nodes));
                            if k > 0 {
                                let (line, _) = ui.allocate_exact_size(Vec2::new(12.0, 26.0), Sense::hover());
                                ui.painter().hline(line.x_range(), line.center().y, Stroke::new(1.5, c.muted));
                            }
                            // a node's row fills the popover; a rope's ends hug their names
                            let width = if node_of { room } else { ui.painter().layout_no_wrap(name.clone(), FontId::proportional(12.5), c.text).size().x + 38.0 };
                            if chip(ui, icon, &name, width, &c).clicked() {
                                act = Some(LinksAct::Focus(id.clone()));
                            }
                            names.push(name);
                        }
                        let tip = if node_of { format!("Remover conexão com {}", names.last().map_or("", String::as_str)) } else { "Remover conexão".to_string() };
                        if remove_button(ui, &tip, &c).clicked() {
                            act = Some(LinksAct::Remove(rope.clone()));
                        }
                    });
                }
            });
        });
        popover.rect = area.response.rect;
        match act {
            Some(LinksAct::Focus(id)) => self.focus_node(&id),
            Some(LinksAct::Remove(rope)) => {
                self.drop_ropes(|e| e.id == rope);
                self.links_popover = Some(popover);
            }
            Some(LinksAct::RemoveAll(id)) => {
                self.unlink_all(&[id]);
                self.links_popover = Some(popover);
            }
            None => self.links_popover = Some(popover),
        }
    }
}
