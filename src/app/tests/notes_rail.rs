//! The pile's tab rail past what fits ("Mais N páginas") and the note menu's Desfocar and Mover Arquivo para….

use super::*;
use crate::app::notes::stack::{more_pages_label, tab_rail};
use crate::app::ui::context::{Entry, Target};

fn pile(app: &mut DrawingApp, n: usize) -> Vec<usize> {
    let mut cards = Vec::new();
    for k in 0..n {
        let [note, label] = app.new_note(Rect::from_min_size(Pos2::new(100.0 + 300.0 * k as f32, 100.0), Vec2::splat(200.0)), format!("pagina {k}"));
        cards.push(app.scene.elements.len());
        app.scene.elements.extend([note, label]);
    }
    app.selected = cards.clone();
    assert!(app.stack_notes());
    cards
}

#[test]
fn tabs_stop_shrinking_at_64_and_the_last_one_becomes_more_pages() {
    let frame = Rect::from_min_size(Pos2::ZERO, Vec2::splat(200.0));
    // 3 fit (180 of rail, 54 per extra tab)
    let fits = tab_rail(frame, 3, 1.0);
    assert!(fits.more.is_none() && fits.tabs.len() == 3 && fits.tabs.iter().all(|t| t.height() >= 64.0));
    // 5 do not: two tabs and "Mais 3 páginas" (at most 64 tall)
    let rail = tab_rail(frame, 5, 1.0);
    assert_eq!(rail.tabs.len(), 2);
    let (more, hidden) = rail.more.unwrap();
    assert_eq!(hidden, 3);
    assert!(rail.tabs.iter().all(|t| t.height() >= 64.0) && more.height() <= 64.0 && (more.top() - (rail.tabs[1].bottom() - 10.0)).abs() < 0.01);
    // a rail of 64 or less keeps one tab
    let tiny = tab_rail(Rect::from_min_size(Pos2::ZERO, Vec2::splat(80.0)), 4, 1.0);
    assert_eq!((tiny.tabs.len(), tiny.more.map(|m| m.1)), (1, Some(3)));
    assert_eq!(more_pages_label(1), "Mais 1 página");
    assert_eq!(more_pages_label(3), "Mais 3 páginas");
}

#[test]
fn the_hidden_pages_are_listed_and_a_row_shows_one() {
    let ctx = Context::default();
    let mut app = DrawingApp::default();
    let cards = pile(&mut app, 6);
    // the pile stands 1.4 times as tall as wide: 270 + 23 wide, 410 tall; that rail holds 6 tabs, so shrink it
    if let ElementKind::Rectangle { height, .. } = &mut app.scene.elements[cards[0]].kind {
        *height = 150.0;
    }
    let hidden = app.hidden_stack_pages(cards[0]);
    assert!(!hidden.is_empty() && hidden.iter().all(|p| cards.contains(p)));
    let last = *hidden.last().unwrap();
    app.context_action(&ctx, &Target::Element(app.scene.elements[cards[0]].id.clone()), &format!("stack_show:{}", app.scene.elements[last].id));
    assert!(app.scene.elements[last].note.front);
}

#[test]
fn the_note_menu_has_desfocar_and_mover_arquivo_in_the_references_order() {
    let ctx = Context::default();
    let mut app = DrawingApp::default();
    let cards = pile(&mut app, 2);
    let id = app.scene.elements[cards[0]].id.clone();
    let rows = app.note_rows(cards[0], &id);
    let labels: Vec<&str> = rows
        .iter()
        .filter_map(|r| match r {
            Entry::Item { label, .. } => Some(label.as_str()),
            Entry::Sub { label, .. } => Some(label.as_str()),
            Entry::Sep => None,
        })
        .collect();
    assert_eq!(
        labels,
        [
            "Renomear",
            "Bloquear Conteúdo",
            "Desfocar",
            "Copiar Conteúdo",
            "Mostrar na Pasta",
            "Mover Arquivo para…",
            "Cor",
            "Adicionar Nota",
            "Remover do Fichário",
            "Apagar Nota"
        ]
    );
    app.context_action(&ctx, &Target::Element(id), "note_blur");
    assert!(app.scene.elements[cards[0]].note.blurred);
}

#[test]
fn a_note_file_moves_to_a_folder_and_comes_back() {
    let dir = std::env::temp_dir().join(format!("mago-nota-{}", uuid::Uuid::new_v4().simple()));
    let (scene, folder) = (dir.join("cena.mago"), dir.join("fora"));
    std::fs::create_dir_all(&folder).unwrap();
    let mut app = DrawingApp::default();
    app.autosave = Some(scene.clone());
    let [note, label] = app.new_note(Rect::from_min_size(Pos2::ZERO, Vec2::splat(200.0)), "Minha nota".into());
    app.scene.elements = vec![note, label];
    let id = app.scene.elements[0].id.clone();
    app.act("note_stack", &json!({ "action": "move_file", "notes": [id.clone()], "folder": folder.to_string_lossy() })).unwrap();
    let moved = crate::file::note_file(&scene, &app.scene.elements[0]);
    assert_eq!(moved.parent().unwrap(), folder);
    assert_eq!(std::fs::read_to_string(&moved).unwrap(), "Minha nota");
    assert!(app.act("note_stack", &json!({ "action": "move_file", "notes": [id.clone()], "folder": dir.join("nada").to_string_lossy() })).is_err());
    // back beside the scene
    app.act("note_stack", &json!({ "action": "move_file", "notes": [id.clone()] })).unwrap();
    assert!(app.scene.elements[0].note.file.is_none() && !moved.exists());
    assert!(crate::file::note_file(&scene, &app.scene.elements[0]).exists());
    // blur through the MCP, toggling and explicit
    app.act("note_stack", &json!({ "action": "blur", "notes": [id.clone()] })).unwrap();
    assert!(app.scene.elements[0].note.blurred);
    app.act("note_stack", &json!({ "action": "blur", "notes": [id], "value": "off" })).unwrap();
    assert!(!app.scene.elements[0].note.blurred);
    let _ = std::fs::remove_dir_all(dir);
}
