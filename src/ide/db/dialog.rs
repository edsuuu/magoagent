//! Data source chooser and General settings. Passwords never enter the saved model.
use super::*;
use conexoes::{Conexao, Motor, Retencao};

pub(super) enum Action { Keep, Save, Cancel }
pub(super) struct Dialog {
    pub connection: Conexao, pub password: String,
    testing: Option<(Session, Receiver<Job>)>, message: Option<(String, bool)>,
}
impl Dialog {
    pub fn new(motor: Motor) -> Self {
        Self::edit(Conexao { id: uuid::Uuid::new_v4().simple().to_string(), motor, nome: motor.nome().into(),
            host: "localhost".into(), porta: motor.porta(), auto_commit: true, ..Default::default() })
    }
    pub fn edit(connection: Conexao) -> Self { Self { connection, password: String::new(), testing: None, message: None } }
    pub fn error(&mut self, error: String) { self.message = Some((error, true)); }
    pub fn show(&mut self, ctx: &egui::Context) -> Action {
        if let Some((_, rx)) = &self.testing {
            match rx.try_recv() {
                Ok(Job::Test(result)) => { self.message = Some(match result { Ok(s) => (s, false), Err(e) => (e, true) }); self.testing = None; }
                Err(std::sync::mpsc::TryRecvError::Disconnected) => { self.error("Teste interrompido.".into()); self.testing = None; }
                _ => {},
            }
        }
        let mut action = Action::Keep;
        let mut open = true;
        egui::Window::new("Fontes de dados").id(Id::unique("db_sources")).open(&mut open).collapsible(false).resizable(false).default_width(520.0).show(ctx, |ui| {
            ui.label(egui::RichText::new("Geral").strong());
            ui.separator();
            let c = &mut self.connection;
            let before = c.clone();
            let previous_password = self.password.clone();
            ui.add_enabled_ui(self.testing.is_none(), |ui| {
                egui::Grid::new("general").num_columns(2).spacing(vec2(12.0, 8.0)).show(ui, |ui| {
                    field(ui, "Nome:", &mut c.nome);
                    if c.motor == Motor::Sqlite {
                        ui.label("Arquivo:");
                        ui.horizontal(|ui| {
                            ui.text_edit_singleline(&mut c.arquivo);
                            if ui.button("…").clicked() {
                                if let Some(path) = rfd::FileDialog::new().add_filter("SQLite", &["sqlite", "sqlite3", "db"]).pick_file() { c.arquivo = path.to_string_lossy().into_owned(); }
                            }
                        }); ui.end_row();
                    } else {
                        field(ui, "Host:", &mut c.host);
                        ui.label("Porta:"); ui.add(egui::DragValue::new(&mut c.porta).range(1..=65535)); ui.end_row();
                        field(ui, "Usuário:", &mut c.usuario);
                        ui.label("Senha:"); ui.add(TextEdit::singleline(&mut self.password).password(true)); ui.end_row();
                        ui.label("Salvar:");
                        egui::ComboBox::from_id_salt("retention").selected_text(retention(c.retencao)).show_ui(ui, |ui| {
                            for r in [Retencao::Nunca, Retencao::AteReiniciar, Retencao::Sessao, Retencao::Sempre] { ui.selectable_value(&mut c.retencao, r, retention(r)); }
                        }); ui.end_row();
                        field(ui, "Banco:", &mut c.banco);
                        ui.label("TLS:"); ui.checkbox(&mut c.tls, "Usar TLS"); ui.end_row();
                    }
                    ui.label("URL:"); ui.label(url(c)); ui.end_row();
                });
            });
            if *c != before || self.password != previous_password { self.message = None; }
            ui.separator();
            let valid = !c.nome.trim().is_empty() && if c.motor == Motor::Sqlite { !c.arquivo.trim().is_empty() } else { !c.host.trim().is_empty() && c.porta > 0 };
            ui.horizontal(|ui| {
                if ui.add_enabled(valid && self.testing.is_none(), egui::Button::new("Testar conexão")).clicked() {
                    let session = Session::new(c.clone(), self.password.clone());
                    let rx = session.request(Op::Test, ctx);
                    self.testing = Some((session, rx)); self.message = None;
                }
                if self.testing.is_some() { ui.spinner(); ui.label("Conectando…"); }
            });
            if let Some((message, error)) = &self.message { ui.colored_label(if *error { t::ERROR } else { t::TEXT }, message); }
            ui.horizontal(|ui| {
                if ui.add_enabled(valid && self.testing.is_none(), egui::Button::new("OK")).clicked() { action = Action::Save; }
                if ui.button("Cancelar").clicked() { action = Action::Cancel; }
            });
        });
        if !open { Action::Cancel } else { action }
    }
}
fn field(ui: &mut Ui, label: &str, value: &mut String) { ui.label(label); ui.text_edit_singleline(value); ui.end_row(); }
fn retention(r: Retencao) -> &'static str { match r { Retencao::Nunca => "Nunca", Retencao::AteReiniciar => "Até reiniciar", Retencao::Sessao => "Sessão", Retencao::Sempre => "Sempre" } }
fn url(c: &Conexao) -> String {
    if c.motor == Motor::Sqlite { format!("jdbc:sqlite:{}", c.arquivo) }
    else {
        let host = if c.host.contains(':') && !c.host.starts_with('[') { format!("[{}]", c.host) } else { c.host.clone() };
        format!("jdbc:mysql://{host}:{}/{}", c.porta, c.banco)
    }
}
pub(super) fn engines(ui: &mut Ui, connections: &[Conexao]) -> Option<Motor> {
    let mut pick = None;
    egui::ScrollArea::vertical().max_height(460.0).show(ui, |ui| {
    ui.label(egui::RichText::new("Recentes").color(t::MUTED));
    let mut seen = vec![];
    for c in connections.iter().rev() {
        if matches!(c.motor, Motor::Sqlite | Motor::Mysql) && !seen.contains(&c.motor) {
            seen.push(c.motor);
            if ui.button(c.motor.nome()).clicked() { pick = Some(c.motor); ui.close(); }
        }
    }
    ui.separator(); ui.label(egui::RichText::new("Suporte completo").color(t::MUTED));
    for name in ["Amazon Aurora MySQL", "Amazon DocumentDB", "Amazon DynamoDB", "Amazon Redshift", "Apache Cassandra", "Apache Derby", "Apache Hive", "Azure SQL Database", "Azure Synapse Analytics", "ClickHouse", "CockroachDB", "Couchbase Server", "Exasol", "Google BigQuery", "Greenplum", "H2", "HSQLDB", "IBM Db2", "MariaDB", "MongoDB", "MySQL", "Oracle", "PostgreSQL", "Redis", "SQL Server", "SQLite", "Snowflake", "Sybase ASE", "Vertica"] {
        let motor = match name { "MySQL" => Some(Motor::Mysql), "SQLite" => Some(Motor::Sqlite), _ => None };
        let label = if motor.is_some() { name.into() } else { format!("{name} — em breve") };
        if ui.add_enabled(motor.is_some(), egui::Button::new(label)).clicked() { pick = motor; ui.close(); }
    }
    ui.separator();
    ui.menu_button("Outros", |ui| {
        ui.label(egui::RichText::new("Suporte básico").color(t::MUTED));
        for name in ["Amazon Athena", "Apache Ignite", "Apache Phoenix", "Apache Spark", "Databricks", "Denodo", "DuckDB", "Elasticsearch", "Firebird", "Google Cloud Spanner", "IBM Informix", "InfluxDB", "InterSystems IRIS", "Mimer SQL", "ODBC", "OpenEdge", "Presto", "SAP HANA", "SingleStore", "Tarantool", "Teradata Vantage", "Tibero", "TiDB", "Trino", "YugabyteDB"] {
            ui.add_enabled(false, egui::Button::new(format!("{name} — em breve")));
        }
    });
    });
    pick
}
