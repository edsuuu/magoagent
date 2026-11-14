//! Ajustes › Voz: the engine of the call, its voice, the pause that ends a sentence and the
//! commands.

use super::kit::*;
use crate::prelude::*;

impl DrawingApp {
    pub(super) fn settings_voice(&mut self, ui: &mut egui::Ui, k: &K) {
        title(ui, k, "MOTOR");
        note(
            ui,
            k,
            "Codex: a voz do app do Codex, que conversa de verdade; está fora do ar (a ChatGPT responde 404), então a chamada não liga com ele. VoiceStudio: ouve e fala por um servidor do VoiceStudio neste computador (open source, com a voz que você escolher da lista dele), sem chave e sem custo. OpenAI: ouve e fala pela API (gpt-transcribe e gpt-4o-mini-tts), com a sua chave da OpenAI (abaixo), cobrada por uso. O app não baixa modelos nem programas de voz.",
        );
        ui.add_space(3.25);
        // Codex's voice needs a page for its sound, which Linux doesn't have here
        let codex = cfg!(not(target_os = "linux"));
        let engines: &[&str] = if codex { &["Codex", "VoiceStudio", "OpenAI"] } else { &["VoiceStudio", "OpenAI"] };
        let now = if codex && self.prefs.voice_codex {
            "Codex"
        } else if self.prefs.voice_openai {
            "OpenAI"
        } else {
            "VoiceStudio"
        };
        let engine = engines.iter().position(|e| *e == now).unwrap_or(0);
        if let Some(i) = row(ui, k, "Motor da chamada", "", |ui| select(ui, k, "Motor da chamada", engines, engine)) {
            self.prefs.voice_codex = engines[i] == "Codex";
            if !self.prefs.voice_codex {
                self.prefs.voice_openai = engines[i] == "OpenAI";
            }
        }
        divider(ui, k);
        if codex && self.prefs.voice_codex {
            title(ui, k, "VOZ");
            let (ids, names): (Vec<&str>, Vec<&str>) = crate::voice::codex::CODEX_VOICES.iter().copied().unzip();
            let now = ids.iter().position(|id| *id == self.prefs.codex_voice).unwrap_or(0);
            if let Some(i) = row(ui, k, "Voz", "A voz do Codex. Vale na próxima chamada.", |ui| select(ui, k, "Voz", &names, now)) {
                self.prefs.codex_voice = ids[i].into();
            }
            return;
        }
        if !self.prefs.voice_openai {
            self.settings_studio_voice(ui, k);
        } else {
            title(ui, k, "VOZ");
            let (ids, names): (Vec<&str>, Vec<&str>) = crate::voice::openai::OPENAI_VOICES.iter().copied().unzip();
            let now = ids.iter().position(|id| *id == self.prefs.openai_voice).unwrap_or(0);
            if let Some(i) = row(ui, k, "Voz", "A voz com que o Maestro fala. Vale na próxima frase.", |ui| select(ui, k, "Voz", &names, now)) {
                self.prefs.openai_voice = ids[i].into();
            }
            let key = &mut self.prefs.openai_key;
            row(ui, k, "Chave da OpenAI", "Fica só neste computador, em prefs.json.", |ui| {
                input(ui, k, egui::Id::unique("settings_openai_key"), "Chave da OpenAI", key, "sk-…", 300.0, true)
            });
            if row(ui, k, "Ouvir esta voz", "", |ui| button(ui, k, "▶ Ouvir").clicked()) {
                match crate::voice::openai::key(&self.prefs) {
                    Some(key) => self.voice_sample = crate::voice::openai::say(&key, &self.prefs.openai_voice, "Olá! Eu sou o Maestro do MagoAgent."),
                    None => self.toast("Ponha a chave da OpenAI em Ajustes › Voz."),
                }
            }
        }
        divider(ui, k);
        title(ui, k, "MICROFONE");
        // the system's input may be a virtual cable that hears nothing: Automático skips those
        let mics = crate::voice::input_names();
        let mut names = vec!["Automático".to_string()];
        names.extend(mics.iter().cloned());
        let shown: Vec<&str> = names.iter().map(String::as_str).collect();
        let now = mics.iter().position(|m| *m == self.prefs.voice_mic).map_or(0, |i| i + 1);
        let hint = "O que a chamada e o Ditar ouvem. Automático usa o do sistema, menos um cabo virtual (que só manda silêncio).";
        if let Some(i) = row(ui, k, "Microfone", hint, |ui| select(ui, k, "Microfone", &shown, now)) {
            self.prefs.voice_mic = if i == 0 { String::new() } else { mics[i - 1].clone() };
        }
        divider(ui, k);
        title(ui, k, "PAUSA");
        let hint = "Quanto silêncio encerra o que você diz. Uma pausa menor que isso faz parte da frase; aumente se ele corta você no meio.";
        // quarters of a second, 1 to 4 s
        let mut quarters = (self.prefs.voice_pause * 4.0).round().clamp(4.0, 16.0) as u8;
        let shown = format!("{} s", self.prefs.voice_pause).replace('.', ",");
        if row(ui, k, "Pausa", hint, |ui| stepper(ui, k, "Pausa", &mut quarters, 4..=16, &shown)) {
            self.prefs.voice_pause = f32::from(quarters) / 4.0;
        }
        divider(ui, k);
        title(ui, k, "COMANDOS DE VOZ");
        note(
            ui,
            k,
            "Chame pelo nome do agente: \"Claude, se muta\" (ele pergunta se é para ficar mudo; com um sim, para de ouvir a conversa e avisa com um toque quando tem resposta), \"Claude, pode falar\", \"Claude, desmuta\", \"Claude, <pedido>\".",
        );
    }
}
