//! Terminals on the canvas: their shells, typing into them, painting them, and the lines that
//! connect them so agents can reach each other through the bridge.

pub(crate) mod actions;
pub(crate) mod card;
mod dialog;
pub(crate) mod find;
pub(crate) mod menu;
pub(crate) mod prefs;
pub(crate) mod procs;

use crate::app::ui::context::{ContextMenu, Target};
use crate::prelude::*;
use alacritty_terminal::term::TermMode;
use std::collections::HashMap;
mod focus;
mod overlay;
mod ropes;
mod sync;

/// The new / edit terminal dialog.
#[derive(Clone, Debug)]
pub(crate) struct TerminalDialog {
    /// The terminal being edited; None creates one in `at`.
    pub(crate) target: Option<String>,
    pub(crate) at: Rect,
    pub(crate) spec: TerminalSpec,
    /// Detalhes or Aparência
    pub(crate) tab: usize,
}

/// The key of the rope that follows the pointer while "Conectar" waits (in `ropes`, not an element).
pub(crate) const CONNECT_DRAFT: &str = "connect-draft";

/// The names new agents draw from, one each.
pub(crate) const AGENT_NAMES: &[&str] = &[
    "Sábia",
    "Fuminho",
    "Fumaça",
    "Faísca",
    "Brasa",
    "Centelha",
    "Neblina",
    "Trovão",
    "Cometa",
    "Coruja",
    "Runa",
    "Poção",
    "Feitiço",
    "Lume",
    "Orvalho",
    "Vagalume",
    "Ventania",
    "Cristal",
    "Eclipse",
    "Aurora",
    "Bruma",
    "Chama",
    "Relâmpago",
    "Corvo",
    "Grimório",
    "Amuleto",
    "Caldeirão",
    "Varinha",
    "Pirilampo",
    "Garoa",
    "Sereno",
    "Tição",
];

/// A terminal's or portal's dark box: what exports and the AI's view show (the app paints the live
/// node over it).
pub(crate) fn node_style(style: Style) -> Style {
    Style { stroke: [0x3a, 0x3a, 0x3a, 255], fill: Some([0x18, 0x18, 0x18, 255]), stroke_width: 1.0, roughness: 0, round: true, ..style }
}

/// `~` for the home folder.
pub(crate) fn display_folder(cwd: &str) -> String {
    let home = std::env::home_dir().map(|h| h.to_string_lossy().into_owned()).unwrap_or_default();
    match cwd.strip_prefix(&home) {
        _ if cwd.is_empty() => "~".into(),
        Some(rest) if !home.is_empty() => format!("~{rest}"),
        _ => cwd.into(),
    }
}

impl DrawingApp {
    /// Ids each element is connected to by a line bound at both ends.
    pub(crate) fn links(&self) -> HashMap<String, Vec<String>> {
        links_in(&self.scene)
    }

    /// What agents can reach: terminals, chats, sticky notes (their label is the text), portals and file trees.
    pub(crate) fn agent_nodes(&self) -> Vec<Node> {
        let scene = &self.scene;
        let links = links_in(scene);
        let labels: HashMap<&str, &str> = scene
            .elements
            .iter()
            .filter_map(|e| match &e.kind {
                ElementKind::Text { value, .. } => Some((e.id.strip_suffix("-label")?, value.as_str())),
                _ => None,
            })
            .collect();
        let label = |id: &str| labels.get(id).map(|v| v.to_string());
        let named = |name: &str, fallback: &str| if name.trim().is_empty() { fallback.to_string() } else { name.trim().to_string() };
        scene
            .elements
            .iter()
            .filter_map(|e| {
                let (kind, name, text) = if let Some(t) = &e.terminal {
                    (NodeKind::Terminal, named(&t.name, "Terminal"), String::new())
                } else if let Some(p) = &e.portal {
                    (NodeKind::Portal, named(&p.name, "Navegador"), p.url.clone())
                } else if e.created.is_some() {
                    let text = label(&e.id).unwrap_or_default();
                    (NodeKind::Note, note_name(&text), text)
                } else if let Some(tree) = &e.filetree {
                    (NodeKind::Tree, tree.name(), tree.root_path().to_string_lossy().into())
                } else if let Some(chat) = &e.chat {
                    (NodeKind::Chat, self.maestro_name(&chat.name), String::new())
                } else {
                    return None;
                };
                Some(Node {
                    id: e.id.clone(),
                    kind,
                    name,
                    text,
                    links: links.get(&e.id).cloned().unwrap_or_default(),
                    // a chat is the Maestro: it recruits, opens portals and connects
                    maestro: e.terminal.as_ref().is_some_and(|t| t.maestro) || e.chat.is_some() && !self.is_recruit_chat(&e.id),
                })
            })
            .collect()
    }
}

/// Element ids roped together in a drawing, both ways.
fn links_in(scene: &Scene) -> HashMap<String, Vec<String>> {
    let mut links: HashMap<String, Vec<String>> = HashMap::new();
    for e in &scene.elements {
        if let (ElementKind::Line { .. }, Some(a), Some(b)) = (&e.kind, &e.ends.start, &e.ends.end)
            && a.element_id != b.element_id
        {
            links.entry(a.element_id.clone()).or_default().push(b.element_id.clone());
            links.entry(b.element_id.clone()).or_default().push(a.element_id.clone());
        }
    }
    links
}

impl DrawingApp {}

/// Insert or Delete held down: egui-winit sends Ctrl+Insert and Shift+Delete as copy and cut on
/// Windows, with no key event to tell them from Ctrl+C and Ctrl+X.
fn edit_key_held() -> bool {
    #[cfg(windows)]
    {
        unsafe extern "system" {
            fn GetKeyState(key: i32) -> i16;
        }
        const VK_INSERT: i32 = 0x2D;
        const VK_DELETE: i32 = 0x2E;
        // the state as of the key message being handled: the top bit is "down"
        unsafe { GetKeyState(VK_INSERT) < 0 || GetKeyState(VK_DELETE) < 0 }
    }
    #[cfg(not(windows))]
    false
}
