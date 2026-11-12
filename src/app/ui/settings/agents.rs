//! Ajustes › Agentes: the chat's defaults, the skills, the plan's consumption and the Maestro's limits
//! (Uso), the memory in use, the hooks (Controle) and the avatar.

use super::kit::*;
use crate::prelude::*;

impl DrawingApp {
    pub(super) fn settings_agents(&mut self, ui: &mut egui::Ui, k: &K) {
        match super::subtabs(ui, k, "agents", &["Chat", "Skills", "Uso", "Memória", "Controle", "Avatar"]) {
            0 => self.settings_chat(ui, k),
            1 => settings_skills(ui, k),
            2 => self.settings_usage(ui, k),
            3 => self.settings_memory(ui, k),
            4 => self.settings_control(ui, k),
            _ => self.settings_avatar(ui, k),
        }
    }

    fn settings_chat(&mut self, ui: &mut egui::Ui, k: &K) {
        title(ui, k, "CHAT");
        note(
            ui,
            k,
            "Um chat novo já começa assim, sem perguntar; a permissão segue a do próprio agente quando ele define uma (o defaultMode do Claude Code). Modelo e esforço ficam nos chips sob a caixa do chat.",
        );
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

    fn settings_usage(&mut self, ui: &mut egui::Ui, k: &K) {
        title(ui, k, "CONSUMO DO PLANO");
        note(ui, k, "O limite do plano, como o Claude Code informa.");
        ui.add_space(3.25);
        text_block(ui, &self.plan_line(), BODY, k.fg);
        let agents = self.agent_usage_lines();
        if !agents.is_empty() {
            divider(ui, k);
            title(ui, k, "AGENTES");
            note(ui, k, "O que cada agente que já respondeu está usando.");
            ui.add_space(3.25);
            for (name, line) in agents {
                text_block(ui, &name, BODY, k.fg);
                text_block(ui, &line, SMALL, k.muted_fg);
                ui.add_space(6.5);
            }
        }
        divider(ui, k);
        title(ui, k, "LIMITES DO MAESTRO");
        note(ui, k, "O que segura o Maestro ao recrutar agentes.");
        ui.add_space(3.25);
        let ceiling = [3, 6, 10].iter().position(|n| *n == self.prefs.max_agents).unwrap_or(3);
        let hint = "Quantos agentes podem rodar juntos. Acima disso o Maestro não recruta mais um.";
        if let Some(i) = row(ui, k, "Agentes ao mesmo tempo", hint, |ui| select(ui, k, "Agentes ao mesmo tempo", &["3", "6", "10", "Sem teto"], ceiling)) {
            self.prefs.max_agents = [3, 6, 10, 0][i];
        }
        let stop = [70, 80, 90].iter().position(|n| *n == self.prefs.usage_stop).unwrap_or(3);
        let hint = "Com a janela de 5 horas do plano nesta porcentagem, o Maestro para de recrutar.";
        if let Some(i) = row(ui, k, "Parar de recrutar em", hint, |ui| select(ui, k, "Parar de recrutar em", &["70%", "80%", "90%", "Nunca"], stop)) {
            self.prefs.usage_stop = [70, 80, 90, 0][i];
        }
        let models = ["", "sonnet", "haiku"];
        let model = models.iter().position(|m| *m == self.prefs.recruit_model).unwrap_or(0);
        let hint = "O modelo dos agentes do Claude Code que o Maestro recruta.";
        if let Some(i) = row(ui, k, "Modelo dos recrutados", hint, |ui| select(ui, k, "Modelo dos recrutados", &["O da CLI", "Sonnet", "Haiku"], model)) {
            self.prefs.recruit_model = models[i].into();
        }
    }

    fn settings_memory(&mut self, ui: &mut egui::Ui, k: &K) {
        title(ui, k, "MEMÓRIA EM USO");
        note(ui, k, "Atualizada a cada 2 segundos.");
        ui.add_space(3.25);
        let snapshot = self.memory.snapshot();
        text_block(ui, &format!("MagoAgent: {}", snapshot.app_bytes.map(memory::size).unwrap_or_else(|| "indisponível".into())), BODY, k.fg);
        for t in &snapshot.terminals {
            text_block(ui, &format!("{}: {} em {} processos", t.name, memory::size(t.bytes), t.processes), BODY, k.fg);
        }
        let total: u64 = snapshot.terminals.iter().map(|t| t.bytes).sum();
        let line = format!(
            "Agentes: {} · {} terminais · {} portais · {} IDEs",
            memory::size(total),
            snapshot.terminals.len(),
            self.scene.elements.iter().filter(|e| e.portal.is_some()).count(),
            self.scene.elements.iter().filter(|e| e.ide.is_some()).count()
        );
        text_block(ui, &line, BODY, k.fg);
        ui.add_space(6.5);
        note(ui, k, "A memória dos agentes e das páginas dos portais pertence a processos separados, não ao MagoAgent.");
    }

    fn settings_control(&mut self, ui: &mut egui::Ui, k: &K) {
        title(ui, k, "HOOKS DO CLAUDE CODE");
        note(
            ui,
            k,
            "O claude que o app inicia (terminais e chats) avisa o app de cada passo: pedidos de permissão, perguntas, erros e quando termina. O Maestro pode responder às permissões, e a linha de status dessas sessões passa a ser a do app (modelo, contexto e o limite do plano).",
        );
        ui.add_space(3.25);
        toggle(
            ui,
            k,
            "Acompanhar os agentes pelos hooks",
            "Vale para o que começar depois; precisa de um Claude Code com --settings.",
            &mut self.prefs.agent_hooks,
        );
        ui.add_space(6.5);
        note(ui, k, "Usa um arquivo de ajustes só do app (--settings), sem mexer nos seus.");
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
        let head = galley(ui, &skill.name, BODY, BODY_LINE, k.fg, ui.available_width());
        let body = galley(ui, &skill.description, SMALL, SMALL_LINE, k.muted_fg, ui.available_width());
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

/// A paragraph at `size`, wrapped to the column.
fn text_block(ui: &mut egui::Ui, text: &str, size: f32, color: Color32) {
    let line = if size == BODY { BODY_LINE } else { SMALL_LINE };
    let g = galley(ui, text, size, line, color, ui.available_width());
    let (rect, _) = ui.allocate_exact_size(g.size().max(Vec2::new(ui.available_width(), 0.0)), Sense::hover());
    ui.painter().galley(rect.min, g, color);
}
