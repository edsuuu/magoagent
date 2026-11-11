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
            "Codex: a voz do app do Codex, que conversa de verdade (dá para interromper) e passa os pedidos ao Codex na pasta do chat; usa o login do ChatGPT do Codex, dentro do limite do seu plano. OpenAI: ouve e fala pela API (gpt-transcribe e gpt-4o-mini-tts), com a chave de Ajustes › Agentes › Contas, cobrada por uso. Local: whisper.cpp e Piper neste computador, sem internet.",
        );
        ui.add_space(3.25);
        // Codex's voice needs a page for its sound, which Linux doesn't have here
        let codex = cfg!(not(target_os = "linux"));
        let engines: &[&str] = if codex { &["Codex", "OpenAI", "Local"] } else { &["OpenAI", "Local"] };
        let now = if codex && self.prefs.voice_codex {
            "Codex"
        } else if self.prefs.voice_openai {
            "OpenAI"
        } else {
            "Local"
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
            let (ids, names): (Vec<&str>, Vec<&str>) = crate::voice_codex::CODEX_VOICES.iter().copied().unzip();
            let now = ids.iter().position(|id| *id == self.prefs.codex_voice).unwrap_or(0);
            if let Some(i) = row(ui, k, "Voz", "A voz do Codex. Vale na próxima chamada.", |ui| select(ui, k, "Voz", &names, now)) {
                self.prefs.codex_voice = ids[i].into();
            }
            return;
        }
        if self.prefs.voice_openai {
            title(ui, k, "VOZ");
            let (ids, names): (Vec<&str>, Vec<&str>) = crate::voice_openai::OPENAI_VOICES.iter().copied().unzip();
            let now = ids.iter().position(|id| *id == self.prefs.openai_voice).unwrap_or(0);
            if let Some(i) = row(ui, k, "Voz", "A voz com que o Maestro fala. Vale na próxima frase.", |ui| select(ui, k, "Voz", &names, now)) {
                self.prefs.openai_voice = ids[i].into();
            }
            if row(ui, k, "Ouvir esta voz", "", |ui| button(ui, k, "▶ Ouvir").clicked()) {
                match crate::voice_openai::key(&self.prefs) {
                    Some(key) => self.voice_sample = crate::voice_openai::say(&key, &self.prefs.openai_voice, "Olá! Eu sou o Maestro do MagoAgent."),
                    None => self.toast("Ponha a chave da OpenAI em Ajustes › Agentes › Contas."),
                }
            }
        } else {
            self.settings_local_voice(ui, k);
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

    /// Piper's voices and whisper.cpp's GPU, when the call runs on this computer.
    fn settings_local_voice(&mut self, ui: &mut egui::Ui, k: &K) {
        let fetching = crate::install::LOCAL_VOICE.lock().unwrap().clone();
        let hint = if fetching.is_empty() {
            "A voz com que o Maestro fala. Vale na próxima vez que a voz for ligada.".to_string()
        } else {
            format!("A voz com que o Maestro fala. Em segundo plano: {fetching}")
        };
        title(ui, k, "VOZ");
        let voices: Vec<&str> = crate::voice::VOICES.iter().copied().filter(|v| v.starts_with("pt_BR")).collect();
        let names: Vec<String> = voices.iter().map(|v| crate::voice::voice_name(v)).collect();
        let names: Vec<&str> = names.iter().map(String::as_str).collect();
        let now = voices.iter().position(|v| *v == self.prefs.voice).unwrap_or(0);
        if let Some(i) = row(ui, k, "Voz (português do Brasil)", &hint, |ui| select(ui, k, "Voz", &names, now)) {
            self.prefs.voice = voices[i].to_string();
        }
        if row(ui, k, "Ouvir esta voz", "", |ui| button(ui, k, "▶ Ouvir").clicked()) {
            let piper = crate::voice::voice_dir().and_then(|d| crate::voice::piper(&d, &self.prefs.voice));
            let pt = self.prefs.voice.starts_with("pt");
            let text = if pt { "Olá! Eu sou o Maestro do MagoAgent." } else { "Hi! I'm the MagoAgent Maestro." };
            self.voice_sample = Player::say(text, piper.as_ref(), if pt { "pt" } else { "en" });
        }
        divider(ui, k);
        let (gpu, usable) = crate::voice::gpu();
        title(ui, k, "PROCESSAMENTO");
        note(ui, k, &format!("GPU encontrada: {gpu}."));
        ui.add_space(3.25);
        let hint = if usable {
            "O whisper.cpp roda na GPU com o modelo grande, que entende melhor o português."
        } else {
            "Sem build de GPU pronto aqui: fica na CPU."
        };
        toggle(ui, k, "Usar a GPU para ouvir", hint, &mut self.prefs.voice_gpu);
    }
}
