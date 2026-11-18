//! The shortcuts: the island's global ones, which answer with any program in front, and the app's own, which answer
//! in its window (`local.rs`). Each one has its keys set, or none, in Ajustes › Atalhos (`editor.rs`: no two that can
//! be pressed at the same time with the same keys); `Prefs.shortcuts` keeps only what the person changed, by id, so an
//! older prefs.json keeps every default.
//!
//! The global ones: Ctrl+Alt+Espaço opens the island's chat, Ctrl+Alt+A goes to the question or permission that waits,
//! Ctrl+Alt+T opens the terminal of the session in focus, Ctrl+Alt+→ and ← walk the pills and Ctrl+Alt+S mutes; each
//! can also be turned off. The system registers them (`os.rs`) again as soon as their keys change; what each one does
//! on the island is in `island/keys.rs`.
//!
//! Why these keys: on Windows Ctrl+Alt is AltGr, and on the Brazilian ABNT2 keyboard AltGr with Q, W, E, C or a digit
//! from 1 to 6 types a character (`keys::ABNT2_ALTGR`). The defaults stay off those keys, and the recorder warns of a
//! combination that is on one.

use crate::prelude::*;
use std::collections::{BTreeMap, HashSet};

pub(crate) mod editor;
pub(crate) mod keys;
pub(crate) mod local;
pub(crate) mod os;

pub(crate) use keys::{ALTGR_NOTE, Combo, altgr_warning, display, parse, parse_keys};

/// What a global shortcut does.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum GlobalAction {
    OpenChat,
    GoToAlert,
    JumpToTerminal,
    NextPill,
    PreviousPill,
    MuteToggle,
}

/// What a shortcut does: on the island, from anywhere, or in the app's window.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Act {
    Global(GlobalAction),
    App(local::AppAction),
}

/// Where a shortcut answers.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Scope {
    /// With any program in front: the system holds the keys.
    Global,
    /// In the app's window.
    App,
    /// On the island, open in its own window.
    Island,
}

impl Scope {
    /// Whether a shortcut here and one in `other` can be pressed at the same time: a global one is everywhere.
    pub(crate) fn overlaps(self, other: Scope) -> bool {
        self == other || self == Scope::Global || other == Scope::Global
    }
}

/// One shortcut: what it does, the id it is kept by in prefs.json (never rename one), the group Ajustes lists it
/// under, what Ajustes calls it and its keys until the person sets others (the first is shown first; the others
/// answer as well).
pub(crate) struct ShortcutDef {
    pub(crate) act: Act,
    pub(crate) id: &'static str,
    pub(crate) group: &'static str,
    pub(crate) label: &'static str,
    pub(crate) defaults: &'static [&'static str],
}

impl ShortcutDef {
    pub(crate) fn scope(&self) -> Scope {
        match self.act {
            Act::Global(_) => Scope::Global,
            Act::App(_) => Scope::App,
        }
    }
}

/// The group of the global shortcuts in Ajustes.
pub(crate) const GLOBAL_GROUP: &str = "ATALHOS GLOBAIS DA ILHA";

const fn global(action: GlobalAction, id: &'static str, label: &'static str, defaults: &'static [&'static str]) -> ShortcutDef {
    ShortcutDef { act: Act::Global(action), id, group: GLOBAL_GROUP, label, defaults }
}

/// The global shortcuts; the system tells of each one by its index here (`os.rs`).
pub(crate) static ACTIONS: [ShortcutDef; 6] = [
    global(GlobalAction::OpenChat, "openChat", "Abrir o chat da ilha", &["Ctrl+Alt+Space"]),
    global(GlobalAction::GoToAlert, "goToAlert", "Ir à pergunta ou permissão pendente", &["Ctrl+Alt+A"]),
    global(GlobalAction::JumpToTerminal, "jumpToTerminal", "Abrir o terminal da sessão", &["Ctrl+Alt+T"]),
    global(GlobalAction::NextPill, "nextPill", "Pílula seguinte", &["Ctrl+Alt+Right"]),
    global(GlobalAction::PreviousPill, "prevPill", "Pílula anterior", &["Ctrl+Alt+Left"]),
    global(GlobalAction::MuteToggle, "muteToggle", "Ligar ou desligar o mudo", &["Ctrl+Alt+S"]),
];

/// Every shortcut whose keys can be set, as Ajustes lists them: the global ones, then the app's.
pub(crate) fn every() -> impl Iterator<Item = &'static ShortcutDef> {
    ACTIONS.iter().chain(local::APP.iter())
}

/// The shortcut kept by `id`.
pub(crate) fn find(id: &str) -> Option<&'static ShortcutDef> {
    every().find(|def| def.id == id)
}

/// What the person chose for one shortcut; empty `keys` is no key at all. Only a global one is turned off.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub(crate) struct Binding {
    pub(crate) keys: String,
    pub(crate) enabled: bool,
}

impl Default for Binding {
    fn default() -> Self {
        Self { keys: String::new(), enabled: true }
    }
}

/// `Prefs.shortcuts`: only the shortcuts the person changed, by their id.
pub(crate) type Bindings = BTreeMap<String, Binding>;

/// The binding in force for `def`: the one kept, else its first default keys, on.
pub(crate) fn effective(def: &ShortcutDef, stored: &Bindings) -> Binding {
    stored.get(def.id).cloned().unwrap_or_else(|| Binding { keys: def.defaults.first().copied().unwrap_or_default().into(), enabled: true })
}

/// Whether `def` answers: a global one turned off does not, though its keys stay its own.
pub(crate) fn enabled(def: &ShortcutDef, stored: &Bindings) -> bool {
    stored.get(def.id).is_none_or(|binding| binding.enabled)
}

/// The keys that are `def`'s: the ones kept (turned off or not), else all of its defaults; none for "Sem tecla".
pub(crate) fn keys_of(def: &ShortcutDef, stored: &Bindings) -> Vec<Combo> {
    match stored.get(def.id) {
        Some(binding) => parse_keys(&binding.keys).into_iter().collect(),
        None => def.defaults.iter().filter_map(|keys| parse_keys(keys)).collect(),
    }
}

/// `def`'s keys as Ajustes shows them: "Ctrl+Shift+Z ou Ctrl+Y", "Sem tecla".
pub(crate) fn shown_keys(def: &ShortcutDef, stored: &Bindings) -> String {
    let combos = keys_of(def, stored);
    if combos.is_empty() {
        return display("");
    }
    combos.iter().map(|combo| keys::combo_label(combo, def.scope() == Scope::App)).collect::<Vec<_>>().join(" ou ")
}

/// Keeps `binding` for `def`; one that is the default (its only keys, on) is not kept at all.
pub(crate) fn set_binding(stored: &mut Bindings, def: &ShortcutDef, binding: Binding) {
    let default = matches!(def.defaults, [keys] if parse_keys(keys).is_some() && parse_keys(keys) == parse_keys(&binding.keys));
    if binding.enabled && default {
        stored.remove(def.id);
    } else {
        stored.insert(def.id.into(), binding);
    }
}

/// How a global shortcut stands, as Ajustes shows it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Status {
    /// Registered: it answers.
    Active,
    /// Turned off, or without a key.
    Off,
    /// Another program already holds the combination.
    InUse,
    /// Another shortcut of the island has the same combination.
    Duplicate,
    /// Not a combination a shortcut can be.
    Invalid,
    /// This system registers no global shortcuts yet.
    Unsupported,
    /// Not asked of the system now: the island is off, a shortcut is being recorded, or the answer is on its way.
    Paused,
}

impl Status {
    pub(crate) fn label(self) -> &'static str {
        match self {
            Status::Active => "Ativo.",
            Status::Off => "Desligado.",
            Status::InUse => "Em uso por outro programa: escolha outras teclas.",
            Status::Duplicate => "As mesmas teclas de outro atalho da ilha.",
            Status::Invalid => "Combinação inválida.",
            Status::Unsupported => "Os atalhos globais só valem no Windows, por enquanto.",
            Status::Paused => "",
        }
    }
}

/// What each action registers, in `ACTIONS`' order, before the system is asked anything: its combination, or why none.
pub(crate) fn plan(stored: &Bindings) -> Vec<Result<Combo, Status>> {
    let mut seen = HashSet::new();
    ACTIONS
        .iter()
        .map(|def| {
            let binding = effective(def, stored);
            if !binding.enabled || binding.keys.trim().is_empty() {
                return Err(Status::Off);
            }
            let combo = parse(&binding.keys).ok_or(Status::Invalid)?;
            if seen.insert(combo) { Ok(combo) } else { Err(Status::Duplicate) }
        })
        .collect()
}

/// The shortcuts as they stand now.
#[derive(Default)]
pub(crate) struct Hotkeys {
    listener: Option<os::Listener>,
    /// What the listener was started for: the plan, and whether the keys were to be heard at all. Only a change of
    /// these asks the system again (the app's own shortcuts changing don't).
    applied: Option<(Vec<Result<Combo, Status>>, bool)>,
    /// How each global action stands, in `ACTIONS`' order.
    pub(crate) statuses: Vec<Status>,
    /// The shortcut whose keys Ajustes is changing: no global one is registered meanwhile, so that a combination
    /// already held is recorded instead of run, and the app's own don't run either.
    pub(crate) recording: Option<editor::Editing>,
    /// The island's pills in the order shown and the one in focus, as last drawn (`island/keys.rs`).
    pub(crate) pills: Vec<String>,
    pub(crate) shown: Option<String>,
    /// The pill the keyboard chose: it stays in focus until the pointer picks another or the island folds.
    pub(crate) pill: Option<String>,
    /// The island's window is to take the keyboard on its next frame.
    pub(crate) take_keyboard: bool,
    /// Ctrl+Alt+T: the pill whose terminal is asked for. The app's own agents open at once; a session outside the app
    /// is the hooks' to bring to the front, which take it from here.
    pub(crate) terminal_jump: Option<String>,
}

impl Hotkeys {
    /// A listener whose events the test sends: no shortcut is asked of the system.
    #[cfg(test)]
    pub(crate) fn listen_to(&mut self, events: std::sync::mpsc::Receiver<os::Event>, bindings: &Bindings, heard: bool) {
        self.listener = Some(os::Listener::fake(events));
        self.applied = Some((plan(bindings), heard));
        self.statuses = plan(bindings).into_iter().map(|outcome| outcome.err().unwrap_or(Status::Paused)).collect();
    }
}

impl DrawingApp {
    /// The MCP's `set_setting` for the shortcut `id` (`shortcut_<id>`): its keys ("Ctrl+Alt+K"), "none" for no key,
    /// "default" for the keys it came with, or, for a global one, "off" or "on". Keys another shortcut already has, or
    /// that never change, are refused, saying whose they are.
    pub(crate) fn act_shortcut(&mut self, id: &str, value: &str) -> Result<String, String> {
        let known = || every().map(|def| def.id).collect::<Vec<_>>().join(", ");
        let def = find(id).ok_or_else(|| format!("atalho desconhecido: {id}. Há: {}.", known()))?;
        let stored = &mut self.prefs.shortcuts;
        let global = def.scope() == Scope::Global;
        let refused = |problem: editor::Problem, combo: Option<&Combo>| format!("{} Nada mudou.", problem.words(def, combo));
        match value.trim() {
            switch @ ("off" | "desligado" | "on" | "ligado") if global => {
                let binding = Binding { enabled: matches!(switch, "on" | "ligado"), ..effective(def, stored) };
                set_binding(stored, def, binding);
            }
            "none" | "nenhuma" | "clear" | "limpar" | "off" | "desligado" => {
                editor::apply(def, editor::Pending::Clear, stored).map_err(|problem| refused(problem, None))?
            }
            "default" | "padrão" => editor::apply(def, editor::Pending::Default, stored).map_err(|problem| refused(problem, None))?,
            keys => {
                let combo = parse_keys(keys).ok_or_else(|| format!("{keys} não é uma combinação de teclas: use algo como Ctrl+Alt+K."))?;
                editor::apply(def, editor::Pending::Keys(combo), stored).map_err(|problem| refused(problem, Some(&combo)))?;
            }
        }
        let off = if global && !enabled(def, stored) { ", desligado" } else { "" };
        Ok(format!("{}: {}{off}.", def.label, shown_keys(def, stored)))
    }

    /// Every frame: the shortcuts registered are the ones Ajustes has, and the ones pressed run. None is registered
    /// with the island off, while one is being changed, in a test or under a script.
    pub(crate) fn hotkeys_tick(&mut self, ctx: &Context) {
        if !self.settings_open || settings::SETTINGS_TABS.get(self.settings_tab) != Some(&"Atalhos") {
            self.hotkeys.recording = None;
        }
        let heard = self.prefs.island && self.hotkeys.recording.is_none();
        let plan = plan(&self.prefs.shortcuts);
        if self.hotkeys.applied.as_ref().is_none_or(|(applied, was_heard)| *applied != plan || *was_heard != heard) {
            // the old thread lets its keys go before the new one asks for them
            self.hotkeys.listener = None;
            let keys: Vec<(usize, Combo)> = plan.iter().enumerate().filter_map(|(index, outcome)| Some((index, (*outcome).ok()?))).collect();
            let real = heard && !cfg!(test) && self.script.is_none();
            if real && !keys.is_empty() {
                let wake = ctx.clone();
                self.hotkeys.listener = os::listen(keys, move || wake.request_repaint());
            }
            let waiting = if real && self.hotkeys.listener.is_none() { Status::Unsupported } else { Status::Paused };
            self.hotkeys.statuses = plan.iter().map(|outcome| outcome.err().unwrap_or(waiting)).collect();
            self.hotkeys.applied = Some((plan, heard));
        }
        let events: Vec<os::Event> = self.hotkeys.listener.iter().flat_map(|listener| listener.events.try_iter()).collect();
        for event in events {
            match event {
                os::Event::Registered(results) => {
                    for (index, registered) in results {
                        if let Some(status) = self.hotkeys.statuses.get_mut(index) {
                            *status = if registered { Status::Active } else { Status::InUse };
                        }
                    }
                }
                os::Event::Pressed(index) => {
                    if let Some(Act::Global(action)) = ACTIONS.get(index).map(|def| def.act) {
                        self.run_global(ctx, action);
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn bound(keys: &str, enabled: bool) -> Binding {
        Binding { keys: keys.into(), enabled }
    }

    #[test]
    fn every_default_is_ctrl_alt_on_a_key_abnt2_types_nothing_with_and_no_two_are_the_same() {
        let plan = plan(&Bindings::new());
        assert_eq!(plan.len(), ACTIONS.len());
        for (def, outcome) in ACTIONS.iter().zip(&plan) {
            let combo = outcome.unwrap_or_else(|status| panic!("{}: {status:?}", def.id));
            assert!(keys::is_altgr(&combo), "{}", def.id);
            assert!(!keys::ABNT2_ALTGR.iter().any(|(key, _)| *key == combo.key), "{} types a character on ABNT2", def.id);
            assert_eq!([keys::format(&combo).as_str()], def.defaults, "{}: kept as it is written", def.id);
        }
        let ids: Vec<&str> = ACTIONS.iter().map(|def| def.id).collect();
        assert_eq!(ids, ["openChat", "goToAlert", "jumpToTerminal", "nextPill", "prevPill", "muteToggle"], "the ids are kept in prefs.json");
    }

    #[test]
    fn no_two_defaults_clash_and_none_is_reserved() {
        let stored = Bindings::new();
        for def in every() {
            for combo in keys_of(def, &stored) {
                assert_eq!(editor::check(def, &combo, &stored), Ok(()), "{}: {}", def.id, keys::format(&combo));
            }
        }
    }

    #[test]
    fn the_keys_set_in_ajustes_replace_the_defaults_and_one_can_be_turned_off() {
        let mut stored = Bindings::new();
        let (chat, alert, terminal, mute) = (&ACTIONS[0], &ACTIONS[1], &ACTIONS[2], &ACTIONS[5]);
        set_binding(&mut stored, chat, bound("Ctrl+Shift+K", true));
        set_binding(&mut stored, mute, bound("Ctrl+Alt+S", false));
        set_binding(&mut stored, terminal, bound("", true));
        assert_eq!(effective(chat, &stored).keys, "Ctrl+Shift+K");
        assert_eq!(effective(alert, &stored), bound("Ctrl+Alt+A", true), "untouched: its default");
        let plan = plan(&stored);
        assert_eq!(plan[0], Ok(Combo { ctrl: true, alt: false, shift: true, win: false, key: Key::K }));
        assert_eq!((plan[2], plan[5]), (Err(Status::Off), Err(Status::Off)), "without a key, and turned off");
        assert_eq!(keys_of(mute, &stored), [parse("Ctrl+Alt+S").unwrap()], "turned off, its keys are still its own");
        assert!(keys_of(terminal, &stored).is_empty());
        assert_eq!(shown_keys(terminal, &stored), "Sem tecla");
        // back to the default: nothing is kept for it
        set_binding(&mut stored, mute, bound("Ctrl+Alt+S", true));
        assert!(!stored.contains_key("muteToggle"));
        // kept in prefs.json by id; an entry written by hand without `enabled` stays on
        let json = serde_json::to_string(&stored).unwrap();
        assert_eq!(json, r#"{"jumpToTerminal":{"keys":"","enabled":true},"openChat":{"keys":"Ctrl+Shift+K","enabled":true}}"#);
        let by_hand: Bindings = serde_json::from_str(r#"{"openChat":{"keys":"Ctrl+Alt+K"}}"#).unwrap();
        assert!(by_hand["openChat"].enabled);
    }

    #[test]
    fn what_is_saved_comes_back_from_the_prefs_and_older_prefs_keep_every_default() {
        let mut prefs = Prefs::default();
        let (save, redo, chat) = (find("save").unwrap(), find("redo").unwrap(), find("openChat").unwrap());
        editor::apply(save, editor::Pending::Keys(parse_keys("Ctrl+Alt+F5").unwrap()), &mut prefs.shortcuts).unwrap();
        editor::apply(redo, editor::Pending::Clear, &mut prefs.shortcuts).unwrap();
        let json = serde_json::to_string(&prefs).unwrap();
        let back: Prefs = serde_json::from_str(&json).unwrap();
        assert_eq!(back.shortcuts, prefs.shortcuts);
        assert_eq!(shown_keys(save, &back.shortcuts), keys_label("Cmd+Alt+F5"));
        assert_eq!(shown_keys(redo, &back.shortcuts), "Sem tecla");
        // a prefs.json from before the app's shortcuts could change: its global keys, and the defaults for the rest
        let old: Prefs = serde_json::from_str(r#"{"shortcuts":{"openChat":{"keys":"Ctrl+Alt+K","enabled":true}}}"#).unwrap();
        assert_eq!(shown_keys(chat, &old.shortcuts), "Ctrl+Alt+K");
        assert_eq!(shown_keys(save, &old.shortcuts), keys_label("Cmd+S"));
        assert_eq!(shown_keys(redo, &old.shortcuts), keys_label("Cmd+Shift+Z ou Cmd+Y"));
        let older: Prefs = serde_json::from_str("{}").unwrap();
        assert!(older.shortcuts.is_empty(), "no shortcuts at all: every one on its default");
    }

    #[test]
    fn the_same_keys_twice_register_once_and_a_bad_combination_registers_nothing() {
        let mut stored = Bindings::new();
        stored.insert("jumpToTerminal".into(), bound("Ctrl+Alt+A", true));
        stored.insert("muteToggle".into(), bound("S", true));
        let planned = plan(&stored);
        assert!(planned[1].is_ok(), "the first one keeps the keys");
        assert_eq!((planned[2], planned[5]), (Err(Status::Duplicate), Err(Status::Invalid)));
        // a shortcut turned off is not registered
        stored.insert("goToAlert".into(), bound("Ctrl+Alt+A", false));
        assert!(plan(&stored)[2].is_ok());
    }

    #[test]
    fn the_mcp_sets_a_shortcuts_keys_turns_it_off_and_brings_the_default_back() {
        let mut app = DrawingApp::default();
        let set = |app: &mut DrawingApp, name: &str, value: &str| app.act("set_setting", &json!({ "name": name, "value": value }));
        assert_eq!(set(&mut app, "shortcut_openChat", "ctrl+shift+K").unwrap(), "Abrir o chat da ilha: Ctrl+Shift+K.");
        assert_eq!(app.prefs.shortcuts["openChat"], bound("Ctrl+Shift+K", true));
        assert_eq!(set(&mut app, "shortcut_openChat", "off").unwrap(), "Abrir o chat da ilha: Ctrl+Shift+K, desligado.");
        assert_eq!(plan(&app.prefs.shortcuts)[0], Err(Status::Off));
        set(&mut app, "shortcut_openChat", "default").unwrap();
        assert!(app.prefs.shortcuts.is_empty(), "the default is not kept");
        assert!(set(&mut app, "shortcut_openChat", "J").unwrap_err().contains("Ctrl ou Alt"));
        assert!(set(&mut app, "shortcut_nada", "off").unwrap_err().contains("muteToggle"));
        // the app's own: the keys of another one are refused, saying whose
        let taken = set(&mut app, "shortcut_save", "Ctrl+Z").unwrap_err();
        assert!(taken.contains("Desfazer") && taken.contains("Nada mudou"), "{taken}");
        assert!(set(&mut app, "shortcut_openChat", "Ctrl+S").unwrap_err().contains("Salvar"), "nor a global one over an app's");
        assert_eq!(set(&mut app, "shortcut_save", "Ctrl+Alt+F5").unwrap(), keys_label("Salvar: Cmd+Alt+F5."));
        assert_eq!(set(&mut app, "shortcut_save", "none").unwrap(), "Salvar: Sem tecla.");
        assert_eq!(set(&mut app, "shortcut_save", "default").unwrap(), keys_label("Salvar: Cmd+S."));
        // the tool names every shortcut
        let tools = crate::mcp::app_tools::app_tools();
        let tool = tools.iter().find(|tool| tool["name"] == "set_setting").unwrap();
        let names = tool["inputSchema"]["properties"]["name"]["enum"].as_array().unwrap();
        for def in every() {
            assert!(names.contains(&json!(format!("shortcut_{}", def.id))), "{}", def.id);
        }
        assert!(tool["description"].as_str().unwrap().contains("shortcut_save (salvar)"));
    }

    #[test]
    fn a_shortcut_pressed_runs_its_action_and_the_system_says_which_ones_it_took() {
        let ctx = Context::default();
        let mut app = DrawingApp::default();
        app.prefs.island = true;
        let (sender, events) = std::sync::mpsc::channel();
        let bindings = app.prefs.shortcuts.clone();
        app.hotkeys.listen_to(events, &bindings, true);
        sender.send(os::Event::Registered(vec![(0, true), (1, false)])).unwrap();
        let sound = app.prefs.island_sound;
        sender.send(os::Event::Pressed(5)).unwrap();
        app.hotkeys_tick(&ctx);
        assert_eq!(app.hotkeys.statuses[..3], [Status::Active, Status::InUse, Status::Paused]);
        assert_eq!(app.prefs.island_sound, !sound, "Ctrl+Alt+S: the mute");
        // an app's shortcut changing asks the system for nothing again
        app.prefs.shortcuts.insert("save".into(), bound("Ctrl+Alt+F5", true));
        app.hotkeys_tick(&ctx);
        assert!(app.hotkeys.listener.is_some(), "the global ones stay registered");
        // a test asks the system for nothing: a global one changed only stops the listener, at once
        app.prefs.shortcuts.insert("openChat".into(), bound("Ctrl+Shift+K", true));
        app.hotkeys_tick(&ctx);
        assert!(app.hotkeys.listener.is_none());
        assert_eq!(app.hotkeys.statuses, [Status::Paused; 6]);
        // recording is over once Ajustes is closed, so the shortcuts come back
        app.hotkeys.recording = Some(editor::Editing::new("openChat"));
        app.hotkeys_tick(&ctx);
        assert_eq!(app.hotkeys.recording, None);
    }
}
