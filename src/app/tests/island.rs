//! The island: the pill with the summary, opening with the pointer or a question, the new chat in a workspace of its own.

use super::*;
use crate::app::chat::decide::PersonAsk;
use crate::app::island::{CLOSED_H, CLOSED_W, COMPACT_H, COMPACT_W, EXPANDED_W};

const SIZE: Vec2 = Vec2::new(1200.0, 800.0);

fn chat(id: &str, name: &str) -> crate::app::chat::direct::Chat {
    crate::app::chat::direct::Chat {
        id: id.into(),
        agent: "claude".into(),
        name: name.into(),
        cwd: "C:/p".into(),
        session: None,
        mode: 0,
        model: String::new(),
        effort: String::new(),
        recruit: false,
        named: false,
    }
}

fn app_with_island() -> DrawingApp {
    let mut app = DrawingApp::default();
    app.prefs.island = true;
    app.chats = vec![chat("a", "Claude Code · loja"), chat("b", "Codex · site")];
    app
}

fn frame_at(app: &mut DrawingApp, ctx: &Context, time: f64, events: Vec<egui::Event>) {
    let raw = egui::RawInput { screen_rect: Some(Rect::from_min_size(Pos2::ZERO, SIZE)), time: Some(time), events, ..Default::default() };
    let mut output = ctx.run_ui(raw, |_| app.island_ui(ctx));
    output.textures_delta.clear();
}

/// Frames from `from` to `to` seconds, 50 ms apart.
fn run(app: &mut DrawingApp, ctx: &Context, from: f64, to: f64, events: Vec<egui::Event>) {
    let mut time = from;
    while time < to {
        frame_at(app, ctx, time, events.clone());
        time += 0.05;
    }
}

#[test]
fn the_island_is_a_notch_that_opens_with_the_pointer_and_folds_when_it_leaves() {
    let ctx = Context::default();
    let mut app = app_with_island();
    run(&mut app, &ctx, 1.0, 1.3, vec![]);
    assert_eq!(app.island.size(), Vec2::new(CLOSED_W, CLOSED_H), "closed: the small notch");
    // it hangs from the top centre: the pointer on it shows compact, a click opens the panel
    let on_it = Pos2::new(SIZE.x / 2.0, CLOSED_H / 2.0);
    run(&mut app, &ctx, 2.0, 2.5, vec![egui::Event::PointerMoved(on_it)]);
    assert!((app.island.size().x - COMPACT_W).abs() < 4.0, "compact on hover: {}", app.island.size().x);
    let press = |pressed| egui::Event::PointerButton { pos: on_it, button: egui::PointerButton::Primary, pressed, modifiers: Default::default() };
    frame_at(&mut app, &ctx, 2.6, vec![press(true)]);
    frame_at(&mut app, &ctx, 2.65, vec![press(false)]);
    run(&mut app, &ctx, 2.7, 4.0, vec![egui::Event::PointerMoved(on_it)]);
    assert_eq!(app.island.size().x, EXPANDED_W, "open");
    assert_eq!(app.island.size().y, 160.0);
    // the pointer left: it stays 15 s, then folds to compact
    frame_at(&mut app, &ctx, 4.05, vec![egui::Event::PointerGone]);
    run(&mut app, &ctx, 4.1, 18.0, vec![]);
    assert!(app.island.size().x > COMPACT_W + 50.0, "still open under 15 s");
    run(&mut app, &ctx, 20.0, 22.0, vec![]);
    assert!((app.island.size().x - COMPACT_W).abs() < 4.0, "folded to compact after 15 s: {}", app.island.size().x);
    run(&mut app, &ctx, 80.0, 82.0, vec![]);
    assert_eq!(app.island.size(), Vec2::new(CLOSED_W, CLOSED_H), "60 s after folding to compact, it returns to the notch");
    // off: nothing is kept
    app.prefs.island = false;
    frame_at(&mut app, &ctx, 7.0, vec![]);
    assert_eq!(app.island.size(), Vec2::ZERO);
}

#[test]
fn opened_it_folds_after_the_cursor_left_even_though_egui_never_heard_it_leave() {
    // Windows: outside the island the window lets the cursor through, so egui keeps the last pointer on it
    let ctx = Context::default();
    let mut app = app_with_island();
    let on_it = Pos2::new(SIZE.x / 2.0, CLOSED_H / 2.0);
    app.island.over = Some(true);
    run(&mut app, &ctx, 1.0, 1.5, vec![egui::Event::PointerMoved(on_it)]);
    let press = |pressed| egui::Event::PointerButton { pos: on_it, button: egui::PointerButton::Primary, pressed, modifiers: Default::default() };
    frame_at(&mut app, &ctx, 1.6, vec![press(true)]);
    frame_at(&mut app, &ctx, 1.65, vec![press(false)]);
    run(&mut app, &ctx, 1.7, 3.0, vec![]);
    assert_eq!(app.island.size().x, EXPANDED_W, "open");
    // the system's cursor went elsewhere, with no PointerGone: 15 s later it folds
    app.island.over = Some(false);
    run(&mut app, &ctx, 3.0, 17.0, vec![]);
    assert_eq!(app.island.size().x, EXPANDED_W, "still open under 15 s");
    run(&mut app, &ctx, 17.0, 19.5, vec![]);
    assert!((app.island.size().x - COMPACT_W).abs() < 4.0, "folded to compact: {}", app.island.size().x);
}

#[test]
fn closed_the_wizard_looks_around_by_itself_and_awake_its_eyes_are_on_the_pointer() {
    use crate::app::mascot::mago::mago_gaze;
    let ctx = Context::default();
    let mut app = app_with_island();
    // closed, the pointer far to the right of it: not followed
    let far = Pos2::new(SIZE.x - 10.0, 400.0);
    run(&mut app, &ctx, 1.0, 2.0, vec![egui::Event::PointerMoved(far)]);
    assert_eq!(app.island.size(), Vec2::new(CLOSED_W, CLOSED_H));
    let alone = mago_gaze("island-main").unwrap();
    assert!(alone.x.abs() <= 0.5 && alone.y.abs() <= 0.2, "its own slow wander: {alone:?}");
    // the pointer comes to it and goes away again: awake (compact), the eyes go after the pointer
    let on_it = Pos2::new(SIZE.x / 2.0, CLOSED_H / 2.0);
    run(&mut app, &ctx, 2.0, 2.2, vec![egui::Event::PointerMoved(on_it)]);
    run(&mut app, &ctx, 2.2, 4.0, vec![egui::Event::PointerMoved(far)]);
    assert_eq!(app.island.size(), Vec2::new(COMPACT_W, COMPACT_H));
    let following = mago_gaze("island-main").unwrap();
    assert!(following.x > 0.9, "on the pointer, far to the right: {following:?}");
}

#[test]
fn a_question_takes_the_island_over() {
    let ctx = Context::default();
    let mut app = app_with_island();
    run(&mut app, &ctx, 1.0, 1.3, vec![]);
    assert_eq!(app.island.size().x, CLOSED_W);
    let (reply, _answers) = std::sync::mpsc::channel();
    app.person_asks.get_mut().push(PersonAsk::new("a".into(), &json!({ "tool_name": "Bash", "input": { "command": "rm -rf x" } }), reply));
    frame_at(&mut app, &ctx, 2.0, vec![]);
    assert!(app.island.size().x > CLOSED_W, "it grows with no pointer on it");
}

/// An app on a drawing of its own in a fresh folder, with an agent installed.
fn app_in(name: &str) -> (DrawingApp, PathBuf, Vec<Preset>) {
    let dir = std::env::temp_dir().join(format!("mago-ilha-{name}-{}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    // today's drawing already there: with none, it is a first run and there is no space yet
    fs::write(dir.join("autosave.mago"), r#"{"version":1,"elements":[]}"#).unwrap();
    let mut app = DrawingApp { autosave: Some(dir.join("autosave.mago")), ..Default::default() };
    app.load_spaces();
    app.load_autosave();
    (app, dir, vec![Preset::new("Claude Code", "claude")])
}

#[test]
fn a_new_chat_gets_a_workspace_of_its_own_and_the_one_in_progress_stays() {
    use crate::app::island::NewForm;
    let (mut app, dir, installed) = app_in("novo");
    // the work in progress in the main space
    let element = app.new_element(ElementKind::Rectangle { x: 10.0, y: 10.0, width: 50.0, height: 50.0 });
    app.snapshot();
    app.scene.elements.push(element);
    let project = dir.join("loja");
    fs::create_dir_all(&project).unwrap();
    let form = NewForm { folder: project.to_string_lossy().into_owned(), ..Default::default() };
    let message = app.island_new_chat(&form, &installed).unwrap();
    assert!(message.contains("loja"), "{message}");
    assert_eq!(app.current_space().map(|space| space.name.as_str()), Some("loja"), "the new space is the one shown");
    assert_eq!(app.chats.len(), 1);
    assert_eq!(app.chats[0].cwd, project.to_string_lossy());
    assert_eq!(app.chat_agent.as_deref(), Some(app.chats[0].id.as_str()));
    assert!(!app.scene.elements.is_empty() && app.scene.elements.iter().all(|e| e.chat.is_some()), "only its card is on the new drawing");
    // the other space keeps its drawing
    app.switch_space("principal").unwrap();
    assert!(app.scene.elements.iter().any(|e| e.chat.is_none()), "the work in progress is there");
    // the same folder again: another space, not an error
    app.island_new_chat(&form, &installed).unwrap();
    assert_eq!(app.current_space().map(|space| space.name.as_str()), Some("loja 2"));
    // a folder that is not there, and no agent
    let gone = NewForm { folder: dir.join("nada").to_string_lossy().into_owned(), ..form.clone() };
    assert!(app.island_new_chat(&gone, &installed).unwrap_err().contains("não existe"));
    assert!(app.island_new_chat(&form, &[]).unwrap_err().contains("Nenhum agente"));
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn a_new_chat_can_run_in_a_worktree_of_its_own() {
    use crate::app::island::NewForm;
    let (mut app, dir, installed) = app_in("worktree");
    let repo = dir.join("repo");
    fs::create_dir_all(&repo).unwrap();
    let git = |args: &[&str]| crate::editor::run_git(&repo, args);
    // no git on this machine: nothing to check here
    if git(&["init"]).is_err() {
        return;
    }
    fs::write(repo.join("a.txt"), "a").unwrap();
    git(&["add", "-A"]).unwrap();
    git(&["-c", "user.name=t", "-c", "user.email=t@t", "commit", "-m", "base"]).unwrap();
    // changes in progress in the repository
    fs::write(repo.join("a.txt"), "mudou").unwrap();
    let form = NewForm { name: "Revisão".into(), folder: repo.to_string_lossy().into_owned(), worktree: true, ..Default::default() };
    let message = app.island_new_chat(&form, &installed).unwrap();
    assert!(message.contains("worktree"), "{message}");
    let cwd = PathBuf::from(&app.chats[0].cwd);
    // the Windows runner gives TEMP as a short 8.3 path and git answers with the long one: compare the end
    assert!(cwd.parent().is_some_and(|parent| parent.ends_with(PathBuf::from(".magoagent").join("worktrees"))), "{}", cwd.display());
    assert_eq!(fs::read_to_string(cwd.join("a.txt")).unwrap(), "a", "the copy is of the commit");
    assert_eq!(fs::read_to_string(repo.join("a.txt")).unwrap(), "mudou", "the repository's changes stay");
    assert_eq!(app.current_space().map(|space| space.name.as_str()), Some("Revisão"));
    // not a repository: refused, and no space made
    let plain = dir.join("plain");
    fs::create_dir_all(&plain).unwrap();
    let spaces = app.spaces.list.len();
    let refused = NewForm { folder: plain.to_string_lossy().into_owned(), ..form };
    assert!(app.island_new_chat(&refused, &installed).unwrap_err().contains("repositório"));
    assert_eq!(app.spaces.list.len(), spaces);
    let _ = git(&["worktree", "remove", "--force", &cwd.to_string_lossy()]);
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn the_island_is_on_by_default_and_its_place_is_kept_in_the_prefs() {
    let prefs = Prefs::default();
    assert!(prefs.island && prefs.island_at.is_none());
    let mut saved = prefs.clone();
    saved.island = false;
    let back: Prefs = serde_json::from_str(&serde_json::to_string(&saved).unwrap()).unwrap();
    assert!(!back.island, "turned off, it stays off");
    saved.island = true;
    saved.island_at = Some([2880.0, 0.0]);
    let back: Prefs = serde_json::from_str(&serde_json::to_string(&saved).unwrap()).unwrap();
    assert!(back.island && back.island_at == Some([2880.0, 0.0]));
    // prefs from before the island still load, with it on
    let old: Prefs = serde_json::from_str("{}").unwrap();
    assert!(old.island);
}

#[test]
fn the_island_waits_for_the_welcome_to_be_done() {
    let ctx = Context::default();
    let mut app = app_with_island();
    app.welcome = Some(Default::default());
    run(&mut app, &ctx, 1.0, 2.0, vec![]);
    assert_eq!(app.island.size(), Vec2::ZERO, "no island over the welcome");
    // a session outside asking meanwhile: back to its terminal, no card waiting behind the welcome
    let (reply, answer) = std::sync::mpsc::channel();
    let payload =
        json!({ "hook_event_name": "PermissionRequest", "session_id": "s1", "cwd": "/tmp/loja", "tool_name": "Bash", "tool_input": { "command": "ls" } });
    app.take_outside_hook(&payload, Some(reply));
    assert!(app.person_asks.borrow().is_empty());
    assert!(answer.try_recv().is_ok(), "the hook was answered at once: the terminal asks");
    // through the welcome, the island comes (compact: the session outside is at work)
    app.welcome = None;
    run(&mut app, &ctx, 2.0, 3.0, vec![]);
    assert!((app.island.size().x - COMPACT_W).abs() < 4.0, "{:?}", app.island.size());
}

#[test]
fn an_agent_opened_from_the_island_shows_in_its_conversation_or_in_the_big_chat() {
    let mut app = app_with_island();
    app.island_show("b", 5.0);
    assert!(app.island.tab == crate::app::island::Tab::Chat && app.chat_agent.as_deref() == Some("b"));
    assert!(!app.raise_window, "the island has it: the app stays where it is");
    // with the chat screen open the agent opens there and the app's window comes up
    let mut app = app_with_island();
    app.chat_open = true;
    app.island_show("a", 5.0);
    assert!(app.raise_window && app.chat_agent.as_deref() == Some("a"));
}
