//! Atalhos, Dados and Sobre as rows, and the question before Limpar todos os dados.

use super::*;
use crate::app::ui::settings;

impl DrawingApp {
    pub(super) fn settings_swift_hotkeys(&self) -> Vec<Group> {
        let mut names: Vec<&str> = hotkeys::every().map(|def| def.group).collect();

        names.dedup();

        let mut groups: Vec<Group> = names
            .into_iter()
            .map(|name| {
                let mut items = vec![];

                if name == GLOBAL_GROUP {
                    items.push(note("Valem com qualquer programa na frente. Clique nas teclas de um atalho para gravar outras."));

                    if !self.prefs.island {
                        items.push(note("A ilha está desligada: eles voltam a valer quando ela for ligada em Ajustes › Geral."));
                    }
                }

                items.extend(hotkeys::every().filter(|def| def.group == name).map(|def| self.settings_swift_shortcut(def)));

                if name == GLOBAL_GROUP && self.hotkeys.statuses.contains(&hotkeys::Status::Unsupported) {
                    items.push(note(hotkeys::Status::Unsupported.label()));
                }

                group(name, items)
            })
            .collect();

        groups.push(group("TECLAS FIXAS", vec![note(&crate::app::ui::keys_label(settings::hotkeys::FIXED))]));
        groups
    }

    /// One shortcut as `hotkeys.rs`'s row tells it: the keys (or what is being recorded), the line under its name.
    pub(super) fn settings_swift_shortcut(&self, def: &'static ShortcutDef) -> Item {
        use crate::app::hotkeys::editor::Pending;
        let stored = &self.prefs.shortcuts;
        let editing = self.hotkeys.recording.clone().filter(|editing| editing.id == def.id);
        let (global, app) = (def.scope() == Scope::Global, def.scope() == Scope::App);

        let hint = match &editing {
            Some(editing) => {
                let warning = match editing.pending {
                    None if global => hotkeys::ALTGR_NOTE.to_string(),
                    Some(Pending::Keys(combo)) if global && editing.problem.is_none() => hotkeys::altgr_warning(&keys::format(&combo)).unwrap_or_default(),
                    _ => String::new(),
                };

                format!("{} {warning}", editing.hint()).trim().to_string()
            }
            None if global => {
                let index = ACTIONS.iter().position(|other| other.id == def.id);
                let status = index.and_then(|index| self.hotkeys.statuses.get(index)).map_or("", |status| status.label());
                let binding = hotkeys::effective(def, stored);
                let warning = hotkeys::altgr_warning(&binding.keys).filter(|_| binding.enabled).unwrap_or_default();

                format!("{status} {warning}").trim().to_string()
            }
            None if matches!(def.act, hotkeys::Act::App(action) if action.everywhere()) => "Vale até com o teclado num terminal ou num chat.".into(),
            None => String::new(),
        };

        let text = match editing.as_ref().map(|editing| editing.pending) {
            Some(Some(Pending::Keys(combo))) => keys::combo_label(&combo, app),
            Some(Some(Pending::Clear)) => keys::display(""),
            Some(Some(Pending::Default)) => hotkeys::shown_keys(def, &hotkeys::Bindings::new()),
            // Swift shows the modifiers held while it waits
            Some(None) => String::new(),
            None => hotkeys::shown_keys(def, stored),
        };

        let keys = Keys {
            text,
            editing: editing.is_some(),
            pending: editing.as_ref().is_some_and(|editing| editing.pending.is_some()),
            problem: editing.as_ref().is_some_and(|editing| editing.problem.is_some()),
            global,
            enabled: hotkeys::enabled(def, stored),
            can_clear: !hotkeys::keys_of(def, stored).is_empty(),
            changed: stored.contains_key(def.id),
        };

        Item { keys: Some(keys), ..item(def.id, "hotkey", def.label, &hint) }
    }

    pub(super) fn settings_swift_data(&self) -> Vec<Group> {
        let drawing = self.autosave.clone().or_else(autosave_path);
        let folder = drawing.as_ref().and_then(|path| path.parent().map(|parent| parent.to_path_buf()));
        let open = |folder: &std::path::Path, label: &str| vec![button("open_path", label, Some(&folder.to_string_lossy()))];

        let mut canvas = vec![note(
            "O canvas se salva sozinho a cada mudança. Cada espaço de trabalho tem o seu desenho, um arquivo .mago; a lista dos espaços fica no espacos.json, na mesma pasta.",
        )];

        if let (Some(drawing), Some(folder)) = (&drawing, &folder) {
            let space = self.spaces.list.iter().find(|space| space.id == self.spaces.current).map(|space| space.name.clone());

            let hint = match space {
                Some(name) => format!("{} (espaço {name})", drawing.display()),
                None => drawing.display().to_string(),
            };

            canvas.push(row("data_drawing", "Desenho em uso", &hint, open(folder, "Abrir pasta")));
        }

        let mut app = vec![];

        if let Some(folder) = &folder {
            app.push(row("data_folder", "Pasta de dados", &folder.display().to_string(), open(folder, "Abrir")));

            app.push(row(
                "data_what",
                "O que fica nela",
                "Os desenhos e os espaços de trabalho, as tarefas, o fichário, as preferências e o que foi recuperado de um fechamento. Tudo neste computador.",
                vec![],
            ));
        }

        let memory = if cfg!(test) { String::new() } else { crate::app::chat::tools::memory_text() };
        let notes = memory.lines().filter(|line| !line.trim().is_empty()).count();

        let hint = match notes {
            0 => "Nada guardado ainda.".to_string(),
            1 => "1 anotação sobre como você trabalha, para as próximas conversas.".to_string(),
            count => format!("{count} anotações sobre como você trabalha, para as próximas conversas."),
        };

        let file = crate::app::chat::tools::memory_path().filter(|path| path.is_file());

        app.push(row("data_memory", "Memória da Luna", &hint, file.iter().flat_map(|path| open(path, "Abrir")).collect()));
        let mut chats = vec![];

        if let Some(folder) = &folder {
            let dir = folder.join("chats");
            let hint = format!("{}: a lista dos chats (chats.json) e o que foi dito nos do Codex (a pasta chats).", folder.display());

            chats.push(row("data_chats", "Chats do app", &hint, open(if dir.is_dir() { &dir } else { folder }, "Abrir")));
        }

        if let Some(projects) = crate::transcript::claude_dir().map(|dir| dir.join("projects")) {
            let hint = format!("{}: o Claude Code guarda as conversas dele, e o app as lê de lá.", projects.display());

            chats.push(row("data_claude", "Conversas do Claude Code", &hint, if projects.is_dir() { open(&projects, "Abrir") } else { vec![] }));
        }

        if let Some(projects) = projects_dir() {
            let hint = format!("{}: onde trabalha um chat sem pasta escolhida.", projects.display());

            chats.push(row("data_projects", "Chats sem pasta", &hint, if projects.is_dir() { open(&projects, "Abrir") } else { vec![] }));
        }

        let wipe = vec![Button { destructive: true, ..button("wipe", "Limpar…", None) }];

        vec![
            group("DESENHOS DO CANVAS", canvas),
            group("DADOS DO APP", app),
            group("CHATS", chats),
            group(
                "LIMPAR",
                vec![row(
                    "wipe",
                    "Limpar todos os dados",
                    "Apaga os desenhos, os chats, os ajustes e as senhas guardadas e abre o MagoAgent do zero. Pergunta antes.",
                    wipe,
                )],
            ),
        ]
    }

    pub(super) fn settings_swift_about(&self) -> Vec<Group> {
        let state = &self.updates.state;
        let busy = matches!(state, UpdateState::Checking | UpdateState::Downloading(_));
        let available = matches!(state, UpdateState::Available(_));

        let buttons = if available {
            vec![Button { primary: true, ..button("update_install", "Atualizar", None) }]
        } else if !busy {
            vec![button("update_check", "Verificar atualizações", None)]
        } else {
            vec![]
        };

        let mut version = vec![row("version", &format!("{APP_TITLE} {}", env!("CARGO_PKG_VERSION")), &self.updates.state_line(), buttons)];

        version.push(select(
            "update_check",
            "Procurar atualizações",
            "Sempre: ao abrir o app e a cada hora. Ao abrir: só quando o app abre. Nunca: só por este botão.",
            &["Sempre", "Ao abrir", "Nunca"],
            usize::from(self.prefs.update_check.min(2)),
        ));

        if self.updates.confirming && available {
            version.push(note(CLOSES_AND_REOPENS));

            version.push(row(
                "update_confirm",
                "",
                "",
                vec![Button { primary: true, ..button("update_install_now", "Atualizar e reiniciar", None) }, button("update_cancel", "Cancelar", None)],
            ));
        }

        let release = match state {
            UpdateState::Available(r) | UpdateState::UpToDate(r) | UpdateState::Downloading(r) | UpdateState::Started(r) => Some(r.clone()),
            _ => None,
        };

        let mut groups = vec![group("VERSÃO", version)];

        match release {
            Some(release) => {
                let mut news = vec![];

                if matches!(state, UpdateState::Available(release) if !installs_in_place(release)) {
                    news.push(note("Neste sistema a atualização se baixa pela página da release: o botão a abre."));
                }

                let notes = plain_notes(&release.notes);

                news.push(item("", "notes", "", if notes.is_empty() { "Essa release não traz notas." } else { &notes }));
                news.push(row("release", "", "", vec![button("open", "Abrir a página da release", Some(&release.page))]));
                groups.push(group(&format!("NOVIDADES DA {}", release.version), news));
            }
            None => groups[0].items.push(row("releases", "", "", vec![button("open", "Ver as releases no GitHub", Some(RELEASES_PAGE))])),
        }

        let rows = [
            ("O app", "Não envia nada por conta própria. Olha por uma versão nova no GitHub como Procurar atualizações manda."),
            ("Agentes", "O Claude Code e o Codex instalados falam com os serviços deles, com o login de cada um."),
            ("Navegadores", "Sites comuns, cada navegador com o seu perfil."),
            ("Microfone", "Só fica aberto durante a chamada; o que você diz vai ao motor de Ajustes › Voz (o VoiceStudio do seu computador ou a OpenAI)."),
        ];

        let mut privacy: Vec<Item> = rows.iter().map(|(label, hint)| info(label, hint)).collect();

        privacy.push(note("Tudo roda neste computador: o desenho, os chats e os dados do app ficam na pasta de Ajustes › Dados."));
        groups.push(group("PRIVACIDADE", privacy));
        groups
    }
}

/// What Limpar todos os dados says before it deletes (the egui question's lines, then the folders).
pub(super) fn wipe_lines() -> Vec<String> {
    let plan = crate::app::system::wipe::plan();

    let mut lines = vec![
        "O MagoAgent fecha os agentes e os terminais, apaga e abre de novo como na primeira vez. Vão embora:".to_string(),
        "• os desenhos, os espaços, as notas e as tarefas".into(),
        "• os chats e as conversas guardadas".into(),
        "• os ajustes e a memória da Luna".into(),
    ];

    if plan.keychain {
        lines.push("• o cofre de senhas do navegador e os tokens do GitHub, do Spotify e do WhatsApp".into());
        lines.push("• a pasta ~/.magoagent, onde trabalham os chats sem pasta escolhida".into());
    } else {
        lines.push("Esta é a versão de desenvolvimento: só a pasta dela é apagada (o cofre e o ~/.magoagent são do app instalado).".into());
    }

    lines.push("Os hooks instalados no Claude Code e no Codex ficam como estão. Não dá para desfazer.".into());
    lines.push(plan.folders.iter().map(|folder| folder.display().to_string()).collect::<Vec<_>>().join("\n"));
    lines
}
