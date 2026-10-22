use super::*;
use session::{Object, ObjectKind};
fn text(s: &str) -> Cell { Cell::Text(s.into()) }
fn table() -> Table {
    Table { name: "pe\"ople".into(), schema: "te\"st".into(), dialect: Dialect::Postgres, columns: vec![
        Column { name: "id".into(), kind: "bigint".into(), pk: 1, auto_increment: true, ..Default::default() },
        Column { name: "part".into(), kind: "text".into(), pk: 2, ..Default::default() },
        Column { name: "na\"me".into(), kind: "text".into(), ..Default::default() },
        Column { name: "computed".into(), kind: "integer".into(), generated: true, ..Default::default() },
    ], ..Default::default() }
}
#[test]
fn postgres_quotes_paging_and_primary_key_mutations() {
    use postgres_sql::*;
    assert_eq!(ident("a\"b"), "\"a\"\"b\"");
    assert_eq!(literal(&text("O'Brien")), "'O''Brien'");
    assert_eq!(literal(&text("\\\n\r\t'á\x1a")), "E'\\\\\\u000A\\u000D\\u0009''á\\u001A'");
    assert_eq!(literal(&text("\0")), "E'\\u0000'");
    assert_eq!(literal(&text("")), "''");
    assert_eq!(literal(&Cell::Null), "NULL");
    assert_eq!(literal(&Cell::Blob("00FF".into())), "'\\x00FF'::bytea");
    let t = table();
    let before = vec![text("9223372036854775807"), text("O'Brien"), text("antes"), text("5")];
    let mut row = DataRow { original: Some(before.clone()), values: before, rowid: Cell::Null, deleted: false };
    row.values[2] = text("depois"); row.values[3] = text("999");
    assert_eq!(statements(&t, &[row.clone()]).unwrap(), ["UPDATE \"te\"\"st\".\"pe\"\"ople\" SET \"na\"\"me\" = 'depois' WHERE \"id\" = 9223372036854775807 AND \"part\" = 'O''Brien';"]);
    row.deleted = true;
    assert_eq!(statements(&t, &[row.clone()]).unwrap(), ["DELETE FROM \"te\"\"st\".\"pe\"\"ople\" WHERE \"id\" = 9223372036854775807 AND \"part\" = 'O''Brien';"]);
    row.deleted = false; row.original = None; row.values[0] = Cell::Null;
    assert_eq!(statements(&t, &[row.clone()]).unwrap(), ["INSERT INTO \"te\"\"st\".\"pe\"\"ople\" (\"part\", \"na\"\"me\") VALUES ('O''Brien', 'depois');"]);
    assert!(dml(&t, &[row]).unwrap().starts_with("BEGIN;\nINSERT INTO"));
    assert_eq!(page_sql(&t, "\"id\" > 7", "\"id\" DESC", 500), "SELECT \"id\" AS \"c0\", \"part\" AS \"c1\", \"na\"\"me\" AS \"c2\", \"computed\" AS \"c3\" FROM \"te\"\"st\".\"pe\"\"ople\" WHERE \"id\" > 7 ORDER BY \"id\" DESC LIMIT 500 OFFSET 500");
    assert!(!page_sql(&t, " ", "", 0).contains("WHERE"));
    let mut no_pk = t.clone(); for c in &mut no_pk.columns { c.pk = 0; }
    assert!(matches!(no_pk.identity(), Identity::ReadOnly)); assert!(statements(&no_pk, &[]).is_err());
    let mut view = t.clone(); view.view = true; assert!(statements(&view, &[]).is_err());
    let only_id = Table { columns: vec![t.columns[0].clone()], ..t };
    assert!(statements(&only_id, &[DataRow { original: None, values: vec![Cell::Null], rowid: Cell::Null, deleted: false }]).unwrap()[0].ends_with("DEFAULT VALUES;"));
}
#[test]
fn postgres_console_dollar_quotes_escape_strings_and_nested_comments() {
    use view::instruction_mode;
    let sql = "DO $$ BEGIN RAISE NOTICE 'a;b'; END $$;\nSELECT $tag$a;b$tag$; SELECT 3;";
    assert_eq!(instruction_mode(sql, (0,0), Dialect::Postgres, false), "DO $$ BEGIN RAISE NOTICE 'a;b'; END $$;");
    let i = sql.find("SELECT $tag$").unwrap();
    assert_eq!(instruction_mode(sql, (i,i), Dialect::Postgres, false), "SELECT $tag$a;b$tag$;");
    let sql = "SELECT E'a\\\';b'; /* a /* ; */ ; */ SELECT 2; SELECT 3";
    assert_eq!(instruction_mode(sql, (0,0), Dialect::Postgres, false), "SELECT E'a\\\';b';");
    assert_eq!(instruction::ranges(sql, Dialect::Postgres, false).len(), 3);
    assert_eq!(instruction_mode("SELECT 'á;b'; SELECT 2", (0,6), Dialect::Postgres, false), "SELECT");
    assert_eq!(instruction::words("/* a /* b */ c */ -- comment\nROLLBACK TO SAVEPOINT s"), ["ROLLBACK", "TO", "SAVEPOINT", "S"]);
}

struct Server { c: postgres::Connection, name: String }
impl Server {
    fn new() -> Self {
        let url = std::env::var("MAGO_PG_URL").expect("Defina MAGO_PG_URL para um banco PostgreSQL de teste");
        let mut c = postgres::Connection::from_url(&url).unwrap();
        let name = format!("mago_test_{}", uuid::Uuid::new_v4().simple());
        c.query(&format!("CREATE SCHEMA {name}; SET search_path TO {name}, pg_catalog")).unwrap();
        Self { c, name }
    }
}
impl Drop for Server {
    fn drop(&mut self) { let _ = self.c.rollback(); let _ = self.c.set_auto(true); let _ = self.c.query(&format!("DROP SCHEMA {} CASCADE", self.name)); }
}
fn object(t: &Table) -> Object {
    Object { schema: t.schema.clone(), name: t.name.clone(), kind: if t.materialized { ObjectKind::MaterializedView } else if t.view { ObjectKind::View } else { ObjectKind::Table }, oid: None }
}
#[test]
#[ignore = "Requer MAGO_PG_URL; cria e remove um schema temporário"]
fn postgres_server_catalog_ddl_grid_submit_and_manual_transactions() {
    let mut server = Server::new();
    let c = &mut server.c;
    assert!(c.version().unwrap().starts_with("Conectado: PostgreSQL "));
    c.query("CREATE TABLE people(id BIGINT GENERATED BY DEFAULT AS IDENTITY PRIMARY KEY, name TEXT, raw BYTEA, n INT GENERATED ALWAYS AS (length(name)) STORED); CREATE INDEX people_name ON people(name); CREATE TABLE child(id SERIAL PRIMARY KEY, parent BIGINT REFERENCES people(id)); CREATE TABLE no_key(value INT); CREATE VIEW names AS SELECT name FROM people; CREATE MATERIALIZED VIEW names_materialized AS SELECT name FROM people; CREATE SEQUENCE independent_sequence").unwrap();
    c.query("CREATE FUNCTION one_value() RETURNS integer LANGUAGE sql AS $$ SELECT 1; $$; CREATE FUNCTION one_value(integer) RETURNS integer LANGUAGE sql AS $body$ SELECT $1; $body$; CREATE PROCEDURE nothing() LANGUAGE plpgsql AS $$ BEGIN NULL; END $$; CREATE FUNCTION on_insert() RETURNS trigger LANGUAGE plpgsql AS $$ BEGIN NEW.name := COALESCE(NEW.name, 'novo'); RETURN NEW; END $$; CREATE TRIGGER people_insert BEFORE INSERT ON people FOR EACH ROW EXECUTE FUNCTION on_insert()").unwrap();
    let catalog = c.catalog().unwrap();
    assert!(!catalog.schemas.iter().any(|s| s.name.starts_with("pg_toast") || s.name.starts_with("pg_temp_")));
    assert!(catalog.schemas.iter().rev().take(2).all(|s| matches!(s.name.as_str(), "pg_catalog" | "information_schema")));
    let schema = catalog.schemas.iter().find(|s| s.name == server.name).unwrap();
    let t = schema.tables.iter().find(|t| t.name == "people").unwrap();
    assert!(t.columns[0].auto_increment && t.columns[0].pk == 1 && t.columns[3].generated);
    assert!(!t.triggers.is_empty()); assert!(t.indexes.iter().any(|(n,_)| n == "people_name"));
    let child = schema.tables.iter().find(|t| t.name == "child").unwrap();
    assert!(child.columns[0].auto_increment && !child.foreign.is_empty());
    assert!(schema.tables.iter().any(|t| t.materialized));
    assert!(matches!(schema.tables.iter().find(|t| t.name == "no_key").unwrap().identity(), Identity::ReadOnly));
    assert_eq!(schema.objects.iter().filter(|o| o.name.starts_with("one_value(")).count(), 2);
    for o in &schema.objects { assert!(!c.ddl(o).unwrap().is_empty()); }
    for t in &schema.tables { assert!(!c.ddl(&object(t)).unwrap().is_empty()); }
    assert!(c.ddl(&t.triggers[0]).unwrap().contains("CREATE TRIGGER"));
    assert!(c.ddl(&Object { schema: t.schema.clone(), name: "people_name".into(), kind: ObjectKind::Index, oid: None }).unwrap().contains("CREATE INDEX"));
    let ddl = c.ddl(&object(t)).unwrap();
    assert!(ddl.contains("GENERATED BY DEFAULT AS IDENTITY") && ddl.contains("PRIMARY KEY") && ddl.contains("CREATE INDEX"));
    let new = DataRow { original: None, values: vec![Cell::Null, text("O'Brien\\\ná"), Cell::Blob("00FF".into()), Cell::Null], rowid: Cell::Null, deleted: false };
    c.submit(t, &[new.clone()]).unwrap();
    let page = c.page(t, "id>0", "id DESC", 0).unwrap();
    assert_eq!(page.count, 1); assert_eq!(page.rows[0].values[1..3], new.values[1..3]);
    assert_eq!(c.page(t, "id>0", "id ASC", 500).unwrap().count, 1);
    let mut row = page.rows[0].clone(); row.values[1] = text("alterado");
    let mut duplicate = row.clone(); duplicate.original = None;
    assert!(c.submit(t, &[row.clone(), duplicate.clone()]).is_err());
    assert_eq!(c.page(t, "id=1", "", 0).unwrap().rows[0].values[1], new.values[1]);
    c.set_auto(false).unwrap();
    c.query("INSERT INTO child(parent) VALUES (1)").unwrap();
    assert!(c.submit(t, &[row.clone(), duplicate]).is_err());
    assert_eq!(c.query("SELECT count(*) FROM child").unwrap().rows[0][0], text("1"));
    c.submit(t, &[row.clone()]).unwrap(); c.rollback().unwrap();
    assert_eq!(c.page(t, "id=1", "", 0).unwrap().rows[0].values[1], new.values[1]);
    assert_eq!(c.query("SELECT count(*) FROM child").unwrap().rows[0][0], text("0"));
    c.submit(t, &[row]).unwrap(); c.commit().unwrap();
    let mut row = c.page(t, "id=1", "", 0).unwrap().rows.remove(0); row.deleted = true;
    c.submit(t, &[row]).unwrap(); c.commit().unwrap();
    assert_eq!(c.page(t, "id=1", "", 0).unwrap().count, 0);
    c.set_auto(true).unwrap();
    let g = c.query("SELECT '\\x00ff'::bytea, '\\x00ff'::text, NULL, 9223372036854775807::bigint, '{\"a\":1}'::jsonb; DO $$ BEGIN NULL; END $$").unwrap();
    assert_eq!(&g.rows[0][..4], &[Cell::Blob("00FF".into()), text("\\x00ff"), Cell::Null, text("9223372036854775807")]);
    assert!(c.query("SELECT 1; SELECT 2 WHERE false; SET application_name='mago-test'").unwrap().rows.is_empty());
    c.query("BEGIN; INSERT INTO no_key VALUES (1)").unwrap(); c.rollback().unwrap();
    assert_eq!(c.query("SELECT count(*) FROM no_key").unwrap().rows[0][0], text("0"));
}

#[test]
#[ignore = "Requer MAGO_PG_URL; cria e remove um schema temporário"]
fn postgres_server_tabs_share_manual_transaction_and_disconnect_rolls_back() {
    let mut server = Server::new();
    server.c.query("CREATE TABLE t(id INT PRIMARY KEY, value TEXT); INSERT INTO t VALUES (1, 'original')").unwrap();
    let config: ::postgres::Config = std::env::var("MAGO_PG_URL").unwrap().parse().unwrap();
    let host = match &config.get_hosts()[0] {
        ::postgres::config::Host::Tcp(h) => h.clone(),
        #[cfg(unix)]
        ::postgres::config::Host::Unix(p) => p.to_string_lossy().into_owned(),
    };
    let c = Conexao { motor: Motor::Postgres, host, porta: config.get_ports().first().copied().unwrap_or(5432),
        banco: config.get_dbname().unwrap_or("postgres").into(), usuario: config.get_user().unwrap_or("postgres").into(),
        tls: config.get_ssl_mode() == ::postgres::config::SslMode::Require, auto_commit: true, ..Default::default() };
    let session = Session::new(c, String::from_utf8(config.get_password().unwrap_or_default().to_vec()).unwrap());
    let other_tab = session.clone();
    let ctx = egui::Context::default();
    let wait = |session: &Session, op| session.request(op, &ctx).recv_timeout(Duration::from_secs(65)).unwrap();
    assert!(matches!(wait(&session, Op::Console(format!("SET search_path TO {}, pg_catalog", server.name))), Job::Console(Ok(_), _)));
    assert!(matches!(wait(&session, Op::Auto(false)), Job::Submit(Ok(()))));
    assert!(!other_tab.auto());
    assert!(matches!(wait(&session, Op::Console("UPDATE t SET value='alterado' WHERE id=1".into())), Job::Console(Ok(_), _)));
    let Job::Console(Ok(grid), _) = wait(&other_tab, Op::Console("SELECT value FROM t WHERE id=1".into())) else { panic!("SELECT falhou") };
    assert_eq!(grid.rows[0][0], text("alterado"));
    let revision = session.revision();
    assert!(matches!(wait(&other_tab, Op::Rollback), Job::Submit(Ok(()))));
    assert!(session.revision() > revision);
    assert!(matches!(wait(&session, Op::Console("UPDATE t SET value='confirmado' WHERE id=1".into())), Job::Console(Ok(_), _)));
    assert!(matches!(wait(&other_tab, Op::Commit), Job::Submit(Ok(()))));
    assert_eq!(server.c.query("SELECT value FROM t WHERE id=1").unwrap().rows[0][0], text("confirmado"));
    assert!(matches!(wait(&session, Op::Console("UPDATE t SET value='ao desconectar' WHERE id=1".into())), Job::Console(Ok(_), _)));
    drop(other_tab); drop(session);
    server.c.query("SET lock_timeout='5s'; UPDATE t SET value=value WHERE id=1").unwrap();
    assert_eq!(server.c.query("SELECT value FROM t WHERE id=1").unwrap().rows[0][0], text("confirmado"));
}
