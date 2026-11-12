//! A chat on the canvas: a card (the agent's mark, the name, the folder, its state and the last
//! message) that opens the chat screen and is roped to terminals and to other chats like any
//! window. A chat started in Chats is always the Maestro on the canvas (its agent has the Maestro's
//! hands, chat/tools.rs): the card says so, the bridge calls it so (`maestro_name`), and what it is
//! roped to is the team it commands. The card's element has the chat's own id, so ropes, the
//! bridge and the events need no second id for it. The conversation stays in chats.json
//! (chat/direct.rs) whatever happens to the card: taking the card off the canvas keeps the chat in
//! the list, removing the chat takes its card.

use crate::app::chat::list::clipped;
use crate::app::ui::agents::{AMBER, AgentState, ago};
use crate::app::ui::context::{Entry, Target, entry, item, sub};
use crate::prelude::*;
use crate::transcript::ChatItem;

/// Some makes a rectangle a chat's card. What it holds is a copy of chats.json, for exports and
/// the agents' view of the drawing, kept current by `chat_nodes_tick`.
#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub(crate) struct ChatSpec {
    pub(crate) name: String,
    /// "claude" or "codex".
    pub(crate) agent: String,
    pub(crate) cwd: String,
    /// A recruit's card folded to its name and task: the height it had open (chat/recruit_card.rs).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) folded: Option<f32>,
}

/// The size a new chat's card is born with, and the smallest it can be resized to.
pub(crate) const CHAT_NODE_SIZE: Vec2 = Vec2::new(300.0, 150.0);
pub(crate) const CHAT_NODE_MIN: Vec2 = Vec2::new(220.0, 110.0);

/// `magoagent list|ask|…` works in what the app starts (a terminal's shell, a chat's agent): this
/// app's folder first on PATH.
pub(crate) fn app_path() -> Option<(String, String)> {
    let exe = std::env::current_exe().ok()?;
    let path = std::env::var_os("PATH").unwrap_or_default();
    let dirs = exe.parent().map(PathBuf::from).into_iter().chain(std::env::split_paths(&path));
    Some(("PATH".into(), std::env::join_paths(dirs).ok()?.to_string_lossy().into()))
}

/// The mascot's size on screen under which its squares stop reading as a figure.
const MIN_FIGURE: f32 = 26.0;

impl DrawingApp {
    /// The chat's card on the canvas shown.
    pub(crate) fn chat_node(&self, id: &str) -> Option<usize> {
        self.scene.elements.iter().position(|e| e.id == id && e.chat.is_some())
    }

    /// The chat is a Maestro's recruit (an ordinary agent), not a Maestro.
    pub(crate) fn is_recruit_chat(&self, id: &str) -> bool {
        self.chats.iter().any(|c| c.id == id && c.recruit)
    }

    /// What the chat named `chat` is called on the canvas, by the agents and in the lists: the
    /// Maestro, with the chat's own name when the canvas shown has more than one.
    pub(crate) fn maestro_name(&self, chat: &str) -> String {
        // a recruit is no Maestro: it goes by its own agent name
        if let Some(c) = self.chats.iter().find(|c| c.recruit && (c.name == chat || c.id == chat)) {
            return c.name.clone();
        }
        if self.scene.elements.iter().filter(|e| e.chat.is_some() && !self.is_recruit_chat(&e.id)).count() > 1 {
            format!("Maestro ({})", chat.trim())
        } else {
            "Maestro".into()
        }
    }

    /// Where a new card goes: the free place nearest to `middle`, looked for around it a card at a
    /// time (a window under the middle of the view would otherwise push it down out of sight).
    pub(crate) fn chat_spot(&self, middle: Pos2) -> Rect {
        let taken: Vec<Rect> = self.scene.elements.iter().filter(|e| e.is_node() || e.created.is_some()).map(|e| element_bounds(e).expand(20.0)).collect();
        let step = CHAT_NODE_SIZE + Vec2::splat(40.0);
        // ponytail: four cards each way, nearest first; past that, straight down like a recruit (`free_spot`)
        let mut places: Vec<Vec2> = (-4..=4).flat_map(|i| (-4..=4).map(move |j| Vec2::new(i as f32 * step.x, j as f32 * step.y))).collect();
        places.sort_by(|a, b| a.length_sq().total_cmp(&b.length_sq()));
        let free = places.into_iter().map(|d| Rect::from_center_size(middle + d, CHAT_NODE_SIZE)).find(|r| !taken.iter().any(|t| t.intersects(*r)));
        free.unwrap_or_else(|| self.free_spot(Rect::from_center_size(middle, CHAT_NODE_SIZE)))
    }

    /// The chat's card on this canvas: the one already there, or a new one centered on `at`, else
    /// in a free spot near the middle of the view. None for what is not a chat.
    pub(crate) fn add_chat_node(&mut self, id: &str, at: Option<Pos2>) -> Option<usize> {
        if let Some(i) = self.chat_node(id) {
            return Some(i);
        }
        let chat = self.chats.iter().find(|c| c.id == id)?;
        let spec = ChatSpec { name: chat.name.clone(), agent: chat.agent.clone(), cwd: chat.cwd.clone(), folded: None };
        // a canvas not laid out yet (the first frame, a test) has no middle: the drawing's origin
        let middle = if self.canvas_rect.is_positive() { self.to_world(self.canvas_rect.center()) } else { Pos2::ZERO };
        let want = match at {
            Some(at) => Rect::from_center_size(at, CHAT_NODE_SIZE),
            None => self.chat_spot(middle),
        };
        let mut node = self.new_element(ElementKind::Rectangle { x: want.min.x, y: want.min.y, width: want.width(), height: want.height() });
        node.id = id.to_string();
        node.style = node_style(node.style);
        node.chat = Some(spec);
        self.snapshot();
        self.scene.elements.push(node);
        Some(self.scene.elements.len() - 1)
    }

    /// "Mostrar no canvas": the canvas, on the chat's card (made now when this canvas has none),
    /// selected and in the middle of the view.
    pub(crate) fn show_chat_on_canvas(&mut self, id: &str) -> Result<String, String> {
        let had = self.chat_node(id).is_some();
        let i = self.add_chat_node(id, None).ok_or("Esse chat não existe mais.")?;
        self.show_canvas();
        self.selected = vec![i];
        self.glide_to(self.scene.elements[i].center());
        let name = self.agent_label(id);
        Ok(if had { format!("Canvas no cartão do chat {name}.") } else { format!("Chat {name} agora tem um cartão no canvas.") })
    }

    /// The chat is gone (Remover chat): so is its card on this canvas; its ropes follow (`swing_ropes`).
    /// The cards other workspaces keep of it go when they come on screen (`chat_nodes_tick`).
    pub(crate) fn drop_chat_node(&mut self, id: &str) {
        if self.chat_node(id).is_some() {
            self.keeping_selection(|app| app.scene.elements.retain(|e| !(e.id == id && e.chat.is_some())));
            self.dirty = true;
        }
    }

    /// Each frame: a card whose chat no longer exists goes (an undo brought it back, a workspace
    /// kept it, a drawing came from elsewhere), the others say what chats.json says now; the cards
    /// in view read their conversation once a second, for the last message they show; and the
    /// questions other agents asked go to the chats free to hear them (chat/ask.rs).
    pub(crate) fn chat_nodes_tick(&mut self, ctx: &Context) {
        let chats = &self.chats;
        let orphan = |e: &Element| e.chat.is_some() && !chats.iter().any(|c| c.id == e.id);
        if !self.in_gesture() && self.scene.elements.iter().any(orphan) {
            self.keeping_selection(|app| {
                let chats = &app.chats;
                app.scene.elements.retain(|e| e.chat.is_none() || chats.iter().any(|c| c.id == e.id));
            });
            self.dirty = true;
        }
        for e in &mut self.scene.elements {
            let Some(spec) = e.chat.as_mut() else { continue };
            let Some(chat) = self.chats.iter().find(|c| c.id == e.id) else { continue };
            if spec.name != chat.name || spec.agent != chat.agent || spec.cwd != chat.cwd {
                (spec.name, spec.agent, spec.cwd) = (chat.name.clone(), chat.agent.clone(), chat.cwd.clone());
                self.dirty = true;
            }
        }
        if !self.chat_open && self.now - self.chat_nodes_read >= 1.0 {
            self.chat_nodes_read = self.now;
            let view = self.canvas_rect;
            let seen: Vec<String> = self
                .scene
                .elements
                .iter()
                .filter(|e| e.chat.is_some() && (self.terminal_frame(e).0.intersects(view) || self.is_recruit_chat(&e.id)))
                .map(|e| e.id.clone())
                .collect();
            for id in seen {
                self.chat_refresh(&id);
            }
        }
        // a recruit's first message, once its chat is free (a failure is said once by `chat_send`, not retried)
        let mut i = 0;
        while i < self.chat_starts.len() {
            let (id, prompt) = self.chat_starts[i].clone();
            if self.chat_runs.contains_key(&id) {
                i += 1;
                continue;
            }
            self.chat_starts.remove(i);
            if self.is_chat(&id) {
                self.chat_send(ctx, &id, &prompt);
            }
        }
        self.chat_asks_tick(ctx);
        self.recruit_popups_tick();
    }

    /// A double-click on the card, or its bar's button: the chat screen, on this chat.
    /// The card's chat in the mini chat (its own window), the canvas staying in view.
    pub(crate) fn open_mini_chat(&mut self, id: &str) {
        self.moving = false;
        self.chat_agent = Some(id.to_string());
        self.chat_open = false;
        self.mini_set(true);
    }

    pub(crate) fn open_chat_node(&mut self, index: usize) {
        let id = self.scene.elements[index].id.clone();
        self.moving = false;
        self.chat_on(&id);
    }

    /// What the chat's agent gets in its environment, as a terminal's shell does: the bridge, who it
    /// is by a token of its own (bridge.rs `authorize`; it lasts while the answer runs) and
    /// `magoagent` on its PATH. It is no terminal (an app started from a canvas terminal would hand
    /// that terminal's id down), and what it draws through the MCP is signed by the Maestro.
    pub(crate) fn chat_env(&self, id: &str) -> Vec<(String, String)> {
        let name = self.chats.iter().find(|c| c.id == id).map(|c| self.maestro_name(&c.name)).unwrap_or_default();
        let mut env =
            vec![("MAGOAGENT_CHAT_ID".to_string(), id.to_string()), ("MAGOAGENT_TERMINAL_ID".into(), String::new()), ("MAGOAGENT_TERMINAL_NAME".into(), name)];
        if let Some(bridge) = &self.bridge {
            env.push(("MAGOAGENT_BRIDGE".into(), bridge.address.clone()));
            env.push(("MAGOAGENT_TOKEN".into(), bridge.shared.lock().unwrap().terminal_token(id)));
        }
        env.extend(app_path());
        env
    }

    /// The last thing said in the chat, on one line: whose it is, and how it starts.
    pub(crate) fn chat_preview(&self, id: &str) -> Option<String> {
        let (who, text) = self.transcripts.get(id)?.items.iter().rev().find_map(|item| match item {
            // what another agent asked it is that agent's, not the person's (chat/ask.rs)
            ChatItem::User(text) => Some(crate::app::chat::ask::asked_by(text).unwrap_or(("Você", text))),
            ChatItem::Agent(text) => Some(("", text.as_str())),
            ChatItem::Error(text) => Some(("Erro", text.as_str())),
            _ => None,
        })?;
        // ponytail: the first 60 words, cut again by the card's rows; a longer look is the chat itself
        let words = text.split_whitespace().take(60).collect::<Vec<_>>().join(" ");
        Some(if who.is_empty() { words } else { format!("{who}: {words}") })
    }

    /// The card: a title strip like a terminal's (the agent's mark, "Maestro" with the crown of a
    /// Maestro terminal, the chat's name, a dot for its state), the agent (Claude's mascot, thinking
    /// while it answers) beside the state and the last message, and the folder strip.
    pub(crate) fn paint_chat_node(&self, painter: &Painter, element: &Element, rect: Rect, zoom: f32) {
        let Some(spec) = &element.chat else { return };
        let as_terminal = TerminalSpec { name: spec.name.clone(), command: spec.agent.clone(), ..Default::default() };
        let (look, _) = self.terminal_look(&as_terminal);
        let radius = (8.0 * zoom).min(255.0) as u8;
        let outline = if look == Look::default() { Color32::from_gray(0x3a) } else { look.fg.gamma_multiply(0.2) };
        painter.rect(rect, radius, look.bg, Stroke::new(1.0, outline), StrokeKind::Inside);
        let painter = painter.with_clip_rect(rect.shrink(1.0).intersect(painter.clip_rect()));
        let (strip, muted, light) = (look.strip(), look.muted(), look.fg);
        // the reference app's terminal strip: 8.125 in from the edge, 4.875 between its parts
        let (pad, gap) = (8.125 * zoom, 4.875 * zoom);
        let header = Rect::from_min_size(rect.min, Vec2::new(rect.width(), TERMINAL_HEADER * zoom));
        painter.rect_filled(header, egui::CornerRadius { nw: radius, ne: radius, sw: 0, se: 0 }, strip);
        painter.hline(header.x_range(), header.bottom() - 0.5, Stroke::new(1.0, look.rule()));
        let state = self.chat_state(&element.id).unwrap_or(AgentState::Idle(None));
        let working = matches!(state, AgentState::Working(_));
        let dot = if working { PRIMARY } else { Color32::from_rgb(0x2f, 0x9e, 0x44) };
        painter.circle_filled(Pos2::new(header.right() - pad - 3.25 * zoom, header.center().y), 3.25 * zoom, dot);
        let mut x = header.left() + pad;
        if zoom >= 0.6 {
            terminal_icon_scaled(&painter, &as_terminal, Pos2::new(x + 5.5 * zoom, header.center().y), light, zoom);
        }
        x += 11.0 * zoom + gap;
        // who it is on the canvas, then which chat
        let recruit = self.is_recruit_chat(&element.id);
        let who = if recruit { "Agente" } else { "Maestro" };
        let title = painter.text(Pos2::new(x, header.center().y), egui::Align2::LEFT_CENTER, who, FontId::proportional(9.75 * zoom), light);
        x = title.right() + gap;
        if zoom >= 0.6 && !recruit {
            maestro_crown(&painter, Pos2::new(x + 6.5 * zoom, header.center().y), zoom, AMBER);
            x += 13.0 * zoom + gap;
        }
        // room left for the mini chat's button (chat/node_ui.rs) before the dot, and a recruit's fold
        let buttons = if recruit { 50.0 } else { 30.0 };
        let name = clipped(&painter, spec.name.clone(), FontId::proportional(9.75 * zoom), muted, (header.right() - pad - buttons * zoom - x).max(0.0));
        painter.galley(Pos2::new(x, header.center().y - name.size().y / 2.0), name, muted);
        if recruit {
            // no folder strip: its work goes to the bottom (chat/recruit_card.rs), or its task when small or folded
            if spec.folded.is_some() || !crate::app::chat::node_ui::chat_live(rect, zoom) {
                self.paint_recruit_task(&painter, &element.id, Rect::from_min_max(header.left_bottom(), rect.max).shrink(pad), zoom, light, muted);
            }
            return;
        }
        let footer = Rect::from_min_max(Pos2::new(rect.left(), rect.bottom() - TERMINAL_FOOTER * zoom), rect.max);
        let body = Rect::from_min_max(header.left_bottom(), footer.right_top()).shrink(pad);
        if crate::app::chat::node_ui::chat_live(rect, zoom) {
            // the conversation itself goes there (chat_node_overlay)
            self.paint_chat_footer(&painter, footer, radius, strip, muted, pad, &spec.cwd, look.rule());
            return;
        }
        // the agent: Claude's mascot (idle, or what it does while it works), Codex's mark
        let figure = (body.height().min(64.0 * zoom)).max(0.0);
        let at = Pos2::new(body.left() + figure / 2.0, body.center().y);
        let time = self.now;
        // its squares under ~1 px blur into marks that read as letters: too small, no figure
        let painter = painter.with_clip_rect(body.intersect(painter.clip_rect()));
        if figure < MIN_FIGURE {
        } else if crate::app::mascot::poses::claude(&as_terminal) {
            let pose = match state {
                AgentState::Working(since) => crate::app::mascot::poses::working(None, "", time - since.min(time), since.to_bits()),
                _ => "artboard-24cla",
            };
            crate::app::mascot::poses::paint_pose(&painter, pose, at, figure, time, working, 1.0);
        } else {
            crate::ide::icons::paint_tint(&painter, crate::ide::icons::OPENAI, Rect::from_center_size(at, Vec2::splat(figure * 0.55)), light);
        }
        if working {
            // the mascot taps its feet, the time runs
            painter.ctx().request_repaint_after(std::time::Duration::from_millis(80));
        }
        // its state, then the last message as far as the card has room for
        let text_left = body.left() + if figure < MIN_FIGURE { 0.0 } else { figure + gap * 2.0 };
        let width = (body.right() - text_left).max(0.0);
        let waiting = self.chat_asks.iter().filter(|a| a.chat == element.id).count();
        let status = match state {
            AgentState::Working(since) => format!("Respondendo… {}", ago(time - since)),
            _ if waiting > 0 => "Pergunta na fila".to_string(),
            _ => "Pronto".to_string(),
        };
        let status = clipped(&painter, status, FontId::proportional(10.5 * zoom), if working { PRIMARY } else { muted }, width);
        let line = status.size().y;
        painter.galley(Pos2::new(text_left, body.top()), status, muted);
        let rows = ((body.height() - line - gap) / (12.5 * zoom)).floor();
        if rows >= 1.0 && width > 24.0 * zoom {
            let preview = self.chat_preview(&element.id);
            let ink = if preview.is_some() { light.gamma_multiply(0.85) } else { muted };
            let text = preview.unwrap_or_else(|| "Sem mensagens ainda. Clique duplo para abrir o chat.".into());
            let mut job = egui::text::LayoutJob::simple(text, FontId::proportional(10.5 * zoom), ink, width);
            job.wrap.max_rows = rows as usize;
            job.wrap.overflow_character = Some('…');
            painter.galley(Pos2::new(text_left, body.top() + line + gap), painter.layout_job(job), ink);
        }
        let painter = painter.with_clip_rect(rect.shrink(1.0).intersect(painter.clip_rect()));
        self.paint_chat_footer(&painter, footer, radius, strip, muted, pad, &spec.cwd, look.rule());
    }

    /// The folder strip: where its agent runs.
    #[allow(clippy::too_many_arguments)]
    fn paint_chat_footer(&self, painter: &Painter, footer: Rect, radius: u8, strip: Color32, muted: Color32, pad: f32, cwd: &str, rule: Color32) {
        let zoom = footer.height() / TERMINAL_FOOTER;
        painter.rect_filled(footer, egui::CornerRadius { nw: 0, ne: 0, sw: radius, se: radius }, strip);
        painter.hline(footer.x_range(), footer.top() + 0.5, Stroke::new(1.0, rule));
        let folder = clipped(painter, display_folder(cwd), FontId::monospace(10.0 * zoom), muted, footer.width() - 2.0 * pad);
        painter.galley(Pos2::new(footer.left() + pad, footer.center().y - folder.size().y / 2.0), folder, muted);
    }

    /// The toolbar's Chat, clicked on the canvas at `at`: a new chat as Ajustes presets it, its card
    /// there, selected; the tool goes back to Seleção.
    pub(crate) fn place_chat(&mut self, at: Pos2) {
        let before = self.chats.len();
        if self.start_chat(false)
            && let Some(chat) = self.chats.get(before).map(|c| c.id.clone())
            && let Some(i) = self.chat_node(&chat)
        {
            let from = self.scene.elements[i].center();
            self.scene.elements[i].move_by(at - from);
            self.selected = vec![i];
        }
        self.tool = Tool::Select;
    }

    /// Adicionar › Chat, in the canvas's menu: a new chat, or one of the list that has no card here.
    pub(crate) fn chat_add_entry(&self) -> Entry {
        let off: Vec<Entry> = self.chats.iter().filter(|c| self.chat_node(&c.id).is_none()).map(|c| item(&format!("chat_node:{}", c.id), &c.name)).collect();
        if off.is_empty() { item("add_chat", "Chat") } else { sub("Chat", [vec![item("add_chat", "Novo Chat"), Entry::Sep], off].concat()) }
    }

    /// The card's own menu: no copy of it (a second card of the same conversation), and Apagar
    /// says what it takes.
    pub(crate) fn chat_node_entries(&self, id: &str) -> Vec<Entry> {
        let locked = self.scene.elements.iter().any(|e| e.id == id && e.locked);
        let mut rows = vec![item("open_chat", "Abrir chat"), item("open_mini_chat", "Abrir no mini chat"), Entry::Sep];
        rows.extend(self.disconnect_rows(id).unwrap_or_default());
        rows.extend([
            item("lock", if locked { "Desbloquear" } else { "Bloquear" }),
            item("front", "Trazer para Frente"),
            item("back", "Enviar para Trás"),
            Entry::Sep,
            entry("delete", "Tirar do canvas", None, true, true),
        ]);
        rows
    }

    /// The rows above, run: true when `id` was one of them.
    pub(crate) fn chat_menu_action(&mut self, target: &Target, id: &str) -> bool {
        match (target, id) {
            (Target::Canvas(at), "add_chat") => {
                // as Ajustes presets it, with its card where the menu opened
                let before = self.chats.len();
                if self.start_chat(false)
                    && let Some(chat) = self.chats.get(before).map(|c| c.id.clone())
                    && let Some(i) = self.chat_node(&chat)
                {
                    let from = self.scene.elements[i].center();
                    self.scene.elements[i].move_by(*at - from);
                }
            }
            (Target::Canvas(at), id) if id.starts_with("chat_node:") => {
                if let Some(i) = self.add_chat_node(&id["chat_node:".len()..], Some(*at)) {
                    self.selected = vec![i];
                }
            }
            (Target::Element(node), "open_chat") => {
                if let Some(i) = self.chat_node(node) {
                    self.open_chat_node(i);
                }
            }
            (Target::Element(node), "open_mini_chat") => self.open_mini_chat(node),
            _ => return false,
        }
        true
    }

    /// The MCP's `chat_node`: the chat's card on the canvas shown (`show`), a new chat born with
    /// its card there (`new`), or the card taken off the canvas, the conversation kept (`remove`).
    pub(crate) fn act_chat_node(&mut self, args: &Value) -> Result<String, String> {
        let text = |key: &str| args[key].as_str().unwrap_or_default().trim().to_string();
        match text("action").as_str() {
            "" | "show" => {
                let id = self.chat_named(&text("chat"))?;
                self.show_chat_on_canvas(&id)
            }
            "new" => {
                // `chat_new` opens the chat's screen; here the canvas stays, on the new card
                self.act("chat_new", args)?;
                let id = self.chat_agent.clone().ok_or("o chat não foi criado")?;
                self.show_chat_on_canvas(&id)?;
                Ok(format!("Chat novo no canvas: {}.", self.agent_label(&id)))
            }
            "remove" => {
                let id = self.chat_named(&text("chat"))?;
                let name = self.agent_label(&id);
                let i = self.chat_node(&id).ok_or_else(|| format!("O chat {name} não tem cartão neste canvas."))?;
                self.selected = vec![i];
                self.delete_selected();
                Ok(format!("Cartão do chat {name} tirado do canvas (dá para desfazer); a conversa continua na lista de chats."))
            }
            "fold" => {
                let id = self.chat_named(&text("chat"))?;
                let name = self.agent_label(&id);
                if !self.is_recruit_chat(&id) {
                    return Err(format!("{name} não é um agente recrutado: só o cartão de um recruta se recolhe."));
                }
                match self.fold_chat_card(&id) {
                    Some(true) => Ok(format!("Cartão de {name} recolhido: só o nome e a tarefa.")),
                    Some(false) => Ok(format!("Cartão de {name} aberto: mostra o que ele faz.")),
                    None => Err(format!("O chat {name} não tem cartão neste canvas.")),
                }
            }
            other => Err(format!("action é show, new, remove ou fold, não {other}.")),
        }
    }

    /// The MCP's `disconnect`: the rope between two things on the canvas, cut (terminals, chats,
    /// notes, portals, IDEs, by name or id).
    pub(crate) fn act_disconnect(&mut self, a: &str, b: &str) -> Result<String, String> {
        let (first, second) = (self.node_named(a)?, self.node_named(b)?);
        let (first, second) = (self.scene.elements[first].id.clone(), self.scene.elements[second].id.clone());
        if !self.links().get(&first).is_some_and(|l| l.contains(&second)) {
            return Err(format!("{a} e {b} não estão conectados."));
        }
        self.unlink(&first, &second);
        // a locked rope stays (`unlink` says so)
        if self.links().get(&first).is_some_and(|l| l.contains(&second)) {
            return Err(format!("A conexão entre {a} e {b} está bloqueada."));
        }
        self.publish_nodes();
        Ok(format!("{a} e {b} desconectados (dá para desfazer)."))
    }
}
