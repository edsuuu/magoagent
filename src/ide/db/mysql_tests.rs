use super::*;

fn text(s: &str) -> Cell { Cell::Text(s.into()) }
fn table() -> Table {
    Table { name: "pe`ople".into(), schema: "te`st".into(), dialect: Dialect::Mysql, columns: vec![
        Column { name: "id".into(), kind: "bigint unsigned".into(), pk: 1, auto_increment: true, ..Default::default() },
        Column { name: "part".into(), kind: "varchar(50)".into(), pk: 2, ..Default::default() },
        Column { name: "na`me".into(), kind: "text".into(), ..Default::default() },
        Column { name: "computed".into(), generated: true, ..Default::default() },
    ], ..Default::default() }
}
#[test]
fn mysql_quotes_paging_and_primary_key_mutations() {
    assert_eq!(mysql_sql::ident("a`b"), "`a``b`");
    assert_eq!(mysql_sql::literal(&text("O'Brien")), "'O''Brien'");
    assert_eq!(mysql_sql::literal(&text("\\\0\n\r\x1a'á")), "CONVERT(X'5C000A0D1A27C3A1' USING utf8mb4)");
    assert_eq!(mysql_sql::literal(&text("")), "''");
    assert_eq!(mysql_sql::literal(&Cell::Null), "NULL");
    assert_eq!(mysql_sql::literal(&Cell::Blob("00FF".into())), "X'00FF'");
    let t = table();
    let original = vec![text("18446744073709551615"), text("O'Brien"), text("antes"), text("ignorado")];
    let mut row = DataRow { original: Some(original.clone()), values: original.clone(), rowid: Cell::Null, deleted: false };
    row.values[0] = text("7"); row.values[2] = text("depois"); row.values[3] = text("não enviar");
    let update = mysql_sql::statements(&t, &[row.clone()]).unwrap();
    assert_eq!(update, ["UPDATE `te``st`.`pe``ople` SET `id` = 7, `na``me` = 'depois' WHERE `id` = 18446744073709551615 AND `part` = 'O''Brien';"]);
    row.deleted = true;
    assert_eq!(mysql_sql::statements(&t, &[row.clone()]).unwrap(), ["DELETE FROM `te``st`.`pe``ople` WHERE `id` = 18446744073709551615 AND `part` = 'O''Brien';"]);
    row.original = None; row.deleted = false; row.values[0] = Cell::Null;
    assert_eq!(mysql_sql::statements(&t, &[row.clone()]).unwrap(), ["INSERT INTO `te``st`.`pe``ople` (`part`, `na``me`) VALUES ('O''Brien', 'depois');"]);
    assert_eq!(mysql_sql::dml(&t, &[row]).unwrap(), "START TRANSACTION;\nINSERT INTO `te``st`.`pe``ople` (`part`, `na``me`) VALUES ('O''Brien', 'depois');\nCOMMIT;");
    assert_eq!(mysql_sql::page_sql(&t, "`id` > 7", "`id` DESC", 500), "SELECT `id` AS `c0`, `part` AS `c1`, `na``me` AS `c2`, `computed` AS `c3` FROM `te``st`.`pe``ople` WHERE `id` > 7 ORDER BY `id` DESC LIMIT 500 OFFSET 500");
    assert_eq!(mysql_sql::page_sql(&t, "  ", "", 0), "SELECT `id` AS `c0`, `part` AS `c1`, `na``me` AS `c2`, `computed` AS `c3` FROM `te``st`.`pe``ople` LIMIT 500 OFFSET 0");
    let mut no_pk = t.clone(); for c in &mut no_pk.columns { c.pk = 0; }
    assert!(matches!(no_pk.identity(), Identity::ReadOnly));
    assert!(mysql_sql::dml(&no_pk, &[]).is_err());
    let mut view = t; view.view = true;
    assert!(matches!(view.identity(), Identity::ReadOnly));
}

#[test]
fn session_queue_reports_each_request_without_connecting() {
    let c = conexoes::Conexao { motor: Motor::Postgres, ..Default::default() };
    let session = Session::new(c, String::new());
    let ctx = egui::Context::default();
    let first = session.request(Op::Test, &ctx);
    let second = session.clone().request(Op::Console("SELECT 1".into()), &ctx);
    assert!(matches!(first.recv_timeout(Duration::from_secs(2)).unwrap(), Job::Test(Err(_))));
    assert!(matches!(second.recv_timeout(Duration::from_secs(2)).unwrap(), Job::Console(Err(_), _)));
}

struct Server { c: mysql::Connection, name: String }
impl Server {
    fn new() -> Self {
        let url = std::env::var("MAGO_MYSQL_URL").expect("Defina MAGO_MYSQL_URL para um servidor de teste");
        let opts = ::mysql::Opts::from_url(&url).unwrap();
        let config = Conexao { motor: Motor::Mysql, host: opts.get_ip_or_hostname().to_string(), porta: opts.get_tcp_port(),
            usuario: opts.get_user().unwrap_or_default().into(), banco: opts.get_db_name().unwrap_or_default().into(), auto_commit: true, ..Default::default() };
        let mut c = mysql::Connection::connect(&config, opts.get_pass().unwrap_or_default().into()).unwrap();
        let name = format!("mago_test_{}", uuid::Uuid::new_v4().simple());
        c.query(&format!("CREATE DATABASE `{name}`")).unwrap();
        c.query(&format!("USE `{name}`")).unwrap();
        Self { c, name }
    }
}
impl Drop for Server {
    fn drop(&mut self) { let _ = self.c.rollback(); let _ = self.c.query(&format!("DROP DATABASE `{}`", self.name)); }
}

#[test]
#[ignore = "Requer servidor de teste em MAGO_MYSQL_URL; cria e remove um schema temporário"]
fn mysql_server_catalog_ddl_grid_submit_and_manual_transactions() {
    let mut server = Server::new();
    let c = &mut server.c;
    assert!(c.version().unwrap().starts_with("Conectado:"));
    c.query("CREATE TABLE people(id BIGINT UNSIGNED PRIMARY KEY AUTO_INCREMENT, name TEXT, raw VARBINARY(10), n INT GENERATED ALWAYS AS (CHAR_LENGTH(name))); CREATE INDEX people_name ON people(name(20)); CREATE TABLE child(id INT PRIMARY KEY, parent BIGINT UNSIGNED, FOREIGN KEY(parent) REFERENCES people(id)); CREATE TABLE no_key(value INT); CREATE VIEW names AS SELECT name FROM people").unwrap();
    c.query("CREATE PROCEDURE count_people() SELECT COUNT(*) FROM people").unwrap();
    c.query("CREATE FUNCTION one_value() RETURNS INT DETERMINISTIC RETURN 1").unwrap();
    c.query("CREATE TRIGGER people_insert BEFORE INSERT ON people FOR EACH ROW SET NEW.name = COALESCE(NEW.name, 'novo')").unwrap();
    c.query("CREATE EVENT someday ON SCHEDULE AT CURRENT_TIMESTAMP + INTERVAL 1 YEAR DO SELECT 1").unwrap();
    let catalog = c.catalog().unwrap();
    let schema = catalog.schemas.iter().find(|s| s.name == server.name).unwrap();
    let table = schema.tables.iter().find(|t| t.name == "people").unwrap();
    assert!(table.columns[0].auto_increment && table.columns[0].pk == 1 && table.columns[3].generated);
    assert!(table.indexes.iter().any(|(n, _)| n == "people_name"));
    assert_eq!(schema.objects.len(), 4);
    assert!(!schema.tables.iter().find(|t| t.name == "child").unwrap().foreign.is_empty());
    assert!(schema.tables.iter().find(|t| t.name == "no_key").is_some_and(|t| matches!(t.identity(), Identity::ReadOnly)));
    for o in &schema.objects { assert!(!c.ddl(o).unwrap().is_empty()); }
    for t in &schema.tables { assert!(!c.ddl(&session::Object { oid: None, schema: t.schema.clone(), name: t.name.clone(), kind: if t.view { session::ObjectKind::View } else { session::ObjectKind::Table } }).unwrap().is_empty()); }
    let new = DataRow { original: None, values: vec![Cell::Null, text("O'Brien\\\0\ná"), Cell::Blob("00FF".into()), Cell::Null], rowid: Cell::Null, deleted: false };
    for mode in ["", "NO_BACKSLASH_ESCAPES"] {
        c.query(&format!("SET SESSION sql_mode='{mode}'")).unwrap();
        c.submit(table, &[new.clone()]).unwrap();
        let page = c.page(table, "", "id DESC", 0).unwrap();
        assert_eq!(page.rows[0].values[1..3], new.values[1..3]);
    }
    assert_eq!(c.page(table, "id>0", "id ASC", 500).unwrap().count, 2);
    let mut row = c.page(table, "id=1", "", 0).unwrap().rows.remove(0);
    row.values[1] = text("alterado");
    let mut duplicate = row.clone(); duplicate.original = None;
    assert!(c.submit(table, &[row.clone(), duplicate]).is_err());
    assert_eq!(c.page(table, "id=1", "", 0).unwrap().rows[0].values[1], new.values[1]);
    c.set_auto(false).unwrap();
    c.submit(table, &[row.clone()]).unwrap();
    assert_eq!(c.page(table, "id=1", "", 0).unwrap().rows[0].values[1], text("alterado"));
    c.rollback().unwrap();
    assert_eq!(c.page(table, "id=1", "", 0).unwrap().rows[0].values[1], new.values[1]);
    c.submit(table, &[row]).unwrap(); c.commit().unwrap();
    let mut row = c.page(table, "id=1", "", 0).unwrap().rows.remove(0); row.deleted = true;
    c.submit(table, &[row]).unwrap(); c.commit().unwrap();
    assert_eq!(c.page(table, "id=1", "", 0).unwrap().count, 0);
}

#[test]
fn mysql_console_understands_backslash_quotes_and_hash_comments() {
    use view::instruction_mode;
    let sql = "SELECT 'a\\\';b'; # ignored ;\nSELECT 2; SELECT 3";
    let cursor = sql.find("SELECT 2").unwrap();
    assert_eq!(instruction_mode(sql, (cursor, cursor), Dialect::Mysql, false), "# ignored ;\nSELECT 2;");
    assert_eq!(instruction_mode("SELECT 'a\\'; SELECT 2;", (0, 0), Dialect::Mysql, true), "SELECT 'a\\';");
    assert_eq!(instruction_mode("SELECT 5--2; SELECT 7", (0, 0), Dialect::Mysql, false), "SELECT 5--2;");
}

#[test]
#[ignore = "Requer servidor de teste em MAGO_MYSQL_URL; cria e remove um schema temporário"]
fn mysql_server_tabs_share_manual_transaction_and_use_schema() {
    let mut server = Server::new();
    server.c.query("CREATE TABLE t(id INT PRIMARY KEY)").unwrap();
    let url = std::env::var("MAGO_MYSQL_URL").unwrap();
    let opts = ::mysql::Opts::from_url(&url).unwrap();
    let config = Conexao { motor: Motor::Mysql, host: opts.get_ip_or_hostname().to_string(), porta: opts.get_tcp_port(),
        usuario: opts.get_user().unwrap_or_default().into(), auto_commit: true, ..Default::default() };
    let session = Session::new(config, opts.get_pass().unwrap_or_default().into());
    let other_tab = session.clone();
    let ctx = egui::Context::default();
    let wait = |session: &Session, op| session.request(op, &ctx).recv_timeout(Duration::from_secs(65)).unwrap();
    assert!(matches!(wait(&session, Op::Auto(false)), Job::Submit(Ok(()))));
    assert!(!other_tab.auto());
    assert!(matches!(wait(&session, Op::Console(format!("USE `{}`", server.name))), Job::Console(Ok(_), _)));
    assert!(matches!(wait(&session, Op::Console("INSERT INTO t VALUES (1)".into())), Job::Console(Ok(_), _)));
    let Job::Console(Ok(grid), _) = wait(&other_tab, Op::Console("SELECT id FROM t".into())) else { panic!("SELECT falhou") };
    assert_eq!(grid.rows, [vec![text("1")]]);
    let revision = session.revision();
    assert!(matches!(wait(&other_tab, Op::Rollback), Job::Submit(Ok(()))));
    assert!(session.revision() > revision);
    let Job::Console(Ok(grid), _) = wait(&session, Op::Console("SELECT COUNT(*) FROM t".into())) else { panic!("SELECT falhou") };
    assert_eq!(grid.rows, [vec![text("0")]]);
    assert!(matches!(wait(&session, Op::Console("SET SESSION sql_mode='NO_BACKSLASH_ESCAPES'".into())), Job::Console(Ok(_), _)));
    assert!(other_tab.no_backslash());
    assert!(matches!(wait(&session, Op::Auto(true)), Job::Submit(Ok(()))));
    assert!(other_tab.auto());
}
