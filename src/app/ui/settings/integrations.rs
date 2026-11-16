//! Ajustes › Integrações: the hooks of the sessions outside the app first (`hooks.rs`), then one group per integration
//! (`app/integrations/`): its dot (green connected, red not), its name and what it does, its switch, then its own fields
//! and buttons. Off, only the head shows.

use super::kit::*;
use crate::app::chat::list::clipped;
use crate::app::integrations::spotify::{Command, Player};
use crate::app::integrations::{github, hermes, open_page};
use crate::prelude::*;

const GREEN: Color32 = Color32::from_rgb(0x34, 0xc7, 0x59);
const AMBER: Color32 = Color32::from_rgb(0xf5, 0xa5, 0x24);

/// An integration's head: the dot, the name, what it does under it, and its switch at the right. True when flipped.
fn head(ui: &mut egui::Ui, k: &K, name: &str, hint: &str, connected: bool, value: &mut bool) -> bool {
    let width = ui.available_width();
    let text = galley(ui, name, BODY, BODY_LINE, k.fg, width - 67.0);
    let hint_galley = galley(ui, hint, SMALL, SMALL_LINE, k.muted_fg, width - 67.0);
    let (pad, name_height) = (4.875, text.size().y);
    let (rect, _) = ui.allocate_exact_size(Vec2::new(width, 2.0 * pad + name_height + 1.625 + hint_galley.size().y), Sense::hover());
    ui.painter().circle_filled(Pos2::new(rect.left() + 5.0, rect.top() + pad + name_height / 2.0), 4.0, if connected { GREEN } else { k.destructive });
    ui.painter().galley(Pos2::new(rect.left() + 16.0, rect.top() + pad), text, k.fg);
    ui.painter().galley(Pos2::new(rect.left() + 16.0, rect.top() + pad + name_height + 1.625), hint_galley, k.muted_fg);
    switch(ui, k, Pos2::new(rect.right() - 38.0, rect.top() + pad), name, value)
}

/// A line that opens a page: a mark in its color, the words cut to fit, and what is beside them at the right.
fn link(ui: &mut egui::Ui, k: &K, mark: (&str, Color32), text: &str, beside: &str) -> bool {
    let (rect, response) = ui.allocate_exact_size(Vec2::new(ui.available_width(), 24.0), Sense::click());
    let response = response.on_hover_cursor(CursorIcon::PointingHand);
    response.widget_info(|| egui::WidgetInfo::labeled(egui::Role::Link, true, format!("{text} ({beside})")));
    if response.hovered() {
        ui.painter().rect_filled(rect, 6.5, k.muted.gamma_multiply(if k.dark { 0.5 } else { 1.0 }));
    }
    ui.painter().text(Pos2::new(rect.left() + 9.0, rect.center().y), egui::Align2::CENTER_CENTER, mark.0, FontId::proportional(SMALL), mark.1);
    let beside = ui.painter().layout_no_wrap(beside.to_string(), FontId::proportional(SMALL), k.muted_fg);
    let words = clipped(ui.painter(), text.to_string(), FontId::proportional(SMALL), k.fg, rect.width() - 22.0 - beside.size().x - 16.0);
    ui.painter().galley(Pos2::new(rect.left() + 22.0, rect.center().y - words.size().y / 2.0), words, k.fg);
    ui.painter().galley(Pos2::new(rect.right() - 6.5 - beside.size().x, rect.center().y - beside.size().y / 2.0), beside, k.muted_fg);
    response.clicked()
}

fn check_color(k: &K, state: github::CiState) -> Color32 {
    match state {
        github::CiState::Success => GREEN,
        github::CiState::Failure => k.destructive,
        github::CiState::Pending => AMBER,
        github::CiState::Unknown => k.muted_fg,
    }
}

/// The contribution calendar: a square a day, a column a week, the greener the more was done.
fn calendar(ui: &mut egui::Ui, k: &K, activity: &github::Activity) {
    const CELL: f32 = 8.0;
    let (rect, _) = ui.allocate_exact_size(Vec2::new(ui.available_width(), 7.0 * (CELL + 2.0)), Sense::hover());
    for (column, week) in activity.weeks.iter().enumerate() {
        for (line, level) in week.iter().enumerate() {
            let at = Pos2::new(rect.left() + column as f32 * (CELL + 2.0), rect.top() + line as f32 * (CELL + 2.0));
            let color = if *level == 0 { k.muted } else { k.muted.lerp_to_gamma(GREEN, f32::from(*level) / 4.0) };
            ui.painter().rect_filled(Rect::from_min_size(at, Vec2::splat(CELL)), 2.0, color);
        }
    }
}

impl DrawingApp {
    pub(super) fn settings_integrations(&mut self, ui: &mut egui::Ui, k: &K) {
        // the hooks first, easy to find: the sessions of Claude Code and Codex outside the app (hooks.rs)
        self.settings_hooks(ui, k);
        divider(ui, k);
        // then a card for each service (the page's line says what they are and where the secrets go)
        title(ui, k, "SERVIÇOS");
        block(ui, k, |_| ());
        self.settings_github(ui, k);
        divider(ui, k);
        block(ui, k, |_| ());
        self.settings_spotify(ui, k);
        divider(ui, k);
        // Spotify's Connect receiver is only for Linux
        if cfg!(not(any(windows, target_os = "macos"))) {
            block(ui, k, |_| ());
            self.settings_soloist(ui, k);
            divider(ui, k);
        }
        if crate::app::integrations::hermes::available() {
            block(ui, k, |_| ());
            self.settings_hermes(ui, k);
            divider(ui, k);
        }
        block(ui, k, |_| ());
        self.settings_whatsapp(ui, k);
    }

    fn settings_whatsapp(&mut self, ui: &mut egui::Ui, k: &K) {
        use crate::app::integrations::whatsapp::Link;
        let (checking, link) = (self.integrations.whatsapp.checking, self.integrations.whatsapp.link.clone());
        let connected = link == Link::Connected;
        let hint = "Gateway GOWA local já iniciado por você (o app não baixa nem inicia o gateway). Só loopback: 127.0.0.1 ou localhost.";
        head(ui, k, "WhatsApp", hint, connected, &mut self.prefs.integrations.whatsapp);
        if !self.prefs.integrations.whatsapp {
            return;
        }
        let (mut save_cred, mut test, mut remove_cred, mut base_changed) = (false, false, false, false);
        row(ui, k, "URL do gateway", "http://127.0.0.1:3000 (padrão GOWA)", |ui| {
            let field = input(
                ui,
                k,
                egui::Id::unique("settings_whatsapp_base"),
                "URL do gateway",
                &mut self.prefs.integrations.whatsapp_base,
                "http://127.0.0.1:3000",
                250.0,
                false,
            );
            base_changed = field.changed();
        });
        let hint = "Credencial GOWA no formato usuário:senha, quando o gateway pede; vazio para gateway sem autenticação.";
        row(ui, k, "Credencial", hint, |ui| {
            save_cred = button(ui, k, "Salvar").clicked();
            let field = input(
                ui,
                k,
                egui::Id::unique("settings_whatsapp_credential"),
                "Credencial do gateway",
                &mut self.integrations.whatsapp.credential_field,
                "user:pass ou vazio",
                250.0,
                true,
            );
            save_cred |= field.lost_focus() && ui.input(|input| input.key_pressed(egui::Key::Enter));
        });
        let status = if checking {
            "Testando…"
        } else {
            match link {
                Link::Unknown => "ainda não testado",
                Link::Connected => "gateway conectado",
                Link::NeedsPairing => "gateway disponível; WhatsApp precisa ser pareado nele",
                Link::Unavailable => "gateway indisponível ou resposta inválida",
            }
        };
        row(ui, k, status, "", |ui| {
            remove_cred = button(ui, k, "Remover credencial").clicked();
            test = button(ui, k, if checking { "Testando…" } else { "Testar" }).clicked();
        });
        if base_changed {
            self.integrations.whatsapp.forget();
        }
        if save_cred {
            let credential = std::mem::take(&mut self.integrations.whatsapp.credential_field);
            let said = self.whatsapp_save_credential(&credential).unwrap_or_else(|why| why);
            self.toast(said);
            self.integrations.whatsapp.forget();
        } else if test {
            if !self.whatsapp_test() {
                self.toast("URL inválida: precisa ser http://127.0.0.1:porta ou http://localhost:porta");
            }
        } else if remove_cred {
            let said = self.whatsapp_remove_credential().unwrap_or_else(|why| format!("A credencial não foi removida: {why}"));
            self.toast(said);
            self.integrations.whatsapp.forget();
        }
    }

    /// Hermes Agent: the switch shows its sessions on the island; the buttons put our plugin in the person's Hermes
    /// and take it out, "Instalar…" saying first what it will write.
    fn settings_hermes(&mut self, ui: &mut egui::Ui, k: &K) {
        let status = match (self.prefs.integrations.hermes, hermes::home(), std::env::current_exe()) {
            (true, Some(home), Ok(exe)) => hermes::status_at(&home, &exe.to_string_lossy()),
            _ => hermes::Status::NotInstalled,
        };
        let hint = "As sessões do Hermes Agent na ilha: quando começam, trabalham, pedem aprovação e terminam. Por um plugin nosso no seu Hermes.";
        head(ui, k, "Hermes Agent", hint, status == hermes::Status::Installed, &mut self.prefs.integrations.hermes);
        if !self.prefs.integrations.hermes {
            self.integrations.hermes_confirm = false;
            return;
        }
        let mut action = "";
        if self.integrations.hermes_confirm {
            let will = "Instalar cria a pasta plugins/magoagent no seu Hermes (plugin.yaml e __init__.py) e põe magoagent em plugins.enabled do config.yaml dele, com um backup datado ao lado. O plugin só avisa o MagoAgent do que a sessão faz; nunca manda as mensagens.";
            row(ui, k, "Instalar o plugin no Hermes?", will, |ui| {
                if button(ui, k, "Cancelar").clicked() {
                    action = "cancel";
                }
                if button(ui, k, "Instalar").clicked() {
                    action = "install";
                }
            });
        } else {
            row(ui, k, "Plugin do MagoAgent no Hermes", status.words(), |ui| {
                if status != hermes::Status::NotInstalled && status != hermes::Status::NoHermes && button(ui, k, "Remover").clicked() {
                    action = "remove";
                }
                if status != hermes::Status::NoHermes && button(ui, k, "Instalar…").clicked() {
                    action = "confirm";
                }
            });
        }
        match action {
            "" => {}
            "confirm" => self.integrations.hermes_confirm = true,
            "cancel" => self.integrations.hermes_confirm = false,
            _ => {
                self.integrations.hermes_confirm = false;
                let said = self.act_hermes(action).unwrap_or_else(|why| why);
                self.toast(said);
            }
        }
    }

    fn settings_spotify(&mut self, ui: &mut egui::Ui, k: &K) {
        let hint = if cfg!(any(windows, target_os = "macos")) {
            "A música que toca, com a capa e os botões de tocar, pausar e pular. Sem conta e sem chave: fala com o Spotify aberto neste computador."
        } else {
            "A música que toca, com a capa e os botões de tocar, pausar e pular. Sem conta e sem chave: fala com o Spotify (ou o Spotifast) aberto neste computador."
        };
        let playing = self.integrations.spotify.now.as_ref().is_some_and(|now| now.on(Player::App));
        head(ui, k, "Spotify", hint, playing, &mut self.prefs.integrations.spotify);
        if self.prefs.integrations.spotify {
            self.settings_player(ui, k, Player::App);
        }
    }

    /// Spotify's Soloist (Linux): its own switch, what it plays with the same buttons, and Configurar.
    fn settings_soloist(&mut self, ui: &mut egui::Ui, k: &K) {
        let hint = "O receptor do Spotify Connect para Linux: a música que você escolhe no celular ou no site toca aqui e aparece na ilha, com os botões. Sem conta e sem chave: fala com o Soloist aberto com a API local (-w 127.0.0.1:0) e com o que o Spotifast abre.";
        let playing = self.integrations.spotify.now.as_ref().is_some_and(|now| now.on(Player::Soloist));
        head(ui, k, "Soloist", hint, playing, &mut self.prefs.integrations.soloist);
        if self.prefs.integrations.soloist {
            self.settings_player(ui, k, Player::Soloist);
            self.settings_soloist_setup(ui, k);
        } else if self.integrations.soloist_setup.open {
            self.integrations.soloist_setup.close();
        }
    }

    /// Configurar: Soloist's five steps (`integrations/spotify/setup`), each done, now or to do by what is found, with
    /// the buttons that do what is missing; then the service's Status and Parar/Ligar. The key's field hides what is
    /// pasted and is never kept.
    fn settings_soloist_setup(&mut self, ui: &mut egui::Ui, k: &K) {
        use crate::app::integrations::spotify::setup::{self, Job, KeyFile, Mark, Program, TITLES};
        let mut press = "";
        let open = self.integrations.soloist_setup.open;
        let hint = "Baixa o Soloist do Spotify, guarda a chave e liga com o computador, passo a passo, sem outro programa. O que já está feito fica como está.";
        row(ui, k, "Configurar o Soloist", hint, |ui| {
            if button(ui, k, if open { "Fechar" } else { "Configurar" }).clicked() {
                press = if open { "close" } else { "open" };
            }
        });
        if open {
            self.soloist_setup_watch(ui.ctx());
        }
        let setup = &mut self.integrations.soloist_setup;
        match (open, setup.status.clone()) {
            (false, _) => {}
            (true, None) => note(ui, k, "Conferindo o que já está feito…"),
            (true, Some(status)) => {
                let typed = !setup.key_field.trim().is_empty();
                let marks = setup::marks(&status, typed);
                let hints = setup::hints(&status, typed, &setup.name_field);
                let label = |index: usize| format!("{}{}. {}", if marks[index] == Mark::Done { "✓ " } else { "" }, index + 1, TITLES[index]);
                let hint = |index: usize| {
                    let word = marks[index].words();
                    format!("{}{} · {}", word[..1].to_uppercase(), &word[1..], hints[index])
                };
                row(ui, k, &label(0), &hint(0), |ui| {
                    let wanted = status.foreign.is_none() && !matches!(status.program, Program::Ready(_));
                    let text = match status.program {
                        Program::Expired => "Atualizar",
                        Program::Broken(_) => "Baixar de novo",
                        _ => "Baixar",
                    };
                    if wanted && button(ui, k, text).clicked() {
                        press = "download";
                    }
                });
                row(ui, k, &label(1), &hint(1), |ui| {
                    if button(ui, k, "Abrir o painel").clicked() {
                        press = "dashboard";
                    }
                });
                let key_id = egui::Id::unique("settings_soloist_key");
                row(ui, k, "Chave", "", |ui| {
                    if button(ui, k, "Colar").clicked() {
                        press = "paste";
                    }
                    input(ui, k, key_id, "Chave do Soloist", &mut setup.key_field, "spak_…", 250.0, true);
                });
                row(ui, k, &label(2), &hint(2), |ui| {
                    input(ui, k, egui::Id::unique("settings_soloist_name"), "Nome do dispositivo", &mut setup.name_field, "Soloist", 200.0, false);
                });
                row(ui, k, &label(3), &hint(3), |ui| {
                    let ready = matches!(status.program, Program::Ready(_)) && (typed || status.key != KeyFile::Missing);
                    if ready && (marks[3] != Mark::Done || typed) && button(ui, k, if typed { "Salvar e ligar" } else { "Ligar" }).clicked() {
                        press = "turn_on";
                    }
                });
                row(ui, k, &label(4), &hint(4), |_| ());
                if !status.vpn.is_empty() {
                    note(ui, k, &setup::vpn_warning(&status.vpn));
                }
                if status.unit {
                    let state = format!("{} · {}", status.active, if status.enabled { "abre sozinho ao entrar" } else { "não abre sozinho" });
                    row(ui, k, "Serviço soloist.service", &state, |ui| {
                        if button(ui, k, if status.running() { "Parar" } else { "Ligar" }).clicked() {
                            press = if status.running() { "stop" } else { "start" };
                        }
                        if button(ui, k, "Status").clicked() {
                            press = "details";
                        }
                    });
                }
                if let Some(busy) = setup.busy {
                    note(ui, k, busy);
                } else if let Some(Err(why)) = &setup.said {
                    note(ui, k, &format!("Não deu: {why}"));
                }
                if let Some(details) = &setup.details {
                    note(ui, k, details);
                }
                if press == "paste" {
                    // the system's clipboard comes as a paste into the key's field, which takes the focus
                    ui.memory_mut(|memory| memory.request_focus(key_id));
                    ui.ctx().send_viewport_cmd(egui::ViewportCommand::RequestPaste);
                }
            }
        }
        let setup = &mut self.integrations.soloist_setup;
        let job = match press {
            "open" => {
                setup.open();
                None
            }
            "close" => {
                setup.close();
                None
            }
            "dashboard" => {
                open_page(setup::DASHBOARD);
                None
            }
            "download" => Some(Job::Download),
            "turn_on" => {
                let key = std::mem::take(&mut setup.key_field);
                Some(Job::TurnOn { key: (!key.trim().is_empty()).then(|| setup::Secret::new(&key)), name: setup.name_field.clone() })
            }
            "start" => Some(Job::Start),
            "stop" => Some(Job::Stop),
            "details" => Some(Job::Details),
            _ => None,
        };
        if let Some(job) = job
            && let Err(why) = self.soloist_setup_run(job)
        {
            self.toast(format!("Soloist: {why}"));
        }
    }

    /// One player's line: what it plays and its buttons while it is the one shown, else why not.
    fn settings_player(&mut self, ui: &mut egui::Ui, k: &K, player: Player) {
        let now = self.integrations.spotify.now.clone().filter(|now| now.on(player));
        let mut asked = None;
        row(ui, k, &self.integrations.spotify.report_of(player), "", |ui| {
            let Some(now) = &now else { return };
            for (label, command) in
                [("Próxima", Command::Next), (if now.playing { "Pausar" } else { "Tocar" }, Command::PlayPause), ("Anterior", Command::Previous)]
            {
                if button(ui, k, label).clicked() {
                    asked = Some(command);
                }
            }
        });
        if let Some(command) = asked {
            self.spotify_control(command);
        }
    }

    fn settings_github(&mut self, ui: &mut egui::Ui, k: &K) {
        let hint = "Os seus pull requests com os checks, as revisões que pedem a você, o CI da branch principal dos seus repositórios e a sua atividade.";
        head(ui, k, "GitHub", hint, self.integrations.github.connected(), &mut self.prefs.integrations.github);
        if !self.prefs.integrations.github {
            return;
        }
        let (mut save, mut test, mut remove) = (false, false, false);
        let hint = "Clássico com o escopo repo, ou fine-grained com leitura em Commit statuses e Actions e leitura e escrita em Pull requests (para salvar a descrição e comentar pela ilha).";
        row(ui, k, "Token pessoal", hint, |ui| {
            save = button(ui, k, "Salvar").clicked();
            if button(ui, k, "Criar token…").clicked() {
                open_page(github::TOKENS_PAGE);
            }
            let field = input(
                ui,
                k,
                egui::Id::unique("settings_github_token"),
                "Token pessoal do GitHub",
                &mut self.integrations.token_field,
                "ghp_… ou github_pat_…",
                250.0,
                true,
            );
            save |= field.lost_focus() && ui.input(|input| input.key_pressed(egui::Key::Enter));
        });
        let github = &self.integrations.github;
        if github.has_token() {
            let (status, detail) = match (&github.pulse, &github.error) {
                (_, Some(error)) => ("Não conectado".to_string(), error.clone()),
                (Some(pulse), None) => (format!("Conectado como @{}", pulse.login), pulse.summary()),
                (None, None) => ("Falando com o GitHub…".to_string(), "O token está no cofre do sistema.".to_string()),
            };
            row(ui, k, &status, &detail, |ui| {
                remove = button(ui, k, "Remover").clicked();
                test = button(ui, k, if github.testing { "Testando…" } else { "Testar" }).clicked();
            });
        } else {
            note(ui, k, "Sem token: crie um, cole acima e salve. Ele vai para o cofre do sistema, nunca para um arquivo.");
        }
        if let Some(pulse) = &github.pulse {
            for request in pulse.open() {
                if link(ui, k, (request.ci.label().0, check_color(k, request.ci)), &format!("{}  {}", request.id, request.title), &request.state_words()) {
                    open_page(&request.url);
                }
            }
            for request in &pulse.to_review {
                if link(ui, k, ("•", AMBER), &format!("{}  {}", request.id, request.title), "pede a sua revisão") {
                    open_page(&request.url);
                }
            }
        }
        if let Some(activity) = &github.activity {
            ui.add_space(6.5);
            note(ui, k, &format!("{} contribuições no último ano · {} hoje", activity.total, activity.today));
            ui.add_space(3.25);
            calendar(ui, k, activity);
            // the card's padding under the grid, as a row's (it ended flush with the card's edge)
            ui.add_space(14.0);
        }
        if save {
            let token = std::mem::take(&mut self.integrations.token_field);
            let said = self.github_save_token(&token).unwrap_or_else(|why| why);
            self.toast(said);
        } else if test {
            self.github_test();
        } else if remove {
            let said = self.github_remove_token().unwrap_or_else(|why| format!("O token não foi removido: {why}"));
            self.toast(said);
        }
    }
}
