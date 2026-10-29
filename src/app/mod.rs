//! The window: `DrawingApp` state and the per-frame loop.

use crate::prelude::*;

pub(crate) mod tool;
pub(crate) mod ui;
mod arrows;
mod editing;
mod files;
mod maestro;
mod notes;
mod paint;
mod pointer;
mod selection;
mod shortcuts;
pub(crate) mod chat;
pub(crate) mod chat_images;
pub(crate) mod chat_viewer;
pub(crate) mod ide;
#[cfg_attr(target_os = "linux", allow(dead_code))] // Design runs in portals, which Linux doesn't open yet
pub(crate) mod design;
pub(crate) mod portal;
pub(crate) mod script;
pub(crate) mod spaces;
pub(crate) mod floors;
pub(crate) mod terminal;
#[cfg(test)]
mod tests;

/// Resize gesture in the selection's own frame (rotated for a single rotated element):
/// the opposite corner stays fixed, elements are rescaled from their originals.
pub(crate) struct Resize {
    pub(crate) anchor: Pos2,
    pub(crate) grab: Vec2,
    pub(crate) from: Rect,
    pub(crate) center: Pos2,
    pub(crate) angle: f32,
    pub(crate) originals: Vec<Element>,
    /// 1 on the axes the pointer moves, 0 on the axis an edge grip keeps (`pin`, the grabbed corner)
    pub(crate) free: Vec2,
    pub(crate) pin: Pos2,
}

pub(crate) struct Rotate {
    pub(crate) pivot: Pos2,
    pub(crate) start: f32,
    pub(crate) originals: Vec<Element>,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) enum Layer {
    Backward,
    Forward,
    Back,
    Front,
}

pub(crate) struct DrawingApp {
    pub(crate) scene: Scene,
    pub(crate) undo: Vec<Scene>,
    pub(crate) redo: Vec<Scene>,
    pub(crate) tool: Tool,
    pub(crate) tool_locked: bool,
    pub(crate) selected: Vec<usize>,
    pub(crate) style: Style,
    pub(crate) drawing_start: Option<Pos2>,
    pub(crate) drawing_points: Vec<[f32; 2]>,
    pub(crate) moving: bool,
    pub(crate) move_snapshotted: bool,
    pub(crate) last_pointer: Option<Pos2>,
    pub(crate) mods: Modifiers,
    pub(crate) file_path: Option<PathBuf>,
    pub(crate) status: String,
    pub(crate) status_time: f64,
    pub(crate) zoom: f32,
    pub(crate) pan: Vec2,
    pub(crate) canvas_rect: Rect,
    pub(crate) marquee: Option<Pos2>,
    pub(crate) resizing: Option<Resize>,
    pub(crate) rotating: Option<Rotate>,
    pub(crate) line_editor: Option<usize>,
    pub(crate) point_drag: Option<usize>,
    /// A press on a line's midpoint handle: the point it adds (index, press position) comes with the drag.
    pub(crate) midpoint_press: Option<(usize, Pos2)>,
    pub(crate) selected_point: Option<usize>,
    pub(crate) multi: Option<usize>,
    pub(crate) arrow_curved: bool,
    pub(crate) heads: (Option<Head>, Option<Head>),
    pub(crate) head_picker: Option<bool>,
    pub(crate) head_picker_more: bool,
    pub(crate) panning: bool,
    pub(crate) erasing: Option<Vec<usize>>,
    pub(crate) editing: Option<usize>,
    pub(crate) edit_focused: bool,
    /// The note text's last selection (label index, from, to) while its editor had the keys: a
    /// click on the note bar takes them away before the button acts.
    pub(crate) note_selection: Option<(usize, usize, usize)>,
    pub(crate) clipboard: Vec<Element>,
    pub(crate) menu_open: bool,
    pub(crate) export_open: bool,
    pub(crate) export_background: bool,
    pub(crate) export_scale: f32,
    /// SVG, not PNG: the last format picked.
    pub(crate) export_svg: bool,
    pub(crate) system_dark: bool,
    pub(crate) palette_open: bool,
    pub(crate) palette_query: String,
    pub(crate) palette_index: usize,
    pub(crate) palette_focused: bool,
    pub(crate) extra_open: bool,
    /// The toolbar's drawing row, opened by its last button.
    pub(crate) draw_open: bool,
    pub(crate) now: f64,
    pub(crate) laser: Vec<(Pos2, f64)>,
    pub(crate) laser_down: bool,
    pub(crate) lasso: Option<Vec<Pos2>>,
    /// Ids of the locked element (or whole group / card) last clicked: shows the unlock button.
    pub(crate) active_locked: Option<Vec<String>>,
    /// Elements an AI just added, drawn one after another (front one since `reveal_since`).
    pub(crate) reveal: std::collections::VecDeque<String>,
    pub(crate) reveal_since: f64,
    /// Who is drawing, and where its cursor last stopped (world) and when.
    pub(crate) agent: Option<(String, Pos2, f64)>,
    /// Where the agent's cursor was drawn last (world) and when: it glides instead of jumping.
    pub(crate) agent_drawn: std::cell::Cell<Option<(Pos2, f64)>>,
    pub(crate) prefs: Prefs,
    pub(crate) saved_prefs: Prefs,
    pub(crate) prefs_open: bool,
    /// Ajustes (⌘,), and the quick start being edited in it.
    pub(crate) settings_open: bool,
    pub(crate) settings_edit: Option<usize>,
    pub(crate) locked_open: bool,
    pub(crate) press_world: Pos2,
    pub(crate) move_origin: Option<(Vec<usize>, Vec<Element>, Rect)>,
    pub(crate) snap_guides: Vec<[Pos2; 2]>,
    pub(crate) style_edit_active: bool,
    pub(crate) dirty: bool,
    /// When `dirty` last went from false to true (`ui`'s clock): the autosave waits 0.4 s after it.
    pub(crate) dirty_since: f64,
    /// When the Design overlay of a page that can't send by ipc was last asked for its messages.
    pub(crate) design_polled: f64,
    /// The camera on its way to a node: when it set off, from where, to where (pans).
    pub(crate) glide: Option<(f64, Vec2, Vec2)>,
    /// Prompts left half-written when their terminal's workspace was hidden, by terminal id.
    pub(crate) drafts: std::collections::HashMap<String, String>,
    /// The selection Organizar last arranged and the layout it used: again cycles the layout.
    pub(crate) tidy_last: Option<(Vec<String>, usize)>,
    /// An agent's `notify` off the Mac: the next frame asks the taskbar for a look.
    pub(crate) attention: bool,
    /// When the hidden window last asked to paint (`logic`).
    pub(crate) hidden_paint: Option<std::time::Instant>,
    pub(crate) autosave: Option<PathBuf>,
    /// Scene as last read from / written to the shared file, for merging outside edits.
    pub(crate) synced: Scene,
    pub(crate) synced_stamp: Option<SystemTime>,
    pub(crate) last_poll: f64,
    /// Running shells by terminal element id (outside the scene, so undo never copies them).
    pub(crate) terminals: std::collections::HashMap<String, Result<TerminalSession, String>>,
    /// The terminal being typed into.
    pub(crate) terminal_focus: Option<String>,
    pub(crate) terminal_scroll: f32,
    /// The terminal enlarged over the canvas (double-click on its title strip).
    pub(crate) terminal_elevated: Option<String>,
    /// Agent terminals printing since (app time), for the "terminou" toast.
    pub(crate) terminal_busy: std::collections::HashMap<String, f64>,
    /// When each agent went quiet, and the ones that finished while you looked elsewhere.
    pub(crate) terminal_idle: std::collections::HashMap<String, f64>,
    pub(crate) terminal_unread: std::collections::HashSet<String>,
    /// The agents panel (⌘J) and where it sat last frame, so pages step aside from it.
    pub(crate) agents_open: bool,
    /// Agents stopped on a question in their terminal, read twice a second (agents.rs).
    pub(crate) terminal_waiting: std::collections::HashSet<String>,
    pub(crate) waiting_checked: f64,
    /// The Claude Code session each terminal runs, read with `terminal_waiting`.
    pub(crate) agent_sessions: std::collections::HashMap<String, crate::transcript::Session>,
    /// Terminals that ran one, by the start of their shell: a session gone means its claude left.
    pub(crate) claude_seen: std::collections::HashMap<String, i64>,
    /// What the agents' states and the bridge proved, newest last (the last 200), and each
    /// terminal's state at the last check.
    pub(crate) agent_events: std::collections::VecDeque<crate::app::ui::agents::AgentEvent>,
    pub(crate) agent_seen: std::collections::HashMap<String, crate::app::ui::agents::AgentState>,
    /// The IDE each terminal's Claude Code was given when its shell started, and whether the
    /// person was told its rope to it is undone.
    pub(crate) terminal_ides: std::collections::HashMap<String, (String, bool)>,
    /// The chat screen (⌘1): whose conversation, the box's text, what was read so far.
    pub(crate) chat_open: bool,
    pub(crate) chat_agent: Option<String>,
    pub(crate) chat_draft: String,
    pub(crate) transcripts: std::collections::HashMap<String, crate::transcript::Transcript>,
    pub(crate) chat_polled: f64,
    /// The conversation's thumbnails, by `image_key`.
    pub(crate) chat_images: std::cell::RefCell<std::collections::HashMap<(usize, String), chat_images::Decoding>>,
    /// The picture open over the chat and the ones waiting in its box.
    pub(crate) chat_pics: std::cell::RefCell<chat_images::ChatPics>,
    /// Where each note's text ended when last painted (its own points, y): a click below it only selects.
    pub(crate) note_text_bottom: std::cell::RefCell<std::collections::HashMap<String, f32>>,
    /// The note text a press landed on: its release, if it didn't move, types into it.
    pub(crate) note_press: Option<usize>,
    /// The agent's live terminal shown above its conversation ("Terminal" in the chat's header).
    pub(crate) chat_terminal: bool,
    /// The box's height last frame: the conversation above it takes the rest.
    pub(crate) chat_composer: f32,
    pub(crate) terminal_dialog: Option<TerminalDialog>,
    pub(crate) portal_dialog: Option<PortalDialog>,
    /// Live editors by IDE element id (outside the scene, like the shells), the one that has
    /// the keyboard (the canvas's shortcuts are off meanwhile), and the new / edit IDE dialog.
    pub(crate) ides: std::collections::HashMap<String, crate::ide::Ide>,
    pub(crate) ide_focus: Option<String>,
    /// The IDE card on the canvas whose tree has the keys (arrows, Enter, Esc) and shows hover.
    pub(crate) card_focus: Option<String>,
    pub(crate) ide_dialog: Option<IdeDialog>,
    /// When the enlarged terminal started growing, and the one shrinking back (with when).
    pub(crate) elevate_since: f64,
    pub(crate) leaving: Option<(String, f64)>,
    /// The action bar button that was just clicked, and when: its icon hops.
    pub(crate) hop: Option<(&'static str, f64)>,
    /// Each rope's simulated chain: it swings behind moved ends and settles under its weight.
    pub(crate) ropes: std::collections::HashMap<String, RopeSim>,
    /// Live pages by portal element id, and what they report back.
    #[cfg(not(target_os = "linux"))]
    pub(crate) portals: std::collections::HashMap<String, PortalView>,
    #[cfg(not(target_os = "linux"))]
    pub(crate) portal_events: (std::sync::mpsc::Sender<PortalEvent>, std::sync::mpsc::Receiver<PortalEvent>),
    /// The address being typed in a portal's bar (portal id, text, whether to take focus).
    pub(crate) portal_url_edit: Option<(String, String, bool)>,
    /// The portal Design is armed on, if any (design.rs).
    pub(crate) design: Option<design::DesignSession>,
    /// Each portal's kept annotations, numbered in the order they were kept.
    pub(crate) annotations: std::collections::HashMap<String, Vec<design::Annotation>>,
    /// A Design send waiting for the app's own Enviar (design.rs).
    pub(crate) design_confirm: Option<design::DesignConfirm>,
    /// Bottom of the toolbars at the top of the window, last frame.
    pub(crate) toolbar_bottom: f32,
    /// The prompt composer under a terminal: its id and the message being written.
    pub(crate) composer: Option<(String, String)>,
    /// "Conectar": the element the next click connects to.
    pub(crate) connecting: Option<String>,
    /// Where agents in the terminals reach the app (None in tests).
    pub(crate) bridge: Option<Bridge>,
    /// MAGOAGENT_SCRIPT: scripted mouse and keyboard for visual checks (app/script.rs).
    pub(crate) script: Option<Script>,
    /// MAGOAGENT_PERF: frames and their CPU, measured (app/script.rs).
    pub(crate) perf: Option<script::Perf>,
    /// The right-click menu open now (ui/context.rs).
    pub(crate) context: Option<ui::context::ContextMenu>,
    /// Renomear: the element being renamed and the name typed so far.
    pub(crate) rename: Option<(String, String)>,
    /// The note bar's color popover: where it opened, and its panel's rect last frame.
    pub(crate) note_color: Option<(Pos2, Rect)>,
    /// The connections popover: a node's ropes (its badge) or the ropes under a click (ui/links.rs).
    pub(crate) links_popover: Option<ui::links::LinksPopover>,
    /// The palette's node search, while "Conectar a…" or "Desconectar…" waits for a node.
    pub(crate) palette_links: Option<ui::links::LinkAction>,
    /// The workspaces, the main drawing they sit beside (None in tests: no files), the ones left
    /// this session, and their list and dialog (spaces.rs).
    pub(crate) spaces: Spaces,
    pub(crate) spaces_home: Option<PathBuf>,
    pub(crate) parked: std::collections::HashMap<String, spaces::Parked>,
    pub(crate) spaces_open: bool,
    pub(crate) space_rename: Option<(String, String)>,
    pub(crate) space_dialog: Option<spaces::SpaceDialog>,
    /// What the git threads of the floors report (floors.rs).
    pub(crate) space_events: (std::sync::mpsc::Sender<floors::SpaceJob>, std::sync::mpsc::Receiver<floors::SpaceJob>),
}

impl Default for DrawingApp {
    fn default() -> Self {
        Self {
            scene: Scene::default(),
            undo: Vec::new(),
            redo: Vec::new(),
            tool: Tool::Select,
            tool_locked: false,
            selected: Vec::new(),
            style: Style::default(),
            drawing_start: None,
            drawing_points: Vec::new(),
            moving: false,
            move_snapshotted: false,
            last_pointer: None,
            mods: Modifiers::NONE,
            file_path: None,
            status: String::new(),
            status_time: 0.0,
            zoom: 1.0,
            pan: Vec2::ZERO,
            canvas_rect: Rect::NOTHING,
            marquee: None,
            resizing: None,
            rotating: None,
            line_editor: None,
            point_drag: None,
            midpoint_press: None,
            selected_point: None,
            multi: None,
            arrow_curved: true,
            heads: (None, Some(Head::Arrow)),
            head_picker: None,
            head_picker_more: false,
            panning: false,
            erasing: None,
            editing: None,
            edit_focused: false,
            note_selection: None,
            clipboard: Vec::new(),
            menu_open: false,
            export_open: false,
            export_background: true,
            export_scale: 2.0,
            export_svg: false,
            system_dark: false,
            palette_open: false,
            palette_query: String::new(),
            palette_index: 0,
            palette_focused: false,
            extra_open: false,
            draw_open: false,
            now: 0.0,
            laser: Vec::new(),
            laser_down: false,
            lasso: None,
            active_locked: None,
            reveal: Default::default(),
            reveal_since: 0.0,
            agent: None,
            agent_drawn: Default::default(),
            prefs: Prefs::default(),
            saved_prefs: Prefs::default(),
            prefs_open: false,
            settings_open: false,
            settings_edit: None,
            locked_open: false,
            press_world: Pos2::ZERO,
            move_origin: None,
            snap_guides: Vec::new(),
            style_edit_active: false,
            dirty: false,
            dirty_since: 0.0,
            design_polled: 0.0,
            glide: None,
            drafts: Default::default(),
            tidy_last: None,
            hidden_paint: None,
            attention: false,
            autosave: None,
            synced: Scene::default(),
            synced_stamp: None,
            last_poll: 0.0,
            terminals: Default::default(),
            terminal_focus: None,
            terminal_scroll: 0.0,
            terminal_elevated: None,
            terminal_busy: Default::default(),
            terminal_idle: Default::default(),
            terminal_unread: Default::default(),
            agents_open: false,
            terminal_waiting: Default::default(),
            waiting_checked: 0.0,
            agent_sessions: Default::default(),
            claude_seen: Default::default(),
            agent_events: Default::default(),
            agent_seen: Default::default(),
            terminal_ides: Default::default(),
            chat_open: false,
            chat_agent: None,
            chat_draft: String::new(),
            transcripts: Default::default(),
            chat_polled: 0.0,
            chat_images: Default::default(),
            chat_pics: Default::default(),
            note_text_bottom: Default::default(),
            note_press: None,
            chat_terminal: false,
            chat_composer: 0.0,
            terminal_dialog: None,
            connecting: None,
            composer: None,
            toolbar_bottom: 0.0,
            portal_dialog: None,
            ides: Default::default(),
            ide_focus: None,
            card_focus: None,
            ide_dialog: None,
            elevate_since: 0.0,
            leaving: None,
            hop: None,
            ropes: Default::default(),
            #[cfg(not(target_os = "linux"))]
            portals: Default::default(),
            #[cfg(not(target_os = "linux"))]
            portal_events: std::sync::mpsc::channel(),
            portal_url_edit: None,
            design: None,
            annotations: Default::default(),
            design_confirm: None,
            bridge: None,
            script: None,
            perf: None,
            context: None,
            rename: None,
            note_color: None,
            links_popover: None,
            palette_links: None,
            spaces: Spaces::default(),
            spaces_home: None,
            parked: Default::default(),
            spaces_open: false,
            space_rename: None,
            space_dialog: None,
            space_events: std::sync::mpsc::channel(),
        }
    }
}

impl eframe::App for DrawingApp {
    /// Closing mid-edit keeps the text being typed and anything not yet autosaved (that only runs
    /// while idle).
    fn on_exit(&mut self, _gl: Option<&eframe::glow::Context>) {
        self.save_on_exit();
        self.save_parked();
        self.keep_spaces();
    }

    /// Minimized (Windows) or covered (macOS), eframe stops calling `ui`, where the bridge, the
    /// IDE's Claude Code calls and the agents' states run: it paints unseen every 400 ms.
    fn logic(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        const EVERY: std::time::Duration = std::time::Duration::from_millis(400);
        if ctx.input(|i| i.viewport().visible()) != Some(false) {
            self.hidden_paint = None;
            return;
        }
        // every request reaches here while hidden: only one in 400 ms paints. The input's time
        // stands still meanwhile, so the clock is the system's
        let since = self.hidden_paint.map_or(EVERY, |t| t.elapsed());
        if since >= EVERY {
            self.hidden_paint = Some(std::time::Instant::now());
            ctx.send_viewport_cmd(egui::ViewportCommand::RequestPaintWhileHidden);
            ctx.request_repaint_after(EVERY);
        } else {
            ctx.request_repaint_after(EVERY - since);
        }
    }

    fn raw_input_hook(&mut self, ctx: &egui::Context, raw: &mut egui::RawInput) {
        if let Some(script) = &mut self.script {
            script.feed(ctx, raw);
        }
        // Tab on the bare canvas moves no focus: egui would hand the keys to the first button, and
        // the shortcuts and Space would stop until the next click
        if !self.keyboard_taken(ctx) {
            raw.events.retain(|e| !matches!(e, egui::Event::Key { key: Key::Tab, .. }));
        }
    }

    fn ui(&mut self, root: &mut egui::Ui, frame: &mut eframe::Frame) {
        let ctx = root.ctx().clone();
        if let Some(perf) = &mut self.perf {
            perf.frame(ctx.input(|i| i.time), frame.info().cpu_usage, &ctx.repaint_causes());
        }
        if let Some(theme) = ctx.system_theme() {
            self.system_dark = theme == egui::Theme::Dark;
        }
        configure_visuals(&ctx, self.dark());
        #[cfg(target_os = "macos")]
        crate::place_window_buttons((TOP_ROW + 22.0) as f64);
        self.screen_keys(&ctx);
        self.handle_shortcuts(&ctx);
        // before the terminals: a roped terminal that starts gets its IDE's Claude Code port
        self.sync_ides(&ctx);
        self.sync_terminals(&ctx);
        self.check_agents();
        if self.swing_ropes(ctx.input(|i| i.stable_dt).min(0.05)) {
            ctx.request_repaint();
        }
        egui::CentralPanel::no_frame().show(root, |ui| {
            // not focusable: Tab would park the keyboard on the canvas and turn the shortcuts and Space off
            let (response, painter) = ui.allocate_painter(ui.available_size(), Sense::CLICK | Sense::DRAG);
            let rect = response.rect;
            self.handle_canvas(&response, rect);
            if response.hovered() && !self.chat_open && ctx.current_pass_index() == 0 {
                let (scroll, zoom, pointer) = ctx.input(|i| (i.smooth_scroll_delta, i.zoom_delta(), i.pointer.hover_pos()));
                let shift = ctx.input(|i| i.modifiers.shift);
                // over a card the wheel scrolls its tree or panel, not the canvas
                let on_card = match pointer {
                    Some(p) if scroll.y != 0.0 && zoom == 1.0 => self.ide_wheel(self.to_world(p), scroll.y),
                    _ => false,
                };
                if on_card {
                } else if zoom != 1.0 {
                    self.wheel_zoom(zoom, pointer.unwrap_or(rect.center()));
                } else if scroll != Vec2::ZERO && self.prefs.mouse_wheel_zooms && !shift {
                    // Dispositivo de entrada = Mouse: the wheel zooms
                    self.wheel_zoom(1.0 + scroll.y * 0.002, pointer.unwrap_or(rect.center()));
                } else if scroll != Vec2::ZERO {
                    self.glide = None;
                    self.pan += scroll;
                }
            }
            self.update_cursor(&ctx, &response);
            self.paint_scene(&painter, rect);
        });
        self.text_editor(&ctx);
        self.floating_controls(&ctx);
        self.note_color_ui(&ctx);
        self.links_popover_ui(&ctx);
        self.context_ui(&ctx);
        self.rename_ui(&ctx);
        if self.chat_open {
            self.chat_screen(&ctx);
        } else {
            self.terminal_overlay(&ctx);
            self.ide_overlay(&ctx);
            self.portal_chrome(&ctx);
        }
        #[cfg(not(target_os = "linux"))]
        self.sync_portals(&*frame, &ctx);
        #[cfg(target_os = "macos")]
        if let Some(path) = self.script.as_mut().and_then(Script::take_pages) {
            self.save_page_shots(&path);
        }
        let _ = frame;
        let idle = !self.in_gesture() && self.multi.is_none() && self.editing.is_none();
        self.prefs.tool_locked = self.tool_locked;
        if self.prefs != self.saved_prefs {
            self.saved_prefs = self.prefs.clone();
            if let (Some(path), true) = (prefs_path(), self.autosave.is_some()) {
                let _ = serde_json::to_string_pretty(&self.prefs).map(|json| write_atomic(&path, json));
            }
        }
        let now = ctx.input(|i| i.time);
        self.laser.retain(|(_, t)| now - *t < 1.0);
        if let Some(id) = self.reveal.front() {
            match self.scene.elements.iter().find(|e| &e.id == id) {
                Some(e) if self.reveal_progress(e) < 1.0 => {}
                done => {
                    if let (Some(e), Some(agent)) = (done, &mut self.agent) {
                        (agent.1, agent.2) = (reveal_pen(e), now);
                    }
                    self.reveal.pop_front();
                    self.reveal_since = now;
                }
            }
            ctx.request_repaint();
        } else if self.agent.as_ref().is_some_and(|a| now - a.2 < AGENT_LINGER) {
            ctx.request_repaint();
        }
        if !self.laser.is_empty() {
            ctx.request_repaint();
        }
        if idle && now - self.last_poll > 0.4 {
            self.last_poll = now;
            self.sync_from_file();
        }
        // changes are written 0.4 s after the first one not yet on disk: a burst (arrow nudges,
        // typing) goes in a write every 0.4 s, not one per change, and a long one is never all lost
        if !self.dirty {
            self.dirty_since = now;
        }
        if self.dirty && idle && now - self.dirty_since >= 0.4 {
            self.write_autosave();
        }
        if self.glide.is_some() {
            ctx.request_repaint();
        }
        if std::mem::take(&mut self.attention) {
            ctx.send_viewport_cmd(egui::ViewportCommand::RequestUserAttention(egui::UserAttentionType::Informational));
        }
        ctx.request_repaint_after(std::time::Duration::from_millis(400));
    }
}

impl DrawingApp {
    pub(crate) fn toast(&mut self, message: impl Into<String>) {
        self.status = message.into();
        self.status_time = -1.0;
    }

    pub(crate) fn snapshot(&mut self) {
        self.undo.push(self.scene.clone());
        if self.undo.len() > 100 {
            self.undo.remove(0);
        }
        self.redo.clear();
        self.dirty = true;
    }

    /// A press still held on the canvas: the scene and the selection's indices are mid-change, so
    /// the file merge, the loose-rope cleanup and the shortcuts wait for the release.
    pub(crate) fn in_gesture(&self) -> bool {
        self.moving
            || self.panning
            || self.resizing.is_some()
            || self.rotating.is_some()
            || self.point_drag.is_some()
            || self.midpoint_press.is_some()
            || self.drawing_start.is_some()
            || self.erasing.is_some()
            || self.marquee.is_some()
            || self.lasso.is_some()
            || self.note_press.is_some()
    }

    /// Esc mid-drag: what the drag changed goes back to before the press, and the drag ends.
    pub(crate) fn cancel_gesture(&mut self) {
        // these took a snapshot when they began changing the scene
        let changed = self.move_snapshotted || self.resizing.is_some() || self.rotating.is_some() || self.point_drag.is_some();
        // only then: a marquee or a pan took none, and the last step must stay
        if let Some(previous) = changed.then(|| self.undo.pop()).flatten() {
            self.scene = previous;
            self.dirty = true;
        }
        self.moving = false;
        self.move_snapshotted = false;
        self.move_origin = None;
        self.panning = false;
        self.resizing = None;
        self.rotating = None;
        self.point_drag = None;
        self.midpoint_press = None;
        self.drawing_start = None;
        self.drawing_points.clear();
        self.erasing = None;
        self.marquee = None;
        self.lasso = None;
        self.snap_guides.clear();
        self.note_press = None;
    }

    pub(crate) fn undo(&mut self) {
        if let Some(previous) = self.undo.pop() {
            self.redo.push(std::mem::replace(&mut self.scene, previous));
            self.selected.clear();
            self.dirty = true;
        }
    }

    pub(crate) fn redo(&mut self) {
        if let Some(next) = self.redo.pop() {
            self.undo.push(std::mem::replace(&mut self.scene, next));
            self.selected.clear();
            self.dirty = true;
        }
    }

    pub(crate) fn new_element(&self, kind: ElementKind) -> Element {
        let mut style = self.style;
        if matches!(kind, ElementKind::Line { .. } | ElementKind::FreeDraw { .. }) {
            // arrows default to curved, lines and freehand to sharp (reference defaults)
            style.round = matches!(kind, ElementKind::Line { arrow: true, .. }) && self.arrow_curved;
            style.fill = None;
        }
        Element {
            id: new_id(),
            group_id: None,
            kind,
            angle: 0.0,
            ends: ArrowEnds { start_head: self.heads.0, end_head: self.heads.1, ..Default::default() },
            locked: false,
            frame: None,
            image: None,
            created: None,
            terminal: None,
            portal: None,
            ide: None,
            note: NoteSpec::default(),
            rope: false,
            style,
        }
    }

    /// A sticky note over `rect` and its label `<id>-label`, grouped, the label at its center.
    pub(crate) fn new_note(&self, rect: Rect, text: String) -> [Element; 2] {
        let mut note = self.new_element(ElementKind::Rectangle { x: rect.min.x, y: rect.min.y, width: rect.width(), height: rect.height() });
        note.style = sticky_style(note.style);
        note.created = Some(now_ms());
        note.group_id = Some(new_id());
        let mut label = self.new_element(ElementKind::Text { x: rect.center().x, y: rect.center().y, value: text });
        label.id = format!("{}-label", note.id);
        label.group_id = note.group_id.clone();
        label.style.stroke = note.style.stroke;
        [note, label]
    }

    pub(crate) fn origin(&self) -> Vec2 {
        self.canvas_rect.min.to_vec2() + self.pan
    }

    pub(crate) fn to_world(&self, screen: Pos2) -> Pos2 {
        (screen - self.origin()) / self.zoom
    }

    pub(crate) fn to_screen(&self, world: Pos2) -> Pos2 {
        world * self.zoom + self.origin()
    }
}
