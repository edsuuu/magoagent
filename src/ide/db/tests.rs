use super::*;

struct Database { dir: PathBuf, file: PathBuf }
impl Database {
    fn new() -> Self {
        let dir = std::env::temp_dir().join(format!("mago-db-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir(&dir).unwrap();
        Self { file: dir.join("teste.sqlite"), dir }
    }
}
impl Drop for Database {
    fn drop(&mut self) { let _ = std::fs::remove_dir_all(&self.dir); }
}
fn text(s: &str) -> Cell { Cell::Text(s.into()) }

#[test]
fn the_tree_hears_back_with_the_database_panel_closed() {
    let ctx = egui::Context::default();
    let missing = std::env::temp_dir().join(format!("mago-db-{}", uuid::Uuid::new_v4()));
    let mut ide = Ide::new(ctx.clone(), "db", &missing.to_string_lossy());
    ide.left = None;
    let (tx, rx) = std::sync::mpsc::channel();
    ide.db.loading.insert("s".into(), rx);
    tx.send(Job::Schema(Err("sem rede".into()))).unwrap();
    ide.background(&ctx);
    assert!(ide.db.loading.is_empty());
    assert_eq!(ide.db.error.as_deref(), Some("sem rede"));
}

#[test]
fn sqlite_dml_paging_submit_and_rollback_in_temporary_database() {
    let db = Database::new();
    let file = &db.file;
    execute(file, "CREATE TABLE users(id INTEGER PRIMARY KEY, name TEXT NOT NULL, amount NUMERIC, optional TEXT, raw BLOB, doubled INT GENERATED ALWAYS AS (id*2));
        CREATE INDEX user_name ON users(name);
        CREATE TABLE child(id INTEGER PRIMARY KEY, user_id INT NOT NULL REFERENCES users(id));
        CREATE TABLE composite(a TEXT, b INTEGER, value TEXT, PRIMARY KEY(a,b)) WITHOUT ROWID;
        CREATE TABLE shadow(rowid TEXT, value TEXT);
        CREATE TABLE unsafe(rowid, _rowid_, oid, value);
        WITH RECURSIVE n(x) AS (VALUES(1) UNION ALL SELECT x+1 FROM n WHERE x<1005)
        INSERT INTO users(id,name,amount,optional,raw) SELECT x,'Ana',1,NULL,X'00FF1E1F' FROM n;
        INSERT INTO composite VALUES ('O''Brien',7,'antes');
        INSERT INTO shadow VALUES ('sombra','antes');
        INSERT INTO child VALUES (1,1);").unwrap();
    assert!(is_sqlite(file));
    let tables = schema(file).unwrap();
    let table = tables.iter().find(|t| t.name == "users").unwrap();
    assert!(table.columns[0].pk == 1 && table.columns[1].notnull && table.columns[5].generated);
    assert!(table.indexes.iter().any(|(name, cols)| name == "user_name" && cols == "name"));
    assert_eq!(tables.iter().find(|t| t.name == "child").unwrap().foreign, ["user_id → users.id"]);
    assert!(matches!(tables.iter().find(|t| t.name == "unsafe").unwrap().identity(), Identity::ReadOnly));
    let shadow = tables.iter().find(|t| t.name == "shadow").unwrap();
    assert!(matches!(shadow.identity(), Identity::Rowid(name) if name == "_rowid_"));

    let filter = "id > 2 AND name = 'Ana'";
    let first = load_page(file, table, filter, "id ASC", 0).unwrap();
    let next = load_page(file, table, filter, "id ASC", PAGE).unwrap();
    let last = load_page(file, table, filter, "id ASC", PAGE * 2).unwrap();
    assert_eq!((first.count, first.rows.len(), next.count, next.rows.len(), last.count, last.rows.len()), (1003,500,1003,500,1003,3));
    assert_eq!((first.rows[0].values[0].text(), next.rows[0].values[0].text(), last.rows[2].values[0].text()), ("3","503","1005"));
    let empty = load_page(file, table, "id < 0", "id DESC", 0).unwrap();
    assert_eq!((empty.count, empty.rows.len()), (0,0));
    let past_end = load_page(file, table, filter, "", 2000).unwrap();
    assert_eq!((past_end.count, past_end.rows.len()), (1003,0));

    let mut row = first.rows[0].clone();
    assert_eq!(row.values[3], Cell::Null);
    assert_eq!(row.values[4], Cell::Blob("00FF1E1F".into()));
    row.values[1] = text("O'Brien; DELETE FROM users; --");
    row.values[2] = text("-1.25e2");
    row.values[3] = text("NULL");
    let mut deleted = first.rows[1].clone();
    deleted.deleted = true;
    let new = DataRow { original: None, values: vec![Cell::Null, text("001"), text(".5"), Cell::Null, Cell::Blob("ABCD".into()), Cell::Null], rowid: Cell::Null, deleted: false };
    let rows = vec![row, deleted, new];
    let sql = dml(table, &rows).unwrap();
    assert!(sql.starts_with("BEGIN IMMEDIATE;\nUPDATE \"users\" SET"));
    assert!(sql.contains("\"name\" = 'O''Brien; DELETE FROM users; --'"));
    assert!(sql.contains("\"amount\" = -1.25e2") && sql.contains("\"optional\" = 'NULL'"));
    assert!(sql.contains("WHERE \"rowid\" = 3") && sql.contains("DELETE FROM \"users\" WHERE \"rowid\" = 4"));
    assert!(sql.contains("NULL, '001', .5, NULL, X'ABCD'") && !sql.contains("\"doubled\" ="));
    submit(file, table, &rows).unwrap();
    let changed = load_page(file, table, "id=3", "", 0).unwrap();
    assert_eq!(changed.rows[0].values[1], rows[0].values[1]);
    assert_eq!(changed.rows[0].values[2], text("-125"));
    assert_eq!(changed.rows[0].values[3], text("NULL"));
    assert_eq!(changed.rows[0].values[4], rows[0].values[4]);
    assert!(load_page(file, table, "id=4", "", 0).unwrap().rows.is_empty());
    assert_eq!(load_page(file, table, "name='001'", "", 0).unwrap().rows[0].values[3], Cell::Null);

    let before = changed.rows[0].clone();
    let mut update = before.clone(); update.values[1] = text("não deve ficar");
    let duplicate = DataRow { original: None, values: before.values.clone(), rowid: Cell::Null, deleted: false };
    assert!(submit(file, table, &[update, duplicate]).is_err());
    assert_eq!(load_page(file, table, "id=3", "", 0).unwrap().rows[0].values, before.values);
    let mut parent = load_page(file, table, "id=1", "", 0).unwrap().rows.remove(0);
    parent.deleted = true;
    assert!(submit(file, table, &[parent]).is_err());
    assert_eq!(load_page(file, table, "id=1", "", 0).unwrap().rows.len(), 1);

    let composite = tables.iter().find(|t| t.name == "composite").unwrap();
    assert!(matches!(composite.identity(), Identity::Primary(ref pk) if pk == &[0,1]));
    let mut row = load_page(file, composite, "", "", 0).unwrap().rows.remove(0);
    row.values[0] = text("nova chave"); row.values[2] = text("depois");
    let sql = dml(composite, &[row.clone()]).unwrap();
    assert!(sql.contains("WHERE \"a\" IS 'O''Brien' AND \"b\" IS 7"));
    submit(file, composite, &[row]).unwrap();
    let mut row = load_page(file, composite, "", "", 0).unwrap().rows.remove(0);
    assert_eq!(row.values[0], text("nova chave"));
    row.deleted = true;
    submit(file, composite, &[row]).unwrap();
    assert_eq!(load_page(file, composite, "", "", 0).unwrap().count, 0);

    let mut row = load_page(file, shadow, "", "", 0).unwrap().rows.remove(0);
    row.values[1] = text("depois");
    assert!(dml(shadow, &[row.clone()]).unwrap().contains("WHERE \"_rowid_\" = 1"));
    submit(file, shadow, &[row]).unwrap();
    execute(file, "INSERT INTO users(id,name) VALUES(9223372036854775807,'limite')").unwrap();
    let mut row = load_page(file, table, "name='limite'", "", 0).unwrap().rows.remove(0);
    row.values[1] = text("limite alterado");
    assert!(dml(table, &[row.clone()]).unwrap().contains("WHERE \"rowid\" = 9223372036854775807"));
    submit(file, table, &[row]).unwrap();

    // Quoted numeric-looking text, empty strings and control bytes keep their meaning.
    let mut row = before;
    row.values[1] = text("001"); row.values[2] = text("1e999"); row.values[3] = text("\0\x1e\x1f'á");
    let sql = dml(table, &[row.clone()]).unwrap();
    assert!(sql.contains("\"name\" = '001'") && sql.contains("\"amount\" = '1e999'") && sql.contains("CAST(X'"));
    row.values[2] = text("not a number");
    submit(file, table, &[row.clone()]).unwrap();
    assert_eq!(load_page(file, table, "id=3", "", 0).unwrap().rows[0].values[3], row.values[3]);
    row = load_page(file, table, "id=3", "", 0).unwrap().rows.remove(0); row.values[3] = text("");
    submit(file, table, &[row]).unwrap();
    assert_eq!(load_page(file, table, "id=3", "", 0).unwrap().rows[0].values[3], text(""));

    let grid = execute(file, "-- comentário inicial\nSELECT 'NULL' AS z, NULL AS a, '' AS z; SELECT 2 AS z, 1 AS a").unwrap();
    assert_eq!(grid.columns, ["z", "a"]);
    assert_eq!(grid.rows[0], [text("2"), text("1")]);
    let grid = query(file, "SELECT NULL AS a, 'NULL' AS b, '' AS c").unwrap();
    assert_eq!(grid.rows[0], [Cell::Null, text("NULL"), text("")]);
    assert!(query(file, "DELETE FROM users").is_err());
    assert!(query(file, ".shell echo oi").is_err());
    assert!(execute(file, ".shell echo oi").is_err());
    assert!(execute(file, &format!("ATTACH '{}' AS x; CREATE TABLE x.t(a)", db.dir.join("x.db").display())).is_err());
    assert!(execute(file, &format!("SELECT writefile('{}', 'x')", db.dir.join("w.txt").display())).is_err());
    assert!(execute(file, "SELECT load_extension('x')").is_err());
    assert!(execute(file, &format!("VACUUM INTO '{}'", db.dir.join("vacuum.sqlite").display())).is_err());
    let mapped = query(file, "SELECT 1.0, 0.5, X'00FF', 7").unwrap();
    assert_eq!(mapped.rows[0], [text("1.0"), text("0.5"), Cell::Blob("00FF".into()), text("7")]);
    assert!(query(file, "WITH RECURSIVE n(x) AS (VALUES(1) UNION ALL SELECT x+1 FROM n WHERE x<1000000) SELECT x, 'abcdefghij' FROM n").is_err_and(|e| e.contains("8 MiB")));
    assert!(sql::run(file, "WITH RECURSIVE n(x) AS (VALUES(1) UNION ALL SELECT x+1 FROM n) SELECT count(*) FROM n", false, Duration::from_millis(50)).is_err_and(|e| e.contains("interrompida")));
    assert_eq!(std::fs::read_dir(&db.dir).unwrap().count(), 1, "só a base temporária na pasta");
    // a missing or foreign file fails with a message, never hangs
    assert!(query(&db.dir.join("falta.sqlite"), "SELECT 1").is_err_and(|e| e.contains("Não foi possível abrir")));
    std::fs::write(db.dir.join("texto.sqlite"), "não é um banco").unwrap();
    assert!(query(&db.dir.join("texto.sqlite"), "SELECT 1").is_err());
}

#[test]
fn console_uses_selection_or_statement_at_unicode_cursor() {
    use super::view::instruction;
    let sql = "SELECT 'á;O''Brien'; -- ; ignorado\nSELECT \"b;c\", [d;e] /* ; */; SELECT 3";
    let cursor = sql.find("SELECT \"b").unwrap();
    let cursor = sql[..cursor].chars().count();
    assert_eq!(instruction(sql, (cursor, cursor)), "-- ; ignorado\nSELECT \"b;c\", [d;e] /* ; */;");
    assert_eq!(instruction(sql, (0, 8)), "SELECT '");
    let end = sql.chars().count();
    assert_eq!(instruction(sql, (end, end)), "SELECT 3");
    assert_eq!(instruction("SELECT 1;", (0,0)), "SELECT 1;");
    assert_eq!(instruction("SELECT 1;", (9,9)), "SELECT 1;");
    assert_eq!(instruction("SELECT 1;\n ", (11,11)), "SELECT 1;");
}
