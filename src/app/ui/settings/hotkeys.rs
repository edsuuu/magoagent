//! Ajustes › Atalhos: every shortcut whose keys can change (app/hotkeys/), by group, with a search over them. A click
//! on a shortcut's keys records others, which wait, shown, for Salvar (or Enter) or Cancelar (or Esc), and are refused,
//! saying whose they are, when another shortcut or a fixed key has them. Each one also has Limpar (no key) and Padrão
//! (its own keys back); a global one has its switch.

use super::kit::*;
use crate::app::hotkeys::editor::{Editing, Pending};
use crate::app::hotkeys::keys::{self, Recorded};
use crate::app::hotkeys::{self, ACTIONS, ALTGR_NOTE, Binding, GLOBAL_GROUP, Scope, ShortcutDef, Status, altgr_warning, editor};
use crate::prelude::*;

/// What a shortcut's row was asked to do.
#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum Ask {
    Record,
    Save,
    Cancel,
    Clear,
    Default,
    Switch(bool),
}

/// Whether a shortcut answers to the search: its name or its keys as shown, any case.
fn shortcut_matches(label: &str, keys: &str, query: &str) -> bool {
    let query = query.trim().to_lowercase();

    query.is_empty() || label.to_lowercase().contains(&query) || keys.to_lowercase().contains(&query)
}

/// The keys no shortcut takes, as the page tells them (written the Mac way: `keys_label` turns them into Ctrl).
pub(super) const FIXED: &str = "Não mudam, e nenhum atalho pode usá-las: as letras e os números das ferramentas (V, H, C, G, W, N…; as de desenho só \
                     com a fileira de desenho aberta), Esc, Enter, Delete, as setas, o Espaço, Cmd+C, Cmd+X e Cmd+V; na ilha aberta, Cmd+1 a 9, \
                     Cmd+O, Cmd+K, Cmd+, e Esc; e as do sistema, como Alt+Tab e Alt+F4. Dentro de um terminal, as teclas são dele.";

impl DrawingApp {
    /// The page: the search, then each group's shortcuts (the global ones first) and the keys that never change.
    pub(super) fn settings_hotkeys(&mut self, ui: &mut egui::Ui, k: &K) {
        // before anything is drawn: the recorder takes this frame's keys, and a button never sees Enter or Space
        self.record_hotkey(ui);
        let id = egui::Id::unique("settings_shortcut_search");
        let mut query = ui.ctx().data(|data| data.get_temp::<String>(id)).unwrap_or_default();
        let width = ui.available_width();

        input(ui, k, id.with("field"), "Buscar atalho", &mut query, "Buscar pelo nome ou pela tecla", width, false);
        ui.ctx().data_mut(|data| data.insert_temp(id, query.clone()));
        ui.add_space(16.25);
        let mut groups: Vec<&str> = hotkeys::every().map(|def| def.group).collect();

        groups.dedup();
        let mut shown = 0;

        for group in groups {
            let rows: Vec<&'static ShortcutDef> = hotkeys::every()
                .filter(|def| def.group == group && shortcut_matches(def.label, &hotkeys::shown_keys(def, &self.prefs.shortcuts), &query))
                .collect();

            if rows.is_empty() {
                continue;
            }

            if shown > 0 {
                divider(ui, k);
            }

            shown += rows.len();
            title(ui, k, group);

            if group == GLOBAL_GROUP {
                note(ui, k, "Valem com qualquer programa na frente. Clique nas teclas de um atalho para gravar outras.");

                if !self.prefs.island {
                    note(ui, k, "A ilha está desligada: eles voltam a valer quando ela for ligada em Ajustes › Geral.");
                }
            }

            for def in rows {
                self.shortcut_row(ui, k, def);
            }

            if group == GLOBAL_GROUP && self.hotkeys.statuses.contains(&Status::Unsupported) {
                note(ui, k, Status::Unsupported.label());
            }
        }

        if shown == 0 {
            note(ui, k, "Nenhum atalho com esse nome ou essa tecla.");
        }

        divider(ui, k);
        title(ui, k, "TECLAS FIXAS");
        note(ui, k, &keys_label(FIXED));
        divider(ui, k);
    }

    /// One shortcut: its name and how it stands at the left; at the right its keys (a click records others), Limpar
    /// and Padrão, and a global one's switch. While it changes: what was pressed, Salvar and Cancelar.
    fn shortcut_row(&mut self, ui: &mut egui::Ui, k: &K, def: &'static ShortcutDef) {
        let stored = &self.prefs.shortcuts;
        let editing = self.hotkeys.recording.clone().filter(|editing| editing.id == def.id);
        let global = def.scope() == Scope::Global;
        let app = def.scope() == Scope::App;

        let hint = match &editing {
            Some(editing) => {
                let warning = match editing.pending {
                    None if global => ALTGR_NOTE.to_string(),
                    Some(Pending::Keys(combo)) if global && editing.problem.is_none() => altgr_warning(&keys::format(&combo)).unwrap_or_default(),
                    _ => String::new(),
                };

                format!("{} {warning}", editing.hint()).trim().to_string()
            }
            None if global => {
                let index = ACTIONS.iter().position(|other| other.id == def.id);
                let status = index.and_then(|index| self.hotkeys.statuses.get(index)).map_or("", |status| status.label());
                let binding = hotkeys::effective(def, stored);
                let warning = altgr_warning(&binding.keys).filter(|_| binding.enabled).unwrap_or_default();

                format!("{status} {warning}").trim().to_string()
            }
            None if matches!(def.act, hotkeys::Act::App(action) if action.everywhere()) => "Vale até com o teclado num terminal ou num chat.".into(),
            None => String::new(),
        };

        let keys_text = match editing.as_ref().map(|editing| editing.pending) {
            Some(Some(Pending::Keys(combo))) => keys::combo_label(&combo, app),
            Some(Some(Pending::Clear)) => keys::display(""),
            Some(Some(Pending::Default)) => hotkeys::shown_keys(def, &hotkeys::Bindings::new()),
            Some(None) => match keys::held_label(ui.input(|input| input.modifiers)) {
                held if held.is_empty() => "Pressione as teclas…".to_string(),
                held => keys_label(&held),
            },
            None => hotkeys::shown_keys(def, stored),
        };

        let (has_keys, changed) = (!hotkeys::keys_of(def, stored).is_empty(), stored.contains_key(def.id));
        let mut on = hotkeys::enabled(def, stored);

        let ask = row(ui, k, def.label, &hint, |ui| {
            let mut ask = None;

            // from the right edge inwards
            if let Some(editing) = &editing {
                if button(ui, k, "Cancelar").clicked() {
                    ask = Some(Ask::Cancel);
                }

                if editing.pending.is_some() && save_button(ui, k, editing.problem.is_none()).clicked() {
                    ask = Some(Ask::Save);
                }

                let edge = if editing.problem.is_some() { k.destructive } else { k.primary };

                keys_button(ui, k, &keys_text, Some(edge));

                return ask;
            }

            if global {
                let (rect, _) = ui.allocate_exact_size(Vec2::new(38.0, 22.0), Sense::hover());

                if switch(ui, k, rect.min, def.label, &mut on) {
                    ask = Some(Ask::Switch(on));
                }
            }

            if ghost_button(ui, k, Icon::Restart, &format!("Voltar ao padrão: {}", def.label), changed, false).tip("Voltar ao padrão").clicked() {
                ask = Some(Ask::Default);
            }

            if ghost_button(ui, k, Icon::Trash, &format!("Limpar: {}", def.label), has_keys, true).tip("Limpar: deixar sem tecla").clicked() {
                ask = Some(Ask::Clear);
            }

            let keys = keys_button(ui, k, &keys_text, None).tip("Clique para gravar outras teclas");

            if keys.clicked() {
                keys.surrender_focus();
                ask = Some(Ask::Record);
            }

            ask
        });

        if let Some(ask) = ask {
            self.hotkey_ask(def, ask);
        }
    }

    /// What a shortcut's row asked for (the egui page's buttons and the native one's), applied.
    pub(super) fn hotkey_ask(&mut self, def: &'static ShortcutDef, ask: Ask) {
        let stored = &mut self.prefs.shortcuts;

        match ask {
            Ask::Record => self.hotkeys.recording = Some(Editing::new(def.id)),
            Ask::Cancel => self.hotkeys.recording = None,
            Ask::Save => {
                if let Some(editing) = self.hotkeys.recording.as_mut()
                    && editing.save(stored)
                {
                    self.hotkeys.recording = None;
                }
            }
            Ask::Clear => {
                let _ = editor::apply(def, Pending::Clear, stored);

                self.hotkeys.recording = None;
            }
            // the default keys back, unless another shortcut took them since: then they wait, with whose they are
            Ask::Default => {
                self.hotkeys.recording = editor::apply(def, Pending::Default, stored).err().map(|problem| Editing {
                    id: def.id,
                    pending: Some(Pending::Default),
                    problem: Some(problem),
                });
            }
            Ask::Switch(on) => {
                let binding = Binding { enabled: on, ..hotkeys::effective(def, stored) };

                hotkeys::set_binding(stored, def, binding);
            }
        }
    }

    /// The keys pressed while a shortcut changes: taken out of the frame, each goes to the recorder (Copiar, Recortar
    /// and Colar come as their own events, and are told apart as Ctrl+C, Ctrl+X and Ctrl+V).
    fn record_hotkey(&mut self, ui: &egui::Ui) {
        let Some(mut editing) = self.hotkeys.recording.take() else { return };

        let taken: Vec<egui::Event> = ui.ctx().input_mut(|input| {
            let (keys, rest) = std::mem::take(&mut input.events).into_iter().partition(|event| {
                matches!(event, egui::Event::Key { .. } | egui::Event::Copy | egui::Event::Cut | egui::Event::Paste(_) | egui::Event::PasteImage(_))
            });

            input.events = rest;
            keys
        });

        for event in taken {
            let clipboard = |keys: &str| keys::parse_keys(keys).map(Recorded::Keys);

            let recorded = match event {
                egui::Event::Key { key, pressed: true, repeat: false, modifiers, .. } => Some(keys::record(key, modifiers)),
                egui::Event::Copy => clipboard("Ctrl+C"),
                egui::Event::Cut => clipboard("Ctrl+X"),
                egui::Event::Paste(_) | egui::Event::PasteImage(_) => clipboard("Ctrl+V"),
                _ => None,
            };

            if let Some(recorded) = recorded
                && editing.press(recorded, &mut self.prefs.shortcuts)
            {
                return;
            }
        }

        self.hotkeys.recording = Some(editing);
    }
}

/// A shortcut's keys as a button 30 high, at least 96 wide: a click records others. `edge` is its line while it
/// changes (the accent; red while what was pressed can't be saved).
fn keys_button(ui: &mut egui::Ui, k: &K, text: &str, edge: Option<Color32>) -> egui::Response {
    let none = text == keys::display("") || text.ends_with('…');
    let color = if none { k.muted_fg } else { k.fg };
    let galley = ui.painter().layout_no_wrap(text.into(), FontId::proportional(12.5), color);
    let (rect, response) = ui.allocate_exact_size(Vec2::new((galley.size().x + 24.0).max(96.0), 30.0), Sense::click());
    let response = response.on_hover_cursor(CursorIcon::PointingHand);

    response.widget_info(|| egui::WidgetInfo::labeled(egui::Role::Button, true, text));
    let fill = if response.hovered() { k.button.lerp_to_gamma(k.fg, 0.06) } else { k.button };

    ui.painter().rect_filled(rect, 8.0, fill);
    ui.painter().rect_stroke(rect, 8.0, Stroke::new(if edge.is_some() { 1.5 } else { 1.0 }, edge.unwrap_or(k.button_line)), StrokeKind::Inside);
    ui.painter().galley(rect.center() - galley.size() / 2.0, galley, color);
    response
}

/// Salvar, in the accent; dimmed and deaf while what was pressed can't be saved.
fn save_button(ui: &mut egui::Ui, k: &K, enabled: bool) -> egui::Response {
    let galley = ui.painter().layout_no_wrap("Salvar".into(), FontId::proportional(12.5), Color32::WHITE);
    let (rect, response) = ui.allocate_exact_size(Vec2::new(galley.size().x + 24.0, 30.0), if enabled { Sense::click() } else { Sense::hover() });
    let response = if enabled { response.on_hover_cursor(CursorIcon::PointingHand) } else { response };

    response.widget_info(|| egui::WidgetInfo::labeled(egui::Role::Button, enabled, "Salvar"));

    let fill = match (enabled, response.hovered()) {
        (false, _) => k.primary.gamma_multiply(0.35),
        (true, true) => k.primary.lerp_to_gamma(Color32::WHITE, 0.12),
        (true, false) => k.primary,
    };

    ui.painter().rect_filled(rect, 8.0, fill);
    let text = if enabled { Color32::WHITE } else { Color32::WHITE.gamma_multiply(0.6) };

    ui.painter().galley(rect.center() - galley.size() / 2.0, galley, text);
    response
}

#[test]
fn the_shortcut_search_matches_the_name_or_the_keys() {
    assert!(shortcut_matches("Salvar como", "Ctrl+Shift+S", ""));
    assert!(shortcut_matches("Salvar como", "Ctrl+Shift+S", " SALVAR "));
    assert!(shortcut_matches("Desfazer", "Ctrl+Z", "z"));
    assert!(shortcut_matches("Ajustes", "Ctrl+,", "ctrl+,"));
    assert!(!shortcut_matches("Desfazer", "Ctrl+Z", "salvar"));
}
