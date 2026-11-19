//! Ajustes in SwiftUI (macOS, `MAGOAGENT_SWIFTUI=settings`): topic "settings" holds the open page as groups of rows (kind,
//! text, value, options, buttons), each with an id; Swift draws any row by its kind and answers with `ui.settings.*`:
//! `tab`, `close`, `set {id, value}` (a switch's bool, a choice's index, a field's text, a stepper's number), `press
//! {id, arg, value}` (a button), `hotkey {op, id, ...}` (the shortcut recorder) and the two questions' yes and no.
//! The pages as rows are in `pages.rs`. Every row does what the egui page's does (`general.rs` and its siblings), the texts the same.

use crate::app::hotkeys::{self, ACTIONS, GLOBAL_GROUP, Scope, ShortcutDef, keys};
use crate::app::island::{Place, probe};
use crate::app::system::updates::{CLOSES_AND_REOPENS, RELEASES_PAGE, UpdateState, installs_in_place, plain_notes};
use crate::prelude::*;
mod acts;
mod app;
mod pages;
mod services;
use app::wipe_lines;
use std::sync::Mutex;
use std::time::{Duration, Instant};

#[derive(Serialize, Default)]
struct Button {
    id: String,
    label: String,
    /// Sent back with the press (a path, a link).
    arg: Option<String>,
    /// Swift puts this on the pasteboard itself.
    copy: Option<String>,
    primary: bool,
    destructive: bool,
}

#[derive(Serialize, Default)]
#[serde(rename_all = "camelCase")]
struct Keys {
    text: String,
    editing: bool,
    /// Something was pressed and waits for Salvar.
    pending: bool,
    /// What was pressed cannot be saved.
    problem: bool,
    global: bool,
    enabled: bool,
    can_clear: bool,
    changed: bool,
}

/// kind: toggle, choice (segmented), select (menu), stepper, text, note, info, button, link, head (an integration's switch),
/// hotkey, preset, notes (a paragraph block).
#[derive(Serialize, Default)]
struct Item {
    id: String,
    kind: &'static str,
    label: String,
    hint: String,
    value: Value,
    options: Vec<String>,
    min: i64,
    max: i64,
    suffix: String,
    placeholder: String,
    secret: bool,
    /// ok, bad, warn or nothing: the dot or mark's colour.
    tone: &'static str,
    buttons: Vec<Button>,
    keys: Option<Keys>,
}

#[derive(Serialize)]
struct Group {
    title: String,
    items: Vec<Item>,
}

#[derive(Serialize)]
struct Preview {
    title: String,
    confirm: String,
    state: String,
    path: String,
    diff: Vec<String>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct Snapshot {
    tabs: Vec<&'static str>,
    tab: usize,
    title: &'static str,
    subtitle: &'static str,
    groups: Vec<Group>,
    /// The question before Claude Code's settings.json is written.
    hooks_preview: Option<Preview>,
    /// Limpar todos os dados asks first: the lines and the folders.
    wipe: Option<Vec<String>>,
}

fn group(title: &str, items: Vec<Item>) -> Group {
    Group { title: title.into(), items }
}

fn item(id: &str, kind: &'static str, label: &str, hint: &str) -> Item {
    Item { id: id.into(), kind, label: label.into(), hint: hint.into(), ..Default::default() }
}

fn toggle(id: &str, label: &str, hint: &str, on: bool) -> Item {
    Item { value: json!(on), ..item(id, "toggle", label, hint) }
}

fn pick(kind: &'static str, id: &str, label: &str, hint: &str, options: &[impl AsRef<str>], now: usize) -> Item {
    Item { value: json!(now), options: options.iter().map(|option| option.as_ref().to_string()).collect(), ..item(id, kind, label, hint) }
}

fn choice(id: &str, label: &str, hint: &str, options: &[impl AsRef<str>], now: usize) -> Item {
    pick("choice", id, label, hint, options, now)
}

fn select(id: &str, label: &str, hint: &str, options: &[impl AsRef<str>], now: usize) -> Item {
    pick("select", id, label, hint, options, now)
}

fn stepper(id: &str, label: &str, hint: &str, value: i64, range: (i64, i64), suffix: &str) -> Item {
    Item { value: json!(value), min: range.0, max: range.1, suffix: suffix.into(), ..item(id, "stepper", label, hint) }
}

fn text(id: &str, label: &str, hint: &str, value: &str, placeholder: &str, secret: bool) -> Item {
    // a secret never goes to Swift: the field starts empty and what is typed comes back with the press
    let value = if secret { "" } else { value };

    Item { value: json!(value), placeholder: placeholder.into(), secret, ..item(id, "text", label, hint) }
}

fn note(words: &str) -> Item {
    item("", "note", "", words)
}

fn info(label: &str, hint: &str) -> Item {
    item("", "info", label, hint)
}

fn button(id: &str, label: &str, arg: Option<&str>) -> Button {
    Button { id: id.into(), label: label.into(), arg: arg.map(str::to_string), ..Default::default() }
}

/// A row of words with its buttons (the first built is the one at the right edge).
fn row(id: &str, label: &str, hint: &str, buttons: Vec<Button>) -> Item {
    Item { buttons, ..item(id, "button", label, hint) }
}

/// An integration's head: its dot (green connected, red not), name, what it does and its switch.
fn head(id: &str, label: &str, hint: &str, connected: bool, on: bool) -> Item {
    Item { value: json!(on), tone: if connected { "ok" } else { "bad" }, ..item(id, "head", label, hint) }
}

/// How long a snapshot lives before it is made again (the microphones, the files read); every verb ends it.
const LIFE: Duration = Duration::from_millis(1000);
static CACHE: Mutex<Option<(Instant, usize, Value)>> = Mutex::new(None);

fn forget() {
    *CACHE.lock().unwrap_or_else(|poisoned| poisoned.into_inner()) = None;
}

impl DrawingApp {
    /// Every frame: Ajustes' page, published when it is open (the egui sheet steps aside, `native_on`).
    pub(crate) fn native_settings(&mut self) {
        if !self.settings_open {
            forget();

            return;
        }

        let tab = self.settings_tab.min(super::SETTINGS_TABS.len() - 1);
        let cache = CACHE.lock().unwrap_or_else(|poisoned| poisoned.into_inner());

        if let Some((at, shown, value)) = cache.as_ref()
            && *shown == tab
            && at.elapsed() < LIFE
        {
            crate::app::native::publish("settings", value);

            return;
        }

        let tab_changed = cache.as_ref().is_none_or(|(_, shown, _)| *shown != tab);

        drop(cache);
        self.native_settings_prepare(tab, tab_changed);
        let snapshot = serde_json::to_value(self.settings_snapshot(tab)).unwrap_or_default();

        crate::app::native::publish("settings", &snapshot);
        *CACHE.lock().unwrap_or_else(|poisoned| poisoned.into_inner()) = Some((Instant::now(), tab, snapshot));
    }

    /// What a page needs started before it is read: the MCP registrations looked at when Agentes opens, the VoiceStudio's
    /// voices asked for.
    fn native_settings_prepare(&mut self, tab: usize, tab_changed: bool) {
        // ponytail: a throwaway context, the answers do not wake the frame loop; the next snapshot (1 s) shows them
        let ctx = Context::default();

        match super::SETTINGS_TABS[tab] {
            "Agentes" => {
                if tab_changed {
                    crate::app::system::mcp_register::mcp_check(&ctx);
                }

                for said in crate::app::system::mcp_register::mcp_said() {
                    self.toast(said);
                }
            }
            "Voz" if !self.prefs.voice_codex && !self.prefs.voice_openai => {
                use crate::voice::studio::{StudioServer, StudioVoices, fetch_voices};
                let idle = matches!(*self.studio_voices.lock().unwrap(), StudioVoices::Idle);

                if idle {
                    fetch_voices(StudioServer::from_prefs(&self.prefs), self.studio_voices.clone(), ctx);
                }
            }
            _ => {}
        }
    }

    fn settings_snapshot(&self, tab: usize) -> Snapshot {
        let groups = match super::SETTINGS_TABS[tab] {
            "Geral" => self.settings_swift_general(),
            "Terminal" => self.settings_swift_terminal(),
            "Agentes" => self.settings_swift_agents(),
            "Chat" => self.settings_swift_chat(),
            "Voz" => self.settings_swift_voice(),
            "Ok Mago" => self.settings_swift_ok_mago(),
            "Integrações" => self.settings_swift_integrations(),
            "Atalhos" => self.settings_swift_hotkeys(),
            "Dados" => self.settings_swift_data(),
            _ => self.settings_swift_about(),
        };

        let hooks_preview = self.claude_hooks_preview.as_ref().map(|(path, relay, before, after)| {
            let (title, confirm, state) = super::hooks_preview::preview_words(crate::app::maestro::claude_hooks::state_in(before, relay), before == after);
            let (before_text, after_text) = (serde_json::to_string_pretty(before).unwrap_or_default(), serde_json::to_string_pretty(after).unwrap_or_default());
            let (left, right): (Vec<&str>, Vec<&str>) = (before_text.lines().collect(), after_text.lines().collect());
            let line = |lines: &[&str], index: Option<usize>| index.and_then(|index| lines.get(index)).copied().unwrap_or_default().to_string();
            let mut diff = vec![];

            for row in crate::editor::diff_rows(&before_text, &after_text) {
                match (row.kind == crate::editor::RowKind::Change, row.left, row.right) {
                    (true, left_at, right_at) => {
                        diff.extend(left_at.map(|at| format!("− {}", line(&left, Some(at)))));
                        diff.extend(right_at.map(|at| format!("+ {}", line(&right, Some(at)))));
                    }
                    (false, Some(at), Some(_)) => diff.push(format!("  {}", line(&left, Some(at)))),
                    (false, Some(at), None) => diff.push(format!("− {}", line(&left, Some(at)))),
                    (false, None, Some(at)) => diff.push(format!("+ {}", line(&right, Some(at)))),
                    _ => {}
                }
            }

            Preview { title: title.into(), confirm: confirm.into(), state: state.into(), path: path.display().to_string(), diff }
        });

        let wipe = self.wipe_confirm.then(wipe_lines);

        Snapshot { tabs: super::SETTINGS_TABS.to_vec(), tab, title: super::SETTINGS_TABS[tab], subtitle: super::SUBTITLES[tab], groups, hooks_preview, wipe }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn app_on(tab: &str) -> DrawingApp {
        let mut app = DrawingApp::default();

        app.settings_open = true;
        app.settings_tab = super::super::SETTINGS_TABS.iter().position(|name| *name == tab).unwrap();
        app
    }

    fn values(snapshot: &Snapshot) -> Vec<(&str, &Value)> {
        snapshot.groups.iter().flat_map(|group| &group.items).filter(|item| !item.id.is_empty()).map(|item| (item.id.as_str(), &item.value)).collect()
    }

    #[test]
    fn the_pages_are_the_tabs_in_order_and_sobre_has_no_credits() {
        let app = app_on("Geral");
        let snapshot = app.settings_snapshot(0);

        assert_eq!(snapshot.tabs, super::super::SETTINGS_TABS.to_vec());

        for tab in 0..super::super::SETTINGS_TABS.len() {
            let snapshot = app.settings_snapshot(tab);
            let json = serde_json::to_string(&snapshot).unwrap();

            assert!(!json.contains("CRÉDITOS") && !json.contains("Ver os avisos"), "{}", snapshot.title);
            assert!(!snapshot.groups.is_empty(), "{}", snapshot.title);
        }
    }

    #[test]
    fn a_row_set_comes_back_in_the_snapshot() {
        let mut app = app_on("Geral");
        let tab = |app: &DrawingApp| app.settings_snapshot(app.settings_tab);

        // every switch flips, every choice moves to its last option, a stepper and a field take their value
        for (id, kind, last) in
            tab(&app).groups.iter().flat_map(|group| &group.items).map(|item| (item.id.clone(), item.kind, item.options.len())).collect::<Vec<_>>()
        {
            // the login item is the system's: never touched by a test
            if id.is_empty() || id == "run_at_login" || id == "island_place" {
                continue;
            }

            let wanted = match kind {
                "toggle" => json!(!tab(&app).groups.iter().flat_map(|group| &group.items).find(|item| item.id == id).unwrap().value.as_bool().unwrap()),
                "choice" | "select" => json!(last - 1),
                "stepper" => json!(7),
                "text" => json!("x"),
                _ => continue,
            };

            app.native_settings_act("set", &json!({ "id": id, "value": wanted })).unwrap();
            let snapshot = tab(&app);
            let now = values(&snapshot).into_iter().find(|(each, _)| *each == id).unwrap().1.clone();

            assert_eq!(now, if kind == "text" { json!("x") } else { wanted }, "{id}");
        }

        assert!(app.native_settings_act("set", &json!({ "id": "nada", "value": 1 })).is_err());
    }

    #[test]
    fn a_secret_never_leaves_for_swift() {
        let mut app = app_on("Voz");

        app.prefs.voice_openai = true;
        app.prefs.openai_key = "sk-segredo".into();
        let json = serde_json::to_string(&app.settings_snapshot(app.settings_tab)).unwrap();

        assert!(!json.contains("sk-segredo"));
    }

    #[test]
    fn the_recorder_waits_for_salvar_and_refuses_keys_another_shortcut_has() {
        let mut app = app_on("Atalhos");
        let hotkey = |app: &mut DrawingApp, args: Value| app.native_settings_act("hotkey", &args);

        let keys_of = |app: &DrawingApp, id: &str| {
            let snapshot = app.settings_snapshot(app.settings_tab);

            snapshot.groups.into_iter().flat_map(|group| group.items).find(|item| item.id == id).unwrap().keys.unwrap()
        };

        hotkey(&mut app, json!({ "op": "record", "id": "save" })).unwrap();
        assert!(keys_of(&app, "save").editing);
        // Cmd+Z is Desfazer's: shown, refused
        hotkey(&mut app, json!({ "op": "key", "id": "save", "key": "Z", "cmd": true })).unwrap();
        let taken = keys_of(&app, "save");

        assert!(taken.pending && taken.problem);
        hotkey(&mut app, json!({ "op": "save", "id": "save" })).unwrap();
        assert!(keys_of(&app, "save").editing, "still waiting");
        // Cmd+Alt+F5 is free: Salvar applies it
        hotkey(&mut app, json!({ "op": "key", "id": "save", "key": "F5", "cmd": true, "alt": true })).unwrap();
        assert!(!keys_of(&app, "save").problem);
        hotkey(&mut app, json!({ "op": "save", "id": "save" })).unwrap();
        let saved = keys_of(&app, "save");

        assert!(!saved.editing && saved.changed && saved.text.contains("F5"), "{}", saved.text);
        hotkey(&mut app, json!({ "op": "clear", "id": "save" })).unwrap();
        assert!(!keys_of(&app, "save").can_clear);
        hotkey(&mut app, json!({ "op": "default", "id": "save" })).unwrap();
        assert!(!keys_of(&app, "save").changed);
        // Esc alone ends the recording
        hotkey(&mut app, json!({ "op": "record", "id": "save" })).unwrap();
        hotkey(&mut app, json!({ "op": "key", "id": "save", "key": "Escape" })).unwrap();
        assert!(!keys_of(&app, "save").editing);
    }

    #[test]
    fn the_presets_move_and_go() {
        let mut app = app_on("Terminal");
        let count = app.prefs.presets.len();
        let first = app.prefs.presets[0].name.clone();

        app.native_settings_act("press", &json!({ "id": "preset.0.down" })).unwrap();
        assert_eq!(app.prefs.presets[1].name, first);
        app.native_settings_act("press", &json!({ "id": "preset.1.remove" })).unwrap();
        assert_eq!(app.prefs.presets.len(), count - 1);
        app.native_settings_act("press", &json!({ "id": "presets_add" })).unwrap();
        assert_eq!(app.prefs.presets.len(), count);
    }

    #[test]
    fn closing_clears_the_questions_and_the_tab_moves() {
        let mut app = app_on("Dados");

        app.wipe_confirm = true;
        assert!(app.settings_snapshot(app.settings_tab).wipe.is_some());
        app.native_settings_act("tab", &json!({ "index": 9 })).unwrap();
        assert_eq!(app.settings_tab, 9);
        app.native_settings_act("close", &json!({})).unwrap();
        assert!(!app.settings_open && !app.wipe_confirm);
    }
}
