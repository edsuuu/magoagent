//! Ajustes › Integrações as rows: the hooks outside the app, GitHub, Spotify, Hermes Agent and WhatsApp.

use super::*;
use crate::app::ui::settings;

impl DrawingApp {
    pub(super) fn settings_swift_integrations(&self) -> Vec<Group> {
        use crate::app::integrations::{github, hermes, spotify::Player, whatsapp::Link as Gateway};
        use crate::app::maestro::claude_hooks::{self, HooksState};
        let prefs = &self.prefs;

        let hooks = |state: HooksState, path: Option<&std::path::Path>, preview: bool, id: &str| {
            let (label, labels) = settings::hooks::hooks_row(state, preview);

            let buttons = labels
                .iter()
                .map(|name| button(if *name == "Remover" { "hooks_remove" } else { "hooks_install" }, name, Some(id)))
                .map(|button| Button { primary: button.id == "hooks_install", ..button })
                .collect();

            row(&format!("hooks.{id}"), label, &settings::hooks::hooks_hint(state, path), buttons)
        };

        let (path, relay) = (claude_hooks::settings_path(), claude_hooks::relay_path());

        let state = match (&path, &relay) {
            (Some(path), Some(relay)) => claude_hooks::state_at(path, &relay.to_string_lossy()),
            _ => HooksState::Missing,
        };

        let mut claude = vec![note(
            "A ilha mostra as perguntas e os pedidos de permissão de qualquer sessão do Claude Code (o Claude Desktop, o VS Code, outro terminal) e você responde por ela. Instalar acrescenta os hooks do MagoAgent ao seu settings.json, com backup antes e sem apagar nada; Remover tira só os do MagoAgent. Vale para as sessões que começarem depois.",
        )];

        if !prefs.island {
            claude.push(toggle("island", "Mostrar a ilha", "A ilha está desligada: sem ela, nada disso aparece. É a mesma chave de Ajustes › Geral.", false));
        }

        claude.push(hooks(state, path.as_deref(), true, "claude"));
        let codex_path = crate::app::maestro::codex_hooks::codex_settings_path();
        let installed = codex_path.as_ref().is_some_and(|path| crate::app::maestro::codex_hooks::installed_at(path));

        let codex = vec![
            note(
                "Instalar acrescenta os hooks do MagoAgent ao hooks.json do Codex, com backup antes e sem apagar nada; Remover tira só os do MagoAgent. Vale para as sessões novas.",
            ),
            hooks(if installed { HooksState::Installed } else { HooksState::Missing }, codex_path.as_deref(), false, "codex"),
        ];

        let mut groups = vec![group("CLAUDE CODE FORA DO APP", claude), group("CODEX FORA DO APP", codex)];

        let integrations = &self.integrations;

        // GitHub
        let mut github_items = vec![head(
            "github",
            "GitHub",
            "Os seus pull requests com os checks, as revisões que pedem a você, o CI da branch principal dos seus repositórios e a sua atividade.",
            integrations.github.connected(),
            prefs.integrations.github,
        )];

        if prefs.integrations.github {
            let mut token = text(
                "github_token",
                "Token pessoal",
                "Clássico com o escopo repo, ou fine-grained com leitura em Commit statuses e Actions e leitura e escrita em Pull requests (para salvar a descrição e comentar pela ilha).",
                "",
                "ghp_… ou github_pat_…",
                true,
            );

            token.buttons = vec![Button { primary: true, ..button("github_save", "Salvar", None) }, button("open", "Criar token…", Some(github::TOKENS_PAGE))];
            github_items.push(token);
            let state = &integrations.github;

            if state.has_token() {
                let (status, detail) = match (&state.pulse, &state.error) {
                    (_, Some(error)) => ("Não conectado".to_string(), error.clone()),
                    (Some(pulse), None) => (format!("Conectado como @{}", pulse.login), pulse.summary()),
                    (None, None) => ("Falando com o GitHub…".to_string(), "O token está no cofre do sistema.".to_string()),
                };

                github_items.push(row(
                    "github_status",
                    &status,
                    &detail,
                    vec![button("github_test", if state.testing { "Testando…" } else { "Testar" }, None), button("github_remove", "Remover", None)],
                ));
            } else {
                github_items.push(note("Sem token: crie um, cole acima e salve. Ele vai para o cofre do sistema, nunca para um arquivo."));
            }

            if let Some(pulse) = &state.pulse {
                let tone = |ci: github::CiState| match ci {
                    github::CiState::Success => "ok",
                    github::CiState::Failure => "bad",
                    github::CiState::Pending => "warn",
                    github::CiState::Unknown => "",
                };

                for request in pulse.open() {
                    github_items.push(Item {
                        tone: tone(request.ci),
                        suffix: request.ci.label().0.into(),
                        buttons: vec![button("open", &request.state_words(), Some(&request.url))],
                        ..item("", "link", &format!("{}  {}", request.id, request.title), "")
                    });
                }

                for request in &pulse.to_review {
                    github_items.push(Item {
                        tone: "warn",
                        suffix: "•".into(),
                        buttons: vec![button("open", "pede a sua revisão", Some(&request.url))],
                        ..item("", "link", &format!("{}  {}", request.id, request.title), "")
                    });
                }
            }

            if let Some(activity) = &state.activity {
                github_items.push(note(&format!("{} · {} hoje", activity.summary(), activity.today)));
            }
        }

        groups.push(group("GITHUB", github_items));

        // Spotify
        let playing = integrations.spotify.now.as_ref().is_some_and(|now| now.on(Player::App));

        let mut spotify = vec![head(
            "spotify",
            "Spotify",
            "A música que toca, com a capa e os botões de tocar, pausar e pular. Sem conta e sem chave: fala com o Spotify aberto neste computador.",
            playing,
            prefs.integrations.spotify,
        )];

        if prefs.integrations.spotify {
            let now = integrations.spotify.now.clone().filter(|now| now.on(Player::App));

            let buttons = now
                .map(|now| {
                    vec![
                        button("spotify_previous", "Anterior", None),
                        button("spotify_play_pause", if now.playing { "Pausar" } else { "Tocar" }, None),
                        button("spotify_next", "Próxima", None),
                    ]
                })
                .unwrap_or_default();

            spotify.push(row("spotify_player", &integrations.spotify.report_of(Player::App), "", buttons));

            spotify.push(toggle(
                "luna_dances",
                "A Luna dança com a música",
                "Na ilha, ao lado da música. No macOS ela balança no ritmo do relógio: o app não ouve o som do sistema.",
                prefs.integrations.luna_dances,
            ));

            spotify.extend(self.settings_swift_spotify_account());
        }

        groups.push(group("SPOTIFY", spotify));

        if hermes::available() {
            let status = match (prefs.integrations.hermes, hermes::home(), std::env::current_exe()) {
                (true, Some(home), Ok(exe)) => hermes::status_at(&home, &exe.to_string_lossy()),
                _ => hermes::Status::NotInstalled,
            };

            let mut items = vec![head(
                "hermes",
                "Hermes Agent",
                "As sessões do Hermes Agent na ilha: quando começam, trabalham, pedem aprovação e terminam. Por um plugin nosso no seu Hermes.",
                status == hermes::Status::Installed,
                prefs.integrations.hermes,
            )];

            if prefs.integrations.hermes {
                if integrations.hermes_confirm {
                    let will = "Instalar cria a pasta plugins/magoagent no seu Hermes (plugin.yaml e __init__.py) e põe magoagent em plugins.enabled do config.yaml dele, com um backup datado ao lado. O plugin só avisa o MagoAgent do que a sessão faz; nunca manda as mensagens.";

                    items.push(row(
                        "hermes_ask",
                        "Instalar o plugin no Hermes?",
                        will,
                        vec![button("hermes_cancel", "Cancelar", None), Button { primary: true, ..button("hermes_install", "Instalar", None) }],
                    ));
                } else {
                    let mut buttons = vec![];

                    if status != hermes::Status::NoHermes {
                        buttons.push(button("hermes_confirm", "Instalar…", None));
                    }

                    if status != hermes::Status::NoHermes && status != hermes::Status::NotInstalled {
                        buttons.push(button("hermes_remove", "Remover", None));
                    }

                    items.push(row("hermes_plugin", "Plugin do MagoAgent no Hermes", status.words(), buttons));
                }
            }

            groups.push(group("HERMES AGENT", items));
        }

        // WhatsApp
        let whatsapp = &integrations.whatsapp;

        let mut items = vec![head(
            "whatsapp",
            "WhatsApp",
            "Gateway GOWA local já iniciado por você (o app não baixa nem inicia o gateway). Só loopback: 127.0.0.1 ou localhost.",
            whatsapp.link == Gateway::Connected,
            prefs.integrations.whatsapp,
        )];

        if prefs.integrations.whatsapp {
            items.push(text(
                "whatsapp_base",
                "URL do gateway",
                "http://127.0.0.1:3000 (padrão GOWA)",
                &prefs.integrations.whatsapp_base,
                "http://127.0.0.1:3000",
                false,
            ));

            let mut credential = text(
                "whatsapp_credential",
                "Credencial",
                "Credencial GOWA no formato usuário:senha, quando o gateway pede; vazio para gateway sem autenticação.",
                "",
                "user:pass ou vazio",
                true,
            );

            credential.buttons = vec![Button { primary: true, ..button("whatsapp_save", "Salvar", None) }];
            items.push(credential);
            let status = if whatsapp.checking { "Testando…" } else { whatsapp.report() };

            items.push(row(
                "whatsapp_status",
                status,
                "",
                vec![
                    button("whatsapp_test", if whatsapp.checking { "Testando…" } else { "Testar" }, None),
                    button("whatsapp_remove", "Remover credencial", None),
                ],
            ));
        }

        groups.push(group("WHATSAPP", items));
        groups
    }

    /// Conta do Spotify: the status with Conectar, the Client ID of the person's own app and the Redirect URI to register.
    pub(super) fn settings_swift_spotify_account(&self) -> Vec<Item> {
        use crate::app::integrations::spotify::account::{DASHBOARD, Link, REDIRECT_URI};
        let account = &self.integrations.spotify.account;

        let (status, detail) = match &account.link {
            Link::Linked if account.name.is_empty() => ("Conta do Spotify conectada".to_string(), "O token fica no cofre do sistema.".to_string()),
            Link::Linked => (format!("Conectada como {}", account.name), "O token fica no cofre do sistema.".to_string()),
            Link::Waiting => ("Esperando o navegador…".to_string(), "Autorize no site do Spotify que abriu; depois volte aqui.".to_string()),
            Link::Failed(why) => ("Conta do Spotify não conectada".to_string(), why.clone()),
            Link::Off => (
                "Conta do Spotify".to_string(),
                "Opcional: traz a fila do app, as Curtidas, as suas playlists, a busca e o coração para a página do Spotify na ilha.".to_string(),
            ),
        };

        let link = match account.link {
            Link::Linked => button("spotify_disconnect", "Desconectar", None),
            Link::Waiting => button("spotify_cancel", "Cancelar", None),
            Link::Off | Link::Failed(_) => Button { primary: true, ..button("spotify_connect", "Conectar", None) },
        };

        let mut client = text(
            "spotify_client_id",
            "Client ID",
            "O Client ID do seu app no painel do Spotify para desenvolvedores (sem segredo: o app usa PKCE). Sem app: Criar app, cole o Redirect URI abaixo, marque Web API e salve; o Client ID aparece na página dele.",
            &self.prefs.integrations.spotify_client_id,
            "32 letras e números",
            false,
        );

        client.buttons = vec![button("open", "Abrir painel…", Some(DASHBOARD))];

        vec![
            row("spotify_account", &status, &detail, vec![link]),
            client,
            row(
                "spotify_redirect",
                REDIRECT_URI,
                "Redirect URI: cadastre este endereço exato no app do Spotify (Edit settings › Redirect URIs).",
                vec![Button { copy: Some(REDIRECT_URI.into()), ..button("copy", "Copiar", None) }],
            ),
            note(
                "Já tem um Client ID do Spotifast (callback http://127.0.0.1:8989/login)? O mesmo serve: no painel, adicione também o Redirect URI acima ao app. Em Development Mode, a sua conta precisa estar na lista de usuários do app.",
            ),
        ]
    }
}
