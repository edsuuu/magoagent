//! Ajustes › Integrações › MCP em outros agentes: the app's MCP in the person's own Claude Code and Codex
//! (system/mcp_register.rs). Whether it is there is asked once a run; Registrar puts it in, Remover takes it out.

use super::kit::*;
use crate::app::system::mcp_register::*;
use crate::prelude::*;

impl DrawingApp {
    pub(super) fn settings_mcp_elsewhere(&mut self, ui: &mut egui::Ui, k: &K) {
        // asked only for the agents not asked yet in this run
        mcp_check(ui.ctx());

        for said in mcp_said() {
            self.toast(said);
        }

        title(ui, k, "MCP EM OUTROS AGENTES");
        note(ui, k, "Registra o MCP do MagoAgent no Claude Code ou no Codex de fora do app, para as sessões de lá também usarem o canvas.");
        ui.add_space(3.25);

        for (agent, name) in MCP_AGENTS {
            let status = mcp_status(agent);
            let doing = mcp_doing(agent);

            let state = match (doing, status) {
                (Some(doing), _) => doing.to_string(),
                (None, McpStatus::Checking) => "Verificando…".into(),
                (None, McpStatus::Registered) => "Registrado".into(),
                (None, McpStatus::NotRegistered) => "Não registrado".into(),
                (None, McpStatus::Missing) => format!("{name} não encontrado"),
            };

            row(ui, k, name, &state, |ui| {
                let label = match mcp_action(status, doing.is_some()) {
                    Some(McpAction::Remove) => Some((McpAction::Remove, "Remover".to_string())),
                    Some(McpAction::Register) => Some((McpAction::Register, format!("Registrar no {name}"))),
                    None => None,
                };

                if let Some((action, label)) = label
                    && button(ui, k, &label).clicked()
                {
                    mcp_soon(agent, action, ui.ctx());
                }
            });
        }
    }
}
