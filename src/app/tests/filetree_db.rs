//! The Banco de Dados of a file tree: a SQLite file opened in the tree is a source, its tables list and open as tabs of the editor
//! pane, and the MCP reads it (and only reads).

use super::filetree::{temp, tree_frame, tree_on};
use super::*;
use crate::app::filetree::editor::Body;

/// A tree on a folder with `loja.sqlite` (two clients); its id and the file.
fn tree_with_db(name: &str) -> (DrawingApp, String, PathBuf) {
    let dir = temp(name);
    let file = dir.join("loja.sqlite");
    let conn = rusqlite::Connection::open(&file).unwrap();
    conn.execute_batch("CREATE TABLE clientes(id INTEGER PRIMARY KEY, nome TEXT); INSERT INTO clientes VALUES (1,'Ana'),(2,'Bia');").unwrap();
    drop(conn);
    let mut app = DrawingApp::default();
    // never the person's own connections.json
    app.filetrees.db_store = Some(dir.join("connections.json"));
    let id = tree_on(&mut app, &dir);
    (app, id, file)
}

#[test]
fn a_sqlite_file_opened_in_the_tree_is_a_source_of_the_panel() {
    let (mut app, id, file) = tree_with_db("db-open");
    assert!(!app.ft_spec(&id).unwrap().db);
    app.act_file_tree(&json!({ "action": "open", "path": file.to_string_lossy() })).unwrap();
    assert!(app.ft_spec(&id).unwrap().db, "the panel shows");
    assert!(!app.ft_spec(&id).unwrap().editor, "the file is not opened as text");
    let sources = app.act_file_tree(&json!({ "action": "db_sources" })).unwrap();
    assert!(sources.contains("loja.sqlite (SQLite, conectada)"), "{sources}");
    // opening it again does not make a second source
    app.ft_activate(&id, file.clone());
    assert_eq!(app.act_file_tree(&json!({ "action": "db_sources" })).unwrap().lines().count(), 1);
    assert!(app.act_file_tree(&json!({ "action": "list" })).unwrap().contains("Banco de Dados: aberto"));
    // the panel's own frame draws and asks nothing
    let ctx = Context::default();
    for i in 0..3 {
        tree_frame(&mut app, &ctx, i as f64 * 0.1, vec![]);
    }
}

#[test]
fn tables_list_and_open_as_tabs_of_the_editor_pane() {
    let (mut app, id, file) = tree_with_db("db-tabs");
    app.act_file_tree(&json!({ "action": "db_add", "path": file.to_string_lossy() })).unwrap();
    let tables = app.act_file_tree(&json!({ "action": "db_tables" })).unwrap();
    assert!(tables.contains("clientes") && tables.contains("nome"), "{tables}");
    app.act_file_tree(&json!({ "action": "db_open", "name": "clientes" })).unwrap();
    assert!(app.ft_spec(&id).unwrap().editor, "the pane shows for the grid");
    let tab = |app: &DrawingApp| app.filetrees.editors[&id].tabs.iter().filter(|t| matches!(t.body, Body::Sheet(_))).count();
    assert_eq!(tab(&app), 1);
    // the same table does not open twice; a console always does
    app.act_file_tree(&json!({ "action": "db_open", "name": "clientes" })).unwrap();
    assert_eq!(tab(&app), 1);
    app.act_file_tree(&json!({ "action": "db_open" })).unwrap();
    assert_eq!(tab(&app), 2);
    assert!(app.act_file_tree(&json!({ "action": "list" })).unwrap().contains("clientes [loja.sqlite]"));
    assert!(app.act_file_tree(&json!({ "action": "db_open", "name": "nada" })).is_err());
    let ctx = Context::default();
    for i in 0..5 {
        tree_frame(&mut app, &ctx, i as f64 * 0.1, vec![]);
    }
}

#[test]
fn the_mcp_runs_reads_and_refuses_writes() {
    let (mut app, _id, file) = tree_with_db("db-query");
    app.act_file_tree(&json!({ "action": "db_add", "path": file.to_string_lossy() })).unwrap();
    let read = app.act_file_tree(&json!({ "action": "db_query", "query": "SELECT nome FROM clientes ORDER BY id" })).unwrap();
    assert!(read.contains("Ana") && read.contains("Bia") && read.contains("2 linha(s)"), "{read}");
    let limited = app.act_file_tree(&json!({ "action": "db_query", "query": "select * from clientes", "limit": 1 })).unwrap();
    assert!(limited.contains("mostrando 1"), "{limited}");
    for write in [
        "DELETE FROM clientes",
        "INSERT INTO clientes VALUES (3,'Cris')",
        "UPDATE clientes SET nome='x'",
        "DROP TABLE clientes",
        "SELECT 1; DELETE FROM clientes",
        "WITH x AS (SELECT 1) DELETE FROM clientes",
        "PRAGMA writable_schema=1",
        "ATTACH 'outro.db' AS o",
    ] {
        assert!(app.act_file_tree(&json!({ "action": "db_query", "query": write })).is_err(), "{write}");
    }
    let rows = rusqlite::Connection::open(&file).unwrap().query_row("SELECT count(*) FROM clientes", [], |r| r.get::<_, i64>(0)).unwrap();
    assert_eq!(rows, 2, "nothing was written");
}

#[test]
fn only_reads_pass_the_guard() {
    use crate::db::read_only_sql;
    for ok in ["SELECT 1", " select * from t where a = 'x' ;", "WITH a AS (SELECT 1) SELECT * FROM a", "EXPLAIN SELECT 1"] {
        assert!(read_only_sql(ok).is_ok(), "{ok}");
    }
    for bad in ["", "DELETE FROM t", "SELECT * INTO copia FROM t", "SELECT 1; SELECT 2", "WITH a AS (DELETE FROM t RETURNING *) SELECT * FROM a"] {
        assert!(read_only_sql(bad).is_err(), "{bad}");
    }
}
