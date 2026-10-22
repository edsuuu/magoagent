//! One queue and one worker per data source. Tabs share the same wire session.
use super::*;
use conexoes::{Conexao, Motor};
use std::sync::{Arc, atomic::{AtomicBool, AtomicU64, Ordering}, mpsc::{channel, Sender}};

#[derive(Clone, Default)]
pub(super) struct Catalog { pub schemas: Vec<Schema> }
#[derive(Clone, Default)]
pub(super) struct Schema { pub name: String, pub tables: Vec<Table>, pub objects: Vec<Object> }
#[derive(Clone)]
pub(super) struct Object { pub schema: String, pub name: String, pub kind: ObjectKind }
#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum ObjectKind { Table, View, Procedure, Function, Trigger, Event }
impl ObjectKind {
    pub fn sql(self) -> &'static str { match self { Self::Table => "TABLE", Self::View => "VIEW", Self::Procedure => "PROCEDURE", Self::Function => "FUNCTION", Self::Trigger => "TRIGGER", Self::Event => "EVENT" } }
}

pub(super) enum Op {
    Schema, Test, Page(Table, String, String, usize), Submit(Table, Vec<DataRow>),
    Console(String), Auto(bool), Commit, Rollback, Ddl(Object),
}
pub(super) enum Job {
    Schema(Result<Catalog, String>), Test(Result<String, String>), Page(Result<Page, String>),
    Submit(Result<(), String>), Console(Result<Grid, String>, u128), Ddl(Result<String, String>),
}
struct Request { op: Op, reply: Sender<Job>, ctx: egui::Context }
#[derive(Clone)]
pub(super) struct Session {
    tx: Sender<Request>, pub id: String, pub mysql: bool, pub readonly: bool,
    auto: Arc<AtomicBool>,
    revision: Arc<AtomicU64>,
    no_backslash: Arc<AtomicBool>,
}

enum Conn { Sqlite(PathBuf), Mysql(mysql::Connection) }
impl Session {
    pub fn new(c: Conexao, password: String) -> Self {
        let (tx, rx) = channel::<Request>();
        let auto = Arc::new(AtomicBool::new(c.auto_commit));
        let revision = Arc::new(AtomicU64::new(0));
        let no_backslash = Arc::new(AtomicBool::new(false));
        let session = Self { tx, id: c.id.clone(), mysql: c.motor.mysql(), readonly: c.somente_leitura, auto: auto.clone(), revision: revision.clone(), no_backslash: no_backslash.clone() };
        std::thread::spawn(move || {
            // Connect lazily; disconnecting drops a failed session and its credentials.
            let mut config = Some((c.clone(), password));
            let mut conn: Option<Result<Conn, String>> = None;
            while let Ok(req) = rx.recv() {
                if conn.is_none() {
                    let (c, password) = config.take().unwrap();
                    conn = Some(match c.motor {
                        Motor::Sqlite => Ok(Conn::Sqlite(c.arquivo.into())),
                        Motor::Mysql | Motor::Mariadb => mysql::Connection::connect(&c, password).map(Conn::Mysql),
                        _ => Err("Motor em breve.".into()),
                    });
                }
                let changed = matches!(&req.op, Op::Submit(..) | Op::Console(_) | Op::Auto(_) | Op::Commit | Op::Rollback);
                let result = match conn.as_mut().unwrap() {
                    Ok(conn) => conn.handle(req.op, &auto, c.somente_leitura),
                    Err(e) => failure(req.op, e.clone()),
                };
                if let Some(Ok(Conn::Mysql(c))) = conn.as_mut() {
                    if let Ok((is_auto, escapes)) = c.state() {
                        auto.store(is_auto, Ordering::Relaxed);
                        no_backslash.store(escapes, Ordering::Relaxed);
                    }
                }
                if changed && !matches!(&result, Job::Submit(Err(_))) { revision.fetch_add(1, Ordering::Relaxed); }
                let _ = req.reply.send(result);
                req.ctx.request_repaint();
            }
            if let Some(Ok(Conn::Mysql(mut c))) = conn { let _ = c.rollback(); }
        });
        session
    }
    pub fn auto(&self) -> bool { self.auto.load(Ordering::Relaxed) }
    pub fn revision(&self) -> u64 { self.revision.load(Ordering::Relaxed) }
    pub fn no_backslash(&self) -> bool { self.no_backslash.load(Ordering::Relaxed) }
    pub fn request(&self, op: Op, ctx: &egui::Context) -> Receiver<Job> {
        let (reply, rx) = channel();
        let _ = self.tx.send(Request { op, reply, ctx: ctx.clone() });
        rx
    }
}

fn failure(op: Op, e: String) -> Job {
    match op {
        Op::Schema => Job::Schema(Err(e)), Op::Test => Job::Test(Err(e)), Op::Page(..) => Job::Page(Err(e)),
        Op::Submit(..) | Op::Auto(_) | Op::Commit | Op::Rollback => Job::Submit(Err(e)),
        Op::Console(_) => Job::Console(Err(e), 0), Op::Ddl(_) => Job::Ddl(Err(e)),
    }
}
impl Conn {
    fn handle(&mut self, op: Op, auto: &AtomicBool, readonly: bool) -> Job {
        match (self, op) {
            (Self::Sqlite(file), Op::Schema) => Job::Schema(sql::schema(file).map(|tables| Catalog { schemas: vec![Schema { name: "main".into(), tables, objects: vec![] }] })),
            (Self::Mysql(c), Op::Schema) => Job::Schema(c.catalog()),
            (Self::Sqlite(file), Op::Test) => Job::Test(sql::query(file, "SELECT sqlite_version()").map(|g| format!("Conectado: SQLite {}", g.rows[0][0].text()))),
            (Self::Mysql(c), Op::Test) => Job::Test(c.version()),
            (Self::Sqlite(file), Op::Page(t, f, o, n)) => Job::Page(sql::load_page(file, &t, &f, &o, n)),
            (Self::Mysql(c), Op::Page(t, f, o, n)) => Job::Page(c.page(&t, &f, &o, n)),
            (Self::Sqlite(file), Op::Submit(t, rows)) => Job::Submit(if readonly { Err("Conexão somente leitura.".into()) } else { sql::submit(file, &t, &rows) }),
            (Self::Mysql(c), Op::Submit(t, rows)) => Job::Submit(if readonly { Err("Conexão somente leitura.".into()) } else { c.submit(&t, &rows) }),
            (Self::Sqlite(file), Op::Console(s)) => {
                let start = Instant::now();
                let r = if readonly { sql::query(file, &s) } else { sql::execute(file, &format!("PRAGMA foreign_keys=ON;\n{s}")) };
                Job::Console(r, start.elapsed().as_millis())
            }
            (Self::Mysql(c), Op::Console(s)) => { let start = Instant::now(); Job::Console(c.query(&s), start.elapsed().as_millis()) }
            (Self::Mysql(c), Op::Auto(value)) => {
                let result = c.set_auto(value);
                if result.is_ok() { auto.store(value, Ordering::Relaxed); }
                Job::Submit(result)
            }
            (Self::Mysql(c), Op::Commit) => Job::Submit(c.commit()),
            (Self::Mysql(c), Op::Rollback) => Job::Submit(c.rollback()),
            (Self::Mysql(c), Op::Ddl(o)) => Job::Ddl(c.ddl(&o)),
            (Self::Sqlite(file), Op::Ddl(o)) => Job::Ddl(sql::query(file, &format!("SELECT sql FROM sqlite_master WHERE name = {}", format!("'{}'", o.name.replace('\'', "''")))).map(|g| g.rows.first().map(|r| r[0].text().to_string()).unwrap_or_default())),
            (_, op) => failure(op, "Operação indisponível para este motor.".into()),
        }
    }
}
