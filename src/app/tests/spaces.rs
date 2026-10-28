//! Workspaces: a drawing each, beside the main one, switched in place.

use super::*;

/// A folder of its own per test, holding the main drawing; removed at the end.
struct Home(PathBuf);

impl Home {
    fn new(name: &str) -> Self {
        let dir = std::env::temp_dir().join(format!("magoagent-spaces-{name}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        Home(dir)
    }

    fn main(&self) -> PathBuf {
        self.0.join("autosave.mago")
    }

    /// The app as it starts: the list, then the active space's drawing.
    fn app(&self) -> DrawingApp {
        let mut app = DrawingApp { autosave: Some(self.main()), ..Default::default() };
        app.load_spaces();
        app.load_autosave();
        app
    }
}

impl Drop for Home {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn add_box(app: &mut DrawingApp, x: f32) {
    let element = app.new_element(ElementKind::Rectangle { x, y: 0.0, width: 100.0, height: 50.0 });
    app.snapshot();
    app.scene.elements.push(element);
}

fn left(app: &DrawingApp) -> f32 {
    element_bounds(&app.scene.elements[0]).min.x
}

fn terminal(app: &DrawingApp, name: &str) -> Element {
    let mut element = app.new_element(ElementKind::Rectangle { x: 0.0, y: 0.0, width: 400.0, height: 300.0 });
    element.terminal = Some(TerminalSpec { name: name.into(), ..Default::default() });
    element
}

#[test]
fn with_one_space_nothing_new_is_written() {
    let home = Home::new("one");
    let mut app = home.app();
    assert_eq!(app.spaces, Spaces::default());
    add_box(&mut app, 0.0);
    app.save_on_exit();
    app.keep_spaces();
    assert!(home.main().exists());
    assert!(!home.0.join("workspaces.json").exists(), "a single space writes no list");
}

#[test]
fn a_space_keeps_its_drawing_undo_and_camera_when_left() {
    let home = Home::new("switch");
    let mut app = home.app();
    add_box(&mut app, 0.0);
    (app.pan, app.zoom) = (Vec2::new(30.0, -40.0), 1.5);
    app.selected = vec![0];
    app.create_space("Outro", Some([0x3d, 0x7e, 0xf0]), "");
    let other = app.spaces.active.clone();
    assert_ne!(other, MAIN_SPACE);
    assert!(app.scene.elements.is_empty() && app.undo.is_empty() && app.selected.is_empty());
    assert_eq!((app.pan, app.zoom), (Vec2::ZERO, 1.0));
    add_box(&mut app, 500.0);
    app.switch_space(MAIN_SPACE);
    assert_eq!(app.scene.elements.len(), 1);
    assert_eq!(left(&app), 0.0);
    assert_eq!(app.undo.len(), 1, "undo is per space");
    assert_eq!((app.pan, app.zoom), (Vec2::new(30.0, -40.0), 1.5));
    app.switch_space(&other);
    assert_eq!(left(&app), 500.0);
    // each drawing in its own file, and the list says which one was open
    let read = |path: PathBuf| read_scene_file(&path).unwrap().0.elements.len();
    assert_eq!((read(home.main()), read(space_scene(&home.main(), &other))), (1, 1));
    assert_eq!(read_spaces(&home.main()).0.active, other);
    // a restart opens the space left open, with its camera
    drop(app);
    let app = home.app();
    assert_eq!((app.spaces.active.as_str(), left(&app)), (other.as_str(), 500.0));
    assert_eq!(app.spaces.spaces[1].color, Some([0x3d, 0x7e, 0xf0]));
}

#[test]
fn a_hidden_space_that_changed_is_written_on_close_with_what_its_agent_drew() {
    let home = Home::new("hidden");
    let mut app = home.app();
    add_box(&mut app, 0.0);
    let id = app.scene.elements[0].id.clone();
    app.create_space("Outro", None, "");
    // hidden, its box changes (a page of it navigating does this) and its agent draws in its file
    let (element, dirty) = app.element_anywhere_mut(&id).unwrap();
    (element.locked, *dirty) = (true, true);
    let (mut outside, _) = read_scene_file(&home.main()).unwrap();
    let mut drawn = app.new_element(ElementKind::Rectangle { x: 300.0, y: 0.0, width: 50.0, height: 50.0 });
    drawn.id = "do-agente".into();
    outside.elements.push(drawn);
    std::thread::sleep(std::time::Duration::from_millis(20));
    write_scene_file(&home.main(), &outside).unwrap();
    eframe::App::on_exit(&mut app, None);
    let (saved, _) = read_scene_file(&home.main()).unwrap();
    assert!(saved.elements.iter().any(|e| e.id == id && e.locked), "the change made while hidden");
    assert!(saved.elements.iter().any(|e| e.id == "do-agente"), "and what the agent drew");
}

fn portal(app: &DrawingApp, url: &str) -> Element {
    let mut element = app.new_element(ElementKind::Rectangle { x: 500.0, y: 0.0, width: 400.0, height: 300.0 });
    element.portal = Some(PortalSpec { name: "Docs".into(), url: url.into(), ..Default::default() });
    element
}

#[test]
fn an_unloaded_space_is_written_let_go_and_read_back_when_shown() {
    let home = Home::new("unload");
    let mut app = home.app();
    app.create_space("Outro", None, "");
    let other = app.spaces.active.clone();
    let page = portal(&app, "https://a.dev");
    let id = page.id.clone();
    app.scene.elements.push(page);
    app.dirty = true;
    app.switch_space(MAIN_SPACE);
    // hidden, its page navigates
    let (element, dirty) = app.element_anywhere_mut(&id).unwrap();
    (element.portal.as_mut().unwrap().url, *dirty) = ("https://b.dev".into(), true);
    app.unload_space(&other, false);
    assert!(!app.parked.contains_key(&other) && !app.all_elements().any(|e| e.id == id), "let go");
    let (saved, _) = read_scene_file(&space_scene(&home.main(), &other)).unwrap();
    assert_eq!(saved.elements[0].portal.as_ref().unwrap().url, "https://b.dev");
    app.switch_space(&other);
    assert!(app.scene.elements.iter().any(|e| e.id == id), "showing it reads it back");
}

#[test]
fn a_space_kept_unloaded_starts_nothing_until_reloaded() {
    let home = Home::new("keep");
    let mut app = home.app();
    app.create_space("Outro", None, "");
    let other = app.spaces.active.clone();
    let (shell, page) = (terminal(&app, "Shell"), portal(&app, "about:blank"));
    app.scene.elements.extend([shell, page]);
    app.dirty = true;
    app.switch_space(MAIN_SPACE);
    // hidden, its agent draws a terminal of its own in its file
    let file = space_scene(&home.main(), &other);
    let (mut outside, _) = read_scene_file(&file).unwrap();
    outside.elements.push(terminal(&app, "Do agente"));
    std::thread::sleep(std::time::Duration::from_millis(20));
    write_scene_file(&file, &outside).unwrap();
    app.unload_space(&other, true);
    assert!(!app.parked.contains_key(&other));
    let (saved, _) = read_scene_file(&file).unwrap();
    assert!(saved.elements.len() == 3 && saved.elements.iter().all(|e| e.unloaded()), "written unloaded, the agent's too");
    app.switch_space(&other);
    app.sync_terminals(&Context::default());
    assert!(app.terminals.is_empty(), "showing it starts no shell");
    app.switch_space(MAIN_SPACE);
    app.reload_space(&other);
    assert_eq!(app.spaces.active, other);
    assert!(app.scene.elements.iter().all(|e| !e.unloaded()));
    assert_eq!(app.undo.len(), 1, "one step");
}

#[test]
fn a_terminal_keeps_its_session_across_a_space_switch() {
    let home = Home::new("session");
    let mut app = home.app();
    let t = terminal(&app, "Shell");
    let id = t.id.clone();
    app.scene.elements.push(t);
    // a shell that couldn't start stands in for one: the sync starts none over it
    app.terminals.insert(id.clone(), Err("parado".into()));
    let ctx = Context::default();
    app.create_space("Outro", None, "");
    app.sync_terminals(&ctx);
    assert!(app.terminals.contains_key(&id), "hidden, it goes on");
    app.switch_space(MAIN_SPACE);
    app.sync_terminals(&ctx);
    assert!(app.terminals.contains_key(&id), "and is there when it shows again");
}

#[test]
fn a_prompt_being_written_waits_for_its_terminal_across_a_switch() {
    let home = Home::new("draft");
    let mut app = home.app();
    let t = terminal(&app, "Revisor");
    let id = t.id.clone();
    app.scene.elements.push(t);
    app.composer = Some((id.clone(), "revise o login".into()));
    app.create_space("Outro", None, "");
    assert_eq!(app.composer, None, "it closes with its workspace");
    app.switch_space(MAIN_SPACE);
    app.toggle_composer(id.clone());
    assert_eq!(app.composer, Some((id.clone(), "revise o login".into())), "and opens again with the text");
    app.toggle_composer(id);
    assert_eq!(app.composer, None);
}

#[test]
fn a_list_that_lost_its_main_or_active_space_still_opens_the_main_one() {
    let home = Home::new("broken");
    let space = |id: &str| Space { id: id.into(), name: id.into(), ..Default::default() };
    let list = Spaces { version: 1, active: "gone".into(), spaces: vec![space("a"), space("../fora")] };
    write_spaces(&home.main(), &list).unwrap();
    let (spaces, bad) = read_spaces(&home.main());
    assert_eq!((spaces.active.as_str(), bad), (MAIN_SPACE, None));
    assert_eq!(spaces.spaces.iter().map(|s| s.id.as_str()).collect::<Vec<_>>(), [MAIN_SPACE, "a"]);
    // one it can't read goes aside, so the next write doesn't lose it
    fs::write(home.0.join("workspaces.json"), "{ not json").unwrap();
    let (spaces, bad) = read_spaces(&home.main());
    assert_eq!(spaces, Spaces::default());
    assert_eq!(fs::read_to_string(bad.unwrap()).unwrap(), "{ not json");
    assert!(!home.0.join("workspaces.json").exists());
}

#[test]
fn a_delete_or_an_edit_of_the_main_space_alone_is_written() {
    let home = Home::new("written");
    let mut app = home.app();
    app.create_space("Outro", None, "");
    let other = app.spaces.active.clone();
    app.delete_space(&other);
    app.keep_spaces();
    drop(app);
    let mut app = home.app();
    assert_eq!(app.spaces.spaces.len(), 1, "a deleted space doesn't come back");
    app.spaces.spaces[0].root = "C:/proj".into();
    app.keep_spaces();
    drop(app);
    assert_eq!(home.app().spaces.spaces[0].root, "C:/proj");
}

#[test]
fn ctrl_s_saves_each_space_to_its_own_file() {
    let home = Home::new("file");
    let mut app = home.app();
    let file = home.0.join("cliente.mago");
    add_box(&mut app, 0.0);
    app.save_to(file.clone());
    // no file there: Ctrl+S asks where (save() would open the dialog), it doesn't write here
    app.create_space("Outro", None, "");
    assert_eq!(app.file_path, None, "the other space has no file yet");
    app.switch_space(MAIN_SPACE);
    assert_eq!(app.file_path.as_ref(), Some(&file));
}

#[test]
fn a_switch_mid_drag_lets_go_of_the_drawing_it_held() {
    let home = Home::new("drag");
    let mut app = home.app();
    add_box(&mut app, 0.0);
    app.selected = vec![0];
    (app.moving, app.erasing) = (true, Some(vec![0]));
    app.create_space("Outro", None, "");
    assert!(!app.in_gesture());
    // a drag that changed nothing yet took no undo step: the last one stays
    app.switch_space(MAIN_SPACE);
    assert_eq!(app.undo.len(), 1);
}

#[test]
fn a_floor_made_after_its_dialog_was_cancelled_is_listed_but_not_shown() {
    let home = Home::new("cancel");
    let mut app = home.app();
    let floor = Space { id: "f".into(), name: "Tela".into(), parent: Some(MAIN_SPACE.into()), ..Default::default() };
    // the person already opened another dialog meanwhile
    let open = crate::app::spaces::SpaceDialog::Edit { id: None, name: "Novo".into(), color: None, root: String::new() };
    app.space_dialog = Some(open);
    app.space_events.0.send(floors::SpaceJob::Floor(Ok(floor.clone()))).unwrap();
    app.space_jobs(&egui::Context::default());
    assert_eq!(app.spaces.active, MAIN_SPACE);
    assert!(app.spaces.spaces.iter().any(|s| s.id == "f"));
    assert!(matches!(app.space_dialog, Some(crate::app::spaces::SpaceDialog::Edit { .. })));
    // still waiting for it: it shows
    let floor = Space { id: "g".into(), ..floor };
    app.space_dialog = Some(crate::app::spaces::SpaceDialog::Floor { parent: MAIN_SPACE.into(), name: "Tela ".into(), isolate: true, branch: String::new(), busy: true, error: None });
    app.space_events.0.send(floors::SpaceJob::Floor(Ok(floor))).unwrap();
    app.space_jobs(&egui::Context::default());
    assert_eq!((app.spaces.active.as_str(), app.space_dialog.is_none()), ("g", true));
}

#[test]
fn a_workspace_drawing_opened_on_its_own_reads_no_registry() {
    let home = Home::new("own");
    let mut app = home.app();
    app.create_space("Outro", None, "");
    let other = space_scene(&home.main(), &app.spaces.active);
    drop(app);
    let mut app = DrawingApp { autosave: Some(other), ..Default::default() };
    app.load_spaces();
    assert_eq!((app.spaces.spaces.len(), app.spaces_home.is_none()), (1, true));
}

#[test]
fn each_terminal_draws_in_its_own_space_and_agents_still_see_it() {
    let home = Home::new("terminals");
    let mut app = home.app();
    let first = terminal(&app, "Primeiro");
    app.scene.elements.push(first.clone());
    app.create_space("Outro", None, "");
    let other = app.spaces.active.clone();
    let second = terminal(&app, "Segundo");
    app.scene.elements.push(second.clone());
    assert_eq!(app.scene_of(&first.id), Some(home.main()));
    assert_eq!(app.scene_of(&second.id), Some(space_scene(&home.main(), &other)));
    assert!(app.hidden(&first.id) && !app.hidden(&second.id));
    let scene = |id: &str| app.terminal_env(id, "x").into_iter().find(|(k, _)| k == "MAGOAGENT_SCENE").map(|(_, v)| PathBuf::from(v));
    assert_eq!(scene(&first.id), Some(home.main()));
    let names: Vec<String> = app.agent_nodes().into_iter().map(|n| n.name).collect();
    assert!(names.contains(&"Primeiro".to_string()) && names.contains(&"Segundo".to_string()));
    // what an agent asks of the hidden one is refused, not done in the wrong drawing
    let (reply, answer) = std::sync::mpsc::channel();
    app.apply_agent_command(UiCommand::Connect { a: second.id.clone(), b: first.id.clone(), reply });
    assert!(answer.recv().unwrap().unwrap_err().contains("outro espaço de trabalho"));
    assert!(!app.scene.elements.iter().any(|e| e.rope));
    // the ones with no answer channel: the bridge refuses them instead of saying "done"
    app.switch_space(MAIN_SPACE);
    let note = app.new_note(Rect::from_min_size(Pos2::ZERO, Vec2::splat(200.0)), "Plano".into());
    let note_id = note[0].id.clone();
    app.switch_space(&other);
    app.parked.get_mut(MAIN_SPACE).unwrap().scene.elements.extend(note);
    let shared = std::sync::Mutex::new(Shared { nodes: app.agent_nodes(), ..Default::default() });
    let (sender, commands) = std::sync::mpsc::channel();
    let refused = handle(&json!({ "cmd": "note_write", "name": "Plano", "text": "novo" }), &shared, &sender).unwrap_err();
    assert!(refused.contains("outro espaço de trabalho"), "{refused}");
    assert!(commands.try_recv().is_err() && app.hidden(&note_id));
}

#[test]
fn new_terminals_start_in_the_space_folder() {
    let mut app = DrawingApp::default();
    app.spaces.spaces.push(Space { id: "p".into(), name: "Projeto".into(), root: "C:/projeto".into(), ..Default::default() });
    assert_eq!(app.new_folder(), "");
    app.switch_space("p");
    assert_eq!(app.new_folder(), "C:/projeto");
    app.open_terminal_dialog(Pos2::ZERO, Pos2::ZERO);
    assert_eq!(app.terminal_dialog.unwrap().spec.cwd, "C:/projeto");
}

#[test]
fn ctrl_arrows_go_round_the_spaces_and_the_main_one_is_never_deleted() {
    let ctx = Context::default();
    let mut app = DrawingApp::default();
    for id in ["a", "b"] {
        app.spaces.spaces.push(Space { id: id.into(), name: id.into(), ..Default::default() });
    }
    let key = |key| egui::Event::Key { key, physical_key: None, pressed: true, repeat: false, modifiers: Modifiers::COMMAND };
    frame(&mut app, &ctx, vec![key(Key::ArrowDown)]);
    assert_eq!(app.spaces.active, "a");
    frame(&mut app, &ctx, vec![key(Key::ArrowUp)]);
    frame(&mut app, &ctx, vec![key(Key::ArrowUp)]);
    assert_eq!(app.spaces.active, "b");
    app.delete_space(MAIN_SPACE);
    app.delete_space("b");
    assert_eq!(app.spaces.active, MAIN_SPACE);
    assert_eq!(app.spaces.spaces.iter().map(|s| s.id.as_str()).collect::<Vec<_>>(), [MAIN_SPACE, "a"]);
    assert!(!app.parked.contains_key("b"));
}

#[test]
fn the_new_space_dialog_creates_only_with_a_name() {
    let ctx = Context::default();
    let mut app = DrawingApp::default();
    let enter = egui::Event::Key { key: Key::Enter, physical_key: None, pressed: true, repeat: false, modifiers: Modifiers::NONE };
    app.space_dialog = Some(crate::app::spaces::SpaceDialog::Edit { id: None, name: "  ".into(), color: None, root: String::new() });
    frame(&mut app, &ctx, vec![enter.clone()]);
    assert!(app.space_dialog.is_some() && app.spaces.spaces.len() == 1);
    app.space_dialog = Some(crate::app::spaces::SpaceDialog::Edit { id: None, name: "Cliente".into(), color: None, root: String::new() });
    frame(&mut app, &ctx, vec![enter]);
    assert!(app.space_dialog.is_none());
    assert_eq!(app.spaces.spaces.iter().find(|s| s.id == app.spaces.active).map(|s| s.name.as_str()), Some("Cliente"));
}

/// A repository with one commit, as a person's project would be.
fn repo(dir: &std::path::Path, commit: bool) {
    let run = |args: &[&str]| assert!(crate::ide::command("git").args(args).current_dir(dir).output().unwrap().status.success(), "git {args:?}");
    fs::create_dir_all(dir).unwrap();
    run(&["init", "-q", "-b", "main"]);
    if commit {
        fs::write(dir.join("a.txt"), "a").unwrap();
        run(&["add", "."]);
        run(&["-c", "user.name=t", "-c", "user.email=t@t", "-c", "commit.gpgsign=false", "commit", "-q", "-m", "um"]);
    }
}

fn branches(dir: &std::path::Path) -> String {
    crate::ide::git(dir, &["branch", "--list"]).unwrap()
}

/// Until a git thread answered (a floor came, or a toast said why not), five seconds at most.
fn settle(app: &mut DrawingApp) {
    let before = app.spaces.spaces.len();
    for _ in 0..50 {
        app.space_jobs(&egui::Context::default());
        if app.spaces.spaces.len() != before || !app.status.is_empty() {
            return;
        }
        std::thread::sleep(std::time::Duration::from_millis(100));
    }
}

#[test]
fn a_floor_without_isolation_is_a_canvas_under_its_workspace() {
    let mut app = DrawingApp::default();
    for (id, root) in [("p", "C:/projeto"), ("q", "")] {
        app.spaces.spaces.push(Space { id: id.into(), name: id.into(), root: root.into(), ..Default::default() });
    }
    app.create_floor("p", "Rascunho", false, "").unwrap();
    app.create_floor("p", "Outro", false, "").unwrap();
    let order: Vec<(&str, Option<&str>)> = app.spaces.spaces.iter().map(|s| (s.name.as_str(), s.parent.as_deref())).collect();
    assert_eq!(order, [("Principal", None), ("p", None), ("Rascunho", Some("p")), ("Outro", Some("p")), ("q", None)]);
    let active = app.spaces.spaces.iter().find(|s| s.id == app.spaces.active).map(|s| s.name.clone());
    assert_eq!((active.as_deref(), app.new_folder().as_str()), (Some("Outro"), "C:/projeto"));
    // a workspace with floors stays until they go
    app.delete_space("p");
    assert_eq!(app.spaces.spaces.len(), 5);
    assert_eq!(floors::branch_from("Nova Tela 2"), "nova-tela-2");
}

#[test]
fn an_isolated_floor_gets_its_own_worktree_and_goes_only_when_clean() {
    let home = Home::new("floor");
    let project = home.0.join("projeto");
    repo(&project, true);
    let mut app = home.app();
    app.create_space("Projeto", None, &project.to_string_lossy());
    let parent = app.spaces.active.clone();
    // as the dialog does: it waits while git works, and the floor shows when it comes
    app.space_dialog = Some(crate::app::spaces::SpaceDialog::Floor { parent: parent.clone(), name: "Nova tela".into(), isolate: true, branch: String::new(), busy: true, error: None });
    app.create_floor(&parent, "Nova tela", true, "").unwrap();
    settle(&mut app);
    let floor = app.spaces.spaces.iter().find(|s| s.parent.as_deref() == Some(&parent)).cloned().expect(&app.status);
    let worktree = floor.worktree.clone().unwrap();
    assert_eq!((worktree.branch.as_str(), worktree.base.as_str()), ("nova-tela", "main"));
    assert_eq!(read_spaces(&home.main()).0.spaces, app.spaces.spaces, "the floor is on disk too");
    assert!(std::path::Path::new(&worktree.path).starts_with(home.0.join("andares")));
    assert!(std::path::Path::new(&worktree.path).join("a.txt").exists());
    assert!(branches(&project).contains("nova-tela"));
    assert_eq!((app.spaces.active.as_str(), app.new_folder()), (floor.id.as_str(), worktree.path.clone()));
    // a change not committed keeps it
    fs::write(std::path::Path::new(&worktree.path).join("b.txt"), "b").unwrap();
    app.delete_space(&floor.id);
    assert!(app.spaces.spaces.iter().any(|s| s.id == floor.id));
    assert!(app.status.contains("alterações sem commit"));
    fs::remove_file(std::path::Path::new(&worktree.path).join("b.txt")).unwrap();
    // new files count even when the repository hides them from status
    let wt = std::path::Path::new(&worktree.path);
    let git = |args: &[&str]| crate::ide::git(wt, args).unwrap();
    git(&["config", "status.showUntrackedFiles", "no"]);
    fs::write(wt.join("c.txt"), "c").unwrap();
    app.delete_space(&floor.id);
    assert!(app.status.contains("alterações sem commit"), "{}", app.status);
    fs::remove_file(wt.join("c.txt")).unwrap();
    // and so does a commit on no branch
    git(&["checkout", "-q", "--detach"]);
    git(&["-c", "user.name=t", "-c", "user.email=t@t", "-c", "commit.gpgsign=false", "commit", "-q", "--allow-empty", "-m", "solto"]);
    app.delete_space(&floor.id);
    assert!(app.status.contains("fora de qualquer branch"), "{}", app.status);
    git(&["checkout", "-q", "nova-tela"]);
    app.delete_space(&floor.id);
    assert!(!app.spaces.spaces.iter().any(|s| s.id == floor.id));
    assert_eq!(app.spaces.active, parent);
    for _ in 0..60 {
        if !std::path::Path::new(&worktree.path).exists() {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(100));
    }
    assert!(!std::path::Path::new(&worktree.path).exists(), "the worktree folder goes");
    assert!(branches(&project).contains("nova-tela"), "the branch stays");
}

#[test]
fn a_floor_that_cannot_have_a_checkout_says_what_to_do() {
    let home = Home::new("refusals");
    let floors = home.0.join("andares");
    let plain = home.0.join("pasta");
    fs::create_dir_all(&plain).unwrap();
    assert!(floors::add_worktree(&plain, "x", &floors).unwrap_err().contains("não é um repositório Git"));
    let empty = home.0.join("vazio");
    repo(&empty, false);
    assert!(floors::add_worktree(&empty, "x", &floors).unwrap_err().contains("ainda não tem commits"));
    let project = home.0.join("projeto");
    repo(&project, true);
    assert!(floors::add_worktree(&project, "a b", &floors).unwrap_err().contains("não é um nome de branch válido"));
    // a branch open in another checkout, said with where
    assert!(floors::add_worktree(&project, "main", &floors).unwrap_err().contains("já está aberta em"));
    // a floor folder deleted by hand: its branch says how to free it, its name is skipped
    let gone = floors::add_worktree(&project, "x/tela", &floors).unwrap();
    fs::remove_dir_all(&gone.path).unwrap();
    assert!(floors::add_worktree(&project, "x/tela", &floors).unwrap_err().contains("git worktree prune"));
    let next = floors::add_worktree(&project, "x-tela", &floors).unwrap();
    assert!(next.path.ends_with("x-tela-2"), "{}", next.path);
    // removing a floor's checkout whose folder went takes only it off git's list
    let other = floors::add_worktree(&project, "y", &floors).unwrap();
    fs::remove_dir_all(&other.path).unwrap();
    floors::remove_worktree(&gone).unwrap();
    let listed = crate::ide::git(&project, &["worktree", "list", "--porcelain"]).unwrap();
    let folders: Vec<&str> = listed.lines().filter_map(|l| l.strip_prefix("worktree ")).collect();
    assert!(!folders.iter().any(|f| f.ends_with("/x-tela")) && folders.iter().any(|f| f.ends_with("/y")), "{listed}");
    // the workspace needs a folder first
    let mut app = home.app();
    app.create_space("Sem pasta", None, "");
    let parent = app.spaces.active.clone();
    assert!(app.create_floor(&parent, "x", true, "").unwrap_err().contains("não tem diretório de trabalho"));
}

#[test]
fn the_agents_of_a_hidden_space_are_listed_and_opening_one_brings_it_up() {
    let home = Home::new("agentes");
    let mut app = home.app();
    let here = terminal(&app, "Aqui");
    app.scene.elements.push(here);
    app.create_space("Outro", None, "");
    let other = app.spaces.active.clone();
    let there = terminal(&app, "Lá");
    let there_id = there.id.clone();
    app.scene.elements.push(there);
    app.switch_space(MAIN_SPACE);
    let rows: Vec<(String, Option<String>)> = app.agent_rows().into_iter().map(|r| (r.spec.name, r.space)).collect();
    assert_eq!(rows, [("Aqui".to_string(), None), ("Lá".to_string(), Some("Outro".to_string()))]);
    // the chat only reads it; on the canvas its workspace comes up
    app.chat_open = true;
    app.open_agent(&there_id);
    assert_eq!((app.spaces.active.as_str(), app.chat_agent.as_deref()), (MAIN_SPACE, Some(there_id.as_str())));
    app.chat_open = false;
    app.open_agent(&there_id);
    assert_eq!((&app.spaces.active, app.terminal_focus.as_ref()), (&other, Some(&there_id)));
}
