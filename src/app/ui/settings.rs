//! Ajustes (⌘,), after the reference app's: a modal with the sections on the left (Geral,
//! Terminal, Chat, API, Avatar, Sobre) and the section's options on the right.

use crate::prelude::*;

pub(crate) const SETTINGS_TABS: [&str; 9] = ["Geral", "Terminal", "IDE", "Chat", "Voz", "API", "Avatar", "Desempenho", "Sobre"];
pub(crate) const LANGUAGES: [&str; 3] = ["Automático", "Português", "English"];

/// The chat's agent run on an API key instead of its own login: the variable and the key, when
/// the person turned that on and typed one.
pub(crate) fn api_env(prefs: &Prefs, agent: &str) -> Option<(&'static str, String)> {
    if !prefs.use_api_keys {
        return None;
    }
    let (name, key) = if agent == "codex" { ("OPENAI_API_KEY", &prefs.openai_key) } else { ("ANTHROPIC_API_KEY", &prefs.anthropic_key) };
    let key = key.trim();
    (!key.is_empty()).then(|| (name, key.to_string()))
}

impl DrawingApp {
    pub(crate) fn settings_ui(&mut self, ctx: &Context) {
        let c = self.colors();
        let mut close = ctx.input(|i| i.key_pressed(Key::Escape));
        let (mut remove, mut add, mut reset) = (None, false, false);
        egui::Area::new(egui::Id::unique("settings")).order(egui::Order::Foreground).anchor(egui::Align2::CENTER_CENTER, Vec2::ZERO).show(ctx, |ui| {
            egui::Frame::new()
                .fill(c.island)
                .corner_radius(14)
                .inner_margin(egui::Margin::symmetric(22, 18))
                .stroke(Stroke::new(1.0, c.surface_high))
                .shadow(egui::epaint::Shadow { offset: [0, 12], blur: 36, spread: 0, color: Color32::from_black_alpha(90) })
                .show(ui, |ui| {
                    ui.set_width(DIALOG_WIDTH + 190.0);
                    ui.horizontal(|ui| {
                        ui.label(egui::RichText::new("Ajustes").size(15.0).strong().color(c.text));
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            let x = square_button(ui, 26.0, false, Color32::TRANSPARENT, &c);
                            for (a, b) in [(Vec2::new(-4.5, -4.5), Vec2::new(4.5, 4.5)), (Vec2::new(-4.5, 4.5), Vec2::new(4.5, -4.5))] {
                                ui.painter().line_segment([x.rect.center() + a, x.rect.center() + b], Stroke::new(1.5, c.muted));
                            }
                            close |= x.on_hover_text("Fechar (Esc)").clicked();
                        });
                    });
                    ui.add_space(10.0);
                    ui.horizontal_top(|ui| {
                        // the sections, down the left
                        ui.vertical(|ui| {
                            ui.set_width(130.0);
                            for (i, name) in SETTINGS_TABS.iter().enumerate() {
                                let (rect, response) = ui.allocate_exact_size(Vec2::new(130.0, 30.0), Sense::click());
                                let chosen = self.settings_tab == i;
                                if chosen || response.hovered() {
                                    ui.painter().rect_filled(rect, 8, if chosen { c.selected } else { c.surface_low });
                                }
                                let color = if chosen { c.text } else { c.muted };
                                ui.painter().text(
                                    rect.left_center() + Vec2::new(12.0, 0.0),
                                    egui::Align2::LEFT_CENTER,
                                    name,
                                    FontId::proportional(13.0),
                                    color,
                                );
                                if response.on_hover_cursor(CursorIcon::PointingHand).clicked() {
                                    self.settings_tab = i;
                                    self.settings_edit = None;
                                }
                            }
                        });
                        ui.add_space(16.0);
                        // the minimum too, or it keeps the height the area first measured (400 at most,
                        // `default_area_size`) and a longer tab scrolls early
                        egui::ScrollArea::vertical().max_height(480.0).min_scrolled_height(480.0).show(ui, |ui| {
                            // the scroll area takes the row's layout: without this its sections go across
                            ui.vertical(|ui| {
                                ui.set_width(DIALOG_WIDTH);
                                // a short tab is as tall as a long one: the dialog keeps its place and the
                                // sections on the left stay under the pointer that picks them
                                ui.set_min_height(480.0);
                                match self.settings_tab {
                                    0 => self.settings_general(ui, &c),
                                    1 => (remove, add, reset) = self.settings_terminal(ui, &c),
                                    2 => self.settings_ide(ui, &c),
                                    3 => self.settings_chat(ui, &c),
                                    4 => self.settings_voice(ui, &c),
                                    5 => self.settings_api(ui, &c),
                                    6 => self.settings_avatar(ui, &c),
                                    7 => self.settings_performance(ui, &c),
                                    _ => settings_about(ui, &c),
                                }
                            });
                        });
                    });
                });
        });
        if let Some(i) = remove {
            self.prefs.presets.remove(i);
            self.settings_edit = None;
        }
        if add {
            self.prefs.presets.push(Preset { name: String::new(), command: String::new(), on: true });
            self.settings_edit = Some(self.prefs.presets.len() - 1);
            ctx.memory_mut(|m| m.request_focus(preset_field(self.prefs.presets.len() - 1, 0)));
        }
        if reset {
            self.prefs.presets = default_presets();
            self.settings_edit = None;
        }
        if close {
            self.settings_open = false;
            self.settings_edit = None;
        }
    }

    fn settings_performance(&mut self, ui: &mut egui::Ui, c: &UiColors) {
        section(ui, "CONSUMO", "O limite do plano, como o Claude Code informa, e o que segura o Maestro ao recrutar agentes.", c);
        ui.label(format!("Plano: {}.", self.plan_line()));
        for (name, line) in self.agent_usage_lines() {
            ui.label(format!("{name}: {line}"));
        }
        let label = |ui: &mut egui::Ui, text: &str| {
            ui.add_space(8.0);
            ui.label(egui::RichText::new(text).size(11.5).color(dialog_label(c)));
        };
        label(ui, "Agentes rodando ao mesmo tempo (acima disso o Maestro não recruta)");
        let mut ceiling = [3, 6, 10].iter().position(|n| *n == self.prefs.max_agents).unwrap_or(3);
        if segmented(ui, &mut ceiling, &["3", "6", "10", "Sem teto"], c) {
            self.prefs.max_agents = [3, 6, 10, 0][ceiling];
        }
        label(ui, "Parar de recrutar com a janela de 5 horas do plano em");
        let mut stop = [70, 80, 90].iter().position(|n| *n == self.prefs.usage_stop).unwrap_or(3);
        if segmented(ui, &mut stop, &["70%", "80%", "90%", "Nunca"], c) {
            self.prefs.usage_stop = [70, 80, 90, 0][stop];
        }
        label(ui, "Modelo dos agentes que o Maestro recruta (Claude Code)");
        let models = ["", "sonnet", "haiku"];
        let mut model = models.iter().position(|m| *m == self.prefs.recruit_model).unwrap_or(0);
        if segmented(ui, &mut model, &["O da CLI", "Sonnet", "Haiku"], c) {
            self.prefs.recruit_model = models[model].into();
        }
        ui.add_space(12.0);
        section(ui, "MEMÓRIA", "Atualizada a cada 2 segundos.", c);
        let snapshot = self.memory.snapshot();
        ui.label(format!("MagoAgent: {}", snapshot.app_bytes.map(memory::size).unwrap_or_else(|| "indisponível".into())));
        for terminal in &snapshot.terminals {
            ui.label(format!("{}: {} em {} processos", terminal.name, memory::size(terminal.bytes), terminal.processes));
        }
        let total: u64 = snapshot.terminals.iter().map(|t| t.bytes).sum();
        ui.separator();
        ui.label(format!(
            "Agentes: {} · {} terminais · {} portais · {} IDEs",
            memory::size(total),
            snapshot.terminals.len(),
            self.scene.elements.iter().filter(|e| e.portal.is_some()).count(),
            self.scene.elements.iter().filter(|e| e.ide.is_some()).count()
        ));
        ui.label(egui::RichText::new("A memória dos agentes e das páginas dos portais pertence a processos separados, não ao MagoAgent.").color(c.muted));
    }

    fn settings_general(&mut self, ui: &mut egui::Ui, c: &UiColors) {
        section(ui, "IDIOMA", "O Maestro fala no idioma em que você fala com ele; aqui você fixa um.", c);
        segmented(ui, &mut self.prefs.language, &LANGUAGES, c);
        section(ui, "APARÊNCIA", "Sistema segue a configuração do seu sistema operacional.", c);
        let mut theme = match self.prefs.theme {
            Theme::System => 0,
            Theme::Light => 1,
            Theme::Dark => 2,
        };
        if segmented(ui, &mut theme, &["Sistema", "Claro", "Escuro"], c) {
            self.prefs.theme = [Theme::System, Theme::Light, Theme::Dark][theme];
        }
        section(ui, "NAVEGAÇÃO", "", c);
        choice(ui, "Abrir o app em", ["Canvas", "Chat"], &mut self.prefs.start_in_chat, c);
        choice(ui, "Dispositivo de entrada", ["Trackpad", "Mouse"], &mut self.prefs.mouse_wheel_zooms, c);
        section(ui, "CANVAS", "", c);
        choice(ui, "Modo de seleção por caixa", ["Contido", "Sobreposto"], &mut self.prefs.box_overlap, c);
        toggle(ui, "Grade", "Mostra a grade e encaixa nela o que você desenha e move.", &mut self.prefs.grid, c);
        toggle(ui, "Mostrar dicas", "Dicas sobre os botões e atalhos.", &mut self.prefs.show_hints, c);
    }

    fn settings_ide(&mut self, ui: &mut egui::Ui, c: &UiColors) {
        section(ui, "EDITOR", "", c);
        toggle(ui, "Ligaduras no código", "=>, != e :: como um só sinal na fonte do IDE.", &mut self.prefs.ligatures, c);
    }

    fn settings_terminal(&mut self, ui: &mut egui::Ui, c: &UiColors) -> (Option<usize>, bool, bool) {
        let (mut remove, mut add, mut reset) = (None, false, false);
        section(ui, "LINHAS DE HISTÓRICO", "Vale para os terminais abertos depois da mudança.", c);
        let mut history = match self.prefs.terminal_history {
            1_000 => 0,
            5_000 => 1,
            _ => 2,
        };
        if segmented(ui, &mut history, &["1.000", "5.000", "10.000"], c) {
            self.prefs.terminal_history = [1_000, 5_000, 10_000][history];
        }
        ui.add_space(12.0);
        ui.label(egui::RichText::new("PRESETS DE INÍCIO RÁPIDO").size(11.0).strong().color(dialog_label(&c)));
        ui.label(egui::RichText::new("Até 10 presets ativos aparecem ao criar um terminal.").size(12.0).color(dialog_label(&c)));
        ui.add_space(6.0);
        group_frame(&c).inner_margin(6).show(ui, |ui| {
            ui.set_width(ui.available_width());
            for i in 0..self.prefs.presets.len() {
                let editing = self.settings_edit == Some(i);
                ui.horizontal(|ui| {
                    ui.set_min_height(38.0);
                    let preset = &mut self.prefs.presets[i];
                    // the check: a filled circle when it shows in the dialog
                    let (dot, check) = ui.allocate_exact_size(Vec2::splat(22.0), Sense::click());
                    if preset.on {
                        ui.painter().circle_filled(dot.center(), 8.0, PRIMARY);
                        paint_icon(ui.painter(), Icon::Check, dot.center(), Color32::WHITE);
                    } else {
                        ui.painter().circle_stroke(dot.center(), 7.5, Stroke::new(1.2, c.muted));
                    }
                    if check.on_hover_text(if preset.on { "Esconder do diálogo" } else { "Mostrar no diálogo" }).clicked() {
                        preset.on = !preset.on;
                    }
                    let (mark, _) = ui.allocate_exact_size(Vec2::splat(24.0), Sense::hover());
                    terminal_icon(ui.painter(), &TerminalSpec { command: preset.command.clone(), ..Default::default() }, mark.center(), c.text);
                    if editing {
                        for (k, (text, hint, width)) in
                            [(&mut preset.name, "Nome", 120.0), (&mut preset.command, "comando (vazio: shell)", 170.0)].into_iter().enumerate()
                        {
                            let field = egui::TextEdit::singleline(text).id(preset_field(i, k)).hint_text(hint).desired_width(width);
                            ui.add(field.margin(egui::Margin::symmetric(8, 5)).background_color(c.island));
                        }
                    } else {
                        ui.vertical(|ui| {
                            ui.spacing_mut().item_spacing.y = 0.0;
                            let (name, color) = if preset.name.trim().is_empty() { ("sem nome", c.muted) } else { (preset.name.as_str(), c.text) };
                            ui.label(egui::RichText::new(name).size(13.5).color(color));
                            let command = if preset.command.is_empty() { "shell" } else { &preset.command };
                            ui.label(egui::RichText::new(command).size(11.5).monospace().color(c.muted));
                        });
                    }
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        // ⊖, red under the pointer, as in the reference app
                        let (rect, button) = ui.allocate_exact_size(Vec2::splat(26.0), Sense::click());
                        let tint = if button.hovered() { Color32::from_rgb(0xe0, 0x31, 0x31) } else { c.muted };
                        ui.painter().circle_stroke(rect.center(), 7.5, Stroke::new(1.2, tint));
                        ui.painter().hline(rect.center().x - 3.5..=rect.center().x + 3.5, rect.center().y, Stroke::new(1.4, tint));
                        if button.on_hover_cursor(CursorIcon::PointingHand).on_hover_text("Remover").clicked() {
                            remove = Some(i);
                        }
                        let (icon, tip) = if editing { (Icon::Check, "Pronto") } else { (Icon::Edit, "Editar") };
                        if icon_button(ui, 26.0, editing, Color32::TRANSPARENT, icon, c.muted, &c).on_hover_text(tip).clicked() {
                            self.settings_edit = if editing { None } else { Some(i) };
                            // straight into the name, ready to type
                            ui.memory_mut(|m| m.request_focus(preset_field(i, 0)));
                        }
                    });
                });
            }
        });
        ui.add_space(10.0);
        ui.horizontal(|ui| {
            let button = |text: &str| {
                egui::Button::new(egui::RichText::new(text).size(12.5).color(c.text)).fill(c.surface_low).corner_radius(8).min_size(Vec2::new(0.0, 28.0))
            };
            add = ui.add(button("+  Adicionar preset")).clicked();
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                reset = ui.add(button("Redefinir para padrão")).clicked();
            });
        });
        section(ui, "AGENTES", "", c);
        toggle(
            ui,
            "Acompanhar os agentes pelos hooks do Claude Code",
            "O claude que o app inicia (terminais e chats) avisa o app de cada passo: pedidos de permissão, perguntas, erros e quando termina, \
             e o Maestro pode responder às permissões. Usa um arquivo de ajustes só do app (--settings), sem mexer nos seus. Vale para o que \
             começar depois; precisa de um Claude Code com --settings.",
            &mut self.prefs.agent_hooks,
            c,
        );
        (remove, add, reset)
    }

    fn settings_chat(&mut self, ui: &mut egui::Ui, c: &UiColors) {
        section(ui, "CHAT", "Um chat novo já começa assim, sem perguntar. Modelo e esforço ficam nos chips sob a caixa do chat.", c);
        ui.label(egui::RichText::new("Agente").size(11.5).color(dialog_label(c)));
        ui.add_space(4.0);
        let mut agent = (self.prefs.chat_agent == "codex") as usize;
        if segmented(ui, &mut agent, &["Claude Code", "Codex"], c) {
            self.prefs.chat_agent = ["claude", "codex"][agent].into();
        }
        ui.add_space(8.0);
        dialog_folder(ui, "Pasta de trabalho", &mut self.prefs.chat_folder, "Pasta projetos do MagoAgent", "Pasta dos chats", c, &mut self.prefs.folders);
        ui.add_space(8.0);
        ui.label(egui::RichText::new("O agente pode").size(11.5).color(dialog_label(c)));
        ui.add_space(4.0);
        segmented(ui, &mut self.prefs.chat_mode, &crate::app::chat_direct::CHAT_MODES, c);
    }

    fn settings_voice(&mut self, ui: &mut egui::Ui, c: &UiColors) {
        section(
            ui,
            "MOTOR",
            "Codex: a voz do app do Codex, que conversa de verdade (dá para interromper) e passa os pedidos ao Codex na pasta do chat; usa o login do ChatGPT do Codex, dentro do limite do seu plano. OpenAI: ouve e fala pela API (gpt-transcribe e gpt-4o-mini-tts), com a chave de Ajustes › API, cobrada por uso. Local: whisper.cpp e Piper neste computador, sem internet.",
            c,
        );
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
        let mut engine = engines.iter().position(|e| *e == now).unwrap_or(0);
        if segmented(ui, &mut engine, engines, c) {
            self.prefs.voice_codex = engines[engine] == "Codex";
            if !self.prefs.voice_codex {
                self.prefs.voice_openai = engines[engine] == "OpenAI";
            }
        }
        if codex && self.prefs.voice_codex {
            section(ui, "VOZ", "A voz do Codex. Vale na próxima chamada.", c);
            ui.horizontal_wrapped(|ui| {
                for (id, name) in crate::voice_codex::CODEX_VOICES {
                    if ui.selectable_label(self.prefs.codex_voice == id, name).clicked() {
                        self.prefs.codex_voice = id.into();
                    }
                }
            });
            return;
        }
        if self.prefs.voice_openai {
            section(ui, "VOZ", "A voz com que o Maestro fala. Vale na próxima frase.", c);
            ui.horizontal_wrapped(|ui| {
                for (id, name) in crate::voice_openai::OPENAI_VOICES {
                    if ui.selectable_label(self.prefs.openai_voice == id, name).clicked() {
                        self.prefs.openai_voice = id.into();
                    }
                }
            });
            ui.add_space(6.0);
            if ui.button("▶ Ouvir esta voz").clicked() {
                match crate::voice_openai::key(&self.prefs) {
                    Some(key) => self.voice_sample = crate::voice_openai::say(&key, &self.prefs.openai_voice, "Olá! Eu sou o Maestro do MagoAgent."),
                    None => self.toast("Ponha a chave da OpenAI em Ajustes › API."),
                }
            }
        } else {
            self.settings_local_voice(ui, c);
        }
        section(ui, "PAUSA", "Quanto silêncio encerra o que você diz. Uma pausa menor que isso faz parte da frase; aumente se ele corta você no meio.", c);
        ui.add(egui::Slider::new(&mut self.prefs.voice_pause, 1.0..=4.0).step_by(0.25).suffix(" s"));
        section(
            ui,
            "COMANDOS DE VOZ",
            "Chame pelo nome do agente: \"Claude, se muta\" (ele pergunta se é para ficar mudo; com um sim, para de ouvir a conversa e avisa com um toque quando tem resposta), \"Claude, pode falar\", \"Claude, desmuta\", \"Claude, <pedido>\".",
            c,
        );
    }

    /// Piper's voices and whisper.cpp's GPU, when the call runs on this computer.
    fn settings_local_voice(&mut self, ui: &mut egui::Ui, c: &UiColors) {
        let fetching = crate::install::LOCAL_VOICE.lock().unwrap().clone();
        let note = if fetching.is_empty() {
            "A voz com que o Maestro fala. Vale na próxima vez que a voz for ligada.".to_string()
        } else {
            format!("A voz com que o Maestro fala. Em segundo plano: {fetching}")
        };
        section(ui, "VOZ", &note, c);
        ui.vertical(|ui| {
            // the Portuguese voices, then the English ones
            for (title, lang) in [("Português do Brasil", "pt_BR")] {
                ui.label(egui::RichText::new(title).size(11.5).color(dialog_label(c)));
                ui.horizontal_wrapped(|ui| {
                    for voice in crate::voice::VOICES.iter().filter(|v| v.starts_with(lang)) {
                        if ui.selectable_label(self.prefs.voice == *voice, crate::voice::voice_name(voice)).clicked() {
                            self.prefs.voice = voice.to_string();
                        }
                    }
                });
                ui.add_space(4.0);
            }
            ui.add_space(6.0);
            if ui.button("▶ Ouvir esta voz").clicked() {
                let piper = crate::voice::voice_dir().and_then(|d| crate::voice::piper(&d, &self.prefs.voice));
                let pt = self.prefs.voice.starts_with("pt");
                let text = if pt { "Olá! Eu sou o Maestro do MagoAgent." } else { "Hi! I'm the MagoAgent Maestro." };
                self.voice_sample = Player::say(text, piper.as_ref(), if pt { "pt" } else { "en" });
            }
        });
        let (gpu, usable) = crate::voice::gpu();
        section(ui, "PROCESSAMENTO", &format!("GPU encontrada: {gpu}."), c);
        toggle(
            ui,
            "Usar a GPU para ouvir",
            if usable {
                "O whisper.cpp roda na GPU com o modelo grande, que entende melhor o português."
            } else {
                "Sem build de GPU pronto aqui: fica na CPU."
            },
            &mut self.prefs.voice_gpu,
            c,
        );
    }

    fn settings_api(&mut self, ui: &mut egui::Ui, c: &UiColors) {
        section(
            ui,
            "LOCAL OU API",
            "Desligado, os chats rodam o claude e o codex com o login deles (sua assinatura). Ligado, rodam com a chave de API abaixo, cobrada por uso.",
            c,
        );
        toggle(
            ui,
            "Usar as chaves de API nos chats",
            "Vale a partir da próxima mensagem; o claude recebe ANTHROPIC_API_KEY e o codex OPENAI_API_KEY.",
            &mut self.prefs.use_api_keys,
            c,
        );
        section(ui, "CHAVES DE API", "Ficam só neste computador, em prefs.json.", c);
        dialog_secret(ui, "Anthropic", &mut self.prefs.anthropic_key, "sk-ant-…", c);
        dialog_secret(ui, "OpenAI", &mut self.prefs.openai_key, "sk-…", c);
    }

    fn settings_avatar(&mut self, ui: &mut egui::Ui, c: &UiColors) {
        section(ui, "AVATAR", "A barra do Maestro, por cima de tudo na tela, com o mascote do agente do chat.", c);
        toggle(ui, "Mostrar o avatar", "O × da barra o esconde.", &mut self.prefs.avatar, c);
        let mut follow = self.prefs.avatar_at.is_none();
        toggle(ui, "Seguir a janela em uso", "Desligado, a barra fica onde você a deixar: arraste-a por qualquer ponto.", &mut follow, c);
        if follow != self.prefs.avatar_at.is_none() {
            self.avatar_track_window(follow);
        }
    }
}

fn settings_about(ui: &mut egui::Ui, c: &UiColors) {
    section(ui, "SOBRE", "", c);
    ui.label(egui::RichText::new(format!("{APP_TITLE} {}", env!("CARGO_PKG_VERSION"))).size(13.5).color(c.text));
    ui.label(
        egui::RichText::new(
            "Tudo roda neste computador: os agentes são o claude e o codex instalados, com o login deles, salvo as chaves de API em Ajustes › API.",
        )
        .size(12.0)
        .color(dialog_label(c)),
    );
}

/// A section's title and, when there is one, its line of explanation.
fn section(ui: &mut egui::Ui, title: &str, hint: &str, c: &UiColors) {
    ui.add_space(12.0);
    ui.label(egui::RichText::new(title).size(11.0).strong().color(dialog_label(c)));
    if !hint.is_empty() {
        ui.label(egui::RichText::new(hint).size(12.0).color(dialog_label(c)));
    }
    ui.add_space(6.0);
}

/// A two-way choice as pills: the second label is `true`.
fn choice(ui: &mut egui::Ui, label: &str, labels: [&str; 2], second: &mut bool, c: &UiColors) {
    ui.add_space(4.0);
    ui.label(egui::RichText::new(label).size(11.5).color(dialog_label(c)));
    let mut i = *second as usize;
    if segmented(ui, &mut i, &labels, c) {
        *second = i == 1;
    }
}

/// A switch with its label and explanation.
fn toggle(ui: &mut egui::Ui, label: &str, hint: &str, value: &mut bool, c: &UiColors) {
    ui.add_space(6.0);
    ui.horizontal(|ui| {
        let (rect, response) = ui.allocate_exact_size(Vec2::new(36.0, 20.0), Sense::click());
        let on = *value;
        ui.painter().rect_filled(rect, 10.0, if on { PRIMARY } else { c.surface_high });
        let knob = Pos2::new(if on { rect.right() - 10.0 } else { rect.left() + 10.0 }, rect.center().y);
        ui.painter().circle_filled(knob, 8.0, Color32::WHITE);
        if response.on_hover_cursor(CursorIcon::PointingHand).clicked() {
            *value = !on;
        }
        ui.add_space(6.0);
        ui.vertical(|ui| {
            ui.spacing_mut().item_spacing.y = 2.0;
            ui.label(egui::RichText::new(label).size(13.0).color(c.text));
            if !hint.is_empty() {
                ui.label(egui::RichText::new(hint).size(11.5).color(dialog_label(c)));
            }
        });
    });
}

/// The name (0) or command (1) box of preset `i` while it is edited.
fn preset_field(i: usize, k: usize) -> egui::Id {
    egui::Id::unique(("preset_field", i, k))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_api_keys_reach_the_agents_only_when_turned_on() {
        let mut prefs = Prefs { anthropic_key: " sk-ant-1 ".into(), openai_key: String::new(), ..Default::default() };
        assert_eq!(api_env(&prefs, "claude"), None, "off by default");
        prefs.use_api_keys = true;
        assert_eq!(api_env(&prefs, "claude"), Some(("ANTHROPIC_API_KEY", "sk-ant-1".to_string())));
        assert_eq!(api_env(&prefs, "codex"), None, "no key typed");
        prefs.openai_key = "sk-2".into();
        assert_eq!(api_env(&prefs, "codex"), Some(("OPENAI_API_KEY", "sk-2".to_string())));
    }

    #[test]
    fn every_section_of_the_settings_shows() {
        let mut app = DrawingApp::default();
        app.settings_open = true;
        let ctx = Context::default();
        for tab in 0..SETTINGS_TABS.len() {
            app.settings_tab = tab;
            ctx.run_ui(egui::RawInput::default(), |ui| app.settings_ui(ui.ctx())).textures_delta.clear();
        }
        assert!(app.settings_open);
    }

    /// The texts tab `tab` of Ajustes paints, where.
    fn shown(app: &mut DrawingApp, tab: usize) -> Vec<(String, Rect)> {
        app.settings_open = true;
        app.settings_tab = tab;
        let ctx = Context::default();
        let mut texts = vec![];
        // a new area shows from its second frame
        for _ in 0..2 {
            let mut output = ctx.run_ui(egui::RawInput::default(), |ui| app.settings_ui(ui.ctx()));
            output.textures_delta.clear();
            texts = output
                .shapes
                .iter()
                .filter_map(|s| if let egui::Shape::Text(t) = &s.shape { Some((t.galley.text().to_string(), t.visual_bounding_rect())) } else { None })
                .collect::<Vec<_>>();
        }
        texts
    }

    #[test]
    fn a_tabs_sections_go_down_not_across() {
        let texts = shown(&mut DrawingApp::default(), 0);
        let at = |name: &str| texts.iter().find(|(t, _)| t == name).map(|(_, r)| *r).unwrap_or_else(|| panic!("{name}: {texts:?}"));
        let (idioma, aparencia) = (at("IDIOMA"), at("APARÊNCIA"));
        assert!(aparencia.top() > idioma.bottom() && (aparencia.left() - idioma.left()).abs() < 1.0, "{idioma:?} then {aparencia:?}");
    }

    /// The dialog is centered: a tab shorter than the last would move the list under the pointer.
    #[test]
    fn the_sections_list_stays_put_from_a_long_tab_to_a_short_one() {
        let sobre = |tab: usize| shown(&mut DrawingApp::default(), tab).into_iter().find(|(t, _)| t == "Sobre").map(|(_, r)| r).expect("the list names Sobre");
        let (long, short) = (sobre(0), sobre(6));
        assert!((long.top() - short.top()).abs() < 1.0, "{long:?} on Geral, {short:?} on Avatar");
    }

    /// What the ☰ menu held lives here now: the canvas's in Geral, the editor's in IDE.
    #[test]
    fn ajustes_holds_what_the_menu_had() {
        let mut app = DrawingApp::default();
        let has = |texts: &[(String, Rect)], name: &str| texts.iter().any(|(t, _)| t == name);
        let geral = shown(&mut app, 0);
        for name in ["Abrir o app em", "Dispositivo de entrada", "Modo de seleção por caixa", "Grade", "Mostrar dicas"] {
            assert!(has(&geral, name), "{name} in Geral: {geral:?}");
        }
        assert!(!has(&geral, "Ligaduras no código"));
        let ide = shown(&mut app, SETTINGS_TABS.iter().position(|t| *t == "IDE").unwrap());
        assert!(has(&ide, "Ligaduras no código"), "{ide:?}");
    }
}
