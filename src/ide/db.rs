//! Database sources, background sessions and editor tabs.
use super::*;
use sql::*;
use session::{Catalog, Job, Op, Session};
use conexoes::{Conexao, Motor, Senhas};

mod conexoes;
mod dialog;
mod grid;
mod look;
mod mysql;
mod mysql_sql;
mod session;
mod sql;
mod tree;
mod view;
#[cfg(test)]
mod tests;
#[cfg(test)]
mod mysql_tests;

#[cfg(not(test))]
type Vault = conexoes::Sistema;
#[cfg(test)]
type Vault = conexoes::Memoria;

#[derive(Default)]
pub(super) struct Db {
    initialized: bool,
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
}
pub(super) use view::Sheet;

fn is_sqlite(path: &Path) -> bool {
    let mut head = [0u8; 16];
    std::fs::File::open(path).and_then(|mut f| std::io::Read::read_exact(&mut f, &mut head)).is_ok() && &head == b"SQLite format 3\0"
}

impl Ide {
    pub(super) fn db_panel(&mut self, ui: &mut Ui) {
        if !self.db.initialized && self.files_rx.is_none() {
            self.db.conexoes = conexoes::carregar();
            let found: Vec<_> = self.files.iter().filter(|p| matches!(p.extension().and_then(|e| e.to_str()), Some("sqlite" | "sqlite3" | "db")) && is_sqlite(p)).cloned().collect();
            let added = found.iter().fold(false, |added, file| conexoes::garantir_sqlite(&mut self.db.conexoes, file) || added);
            if added { self.db.error = conexoes::salvar(&self.db.conexoes).err(); }
            self.db.initialized = true;
            if let Some(c) = self.db.conexoes.iter().find(|c| c.motor == Motor::Sqlite).cloned() { self.connect_db(c, String::new()); }
        }
        self.poll_catalogs();
        for tab in &mut self.tabs { if let Some(sheet) = tab.sheet.as_mut() { sheet.poll(&self.ctx); } }
        tool_header(ui, "Banco de dados");
        let mut add = None;
        let (mut console, mut refresh, mut disconnect) = (false, false, false);
        ui.horizontal(|ui| {
            ui.add_space(8.0);
            ui.spacing_mut().item_spacing.x = 2.0;
            ui.menu_button("+", |ui| {
                ui.menu_button("Fonte de dados", |ui| { add = dialog::engines(ui, &self.db.conexoes); });
            });
            let on = self.db.active.as_ref().is_some_and(|id| self.db.sessions.contains_key(id));
            refresh = look::tool(ui, look::Glyph::Refresh, "Recarregar a estrutura", on);
            console = look::tool(ui, look::Glyph::Run, "Novo console SQL", on);
            let clean = self.tabs.iter().all(|t| t.sheet.as_ref().is_none_or(|s| Some(&s.session.id) != self.db.active.as_ref() || !s.busy()));
            disconnect = look::tool(ui, look::Glyph::Remove, "Desconectar (reverte a transação pendente)", on && clean);
        });
        look::rule(ui);
        if let Some(motor) = add { self.db.dialog = Some(dialog::Dialog::new(motor)); }
        if self.db.conexoes.is_empty() { ui.label("Crie uma fonte de dados pelo botão +."); }
        if let Some(error) = &self.db.error { ui.colored_label(t::ERROR, error); }
        if !self.db.loading.is_empty() { ui.label("Carregando estrutura…"); }
        let action = tree::show(ui, &self.db);
        if let Some(key) = action.toggle {
            if !self.db.open.remove(&key) { self.db.open.insert(key); }
        }
        if let Some(id) = action.pick {
            self.db.active = Some(id.clone());
            if !self.db.sessions.contains_key(&id) {
                if let Some(c) = self.db.conexoes.iter().find(|c| c.id == id).cloned() {
                    if c.motor == Motor::Sqlite { self.connect_db(c, String::new()); }
                    else if let Some(password) = self.db.senhas.ler(&c) { self.connect_db(c, password); }
                    else { self.db.dialog = Some(dialog::Dialog::edit(c)); }
                }
            }
        }
        if refresh { self.refresh_db(); }
        if let Some((id, table)) = action.table { self.db.active = Some(id); self.open_table(Some(table)); }
        if let Some((id, object)) = action.ddl { self.db.active = Some(id); self.open_ddl(object); }
        if console { self.open_table(None); }
        if disconnect { self.disconnect_db(); }
        if let Some(mut dialog) = self.db.dialog.take() {
            match dialog.show(ui.ctx()) {
                dialog::Action::Keep => self.db.dialog = Some(dialog),
                dialog::Action::Cancel => {},
                dialog::Action::Save => {
                    let c = dialog.connection.clone();
                    let mut list = self.db.conexoes.clone();
                    if let Some(old) = list.iter_mut().find(|old| old.id == c.id) { *old = c.clone(); } else { list.push(c.clone()); }
                    let result = (|| {
                        if let Some(old) = self.db.conexoes.iter().find(|old| old.id == c.id && old.retencao != c.retencao) { self.db.senhas.apagar(old)?; }
                        self.db.senhas.guardar(&c, &dialog.password)?;
                        conexoes::salvar(&list)
                    })();
                    match result {
                        Ok(()) => { self.db.conexoes = list; self.connect_db(c, dialog.password); }
                        Err(e) => { dialog.error(e); self.db.dialog = Some(dialog); }
                    }
                }
            }
        }
    }
    fn connect_db(&mut self, c: Conexao, password: String) {
        let id = c.id.clone();
        let session = Session::new(c, password);
        self.db.sessions.insert(id.clone(), session);
        self.db.active = Some(id.clone());
        self.db.open.insert(id);
        self.refresh_db();
    }
    fn refresh_db(&mut self) {
        let Some(id) = self.db.active.clone() else { return };
        if let Some(session) = self.db.sessions.get(&id) {
            if !self.db.loading.contains_key(&id) { self.db.loading.insert(id, session.request(Op::Schema, &self.ctx)); }
        }
    }
    fn poll_catalogs(&mut self) {
        let mut finished = vec![];
        for (id, rx) in &self.db.loading {
            match rx.try_recv() {
                Ok(Job::Schema(result)) => {
                    match result {
                        Ok(catalog) => {
                            for schema in &catalog.schemas {
                                self.db.open.insert(tree::schema_key(id, &schema.name));
                                self.db.open.insert(tree::folder_key(id, &schema.name, "tabelas"));
                            }
                            self.db.catalogs.insert(id.clone(), catalog);
                            self.db.error = None;
                        }
                        Err(e) => self.db.error = Some(e),
                    }
                    finished.push(id.clone());
                }
                Err(std::sync::mpsc::TryRecvError::Disconnected) => { self.db.error = Some("Sessão interrompida.".into()); finished.push(id.clone()); }
                _ => {},
            }
        }
        for id in finished { self.db.loading.remove(&id); }
    }
    fn disconnect_db(&mut self) {
        let Some(id) = self.db.active.take() else { return };
        self.tabs.retain(|t| t.sheet.as_ref().is_none_or(|s| s.session.id != id));
        self.active = self.active.min(self.tabs.len().saturating_sub(1));
        self.db.sessions.remove(&id);
        self.db.catalogs.remove(&id);
        self.db.loading.remove(&id);
        if let Some(c) = self.db.conexoes.iter().find(|c| c.id == id) { self.db.senhas.desconectar(c); }
    }
    fn open_table(&mut self, meta: Option<Table>) {
        let Some(session) = self.db.active.as_ref().and_then(|id| self.db.sessions.get(id)).cloned() else { return };
        let table = meta.as_ref().map(|t| t.name.clone());
        let schema = meta.as_ref().map(|t| t.schema.as_str()).unwrap_or("");
        if let Some(i) = self.tabs.iter().position(|t| t.sheet.as_ref().is_some_and(|s| s.matches(&session.id, schema, table.as_deref()))) { self.active = i; return; }
        self.db.seq += 1;
        let path = self.db.conexoes.iter().find(|c| c.id == session.id).map(|c| if c.motor == Motor::Sqlite { PathBuf::from(&c.arquivo) } else { PathBuf::from(format!("{}@{}", c.nome, c.host)) }).unwrap_or_default();
        let mut sheet = Sheet::new(self.id.with(("db_sheet", self.db.seq)), meta, session);
        if table.is_some() { sheet.reload(&self.ctx); }
        let mut tab = Tab::new(path, String::new(), false);
        tab.sheet = Some(Box::new(sheet));
        self.tabs.push(tab);
        self.active = self.tabs.len() - 1;
    }
    fn open_ddl(&mut self, object: session::Object) {
        self.open_table(None);
        if let Some(sheet) = self.tabs.get_mut(self.active).and_then(|t| t.sheet.as_mut()) { sheet.ddl(object, &self.ctx); }
    }
    pub(super) fn sheet_view(&mut self, ui: &mut Ui) {
        if let Some(sheet) = self.tabs[self.active].sheet.as_mut() { sheet.ui(ui); }
    }
}
