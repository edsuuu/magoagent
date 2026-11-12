//! Canvas groups ("Grupo"): a named dashed frame around the elements that share a `frame_group`,
//! with a pill at its top left that drags them all, renames, colors and ungroups. Apart from
//! `group_id`, which binds a note to its text and a pile's pages together; the frame does not make
//! its members move together when one is dragged.

use crate::app::ui::context::{Entry, item, sub, tick};
use crate::prelude::*;

/// The name a group has until it is named.
pub(crate) const DEFAULT_NAME: &str = "Grupo";
/// World units between the members and the frame.
const PADDING: f32 = 16.0;
const HEADER_HEIGHT: f32 = 22.0;
const HEADER_GAP: f32 = 6.0;
const CORNER: f32 = 12.0;

impl DrawingApp {
    /// Indices of the elements in group `gid`.
    pub(crate) fn frame_members(&self, gid: &str) -> Vec<usize> {
        (0..self.scene.elements.len()).filter(|&i| self.scene.elements[i].frame_group.as_deref() == Some(gid)).collect()
    }

    /// The frame of a group in world space: its members' box plus the padding. Only with two or
    /// more members left (a note's text does not count apart from its card).
    pub(crate) fn group_frame(&self, gid: &str) -> Option<Rect> {
        let members = self.frame_members(gid);
        if members.iter().filter(|&&i| !self.is_card_text(i)).count() < 2 {
            return None;
        }
        let bounds = members.iter().map(|&i| element_bounds(&self.scene.elements[i])).reduce(|a, b| a.union(b))?;
        Some(bounds.expand(PADDING))
    }

    /// The pill of a group on screen. ponytail: its width is estimated from the name's length, not
    /// measured; exact text metrics when a font shows it off.
    fn group_pill(&self, group: &FrameGroup) -> Option<Rect> {
        let frame = self.group_frame(&group.id)?;
        let min = self.to_screen(frame.min);
        let width = 24.0 + group.name.chars().count() as f32 * 6.2;
        Some(Rect::from_min_size(Pos2::new(min.x, min.y - HEADER_GAP - HEADER_HEIGHT), Vec2::new(width, HEADER_HEIGHT)))
    }

    /// The group whose pill is under a screen point.
    pub(crate) fn group_pill_at(&self, pos: Pos2) -> Option<String> {
        self.scene.groups.iter().rev().find(|g| self.group_pill(g).is_some_and(|r| r.contains(pos))).map(|g| g.id.clone())
    }

    /// "Grupo": the selected elements (with their notes' text) join a new group, named "Grupo".
    /// It opens the name field at once; empty keeps the default.
    pub(crate) fn group_selection(&mut self) {
        let chosen: Vec<usize> = self.selected.iter().copied().filter(|&i| !self.is_card_text(i)).collect();
        if let Some(gid) = self.group_elements(&chosen, DEFAULT_NAME) {
            self.open_group_rename(&gid, "");
        }
    }

    pub(crate) fn group_elements(&mut self, chosen: &[usize], name: &str) -> Option<String> {
        let mut chosen = chosen.to_vec();
        chosen.sort_unstable();
        chosen.dedup();
        if chosen.len() < 2 {
            return None;
        }
        self.snapshot();
        let gid = new_id();
        for i in self.with_card_text(&chosen) {
            self.scene.elements[i].frame_group = Some(gid.clone());
        }
        let elements = &self.scene.elements;
        self.scene.groups.retain(|g| elements.iter().any(|e| e.frame_group.as_deref() == Some(g.id.as_str())));
        self.scene.groups.push(FrameGroup { id: gid.clone(), name: name.into(), color: None });
        Some(gid)
    }

    /// "Desagrupar": every group the selection touches goes away; its members stay where they are.
    pub(crate) fn ungroup_selection(&mut self) {
        let gids: Vec<String> = self.selected.iter().filter_map(|&i| self.scene.elements.get(i)?.frame_group.clone()).collect();
        for gid in gids {
            self.ungroup(&gid);
        }
    }

    pub(crate) fn ungroup(&mut self, gid: &str) -> bool {
        if !self.scene.groups.iter().any(|g| g.id == gid) {
            return false;
        }
        self.snapshot();
        for e in self.scene.elements.iter_mut().filter(|e| e.frame_group.as_deref() == Some(gid)) {
            e.frame_group = None;
        }
        self.scene.groups.retain(|g| g.id != gid);
        true
    }

    pub(crate) fn selected_has_group(&self) -> bool {
        self.selected.iter().any(|&i| self.scene.elements.get(i).is_some_and(|e| e.frame_group.is_some()))
    }

    pub(crate) fn rename_frame_group(&mut self, gid: &str, name: &str) {
        let name = if name.trim().is_empty() { DEFAULT_NAME } else { name.trim() };
        if self.scene.groups.iter().any(|g| g.id == gid && g.name != name) {
            self.snapshot();
            if let Some(g) = self.scene.groups.iter_mut().find(|g| g.id == gid) {
                g.name = name.into();
            }
        }
    }

    pub(crate) fn color_frame_group(&mut self, gid: &str, color: Option<[u8; 4]>) {
        if self.scene.groups.iter().any(|g| g.id == gid && g.color != color) {
            self.snapshot();
            if let Some(g) = self.scene.groups.iter_mut().find(|g| g.id == gid) {
                g.color = color;
            }
        }
    }

    /// Opens the name field of a group (the Renomear dialog, keyed `group:<id>`).
    pub(crate) fn open_group_rename(&mut self, gid: &str, current: &str) {
        self.rename = Some((format!("group:{gid}"), current.into()));
    }

    /// A press on a group's pill picks its unlocked members up: the drag moves them all.
    pub(crate) fn press_group_pill(&mut self, gid: &str) {
        self.selected = self.frame_members(gid).into_iter().filter(|&i| !self.scene.elements[i].locked && !self.is_card_text(i)).collect();
        self.moving = !self.selected.is_empty();
        self.move_snapshotted = false;
    }

    /// The rows of the menu on a group's pill.
    pub(crate) fn group_entries(&self, gid: &str) -> Vec<Entry> {
        let current = self.scene.groups.iter().find(|g| g.id == gid).and_then(|g| g.color);
        let mut colors = vec![tick("gcolor:none", "Sem cor", current.is_none())];
        colors.extend(NOTE_PAPERS.iter().enumerate().map(|(k, (c, name))| tick(&format!("gcolor:{k}"), name, current == Some(*c))));
        vec![item("rename", "Renomear"), sub("Cor", colors), Entry::Sep, item("ungroup", "Desagrupar")]
    }

    pub(crate) fn group_menu_action(&mut self, gid: &str, id: &str) {
        match id {
            "rename" => {
                let name = self.scene.groups.iter().find(|g| g.id == gid).map(|g| g.name.clone()).unwrap_or_default();
                self.open_group_rename(gid, &name);
            }
            "ungroup" => {
                self.ungroup(gid);
            }
            _ => {
                if let Some(k) = id.strip_prefix("gcolor:") {
                    self.color_frame_group(gid, k.parse::<usize>().ok().and_then(|k| NOTE_PAPERS.get(k)).map(|p| p.0));
                }
            }
        }
    }

    /// The frames and their pills, under the elements.
    pub(crate) fn paint_groups(&self, painter: &Painter) {
        let c = self.colors();
        for group in &self.scene.groups {
            let (Some(frame), Some(pill)) = (self.group_frame(&group.id), self.group_pill(group)) else { continue };
            let tint = match group.color {
                Some([r, g, b, _]) => Color32::from_rgb(r, g, b).gamma_multiply(0.75),
                None => c.muted.gamma_multiply(0.35),
            };
            let min = self.to_screen(frame.min);
            let screen = Rect::from_min_max(min, self.to_screen(frame.max));
            painter.extend(Shape::dashed_line(&rounded_path(screen, CORNER * self.zoom), Stroke::new(1.5, tint), 6.0, 4.0));
            let fill = group.color.map_or(Color32::TRANSPARENT, |[r, g, b, _]| Color32::from_rgb(r, g, b).gamma_multiply(0.12));
            painter.rect(pill, HEADER_HEIGHT / 2.0, fill, Stroke::new(1.0, tint), StrokeKind::Inside);
            painter.text(Pos2::new(pill.min.x + 12.0, pill.center().y), egui::Align2::LEFT_CENTER, &group.name, FontId::proportional(11.0), c.text);
        }
    }

    /// The MCP's `canvas_group`: list, create (from element names or ids), rename, color, ungroup.
    pub(crate) fn act_canvas_group(&mut self, args: &Value) -> Result<String, String> {
        let text = |key: &str| args[key].as_str().unwrap_or_default().trim().to_string();
        let find = |app: &DrawingApp| {
            let wanted = text("group").to_lowercase();
            app.scene
                .groups
                .iter()
                .find(|g| g.id == text("group") || g.name.to_lowercase() == wanted)
                .map(|g| g.id.clone())
                .ok_or(format!("Nenhum grupo se chama {}.", text("group")))
        };
        match text("action").as_str() {
            "list" => {
                let lines: Vec<String> = self
                    .scene
                    .groups
                    .iter()
                    .map(|g| {
                        let members: Vec<&str> =
                            self.frame_members(&g.id).into_iter().filter(|&i| !self.is_card_text(i)).map(|i| self.scene.elements[i].id.as_str()).collect();
                        let color = g.color.and_then(|c| NOTE_PAPERS.iter().find(|p| p.0 == c)).map_or("sem cor", |p| p.1);
                        format!("{} ({}, {}): {}", g.name, g.id, color, members.join(", "))
                    })
                    .collect();
                Ok(if lines.is_empty() { "Nenhum grupo.".into() } else { lines.join("\n") })
            }
            "create" => {
                let mut chosen = Vec::new();
                for n in args["elements"].as_array().into_iter().flatten().filter_map(|n| n.as_str()) {
                    chosen.push(self.scene.elements.iter().position(|e| e.id == n.trim()).map_or_else(|| self.node_named(n), Ok)?);
                }
                let name = if text("name").is_empty() { DEFAULT_NAME.to_string() } else { text("name") };
                let gid = self.group_elements(&chosen, &name).ok_or("diga 2 ou mais elementos em `elements`")?;
                Ok(format!("Grupo {name} criado ({gid})."))
            }
            "rename" => {
                let gid = find(self)?;
                self.rename_frame_group(&gid, &text("name"));
                Ok("Grupo renomeado.".into())
            }
            "color" => {
                let gid = find(self)?;
                let wanted = text("color").to_lowercase();
                let color = if wanted.is_empty() || wanted == "none" || wanted == "sem cor" {
                    None
                } else {
                    Some(
                        NOTE_PAPERS
                            .iter()
                            .find(|p| p.1.to_lowercase() == wanted)
                            .ok_or_else(|| format!("cor `{wanted}` desconhecida: use um papel de nota ou `none`"))?
                            .0,
                    )
                };
                self.color_frame_group(&gid, color);
                Ok("Cor do grupo alterada.".into())
            }
            "ungroup" => {
                let gid = find(self)?;
                self.ungroup(&gid);
                Ok("Grupo desfeito.".into())
            }
            other => Err(format!("ação desconhecida: {other}")),
        }
    }
}

/// A rounded rectangle as a closed polyline, clockwise from the top of the left edge.
fn rounded_path(r: Rect, radius: f32) -> Vec<Pos2> {
    let radius = radius.min(r.width() / 2.0).min(r.height() / 2.0).max(0.0);
    let corners = [
        (Pos2::new(r.min.x + radius, r.min.y + radius), 1.0),
        (Pos2::new(r.max.x - radius, r.min.y + radius), 1.5),
        (Pos2::new(r.max.x - radius, r.max.y - radius), 0.0),
        (Pos2::new(r.min.x + radius, r.max.y - radius), 0.5),
    ];
    let mut points = Vec::new();
    for (center, turns) in corners {
        for k in 0..=6 {
            let a = (turns + 0.5 * k as f32 / 6.0) * std::f32::consts::PI;
            points.push(center + Vec2::new(a.cos(), a.sin()) * radius);
        }
    }
    points.push(points[0]);
    points
}
