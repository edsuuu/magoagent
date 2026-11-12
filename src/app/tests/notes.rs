//! Notes and menus: the click that focuses a note, the bold button, the right-click menus.

use super::*;
use crate::app::notes::{format_text, rename_note_text};
use crate::app::ui::context::{Entry, Target};

fn ids(entries: &[Entry], out: &mut Vec<String>) {
    for entry in entries {
        match entry {
            Entry::Item { id, .. } => out.push(id.clone()),
            Entry::Sub { items, .. } => ids(items, out),
            Entry::Sep => {}
        }
    }
}

#[test]
fn markdown_buttons_wrap_the_selection_or_take_the_caret() {
    assert_eq!(format_text("abc", 0, 3, "**"), ("**abc**".into(), 2, 5));
    assert_eq!(format_text("abc", 1, 1, "**"), ("a****bc".into(), 3, 3));
    // the same mark around the selection again takes it off
    assert_eq!(format_text("**abc**", 2, 5, "**"), ("abc".into(), 0, 3));
    // line marks toggle on the lines the selection touches
    assert_eq!(format_text("um\ndois", 4, 4, "- "), ("um\n- dois".into(), 9, 9));
    assert_eq!(format_text("- um", 2, 2, "- "), ("um".into(), 2, 2));
    assert_eq!(format_text("# T", 0, 0, "## "), ("## T".into(), 4, 4));
    assert_eq!(rename_note_text("lista\nmais", "Compras"), "# Compras\nmais");
}

#[test]
fn an_empty_note_can_be_typed_into_after_leaving_it() {
    let ctx = Context::default();
    install_fonts(&ctx, false);
    let mut app = DrawingApp::default();
    frame(&mut app, &ctx, vec![]);
    app.tool = Tool::Sticky;
    drag(&mut app, &ctx, Pos2::new(400.0, 300.0), Pos2::new(401.0, 300.0));
    // leaving an empty note drops its empty text; Enter on the note brings the text back and edits it
    app.finish_text();
    assert_eq!(app.scene.elements.len(), 1);
    app.begin_text_edit(0);
    assert_eq!(app.scene.elements.len(), 2);
    frame(&mut app, &ctx, vec![]);
    frame(&mut app, &ctx, vec![egui::Event::Text("Oi".into())]);
    let ElementKind::Text { value, .. } = &app.scene.elements[1].kind else { panic!() };
    assert_eq!(value, "Oi");
    // and so does a double-click on it
    app.scene.elements[1] = app.new_element(ElementKind::Text { x: 0.0, y: 0.0, value: String::new() });
    app.scene.elements[1].id = format!("{}-label", app.scene.elements[0].id);
    app.finish_text();
    app.selected.clear();
    app.scene.elements.truncate(1);
    app.tool = Tool::Select;
    let at = app.to_screen(element_bounds(&app.scene.elements[0]).center());
    double_click(&mut app, &ctx, at, 1.0);
    assert!(app.editing.is_some(), "a double-click on an empty note edits it");
}

#[test]
fn the_custom_color_picker_stays_open_and_changes_the_note() {
    let ctx = Context::default();
    install_fonts(&ctx, false);
    let mut app = DrawingApp::default();
    let [note, text] = app.new_note(Rect::from_min_size(Pos2::new(300.0, 300.0), Vec2::splat(200.0)), "Oi".into());
    app.scene.elements = vec![note, text];
    app.selected = vec![0];
    let before = app.scene.elements[0].style.fill;
    app.note_color = Some((Pos2::new(100.0, 100.0), Rect::NOTHING));
    let mut run = |app: &mut DrawingApp, events: Vec<egui::Event>| {
        let input = egui::RawInput { screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(800.0, 600.0))), events, ..Default::default() };
        let mut output = ctx.run_ui(input, |ui| {
            let _ = ui;
            app.note_color_ui(&ctx);
        });
        output.textures_delta.clear();
    };
    run(&mut app, vec![]);
    let popover = app.note_color.expect("open").1;
    assert!(popover.is_positive(), "{popover:?}");
    // a press and a drag deep inside the popover (the picker's square) neither close it nor miss the note
    let at = popover.min + Vec2::new(70.0, 175.0);
    run(&mut app, vec![egui::Event::PointerMoved(at)]);
    run(&mut app, vec![button(at, true)]);
    run(&mut app, vec![egui::Event::PointerMoved(at + Vec2::new(20.0, -10.0))]);
    run(&mut app, vec![button(at, false)]);
    assert!(app.note_color.is_some(), "the picker stays open");
    assert_ne!(app.scene.elements[0].style.fill, before, "dragging on the square changes the paper");
}

#[test]
fn notes_stack_into_a_pile_flip_move_together_and_spill() {
    let mut app = DrawingApp::default();
    let mut cards = Vec::new();
    for (k, text) in ["um", "dois", "tres"].into_iter().enumerate() {
        let [note, label] = app.new_note(Rect::from_min_size(Pos2::new(100.0 + 300.0 * k as f32, 100.0), Vec2::splat(200.0)), text.into());
        cards.push(app.scene.elements.len());
        app.scene.elements.extend([note, label]);
    }
    app.selected = cards.clone();
    assert!(app.stack_notes());
    // only the first (reading order) shows, every page sits on it
    assert_eq!(app.selected, vec![cards[0]]);
    let origin = element_bounds(&app.scene.elements[cards[0]]).min;
    assert!(cards.iter().all(|c| element_bounds(&app.scene.elements[*c]).min == origin));
    let hidden: Vec<bool> = app.scene.elements.iter().map(|e| hidden_page(&app.scene.elements, e)).collect();
    assert_eq!(hidden, [false, false, true, true, true, true]);
    assert!(app.is_card_text(cards[1]) && !app.is_card_text(cards[0]));
    // a click finds the front only, and the pile moves with all its pages
    assert_eq!(app.hit(origin + Vec2::splat(100.0)), Some(cards[0]));
    assert_eq!(app.with_card_text(&[cards[0]]).len(), 6);
    // flipping shows the next page and wraps
    assert!(app.flip_stack(cards[0], 1));
    assert_eq!(app.hit(origin + Vec2::splat(100.0)), Some(cards[1]));
    assert!(app.flip_stack(cards[1], -1) && app.flip_stack(cards[0], -1));
    assert_eq!(app.selected, vec![cards[2]]);
    // dropping a loose note on the pile joins it
    let [note, label] = app.new_note(Rect::from_min_size(Pos2::new(900.0, 900.0), Vec2::splat(200.0)), "quatro".into());
    app.scene.elements.extend([note, label]);
    let loose = app.scene.elements.len() - 2;
    app.selected = vec![loose];
    app.scene.elements[loose].move_by(origin - Pos2::new(900.0, 900.0) + Vec2::splat(20.0));
    app.drop_into_stack();
    assert_eq!(app.stack_pages(loose).len(), 4);
    // spilling brings every page back, 24 px apart, as loose notes
    assert!(app.unstack(loose));
    assert!(app.scene.elements.iter().all(|e| !e.note.stacked));
    let pages = app.selected.clone();
    assert_eq!(pages.len(), 4);
    assert_eq!(element_bounds(&app.scene.elements[pages[1]]).min - element_bounds(&app.scene.elements[pages[0]]).min, Vec2::splat(24.0));
    // the pile survives the file
    app.selected = pages;
    assert!(app.stack_notes());
    let back = crate::tests::round_trip(&app.scene.elements);
    assert_eq!(back.iter().filter(|e| e.note.stacked).count(), 4);
    assert_eq!(back.iter().filter(|e| e.note.front).count(), 1);
    // a hidden page stays out of the export
    let svg = scene_svg(&app.scene, None);
    assert!(svg.matches("<text").count() <= 1, "{svg}");
    // and the MCP drives it by note name
    assert!(app.act_note_stack("next", &["um".into()]).is_ok());
    assert!(app.act_note_stack("unstack", &["um".into()]).is_ok());
    assert!(app.act_note_stack("unstack", &["um".into()]).is_err());
}

#[test]
fn group_rows_need_two_elements_and_ungroup_a_group() {
    let mut app = DrawingApp::default();
    let mut image = app.new_element(ElementKind::Rectangle { x: 0.0, y: 0.0, width: 50.0, height: 50.0 });
    image.image = Some("data:image/png;base64,".into());
    let id = image.id.clone();
    let other = app.new_element(ElementKind::Rectangle { x: 100.0, y: 0.0, width: 50.0, height: 50.0 });
    app.scene.elements = vec![image, other];
    let shown = |app: &DrawingApp| {
        let mut shown = Vec::new();
        ids(&app.context_entries(&Target::Element(id.clone())), &mut shown);
        shown
    };
    app.selected = vec![0, 0];
    assert!(!shown(&app).iter().any(|s| s == "group" || s == "ungroup"), "{:?}", shown(&app));
    app.selected = vec![0, 1];
    assert!(shown(&app).contains(&"group".to_string()) && !shown(&app).contains(&"ungroup".to_string()));
    app.group_selection();
    assert!(shown(&app).contains(&"ungroup".to_string()));
}

#[test]
fn a_right_click_on_a_terminal_opens_its_menu() {
    let ctx = Context::default();
    let mut app = DrawingApp::default();
    frame(&mut app, &ctx, vec![]);
    let mut shell = app.new_element(ElementKind::Rectangle { x: 100.0, y: 100.0, width: 400.0, height: 300.0 });
    shell.terminal = Some(TerminalSpec { name: "Shell".into(), ..Default::default() });
    let id = shell.id.clone();
    app.scene.elements.push(shell);
    let at = app.to_screen(Pos2::new(300.0, 250.0));
    let secondary = |pressed| egui::Event::PointerButton { pos: at, button: egui::PointerButton::Secondary, pressed, modifiers: Default::default() };
    frame(&mut app, &ctx, vec![egui::Event::PointerMoved(at), secondary(true)]);
    let menu = app.context.as_ref().expect("the right click opens a menu");
    assert_eq!(menu.target, Target::Element(id.clone()));
    let mut shown = Vec::new();
    ids(&app.context_entries(&menu.target), &mut shown);
    // the reference app's order
    let order =
        ["copy_sel", "paste_text", "select_all_text", "rename", "edit", "monitor", "blur", "reload", "unload", "use_size", "copy_el", "dup", "lock", "delete"];
    let at: Vec<usize> = order.iter().map(|w| shown.iter().position(|s| s == w).unwrap_or_else(|| panic!("terminal menu has {w}: {shown:?}"))).collect();
    assert!(at.windows(2).all(|w| w[0] < w[1]), "terminal menu order: {shown:?}");
    // Desfocar hides its screen and takes the keys from it
    app.terminal_focus = Some(id.clone());
    app.context_action(&ctx, &Target::Element(id.clone()), "blur");
    assert_eq!(app.terminal_focus, None);
    assert!(app.scene.elements[0].terminal.as_ref().unwrap().look.blurred);
}

/// The title strip of a file tree is the canvas's: a right click there opens the canvas's menu, and
/// drawing it once read the frame number inside `data_mut` (egui's lock taken twice), which stopped
/// the app for good (in a debug build egui panics after 10 s instead).
#[test]
fn a_right_click_on_a_file_trees_title_draws_its_menu() {
    let ctx = Context::default();
    let mut app = DrawingApp::default();
    frame(&mut app, &ctx, vec![]);
    let mut tree = app.new_element(ElementKind::Rectangle { x: 100.0, y: 100.0, width: 400.0, height: 300.0 });
    tree.filetree = Some(Default::default());
    let id = tree.id.clone();
    app.scene.elements.push(tree);
    let at = app.to_screen(Pos2::new(300.0, 110.0));
    let secondary = |pressed| egui::Event::PointerButton { pos: at, button: egui::PointerButton::Secondary, pressed, modifiers: Default::default() };
    frame(&mut app, &ctx, vec![egui::Event::PointerMoved(at), secondary(true)]);
    assert_eq!(app.context.as_ref().map(|m| m.target.clone()), Some(Target::Element(id)));
    for _ in 0..2 {
        ctx.run_ui(Default::default(), |_| app.context_ui(&ctx)).textures_delta.clear();
    }
    let menu = app.context.as_ref().expect("the menu stays open, drawn");
    let mut shown = Vec::new();
    ids(&app.context_entries(&menu.target), &mut shown);
    assert!(shown.starts_with(&["show_folder".to_string(), "edit".to_string()]), "{shown:?}");
}

#[test]
fn duplicate_from_the_menu_makes_a_copy() {
    let ctx = Context::default();
    let mut app = DrawingApp::default();
    let rect = app.new_element(ElementKind::Rectangle { x: 0.0, y: 0.0, width: 50.0, height: 50.0 });
    let id = rect.id.clone();
    app.scene.elements.push(rect);
    app.selected = vec![0];
    app.context_action(&ctx, &Target::Element(id), "dup");
    assert_eq!(app.scene.elements.len(), 2);
    assert_ne!(app.scene.elements[0].id, app.scene.elements[1].id);
}

#[test]
fn a_click_on_a_note_focuses_it_and_typing_goes_in() {
    let ctx = Context::default();
    let mut app = DrawingApp::default();
    frame(&mut app, &ctx, vec![]);
    let [note, label] = app.new_note(Rect::from_min_size(Pos2::new(60.0, 60.0), Vec2::splat(260.0)), "# Plano".into());
    app.scene.elements.extend([note, label]);
    let at = app.to_screen(Pos2::new(150.0, 150.0));
    click(&mut app, &ctx, at);
    assert_eq!(app.editing, Some(1), "the click puts the note's text in focus");
    assert_eq!(app.selected, vec![0]);
}

#[test]
fn a_click_types_into_a_note_only_on_its_text_and_without_a_modifier() {
    let ctx = Context::default();
    let mut app = DrawingApp::default();
    frame(&mut app, &ctx, vec![]);
    // off the Mac the canvas starts under the title bar, which these frames leave out
    app.pan = Vec2::new(0.0, TITLE_BAR);
    let [note, label] = app.new_note(Rect::from_min_size(Pos2::new(60.0, 60.0), Vec2::splat(260.0)), "uma linha".into());
    let id = note.id.clone();
    app.scene.elements.extend([note, label]);
    // as painted (these frames don't paint): its text ends 80 below the note's top
    app.note_text_bottom.borrow_mut().insert(id, 140.0);
    let below = app.to_screen(Pos2::new(150.0, 250.0));
    click(&mut app, &ctx, below);
    assert_eq!((app.selected.clone(), app.editing), (vec![0], None), "below the text: only selected");
    let header = app.to_screen(Pos2::new(150.0, 66.0));
    click(&mut app, &ctx, header);
    assert_eq!(app.editing, None, "the header doesn't type");
    // under the 44 px strip that picks up a note at rest
    let on_text = app.to_screen(Pos2::new(150.0, 120.0));
    for (modifiers, name) in [(Modifiers::CTRL, "Ctrl"), (Modifiers::SHIFT, "Shift"), (Modifiers::ALT, "Alt")] {
        let press = |pressed| egui::Event::PointerButton { pos: on_text, button: egui::PointerButton::Primary, pressed, modifiers };
        frame(&mut app, &ctx, vec![egui::Event::PointerMoved(on_text), press(true)]);
        frame(&mut app, &ctx, vec![press(false)]);
        assert_eq!(app.editing, None, "nor a click with {name}");
    }
    click(&mut app, &ctx, on_text);
    assert_eq!(app.editing, Some(1), "a plain click on the text does");
}

#[test]
fn bold_wraps_the_selected_text_of_the_note() {
    let ctx = Context::default();
    let mut app = DrawingApp::default();
    frame(&mut app, &ctx, vec![]);
    let [note, label] = app.new_note(Rect::from_min_size(Pos2::new(60.0, 60.0), Vec2::splat(260.0)), "abc".into());
    app.scene.elements.extend([note, label]);
    app.selected = vec![0];
    // the click on the bar has already taken the keys from the editor: what counts is the
    // selection it had while it held them
    app.editing = None;
    app.note_selection = Some((1, 0, 3));
    app.note_format(&ctx, 0, "**");
    let ElementKind::Text { value, .. } = &app.scene.elements[1].kind else { panic!("the note's label") };
    assert_eq!(value, "**abc**");
}

/// A note over (60, 60) 300×200 with a terminal "Shell" elsewhere and these buttons; the note's index is 0.
fn note_with_buttons(app: &mut DrawingApp, buttons: Vec<NoteButton>) {
    app.pan = Vec2::new(0.0, TITLE_BAR);
    let [mut note, label] = app.new_note(Rect::from_min_size(Pos2::new(60.0, 60.0), Vec2::new(300.0, 200.0)), "# Painel".into());
    note.note.buttons = buttons;
    let mut shell = app.new_element(ElementKind::Rectangle { x: 500.0, y: 100.0, width: 400.0, height: 300.0 });
    shell.terminal = Some(TerminalSpec { name: "Shell".into(), ..Default::default() });
    app.scene.elements.extend([note, label, shell]);
}

/// A click on button `k` of note 0, as the pointer does it.
fn click_button(app: &mut DrawingApp, ctx: &Context, k: usize) {
    let note = &app.scene.elements[0];
    let b = local_bounds(note);
    let (rects, h) = crate::app::note_buttons::layout(&note.note.buttons, b.width() - 2.0 * NOTE_PAD);
    let at = app.to_screen(b.min + Vec2::new(NOTE_PAD, b.height() - NOTE_PAD - h) + rects[k].center().to_vec2());
    frame(app, ctx, vec![egui::Event::PointerMoved(at)]);
    frame(app, ctx, vec![button(at, true)]);
    frame(app, ctx, vec![button(at, false)]);
}

#[test]
fn a_note_button_runs_its_action_and_a_bad_one_only_warns() {
    let ctx = Context::default();
    let mut app = DrawingApp::default();
    frame(&mut app, &ctx, vec![]);
    let bad = NoteButton { label: "Quebrado".into(), action: "nao_existe".into(), args: Value::Null };
    let close = NoteButton { label: "Fechar".into(), action: "close_window".into(), args: json!({ "name": "Shell" }) };
    note_with_buttons(&mut app, vec![bad.clone(), close.clone()]);
    frame(&mut app, &ctx, vec![]);
    assert!(app.scene.elements.iter().any(|e| e.terminal.is_some()), "loading a scene runs nothing");
    click_button(&mut app, &ctx, 0);
    assert!(app.status.contains("nao_existe"), "the error goes to the toast: {}", app.status);
    assert_eq!(app.scene.elements.len(), 3, "and nothing else happens");
    click_button(&mut app, &ctx, 1);
    assert!(app.scene.elements.iter().all(|e| e.terminal.is_none()), "the click ran close_window through the dispatcher");
    assert_eq!(crate::app::note_buttons::describe(&close), "close_window {\"name\":\"Shell\"}");
}

#[test]
fn a_note_button_can_be_removed_and_buttons_survive_save_and_reopen() {
    let mut app = DrawingApp::default();
    let one = |label: &str| NoteButton { label: label.into(), action: "chat_list".into(), args: Value::Null };
    note_with_buttons(&mut app, vec![one("A"), one("B")]);
    app.remove_note_button(0, 0);
    assert_eq!(app.scene.elements[0].note.buttons, vec![one("B")]);
    let dir = std::env::temp_dir().join(format!("mago-note-buttons-{}", new_id()));
    let path = dir.join("cena.mago");
    write_scene_file(&path, &app.scene).unwrap();
    let (back, _) = read_scene_file(&path).unwrap();
    assert_eq!(back.elements[0].note.buttons, vec![one("B")]);
    let _ = fs::remove_dir_all(dir);
}

#[test]
fn a_note_with_buttons_grows_its_floor() {
    let mut app = DrawingApp::default();
    let long = |label: &str| NoteButton { label: label.into(), action: "chat_list".into(), args: Value::Null };
    let [mut note, _] = app.new_note(Rect::from_min_size(Pos2::ZERO, Vec2::new(250.0, 160.0)), String::new());
    assert_eq!(note.min_size(), Some(NOTE_MIN));
    note.note.buttons = vec![long("Abrir o projeto"), long("Ver o relatório"), long("Rodar os testes")];
    let floor = note.min_size().unwrap();
    assert!(floor.y > NOTE_MIN.y + 40.0, "three wide buttons wrap onto rows: {floor:?}");
}

#[test]
fn a_note_paper_brings_its_ink_and_old_colors_turn_into_the_nearest_paper() {
    let mut app = DrawingApp::default();
    let [mut note, _] = app.new_note(Rect::from_min_size(Pos2::ZERO, NOTE_SIZE), String::new());
    for (k, (paper, _)) in NOTE_PAPERS.iter().enumerate() {
        note.style.fill = Some(*paper);
        let [r, g, b] = NOTE_INK[k];
        assert_eq!(note_colors(&note, 100).1, Color32::from_rgb(r, g, b));
    }
    // a scene from before: a sticky note with its own yellow and no settings opens as a note of the new kind
    let old = json!([
        { "id": "n", "type": "stickynote", "x": 10, "y": 10, "width": 200, "height": 200, "backgroundColor": "#ffdf6b", "created": 1700000000000i64, "groupIds": ["g"] },
        { "id": "n-label", "type": "text", "x": 20, "y": 20, "text": "texto antigo", "groupIds": ["g"] },
    ]);
    let elements = from_interop_elements(&old);
    assert_eq!(elements[0].style.fill, Some([0xff, 0xe0, 0xb2, 255]), "nearest paper");
    assert_eq!(elements[0].note, NoteSpec::default());
    assert!(matches!(&elements[1].kind, ElementKind::Text { value, .. } if value == "texto antigo"));
    assert_eq!(note_label(&elements, 0), Some(1));
}

#[test]
fn each_note_is_a_markdown_file_beside_the_scene() {
    let mut app = DrawingApp::default();
    let [note, label] = app.new_note(Rect::from_min_size(Pos2::ZERO, NOTE_SIZE), "# Plano\n- um".into());
    let md = format!("{}.md", note.id);
    app.scene.elements.extend([note, label]);
    let dir = std::env::temp_dir().join(format!("mago-note-files-{}", new_id()));
    let path = dir.join("cena.mago");
    write_scene_file(&path, &app.scene).unwrap();
    let file = dir.join("notas").join(md);
    assert_eq!(fs::read_to_string(&file).unwrap(), "# Plano\n- um");
    // a file written in the notes folder after the scene (edited outside) is what the note reads back
    fs::write(&file, "# Plano\n- dois").unwrap();
    let later = SystemTime::now() + std::time::Duration::from_secs(30);
    fs::File::options().write(true).open(&file).unwrap().set_modified(later).unwrap();
    let (back, _) = read_scene_file(&path).unwrap();
    assert!(matches!(&back.elements[1].kind, ElementKind::Text { value, .. } if value == "# Plano\n- dois"));
    let _ = fs::remove_dir_all(dir);
}

#[test]
fn a_dragged_note_takes_the_dragged_size_and_the_color_menu_sets_the_ink() {
    let ctx = Context::default();
    let mut app = DrawingApp::default();
    frame(&mut app, &ctx, vec![]);
    app.tool = Tool::Sticky;
    drag(&mut app, &ctx, Pos2::new(400.0, 300.0), Pos2::new(740.0, 440.0));
    assert_eq!(element_bounds(&app.scene.elements[0]).size(), Vec2::new(340.0, 140.0), "not squared");
    app.finish_text();
    let id = app.scene.elements[0].id.clone();
    app.context_action(&ctx, &Target::Element(id), "color:2");
    assert_eq!(app.scene.elements[0].style.fill, Some(NOTE_PAPERS[2].0));
    assert_eq!(note_colors(&app.scene.elements[0], 100).1, Color32::from_rgb(0x0d, 0x47, 0xa1), "blue paper, its own ink");
}

#[test]
fn the_note_and_canvas_menus_follow_the_reference_app() {
    let ctx = Context::default();
    let mut app = DrawingApp::default();
    let [note, label] = app.new_note(Rect::from_min_size(Pos2::new(60.0, 60.0), Vec2::splat(260.0)), "# Plano".into());
    let id = note.id.clone();
    app.scene.elements.extend([note, label]);
    let mut shown = Vec::new();
    ids(&app.context_entries(&Target::Element(id)), &mut shown);
    let order = ["rename", "lock_content", "copy_content", "show_folder", "copy_el", "dup", "color:0", "lock", "delete"];
    let at: Vec<usize> = order.iter().map(|w| shown.iter().position(|s| s == w).unwrap_or_else(|| panic!("note menu has {w}: {shown:?}"))).collect();
    assert!(at.windows(2).all(|w| w[0] < w[1]), "note menu order: {shown:?}");
    // without presets "Terminal" is a plain row that opens the new terminal sheet
    app.prefs.presets.iter_mut().for_each(|p| p.on = false);
    let mut canvas = Vec::new();
    ids(&app.context_entries(&Target::Canvas(Pos2::ZERO)), &mut canvas);
    assert_eq!(canvas[0], "add_terminal");
    app.context_action(&ctx, &Target::Canvas(Pos2::ZERO), "add_terminal");
    assert!(app.terminal_dialog.is_some());
}
