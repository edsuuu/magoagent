//! The pages of Ajustes as rows: what each egui page (`general.rs` and its siblings) draws, as data for Swift.

use super::*;
use crate::app::ui::settings;

impl DrawingApp {
    pub(super) fn settings_swift_general(&self) -> Vec<Group> {
        let prefs = &self.prefs;

        let theme = match prefs.theme {
            Theme::System => 0,
            Theme::Light => 1,
            Theme::Dark => 2,
        };

        let rates: Vec<String> = FRAME_RATES.iter().map(|rate| rate.to_string()).collect();
        let system = crate::app::system::run_at_login::system_name();

        vec![
            group(
                "APARÊNCIA",
                vec![
                    choice("theme", "Tema", "Sistema segue o do seu sistema operacional.", &["Sistema", "Claro", "Escuro"], theme),
                    select("language", "Idioma", "A Luna fala no idioma em que você fala com ela; aqui você fixa um.", &settings::LANGUAGES, prefs.language),
                    choice(
                        "frame_rate",
                        "Quadros por segundo",
                        "Da ilha e das animações. Mais fica mais suave e gasta mais processador; acima da taxa da sua tela não muda nada.",
                        &rates,
                        FRAME_RATES.iter().position(|rate| *rate == prefs.frame_rate).unwrap_or(1),
                    ),
                ],
            ),
            group(
                "INÍCIO",
                vec![
                    choice("start_in_chat", "Abrir o app em", "", &["Canvas", "Chat"], usize::from(prefs.start_in_chat)),
                    toggle(
                        "run_at_login",
                        &format!("Iniciar com o {system}"),
                        &format!("Abre o MagoAgent ao entrar na sua conta do {system}."),
                        prefs.run_at_login,
                    ),
                    toggle(
                        "close_to_background",
                        "Fechar mantém em segundo plano",
                        "Fechar a janela deixa o MagoAgent rodando, com os terminais e os agentes. Ele volta pelo ícone perto do relógio; Ctrl+Q sai de verdade.",
                        prefs.close_to_background,
                    ),
                    row(
                        "tour",
                        "Apresentação",
                        "O passo a passo que mostra onde fica cada parte do app.",
                        vec![button("tour", "Ver a apresentação de novo", None)],
                    ),
                ],
            ),
        ]
    }

    pub(super) fn settings_swift_island(&self) -> Vec<Group> {
        let prefs = &self.prefs;
        // none chosen when it was let go somewhere else on the screen; never moved, the SwiftUI island is where it starts,
        // the top (it saves no place until it is moved)
        let place = crate::app::native::context()
            .and_then(|ctx| self.island_place(ctx))
            .or_else(|| prefs.island_at.is_none().then_some(Place::Top))
            .and_then(|place| Place::ALL.iter().position(|each| *each == place))
            .map_or(-1, |index| index as i64);

        let close = settings::island::CLOSE_AFTER.iter().position(|seconds| *seconds == prefs.island_close_after).unwrap_or(2);
        let compact = settings::island::COMPACT_AFTER.iter().position(|seconds| *seconds == prefs.island_compact_after).unwrap_or(2);

        vec![group(
            "ILHA DO MAESTRO",
            vec![
                toggle(
                    "island",
                    "Mostrar a ilha",
                    "Uma pílula no topo da tela, por cima de tudo: os agentes, o que precisa de você, o chat e a voz.",
                    prefs.island,
                ),
                Item {
                    value: json!(place),
                    ..choice(
                        "island_place",
                        "Lugar da ilha",
                        "No topo, embaixo (abre para cima) ou numa lateral, onde fechada ela fica em pé. Solta perto de uma borda, a ilha também gruda nela.",
                        &Place::NAMES,
                        0,
                    )
                },
                row(
                    "island_reset",
                    "Trazer a ilha de volta",
                    "Sumiu ou ficou fora da tela? Volta a ilha ao meio do topo do monitor onde está a janela do MagoAgent.",
                    vec![button("island_reset", "Voltar ao lugar padrão", None)],
                ),
                choice("island_close_after", "Fechar sozinho", "Quanto a ilha aberta espera depois que o mouse sai.", &["5 s", "10 s", "15 s", "30 s"], close),
                choice(
                    "island_compact_after",
                    "Encolher para o notch",
                    "Quanto a ilha compacta (a pílula com a Luna, a música e os agentes) espera, depois que o mouse sai, até virar o notch pequeno.",
                    &["15 s", "30 s", "1 min", "5 min", "Nunca"],
                    compact,
                ),
            ],
        )]
    }

    pub(super) fn settings_swift_canvas(&self) -> Vec<Group> {
        let prefs = &self.prefs;
        let outline = WindowOutline::ALL.iter().position(|outline| *outline == prefs.window_outline).unwrap_or(0);

        vec![group(
            "CANVAS",
            vec![
                choice(
                    "paper",
                    "Fundo",
                    "A grade de 20 px. Para encaixar nela o que você desenha e move, Ctrl+' (Alinhar à grade).",
                    &["Liso", "Grade"],
                    usize::from(prefs.paper),
                ),
                choice(
                    "box_overlap",
                    "Seleção por caixa",
                    "Contido pega só o que cabe inteiro na caixa; Sobreposto, também o que ela toca.",
                    &["Contido", "Sobreposto"],
                    usize::from(prefs.box_overlap),
                ),
                select(
                    "window_outline",
                    "Contorno da seleção",
                    "A borda azul de um terminal, nota, navegador, chat ou árvore selecionado.",
                    &WindowOutline::NAMES,
                    outline,
                ),
                toggle("show_hints", "Dicas nos botões", "Dicas sobre os botões e atalhos.", prefs.show_hints),
            ],
        )]
    }

    pub(super) fn settings_swift_navigation(&self) -> Vec<Group> {
        let prefs = &self.prefs;
        let wheel = if prefs.wheel_auto { 0 } else { 1 + usize::from(prefs.mouse_wheel_zooms) };

        vec![group(
            "NAVEGAÇÃO E EDITOR",
            vec![
                choice("wheel", "Dispositivo", "Automático: a roda de um mouse dá zoom e o touchpad move o canvas.", &["Automático", "Trackpad", "Mouse"], wheel),
                stepper("zoom_step", "Passo do zoom", "O quanto cada passo da roda muda o zoom.", i64::from(prefs.zoom_step), (2, 50), "%"),
                toggle("ligatures", "Ligaduras no código", "=>, != e :: como um só sinal na Árvore de Arquivos.", prefs.ligatures),
                text(
                    "hidden_shown",
                    "Ocultos que aparecem",
                    "Com os ocultos escondidos, a Árvore de Arquivos ainda mostra estes (separados por vírgula).",
                    &prefs.hidden_shown,
                    ".claude, .idea",
                    220.0,
                    false,
                ),
            ],
        )]
    }

    pub(super) fn settings_swift_terminal(&self) -> Vec<Group> {
        let prefs = &self.prefs;
        let mut presets = vec![note("Até 10 presets ativos aparecem ao criar um terminal.")];
        let hex = |[red, green, blue]: [u8; 3]| format!("#{red:02x}{green:02x}{blue:02x}");
        let colors: Vec<Option<String>> = crate::app::ui::TERMINAL_COLORS.iter().map(|color| color.map(hex)).collect();

        for (index, preset) in prefs.presets.iter().enumerate() {
            let icon = crate::app::ui::TERMINAL_ICONS.iter().position(|key| *key == preset.icon).unwrap_or(0);
            let color = crate::app::ui::TERMINAL_COLORS.iter().position(|color| *color == preset.color).unwrap_or(0);
            // the mark: the chosen one, else the command's first word (`icon_key`)
            let mark = if preset.icon.is_empty() { preset.command.split_whitespace().next().unwrap_or_default() } else { &preset.icon };

            presets.push(Item {
                value: json!({
                    "on": preset.on, "name": preset.name, "command": preset.command, "icon": icon, "mark": mark, "color": color,
                    "colors": colors, "editing": self.settings_edit == Some(index), "first": index == 0, "last": index + 1 == prefs.presets.len(),
                }),
                options: crate::app::ui::TERMINAL_ICON_NAMES.iter().map(|name| name.to_string()).collect(),
                ..item(&format!("preset.{index}"), "preset", &preset.name, &preset.command)
            });
        }

        presets.push(button_block("ends", vec![button("presets_add", "+  Adicionar Preset", None), button("presets_reset", "Redefinir para Padrão", None)]));
        let themes = crate::terminal::theme_choices();
        let names: Vec<&str> = themes.iter().map(|(_, name)| *name).collect();
        let range = crate::terminal::FONT_SIZES;
        let size = prefs.terminal.font_size.round().clamp(*range.start(), *range.end()) as i64;
        let shells = crate::app::terminal::prefs::detected_shells();
        let mut shell_names = vec!["Padrão do sistema"];

        shell_names.extend(shells.iter().map(|(name, _)| name.as_str()));

        let history = match prefs.terminal_history {
            1_000 => 0,
            5_000 => 1,
            _ => 2,
        };

        vec![
            group("INÍCIO RÁPIDO", presets),
            group(
                "APARÊNCIA",
                vec![
                    select(
                        "terminal_theme",
                        "Tema",
                        "As cores de todos os terminais.",
                        &names,
                        themes.iter().position(|(id, _)| *id == prefs.terminal.theme).unwrap_or(0),
                    ),
                    stepper(
                        "terminal_font",
                        "Tamanho da fonte",
                        "A fonte é sempre a Hack: só o tamanho muda.",
                        size,
                        (*range.start() as i64, *range.end() as i64),
                        " pt",
                    ),
                ],
            ),
            group(
                "SHELL E COMPORTAMENTO",
                vec![
                    select(
                        "shell",
                        "Shell",
                        "O que um terminal novo abre, quando ele não escolhe outro.",
                        &shell_names,
                        shells.iter().position(|(_, path)| *path == prefs.terminal.shell).map_or(0, |index| index + 1),
                    ),
                    toggle(
                        "copy_on_select",
                        "Copiar ao selecionar",
                        "O que se seleciona num terminal vai para a área de transferência.",
                        prefs.terminal.copy_on_select,
                    ),
                    choice(
                        "terminal_history",
                        "Linhas de histórico",
                        "Vale para os terminais abertos depois da mudança.",
                        &["1.000", "5.000", "10.000"],
                        history,
                    ),
                ],
            ),
        ]
    }

    pub(super) fn settings_swift_agents(&self) -> Vec<Group> {
        let mut groups = vec![];
        let lines = self.agent_usage_lines();

        if !lines.is_empty() {
            let mut items = vec![note("O que cada agente que já respondeu está usando.")];

            items.extend(lines.iter().map(|(name, line)| item("", "usage", name, line)));
            groups.push(group("AGENTES", items));
        }

        let stop = [70, 80, 90].iter().position(|n| *n == self.prefs.usage_stop).unwrap_or(3);

        groups.push(group(
            "LIMITES DO MAESTRO",
            vec![
                note("O que segura a Luna ao recrutar agentes."),
                select(
                    "agent_mode",
                    "Invocação dos agentes",
                    "Terminal: Claude Code completo em um terminal. Chat: conversa leve pelo chat do app, usando o login do CLI; gasta bem menos memória.",
                    &["Terminal", "Chat"],
                    usize::from(self.prefs.agent_mode == AgentMode::Chat),
                ),
                select(
                    "usage_stop",
                    "Parar de recrutar em",
                    "Com a janela de 5 horas do plano nesta porcentagem, a Luna para de recrutar.",
                    &["70%", "80%", "90%", "Nunca"],
                    stop,
                ),
            ],
        ));

        let hint = format!("Os agentes do app não guardam trabalho em {}, que pode sumir ao reiniciar: usam a pasta do projeto.", settings::agents::temp_names());

        groups.push(group(
            "ONDE OS AGENTES TRABALHAM",
            vec![toggle("work_outside_temp", "Trabalho fora da pasta temporária", &hint, self.prefs.work_outside_temp)],
        ));

        groups
    }

    pub(super) fn settings_swift_chat(&self) -> Vec<Group> {
        let prefs = &self.prefs;
        let mode = prefs.chat_mode.min(crate::app::chat::direct::CHAT_MODES.len() - 1);
        let mut choices = vec!["Padrão do modelo"];

        choices.extend(crate::app::chat::context::COMPACT_CHOICES);
        let compact = crate::app::chat::context::COMPACT_CHOICES.iter().position(|size| *size == prefs.chat_autocompact).map_or(0, |index| index + 1);
        let recents: Vec<String> = prefs.folders.choices().into_iter().map(|(path, _)| path).collect();
        let mut folder = text("chat_folder", "Pasta de trabalho", "", &prefs.chat_folder, "~/.magoagent", 230.0, false);

        // the menu "Recentes" (Swift's Procurar… is the system's folder panel)
        folder.options = recents;
        folder.buttons = vec![button("choose_folder", "Procurar…", None)];

        vec![
            group(
                "CHAT",
                vec![
                    note(
                        "Um chat novo já começa assim, sem perguntar; a permissão segue a do próprio agente quando ele define uma (o defaultMode do Claude Code). Modelo e esforço ficam nos chips sob a caixa do chat.",
                    ),
                    select("chat_agent", "Agente", "", &["Claude Code", "Codex"], usize::from(prefs.chat_agent == "codex")),
                    folder,
                    select("chat_mode", "O agente pode", "", &crate::app::chat::direct::CHAT_MODES, mode),
                ],
            ),
            group(
                "CONTEXTO",
                vec![
                    select(
                        "chat_autocompact",
                        "Compactar automaticamente em",
                        "Com quantos tokens o Claude Code compacta a conversa sozinho. Vale para a próxima mensagem de cada chat.",
                        &choices,
                        compact,
                    ),
                    toggle("chat_panel", "Painel Contexto aberto", "O painel ao lado do chat com o uso da janela, as ferramentas e os MCPs.", prefs.chat_panel),
                ],
            ),
        ]
    }

    pub(super) fn settings_swift_skills(&self) -> Vec<Group> {
        let mut style = text(
            "style_skill",
            "Skill de estilo",
            "O SKILL.md com o seu estilo de código: a etapa Padronização do fluxo da Luna manda o agente lê-lo e conferir o diff regra a regra.",
            &self.prefs.style_skill,
            "~/.magoagent/Skills/estilo/SKILL.md",
            230.0,
            false,
        );

        style.buttons = vec![button("style_skill_open", "Abrir", None)];
        let mut skills = vec![note("O Claude Code que o app inicia recebe estas skills de uma pasta só do app: o seu ~/.claude fica como está.")];

        match SKILLS.lock().unwrap_or_else(|poisoned| poisoned.into_inner()).as_ref() {
            None => skills.push(note("Lendo a pasta…")),
            Some(listed) if listed.is_empty() => skills.push(note("Nenhuma ainda: cada skill é uma pasta dentro de skills/, com um SKILL.md.")),
            Some(listed) => skills.extend(listed.iter().map(|skill| info(&skill.name, &skill.description))),
        }

        if let Some(folder) = crate::app::maestro::skills::skills_path() {
            let shown = folder.join("skills").display().to_string();

            skills.push(row("skills_open", "Pasta das skills", &shown, vec![button("skills_open", "Abrir", Some(&folder.to_string_lossy()))]));
        }

        vec![group("SKILL DE ESTILO", vec![style]), group("SKILLS DOS AGENTES DO APP", skills)]
    }
}
