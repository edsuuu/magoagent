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
    let (long, short) = (sobre(1), sobre(6));
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
    window.click_text("Avatar");
    assert!(window.has("Mostrar o avatar"), "{:?}", window.texts);
    assert!(!window.app.prefs.avatar);
    // the switch: 19 from the right of the 668 column, level with its label
    let label = window.at_text("Mostrar o avatar");
    window.click(Pos2::new(label.left() + 668.0 - 19.0, label.center().y + 2.0));
    assert!(window.app.prefs.avatar, "the switch flips it");
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
    for name in ["Chat", "Skills", "Uso", "Memória", "Controle", "Avatar"] {
        assert!(window.has(name), "{name}: {:?}", window.texts);
    }
    assert!(!window.has("Contas"), "the Contas tab is gone");
    window.click_text("Uso");
    for name in ["CONSUMO DO PLANO", "LIMITES DO MAESTRO", "Agentes ao mesmo tempo", "Parar de recrutar em"] {
        assert!(window.has(name), "{name} in Uso: {:?}", window.texts);
    }
    assert!(!window.has("MEMÓRIA EM USO"), "the memory is its own tab");
    window.click_text("Memória");
    assert!(window.has("MEMÓRIA EM USO"), "{:?}", window.texts);
    // the API keys are back, in their own sub-tab
    window.click_text("Chaves de API");
    assert!(window.has("CHAVES DE API") && window.has("Usar as chaves de API nos chats"), "{:?}", window.texts);
}

#[test]
fn the_usage_pill_opens_agentes_uso() {
    let mut window = Window::at(0);
    let ctx = egui::Context::default();
    window.app.open_settings_at(&ctx, "Agentes", "Uso");
    assert_eq!(super::SETTINGS_TABS[window.app.settings_tab], "Agentes");
    assert_eq!(ctx.data(|d| d.get_temp::<usize>(egui::Id::unique(("settings_sub", "agents")))), Some(2));
}

#[test]
fn the_voice_tab_is_hidden_but_the_others_keep_their_places() {
    let window = Window::at(0);
    assert!(!window.has("Voz"), "{:?}", window.texts);
    assert_eq!(SETTINGS_TABS[3], "Voz", "the code stays; only the tab is not drawn");
    let (atalhos, sobre) = (Window::at(4), Window::at(6));
    assert!(atalhos.has("Atalhos") && sobre.has("Sobre"));
}

#[test]
fn typing_in_the_shortcut_search_narrows_the_list() {
    let mut window = Window::at(4);
    assert!(window.has("Desfazer") && window.has("Salvar"));
    window.click_text("Buscar pelo nome ou pela tecla");
    window.frame(vec![egui::Event::Text("salv".into())]);
    window.frame(vec![]);
    assert!(window.has("Salvar") && !window.has("Desfazer"), "{:?}", window.texts);
}

#[test]
fn sobre_offers_the_check_and_then_the_update_with_its_notes() {
    use crate::app::system::updates::{Release, UpdateState};
    let mut window = Window::at(6);
    assert!(window.has("Verificar atualizações"), "{:?}", window.texts);
    let release = Release { version: "99.0.0".into(), notes: "## Novo\n- mais legível".into(), page: String::new(), installer: None };
    window.app.updates.state = UpdateState::Available(release);
    window.frame(vec![]);
    assert!(window.has("Atualizar") && window.has("NOVIDADES DA 99.0.0"), "{:?}", window.texts);
    assert!(!window.has("Verificar atualizações"));
}
