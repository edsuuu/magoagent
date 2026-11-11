//! Ajustes › Agentes: the chat's defaults, the API keys (Contas), the skills, the consumption the
//! Maestro keeps to (Uso), the hooks (Controle) and the avatar.

use super::kit::*;
use crate::prelude::*;

impl DrawingApp {
    pub(super) fn settings_agents(&mut self, ui: &mut egui::Ui, k: &K) {
        match super::subtabs(ui, k, "agents", &["Chat", "Contas", "Skills", "Uso", "Controle", "Avatar"]) {
            0 => self.settings_chat(ui, k),
            1 => self.settings_accounts(ui, k),
            2 => settings_skills(ui, k),
            3 => self.settings_usage(ui, k),
            4 => self.settings_control(ui, k),
            _ => self.settings_avatar(ui, k),
        }
    }

    fn settings_chat(&mut self, ui: &mut egui::Ui, k: &K) {
        title(ui, k, "CHAT");
        note(ui, k, "Um chat novo já começa assim, sem perguntar. Modelo e esforço ficam nos chips sob a caixa do chat.");
        ui.add_space(3.25);
        let agent = usize::from(self.prefs.chat_agent == "codex");
        if let Some(i) = row(ui, k, "Agente", "", |ui| select(ui, k, "Agente", &["Claude Code", "Codex"], agent)) {
            self.prefs.chat_agent = ["claude", "codex"][i].into();
        }
        self.folder_row(ui, k);
        let mode = self.prefs.chat_mode.min(crate::app::chat_direct::CHAT_MODES.len() - 1);
        if let Some(i) = row(ui, k, "O agente pode", "", |ui| select(ui, k, "O agente pode", &crate::app::chat_direct::CHAT_MODES, mode)) {
            self.prefs.chat_mode = i;
        }
    }

    /// "Pasta de trabalho": the path, Recentes (once there are folders to offer) and Procurar…
    fn folder_row(&mut self, ui: &mut egui::Ui, k: &K) {
        let c = UiColors::new(k.dark);
        let prefs = &mut self.prefs;
        row(ui, k, "Pasta de trabalho", "", |ui| {
            if button(ui, k, "Procurar…").clicked()
                && let Some(dir) = rfd::FileDialog::new().set_title("Pasta dos chats").pick_folder()
            {
                prefs.chat_folder = dir.to_string_lossy().into();
            }
            if !prefs.folders.is_empty() {
                let response = button(ui, k, "Recentes").tip("Pastas recentes e favoritas");
                // starring keeps the menu open; picking a folder closes it
                egui::Popup::menu(&response).frame(popup_frame(k)).close_behavior(egui::PopupCloseBehavior::CloseOnClickOutside).show(|ui| {
                    ui.set_width(320.0);
                    ui.spacing_mut().item_spacing.y = 1.0;
                    match folder_rows(ui, &prefs.folders, &c) {
                        Some(FolderPick::Use(path)) => {
                            prefs.chat_folder = path;
                            ui.close();
                        }
                        Some(FolderPick::Star(path)) => _ = prefs.folders.toggle_favorite(&path),
                        None => {}
                    }
                });
            }
            input(ui, k, egui::Id::unique("settings_chat_folder"), "Pasta de trabalho", &mut prefs.chat_folder, "Pasta projetos do MagoAgent", 230.0, false);
        });
    }

    fn settings_accounts(&mut self, ui: &mut egui::Ui, k: &K) {
        title(ui, k, "LOCAL OU API");
        note(
            ui,
            k,
            "Desligado, os chats rodam o claude e o codex com o login deles (sua assinatura). Ligado, rodam com a chave de API abaixo, cobrada por uso.",
        );
        ui.add_space(3.25);
        let hint = "Vale a partir da próxima mensagem; o claude recebe ANTHROPIC_API_KEY e o codex OPENAI_API_KEY.";
        toggle(ui, k, "Usar as chaves de API nos chats", hint, &mut self.prefs.use_api_keys);
        divider(ui, k);
        title(ui, k, "CHAVES DE API");
        note(ui, k, "Ficam só neste computador, em prefs.json.");
        ui.add_space(3.25);
        for (label, key, hint) in [("Anthropic", &mut self.prefs.anthropic_key, "sk-ant-…"), ("OpenAI", &mut self.prefs.openai_key, "sk-…")] {
            row(ui, k, label, "", |ui| input(ui, k, egui::Id::unique(("settings_key", label)), label, key, hint, 300.0, true));
        }
    }

    fn settings_usage(&mut self, ui: &mut egui::Ui, k: &K) {
        title(ui, k, "CONSUMO");
        note(ui, k, "O limite do plano, como o Claude Code informa, e o que segura o Maestro ao recrutar agentes.");
        ui.add_space(3.25);
        let mut lines = vec![format!("Plano: {}.", self.plan_line())];
        lines.extend(self.agent_usage_lines().into_iter().map(|(name, line)| format!("{name}: {line}")));
        for line in lines {
            let g = galley(ui, &line, 13.0, 18.2, k.fg, ui.available_width());
            let (rect, _) = ui.allocate_exact_size(g.size().max(Vec2::new(ui.available_width(), 0.0)), Sense::hover());
            ui.painter().galley(rect.min, g, k.fg);
        }
        let ceiling = [3, 6, 10].iter().position(|n| *n == self.prefs.max_agents).unwrap_or(3);
        let hint = "Acima disso o Maestro não recruta.";
        if let Some(i) =
            row(ui, k, "Agentes rodando ao mesmo tempo", hint, |ui| select(ui, k, "Agentes rodando ao mesmo tempo", &["3", "6", "10", "Sem teto"], ceiling))
        {
            self.prefs.max_agents = [3, 6, 10, 0][i];
        }
        let stop = [70, 80, 90].iter().position(|n| *n == self.prefs.usage_stop).unwrap_or(3);
        let label = "Parar de recrutar com a janela de 5 horas do plano em";
        if let Some(i) = row(ui, k, label, "", |ui| select(ui, k, label, &["70%", "80%", "90%", "Nunca"], stop)) {
            self.prefs.usage_stop = [70, 80, 90, 0][i];
        }
        let models = ["", "sonnet", "haiku"];
        let model = models.iter().position(|m| *m == self.prefs.recruit_model).unwrap_or(0);
        let label = "Modelo dos agentes que o Maestro recruta (Claude Code)";
        if let Some(i) = row(ui, k, label, "", |ui| select(ui, k, label, &["O da CLI", "Sonnet", "Haiku"], model)) {
            self.prefs.recruit_model = models[i].into();
        }
        divider(ui, k);
        title(ui, k, "MEMÓRIA");
        note(ui, k, "Atualizada a cada 2 segundos.");
        ui.add_space(3.25);
        let snapshot = self.memory.snapshot();
        let mut lines = vec![format!("MagoAgent: {}", snapshot.app_bytes.map(memory::size).unwrap_or_else(|| "indisponível".into()))];
        lines.extend(snapshot.terminals.iter().map(|t| format!("{}: {} em {} processos", t.name, memory::size(t.bytes), t.processes)));
        let total: u64 = snapshot.terminals.iter().map(|t| t.bytes).sum();
        lines.push(format!(
            "Agentes: {} · {} terminais · {} portais · {} IDEs",
            memory::size(total),
            snapshot.terminals.len(),
            self.scene.elements.iter().filter(|e| e.portal.is_some()).count(),
            self.scene.elements.iter().filter(|e| e.ide.is_some()).count()
        ));
        for line in lines {
            let g = galley(ui, &line, 13.0, 18.2, k.fg, ui.available_width());
            let (rect, _) = ui.allocate_exact_size(g.size().max(Vec2::new(ui.available_width(), 0.0)), Sense::hover());
            ui.painter().galley(rect.min, g, k.fg);
        }
        note(ui, k, "A memória dos agentes e das páginas dos portais pertence a processos separados, não ao MagoAgent.");
    }

    fn settings_control(&mut self, ui: &mut egui::Ui, k: &K) {
        title(ui, k, "AGENTES");
        toggle(
            ui,
            k,
            "Acompanhar os agentes pelos hooks do Claude Code",
            "O claude que o app inicia (terminais e chats) avisa o app de cada passo: pedidos de permissão, perguntas, erros e quando termina, \
             e o Maestro pode responder às permissões; a linha de status dessas sessões passa a ser a do app (modelo, contexto e o limite do plano). Usa um arquivo de ajustes só do app (--settings), sem mexer nos seus. Vale para o que \
             começar depois; precisa de um Claude Code com --settings.",
            &mut self.prefs.agent_hooks,
        );
    }

    fn settings_avatar(&mut self, ui: &mut egui::Ui, k: &K) {
        title(ui, k, "AVATAR");
        note(ui, k, "A barra do Maestro, por cima de tudo na tela, com o mascote do agente do chat.");
        ui.add_space(3.25);
        toggle(ui, k, "Mostrar o avatar", "O × da barra o esconde.", &mut self.prefs.avatar);
        let mut follow = self.prefs.avatar_at.is_none();
        toggle(ui, k, "Seguir a janela em uso", "Desligado, a barra fica onde você a deixar: arraste-a por qualquer ponto.", &mut follow);
        if follow != self.prefs.avatar_at.is_none() {
            self.avatar_track_window(follow);
        }
    }
}

/// Ajustes › Agentes › Skills: the folder's skills and a button to its folder.
fn settings_skills(ui: &mut egui::Ui, k: &K) {
    title(ui, k, "SKILLS");
    note(ui, k, "As skills desta pasta vão para o Claude Code que o app inicia, no chat e nos terminais. O Codex não tem um equivalente, então não recebe.");
    ui.add_space(3.25);
    let Some(path) = crate::app::skills::skills_path() else { return };
    // ponytail: the folder is read each frame while this tab shows; keep the list for a second or two if it grows
    let skills = crate::app::skills::list(&path);
    if skills.is_empty() {
        note(ui, k, "Nenhuma skill ainda: cada skill é uma pasta com um SKILL.md dentro de skills, na pasta abaixo.");
    }
    for skill in skills {
        let head = galley(ui, &skill.name, 13.0, 18.2, k.fg, ui.available_width());
        let body = galley(ui, &skill.description, 11.0, 15.1, k.muted_fg, ui.available_width());
        let (rect, _) = ui.allocate_exact_size(Vec2::new(ui.available_width(), head.size().y + body.size().y + 6.5), Sense::hover());
        let below = rect.top() + head.size().y;
        ui.painter().galley(rect.min, head, k.fg);
        ui.painter().galley(Pos2::new(rect.left(), below), body, k.muted_fg);
    }
    ui.add_space(6.5);
    note(ui, k, &path.display().to_string());
    ui.add_space(3.25);
    if button(ui, k, "Abrir a pasta").clicked() {
        let _ = crate::app::skills::open(&path);
    }
}
