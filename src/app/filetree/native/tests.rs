use super::doc::{byte_of, compare_out, spans, tokens_of};
use super::snapshot::kind_id;
use crate::app::filetree::*;
use crate::prelude::*;

fn temp(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("magoagent-native-filetree-{name}-{}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    dir
}

/// A tree on `root`, listed and with the row of each open folder built.
fn tree_on(app: &mut DrawingApp, root: &Path) -> String {
    app.act_file_tree(&json!({ "action": "create", "path": root.to_string_lossy() })).unwrap();
    let id = app.scene.elements.iter().find(|e| e.filetree.is_some()).unwrap().id.clone();
    app.describe_tree(&id);
    id
}

fn act(app: &mut DrawingApp, verb: &str, mut args: Value, id: &str) -> Result<String, String> {
    args["id"] = json!(id);
    app.native_filetree_act(verb, &args)
}

#[test]
fn the_snapshot_has_the_rows_with_their_depth_icons_and_state() {
    let dir = temp("rows");
    fs::create_dir_all(dir.join("src")).unwrap();
    fs::write(dir.join("src/main.rs"), "fn main() {}\n").unwrap();
    fs::write(dir.join("notes.md"), "# a\n").unwrap();
    let mut app = DrawingApp::default();
    let id = tree_on(&mut app, &dir);
    act(&mut app, "toggle", json!({ "path": dir.join("src").to_string_lossy() }), &id).unwrap();
    app.describe_tree(&id);
    let state = serde_json::to_value(app.native_tree_state(&id, false, &Default::default()).unwrap()).unwrap();
    let rows = state["rows"].as_array().unwrap();
    let names: Vec<(&str, u64, bool)> =
        rows.iter().map(|r| (r["name"].as_str().unwrap(), r["depth"].as_u64().unwrap(), r["open"].as_bool().unwrap())).collect();
    assert_eq!(names, [("src", 0, true), ("main.rs", 1, false), ("notes.md", 0, false)]);
    assert_eq!(rows[0]["icon"]["key"], "nodes_folder");
    assert_eq!(rows[2]["icon"]["key"], "fileTypes_markdown");
    // no svg for rust: a badge with its letters
    assert_eq!(rows[1]["icon"]["label"], "R");
    assert_eq!(state["editorOpen"], false);
    assert_eq!(state["sorts"].as_array().unwrap().len(), 5);
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn the_verbs_select_open_create_and_rename_through_the_trees_own_functions() {
    let dir = temp("verbs");
    fs::write(dir.join("a.txt"), "um\n").unwrap();
    let mut app = DrawingApp::default();
    let id = tree_on(&mut app, &dir);
    // a click selects; with the editor shown, it also previews the file
    act(&mut app, "editor", json!({ "show": true }), &id).unwrap();
    act(&mut app, "select", json!({ "path": dir.join("a.txt").to_string_lossy() }), &id).unwrap();
    let ed = &app.filetrees.editors[&id];
    assert_eq!(ed.active.as_deref(), Some(dir.join("a.txt").as_path()));
    assert_eq!(ed.preview.as_deref(), Some(dir.join("a.txt").as_path()));
    let made = act(&mut app, "create", json!({ "dir": dir.to_string_lossy(), "name": "b.txt" }), &id).unwrap();
    assert!(Path::new(&made).is_file());
    let moved = act(&mut app, "rename", json!({ "path": made, "name": "c.txt" }), &id).unwrap();
    assert_eq!(Path::new(&moved).file_name().unwrap(), "c.txt");
    assert!(!dir.join("b.txt").exists());
    // what changes the disk stays inside the tree
    let outside = temp("outside");
    assert!(act(&mut app, "create", json!({ "dir": outside.to_string_lossy(), "name": "x" }), &id).is_err());
    assert!(act(&mut app, "bogus", json!({}), &id).unwrap_err().contains("desconhecido"));
    let _ = fs::remove_dir_all(&dir);
    let _ = fs::remove_dir_all(&outside);
}

#[test]
fn an_edit_in_utf16_units_goes_into_the_buffer_which_stays_dirty_until_saved() {
    let dir = temp("edit");
    let file = dir.join("t.txt");
    fs::write(&file, "ola 😀 fim\n").unwrap();
    let mut app = DrawingApp::default();
    let id = tree_on(&mut app, &dir);
    act(&mut app, "activate", json!({ "path": file.to_string_lossy() }), &id).unwrap();
    // "ola " is 4 units and the emoji 2: "fim" starts at 7
    let edit = |app: &mut DrawingApp, from: u64, to: u64, text: &str, rev: u64| {
        act(app, "edit", json!({ "path": file.to_string_lossy(), "from": from, "to": to, "text": text, "rev": rev }), &id)
    };
    edit(&mut app, 7, 10, "FIM", 1).unwrap();
    assert!(app.filetrees.editors[&id].dirty());
    assert_eq!(app.filetrees.native.trees[&id].applied_rev, 1);
    // the middle of the emoji is not a place: refused, and the next frame sends the text again
    assert!(edit(&mut app, 5, 5, "x", 2).is_err());
    assert_eq!(app.filetrees.native.trees[&id].text_hash, 0);
    act(&mut app, "save", json!({}), &id).unwrap();
    assert_eq!(fs::read_to_string(&file).unwrap(), "ola 😀 FIM\n");
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn utf16_offsets_map_to_bytes_and_the_spans_count_in_utf16() {
    assert_eq!(byte_of("a😀b", 1), Some(1));
    assert_eq!(byte_of("a😀b", 2), None);
    assert_eq!(byte_of("a😀b", 3), Some(5));
    assert_eq!(byte_of("a😀b", 4), Some(6));
    assert_eq!(byte_of("a😀b", 5), None);
    let text = "/* 😀 */ fn x() {}\n";
    let tokens = tokens_of(text, Path::new("a.rs"));
    let (list, range) = spans(text, &tokens, (0, 1));
    assert_eq!(range, [0, text.encode_utf16().count()]);
    let triples: Vec<(u32, u32, u32)> = list.chunks(3).map(|t| (t[0], t[1], t[2])).collect();
    // the comment: 8 units though 9 bytes, then `fn` right after the space
    assert_eq!(triples[0], (0, 8, kind_id(crate::editor::highlight::St::Comment)));
    assert!(triples.contains(&(9, 11, kind_id(crate::editor::highlight::St::Keyword))), "{triples:?}");
}

#[test]
fn a_big_text_is_colored_only_around_the_lines_in_view() {
    let text = "let x = 1;\n".repeat(20_000);
    let tokens = tokens_of(&text, Path::new("a.rs"));
    let (list, range) = spans(&text, &tokens, (10_000, 10_040));
    assert!(range[0] > 0 && range[1] < text.len(), "{range:?}");
    assert!(list.len() < 3 * 6 * 700, "{}", list.len());
    assert!(list.chunks(3).all(|t| t[0] as usize >= range[0] && t[1] as usize <= range[1]));
}

#[test]
fn the_compare_rows_pair_the_lines_and_mark_the_changed_words() {
    let compare =
        crate::app::filetree::compare::Compare::new(Path::new("a.txt"), "one\ntwo words here\nthree\n".into(), "one\ntwo other here\nthree\nfour\n".into());
    let out = serde_json::to_value(compare_out(&compare)).unwrap();
    let rows = out["rows"].as_array().unwrap();
    let kinds: Vec<&str> = rows.iter().map(|r| r["kind"].as_str().unwrap()).collect();
    assert_eq!(kinds, ["same", "change", "same", "insert"]);
    assert_eq!(rows[1]["left"], "two words here");
    assert_eq!(rows[1]["right"], "two other here");
    assert_eq!(rows[1]["leftWords"], json!([[4, 9]]));
    assert_eq!(rows[3]["right"], "four");
    assert_eq!(rows[3]["left"], Value::Null);
    assert_eq!(out["blocks"], json!([1, 3]));
    assert_eq!((out["removed"].as_u64(), out["added"].as_u64()), (Some(1), Some(2)));
}

#[test]
fn the_link_back_button_is_published_on_its_tree_and_both_verbs_take_it_away() {
    let dir = temp("link-back");
    fs::write(dir.join("a.txt"), "um\n").unwrap();
    let mut app = DrawingApp::default();
    let id = tree_on(&mut app, &dir);
    let words = |app: &DrawingApp| serde_json::to_value(app.native_tree_state(&id, false, &Default::default()).unwrap()).unwrap()["linkBack"].clone();
    assert!(words(&app).is_null());
    let back = |chat_open| link::LinkReturn { chat_open, chat_agent: None, pan: Vec2::ZERO, zoom: 1.0, elevated: None, card_focus: None, tree: id.clone() };
    app.filetrees.link_back = Some(back(true));
    assert_eq!(words(&app), "Voltar ao chat");
    act(&mut app, "link_back", json!({}), &id).unwrap();
    assert!(app.filetrees.link_back.is_none() && words(&app).is_null());
    // going back to the chat opens it: the button has no place there
    app.chat_open = false;
    app.filetrees.link_back = Some(back(false));
    assert_eq!(words(&app), "Voltar");
    act(&mut app, "link_dismiss", json!({}), &id).unwrap();
    assert!(app.filetrees.link_back.is_none());
    let _ = fs::remove_dir_all(&dir);
}
