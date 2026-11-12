//! Database sources, background sessions and editor tabs. Hosted by the IDE node and by the file
//! tree node: each keeps a `Db` and its tabs of `Sheet`s, and applies the `Event`s the panel returns.
use crate::ide::chrome::row;
use crate::ide::text::byte_at;
use crate::ide::{chrome, highlight};
use crate::ide::{icons, theme as t};
use conexoes::{Conexao, Motor, Senhas};
use eframe::egui::{self, Align, Align2, Color32, Frame, Id, Key, Margin, Modifiers, Rect, Sense, Stroke, TextEdit, Ui, pos2, vec2};
use session::{Catalog, Job, Op, Session};
use sql::*;
use std::path::{Path, PathBuf};
use std::sync::mpsc::Receiver;
use std::time::{Duration, Instant};

mod conexoes;
mod dialog;
mod grid;
mod instruction;
mod look;
#[cfg(test)]
mod menu_tests;
mod mongo;
mod mongo_console;
mod mysql;
mod mysql_sql;
#[cfg(test)]
mod mysql_tests;
#[cfg(test)]
mod nosql_tests;
mod postgres;
mod postgres_catalog;
mod postgres_sql;
#[cfg(test)]
mod postgres_tests;
mod redis;
mod redis_console;
mod redis_tree;
mod session;
mod sql;
#[cfg(test)]
mod tests;
mod tree;
mod view;

#[cfg(not(test))]
type Vault = conexoes::Sistema;
#[cfg(test)]
type Vault = conexoes::Memoria;

#[derive(Default)]
pub(crate) struct Db {
    initialized: bool,
    /// connections.json was read: at once; the project's SQLite files join after the file listing
    loaded: bool,
    conexoes: Vec<Conexao>,
    active: Option<String>,
    sessions: std::collections::HashMap<String, Session>,
    catalogs: std::collections::HashMap<String, Catalog>,
    loading: std::collections::HashMap<String, Receiver<Job>>,
    senhas: Senhas<Vault>,
    dialog: Option<dialog::Dialog>,
    open: std::collections::HashSet<String>,
    error: Option<String>,
    seq: u64,
    /// Where the connections are saved; None is the app's own `connections.json`.
    store: Option<PathBuf>,
}
pub(crate) use view::Sheet;

/// What the panel asks of its host: a tab for a table, a console or an object's DDL, or the tabs
/// of a source that was disconnected closed.
pub(crate) enum Event {
    Open(Open),
    Closed(String),
}

/// A table's grid, a console, or an object's DDL in a console, of the source that was active.
pub(crate) struct Open {
    session: String,
    meta: Option<Table>,
    ddl: Option<session::Object>,
}
impl Open {
    /// A tab of `sheet` already shows it (a console is never shared).
    pub(crate) fn is_shown_by(&self, sheet: &Sheet) -> bool {
        self.ddl.is_none() && sheet.matches(&self.session, self.meta.as_ref().map_or("", |t| t.schema.as_str()), self.meta.as_ref().map(|t| t.name.as_str()))
    }
}

pub(crate) fn is_sqlite(path: &Path) -> bool {
    let mut head = [0u8; 16];
    std::fs::File::open(path).and_then(|mut f| std::io::Read::read_exact(&mut f, &mut head)).is_ok() && &head == b"SQLite format 3\0"
}

/// A file the tree opens that is a SQLite source: by its name, then by its first bytes.
pub(crate) fn sqlite_file(path: &Path) -> bool {
    matches!(path.extension().and_then(|e| e.to_str()).map(str::to_lowercase).as_deref(), Some("sqlite" | "sqlite3" | "db")) && is_sqlite(path)
}

/// Only a read: what the MCP may run. SQLite is also opened read only, so a trick still fails there.
pub(crate) fn read_only_sql(sql: &str) -> Result<(), String> {
    let text = sql.trim().trim_end_matches(';').trim();
    let lower = text.to_lowercase();
    let words: Vec<&str> = lower.split(|c: char| !c.is_alphanumeric() && c != '_').filter(|w| !w.is_empty()).collect();
    if !matches!(words.first().copied(), Some("select" | "with" | "explain" | "values")) {
        return Err("Só consultas de leitura (SELECT) são aceitas.".into());
    }
    // one statement, and nothing that writes inside a read (a CTE's DELETE, SELECT … INTO)
    let bad = ["insert", "update", "delete", "drop", "alter", "create", "replace", "truncate", "attach", "detach", "pragma", "vacuum", "into", "grant", "call"];
    if text.contains(';') || words.iter().any(|w| bad.contains(w)) {
        return Err("Só consultas de leitura (SELECT) são aceitas: uma instrução, sem escrita.".into());
    }
    Ok(())
}

impl Db {
    /// A `Db` that saves its connections in `store` (None: the app's own file; tests use a temporary one).
    pub(crate) fn with_store(store: Option<PathBuf>) -> Self {
        Self { store, ..Self::default() }
    }

    fn list(&self) -> Vec<Conexao> {
        self.store.as_deref().map_or_else(conexoes::carregar, conexoes::carregar_de)
    }

    fn edit(&self, f: impl FnOnce(&mut Vec<Conexao>) -> bool) -> Result<Vec<Conexao>, String> {
        match &self.store {
            Some(path) => conexoes::editar_em(path, f),
            None => conexoes::editar(f),
        }
    }

    /// The data sources window is up (an Esc then belongs to it, not to the canvas).
    pub(crate) fn dialog_open(&self) -> bool {
        self.dialog.is_some()
    }

    /// The saved sources, read once.
    pub(crate) fn ensure_loaded(&mut self) {
        if !self.loaded {
            self.conexoes = self.list();
            self.loaded = true;
        }
    }

    /// The project's SQLite files join the sources once, and the first one connects (the IDE's).
    pub(crate) fn discover(&mut self, files: &[PathBuf], ctx: &egui::Context) {
        self.ensure_loaded();
        if self.initialized {
            return;
        }
        let found: Vec<_> = files.iter().filter(|p| sqlite_file(p)).collect();
        match self.edit(|list| found.iter().fold(false, |added, file| conexoes::garantir_sqlite(list, file) || added)) {
            Ok(list) => self.conexoes = list,
            Err(e) => self.error = Some(e),
        }
        self.initialized = true;
        if let Some(c) = self.conexoes.iter().find(|c| c.motor == Motor::Sqlite).cloned() {
            self.connect(c, String::new(), ctx);
        }
    }

    /// A SQLite file the person opened: it is a source (once), connected and the one acted on.
    pub(crate) fn add_sqlite(&mut self, file: &Path, ctx: &egui::Context) -> Result<(), String> {
        if !is_sqlite(file) {
            return Err("Não é um arquivo SQLite.".into());
        }
        self.ensure_loaded();
        self.conexoes = self.edit(|list| conexoes::garantir_sqlite(list, file))?;
        let path = file.to_string_lossy();
        let c = self.conexoes.iter().find(|c| c.motor == Motor::Sqlite && c.arquivo == path).cloned().ok_or("Fonte não encontrada.")?;
        if self.sessions.contains_key(&c.id) {
            self.active = Some(c.id);
        } else {
            self.connect(c, String::new(), ctx);
        }
        Ok(())
    }

    /// What the database does with its panel closed too: the trees' answers.
    pub(crate) fn background(&mut self) {
        self.poll_catalogs();
    }

    /// The panel: the toolbar over the sources' tree. `busy(source)` says a tab of it has a pending change.
    pub(crate) fn panel(&mut self, ui: &mut Ui, ctx: &egui::Context, busy: &dyn Fn(&str) -> bool) -> Vec<Event> {
        let mut events = vec![];
        self.ensure_loaded();
        let mut add = None;
        let (mut console, mut refresh, mut disconnect, mut properties) = (false, false, false, false);
        ui.horizontal(|ui| {
            // the first glyph starts under the title's first letter (10 in), the buttons being 26 wide
            ui.add_space(4.0);
            ui.spacing_mut().item_spacing.x = 2.0;
            // "+" is a button like its neighbours; its menu is the app's own (rows, border and type as the right-click menus)
            let plus = look::tool_button(ui, look::Glyph::Add, "Nova fonte de dados", true);
            egui::Popup::menu(&plus)
                .align(egui::RectAlign::BOTTOM_START)
                .gap(4.0)
                .frame(crate::app::ui::menu_look::menu_frame(ui.visuals().dark_mode))
                .close_behavior(egui::PopupCloseBehavior::CloseOnClick)
                .show(|ui| add = dialog::engines(ui, &self.conexoes));
            properties = look::tool(ui, look::Glyph::Properties, "Propriedades da fonte de dados", !self.conexoes.is_empty());
            let on = self.active.as_ref().is_some_and(|id| self.sessions.contains_key(id));
            refresh = look::tool(ui, look::Glyph::Refresh, "Recarregar a estrutura", on);
            console = look::tool(ui, look::Glyph::Run, "Novo console", on);
            let clean = self.active.as_ref().is_none_or(|id| !busy(id));
            disconnect = look::tool(ui, look::Glyph::Remove, "Desconectar (reverte a transação pendente)", on && clean);
        });
        look::rule(ui);
        // the list as saved now (another IDE may have changed it) before the window shows it
        if add.is_some() || properties {
            self.conexoes = self.list();
        }
        if let Some(motor) = add {
            self.dialog = Some(dialog::Dialog::new(motor));
        }
        if properties {
            let c = self.active.as_ref().and_then(|id| self.conexoes.iter().find(|c| &c.id == id)).or(self.conexoes.first()).cloned();
            self.dialog = c.map(|c| {
                let password = self.senhas.ler(&c).unwrap_or_default();
                dialog::Dialog::edit(c, password)
            });
        }
        if self.conexoes.is_empty() {
            ui.horizontal(|ui| {
                ui.add_space(14.0);
                ui.label(egui::RichText::new("Crie uma fonte de dados pelo botão +.").color(t::MUTED));
            });
        }
        if let Some(error) = &self.error {
            ui.colored_label(t::ERROR, error);
        }
        if !self.loading.is_empty() {
            ui.label("Carregando estrutura…");
        }
        let action = tree::show(ui, self);
        if let Some(key) = action.toggle {
            if !self.open.remove(&key) {
                self.open.insert(key);
            }
        }
        if let Some(id) = action.pick {
            self.active = Some(id.clone());
            if !self.sessions.contains_key(&id) {
                if let Some(c) = self.conexoes.iter().find(|c| c.id == id).cloned() {
                    if c.motor == Motor::Sqlite || c.autenticacao == conexoes::Autenticacao::Sem {
                        self.connect(c, String::new(), ctx);
                    } else if let Some(password) = self.senhas.ler(&c) {
                        self.connect(c, password, ctx);
                    } else {
                        self.dialog = Some(dialog::Dialog::edit(c, String::new()));
                    }
                }
            }
        }
        if refresh {
            self.refresh(ctx);
        }
        if let Some((id, table)) = action.table {
            self.active = Some(id.clone());
            events.push(Event::Open(Open { session: id, meta: Some(table), ddl: None }));
        }
        if let Some((id, object)) = action.ddl {
            self.active = Some(id.clone());
            events.push(Event::Open(Open { session: id, meta: None, ddl: Some(object) }));
        }
        if console {
            if let Some(id) = self.active.clone() {
                events.push(Event::Open(Open { session: id, meta: None, ddl: None }));
            }
        }
        if disconnect {
            events.extend(self.disconnect().map(Event::Closed));
        }
        events
    }

    /// The data sources window, wherever the host shows (not only with the Database panel open).
    pub(crate) fn dialog(&mut self, ctx: &egui::Context) -> Vec<Event> {
        let mut events = vec![];
        if let Some(mut dialog) = self.dialog.take() {
            let senhas = &self.senhas;
            match dialog.show(ctx, &self.conexoes, |c| senhas.ler(c)) {
                dialog::Action::Keep => self.dialog = Some(dialog),
                dialog::Action::Cancel => {}
                dialog::Action::Save { close } => {
                    let c = dialog.connection.clone();
                    let old = self.conexoes.iter().find(|old| old.id == c.id).cloned();
                    let result = (|| {
                        if let Some(old) = old.as_ref().filter(|old| old.retencao != c.retencao) {
                            self.senhas.apagar(old)?;
                        }
                        self.senhas.guardar(&c, &dialog.password)?;
                        self.edit(|list| {
                            if let Some(old) = list.iter_mut().find(|old| old.id == c.id) {
                                *old = c.clone();
                            } else {
                                list.push(c.clone());
                            }
                            true
                        })
                    })();
                    match result {
                        Ok(list) => {
                            self.conexoes = list;
                            // Apply on an unchanged, connected source keeps its session and open tabs
                            if old.as_ref() != Some(&c) || dialog.edited || !self.sessions.contains_key(&c.id) {
                                self.connect(c, dialog.password.clone(), ctx);
                            }
                            dialog.edited = false;
                            if !close {
                                self.dialog = Some(dialog);
                            }
                        }
                        Err(e) => {
                            dialog.error(e);
                            self.dialog = Some(dialog);
                        }
                    }
                }
                dialog::Action::Remove(id) => {
                    let gone = self.conexoes.iter().find(|c| c.id == id).cloned();
                    match gone.as_ref().map_or(Ok(()), |c| self.senhas.apagar(c)).and_then(|()| {
                        self.edit(|list| {
                            list.retain(|c| c.id != id);
                            true
                        })
                    }) {
                        Ok(list) => {
                            let active = self.active.replace(id.clone());
                            events.extend(self.disconnect().map(Event::Closed));
                            self.active = active.filter(|a| *a != id);
                            self.open.remove(&id);
                            self.conexoes = list;
                            self.dialog = self.conexoes.first().cloned().map(|c| {
                                let password = self.senhas.ler(&c).unwrap_or_default();
                                dialog::Dialog::edit(c, password)
                            });
                        }
                        Err(e) => {
                            dialog.error(e);
                            self.dialog = Some(dialog);
                        }
                    }
                }
            }
        }
        events
    }
    fn connect(&mut self, c: Conexao, password: String, ctx: &egui::Context) {
        let id = c.id.clone();
        let session = Session::new(c, password);
        self.sessions.insert(id.clone(), session);
        self.active = Some(id.clone());
        self.open.insert(id);
        self.refresh(ctx);
    }
    fn refresh(&mut self, ctx: &egui::Context) {
        let Some(id) = self.active.clone() else { return };
        if let Some(session) = self.sessions.get(&id) {
            if !self.loading.contains_key(&id) {
                self.loading.insert(id, session.request(Op::Schema, ctx));
            }
        }
    }
    fn poll_catalogs(&mut self) {
        let mut finished = vec![];
        for (id, rx) in &self.loading {
            match rx.try_recv() {
                Ok(Job::Schema(result)) => {
                    match result {
                        Ok(catalog) => {
                            for schema in &catalog.schemas {
                                if session_dialect(&self.conexoes, id) == Dialect::Redis && !schema.expanded {
                                    self.open.remove(&tree::schema_key(id, &schema.name));
                                    continue;
                                }
                                self.open.insert(tree::schema_key(id, &schema.name));
                                self.open.insert(tree::folder_key(
                                    id,
                                    &schema.name,
                                    if session_dialect(&self.conexoes, id) == Dialect::Mongo { "coleções" } else { "tabelas" },
                                ));
                            }
                            self.catalogs.insert(id.clone(), catalog);
                            self.error = None;
                        }
                        Err(e) => self.error = Some(e),
                    }
                    finished.push(id.clone());
                }
                Err(std::sync::mpsc::TryRecvError::Disconnected) => {
                    self.error = Some("Sessão interrompida.".into());
                    finished.push(id.clone());
                }
                _ => {}
            }
        }
        for id in finished {
            self.loading.remove(&id);
        }
    }
    /// Drops the active source's session; its id, for the host to close its tabs.
    fn disconnect(&mut self) -> Option<String> {
        let id = self.active.take()?;
        self.sessions.remove(&id);
        self.catalogs.remove(&id);
        self.loading.remove(&id);
        if let Some(c) = self.conexoes.iter().find(|c| c.id == id) {
            self.senhas.desconectar(c);
        }
        Some(id)
    }

    /// A tab for `open`, made now: the name of its source (the file for SQLite, `nome@host`
    /// otherwise, as a path to label the tab) and the sheet. None when its source is not connected.
    pub(crate) fn new_sheet(&mut self, ctx: &egui::Context, id: Id, open: &Open) -> Option<(PathBuf, Sheet)> {
        let session = self.sessions.get(&open.session).cloned()?;
        self.seq += 1;
        let path = self
            .conexoes
            .iter()
            .find(|c| c.id == session.id)
            .map(|c| if c.motor == Motor::Sqlite { PathBuf::from(&c.arquivo) } else { PathBuf::from(format!("{}@{}", c.nome, c.host)) })
            .unwrap_or_default();
        let mut sheet = Sheet::new(id.with(("db_sheet", self.seq)), open.meta.clone(), session);
        if open.meta.is_some() {
            sheet.reload(ctx);
        }
        if let Some(object) = open.ddl.clone() {
            sheet.ddl(object, ctx);
        }
        Some((path, sheet))
    }

    /// The `Open` of table `table` of source `source` (by name), for a host that opens it by a name; without a table, a console.
    /// The source connects now when it can (SQLite reads its structure at once, a server's catalog must be loaded).
    pub(crate) fn open_named(&mut self, source: &str, table: &str, ctx: &egui::Context) -> Result<Open, String> {
        let c = self.source_named(source)?;
        self.session_for(&c, ctx)?;
        if table.is_empty() {
            return Ok(Open { session: c.id, meta: None, ddl: None });
        }
        let tables: Vec<Table> = if c.motor == Motor::Sqlite {
            sql::schema(Path::new(&c.arquivo))?
        } else {
            let catalog = self.catalogs.get(&c.id).ok_or("A fonte ainda não carregou a estrutura: abra-a no painel.")?;
            catalog.schemas.iter().flat_map(|s| s.tables.clone()).collect()
        };
        let meta = tables.into_iter().find(|t| t.name.eq_ignore_ascii_case(table));
        meta.map(|meta| Open { session: c.id, meta: Some(meta), ddl: None }).ok_or_else(|| format!("Tabela {table} não encontrada."))
    }

    /// One line per source: its name, engine and whether it is connected.
    pub(crate) fn sources_text(&mut self) -> String {
        self.ensure_loaded();
        if self.conexoes.is_empty() {
            return "Nenhuma fonte de dados.\n".into();
        }
        self.conexoes
            .iter()
            .map(|c| {
                let target = if c.motor == Motor::Sqlite { c.arquivo.clone() } else { format!("{}@{}", c.nome, c.host) };
                let on = if self.sessions.contains_key(&c.id) { "conectada" } else { "desconectada" };
                format!("{} ({}, {on}): {target}\n", c.nome, c.motor.nome())
            })
            .collect()
    }

    /// The source an agent means: by name, id or file; the only one when none is said.
    fn source_named(&mut self, name: &str) -> Result<Conexao, String> {
        self.ensure_loaded();
        let wanted = name.trim().to_lowercase();
        let pick = if wanted.is_empty() && self.conexoes.len() == 1 {
            self.conexoes.first()
        } else {
            self.conexoes.iter().find(|c| c.id == name.trim() || c.nome.to_lowercase() == wanted || c.arquivo.to_lowercase() == wanted)
        };
        pick.cloned().ok_or_else(|| "Diga qual fonte de dados (veja a lista em `db_sources`).".to_string())
    }

    /// The tables (and their columns) of source `name`, as text. SQLite is read now; a server's
    /// is asked of its session, waiting a few seconds for it.
    pub(crate) fn tables_text(&mut self, name: &str, ctx: &egui::Context) -> Result<String, String> {
        let c = self.source_named(name)?;
        let schemas: Vec<(String, Vec<Table>)> = if c.motor == Motor::Sqlite {
            vec![("main".into(), sql::schema(Path::new(&c.arquivo))?)]
        } else {
            let session = self.session_for(&c, ctx)?;
            match session.request(Op::Schema, ctx).recv_timeout(WAIT) {
                Ok(Job::Schema(r)) => r?.schemas.into_iter().map(|s| (s.name, s.tables)).collect(),
                _ => return Err("A fonte não respondeu a tempo.".into()),
            }
        };
        let mut out = format!("Fonte {} ({})\n", c.nome, c.motor.nome());
        for (schema, tables) in schemas {
            out += &format!("{schema}\n");
            for t in tables {
                let cols: Vec<String> =
                    t.columns.iter().map(|c| format!("{}{} {}", c.name, if c.pk > 0 { "*" } else { "" }, c.kind).trim_end().to_string()).collect();
                out += &format!("  {}{}: {}\n", t.name, if t.view { " (visão)" } else { "" }, cols.join(", "));
            }
        }
        Ok(out)
    }

    /// Runs a read (SELECT) on source `name`: the rows as text, at most `limit`.
    pub(crate) fn query_text(&mut self, name: &str, sql: &str, limit: usize, ctx: &egui::Context) -> Result<String, String> {
        read_only_sql(sql)?;
        let c = self.source_named(name)?;
        let grid = match c.motor {
            Motor::Sqlite => sql::query(Path::new(&c.arquivo), sql)?,
            Motor::Mongo | Motor::Redis => return Err("Só SQL: esta fonte não é relacional.".into()),
            _ => {
                let session = self.session_for(&c, ctx)?;
                match session.request(Op::Console(sql.to_string()), ctx).recv_timeout(WAIT) {
                    Ok(Job::Console(r, _)) => r?,
                    _ => return Err("A fonte não respondeu a tempo.".into()),
                }
            }
        };
        let shown = grid.rows.len().min(limit);
        let mut out = format!("{}\n", grid.columns.join(" | "));
        for row in grid.rows.iter().take(shown) {
            out += &format!("{}\n", row.iter().map(|c| c.text()).collect::<Vec<_>>().join(" | "));
        }
        out += &if grid.rows.len() > shown { format!("{} linhas, mostrando {shown}.\n", grid.rows.len()) } else { format!("{} linha(s).\n", grid.rows.len()) };
        Ok(out)
    }

    /// The session of `c`, connecting it when it needs no typed password.
    fn session_for(&mut self, c: &Conexao, ctx: &egui::Context) -> Result<Session, String> {
        if !self.sessions.contains_key(&c.id) {
            let password = if c.autenticacao == conexoes::Autenticacao::Sem { Some(String::new()) } else { self.senhas.ler(c) };
            let password = password.ok_or("Conecte a fonte pelo painel Banco de Dados (a senha não está guardada).")?;
            self.connect(c.clone(), password, ctx);
        }
        self.sessions.get(&c.id).cloned().ok_or_else(|| "Fonte não conectada.".to_string())
    }
}

/// How long the MCP waits for a server's answer.
const WAIT: Duration = Duration::from_secs(15);

fn session_dialect(connections: &[Conexao], id: &str) -> Dialect {
    connections.iter().find(|c| c.id == id).map(|c| c.motor.dialect()).unwrap_or_default()
}
