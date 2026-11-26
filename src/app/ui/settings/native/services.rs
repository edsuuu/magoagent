//! Ajustes › Integrações as rows: the hooks and the MCP outside the app, then GitHub, Spotify, Hermes Agent and WhatsApp.

use super::*;
use crate::app::ui::settings;

impl DrawingApp {
    pub(super) fn settings_swift_integrations(&self) -> Vec<Group> {
        use crate::app::integrations::{github, hermes, whatsapp::Link as Gateway};
        use crate::app::maestro::claude_hooks::{self, HooksState};
        let prefs = &self.prefs;

        let hooks = |state: HooksState, path: Option<&std::path::Path>, preview: bool, id: &str| {
            let (label, labels) = settings::hooks::hooks_row(state, preview);
            // the egui row lays them out from the right edge: the first one is the rightmost
            let buttons = labels.iter().rev().map(|name| button(if *name == "Remover" { "hooks_remove" } else { "hooks_install" }, name, Some(id))).collect();

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

        let mut groups = vec![group("CLAUDE CODE FORA DO APP", claude), group("CODEX FORA DO APP", codex), group("MCP EM OUTROS AGENTES", mcp_rows())];
        // then a card for each service under one title (the page's line says what they are and where the secrets go)
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
            let state = &integrations.github;

            // with a token saved only its line shows (Remover brings the field back)
            if !state.has_token() {
                let mut token = text(
                    "github_token",
                    "Token pessoal",
                    "Clássico com o escopo repo, ou fine-grained com leitura em Commit statuses e Actions e leitura e escrita em Pull requests (para salvar a descrição e comentar pela ilha).",
                    "",
                    "ghp_… ou github_pat_…",
                    250.0,
                    true,
                );

                token.buttons = vec![button("open", "Criar token…", Some(github::TOKENS_PAGE)), button("github_save", "Salvar", None)];
                github_items.push(token);
                github_items.push(note("Sem token: crie um, cole acima e salve. Ele vai para o cofre do sistema, nunca para um arquivo."));
            } else {
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
                // a column a week, each day its line and its level (0 to 4)
                let weeks: Vec<Vec<[u8; 2]>> = activity.weeks.iter().map(|week| week.iter().map(|day| [day.weekday, day.level]).collect()).collect();

                github_items.push(Item { value: json!(weeks), ..item("", "calendar", "", "") });
            }
        }

        groups.push(group("SERVIÇOS", github_items));

        // Spotify: what plays shows on the island, not here
        let mut spotify = vec![head(
            "spotify",
            "Spotify",
            "A música que toca, na ilha, com a capa e os botões. Sem conta e sem chave: fala com o Spotify aberto neste computador.",
            integrations.spotify.now.is_some(),
            prefs.integrations.spotify,
        )];

        if prefs.integrations.spotify {
            spotify.push(toggle(
                "luna_dances",
                "A Luna dança com a música",
                "Na ilha, ao lado da música. No macOS ela balança no ritmo do relógio: o app não ouve o som do sistema.",
                prefs.integrations.luna_dances,
            ));

            spotify.extend(self.settings_swift_spotify_account());
        }

        groups.push(group("", spotify));

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
                        vec![button("hermes_install", "Instalar", None), button("hermes_cancel", "Cancelar", None)],
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

            groups.push(group("", items));
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
                250.0,
                false,
            ));

            let mut credential = text(
                "whatsapp_credential",
                "Credencial",
                "Credencial GOWA no formato usuário:senha, quando o gateway pede; vazio para gateway sem autenticação.",
                "",
                "user:pass ou vazio",
                250.0,
                true,
            );

            credential.buttons = vec![button("whatsapp_save", "Salvar", None)];
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

        groups.push(Group { scroll_id: Some("whatsapp"), ..group("", items) });
        groups
    }

    /// Conta do Spotify, in steps until there is a Client ID (`settings_spotify_account`): with it only Conectar shows
    /// (Desconectar once linked); a failed login shows the steps again.
    fn settings_swift_spotify_account(&self) -> Vec<Item> {
        use crate::app::integrations::spotify::account::{DASHBOARD, Link, REDIRECT_URI, client_id_looks_right};
        let account = &self.integrations.spotify.account;
        let has_app = client_id_looks_right(self.prefs.integrations.spotify_client_id.trim());

        let (status, detail) = match &account.link {
            Link::Linked if account.name.is_empty() => ("Conta do Spotify conectada".to_string(), "O token fica no cofre do sistema.".to_string()),
            Link::Linked => (format!("Conectada como {}", account.name), "O token fica no cofre do sistema.".to_string()),
            Link::Waiting => ("Esperando o navegador…".to_string(), "Autorize no site do Spotify que abriu; depois volte aqui.".to_string()),
            Link::Failed(why) => ("Conta do Spotify não conectada".to_string(), why.clone()),
            Link::Off if has_app => {
                ("Conta do Spotify".to_string(), "Traz a fila, as Curtidas, as suas playlists, a busca e o coração para a ilha.".to_string())
            }
            Link::Off => (
                "Conta do Spotify".to_string(),
                "Opcional, para a fila, as Curtidas, as playlists e a busca na ilha: crie um app no painel do Spotify e siga os passos.".to_string(),
            ),
        };

        let link = match account.link {
            Link::Linked => vec![button("spotify_disconnect", "Desconectar", None)],
            Link::Waiting => vec![button("spotify_cancel", "Cancelar", None)],
            Link::Off | Link::Failed(_) if has_app => vec![button("spotify_connect", "Conectar", None)],
            Link::Off | Link::Failed(_) => vec![],
        };

        let mut items = vec![row("spotify_account", &status, &detail, link)];

        // the steps, until the Client ID is pasted (and again when a login failed, to fix the app)
        if !has_app || matches!(account.link, Link::Failed(_)) {
            items.push(row(
                "spotify_dashboard",
                "1. Abrir o painel do Spotify",
                "No painel do Spotify para desenvolvedores: Create app, com Web API marcada.",
                vec![button("open", "Abrir painel", Some(DASHBOARD))],
            ));

            items.push(row(
                "spotify_redirect",
                "2. Copiar o Redirect URI",
                &format!("{REDIRECT_URI}: cole em Redirect URIs, no app criado, e salve."),
                vec![Button { copy: Some(REDIRECT_URI.into()), ..button("spotify_redirect_copy", "Copiar", None) }],
            ));

            items.push(text(
                "spotify_client_id",
                "3. Colar o Client ID",
                "Está na página do app, no painel. Sem segredo: o app usa PKCE.",
                &self.prefs.integrations.spotify_client_id,
                "32 letras e números",
                250.0,
                false,
            ));
        }

        items
    }
}

/// MCP em outros agentes (`mcp.rs`): each agent, how its registration stands and Registrar or Remover.
fn mcp_rows() -> Vec<Item> {
    use crate::app::system::mcp_register::*;
    let mut items = vec![note("Registra o MCP do MagoAgent no Claude Code ou no Codex de fora do app, para as sessões de lá também usarem o canvas.")];

    for (agent, name) in MCP_AGENTS {
        let (status, doing) = (mcp_status(agent), mcp_doing(agent));

        let state = match (doing, status) {
            (Some(doing), _) => doing.to_string(),
            (None, McpStatus::Checking) => "Verificando…".into(),
            (None, McpStatus::Registered) => "Registrado".into(),
            (None, McpStatus::NotRegistered) => "Não registrado".into(),
            (None, McpStatus::Missing) => format!("{name} não encontrado"),
        };

        let buttons = match mcp_action(status, doing.is_some()) {
            Some(McpAction::Remove) => vec![button("mcp_remove", "Remover", Some(agent))],
            Some(McpAction::Register) => vec![button("mcp_register", &format!("Registrar no {name}"), Some(agent))],
            None => vec![],
        };

        items.push(row(&format!("mcp.{agent}"), name, &state, buttons));
    }

    items
}
