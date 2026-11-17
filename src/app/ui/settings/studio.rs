//! Ajustes › Voz when the engine is VoiceStudio: the server's address, its key, and the voice
//! chosen from the list the server gives.

use super::kit::*;
use crate::prelude::*;
use crate::voice::studio::{StudioServer, StudioVoices, fetch_voices};

impl DrawingApp {
    pub(super) fn settings_studio_voice(&mut self, ui: &mut egui::Ui, k: &K) {
        title(ui, k, "SERVIDOR");
        note(
            ui,
            k,
            "O VoiceStudio é um programa à parte: abra o app dele ou suba o servidor (docker run -d -p 127.0.0.1:3900:3900 ghcr.io/debpalash/voicestudio:stable) e a chamada fala e ouve por ele.",
        );
        ui.add_space(3.25);
        let before = (self.prefs.voice_studio_url.clone(), self.prefs.voice_studio_key.clone());
        let url = &mut self.prefs.voice_studio_url;
        row(ui, k, "Endereço", "Onde o servidor responde. O padrão é o do VoiceStudio.", |ui| {
            input(ui, k, egui::Id::unique("settings_studio_url"), "Endereço do VoiceStudio", url, crate::voice::studio::STUDIO_URL, 300.0, false)
        });
        let key = &mut self.prefs.voice_studio_key;
        row(ui, k, "Chave de API", "Só se o servidor pedir (OMNIVOICE_API_KEY). Fica neste computador, em prefs.json.", |ui| {
            input(ui, k, egui::Id::unique("settings_studio_key"), "Chave de API do VoiceStudio", key, "opcional", 300.0, true)
        });
        // another address or key: the list is asked again
        if before != (self.prefs.voice_studio_url.clone(), self.prefs.voice_studio_key.clone()) {
            *self.studio_voices.lock().unwrap() = StudioVoices::Idle;
        }
        let server = StudioServer::from_prefs(&self.prefs);
        let list = self.studio_voices.lock().unwrap().clone();
        match list {
            StudioVoices::Idle => fetch_voices(server.clone(), self.studio_voices.clone(), ui.ctx().clone()),
            StudioVoices::Loading => ui.ctx().request_repaint_after(std::time::Duration::from_millis(200)),
            _ => {}
        }
        divider(ui, k);
        title(ui, k, "VOZ");
        match &list {
            StudioVoices::Done(voices) if !voices.is_empty() => {
                let names: Vec<&str> = voices.iter().map(|(_, name)| name.as_str()).collect();
                let now = voices.iter().position(|(id, _)| *id == server.voice).unwrap_or(0);
                if let Some(i) =
                    row(ui, k, "Voz", "A voz com que a Luna fala, da lista do servidor. Vale na próxima frase.", |ui| select(ui, k, "Voz", &names, now))
                {
                    self.prefs.voice_studio_voice = voices[i].0.clone();
                }
            }
            StudioVoices::Failed(why) => note(ui, k, why),
            StudioVoices::Done(_) => note(ui, k, "O servidor não listou nenhuma voz."),
            _ => note(ui, k, "Buscando as vozes do servidor…"),
        }
        if row(ui, k, "Ouvir esta voz", "", |ui| button(ui, k, "▶ Ouvir").clicked()) {
            match &list {
                StudioVoices::Failed(why) => self.toast(why.clone()),
                _ => self.voice_sample = server.say("Olá! Eu sou a Luna, a assistente do MagoAgent."),
            }
        }
        if row(ui, k, "Atualizar a lista", "Depois de criar ou clonar uma voz no VoiceStudio.", |ui| button(ui, k, "Atualizar").clicked()) {
            fetch_voices(server, self.studio_voices.clone(), ui.ctx().clone());
        }
    }
}
