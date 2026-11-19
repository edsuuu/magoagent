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
        note(ui, k, "O que segura a Luna ao recrutar agentes.");
        ui.add_space(3.25);
        let mode = usize::from(self.prefs.agent_mode == AgentMode::Chat);
        let hint = "Terminal: Claude Code completo em um terminal. Chat: conversa leve pelo chat do app, usando o login do CLI; gasta bem menos memória.";

        if let Some(i) = row(ui, k, "Invocação dos agentes", hint, |ui| select(ui, k, "Invocação dos agentes", &["Terminal", "Chat"], mode)) {
            self.prefs.agent_mode = [AgentMode::Terminal, AgentMode::Chat][i];
        }

        let stop = [70, 80, 90].iter().position(|n| *n == self.prefs.usage_stop).unwrap_or(3);
        let hint = "Com a janela de 5 horas do plano nesta porcentagem, a Luna para de recrutar.";

        if let Some(i) = row(ui, k, "Parar de recrutar em", hint, |ui| select(ui, k, "Parar de recrutar em", &["70%", "80%", "90%", "Nunca"], stop)) {
            self.prefs.usage_stop = [70, 80, 90, 0][i];
        }

        divider(ui, k);
        // what the agents are told about the temporary folder (maestro/temp_folder.rs)
        title(ui, k, "ONDE OS AGENTES TRABALHAM");
        let hint = format!("Os agentes do app não guardam trabalho em {}, que pode sumir ao reiniciar: usam a pasta do projeto.", temp_names());

        toggle(ui, k, "Trabalho fora da pasta temporária", &hint, &mut self.prefs.work_outside_temp);
        divider(ui, k);
        self.settings_mcp_elsewhere(ui, k);

        if cfg!(windows) {
            divider(ui, k);
            self.settings_wsl(ui, k);
        }
    }

    /// O MCP do app no Claude Code e no Codex da pessoa (system/mcp_register.rs): o estado é lido quando a página abre.
    fn settings_mcp_elsewhere(&mut self, ui: &mut egui::Ui, k: &K) {
        use crate::app::system::mcp_register::*;
        // the page drawn the pass before: still open; a gap: it opened now
        let (seen, pass) = (egui::Id::unique("settings_mcp_seen"), ui.ctx().cumulative_pass_nr());
        let before = ui.ctx().data_mut(|d| std::mem::replace(d.get_temp_mut_or(seen, 0u64), pass));

        if before == 0 || pass - before > 1 {
            mcp_check(ui.ctx());
        }

        for said in mcp_said() {
            self.toast(said);
        }

        title(ui, k, "MCP EM OUTROS AGENTES");
        note(ui, k, "Registra o MCP do MagoAgent no Claude Code ou no Codex de fora do app, para as sessões de lá também usarem o canvas.");
        ui.add_space(3.25);

        for (agent, name) in MCP_AGENTS {
            let state = match mcp_status(agent) {
                _ if mcp_busy(agent) => "Verificando…".to_string(),
                McpStatus::Checking => "Verificando…".into(),
                McpStatus::Registered => "Registrado".into(),
                McpStatus::NotRegistered => "Não registrado".into(),
                McpStatus::Missing => format!("{name} não encontrado"),
            };

            row(ui, k, name, &state, |ui| {
                if button(ui, k, &format!("Registrar no {name}")).clicked() {
                    mcp_register_soon(agent, ui.ctx());
                }
            });
        }
    }

    /// Para quem tem os agentes só dentro do WSL (system/wsl.rs): em que distro rodam.
    fn settings_wsl(&mut self, ui: &mut egui::Ui, k: &K) {
        title(ui, k, "WSL");

        let found = match wsl_distro() {
            Some(d) if d.agents.is_empty() => format!("{}: nem o claude nem o codex instalados lá.", d.name),
            Some(d) => format!("{}: {}.", d.name, d.agents.join(" e ")),
            None if self.prefs.wsl == WSL_OFF => "Os agentes do WSL ficam de fora.".into(),
            None => "Para o Claude Code ou o Codex instalados só dentro do WSL.".into(),
        };

        note(ui, k, &found);
        ui.add_space(3.25);
        let mut options: Vec<String> = vec!["Automático".into(), "Desligado".into()];

        options.extend(wsl_distros());

        if !self.prefs.wsl.is_empty() && self.prefs.wsl != WSL_OFF && !options.contains(&self.prefs.wsl) {
            options.push(self.prefs.wsl.clone());
        }

        let chosen = match self.prefs.wsl.as_str() {
            "" => 0,
            WSL_OFF => 1,
            name => options.iter().position(|o| o == name).unwrap_or(0),
        };

        let names: Vec<&str> = options.iter().map(String::as_str).collect();
        let hint = "Automático: a distro padrão, quando o Windows não tem nenhum agente. Uma distro: os agentes rodam sempre nela.";

        if let Some(i) = row(ui, k, "Agentes no WSL", hint, |ui| select(ui, k, "Agentes no WSL", &names, chosen)) {
            self.prefs.wsl = match i {
                0 => String::new(),
                1 => WSL_OFF.into(),
                i => options[i].clone(),
            };

            wsl_choose(&self.prefs.wsl);
            self.chat_installed = None;
        }
    }
}

/// The temporary folders the hint names, as this system has them.
fn temp_names() -> &'static str {
    if cfg!(windows) {
        "%TEMP%"
    } else if cfg!(target_os = "macos") {
        "/tmp ou /var/folders"
    } else {
        "/tmp"
    }
}

/// A paragraph at `size`, wrapped to the column.
fn text_block(ui: &mut egui::Ui, text: &str, size: f32, color: Color32) {
    let line = if size == BODY { BODY_LINE } else { SMALL_LINE };
    let g = galley(ui, text, size, line, color, ui.available_width());
    let (rect, _) = ui.allocate_exact_size(g.size().max(Vec2::new(ui.available_width(), 0.0)), Sense::hover());

    ui.painter().galley(rect.min, g, color);
}
