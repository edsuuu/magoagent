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
        let ctx = Context::default();
        // the titles are in the system face's weight 600, a family of the app's own
        crate::app::ui::install_fonts(&ctx, false);
        let mut window = Self { app, ctx, time: 0.0, texts: vec![] };
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
    let mut window = Window::at(2);
    for name in ["Uso", "Hooks"] {
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
    assert_eq!(ctx.data(|d| d.get_temp::<usize>(egui::Id::unique(("settings_sub", "agents")))), Some(0));
}

#[test]
fn the_voice_tab_is_back_with_the_call_and_the_others_keep_their_places() {
    let window = Window::at(0);
    assert!(window.has("Voz"), "{:?}", window.texts);
    assert_eq!(SETTINGS_TABS[4], "Voz");
    let (atalhos, sobre) = (Window::at(9), Window::at(11));
    assert!(atalhos.has("Atalhos") && sobre.has("Sobre"));
}

#[test]
fn typing_in_the_shortcut_search_narrows_the_list() {
    let mut window = Window::at(9);
    assert!(window.has("Desfazer") && window.has("Salvar"));
    window.click_text("Buscar pelo nome ou pela tecla");
    window.frame(vec![egui::Event::Text("salv".into())]);
    window.frame(vec![]);
    assert!(window.has("Salvar") && !window.has("Desfazer"), "{:?}", window.texts);
}

#[test]
fn sobre_offers_the_check_and_then_the_update_with_its_notes() {
    use crate::app::system::updates::{Release, UpdateState};
    let mut window = Window::at(11);
    assert!(window.has("Verificar atualizações"), "{:?}", window.texts);
    let release = Release { version: "99.0.0".into(), notes: "## Novo\n- mais legível".into(), page: String::new(), installer: None, digest: None };
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
    let release =
        Release { version: "99.0.0".into(), notes: String::new(), page: String::new(), installer: Some("https://github.com/x.msi".into()), digest: None };
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
    let mut window = Window::at(6);
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
    assert!(window.has("ARQUIVO") && window.has("Salvar") && window.has("Ctrl+Shift+Z ou Ctrl+Y") && window.has("TECLAS FIXAS"), "{:?}", window.texts);
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
    press(&mut window, Key::S, Modifiers::CTRL);
    assert!(says(&window, "já é o atalho «Salvar»"), "{:?}", window.texts);
    // free keys wait, shown, until Salvar
    let ctrl_shift = Modifiers { ctrl: true, shift: true, ..Modifiers::NONE };
    press(&mut window, Key::K, ctrl_shift);
    assert!(window.has("Ctrl+Shift+K") && says(&window, "Salvar (ou Enter) aplica"), "{:?}", window.texts);
    assert!(window.app.prefs.shortcuts.is_empty(), "not saved yet");
    window.click_text("Salvar");
    assert_eq!(window.app.hotkeys.recording, None);
    assert_eq!(window.app.prefs.shortcuts["openChat"].keys, "Ctrl+Shift+K");
    assert!(window.has("Ctrl+Shift+K"), "{:?}", window.texts);
    // Esc drops what was pressed, not Ajustes; so does Cancelar
    let ctrl_alt = Modifiers { ctrl: true, alt: true, ..Modifiers::NONE };
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
    window.scroll_to("Ctrl+Z");
    let clear = ghost_right_of(&window, "Ctrl+S", 1.0);
    window.click(clear);
    assert_eq!(window.app.prefs.shortcuts["save"].keys, "", "Salvar without a key");
    // Desfazer takes Ctrl+S, now free
    window.click_text("Ctrl+Z");
    press(&mut window, Key::S, Modifiers::CTRL);
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
    let undo = ghost_right_of(&window, "Ctrl+S", 2.0);
    window.click(undo);
    window.click(back);
    assert!(window.app.prefs.shortcuts.is_empty(), "every default: {:?}", window.app.prefs.shortcuts);
    assert!(window.has("Ctrl+S") && window.has("Ctrl+Z"));
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
    // Agentes keeps Uso and Hooks only
    let agents = Window::at(tab_of("Agentes"));
    assert!(agents.has("Uso") && agents.has("Hooks") && !agents.has("Pasta de trabalho"), "{:?}", agents.texts);
}

#[test]
fn every_page_is_in_the_sidebar_at_once() {
    let window = Window::at(SETTINGS_TABS.len() - 1);
    for name in SETTINGS_TABS {
        assert!(window.has(name), "{name}: {:?}", window.texts);
    }
}

#[test]
fn dicionario_adds_and_removes_words_and_replacements() {
    let mut window = Window::at(tab_of("Dicionário"));
    window.click_text("Supabase");
    window.frame(vec![egui::Event::Text("Supabase".into())]);
    window.click_text("Adicionar");
    assert_eq!(window.app.prefs.dictionary.words, ["Supabase"], "{:?}", window.texts);
    // the word is listed under the field, whose hint is the same word
    assert_eq!(window.texts.iter().filter(|(text, _)| text == "Supabase").count(), 2, "{:?}", window.texts);
    window.click_text("cloud code");
    window.frame(vec![egui::Event::Text("cloud code".into())]);
    window.click_text("Claude Code");
    window.frame(vec![egui::Event::Text("Claude Code".into())]);
    // the second "Adicionar" is the replacement's
    let add = window.texts.iter().filter(|(text, _)| text == "Adicionar").map(|(_, rect)| rect.center()).next_back().unwrap();
    window.click(add);
    assert_eq!(window.app.prefs.dictionary.apply("abre o cloud code"), "abre o Claude Code");
}

#[test]
fn ok_mago_switches_are_the_real_preferences() {
    let mut window = Window::at(tab_of("Ok Mago"));
    for name in ["Responder a “Ok Mago”", "Responder em voz alta", "Iniciar a chamada"] {
        assert!(window.has(name), "{name}: {:?}", window.texts);
    }
    assert!(window.app.prefs.ok_mago && window.app.prefs.speak_answers);
    let switch = window.switch_of("Responder em voz alta");
    window.click(switch);
    assert!(!window.app.prefs.speak_answers);
}

#[test]
fn email_fills_the_gmail_example_and_hands_it_to_the_agents() {
    let mut window = Window::at(tab_of("Email"));
    assert!(window.has("Programa") && window.has("Argumentos") && window.has("Conexão"), "{:?}", window.texts);
    window.click_text("Usar o do Gmail");
    assert_eq!(window.app.prefs.email.command, "npx");
    assert!(window.app.prefs.email.args.contains("server-gmail"));
    window.scroll_to("Dar o servidor aos agentes");
    let switch = window.switch_of("Dar o servidor aos agentes");
    window.click(switch);
    assert!(window.app.prefs.email.on);
    assert!(crate::app::chat::tools::mcp_servers(&window.app.prefs)["email"]["command"] == "npx");
}
