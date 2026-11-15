use super::*;

/// Ajustes under test: a window, the app and a clock that moves 0.1 s a frame, so the sheet has opened.
struct Window {
    app: DrawingApp,
    ctx: Context,
    time: f64,
    texts: Vec<(String, Rect)>,
}

impl Window {
    fn new() -> Self {
        let mut app = DrawingApp::default();
        app.settings_open = true;
        let mut window = Self { app, ctx: Context::default(), time: 0.0, texts: vec![] };
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
        let mut output = self.ctx.run_ui(input, |ui| self.app.settings_ui(ui.ctx()));
        output.textures_delta.clear();
        self.texts = output
            .shapes
            .iter()
            .filter_map(|s| if let egui::Shape::Text(t) = &s.shape { Some((t.galley.text().to_string(), t.visual_bounding_rect())) } else { None })
            .collect();
    }

    fn at_text(&self, name: &str) -> Rect {
        self.texts.iter().find(|(t, _)| t == name).map(|(_, r)| *r).unwrap_or_else(|| panic!("{name}: {:?}", self.texts))
    }

    fn has(&self, name: &str) -> bool {
        self.texts.iter().any(|(t, _)| t == name)
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
    let (idioma, aparencia) = (window.at_text("IDIOMA"), window.at_text("APARÊNCIA"));
    assert!(aparencia.top() > idioma.bottom() && (aparencia.left() - idioma.left()).abs() < 2.0, "{idioma:?} then {aparencia:?}");
}

/// The sheet is centered at one size: the row of tabs stays under the pointer from a long tab to a short one.
#[test]
fn the_tabs_stay_put_from_a_long_tab_to_a_short_one() {
    let sobre = |tab: usize| Window::at(tab).at_text("Sobre");
    let (long, short) = (sobre(1), sobre(7));
    assert!((long.top() - short.top()).abs() < 1.0 && (long.left() - short.left()).abs() < 1.0, "{long:?} on Terminal, {short:?} on Sobre");
}

/// The sheet's measures against the reference app's at 1200 x 800: 760 x 640 in the middle, 700 of
/// tabs, content from 274 (668 wide) at 140 under the top of the tabs.
#[test]
fn the_sheet_measures_what_the_reference_does() {
    let mut window = Window::at(0);
    let first = window.at_text("Geral");
    let idioma = window.at_text("IDIOMA");
    // the screen of a default Context is 10 000 points; the sheet is in the middle of it
    let sheet_top = window.ctx.content_rect().center().y - 320.0;
    assert!((first.center().y - (sheet_top + 9.75 + 15.7)).abs() < 2.0, "{first:?} from {sheet_top}");
    window.frame(vec![]);
    // the title's line (15.4 high) starts at 185 - 80 under the sheet's top; its middle is the text's
    assert!((idioma.center().y - (sheet_top + 44.0 + 16.25 + 29.0 + 16.25 + 7.7)).abs() < 2.0, "{idioma:?}");
}

/// What the ☰ menu held lives here now: the canvas's in Geral › Tela, the editor's too.
#[test]
fn ajustes_holds_what_the_menu_had() {
    let mut window = Window::at(0);
    for name in ["Idioma", "Aparência", "Abrir o app em"] {
        assert!(window.has(name), "{name} in Sistema: {:?}", window.texts);
    }
    window.click_text("Tela");
    for name in ["Modo de seleção por caixa", "Mostrar dicas", "Dispositivo de entrada", "Passo do zoom", "Ligaduras no código", "Fundo"] {
        assert!(window.has(name), "{name} in Tela: {:?}", window.texts);
    }
}

#[test]
fn a_click_opens_a_section_and_flips_a_switch() {
    let mut window = Window::new();
    window.click_text("Agentes");
    assert_eq!(window.app.settings_tab, 2, "the tab opens");
    window.click_text("Geral");
    window.click_text("Tela");
    let hints = window.app.prefs.show_hints;
    // the switch: 19 from the right of the 668 column, level with its label
    let label = window.at_text("Mostrar dicas");
    window.click(Pos2::new(label.left() + 668.0 - 19.0, label.center().y + 2.0));
    assert_eq!(window.app.prefs.show_hints, !hints, "the switch flips it");
}

#[test]
fn a_select_changes_its_setting_and_the_zoom_step_steps() {
    let mut window = Window::new();
    window.click_text("Tela");
    let step = window.app.prefs.zoom_step;
    assert_eq!(step, 10);
    // the plus of the stepper, 22.75 wide at the right end of the row
    let label = window.at_text("Passo do zoom");
    window.click(Pos2::new(label.left() + 668.0 - 11.0, label.center().y));
    assert_eq!(window.app.prefs.zoom_step, 11, "+ adds one");
    window.click_text("Sistema");
    window.click_text("Escuro");
    window.click_text("Claro");
    assert_eq!(window.app.prefs.theme, Theme::Light, "the menu picks it");
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
    let mut window = Window::at(2);
    for name in ["Chat", "Uso"] {
        assert!(window.has(name), "{name}: {:?}", window.texts);
    }
    // asked out: the skills, the memory in use, the hooks (Controle) and the avatar
    for name in ["Skills", "Memória", "Controle", "Avatar"] {
        assert!(!window.has(name), "{name}: {:?}", window.texts);
    }
    assert!(!window.has("Contas"), "the Contas tab is gone");
    window.click_text("Uso");
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
    let ctx = egui::Context::default();
    window.app.open_settings_at(&ctx, "Agentes", "Uso");
    assert_eq!(super::SETTINGS_TABS[window.app.settings_tab], "Agentes");
    assert_eq!(ctx.data(|d| d.get_temp::<usize>(egui::Id::unique(("settings_sub", "agents")))), Some(1));
}

#[test]
fn the_voice_tab_is_back_with_the_call_and_the_others_keep_their_places() {
    let window = Window::at(0);
    assert!(window.has("Voz"), "{:?}", window.texts);
    assert_eq!(SETTINGS_TABS[3], "Voz");
    let (atalhos, sobre) = (Window::at(5), Window::at(7));
    assert!(atalhos.has("Atalhos") && sobre.has("Sobre"));
}

#[test]
fn typing_in_the_shortcut_search_narrows_the_list() {
    let mut window = Window::at(5);
    assert!(window.has("Desfazer") && window.has("Salvar"));
    window.click_text("Buscar pelo nome ou pela tecla");
    window.frame(vec![egui::Event::Text("salv".into())]);
    window.frame(vec![]);
    assert!(window.has("Salvar") && !window.has("Desfazer"), "{:?}", window.texts);
}

#[test]
fn sobre_offers_the_check_and_then_the_update_with_its_notes() {
    use crate::app::system::updates::{Release, UpdateState};
    let mut window = Window::at(7);
    assert!(window.has("Verificar atualizações"), "{:?}", window.texts);
    let release = Release { version: "99.0.0".into(), notes: "## Novo\n- mais legível".into(), page: String::new(), installer: None };
    window.app.updates.state = UpdateState::Available(release);
    window.frame(vec![]);
    assert!(window.has("Atualizar") && window.has("NOVIDADES DA 99.0.0"), "{:?}", window.texts);
    assert!(!window.has("Verificar atualizações"));
}

/// Integrações: off, GitHub is only its head; on, the token's row; with an answer, who and the pull requests.
#[test]
fn integracoes_shows_github_with_its_token_row_and_its_pull_requests() {
    use crate::app::integrations::github::{GitHub, Pulse};
    assert_eq!(SETTINGS_TABS[4], "Integrações");
    let mut window = Window::at(4);
    assert!(window.has("GitHub") && !window.has("Token pessoal"), "{:?}", window.texts);
    // its switch: 19 from the right of the 668 column, level with its name (16 after the dot)
    let name = window.at_text("GitHub");
    window.click(Pos2::new(name.left() - 16.0 + 668.0 - 19.0, name.center().y));
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
    let mut window = Window::at(4);
    assert!(window.has("Spotify") && !window.has("Próxima"), "{:?}", window.texts);
    window.app.prefs.integrations.spotify = true;
    window.frame(vec![]);
    assert!(window.texts.iter().any(|(text, _)| text.contains("não está aberto")) && !window.has("Próxima"), "closed: no buttons: {:?}", window.texts);
    window.app.integrations.spotify.now = Some(Now { track: Track::new("Canção", "Banda", "", "").unwrap(), playing: true });
    window.frame(vec![]);
    assert!(window.has("Tocando: Canção — Banda") && window.has("Pausar") && window.has("Anterior"), "{:?}", window.texts);
    window.click_text("Próxima");
    window.click_text("Pausar");
    assert_eq!(window.app.integrations.spotify.asked, [Command::Next, Command::PlayPause]);
    window.app.integrations.spotify.now.as_mut().unwrap().playing = false;
    window.frame(vec![]);
    assert!(window.has("Pausado: Canção — Banda") && window.has("Tocar"), "{:?}", window.texts);
}

/// Hermes Agent: on, the plugin's state and its buttons; "Instalar…" says what it will write before writing.
#[test]
fn integracoes_asks_before_installing_the_hermes_plugin() {
    use crate::app::integrations::hermes;
    let home = hermes::home().unwrap();
    assert!(home.starts_with(std::env::temp_dir()), "never the person's own Hermes");
    let mut window = Window::at(4);
    assert!(window.has("Hermes Agent") && !window.has("Instalar…"), "{:?}", window.texts);
    window.app.prefs.integrations.hermes = true;
    window.frame(vec![]);
    // no Hermes on this (the tests') machine: it says so and offers nothing to press
    assert!(window.has("Plugin do MagoAgent no Hermes") && window.has(hermes::Status::NoHermes.words()), "{:?}", window.texts);
    assert!(!window.has("Instalar…") && !window.has("Remover"), "{:?}", window.texts);
    // "Instalar…" was pressed: what it will write, and the two answers
    window.app.integrations.hermes_confirm = true;
    window.frame(vec![]);
    assert!(window.has("Instalar o plugin no Hermes?") && window.has("Instalar") && window.has("Cancelar"), "{:?}", window.texts);
    assert!(window.texts.iter().any(|(text, _)| text.contains("plugins.enabled") && text.contains("backup")), "{:?}", window.texts);
    window.click_text("Cancelar");
    assert!(!window.app.integrations.hermes_confirm && !window.has("Instalar o plugin no Hermes?"));
    window.app.integrations.hermes_confirm = true;
    window.frame(vec![]);
    window.click_text("Instalar");
    assert!(!window.app.integrations.hermes_confirm);
    assert_eq!(window.app.status, hermes::Status::NoHermes.words(), "what came of it is said");
    assert!(!home.exists(), "nothing was written");
}

#[test]
fn a_global_shortcut_is_recorded_by_a_click_on_its_keys_with_the_altgr_warning() {
    let mut window = Window::at(5);
    assert!(window.has("ATALHOS GLOBAIS DA ILHA") && window.has("Abrir o chat da ilha"), "{:?}", window.texts);
    assert!(window.has("Ctrl+Alt+Espaço") && window.has("Ctrl+Alt+Direita"), "the default keys: {:?}", window.texts);
    window.click_text("Ctrl+Alt+Espaço");
    assert_eq!(window.app.hotkeys.recording, Some(0));
    assert!(window.texts.iter().any(|(text, _)| text.contains("ABNT2") && text.contains("AltGr")), "the recorder warns of AltGr: {:?}", window.texts);
    // pressed, then let go the frame after: a key still down would come again as a repetition
    let key = |key, modifiers, pressed| egui::Event::Key { key, physical_key: None, pressed, repeat: false, modifiers };
    // a key alone is no global shortcut: the recorder waits for a combination
    window.frame(vec![key(Key::K, Modifiers::NONE, true)]);
    window.frame(vec![key(Key::K, Modifiers::NONE, false)]);
    assert_eq!(window.app.hotkeys.recording, Some(0));
    assert!(window.texts.iter().any(|(text, _)| text.contains("Junte Ctrl ou Alt")), "{:?}", window.texts);
    window.frame(vec![key(Key::K, Modifiers { ctrl: true, shift: true, ..Modifiers::NONE }, true)]);
    window.frame(vec![key(Key::K, Modifiers { ctrl: true, shift: true, ..Modifiers::NONE }, false)]);
    assert_eq!(window.app.hotkeys.recording, None);
    assert_eq!(window.app.prefs.shortcuts["openChat"].keys, "Ctrl+Shift+K");
    assert!(window.has("Ctrl+Shift+K"), "{:?}", window.texts);
    // AltGr+E types ° on ABNT2: the warning stays under the shortcut
    window.click_text("Ctrl+Shift+K");
    window.frame(vec![key(Key::E, Modifiers { ctrl: true, alt: true, ..Modifiers::NONE }, true)]);
    window.frame(vec![key(Key::E, Modifiers { ctrl: true, alt: true, ..Modifiers::NONE }, false)]);
    assert!(window.texts.iter().any(|(text, _)| text.contains("«°»")), "{:?}", window.texts);
    // Esc ends the recording, not Ajustes; Backspace leaves the shortcut without a key
    window.click_text("Ctrl+Alt+E");
    window.frame(vec![key(Key::Escape, Modifiers::NONE, true)]);
    window.frame(vec![key(Key::Escape, Modifiers::NONE, false)]);
    assert!(window.app.settings_open && window.app.hotkeys.recording.is_none());
    assert_eq!(window.app.prefs.shortcuts["openChat"].keys, "Ctrl+Alt+E");
    window.click_text("Ctrl+Alt+E");
    window.frame(vec![key(Key::Backspace, Modifiers::NONE, true)]);
    window.frame(vec![key(Key::Backspace, Modifiers::NONE, false)]);
    assert_eq!(window.app.prefs.shortcuts["openChat"].keys, "");
    assert!(window.has("Sem tecla"), "{:?}", window.texts);
}
