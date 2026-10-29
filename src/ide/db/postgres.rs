//! Synchronous PostgreSQL transport. Values always use the text protocol.
use super::*;
use ::postgres::{
    Client, Config, NoTls, SimpleQueryMessage,
    config::SslMode,
    types::{Kind, Type},
};

pub(super) struct Connection {
    conn: Client,
    auto: bool,
    in_tx: bool,
}
fn error(e: ::postgres::Error) -> String {
    e.as_db_error().map(|e| format!("{}: {}", e.code().code(), e.message())).unwrap_or_else(|| e.to_string())
}
impl Connection {
    pub fn connect(c: &conexoes::Conexao, password: String) -> Result<Self, String> {
        let mut config = Config::new();
        config
            .host(&c.host)
            .port(c.porta)
            .user(&c.usuario)
            .password(password)
            .dbname(if c.banco.is_empty() { "postgres" } else { &c.banco })
            .connect_timeout(Duration::from_secs(10))
            .options("-c standard_conforming_strings=on -c bytea_output=hex -c statement_timeout=60000");
        if c.somente_leitura {
            config.options("-c standard_conforming_strings=on -c bytea_output=hex -c statement_timeout=60000 -c default_transaction_read_only=on");
        }
        Self::open(config, c.tls, c.auto_commit)
    }
    fn open(mut config: Config, tls: bool, auto: bool) -> Result<Self, String> {
        let conn = if tls {
            config.ssl_mode(SslMode::Require);
            let _ = rustls::crypto::ring::default_provider().install_default();
            config.connect(tokio_postgres_rustls::MakeRustlsConnect::with_webpki_roots()).map_err(error)?
        } else {
            config.ssl_mode(SslMode::Disable);
            config.connect(NoTls).map_err(error)?
        };
        Ok(Self { conn, auto, in_tx: false })
    }
    #[cfg(test)]
    pub fn from_url(url: &str) -> Result<Self, String> {
        let mut config: Config = url.parse().map_err(error)?;
        let tls = config.get_ssl_mode() == SslMode::Require;
        config.options("-c standard_conforming_strings=on -c bytea_output=hex -c statement_timeout=60000");
        Self::open(config, tls, true)
    }
    fn begin(&mut self) -> Result<(), String> {
        if !self.in_tx {
            self.raw("BEGIN")?;
            self.in_tx = true;
        }
        Ok(())
    }
    // Describing supplies bytea types that SimpleColumn omits; no values use the binary protocol.
    fn raw(&mut self, sql: &str) -> Result<Grid, String> {
        let columns: Vec<_> = self.conn.prepare(sql).map_err(error)?.columns().iter().map(|c| bytea(c.type_())).collect();
        let mut grid = Grid { columns: vec![], rows: vec![] };
        let mut bytes = 0;
        for message in self.conn.simple_query(sql).map_err(error)? {
            match message {
                SimpleQueryMessage::RowDescription(cols) => grid = Grid { columns: cols.iter().map(|c| c.name().into()).collect(), rows: vec![] },
                SimpleQueryMessage::Row(row) => {
                    let cells: Vec<_> = (0..row.len()).map(|i| cell(row.get(i), columns.get(i).copied().unwrap_or(false))).collect();
                    bytes += cells.iter().map(|c| c.text().len().max(1)).sum::<usize>();
                    if bytes > 8 << 20 {
                        return Err("Resultado excedeu o limite de 8 MiB; refine a consulta.".into());
                    }
                    grid.rows.push(cells);
                }
                _ => {}
            }
        }
        Ok(grid)
    }
    pub fn query(&mut self, sql: &str) -> Result<Grid, String> {
        let mut grid = Grid { columns: vec![], rows: vec![] };
        for range in instruction::ranges(sql, Dialect::Postgres, false) {
            let sql = &sql[range];
            let words = instruction::words(sql);
            let first = words.first().map(String::as_str).unwrap_or("");
            if first.is_empty() {
                continue;
            }
            let control = matches!(first, "BEGIN" | "START" | "COMMIT" | "END" | "ROLLBACK" | "ABORT");
            if !self.auto && !control {
                self.begin()?;
            }
            let result = self.raw(sql)?;
            match first {
                "BEGIN" | "START" => self.in_tx = true,
                "COMMIT" | "END" | "ABORT" | "ROLLBACK" if !words.iter().any(|w| w == "TO") => {
                    self.in_tx = words.windows(2).any(|w| w[0] == "AND" && w[1] == "CHAIN");
                }
                _ => {}
            }
            if !result.columns.is_empty() {
                grid = result;
            }
        }
        Ok(grid)
    }
    pub fn version(&mut self) -> Result<String, String> {
        Ok(format!("Conectado: PostgreSQL {}", self.query("SHOW server_version")?.rows[0][0].text()))
    }
    pub fn set_auto(&mut self, value: bool) -> Result<(), String> {
        if value && !self.auto {
            self.commit()?;
        }
        self.auto = value;
        Ok(())
    }
    pub fn commit(&mut self) -> Result<(), String> {
        self.raw("COMMIT")?;
        self.in_tx = false;
        Ok(())
    }
    pub fn rollback(&mut self) -> Result<(), String> {
        self.raw("ROLLBACK")?;
        self.in_tx = false;
        Ok(())
    }
    pub fn page(&mut self, t: &Table, filter: &str, order: &str, offset: usize) -> Result<Page, String> {
        let page = postgres_sql::page_sql(t, filter, order, offset).replacen("SELECT ", "SELECT 1 AS __mago_present, ", 1);
        let grid = self
            .query(&format!("SELECT n.*, p.* FROM (SELECT COUNT(*) AS __mago_count {}) n LEFT JOIN ({page}) p ON TRUE", postgres_sql::from_where(t, filter)))?;
        let count = grid.rows.first().ok_or("Contagem ausente")?[0].text().parse().map_err(|_| "Contagem inválida")?;
        let rows = grid
            .rows
            .into_iter()
            .filter(|r| r[1] != Cell::Null)
            .map(|r| {
                let values = r[2..].to_vec();
                DataRow { original: Some(values.clone()), values, rowid: Cell::Null, deleted: false }
            })
            .collect();
        Ok(Page { rows, count, ttl: None })
    }
    pub fn submit(&mut self, t: &Table, rows: &[DataRow]) -> Result<(), String> {
        let statements = postgres_sql::statements(t, rows)?;
        if statements.is_empty() {
            return Ok(());
        }
        let own = self.auto && !self.in_tx;
        self.begin()?;
        if !own {
            self.raw("SAVEPOINT mago_submit")?;
        }
        for sql in statements {
            if let Err(e) = self.raw(&sql) {
                let rollback = if own {
                    self.rollback()
                } else {
                    self.raw("ROLLBACK TO SAVEPOINT mago_submit; ").and_then(|_| self.raw("RELEASE SAVEPOINT mago_submit")).map(|_| ())
                };
                return Err(match rollback {
                    Ok(_) => e,
                    Err(r) => format!("{e}\nFalha ao reverter: {r}"),
                });
            }
        }
        if own { self.commit() } else { self.raw("RELEASE SAVEPOINT mago_submit").map(|_| ()) }
    }
}
fn bytea(t: &Type) -> bool {
    *t == Type::BYTEA || matches!(t.kind(), Kind::Domain(base) if bytea(base))
}
fn cell(value: Option<&str>, binary: bool) -> Cell {
    match value {
        None => Cell::Null,
        Some(s) if binary => Cell::Blob(s.strip_prefix("\\x").unwrap_or(s).to_ascii_uppercase()),
        Some(s) => Cell::Text(s.into()),
    }
}
