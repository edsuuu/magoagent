//! macOS: the SwiftUI interface (macos/MagoUI) beside egui, area by area (`MAGOAGENT_SWIFTUI`). Swift sends actions
//! through `act` (an envelope queued here and run in the frame, by `DrawingApp::act` or a `ui.*`, the window's own); the app
//! publishes topics, each sent only when its JSON changed. Everything on the main thread. Contract: docs/swiftui.md.

use crate::prelude::*;
use std::ffi::{CStr, CString, c_char};
use std::sync::{Mutex, OnceLock};

/// The areas moving to SwiftUI, in `MAGOAGENT_SWIFTUI=all`, in the order Swift starts them (the views the shell
/// embeds first). `probe`, the bridge's proof, only by its name.
pub(crate) const AREAS: [&str; 7] = ["chat", "terminal", "filetree", "settings", "canvas", "island", "shell"];

/// What Rust hands Swift (`MagoCallbacks` in macos/MagoUI/Sources/MagoBridgeC/include/mago_ui.h): same fields, same order.
#[repr(C)]
struct MagoCallbacks {
    act: extern "C" fn(json: *const c_char),
}

unsafe extern "C" {
    fn mago_ui_start(callbacks: *const MagoCallbacks);
    fn mago_ui_publish(topic: *const c_char, json: *const c_char);
    fn mago_ui_stop();
    fn mago_ui_font(bytes: *const u8, length: std::ffi::c_ulong);
}

/// Swift's envelope: `{"id": 7, "action": "chat_new", "args": {...}}`.
#[derive(Deserialize)]
struct Envelope {
    id: u64,
    action: String,
    #[serde(default)]
    args: Value,
}

/// The answer to an envelope, topic `reply`.
#[derive(Serialize)]
struct Reply {
    id: u64,
    ok: bool,
    text: String,
}

/// The `probe` area's topic `app` (`AppTopic` in macos/MagoUI/Sources/MagoUI/Probe/Probe.swift).
#[derive(Serialize)]
struct AppTopic {
    version: &'static str,
    agents: usize,
}

/// The envelopes Swift sent, waiting for the frame.
static INBOX: Mutex<Vec<String>> = Mutex::new(Vec::new());
/// The answers of the actions that run on a thread (`act_later`), waiting for the frame.
static LATER: Mutex<Vec<Reply>> = Mutex::new(Vec::new());
/// The last JSON sent per topic.
static SENT: Mutex<Option<std::collections::HashMap<String, String>>> = Mutex::new(None);
/// The app's context once started: Swift's `act` asks it for a frame.
static CONTEXT: OnceLock<Context> = OnceLock::new();

/// Whether `area` is drawn by SwiftUI: `MAGOAGENT_SWIFTUI` is `all` or a comma list (`island,chat`); unset, none.
pub(crate) fn enabled(area: &str) -> bool {
    static ASKED: OnceLock<String> = OnceLock::new();
    asks_for(ASKED.get_or_init(|| std::env::var("MAGOAGENT_SWIFTUI").unwrap_or_default()), area)
}

fn asks_for(asked: &str, area: &str) -> bool {
    asked.split(',').map(str::trim).any(|name| name.eq_ignore_ascii_case(area) || (name == "all" && AREAS.contains(&area)))
}

/// Starts the Swift side when some area is on, and tells it which (topic `areas`).
pub(crate) fn start(context: &Context) {
    let areas: Vec<&str> = AREAS.into_iter().chain(["probe"]).filter(|area| enabled(area)).collect();
    if areas.is_empty() || CONTEXT.set(context.clone()).is_err() {
        return;
    }
    static CALLBACKS: MagoCallbacks = MagoCallbacks { act: take_envelope };
    // SAFETY: on the main thread (eframe's creation callback); the struct is static, Swift copies it anyway
    unsafe { mago_ui_start(&CALLBACKS) };
    // the app's own fonts, before any area draws: the interface's, the hand's and the terminal's (egui's Hack)
    let defaults = egui::FontDefinitions::default();
    let hack: Option<&[u8]> = defaults.font_data.get("Hack").map(|data| (*data.font).as_ref());
    for font in [crate::geometry::ASSISTANT_FONT, crate::geometry::HAND_FACE].into_iter().chain(hack) {
        // SAFETY: on the main thread; the bytes live through the call and Swift copies them
        unsafe { mago_ui_font(font.as_ptr(), font.len() as std::ffi::c_ulong) };
    }
    publish("areas", &areas);
}

/// The app is closing: Swift closes its windows.
pub(crate) fn stop() {
    if CONTEXT.get().is_some() {
        // SAFETY: on the main thread (eframe's on_exit), after `start`
        unsafe { mago_ui_stop() };
    }
}

/// Sends `value` to Swift under `topic` when its JSON differs from the last sent. Main thread only.
pub(crate) fn publish<T: Serialize>(topic: &str, value: &T) {
    if CONTEXT.get().is_none() {
        return;
    }
    let Ok(json) = serde_json::to_string(value) else { return };
    let mut sent = SENT.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
    let Some(json) = changed(sent.get_or_insert_default(), topic, json) else { return };
    drop(sent);
    // a NUL inside a string is escaped by serde (\u0000): only a topic name could hold one
    let (Ok(topic), Ok(json)) = (CString::new(topic), CString::new(json)) else { return };
    // SAFETY: on the main thread; both strings live through the call and Swift copies them
    unsafe { mago_ui_publish(topic.as_ptr(), json.as_ptr()) };
}

/// `json` when it is not what `sent` holds for `topic` (and it becomes what it holds); None when unchanged.
fn changed(sent: &mut std::collections::HashMap<String, String>, topic: &str, json: String) -> Option<String> {
    if sent.get(topic) == Some(&json) {
        return None;
    }
    sent.insert(topic.to_string(), json.clone());
    Some(json)
}

/// Swift's `act`: the envelope waits for the frame, which comes at once.
extern "C" fn take_envelope(json: *const c_char) {
    if json.is_null() {
        return;
    }
    // SAFETY: Swift passes a NUL-terminated UTF-8 string alive through this call
    let json = unsafe { CStr::from_ptr(json) }.to_string_lossy().into_owned();
    INBOX.lock().unwrap_or_else(|poisoned| poisoned.into_inner()).push(json);
    if let Some(context) = CONTEXT.get() {
        context.request_repaint();
    }
}

impl DrawingApp {
    /// Each frame (in `logic`, so also with the window hidden): Swift's actions run, their answers and the topics go.
    pub(crate) fn native_tick(&mut self) {
        if CONTEXT.get().is_none() {
            return;
        }
        let envelopes = std::mem::take(&mut *INBOX.lock().unwrap_or_else(|poisoned| poisoned.into_inner()));
        for json in envelopes {
            if let Some(reply) = self.native_act(&json) {
                publish("reply", &reply);
            }
        }
        for reply in std::mem::take(&mut *LATER.lock().unwrap_or_else(|poisoned| poisoned.into_inner())) {
            publish("reply", &reply);
        }
        if enabled("probe") {
            publish("app", &AppTopic { version: env!("CARGO_PKG_VERSION"), agents: self.terminals.len() });
        }
        // AREAS: one line per area, each publishing its topics (`native_<area>` in the area's folder), e.g.
        // if enabled("island") { self.native_island(); }
        if enabled("chat") {
            self.native_chat();
        }
        if enabled("canvas") {
            self.native_canvas();
        }
        if enabled("island") {
            self.native_island();
        }
        if enabled("terminal") {
            self.native_terminal();
        }
        if enabled("settings") {
            self.native_settings();
        }
        if enabled("shell") {
            self.native_shell();
        }
    }

    /// One envelope: its answer, or None when it runs on a thread (the answer comes through `LATER`).
    fn native_act(&mut self, json: &str) -> Option<Reply> {
        let envelope = match serde_json::from_str::<Envelope>(json) {
            Ok(envelope) => envelope,
            Err(error) => return Some(Reply { id: 0, ok: false, text: format!("envelope inválido: {error}") }),
        };
        let result = if let Some(name) = envelope.action.strip_prefix("ui.") {
            self.ui_action(name, &envelope.args)
        } else if let Some(job) = self.act_later(&envelope.action, &envelope.args) {
            // what waits for the network or a CLI answers from a thread, as the MCP's own `act` does
            let id = envelope.id;
            std::thread::spawn(move || {
                let (ok, text) = match job() {
                    Ok(text) => (true, text),
                    Err(text) => (false, text),
                };
                LATER.lock().unwrap_or_else(|poisoned| poisoned.into_inner()).push(Reply { id, ok, text });
                if let Some(context) = CONTEXT.get() {
                    context.request_repaint();
                }
            });
            return None;
        } else {
            self.act(&envelope.action, &envelope.args)
        };
        let (ok, text) = match result {
            Ok(text) => (true, text),
            Err(text) => (false, text),
        };
        Some(Reply { id: envelope.id, ok, text })
    }

    /// The actions only the interface asks for, what has no MCP tool because only a window needs it: `ui.ping`, and
    /// `ui.<area>.<verb>` handed to the area's `native_<area>_act(verb, args)`.
    fn ui_action(&mut self, name: &str, args: &Value) -> Result<String, String> {
        match name.split_once('.') {
            None if name == "ping" => Ok("pong".into()),
            // AREAS: one arm per area (with `_args` renamed `args`), e.g.
            // Some(("island", verb)) => self.native_island_act(verb, args),
            Some(("chat", verb)) => self.native_chat_act(verb, args),
            Some(("canvas", verb)) => self.native_canvas_act(verb, args),
            Some(("island", verb)) => self.native_island_act(verb, args),
            Some(("shell", verb)) => self.native_shell_act(verb, args),
            Some(("filetree", verb)) => self.native_filetree_act(verb, args),
            Some(("terminal", verb)) => self.native_terminal_act(verb, args),
            Some(("settings", verb)) => self.native_settings_act(verb, args),
            _ => Err(format!("ação de interface desconhecida: ui.{name}")),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_topic_goes_only_when_its_json_changed() {
        let mut sent = std::collections::HashMap::new();
        assert_eq!(changed(&mut sent, "app", "{\"agents\":1}".into()).as_deref(), Some("{\"agents\":1}"), "the first one goes");
        assert_eq!(changed(&mut sent, "app", "{\"agents\":1}".into()), None, "the same again stays");
        assert!(changed(&mut sent, "island", "{\"agents\":1}".into()).is_some(), "each topic has its own last");
        assert!(changed(&mut sent, "app", "{\"agents\":2}".into()).is_some(), "a change goes");
    }

    #[test]
    fn the_switch_takes_all_or_a_list() {
        assert!(asks_for("all", "island") && asks_for("all", "filetree"));
        assert!(!asks_for("all", "probe"), "the probe only by its name");
        assert!(asks_for("island, chat", "chat") && asks_for("probe", "probe"));
        assert!(!asks_for("island,chat", "canvas") && !asks_for("", "island"));
    }

    #[test]
    fn envelopes_run_the_apps_actions_and_the_interfaces_own() {
        let mut app = DrawingApp::default();
        let reply = app.native_act(r#"{"id": 3, "action": "ui.ping"}"#).unwrap();
        assert_eq!((reply.id, reply.ok, reply.text.as_str()), (3, true, "pong"));
        let reply = app.native_act(r#"{"id": 4, "action": "ui.nada.abrir", "args": {}}"#).unwrap();
        assert!(!reply.ok && reply.text.contains("ui.nada.abrir"), "an area that is not there says so");
        // one of the MCP's, through `DrawingApp::act`
        let reply = app.native_act(r#"{"id": 5, "action": "tasks", "args": {"action": "create", "title": "da interface"}}"#).unwrap();
        assert!(reply.ok, "{}", reply.text);
        let reply = app.native_act("não é json").unwrap();
        assert!(!reply.ok && reply.id == 0);
    }
}
