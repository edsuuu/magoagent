//! The island's global shortcuts: they answer with any program in front. Ctrl+Alt+Espaço opens the island's chat,
//! Ctrl+Alt+A goes to the question or permission that waits, Ctrl+Alt+T opens the terminal of the session in focus,
//! Ctrl+Alt+→ and ← walk the pills and Ctrl+Alt+S mutes. Each has its keys set, or is turned off, in Ajustes › Atalhos
//! (`Prefs.shortcuts` keeps only what the person changed). The system registers them (`os.rs`); what each one does on
//! the island is in `island/keys.rs`.
//!
//! Why these keys: on Windows Ctrl+Alt is AltGr, and on the Brazilian ABNT2 keyboard AltGr with Q, W, E, C or a digit
//! from 1 to 6 types a character (`ABNT2_ALTGR`). The defaults stay off those keys, and the recorder warns of a
//! combination that is on one.

use crate::prelude::*;
use std::collections::{BTreeMap, HashSet};

pub(crate) mod os;

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

/// One global shortcut: what it does, the id it is kept by in prefs.json (never rename one), what Ajustes calls it
/// and its keys until the person sets others.
pub(crate) struct ActionDef {
    pub(crate) action: GlobalAction,
    pub(crate) id: &'static str,
    pub(crate) label: &'static str,
    pub(crate) default_keys: &'static str,
}

const fn action(action: GlobalAction, id: &'static str, label: &'static str, default_keys: &'static str) -> ActionDef {
    ActionDef { action, id, label, default_keys }
}

pub(crate) const ACTIONS: [ActionDef; 6] = [
    action(GlobalAction::OpenChat, "openChat", "Abrir o chat da ilha", "Ctrl+Alt+Space"),
    action(GlobalAction::GoToAlert, "goToAlert", "Ir à pergunta ou permissão pendente", "Ctrl+Alt+A"),
    action(GlobalAction::JumpToTerminal, "jumpToTerminal", "Abrir o terminal da sessão", "Ctrl+Alt+T"),
    action(GlobalAction::NextPill, "nextPill", "Pílula seguinte", "Ctrl+Alt+Right"),
    action(GlobalAction::PreviousPill, "prevPill", "Pílula anterior", "Ctrl+Alt+Left"),
    action(GlobalAction::MuteToggle, "muteToggle", "Ligar ou desligar o mudo", "Ctrl+Alt+S"),
];

/// What the person chose for one shortcut; empty `keys` is no key at all.
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

/// The binding in force for `def`: the one kept, else its default keys, on.
pub(crate) fn effective(def: &ActionDef, stored: &Bindings) -> Binding {
    stored.get(def.id).cloned().unwrap_or_else(|| Binding { keys: def.default_keys.into(), enabled: true })
}

/// Keeps `binding` for `def`; one equal to the default is not kept at all.
pub(crate) fn set_binding(stored: &mut Bindings, def: &ActionDef, binding: Binding) {
    if binding.enabled && binding.keys == def.default_keys {
        stored.remove(def.id);
    } else {
        stored.insert(def.id.into(), binding);
    }
}

/// A combination of keys: the modifiers and the one key pressed with them.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub(crate) struct Combo {
    pub(crate) ctrl: bool,
    pub(crate) alt: bool,
    pub(crate) shift: bool,
    pub(crate) win: bool,
    pub(crate) key: Key,
}

/// The Windows virtual key of `key`, for the keys a shortcut may have: letters, digits, F1 to F12, the arrows and
/// the named keys below. None for the rest (Esc is never one: it cancels).
pub(crate) fn virtual_key(key: Key) -> Option<u32> {
    let name = key.name();
    if let [byte] = name.as_bytes()
        && byte.is_ascii_alphanumeric()
    {
        // 'A'..'Z' and '0'..'9' are their own virtual keys
        return Some(u32::from(*byte));
    }
    if let Some(number) = name.strip_prefix('F').and_then(|digits| digits.parse::<u32>().ok()) {
        return (1..=12).contains(&number).then_some(0x70 + number - 1);
    }
    Some(match key {
        Key::Space => 0x20,
        Key::Enter => 0x0D,
        Key::Tab => 0x09,
        Key::PageUp => 0x21,
        Key::PageDown => 0x22,
        Key::End => 0x23,
        Key::Home => 0x24,
        Key::ArrowLeft => 0x25,
        Key::ArrowUp => 0x26,
        Key::ArrowRight => 0x27,
        Key::ArrowDown => 0x28,
        Key::Insert => 0x2D,
        Key::Delete => 0x2E,
        Key::Comma => 0xBC,
        Key::Minus => 0xBD,
        Key::Period => 0xBE,
        _ => return None,
    })
}

/// "Ctrl+Alt+Space" as a combination. None for anything else, and for a key alone or with Shift only: a global
/// shortcut needs Ctrl, Alt or the Windows key, or it would take that key from every program.
pub(crate) fn parse(text: &str) -> Option<Combo> {
    let mut parts: Vec<&str> = text.split('+').map(str::trim).collect();
    let name = parts.pop()?;
    // any case: the MCP hands the keys over in lower case
    let named = Key::from_name(name).or_else(|| Key::ALL.iter().copied().find(|key| key.name().eq_ignore_ascii_case(name)));
    let key = named.filter(|key| virtual_key(*key).is_some())?;
    let mut combo = Combo { ctrl: false, alt: false, shift: false, win: false, key };
    for part in parts {
        match part.to_lowercase().as_str() {
            "ctrl" | "control" => combo.ctrl = true,
            "alt" => combo.alt = true,
            "shift" => combo.shift = true,
            "win" | "super" | "meta" => combo.win = true,
            _ => return None,
        }
    }
    (combo.ctrl || combo.alt || combo.win).then_some(combo)
}

/// The combination as it is kept: "Ctrl+Alt+Shift+Win+Key".
pub(crate) fn format(combo: &Combo) -> String {
    let modifiers = [(combo.ctrl, "Ctrl"), (combo.alt, "Alt"), (combo.shift, "Shift"), (combo.win, "Win")];
    modifiers.into_iter().filter(|(held, _)| *held).map(|(_, name)| name).chain([combo.key.name()]).collect::<Vec<_>>().join("+")
}

/// The keys as the person reads them: the named ones in Portuguese; "Sem tecla" for none.
pub(crate) fn display(keys: &str) -> String {
    let Some(combo) = parse(keys) else { return if keys.trim().is_empty() { "Sem tecla".into() } else { keys.into() } };
    let key = match combo.key {
        Key::Space => "Espaço",
        Key::ArrowLeft => "Esquerda",
        Key::ArrowRight => "Direita",
        Key::ArrowUp => "Cima",
        Key::ArrowDown => "Baixo",
        key => key.name(),
    };
    format(&combo).rsplit_once('+').map_or_else(|| key.to_string(), |(modifiers, _)| format!("{modifiers}+{key}"))
}

/// What AltGr types on the Brazilian ABNT2 keyboard, by key.
const ABNT2_ALTGR: [(Key, &str); 10] = [
    (Key::Q, "/"),
    (Key::W, "?"),
    (Key::E, "°"),
    (Key::C, "₢"),
    (Key::Num1, "¹"),
    (Key::Num2, "²"),
    (Key::Num3, "³"),
    (Key::Num4, "£"),
    (Key::Num5, "¢"),
    (Key::Num6, "¬"),
];

/// Windows reads the combination as AltGr: Ctrl and Alt held, without the Windows key.
pub(crate) fn is_altgr(combo: &Combo) -> bool {
    combo.ctrl && combo.alt && !combo.win
}

/// What the recorder says of any Ctrl+Alt combination.
pub(crate) const ALTGR_NOTE: &str = "No teclado ABNT2, Ctrl+Alt é o AltGr: uma combinação com ele pode conflitar com o que o AltGr digita.";

/// The warning for `keys` that are AltGr on the ABNT2 keyboard and type a character there; None for the rest.
pub(crate) fn altgr_warning(keys: &str) -> Option<String> {
    let combo = parse(keys).filter(is_altgr)?;
    let (_, typed) = ABNT2_ALTGR.iter().find(|(key, _)| *key == combo.key)?;
    Some(format!("No teclado ABNT2 esta combinação é o AltGr e digita «{typed}»: o atalho toma o lugar dela."))
}

/// What a key pressed in the recorder is.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum Recorded {
    /// Esc alone: the keys stay as they were.
    Cancel,
    /// Backspace or Delete alone: no key at all.
    Clear,
    Keys(String),
    /// A key alone, or with Shift only.
    NeedsModifier,
    /// A key a shortcut can't have.
    Unsupported,
}

/// What `key`, pressed with `modifiers` in the recorder, is.
pub(crate) fn record(key: Key, modifiers: Modifiers) -> Recorded {
    let bare = !modifiers.ctrl && !modifiers.alt && !modifiers.shift;
    match key {
        Key::Escape if bare => return Recorded::Cancel,
        Key::Backspace | Key::Delete if bare => return Recorded::Clear,
        _ => {}
    }
    if virtual_key(key).is_none() {
        return Recorded::Unsupported;
    }
    if !modifiers.ctrl && !modifiers.alt {
        return Recorded::NeedsModifier;
    }
    Recorded::Keys(format(&Combo { ctrl: modifiers.ctrl, alt: modifiers.alt, shift: modifiers.shift, win: false, key }))
}

/// How a shortcut stands, as Ajustes shows it.
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

/// The global shortcuts as they stand now.
#[derive(Default)]
pub(crate) struct Hotkeys {
    listener: Option<os::Listener>,
    /// What the listener was started for: the bindings, and whether they were to be heard at all.
    applied: Option<(Bindings, bool)>,
    /// How each action stands, in `ACTIONS`' order.
    pub(crate) statuses: Vec<Status>,
    /// The action whose keys Ajustes is recording: nothing is registered meanwhile, so that a combination already
    /// held is recorded instead of run.
    pub(crate) recording: Option<usize>,
    /// What the last key pressed in the recorder could not be.
    pub(crate) refused: Option<Recorded>,
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
        self.applied = Some((bindings.clone(), heard));
        self.statuses = plan(bindings).into_iter().map(|outcome| outcome.err().unwrap_or(Status::Paused)).collect();
    }
}

impl DrawingApp {
    /// The MCP's `set_setting` for the global shortcut `id` (`shortcut_<id>`): its keys ("Ctrl+Alt+K"), "off" or "on",
    /// or "default" for the keys it came with.
    pub(crate) fn act_shortcut(&mut self, id: &str, value: &str) -> Result<String, String> {
        let known = || ACTIONS.iter().map(|def| def.id).collect::<Vec<_>>().join(", ");
        let def = ACTIONS.iter().find(|def| def.id == id).ok_or_else(|| format!("atalho desconhecido: {id}. Há: {}.", known()))?;
        let mut binding = effective(def, &self.prefs.shortcuts);
        match value.trim() {
            "off" | "desligado" => binding.enabled = false,
            "on" | "ligado" => binding.enabled = true,
            "default" | "padrão" => binding = Binding { keys: def.default_keys.into(), enabled: true },
            keys => {
                let combo = parse(keys).ok_or_else(|| format!("{keys} não é um atalho global: use Ctrl, Alt ou Win com uma tecla, como Ctrl+Alt+K."))?;
                binding = Binding { keys: format(&combo), enabled: true };
            }
        }
        let now = format!("{}: {}{}.", def.label, display(&binding.keys), if binding.enabled { "" } else { ", desligado" });
        set_binding(&mut self.prefs.shortcuts, def, binding);
        Ok(now)
    }

    /// Every frame: the shortcuts registered are the ones Ajustes has, and the ones pressed run. None is registered
    /// with the island off, while one is being recorded, in a test or under a script.
    pub(crate) fn hotkeys_tick(&mut self, ctx: &Context) {
        if !self.settings_open || settings::SETTINGS_TABS.get(self.settings_tab) != Some(&"Atalhos") {
            (self.hotkeys.recording, self.hotkeys.refused) = (None, None);
        }
        let heard = self.prefs.island && self.hotkeys.recording.is_none();
        if self.hotkeys.applied.as_ref().is_none_or(|(bindings, was_heard)| *bindings != self.prefs.shortcuts || *was_heard != heard) {
            // the old thread lets its keys go before the new one asks for them
            self.hotkeys.listener = None;
            let plan = plan(&self.prefs.shortcuts);
            let keys: Vec<(usize, Combo)> = plan.iter().enumerate().filter_map(|(index, outcome)| Some((index, (*outcome).ok()?))).collect();
            let real = heard && !cfg!(test) && self.script.is_none();
            if real && !keys.is_empty() {
                let wake = ctx.clone();
                self.hotkeys.listener = os::listen(keys, move || wake.request_repaint());
            }
            let waiting = if real && self.hotkeys.listener.is_none() { Status::Unsupported } else { Status::Paused };
            self.hotkeys.statuses = plan.into_iter().map(|outcome| outcome.err().unwrap_or(waiting)).collect();
            self.hotkeys.applied = Some((self.prefs.shortcuts.clone(), heard));
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
                    if let Some(def) = ACTIONS.get(index) {
                        self.run_global(ctx, def.action);
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
            assert!(is_altgr(&combo), "{}", def.id);
            assert!(!ABNT2_ALTGR.iter().any(|(key, _)| *key == combo.key), "{} types a character on ABNT2", def.id);
            assert_eq!(format(&combo), def.default_keys, "{}: kept as it is written", def.id);
        }
        let ids: Vec<&str> = ACTIONS.iter().map(|def| def.id).collect();
        assert_eq!(ids, ["openChat", "goToAlert", "jumpToTerminal", "nextPill", "prevPill", "muteToggle"], "the ids are kept in prefs.json");
    }

    #[test]
    fn a_combination_needs_ctrl_alt_or_win_and_a_key_the_system_takes() {
        assert_eq!(parse(""), None);
        assert_eq!(parse("A"), None, "a key alone would take A from every program");
        assert_eq!(parse("Shift+A"), None, "and so would Shift+A");
        assert_eq!(parse("Ctrl+Alt"), None);
        assert_eq!(parse("Ctrl+Escape"), None);
        assert_eq!(parse("Ctrl+Alt+Nada"), None);
        assert_eq!(parse("Ctrl+Hyper+A"), None);
        let combo = parse("ctrl + alt + space").unwrap();
        assert_eq!(combo, Combo { ctrl: true, alt: true, shift: false, win: false, key: Key::Space });
        assert_eq!(format(&combo), "Ctrl+Alt+Space");
        assert_eq!(format(&parse("Win+Shift+Control+F5").unwrap()), "Ctrl+Shift+Win+F5");
        assert_eq!(parse("Ctrl+Alt+ArrowRight"), parse("Ctrl+Alt+Right"), "either spelling of an arrow");
        // what the system is asked for
        assert_eq!(
            (virtual_key(Key::A), virtual_key(Key::Z), virtual_key(Key::Num0), virtual_key(Key::Num9)),
            (Some(0x41), Some(0x5A), Some(0x30), Some(0x39))
        );
        assert_eq!((virtual_key(Key::F1), virtual_key(Key::F12), virtual_key(Key::F13)), (Some(0x70), Some(0x7B), None));
        assert_eq!(
            (virtual_key(Key::Space), virtual_key(Key::ArrowLeft), virtual_key(Key::ArrowRight), virtual_key(Key::Escape)),
            (Some(0x20), Some(0x25), Some(0x27), None)
        );
        // and what the person reads
        assert_eq!(display("Ctrl+Alt+Space"), "Ctrl+Alt+Espaço");
        assert_eq!(display("Ctrl+Alt+Right"), "Ctrl+Alt+Direita");
        assert_eq!(display("Ctrl+Shift+K"), "Ctrl+Shift+K");
        assert_eq!(display(""), "Sem tecla");
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
    fn the_same_keys_twice_register_once_and_a_bad_combination_registers_nothing() {
        let mut stored = Bindings::new();
        stored.insert("jumpToTerminal".into(), bound("Ctrl+Alt+A", true));
        stored.insert("muteToggle".into(), bound("S", true));
        let planned = plan(&stored);
        assert!(planned[1].is_ok(), "the first one keeps the keys");
        assert_eq!((planned[2], planned[5]), (Err(Status::Duplicate), Err(Status::Invalid)));
        // a shortcut turned off does not hold its keys
        stored.insert("goToAlert".into(), bound("Ctrl+Alt+A", false));
        assert!(plan(&stored)[2].is_ok());
    }

    #[test]
    fn the_recorder_takes_a_combination_and_warns_of_altgr_on_abnt2() {
        let (ctrl_alt, none) = (Modifiers { ctrl: true, alt: true, ..Modifiers::NONE }, Modifiers::NONE);
        assert_eq!(record(Key::Escape, none), Recorded::Cancel);
        assert_eq!(record(Key::Backspace, none), Recorded::Clear);
        assert_eq!(record(Key::K, none), Recorded::NeedsModifier);
        assert_eq!(record(Key::K, Modifiers::SHIFT), Recorded::NeedsModifier);
        assert_eq!(record(Key::Escape, ctrl_alt), Recorded::Unsupported);
        assert_eq!(record(Key::K, ctrl_alt), Recorded::Keys("Ctrl+Alt+K".into()));
        assert_eq!(record(Key::Space, Modifiers { ctrl: true, shift: true, ..none }), Recorded::Keys("Ctrl+Shift+Space".into()));
        // Ctrl+Alt is AltGr: on ABNT2 AltGr+E types the degree sign
        let typed = altgr_warning("Ctrl+Alt+E").unwrap();
        assert!(typed.contains("ABNT2") && typed.contains("AltGr") && typed.contains("«°»"), "{typed}");
        assert_eq!(altgr_warning("Ctrl+Alt+K"), None, "AltGr+K types nothing there");
        assert_eq!(altgr_warning("Ctrl+Shift+E"), None, "not AltGr without Alt");
        assert_eq!(altgr_warning("Ctrl+Alt+Win+E"), None, "nor with the Windows key");
        assert!(ALTGR_NOTE.contains("ABNT2") && ALTGR_NOTE.contains("AltGr"));
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
        assert!(set(&mut app, "shortcut_openChat", "K").unwrap_err().contains("Ctrl, Alt ou Win"));
        assert!(set(&mut app, "shortcut_nada", "off").unwrap_err().contains("muteToggle"));
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
        // a test asks the system for nothing: changed keys only stop the listener
        app.prefs.shortcuts.insert("openChat".into(), bound("Ctrl+Shift+K", true));
        app.hotkeys_tick(&ctx);
        assert!(app.hotkeys.listener.is_none());
        assert_eq!(app.hotkeys.statuses, [Status::Paused; 6]);
        // recording is over once Ajustes is closed, so the shortcuts come back
        app.hotkeys.recording = Some(0);
        app.hotkeys_tick(&ctx);
        assert_eq!(app.hotkeys.recording, None);
    }
}
