//! The window: `DrawingApp` state and the per-frame loop.

use crate::prelude::*;

mod actions;
mod arrows;
pub(crate) mod avatar;
mod binder;
pub(crate) mod chat;
pub(crate) mod chat_direct;
pub(crate) mod chat_footer;
pub(crate) mod chat_images;
pub(crate) mod chat_list;
pub(crate) mod chat_markdown;
pub(crate) mod chat_search;
pub(crate) mod chat_steps;
pub(crate) mod chat_tools;
pub(crate) mod chat_viewer;
mod demands;
#[cfg_attr(target_os = "linux", allow(dead_code))] // Design runs in portals, which Linux doesn't open yet
pub(crate) mod design;
mod dock;
mod drops;
mod editing;
pub(crate) mod environments;
mod files;
pub(crate) mod filetree;
mod filetree_code;
mod filetree_db;
mod filetree_editor;
mod filetree_find;
mod filetree_input;
mod filetree_menu;
mod filetree_ops;
mod filetree_pane;
mod filetree_tabs;
pub(crate) mod filetree_ui;
mod filetree_view;
mod filetree_vision;
pub(crate) mod floorfx;
pub(crate) mod floors;
pub(crate) mod ide;
mod maestro;
pub(crate) mod mascot;
pub(crate) mod memory;
pub(crate) mod minimap;
pub(crate) mod note_buttons;
pub(crate) mod notes;
mod paint;
mod pointer;
pub(crate) mod portal;
pub(crate) mod portal_console;
pub(crate) mod portal_extras;
mod portal_shot;
pub(crate) mod script;
mod selection;
mod shortcuts;
pub(crate) mod skills;
pub(crate) mod space_groups;
mod tasks;
pub(crate) mod terminal;
mod terminal_actions;
mod terminal_card;
mod terminal_find;
pub(crate) mod terminal_menu;
pub(crate) mod terminal_prefs;
mod terminal_procs;
#[cfg(test)]
mod tests;
mod timeline;
pub(crate) mod tool;
pub(crate) mod ui;
pub(crate) mod usage;
pub(crate) mod usage_probe;
pub(crate) mod voice;
pub(crate) mod voice_orb;
#[cfg(not(target_os = "linux"))]
pub(crate) mod voice_page;
pub(crate) mod voice_popup;
pub(crate) mod workspaces;

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
    /// The IDE card whose bar between the tree and the code is being dragged.
    pub(crate) card_split: Option<usize>,
    /// Whether the fonts in use join the code's ligatures (`Prefs::ligatures` once they're rebuilt).
    pub(crate) ligatures_shown: bool,
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
    /// The shape an arrow's tip reached and when (`now`): its outline pops out once from then.
    pub(crate) bind_pulse: std::cell::Cell<Option<(String, f64)>>,
    pub(crate) prefs: Prefs,
    pub(crate) saved_prefs: Prefs,
    /// Ajustes (⌘,), and the quick start being edited in it.
    pub(crate) settings_open: bool,
    pub(crate) settings_edit: Option<usize>,
    /// The section open in Ajustes (settings.rs, `SETTINGS_TABS`).
    pub(crate) settings_tab: usize,
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
    /// The minimap's held area and focus.
    pub(crate) minimap_area: minimap::MinimapArea,
    /// Prompts left half-written when their terminal's workspace was hidden, by terminal id.
    pub(crate) drafts: std::collections::HashMap<String, String>,
    /// The selection Organizar last arranged and the layout it used: again cycles the layout.
    pub(crate) tidy_last: Option<(Vec<String>, usize)>,
    /// An agent's `notify` off the Mac: the next frame asks the taskbar for a look.
    pub(crate) attention: bool,
    /// When the hidden window last asked to paint (`logic`).
    pub(crate) hidden_paint: Option<std::time::Instant>,
    /// The window's caption is the system's (ui/titlebar.rs::native_caption), once hooked.
    pub(crate) caption_hooked: bool,
    /// The chat in a window of its own (the call popup's pencil).
    pub(crate) chat_popup: bool,
    /// The last press on the title bar's empty stretch, for a second one to maximize.
    pub(crate) caption_press: Option<(f64, Pos2)>,
    /// The top-left buttons as the title bar last drew them: their presses are not the window's drag.
    pub(crate) title_buttons: Rect,
    /// What the sidebar's Filtrar holds: the spaces listed are the ones with it in their name.
    pub(crate) space_filter: String,
    /// The sidebar on its way open or folded: since when, from which width, toward which
    /// (negative before the first frame).
    pub(crate) sidebar_slide: (f64, f32, f32),
    /// The bottom right row's pills (ui/usage_pill.rs, ui/floors.rs).
    pub(crate) usage_ui: crate::app::ui::usage_pill::UsagePill,
    pub(crate) floors_ui: crate::app::ui::floors::FloorsUi,
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
    /// What the terminal cards keep between frames: the search bar, the processes popover, a drag selecting.
    pub(crate) term_ui: terminal_menu::TermUi,
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
    /// Finds those sessions off this thread (transcript/watch.rs).
    pub(crate) session_watch: crate::transcript::watch::SessionWatch,
    /// Terminals that ran one, by the start of their shell: a session gone means its claude left.
    pub(crate) claude_seen: std::collections::HashMap<String, i64>,
    /// What the agents' states and the bridge proved, newest last (the last 200), and each
    /// terminal's state at the last check.
    pub(crate) agent_events: std::collections::VecDeque<crate::app::ui::agents::AgentEvent>,
    pub(crate) agent_seen: std::collections::HashMap<String, crate::app::ui::agents::AgentState>,
    /// How much of the plan's limits is used and what each agent spends, as their sessions tell (usage.rs).
    pub(crate) usage: usage::Usage,
    /// Messages on their way along a rope (who sent, who got it, when, app seconds): its signal (paint.rs).
    pub(crate) rope_pulses: Vec<(String, String, f64)>,
    /// The events' database and the hooks' permission requests waiting for an answer (demands.rs).
    pub(crate) hooks: Hooks,
    /// The IDE each terminal's Claude Code was given when its shell started, and whether the
    /// person was told its rope to it is undone.
    pub(crate) terminal_ides: std::collections::HashMap<String, (String, bool)>,
    /// The chat screen (⌘1): whose conversation, the box's text, what was read so far.
    pub(crate) chat_open: bool,
    pub(crate) chat_agent: Option<String>,
    pub(crate) chat_draft: String,
    /// The agents installed here and when that was looked up (`installed_agents`).
    pub(crate) chat_installed: Option<(f64, Vec<Preset>)>,
    /// The direct chats (chat_direct.rs), the answers being written, and the form of a new one.
    pub(crate) chats: Vec<chat_direct::Chat>,
    pub(crate) chat_runs: std::collections::HashMap<String, chat_direct::ChatRun>,
    /// Screenshots the Maestro asked for, and the IDE whose Commit panel a review opens (chat_tools.rs).
    pub(crate) shots: Vec<chat_tools::Shot>,
    pub(crate) review_pending: Option<String>,
    /// An agent's file waiting for its new IDE's editor, and window commands it asked for (actions.rs).
    pub(crate) open_pending: Option<(String, PathBuf, Option<usize>)>,
    pub(crate) viewport_commands: Vec<egui::ViewportCommand>,
    /// Messages an agent sent to a chat, sent on the next frame (it needs the context).
    pub(crate) chat_outbox: Vec<(String, String)>,
    /// The voice call (voice.rs, app/voice.rs): its state, and the server and Piper its thread found.
    pub(crate) voice: Voice,
    pub(crate) voice_server: std::sync::Arc<std::sync::Mutex<Option<std::process::Child>>>,
    pub(crate) voice_piper: std::sync::Arc<std::sync::Mutex<Option<(PathBuf, PathBuf)>>>,
    /// Ajustes › Voz › Ouvir: the sample being said.
    pub(crate) voice_sample: Option<Player>,
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
    /// Everything under the conversation last frame: the band, the box, the hints.
    pub(crate) chat_footer: f32,
    /// Folded runs of tool steps the person opened, by their first call's id.
    pub(crate) chat_unfolded: std::collections::HashSet<String>,
    /// The list's « / » (⌘B): wide, it folds the list to initials; narrow, it opens the whole list.
    pub(crate) chat_list_toggled: bool,
    /// ⌘F's words: only the messages that say them show; None, the field is closed.
    pub(crate) chat_search: Option<String>,
    /// The conversation whose earlier messages were asked for, and how many pages of them (chat.rs).
    pub(crate) chat_earlier: (String, usize),
    /// Messages written to agents waiting on their terminal: (terminal id, text), sent when they
    /// stop waiting (`flush_chat_queue`).
    pub(crate) chat_queue: Vec<(String, String)>,
    pub(crate) terminal_dialog: Option<TerminalDialog>,
    pub(crate) portal_dialog: Option<PortalDialog>,
    pub(crate) portal_extras: portal_extras::PortalExtras,
    /// Live editors by IDE element id (outside the scene, like the shells), the one that has
    /// the keyboard (the canvas's shortcuts are off meanwhile), and the new / edit IDE dialog.
    pub(crate) ides: std::collections::HashMap<String, crate::ide::Ide>,
    pub(crate) ide_focus: Option<String>,
    /// The IDE card on the canvas whose tree has the keys (arrows, Enter, Esc) and shows hover.
    pub(crate) card_focus: Option<String>,
    pub(crate) ide_dialog: Option<IdeDialog>,
    /// The file tree nodes' live views, and their dialog and questions (filetree.rs).
    pub(crate) filetrees: FileTrees,
    /// When the enlarged terminal started growing, and the one shrinking back (with when).
    pub(crate) elevate_since: f64,
    pub(crate) leaving: Option<(String, f64)>,
    /// The enlarged window docked at a side (right?, width), and its title being dragged: where it
    /// was grabbed, inside it, and the pointer now.
    pub(crate) dock: Option<(bool, f32)>,
    pub(crate) dock_drag: Option<(Vec2, Pos2)>,
    /// The action bar button that was just clicked, and when: its icon hops.
    pub(crate) hop: Option<(&'static str, f64)>,
    /// Each rope's simulated chain: it swings behind moved ends and settles under its weight.
    pub(crate) ropes: std::collections::HashMap<String, RopeSim>,
    /// Live pages by portal element id, and what they report back.
    #[cfg(not(target_os = "linux"))]
    pub(crate) portals: std::collections::HashMap<String, PortalView>,
    /// The page that holds a call's sound on Codex's voice (voice_page.rs).
    #[cfg(not(target_os = "linux"))]
    pub(crate) voice_page: Option<crate::app::voice_page::VoicePage>,
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
    /// The toolbar alone (empty when hidden): the title bar leaves it its clicks.
    pub(crate) toolbar_span: Rect,
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
    pub(crate) memory: memory::Monitor,
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
    /// Prompts that never reached their terminal, back from the thread that typed them (nodes.rs).
    pub(crate) returned_prompts: (std::sync::mpsc::Sender<ui::nodes::Returned>, std::sync::mpsc::Receiver<ui::nodes::Returned>),
    /// The wizard above everything on the screen (avatar.rs), on with `prefs.avatar`.
    pub(crate) avatar: avatar::Avatar,
    /// The tasks (tasks.rs) and the agents panel's tabs.
    pub(crate) tasks: tasks::TaskBoard,
    /// The workspaces (workspaces.rs): the list, the shells whose space is off view (terminal id →
    /// space, spec), each space's camera and selection while off view, and the sidebar's cards.
    pub(crate) spaces: workspaces::Workspaces,
    pub(crate) away: std::collections::HashMap<String, (String, TerminalSpec)>,
    pub(crate) space_views: std::collections::HashMap<String, (f32, Vec2, Vec<String>)>,
    pub(crate) space_dialog: Option<workspaces::SpaceDialog>,
    pub(crate) space_rename: Option<(String, String)>,
}

impl Default for DrawingApp {
    fn default() -> Self {
        Self {
            scene: Scene::default(),
            undo: Vec::new(),
            redo: Vec::new(),
            tool: Tool::Select,
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
            card_split: None,
            ligatures_shown: false,
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
            bind_pulse: Default::default(),
            prefs: Prefs::default(),
            saved_prefs: Prefs::default(),
            settings_open: false,
            settings_edit: None,
            settings_tab: 0,
            press_world: Pos2::ZERO,
            move_origin: None,
            snap_guides: Vec::new(),
            style_edit_active: false,
            dirty: false,
            dirty_since: 0.0,
            design_polled: 0.0,
            glide: None,
            minimap_area: Default::default(),
            drafts: Default::default(),
            tidy_last: None,
            hidden_paint: None,
            caption_hooked: false,
            chat_popup: false,
            caption_press: None,
            title_buttons: Rect::NOTHING,
            space_filter: String::new(),
            sidebar_slide: (0.0, 0.0, -1.0),
            usage_ui: Default::default(),
            floors_ui: Default::default(),
            attention: false,
            autosave: None,
            synced: Scene::default(),
            synced_stamp: None,
            last_poll: 0.0,
            terminals: Default::default(),
            terminal_focus: None,
            terminal_scroll: 0.0,
            term_ui: Default::default(),
            terminal_elevated: None,
            terminal_busy: Default::default(),
            terminal_idle: Default::default(),
            terminal_unread: Default::default(),
            agents_open: false,
            terminal_waiting: Default::default(),
            waiting_checked: 0.0,
            agent_sessions: Default::default(),
            session_watch: Default::default(),
            claude_seen: Default::default(),
            agent_events: Default::default(),
            usage: Default::default(),
            agent_seen: Default::default(),
            rope_pulses: Vec::new(),
            hooks: Default::default(),
            terminal_ides: Default::default(),
            chat_open: false,
            chat_agent: None,
            chat_draft: String::new(),
            chat_installed: None,
            chats: Vec::new(),
            chat_runs: Default::default(),
            shots: Vec::new(),
            review_pending: None,
            open_pending: None,
            viewport_commands: Vec::new(),
            chat_outbox: Vec::new(),
            voice: Voice::default(),
            voice_server: Default::default(),
            voice_piper: Default::default(),
            voice_sample: None,
            transcripts: Default::default(),
            chat_polled: 0.0,
            chat_images: Default::default(),
            chat_pics: Default::default(),
            note_text_bottom: Default::default(),
            note_press: None,
            chat_terminal: false,
            chat_composer: 0.0,
            chat_footer: 0.0,
            chat_unfolded: Default::default(),
            chat_list_toggled: false,
            chat_search: None,
            chat_earlier: (String::new(), 0),
            chat_queue: Vec::new(),
            terminal_dialog: None,
            connecting: None,
            composer: None,
            toolbar_bottom: 0.0,
            toolbar_span: Rect::NOTHING,
            portal_dialog: None,
            portal_extras: Default::default(),
            ides: Default::default(),
            ide_focus: None,
            card_focus: None,
            ide_dialog: None,
            filetrees: Default::default(),
            elevate_since: 0.0,
            leaving: None,
            dock: None,
            dock_drag: None,
            hop: None,
            ropes: Default::default(),
            #[cfg(not(target_os = "linux"))]
            portals: Default::default(),
            #[cfg(not(target_os = "linux"))]
            voice_page: None,
            #[cfg(not(target_os = "linux"))]
            portal_events: std::sync::mpsc::channel(),
            portal_url_edit: None,
            design: None,
            annotations: Default::default(),
            design_confirm: None,
            bridge: None,
            script: None,
            perf: None,
            memory: memory::Monitor::new(),
            context: None,
            rename: None,
            note_color: None,
            links_popover: None,
            palette_links: None,
            returned_prompts: std::sync::mpsc::channel(),
            avatar: avatar::Avatar::default(),
            tasks: Default::default(),
            spaces: Default::default(),
            away: Default::default(),
            space_views: Default::default(),
            space_dialog: None,
            space_rename: None,
        }
    }
}

impl eframe::App for DrawingApp {
    /// Closing mid-edit keeps the text being typed and anything not yet autosaved (that only runs
    /// while idle).
    fn on_exit(&mut self, _gl: Option<&eframe::glow::Context>) {
        self.save_on_exit();
    }

    /// eframe clears every viewport with this one color: the avatar's and the call popup's windows
    /// need a see-through one (else a square shows around their rounded shapes), and the app's own
    /// window paints its whole background anyway (canvas and chat).
    fn clear_color(&self, visuals: &egui::Visuals) -> [f32; 4] {
        if self.avatar.shown || self.voice.on { [0.0; 4] } else { visuals.window_fill().to_normalized_gamma_f32() }
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
        if TITLE_BAR > 0.0 {
            edge_resize(ctx, raw);
            self.caption_drag(ctx, raw);
        }
        // Tab on the bare canvas moves no focus: egui would hand the keys to the first button, and
        // the shortcuts and Space would stop until the next click
        if !self.keyboard_taken(ctx) {
            raw.events.retain(|e| !matches!(e, egui::Event::Key { key: Key::Tab, .. }));
        }
    }

    fn ui(&mut self, root: &mut egui::Ui, frame: &mut eframe::Frame) {
        let ctx = root.ctx().clone();
        // the frame's clock before anything is laid out by it: read only in `handle_canvas`, the canvas's
        // edge was a frame behind the sidebar sliding beside it, and the window's ground showed between them
        self.now = ctx.input(|i| i.time);
        if let Some(perf) = &mut self.perf {
            perf.frame(ctx.input(|i| i.time), frame.info().cpu_usage, &ctx.repaint_causes());
        }
        if let Some(theme) = ctx.system_theme() {
            self.system_dark = theme == egui::Theme::Dark;
        }
        configure_visuals(&ctx, self.dark());
        if self.prefs.ligatures != self.ligatures_shown {
            install_fonts(&ctx, self.prefs.ligatures);
            self.ligatures_shown = self.prefs.ligatures;
        }
        #[cfg(target_os = "macos")]
        crate::place_window_buttons(LIGHTS_Y as f64);
        self.screen_keys(&ctx);
        self.handle_shortcuts(&ctx);
        // before the terminals: a roped terminal that starts gets its IDE's Claude Code port
        self.sync_ides(&ctx);
        self.sync_filetrees(&ctx);
        self.sync_terminals(&ctx);
        self.memory.update(
            self.terminals
                .iter()
                .filter_map(|(id, session)| {
                    let session = session.as_ref().ok()?;
                    let name = self
                        .scene
                        .elements
                        .iter()
                        .find(|e| e.id == *id)
                        .and_then(|e| e.terminal.as_ref())
                        .or(self.away.get(id).map(|(_, spec)| spec))
                        .map(|t| t.name.clone())
                        .unwrap_or_else(|| id.clone());
                    Some((name, session.shell_pid))
                })
                .collect(),
        );
        self.voice_tick(&ctx);
        self.check_agents();
        self.tasks_tick();
        self.usage_tick();
        self.poll_chats();
        self.avatar_ui(&ctx);
        self.voice_popup(&ctx);
        self.chat_popup_ui(&ctx);
        self.maestro_tick(&ctx);
        self.actions_tick(&ctx);
        if self.swing_ropes(ctx.input(|i| i.stable_dt).min(0.05)) {
            ctx.request_repaint();
        }
        // every window clears see-through while a popup is up (clear_color): this one's ground stays solid
        root.painter().rect_filled(root.max_rect(), 0, ctx.global_style().visuals.window_fill());
        egui::CentralPanel::no_frame().show(root, |ui| {
            // not focusable: Tab would park the keyboard on the canvas and turn the shortcuts and Space off
            // the workspaces' sidebar keeps the left strip (its clicks and the pages stay off it)
            let rect = ui.available_rect_before_wrap();
            let rect = rect.with_min_x(rect.min.x + self.sidebar_width());
            let response = ui.allocate_rect(rect, Sense::CLICK | Sense::DRAG);
            let painter = ui.painter_at(rect);
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
                } else if zoom != 1.0 && ctx.input(|i| i.modifiers.command) {
                    // Ctrl + wheel: egui makes a notch a factor of e^0.2 (40 points at 1/200); the
                    // reference app adds a step per notch instead
                    self.wheel_step(zoom.ln() / 0.2, pointer.unwrap_or(rect.center()));
                } else if zoom != 1.0 {
                    // a pinch
                    self.wheel_zoom(zoom, pointer.unwrap_or(rect.center()));
                } else if scroll != Vec2::ZERO && self.prefs.mouse_wheel_zooms && !shift {
                    // Dispositivo de entrada = Mouse: the wheel zooms, a notch (40 points) a step
                    self.wheel_step(scroll.y / 40.0, pointer.unwrap_or(rect.center()));
                } else if scroll != Vec2::ZERO {
                    self.glide = None;
                    self.pan += scroll;
                }
            }
            self.update_cursor(&ctx, &response);
            self.paint_stage(&ctx, &painter, rect);
        });
        if !self.floors_ui.fx.engaged {
            self.text_editor(&ctx);
            self.note_buttons_ui(&ctx);
        }
        self.floating_controls(&ctx);
        self.sidebar(&ctx);
        if TITLE_BAR > 0.0 {
            self.title_bar(&ctx);
        }
        self.note_color_ui(&ctx);
        self.links_popover_ui(&ctx);
        self.context_ui(&ctx);
        self.rename_ui(&ctx);
        if self.chat_open {
            self.chat_screen(&ctx);
        } else if self.floors_ui.fx.engaged {
            // the floors' stage: the canvas is a card in 3D, what floats over its nodes waits
        } else {
            self.terminal_overlay(&ctx);
            self.terminal_extras(&ctx);
            self.ide_overlay(&ctx);
            self.filetree_overlay(&ctx);
            self.portal_chrome(&ctx);
            self.dock_grip(&ctx);
        }
        #[cfg(windows)]
        if !self.caption_hooked {
            self.caption_hooked = crate::app::ui::titlebar::native_caption::install(&*frame);
        }
        #[cfg(not(target_os = "linux"))]
        self.sync_portals(&*frame, &ctx);
        #[cfg(not(target_os = "linux"))]
        self.sync_voice_page(&*frame, &ctx);
        #[cfg(target_os = "macos")]
        if let Some(path) = self.script.as_mut().and_then(Script::take_pages) {
            self.save_page_shots(&path);
        }
        let _ = frame;
        let idle = !self.in_gesture() && self.multi.is_none() && self.editing.is_none();
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
            || self.card_split.is_some()
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
            filetree: None,
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
