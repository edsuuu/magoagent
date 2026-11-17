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
    /// Indices of the elements in group `group_id`.
    pub(crate) fn frame_members(&self, group_id: &str) -> Vec<usize> {
        (0..self.scene.elements.len()).filter(|&index| self.scene.elements[index].frame_group.as_deref() == Some(group_id)).collect()
    }

    /// The frame of a group in world space: its members' box plus the padding. Only with two or
    /// more members left (a note's text does not count apart from its card).
    pub(crate) fn group_frame(&self, group_id: &str) -> Option<Rect> {
        let members = self.frame_members(group_id);
        if members.iter().filter(|&&index| !self.is_card_text(index)).count() < 2 {
            return None;
        }
        let bounds = members.iter().map(|&index| element_bounds(&self.scene.elements[index])).reduce(|first, second| first.union(second))?;
        Some(bounds.expand(PADDING))
    }

    /// The pill of a group on screen. ponytail: its width is estimated from the name's length, not
    /// measured; exact text metrics when a font shows it off.
    pub(crate) fn group_pill(&self, group: &FrameGroup) -> Option<Rect> {
        let frame = self.group_frame(&group.id)?;
        let min = self.to_screen(frame.min);
        let width = 24.0 + group.name.chars().count() as f32 * 6.2;
        Some(Rect::from_min_size(Pos2::new(min.x, min.y - HEADER_GAP - HEADER_HEIGHT), Vec2::new(width, HEADER_HEIGHT)))
    }

    /// The group whose pill is under a screen point.
    pub(crate) fn group_pill_at(&self, pos: Pos2) -> Option<String> {
        self.scene.groups.iter().rev().find(|group| self.group_pill(group).is_some_and(|rect| rect.contains(pos))).map(|group| group.id.clone())
    }

    /// "Grupo": the selected elements (with their notes' text) join a new group, named "Grupo".
    /// It opens the name field at once; empty keeps the default.
    pub(crate) fn group_selection(&mut self) {
        let chosen: Vec<usize> = self.selected.iter().copied().filter(|&index| !self.is_card_text(index)).collect();
        if let Some(group_id) = self.group_elements(&chosen, DEFAULT_NAME) {
            self.open_group_rename(&group_id, "");
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
        let group_id = new_id();
        for index in self.with_card_text(&chosen) {
            self.scene.elements[index].frame_group = Some(group_id.clone());
        }
        let elements = &self.scene.elements;
        self.scene.groups.retain(|group| elements.iter().any(|element| element.frame_group.as_deref() == Some(group.id.as_str())));
        self.scene.groups.push(FrameGroup { id: group_id.clone(), name: name.into(), color: None });
        Some(group_id)
    }

    /// "Desagrupar": every group the selection touches goes away; its members stay where they are.
    pub(crate) fn ungroup_selection(&mut self) {
        let group_ids: Vec<String> = self.selected.iter().filter_map(|&index| self.scene.elements.get(index)?.frame_group.clone()).collect();
        for group_id in group_ids {
            self.ungroup(&group_id);
        }
    }

    pub(crate) fn ungroup(&mut self, group_id: &str) -> bool {
        if !self.scene.groups.iter().any(|group| group.id == group_id) {
            return false;
        }
        self.snapshot();
        for element in self.scene.elements.iter_mut().filter(|element| element.frame_group.as_deref() == Some(group_id)) {
            element.frame_group = None;
        }
        self.scene.groups.retain(|group| group.id != group_id);
        true
    }

    pub(crate) fn selected_has_group(&self) -> bool {
        self.selected.iter().any(|&index| self.scene.elements.get(index).is_some_and(|element| element.frame_group.is_some()))
    }

    pub(crate) fn rename_frame_group(&mut self, group_id: &str, name: &str) {
        let name = if name.trim().is_empty() { DEFAULT_NAME } else { name.trim() };
        if self.scene.groups.iter().any(|group| group.id == group_id && group.name != name) {
            self.snapshot();
            if let Some(group) = self.scene.groups.iter_mut().find(|group| group.id == group_id) {
                group.name = name.into();
            }
        }
    }

    pub(crate) fn color_frame_group(&mut self, group_id: &str, color: Option<[u8; 4]>) {
        if self.scene.groups.iter().any(|group| group.id == group_id && group.color != color) {
            self.snapshot();
            if let Some(group) = self.scene.groups.iter_mut().find(|group| group.id == group_id) {
                group.color = color;
            }
        }
    }

    /// Opens the name field of a group (the Renomear dialog, keyed `group:<id>`).
    pub(crate) fn open_group_rename(&mut self, group_id: &str, current: &str) {
        self.rename = Some((format!("group:{group_id}"), current.into()));
    }

    /// A press on a group's pill picks its unlocked members up: the drag moves them all.
    pub(crate) fn press_group_pill(&mut self, group_id: &str) {
        self.selected = self.frame_members(group_id).into_iter().filter(|&index| !self.scene.elements[index].locked && !self.is_card_text(index)).collect();
        self.moving = !self.selected.is_empty();
        self.move_snapshotted = false;
    }

    /// The rows of the menu on a group's pill.
    pub(crate) fn group_entries(&self, group_id: &str) -> Vec<Entry> {
        let current = self.scene.groups.iter().find(|group| group.id == group_id).and_then(|group| group.color);
        let mut colors = vec![tick("gcolor:none", "Sem cor", current.is_none())];
        colors.extend(NOTE_PAPERS.iter().enumerate().map(|(index, (color, name))| tick(&format!("gcolor:{index}"), name, current == Some(*color))));
        vec![item("rename", "Renomear"), sub("Cor", colors), Entry::Sep, item("ungroup", "Desagrupar")]
    }

    pub(crate) fn group_menu_action(&mut self, group_id: &str, id: &str) {
        match id {
            "rename" => {
                let name = self.scene.groups.iter().find(|group| group.id == group_id).map(|group| group.name.clone()).unwrap_or_default();
                self.open_group_rename(group_id, &name);
            }
            "ungroup" => {
                self.ungroup(group_id);
            }
            _ => {
                if let Some(index_text) = id.strip_prefix("gcolor:") {
                    self.color_frame_group(group_id, index_text.parse::<usize>().ok().and_then(|index| NOTE_PAPERS.get(index)).map(|paper| paper.0));
                }
            }
        }
    }

    /// The frames and their pills, under the elements.
    pub(crate) fn paint_groups(&self, painter: &Painter) {
        let colors = self.colors();
        for group in &self.scene.groups {
            let (Some(frame), Some(pill)) = (self.group_frame(&group.id), self.group_pill(group)) else { continue };
            let tint = match group.color {
                Some([red, green, blue, _]) => Color32::from_rgb(red, green, blue).gamma_multiply(0.75),
                None => colors.muted.gamma_multiply(0.35),
            };
            let min = self.to_screen(frame.min);
            let screen = Rect::from_min_max(min, self.to_screen(frame.max));
            painter.extend(Shape::dashed_line(&rounded_path(screen, CORNER * self.zoom), Stroke::new(1.5, tint), 6.0, 4.0));
            let fill = group.color.map_or(Color32::TRANSPARENT, |[red, green, blue, _]| Color32::from_rgb(red, green, blue).gamma_multiply(0.12));
            painter.rect(pill, HEADER_HEIGHT / 2.0, fill, Stroke::new(1.0, tint), StrokeKind::Inside);
            painter.text(Pos2::new(pill.min.x + 12.0, pill.center().y), egui::Align2::LEFT_CENTER, &group.name, FontId::proportional(12.0), colors.text);
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
                .find(|group| group.id == text("group") || group.name.to_lowercase() == wanted)
                .map(|group| group.id.clone())
                .ok_or(format!("Nenhum grupo se chama {}.", text("group")))
        };
        // `elements` by id or name
        let chosen = |app: &DrawingApp| -> Result<Vec<usize>, String> {
            let names = args["elements"].as_array().into_iter().flatten().filter_map(|entry| entry.as_str());
            names.map(|name| app.scene.elements.iter().position(|element| element.id == name.trim()).map_or_else(|| app.node_named(name), Ok)).collect()
        };
        match text("action").as_str() {
            "list" => {
                let lines: Vec<String> = self
                    .scene
                    .groups
                    .iter()
                    .map(|group| {
                        let members: Vec<&str> = self
                            .frame_members(&group.id)
                            .into_iter()
                            .filter(|&index| !self.is_card_text(index))
                            .map(|index| self.scene.elements[index].id.as_str())
                            .collect();
                        let color =
                            group.color.and_then(|paper_color| NOTE_PAPERS.iter().find(|paper| paper.0 == paper_color)).map_or("sem cor", |paper| paper.1);
                        format!("{} ({}, {}): {}", group.name, group.id, color, members.join(", "))
                    })
                    .collect();
                Ok(if lines.is_empty() { "Nenhum grupo.".into() } else { lines.join("\n") })
            }
            "create" => {
                let chosen = chosen(self)?;
                let name = if text("name").is_empty() { DEFAULT_NAME.to_string() } else { text("name") };
                let group_id = self.group_elements(&chosen, &name).ok_or("diga 2 ou mais elementos em `elements`")?;
                Ok(format!("Grupo {name} criado ({group_id})."))
            }
            // Organizar (⇧Ctrl T): the elements chosen in a grid; again on the same ones, a row, then a column
            "tidy" => {
                let chosen = chosen(self)?;
                if chosen.len() < 2 {
                    return Err("diga 2 ou mais elementos em `elements`".into());
                }
                self.selected = chosen;
                self.tidy_selection();
                let layout = self.tidy_last.as_ref().map_or(0, |(_, layout)| *layout);
                Ok(format!("Organizados em {}.", ["grade", "linha", "coluna"][layout]))
            }
            "rename" => {
                let group_id = find(self)?;
                self.rename_frame_group(&group_id, &text("name"));
                Ok("Grupo renomeado.".into())
            }
            "color" => {
                let group_id = find(self)?;
                let wanted = text("color").to_lowercase();
                let color = if wanted.is_empty() || wanted == "none" || wanted == "sem cor" {
                    None
                } else {
                    Some(
                        NOTE_PAPERS
                            .iter()
                            .find(|paper| paper.1.to_lowercase() == wanted)
                            .ok_or_else(|| format!("cor `{wanted}` desconhecida: use um papel de nota ou `none`"))?
                            .0,
                    )
                };
                self.color_frame_group(&group_id, color);
                Ok("Cor do grupo alterada.".into())
            }
            "ungroup" => {
                let group_id = find(self)?;
                self.ungroup(&group_id);
                Ok("Grupo desfeito.".into())
            }
            other => Err(format!("ação desconhecida: {other}")),
        }
    }
}

/// A rounded rectangle as a closed polyline, clockwise from the top of the left edge.
fn rounded_path(rect: Rect, radius: f32) -> Vec<Pos2> {
    let radius = radius.min(rect.width() / 2.0).min(rect.height() / 2.0).max(0.0);
    let corners = [
        (Pos2::new(rect.min.x + radius, rect.min.y + radius), 1.0),
        (Pos2::new(rect.max.x - radius, rect.min.y + radius), 1.5),
        (Pos2::new(rect.max.x - radius, rect.max.y - radius), 0.0),
        (Pos2::new(rect.min.x + radius, rect.max.y - radius), 0.5),
    ];
    let mut points = Vec::new();
    for (center, turns) in corners {
        for step in 0..=6 {
            let angle = (turns + 0.5 * step as f32 / 6.0) * std::f32::consts::PI;
            points.push(center + Vec2::new(angle.cos(), angle.sin()) * radius);
        }
    }
    points.push(points[0]);
    points
}
