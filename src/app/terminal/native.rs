//! The terminal's SwiftUI view (macOS, `MAGOAGENT_SWIFTUI=terminal`): the topic `terminal:<id>` of
//! each terminal on view (the card's strip, the grid as rows of runs, the cursor, the search, the
//! menu's rows, the message box's draft) and the `ui.terminal.*` verbs the view sends back: keys,
//! text, paste, selection, wheel, size. Everything else the card does goes through the MCP's
//! actions (`terminal_menu`, `terminal_find`, `terminal_restart`, `terminal_processes`).

use crate::app::native::publish;
use crate::app::ui::agents::AgentState;
use crate::app::ui::context::{Entry, Target};
use crate::prelude::*;
use alacritty_terminal::term::TermMode;
use std::collections::{HashMap, HashSet};
use std::sync::{LazyLock, Mutex};

/// What the view tells the app, kept between frames: which terminals it shows and the size of
/// the grid it fits (cols, rows) in each.
#[derive(Default)]
struct Shared {
    shown: HashSet<String>,
    grid: HashMap<String, (usize, usize)>,
}

static SHARED: LazyLock<Mutex<Shared>> = LazyLock::new(Default::default);

/// The view shows terminal `id` (so its output repaints the app and its topic is published).
pub(crate) fn shows(id: &str) -> bool {
    SHARED.lock().unwrap().shown.contains(id)
}

/// The grid the view fits in terminal `id`, when it said (the card's size in the scene is the fallback).
pub(crate) fn grid_of(id: &str) -> Option<(usize, usize)> {
    SHARED.lock().unwrap().grid.get(id).copied()
}

#[derive(Serialize)]
struct LookOut {
    bg: u32,
    fg: u32,
    cursor: u32,
}

#[derive(Serialize)]
struct FindOut {
    query: String,
    /// "2 de 5", "Nenhum resultado" or empty.
    summary: String,
    count: usize,
    invalid: bool,
}

#[derive(Serialize, Default)]
struct EntryOut {
    id: String,
    label: String,
    check: Option<bool>,
    enabled: bool,
    keys: String,
    sep: bool,
    items: Vec<EntryOut>,
}

#[derive(Serialize)]
struct State {
    id: String,
    /// The name in the card's strip (a terminal with none is "Terminal").
    name: String,
    /// The folder strip: where it runs.
    folder: String,
    /// The mark: claude, codex, opencode, terminal… (`TERMINAL_ICONS`).
    icon: String,
    tint: Option<u32>,
    maestro: bool,
    /// "Sonnet 5.5 · Alto": the model and effort of a recruit, beside its name.
    tag: Option<String>,
    /// The agent's state: working, active, waiting, idle or ended; none for a shell with no agent.
    state: Option<&'static str>,
    /// The dot of the strip: the state's color, red while it waits for the person.
    dot: Option<u32>,
    /// running, exited, error, unloaded or starting.
    status: &'static str,
    /// What the folder strip says when it is not running.
    note: Option<String>,
    /// The font's size in pt; a cell is `font * 1233 / 2048` by `font * 17 / 14` points.
    font: f32,
    look: LookOut,
    blurred: bool,
    locked: bool,
    focused: bool,
    elevated: bool,
    copy_on_select: bool,
    grid: Option<GridSnapshot>,
    find: Option<FindOut>,
    entries: Vec<EntryOut>,
    /// The message box under the terminal, open, with its draft.
    composer: Option<String>,
}

fn color(c: Color32) -> u32 {
    u32::from_be_bytes([0, c.r(), c.g(), c.b()])
}

fn entries_out(rows: &[Entry]) -> Vec<EntryOut> {
    rows.iter()
        .map(|row| match row {
            Entry::Sep => EntryOut { sep: true, ..Default::default() },
            Entry::Item { id, label, check, enabled, keys, .. } => {
                EntryOut { id: id.clone(), label: label.clone(), check: *check, enabled: *enabled, keys: keys.clone(), ..Default::default() }
            }
            Entry::Sub { label, items } => EntryOut { label: label.clone(), enabled: true, items: entries_out(items), ..Default::default() },
        })
        .collect()
}

/// The mark by the terminal's icon, or by its command (as the card's strip picks it).
fn icon_of(spec: &TerminalSpec) -> String {
    let command = spec.command.split_whitespace().next().unwrap_or_default();
    match spec.icon.as_str() {
        "" if matches!(command, "claude" | "codex" | "opencode") => command.into(),
        "" => "terminal".into(),
        key => key.into(),
    }
}

/// Positive: the number in `args`, or 0.
fn number(args: &Value, key: &str) -> i64 {
    args[key].as_i64().unwrap_or(0)
}

impl DrawingApp {
    /// Every frame: the topic of each terminal the view shows. A new frame of the grid is built only for
    /// those, and the bridge sends the ones that changed.
    // ponytail: the grid is rebuilt every frame for each terminal on view (a hash per row, a few µs); a terminal on view with
    // nothing new could skip it by the shell's output stamp and the scroll, selection and search it had
    pub(crate) fn native_terminal(&mut self) {
        let shown: Vec<String> = SHARED.lock().unwrap().shown.iter().cloned().collect();
        for id in shown {
            if let Some(state) = self.terminal_state(&id) {
                publish(&format!("terminal:{id}"), &state);
            }
        }
    }

    fn terminal_state(&mut self, id: &str) -> Option<State> {
        let i = self.terminal_index(id)?;
        if self.term_ui.find.as_ref().is_some_and(|f| f.id == id) {
            self.refresh_find();
        }
        let element = &self.scene.elements[i];
        let spec = element.terminal.clone().unwrap_or_default();
        let (look, font) = self.terminal_look(&spec);
        let focused = self.terminal_focus.as_deref() == Some(id);
        let (session, error) = match self.terminals.get(id) {
            Some(Ok(session)) => (Some(session), None),
            Some(Err(error)) => (None, Some(error.clone())),
            None => (None, None),
        };
        let exited = session.is_some_and(|s| s.exited());
        let (status, note) = match (session, &error) {
            (_, Some(error)) => ("error", Some(format!("Não foi possível abrir o shell: {error}"))),
            (Some(_), _) if exited => ("exited", Some("Processo encerrado — clique para reiniciar".to_string())),
            (Some(_), _) => ("running", None),
            (None, _) if spec.unloaded => ("unloaded", Some("Este terminal está descarregado.".to_string())),
            (None, _) => ("starting", None),
        };
        let agent = session.map(|_| self.agent_state(id));
        let attention = !focused && (matches!(agent, Some(AgentState::Waiting)) || self.terminal_unread.contains(id));
        let dot = match agent {
            _ if attention => Some(0xff453a),
            Some(AgentState::Ended) => Some(0xe03131),
            Some(AgentState::Working(_) | AgentState::Active(_)) => Some(color(PRIMARY)),
            Some(AgentState::Waiting) => Some(color(crate::app::ui::agents::AMBER)),
            Some(AgentState::Idle(_)) => Some(0x2f9e44),
            None => None,
        };
        let state = agent.as_ref().map(|state| match state {
            AgentState::Working(_) => "working",
            AgentState::Active(_) => "active",
            AgentState::Waiting => "waiting",
            AgentState::Idle(_) => "idle",
            AgentState::Ended => "ended",
        });
        let marks = self.find_marks(id);
        let grid = session.map(|s| s.handle.grid_snapshot(&look, &marks));
        let find = self.term_ui.find.as_ref().filter(|f| f.id == id).map(|f| FindOut {
            query: f.query.clone(),
            summary: f.summary(),
            count: f.found.matches.len(),
            invalid: f.found.invalid,
        });
        Some(State {
            id: id.into(),
            name: spec.label().to_string(),
            folder: display_folder(&spec.cwd),
            icon: icon_of(&spec),
            tint: spec.color.map(|[r, g, b]| u32::from_be_bytes([0, r, g, b])),
            maestro: spec.maestro,
            tag: crate::app::terminal::card::recruit_tag(&spec),
            state,
            dot,
            status,
            note,
            font,
            look: LookOut { bg: color(look.bg), fg: color(look.fg), cursor: color(look.cursor) },
            blurred: spec.look.blurred,
            locked: element.locked,
            focused,
            elevated: self.terminal_elevated.as_deref() == Some(id),
            copy_on_select: self.prefs.terminal.copy_on_select,
            grid,
            find,
            entries: entries_out(&self.terminal_entries(i)),
            composer: self.composer.as_ref().filter(|(on, _)| on == id).map(|(_, text)| text.clone()),
        })
    }

    /// The verbs of the SwiftUI terminal (`ui.terminal.<verb>`): all take the terminal's `id`.
    pub(crate) fn native_terminal_act(&mut self, verb: &str, args: &Value) -> Result<String, String> {
        let verb = verb.strip_prefix("ui.terminal.").unwrap_or(verb);
        let id = args["id"].as_str().unwrap_or_default().to_string();
        self.terminal_index(&id).ok_or_else(|| format!("Nenhum terminal tem o id {id}."))?;
        match verb {
            "visible" => {
                let mut shared = SHARED.lock().unwrap();
                if args["visible"].as_bool().unwrap_or(true) {
                    shared.shown.insert(id);
                } else {
                    shared.shown.remove(&id);
                }
                return Ok(String::new());
            }
            "focus" => {
                let on = args["focused"].as_bool().unwrap_or(true);
                if on {
                    self.terminal_focus = Some(id.clone());
                } else if self.terminal_focus.as_deref() == Some(id.as_str()) {
                    self.terminal_focus = None;
                }
                if let Some(Ok(session)) = self.terminals.get(&id) {
                    session.set_focused(on);
                }
                return Ok(String::new());
            }
            // the shell's size is the one the view fits (the card's size in the scene is only the fallback)
            // ponytail: the grid stays what the view said even once it is gone; it says again when it shows
            "resize" => {
                let size = (number(args, "cols").clamp(2, 1000) as usize, number(args, "rows").clamp(1, 1000) as usize);
                SHARED.lock().unwrap().grid.insert(id.clone(), size);
                if let Some(Ok(session)) = self.terminals.get_mut(&id) {
                    session.resize(size);
                }
                return Ok(String::new());
            }
            "composer" => {
                // the draft the person is writing, kept here so a prompt that comes back joins it
                if let Some((_, text)) = self.composer.as_mut().filter(|(on, _)| *on == id) {
                    *text = args["text"].as_str().unwrap_or_default().to_string();
                }
                return Ok(String::new());
            }
            "prompt" => {
                let text = args["text"].as_str().unwrap_or_default();
                if text.trim().is_empty() {
                    return Err("diga o `text` a enviar".into());
                }
                if !self.send_prompt(&id, text) {
                    return Err(self.status.clone());
                }
                if let Some((_, draft)) = self.composer.as_mut().filter(|(on, _)| *on == id) {
                    draft.clear();
                }
                return Ok("Enviado.".into());
            }
            "menu" => {
                let entry = args["entry"].as_str().unwrap_or_default();
                self.context_action(&Context::default(), &Target::Element(id.clone()), entry);
                return Ok(String::new());
            }
            _ => {}
        }
        let Some(Ok(session)) = self.terminals.get(&id) else {
            return Err(format!("{} não está rodando.", self.terminal_name(&id)));
        };
        let handle = &session.handle;
        match verb {
            "input" => {
                let bytes = match args["bytes"].as_array() {
                    Some(bytes) => bytes.iter().filter_map(|b| b.as_u64().map(|b| b as u8)).collect(),
                    None => args["text"].as_str().unwrap_or_default().as_bytes().to_vec(),
                };
                if !bytes.is_empty() {
                    // typing over a selection ends it
                    handle.clear_selection();
                    handle.write(bytes);
                }
            }
            // a key by egui's name ("ArrowUp", "A", "F5"…) and its modifiers: the bytes it sends are `key_bytes`'
            "key" => {
                let name = args["key"].as_str().unwrap_or_default();
                let key = Key::from_name(name).ok_or_else(|| format!("tecla desconhecida: {name}"))?;
                let on = |modifier: &str| args["mods"][modifier].as_bool().unwrap_or(false);
                let mods = Modifiers { alt: on("alt"), ctrl: on("ctrl"), shift: on("shift"), mac_cmd: on("cmd"), command: on("cmd") };
                if let Some(bytes) = key_bytes(key, mods, handle.mode().contains(TermMode::APP_CURSOR)) {
                    handle.clear_selection();
                    handle.write(bytes);
                }
            }
            "paste" => {
                let bracketed = handle.mode().contains(TermMode::BRACKETED_PASTE);
                // a picture goes as the path of the png the view saved
                let text = match args["path"].as_str() {
                    Some(path) => format!("{} ", crate::app::terminal::menu::shell_quote(path)),
                    None => args["text"].as_str().unwrap_or_default().to_string(),
                };
                if !text.is_empty() {
                    handle.clear_selection();
                    handle.write(paste_bytes(&text, bracketed));
                }
            }
            "copy" => return Ok(handle.selected_text()),
            "clear" => handle.clear(),
            // lines positive up; where the pointer is (col, row) matters when the program takes the wheel
            "scroll" => {
                let lines = number(args, "lines") as i32;
                let at = (number(args, "col").max(0) as usize, number(args, "row").max(0) as usize);
                match wheel_bytes(lines, handle.mode(), at, args["shift"].as_bool().unwrap_or(false)) {
                    Some(bytes) => handle.write(bytes),
                    None => session.scroll(lines),
                }
            }
            // phase: begin (pick: chars, word or line), extend, all or clear. The reply is the text to copy when
            // "copiar ao selecionar" asks for it
            "select" => {
                let cell =
                    GridCell { row: number(args, "row") as i32, col: number(args, "col").max(0) as usize, right: args["right"].as_bool().unwrap_or(false) };
                let phase = args["phase"].as_str().unwrap_or_default();
                match phase {
                    "begin" => {
                        let pick = match args["pick"].as_str() {
                            Some("word") => Pick::Word,
                            Some("line") => Pick::Line,
                            _ => Pick::Chars,
                        };
                        handle.begin_selection(cell, pick);
                    }
                    "extend" => handle.extend_selection(cell),
                    "all" => handle.select_all(),
                    "clear" => handle.clear_selection(),
                    "end" => {}
                    other => return Err(format!("fase desconhecida: {other} (begin, extend, end, all, clear)")),
                }
                let picked_whole = phase == "begin" && args["pick"].as_str().is_some_and(|p| p != "chars");
                if self.prefs.terminal.copy_on_select && (phase == "end" || picked_whole) {
                    return Ok(handle.selected_text());
                }
            }
            other => return Err(format!("verbo desconhecido: ui.terminal.{other}")),
        }
        Ok(String::new())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// An app with a terminal running `program`, its shell started and on view.
    fn app_running(program: &str, args: &[&str]) -> (DrawingApp, String) {
        let mut app = DrawingApp::default();
        let mut e = app.new_element(ElementKind::Rectangle { x: 0.0, y: 0.0, width: 640.0, height: 400.0 });
        e.terminal = Some(TerminalSpec { name: "Um".into(), ..Default::default() });
        let id = e.id.clone();
        app.scene.elements.push(e);
        let session = TerminalSession::spawn("", "", (40, 6), None, vec![], Some((program, args)), 100).unwrap();
        app.terminals.insert(id.clone(), Ok(session));
        app.native_terminal_act("visible", &json!({ "id": id })).unwrap();
        (app, id)
    }

    fn wait_for(app: &DrawingApp, id: &str, text: &str) {
        for _ in 0..100 {
            if matches!(app.terminals.get(id), Some(Ok(s)) if s.handle.screen_text().contains(text)) {
                return;
            }
            std::thread::sleep(std::time::Duration::from_millis(50));
        }
        panic!("{text} never showed");
    }

    #[test]
    fn the_state_of_a_terminal_on_view_has_its_strip_grid_and_menu() {
        let (mut app, id) = app_running("sh", &["-c", "printf 'hello world'; sleep 5"]);
        wait_for(&app, &id, "hello world");
        assert!(shows(&id));
        let state = serde_json::to_value(app.terminal_state(&id).unwrap()).unwrap();
        assert_eq!((state["name"].as_str(), state["status"].as_str(), state["icon"].as_str()), (Some("Um"), Some("running"), Some("terminal")));
        assert_eq!(state["grid"]["cols"], 40);
        assert_eq!(state["grid"]["rows"][0]["runs"][0][2], "hello world");
        assert_eq!(state["look"]["bg"], 0x181818);
        let labels: Vec<&str> = state["entries"].as_array().unwrap().iter().filter_map(|e| e["label"].as_str()).collect();
        assert!(labels.starts_with(&["Copiar", "Colar", "Selecionar Tudo"]), "{labels:?}");
        assert!(state["find"].is_null() && state["composer"].is_null() && state["tag"].is_null());
        // the message box and the search show in it
        app.act("terminal_menu", &json!({ "name": id, "action": "message_box" })).unwrap();
        app.act("terminal_find", &json!({ "name": id, "query": "world" })).unwrap();
        let state = serde_json::to_value(app.terminal_state(&id).unwrap()).unwrap();
        assert_eq!(state["composer"], "");
        assert_eq!((state["find"]["count"].as_u64(), state["find"]["summary"].as_str()), (Some(1), Some("1 de 1")));
        // a hidden terminal is not published, an unknown one is refused
        app.native_terminal_act("ui.terminal.visible", &json!({ "id": id, "visible": false })).unwrap();
        assert!(!shows(&id));
        assert!(app.native_terminal_act("input", &json!({ "id": "nope", "text": "x" })).is_err());
    }

    #[test]
    fn text_keys_and_paste_reach_the_shell_as_the_egui_terminal_sends_them() {
        let (mut app, id) = app_running("cat", &[]);
        let act = |app: &mut DrawingApp, verb: &str, args: Value| {
            let mut full = json!({ "id": id });
            full.as_object_mut().unwrap().extend(args.as_object().unwrap().clone());
            app.native_terminal_act(verb, &full)
        };
        act(&mut app, "input", json!({ "text": "typed\r" })).unwrap();
        wait_for(&app, &id, "typed");
        // ArrowUp is ESC [ A, which the tty shows as ^[[A; Ctrl+Option and unknown keys send nothing
        act(&mut app, "key", json!({ "key": "ArrowUp" })).unwrap();
        wait_for(&app, &id, "^[[A");
        assert!(act(&mut app, "key", json!({ "key": "NoSuchKey" })).is_err());
        act(&mut app, "key", json!({ "key": "Backspace", "mods": { "cmd": true } })).unwrap();
        act(&mut app, "paste", json!({ "text": "pasted line" })).unwrap();
        wait_for(&app, &id, "pasted line");
        // select the pasted text with the mouse's verbs and read it back
        let screen = app.live_text(&id);
        let row = screen.lines().position(|l| l.contains("pasted line")).unwrap();
        let col = screen.lines().nth(row).unwrap().find("pasted").unwrap();
        act(&mut app, "select", json!({ "phase": "begin", "row": row, "col": col })).unwrap();
        act(&mut app, "select", json!({ "phase": "extend", "row": row, "col": col + 5, "right": true })).unwrap();
        assert_eq!(act(&mut app, "copy", json!({})).unwrap(), "pasted");
        let state = serde_json::to_value(app.terminal_state(&id).unwrap()).unwrap();
        let selected = state["grid"]["rows"][row]["runs"].as_array().unwrap().iter().any(|run| run[4].as_i64().is_some_and(|bg| bg >= 0));
        assert!(selected, "the selection shows in the run backgrounds");
        act(&mut app, "select", json!({ "phase": "clear" })).unwrap();
        assert_eq!(act(&mut app, "copy", json!({})).unwrap(), "");
        // a double click picks the word, and copy-on-select hands its text back
        app.prefs.terminal.copy_on_select = true;
        let said = act(&mut app, "select", json!({ "phase": "begin", "pick": "word", "row": row, "col": col + 1 })).unwrap();
        assert_eq!(said, "pasted");
    }

    #[test]
    fn the_view_sets_the_shells_size_and_the_message_box_sends_a_prompt() {
        let (mut app, id) = app_running("cat", &[]);
        app.native_terminal_act("resize", &json!({ "id": id, "cols": 100, "rows": 30 })).unwrap();
        assert_eq!(grid_of(&id), Some((100, 30)));
        let state = serde_json::to_value(app.terminal_state(&id).unwrap()).unwrap();
        assert_eq!((state["grid"]["cols"].as_u64(), state["grid"]["rows"].as_array().map(Vec::len)), (Some(100), Some(30)));
        assert!(app.native_terminal_act("prompt", &json!({ "id": id, "text": "  " })).is_err());
        app.native_terminal_act("focus", &json!({ "id": id })).unwrap();
        assert_eq!(app.terminal_focus.as_deref(), Some(id.as_str()));
        app.native_terminal_act("focus", &json!({ "id": id, "focused": false })).unwrap();
        assert!(app.terminal_focus.is_none());
        assert!(app.native_terminal_act("fly", &json!({ "id": id })).is_err());
    }

    impl DrawingApp {
        fn live_text(&self, id: &str) -> String {
            match self.terminals.get(id) {
                Some(Ok(session)) => session.handle.screen_text(),
                _ => String::new(),
            }
        }
    }
}
