//! The file tree node: made with the toolbar button and its dialog, a folder's listing read off
//! the UI thread, open and closed, marked by git, kept in the saved drawing and run by agents.

use super::*;
use crate::app::filetree::*;
use std::path::Path;
mod pane;

pub(super) fn temp(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("magoagent-filetree-{name}-{}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    dir
}

/// One frame with the file tree's parts over the canvas.
pub(super) fn tree_frame(app: &mut DrawingApp, ctx: &Context, time: f64, events: Vec<egui::Event>) {
    let mut output = ctx.run_ui(
        egui::RawInput { screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(800.0, 600.0))), time: Some(time), events, ..Default::default() },
        |ui| {
            app.handle_shortcuts(ctx);
            app.sync_filetrees(ctx);
            egui::CentralPanel::no_frame().show(ui, |ui| {
                let (response, _) = ui.allocate_painter(ui.available_size(), Sense::CLICK | Sense::DRAG);
                app.handle_canvas(&response, response.rect);
            });
            app.filetree_overlay(ctx);
        },
    );
    output.textures_delta.clear();
}

/// A tree on `root`, as an agent makes one; its id.
pub(super) fn tree_on(app: &mut DrawingApp, root: &Path) -> String {
    // a frame first: the view's middle is where it goes
    frame(app, &Context::default(), vec![]);
    app.act_file_tree(&json!({ "action": "create", "path": root.to_string_lossy() })).unwrap();
    app.scene.elements.iter().find(|e| e.filetree.is_some()).unwrap().id.clone()
}

fn rows(app: &mut DrawingApp, id: &str) -> Vec<String> {
    let text = app.describe_tree(id);
    text.lines().skip(1).map(|l| l.trim_end().to_string()).collect()
}

#[test]
fn the_toolbar_button_asks_for_a_folder_and_makes_the_node() {
    let ctx = Context::default();
    let dir = temp("dialog");
    let mut app = DrawingApp::default();
    // between Terminal and Portal, as the reference app's bar
    let at = |t: Tool| Tool::TOOLBAR.iter().position(|x| *x == t).unwrap();
    assert_eq!(at(Tool::FileTree) + 1, at(Tool::Portal));
    assert_eq!(Tool::FileTree.label(), "Árvore de Arquivos");
    frame(&mut app, &ctx, vec![]);
    app.select_tool(Tool::FileTree);
    drag(&mut app, &ctx, Pos2::new(400.0, 300.0), Pos2::new(401.0, 300.0));
    assert_eq!(app.tool, Tool::Select);
    assert!(app.filetree_dialog_open() && app.scene.elements.is_empty(), "the folder is asked first");
    // a folder that isn't there keeps the dialog open
    app.filetrees.dialog.as_mut().unwrap().root = dir.join("nada").to_string_lossy().into();
    app.confirm_filetree_dialog();
    assert!(app.filetrees.dialog.is_some() && app.scene.elements.is_empty());
    assert!(app.status.contains("Não foi possível criar a árvore de arquivos"), "{}", app.status);
    app.filetrees.dialog.as_mut().unwrap().root = dir.to_string_lossy().into();
    app.confirm_filetree_dialog();
    let node = &app.scene.elements[0];
    assert_eq!(node.filetree, Some(FileTreeSpec { root: dir.to_string_lossy().into(), ..Default::default() }));
    assert_eq!(local_bounds(node).size(), FILETREE_SIZE);
    assert!(node.is_node() && !node.rotatable());
    assert_eq!(node.min_size(), Some(FILETREE_MIN));
    // its drag-out box is the node's own, as big as the reference's 300 × 300 at least
    app.select_tool(Tool::FileTree);
    drag(&mut app, &ctx, Pos2::new(100.0, 100.0), Pos2::new(130.0, 120.0));
    assert!(app.filetree_dialog_open());
    assert!(app.filetrees.dialog.as_ref().unwrap().at.size().min_elem() >= 300.0);
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn folders_come_first_numbers_in_order_and_hidden_ones_can_go() {
    let dir = temp("order");
    for name in ["b.txt", "a10.txt", "a2.txt", ".hid"] {
        fs::write(dir.join(name), "x").unwrap();
    }
    fs::create_dir_all(dir.join("zdir")).unwrap();
    fs::write(dir.join("zdir/inner.txt"), "x").unwrap();
    let mut app = DrawingApp::default();
    let id = tree_on(&mut app, &dir);
    let names = |rows: Vec<String>| {
        let name = |r: &String| r.trim().trim_start_matches("> ").trim_start_matches("v ").split(' ').next().unwrap_or_default().to_string();
        rows.iter().map(name).collect::<Vec<_>>()
    };
    assert_eq!(names(rows(&mut app, &id)), ["zdir", ".hid", "a2.txt", "a10.txt", "b.txt"]);
    // the reference app shows hidden files at first; hiding them is the view menu's
    app.act_file_tree(&json!({ "action": "hidden", "show": false })).unwrap();
    assert_eq!(names(rows(&mut app, &id)), ["zdir", "a2.txt", "a10.txt", "b.txt"]);
    // by size, biggest first; reversed, smallest
    fs::write(dir.join("b.txt"), "xxxxxx").unwrap();
    app.act_file_tree(&json!({ "action": "sort", "by": "size" })).unwrap();
    assert_eq!(names(rows(&mut app, &id)), ["zdir", "b.txt", "a2.txt", "a10.txt"]);
    app.act_file_tree(&json!({ "action": "sort", "by": "size", "reverse": true })).unwrap();
    assert_eq!(names(rows(&mut app, &id)), ["zdir", "a10.txt", "a2.txt", "b.txt"]);
    assert!(app.act_file_tree(&json!({ "action": "sort", "by": "cor" })).is_err());
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn folders_open_and_close_and_the_view_goes_in_and_back() {
    let dir = temp("expand");
    fs::create_dir_all(dir.join("src/deep")).unwrap();
    fs::write(dir.join("src/main.rs"), "x").unwrap();
    fs::write(dir.join("src/deep/leaf.txt"), "x").unwrap();
    let mut app = DrawingApp::default();
    let id = tree_on(&mut app, &dir);
    assert_eq!(rows(&mut app, &id), ["> src"]);
    app.act_file_tree(&json!({ "action": "expand", "path": "src" })).unwrap();
    assert_eq!(rows(&mut app, &id), ["v src", "  > deep", "    main.rs"]);
    app.act_file_tree(&json!({ "action": "expand", "path": "src/deep" })).unwrap();
    assert_eq!(rows(&mut app, &id), ["v src", "  v deep", "      leaf.txt", "    main.rs"]);
    // closing a folder keeps what was open inside it for when it opens again
    app.act_file_tree(&json!({ "action": "collapse", "path": "src" })).unwrap();
    assert_eq!(rows(&mut app, &id), ["> src"]);
    app.act_file_tree(&json!({ "action": "expand", "path": "src" })).unwrap();
    assert_eq!(rows(&mut app, &id).len(), 4);
    app.act_file_tree(&json!({ "action": "collapse_all" })).unwrap();
    assert_eq!(rows(&mut app, &id), ["> src"]);
    // into a folder, then back up and forward again; nothing leaves the root
    app.act_file_tree(&json!({ "action": "navigate", "path": "src" })).unwrap();
    assert_eq!(app.ft_spec(&id).unwrap().name(), "src");
    assert_eq!(rows(&mut app, &id), ["> deep", "  main.rs"]);
    assert!(app.act_file_tree(&json!({ "action": "navigate", "path": dir.parent().unwrap().to_string_lossy() })).is_err());
    app.act_file_tree(&json!({ "action": "back" })).unwrap();
    assert_eq!(app.ft_spec(&id).unwrap().current, "");
    app.act_file_tree(&json!({ "action": "back" })).unwrap();
    assert_eq!(app.ft_spec(&id).unwrap().current, "", "the root has nothing above it");
    app.act_file_tree(&json!({ "action": "forward" })).unwrap();
    assert_eq!(app.ft_spec(&id).unwrap().name(), "src");
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn the_listing_is_read_off_the_ui_thread_and_follows_the_disk() {
    let ctx = Context::default();
    let dir = temp("poll");
    fs::write(dir.join("one.txt"), "x").unwrap();
    let spec = FileTreeSpec { root: dir.to_string_lossy().into(), ..Default::default() };
    let mut view = TreeView::new();
    view.poll(&spec, &ctx, 0.0);
    assert!(matches!(view.listings.get(&dir), Some(Listing::Loading)), "the folder is being read, not waited for");
    let wait = |view: &mut TreeView, spec: &FileTreeSpec, now: f64, want: usize| {
        for _ in 0..200 {
            view.poll(spec, &ctx, now);
            if view.dirty {
                view.rebuild(spec);
            }
            if view.rows.len() == want {
                return;
            }
            std::thread::sleep(std::time::Duration::from_millis(25));
        }
        panic!("rows {:?}", view.rows);
    };
    wait(&mut view, &spec, 0.0, 1);
    // a file made outside shows at the next read (every couple of seconds)
    fs::write(dir.join("two.txt"), "x").unwrap();
    wait(&mut view, &spec, 10.0, 2);
    fs::remove_file(dir.join("one.txt")).unwrap();
    wait(&mut view, &spec, 20.0, 1);
    // a folder that is gone says why
    let gone = FileTreeSpec { root: dir.join("nada").to_string_lossy().into(), ..Default::default() };
    let mut view = TreeView::new();
    for _ in 0..200 {
        view.poll(&gone, &ctx, 0.0);
        if matches!(view.listings.get(&gone.shown()), Some(Listing::Failed(_))) {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(25));
    }
    assert!(view.empty_note(&gone).unwrap().starts_with("Não foi possível ler esta pasta."));
    let _ = fs::remove_dir_all(&dir);
}

pub(super) fn git(dir: &Path, args: &[&str]) {
    let out = crate::ide::command("git").args(args).current_dir(dir).output().unwrap();
    assert!(out.status.success(), "git {args:?}: {} {}", out.status, String::from_utf8_lossy(&out.stderr));
}

#[test]
fn git_marks_each_file_and_rolls_up_into_its_folders() {
    if crate::ide::command("git").arg("--version").output().is_err() {
        return;
    }
    let dir = temp("git");
    assert!(!TreeGit::read(&dir).repo || dir.join(".git").exists() || dir.ancestors().any(|a| a.join(".git").exists()), "outside a repository: no marks");
    git(&dir, &["init", "-q"]);
    git(&dir, &["config", "user.email", "t@example.com"]);
    git(&dir, &["config", "user.name", "t"]);
    fs::create_dir_all(dir.join("pkg")).unwrap();
    fs::write(dir.join("a.txt"), "one").unwrap();
    fs::write(dir.join("pkg/x.txt"), "one").unwrap();
    git(&dir, &["add", "."]);
    git(&dir, &["commit", "-qm", "first"]);
    fs::write(dir.join("a.txt"), "two").unwrap();
    fs::write(dir.join("pkg/x.txt"), "two").unwrap();
    fs::write(dir.join("new.txt"), "x").unwrap();
    fs::create_dir_all(dir.join("fresh")).unwrap();
    fs::write(dir.join("fresh/f.txt"), "x").unwrap();
    fs::write(dir.join("fresh/g.txt"), "x").unwrap();
    let info = TreeGit::read(&dir);
    assert!(info.repo && !info.branch.is_empty());
    // counted file by file, as the reference's `git status -uall`: the new folder is its two files
    assert_eq!(info.changes, 5);
    let of = |p: &str, d: bool| info.of(&dir.join(p), d);
    assert_eq!(of("a.txt", false), Some((crate::ide::Status::Modified, false)));
    assert_eq!(of("new.txt", false), Some((crate::ide::Status::Untracked, false)));
    // a folder holding a change wears its mark, but only as a color (rolled up)
    assert_eq!(of("pkg", true), Some((crate::ide::Status::Modified, true)));
    // a folder of new files wears their mark as a color, and each file its own
    assert_eq!(of("fresh", true), Some((crate::ide::Status::Untracked, true)));
    assert_eq!(of("fresh/f.txt", false), Some((crate::ide::Status::Untracked, false)));
    assert_eq!(of("pkg/x.txt", false), Some((crate::ide::Status::Modified, false)));
    assert_eq!(of("clean.txt", false), None);
    // the agents' view of the tree carries the marks and the branch
    let mut app = DrawingApp::default();
    let id = tree_on(&mut app, &dir);
    let text = app.describe_tree(&id);
    assert!(text.contains("ramo") && text.contains("5 alterado(s)"), "{text}");
    assert!(text.contains("a.txt M") && text.contains("new.txt U"), "{text}");
    // the tree on a folder inside the repository sees the same marks, from where it starts
    let sub = dir.join("pkg");
    assert_eq!(TreeGit::read(&sub).of(&sub.join("x.txt"), false), Some((crate::ide::Status::Modified, false)));
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn a_change_and_the_folders_over_it_are_blue_and_the_ignored_gray() {
    use crate::app::filetree::ui::name_color;
    use crate::ide::Status;
    if crate::ide::command("git").arg("--version").output().is_err() {
        return;
    }
    let dir = temp("colors");
    git(&dir, &["init", "-q"]);
    git(&dir, &["config", "user.email", "t@example.com"]);
    git(&dir, &["config", "user.name", "t"]);
    fs::create_dir_all(dir.join("src/deep")).unwrap();
    fs::write(dir.join("src/deep/a.rs"), "one").unwrap();
    fs::write(dir.join("src/gone.txt"), "one").unwrap();
    fs::write(dir.join(".gitignore"), "build/\n*.log\n").unwrap();
    git(&dir, &["add", "."]);
    git(&dir, &["commit", "-qm", "first"]);
    fs::write(dir.join("src/deep/a.rs"), "two").unwrap();
    fs::remove_file(dir.join("src/gone.txt")).unwrap();
    fs::create_dir_all(dir.join("build/obj")).unwrap();
    fs::write(dir.join("build/obj/out.bin"), "x").unwrap();
    fs::write(dir.join("src/debug.log"), "x").unwrap();
    let info = TreeGit::read(&dir);
    // what git ignores is no change, and wears no letter
    assert_eq!(info.changes, 2);
    assert_eq!(info.of(&dir.join("build"), true), None);
    let ignored = |p: &str| info.ignored(&dir.join(p));
    assert!(ignored("build") && ignored("build/obj") && ignored("build/obj/out.bin") && ignored("src/debug.log"));
    assert!(!ignored("src") && !ignored("src/deep/a.rs") && !ignored(".gitignore"));
    // the name's color, on the dark card: the IDE's blue for the change and every folder over it
    let color = |p: &str, folder: bool| name_color(info.of(&dir.join(p), folder), ignored(p), true);
    let blue = Some(Status::Modified.color());
    assert_eq!([color("src/deep/a.rs", false), color("src/deep", true), color("src", true)], [blue; 3]);
    assert_eq!(color("build", true), Some(Color32::from_gray(0x8c)));
    assert_eq!(color("src/debug.log", false), Some(Color32::from_gray(0x8c)));
    assert_eq!(color(".gitignore", false), None);
    // a folder that lost a file is blue too, not the gray of a deleted file
    assert_eq!(info.of(&dir.join("src"), true), Some((Status::Deleted, true)));
    assert_eq!(name_color(Some((Status::Added, false)), false, true), Some(Status::Added.color()));
    // and each has its color on the light card
    assert_ne!(name_color(Some((Status::Modified, false)), false, false), blue);
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn the_node_survives_saving_and_the_agents_json() {
    let dir = temp("save");
    let mut app = DrawingApp::default();
    let id = tree_on(&mut app, &dir);
    app.act_file_tree(&json!({ "action": "hidden", "show": false })).unwrap();
    app.act_file_tree(&json!({ "action": "sort", "by": "modified", "reverse": true })).unwrap();
    let node = app.scene.elements.iter().find(|e| e.id == id).unwrap().clone();
    assert_eq!(node.filetree.as_ref().map(|s| (s.hide_hidden, s.sort, s.reversed)), Some((true, SortBy::DateModified, true)));
    // saved and opened again
    let scene = Scene { elements: vec![node.clone()], ..Default::default() };
    assert_eq!(parse_scene(&serde_json::to_string(&scene).unwrap()).unwrap(), scene);
    // through the element JSON the clipboard and the MCP use
    let back = crate::tests::round_trip(&[node.clone()]);
    assert_eq!((back[0].filetree.as_ref(), back[0].kind.clone()), (node.filetree.as_ref(), node.kind.clone()));
    assert_eq!(interop_elements(&[node])[0]["type"], "filetree");
    // plain shapes don't grow the field in the saved file
    assert!(
        !serde_json::to_string(&DrawingApp::default().new_element(ElementKind::Rectangle { x: 0.0, y: 0.0, width: 1.0, height: 1.0 }))
            .unwrap()
            .contains("filetree")
    );
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn files_are_made_renamed_copied_moved_and_never_overwritten() {
    let dir = temp("ops");
    fs::create_dir_all(dir.join("box")).unwrap();
    let mut app = DrawingApp::default();
    let id = tree_on(&mut app, &dir);
    app.act_file_tree(&json!({ "action": "new_file", "name": "note.txt" })).unwrap();
    app.act_file_tree(&json!({ "action": "new_folder", "name": "more" })).unwrap();
    assert!(dir.join("note.txt").is_file() && dir.join("more").is_dir());
    // a name the system refuses, one already taken and an empty one make nothing
    assert!(app.act_file_tree(&json!({ "action": "new_file", "name": "a/b" })).is_err());
    assert!(app.act_file_tree(&json!({ "action": "new_file", "name": "note.txt" })).is_err(), "never replaces a file");
    assert!(app.act_file_tree(&json!({ "action": "new_file", "name": "  " })).is_err());
    app.act_file_tree(&json!({ "action": "rename", "path": "note.txt", "name": "memo.txt" })).unwrap();
    assert!(dir.join("memo.txt").is_file() && !dir.join("note.txt").exists());
    assert!(app.act_file_tree(&json!({ "action": "rename", "path": "memo.txt", "name": "more" })).is_err());
    // copy into a folder, twice: the second gets a name of its own
    app.act_file_tree(&json!({ "action": "copy", "path": "memo.txt" })).unwrap();
    app.act_file_tree(&json!({ "action": "paste", "path": "box" })).unwrap();
    app.act_file_tree(&json!({ "action": "paste", "path": "box" })).unwrap();
    assert!(dir.join("box/memo.txt").is_file() && dir.join("box/memo cópia.txt").is_file());
    assert!(dir.join("memo.txt").is_file(), "copying keeps the file");
    // cut and paste moves; a folder can't go into itself
    app.act_file_tree(&json!({ "action": "cut", "path": "memo.txt" })).unwrap();
    app.act_file_tree(&json!({ "action": "paste", "path": "more" })).unwrap();
    assert!(dir.join("more/memo.txt").is_file() && !dir.join("memo.txt").exists());
    app.act_file_tree(&json!({ "action": "copy", "path": "box" })).unwrap();
    assert!(app.act_file_tree(&json!({ "action": "paste", "path": "box" })).is_err());
    // the asks the dialogs make end in the same operations
    app.ft_answer(TreeAsk::Name { tree: id.clone(), at: dir.clone(), kind: NameKind::NewFile, text: String::new() }, "feito.txt".into());
    assert!(dir.join("feito.txt").is_file() && app.status == "Arquivo criado.");
    app.ft_answer(TreeAsk::Name { tree: id, at: dir.join("feito.txt"), kind: NameKind::Rename, text: String::new() }, "pronto.txt".into());
    assert!(dir.join("pronto.txt").is_file() && app.status == "Renomeado.");
    // outside the tree nothing is touched
    let outside = dir.parent().unwrap().join("fora.txt").to_string_lossy().to_string();
    assert!(app.act_file_tree(&json!({ "action": "rename", "path": outside, "name": "x.txt" })).is_err());
    let _ = fs::remove_dir_all(&dir);
}
