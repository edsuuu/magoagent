//! The Chats area off the canvas (chat/list/running.rs): everything running grouped by space, the chips and the search,
//! the answers on a row, Ver no canvas, and the title bar's badge and title.

use super::*;
use crate::app::chat::decide::PersonAsk;
use crate::app::chat::list::{LIST, Show};
use crate::app::workspaces::Workspace;

const SIZE: Vec2 = Vec2::new(1400.0, 900.0);

/// One frame of the chat screen and the title bar: the texts painted, where.
fn shown(app: &mut DrawingApp, ctx: &Context, events: Vec<egui::Event>) -> Vec<(String, Rect)> {
    let raw = egui::RawInput { screen_rect: Some(Rect::from_min_size(Pos2::ZERO, SIZE)), events, ..Default::default() };

    let mut output = ctx.run_ui(raw, |_| {
        app.chat_screen(ctx);
        app.title_bar(ctx);
    });

    output.textures_delta.clear();

    output
        .shapes
        .iter()
        .filter_map(|s| if let Shape::Text(t) = &s.shape { Some((t.galley.text().to_string(), t.visual_bounding_rect())) } else { None })
        .collect()
}

fn at(texts: &[(String, Rect)], prefix: &str) -> Option<Rect> {
    texts.iter().find(|(text, _)| text.starts_with(prefix)).map(|(_, rect)| *rect)
}

/// Where a text starting with `prefix` was painted in the list (not the conversation's header), the topmost.
fn in_list(texts: &[(String, Rect)], prefix: &str) -> Option<Rect> {
    let inside = texts.iter().filter(|(text, rect)| text.starts_with(prefix) && rect.left() < LIST + 16.0).map(|(_, rect)| *rect);

    inside.min_by(|a, b| a.top().total_cmp(&b.top()))
}

fn tap(app: &mut DrawingApp, ctx: &Context, pos: Pos2) -> Vec<(String, Rect)> {
    shown(app, ctx, vec![egui::Event::PointerMoved(pos), button(pos, true)]);
    shown(app, ctx, vec![button(pos, false)]);
    shown(app, ctx, vec![])
}

/// Chat `id` asks the person `request` (a permission, or an AskUserQuestion), shown long enough to take clicks.
fn ask(app: &mut DrawingApp, id: &str, request: Value) -> std::sync::mpsc::Receiver<Result<String, String>> {
    let (reply, answer) = std::sync::mpsc::channel();
    let mut ask = PersonAsk::new(id.into(), &request, reply);

    ask.shown = Some(-100.0);
    app.person_asks.get_mut().push(ask);
    answer
}

/// An app with the space Loja on screen: Revisor's card is on its canvas and Revisor asks for a permission; Migrador
/// has no card and rests; a Claude Code session outside the app works.
fn busy_app() -> (DrawingApp, std::sync::mpsc::Receiver<Result<String, String>>) {
    let mut app = DrawingApp::default();

    app.prefs.island = true;
    app.spaces.list.push(Workspace { id: "loja".into(), name: "Loja".into(), color: [0x0a, 0x84, 0xff], ..Default::default() });
    app.spaces.current = "loja".into();
    let revisor = push_chat(&mut app, "Revisor");

    push_chat(&mut app, "Migrador");
    app.add_chat_node(&revisor, None);
    let answer = ask(&mut app, &revisor, json!({ "tool_name": "Bash", "input": { "command": "cargo test" } }));

    app.take_outside_hook(&json!({ "hook_event_name": "UserPromptSubmit", "session_id": "fora-1", "cwd": "/tmp/site", "prompt": "arrume o rodapé" }), None);
    app.chat_open = true;
    app.chat_agent = Some("migrador".into());
    (app, answer)
}

#[test]
fn the_list_groups_everything_running_by_space_and_the_chips_and_the_search_filter_it() {
    let ctx = Context::default();
    let (mut app, _answer) = busy_app();

    shown(&mut app, &ctx, vec![]);
    let texts = shown(&mut app, &ctx, vec![]);
    let (loja, none, outside) = (in_list(&texts, "LOJA"), in_list(&texts, "SEM ESPAÇO"), in_list(&texts, "FORA DO APP"));
    let (loja, none, outside) = (loja.expect("the space"), none.expect("no space"), outside.unwrap_or_else(|| panic!("outside: {texts:?}")));

    assert!(loja.top() < none.top() && none.top() < outside.top(), "the spaces, then none, then outside");
    let revisor = in_list(&texts, "Revisor").expect("Revisor's row");
    let migrador = in_list(&texts, "Migrador").expect("Migrador's row");

    assert!(loja.top() < revisor.top() && revisor.top() < none.top() && none.top() < migrador.top(), "each under its space");
    // each row's chip, and the session outside's last line is its prompt
    assert!(in_list(&texts, "Pronto").is_some() && in_list(&texts, "arrume o rodapé").is_some(), "{texts:?}");
    assert!(in_list(&texts, "Permissão: cargo test").is_some(), "what it waits on, under its name");

    // the chips over the list
    for chip in ["Todos", "Trabalhando", "Esperando", "Prontos"] {
        assert!(in_list(&texts, chip).is_some_and(|rect| rect.top() < loja.top()), "{chip}: {texts:?}");
    }

    let texts = tap(&mut app, &ctx, in_list(&texts, "Esperando").unwrap().center());

    assert_eq!(app.chat_listing.show, Show::Waiting);

    assert!(
        in_list(&texts, "Revisor").is_some() && in_list(&texts, "Migrador").is_none() && in_list(&texts, "FORA DO APP").is_none(),
        "only who waits: {texts:?}"
    );

    // the search, over everything
    app.chat_listing.show = Show::All;
    app.chat_listing.filter = "rodapé".into();
    let texts = shown(&mut app, &ctx, vec![]);

    assert!(at(&texts, "FORA DO APP").is_some() && at(&texts, "LOJA").is_none(), "the last line found: {texts:?}");
    app.chat_listing.filter = "nada assim".into();
    let texts = shown(&mut app, &ctx, vec![]);

    assert!(at(&texts, "Nada com esse filtro.").is_some());
}

#[test]
fn a_permission_and_a_question_are_answered_right_on_their_row() {
    let ctx = Context::default();
    let (mut app, answer) = busy_app();

    shown(&mut app, &ctx, vec![]);
    let texts = shown(&mut app, &ctx, vec![]);
    let allow = at(&texts, "Permitir").unwrap_or_else(|| panic!("the row's buttons: {texts:?}"));

    assert!(at(&texts, "Negar").is_some_and(|deny| deny.left() > allow.right()));
    tap(&mut app, &ctx, allow.center());
    assert!(app.person_asks.borrow().is_empty(), "answered");
    assert!(answer.try_recv().unwrap().unwrap().contains("allow"), "as the card answers it");

    // a question: its options are the buttons
    let questions = json!({ "tool_name": "AskUserQuestion", "input": { "questions": [
        { "question": "Qual cor?", "header": "Cor", "multiSelect": false, "options": [{ "label": "Azul", "description": "" }, { "label": "Verde", "description": "" }] },
    ] } });

    let answer = ask(&mut app, "revisor", questions);

    shown(&mut app, &ctx, vec![]);
    let texts = shown(&mut app, &ctx, vec![]);

    assert!(at(&texts, "Qual cor?").is_some(), "the question under the name: {texts:?}");
    tap(&mut app, &ctx, at(&texts, "Verde").expect("an option").center());
    assert!(app.person_asks.borrow().is_empty());
    assert!(answer.try_recv().unwrap().unwrap().contains("Verde"));
}

#[test]
fn ver_no_canvas_goes_to_the_agents_space_and_card() {
    let dir = std::env::temp_dir().join(format!("mago-chats-ver-{}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);

    fs::create_dir_all(&dir).unwrap();
    fs::write(dir.join("autosave.mago"), r#"{"version":1,"elements":[]}"#).unwrap();
    let mut app = DrawingApp { autosave: Some(dir.join("autosave.mago")), ..Default::default() };

    app.load_spaces();
    app.load_autosave();
    // a Claude Code terminal of the space Principal
    let mut terminal = app.new_element(ElementKind::Rectangle { x: 2000.0, y: 1500.0, width: 400.0, height: 300.0 });

    terminal.terminal = Some(TerminalSpec { name: "Zelador".into(), command: "claude".into(), ..Default::default() });
    let id = terminal.id.clone();

    app.scene.elements.push(terminal);
    app.terminals.insert(id.clone(), Err("sem shell no teste".into()));
    app.dirty = true;
    // the person is in another space and opens its conversation
    let other = app.create_space("Outro", "").unwrap();

    app.switch_space(&other).unwrap();
    app.chat_on(&id);
    let ctx = Context::default();

    shown(&mut app, &ctx, vec![]);
    let texts = shown(&mut app, &ctx, vec![]);
    let group = at(&texts, "PRINCIPAL").unwrap_or_else(|| panic!("its space's group: {texts:?}"));

    assert!(in_list(&texts, "Zelador").is_some_and(|row| row.top() > group.top()));
    assert!(texts.iter().any(|(text, _)| text.ends_with("· Principal")), "the header says its space: {texts:?}");
    tap(&mut app, &ctx, at(&texts, "Ver no canvas").expect("the header's button").center());
    assert!(!app.chat_open, "the canvas");
    assert_eq!(app.spaces.current, "principal", "in the agent's space");
    assert_eq!(app.terminal_focus.as_deref(), Some(id.as_str()), "on its card");
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn the_chat_button_shows_who_waits_and_the_title_what_runs() {
    let ctx = Context::default();
    let (mut app, _answer) = busy_app();

    app.chat_open = false;
    shown(&mut app, &ctx, vec![]);
    let texts = shown(&mut app, &ctx, vec![]);
    let button = app.top_button(1);
    let badge = texts.iter().find(|(text, rect)| text == "1" && rect.center().distance(button.right_top()) < 14.0);

    assert!(badge.is_some(), "an amber 1 on the chat's button: {texts:?}");
    assert!(at(&texts, "Chats").is_none(), "no title over the canvas");
    app.chat_open = true;
    let texts = shown(&mut app, &ctx, vec![]);
    // macOS has no title bar: the title would sit under the chat's header, so only the buttons are there
    let titled = crate::app::ui::TITLE_BAR > 0.0;

    assert_eq!(at(&texts, "Chats").is_some() && at(&texts, " · 2 rodando · 1 esperando").is_some(), titled, "{texts:?}");
    // answered, the badge goes
    app.answer_person("revisor", true);
    let texts = shown(&mut app, &ctx, vec![]);

    assert!(!texts.iter().any(|(text, rect)| text == "1" && rect.center().distance(button.right_top()) < 14.0));
    assert_eq!(at(&texts, " · 1 rodando · 0 esperando").is_some(), titled);
}

#[test]
fn the_mcp_opens_the_area_on_a_chip_and_says_what_it_lists() {
    let (mut app, _answer) = busy_app();

    app.chat_open = false;
    let report = app.act("chats_view", &json!({ "filter": "waiting" })).unwrap();

    assert!(app.chat_open && app.chat_listing.show == Show::Waiting);
    assert!(report.starts_with("Chats · 2 rodando · 1 esperando (filtro: Esperando)"), "{report}");
    assert!(report.contains("Loja:\n- Revisor — Esperando — Permissão: cargo test") && !report.contains("Migrador"), "{report}");
    let report = app.act("chats_view", &json!({ "filter": "all", "query": "migra" })).unwrap();

    assert!(report.contains("Sem espaço:\n- Migrador — Pronto") && !report.contains("Revisor"), "{report}");
    assert!(app.act("chats_view", &json!({ "filter": "outro" })).is_err());
    app.act("chats_view", &json!({ "state": "close" })).unwrap();
    assert!(!app.chat_open);
}

/// A line of Claude Code's conversation file.
fn said(kind: &str, text: &str) -> String {
    let content = if kind == "user" { json!(text) } else { json!([{ "type": "text", "text": text }]) };

    format!("{}\n", json!({ "type": kind, "message": { "content": content } }))
}

/// A session outside, picked in the list, shows its conversation in the chat's own area (and in the island), read
/// from the file Claude Code keeps under the folder it names after the session's; one with no message yet says so.
#[test]
fn a_session_outside_picked_in_the_list_opens_its_conversation_here_and_in_the_island() {
    let dir = std::env::temp_dir().join(format!("mago-chats-fora-{}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    // where Claude Code files it: every UTF-16 unit not an ASCII letter or digit is a dash
    let cwd = r"C:\Users\eu\Área de Trabalho\loja_v2.0";

    assert_eq!(crate::transcript::project_folder(cwd), "C--Users-eu--rea-de-Trabalho-loja-v2-0");
    assert_eq!(crate::transcript::project_folder(r"C:\Users\edsu\.magoagent"), "C--Users-edsu--magoagent", "as on the owner's machine");
    assert_eq!(crate::transcript::project_folder("/home/eu/a😀b"), "-home-eu-a--b", "an emoji is two units");
    let file = crate::transcript::transcript_path(&dir, &crate::transcript::project_folder(cwd), "fora-lida");

    fs::create_dir_all(file.parent().unwrap()).unwrap();
    fs::write(&file, said("user", "arrume o login") + &said("assistant", "Vou olhar o formulário.")).unwrap();
    let ctx = Context::default();
    let mut app = DrawingApp::default();

    app.prefs.island = true;
    let hook = json!({ "hook_event_name": "UserPromptSubmit", "session_id": "fora-lida", "cwd": cwd, "transcript_path": file.display().to_string(), "prompt": "arrume o login" });

    app.take_outside_hook(&hook, None);
    // a session just started: its file comes with the first message
    let empty = dir.join("projects").join("-tmp-novo").join("fora-nova.jsonl");

    app.take_outside_hook(
        &json!({ "hook_event_name": "SessionStart", "session_id": "fora-nova", "cwd": "/tmp/novo", "transcript_path": empty.display().to_string() }),
        None,
    );

    let pill = app.outside_pills().into_iter().find(|pill| pill.session == "fora-lida").expect("its pill").id;

    app.chat_open = true;
    shown(&mut app, &ctx, vec![]);
    let texts = shown(&mut app, &ctx, vec![]);
    let row = in_list(&texts, "arrume o login").unwrap_or_else(|| panic!("its row: {texts:?}"));
    let texts = tap(&mut app, &ctx, row.center());

    assert_eq!(app.chat_agent.as_deref(), Some(pill.as_str()), "the chat shows it");
    assert_eq!(app.island.focus(), Some(pill.as_str()), "and so does the island");
    let main = |texts: &[(String, Rect)], words: &str| texts.iter().any(|(text, rect)| text == words && rect.left() > LIST + 16.0);

    assert!(main(&texts, "Vou olhar o formulário.") && main(&texts, "arrume o login"), "the conversation in the chat's area: {texts:?}");
    assert!(texts.iter().all(|(text, _)| !text.contains("não pode ser lida")), "{texts:?}");
    // it goes on: the new answer shows in a moment
    std::io::Write::write_all(&mut fs::OpenOptions::new().append(true).open(&file).unwrap(), said("assistant", "Pronto, corrigido.").as_bytes()).unwrap();
    app.now += 1.0;
    let texts = shown(&mut app, &ctx, vec![]);

    assert!(main(&texts, "Pronto, corrigido."), "{texts:?}");
    // the one with no message yet
    let row = in_list(&texts, "Sem atividade ainda").unwrap_or_else(|| panic!("its row: {texts:?}"));
    let texts = tap(&mut app, &ctx, row.center());

    assert!(main(&texts, "Ainda não há mensagens nesta sessão."), "{texts:?}");
    let _ = fs::remove_dir_all(dir);
}

/// Luna (a chat with its card) and the two agents it recruited in chat mode, roped to it: Ana works, Bia rests.
fn luna_and_team(app: &mut DrawingApp) -> String {
    let luna = push_chat(app, "Luna");

    app.add_chat_node(&luna, None);

    for name in ["Ana", "Bia"] {
        let recruit = push_chat(app, name);

        app.chats.last_mut().unwrap().recruit = true;
        app.add_chat_node(&recruit, None);
        app.rope_between(&luna, &recruit, false);
    }

    app.chat_runs.insert("ana".into(), crate::app::chat::direct::ChatRun::held(app.now));
    app.chat_open = true;
    app.chat_agent = Some(luna.clone());
    luna
}

/// A second of frames: what eases has eased.
fn settled(app: &mut DrawingApp, ctx: &Context) -> Vec<(String, Rect)> {
    (0..70).map(|_| shown(app, ctx, vec![])).last().unwrap()
}

#[test]
fn a_maestros_recruits_fold_under_it_in_conversas_and_a_chip_shows_the_ones_it_finds() {
    let ctx = Context::default();
    let mut app = DrawingApp::default();
    let luna = luna_and_team(&mut app);
    let texts = settled(&mut app, &ctx);
    let fold = in_list(&texts, "2 agentes · 1 trabalhando").unwrap_or_else(|| panic!("the fold under Luna: {texts:?}"));

    assert!(in_list(&texts, "Luna").is_some_and(|row| row.top() < fold.top()));
    assert!(in_list(&texts, "Ana").is_none() && in_list(&texts, "Bia").is_none(), "folded until opened: {texts:?}");
    tap(&mut app, &ctx, fold.center());
    assert!(app.team_open(&luna), "kept in the preferences");
    let texts = settled(&mut app, &ctx);
    let (ana, bia) = (in_list(&texts, "Ana").expect("opened"), in_list(&texts, "Bia").expect("opened"));

    assert!(fold.top() < ana.top() && ana.top() < bia.top() && ana.left() > in_list(&texts, "Luna").unwrap().left(), "under the fold, indented");
    tap(&mut app, &ctx, fold.center());
    assert!(!app.team_open(&luna));
    let texts = settled(&mut app, &ctx);

    assert!(in_list(&texts, "Ana").is_none(), "folded back");
    // a chip shows whom it keeps, with no fold in the way
    app.chat_listing.show = Show::Working;
    let texts = settled(&mut app, &ctx);

    assert!(in_list(&texts, "Ana").is_some() && in_list(&texts, "2 agentes").is_none(), "{texts:?}");
    // the MCP opens and folds them by Luna's name, and lists them under it
    app.chat_listing.show = Show::All;
    let said = app.act("chats_view", &json!({ "team": "Luna" })).unwrap();

    assert!(app.team_open(&luna) && said.starts_with("Agentes de Luna abertos (2 agentes · 1 trabalhando)."), "{said}");

    assert!(
        said.contains("\n- Luna") && said.contains("\n  agentes dele: 2 agentes · 1 trabalhando (abertos na lista)") && said.contains("\n  - Ana"),
        "{said}"
    );

    app.act("chats_view", &json!({ "team": "luna", "team_open": false })).unwrap();
    assert!(!app.team_open(&luna));
}

/// The conversation before /clear is a row of Conversas of its own, which opens it.
#[test]
fn the_conversation_before_clear_stays_in_conversas_and_opens() {
    let ctx = Context::default();
    let mut app = DrawingApp::default();
    let luna = push_chat(&mut app, "Arrumar o login");

    app.add_chat_node(&luna, None);
    // a Codex chat: what it said is the app's to keep (a Claude Code one's is read from its session file)
    (app.chats[0].agent, app.chats[0].session, app.chats[0].named) = ("codex".into(), Some("s1".into()), true);
    app.chat_transcript(&luna).items.push(crate::transcript::ChatItem::Agent("login arrumado".into()));
    app.chat_open = true;
    app.chat_agent = Some(luna.clone());
    app.chat_clear(&luna);
    let texts = settled(&mut app, &ctx);
    let earlier = in_list(&texts, "Arrumar o login").unwrap_or_else(|| panic!("the conversation before /clear: {texts:?}"));
    let cleared = in_list(&texts, "Codex").unwrap_or_else(|| panic!("the chat cleared, under its agent's name until its next message: {texts:?}"));
    // its row and the top of its conversation say it is a new session, since when
    let mark = crate::app::chat::direct::new_session_mark(now_ms());

    assert!(in_list(&texts, &mark).is_some_and(|line| line.top() > cleared.top()), "{mark}: {texts:?}");
    assert!(texts.iter().any(|(text, rect)| *text == mark && rect.left() > LIST + 16.0), "atop the conversation: {texts:?}");
    tap(&mut app, &ctx, earlier.center());
    assert_eq!(app.chat_agent.as_deref(), Some(app.chats[1].id.as_str()));
    let texts = settled(&mut app, &ctx);

    assert!(at(&texts, "login arrumado").is_some(), "what was said, there to read: {texts:?}");
}
