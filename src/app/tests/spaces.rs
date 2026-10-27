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
    assert_eq!(read_spaces(&home.main()).active, other);
    // a restart opens the space left open, with its camera
    drop(app);
    let app = home.app();
    assert_eq!((app.spaces.active.as_str(), left(&app)), (other.as_str(), 500.0));
    assert_eq!(app.spaces.spaces[1].color, Some([0x3d, 0x7e, 0xf0]));
}

#[test]
fn a_list_that_lost_its_main_or_active_space_still_opens_the_main_one() {
    let home = Home::new("broken");
    let space = |id: &str| Space { id: id.into(), name: id.into(), ..Default::default() };
    let list = Spaces { version: 1, active: "gone".into(), spaces: vec![space("a"), space("../fora")] };
    write_spaces(&home.main(), &list).unwrap();
    let spaces = read_spaces(&home.main());
    assert_eq!(spaces.active, MAIN_SPACE);
    assert_eq!(spaces.spaces.iter().map(|s| s.id.as_str()).collect::<Vec<_>>(), [MAIN_SPACE, "a"]);
    fs::write(home.0.join("workspaces.json"), "{ not json").unwrap();
    assert_eq!(read_spaces(&home.main()), Spaces::default());
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
        app.space_jobs();
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
    app.create_floor(&parent, "Nova tela", true, "").unwrap();
    settle(&mut app);
    let floor = app.spaces.spaces.iter().find(|s| s.parent.as_deref() == Some(&parent)).cloned().expect(&app.status);
    let worktree = floor.worktree.clone().unwrap();
    assert_eq!((worktree.branch.as_str(), worktree.base.as_str()), ("nova-tela", "main"));
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
    // the workspace needs a folder first
    let mut app = home.app();
    app.create_space("Sem pasta", None, "");
    let parent = app.spaces.active.clone();
    assert!(app.create_floor(&parent, "x", true, "").unwrap_err().contains("não tem diretório de trabalho"));
}
