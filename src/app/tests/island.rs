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
    // it hangs from the top centre: the pointer resting on it opens the panel
    let on_it = Pos2::new(SIZE.x / 2.0, CLOSED_H / 2.0);
    run(&mut app, &ctx, 2.0, 3.5, vec![egui::Event::PointerMoved(on_it)]);
    assert_eq!(app.island.size().x, EXPANDED_W, "open");
    assert_eq!(app.island.size().y, 160.0);
    // the pointer left: it stays a moment, then folds
    frame_at(&mut app, &ctx, 3.55, vec![egui::Event::PointerGone]);
    run(&mut app, &ctx, 3.6, 3.8, vec![]);
    assert!(app.island.size().x > COMPACT_W + 50.0, "a moment after");
    run(&mut app, &ctx, 4.5, 6.5, vec![]);
    assert_eq!(app.island.size(), Vec2::new(COMPACT_W, COMPACT_H), "folded to the compact island, which waits a minute");
    run(&mut app, &ctx, 70.0, 70.5, vec![]);
    assert_eq!(app.island.size(), Vec2::new(CLOSED_W, CLOSED_H), "and then to the notch");
    // off: nothing is kept
    app.prefs.island = false;
    frame_at(&mut app, &ctx, 7.0, vec![]);
    assert_eq!(app.island.size(), Vec2::ZERO);
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
    let form = NewForm { name: String::new(), folder: project.to_string_lossy().into_owned(), worktree: false };
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
    let form = NewForm { name: "Revisão".into(), folder: repo.to_string_lossy().into_owned(), worktree: true };
    let message = app.island_new_chat(&form, &installed).unwrap();
    assert!(message.contains("worktree"), "{message}");
    let cwd = PathBuf::from(&app.chats[0].cwd);
    assert!(cwd.starts_with(repo.join(".magoagent").join("worktrees")), "{}", cwd.display());
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
fn the_island_is_off_by_default_and_its_place_is_kept_in_the_prefs() {
    let prefs = Prefs::default();
    assert!(!prefs.island && prefs.island_at.is_none());
    let mut saved = prefs.clone();
    saved.island = true;
    saved.island_at = Some([2880.0, 0.0]);
    let back: Prefs = serde_json::from_str(&serde_json::to_string(&saved).unwrap()).unwrap();
    assert!(back.island && back.island_at == Some([2880.0, 0.0]));
    // prefs from before the island still load
    let old: Prefs = serde_json::from_str("{}").unwrap();
    assert!(!old.island);
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
