//! Synchronous Redis transport; SCAN is bounded and mutations share MULTI/EXEC.
use super::*;
use ::redis::{ConnectionAddr, ConnectionInfo, FromRedisValue, RedisConnectionInfo, Value};
use redis_console::{bytes, cell};
use session::{Catalog, Schema};

pub(super) struct Connection {
    pub wire: ::redis::Connection,
    pub database: i64,
}
fn err(e: impl std::fmt::Display) -> String {
    e.to_string()
}
fn text(s: impl ToString) -> Cell {
    Cell::Text(s.to_string())
}
fn column(name: &str, pk: bool) -> Column {
    Column { name: name.into(), pk: usize::from(pk), ..Default::default() }
}
pub(super) fn columns(kind: &str) -> Vec<Column> {
    match kind {
        "string" => vec![column("valor", true)],
        "hash" => vec![column("campo", true), column("valor", false)],
        "list" => vec![Column { generated: true, ..column("índice", true) }, column("valor", false)],
        "set" => vec![column("membro", true)],
        "zset" => vec![column("membro", true), column("score", false)],
        "stream" => vec![column("id", true), column("campos", false)],
        _ => vec![],
    }
}
impl Connection {
    pub fn info(c: &Conexao, password: String) -> Result<ConnectionInfo, String> {
        let database = if c.banco.is_empty() { 0 } else { c.banco.parse::<i64>().map_err(|_| "Banco Redis deve ser um número não negativo.")? };
        if database < 0 {
            return Err("Banco Redis deve ser um número não negativo.".into());
        }
        let auth = c.autenticacao == conexoes::Autenticacao::UsuarioSenha;
        Ok(ConnectionInfo {
            addr: if c.tls {
                ConnectionAddr::TcpTls { host: c.host.clone(), port: c.porta, insecure: false, tls_params: None }
            } else {
                ConnectionAddr::Tcp(c.host.clone(), c.porta)
            },
            redis: RedisConnectionInfo {
                db: database,
                username: (auth && !c.usuario.is_empty()).then(|| c.usuario.clone()),
                password: (auth && (!password.is_empty() || !c.usuario.is_empty())).then_some(password),
                protocol: ::redis::ProtocolVersion::RESP3,
            },
        })
    }
    pub fn connect(c: &Conexao, password: String) -> Result<Self, String> {
        let info = Self::info(c, password)?;
        let database = info.redis.db;
        let wire = ::redis::Client::open(info).map_err(err)?.get_connection_with_timeout(Duration::from_secs(10)).map_err(err)?;
        wire.set_read_timeout(Some(Duration::from_secs(60))).map_err(err)?;
        wire.set_write_timeout(Some(Duration::from_secs(60))).map_err(err)?;
        Ok(Self { wire, database })
    }
    fn command<T: FromRedisValue>(&mut self, name: &str, args: &[&str]) -> Result<T, String> {
        ::redis::cmd(name).arg(args).query(&mut self.wire).map_err(err)
    }
    fn select(&mut self, database: i64) -> Result<(), String> {
        self.command::<()>("SELECT", &[&database.to_string()])?;
        self.database = database;
        Ok(())
    }
    pub fn version(&mut self) -> Result<String, String> {
        let info: String = self.command("INFO", &["server"])?;
        Ok(format!("Conectado: Redis {}", info.lines().find_map(|l| l.strip_prefix("redis_version:")).unwrap_or("?").trim()))
    }
    pub fn catalog(&mut self) -> Result<Catalog, String> {
        let selected = self.database;
        let result = self.catalog_inner(selected);
        let restored = self.select(selected);
        match (result, restored) {
            (Err(e), _) => Err(e),
            (_, Err(e)) => Err(e),
            (Ok(c), _) => Ok(c),
        }
    }
    fn catalog_inner(&mut self, selected: i64) -> Result<Catalog, String> {
        let config: Result<Vec<(String, String)>, _> = self.command("CONFIG", &["GET", "databases"]);
        let n = config.ok().and_then(|c| c.first().and_then(|(_, s)| s.parse::<usize>().ok())).unwrap_or(16);
        let info: String = self.command("INFO", &["keyspace"])?;
        let mut catalog = Catalog::default();
        for i in 0..n {
            let name = format!("db{i}");
            let count = info
                .lines()
                .find_map(|s| s.strip_prefix(&format!("{name}:keys=")).and_then(|s| s.split(',').next()).and_then(|s| s.parse::<usize>().ok()))
                .unwrap_or(0);
            let mut schema = Schema { name, count: Some(count), expanded: count > 0 || i as i64 == selected, ..Default::default() };
            if schema.expanded {
                self.select(i as i64)?;
                let (keys, truncated) = self.scan()?;
                schema.warning = truncated.then(|| "Mais de 10 000 chaves; exibindo as primeiras 10 000.".into());
                for key in keys {
                    let kind: String = self.command("TYPE", &[&key])?;
                    if kind == "none" {
                        continue;
                    }
                    schema.tables.push(Table {
                        name: key,
                        schema: schema.name.clone(),
                        dialect: Dialect::Redis,
                        columns: columns(&kind),
                        view: kind == "stream",
                        kind,
                        ..Default::default()
                    });
                }
            }
            catalog.schemas.push(schema);
        }
        Ok(catalog)
    }
    fn scan(&mut self) -> Result<(Vec<String>, bool), String> {
        let (mut cursor, mut keys) = (0u64, std::collections::BTreeSet::new());
        loop {
            let (next, batch): (u64, Vec<String>) = self.command("SCAN", &[&cursor.to_string(), "COUNT", "1000"])?;
            for key in batch {
                keys.insert(key);
                if keys.len() > 10_000 {
                    let keys = keys.into_iter().take(10_000).collect();
                    return Ok((keys, true));
                }
            }
            cursor = next;
            if cursor == 0 {
                return Ok((keys.into_iter().collect(), false));
            }
        }
    }
    pub fn page(&mut self, t: &Table, _filter: &str, _order: &str, offset: usize) -> Result<Page, String> {
        let selected = self.database;
        let result = self.page_inner(t, offset);
        let restored = self.select(selected);
        match (result, restored) {
            (Err(e), _) => Err(e),
            (_, Err(e)) => Err(e),
            (Ok(p), _) => Ok(p),
        }
    }
    fn page_inner(&mut self, t: &Table, offset: usize) -> Result<Page, String> {
        self.select(t.schema.strip_prefix("db").ok_or("Banco inválido")?.parse().map_err(err)?)?;
        let kind: String = self.command("TYPE", &[&t.name])?;
        if kind != t.kind {
            return Err("O tipo da chave mudou ou a chave não existe. Recarregue a estrutura.".into());
        }
        let ttl = Some(self.command::<i64>("TTL", &[&t.name])?);
        let end = (offset + PAGE - 1).to_string();
        let start = offset.to_string();
        let (values, count): (Vec<Vec<Cell>>, usize) = match kind.as_str() {
            "string" => {
                let v = self.command::<Value>("GET", &[&t.name])?;
                (if offset == 0 { vec![vec![cell(v)]] } else { vec![] }, 1)
            }
            "hash" => {
                let mut all: Vec<(Vec<u8>, Vec<u8>)> = self.command("HGETALL", &[&t.name])?;
                all.sort_by(|a, b| a.0.cmp(&b.0));
                let count = all.len();
                (all.into_iter().skip(offset).take(PAGE).map(|(k, v)| vec![cell(Value::BulkString(k)), cell(Value::BulkString(v))]).collect(), count)
            }
            "list" => {
                let n = self.command::<usize>("LLEN", &[&t.name])?;
                let all: Vec<Value> = self.command("LRANGE", &[&t.name, &start, &end])?;
                (all.into_iter().enumerate().map(|(i, v)| vec![text(offset + i), cell(v)]).collect(), n)
            }
            "set" => {
                let mut all: Vec<Vec<u8>> = self.command("SMEMBERS", &[&t.name])?;
                all.sort();
                let n = all.len();
                (all.into_iter().skip(offset).take(PAGE).map(|v| vec![cell(Value::BulkString(v))]).collect(), n)
            }
            "zset" => {
                let n = self.command("ZCARD", &[&t.name])?;
                let all: Vec<(Vec<u8>, f64)> = self.command("ZRANGE", &[&t.name, &start, &end, "WITHSCORES"])?;
                (all.into_iter().map(|(v, s)| vec![cell(Value::BulkString(v)), text(s)]).collect(), n)
            }
            "stream" => {
                let n = self.command("XLEN", &[&t.name])?;
                let all: Vec<(String, Value)> = self.command("XRANGE", &[&t.name, "-", "+", "COUNT", &(offset + PAGE).to_string()])?;
                (all.into_iter().skip(offset).map(|(id, fields)| vec![text(id), cell(fields)]).collect(), n)
            }
            _ => return Err(format!("O valor da chave do tipo \"{kind}\" não pode ser mostrado como tabela.")),
        };
        let rows = values
            .into_iter()
            .map(|values| DataRow { rowid: values.first().cloned().unwrap_or(Cell::Null), original: Some(values.clone()), values, deleted: false })
            .collect();
        Ok(Page { rows, count, ttl })
    }
    pub fn submit(&mut self, t: &Table, rows: &[DataRow]) -> Result<(), String> {
        // Build and validate before sending MULTI, including all score/index conversions.
        let mut pipe = ::redis::pipe();
        pipe.atomic();
        let mut command_count = 0;
        for row in rows.iter().filter(|r| r.pending()) {
            let v = row.values.iter().map(bytes).collect::<Result<Vec<_>, _>>()?;
            let old = row.original.as_ref().map(|r| r.iter().map(bytes).collect::<Result<Vec<_>, _>>()).transpose()?;
            let mut cmd = |name: &str, args: Vec<Vec<u8>>| {
                pipe.cmd(name).arg(&t.name);
                for a in args {
                    pipe.arg(a);
                }
                pipe.ignore();
                command_count += 1;
            };
            match t.kind.as_str() {
                "string" => {
                    if row.deleted {
                        return Err("Use DEL no console para apagar a chave string.".into());
                    }
                    if v.len() != 1 {
                        return Err("Valor string inválido.".into());
                    }
                    cmd("SET", vec![v[0].clone(), b"KEEPTTL".to_vec()]);
                }
                "hash" => {
                    if let Some(o) = &old {
                        if row.deleted || o[0] != v[0] {
                            cmd("HDEL", vec![o[0].clone()]);
                        }
                    }
                    if !row.deleted {
                        cmd("HSET", v);
                    }
                }
                "list" => {
                    if row.deleted {
                        return Err("Apagar posições de lista não é suportado; use o console.".into());
                    }
                    if let Some(o) = old {
                        cmd("LSET", vec![o[0].clone(), v[1].clone()]);
                    } else {
                        cmd("RPUSH", vec![v[1].clone()]);
                    }
                }
                "set" | "zset" => {
                    if let Some(o) = old {
                        if row.deleted || o[0] != v[0] {
                            cmd(if t.kind == "set" { "SREM" } else { "ZREM" }, vec![o[0].clone()]);
                        }
                    }
                    if !row.deleted {
                        if t.kind == "zset" {
                            let score = row.values[1].text().parse::<f64>().map_err(|_| "Score inválido.")?;
                            if score.is_nan() {
                                return Err("Score inválido.".into());
                            }
                            cmd("ZADD", vec![v[1].clone(), v[0].clone()]);
                        } else {
                            cmd("SADD", vec![v[0].clone()]);
                        }
                    }
                }
                _ => return Err("Tipo de chave somente leitura.".into()),
            }
        }
        if command_count == 0 {
            return Ok(());
        }
        let selected = self.database;
        self.select(t.schema.strip_prefix("db").ok_or("Banco inválido")?.parse().map_err(err)?)?;
        let result = pipe.query::<()>(&mut self.wire).map_err(err);
        let restore = self.select(selected);
        result.and(restore)
    }
    pub fn query(&mut self, source: &str, readonly: bool) -> Result<Grid, String> {
        let mut result = redis_console::grid(Value::Nil);
        for line in source.lines().filter(|s| !s.trim().is_empty()) {
            let args = redis_console::parse(line)?;
            let Some(name) = args.first() else { continue };
            let name = std::str::from_utf8(name).map_err(err)?.to_ascii_uppercase();
            if readonly
                && !matches!(
                    name.as_str(),
                    "GET"
                        | "MGET"
                        | "HGET"
                        | "HGETALL"
                        | "HSCAN"
                        | "LRANGE"
                        | "LLEN"
                        | "SCARD"
                        | "SMEMBERS"
                        | "SSCAN"
                        | "ZRANGE"
                        | "ZCARD"
                        | "ZSCORE"
                        | "ZSCAN"
                        | "XRANGE"
                        | "XLEN"
                        | "TTL"
                        | "PTTL"
                        | "TYPE"
                        | "SCAN"
                        | "INFO"
                        | "DBSIZE"
                        | "PING"
                        | "SELECT"
                )
            {
                return Err("Comando indisponível na conexão somente leitura.".into());
            }
            // ponytail: interactive/blocking commands need a dedicated console connection.
            if matches!(name.as_str(), "MULTI" | "EXEC" | "DISCARD" | "WATCH" | "UNWATCH" | "SUBSCRIBE" | "PSUBSCRIBE" | "SSUBSCRIBE" | "MONITOR") {
                return Err("Comando indisponível neste console compartilhado.".into());
            }
            let value: Value = ::redis::cmd(&name).arg(&args[1..]).query(&mut self.wire).map_err(err)?;
            if name == "SELECT" {
                self.database = std::str::from_utf8(args.get(1).ok_or("Banco ausente")?).map_err(err)?.parse().map_err(err)?;
            }
            result = redis_console::grid(value);
        }
        Ok(result)
    }
}
