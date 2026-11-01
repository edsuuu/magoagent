//! What the agents do on the app through the MCP's `act` (bridge.rs): the IDE on a folder with
//! its structure, a file or a PDF opened where it reads, a window resized. Each action is a name
//! and its JSON arguments; a new one is a case here and a tool in mcp/calls.rs.

use crate::app::chat_direct::folder;
use crate::prelude::*;
use std::path::Path;

/// Files a page shows better than the editor: they open in a portal.
const PAGE_KINDS: [&str; 9] = ["pdf", "png", "jpg", "jpeg", "gif", "webp", "svg", "html", "htm"];

impl DrawingApp {
    /// The bridge's `act`: one of the app's actions, by name.
    pub(crate) fn act(&mut self, action: &str, args: &Value) -> Result<String, String> {
        let text = |key: &str| args[key].as_str().unwrap_or_default().trim().to_string();
        match action {
            "ide_folder" => self.act_ide_folder(&text("path")),
            "open_path" => self.act_open_path(&text("path"), args["line"].as_u64().map(|l| l as usize)),
            "resize_window" => self.act_resize(&text("target"), &text("state"), args["width"].as_f64(), args["height"].as_f64()),
            other => Err(format!("ação desconhecida: {other}")),
        }
    }

    /// The IDE on `path` (the one already there, or a new one), enlarged, and the folder's
    /// structure for the agent.
    fn act_ide_folder(&mut self, path: &str) -> Result<String, String> {
        let index = self.ide_for_folder(path)?;
        self.show_ide(index);
        let root = self.scene.elements[index].ide.as_ref().map(|i| i.root.clone()).unwrap_or_default();
        Ok(format!("A pasta {root} está aberta no IDE. Estrutura:\n{}", folder_tree(Path::new(&root), 3, 300)))
    }

    /// The IDE whose project is `path`, made in the middle of the view when there is none.
    pub(crate) fn ide_for_folder(&mut self, path: &str) -> Result<usize, String> {
        let root = folder(path).filter(|p| p.is_dir()).ok_or_else(|| format!("A pasta {path} não existe."))?;
        let root = root.to_string_lossy().trim_end_matches(['/', '\\']).to_string();
        let same = |e: &Element| e.ide.as_ref().is_some_and(|i| folder(&i.root).is_some_and(|p| p.to_string_lossy().trim_end_matches(['/', '\\']) == root));
        if let Some(index) = self.scene.elements.iter().position(same) {
            return Ok(index);
        }
        let at = Rect::from_center_size(self.to_world(self.canvas_rect.center()), IDE_SIZE);
        let mut ide = self.new_element(ElementKind::Rectangle { x: at.min.x, y: at.min.y, width: at.width(), height: at.height() });
        ide.style = node_style(ide.style);
        let name = Path::new(&root).file_name().map_or("IDE".to_string(), |n| n.to_string_lossy().to_string());
        ide.ide = Some(IdeSpec { name, root, ..Default::default() });
        self.snapshot();
        self.scene.elements.push(ide);
        Ok(self.scene.elements.len() - 1)
    }

    fn show_ide(&mut self, index: usize) {
        self.chat_open = false;
        self.enlarge_ide(index);
        self.ide_focus = Some(self.scene.elements[index].id.clone());
    }

    /// A folder opens its IDE; a PDF, a picture or a page opens in a portal; any other file in the
    /// IDE of its project (or of its folder), at `line`.
    fn act_open_path(&mut self, path: &str, line: Option<usize>) -> Result<String, String> {
        let full = folder(path).filter(|p| p.exists()).ok_or_else(|| format!("{path} não existe."))?;
        if full.is_dir() {
            return self.act_ide_folder(path);
        }
        let ext = full.extension().map(|e| e.to_string_lossy().to_lowercase()).unwrap_or_default();
        if PAGE_KINDS.contains(&ext.as_str()) {
            let url = format!("file:///{}", full.to_string_lossy().replace('\\', "/").trim_start_matches('/'));
            let name = full.file_name().map_or(String::new(), |n| n.to_string_lossy().to_string());
            let at = self.free_spot(Rect::from_center_size(self.to_world(self.canvas_rect.center()), Vec2::new(720.0, 860.0)));
            let mut portal = self.new_element(ElementKind::Rectangle { x: at.min.x, y: at.min.y, width: at.width(), height: at.height() });
            portal.style = node_style(portal.style);
            portal.portal = Some(PortalSpec { name: name.clone(), url, ..Default::default() });
            self.snapshot();
            self.scene.elements.push(portal);
            self.publish_nodes();
            self.show_canvas();
            self.fit_view(at, None);
            return Ok(format!("{name} aberto num portal no canvas."));
        }
        // the IDE whose project holds it, else one on its folder
        let inside = |e: &Element| e.ide.as_ref().and_then(|i| folder(&i.root)).is_some_and(|root| full.starts_with(root));
        let index = match self.scene.elements.iter().position(inside) {
            Some(index) => index,
            None => self.ide_for_folder(&full.parent().unwrap_or(Path::new("")).to_string_lossy())?,
        };
        self.show_ide(index);
        // a new IDE's editor opens on the next frame (sync_ides): the file waits for it
        self.open_pending = Some((self.scene.elements[index].id.clone(), full.clone(), line));
        Ok(format!("{} aberto no IDE.", full.display()))
    }

    /// The app's window (`app`: maximize, restore, minimize, or a size) or a terminal, portal or
    /// IDE on the canvas (enlarge, shrink, or a size).
    fn act_resize(&mut self, target: &str, state: &str, width: Option<f64>, height: Option<f64>) -> Result<String, String> {
        let size = |w: f32, h: f32| Vec2::new(width.map_or(w, |v| v as f32).max(200.0), height.map_or(h, |v| v as f32).max(120.0));
        if target.is_empty() || target.eq_ignore_ascii_case("app") {
            let command = match state {
                "maximize" => egui::ViewportCommand::Maximized(true),
                "restore" => egui::ViewportCommand::Maximized(false),
                "minimize" => egui::ViewportCommand::Minimized(true),
                _ if width.is_some() || height.is_some() => egui::ViewportCommand::InnerSize(size(1280.0, 800.0)),
                _ => return Err("diga state (maximize, restore, minimize) ou width/height".into()),
            };
            self.viewport_commands.push(command);
            return Ok("Janela do MagoAgent ajustada.".into());
        }
        let wanted = target.to_lowercase();
        let named = |e: &Element| {
            let names = [e.terminal.as_ref().map(|t| t.label()), e.portal.as_ref().map(|p| p.name.as_str()), e.ide.as_ref().map(|i| i.name.as_str())];
            e.id == target || names.iter().flatten().any(|n| n.to_lowercase() == wanted)
        };
        let index = self.scene.elements.iter().position(named).ok_or_else(|| format!("Nada no canvas se chama {target}."))?;
        let id = self.scene.elements[index].id.clone();
        match state {
            "enlarge" | "maximize" => {
                self.chat_open = false;
                if self.scene.elements[index].ide.is_some() {
                    self.show_ide(index)
                } else {
                    self.set_elevated(Some(id))
                }
                Ok(format!("{target} ampliado."))
            }
            "shrink" | "restore" => {
                if self.terminal_elevated.as_deref() == Some(id.as_str()) {
                    self.set_elevated(None);
                }
                Ok(format!("{target} de volta ao canvas."))
            }
            _ => {
                let ElementKind::Rectangle { width: w, height: h, .. } = &mut self.scene.elements[index].kind else {
                    return Err(format!("{target} não é uma janela"));
                };
                let new = size(*w, *h);
                self.snapshot();
                if let ElementKind::Rectangle { width: w, height: h, .. } = &mut self.scene.elements[index].kind {
                    (*w, *h) = (new.x, new.y);
                }
                Ok(format!("{target} agora tem {:.0}×{:.0}.", new.x, new.y))
            }
        }
    }

    /// Each frame: a file waiting for its new IDE's editor, the window's own commands.
    pub(crate) fn actions_tick(&mut self, ctx: &Context) {
        for command in std::mem::take(&mut self.viewport_commands) {
            ctx.send_viewport_cmd(command);
        }
        if let Some((id, path, line)) = self.open_pending.clone()
            && self.ides.contains_key(&id)
        {
            self.open_pending = None;
            let (reply, _) = std::sync::mpsc::channel();
            self.ide_agent(&id, IdeAction::Open { path, line }, reply);
        }
    }
}

/// A folder's structure as indented lines, `depth` levels and `cap` lines at most; hidden
/// entries and build folders left out.
pub(crate) fn folder_tree(root: &Path, depth: usize, cap: usize) -> String {
    fn walk(dir: &Path, level: usize, depth: usize, lines: &mut Vec<String>, cap: usize) {
        let Ok(read) = fs::read_dir(dir) else { return };
        let mut entries: Vec<(bool, String, PathBuf)> = read
            .flatten()
            .map(|e| (e.path().is_dir(), e.file_name().to_string_lossy().to_string(), e.path()))
            .filter(|(_, name, _)| !name.starts_with('.') && !["target", "node_modules", "__pycache__"].contains(&name.as_str()))
            .collect();
        entries.sort_by(|a, b| b.0.cmp(&a.0).then(a.1.to_lowercase().cmp(&b.1.to_lowercase())));
        for (is_dir, name, path) in entries {
            if lines.len() >= cap {
                lines.push("…".into());
                return;
            }
            lines.push(format!("{}{name}{}", "  ".repeat(level), if is_dir { "/" } else { "" }));
            if is_dir && level + 1 < depth {
                walk(&path, level + 1, depth, lines, cap);
            }
        }
    }
    let mut lines = Vec::new();
    walk(root, 0, depth, &mut lines, cap);
    if lines.last().is_some_and(|l| l == "…") {
        lines.dedup();
    }
    lines.join("\n")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_agents_open_a_folder_a_pdf_and_resize_windows() {
        let dir = std::env::temp_dir().join(format!("mago-acts-{}", std::process::id()));
        fs::create_dir_all(dir.join("src/fundo")).unwrap();
        fs::write(dir.join("src/main.rs"), "fn main() {}\n").unwrap();
        fs::write(dir.join("manual.pdf"), "%PDF-1.4\n").unwrap();
        fs::write(dir.join(".oculto"), "").unwrap();
        let tree = folder_tree(&dir, 3, 300);
        assert_eq!(tree, "src/\n  fundo/\n  main.rs\nmanual.pdf");
        let mut app = DrawingApp::default();
        app.canvas_rect = Rect::from_min_size(Pos2::ZERO, Vec2::new(800.0, 600.0));
        app.chat_open = true;
        let path = dir.to_string_lossy().to_string();
        // the folder: an IDE on it, enlarged, and its structure
        assert!(app.act("ide_folder", &json!({ "path": path })).unwrap().contains("main.rs"));
        let ide = app.scene.elements.iter().position(|e| e.ide.is_some()).unwrap();
        assert_eq!(app.terminal_elevated.as_ref(), Some(&app.scene.elements[ide].id));
        assert!(!app.chat_open);
        // a source file goes to that same IDE, once its editor is up
        app.act("open_path", &json!({ "path": dir.join("src/main.rs").to_string_lossy(), "line": 1 })).unwrap();
        assert_eq!(app.scene.elements.iter().filter(|e| e.ide.is_some()).count(), 1);
        assert!(app.open_pending.as_ref().is_some_and(|(id, ..)| *id == app.scene.elements[ide].id));
        // a PDF opens in a portal
        app.act("open_path", &json!({ "path": dir.join("manual.pdf").to_string_lossy() })).unwrap();
        let portal = app.scene.elements.iter().find_map(|e| e.portal.as_ref()).unwrap();
        assert!(portal.url.starts_with("file:///") && portal.url.ends_with("manual.pdf"), "{}", portal.url);
        // resized by name, enlarged, back, and the app's window
        let name = app.scene.elements[ide].ide.as_ref().unwrap().name.clone();
        app.act("resize_window", &json!({ "target": name, "width": 900, "height": 700 })).unwrap();
        assert!(matches!(app.scene.elements[ide].kind, ElementKind::Rectangle { width: 900.0, height: 700.0, .. }));
        app.act("resize_window", &json!({ "target": name, "state": "shrink" })).unwrap();
        assert!(app.terminal_elevated.is_none());
        app.act("resize_window", &json!({ "target": "app", "state": "maximize" })).unwrap();
        assert_eq!(app.viewport_commands, vec![egui::ViewportCommand::Maximized(true)]);
        assert!(app.act("resize_window", &json!({ "target": "nada" })).is_err());
        let _ = fs::remove_dir_all(&dir);
    }
}
