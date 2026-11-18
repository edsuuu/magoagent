//! What topic "shell" says: everything the SwiftUI window draws, built from the state the egui chrome draws from.
//! Keys are camelCase (the one spelling a plain or a snake-case-converting decoder both read as is).

use crate::app::hotkeys::{Act, keys, keys_of, local::APP};
use crate::app::ui::tour::STEPS;
use crate::prelude::*;

/// Every tool, for the id <-> tool mapping of the verbs.
pub(super) const ALL_TOOLS: [Tool; 20] = [
    Tool::Hand,
    Tool::Select,
    Tool::Rectangle,
    Tool::Diamond,
    Tool::Ellipse,
    Tool::Arrow,
    Tool::Line,
    Tool::Draw,
    Tool::Text,
    Tool::Eraser,
    Tool::Frame,
    Tool::Sticky,
    Tool::AutoShape,
    Tool::Laser,
    Tool::Fill,
    Tool::Lasso,
    Tool::Terminal,
    Tool::Portal,
    Tool::FileTree,
    Tool::Chat,
];

/// A tool's id on the wire: its name in lowercase ("filetree", "autoshape").
pub(super) fn tool_id(tool: Tool) -> String {
    format!("{tool:?}").to_lowercase()
}

pub(super) fn tool_from_id(id: &str) -> Option<Tool> {
    ALL_TOOLS.into_iter().find(|tool| tool_id(*tool) == id)
}

#[derive(Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub(super) struct Shell {
    title: String,
    dev: bool,
    dark: bool,
    theme: &'static str,
    /// "canvas" or "chats": which area fills the content.
    screen: &'static str,
    /// No text field, terminal, tree or dialog holds the keyboard: a tool's letter may pick the tool.
    keys_free: bool,
    space: Option<SpaceRef>,
    floor: String,
    chats: Chats,
    sidebar: Sidebar,
    tools: Tools,
    tool: String,
    drawing: bool,
    draw_open: bool,
    hint: Option<String>,
    properties: Option<Properties>,
    can_undo: bool,
    can_redo: bool,
    zoom: u32,
    minimap: bool,
    grid: bool,
    floors: Vec<FloorRow>,
    floors_overview: bool,
    pane: PaneModel,
    dialogs: Dialogs,
    toast: Option<Toast>,
    shortcuts: Vec<MenuKey>,
    settings_open: bool,
    /// The picks the row offers (strokes, backgrounds), as the reference app's.
    stroke_picks: Vec<[u8; 4]>,
    background_picks: Vec<[u8; 4]>,
    /// The tints a space wears (its menu's colors).
    space_colors: Vec<[u8; 3]>,
}

#[derive(Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
struct SpaceRef {
    id: String,
    name: String,
    color: [u8; 3],
}

#[derive(Serialize, PartialEq)]
struct Chats {
    running: usize,
    waiting: usize,
}

#[derive(Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
struct Sidebar {
    hidden: bool,
    /// Agents waiting on the person, in every space.
    waiting: usize,
    clear_hidden: bool,
    agents_closed: bool,
    next_keys: String,
    agents: Vec<AgentModel>,
    spaces: Vec<SpaceRow>,
    groups: Vec<GroupRow>,
}

#[derive(Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
struct AgentModel {
    id: String,
    name: String,
    /// working, waiting, idle or ended.
    state: &'static str,
    label: &'static str,
    note: Option<String>,
    mission: Option<String>,
    chat: bool,
    needs_you: bool,
    program: String,
}

#[derive(Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
struct SpaceRow {
    id: String,
    name: String,
    color: [u8; 3],
    root: String,
    pinned: bool,
    group: String,
    running: usize,
    needs: usize,
    current: bool,
    floors: usize,
}

#[derive(Serialize, PartialEq)]
struct GroupRow {
    id: String,
    name: String,
    open: bool,
}

#[derive(Serialize, PartialEq)]
struct ToolRow {
    id: String,
    label: &'static str,
    badge: &'static str,
}

#[derive(Serialize, PartialEq)]
struct Tools {
    main: Vec<ToolRow>,
    drawing: Vec<ToolRow>,
    extra: Vec<ToolRow>,
}

#[derive(Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
struct Properties {
    show_stroke: bool,
    show_fill: bool,
    show_fill_style: bool,
    show_stroke_options: bool,
    show_arrow: bool,
    show_edges: bool,
    show_font: bool,
    /// Only the fill tool: no transparent pick.
    fill_tool: bool,
    frame_name: Option<String>,
    stroke: [u8; 4],
    fill: Option<[u8; 4]>,
    fill_style: Value,
    stroke_width: f32,
    stroke_style: Value,
    roughness: u8,
    round: bool,
    opacity: u8,
    font_size: f32,
    start_head: Value,
    end_head: Value,
    selection: usize,
    actions: Vec<PropertyAction>,
}

#[derive(Serialize, PartialEq)]
struct PropertyAction {
    id: &'static str,
    label: &'static str,
    keys: String,
}

#[derive(Serialize, PartialEq)]
struct FloorRow {
    id: String,
    name: String,
    color: [u8; 3],
    current: bool,
}

#[derive(Serialize, PartialEq)]
struct PaneModel {
    open: Option<&'static str>,
    buttons: Vec<PaneButton>,
}

#[derive(Serialize, PartialEq)]
struct PaneButton {
    id: &'static str,
    name: &'static str,
}

#[derive(Serialize, PartialEq, Default)]
#[serde(rename_all = "camelCase")]
struct Dialogs {
    terminal: Option<TerminalDialogModel>,
    export: Option<ExportModel>,
    wipe: Option<WipeModel>,
    palette: Option<PaletteModel>,
    welcome: Option<WelcomeModel>,
    tour: Option<TourModel>,
    changelog: Option<ChangelogModel>,
}

#[derive(Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
struct TerminalDialogModel {
    editing: bool,
    chat: bool,
    name: String,
    command: String,
    cwd: String,
    maestro: bool,
    start_in_chat: bool,
    presets: Vec<PresetModel>,
}

#[derive(Serialize, PartialEq)]
struct PresetModel {
    name: String,
    command: String,
    icon: String,
    color: Option<[u8; 3]>,
    selected: bool,
}

#[derive(Serialize, PartialEq)]
struct ExportModel {
    svg: bool,
    scale: f32,
    background: bool,
}

#[derive(Serialize, PartialEq)]
struct WipeModel {
    keychain: bool,
    folders: Vec<String>,
}

#[derive(Serialize, PartialEq)]
struct PaletteModel {
    query: String,
    index: usize,
    /// "Conectar a…" / "Desconectar…" wait for a node.
    nodes: bool,
    rows: Vec<PaletteRow>,
}

#[derive(Serialize, PartialEq)]
struct PaletteRow {
    group: &'static str,
    label: String,
    keys: String,
}

#[derive(Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
struct WelcomeModel {
    no_space: bool,
    error: String,
}

#[derive(Serialize, PartialEq)]
struct TourModel {
    step: usize,
    steps: usize,
}

#[derive(Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
struct ChangelogModel {
    version: String,
    notes: String,
    installed: bool,
    confirming: bool,
}

/// A message of the moment: `id` changes with each one, `seconds` is how long it stays.
#[derive(Serialize, PartialEq)]
struct Toast {
    id: f64,
    text: String,
    seconds: f64,
}

/// A shortcut of the menus: the app's id (`ui.shell.command`'s `name`) and the key as AppKit writes it.
#[derive(Serialize, PartialEq)]
struct MenuKey {
    id: &'static str,
    key: String,
    command: bool,
    option: bool,
    shift: bool,
    /// As Ajustes shows it ("Cmd+S"), for what has no key AppKit could take.
    text: String,
}

/// The key as a menu item's `keyEquivalent`; None for the keys AppKit has no character for (F5, arrows).
fn menu_character(key: Key) -> Option<String> {
    let name = key.symbol_or_name();
    match name {
        "Enter" => Some("\r".into()),
        "Backspace" => Some("\u{8}".into()),
        "Delete" => Some("\u{7f}".into()),
        "Space" => Some(" ".into()),
        _ if name.chars().count() == 1 => Some(name.to_lowercase()),
        _ => None,
    }
}

/// The value of a serde enum as the string it is kept as ("cross_hatch"); "none" for no arrowhead.
pub(super) fn word<T: Serialize>(value: &T) -> Value {
    serde_json::to_value(value).ok().filter(|v| !v.is_null()).unwrap_or_else(|| json!("none"))
}

fn rows(tools: &[Tool]) -> Vec<ToolRow> {
    tools
        .iter()
        .map(|tool| {
            // the terminal's button is the chat's too (its dialog picks one)
            let label = if *tool == Tool::Terminal { "Terminal ou chat" } else { tool.label() };
            ToolRow { id: tool_id(*tool), label, badge: tool.badge() }
        })
        .collect()
}

impl DrawingApp {
    /// The elements the properties row talks about: the selection, else a probe of what the tool would draw.
    pub(super) fn style_kinds(&self) -> Vec<Element> {
        if !self.selected.is_empty() {
            return self.selected.iter().filter_map(|i| self.scene.elements.get(*i).cloned()).collect();
        }
        let probe = match self.tool {
            Tool::Rectangle | Tool::Fill | Tool::Frame => ElementKind::Rectangle { x: 0.0, y: 0.0, width: 1.0, height: 1.0 },
            Tool::Diamond => ElementKind::Diamond { x: 0.0, y: 0.0, width: 1.0, height: 1.0 },
            Tool::Ellipse => ElementKind::Ellipse { x: 0.0, y: 0.0, width: 1.0, height: 1.0 },
            tool => ElementKind::Line { points: vec![], arrow: tool == Tool::Arrow },
        };
        vec![self.new_element(probe)]
    }

    /// Whether the properties row's kinds are all arrows: the arrow type and heads show, and "round" means curved.
    pub(super) fn styling_arrows(&self) -> bool {
        self.style_kinds().iter().all(|e| matches!(e.kind, ElementKind::Line { arrow: true, .. }))
    }

    fn properties_model(&self) -> Option<Properties> {
        if !self.shows_properties() {
            return None;
        }
        let kinds = self.style_kinds();
        let frames_only = if self.selected.is_empty() { self.tool == Tool::Frame } else { kinds.iter().all(|e| e.frame.is_some()) };
        let fill_tool = self.selected.is_empty() && self.tool == Tool::Fill;
        let fillable = !frames_only && kinds.iter().any(Element::fillable);
        let stroked = !frames_only && !fill_tool && kinds.iter().any(|e| !matches!(e.kind, ElementKind::Text { .. }));
        let edged = !frames_only && !fill_tool && kinds.iter().any(|e| matches!(e.kind, ElementKind::Rectangle { .. } | ElementKind::Diamond { .. }));
        let texty = kinds.iter().any(|e| matches!(e.kind, ElementKind::Text { .. }));
        let arrows = self.styling_arrows();
        let style = kinds.first().map(|e| e.style).unwrap_or(self.style);
        let heads = kinds.first().filter(|_| arrows).map_or(self.heads, |e| (e.ends.start_head, e.ends.end_head));
        let frame_name = match self.selected[..] {
            [i] => self.scene.elements.get(i).and_then(|e| e.frame.clone()),
            _ => None,
        };
        let mut actions = vec![];
        if !self.selected.is_empty() {
            let keyed = |text: &str| keys_label(text);
            actions.push(PropertyAction { id: "duplicate", label: "Duplicar", keys: keyed("Cmd+D") });
            actions.push(PropertyAction { id: "delete", label: "Apagar", keys: "Delete".into() });
            if self.selected.len() > 1 {
                actions.push(PropertyAction { id: "group", label: "Agrupar seleção", keys: keyed("Cmd+G") });
            }
            if self.selected_has_group() {
                actions.push(PropertyAction { id: "ungroup", label: "Desagrupar seleção", keys: keyed("Cmd+Shift+G") });
            }
            actions.push(PropertyAction { id: "lock", label: "Bloquear", keys: keyed("Cmd+Shift+L") });
        }
        Some(Properties {
            show_stroke: !frames_only && !fill_tool,
            show_fill: fillable,
            show_fill_style: fillable && (style.fill.is_some() || fill_tool),
            show_stroke_options: stroked,
            show_arrow: arrows,
            show_edges: edged,
            show_font: texty,
            fill_tool,
            frame_name,
            stroke: style.stroke,
            fill: style.fill.filter(|color| color[3] > 0),
            fill_style: word(&style.fill_style),
            stroke_width: style.stroke_width,
            stroke_style: word(&style.stroke_style),
            roughness: style.roughness,
            round: style.round,
            opacity: style.opacity,
            font_size: style.font_size,
            start_head: word(&heads.0),
            end_head: word(&heads.1),
            selection: self.selected.len(),
            actions,
        })
    }

    fn sidebar_model(&self) -> Sidebar {
        let counts = self.space_counts();
        let needing = self.needing_you();
        let agents = self
            .space_agents()
            .iter()
            .map(|row| {
                let (state, label) = match row.state {
                    crate::app::ui::agents::AgentState::Working(_) | crate::app::ui::agents::AgentState::Active(_) => ("working", "Trabalhando"),
                    crate::app::ui::agents::AgentState::Waiting => ("waiting", "Esperando você"),
                    crate::app::ui::agents::AgentState::Idle(_) => ("idle", "Pronto"),
                    crate::app::ui::agents::AgentState::Ended => ("ended", "Encerrado"),
                };
                AgentModel {
                    id: row.id.clone(),
                    name: row.spec.label().to_string(),
                    state,
                    label,
                    note: self.state_note(row),
                    mission: row.mission.clone(),
                    chat: self.is_chat(&row.id),
                    needs_you: needing.contains(&row.id),
                    program: row.spec.command.split_whitespace().next().unwrap_or_default().to_string(),
                }
            })
            .collect();
        let spaces = self
            .spaces
            .list
            .iter()
            .map(|space| {
                let (running, needs) = counts.get(&space.id).copied().unwrap_or_default();
                SpaceRow {
                    id: space.id.clone(),
                    name: space.name.clone(),
                    color: space.color,
                    root: space.root.clone(),
                    pinned: space.pinned,
                    group: space.group.clone(),
                    running,
                    needs,
                    current: space.id == self.spaces.current,
                    floors: space.floors.len(),
                }
            })
            .collect();
        Sidebar {
            hidden: self.prefs.sidebar_hidden,
            waiting: needing.len(),
            clear_hidden: self.prefs.sidebar_clear_hidden,
            agents_closed: self.prefs.sidebar_agents_closed,
            next_keys: self.app_keys_text(crate::app::hotkeys::local::AppAction::NextNeedingYou),
            agents,
            spaces,
            groups: self.spaces.groups.iter().map(|g| GroupRow { id: g.id.clone(), name: g.name.clone(), open: g.open }).collect(),
        }
    }

    fn floors_model(&self) -> Vec<FloorRow> {
        let Some(space) = self.current_space() else { return vec![] };
        let above = space.floors.iter().rev().map(|f| FloorRow { id: f.id.clone(), name: f.name.clone(), color: f.color, current: space.floor == f.id });
        let ground = FloorRow { id: String::new(), name: crate::app::floors::GROUND.into(), color: space.color, current: space.floor.is_empty() };
        above.chain([ground]).collect()
    }

    fn dialogs_model(&self) -> Dialogs {
        let terminal = self.terminal_dialog.as_ref().map(|dialog| {
            let chat_agent = |preset: &Preset| matches!(preset.command.split_whitespace().next(), Some("claude" | "codex"));
            let presets = self.prefs.presets.iter().filter(|preset| preset.on && (!dialog.chat || chat_agent(preset))).take(10);
            TerminalDialogModel {
                editing: dialog.target.is_some(),
                chat: dialog.chat,
                name: dialog.spec.name.clone(),
                command: dialog.spec.command.clone(),
                cwd: dialog.spec.cwd.clone(),
                maestro: dialog.spec.maestro,
                start_in_chat: dialog.spec.chat,
                presets: presets
                    .map(|p| PresetModel {
                        name: p.name.clone(),
                        command: p.command.clone(),
                        icon: p.icon.clone(),
                        color: p.color,
                        selected: dialog.spec.command == p.command,
                    })
                    .collect(),
            }
        });
        let palette = self.palette_open.then(|| PaletteModel {
            query: self.palette_query.clone(),
            index: self.palette_index,
            nodes: self.palette_links.is_some(),
            rows: self.palette_rows().into_iter().map(|row| PaletteRow { group: row.group, keys: keys_label(&row.keys), label: row.label }).collect(),
        });
        Dialogs {
            terminal,
            export: self.export_open.then_some(ExportModel { svg: self.export_svg, scale: self.export_scale, background: self.export_background }),
            wipe: self.wipe_confirm.then(|| {
                let plan = crate::app::system::wipe::plan();
                WipeModel { keychain: plan.keychain, folders: plan.folders.iter().map(|f| f.display().to_string()).collect() }
            }),
            palette,
            welcome: self.welcome.as_ref().map(|w| WelcomeModel { no_space: self.spaces.list.is_empty(), error: w.error.clone() }),
            tour: self.tour.as_ref().map(|t| TourModel { step: t.step, steps: STEPS.len() }),
            changelog: self.changelog.as_ref().map(|c| ChangelogModel {
                version: c.version.clone(),
                notes: c.notes.clone(),
                installed: c.installed,
                confirming: self.updates.confirming,
            }),
        }
    }

    fn toast_model(&self) -> Option<Toast> {
        // -1 until the toast's first frame sets its clock (`status_toast`)
        let live = !self.status.is_empty() && self.status_time >= 0.0 && self.now - self.status_time < super::super::toolbar::toast_seconds(&self.status);
        live.then(|| Toast { id: self.status_time, text: self.status.clone(), seconds: super::super::toolbar::toast_seconds(&self.status) })
    }

    /// The app's shortcuts as AppKit menu keys: Ctrl is the command key here (`hotkeys/local.rs`).
    fn menu_keys(&self) -> Vec<MenuKey> {
        APP.iter()
            .filter(|def| matches!(def.act, Act::App(_)))
            .filter_map(|def| {
                let combo = keys_of(def, &self.prefs.shortcuts).into_iter().next()?;
                Some(MenuKey {
                    id: def.id,
                    key: menu_character(combo.key).unwrap_or_default(),
                    command: combo.ctrl,
                    option: combo.alt,
                    shift: combo.shift,
                    text: keys::combo_label(&combo, true),
                })
            })
            .collect()
    }

    pub(super) fn shell_model(&self) -> Shell {
        let space = self.current_space().map(|s| SpaceRef { id: s.id.clone(), name: s.name.clone(), color: s.color });
        let (running, waiting) = self.chats_counts();
        let keys_free = self.terminal_focus.is_none()
            && self.card_focus.is_none()
            && self.editing.is_none()
            && self.composer.is_none()
            && self.portal_url_edit.is_none()
            && !self.dialog_open()
            && !self.palette_open
            && !self.chat_open;
        Shell {
            title: crate::window_names().0,
            dev: crate::file::development(),
            dark: self.dark(),
            theme: match self.prefs.theme {
                Theme::System => "system",
                Theme::Light => "light",
                Theme::Dark => "dark",
            },
            screen: if self.chat_open { "chats" } else { "canvas" },
            keys_free,
            space,
            floor: self.floor_name(),
            chats: Chats { running, waiting },
            sidebar: self.sidebar_model(),
            tools: Tools { main: rows(&Tool::TOOLBAR), drawing: rows(&Tool::DRAWING), extra: rows(&Tool::EXTRA) },
            tool: tool_id(self.tool),
            drawing: self.drawing(),
            draw_open: self.draw_open,
            hint: self.hint().filter(|_| !self.chat_open).map(keys_label),
            properties: self.properties_model(),
            can_undo: !self.undo.is_empty(),
            can_redo: !self.redo.is_empty(),
            zoom: (self.zoom * 100.0).round() as u32,
            minimap: self.prefs.minimap,
            grid: self.prefs.grid,
            floors: self.floors_model(),
            floors_overview: self.floors_ui.overview,
            pane: PaneModel {
                open: self.panes.open.map(|pane| crate::app::panes::PANES.iter().find(|p| p.0 == pane).map_or("", |p| p.1)),
                buttons: crate::app::panes::PANES.iter().map(|p| PaneButton { id: p.1, name: p.2 }).collect(),
            },
            dialogs: self.dialogs_model(),
            toast: self.toast_model(),
            shortcuts: self.menu_keys(),
            settings_open: self.settings_open,
            stroke_picks: STROKE_PICKS.to_vec(),
            background_picks: BACKGROUND_PICKS.to_vec(),
            space_colors: crate::app::workspaces::space_colors(),
        }
    }
}
