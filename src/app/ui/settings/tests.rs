use super::*;

/// Ajustes under test: a window, the app and a clock that moves 0.1 s a frame, so the sheet has opened.
struct Window {
    app: DrawingApp,
    ctx: Context,
    time: f64,
    texts: Vec<(String, Rect)>,
    /// The cards painted (`kit::close_group`'s rects in the card's colour).
    cards: Vec<Rect>,
}

impl Window {
    fn new() -> Self {
        let mut app = DrawingApp::default();
        app.settings_open = true;
        let ctx = Context::default();
        // the titles are in the system face's weight 600, a family of the app's own
        crate::app::ui::install_fonts(&ctx, false);
        let mut window = Self { app, ctx, time: 0.0, texts: vec![], cards: vec![] };
        for _ in 0..4 {
            window.frame(vec![]);
        }
        window
    }

    fn at(tab: usize) -> Self {
        let mut window = Self::new();
        window.app.settings_tab = tab;
        window.frame(vec![]);
        window
    }

    fn frame(&mut self, events: Vec<egui::Event>) {
        self.time += 0.1;
        let input = egui::RawInput { time: Some(self.time), events, ..Default::default() };
        let mut output = self.ctx.run_ui(input, |ui| {
            self.app.settings_ui(ui.ctx());
            // as the toolbar does: the hooks' question after Ajustes, over it
            if self.app.claude_hooks_preview.is_some() {
                self.app.claude_hooks_preview_ui(ui.ctx());
            }
            if self.app.wipe_confirm {
                self.app.wipe_confirm_ui(ui.ctx());
            }
        });
        output.textures_delta.clear();
        self.texts = output
            .shapes
            .iter()
            .filter_map(|s| if let egui::Shape::Text(t) = &s.shape { Some((t.galley.text().to_string(), t.visual_bounding_rect())) } else { None })
            .collect();
        let card = K::with_cards(self.app.dark()).card;
        self.cards = output.shapes.iter().filter_map(|s| if let egui::Shape::Rect(r) = &s.shape { (r.fill == card).then_some(r.rect) } else { None }).collect();
    }

    /// The card a text is drawn in.
    fn card_of(&self, name: &str) -> Rect {
        let text = self.at_text(name);
        self.cards
            .iter()
            .copied()
            .filter(|card| card.contains(text.center()))
            .min_by(|a, b| a.area().total_cmp(&b.area()))
            .unwrap_or_else(|| panic!("{name} in no card"))
    }

    fn at_text(&self, name: &str) -> Rect {
        self.texts.iter().find(|(t, _)| t == name).map(|(_, r)| *r).unwrap_or_else(|| panic!("{name}: {:?}", self.texts))
    }

    fn has(&self, name: &str) -> bool {
        self.texts.iter().any(|(t, _)| t == name)
    }

    /// `name` is drawn somewhere under `anchor` (in the groups that follow it on the page).
    fn has_under(&self, anchor: &str, name: &str) -> bool {
        let top = self.at_text(anchor).top();
        self.texts.iter().any(|(t, rect)| t == name && rect.top() > top)
    }

    fn click(&mut self, pos: Pos2) {
        let button = |pressed| egui::Event::PointerButton { pos, button: egui::PointerButton::Primary, pressed, modifiers: Modifiers::NONE };
        self.frame(vec![egui::Event::PointerMoved(pos)]);
        self.frame(vec![button(true)]);
        self.frame(vec![button(false)]);
        self.frame(vec![]);
    }

    fn click_text(&mut self, name: &str) {
        let at = self.at_text(name).center();
        self.click(at);
    }

    /// The wheel over the page until `name` is in the window's lower half.
    fn scroll_to(&mut self, name: &str) {
        let middle = self.ctx.content_rect().center();
        for _ in 0..30 {
            if self.at_text(name).center().y < middle.y + 200.0 {
                return;
            }
            let wheel = egui::Event::MouseWheel {
                unit: egui::MouseWheelUnit::Point,
                delta: Vec2::new(0.0, -120.0),
                phase: egui::TouchPhase::Move,
                source: egui::MouseWheelSource::Unknown,
                modifiers: Modifiers::NONE,
            };
            self.frame(vec![egui::Event::PointerMoved(middle), wheel]);
            self.frame(vec![]);
        }
    }

    /// Where the switch of the row named `name` is: at the column's right, in the middle of the row.
    fn switch_of(&self, name: &str) -> Pos2 {
        let label = self.at_text(name);
        Pos2::new(label.left() + kit::COLUMN - 19.0, label.center().y + 8.0)
    }
}

#[test]
fn every_section_of_the_settings_shows() {
    for tab in (0..SETTINGS_TABS.len()).filter(|t| !HIDDEN_TABS.contains(&SETTINGS_TABS[*t])) {
        let window = Window::at(tab);
        assert!(window.app.settings_open);
        assert!(window.has(SETTINGS_TABS[tab]), "{tab}: {:?}", window.texts);
    }
}

#[test]
fn a_tabs_groups_go_down_not_across() {
    let window = Window::at(0);
    let (aparencia, inicio) = (window.at_text("APARÊNCIA"), window.at_text("INÍCIO"));
    assert!(inicio.top() > aparencia.bottom() && (inicio.left() - aparencia.left()).abs() < 2.0, "{aparencia:?} then {inicio:?}");
}

/// The sheet is centered at one size: the row of tabs stays under the pointer from a long tab to a short one (while they are in view).
#[test]
fn the_tabs_stay_put_from_a_long_tab_to_a_short_one() {
    let sobre = |tab: usize| Window::at(tab).at_text("Geral");
    let (long, short) = (sobre(1), sobre(3));
    assert!((long.top() - short.top()).abs() < 1.0 && (long.left() - short.left()).abs() < 1.0, "{long:?} on Terminal, {short:?} on Chat");
}

/// The window's measures against the design: 880 wide in the middle, the sidebar's names 48 in from its left, the page's
/// name and its groups' titles from 232 (the 200 of the sidebar, the main area's 32), the titles 4 further in.
#[test]
fn the_sheet_measures_what_the_design_does() {
    let window = Window::at(0);
    let left = window.ctx.content_rect().center().x - 440.0;
    let names: Vec<Rect> = window.texts.iter().filter(|(text, _)| text == "Geral").map(|(_, rect)| *rect).collect();
    assert_eq!(names.len(), 2, "the sidebar's and the page's: {:?}", window.texts);
    assert!((names[0].left() - (left + 12.0 + 10.0 + 16.0 + 10.0)).abs() < 2.0, "{:?} from {left}", names[0]);
    assert!((names[1].left() - (left + 232.0)).abs() < 3.0, "{:?} from {left}", names[1]);
    let aparencia = window.at_text("APARÊNCIA");
    assert!((aparencia.left() - (left + 236.0)).abs() < 3.0, "{aparencia:?} from {left}");
    assert!(aparencia.top() > names[1].bottom() + 18.0, "under the page's line");
}

/// What the ☰ menu held lives here now, in Geral with the rest: the canvas's and the editor's.
#[test]
fn ajustes_holds_what_the_menu_had() {
    let window = Window::at(0);
    for name in [
        "Tema",
        "Idioma",
        "Abrir o app em",
        "Fechar sozinho",
        "Seleção por caixa",
        "Dicas nos botões",
        "Dispositivo",
        "Passo do zoom",
        "Ligaduras no código",
        "Fundo",
    ] {
        assert!(window.has(name), "{name} in Geral: {:?}", window.texts);
    }
}

#[test]
fn a_click_opens_a_section_and_flips_a_switch() {
    let mut window = Window::new();
    window.click_text("Agentes");
    assert_eq!(window.app.settings_tab, 2, "the tab opens");
    window.click_text("Geral");
    let island = window.app.prefs.island;
    let switch = window.switch_of("Mostrar a ilha");
    window.click(switch);
    assert_eq!(window.app.prefs.island, !island, "the switch flips it");
}

#[test]
fn a_select_changes_its_setting_and_the_zoom_step_steps() {
    let mut window = Window::new();
    window.click_text("Claro");
    assert_eq!(window.app.prefs.theme, Theme::Light, "the segment picks it");
    // a select's menu
    window.scroll_to("Traçado animado");
    window.click_text("Traçado animado");
    window.click_text("Sem traçado");
    assert_eq!(window.app.prefs.window_outline, WindowOutline::Hidden, "the menu picks it");
    window.scroll_to("Passo do zoom");
    assert_eq!(window.app.prefs.zoom_step, 10);
    // the plus of the stepper, 30 wide at the right end of the row, in the middle of its first 48
    let label = window.at_text("Passo do zoom");
    window.click(Pos2::new(label.left() + kit::COLUMN - 15.0, label.center().y + 7.0));
    assert_eq!(window.app.prefs.zoom_step, 11, "+ adds one");
}

#[test]
fn escape_and_a_click_outside_close_it() {
    let mut window = Window::new();
    window.frame(vec![egui::Event::Key { key: Key::Escape, physical_key: None, pressed: true, repeat: false, modifiers: Modifiers::NONE }]);
    assert!(!window.app.settings_open, "Esc closes");
    let mut window = Window::new();
    window.click(Pos2::new(5.0, 5.0));
    assert!(!window.app.settings_open, "a click on the veil closes");
    let mut window = Window::new();
    let inside = window.at_text("Geral").center() + Vec2::new(0.0, 120.0);
    window.click(inside);
    assert!(window.app.settings_open, "a click on the sheet does not");
}

#[test]
fn agentes_has_no_accounts_and_keeps_the_memory_apart() {
    let window = Window::at(tab_of("Agentes"));
    // asked out: the skills, the memory in use, the hooks (now in Integrações) and the avatar; with them the sub-tabs
    for name in ["Skills", "Memória", "Controle", "Avatar", "Hooks", "CLAUDE CODE FORA DO APP"] {
        assert!(!window.has(name), "{name}: {:?}", window.texts);
    }
    assert!(!window.has("Contas"), "the Contas tab is gone");
    for name in ["LIMITES DO MAESTRO", "Parar de recrutar em"] {
        assert!(window.has(name), "{name} in Uso: {:?}", window.texts);
    }
    // asked out: the plan's line, the ceiling and the recruits' model
    for name in ["CONSUMO DO PLANO", "Agentes ao mesmo tempo", "Modelo dos recrutados"] {
        assert!(!window.has(name), "{name} in Uso: {:?}", window.texts);
    }
    assert!(!window.has("Chaves de API"), "the chats run on the agents' own logins only");
}

#[test]
fn the_usage_pill_opens_agentes_uso() {
    let mut window = Window::at(0);
    window.app.settings_open = false;
    window.app.open_settings_at("Agentes");
    assert!(window.app.settings_open);
    assert_eq!(super::SETTINGS_TABS[window.app.settings_tab], "Agentes");
    window.frame(vec![]);
    assert!(window.has("LIMITES DO MAESTRO") && window.has("Parar de recrutar em"), "{:?}", window.texts);
    for name in ["MCP EM OUTROS AGENTES", "Registrar no Claude Code", "Registrar no Codex"] {
        assert!(window.has(name), "{name}: {:?}", window.texts);
    }
}

/// The hooks of the sessions outside the app live in Integrações, first on the page (asked: easier to find), with the
/// services after them.
#[test]
fn the_outside_hooks_are_in_integracoes() {
    let window = Window::at(tab_of("Integrações"));
    for name in ["CLAUDE CODE FORA DO APP", "CODEX FORA DO APP", "SERVIÇOS", "GitHub"] {
        assert!(window.has(name), "{name}: {:?}", window.texts);
    }
    let (claude, codex, services) = (window.at_text("CLAUDE CODE FORA DO APP"), window.at_text("CODEX FORA DO APP"), window.at_text("SERVIÇOS"));
    assert!(claude.top() < codex.top() && codex.top() < services.top() && services.top() < window.at_text("GitHub").top());
    assert!(window.texts.iter().any(|(text, _)| text.contains("Os hooks das sessões de fora")), "the page's line says so");
}

/// Asked: "oculta no menu de configurações a opção Ok Mago" and "oculta também o modo voz". Off the menu, their code kept
/// and the others in their places.
#[test]
fn voz_and_ok_mago_are_off_the_menu_and_the_others_keep_their_places() {
    let window = Window::at(0);
    assert!(!window.has("Voz") && !window.has("Ok Mago"), "{:?}", window.texts);
    assert_eq!(SETTINGS_TABS[4], "Voz");
    let (atalhos, sobre) = (Window::at(tab_of("Atalhos")), Window::at(tab_of("Sobre")));
    assert!(atalhos.has("Atalhos") && sobre.has("Sobre"));
}

#[test]
fn typing_in_the_shortcut_search_narrows_the_list() {
    let mut window = Window::at(tab_of("Atalhos"));
    assert!(window.has("Desfazer") && window.has("Salvar"));
    window.click_text("Buscar pelo nome ou pela tecla");
    window.frame(vec![egui::Event::Text("salv".into())]);
    window.frame(vec![]);
    assert!(window.has("Salvar") && !window.has("Desfazer"), "{:?}", window.texts);
}

#[test]
fn sobre_offers_the_check_and_then_the_update_with_its_notes() {
    use crate::app::system::updates::{Release, UpdateState};
    let mut window = Window::at(tab_of("Sobre"));
    assert!(window.has("Verificar atualizações"), "{:?}", window.texts);
    // when the app looks by itself: Sempre (on opening and every hour) by default
    assert!(window.has("Procurar atualizações") && window.has("Sempre"), "{:?}", window.texts);
    assert_eq!(window.app.prefs.update_check, 0);
    let release =
        Release { version: "99.0.0".into(), notes: "## Novo\n- mais legível".into(), page: String::new(), installer: None, digest: None, size: None };
    window.app.updates.state = UpdateState::Available(release);
    window.frame(vec![]);
    assert!(window.has("Atualizar") && window.has("NOVIDADES DA 99.0.0"), "{:?}", window.texts);
    assert!(!window.has("Verificar atualizações"));
}

/// On Windows "Atualizar" closes and reopens the app: the first click only warns, Cancelar takes it back.
#[test]
fn atualizar_warns_that_the_app_closes_and_reopens_before_doing_anything() {
    use crate::app::system::updates::{Release, UpdateState};
    let mut window = Window::at(super::SETTINGS_TABS.iter().position(|tab| *tab == "Sobre").unwrap());
    let release = Release {
        version: "99.0.0".into(),
        notes: String::new(),
        page: String::new(),
        installer: Some("https://github.com/x.msi".into()),
        digest: None,
        size: None,
    };
    window.app.updates.state = UpdateState::Available(release);
    window.frame(vec![]);
    window.click_text("Atualizar");
    if cfg!(windows) {
        assert!(window.app.updates.confirming && matches!(window.app.updates.state, UpdateState::Available(_)), "nothing started yet");
        assert!(window.has("Atualizar e reiniciar") && window.has("Cancelar"), "{:?}", window.texts);
        window.click_text("Cancelar");
        assert!(!window.app.updates.confirming);
    }
}

/// Integrações: off, GitHub is only its head; on, the token's row; with an answer, who and the pull requests.
#[test]
fn integracoes_shows_github_with_its_token_row_and_its_pull_requests() {
    use crate::app::integrations::github::{GitHub, Pulse};
    assert_eq!(SETTINGS_TABS[6], "Integrações");
    let mut window = Window::at(6);
    assert!(window.has("GitHub") && !window.has("Token pessoal"), "{:?}", window.texts);
    // its switch: 19 from the right of the column, level with its name (16 after the dot)
    let name = window.at_text("GitHub");
    window.click(Pos2::new(name.left() - 16.0 + kit::COLUMN - 19.0, name.center().y));
    assert!(window.app.prefs.integrations.github, "the switch turns it on");
    assert!(window.has("Token pessoal") && window.has("Salvar") && window.has("Criar token…"), "{:?}", window.texts);
    assert!(!window.has("Testar"), "no token yet: nothing to test");
    // something that is not a token: refused, and the field is emptied
    window.app.integrations.token_field = "isso não é um token".into();
    window.click_text("Salvar");
    assert!(window.app.status.contains("não parece um token") && window.app.integrations.token_field.is_empty(), "{}", window.app.status);
    window.app.integrations.github = GitHub::with_token("ghp_0123456789abcdefghijABCDEFGHIJ012345");
    window.app.integrations.github.pulse = Pulse::parse(&serde_json::from_str(include_str!("../../integrations/fixtures/github_pulse.json")).unwrap());
    window.frame(vec![]);
    for text in [
        "Conectado como @ana-dev",
        "3 PRs abertos · 1 com checks falhando · 1 para revisar",
        "Testar",
        "Remover",
        "checks passaram · aprovado",
        "pede a sua revisão",
    ] {
        assert!(window.has(text), "{text}: {:?}", window.texts);
    }
    assert!(window.texts.iter().any(|(text, _)| text.starts_with("ana-dev/site#7")), "{:?}", window.texts);
    assert!(!window.texts.iter().any(|(text, _)| text.contains("ghp_0123")), "the token is never drawn");
}

#[test]
fn integracoes_shows_what_spotify_plays_with_its_buttons() {
    use crate::app::integrations::spotify::{Command, Now, Track};
    let mut window = Window::at(6);
    assert!(window.has("Spotify") && !window.has("Próxima"), "{:?}", window.texts);
    window.app.prefs.integrations.spotify = true;
    window.frame(vec![]);
    assert!(window.texts.iter().any(|(text, _)| text.contains("não está aberto")) && !window.has("Próxima"), "closed: no buttons: {:?}", window.texts);
    window.app.integrations.spotify.now = Some(Now { track: Track::new("Canção", "Banda", "", "").unwrap(), playing: true, ..Default::default() });
    window.frame(vec![]);
    assert!(window.has("Tocando: Canção — Banda") && window.has("Pausar") && window.has("Anterior"), "{:?}", window.texts);
    window.click_text("Próxima");
    window.click_text("Pausar");
    assert_eq!(window.app.integrations.spotify.asked, [Command::Next, Command::PlayPause]);
    window.app.integrations.spotify.now.as_mut().unwrap().playing = false;
    window.frame(vec![]);
    assert!(window.has("Pausado: Canção — Banda") && window.has("Tocar"), "{:?}", window.texts);
}

/// Conta do Spotify: the Client ID, the exact Redirect URI with its copy button, and Conectar / Cancelar / Desconectar.
#[test]
fn integracoes_has_the_spotify_account_with_its_redirect_uri() {
    use crate::app::integrations::spotify::account::{Account, Link, REDIRECT_URI};
    let mut window = Window::at(6);
    assert!(!window.has("Client ID"), "off: no account to show");
    window.app.prefs.integrations.spotify = true;
    window.frame(vec![]);
    window.scroll_to(REDIRECT_URI);
    assert!(window.has("Conta do Spotify") && window.has("Client ID") && window.has(REDIRECT_URI) && window.has("Copiar"), "{:?}", window.texts);
    assert!(window.texts.iter().any(|(text, _)| text.contains("8989/login")), "the Spotifast Client ID serves too: {:?}", window.texts);
    // no Client ID: it says what to paste
    window.click_text("Conectar");
    assert_eq!(window.app.integrations.spotify.account.link, Link::Off);
    window.app.prefs.integrations.spotify_client_id = "0123456789abcdef0123456789abcdef".into();
    window.frame(vec![]);
    window.click_text("Conectar");
    assert_eq!(window.app.integrations.spotify.account.link, Link::Waiting, "no browser in a test");
    window.frame(vec![]);
    window.click_text("Cancelar");
    assert_eq!(window.app.integrations.spotify.account.link, Link::Off);
    window.app.integrations.spotify.account = Account::with_session("ana");
    window.frame(vec![]);
    assert!(window.has("Conectada como Ana") && window.has("Desconectar"), "{:?}", window.texts);
    window.click_text("Desconectar");
    assert!(!window.app.integrations.spotify.account.linked());
}

/// Soloist has its own switch beside Spotify's (asked: "deixa as duas opções"); what it plays shows on its card only.
#[test]
fn integracoes_has_a_switch_of_its_own_for_soloist() {
    use crate::app::integrations::spotify::{Now, Player, Track};
    let mut window = Window::at(6);
    // Spotify's Connect receiver is only for Linux: elsewhere there is no card for it
    if cfg!(any(windows, target_os = "macos")) {
        assert!(window.has("Spotify") && !window.has("Soloist"), "{:?}", window.texts);
        return;
    }
    assert!(window.has("Spotify") && window.has("Soloist"), "{:?}", window.texts);
    window.app.prefs.integrations.soloist = true;
    window.frame(vec![]);
    assert!(window.texts.iter().any(|(text, _)| text.contains("-w 127.0.0.1:0")), "it says how to open it: {:?}", window.texts);
    assert!(!window.app.prefs.integrations.spotify, "the app's switch stays as it was");
    let now = Now { track: Track::new("Canção", "Banda", "", "").unwrap(), playing: true, player: Player::Soloist, ..Default::default() };
    window.app.integrations.spotify.now = Some(now);
    window.frame(vec![]);
    window.scroll_to("Tocando: Canção — Banda (Soloist)");
    assert!(window.has("Tocando: Canção — Banda (Soloist)") && window.has_under("Soloist", "Pausar"), "{:?}", window.texts);
    window.click_text("Pausar");
    assert_eq!(window.app.integrations.spotify.asked, [crate::app::integrations::spotify::Command::PlayPause]);
    // with the app's switch on too, its card says it is not the one playing
    window.app.prefs.integrations.spotify = true;
    window.frame(vec![]);
    assert!(window.texts.iter().any(|(text, _)| text.contains("o Soloist está")), "{:?}", window.texts);
}

/// Soloist's Configurar (Linux): the five steps by what was found, a button only for what is missing (so a setup that
/// is all there changes nothing), the key hidden and never kept, and the service's own buttons.
#[test]
fn integracoes_sets_soloist_up_step_by_step() {
    use crate::app::integrations::spotify::setup::{KeyFile, Login, Program, Status};
    const KEY: &str = "spak_SEGREDO0123456789";
    if cfg!(any(windows, target_os = "macos")) {
        return;
    }
    let mut window = Window::at(6);
    window.app.prefs.integrations.soloist = true;
    window.frame(vec![]);
    window.scroll_to("Configurar");
    window.click_text("Configurar");
    fn setup(window: &mut Window) -> &mut crate::app::integrations::spotify::setup::Setup {
        &mut window.app.integrations.soloist_setup
    }
    assert!(setup(&mut window).open && setup(&mut window).name_field.starts_with("Soloist"), "the name starts as the default");
    assert!(window.has("Conferindo o que já está feito…"), "{:?}", window.texts);
    // the button under the steps' head, not one of the page above
    let below = |window: &Window, name: &str| {
        let top = window.at_text("Configurar o Soloist").top();
        window.texts.iter().filter(|(text, rect)| text == name && rect.top() > top).map(|(_, rect)| rect.center()).next()
    };
    // nothing there: step 1 is now, with its button
    setup(&mut window).status = Some(Status::default());
    window.frame(vec![]);
    window.scroll_to("5. Escolher o Soloist no celular");
    assert!(window.has("1. Baixar o Soloist") && window.has("Chave"), "{:?}", window.texts);
    assert!(window.texts.iter().any(|(text, _)| text.starts_with("Agora · ")), "{:?}", window.texts);
    let download = below(&window, "Baixar").expect("Baixar");
    window.click(download);
    assert_eq!(setup(&mut window).asked, ["download"]);
    assert!(window.has("Baixando…"), "it says what runs");
    setup(&mut window).busy = None;
    // an expired version: Atualizar downloads again
    setup(&mut window).status = Some(Status { program: Program::Expired, ..Default::default() });
    window.frame(vec![]);
    assert!(window.texts.iter().any(|(text, _)| text.contains("Esta versão expirou")) && below(&window, "Atualizar").is_some(), "{:?}", window.texts);
    // all there and running, as on a computer already set up: every step done, nothing offered that writes
    let done = Status {
        program: Program::Ready("soloist 1.3.10".into()),
        key: KeyFile::Private,
        unit: true,
        enabled: true,
        active: "active".into(),
        login: Login::In,
        ..Default::default()
    };
    setup(&mut window).status = Some(done.clone());
    window.frame(vec![]);
    window.scroll_to("Serviço soloist.service");
    for step in ["✓ 1. Baixar o Soloist", "✓ 2. Gerar a chave de desenvolvedor", "✓ 4. Salvar e ligar com o computador", "✓ 5. Escolher o Soloist no celular"]
    {
        assert!(window.has(step), "{step}: {:?}", window.texts);
    }
    for writes in ["Baixar", "Atualizar", "Ligar", "Salvar e ligar"] {
        assert!(below(&window, writes).is_none(), "{writes}: {:?}", window.texts);
    }
    // a new key pasted: it is never drawn, and Salvar e ligar takes it out of the field into the job
    setup(&mut window).key_field = KEY.into();
    window.frame(vec![]);
    assert!(window.texts.iter().all(|(text, _)| !text.contains("SEGREDO")), "the key shows: {:?}", window.texts);
    let save = below(&window, "Salvar e ligar").expect("Salvar e ligar");
    window.click(save);
    assert_eq!(setup(&mut window).asked.last(), Some(&"turn_on_with_key"));
    assert!(setup(&mut window).key_field.is_empty() && !window.app.status.contains("SEGREDO"));
    setup(&mut window).busy = None;
    // the service: Parar while it runs, and its Status
    window.frame(vec![]);
    let stop = below(&window, "Parar").expect("Parar");
    window.click(stop);
    setup(&mut window).busy = None;
    let details = below(&window, "Status").expect("Status");
    window.click(details);
    assert_eq!(setup(&mut window).asked[2..], ["stop", "details"]);
    setup(&mut window).busy = None;
    // Fechar: a key not saved goes with the steps
    setup(&mut window).key_field = "spak_meio".into();
    window.frame(vec![]);
    window.scroll_to("Fechar");
    window.click_text("Fechar");
    assert!(!setup(&mut window).open && setup(&mut window).key_field.is_empty());
    assert!(!window.has("1. Baixar o Soloist"));
}

/// Hermes Agent: on, the plugin's state and its buttons; "Instalar…" says what it will write before writing.
#[test]
fn integracoes_asks_before_installing_the_hermes_plugin() {
    use crate::app::integrations::hermes;
    let home = hermes::home().unwrap();
    assert!(home.starts_with(std::env::temp_dir()), "never the person's own Hermes");
    let mut window = Window::at(6);
    assert!(window.has("Hermes Agent") && !window.has_under("Hermes Agent", "Instalar…"), "{:?}", window.texts);
    window.app.prefs.integrations.hermes = true;
    window.frame(vec![]);
    // no Hermes on this (the tests') machine: it says so and offers nothing to press (the hooks' buttons are above it)
    assert!(window.has("Plugin do MagoAgent no Hermes") && window.has(hermes::Status::NoHermes.words()), "{:?}", window.texts);
    assert!(!window.has_under("Hermes Agent", "Instalar…") && !window.has_under("Hermes Agent", "Remover"), "{:?}", window.texts);
    // "Instalar…" was pressed: what it will write, and the two answers
    window.app.integrations.hermes_confirm = true;
    window.frame(vec![]);
    assert!(window.has("Instalar o plugin no Hermes?") && window.has("Instalar") && window.has("Cancelar"), "{:?}", window.texts);
    assert!(window.texts.iter().any(|(text, _)| text.contains("plugins.enabled") && text.contains("backup")), "{:?}", window.texts);
    // the question can be below the page's fold (fonts differ by system): brought up before the click
    window.scroll_to("Instalar o plugin no Hermes?");
    window.click_text("Cancelar");
    assert!(!window.app.integrations.hermes_confirm && !window.has("Instalar o plugin no Hermes?"));
    window.app.integrations.hermes_confirm = true;
    window.frame(vec![]);
    window.scroll_to("Instalar o plugin no Hermes?");
    // the question's Instalar, the last one drawn (the Codex hooks' own is above, on the page)
    let install = window.texts.iter().filter(|(text, _)| text == "Instalar").map(|(_, rect)| rect.center()).next_back().unwrap();
    window.click(install);
    assert!(!window.app.integrations.hermes_confirm);
    assert_eq!(window.app.status, hermes::Status::NoHermes.words(), "what came of it is said");
    assert!(!home.exists(), "nothing was written");
}

/// A key pressed and let go the frame after: a key still down would come again as a repetition.
fn press(window: &mut Window, key: Key, modifiers: Modifiers) {
    let event = |pressed| egui::Event::Key { key, physical_key: None, pressed, repeat: false, modifiers };
    window.frame(vec![event(true)]);
    window.frame(vec![event(false)]);
}

fn says(window: &Window, words: &str) -> bool {
    window.texts.iter().any(|(text, _)| text.contains(words))
}

#[test]
fn a_shortcut_pressed_waits_for_salvar_and_keys_another_has_are_refused() {
    let mut window = Window::at(tab_of("Atalhos"));
    assert!(window.has("ATALHOS GLOBAIS DA ILHA") && window.has("Abrir o chat da ilha"), "{:?}", window.texts);
    assert!(window.has("Ctrl+Alt+Espaço") && window.has("Ctrl+Alt+Direita"), "the default keys: {:?}", window.texts);
    assert!(
        window.has("ARQUIVO") && window.has("Salvar") && window.has(&keys_label("Cmd+Shift+Z ou Cmd+Y")) && window.has("TECLAS FIXAS"),
        "{:?}",
        window.texts
    );
    window.click_text("Ctrl+Alt+Espaço");
    assert_eq!(window.app.hotkeys.recording.as_ref().map(|editing| editing.id), Some("openChat"));
    assert!(window.has("Pressione as teclas…") && window.has("Cancelar"), "{:?}", window.texts);
    assert!(says(&window, "ABNT2") && says(&window, "AltGr"), "the recorder warns of AltGr: {:?}", window.texts);
    // a key alone is no global shortcut: it waits, refused, and Salvar saves nothing
    press(&mut window, Key::J, Modifiers::NONE);
    assert!(says(&window, "Junte Ctrl ou Alt"), "{:?}", window.texts);
    window.click_text("Salvar");
    assert!(window.app.hotkeys.recording.is_some() && window.app.prefs.shortcuts.is_empty());
    // the keys of an app's shortcut: whose they are
    press(&mut window, Key::S, Modifiers::COMMAND);
    assert!(says(&window, "já é o atalho «Salvar»"), "{:?}", window.texts);
    // free keys wait, shown, until Salvar
    let ctrl_shift = Modifiers { command: true, shift: true, ..Modifiers::NONE };
    press(&mut window, Key::K, ctrl_shift);
    assert!(window.has("Ctrl+Shift+K") && says(&window, "Salvar (ou Enter) aplica"), "{:?}", window.texts);
    assert!(window.app.prefs.shortcuts.is_empty(), "not saved yet");
    window.click_text("Salvar");
    assert_eq!(window.app.hotkeys.recording, None);
    assert_eq!(window.app.prefs.shortcuts["openChat"].keys, "Ctrl+Shift+K");
    assert!(window.has("Ctrl+Shift+K"), "{:?}", window.texts);
    // Esc drops what was pressed, not Ajustes; so does Cancelar
    let ctrl_alt = Modifiers { command: true, alt: true, ..Modifiers::NONE };
    window.click_text("Ctrl+Shift+K");
    press(&mut window, Key::E, ctrl_alt);
    press(&mut window, Key::Escape, Modifiers::NONE);
    assert!(window.app.settings_open && window.app.hotkeys.recording.is_none());
    assert_eq!(window.app.prefs.shortcuts["openChat"].keys, "Ctrl+Shift+K");
    window.click_text("Ctrl+Shift+K");
    press(&mut window, Key::E, ctrl_alt);
    window.click_text("Cancelar");
    assert!(window.app.hotkeys.recording.is_none());
    assert_eq!(window.app.prefs.shortcuts["openChat"].keys, "Ctrl+Shift+K");
    // Enter saves; AltGr+E types ° on ABNT2: the warning stays under the shortcut
    window.click_text("Ctrl+Shift+K");
    press(&mut window, Key::E, ctrl_alt);
    press(&mut window, Key::Enter, Modifiers::NONE);
    assert_eq!(window.app.prefs.shortcuts["openChat"].keys, "Ctrl+Alt+E");
    assert!(says(&window, "«°»"), "{:?}", window.texts);
}

/// The middle of the ghost button `steps` places right of the keys button whose text is `keys` (rows are laid out
/// from the right edge inwards, 6 apart).
fn ghost_right_of(window: &Window, keys: &str, steps: f32) -> Pos2 {
    let text = window.at_text(keys);
    let right = text.center().x + (text.width() + 24.0).max(96.0) / 2.0;
    Pos2::new(right + 6.0 + 12.0 + (steps - 1.0) * 30.0, text.center().y)
}

#[test]
fn limpar_leaves_a_shortcut_without_keys_and_padrao_brings_them_back() {
    let mut window = Window::at(tab_of("Atalhos"));
    let (undo_keys, save_keys) = (keys_label("Cmd+Z"), keys_label("Cmd+S"));
    window.scroll_to(&undo_keys);
    let clear = ghost_right_of(&window, &save_keys, 1.0);
    window.click(clear);
    assert_eq!(window.app.prefs.shortcuts["save"].keys, "", "Salvar without a key");
    // Desfazer takes Ctrl+S, now free
    window.click_text(&undo_keys);
    press(&mut window, Key::S, Modifiers::COMMAND);
    press(&mut window, Key::Enter, Modifiers::NONE);
    assert_eq!(window.app.prefs.shortcuts["undo"].keys, "Ctrl+S");
    // Salvar's default can't come back over it: it waits with whose it is
    let rows: Vec<Rect> = window.texts.iter().filter(|(text, _)| text == "Sem tecla").map(|(_, rect)| *rect).collect();
    let salvar = window.at_text("Salvar");
    let row = rows.iter().find(|rect| (rect.center().y - salvar.center().y).abs() < 20.0).expect("Salvar's keys");
    let back = Pos2::new(row.center().x + (row.width() + 24.0).max(96.0) / 2.0 + 6.0 + 12.0 + 30.0, row.center().y);
    window.click(back);
    assert!(says(&window, "já é o atalho «Desfazer»"), "{:?}", window.texts);
    assert_eq!(window.app.prefs.shortcuts["save"].keys, "", "unchanged");
    window.click_text("Cancelar");
    // Desfazer back to its own, then Salvar
    let undo = ghost_right_of(&window, &save_keys, 2.0);
    window.click(undo);
    window.click(back);
    assert!(window.app.prefs.shortcuts.is_empty(), "every default: {:?}", window.app.prefs.shortcuts);
    assert!(window.has(&save_keys) && window.has(&undo_keys));
}

fn tab_of(name: &str) -> usize {
    SETTINGS_TABS.iter().position(|tab| *tab == name).unwrap()
}

#[test]
fn chat_has_its_own_tab_with_the_new_chat_defaults() {
    let mut window = Window::at(tab_of("Chat"));
    for name in [
        "CHAT",
        "Agente",
        "Pasta de trabalho",
        "O agente pode",
        "Skill de estilo",
        "Compactar automaticamente em",
        "Painel Contexto aberto",
        "Hooks nos agentes do app",
    ] {
        assert!(window.has(name), "{name}: {:?}", window.texts);
    }
    window.scroll_to("Hooks nos agentes do app");
    let switch = window.switch_of("Hooks nos agentes do app");
    window.click(switch);
    assert!(window.app.prefs.agent_hooks, "its switch is the real preference");
    // Agentes keeps the use and the Maestro's limits only
    let agents = Window::at(tab_of("Agentes"));
    assert!(agents.has("LIMITES DO MAESTRO") && !agents.has("Pasta de trabalho"), "{:?}", agents.texts);
}

#[test]
fn every_page_is_in_the_sidebar_at_once() {
    let window = Window::at(SETTINGS_TABS.len() - 1);
    for name in SETTINGS_TABS.into_iter().filter(|name| !super::HIDDEN_TABS.contains(name)) {
        assert!(window.has(name), "{name}: {:?}", window.texts);
    }
}

#[test]
fn ok_mago_switches_are_the_real_preferences() {
    let mut window = Window::at(tab_of("Ok Mago"));
    for name in ["Responder a “Ok Mago”", "Responder em voz alta", "Iniciar a chamada"] {
        assert!(window.has(name), "{name}: {:?}", window.texts);
    }
    assert!(!window.app.prefs.ok_mago && !window.app.prefs.speak_answers, "both come off");
    let switch = window.switch_of("Responder em voz alta");
    window.click(switch);
    assert!(window.app.prefs.speak_answers);
    let switch = window.switch_of("Responder a “Ok Mago”");
    window.click(switch);
    assert!(window.app.prefs.ok_mago);
}

/// Ok Mago comes off: a new install and an old prefs.json that never had the two switches.
#[test]
fn ok_mago_comes_off_by_default_even_in_old_prefs() {
    let fresh = Prefs::default();
    assert!(!fresh.ok_mago && !fresh.speak_answers);
    let old: Prefs = serde_json::from_str(r#"{"theme":"dark","island":true}"#).unwrap();
    assert!(!old.ok_mago && !old.speak_answers);
    let chosen: Prefs = serde_json::from_str(r#"{"ok_mago":true,"speak_answers":true}"#).unwrap();
    assert!(chosen.ok_mago && chosen.speak_answers, "a choice saved stays");
}

/// Email and Dicionário left Ajustes (asked): no page in the sidebar. What they set stays: the mail bridge through the
/// MCP's `email`, the dictionary through its `dictionary`, and Ditar still writes with it.
#[test]
fn email_and_dicionario_are_no_pages_of_ajustes() {
    assert!(!SETTINGS_TABS.contains(&"Email") && !SETTINGS_TABS.contains(&"Dicionário"));
    let window = Window::at(0);
    assert!(!window.has("Email") && !window.has("Dicionário") && window.has("Integrações"), "{:?}", window.texts);
    let mut dictionary = Prefs::default().dictionary;
    assert!(dictionary.add_replacement("cloud code", "Claude Code"));
    assert_eq!(dictionary.apply("abre o cloud code"), "abre o Claude Code");
}

/// The hooks say whether they are installed (asked: "Instalar hooks" stayed there after installing): not installed,
/// Instalar…; installed, "Instalados" with Reinstalar… and Remover. The question says so too, over Ajustes and a veil.
#[test]
fn the_hooks_say_they_are_installed_and_offer_to_reinstall_or_remove() {
    use crate::app::maestro::claude_hooks::{merged, relay_path, settings_path};
    let path = settings_path().unwrap();
    assert!(path.starts_with(std::env::temp_dir()), "never the person's own settings.json");
    let _ = fs::remove_file(&path);
    let mut window = Window::at(tab_of("Integrações"));
    assert!(window.has("Não instalados") && window.has("Instalar…"), "{:?}", window.texts);
    assert!(!window.has("Instalados") && !window.has("Reinstalar…") && !window.has("Remover"), "{:?}", window.texts);
    // installed as this version would, beside what the person had
    let relay = relay_path().unwrap().to_string_lossy().into_owned();
    let installed = serde_json::to_string_pretty(&merged(&json!({ "model": "opus" }), &relay).unwrap()).unwrap();
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(&path, &installed).unwrap();
    window.frame(vec![]);
    assert!(window.has("Instalados") && window.has("Reinstalar…") && window.has("Remover"), "{:?}", window.texts);
    assert!(!window.has("Instalar…"), "Instalar is no longer offered: {:?}", window.texts);
    // Reinstalar… asks first, and the question says they are already there
    window.click_text("Reinstalar…");
    let title = "Reinstalar hooks do Claude Code?";
    assert!(window.app.claude_hooks_preview.is_some() && window.has(title) && window.has("Reinstalar"), "{:?}", window.texts);
    assert!(window.texts.iter().any(|(text, _)| text.contains("já estão instalados")), "{:?}", window.texts);
    // over everything: its card under its title, its veil over the sheet of Ajustes (the sidebar's Geral); and again
    // when it comes back, its layers already somewhere in egui's order (the sheet used to stay over it then)
    let question = egui::Id::unique(("dialog", title));
    let escape = |pressed| egui::Event::Key { key: Key::Escape, physical_key: None, pressed, repeat: false, modifiers: Modifiers::NONE };
    for time in 0..2 {
        if time > 0 {
            window.click_text("Reinstalar…");
        }
        assert_eq!(window.ctx.layer_id_at(window.at_text(title).center()).map(|layer| layer.id), Some(question), "{time}");
        assert_eq!(window.ctx.layer_id_at(window.at_text("Geral").center()).map(|layer| layer.id), Some(question.with("veil")), "{time}");
        // Esc answers the question only: Ajustes stays, and nothing was written
        window.frame(vec![escape(true)]);
        window.frame(vec![escape(false)]);
        assert!(window.app.claude_hooks_preview.is_none() && window.app.settings_open);
        assert_eq!(fs::read_to_string(&path).unwrap(), installed);
        for _ in 0..3 {
            window.frame(vec![]);
        }
    }
    let _ = fs::remove_file(&path);
}

/// What the question says, by what is in the file.
#[test]
fn the_hooks_question_names_what_it_will_do() {
    use crate::app::maestro::claude_hooks::HooksState;
    assert_eq!(hooks_preview::preview_words(HooksState::Missing, false).1, "Instalar");
    assert_eq!(hooks_preview::preview_words(HooksState::Outdated, false).1, "Atualizar");
    assert!(hooks_preview::preview_words(HooksState::Installed, true).2.contains("nada muda"));
    assert_eq!(hooks::hooks_row(HooksState::Installed, false), ("Instalados", &["Reinstalar", "Remover"][..]));
}

/// Dados in three groups, in this order (asked): where the canvas is saved, the app's data, where the chats are.
#[test]
fn dados_says_where_the_canvas_the_data_and_the_chats_are() {
    let mut window = Window::at(tab_of("Dados"));
    window.app.autosave = Some(std::env::temp_dir().join("mago-dados").join("autosave.mago"));
    window.frame(vec![]);
    let titles = ["DESENHOS DO CANVAS", "DADOS DO APP", "CHATS"].map(|name| window.at_text(name).top());
    assert!(titles[0] < titles[1] && titles[1] < titles[2], "{titles:?}");
    for name in ["Desenho em uso", "Abrir pasta", "Pasta de dados", "Memória da Luna", "Chats do app", "Conversas do Claude Code"] {
        assert!(window.has(name), "{name}: {:?}", window.texts);
    }
    assert!(window.has_under("DESENHOS DO CANVAS", "Desenho em uso") && window.at_text("Desenho em uso").top() < titles[1]);
    assert!(window.at_text("Memória da Luna").top() < titles[2] && window.at_text("Chats do app").top() > titles[2]);
    let file = std::env::temp_dir().join("mago-dados").join("autosave.mago").display().to_string();
    assert!(window.texts.iter().any(|(text, _)| text.contains(&file)), "the drawing's own path: {:?}", window.texts);
}

/// Privacy lives in Sobre only (asked): Dados has none, Sobre lists what talks to whom.
#[test]
fn privacy_is_in_sobre_only() {
    let dados = Window::at(tab_of("Dados"));
    assert!(!dados.has("PRIVACIDADE") && !dados.has("Navegadores"), "{:?}", dados.texts);
    let sobre = Window::at(tab_of("Sobre"));
    for name in ["PRIVACIDADE", "O app", "Navegadores", "Microfone"] {
        assert!(sobre.has_under("VERSÃO", name), "{name}: {:?}", sobre.texts);
    }
    assert!(sobre.texts.iter().any(|(text, _)| text.starts_with("O Claude Code e o Codex instalados")), "{:?}", sobre.texts);
    for tab in SETTINGS_TABS.iter().filter(|tab| **tab != "Sobre") {
        assert!(!Window::at(tab_of(tab)).has("PRIVACIDADE"), "{tab}");
    }
}

/// No credits row in Sobre (asked).
#[test]
fn sobre_has_no_credits() {
    let sobre = Window::at(tab_of("Sobre"));
    assert!(!sobre.has("CRÉDITOS") && !sobre.has("Ver os avisos"), "{:?}", sobre.texts);
}

/// Sobre's buttons sit inside their card's padding (asked: "Ver as releases no GitHub" was stuck to its edge), whatever
/// the update's state: no release read yet, a release with its notes, the warning before Atualizar on Windows.
#[test]
fn sobre_buttons_keep_the_cards_padding() {
    use crate::app::system::updates::{Release, UpdateState};
    let mut window = Window::at(tab_of("Sobre"));
    let release = || Release { version: "99.0.0".into(), notes: "- mais legível".into(), page: String::new(), installer: None, digest: None, size: None };
    let states = [(UpdateState::Idle, false), (UpdateState::UpToDate(release()), false), (UpdateState::Available(release()), true)];
    for (state, confirming) in states {
        (window.app.updates.state, window.app.updates.confirming) = (state, confirming);
        window.frame(vec![]);
        window.frame(vec![]);
        let buttons = ["Verificar atualizações", "Ver as releases no GitHub", "Abrir a página da release", "Atualizar e reiniciar", "Cancelar"];
        let shown: Vec<&str> = buttons.into_iter().filter(|name| window.has(name)).collect();
        assert!(shown.len() > 1, "{:?}", window.texts);
        for name in shown {
            let (text, card) = (window.at_text(name), window.card_of(name));
            assert!(card.bottom() - text.bottom() >= 14.0, "{name}: {text:?} in {card:?}");
            assert!(text.top() - card.top() >= 14.0 && text.left() - card.left() >= 14.0, "{name}: {text:?} in {card:?}");
        }
    }
}

/// Ajustes › Dados › Limpar todos os dados asks first: Cancelar leaves everything, Apagar tudo closes the app (the copy
/// outside cleans; none starts in a test).
#[test]
fn limpar_todos_os_dados_asks_before_it_closes_the_app() {
    let mut window = Window::at(SETTINGS_TABS.iter().position(|tab| *tab == "Dados").unwrap());
    window.scroll_to("Limpar todos os dados");
    window.click_text("Limpar…");
    for _ in 0..6 {
        window.frame(vec![]);
    }
    assert!(window.app.wipe_confirm && window.has("Apagar todos os dados do MagoAgent?"));
    window.click_text("Cancelar");
    assert!(!window.app.wipe_confirm && !window.app.quitting, "nothing goes");
    window.click_text("Limpar…");
    for _ in 0..6 {
        window.frame(vec![]);
    }
    window.click_text("Apagar tudo");
    assert!(!window.app.wipe_confirm, "answered");
    assert!(window.app.quitting, "it closes");
}
