//! Ajustes › Atalhos: the island's global shortcuts (app/hotkeys/), each with its keys, recorded by a click on them,
//! and its switch.

use super::kit::*;
use crate::app::hotkeys::{ACTIONS, ALTGR_NOTE, Binding, Recorded, Status, altgr_warning, display, effective, record, set_binding};
use crate::prelude::*;

impl DrawingApp {
    /// The global shortcuts: what each does at the left (how it stands under it); at the right its keys, which a click
    /// records anew, and its switch.
    pub(super) fn settings_hotkeys(&mut self, ui: &mut egui::Ui, k: &K) {
        title(ui, k, "ATALHOS GLOBAIS DA ILHA");
        note(ui, k, "Valem com qualquer programa na frente. Clique nas teclas de um atalho para gravar outras.");
        if !self.prefs.island {
            note(ui, k, "A ilha está desligada: eles voltam a valer quando ela for ligada em Ajustes › Geral.");
        }
        ui.add_space(3.25);
        for (index, def) in ACTIONS.iter().enumerate() {
            let mut binding = effective(def, &self.prefs.shortcuts);
            let recording = self.hotkeys.recording == Some(index);
            let hint = if recording {
                let refused = match self.hotkeys.refused {
                    Some(Recorded::NeedsModifier) => " Junte Ctrl ou Alt à tecla.",
                    Some(Recorded::Unsupported) => " Essa tecla não serve de atalho.",
                    _ => "",
                };
                format!("Pressione a combinação. Esc cancela e Backspace deixa sem tecla.{refused} {ALTGR_NOTE}")
            } else {
                let status = self.hotkeys.statuses.get(index).map_or("", |status| status.label());
                let warning = altgr_warning(&binding.keys).filter(|_| binding.enabled).unwrap_or_default();
                format!("{status} {warning}").trim().to_string()
            };
            let keys = if recording { "Pressione as teclas…".to_string() } else { display(&binding.keys) };
            let (flipped, clicked) = row(ui, k, def.label, &hint, |ui| {
                let (rect, _) = ui.allocate_exact_size(Vec2::new(38.0, 22.0), Sense::hover());
                (switch(ui, k, rect.min, def.label, &mut binding.enabled), button(ui, k, &keys).clicked())
            });
            if flipped {
                set_binding(&mut self.prefs.shortcuts, def, binding);
            }
            if clicked {
                (self.hotkeys.recording, self.hotkeys.refused) = (if recording { None } else { Some(index) }, None);
            }
        }
        self.record_hotkey(ui);
        if self.hotkeys.statuses.contains(&Status::Unsupported) {
            note(ui, k, Status::Unsupported.label());
        }
        divider(ui, k);
    }

    /// The keys pressed while a shortcut is being recorded: a combination becomes its keys, on.
    fn record_hotkey(&mut self, ui: &egui::Ui) {
        let Some(def) = self.hotkeys.recording.and_then(|index| ACTIONS.get(index)) else { return };
        let pressed: Vec<(Key, Modifiers)> = ui.input(|input| {
            let keys = input.events.iter().filter_map(|event| match event {
                egui::Event::Key { key, pressed: true, repeat: false, modifiers, .. } => Some((*key, *modifiers)),
                _ => None,
            });
            keys.collect()
        });
        for (key, modifiers) in pressed {
            let keys = match record(key, modifiers) {
                Recorded::Cancel => None,
                Recorded::Clear => Some(String::new()),
                Recorded::Keys(keys) => Some(keys),
                refused => {
                    self.hotkeys.refused = Some(refused);
                    continue;
                }
            };
            if let Some(keys) = keys {
                set_binding(&mut self.prefs.shortcuts, def, Binding { keys, enabled: true });
            }
            (self.hotkeys.recording, self.hotkeys.refused) = (None, None);
            break;
        }
    }
}
