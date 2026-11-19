//! The keys of a shortcut: a combination is its modifiers and one key, kept in prefs.json as "Ctrl+Alt+Space" or
//! "Ctrl+Shift+Equals", read from a key pressed in Ajustes, matched against the keys that come and shown in Portuguese
//! ("Ctrl+Alt+Espaço", "Ctrl++"). What AltGr types on the Brazilian ABNT2 keyboard is here too.
//!
//! A symbol typed with Shift is kept as the key under it with Shift ("?" is Shift+Slash, "+" is Shift+Equals): the
//! keyboard hands over either one, depending on the layout, and both are the same press.

use crate::prelude::*;

/// A combination of keys: the modifiers and the one key pressed with them. In the app's own shortcuts Ctrl is the
/// command key (⌘ on the Mac).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub(crate) struct Combo {
    pub(crate) ctrl: bool,
    pub(crate) alt: bool,
    pub(crate) shift: bool,
    pub(crate) win: bool,
    pub(crate) key: Key,
}

/// Each symbol typed with Shift and the key it is typed on, on the US and the ABNT2 keyboards.
const SHIFTED: [(Key, Key); 7] = [
    (Key::Plus, Key::Equals),
    (Key::Questionmark, Key::Slash),
    (Key::Exclamationmark, Key::Num1),
    (Key::Pipe, Key::Backslash),
    (Key::Colon, Key::Semicolon),
    (Key::OpenCurlyBracket, Key::OpenBracket),
    (Key::CloseCurlyBracket, Key::CloseBracket),
];

impl Combo {
    /// The combination as it is kept and compared: a symbol typed with Shift becomes the key under it, with Shift.
    pub(crate) fn canonical(mut self) -> Self {
        if let Some((_, under)) = SHIFTED.iter().find(|(symbol, _)| *symbol == self.key) {
            (self.key, self.shift) = (*under, true);
        }

        self
    }

    /// Whether Ctrl, Alt or the Windows key is held: Shift alone types.
    pub(crate) fn has_command(&self) -> bool {
        self.ctrl || self.alt || self.win
    }
}

/// Ctrl, Alt, Shift and the Windows key, which are pressed with a key and never are one.
pub(crate) fn is_modifier(key: Key) -> bool {
    matches!(
        key,
        Key::ShiftLeft | Key::ShiftRight | Key::ControlLeft | Key::ControlRight | Key::AltLeft | Key::AltRight | Key::SuperLeft | Key::SuperRight | Key::Fn
    )
}

/// The command key held: ⌘ on the Mac, Ctrl elsewhere (a key sent by a test or a script may set only one of the two).
pub(crate) fn command_held(modifiers: Modifiers) -> bool {
    if cfg!(target_os = "macos") { modifiers.command } else { modifiers.command || modifiers.ctrl }
}

/// The key `key` pressed with `modifiers`, as a combination of the app's own shortcuts.
pub(crate) fn pressed(key: Key, modifiers: Modifiers) -> Combo {
    Combo { ctrl: command_held(modifiers), alt: modifiers.alt, shift: modifiers.shift, win: false, key }.canonical()
}

/// Whether the key event `event` is `combo`: the same key with the same modifiers, a symbol and the key under it alike.
pub(crate) fn hits(event: &egui::Event, combo: &Combo) -> bool {
    matches!(event, egui::Event::Key { key, pressed: true, modifiers, .. } if pressed(*key, *modifiers) == *combo)
}

/// "Ctrl+Alt+Space" as a combination: the modifiers by name (Ctrl or Cmd, Alt or Option, Shift, Win), and the key by
/// its name or its symbol ("Ctrl+Comma" or "Ctrl+,", "Ctrl++"), in any case. None for anything else.
pub(crate) fn parse_keys(text: &str) -> Option<Combo> {
    let text = text.trim();

    // the plus key is the last "+" of "Ctrl++"
    let (modifiers, name) = match text.strip_suffix("++") {
        Some(rest) => (rest, "+"),
        None if text == "+" => ("", "+"),
        None => text.rsplit_once('+').unwrap_or(("", text)),
    };

    let name = name.trim();
    let key = Key::from_name(name).or_else(|| Key::ALL.iter().copied().find(|key| key.name().eq_ignore_ascii_case(name))).filter(|key| !is_modifier(*key))?;
    let mut combo = Combo { ctrl: false, alt: false, shift: false, win: false, key };

    for part in modifiers.split('+').filter(|_| !modifiers.trim().is_empty()) {
        match part.trim().to_lowercase().as_str() {
            "ctrl" | "control" | "cmd" | "command" => combo.ctrl = true,
            "alt" | "option" => combo.alt = true,
            "shift" => combo.shift = true,
            "win" | "super" | "meta" => combo.win = true,
            _ => return None,
        }
    }

    Some(combo.canonical())
}

/// The keys of a global shortcut: Ctrl, Alt or the Windows key with a key the system takes. None for a key alone or
/// with Shift only, which would take that key from every program.
pub(crate) fn parse(text: &str) -> Option<Combo> {
    parse_keys(text).filter(|combo| combo.has_command() && virtual_key(combo.key).is_some())
}

/// The combination as it is kept: "Ctrl+Alt+Shift+Win+Key".
pub(crate) fn format(combo: &Combo) -> String {
    let modifiers = [(combo.ctrl, "Ctrl"), (combo.alt, "Alt"), (combo.shift, "Shift"), (combo.win, "Win")];

    modifiers.into_iter().filter(|(held, _)| *held).map(|(_, name)| name).chain([combo.key.name()]).collect::<Vec<_>>().join("+")
}

/// What the person reads for `key`: the named ones in Portuguese, the symbols as themselves.
fn key_name(key: Key) -> &'static str {
    match key {
        Key::Space => "Espaço",
        Key::ArrowLeft => "Esquerda",
        Key::ArrowRight => "Direita",
        Key::ArrowUp => "Cima",
        Key::ArrowDown => "Baixo",
        Key::Escape => "Esc",
        Key::Minus => "-",
        Key::Quote => "'",
        key => key.symbol_or_name(),
    }
}

/// The combination as the person reads it: "Ctrl+Alt+Espaço", "Ctrl++", "?"; on the Mac the app's Ctrl is ⌘ (`command`).
pub(crate) fn combo_label(combo: &Combo, command: bool) -> String {
    let mut combo = *combo;
    // "+" and "?" are read as the symbols, not as Shift and the key under them
    let symbol = SHIFTED.iter().take(2).find(|(_, under)| combo.shift && *under == combo.key).map(|(symbol, _)| *symbol);

    if let Some(symbol) = symbol {
        (combo.key, combo.shift) = (symbol, false);
    }

    let control = if command && cfg!(target_os = "macos") { "Cmd" } else { "Ctrl" };
    let modifiers = [(combo.ctrl, control), (combo.alt, "Alt"), (combo.shift, "Shift"), (combo.win, "Win")];

    modifiers.into_iter().filter(|(held, _)| *held).map(|(_, name)| name).chain([key_name(combo.key)]).collect::<Vec<_>>().join("+")
}

/// The keys as the person reads them; "Sem tecla" for none, and as written for what is no combination.
pub(crate) fn display(keys: &str) -> String {
    match parse_keys(keys) {
        Some(combo) => combo_label(&combo, false),
        None if keys.trim().is_empty() => "Sem tecla".into(),
        None => keys.into(),
    }
}

/// The Windows virtual key of `key`, for the keys a global shortcut may have: letters, digits, F1 to F12, the arrows
/// and the named keys below. None for the rest (Esc is never one: it cancels).
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

/// What AltGr types on the Brazilian ABNT2 keyboard, by key.
pub(super) const ABNT2_ALTGR: [(Key, &str); 10] = [
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
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Recorded {
    /// Esc alone: the recording ends and the keys stay as they were.
    Cancel,
    /// Enter alone: the combination pressed is saved.
    Save,
    /// Backspace or Delete alone: no key at all.
    Clear,
    /// Ctrl, Alt, Shift or the Windows key alone: the combination is still being pressed.
    Modifier,
    Keys(Combo),
}

/// What `key`, pressed with `modifiers` in the recorder, is. Whether the combination can be a shortcut is
/// `editor::check`'s.
pub(crate) fn record(key: Key, modifiers: Modifiers) -> Recorded {
    let bare = !modifiers.ctrl && !modifiers.alt && !modifiers.shift && !modifiers.command;

    match key {
        Key::Escape if bare => Recorded::Cancel,
        Key::Enter if bare => Recorded::Save,
        Key::Backspace | Key::Delete if bare => Recorded::Clear,
        key if is_modifier(key) => Recorded::Modifier,
        key => Recorded::Keys(pressed(key, modifiers)),
    }
}

/// The modifiers held while a combination is being pressed, as the recorder shows them ("Ctrl+Alt+…"); empty for none.
pub(crate) fn held_label(modifiers: Modifiers) -> String {
    let held = [(command_held(modifiers), "Ctrl"), (modifiers.alt, "Alt"), (modifiers.shift, "Shift")];
    let names: Vec<&str> = held.into_iter().filter(|(down, _)| *down).map(|(_, name)| name).collect();

    if names.is_empty() { String::new() } else { format!("{}+…", names.join("+")) }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn combo(ctrl: bool, alt: bool, shift: bool, key: Key) -> Combo {
        Combo { ctrl, alt, shift, win: false, key }
    }

    #[test]
    fn a_global_combination_needs_ctrl_alt_or_win_and_a_key_the_system_takes() {
        assert_eq!(parse(""), None);
        assert_eq!(parse("A"), None, "a key alone would take A from every program");
        assert_eq!(parse("Shift+A"), None, "and so would Shift+A");
        assert_eq!(parse("Ctrl+Alt"), None, "only modifiers");
        assert_eq!(parse("Ctrl+Escape"), None);
        assert_eq!(parse("Ctrl+Alt+Nada"), None);
        assert_eq!(parse("Ctrl+Hyper+A"), None);
        assert_eq!(parse("Ctrl+Shift+Questionmark"), None, "the system takes no key for a question mark");
        let parsed = parse("ctrl + alt + space").unwrap();

        assert_eq!(parsed, combo(true, true, false, Key::Space));
        assert_eq!(format(&parsed), "Ctrl+Alt+Space");
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
    }

    #[test]
    fn the_apps_combinations_take_any_key_and_a_symbol_is_the_key_under_it_with_shift() {
        assert_eq!(parse_keys("Shift+1"), Some(combo(false, false, true, Key::Num1)));
        assert_eq!(parse_keys("Delete"), Some(combo(false, false, false, Key::Delete)), "a key alone is a combination too");
        assert_eq!(parse_keys("Ctrl+,"), parse_keys("Cmd+Comma"));
        assert_eq!(parse_keys("Ctrl++"), Some(combo(true, false, true, Key::Equals)), "+ is Shift and =");
        assert_eq!(parse_keys("?"), parse_keys("Shift+Slash"));
        assert_eq!(parse_keys("Ctrl+Shift+}"), parse_keys("Ctrl+Shift+CloseBracket"));
        assert_eq!(parse_keys("Ctrl+ShiftLeft"), None, "a modifier is never the key");
        assert_eq!(parse_keys("Ctrl+"), None);
        assert_eq!(format(&parse_keys("Ctrl++").unwrap()), "Ctrl+Shift+Equals", "kept by the key's name");
        // the keys that come: "!" with Shift is Shift+1, "+" from the keypad has no Shift and is still Shift and =
        let shift = Modifiers::SHIFT;

        assert_eq!(pressed(Key::Exclamationmark, shift), parse_keys("Shift+1").unwrap());
        assert_eq!(pressed(Key::Plus, Modifiers::COMMAND), parse_keys("Ctrl++").unwrap());

        // off the Mac the app's Ctrl is either flag; on it only ⌘ is
        if !cfg!(target_os = "macos") {
            assert_eq!(pressed(Key::S, Modifiers::CTRL), pressed(Key::S, Modifiers::COMMAND), "Ctrl from either flag");
        }

        let event = |key, modifiers| egui::Event::Key { key, physical_key: None, pressed: true, repeat: false, modifiers };
        let save = parse_keys("Ctrl+S").unwrap();

        assert!(hits(&event(Key::S, Modifiers::COMMAND), &save));
        assert!(!hits(&event(Key::S, Modifiers::COMMAND | Modifiers::SHIFT), &save), "Ctrl+Shift+S is another one");
        assert!(!hits(&event(Key::S, Modifiers::NONE), &save));
    }

    #[test]
    fn the_keys_read_in_portuguese() {
        assert_eq!(display("Ctrl+Alt+Space"), "Ctrl+Alt+Espaço");
        assert_eq!(display("Ctrl+Alt+Right"), "Ctrl+Alt+Direita");
        assert_eq!(display("Ctrl+Shift+K"), "Ctrl+Shift+K");
        assert_eq!(display("Ctrl+Shift+Equals"), "Ctrl++");
        assert_eq!(display("Shift+Slash"), "?");
        assert_eq!(display("Shift+1"), "Shift+1");
        assert_eq!(display("Ctrl+Comma"), "Ctrl+,");
        assert_eq!(display("Ctrl+Minus"), "Ctrl+-");
        assert_eq!(display(""), "Sem tecla");
        assert_eq!(held_label(Modifiers { command: true, alt: true, ..Modifiers::NONE }), "Ctrl+Alt+…");
        assert_eq!(held_label(Modifiers::NONE), "");
    }

    #[test]
    fn the_recorder_reads_a_combination_and_its_own_keys() {
        let (ctrl_alt, none) = (Modifiers { command: true, alt: true, ..Modifiers::NONE }, Modifiers::NONE);

        assert_eq!(record(Key::Escape, none), Recorded::Cancel);
        assert_eq!(record(Key::Enter, none), Recorded::Save);
        assert_eq!(record(Key::Backspace, none), Recorded::Clear);
        assert_eq!(record(Key::ControlLeft, Modifiers::COMMAND), Recorded::Modifier, "Ctrl alone: still pressing");
        assert_eq!(record(Key::AltRight, ctrl_alt), Recorded::Modifier);
        assert_eq!(record(Key::K, none), Recorded::Keys(combo(false, false, false, Key::K)), "a key alone: check says why not");
        assert_eq!(record(Key::K, ctrl_alt), Recorded::Keys(combo(true, true, false, Key::K)));
        assert_eq!(record(Key::Escape, ctrl_alt), Recorded::Keys(combo(true, true, false, Key::Escape)));
        assert_eq!(record(Key::Enter, Modifiers::COMMAND), Recorded::Keys(combo(true, false, false, Key::Enter)));
        assert_eq!(record(Key::Exclamationmark, Modifiers::SHIFT), Recorded::Keys(combo(false, false, true, Key::Num1)));
    }

    #[test]
    fn altgr_on_abnt2_is_warned_of() {
        // Ctrl+Alt is AltGr: on ABNT2 AltGr+E types the degree sign
        let typed = altgr_warning("Ctrl+Alt+E").unwrap();

        assert!(typed.contains("ABNT2") && typed.contains("AltGr") && typed.contains("«°»"), "{typed}");
        assert_eq!(altgr_warning("Ctrl+Alt+K"), None, "AltGr+K types nothing there");
        assert_eq!(altgr_warning("Ctrl+Shift+E"), None, "not AltGr without Alt");
        assert_eq!(altgr_warning("Ctrl+Alt+Win+E"), None, "nor with the Windows key");
        assert!(ALTGR_NOTE.contains("ABNT2") && ALTGR_NOTE.contains("AltGr"));
    }
}
