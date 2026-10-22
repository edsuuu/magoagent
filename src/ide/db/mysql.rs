//! Synchronous MySQL/MariaDB transport and information_schema catalog.
use super::*;
use ::mysql::{prelude::Queryable, Conn, OptsBuilder, Value, consts::ColumnType};
use session::{Catalog, Schema, Object, ObjectKind};

pub(super) struct Connection { conn: Conn, auto: bool }
impl Connection {
    pub fn connect(c: &conexoes::Conexao, password: String) -> Result<Self, String> {
        let opts = OptsBuilder::default().ip_or_hostname(Some(c.host.clone())).tcp_port(c.porta)
            .user(Some(c.usuario.clone())).pass(Some(password)).db_name((!c.banco.is_empty()).then(|| c.banco.clone()))
            .ssl_opts(c.tls.then(::mysql::SslOpts::default))
            .prefer_socket(false).tcp_connect_timeout(Some(Duration::from_secs(10)))
            .read_timeout(Some(Duration::from_secs(60))).write_timeout(Some(Duration::from_secs(60)));
        let conn = Conn::new(opts).map_err(|e| e.to_string())?;
        let mut this = Self { conn, auto: true };
        this.query("SET NAMES utf8mb4")?;
        if c.somente_leitura { this.query("SET SESSION TRANSACTION READ ONLY")?; }
        this.set_auto(c.auto_commit)?;
        Ok(this)
    }
    pub fn query(&mut self, sql: &str) -> Result<Grid, String> {
        let mut result = self.conn.query_iter(sql).map_err(|e| e.to_string())?;
        let mut grid = Grid { columns: vec![], rows: vec![] };
        let mut bytes = 0;
        while let Some(mut set) = result.iter() {
            let cols = set.columns().as_ref().to_vec();
            grid = Grid { columns: cols.iter().map(|c| c.name_str().into_owned()).collect(), rows: vec![] };
            for row in &mut set {
                let row = row.map_err(|e| e.to_string())?;
                let cells = row.unwrap().into_iter().zip(&cols).map(|(v, c)| cell(v, c)).collect::<Vec<_>>();
                bytes += cells.iter().map(|c| c.text().len().max(1)).sum::<usize>();
                if bytes > 8 << 20 { return Err("Resultado excedeu o limite de 8 MiB; refine a consulta.".into()); }
                grid.rows.push(cells);
            }
        }
        Ok(grid)
    }
    pub fn version(&mut self) -> Result<String, String> {
        let grid = self.query("SELECT VERSION(), @@version_comment")?;
        let r = grid.rows.first().ok_or("Versão ausente")?;
        let name = if r[0].text().contains("MariaDB") { "MariaDB" } else { "MySQL" };
        Ok(format!("Conectado: {name} {} ({})", r[0].text(), r[1].text()))
    }
    pub fn set_auto(&mut self, value: bool) -> Result<(), String> {
        self.query(if value { "SET autocommit=1" } else { "SET autocommit=0" })?;
        self.auto = value;
        Ok(())
    }
    pub fn state(&mut self) -> Result<(bool, bool), String> {
        let grid = self.query("SELECT @@SESSION.autocommit, @@SESSION.sql_mode")?;
        let r = grid.rows.first().ok_or("Estado da sessão ausente")?;
        self.auto = r[0].text() == "1";
        Ok((self.auto, r[1].text().split(',').any(|s| s == "NO_BACKSLASH_ESCAPES")))
    }
    pub fn commit(&mut self) -> Result<(), String> { self.query("COMMIT").map(|_| ()) }
    pub fn rollback(&mut self) -> Result<(), String> { self.query("ROLLBACK").map(|_| ()) }
    pub fn page(&mut self, t: &Table, filter: &str, order: &str, offset: usize) -> Result<Page, String> {
        // Count and page are one statement, including an empty or past-the-end page.
        let page = mysql_sql::page_sql(t, filter, order, offset);
        let page = page.replacen("SELECT ", "SELECT 1 AS `__mago_present`, ", 1);
        let sql = format!("SELECT n.*, p.* FROM (SELECT COUNT(*) AS `__mago_count` {}) n LEFT JOIN ({page}) p ON 1", mysql_sql::from_where(t, filter));
        let grid = self.query(&sql)?;
        let count = grid.rows.first().ok_or("Contagem ausente")?[0].text().parse().map_err(|_| "Contagem inválida")?;
        let rows = grid.rows.into_iter().filter(|r| r[1] != Cell::Null).map(|r| {
            let values = r[2..].to_vec();
            DataRow { original: Some(values.clone()), values, rowid: Cell::Null, deleted: false }
        }).collect();
        Ok(Page { rows, count, ttl: None })
    }
    pub fn submit(&mut self, t: &Table, rows: &[DataRow]) -> Result<(), String> {
        let statements = mysql_sql::statements(t, rows)?;
        if statements.is_empty() { return Ok(()); }
        if !self.auto {
            // A table read starts the implicit transaction before the savepoint.
            self.query(&format!("SELECT 1 FROM {} LIMIT 1", mysql_sql::target(t)))?;
        }
        self.query(if self.auto { "START TRANSACTION" } else { "SAVEPOINT mago_submit" })?;
        for sql in statements {
            if let Err(e) = self.query(&sql) {
                let rollback = self.query(if self.auto { "ROLLBACK" } else { "ROLLBACK TO SAVEPOINT mago_submit" });
                return Err(match rollback { Ok(_) => e, Err(r) => format!("{e}\nFalha ao reverter: {r}") });
            }
        }
        self.query(if self.auto { "COMMIT" } else { "RELEASE SAVEPOINT mago_submit" }).map(|_| ())
    }
    pub fn ddl(&mut self, o: &Object) -> Result<String, String> {
        let sql = format!("SHOW CREATE {} {}.{}", o.kind.sql(), mysql_sql::ident(&o.schema), mysql_sql::ident(&o.name));
        let grid = self.query(&sql)?;
        let i = grid.columns.iter().position(|c| c.to_ascii_lowercase().starts_with("create ") || c == "SQL Original Statement").ok_or("DDL indisponível")?;
        grid.rows.first().and_then(|r| r.get(i)).filter(|c| **c != Cell::Null).map(|c| c.text().to_string()).ok_or("Sem permissão para ler o DDL".into())
    }
    pub fn catalog(&mut self) -> Result<Catalog, String> {
        let mut catalog = Catalog::default();
        for r in self.query("SELECT SCHEMA_NAME FROM information_schema.SCHEMATA ORDER BY SCHEMA_NAME")?.rows {
            catalog.schemas.push(Schema { name: r[0].text().into(), ..Default::default() });
        }
        for r in self.query("SELECT TABLE_SCHEMA, TABLE_NAME, TABLE_TYPE FROM information_schema.TABLES ORDER BY TABLE_SCHEMA, TABLE_NAME")?.rows {
            if let Some(s) = catalog.schemas.iter_mut().find(|s| s.name == r[0].text()) {
                s.tables.push(Table { schema: s.name.clone(), name: r[1].text().into(), dialect: Dialect::Mysql, view: r[2].text() == "VIEW" || r[2].text() == "SYSTEM VIEW", ..Default::default() });
            }
        }
        for r in self.query("SELECT c.TABLE_SCHEMA,c.TABLE_NAME,c.COLUMN_NAME,c.COLUMN_TYPE,c.IS_NULLABLE,c.COLUMN_DEFAULT,c.EXTRA,COALESCE(k.ORDINAL_POSITION,0) FROM information_schema.COLUMNS c LEFT JOIN information_schema.KEY_COLUMN_USAGE k ON k.TABLE_SCHEMA=c.TABLE_SCHEMA AND k.TABLE_NAME=c.TABLE_NAME AND k.COLUMN_NAME=c.COLUMN_NAME AND k.CONSTRAINT_NAME='PRIMARY' ORDER BY c.TABLE_SCHEMA,c.TABLE_NAME,c.ORDINAL_POSITION")?.rows {
            if let Some(t) = table_mut(&mut catalog, &r) {
                t.columns.push(Column { name: r[2].text().into(), kind: r[3].text().into(), notnull: r[4].text() == "NO", default: (r[5] != Cell::Null).then(|| r[5].text().into()),
                    auto_increment: r[6].text().contains("auto_increment"), generated: r[6].text().contains("VIRTUAL GENERATED") || r[6].text().contains("STORED GENERATED"), pk: r[7].text().parse().unwrap_or(0), children: vec![] });
            }
        }
        for r in self.query("SELECT TABLE_SCHEMA,TABLE_NAME,INDEX_NAME,COLUMN_NAME,NON_UNIQUE FROM information_schema.STATISTICS ORDER BY TABLE_SCHEMA,TABLE_NAME,INDEX_NAME,SEQ_IN_INDEX")?.rows {
            if let Some(t) = table_mut(&mut catalog, &r) {
                let name = r[2].text();
                let column = if r[3] == Cell::Null { "(expressão)" } else { r[3].text() };
                let label = format!("{name}{}", if r[4].text() == "0" { " (único)" } else { "" });
                if let Some((_, cols)) = t.indexes.iter_mut().find(|(n, _)| *n == label) { cols.push_str(&format!(", {column}")); }
                else { t.indexes.push((label, column.into())); }
            }
        }
        for r in self.query("SELECT TABLE_SCHEMA,TABLE_NAME,CONSTRAINT_NAME,COLUMN_NAME,REFERENCED_TABLE_SCHEMA,REFERENCED_TABLE_NAME,REFERENCED_COLUMN_NAME FROM information_schema.KEY_COLUMN_USAGE WHERE REFERENCED_TABLE_NAME IS NOT NULL ORDER BY TABLE_SCHEMA,TABLE_NAME,CONSTRAINT_NAME,ORDINAL_POSITION")?.rows {
            if let Some(t) = table_mut(&mut catalog, &r) { t.foreign.push(format!("{}: {} → {}.{}.{}", r[2].text(), r[3].text(), r[4].text(), r[5].text(), r[6].text())); }
        }
        for (sql, kind) in [
            ("SELECT ROUTINE_SCHEMA,ROUTINE_NAME,ROUTINE_TYPE FROM information_schema.ROUTINES ORDER BY ROUTINE_SCHEMA,ROUTINE_NAME", ObjectKind::Procedure),
            ("SELECT TRIGGER_SCHEMA,TRIGGER_NAME,'TRIGGER' FROM information_schema.TRIGGERS ORDER BY TRIGGER_SCHEMA,TRIGGER_NAME", ObjectKind::Trigger),
            ("SELECT EVENT_SCHEMA,EVENT_NAME,'EVENT' FROM information_schema.EVENTS ORDER BY EVENT_SCHEMA,EVENT_NAME", ObjectKind::Event),
        ] {
            for r in self.query(sql)?.rows {
                if let Some(s) = catalog.schemas.iter_mut().find(|s| s.name == r[0].text()) {
                    s.objects.push(Object { oid: None, schema: s.name.clone(), name: r[1].text().into(), kind: if r[2].text() == "FUNCTION" { ObjectKind::Function } else { kind } });
                }
            }
        }
        Ok(catalog)
    }
}
fn table_mut<'a>(catalog: &'a mut Catalog, row: &[Cell]) -> Option<&'a mut Table> {
    catalog.schemas.iter_mut().find(|s| s.name == row[0].text())?.tables.iter_mut().find(|t| t.name == row[1].text())
}
fn cell(v: Value, c: &::mysql::Column) -> Cell {
    match v {
        Value::NULL => Cell::Null,
        Value::Bytes(b) => {
            let binary = c.character_set() == 63 && matches!(c.column_type(), ColumnType::MYSQL_TYPE_BLOB | ColumnType::MYSQL_TYPE_LONG_BLOB | ColumnType::MYSQL_TYPE_MEDIUM_BLOB | ColumnType::MYSQL_TYPE_TINY_BLOB | ColumnType::MYSQL_TYPE_STRING | ColumnType::MYSQL_TYPE_VAR_STRING | ColumnType::MYSQL_TYPE_VARCHAR | ColumnType::MYSQL_TYPE_BIT | ColumnType::MYSQL_TYPE_GEOMETRY);
            if binary { Cell::Blob(b.iter().map(|b| format!("{b:02X}")).collect()) }
            else { match String::from_utf8(b) { Ok(s) => Cell::Text(s), Err(e) => Cell::Blob(e.as_bytes().iter().map(|b| format!("{b:02X}")).collect()) } }
        }
        v @ (Value::Date(..) | Value::Time(..)) => Cell::Text(v.as_sql(true).trim_matches('\'').into()),
        v => Cell::Text(v.as_sql(true)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn wire_cells_preserve_binary_text_numeric_and_null_values() {
        let text = ::mysql::Column::new(ColumnType::MYSQL_TYPE_BLOB).with_character_set(46)
            .with_flags(::mysql::consts::ColumnFlags::BINARY_FLAG);
        let binary = text.clone().with_character_set(63);
        assert_eq!(cell(Value::Bytes("á".as_bytes().to_vec()), &text), Cell::Text("á".into()));
        assert_eq!(cell(Value::Bytes(vec![0, 255]), &binary), Cell::Blob("00FF".into()));
        assert_eq!(cell(Value::NULL, &binary), Cell::Null);
        assert_eq!(cell(Value::UInt(u64::MAX), &binary), Cell::Text(u64::MAX.to_string()));
        let decimal = ::mysql::Column::new(ColumnType::MYSQL_TYPE_NEWDECIMAL).with_character_set(63);
        assert_eq!(cell(Value::Bytes(b"12345678901234567890.123456789".to_vec()), &decimal), Cell::Text("12345678901234567890.123456789".into()));
    }
}
