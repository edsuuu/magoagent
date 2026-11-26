//! Ajustes in SwiftUI (macOS, `MAGOAGENT_SWIFTUI=settings`): topic "settings" holds the open page as groups of rows (kind,
//! text, value, options, buttons), each with an id; Swift draws any row by its kind and answers with `ui.settings.*`:
//! `tab {name}`, `close`, `set {id, value}` (a switch's bool, a choice's index, a field's text, a stepper's number), `press
//! {id, arg, value}` (a button), `hotkey {op, id, ...}` (the shortcut recorder) and the two questions' yes and no.
//! The pages as rows are in `pages.rs`. Every row does what the egui page's does (`general.rs` and its siblings), the texts the same.

use crate::prelude::*;

use crate::app::hotkeys::{self, ACTIONS, GLOBAL_GROUP, Scope, ShortcutDef, keys};
use crate::app::island::Place;
use crate::app::maestro::skills::Skill;
use crate::app::system::updates::{CLOSES_AND_REOPENS, RELEASES_PAGE, UpdateState, installs_in_place, plain_notes};
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
    /// Swift puts this on the pasteboard itself (and the press still comes, for the toast).
    copy: Option<String>,
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

/// kind: toggle, choice (segmented), select (menu), stepper, text, note, info (a row without a control), button, head (an
/// integration's switch), link (a line that opens a page), calendar (GitHub's), usage (an agent's name over its line),
/// hotkey, preset, block (buttons from the left, out of a row), ends (the first button at the left, the others at the
/// right) and notes (a paragraph over its buttons). Buttons go in the order they are seen, left to right.
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
    /// A field's width.
    width: f32,
    secret: bool,
    /// ok, bad, warn or nothing: the dot or mark's colour.
    tone: &'static str,
    buttons: Vec<Button>,
    keys: Option<Keys>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct Group {
    /// Empty: a card with no title over it (the services after the first, under SERVIÇOS).
    title: String,
    items: Vec<Item>,
    /// What `Snapshot::scroll_to` names to bring this card into view.
    scroll_id: Option<&'static str>,
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
    /// The pages in the sidebar (the hidden ones are not there).
    tabs: Vec<&'static str>,
    title: &'static str,
    subtitle: &'static str,
    groups: Vec<Group>,
    /// The question before Claude Code's settings.json is written.
    hooks_preview: Option<Preview>,
    /// Limpar todos os dados asks first: the lines and the folders.
    wipe: Option<Vec<String>>,
    /// The card the page scrolls to once (WhatsApp, when the island sends the person to set it).
    scroll_to: Option<&'static str>,
}

fn group(title: &str, items: Vec<Item>) -> Group {
    Group { title: title.into(), items, scroll_id: None }
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

fn text(id: &str, label: &str, hint: &str, value: &str, placeholder: &str, width: f32, secret: bool) -> Item {
    // a secret never goes to Swift: the field starts empty and what is typed comes back with the press
    let value = if secret { "" } else { value };

    Item { value: json!(value), placeholder: placeholder.into(), width, secret, ..item(id, "text", label, hint) }
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

/// A row of words with its buttons at the right.
fn row(id: &str, label: &str, hint: &str, buttons: Vec<Button>) -> Item {
    Item { buttons, ..item(id, "button", label, hint) }
}

/// Buttons out of a row (`kind` block or ends), in the card's padding.
fn button_block(kind: &'static str, buttons: Vec<Button>) -> Item {
    Item { buttons, ..item("", kind, "", "") }
}

/// An integration's head: its dot (green connected, red not), name, what it does and its switch.
fn head(id: &str, label: &str, hint: &str, connected: bool, on: bool) -> Item {
    Item { value: json!(on), tone: if connected { "ok" } else { "bad" }, ..item(id, "head", label, hint) }
}

/// How long a snapshot lives before it is made again (the files read); every verb ends it.
const LIFE: Duration = Duration::from_millis(1000);
static CACHE: Mutex<Option<(Instant, usize, Value)>> = Mutex::new(None);
/// The skills of the app's folder, read on a thread when Skills opens (as the egui page's `listed`): None while it reads.
static SKILLS: Mutex<Option<Vec<Skill>>> = Mutex::new(None);

fn forget() {
    *CACHE.lock().unwrap_or_else(|poisoned| poisoned.into_inner()) = None;
}

/// The pages the sidebar shows, in order (Voz and Ok Mago are off the menu, `HIDDEN_TABS`).
fn visible_tabs() -> Vec<&'static str> {
    super::SETTINGS_TABS.into_iter().filter(|name| !super::HIDDEN_TABS.contains(name)).collect()
}

/// The app's context, to wake the frame loop when a thread is done (a throwaway one in the tests).
fn context() -> Context {
    crate::app::native::context().cloned().unwrap_or_default()
}

impl DrawingApp {
    /// Every frame: Ajustes' page, published when it is open (the egui sheet steps aside, `native_on`).
    pub(crate) fn native_settings(&mut self) {
        if !self.settings_open {
            forget();

            return;
        }

        let tab = self.settings_tab.min(super::SETTINGS_TABS.len() - 1);
        // sent once: the page scrolls there and the flag is spent, as the egui page does
        let scroll_to = std::mem::take(&mut self.integrations.scroll_to_whatsapp).then_some("whatsapp");

        if scroll_to.is_some() {
            forget();
        }

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
        let mut temp = Snapshot { scroll_to, ..self.settings_snapshot(tab) }; // TEMPTEST
        if let Ok(n) = std::env::var("MAGOAGENT_TEST_SCROLL") { for (i, g) in temp.groups.iter_mut().enumerate() { g.scroll_id = Some(Box::leak(format!("g{i}").into_boxed_str())); } temp.scroll_to = Some(Box::leak(format!("g{n}").into_boxed_str())); } // TEMPTEST
        let snapshot = serde_json::to_value(temp).unwrap_or_default(); // TEMPTEST

        crate::app::native::publish("settings", &snapshot);
        *CACHE.lock().unwrap_or_else(|poisoned| poisoned.into_inner()) = Some((Instant::now(), tab, snapshot));
    }

    /// What a page needs started before it is read: the MCP registrations asked for on Integrações, the skills read
    /// when Skills opens.
    fn native_settings_prepare(&mut self, tab: usize, tab_changed: bool) {
        match super::SETTINGS_TABS[tab] {
            "Integrações" => {
                // asked only for the agents not asked yet in this run
                crate::app::system::mcp_register::mcp_check(&context());

                for said in crate::app::system::mcp_register::mcp_said() {
                    self.toast(said);
                }
            }
            "Skills" if tab_changed => {
                *SKILLS.lock().unwrap_or_else(|poisoned| poisoned.into_inner()) = None;
                let folder = crate::app::maestro::skills::skills_path();
                let ctx = context();

                std::thread::spawn(move || {
                    let skills = folder.map(|folder| crate::app::maestro::skills::list(&folder)).unwrap_or_default();

                    *SKILLS.lock().unwrap_or_else(|poisoned| poisoned.into_inner()) = Some(skills);
                    forget();
                    ctx.request_repaint();
                });
            }
            _ => {}
        }
    }

    fn settings_snapshot(&self, tab: usize) -> Snapshot {
        let groups = match super::SETTINGS_TABS[tab] {
            "Geral" => self.settings_swift_general(),
            "Ilha" => self.settings_swift_island(),
            "Canvas" => self.settings_swift_canvas(),
            "Navegação e editor" => self.settings_swift_navigation(),
            "Terminal" => self.settings_swift_terminal(),
            "Agentes" => self.settings_swift_agents(),
            "Chat" => self.settings_swift_chat(),
            "Skills" => self.settings_swift_skills(),
            "Integrações" => self.settings_swift_integrations(),
            "Atalhos" => self.settings_swift_hotkeys(),
            "Dados" => self.settings_swift_data(),
            "Sobre" => self.settings_swift_about(),
            // Voz and Ok Mago: off the menu
            _ => vec![],
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

        Snapshot { tabs: visible_tabs(), title: super::SETTINGS_TABS[tab], subtitle: super::SUBTITLES[tab], groups, hooks_preview, wipe, scroll_to: None }
    }
}

#[cfg(test)]
mod tests;
