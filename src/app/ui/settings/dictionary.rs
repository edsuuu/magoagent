//! Ajustes › Dicionário: the words the ears must spell right and what to write for what they hear (`voice/dictionary.rs`).
//! The call and Ditar use both: the words go to the engine as a hint, the replacements run on everything heard.

use super::kit::*;
use crate::prelude::*;

/// A text field's content kept between frames (the fields are not part of the preferences).
fn kept(ui: &egui::Ui, name: &str) -> String {
    ui.data(|d| d.get_temp::<String>(egui::Id::unique(("settings_dictionary", name)))).unwrap_or_default()
}

fn keep(ui: &egui::Ui, name: &str, text: String) {
    ui.data_mut(|d| d.insert_temp(egui::Id::unique(("settings_dictionary", name)), text));
}

/// A field and Enter in it: the text, and whether Enter was pressed in it.
fn field(ui: &mut egui::Ui, k: &K, name: &str, hint: &str, width: f32) -> (String, bool) {
    let mut text = kept(ui, name);
    let response = input(ui, k, egui::Id::unique(("settings_dictionary_field", name)), name, &mut text, hint, width, false);
    let enter = response.lost_focus() && ui.input(|i| i.key_pressed(Key::Enter));
    keep(ui, name, text.clone());
    (text, enter)
}

impl DrawingApp {
    pub(super) fn settings_dictionary(&mut self, ui: &mut egui::Ui, k: &K) {
        title(ui, k, "PALAVRAS");
        note(
            ui,
            k,
            "Nomes, marcas e termos que o ouvido do app erra. Vão ao motor de voz como dica e saem sempre do jeito que você escreveu aqui, na chamada e no Ditar.",
        );
        ui.add_space(3.25);
        let (mut add, mut word) = (false, String::new());
        row(ui, k, "Nova palavra", "", |ui| {
            add = button(ui, k, "Adicionar").clicked();
            let (text, enter) = field(ui, k, "Palavra", "Supabase", 230.0);
            (add, word) = (add || enter, text);
        });
        if add && self.prefs.dictionary.add_word(&word) {
            keep(ui, "Palavra", String::new());
        }
        let mut removed = None;
        for word in &self.prefs.dictionary.words {
            row(ui, k, word, "", |ui| {
                if ghost_button(ui, k, Icon::Trash, &format!("Remover {word}"), true, true).clicked() {
                    removed = Some(word.clone());
                }
            });
        }
        if let Some(word) = removed {
            self.prefs.dictionary.remove_word(&word);
        }
        if self.prefs.dictionary.words.is_empty() {
            note(ui, k, "Nenhuma palavra ainda. Ponha os nomes que o ouvido erra.");
        }
        divider(ui, k);
        title(ui, k, "TROCAS");
        note(ui, k, "Quando o app ouvir o que está à esquerda, escreve o que está à direita (palavra inteira, sem diferenciar maiúsculas).");
        ui.add_space(3.25);
        let (mut add, mut heard, mut written) = (false, String::new(), String::new());
        row(ui, k, "Nova troca", "", |ui| {
            add = button(ui, k, "Adicionar").clicked();
            ui.add_space(8.0);
            let (text, enter) = field(ui, k, "Escrito", "Claude Code", 150.0);
            written = text;
            ui.add_space(8.0);
            let (text, enter_heard) = field(ui, k, "Ouvido", "cloud code", 150.0);
            (heard, add) = (text, add || enter || enter_heard);
        });
        if add && self.prefs.dictionary.add_replacement(&heard, &written) {
            keep(ui, "Ouvido", String::new());
            keep(ui, "Escrito", String::new());
        }
        let mut removed = None;
        for replacement in &self.prefs.dictionary.replacements {
            row(ui, k, &format!("{}  →  {}", replacement.from, replacement.to), "", |ui| {
                if ghost_button(ui, k, Icon::Trash, &format!("Remover {}", replacement.from), true, true).clicked() {
                    removed = Some(replacement.from.clone());
                }
            });
        }
        if let Some(from) = removed {
            self.prefs.dictionary.remove_replacement(&from);
        }
        if self.prefs.dictionary.replacements.is_empty() {
            note(ui, k, "Nenhuma troca ainda. Exemplo: ouvido “cloud code”, escrito “Claude Code”.");
        }
        divider(ui, k);
        note(ui, k, "A voz do Codex na chamada escuta pelo próprio Codex: o dicionário vale para a chamada pelo VoiceStudio ou pela OpenAI e para o Ditar.");
    }
}
