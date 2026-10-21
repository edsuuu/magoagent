//! What an agent can build on the canvas through the bridge, like the reference app's Maestro:
//! recruit another agent (a new terminal below it), open a portal (to its right), connect two
//! things, and notify the person. What it creates is roped to it.

use crate::prelude::*;
use std::sync::mpsc::Sender;

impl DrawingApp {
    /// A box the size of `want` at its place, or further down until it overlaps no window or note.
    fn free_spot(&self, want: Rect) -> Rect {
        let taken: Vec<Rect> = self.scene.elements.iter().filter(|e| e.is_node() || e.created.is_some()).map(element_bounds).collect();
        let mut at = want;
        // ponytail: walks straight down, 30 steps at most; a packed canvas may still overlap
        for _ in 0..30 {
            if !taken.iter().any(|t| t.expand(20.0).intersects(at)) {
                break;
            }
            at = at.translate(Vec2::new(0.0, want.height() + 40.0));
        }
        at
    }

    /// `recruit`: a terminal the caller's size and folder, below it, running `command`.
    pub(crate) fn recruit(&mut self, from: &str, name: &str, command: &str, reply: Sender<Result<String, String>>) {
        let Some(caller) = self.scene.elements.iter().find(|e| e.id == from) else {
            let _ = reply.send(Err("recrute de dentro de um terminal do MagoAgent".into()));
            return;
        };
        let b = local_bounds(caller);
        let cwd = caller.terminal.as_ref().map(|t| t.cwd.clone()).unwrap_or_default();
        let at = self.free_spot(Rect::from_min_size(Pos2::new(b.min.x, b.max.y + 60.0), b.size()));
        let name = self.unique_terminal_name(if name.trim().is_empty() { "Agente" } else { name.trim() }, None);
        let mut terminal = self.new_element(ElementKind::Rectangle { x: at.min.x, y: at.min.y, width: at.width(), height: at.height() });
        terminal.style = node_style(terminal.style);
        terminal.terminal = Some(TerminalSpec { name: name.clone(), cwd, command: command.trim().to_string(), ..Default::default() });
        let id = terminal.id.clone();
        self.snapshot();
        self.scene.elements.push(terminal);
        // one Cmd+Z takes back the recruit and its rope together
        self.rope_between(from, &id, false);
        self.publish_nodes();
        let _ = reply.send(Ok(format!("Agente \"{name}\" criado abaixo de você e conectado; fale com ele com `magoagent ask \"{name}\" …`.")));
    }

    /// `portal create`: a page to the caller's right, roped to it.
    pub(crate) fn create_portal(&mut self, from: &str, url: &str, name: &str, reply: Sender<Result<String, String>>) {
        let Some(caller) = self.scene.elements.iter().find(|e| e.id == from) else {
            let _ = reply.send(Err("abra portais de dentro de um terminal do MagoAgent".into()));
            return;
        };
        let b = local_bounds(caller);
        let url = normalize_url(url);
        let name = match name.trim() {
            "" if host_of(&url).is_empty() => "Portal".to_string(),
            "" => host_of(&url).to_string(),
            name => name.to_string(),
        };
        let at = self.free_spot(Rect::from_min_size(Pos2::new(b.max.x + 60.0, b.min.y), Vec2::new(720.0, 520.0)));
        let mut portal = self.new_element(ElementKind::Rectangle { x: at.min.x, y: at.min.y, width: at.width(), height: at.height() });
        portal.style = node_style(portal.style);
        portal.portal = Some(PortalSpec { name: name.clone(), url, ..Default::default() });
        let id = portal.id.clone();
        self.snapshot();
        self.scene.elements.push(portal);
        self.rope_between(from, &id, false);
        self.publish_nodes();
        let _ = reply.send(Ok(format!("Portal \"{name}\" aberto à sua direita e conectado.")));
    }

    /// `connect`: a rope between two things on the canvas, by id (the bridge resolves names).
    pub(crate) fn connect_for_agent(&mut self, a: &str, b: &str, reply: Sender<Result<String, String>>) {
        let name = |id: &str| self.agent_nodes().into_iter().find(|n| n.id == id).map(|n| n.name);
        // the bridge resolved the ids a moment ago; an end deleted since would make no rope
        let (Some(first), Some(second)) = (name(a), name(b)) else {
            let _ = reply.send(Err("uma das pontas já não está no canvas".into()));
            return;
        };
        if self.links().get(a).is_some_and(|l| l.iter().any(|x| x == b)) {
            let _ = reply.send(Ok(format!("\"{first}\" e \"{second}\" já estavam conectados.")));
            return;
        }
        self.connect(a, b);
        self.publish_nodes();
        let _ = reply.send(Ok(format!("\"{first}\" e \"{second}\" conectados.")));
    }

    /// `notify`: a toast in the app and, on macOS, a system notification.
    pub(crate) fn notify(&mut self, from: &str, text: &str) {
        let who = self.agent_nodes().into_iter().find(|n| n.id == from).map_or("Agente".to_string(), |n| n.name);
        self.toast(format!("{who}: {text}"));
        #[cfg(target_os = "macos")]
        {
            // the text goes as an argument, never inside the script; waited on so no zombie stays
            let (title, text) = (format!("MagoAgent · {who}"), text.to_string());
            std::thread::spawn(move || {
                let _ = std::process::Command::new("osascript")
                    .args(["-e", "on run argv", "-e", "display notification (item 2 of argv) with title (item 1 of argv)", "-e", "end run"])
                    .args([title, text])
                    .status();
            });
        }
    }
}
