//! The file tree node: made with the toolbar button and its dialog, a folder's listing read off
//! the UI thread, open and closed, marked by git, kept in the saved drawing and run by agents.

use super::*;
use crate::app::filetree::*;
use std::path::Path;

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

#[test]
fn a_file_opens_in_the_trees_own_editor_pane_with_no_ide_involved() {
    use crate::app::filetree::editor::Body;
    let ctx = Context::default();
    let dir = temp("open");
    fs::write(dir.join("main.rs"), "fn main() {}\r\n").unwrap();
    fs::write(dir.join("notes.txt"), "one\n").unwrap();
    fs::write(dir.join("blob.bin"), [0u8, 1, 2, 3]).unwrap();
    fs::write(dir.join("big.txt"), vec![b'x'; (2 << 20) + 1]).unwrap();
    let mut app = DrawingApp::default();
    frame(&mut app, &ctx, vec![]);
    // even with an IDE node on the canvas, the tree keeps to its own editor
    app.ide_dialog = Some(IdeDialog {
        target: None,
        at: Rect::from_min_size(Pos2::new(900.0, 0.0), IDE_SIZE),
        spec: IdeSpec { root: dir.to_string_lossy().into(), ..Default::default() },
    });
    app.confirm_ide_dialog();
    let ide = app.scene.elements[0].id.clone();
    let tree = tree_on(&mut app, &dir);
    frame(&mut app, &ctx, vec![]);
    let width = |app: &DrawingApp| match app.scene.elements.iter().find(|e| e.id == tree).unwrap().kind {
        ElementKind::Rectangle { width, .. } => width,
        _ => 0.0,
    };
    let before = width(&app);
    app.ft_activate(&tree, dir.join("main.rs"));
    assert!(app.ides[&ide].active_path().is_none(), "the IDE node is not opened into");
    let spec = app.ft_spec(&tree).unwrap().clone();
    assert!(spec.editor && spec.tree_width == Some(before), "the pane is shown and the tree keeps its width");
    assert_eq!(width(&app), before + 601.0, "the node grows to the right by the pane and its seam");
    let ed = &app.filetrees.editors[&tree];
    assert_eq!((ed.tabs.len(), ed.active.as_deref()), (1, Some(dir.join("main.rs").as_path())));
    assert!(matches!(&ed.tabs[0].body, Body::Text(t) if t.text == "fn main() {}\n"), "read with LF ends");
    // typed in, saved with the file's own line ends
    if let Body::Text(t) = &mut app.filetrees.editors.get_mut(&tree).unwrap().tabs[0].body {
        t.text.push_str("// x\n");
        t.dirty = true;
    }
    assert!(app.filetrees.editors[&tree].dirty());
    app.filetrees.editors.get_mut(&tree).unwrap().save().unwrap();
    assert_eq!(fs::read_to_string(dir.join("main.rs")).unwrap(), "fn main() {}\r\n// x\r\n");
    // a click only previews, the next click takes its place; a double-click keeps the tab
    app.ft_apply(&ctx, &tree, crate::app::filetree::ui::Act::Select(dir.join("notes.txt")));
    app.ft_apply(&ctx, &tree, crate::app::filetree::ui::Act::Select(dir.join("blob.bin")));
    let ed = &app.filetrees.editors[&tree];
    let names: Vec<_> = ed.tabs.iter().map(|t| t.path.file_name().unwrap().to_string_lossy().into_owned()).collect();
    assert_eq!(names, ["main.rs", "blob.bin"], "the notes preview gave way");
    assert!(matches!(&ed.tabs[1].body, Body::Refused(why) if why.starts_with("Este não é um arquivo de texto")));
    app.ft_activate(&tree, dir.join("big.txt"));
    let ed = &app.filetrees.editors[&tree];
    assert!(matches!(&ed.tabs[2].body, Body::Refused(why) if why.contains("além do que o editor embutido comporta")), "over 2 MiB");
    // what the agents see, and close
    assert!(app.describe_tree(&tree).contains("Editor: aberto, abas main.rs, ~blob.bin, >big.txt"), "{}", app.describe_tree(&tree));
    app.act_file_tree(&json!({ "action": "close_file", "path": "big.txt" })).unwrap();
    app.act_file_tree(&json!({ "action": "editor", "show": false })).unwrap();
    assert!(!app.ft_spec(&tree).unwrap().editor);
    assert_eq!(width(&app), before, "the node shrinks back to the tree");
    app.act_file_tree(&json!({ "action": "open", "path": "notes.txt" })).unwrap();
    assert!(app.ft_spec(&tree).unwrap().editor);
    // a folder opens or closes instead
    fs::create_dir_all(dir.join("sub")).unwrap();
    app.ft_activate(&tree, dir.join("sub"));
    assert!(app.filetrees.views[&tree].expanded.contains(&dir.join("sub")));
    app.ft_activate(&tree, dir.join("sub"));
    assert!(app.filetrees.views[&tree].expanded.is_empty());
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn a_picture_and_a_pdf_open_in_the_pane_with_their_size() {
    use crate::app::filetree::editor::Body;
    let ctx = Context::default();
    let dir = temp("media");
    let png = dir.join("p.png");
    image::RgbaImage::from_pixel(7, 5, image::Rgba([1, 2, 3, 255])).save(&png).unwrap();
    let mut app = DrawingApp::default();
    let tree = tree_on(&mut app, &dir);
    frame(&mut app, &ctx, vec![]);
    app.ft_activate(&tree, png);
    let ed = &app.filetrees.editors[&tree];
    assert!(matches!(ed.tabs[0].body, Body::Media(_)));
    assert_eq!(crate::app::filetree::editor::bytes(500), "500 B");
    assert_eq!(crate::app::filetree::editor::bytes(2048), "2,0 KB");
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn a_click_on_a_row_selects_it_and_the_arrows_walk_the_tree() {
    let ctx = Context::default();
    let dir = temp("keys");
    fs::create_dir_all(dir.join("a")).unwrap();
    fs::write(dir.join("a/inside.txt"), "x").unwrap();
    fs::write(dir.join("b.txt"), "x").unwrap();
    let mut app = DrawingApp::default();
    tree_frame(&mut app, &ctx, 0.0, vec![]);
    let id = tree_on(&mut app, &dir);
    // the node sits where the view's middle is; its frame on screen sets where the rows are
    for t in [1.0, 1.5, 2.0] {
        tree_frame(&mut app, &ctx, t, vec![]);
    }
    let frame = app.terminal_frame(&app.scene.elements[0]).0;
    let shown = |name: &str| ctx.memory(|m| m.area_rect(egui::Id::unique((name, id.as_str())))).is_some();
    assert!(shown("ft_nav") && shown("ft_tools") && shown("ft_body"), "the buttons and the rows are over the card");
    // the title strip is the canvas's: no area over it between the buttons
    let strip = Pos2::new(frame.center().x, frame.top() + 10.0);
    let body = ctx.memory(|m| m.area_rect(egui::Id::unique(("ft_body", id.as_str())))).unwrap();
    assert!(!body.contains(strip), "{body:?} {strip:?}");
    let row0 = Pos2::new(frame.left() + 80.0, frame.top() + FT_BAR + 1.0 + FT_ROW * 0.5);
    tree_frame(&mut app, &ctx, 3.0, vec![egui::Event::PointerMoved(row0)]);
    tree_frame(&mut app, &ctx, 3.1, vec![button(row0, true)]);
    tree_frame(&mut app, &ctx, 3.2, vec![button(row0, false)]);
    assert_eq!(app.filetrees.views[&id].selected, Some(dir.join("a")));
    assert_eq!(app.card_focus.as_ref(), Some(&id), "the tree has the keys");
    assert_eq!(app.selected, vec![0]);
    // down to the next row; right opens the folder; Esc lets go
    let key = |key| egui::Event::Key { key, physical_key: None, pressed: true, repeat: false, modifiers: Modifiers::NONE };
    tree_frame(&mut app, &ctx, 4.0, vec![key(Key::ArrowDown)]);
    assert_eq!(app.filetrees.views[&id].selected, Some(dir.join("b.txt")));
    tree_frame(&mut app, &ctx, 4.1, vec![key(Key::ArrowUp), key(Key::ArrowRight)]);
    assert!(app.filetrees.views[&id].expanded.contains(&dir.join("a")));
    tree_frame(&mut app, &ctx, 4.2, vec![key(Key::ArrowDown)]);
    assert_eq!(app.filetrees.views[&id].selected, Some(dir.join("a/inside.txt")));
    tree_frame(&mut app, &ctx, 4.3, vec![key(Key::ArrowLeft)]);
    assert_eq!(app.filetrees.views[&id].selected, Some(dir.join("a")), "left goes up to the folder");
    tree_frame(&mut app, &ctx, 4.4, vec![key(Key::Escape)]);
    assert_eq!((app.card_focus.as_ref(), app.selected.len()), (None, 0));
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn the_filename_search_finds_files_below_the_folder() {
    let ctx = Context::default();
    let dir = temp("search");
    fs::create_dir_all(dir.join("deep/er")).unwrap();
    fs::write(dir.join("deep/er/target.rs"), "x").unwrap();
    fs::write(dir.join("other.txt"), "x").unwrap();
    let spec = FileTreeSpec { root: dir.to_string_lossy().into(), ..Default::default() };
    let mut view = TreeView::new();
    view.filter = "TARG".into();
    for _ in 0..200 {
        view.poll(&spec, &ctx, 0.0);
        if view.dirty {
            view.rebuild(&spec);
        }
        if !view.rows.is_empty() {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(25));
    }
    assert_eq!(view.rows.iter().map(|r| (r.name.as_str(), r.hint.replace('\\', "/"))).collect::<Vec<_>>(), [("target.rs", "deep/er".to_string())]);
    view.filter = "zzz".into();
    view.rebuild(&spec);
    assert_eq!(view.empty_note(&spec).as_deref(), Some("Nenhuma correspondência"));
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn agents_make_and_close_trees_by_name() {
    let dir = temp("agent");
    let mut app = DrawingApp::default();
    assert!(app.act_file_tree(&json!({ "action": "list" })).unwrap_err().contains("Não há árvore"));
    assert!(app.act_file_tree(&json!({ "action": "create", "path": dir.join("nada").to_string_lossy() })).is_err());
    let made = app.act_file_tree(&json!({ "action": "create", "path": dir.to_string_lossy() })).unwrap();
    assert!(made.starts_with("Árvore de arquivos") && made.contains("Esta pasta está vazia."), "{made}");
    // by the folder's name or by id
    let name = dir.file_name().unwrap().to_string_lossy().to_string();
    assert!(app.act_file_tree(&json!({ "action": "list", "tree": name.to_uppercase() })).is_ok());
    assert!(app.act_file_tree(&json!({ "action": "list", "tree": "nenhuma" })).is_err());
    app.act_file_tree(&json!({ "action": "close" })).unwrap();
    assert!(app.scene.elements.is_empty());
    app.undo();
    assert_eq!(app.scene.elements.len(), 1, "closing can be undone");
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn the_search_counts_hits_and_the_tree_rows_apart() {
    let dir = temp("hits");
    fs::create_dir_all(dir.join("deep/er")).unwrap();
    fs::write(dir.join("deep/er/target.rs"), "x").unwrap();
    fs::write(dir.join("target.txt"), "x").unwrap();
    fs::write(dir.join("other.txt"), "x").unwrap();
    let mut app = DrawingApp::default();
    let id = tree_on(&mut app, &dir);
    // the agents' search: the hits with their folders (slashes as the reference writes them) and the tree's own count
    let found = app.act_file_tree(&json!({ "action": "search", "query": "TARGET" })).unwrap();
    assert!(found.contains("Busca \"TARGET\": 2 ocorrências; a árvore tem 3 linhas"), "{found}");
    assert!(found.contains("target.rs (deep/er)") && found.contains("target.txt"), "{found}");
    let view = &app.filetrees.views[&id];
    assert_eq!((view.rows.len(), view.tree_rows, view.hits_cut), (2, 3, false));
    // an empty query is the tree again
    let back = app.act_file_tree(&json!({ "action": "search", "query": "" })).unwrap();
    assert!(!back.contains("Busca") && back.contains("other.txt"), "{back}");
    assert_eq!(crate::app::filetree::ui::hits_note(1), "1 ocorrência");
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn a_name_is_asked_in_a_popover_that_keeps_the_extension() {
    let ctx = Context::default();
    let dir = temp("popover");
    fs::write(dir.join("nota.txt"), "x").unwrap();
    let mut app = DrawingApp::default();
    tree_frame(&mut app, &ctx, 0.0, vec![]);
    let id = tree_on(&mut app, &dir);
    for t in [1.0, 1.5, 2.0] {
        tree_frame(&mut app, &ctx, t, vec![]);
    }
    // F2 asks with the base name; the extension is not the field's
    app.ft_apply(&ctx, &id, crate::app::filetree::ui::Act::Select(dir.join("nota.txt")));
    let row = app.filetrees.views[&id].rows.iter().find(|r| r.name == "nota.txt").cloned();
    app.ft_rename_ask(&ctx, &id, &row);
    match &app.filetrees.ask {
        Some(TreeAsk::Name { text, kind: NameKind::Rename, .. }) => assert_eq!(text, "nota"),
        other => panic!("{other:?}"),
    }
    assert!(app.filetree_dialog_open());
    tree_frame(&mut app, &ctx, 3.0, vec![]);
    assert!(ctx.memory(|m| m.area_rect(egui::Id::unique("ft_name_popover_area"))).is_some(), "the popover is under the row");
    // the field typed, Enter renames to the new base with the old extension
    if let Some(TreeAsk::Name { text, .. }) = app.filetrees.ask.as_mut() {
        *text = "memo".into();
    }
    let enter = egui::Event::Key { key: Key::Enter, physical_key: None, pressed: true, repeat: false, modifiers: Modifiers::NONE };
    tree_frame(&mut app, &ctx, 4.0, vec![enter]);
    assert!(dir.join("memo.txt").is_file() && !dir.join("nota.txt").exists(), "{:?}", app.status);
    assert!(app.filetrees.ask.is_none());
    // Escape leaves it as it was
    app.filetrees.ask = Some(TreeAsk::Name { tree: id, at: dir.join("memo.txt"), kind: NameKind::Rename, text: "x".into() });
    tree_frame(&mut app, &ctx, 5.0, vec![]);
    let esc = egui::Event::Key { key: Key::Escape, physical_key: None, pressed: true, repeat: false, modifiers: Modifiers::NONE };
    tree_frame(&mut app, &ctx, 6.0, vec![esc]);
    assert!(app.filetrees.ask.is_none() && dir.join("memo.txt").is_file());
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn code_in_the_pane_knows_the_trees_project_with_no_ide_involved() {
    use crate::app::filetree::code::{Found, Project};
    use crate::app::filetree::editor::Body;
    /// Frames until `done`: the project is listed, read and searched on threads.
    fn until(app: &mut DrawingApp, ctx: &Context, what: &str, done: impl Fn(&mut DrawingApp) -> bool) {
        for i in 0..400 {
            tree_frame(app, ctx, i as f64 * 0.02, vec![]);
            if done(app) {
                return;
            }
            std::thread::sleep(std::time::Duration::from_millis(10));
        }
        panic!("{what}");
    }
    let ctx = Context::default();
    install_fonts(&ctx, false); // the code is drawn with the IDE's font families
    let dir = temp("code");
    let (lib, index) = (dir.join("lib.php"), dir.join("index.php"));
    fs::write(&lib, "<?php\n\nfunction montar(int $largura, string $titulo) {}\n").unwrap();
    fs::write(&index, "<?php\nmontar(10, 'a');\nnada();\n").unwrap();
    let mut app = DrawingApp::default();
    let tree = tree_on(&mut app, &dir);
    frame(&mut app, &ctx, vec![]);
    app.ft_activate(&tree, index.clone());
    // the parameter names come from a declaration in another file of the tree's root
    let hints = |app: &DrawingApp| match &app.filetrees.editors[&tree].tabs[0].body {
        Body::Text(t) => t.laid.as_ref().map(|l| l.hints.iter().map(|(h, line, _)| (h.label.clone(), *line)).collect::<Vec<_>>()).unwrap_or_default(),
        _ => vec![],
    };
    until(&mut app, &ctx, "no parameter names", |app| hints(app) == [("largura:".to_string(), 1), ("titulo:".to_string(), 1)]);
    // Ctrl+click on the call: searched on a thread, then the declaration's file comes forward with the caret asked there
    let ask = |app: &mut DrawingApp, c: usize| {
        let ed = app.filetrees.editors.get_mut(&tree).unwrap();
        let Body::Text(t) = &ed.tabs[0].body else { panic!("not a text") };
        let (lang, text) = (t.lang, t.text.clone());
        assert_eq!(ed.code.declaration(&index, lang, &text, c, &ctx), None, "a name is searched off the UI thread");
    };
    ask(&mut app, 8);
    until(&mut app, &ctx, "the declaration's file did not open", |app| app.filetrees.editors[&tree].active.as_deref() == Some(lib.as_path()));
    let ed = &app.filetrees.editors[&tree];
    assert!(ed.preview.is_none() && ed.tabs.len() == 2, "opened for good, next to the file it was asked from");
    // a name declared nowhere says so
    ask(&mut app, 23);
    until(&mut app, &ctx, "no word of the missing declaration", |app| app.status == "Declaração de 'nada' não encontrada");
    // where it is, by the project alone
    let mut code = Project::default();
    code.open(dir.clone(), &ctx);
    while code.indexing() {
        code.poll();
        std::thread::sleep(std::time::Duration::from_millis(5));
    }
    let Body::Text(t) = crate::app::filetree::editor::load(&index, &ctx) else { panic!("not a text") };
    assert_eq!(code.declaration(&index, t.lang, &t.text, 8, &ctx), None);
    let found = loop {
        if let Some(found) = code.poll() {
            break found;
        }
        std::thread::sleep(std::time::Duration::from_millis(5));
    };
    assert_eq!(found, Found::At(lib.clone(), 2, 9));
    // a tab nobody typed in never writes its file: closing it leaves what is on disk
    fs::write(&index, "<?php\n// de fora\n").unwrap();
    app.filetrees.editors.get_mut(&tree).unwrap().close(&index).unwrap();
    assert_eq!(fs::read_to_string(&index).unwrap(), "<?php\n// de fora\n");
    let _ = fs::remove_dir_all(&dir);
}
