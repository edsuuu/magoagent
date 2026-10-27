//! The live drawing on disk: never written over when it can't be read, back when it goes away,
//! saved on close.

use super::*;

/// A drawing path of its own per test, with whatever it set aside cleaned up.
struct Drawing(PathBuf);

impl Drawing {
    fn new(name: &str) -> Self {
        let drawing = Drawing(std::env::temp_dir().join(format!("magoagent-persist-{name}-{}.mago", std::process::id())));
        drawing.clean();
        drawing
    }

    fn set_aside(&self) -> Vec<PathBuf> {
        let name = self.0.file_name().unwrap().to_string_lossy().to_string();
        fs::read_dir(self.0.parent().unwrap())
            .unwrap()
            .filter_map(|e| e.ok().map(|e| e.path()))
            .filter(|p| p.file_name().unwrap().to_string_lossy().starts_with(&format!("{name}.bad-")))
            .collect()
    }

    fn clean(&self) {
        for path in self.set_aside().into_iter().chain([self.0.clone()]) {
            let _ = fs::remove_file(path);
        }
    }

    /// Writes as another process would, stamped later than anything this test wrote before.
    fn write_outside(&self, data: &str) {
        fs::write(&self.0, data).unwrap();
        let later = SystemTime::now() + std::time::Duration::from_secs(5);
        fs::File::options().write(true).open(&self.0).unwrap().set_modified(later).unwrap();
    }

    fn app(&self) -> DrawingApp {
        let mut app = DrawingApp { autosave: Some(self.0.clone()), ..Default::default() };
        app.load_autosave();
        app
    }
}

impl Drop for Drawing {
    fn drop(&mut self) {
        self.clean();
    }
}

fn boxed(app: &DrawingApp, x: f32) -> Element {
    app.new_element(ElementKind::Rectangle { x, y: 0.0, width: 100.0, height: 50.0 })
}

#[test]
fn only_this_builds_drawings_parse() {
    let scene = Scene::default();
    let mut newer = serde_json::to_value(&scene).unwrap();
    newer["version"] = json!(SCENE_VERSION + 1);
    assert!(matches!(parse_scene(&newer.to_string()), Err(SceneError::Newer(Some(_)))));
    assert!(matches!(parse_scene("{\"version\":"), Err(SceneError::Unreadable(_))));
    assert!(matches!(parse_scene(&serde_json::to_string(&scene).unwrap()), Ok(s) if s.version == SCENE_VERSION));
}

#[test]
fn an_unreadable_drawing_is_set_aside_at_startup_not_written_over() {
    let drawing = Drawing::new("unreadable");
    drawing.write_outside("{\"version\":1,\"elements\":[");
    let mut app = drawing.app();
    let kept = drawing.set_aside();
    assert_eq!(kept.len(), 1);
    assert_eq!(fs::read_to_string(&kept[0]).unwrap(), "{\"version\":1,\"elements\":[");
    assert!(app.status.contains("ilegível"));
    // a fresh drawing starts in its place
    app.scene.elements.push(boxed(&app, 0.0));
    app.dirty = true;
    app.write_autosave();
    assert_eq!(read_scene_file(&drawing.0).unwrap().0.elements.len(), 1);
}

#[test]
fn a_drawing_from_a_newer_build_is_shown_but_never_written() {
    let drawing = Drawing::new("newer");
    let app = DrawingApp::default();
    let mut file = serde_json::to_value(Scene { elements: vec![boxed(&app, 0.0)], ..Default::default() }).unwrap();
    file["version"] = json!(SCENE_VERSION + 1);
    file["elements"][0]["fieldFromTheFuture"] = json!(true);
    drawing.write_outside(&file.to_string());
    let mut app = drawing.app();
    assert_eq!(app.scene.elements.len(), 1);
    assert!(app.autosave.is_some(), "the preferences still save");
    app.scene.elements.push(boxed(&app, 200.0));
    app.dirty = true;
    app.save_on_exit();
    assert_eq!(fs::read_to_string(&drawing.0).unwrap(), file.to_string());
}

#[test]
fn a_drawing_gone_while_open_is_written_back() {
    let drawing = Drawing::new("gone");
    let mut app = drawing.app();
    app.scene.elements.push(boxed(&app, 0.0));
    app.dirty = true;
    app.write_autosave();
    fs::remove_file(&drawing.0).unwrap();
    // the poll notices, and the next autosave writes it again
    app.sync_from_file();
    assert!(app.dirty);
    app.write_autosave();
    assert_eq!(read_scene_file(&drawing.0).unwrap().0.elements.len(), 1);
    // also when it goes between two polls, with an edit pending
    fs::remove_file(&drawing.0).unwrap();
    app.scene.elements.push(boxed(&app, 200.0));
    app.write_autosave();
    assert_eq!(read_scene_file(&drawing.0).unwrap().0.elements.len(), 2);
}

#[test]
fn a_drawing_gone_and_remade_by_an_ai_keeps_both() {
    let drawing = Drawing::new("remade");
    let mut app = drawing.app();
    app.scene.elements.push(boxed(&app, 0.0));
    app.dirty = true;
    app.write_autosave();
    fs::remove_file(&drawing.0).unwrap();
    app.sync_from_file();
    // the MCP starts a fresh file with its own box before this window writes again
    let theirs = Scene { elements: vec![boxed(&app, 300.0)], ..Default::default() };
    drawing.write_outside(&serde_json::to_string(&theirs).unwrap());
    app.save_on_exit();
    assert_eq!(read_scene_file(&drawing.0).unwrap().0.elements.len(), 2);
}

#[test]
fn a_drawing_spoiled_while_open_is_not_written_over() {
    let drawing = Drawing::new("spoiled");
    let mut app = drawing.app();
    app.scene.elements.push(boxed(&app, 0.0));
    app.dirty = true;
    app.write_autosave();
    drawing.write_outside("não é um desenho");
    app.scene.elements.push(boxed(&app, 200.0));
    app.dirty = true;
    app.sync_from_file();
    app.save_on_exit();
    assert_eq!(fs::read_to_string(&drawing.0).unwrap(), "não é um desenho");
    assert_eq!(app.scene.elements.len(), 2);
}

#[test]
fn closing_mid_edit_keeps_the_text_and_what_an_ai_drew_meanwhile() {
    let drawing = Drawing::new("exit");
    let mut app = drawing.app();
    app.scene.elements.push(boxed(&app, 0.0));
    app.dirty = true;
    app.write_autosave();
    // an AI adds a box, and the user is still typing when the window closes
    let mut external = app.scene.clone();
    external.elements.push(boxed(&app, 200.0));
    drawing.write_outside(&serde_json::to_string(&external).unwrap());
    app.scene.elements.push(app.new_element(ElementKind::Text { x: 0.0, y: 100.0, value: "quase".into() }));
    app.editing = Some(1);
    app.save_on_exit();
    assert_eq!(app.editing, None);
    let saved = read_scene_file(&drawing.0).unwrap().0;
    assert_eq!(saved.elements.len(), 3);
    assert!(saved.elements.iter().any(|e| matches!(&e.kind, ElementKind::Text { value, .. } if value == "quase")));
}
