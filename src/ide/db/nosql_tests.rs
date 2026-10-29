use super::*;
use ::mongodb::bson::{Bson, doc};
use conexoes::Autenticacao;

fn text(s: &str) -> Cell {
    Cell::Text(s.into())
}
#[test]
fn mongo_console_parses_relaxed_json_and_pipelines() {
    use mongo_console::{Command, parse};
    let Command::Collection(c, op, args) = parse("db.c.find({a:1})").unwrap() else { panic!() };
    assert_eq!((c.as_str(), op.as_str()), ("c", "find"));
    assert_eq!(args, vec![Bson::Document(doc! {"a": 1})]);
    let Command::Collection(_, _, args) = parse("db.c.find({name:'João', a: [1,2,],}, {_id: 0,}) ;").unwrap() else { panic!() };
    assert_eq!(args[0].as_document().unwrap().get_str("name"), Ok("João"));
    let Command::Collection(_, op, args) = parse("db.c.aggregate([{$match:{a:1}},{$group:{_id:'$a',total:{$sum:1}}},])").unwrap() else { panic!() };
    assert_eq!(op, "aggregate");
    assert_eq!(args[0].as_array().unwrap().len(), 2);
    assert!(matches!(parse("db.runCommand({ping:1})"), Ok(Command::Run(_))));
    assert!(matches!(parse("use loja"), Ok(Command::Use(db)) if db == "loja"));
    assert!(parse("db.c.find({a:1}").is_err());
    assert!(parse("db.c.find({a:})").is_err());
    assert!(parse("db.c.find({a: ObjectId('abc')})").is_err());
}
#[test]
fn mongo_bson_cells_and_inference_keep_nested_types_and_identity() {
    let oid = Bson::ObjectId(::mongodb::bson::oid::ObjectId::new());
    assert_eq!(mongo::value(mongo::cell(&oid).text()).unwrap(), oid);
    assert_eq!(mongo::value(mongo::cell(&Bson::Int64(i64::MAX)).text()).unwrap(), Bson::Int64(i64::MAX));
    assert_eq!(mongo::input(&mongo::cell(&Bson::String("123".into()))), Bson::String("123".into()));
    assert_eq!(mongo::input(&text("texto livre")), Bson::String("texto livre".into()));
    let nested = doc! {"_id": {"z": Bson::Int64(1), "a": 2}, "ignored": true};
    assert_eq!(mongo::predicate(&mongo::identity(&nested).unwrap()).unwrap(), doc! {"_id": nested.get("_id").unwrap().clone()});
    let target = mongo::predicate(&mongo::identity(&nested).unwrap()).unwrap();
    assert_eq!(target.get_document("_id").unwrap().keys().map(String::as_str).collect::<Vec<_>>(), ["z", "a"]);
    let inferred = mongo::infer(&[doc! {"obj": {"x":1}, "a": Bson::Undefined}, doc! {"obj": {"x":"str"}, "a": Bson::Symbol("s".into())}]);
    assert_eq!(inferred.iter().find(|c| c.name == "a").unwrap().kind, "undefined | symbol");
    assert_eq!(inferred.iter().find(|c| c.name == "obj").unwrap().children[0].kind, "int | string");
    let g = mongo::grid(vec![doc! {"z": 1, "_id": oid, "obj": {"a":1}}, doc! {"z":"x", "extra":true}]);
    assert_eq!(g.columns[0], "_id");
    assert!(g.columns.contains(&"extra".into()));
    assert_eq!(g.rows.len(), 2);
}
#[test]
fn redis_console_quotes_escapes_empty_arguments_and_binary() {
    let args = redis_console::parse(r#"SET "a b" 'João\'s'"#).unwrap();
    assert_eq!(args, vec![b"SET".to_vec(), b"a b".to_vec(), "João's".as_bytes().to_vec()]);
    assert_eq!(redis_console::parse(r#"SET k "a\n\t\r\b\a\x00\xff\"\\""#).unwrap()[2], b"a\n\t\r\x08\x07\x00\xff\"\\");
    assert_eq!(redis_console::parse("SET k ''").unwrap()[2], b"");
    assert_eq!(redis_console::parse(r"GET a\b").unwrap()[1], br"a\b");
    for invalid in ["SET k 'x", "SET k \"x", "GET 'x'y", r#"SET k "\xzz""#] {
        assert!(redis_console::parse(invalid).is_err(), "{invalid}");
    }
    let g = redis_console::grid(::redis::Value::Map(vec![(::redis::Value::SimpleString("a".into()), ::redis::Value::Int(2))]));
    assert_eq!(g.columns, ["campo", "valor"]);
    assert_eq!(g.rows, [vec![text("a"), text("2")]]);
    let g = redis_console::grid(::redis::Value::Array(vec![::redis::Value::Int(1), ::redis::Value::Nil]));
    assert_eq!(g.rows, [vec![text("1")], vec![Cell::Null]]);
    assert_eq!(redis_console::bytes(&Cell::Blob("00FF".into())).unwrap(), [0, 255]);
}
#[test]
fn redis_key_folders_preserve_empty_segments_and_prefix_keys() {
    let tables = ["a", "a:b", "a:b:c", "a::x", ":x", "a:b:"].map(|name| Table { name: name.into(), ..Default::default() });
    let root = redis_tree::group(&tables);
    assert_eq!(root.keys, [0]);
    let a = &root.folders["a"];
    assert_eq!(a.keys, [1]);
    assert_eq!(a.folders["b"].keys, [2, 5]);
    assert_eq!(a.folders[""].keys, [3]);
    assert_eq!(root.folders[""].keys, [4]);
}
#[test]
fn nosql_no_auth_really_omits_stale_credentials() {
    let c = Conexao {
        host: "localhost".into(),
        porta: 27017,
        usuario: "stale".into(),
        autenticacao: Autenticacao::Sem,
        banco_autenticacao: "admin".into(),
        ..Default::default()
    };
    assert!(mongo::Connection::options(&c, "secret".into()).unwrap().credential.is_none());
    let info = redis::Connection::info(&c, "secret".into()).unwrap();
    assert!(info.redis.username.is_none() && info.redis.password.is_none());
    let c = Conexao { autenticacao: Autenticacao::UsuarioSenha, ..c };
    let opts = mongo::Connection::options(&c, "secret".into()).unwrap();
    let auth = opts.credential.unwrap();
    assert_eq!(auth.source.as_deref(), Some("admin"));
    assert_eq!(auth.username.as_deref(), Some("stale"));
    let info = redis::Connection::info(&c, "secret".into()).unwrap();
    assert_eq!(info.redis.username.as_deref(), Some("stale"));
    assert_eq!(info.redis.password.as_deref(), Some("secret"));
}

struct MongoServer {
    c: mongo::Connection,
    name: String,
}
impl MongoServer {
    fn new() -> Self {
        let url = std::env::var("MAGO_MONGO_URL").expect("Defina MAGO_MONGO_URL para um servidor de teste");
        let name = format!("mago_test_{}", uuid::Uuid::new_v4().simple());
        Self { c: mongo::Connection { client: ::mongodb::sync::Client::with_uri_str(&url).unwrap(), database: name.clone() }, name }
    }
}
impl Drop for MongoServer {
    fn drop(&mut self) {
        let _ = self.c.client.database(&self.name).drop().run();
    }
}
#[test]
#[ignore = "Requer MAGO_MONGO_URL; cria e remove um banco temporário"]
fn mongo_server_catalog_grid_submit_and_console() {
    let mut server = MongoServer::new();
    let c = &mut server.c;
    assert!(c.version().unwrap().starts_with("Conectado: MongoDB "));
    c.query("db.people.insertMany([{name:'João', a:1, obj:{x:true}}, {name:'Ana', a:'x', obj:{x:2}}])", false).unwrap();
    c.query("db.runCommand({createIndexes:'people', indexes:[{key:{name:1}, name:'by_name'}]})", false).unwrap();
    let catalog = c.catalog().unwrap();
    let t = catalog.schemas.iter().find(|s| s.name == server.name).unwrap().tables.iter().find(|t| t.name == "people").unwrap();
    assert_eq!(t.columns[0].name, "_id");
    assert!(t.columns.iter().find(|c| c.name == "a").unwrap().kind.contains("string"));
    assert_eq!(t.columns.iter().find(|c| c.name == "obj").unwrap().children[0].kind, "bool | int");
    assert!(t.indexes.iter().any(|(n, _)| n == "by_name"));
    let page = c.page(t, "{}", "{name:1}", 0).unwrap();
    assert_eq!(page.count, 2);
    let i = t.columns.iter().position(|c| c.name == "name").unwrap();
    let mut row = page.rows[0].clone();
    row.values[i] = text("alterado");
    c.submit(t, &[row.clone()]).unwrap();
    assert_eq!(c.query("db.people.find({name:'alterado'}, {_id:0,name:1})", false).unwrap().rows[0][0], text("\"alterado\""));
    let mut new = row.clone();
    new.original = None;
    new.values[0] = Cell::Null;
    c.submit(t, &[new]).unwrap();
    assert_eq!(c.page(t, "{}", "", PAGE).unwrap().count, 3);
    row.deleted = true;
    c.submit(t, &[row]).unwrap();
    assert_eq!(c.query("db.people.countDocuments({})", false).unwrap().rows[0][0], text("2"));
    assert_eq!(c.query("db.people.aggregate([{$match:{name:'alterado'}}])", false).unwrap().rows.len(), 1);
    c.query("db.people.updateMany({}, {$set:{a:2}})", false).unwrap();
    c.query("db.people.updateOne({}, {$set:{a:3}})", false).unwrap();
    assert_eq!(c.query("db.people.findOne({})", false).unwrap().rows.len(), 1);
    c.query("db.people.insertOne({a:4})", false).unwrap();
    c.query("db.people.deleteOne({a:4})", false).unwrap();
    c.query("db.people.deleteMany({})", false).unwrap();
    assert!(c.query("db.people.insertOne({a:1})", true).is_err());
    assert!(c.query("db.people.aggregate([{$out:'copy'}])", true).is_err());
    c.query("use admin", false).unwrap();
    assert_eq!(c.database, "admin");
}

struct RedisServer {
    c: redis::Connection,
    prefix: String,
}
impl RedisServer {
    fn new() -> Self {
        let url = std::env::var("MAGO_REDIS_URL").expect("Defina MAGO_REDIS_URL para um servidor de teste");
        let client = ::redis::Client::open(url).unwrap();
        let database = client.get_connection_info().redis.db;
        Self { c: redis::Connection { wire: client.get_connection().unwrap(), database }, prefix: format!("mago_test_{}", uuid::Uuid::new_v4().simple()) }
    }
    fn table(&mut self, suffix: &str) -> Table {
        let catalog = self.c.catalog().unwrap();
        catalog.schemas.iter().flat_map(|s| &s.tables).find(|t| t.name == format!("{}:{suffix}", self.prefix)).unwrap().clone()
    }
}
impl Drop for RedisServer {
    fn drop(&mut self) {
        for suffix in ["s", "h", "l", "set", "z", "stream"] {
            let _: Result<(), _> = ::redis::cmd("DEL").arg(format!("{}:{suffix}", self.prefix)).query(&mut self.c.wire);
        }
    }
}
#[test]
#[ignore = "Requer MAGO_REDIS_URL; cria e remove chaves com prefixo temporário"]
fn redis_server_catalog_grid_submit_and_console() {
    let mut server = RedisServer::new();
    let p = server.prefix.clone();
    assert!(server.c.version().unwrap().starts_with("Conectado: Redis "));
    server
        .c
        .query(
            &format!("SET {p}:s 'antes' EX 120\nHSET {p}:h a 1\nRPUSH {p}:l a b\nSADD {p}:set a b\nZADD {p}:z 1 a 2 b\nXADD {p}:stream '*' campo valor"),
            false,
        )
        .unwrap();
    let catalog = server.c.catalog().unwrap();
    assert!(catalog.schemas.iter().any(|s| s.name == format!("db{}", server.c.database) && s.count.unwrap() > 0));
    for (suffix, index, replacement) in [("s", 0, "depois"), ("h", 1, "2"), ("l", 1, "c"), ("set", 0, "c"), ("z", 1, "3")] {
        let table = server.table(suffix);
        let page = server.c.page(&table, "", "", 0).unwrap();
        assert!(!page.rows.is_empty());
        let mut row = page.rows[0].clone();
        row.values[index] = text(replacement);
        server.c.submit(&table, &[row]).unwrap();
        let page = server.c.page(&table, "", "", 0).unwrap();
        assert!(page.rows.iter().any(|r| r.values[index] == text(replacement)));
        if suffix == "s" {
            assert!(page.ttl.unwrap() > 0);
        } else {
            let mut new = page.rows[0].clone();
            new.original = None;
            match suffix {
                "h" => new.values[0] = text("new"),
                "l" => new.values[1] = text("new"),
                "set" | "z" => new.values[0] = text("new"),
                _ => {}
            }
            server.c.submit(&table, &[new]).unwrap();
            assert_eq!(server.c.page(&table, "", "", 0).unwrap().count, page.count + 1);
            if suffix != "l" {
                let mut row = server.c.page(&table, "", "", 0).unwrap().rows.remove(0);
                row.deleted = true;
                server.c.submit(&table, &[row]).unwrap();
            }
        }
    }
    let stream = server.table("stream");
    let page = server.c.page(&stream, "", "", 0).unwrap();
    assert_eq!(page.rows[0].values.len(), 2);
    let unknown = Table { kind: "ReJSON-RL".into(), ..server.table("s") };
    assert!(server.c.page(&unknown, "", "", 0).is_err());
    let g = server.c.query(&format!("GET {p}:s"), false).unwrap();
    assert_eq!(g.rows[0][0], text("depois"));
    assert!(server.c.query(&format!("SET {p}:s x"), true).is_err());
}

#[test]
fn nosql_console_selects_current_shell_call_and_redis_line() {
    use view::instruction_mode;
    let source = "use loja\ndb.c.find(\n {a:'x;y',},\n {_id:1}\n);\ndb.c.aggregate([{$match:{a:1}}])";
    let caret = source.find("{a:").unwrap();
    assert_eq!(instruction_mode(source, (caret, caret), Dialect::Mongo, false), "db.c.find(\n {a:'x;y',},\n {_id:1}\n);");
    assert_eq!(instruction_mode(source, (0, 0), Dialect::Mongo, false), "use loja");
    let caret = source.find("db.c.aggregate").unwrap();
    assert!(instruction_mode(source, (caret, caret), Dialect::Mongo, false).starts_with("db.c.aggregate"));
    let source = "SET k 'á b'\nGET k";
    let caret = source.chars().count();
    assert_eq!(instruction_mode(source, (caret, caret), Dialect::Redis, false), "GET k");
    assert_eq!(instruction_mode(source, (0, caret), Dialect::Redis, false), source);
}
