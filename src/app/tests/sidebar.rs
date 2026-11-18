use super::*;
use crate::app::ui::sidebar::SIDEBAR;
use crate::app::usage::probe::{Agent, Meter, Reading};

const SIZE: Vec2 = Vec2::new(1000.0, 800.0);

/// One frame of the sidebar alone; what it painted, each text with its place.
fn side(app: &mut DrawingApp, ctx: &Context, events: Vec<egui::Event>) -> Vec<(String, Rect)> {
    let raw = egui::RawInput { screen_rect: Some(Rect::from_min_size(Pos2::ZERO, SIZE)), events, ..Default::default() };
    let mut output = ctx.run_ui(raw, |_| app.sidebar(ctx));
    output.textures_delta.clear();
    fn texts(shape: &Shape, out: &mut Vec<(String, Rect)>) {
        match shape {
            Shape::Text(text) => out.push((text.galley.text().to_string(), text.visual_bounding_rect())),
            Shape::Vec(shapes) => shapes.iter().for_each(|shape| texts(shape, out)),
            _ => {}
        }
    }
    let mut out = vec![];
    output.shapes.iter().for_each(|shape| texts(&shape.shape, &mut out));
    out
}

fn found(painted: &[(String, Rect)], text: &str) -> Option<Rect> {
    painted.iter().find(|(painted, _)| painted == text).map(|(_, rect)| *rect)
}

/// Draws twice (an area shows from its second frame), presses `text` and gives what the frame after painted.
fn press(app: &mut DrawingApp, ctx: &Context, text: &str) -> Vec<(String, Rect)> {
    side(app, ctx, vec![]);
    let at = found(&side(app, ctx, vec![]), text).unwrap_or_else(|| panic!("{text} not painted")).center();
    side(app, ctx, vec![egui::Event::PointerMoved(at), button(at, true)]);
    side(app, ctx, vec![button(at, false)]);
    side(app, ctx, vec![]);
    side(app, ctx, vec![])
}

/// A terminal on the canvas running `command`.
fn terminal(app: &mut DrawingApp, name: &str, command: &str, x: f32) -> String {
    let mut element = app.new_element(ElementKind::Rectangle { x, y: 0.0, width: 400.0, height: 300.0 });
    element.terminal = Some(TerminalSpec { name: name.into(), command: command.into(), ..Default::default() });
    app.scene.elements.push(element);
    app.scene.elements.last().unwrap().id.clone()
}

/// A permission the hook of agent `id` holds for the person, as Claude Code's PermissionRequest brings it.
fn asks_permission(app: &mut DrawingApp, id: &str) {
    let asks = json!({ "hook_event_name": "PermissionRequest", "tool_name": "Bash", "tool_input": { "command": "ls" }, "session_id": "s" });
    let (reply, _answer) = std::sync::mpsc::channel();
    app.take_hook(id, &asks, Some(reply));
}

/// Two agents on the canvas, each asking the person for a permission.
fn two_agents(app: &mut DrawingApp) -> (String, String) {
    let (first, second) = (terminal(app, "Revisor", "claude", 0.0), terminal(app, "Testador", "claude", 2000.0));
    asks_permission(app, &first);
    asks_permission(app, &second);
    (first, second)
}

#[test]
fn the_card_counts_who_needs_you_in_every_space() {
    let ctx = Context::default();
    let mut app = DrawingApp::default();
    side(&mut app, &ctx, vec![]);
    assert!(found(&side(&mut app, &ctx, vec![]), "Nada esperando por você").is_some(), "nobody waits: the green notice");
    two_agents(&mut app);
    // one more in a space off view
    let spec = TerminalSpec { name: "Longe".into(), command: "claude".into(), ..Default::default() };
    app.away.insert("longe".into(), ("outro".into(), spec));
    asks_permission(&mut app, "longe");
    let painted = side(&mut app, &ctx, vec![]);
    assert!(found(&painted, "3 agentes precisam de você").is_some(), "{painted:?}");
    assert!(found(&painted, "Ir para o próximo").is_some() && found(&painted, &keys_label("Cmd ⇧ A")).is_some(), "the button and its keys");
    // the agents of this space are only the two on its canvas
    assert!(found(&painted, "Revisor").is_some() && found(&painted, "Longe").is_none());
    assert!(painted.iter().any(|(text, _)| text.starts_with("Encerrado")), "its state under its name: {painted:?}");
    let said = app.act("sidebar", &json!({ "action": "status" })).unwrap();
    assert!(said.starts_with("3 agentes precisam de você.") && said.contains("Revisor") && !said.contains("Longe"), "{said}");
}

#[test]
fn ir_para_o_proximo_goes_round_the_ones_that_need_you() {
    let ctx = Context::default();
    let mut app = DrawingApp::default();
    let (first, second) = two_agents(&mut app);
    press(&mut app, &ctx, "Ir para o próximo");
    assert_eq!(app.terminal_focus.as_ref(), Some(&first), "the first one, selected and taking keys");
    assert_eq!(app.selected, [0]);
    // answered in its terminal, it leaves the count
    app.hooks.pending.retain(|(event, _)| event.id != first);
    let painted = side(&mut app, &ctx, vec![]);
    assert!(found(&painted, "1 agente precisa de você").is_some(), "{painted:?}");
    // the MCP's button does the same
    assert_eq!(app.act("sidebar", &json!({ "action": "next" })).unwrap(), "Testador em foco.");
    assert_eq!(app.terminal_focus.as_ref(), Some(&second));
    app.hooks.pending.clear();
    assert!(app.act("sidebar", &json!({ "action": "next" })).unwrap_err().contains("Nenhum agente"));
}

/// Asked: "'Agentes precisam de você' — em 90% dos casos não precisa: foram agentes novos, shells que o Maestro abriu. Ali
/// deve aparecer apenas quando o Maestro me chamar, para ser mais claro". A new agent, one that finished unseen, a shell
/// and a chat at rest call nobody; the Maestro asking the person (`ask_person`) does, and so does a permission.
#[test]
fn only_an_agent_asking_the_person_counts_as_needing_them() {
    use crate::app::ui::agents::{AgentState, calls_person};
    assert!(calls_person(AgentState::Waiting, true, false), "an agent's question or permission");
    assert!(calls_person(AgentState::Idle(None), true, true), "a permission its hook holds");
    assert!(!calls_person(AgentState::Waiting, false, false), "a shell on a menu");
    for state in [AgentState::Idle(None), AgentState::Idle(Some(1.0)), AgentState::Working(1.0), AgentState::Active(1.0), AgentState::Ended] {
        assert!(!calls_person(state, true, false), "{state:?}");
    }
    let ctx = Context::default();
    let mut app = DrawingApp::default();
    // the Maestro's chat, a new agent, one that finished unseen and a shell it opened (tests running)
    app.create_chat(&crate::app::chat::direct::NewChat { agent: 0, cwd: "~/projeto".into(), mode: 0 }, &[Preset::new("Claude Code", "claude")]);
    let luna = app.chat_agent.clone().unwrap();
    terminal(&mut app, "Novo", "claude", 0.0);
    let done = terminal(&mut app, "Pronto", "claude", 1000.0);
    let shell = terminal(&mut app, "Testes", "cargo test", 2000.0);
    app.terminal_unread.extend([done, shell]);
    side(&mut app, &ctx, vec![]);
    assert!(found(&side(&mut app, &ctx, vec![]), "Nada esperando por você").is_some(), "nobody calls");
    assert!(app.needing_you().is_empty() && app.space_counts().values().all(|(_, needs)| *needs == 0));
    // the Maestro asks the person
    let (reply, _answer) = std::sync::mpsc::channel();
    app.person_asks.borrow_mut().push(crate::app::chat::decide::PersonAsk::new(luna.clone(), &json!({ "tool_name": "AskUserQuestion" }), reply));
    assert!(found(&side(&mut app, &ctx, vec![]), "1 agente precisa de você").is_some());
    assert_eq!(app.needing_you(), [luna]);
}

/// Asked: "precisa ter uma opção x para ocultar ou remover". The green notice's × hides it for good; somebody waiting
/// still shows the amber card; the MCP brings it back.
#[test]
fn the_green_notice_closes_by_its_x() {
    let ctx = Context::default();
    let mut app = DrawingApp::default();
    press(&mut app, &ctx, "×");
    assert!(app.prefs.sidebar_clear_hidden, "kept in the preferences");
    assert!(found(&side(&mut app, &ctx, vec![]), "Nada esperando por você").is_none());
    app.act("sidebar", &json!({ "action": "notice_show" })).unwrap();
    assert!(found(&side(&mut app, &ctx, vec![]), "Nada esperando por você").is_some());
    app.act("sidebar", &json!({ "action": "notice_hide" })).unwrap();
    assert!(app.prefs.sidebar_clear_hidden);
}

#[test]
fn the_agents_of_the_space_fold_to_their_count_and_a_row_selects_its_card() {
    let ctx = Context::default();
    let mut app = DrawingApp::default();
    let (_, second) = two_agents(&mut app);
    let painted = press(&mut app, &ctx, "AGENTES DESTE ESPAÇO");
    assert!(app.prefs.sidebar_agents_closed, "folded, and kept in the preferences");
    assert!(found(&painted, "Revisor").is_none() && found(&painted, "2").is_some(), "only the count: {painted:?}");
    press(&mut app, &ctx, "AGENTES DESTE ESPAÇO");
    assert!(!app.prefs.sidebar_agents_closed);
    // the MCP folds and opens it too
    app.act("sidebar", &json!({ "action": "agents_close" })).unwrap();
    assert!(app.prefs.sidebar_agents_closed);
    app.act("sidebar", &json!({ "action": "agents_open" })).unwrap();
    press(&mut app, &ctx, "Testador");
    assert_eq!((app.terminal_focus.as_ref(), app.selected.clone()), (Some(&second), vec![1]), "its card, selected in the middle");
}

#[test]
fn the_foot_shows_each_plan_and_its_detail_fits_the_sidebar() {
    let ctx = Context::default();
    let mut app = DrawingApp::default();
    side(&mut app, &ctx, vec![]);
    assert!(found(&side(&mut app, &ctx, vec![]), "Claude Code · sessão 5 h").is_none(), "nothing read, no line");
    assert!(app.act("sidebar", &json!({ "action": "usage_open" })).is_err());
    let meter = |title: &str, used: f64| Meter { title: title.into(), used, resets_at: Some(now_ms() / 1000 + 3 * 86400) };
    let reading = Reading { plan: Some("Max".into()), meters: vec![meter("Sessão", 0.98), meter("Semanal", 0.43), meter("Fable", 0.79)] };
    app.usage_ui.probe.sender.send((Agent::Claude, Ok(reading.clone()))).unwrap();
    // a plan with no session (Codex's Go) shows the window it has
    app.usage_ui.probe.sender.send((Agent::Codex, Ok(Reading { plan: None, meters: vec![meter("Semanal", 0.2)] }))).unwrap();
    app.usage_ui.probe.collect();
    let painted = side(&mut app, &ctx, vec![]);
    for want in ["Claude Code · sessão 5 h", "98%", "Codex · semana", "20%"] {
        assert!(found(&painted, want).is_some(), "{want}: {painted:?}");
    }
    let painted = press(&mut app, &ctx, "Claude Code · sessão 5 h");
    assert!(app.usage_ui.open);
    for want in ["Uso dos agentes", "Max", "Sessão 5 h", "Semana", "Fable", "43%", "Maestro: 0 de 6 agentes; para de recrutar a 90%.", "Configurações…"] {
        assert!(found(&painted, want).is_some(), "{want}: {painted:?}");
    }
    assert!(painted.iter().any(|(text, _)| text.starts_with("Sessão renova ") && text.contains(" · semana ")), "{painted:?}");
    // no wider than the sidebar, nothing out of it
    let card = ctx.memory(|memory| memory.area_rect(egui::Id::unique("usage_card"))).unwrap();
    assert!(card.left() >= 0.0 && card.right() <= SIDEBAR, "{card:?}");
    assert!(painted.iter().filter(|(_, rect)| card.contains_rect(*rect)).all(|(_, rect)| rect.right() <= SIDEBAR - 10.0), "{painted:?}");
    // Esc puts it away; the MCP opens and closes it
    side(&mut app, &ctx, vec![egui::Event::Key { key: Key::Escape, physical_key: None, pressed: true, repeat: false, modifiers: Default::default() }]);
    assert!(!app.usage_ui.open);
    app.act("sidebar", &json!({ "action": "usage_open" })).unwrap();
    assert!(app.usage_ui.open);
    app.act("sidebar", &json!({ "action": "usage_close" })).unwrap();
    assert!(!app.usage_ui.open);
    // the Maestro reads the plans, and asks for them again
    app.usage_ui.probe.next = 1e9;
    app.act("usage_refresh", &json!({})).unwrap();
    assert_eq!(app.usage_ui.probe.next, 0.0);
    let said = app.act("usage", &json!({})).unwrap();
    assert!(said.contains("Plano lido pela CLI, Claude Code (Max): Sessão 98% (redefine em 3 dias)"), "{said}");
    // asked ("se eu clico em configurações o meu cursor fica cursor de texto"): the link opens Ajustes › Agentes, it
    // doesn't select its text
    app.act("sidebar", &json!({ "action": "usage_open" })).unwrap();
    press(&mut app, &ctx, "Configurações…");
    assert!(app.settings_open && !app.usage_ui.open);
    assert_eq!(crate::app::ui::settings::SETTINGS_TABS[app.settings_tab], "Agentes");
}

#[test]
fn the_corner_buttons_undo_and_redo() {
    let ctx = Context::default();
    let mut app = DrawingApp::default();
    frame(&mut app, &ctx, vec![]);
    frame(&mut app, &ctx, vec![]);
    let pill = ctx.memory(|memory| memory.area_rect(egui::Id::unique("history_pill"))).unwrap();
    assert!(pill.left() >= SIDEBAR && pill.bottom() > 500.0, "bottom left, past the sidebar: {pill:?}");
    let undo = Pos2::new(pill.left() + 6.5 + CORNER_BUTTON / 2.0, pill.center().y);
    let redo = undo + Vec2::new(CORNER_BUTTON + 5.0, 0.0);
    // nothing to undo: the button does nothing
    click(&mut app, &ctx, undo);
    assert!(app.undo.is_empty() && app.redo.is_empty());
    app.snapshot();
    app.scene.elements.push(app.new_element(ElementKind::Rectangle { x: 300.0, y: 200.0, width: 100.0, height: 100.0 }));
    frame(&mut app, &ctx, vec![]);
    click(&mut app, &ctx, undo);
    assert!(app.scene.elements.is_empty() && app.redo.len() == 1, "undone");
    frame(&mut app, &ctx, vec![]);
    click(&mut app, &ctx, redo);
    assert_eq!(app.scene.elements.len(), 1, "redone");
}

#[test]
fn ver_tudo_fits_the_drawing() {
    let ctx = Context::default();
    let mut app = DrawingApp::default();
    two_boxes(&mut app);
    app.pan = Vec2::new(5000.0, 5000.0);
    frame(&mut app, &ctx, vec![]);
    frame(&mut app, &ctx, vec![]);
    let pill = ctx.memory(|memory| memory.area_rect(egui::Id::unique("footer"))).unwrap();
    click(&mut app, &ctx, Pos2::new(pill.left() + 6.5 + CORNER_BUTTON / 2.0, pill.center().y));
    frame(&mut app, &ctx, vec![]);
    let screen = app.canvas_rect;
    for element in &app.scene.elements {
        let bounds = element_bounds(element);
        assert!(screen.contains(app.to_screen(bounds.min)) && screen.contains(app.to_screen(bounds.max)), "{bounds:?} in {screen:?}");
    }
}

/// The canvas felt stuck: the minis of the agents at rest repainted the app at 30 frames a second while nothing moved.
#[test]
fn the_agents_at_rest_in_the_list_ask_for_no_frames() {
    let ctx = Context::default();
    let mut app = DrawingApp::default();
    two_agents(&mut app);
    side(&mut app, &ctx, vec![]);
    assert!(found(&side(&mut app, &ctx, vec![]), "Revisor").is_some(), "its row is there");
    let causes = ctx.repaint_causes();
    assert!(!causes.iter().any(|cause| cause.file.contains("mascot")), "an ended agent's wizard is still: {causes:?}");
}
