//! The app's own shortcuts: they answer in its window, read from the keys of each frame (`canvas/shortcuts.rs`, and
//! `ui/agents.rs` for the ones that answer even with a terminal or a field taking the keys). Each one's keys are set,
//! or cleared, in Ajustes › Atalhos like the global ones; the keys that never change are `editor::reserved`.

use super::{Act, ShortcutDef, keys};
use crate::prelude::*;

/// What one of the app's shortcuts does.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum AppAction {
    Settings,
    Palette,
    ShowChat,
    ShowCanvas,
    OpenTree,
    NextNeedingYou,
    Floors,
    Grid,
    Theme,
    Save,
    SaveAs,
    Open,
    Export,
    Undo,
    Redo,
    SelectAll,
    Duplicate,
    ClearCanvas,
    Group,
    Ungroup,
    Lock,
    Tidy,
    EditPoints,
    LayerForward,
    LayerBackward,
    LayerFront,
    LayerBack,
    ZoomIn,
    ZoomOut,
    ZoomReset,
    FitAll,
    FitSelection,
}

impl AppAction {
    /// It answers even with a terminal, a chat or a field taking the keys: it is read before them.
    pub(crate) fn everywhere(self) -> bool {
        matches!(self, AppAction::Settings | AppAction::ShowChat | AppAction::ShowCanvas | AppAction::OpenTree | AppAction::NextNeedingYou)
    }
}

const fn app(action: AppAction, id: &'static str, group: &'static str, label: &'static str, defaults: &'static [&'static str]) -> ShortcutDef {
    ShortcutDef { act: Act::App(action), id, group, label, defaults }
}

/// The app's shortcuts in the order Ajustes lists them, by group. The ids are kept in prefs.json (never rename one);
/// the keys are written as they are kept (`keys::format`).
pub(crate) static APP: [ShortcutDef; 32] = [
    app(AppAction::Settings, "settings", "GERAL", "Ajustes", &["Ctrl+Comma"]),
    app(AppAction::Palette, "palette", "GERAL", "Paleta de comandos", &["Ctrl+Slash", "Shift+Slash"]),
    app(AppAction::ShowChat, "showChat", "GERAL", "Tela do chat", &["Ctrl+1"]),
    app(AppAction::ShowCanvas, "showCanvas", "GERAL", "Voltar ao canvas", &["Ctrl+2"]),
    app(AppAction::OpenTree, "openTree", "GERAL", "Abrir a Árvore de Arquivos", &["Ctrl+3"]),
    app(AppAction::NextNeedingYou, "nextNeedingYou", "GERAL", "Próximo agente que precisa de você", &["Ctrl+Shift+A"]),
    app(AppAction::Floors, "floors", "GERAL", "Visão dos andares", &["Ctrl+Shift+Backslash"]),
    app(AppAction::Grid, "grid", "GERAL", "Mostrar ou esconder a grade", &["Ctrl+Quote"]),
    app(AppAction::Theme, "theme", "GERAL", "Tema claro ou escuro", &["Alt+Shift+D"]),
    app(AppAction::Save, "save", "ARQUIVO", "Salvar", &["Ctrl+S"]),
    app(AppAction::SaveAs, "saveAs", "ARQUIVO", "Salvar como", &["Ctrl+Shift+S"]),
    app(AppAction::Open, "open", "ARQUIVO", "Abrir", &["Ctrl+O"]),
    app(AppAction::Export, "export", "ARQUIVO", "Exportar imagem", &["Ctrl+Shift+E"]),
    app(AppAction::Undo, "undo", "EDIÇÃO", "Desfazer", &["Ctrl+Z"]),
    app(AppAction::Redo, "redo", "EDIÇÃO", "Refazer", &["Ctrl+Shift+Z", "Ctrl+Y"]),
    app(AppAction::SelectAll, "selectAll", "EDIÇÃO", "Selecionar tudo", &["Ctrl+A"]),
    app(AppAction::Duplicate, "duplicate", "EDIÇÃO", "Duplicar", &["Ctrl+D"]),
    app(AppAction::ClearCanvas, "clearCanvas", "EDIÇÃO", "Limpar o canvas", &["Ctrl+Delete", "Ctrl+Backspace"]),
    app(AppAction::Group, "group", "EDIÇÃO", "Agrupar", &["Ctrl+G"]),
    app(AppAction::Ungroup, "ungroup", "EDIÇÃO", "Desagrupar", &["Ctrl+Shift+G"]),
    app(AppAction::Lock, "lock", "EDIÇÃO", "Travar ou destravar", &["Ctrl+Shift+L"]),
    app(AppAction::Tidy, "tidy", "EDIÇÃO", "Arrumar a seleção", &["Ctrl+Shift+T"]),
    app(AppAction::EditPoints, "editPoints", "EDIÇÃO", "Editar os pontos da linha", &["Ctrl+Enter"]),
    app(AppAction::LayerForward, "layerForward", "EDIÇÃO", "Uma camada à frente", &["Ctrl+CloseBracket"]),
    app(AppAction::LayerBackward, "layerBackward", "EDIÇÃO", "Uma camada atrás", &["Ctrl+OpenBracket"]),
    app(AppAction::LayerFront, "layerFront", "EDIÇÃO", "Trazer para a frente de tudo", &["Ctrl+Shift+CloseBracket", "Ctrl+Alt+CloseBracket"]),
    app(AppAction::LayerBack, "layerBack", "EDIÇÃO", "Enviar para trás de tudo", &["Ctrl+Shift+OpenBracket", "Ctrl+Alt+OpenBracket"]),
    app(AppAction::ZoomIn, "zoomIn", "VISTA", "Aproximar", &["Ctrl+Shift+Equals", "Ctrl+Equals"]),
    app(AppAction::ZoomOut, "zoomOut", "VISTA", "Afastar", &["Ctrl+Minus"]),
    app(AppAction::ZoomReset, "zoomReset", "VISTA", "Voltar a 100%", &["Ctrl+0"]),
    app(AppAction::FitAll, "fitAll", "VISTA", "Ver tudo", &["Shift+1"]),
    app(AppAction::FitSelection, "fitSelection", "VISTA", "Ver a seleção", &["Shift+2"]),
];

/// The shortcut of `action`.
pub(crate) fn def_of(action: AppAction) -> &'static ShortcutDef {
    APP.iter().find(|def| def.act == Act::App(action)).unwrap_or(&APP[0])
}

impl DrawingApp {
    /// The app's shortcuts pressed among `events`, in `APP`'s order.
    pub(crate) fn app_keys(&self, events: &[egui::Event]) -> Vec<AppAction> {
        if !events.iter().any(|event| matches!(event, egui::Event::Key { pressed: true, .. })) {
            return Vec::new();
        }
        APP.iter()
            .filter(|def| super::keys_of(def, &self.prefs.shortcuts).iter().any(|combo| events.iter().any(|event| keys::hits(event, combo))))
            .filter_map(|def| match def.act {
                Act::App(action) => Some(action),
                Act::Global(_) => None,
            })
            .collect()
    }

    /// Takes the presses of `action` out of this frame's keys, so whatever has the keyboard does not get them too:
    /// true when there was one.
    pub(crate) fn take_app_key(&self, ctx: &Context, action: AppAction) -> bool {
        let combos = super::keys_of(def_of(action), &self.prefs.shortcuts);
        if combos.is_empty() {
            return false;
        }
        ctx.input_mut(|input| {
            let before = input.events.len();
            input.events.retain(|event| !combos.iter().any(|combo| keys::hits(event, combo)));
            input.events.len() != before
        })
    }

    /// The first keys of `action` as Ajustes shows them ("Ctrl+S"), for the palette and the tips; empty without a key.
    pub(crate) fn app_keys_text(&self, action: AppAction) -> String {
        let combos = super::keys_of(def_of(action), &self.prefs.shortcuts);
        combos.first().map(|combo| keys::combo_label(combo, true)).unwrap_or_default()
    }

    /// "Diminuir zoom — Ctrl+-": what a button does and the keys of `action`, when it has any.
    pub(crate) fn tip_with_keys(&self, what: &str, action: AppAction) -> String {
        match self.app_keys_text(action) {
            keys if keys.is_empty() => what.to_string(),
            keys => format!("{what} — {keys}"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn key(key: Key, modifiers: Modifiers) -> egui::Event {
        egui::Event::Key { key, physical_key: None, pressed: true, repeat: false, modifiers }
    }

    #[test]
    fn every_default_is_written_as_it_is_kept_and_the_ids_are_unique() {
        for def in &APP {
            for keys in def.defaults {
                let combo = keys::parse_keys(keys).unwrap_or_else(|| panic!("{}: {keys}", def.id));
                assert_eq!(keys::format(&combo), *keys, "{}", def.id);
            }
            assert_eq!(super::super::every().filter(|other| other.id == def.id).count(), 1, "{} twice", def.id);
        }
    }

    #[test]
    fn the_keys_pressed_are_the_shortcuts_set_and_a_cleared_one_answers_to_nothing() {
        let mut app = DrawingApp::default();
        let command = Modifiers::COMMAND;
        assert_eq!(app.app_keys(&[key(Key::S, command)]), [AppAction::Save]);
        assert_eq!(app.app_keys(&[key(Key::S, command | Modifiers::SHIFT)]), [AppAction::SaveAs]);
        assert_eq!(app.app_keys(&[key(Key::Y, command)]), [AppAction::Redo], "the second default answers too");
        assert_eq!(app.app_keys(&[key(Key::Questionmark, Modifiers::SHIFT)]), [AppAction::Palette]);
        assert_eq!(app.app_keys(&[key(Key::Plus, command)]), [AppAction::ZoomIn], "+ from the keypad, without Shift");
        assert_eq!(app.app_keys(&[key(Key::Exclamationmark, Modifiers::SHIFT)]), [AppAction::FitAll], "Shift+1 comes as !");
        assert_eq!(app.app_keys(&[key(Key::Pipe, command | Modifiers::SHIFT)]), [AppAction::Floors]);
        assert!(app.app_keys(&[key(Key::S, Modifiers::NONE)]).is_empty());
        app.prefs.shortcuts.insert("save".into(), super::super::Binding { keys: "Ctrl+Shift+F5".into(), enabled: true });
        assert!(app.app_keys(&[key(Key::S, command)]).is_empty(), "Ctrl+S is free now");
        assert_eq!(app.app_keys(&[key(Key::F5, command | Modifiers::SHIFT)]), [AppAction::Save]);
        assert_eq!(app.app_keys_text(AppAction::Save), "Ctrl+Shift+F5");
        app.prefs.shortcuts.insert("save".into(), super::super::Binding { keys: String::new(), enabled: true });
        assert!(app.app_keys(&[key(Key::F5, command | Modifiers::SHIFT)]).is_empty());
        assert_eq!(app.app_keys_text(AppAction::Save), "");
        assert_eq!(app.tip_with_keys("Salvar", AppAction::Save), "Salvar");
        assert_eq!(
            app.tip_with_keys("Aumentar zoom", AppAction::ZoomIn),
            format!("Aumentar zoom — {}", if cfg!(target_os = "macos") { "Cmd++" } else { "Ctrl++" })
        );
    }
}
