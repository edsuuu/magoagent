//! Ajustes › Ok Mago: calling the Maestro by voice ("Ok Mago, …") during a call, and whether its answers are spoken.
//! The engine, voice and microphone stay in Voz.

use super::kit::*;
use crate::prelude::*;

impl DrawingApp {
    pub(super) fn settings_ok_mago(&mut self, ui: &mut egui::Ui, k: &K) {
        title(ui, k, "CHAMAR");
        let hint = "Na chamada de voz, dizer “Ok Mago, …” chama a Luna com o pedido, mesmo com a chamada em espera (depois do “se muta”). O nome do agente (Claude, Codex) continua valendo.";
        toggle(ui, k, "Responder a “Ok Mago”", hint, &mut self.prefs.ok_mago);
        let (label, hint) = if self.voice.on {
            ("Encerrar a chamada", "A chamada está ligada: diga “Ok Mago” para testar.")
        } else {
            ("Iniciar a chamada", "Liga o microfone no motor escolhido em Ajustes › Voz.")
        };
        if row(ui, k, "Chamada de voz", hint, |ui| button(ui, k, label).clicked()) {
            self.voice_toggle();
        }
        divider(ui, k);
        title(ui, k, "RESPOSTAS");
        let hint = "Desligado, a chamada começa com os alto-falantes silenciados e a Luna responde só por texto (o botão do alto-falante liga de novo).";
        toggle(ui, k, "Responder em voz alta", hint, &mut self.prefs.speak_answers);
        // the privacy of the whole app is in Sobre (asked); this note stays by the switch it is about
        divider(ui, k);
        title(ui, k, "MICROFONE");
        note(
            ui,
            k,
            "O microfone só fica aberto durante a chamada: o app não escuta o tempo todo (isso pediria um modelo de voz local, e o app não baixa modelos). O que você diz vai ao motor de Ajustes › Voz, o VoiceStudio do seu computador ou a OpenAI.",
        );
    }
}
