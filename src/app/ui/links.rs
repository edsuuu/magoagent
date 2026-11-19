//! Connections: the number on the context bar opens the node's list of ropes (each with an ×), a
//! click or right click on a rope opens the ropes under it (between two agents, with their last
//! messages), and "Conectar" mode gives the canvas
//! the next click alone. The palette's "Conectar a…" / "Desconectar…" searches live here too.

use crate::app::ui::agents::ago;
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
    /// Each listed rope between two agents and its last messages, read again when the app's
    /// newest event (`seen`, its time) changes.
    pub(crate) history: Vec<(String, Vec<AgentEvent>)>,
    pub(crate) seen: Option<i64>,
}

/// How many of a connection's messages its popover lists.
const HISTORY: usize = 6;

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

/// A node's icon and name in the lists: the name agents see (mcp/bridge/mod.rs).
fn node_row(e: &Element, nodes: &[Node]) -> (Icon, String) {
    let tool = if e.terminal.is_some() {
        Tool::Terminal
    } else if e.portal.is_some() {
        Tool::Portal
    } else if e.filetree.is_some() {
        Tool::FileTree
    } else if e.chat.is_some() {
        return (Icon::Agents, nodes.iter().find(|n| n.id == e.id).map(|n| n.name.clone()).unwrap_or_default());
    } else {
        Tool::Sticky
    };

    let name = nodes.iter().find(|n| n.id == e.id).map(|n| n.name.clone());

    (Icon::Tool(tool), name.unwrap_or_default())
}

/// A menu's hairline between its parts.
fn separator(ui: &mut egui::Ui, color: Color32) {
    let (line, _) = ui.allocate_exact_size(Vec2::new(ui.available_width(), crate::app::ui::menu_look::SEP_HEIGHT), Sense::hover());

    ui.painter().hline(line.x_range(), line.center().y, Stroke::new(1.0, color));
}

/// A part's title in the menu, small and muted.
fn section_title(ui: &mut egui::Ui, title: &str, color: Color32) {
    let (rect, _) = ui.allocate_exact_size(Vec2::new(ui.available_width(), 24.0), Sense::hover());

    ui.painter().text(
        Pos2::new(rect.left() + crate::app::ui::menu_look::ROW_PAD, rect.center().y),
        egui::Align2::LEFT_CENTER,
        title,
        FontId::proportional(11.0),
        color,
    );
}

/// A node's connection in the popover, as a row of the app's menus: the other end's icon and name (a click goes
/// to it) and, under the pointer, the × that cuts the rope. What was clicked: (go to it, cut it).
fn link_row(ui: &mut egui::Ui, icon: Icon, name: &str, tip: &str) -> (bool, bool) {
    use crate::app::ui::menu_look::*;
    let colors = menu_colors(ui.visuals().dark_mode);
    let (rect, row) = ui.allocate_exact_size(Vec2::new(ui.available_width(), ROW_HEIGHT + 4.0), Sense::click());
    let x = Rect::from_center_size(Pos2::new(rect.right() - ROW_PAD - 7.0, rect.center().y), Vec2::splat(20.0));
    let cut = ui.interact(x, ui.scope_id().with(("link_cut", name)), Sense::click());
    let hot = row.hovered() || cut.hovered();
    let p = ui.painter();

    if hot {
        p.rect_filled(rect, ITEM_RADIUS, colors.hover);
    }

    paint_icon_scaled(
        p,
        icon,
        Pos2::new(rect.left() + ROW_PAD + ICON_SIZE / 2.0, rect.center().y),
        if hot { colors.text } else { colors.muted },
        ICON_SIZE / 16.0,
        0.0,
    );

    let left = rect.left() + ROW_PAD + ICON_SIZE + ROW_GAP;
    let galley = crate::app::chat::list::clipped(p, name.to_string(), FontId::proportional(MENU_TEXT_SIZE + 1.0), colors.text, x.left() - 4.0 - left);

    p.galley(Pos2::new(left, rect.center().y - galley.size().y / 2.0), galley, colors.text);

    if hot {
        let ink = if cut.hovered() { DANGER } else { colors.muted };

        cross(p, x.center(), 3.5, Stroke::new(1.25, ink));
    }

    let cut = cut.on_hover_cursor(CursorIcon::PointingHand).tip(tip).clicked();

    (!cut && row.on_hover_cursor(CursorIcon::PointingHand).clicked(), cut)
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

    /// Takes out the ropes `gone` picks, as one undo step; a locked one stays, and says so.
    fn drop_ropes(&mut self, gone: impl Fn(&Element) -> bool) {
        if self.scene.elements.iter().any(|e| e.rope && e.locked && gone(e)) {
            self.toast("Conexão bloqueada: desbloqueie no menu principal, em Elementos bloqueados, para desconectar");
        }

        let is_gone = |e: &Element| e.rope && !e.locked && gone(e);

        if self.scene.elements.iter().any(&is_gone) {
            self.snapshot();
            self.keeping_selection(|app| app.scene.elements.retain(|e| !is_gone(e)));
        }
    }

    /// Cuts the rope between `a` and `b`.
    pub(crate) fn unlink(&mut self, a: &str, b: &str) {
        self.drop_ropes(|e| rope_ends(e).is_some_and(|(x, y)| (x == a && y == b) || (x == b && y == a)));
    }

    /// "Vincular": roped to `other` it is cut, otherwise it is roped.
    pub(crate) fn toggle_link(&mut self, id: &str, other: &str) {
        if self.links().get(id).is_some_and(|l| l.iter().any(|x| x == other)) {
            self.unlink(id, other);
        } else {
            self.connect(id, other);
        }
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

        self.glide_to(self.scene.elements[i].center());
    }

    /// The badge's click: its popover opens below it, or closes if it is open.
    pub(crate) fn toggle_node_links(&mut self, id: &str, anchor: Rect) {
        if self.links_popover.as_ref().is_some_and(|p| p.of == LinksOf::Node(id.to_string())) {
            self.links_popover = None;

            return;
        }

        let at = Pos2::new(anchor.center().x - 120.0, anchor.bottom() + 8.0);

        self.links_popover = Some(LinksPopover { of: LinksOf::Node(id.to_string()), at, rect: Rect::NOTHING, anchor, history: Vec::new(), seen: None });
    }

    /// A click or right click on ropes: the popover of the ropes under `world`, at `at`. The first
    /// band (3, 8 or 14 px of screen) with a rope says how near the nearest is; every rope within
    /// 6 px more is listed with it, so crossed ropes can be told apart.
    pub(crate) fn open_rope_popover(&mut self, at: Pos2, world: Pos2) {
        let near = |px: f32| -> Vec<String> {
            self.scene.elements.iter().filter(|e| e.rope && !e.locked && self.on_rope(e, world, px / self.zoom)).map(|e| e.id.clone()).collect()
        };

        let Some(band) = [3.0f32, 8.0, 14.0].into_iter().find(|px| !near(*px).is_empty()) else { return };
        let ids = near((band + 6.0).min(14.0));

        self.links_popover = Some(LinksPopover { of: LinksOf::Ropes(ids), at, rect: Rect::NOTHING, anchor: Rect::NOTHING, history: Vec::new(), seen: None });
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

        let pressed_outside =
            ctx.input(|i| i.pointer.any_pressed() && i.pointer.interact_pos().is_some_and(|p| !popover.rect.contains(p) && !popover.anchor.contains(p)));

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

        // a clicked rope between two agents shows what they said to each other
        let newest = Some(self.agent_events.back().map_or(0, |e| e.at));

        if !node_of && popover.seen != newest {
            let agent = |id: &str| self.scene.elements.iter().any(|e| e.id == id && e.terminal.is_some());

            popover.history = rows
                .iter()
                .filter(|(_, ends)| ends.iter().all(|id| agent(id)))
                .map(|(rope, ends)| (rope.clone(), self.connection_events(&ends[0], &ends[1], HISTORY)))
                .collect();

            popover.seen = newest;
        }

        let (c, dark) = (self.colors(), self.dark());
        let mut act = None;

        let area = egui::Area::new(egui::Id::unique("links_popover")).fixed_pos(popover.at).order(egui::Order::Foreground).show(ctx, |ui| {
            crate::app::ui::menu_look::menu_frame(dark).show(ui, |ui| {
                ui.set_width(if popover.history.is_empty() { 240.0 } else { 300.0 });
                ui.spacing_mut().item_spacing = Vec2::new(4.0, 0.0);
                let colors = crate::app::ui::menu_look::menu_colors(dark);

                if node_of {
                    // the count over the list, as a menu's section title
                    let (rect, _) = ui.allocate_exact_size(Vec2::new(ui.available_width(), 24.0), Sense::hover());
                    let title = format!("{} {}", rows.len(), if rows.len() == 1 { "conexão" } else { "conexões" });

                    ui.painter().text(
                        Pos2::new(rect.left() + 6.5, rect.center().y),
                        egui::Align2::LEFT_CENTER,
                        title,
                        FontId::proportional(11.0),
                        colors.muted,
                    );
                }

                for (index, (rope, ends)) in rows.iter().enumerate() {
                    if node_of {
                        let id = &ends[0];

                        let (icon, name) =
                            self.scene.elements.iter().find(|e| &e.id == id).map_or((Icon::Tool(Tool::Sticky), String::new()), |e| node_row(e, &nodes));

                        let (go, cut) = link_row(ui, icon, &name, &format!("Remover conexão com {name}"));

                        if go {
                            act = Some(LinksAct::Focus(id.clone()));
                        } else if cut {
                            act = Some(LinksAct::Remove(rope.clone()));
                        }

                        continue;
                    }

                    // a clicked rope, as the menus: "Conexão", its two ends (a click goes to it), Remover conexão
                    if index > 0 {
                        separator(ui, colors.border);
                    }

                    section_title(ui, "Conexão", colors.muted);

                    for id in ends {
                        let (icon, name) =
                            self.scene.elements.iter().find(|e| &e.id == id).map_or((Icon::Tool(Tool::Sticky), String::new()), |e| node_row(e, &nodes));

                        if crate::app::ui::menu_look::menu_row(ui, Some(icon), &name, "", None) {
                            act = Some(LinksAct::Focus(id.clone()));
                        }
                    }

                    let events = popover.history.iter().find(|(history_rope, _)| history_rope == rope).map(|(_, events)| events.as_slice()).unwrap_or_default();
                    let talked = popover.history.iter().any(|(history_rope, _)| history_rope == rope);

                    if talked && events.is_empty() {
                        ui.label(egui::RichText::new("Ainda não houve conversa por aqui.").size(12.0).color(c.muted));
                    }

                    for e in events {
                        let when = format!("{} · {} → {}", ago(e.age()), e.name, self.agent_label(&e.to));

                        ui.label(egui::RichText::new(when).size(12.0).color(c.muted));
                        ui.add(egui::Label::new(egui::RichText::new(&e.what).size(13.0).color(c.text)).truncate()).tip(&e.what);
                    }

                    separator(ui, colors.border);

                    if crate::app::ui::menu_look::menu_row(ui, Some(Icon::Trash), "Remover conexão", "", Some(DANGER)) {
                        act = Some(LinksAct::Remove(rope.clone()));
                    }
                }

                if let (true, Some(id)) = (node_of, &source)
                    && rows.len() > 1
                {
                    let (line, _) = ui.allocate_exact_size(Vec2::new(ui.available_width(), crate::app::ui::menu_look::SEP_HEIGHT), Sense::hover());

                    ui.painter().hline(line.x_range(), line.center().y, Stroke::new(1.0, colors.border));

                    if crate::app::ui::menu_look::menu_row(ui, Some(Icon::Trash), "Remover todas", "", Some(DANGER)) {
                        act = Some(LinksAct::RemoveAll(id.clone()));
                    }
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
