//! The pages of Ajustes as rows: what each egui page (`general.rs` and its siblings) draws, as data for Swift.

use super::*;
use crate::app::ui::settings;

impl DrawingApp {
    pub(super) fn native_general(&self) -> Vec<Group> {
        let prefs = &self.prefs;
        let theme = match prefs.theme {
            Theme::System => 0,
            Theme::Light => 1,
            Theme::Dark => 2,
        };
        let rates: Vec<String> = FRAME_RATES.iter().map(|rate| rate.to_string()).collect();
        let system = crate::app::system::run_at_login::system_name();
        // ponytail: the saved place only (`island_at`), the island's live anchor is private to island/; none saved is the top
        let place = prefs
            .island_at
            .map_or(Some(Place::Top), |anchor| crate::app::island::macos::work_at(probe(anchor)).and_then(|work| Place::of(anchor, work)))
            .and_then(|place| Place::ALL.iter().position(|each| *each == place))
            .map_or(-1, |index| index as i64);
        let close = settings::general::CLOSE_AFTER.iter().position(|seconds| *seconds == prefs.island_close_after).unwrap_or(2);
        let compact = settings::general::COMPACT_AFTER.iter().position(|seconds| *seconds == prefs.island_compact_after).unwrap_or(2);
        let wheel = if prefs.wheel_auto { 0 } else { 1 + usize::from(prefs.mouse_wheel_zooms) };
        let outline = WindowOutline::ALL.iter().position(|outline| *outline == prefs.window_outline).unwrap_or(0);
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
            group(
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
                    choice(
                        "island_close_after",
                        "Fechar sozinho",
                        "Quanto a ilha aberta espera depois que o mouse sai.",
                        &["5 s", "10 s", "15 s", "30 s"],
                        close,
                    ),
                    choice(
                        "island_compact_after",
                        "Encolher para o notch",
                        "Quanto a ilha compacta (a pílula com a Luna, a música e os agentes) espera, depois que o mouse sai, até virar o notch pequeno.",
                        &["15 s", "30 s", "1 min", "5 min", "Nunca"],
                        compact,
                    ),
                ],
            ),
            group(
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
            ),
            group(
                "NAVEGAÇÃO E EDITOR",
                vec![
                    choice(
                        "wheel",
                        "Dispositivo",
                        "Automático: a roda de um mouse dá zoom e o touchpad move o canvas.",
                        &["Automático", "Trackpad", "Mouse"],
                        wheel,
                    ),
                    stepper("zoom_step", "Passo do zoom", "O quanto cada passo da roda muda o zoom.", i64::from(prefs.zoom_step), (2, 50), "%"),
                    toggle("ligatures", "Ligaduras no código", "=>, != e :: como um só sinal na Árvore de Arquivos.", prefs.ligatures),
                    text(
                        "hidden_shown",
                        "Ocultos que aparecem",
                        "Com os ocultos escondidos, a Árvore de Arquivos ainda mostra estes (separados por vírgula).",
                        &prefs.hidden_shown,
                        ".claude, .idea",
                        false,
                    ),
                ],
            ),
        ]
    }

    pub(super) fn native_terminal(&self) -> Vec<Group> {
        let prefs = &self.prefs;
        let mut presets = vec![note("Até 10 presets ativos aparecem ao criar um terminal.")];
        for (index, preset) in prefs.presets.iter().enumerate() {
            let icon = crate::app::ui::TERMINAL_ICONS.iter().position(|key| *key == preset.icon).unwrap_or(0);
            presets.push(Item {
                value: json!({ "on": preset.on, "name": preset.name, "command": preset.command, "icon": icon, "first": index == 0, "last": index + 1 == prefs.presets.len() }),
                options: crate::app::ui::TERMINAL_ICON_NAMES.iter().map(|name| name.to_string()).collect(),
                ..item(&format!("preset.{index}"), "preset", &preset.name, &preset.command)
            });
        }
        presets.push(row("presets", "", "", vec![button("presets_add", "+  Adicionar Preset", None), button("presets_reset", "Redefinir para Padrão", None)]));
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

    pub(super) fn native_agents(&self) -> Vec<Group> {
        use crate::app::system::mcp_register::*;
        let mut groups = vec![];
        let lines = self.agent_usage_lines();
        if !lines.is_empty() {
            let mut items = vec![note("O que cada agente que já respondeu está usando.")];
            items.extend(lines.iter().map(|(name, line)| info(name, line)));
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
        let mut mcp = vec![note("Registra o MCP do MagoAgent no Claude Code ou no Codex de fora do app, para as sessões de lá também usarem o canvas.")];
        for (agent, name) in MCP_AGENTS {
            let state = match mcp_status(agent) {
                _ if mcp_busy(agent) => "Verificando…".to_string(),
                McpStatus::Checking => "Verificando…".into(),
                McpStatus::Registered => "Registrado".into(),
                McpStatus::NotRegistered => "Não registrado".into(),
                McpStatus::Missing => format!("{name} não encontrado"),
            };
            mcp.push(row(&format!("mcp.{agent}"), name, &state, vec![button("mcp_register", &format!("Registrar no {name}"), Some(agent))]));
        }
        groups.push(group("MCP EM OUTROS AGENTES", mcp));
        groups
    }

    pub(super) fn native_chat(&self) -> Vec<Group> {
        let prefs = &self.prefs;
        let mode = prefs.chat_mode.min(crate::app::chat::direct::CHAT_MODES.len() - 1);
        let mut choices = vec!["Padrão do modelo"];
        choices.extend(crate::app::chat::context::COMPACT_CHOICES);
        let compact = crate::app::chat::context::COMPACT_CHOICES.iter().position(|size| *size == prefs.chat_autocompact).map_or(0, |index| index + 1);
        let recents: Vec<String> = prefs.folders.choices().into_iter().map(|(path, _)| path).collect();
        let mut folder = text("chat_folder", "Pasta de trabalho", "", &prefs.chat_folder, "~/.magoagent", false);
        // the menu "Recentes" (Swift's Procurar… is the system's folder panel)
        folder.options = recents;
        folder.buttons = vec![button("choose_folder", "Procurar…", None)];
        let mut skill = text(
            "style_skill",
            "Skill de estilo",
            "O SKILL.md com o seu estilo de código: a etapa Padronização do fluxo da Luna manda o agente lê-lo e conferir o diff regra a regra.",
            &prefs.style_skill,
            "~/Skill/SKILL.md",
            false,
        );
        skill.buttons = vec![button("style_skill_open", "Abrir", None)];
        let mut island = vec![toggle(
            "agent_hooks",
            "Hooks nos agentes do app",
            "O Claude Code que o app inicia avisa a ilha de cada passo (perguntas, permissões, fim da tarefa) pelos hooks. Vale para as sessões que começarem depois.",
            prefs.agent_hooks,
        )];
        if !prefs.island {
            island.push(toggle("island", "Mostrar a ilha", "A ilha está desligada: sem ela, nada disso aparece. É a mesma chave de Ajustes › Geral.", false));
        }
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
                    skill,
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
            group("ILHA", island),
        ]
    }

    pub(super) fn native_voice(&self) -> Vec<Group> {
        use crate::voice::studio::{StudioServer, StudioVoices};
        let prefs = &self.prefs;
        let engines = ["Codex", "VoiceStudio", "OpenAI"];
        let engine = if prefs.voice_codex {
            0
        } else if prefs.voice_openai {
            2
        } else {
            1
        };
        let mut groups = vec![group(
            "MOTOR",
            vec![
                note(
                    "Codex: a voz do app do Codex, que conversa de verdade; está fora do ar (a ChatGPT responde 404), então a chamada não liga com ele. VoiceStudio: ouve e fala por um servidor do VoiceStudio neste computador (open source, com a voz que você escolher da lista dele), sem chave e sem custo. OpenAI: ouve e fala pela API (gpt-transcribe e gpt-4o-mini-tts), com a sua chave da OpenAI (abaixo), cobrada por uso. O app não baixa modelos nem programas de voz.",
                ),
                select("voice_engine", "Motor da chamada", "", &engines, engine),
            ],
        )];
        fn voices<'a>(list: &'a [(&'a str, &'a str)], now: &str) -> (Vec<&'a str>, usize) {
            (list.iter().map(|(_, name)| *name).collect(), list.iter().position(|(id, _)| *id == now).unwrap_or(0))
        }
        if prefs.voice_codex {
            let (names, now) = voices(&crate::voice::codex::CODEX_VOICES, &prefs.codex_voice);
            groups.push(group("VOZ", vec![select("codex_voice", "Voz", "A voz do Codex. Vale na próxima chamada.", &names, now)]));
            return groups;
        }
        if !prefs.voice_openai {
            let server = StudioServer::from_prefs(prefs);
            let mut servidor = vec![
                note(
                    "O VoiceStudio é um programa à parte: abra o app dele ou suba o servidor (docker run -d -p 127.0.0.1:3900:3900 ghcr.io/debpalash/voicestudio:stable) e a chamada fala e ouve por ele.",
                ),
                text(
                    "studio_url",
                    "Endereço",
                    "Onde o servidor responde. O padrão é o do VoiceStudio.",
                    &prefs.voice_studio_url,
                    crate::voice::studio::STUDIO_URL,
                    false,
                ),
                text(
                    "studio_key",
                    "Chave de API",
                    "Só se o servidor pedir (OMNIVOICE_API_KEY). Fica neste computador, em prefs.json.",
                    &prefs.voice_studio_key,
                    "opcional",
                    false,
                ),
            ];
            // the key is the person's own, on this computer: shown as the egui page does, but never as plain text
            servidor[2].secret = true;
            groups.push(group("SERVIDOR", servidor));
            let list = self.studio_voices.lock().unwrap().clone();
            let mut voice = vec![];
            match &list {
                StudioVoices::Done(found) if !found.is_empty() => {
                    let names: Vec<&str> = found.iter().map(|(_, name)| name.as_str()).collect();
                    let now = found.iter().position(|(id, _)| *id == server.voice).unwrap_or(0);
                    voice.push(select("studio_voice", "Voz", "A voz com que a Luna fala, da lista do servidor. Vale na próxima frase.", &names, now));
                }
                StudioVoices::Failed(why) => voice.push(note(why)),
                StudioVoices::Done(_) => voice.push(note("O servidor não listou nenhuma voz.")),
                _ => voice.push(note("Buscando as vozes do servidor…")),
            }
            voice.push(row("voice_play", "Ouvir esta voz", "", vec![button("voice_play", "▶ Ouvir", None)]));
            voice.push(row(
                "studio_refresh",
                "Atualizar a lista",
                "Depois de criar ou clonar uma voz no VoiceStudio.",
                vec![button("studio_refresh", "Atualizar", None)],
            ));
            groups.push(group("VOZ", voice));
        } else {
            let (names, now) = voices(&crate::voice::openai::OPENAI_VOICES, &prefs.openai_voice);
            groups.push(group(
                "VOZ",
                vec![
                    select("openai_voice", "Voz", "A voz com que a Luna fala. Vale na próxima frase.", &names, now),
                    text("openai_key", "Chave da OpenAI", "Fica só neste computador, em prefs.json.", &prefs.openai_key, "sk-…", true),
                    row("voice_play", "Ouvir esta voz", "", vec![button("voice_play", "▶ Ouvir", None)]),
                ],
            ));
        }
        let mics = crate::voice::input_names();
        let mut names = vec!["Automático".to_string()];
        names.extend(mics.iter().cloned());
        let mic = mics.iter().position(|mic| *mic == prefs.voice_mic).map_or(0, |index| index + 1);
        groups.push(group(
            "MICROFONE",
            vec![select(
                "voice_mic",
                "Microfone",
                "O que a chamada e o Ditar ouvem. Automático usa o do sistema, menos um cabo virtual (que só manda silêncio).",
                &names,
                mic,
            )],
        ));
        let quarters = (prefs.voice_pause * 4.0).round().clamp(4.0, 16.0) as i64;
        let mut pause = stepper(
            "voice_pause",
            "Pausa",
            "Quanto silêncio encerra o que você diz. Uma pausa menor que isso faz parte da frase; aumente se ele corta você no meio.",
            quarters,
            (4, 16),
            " s",
        );
        // quarters of a second, shown as seconds with a comma ("1,5 s")
        pause.placeholder = "quarters".into();
        groups.push(group("PAUSA", vec![pause]));
        groups.push(group(
            "COMANDOS DE VOZ",
            vec![note("Chame pelo nome do agente: \"Claude, se muta\" (ele pergunta se é para ficar mudo; com um sim, para de ouvir a conversa e avisa com um toque quando tem resposta), \"Claude, pode falar\", \"Claude, desmuta\", \"Claude, <pedido>\".")],
        ));
        groups
    }

    pub(super) fn native_ok_mago(&self) -> Vec<Group> {
        let (label, hint) = if self.voice.on {
            ("Encerrar a chamada", "A chamada está ligada: diga “Ok Mago” para testar.")
        } else {
            ("Iniciar a chamada", "Liga o microfone no motor escolhido em Ajustes › Voz.")
        };
        vec![
            group(
                "CHAMAR",
                vec![
                    toggle(
                        "ok_mago",
                        "Responder a “Ok Mago”",
                        "Na chamada de voz, dizer “Ok Mago, …” chama a Luna com o pedido, mesmo com a chamada em espera (depois do “se muta”). O nome do agente (Claude, Codex) continua valendo.",
                        self.prefs.ok_mago,
                    ),
                    row("call", "Chamada de voz", hint, vec![button("call_toggle", label, None)]),
                ],
            ),
            group(
                "RESPOSTAS",
                vec![toggle(
                    "speak_answers",
                    "Responder em voz alta",
                    "Desligado, a chamada começa com os alto-falantes silenciados e a Luna responde só por texto (o botão do alto-falante liga de novo).",
                    self.prefs.speak_answers,
                )],
            ),
            group(
                "MICROFONE",
                vec![note(
                    "O microfone só fica aberto durante a chamada: o app não escuta o tempo todo (isso pediria um modelo de voz local, e o app não baixa modelos). O que você diz vai ao motor de Ajustes › Voz, o VoiceStudio do seu computador ou a OpenAI.",
                )],
            ),
        ]
    }
}
