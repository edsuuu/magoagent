//! The menus egui draws over the canvas, for Swift to show as native ones: a right click's (`ui/context/`, the note's
//! Cor and the node's connections among its rows) and a rope's connections (`ui/links.rs`). A pick runs what egui runs.

use crate::app::ui::agents::ago;
use crate::app::ui::context::Entry;
use crate::app::ui::links::{LinksOf, rope_ends};
use crate::prelude::*;

#[derive(Serialize, PartialEq)]
pub(super) struct MenuModel {
    entries: Vec<MenuEntry>,
}

#[derive(Serialize, PartialEq)]
struct MenuEntry {
    /// item, sub, sep, or title (a line that only says something).
    kind: &'static str,
    id: String,
    label: String,
    check: Option<bool>,
    enabled: bool,
    danger: bool,
    keys: String,
    items: Vec<MenuEntry>,
}

fn row(kind: &'static str, id: String, label: String, danger: bool) -> MenuEntry {
    MenuEntry { kind, id, label, check: None, enabled: true, danger, keys: String::new(), items: vec![] }
}

fn entries(rows: &[Entry]) -> Vec<MenuEntry> {
    rows.iter()
        .map(|entry| match entry {
            Entry::Item { id, label, check, danger, enabled, keys } => {
                MenuEntry { check: *check, enabled: *enabled, keys: keys_label(keys), ..row("item", id.clone(), label.clone(), *danger) }
            }
            Entry::Sub { label, items } => MenuEntry { items: entries(items), ..row("sub", String::new(), label.clone(), false) },
            Entry::Sep => row("sep", String::new(), String::new(), false),
        })
        .collect()
}

impl DrawingApp {
    pub(super) fn menu_model(&self) -> Option<MenuModel> {
        if let Some(menu) = &self.context {
            let rows = menu.items.clone().unwrap_or_else(|| self.context_entries(&menu.target));

            return Some(MenuModel { entries: entries(&rows) });
        }

        self.links_menu()
    }

    /// The connections popover as menu rows: a node's ropes (a click goes to the other end, `links:remove` cuts it), or the
    /// ropes under a click with their two ends and, between two agents, what they said (egui keeps `history` fresh).
    fn links_menu(&self) -> Option<MenuModel> {
        let popover = self.links_popover.as_ref()?;
        let nodes = self.agent_nodes();
        let name = |id: &str| nodes.iter().find(|node| node.id == id).map_or_else(|| id.to_string(), |node| node.name.clone());
        let mut out = vec![];
        let ropes = self.scene.elements.iter().filter(|e| e.rope).filter_map(|e| rope_ends(e).map(|ends| (e.id.as_str(), ends)));

        match &popover.of {
            LinksOf::Node(id) => {
                let joined: Vec<(&str, &str)> =
                    ropes.filter(|(_, (a, b))| a == id || b == id).map(|(rope, (a, b))| (rope, if a == id { b } else { a })).collect();

                let count = if joined.len() == 1 { "1 conexão".to_string() } else { format!("{} conexões", joined.len()) };

                out.push(row("title", String::new(), count, false));

                for (rope, other) in &joined {
                    out.push(row("item", format!("links:focus:{other}"), name(other), false));
                    out.push(row("item", format!("links:remove:{rope}"), format!("Remover conexão com {}", name(other)), true));
                }

                if joined.len() > 1 {
                    out.push(row("sep", String::new(), String::new(), false));
                    out.push(row("item", format!("links:all:{id}"), "Remover todas".into(), true));
                }
            }
            LinksOf::Ropes(ids) => {
                for (rope, (a, b)) in ropes.filter(|(rope, _)| ids.iter().any(|id| id == rope)) {
                    if !out.is_empty() {
                        out.push(row("sep", String::new(), String::new(), false));
                    }

                    out.push(row("title", String::new(), "Conexão".into(), false));
                    out.push(row("item", format!("links:focus:{a}"), name(a), false));
                    out.push(row("item", format!("links:focus:{b}"), name(b), false));

                    if let Some((_, events)) = popover.history.iter().find(|(history, _)| history == rope) {
                        if events.is_empty() {
                            out.push(row("title", String::new(), "Ainda não houve conversa por aqui.".into(), false));
                        }

                        for event in events {
                            let said = format!("{} · {} → {}: {}", ago(event.age()), event.name, self.agent_label(&event.to), event.what);

                            out.push(row("title", String::new(), said, false));
                        }
                    }

                    out.push(row("sep", String::new(), String::new(), false));
                    out.push(row("item", format!("links:remove:{rope}"), "Remover conexão".into(), true));
                }
            }
        }

        (!out.is_empty()).then_some(MenuModel { entries: out })
    }

    /// A right click on the canvas at `pos` (the canvas view's own points): the menu of what is under it.
    pub(super) fn shell_context_open(&mut self, pos: Pos2) {
        let world = self.to_world(pos);

        self.open_context(pos, world);
    }

    /// The row picked in the open menu (`id`), or none ("": the menu went away): what egui does with that row.
    pub(super) fn shell_menu(&mut self, id: &str) {
        if let Some(menu) = self.context.take() {
            if !id.is_empty() {
                // the app's own context: Copiar puts the text on the clipboard through it
                let ctx = crate::app::native::context().cloned().unwrap_or_default();

                self.context_action(&ctx, &menu.target, id);
            }

            return;
        }

        self.links_popover = None;
        let Some((verb, id)) = id.strip_prefix("links:").and_then(|rest| rest.split_once(':')) else { return };

        match verb {
            "focus" => self.focus_node(id),
            "all" => self.unlink_all(&[id.to_string()]),
            _ => {
                let ends = self.scene.elements.iter().find(|e| e.id == id).and_then(rope_ends).map(|(a, b)| (a.to_string(), b.to_string()));

                if let Some((a, b)) = ends {
                    self.unlink(&a, &b);
                }
            }
        }
    }
}
