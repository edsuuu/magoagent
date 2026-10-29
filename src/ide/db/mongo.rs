//! Synchronous Mongo transport; BSON stays Extended JSON in editable cells.
use super::*;
use ::mongodb::{
    bson::{self, Bson, Document, doc},
    options::{ClientOptions, Credential, Tls},
    sync::{Client, Collection},
};
use session::{Catalog, Schema};

pub(super) struct Connection {
    pub client: Client,
    pub database: String,
}
impl Connection {
    pub fn options(c: &Conexao, password: String) -> Result<ClientOptions, String> {
        let host = if c.host.contains(':') && !c.host.starts_with('[') { format!("[{}]", c.host) } else { c.host.clone() };
        let mut options = ClientOptions::parse(format!("mongodb://{host}:{}/", c.porta)).run().map_err(err)?;
        options.server_selection_timeout = Some(Duration::from_secs(10));
        options.connect_timeout = Some(Duration::from_secs(10));
        if c.tls {
            options.tls = Some(Tls::Enabled(Default::default()));
        }
        if c.autenticacao == conexoes::Autenticacao::UsuarioSenha && !c.usuario.is_empty() {
            options.credential = Some(
                Credential::builder()
                    .username(c.usuario.clone())
                    .password(password)
                    .source(if c.banco_autenticacao.is_empty() { "admin".into() } else { c.banco_autenticacao.clone() })
                    .build(),
            );
        }
        Ok(options)
    }
    pub fn connect(c: &Conexao, password: String) -> Result<Self, String> {
        Ok(Self {
            client: Client::with_options(Self::options(c, password)?).map_err(err)?,
            database: if c.banco.is_empty() { "test".into() } else { c.banco.clone() },
        })
    }
    pub fn version(&self) -> Result<String, String> {
        let info = self.client.database(&self.database).run_command(doc! {"buildInfo": 1}).run().map_err(err)?;
        Ok(format!("Conectado: MongoDB {}", info.get_str("version").unwrap_or("?")))
    }
    pub fn collection(&self, database: &str, name: &str) -> Collection<Document> {
        self.client.database(database).collection(name)
    }
    pub fn catalog(&self) -> Result<Catalog, String> {
        let mut catalog = Catalog::default();
        for name in self.client.list_database_names().run().map_err(err)? {
            let mut schema = Schema { name: name.clone(), ..Default::default() };
            // A user that cannot read a database or collection (config.system.sessions even for root) still sees the rest.
            for collection in self.client.database(&name).list_collection_names().run().unwrap_or_default() {
                let c = self.collection(&name, &collection);
                let sample = c.find(doc! {}).limit(100).run().and_then(|docs| docs.collect::<Result<Vec<_>, _>>()).unwrap_or_default();
                let mut columns = infer(&sample);
                if columns.is_empty() {
                    columns.push(Column { name: "_id".into(), pk: 1, ..Default::default() });
                }
                let indexes = c
                    .list_indexes()
                    .run()
                    .and_then(|i| {
                        i.map(|i| i.map(|i| (i.options.and_then(|o| o.name).unwrap_or_default(), Bson::Document(i.keys).into_relaxed_extjson().to_string())))
                            .collect::<Result<Vec<_>, _>>()
                    })
                    .unwrap_or_default();
                schema.tables.push(Table { schema: name.clone(), name: collection, dialect: Dialect::Mongo, columns, indexes, ..Default::default() });
            }
            schema.tables.sort_by(|a, b| a.name.cmp(&b.name));
            catalog.schemas.push(schema);
        }
        catalog.schemas.sort_by(|a, b| a.name.cmp(&b.name));
        Ok(catalog)
    }
    pub fn page(&self, t: &Table, filter: &str, order: &str, offset: usize) -> Result<Page, String> {
        let c = self.collection(&t.schema, &t.name);
        let filter = document(if filter.trim().is_empty() { "{}" } else { filter })?;
        let order = document(if order.trim().is_empty() { "{_id:1}" } else { order })?;
        let count = c.count_documents(filter.clone()).run().map_err(err)? as usize;
        let docs = c.find(filter).sort(order).skip(offset as u64).limit(PAGE as i64).run().map_err(err)?;
        let rows = docs
            .map(|doc| {
                let doc = doc.map_err(err)?;
                let values = t.columns.iter().map(|c| doc.get(&c.name).map(cell).unwrap_or(Cell::Null)).collect::<Vec<_>>();
                let rowid = identity(&doc)?;
                Ok(DataRow { original: Some(values.clone()), values, rowid, deleted: false })
            })
            .collect::<Result<_, String>>()?;
        Ok(Page { rows, count, ttl: None })
    }
    pub fn submit(&self, t: &Table, rows: &[DataRow]) -> Result<(), String> {
        let c = self.collection(&t.schema, &t.name);
        for row in rows.iter().filter(|r| r.pending()) {
            if let Some(original) = &row.original {
                let target = predicate(&row.rowid)?;
                if row.deleted {
                    c.delete_one(target).run().map_err(err)?;
                } else {
                    let mut changes = Document::new();
                    for (i, col) in t.columns.iter().enumerate().filter(|(i, _)| row.values[*i] != original[*i]) {
                        if col.name == "_id" {
                            return Err("O campo _id não pode ser alterado.".into());
                        }
                        changes.insert(&col.name, input(&row.values[i]));
                    }
                    if !changes.is_empty() {
                        c.update_one(target, doc! {"$set": changes}).run().map_err(err)?;
                    }
                }
            } else if !row.deleted {
                let mut doc = Document::new();
                for (col, v) in t.columns.iter().zip(&row.values) {
                    if col.name == "_id" && *v == Cell::Null {
                        continue;
                    }
                    doc.insert(&col.name, input(v));
                }
                c.insert_one(doc).run().map_err(err)?;
            }
        }
        Ok(())
    }
}
pub(super) fn err(e: impl std::fmt::Display) -> String {
    e.to_string()
}
pub(super) fn value(s: &str) -> Result<Bson, String> {
    let json: serde_json::Value = json5::from_str(s).map_err(err)?;
    Bson::try_from(json).map_err(err)
}
pub(super) fn document(s: &str) -> Result<Document, String> {
    match value(s)? {
        Bson::Document(d) => Ok(d),
        _ => Err("Informe um objeto JSON.".into()),
    }
}
pub(super) fn input(c: &Cell) -> Bson {
    match c {
        Cell::Null => Bson::Null,
        _ => value(c.text()).unwrap_or_else(|_| Bson::String(c.text().into())),
    }
}
pub(super) fn cell(v: &Bson) -> Cell {
    // Quotes distinguish a BSON string like "123" from a number when submitting it back.
    Cell::Text(v.clone().into_relaxed_extjson().to_string())
}
pub(super) fn grid(docs: Vec<Document>) -> Grid {
    let columns = infer(&docs).into_iter().map(|c| c.name).collect::<Vec<_>>();
    let rows = docs.iter().map(|d| columns.iter().map(|c| d.get(c).map(cell).unwrap_or(Cell::Null)).collect()).collect();
    Grid { columns, rows }
}
fn kind(v: &Bson) -> &'static str {
    match v {
        Bson::Double(_) => "double",
        Bson::String(_) => "string",
        Bson::Array(_) => "array",
        Bson::Document(_) => "object",
        Bson::Boolean(_) => "bool",
        Bson::Null => "null",
        Bson::Int32(_) => "int",
        Bson::Int64(_) => "long",
        Bson::ObjectId(_) => "objectId",
        Bson::DateTime(_) => "date",
        Bson::Binary(_) => "binData",
        Bson::Decimal128(_) => "decimal",
        Bson::RegularExpression(_) => "regex",
        Bson::Timestamp(_) => "timestamp",
        Bson::MinKey => "minKey",
        Bson::MaxKey => "maxKey",
        Bson::JavaScriptCode(_) => "javascript",
        Bson::JavaScriptCodeWithScope(_) => "javascriptWithScope",
        Bson::Symbol(_) => "symbol",
        Bson::Undefined => "undefined",
        Bson::DbPointer(_) => "dbPointer",
    }
}
fn merge(columns: &mut Vec<Column>, doc: &Document) {
    for (name, v) in doc {
        let i = columns.iter().position(|c| &c.name == name).unwrap_or_else(|| {
            columns.push(Column { name: name.clone(), pk: usize::from(name == "_id"), ..Default::default() });
            columns.len() - 1
        });
        let c = &mut columns[i];
        if !c.kind.split(" | ").any(|k| k == kind(v)) {
            if !c.kind.is_empty() {
                c.kind.push_str(" | ");
            }
            c.kind.push_str(kind(v));
        }
        if let Bson::Document(d) = v {
            merge(&mut c.children, d);
        }
        if let Bson::Array(a) = v {
            for v in a {
                if let Bson::Document(d) = v {
                    merge(&mut c.children, d);
                }
            }
        }
    }
    columns.sort_by(|a, b| (a.name != "_id", &a.name).cmp(&(b.name != "_id", &b.name)));
}
pub(super) fn infer(docs: &[Document]) -> Vec<Column> {
    let mut columns = vec![];
    for doc in docs {
        merge(&mut columns, doc);
    }
    columns
}

// Raw BSON preserves numeric types and field order even for an embedded document _id.
pub(super) fn identity(doc: &Document) -> Result<Cell, String> {
    let id = doc.get("_id").ok_or("Documento sem _id.")?;
    let bytes = bson::to_vec(&doc! {"_id": id.clone()}).map_err(err)?;
    Ok(Cell::Blob(bytes.iter().map(|b| format!("{b:02X}")).collect()))
}
pub(super) fn predicate(id: &Cell) -> Result<Document, String> {
    let Cell::Blob(hex) = id else {
        return Err("Identidade BSON ausente.".into());
    };
    let bytes = (0..hex.len())
        .step_by(2)
        .map(|i| hex.get(i..i + 2).ok_or("Identidade BSON inválida.".into()).and_then(|s| u8::from_str_radix(s, 16).map_err(err)))
        .collect::<Result<Vec<_>, String>>()?;
    bson::from_slice(&bytes).map_err(err)
}
