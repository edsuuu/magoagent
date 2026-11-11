//! A deliberately small shell grammar; arguments are JSON5, never JavaScript evaluation.
use super::*;
use ::mongodb::bson::{Bson, Document, doc};
use mongo::{Connection, document, err, grid};

#[derive(Debug)]
pub(super) enum Command {
    Use(String),
    Run(Document),
    Collection(String, String, Vec<Bson>),
}
pub(super) fn parse(s: &str) -> Result<Command, String> {
    let s = s.trim().trim_end_matches(';').trim();
    if let Some(db) = s.strip_prefix("use ") {
        let db = db.trim();
        if db.is_empty() || db.chars().any(char::is_whitespace) {
            return Err("Informe o banco após use.".into());
        }
        return Ok(Command::Use(db.into()));
    }
    let (head, tail) = s.split_once('(').ok_or("Use db.coleção.operação(...).")?;
    let args = tail.strip_suffix(')').ok_or("Falta fechar os argumentos com ).")?;
    if head == "db.runCommand" {
        return Ok(Command::Run(document(args)?));
    }
    let (collection, method) = head.strip_prefix("db.").and_then(|s| s.rsplit_once('.')).ok_or("Use db.coleção.operação(...).")?;
    if collection.is_empty() {
        return Err("Informe a coleção.".into());
    }
    let Bson::Array(args) = mongo::value(&format!("[{args}]"))? else { unreachable!() };
    Ok(Command::Collection(collection.into(), method.into(), args))
}
fn doc_arg(args: &[Bson], i: usize, optional: bool) -> Result<Document, String> {
    match args.get(i) {
        Some(Bson::Document(d)) => Ok(d.clone()),
        None if optional => Ok(doc! {}),
        _ => Err(format!("Argumento {} deve ser um objeto JSON.", i + 1)),
    }
}
fn docs_arg(args: &[Bson]) -> Result<Vec<Document>, String> {
    match args.first() {
        Some(Bson::Array(a)) => a
            .iter()
            .map(|v| match v {
                Bson::Document(d) => Ok(d.clone()),
                _ => Err("Esperada uma lista de objetos.".into()),
            })
            .collect(),
        _ => Err("Esperada uma lista de objetos.".into()),
    }
}
fn scalar(name: &str, v: Bson) -> Grid {
    Grid { columns: vec![name.into()], rows: vec![vec![mongo::cell(&v)]] }
}
impl Connection {
    pub fn query(&mut self, source: &str, readonly: bool) -> Result<Grid, String> {
        match parse(source)? {
            Command::Use(db) => {
                self.database = db;
                Ok(scalar("banco", Bson::String(self.database.clone())))
            }
            Command::Run(cmd) => {
                if readonly {
                    return Err("runCommand indisponível na conexão somente leitura.".into());
                }
                Ok(grid(vec![self.client.database(&self.database).run_command(cmd).run().map_err(err)?]))
            }
            Command::Collection(name, method, args) => {
                let c = self.collection(&self.database, &name);
                if readonly && !matches!(method.as_str(), "find" | "findOne" | "countDocuments" | "aggregate") {
                    return Err("Conexão somente leitura.".into());
                }
                match method.as_str() {
                    "find" | "findOne" => {
                        if args.len() > 2 {
                            return Err("find aceita filtro e projeção.".into());
                        }
                        let filter = doc_arg(&args, 0, true)?;
                        let projection = doc_arg(&args, 1, true)?;
                        if method == "findOne" {
                            Ok(grid(c.find_one(filter).projection(projection).run().map_err(err)?.into_iter().collect()))
                        } else {
                            Ok(grid(c.find(filter).projection(projection).run().map_err(err)?.collect::<Result<Vec<_>, _>>().map_err(err)?))
                        }
                    }
                    "countDocuments" => Ok(scalar("count", Bson::Int64(c.count_documents(doc_arg(&args, 0, true)?).run().map_err(err)? as i64))),
                    "aggregate" => {
                        let pipeline = docs_arg(&args)?;
                        if readonly && pipeline.iter().any(|d| d.contains_key("$out") || d.contains_key("$merge")) {
                            return Err("Conexão somente leitura.".into());
                        }
                        Ok(grid(c.aggregate(pipeline).run().map_err(err)?.collect::<Result<Vec<_>, _>>().map_err(err)?))
                    }
                    "insertOne" => Ok(scalar("insertedId", c.insert_one(doc_arg(&args, 0, false)?).run().map_err(err)?.inserted_id)),
                    "insertMany" => {
                        let r = c.insert_many(docs_arg(&args)?).run().map_err(err)?;
                        Ok(scalar("insertedCount", Bson::Int64(r.inserted_ids.len() as i64)))
                    }
                    "updateOne" | "updateMany" => {
                        let (filter, update) = (doc_arg(&args, 0, false)?, doc_arg(&args, 1, false)?);
                        let r = if method == "updateOne" { c.update_one(filter, update).run() } else { c.update_many(filter, update).run() }.map_err(err)?;
                        Ok(grid(vec![doc! {"matchedCount": r.matched_count as i64, "modifiedCount": r.modified_count as i64}]))
                    }
                    "deleteOne" | "deleteMany" => {
                        let filter = doc_arg(&args, 0, false)?;
                        let r = if method == "deleteOne" { c.delete_one(filter).run() } else { c.delete_many(filter).run() }.map_err(err)?;
                        Ok(scalar("deletedCount", Bson::Int64(r.deleted_count as i64)))
                    }
                    _ => Err(format!("Operação MongoDB desconhecida: {method}")),
                }
            }
        }
    }
}

// Newlines end shell commands only outside JSON and call arguments.
pub(super) fn ranges(source: &str) -> Vec<std::ops::Range<usize>> {
    let (mut start, mut depth, mut quote, mut escape) = (0, 0usize, None, false);
    let mut ranges = vec![];
    for (i, c) in source.char_indices() {
        if let Some(q) = quote {
            if escape {
                escape = false;
            } else if c == '\\' {
                escape = true;
            } else if c == q {
                quote = None;
            }
        } else if matches!(c, '\'' | '"') {
            quote = Some(c);
        } else if matches!(c, '(' | '[' | '{') {
            depth += 1;
        } else if matches!(c, ')' | ']' | '}') {
            depth = depth.saturating_sub(1);
        } else if depth == 0 && matches!(c, ';' | '\n') {
            if !source[start..i].trim().is_empty() {
                ranges.push(start..i + c.len_utf8());
            }
            start = i + c.len_utf8();
        }
    }
    if !source[start..].trim().is_empty() {
        ranges.push(start..source.len());
    }
    ranges
}
