use super::*;

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
    pub(crate) minimap_area: canvas::minimap::MinimapArea,
    /// Prompts left half-written when their terminal's workspace was hidden, by terminal id.
    pub(crate) drafts: std::collections::HashMap<String, String>,
    /// The selection Organizar last arranged and the layout it used: again cycles the layout.
    pub(crate) tidy_last: Option<(Vec<String>, usize)>,
    /// An agent's `notify` off the Mac: the next frame asks the taskbar for a look.
    pub(crate) attention: bool,
    /// When the hidden window last asked to paint (`logic`).
    pub(crate) hidden_paint: Option<std::time::Instant>,
    /// Fechar mantém em segundo plano (system/tray.rs): the icon by the clock once the window hid, a `raise`
    /// to show it on the next pass, and Ctrl+Q or Sair asking for a real close.
    pub(crate) tray: Option<crate::app::system::tray::Tray>,
    pub(crate) raise_window: bool,
    pub(crate) quitting: bool,
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
    pub(crate) usage_ui: crate::app::usage::pill::UsagePill,
    pub(crate) floors_ui: crate::app::floors::pill::FloorsUi,
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
    pub(crate) term_ui: terminal::menu::TermUi,
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
    /// Atualizações (updates.rs): a última release contra esta versão.
    pub(crate) updates: system::updates::Updates,
    /// Messages on their way along a rope (who sent, who got it, when, app seconds): its signal (paint.rs).
    pub(crate) rope_pulses: Vec<(String, String, f64)>,
    /// The events' database and the hooks' permission requests waiting for an answer (demands.rs).
    pub(crate) hooks: Hooks,
    /// The chat screen (⌘1): whose conversation, the box's text, what was read so far.
    pub(crate) chat_open: bool,
    /// The chat as the last frame had it, and when it was left: the canvas comes back through
    /// the chat's ground fading out, as the chat came in fading over it.
    pub(crate) chat_was_open: bool,
    pub(crate) chat_left_at: Option<f64>,
    /// The mini chat over the canvas (chat/mini.rs).
    pub(crate) mini: chat::mini::Mini,
    /// A portal's page as a picture while the canvas moves, and where its page box was and when
    /// it last changed (`sync_portals`).
    pub(crate) portal_stills: std::collections::HashMap<String, egui::TextureHandle>,
    pub(crate) portal_motion: std::collections::HashMap<String, (Rect, f64)>,
    pub(crate) chat_agent: Option<String>,
    pub(crate) chat_draft: String,
    pub(crate) maestro_note: Option<chat::code_comment::Note>,
    /// The agents installed here and when that was looked up (`installed_agents`).
    pub(crate) chat_installed: Option<(f64, Vec<Preset>)>,
    /// The direct chats (chat/direct.rs), the answers being written, and the form of a new one.
    pub(crate) chats: Vec<chat::direct::Chat>,
    pub(crate) chat_runs: std::collections::HashMap<String, chat::direct::ChatRun>,
    /// What other agents asked the chats over the bridge (chat/ask.rs), and when the cards on the
    /// canvas last read their conversations (chat/node.rs).
    pub(crate) chat_asks: Vec<chat::ask::ChatAsk>,
    /// The first message of each recruit chat, sent when the chat can take it (maestro/mod.rs).
    pub(crate) chat_starts: Vec<(String, String)>,
    pub(crate) chat_nodes_read: f64,
    /// What each recruit chat had done at the last look (tool calls, working), and what popped of it (chat/recruit_card.rs).
    pub(crate) recruit_seen: std::collections::HashMap<String, (usize, bool)>,
    pub(crate) recruit_popups: Vec<chat::recruit_card::RecruitPopup>,
    /// The slash commands running hidden per chat, and the menu's state (chat_commands.rs).
    pub(crate) chat_cmds: std::collections::HashMap<String, chat::commands::CmdRun>,
    pub(crate) slash: chat::commands::Slash,
    /// Screenshots the Maestro asked for (chat/tools.rs).
    pub(crate) shots: Vec<chat::tools::Shot>,
    /// Window commands an agent asked for (actions.rs).
    pub(crate) viewport_commands: Vec<egui::ViewportCommand>,
    /// Messages an agent sent to a chat, sent on the next frame (it needs the context).
    pub(crate) chat_outbox: Vec<(String, String)>,
    /// The same for a slash command of the chat (`chat_command`), run by `chat_submit`.
    pub(crate) chat_cmd_outbox: Vec<(String, String)>,
    /// The voice call (voice.rs, app/voice.rs): its state, and the server and Piper its thread found.
    pub(crate) voice: Voice,
    pub(crate) voice_server: std::sync::Arc<std::sync::Mutex<Option<std::process::Child>>>,
    pub(crate) voice_piper: std::sync::Arc<std::sync::Mutex<Option<(PathBuf, PathBuf)>>>,
    /// Ajustes › Voz › Ouvir: the sample being said.
    pub(crate) voice_sample: Option<Player>,
    pub(crate) transcripts: std::collections::HashMap<String, crate::transcript::Transcript>,
    pub(crate) chat_polled: f64,
    /// The conversation's thumbnails, by `image_key`.
    pub(crate) chat_images: std::cell::RefCell<std::collections::HashMap<(usize, String), chat::images::Decoding>>,
    /// The picture open over the chat and the ones waiting in its box.
    pub(crate) chat_pics: std::cell::RefCell<chat::images::ChatPics>,
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
    /// O painel Contexto: o que se sabe de cada chat (chat/context_ui.rs).
    pub(crate) chat_ctx: chat::context_ui::CtxState,
    /// The list's search and the chats' dates (chat/list.rs).
    pub(crate) chat_listing: chat::list::ListState,
    /// Ditar, while it is on (dictation.rs).
    pub(crate) dictation: Option<voice::dictation::Dictation>,
    /// ⌘F's words: only the messages that say them show; None, the field is closed.
    pub(crate) chat_search: Option<String>,
    /// The conversation whose earlier messages were asked for, and how many pages of them (chat.rs).
    pub(crate) chat_earlier: (String, usize),
    /// Messages written to agents waiting on their terminal: (terminal id, text), sent when they
    /// stop waiting (`flush_chat_queue`).
    pub(crate) chat_queue: Vec<(String, String)>,
    pub(crate) terminal_dialog: Option<TerminalDialog>,
    pub(crate) portal_dialog: Option<PortalDialog>,
    pub(crate) portal_extras: browser::extras::PortalExtras,
    /// The file tree on the canvas whose tree has the keys (arrows, Enter, Esc) and shows hover.
    pub(crate) card_focus: Option<String>,
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
    /// The page that holds a call's sound on Codex's voice (voice/page.rs).
    #[cfg(not(target_os = "linux"))]
    pub(crate) voice_page: Option<crate::app::voice::page::VoicePage>,
    #[cfg(not(target_os = "linux"))]
    pub(crate) portal_events: (std::sync::mpsc::Sender<PortalEvent>, std::sync::mpsc::Receiver<PortalEvent>),
    /// The address being typed in a portal's bar (portal id, text, whether to take focus).
    pub(crate) portal_url_edit: Option<(String, String, bool)>,
    /// The portal Design is armed on, if any (design.rs).
    pub(crate) design: Option<browser::design::DesignSession>,
    /// Each portal's kept annotations, numbered in the order they were kept.
    pub(crate) annotations: std::collections::HashMap<String, Vec<browser::design::Annotation>>,
    /// A Design send waiting for the app's own Enviar (design.rs).
    pub(crate) design_confirm: Option<browser::design::DesignConfirm>,
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
    pub(crate) perf: Option<system::script::Perf>,
    pub(crate) memory: system::memory::Monitor,
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
    pub(crate) avatar: maestro::avatar::Avatar,
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
