//! Changing a shortcut's keys: what is pressed in Ajustes waits, shown, until Salvar (or Enter) applies it or Cancelar
//! (or Esc) drops it; Limpar leaves the shortcut without a key and Padrão brings its own back. Nothing is saved over
//! another shortcut that can be pressed at the same time (a global one is pressed everywhere), over the keys that never
//! change (`reserved`) nor over a combination that makes no shortcut; the person is told which and why.

use super::keys::{self, Combo, Recorded};
use super::{Act, Binding, Bindings, Scope, ShortcutDef};
use crate::prelude::*;

/// Why a combination can't be a shortcut's keys.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum Problem {
    /// Another shortcut that can be pressed at the same time has them: its id.
    Taken(&'static str),
    /// Keys that never change: what they do.
    Reserved(String),
    /// A key alone, or with Shift only, where Ctrl or Alt is needed.
    NeedsModifier,
    /// Ctrl with a letter, for a shortcut that answers inside a terminal and a text field too.
    Typing,
    /// A key the system takes for no global shortcut.
    Unsupported,
}

impl Problem {
    /// What Ajustes says of `combo` for `def`.
    pub(crate) fn words(&self, def: &ShortcutDef, combo: Option<&Combo>) -> String {
        let keys = combo.map_or_else(String::new, |combo| keys::combo_label(combo, def.scope() == Scope::App));
        match self {
            Problem::Taken(id) => {
                let other = super::find(id).map_or(*id, |other| other.label);
                let place = if super::find(id).is_some_and(|other| other.scope() == Scope::Global) { "o atalho global" } else { "o atalho" };
                format!("{keys} já é {place} «{other}»: escolha outra combinação ou limpe a de lá primeiro.")
            }
            Problem::Reserved(what) => format!("{keys} é reservada: {what}."),
            Problem::NeedsModifier if def.scope() == Scope::Global => {
                "Junte Ctrl ou Alt à tecla: sozinha, ela deixaria de funcionar nos outros programas.".into()
            }
            Problem::NeedsModifier => "Junte Ctrl, Alt ou Shift à tecla: sozinha, ela é das ferramentas e da digitação.".into(),
            Problem::Typing => "Ctrl com uma letra é do terminal e dos campos de texto, e este atalho vale até neles: junte Shift ou Alt.".into(),
            Problem::Unsupported => "Essa tecla não serve de atalho global.".into(),
        }
    }
}

/// The keys no shortcut can take, where they answer and what they do there: the system's (everywhere), the app's that
/// never change (the clipboard, Esc, Enter, Delete, the arrows, Space and the tools' letters and digits) and the open
/// island's.
pub(crate) fn reserved() -> Vec<(Combo, Scope, String)> {
    let mut all = Vec::new();
    let mut add = |keys: &str, scope: Scope, what: &str| {
        if let Some(combo) = keys::parse_keys(keys) {
            all.push((combo, scope, what.to_string()));
        }
    };
    for (keys, what) in [
        ("Alt+Tab", "o sistema troca de janela com ela"),
        ("Alt+Shift+Tab", "o sistema troca de janela com ela"),
        ("Alt+F4", "o sistema fecha a janela com ela"),
        ("Alt+Space", "o sistema abre o menu da janela com ela"),
        ("Ctrl+Alt+Delete", "é do sistema"),
    ] {
        add(keys, Scope::Global, what);
    }
    for (keys, what) in [
        ("Ctrl+C", "copiar"),
        ("Ctrl+X", "recortar"),
        ("Ctrl+V", "colar"),
        ("Escape", "cancelar e soltar a seleção"),
        ("Enter", "editar o texto ou entrar no terminal selecionado"),
        ("Delete", "apagar a seleção"),
        ("Backspace", "apagar a seleção"),
        ("Space", "mover a vista, segurando"),
        ("Shift+X", "a ferramenta Desenhar para criar forma"),
        ("9", "inserir imagem"),
    ] {
        add(keys, Scope::App, what);
    }
    for arrow in ["Left", "Right", "Up", "Down"] {
        add(arrow, Scope::App, "mover a seleção");
        add(&format!("Shift+{arrow}"), Scope::App, "mover a seleção");
    }
    for tool in Tool::TOOLBAR.into_iter().chain(Tool::DRAWING).chain(Tool::EXTRA) {
        for key in tool.keys() {
            add(key.name(), Scope::App, &format!("a ferramenta {}", tool.label()));
        }
    }
    for digit in 1..=9 {
        add(&format!("Ctrl+{digit}"), Scope::Island, "na ilha aberta, ir a uma pílula");
        add(&digit.to_string(), Scope::Island, "na ilha, escolher a opção da pergunta");
    }
    for (keys, what) in [
        ("Ctrl+O", "na ilha aberta, abrir a conversa no app"),
        ("Ctrl+K", "na ilha aberta, buscar"),
        ("Ctrl+Comma", "na ilha aberta, os ajustes dela"),
        ("Escape", "na ilha aberta, recolher"),
        ("Y", "na ilha, permitir"),
        ("N", "na ilha, negar"),
    ] {
        add(keys, Scope::Island, what);
    }
    all
}

/// Whether `combo` can be the keys of `def`, with the rest as kept in `stored`.
pub(crate) fn check(def: &ShortcutDef, combo: &Combo, stored: &Bindings) -> Result<(), Problem> {
    let scope = def.scope();
    if scope == Scope::Global && keys::virtual_key(combo.key).is_none() {
        return Err(Problem::Unsupported);
    }
    if let Some((_, _, what)) = reserved().into_iter().find(|(keys, place, _)| keys == combo && place.overlaps(scope)) {
        return Err(Problem::Reserved(what));
    }
    let taken = super::every().find(|other| other.id != def.id && other.scope().overlaps(scope) && super::keys_of(other, stored).contains(combo));
    if let Some(other) = taken {
        return Err(Problem::Taken(other.id));
    }
    let name = combo.key.name();
    let function_key = name.strip_prefix('F').is_some_and(|digits| digits.parse::<u8>().is_ok());
    let letter = matches!(name.as_bytes(), [byte] if byte.is_ascii_uppercase());
    match def.act {
        Act::Global(_) if !combo.has_command() => Err(Problem::NeedsModifier),
        Act::App(_) if !combo.has_command() && !combo.shift && !function_key => Err(Problem::NeedsModifier),
        Act::App(action) if action.everywhere() && combo.ctrl && !combo.alt && !combo.shift && letter => Err(Problem::Typing),
        _ => Ok(()),
    }
}

/// What waits to be saved.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Pending {
    Keys(Combo),
    /// No key at all (Backspace in the recorder).
    Clear,
    /// The shortcut's own keys back (Padrão, refused for a conflict).
    Default,
}

/// Applies `choice` to `def` in `stored`, unless it clashes: a combination recorded turns the shortcut on.
pub(crate) fn apply(def: &ShortcutDef, choice: Pending, stored: &mut Bindings) -> Result<(), Problem> {
    match choice {
        Pending::Keys(combo) => {
            check(def, &combo, stored)?;
            super::set_binding(stored, def, Binding { keys: keys::format(&combo), enabled: true });
        }
        Pending::Clear => {
            let enabled = super::enabled(def, stored);
            super::set_binding(stored, def, Binding { keys: String::new(), enabled });
        }
        Pending::Default => {
            for combo in def.defaults.iter().filter_map(|keys| keys::parse_keys(keys)) {
                check(def, &combo, stored)?;
            }
            stored.remove(def.id);
        }
    }
    Ok(())
}

/// A shortcut whose keys are being changed in Ajustes.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Editing {
    pub(crate) id: &'static str,
    /// What was pressed and waits for Salvar; None while the keys are still awaited.
    pub(crate) pending: Option<Pending>,
    /// Why the pending keys can't be saved.
    pub(crate) problem: Option<Problem>,
}

impl Editing {
    pub(crate) fn new(id: &'static str) -> Self {
        Self { id, pending: None, problem: None }
    }

    /// The shortcut being changed.
    pub(crate) fn def(&self) -> Option<&'static ShortcutDef> {
        super::find(self.id)
    }

    /// A key pressed while recording, in `stored`: true when it ended the change (saved or dropped).
    pub(crate) fn press(&mut self, recorded: Recorded, stored: &mut Bindings) -> bool {
        let Some(def) = self.def() else { return true };
        match recorded {
            Recorded::Cancel => return true,
            Recorded::Save => return self.save(stored),
            Recorded::Modifier => {}
            Recorded::Clear => (self.pending, self.problem) = (Some(Pending::Clear), None),
            Recorded::Keys(combo) => (self.pending, self.problem) = (Some(Pending::Keys(combo)), check(def, &combo, stored).err()),
        }
        false
    }

    /// Salvar: the pending keys applied, true when they were; else why not stays to be read.
    pub(crate) fn save(&mut self, stored: &mut Bindings) -> bool {
        let (Some(def), Some(pending)) = (self.def(), self.pending) else { return false };
        match apply(def, pending, stored) {
            Ok(()) => true,
            Err(problem) => {
                self.problem = Some(problem);
                false
            }
        }
    }

    /// What Ajustes says under the shortcut while it changes: the problem, else how to go on.
    pub(crate) fn hint(&self) -> String {
        let Some(def) = self.def() else { return String::new() };
        let combo = match self.pending {
            Some(Pending::Keys(combo)) => Some(combo),
            Some(Pending::Default) => def.defaults.first().and_then(|keys| keys::parse_keys(keys)),
            _ => None,
        };
        match (&self.problem, self.pending) {
            (Some(problem), _) => problem.words(def, combo.as_ref()),
            (None, None) => "Pressione a combinação. Esc cancela e Backspace deixa sem tecla.".into(),
            (None, Some(_)) => "Salvar (ou Enter) aplica; Cancelar (ou Esc) desiste.".into(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::super::{enabled, find, keys_of, shown_keys};
    use super::*;

    fn combo(text: &str) -> Combo {
        keys::parse_keys(text).unwrap_or_else(|| panic!("{text}"))
    }

    fn check_of(id: &str, keys: &str, stored: &Bindings) -> Result<(), Problem> {
        check(find(id).unwrap(), &combo(keys), stored)
    }

    #[test]
    fn keys_another_shortcut_has_are_refused_saying_whose() {
        let mut stored = Bindings::new();
        assert_eq!(check_of("save", "Ctrl+S", &stored), Ok(()), "its own keys");
        assert_eq!(check_of("save", "Ctrl+Z", &stored), Err(Problem::Taken("undo")));
        assert_eq!(check_of("save", "Ctrl+Y", &stored), Err(Problem::Taken("redo")), "a second default is taken too");
        assert_eq!(check_of("save", "Ctrl+Alt+A", &stored), Err(Problem::Taken("goToAlert")), "a global one is pressed in the app too");
        assert_eq!(check_of("openChat", "Ctrl+S", &stored), Err(Problem::Taken("save")), "and takes the app's keys everywhere");
        assert_eq!(check_of("openChat", "Ctrl+Alt+T", &stored), Err(Problem::Taken("jumpToTerminal")));
        assert_eq!(check_of("zoomIn", "Ctrl+Shift+Equals", &stored), Ok(()));
        assert_eq!(check_of("save", "Ctrl++", &stored), Err(Problem::Taken("zoomIn")), "+ is Shift and =");
        // a global one turned off keeps its keys; one cleared lets them go
        stored.insert("goToAlert".into(), Binding { keys: "Ctrl+Alt+A".into(), enabled: false });
        assert_eq!(check_of("save", "Ctrl+Alt+A", &stored), Err(Problem::Taken("goToAlert")));
        apply(find("undo").unwrap(), Pending::Clear, &mut stored).unwrap();
        assert_eq!(check_of("save", "Ctrl+Z", &stored), Ok(()));
        let words = Problem::Taken("goToAlert").words(find("save").unwrap(), Some(&combo("Ctrl+Alt+A")));
        assert_eq!(words, "Ctrl+Alt+A já é o atalho global «Ir à pergunta ou permissão pendente»: escolha outra combinação ou limpe a de lá primeiro.");
    }

    #[test]
    fn the_keys_that_never_change_and_the_systems_are_refused() {
        let stored = Bindings::new();
        assert_eq!(check_of("save", "Ctrl+C", &stored), Err(Problem::Reserved("copiar".into())));
        assert_eq!(check_of("openChat", "Ctrl+V", &stored), Err(Problem::Reserved("colar".into())), "a global one would take it from the app");
        assert_eq!(check_of("save", "V", &stored), Err(Problem::Reserved("a ferramenta Seleção".into())), "the canvas's tools");
        assert_eq!(check_of("save", "W", &stored), Err(Problem::Reserved("a ferramenta Navegador".into())));
        assert_eq!(check_of("save", "Shift+X", &stored), Err(Problem::Reserved("a ferramenta Desenhar para criar forma".into())));
        assert_eq!(check_of("save", "Shift+Left", &stored), Err(Problem::Reserved("mover a seleção".into())));
        assert_eq!(check_of("openChat", "Alt+Tab", &stored), Err(Problem::Reserved("o sistema troca de janela com ela".into())));
        assert_eq!(check_of("save", "Alt+F4", &stored), Err(Problem::Reserved("o sistema fecha a janela com ela".into())));
        // the open island's keys: only a global shortcut is pressed there too
        assert_eq!(check_of("openChat", "Ctrl+K", &stored), Err(Problem::Reserved("na ilha aberta, buscar".into())));
        assert_eq!(check_of("save", "Ctrl+K", &stored), Ok(()), "the island is another window");
        let words = Problem::Reserved("copiar".into()).words(find("save").unwrap(), Some(&combo("Ctrl+C")));
        assert_eq!(words, format!("{} é reservada: copiar.", if cfg!(target_os = "macos") { "Cmd+C" } else { "Ctrl+C" }));
    }

    #[test]
    fn a_combination_that_makes_no_shortcut_is_refused() {
        let stored = Bindings::new();
        // a global one needs Ctrl, Alt or Win, and a key the system takes
        assert_eq!(check_of("openChat", "J", &stored), Err(Problem::NeedsModifier));
        assert_eq!(check_of("openChat", "Shift+J", &stored), Err(Problem::NeedsModifier));
        assert_eq!(check_of("openChat", "Ctrl+Alt+?", &stored), Err(Problem::Unsupported));
        assert_eq!(check_of("openChat", "Ctrl+Alt+J", &stored), Ok(()));
        // the app's: a modifier, Shift included, or a function key alone
        assert_eq!(check_of("save", "J", &stored), Err(Problem::NeedsModifier));
        assert_eq!(check_of("save", "Shift+J", &stored), Ok(()));
        assert_eq!(check_of("save", "F2", &stored), Ok(()));
        // one that answers inside a terminal too leaves Ctrl and a letter to it
        assert_eq!(check_of("showChat", "Ctrl+J", &stored), Err(Problem::Typing));
        assert_eq!(check_of("showChat", "Ctrl+Shift+J", &stored), Ok(()));
        assert_eq!(check_of("save", "Ctrl+J", &stored), Ok(()), "the canvas's own are not read in a terminal");
        let global = find("openChat").unwrap();
        assert!(Problem::NeedsModifier.words(global, None).starts_with("Junte Ctrl ou Alt"));
        assert!(Problem::NeedsModifier.words(find("save").unwrap(), None).starts_with("Junte Ctrl, Alt ou Shift"));
    }

    #[test]
    fn a_combination_pressed_waits_for_salvar_and_esc_drops_it() {
        let mut stored = Bindings::new();
        let mut editing = Editing::new("save");
        assert!(editing.hint().starts_with("Pressione a combinação"));
        assert!(!editing.press(Recorded::Modifier, &mut stored), "Ctrl alone: still pressing");
        assert_eq!(editing.pending, None);
        assert!(!editing.press(Recorded::Keys(combo("Ctrl+Alt+F5")), &mut stored));
        assert_eq!((editing.pending, &editing.problem), (Some(Pending::Keys(combo("Ctrl+Alt+F5"))), &None));
        assert!(editing.hint().starts_with("Salvar (ou Enter) aplica"));
        assert!(stored.is_empty(), "nothing saved yet");
        // Esc: dropped, the keys stay as they were
        assert!(editing.press(Recorded::Cancel, &mut stored));
        assert!(stored.is_empty());
        // Enter (or Salvar): applied
        let mut editing = Editing::new("save");
        editing.press(Recorded::Keys(combo("Ctrl+Alt+F5")), &mut stored);
        assert!(editing.press(Recorded::Save, &mut stored));
        assert_eq!(shown_keys(find("save").unwrap(), &stored), "Ctrl+Alt+F5");
        // a combination of another one waits with why, and Salvar saves nothing
        let mut editing = Editing::new("save");
        editing.press(Recorded::Keys(combo("Ctrl+Z")), &mut stored);
        assert_eq!(editing.problem, Some(Problem::Taken("undo")));
        assert!(editing.hint().contains("«Desfazer»"), "{}", editing.hint());
        assert!(!editing.save(&mut stored));
        assert_eq!(shown_keys(find("save").unwrap(), &stored), "Ctrl+Alt+F5", "unchanged");
        // another combination pressed takes its place
        editing.press(Recorded::Keys(combo("Ctrl+Alt+F6")), &mut stored);
        assert_eq!(editing.problem, None);
        assert!(editing.save(&mut stored));
        assert_eq!(shown_keys(find("save").unwrap(), &stored), "Ctrl+Alt+F6");
    }

    #[test]
    fn limpar_leaves_no_key_and_padrao_brings_the_default_back_unless_another_took_it() {
        let mut stored = Bindings::new();
        let (save, undo, chat) = (find("save").unwrap(), find("undo").unwrap(), find("openChat").unwrap());
        apply(save, Pending::Clear, &mut stored).unwrap();
        assert!(keys_of(save, &stored).is_empty());
        assert_eq!(shown_keys(save, &stored), "Sem tecla");
        // Backspace in the recorder, then Salvar, does the same
        let mut editing = Editing::new("openChat");
        editing.press(Recorded::Clear, &mut stored);
        assert_eq!(editing.pending, Some(Pending::Clear));
        assert!(editing.save(&mut stored));
        assert!(keys_of(chat, &stored).is_empty() && enabled(chat, &stored), "without a key, still on");
        // Ctrl+S is free: Desfazer takes it, and Salvar's default can't come back over it
        apply(undo, Pending::Keys(combo("Ctrl+S")), &mut stored).unwrap();
        assert_eq!(apply(save, Pending::Default, &mut stored), Err(Problem::Taken("undo")));
        assert!(keys_of(save, &stored).is_empty(), "unchanged");
        apply(undo, Pending::Default, &mut stored).unwrap();
        apply(save, Pending::Default, &mut stored).unwrap();
        apply(chat, Pending::Default, &mut stored).unwrap();
        assert!(stored.is_empty(), "every default: nothing kept");
        // a recorded combination turns a global one back on
        stored.insert("openChat".into(), Binding { keys: "Ctrl+Alt+Space".into(), enabled: false });
        apply(chat, Pending::Keys(combo("Ctrl+Alt+J")), &mut stored).unwrap();
        assert!(enabled(chat, &stored));
    }
}
