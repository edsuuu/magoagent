//! Ajustes › Agentes: what each agent uses and the Maestro's limits. The hooks of the sessions outside the app are in
//! Integrações (`hooks.rs`).

use super::kit::*;
use crate::prelude::*;

impl DrawingApp {
    pub(super) fn settings_agents(&mut self, ui: &mut egui::Ui, k: &K) {
        let agents = self.agent_usage_lines();
        if !agents.is_empty() {
            title(ui, k, "AGENTES");
            note(ui, k, "O que cada agente que já respondeu está usando.");
            ui.add_space(3.25);
            for (name, line) in agents {
                text_block(ui, &name, BODY, k.fg);
                text_block(ui, &line, SMALL, k.muted_fg);
                ui.add_space(6.5);
            }
            divider(ui, k);
        }
        title(ui, k, "LIMITES DO MAESTRO");
        note(ui, k, "O que segura o Maestro ao recrutar agentes.");
        ui.add_space(3.25);
        let mode = usize::from(self.prefs.agent_mode == AgentMode::Chat);
        let hint = "Terminal: Claude Code completo em um terminal. Chat: conversa leve pelo chat do app, usando o login do CLI; gasta bem menos memória.";
        if let Some(i) = row(ui, k, "Invocação dos agentes", hint, |ui| select(ui, k, "Invocação dos agentes", &["Terminal", "Chat"], mode)) {
            self.prefs.agent_mode = [AgentMode::Terminal, AgentMode::Chat][i];
        }
        let stop = [70, 80, 90].iter().position(|n| *n == self.prefs.usage_stop).unwrap_or(3);
        let hint = "Com a janela de 5 horas do plano nesta porcentagem, o Maestro para de recrutar.";
        if let Some(i) = row(ui, k, "Parar de recrutar em", hint, |ui| select(ui, k, "Parar de recrutar em", &["70%", "80%", "90%", "Nunca"], stop)) {
            self.prefs.usage_stop = [70, 80, 90, 0][i];
        }
    }
}

/// A paragraph at `size`, wrapped to the column.
fn text_block(ui: &mut egui::Ui, text: &str, size: f32, color: Color32) {
    let line = if size == BODY { BODY_LINE } else { SMALL_LINE };
    let g = galley(ui, text, size, line, color, ui.available_width());
    let (rect, _) = ui.allocate_exact_size(g.size().max(Vec2::new(ui.available_width(), 0.0)), Sense::hover());
    ui.painter().galley(rect.min, g, color);
}
