use super::*;

impl DrawingApp {
    pub(super) fn act_skills(&self, action: &str) -> Result<String, String> {
        crate::app::maestro::skills::act(&crate::app::maestro::skills::skills_path().ok_or("a pasta de dados não está disponível")?, action)
    }

    /// The MCP's `connection_history`: the last `n` messages between terminals `a` and `b`, by name.
    pub(super) fn act_connection_history(&self, a: &str, b: &str, n: usize) -> Result<String, String> {
        let rows = self.agent_rows();
        let find = |name: &str| {
            rows.iter()
                .find(|r| r.id == name || r.spec.label().to_lowercase() == name.to_lowercase())
                .map(|r| (r.id.clone(), r.spec.label().to_string()))
                .ok_or_else(|| format!("Nenhum agente chamado {name}."))
        };
        let ((a, a_name), (b, b_name)) = (find(a)?, find(b)?);
        let events = self.connection_events(&a, &b, n);
        if events.is_empty() {
            return Ok(format!("{a_name} e {b_name} ainda não conversaram pela conexão."));
        }
        let mut out = format!("Mensagens entre {a_name} e {b_name}, da mais antiga à mais nova:");
        for e in &events {
            out += &format!("\n- {} · {}", crate::app::ui::agents::ago(e.age()), self.event_line(e));
        }
        Ok(out)
    }

    /// The IDE on `path` (the one already there, or a new one), enlarged, and the folder's
    /// structure for the agent.
    pub(super) fn act_ide_folder(&mut self, path: &str) -> Result<String, String> {
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

    pub(super) fn show_ide(&mut self, index: usize) {
        self.chat_open = false;
        self.enlarge_ide(index);
        self.ide_focus = Some(self.scene.elements[index].id.clone());
    }

    /// A folder opens its IDE; a PDF, a picture or a page opens in a portal; any other file in the
    /// IDE of its project (or of its folder), at `line`.
    pub(super) fn act_open_path(&mut self, path: &str, line: Option<usize>) -> Result<String, String> {
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
            return Ok(format!("{name} aberto num navegador no canvas."));
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
    pub(super) fn act_resize(&mut self, target: &str, state: &str, width: Option<f64>, height: Option<f64>) -> Result<String, String> {
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
        let index = self.node_named(target)?;
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

    /// A terminal, portal, IDE or note on the canvas by id or name (as list_agents shows it).
    pub(crate) fn node_named(&self, name: &str) -> Result<usize, String> {
        let wanted = name.trim().to_lowercase();
        let node = self.agent_nodes().into_iter().find(|n| n.id == name.trim() || n.name.to_lowercase() == wanted);
        node.and_then(|n| self.scene.elements.iter().position(|e| e.id == n.id)).ok_or_else(|| format!("Nada no canvas se chama {name}."))
    }

    /// The IDE by name; unnamed, the one enlarged, else the only one.
    pub(super) fn ide_named(&self, name: &str) -> Result<(usize, String), String> {
        let ides: Vec<usize> = (0..self.scene.elements.len()).filter(|&i| self.scene.elements[i].ide.is_some()).collect();
        let pick = if name.is_empty() {
            let enlarged = ides.iter().copied().find(|&i| self.terminal_elevated.as_deref() == Some(self.scene.elements[i].id.as_str()));
            enlarged.or(if ides.len() == 1 { Some(ides[0]) } else { None })
        } else {
            ides.iter().copied().find(|&i| self.scene.elements[i].ide.as_ref().is_some_and(|s| s.name.eq_ignore_ascii_case(name)))
        };
        let index = pick.ok_or_else(|| if ides.is_empty() { "Não há IDE no canvas.".to_string() } else { "Diga qual IDE.".to_string() })?;
        Ok((index, self.scene.elements[index].id.clone()))
    }

    /// The chat whose name has `name` (empty: the one shown).
    pub(crate) fn chat_named(&self, name: &str) -> Result<String, String> {
        if name.trim().is_empty() {
            return self.chat_agent.clone().filter(|id| self.is_chat(id)).ok_or_else(|| "Nenhum chat aberto.".into());
        }
        let wanted = name.trim().to_lowercase();
        let found = self
            .chats
            .iter()
            .find(|c| c.id.to_lowercase() == wanted || c.name.to_lowercase() == wanted)
            .or_else(|| self.chats.iter().find(|c| c.name.to_lowercase().contains(&wanted)));
        found.map(|c| c.id.clone()).ok_or_else(|| format!("Nenhum chat se chama {name}."))
    }
}
