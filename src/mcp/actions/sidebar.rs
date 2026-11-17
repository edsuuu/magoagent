use super::*;
use crate::app::ui::attention::waiting_text;

impl DrawingApp {
    /// The MCP's `sidebar`: what the person does in the sidebar's top and foot (ui/attention.rs, usage/foot.rs).
    pub(super) fn act_sidebar(&mut self, action: &str) -> Result<String, String> {
        match action {
            "next" => {
                let id = self.next_needing_attention().ok_or("Nenhum agente precisa de você.")?;
                Ok(format!("{} em foco.", self.agent_label(&id)))
            }
            "agents_open" | "agents_close" => {
                self.prefs.sidebar_agents_closed = action == "agents_close";
                Ok(if action == "agents_close" { "Agentes deste espaço recolhidos." } else { "Agentes deste espaço abertos." }.into())
            }
            "usage_open" | "usage_close" => {
                if action == "usage_open" && self.usage_ui.probe.shown().is_empty() {
                    return Err("Ainda não há leitura do uso dos planos: peça `usage_refresh` e tente de novo.".into());
                }
                self.usage_ui.open = action == "usage_open";
                // it shows in the sidebar: a folded one opens
                if self.usage_ui.open {
                    self.prefs.sidebar_hidden = false;
                }
                Ok(if self.usage_ui.open { "Uso dos agentes aberto na lateral." } else { "Uso dos agentes fechado." }.into())
            }
            "status" | "" => {
                let agents: Vec<String> = self.space_agents().iter().map(|row| format!("{} ({})", row.spec.label(), row.state.label())).collect();
                let listed = if agents.is_empty() { "nenhum".to_string() } else { agents.join(", ") };
                Ok(format!("{}.\nAgentes deste espaço: {listed}.", waiting_text(self.needing_you().len())))
            }
            other => Err(format!("ação desconhecida da lateral: {other}")),
        }
    }
}
