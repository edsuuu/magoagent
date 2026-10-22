//! "Fontes de dados e drivers", after PhpStorm's Data Sources and Drivers dialog: the saved
//! sources on the left, the selected one's Name, Comment and tabs on the right, Test Connection
//! under them and OK / Apply / Cancel along the bottom. Passwords never enter the saved model.
use super::*;
use conexoes::{Conexao, Motor, Retencao};

pub(super) enum Action { Keep, Save { close: bool }, Remove(String), Cancel }

#[derive(Clone, Copy, PartialEq)]
enum Tab { Geral, Opcoes, SshSsl, Esquemas, Avancado }

/// The engines that connect today, listed first in the + menu.
const ATIVOS: [Motor; 3] = [Motor::Mariadb, Motor::Mysql, Motor::Sqlite];

pub(super) struct Dialog {
    pub connection: Conexao, pub password: String,
    /// The password was typed since the source was picked or applied.
    pub edited: bool,
    testing: Option<(Session, Receiver<Job>)>, message: Option<(String, bool)>,
    tab: Tab, url: String, confirm_remove: bool,
}
impl Dialog {
    pub fn new(motor: Motor) -> Self {
        Self::edit(Conexao { id: uuid::Uuid::new_v4().simple().to_string(), motor, nome: motor.nome().into(),
            host: "localhost".into(), porta: motor.porta(), auto_commit: true, ..Default::default() }, String::new())
    }
    pub fn edit(connection: Conexao, password: String) -> Self {
        Self { connection, password, edited: false, testing: None, message: None, tab: Tab::Geral, url: String::new(), confirm_remove: false }
    }
    pub fn error(&mut self, error: String) { self.message = Some((error, true)); }
    /// `saved` is the list as stored; `password` reads a stored source's password when it is picked.
    pub fn show(&mut self, ctx: &egui::Context, saved: &[Conexao], password: impl Fn(&Conexao) -> Option<String>) -> Action {
        if let Some((_, rx)) = &self.testing {
            match rx.try_recv() {
                Ok(Job::Test(result)) => { self.message = Some(match result { Ok(s) => (s, false), Err(e) => (e, true) }); self.testing = None; }
                Err(std::sync::mpsc::TryRecvError::Disconnected) => { self.error("Teste interrompido.".into()); self.testing = None; }
                _ => {},
            }
        }
        let mut action = Action::Keep;
        let mut open = true;
        let stored = saved.iter().find(|s| s.id == self.connection.id);
        let pending = stored != Some(&self.connection) || self.edited;
        let ready = valid(&self.connection) && self.testing.is_none();
        egui::Window::new("Fontes de dados e drivers").id(Id::unique("db_sources")).open(&mut open).collapsible(false)
            .pivot(Align2::CENTER_CENTER).default_pos(ctx.content_rect().center())
            .fixed_size(vec2(800.0, 580.0)).frame(egui::Frame::NONE.fill(t::POPUP).stroke(Stroke::new(1.0, t::POPUP_BORDER)).corner_radius(10))
            .show(ctx, |ui| {
                egui::Panel::bottom("db_sources_buttons").frame(egui::Frame::new().inner_margin(egui::Margin::symmetric(14, 10))).show(ui, |ui| {
                    ui.with_layout(egui::Layout::right_to_left(Align::Center), |ui| {
                        let size = vec2(76.0, 26.0);
                        if ui.add_enabled(ready, egui::Button::new(egui::RichText::new("OK").color(Color32::WHITE)).fill(t::ACCENT).min_size(size)).clicked() { action = Action::Save { close: true }; }
                        if ui.add_enabled(ready && pending, egui::Button::new("Aplicar").min_size(size)).clicked() { action = Action::Save { close: false }; }
                        if ui.add(egui::Button::new("Cancelar").min_size(size)).clicked() { action = Action::Cancel; }
                    });
                });
                egui::Panel::left("db_sources_list").resizable(false).exact_size(220.0)
                    .frame(egui::Frame::new().fill(t::ISLAND).inner_margin(egui::Margin::symmetric(0, 8)))
                    .show(ui, |ui| self.list(ui, saved, stored.is_some(), &password));
                egui::CentralPanel::default().frame(egui::Frame::new().inner_margin(egui::Margin::symmetric(18, 14))).show(ui, |ui| {
                    egui::Panel::bottom("db_test").frame(egui::Frame::new().inner_margin(egui::Margin::symmetric(0, 8))).show_separator_line(false).show(ui, |ui| self.test_row(ui));
                    self.form(ui, stored.is_some());
                });
            });
        if self.confirm_remove {
            let modal = egui::Modal::new(Id::unique("db_remove")).show(ctx, |ui| {
                ui.set_width(320.0);
                ui.label(format!("Remover a fonte de dados «{}»? A senha guardada também é apagada.", self.connection.nome));
                ui.add_space(10.0);
                ui.with_layout(egui::Layout::right_to_left(Align::Center), |ui| {
                    if ui.button("Remover").clicked() { action = Action::Remove(self.connection.id.clone()); self.confirm_remove = false; }
                    if ui.button("Cancelar").clicked() { self.confirm_remove = false; }
                });
            });
            if modal.should_close() { self.confirm_remove = false; }
        }
        if !open { Action::Cancel } else { action }
    }
    fn list(&mut self, ui: &mut Ui, saved: &[Conexao], stored: bool, password: &impl Fn(&Conexao) -> Option<String>) {
        ui.horizontal(|ui| {
            ui.add_space(12.0);
            ui.label(egui::RichText::new("Fontes de dados").color(t::TEXT).strong());
            ui.add_space(8.0);
            ui.label(egui::RichText::new("Drivers").color(t::SECONDARY)).on_hover_text("em breve");
        });
        ui.horizontal(|ui| {
            ui.add_space(6.0);
            ui.spacing_mut().item_spacing.x = 2.0;
            let mut pick = None;
            ui.menu_button("+", |ui| pick = engines(ui, saved));
            if look::tool(ui, look::Glyph::Remove, "Remover a fonte de dados", stored) { self.confirm_remove = true; }
            if let Some(motor) = pick { *self = Self::new(motor); }
        });
        look::rule(ui);
        ui.add_space(6.0);
        ui.horizontal(|ui| { ui.add_space(12.0); ui.label(egui::RichText::new("Fontes de dados globais").size(11.0).color(t::SECONDARY)); });
        ui.spacing_mut().item_spacing.y = 0.0;
        egui::ScrollArea::vertical().auto_shrink(false).show(ui, |ui| {
            for c in saved {
                let selected = c.id == self.connection.id;
                let name = if selected { &self.connection.nome } else { &c.nome };
                if look::list_row(ui, selected, 0, None, look::Mark::Source, name, c.motor.nome(), None).clicked() && !selected {
                    let tab = self.tab;
                    *self = Self::edit(c.clone(), password(c).unwrap_or_default());
                    self.tab = tab;
                }
            }
            // a new source shows in the list before it is saved, as in the reference
            if !stored { look::list_row(ui, true, 0, None, look::Mark::Source, &self.connection.nome, self.connection.motor.nome(), None); }
        });
    }
    fn form(&mut self, ui: &mut Ui, stored: bool) {
        let before = (self.connection.clone(), self.password.clone());
        ui.add_enabled_ui(self.testing.is_none(), |ui| {
            let width = ui.available_width() - 96.0;
            egui::Grid::new("db_header").num_columns(2).spacing(vec2(10.0, 8.0)).show(ui, |ui| {
                ui.label("Nome:"); ui.add(TextEdit::singleline(&mut self.connection.nome).desired_width(width)); ui.end_row();
                ui.label("Comentário:"); ui.add(TextEdit::singleline(&mut self.connection.comentario).desired_width(width)); ui.end_row();
            });
            ui.add_space(12.0);
            self.tabs(ui);
            ui.add_space(12.0);
            match self.tab {
                Tab::Geral => self.general(ui, stored),
                Tab::Opcoes => options(ui, &mut self.connection),
                Tab::SshSsl => ssh(ui, &mut self.connection),
                Tab::Esquemas | Tab::Avancado => {},
            }
        });
        if self.password != before.1 { self.edited = true; }
        if (&self.connection, &self.password) != (&before.0, &before.1) { self.message = None; }
    }
    fn tabs(&mut self, ui: &mut Ui) {
        let mut underline = None;
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = 20.0;
            for (tab, name, on) in [(Tab::Geral, "Geral", true), (Tab::Opcoes, "Opções", true), (Tab::SshSsl, "SSH/SSL", true), (Tab::Esquemas, "Esquemas", false), (Tab::Avancado, "Avançado", false)] {
                let selected = self.tab == tab;
                let color = if !on { t::SECONDARY } else if selected { t::TEXT } else { t::MUTED };
                let r = ui.add(egui::Label::new(egui::RichText::new(name).color(color)).sense(if on { Sense::click() } else { Sense::hover() }));
                if selected { underline = Some(r.rect.x_range()); }
                if !on { r.on_hover_text("em breve"); } else if r.clicked() { self.tab = tab; }
            }
        });
        let (line, _) = ui.allocate_exact_size(vec2(ui.available_width(), 3.0), Sense::hover());
        ui.painter().hline(line.x_range(), line.bottom(), Stroke::new(1.0, t::POPUP_BORDER));
        if let Some(x) = underline { ui.painter().hline(x, line.center().y, Stroke::new(2.0, t::ACCENT)); }
    }
    fn general(&mut self, ui: &mut Ui, stored: bool) {
        let Self { connection: c, password, url, .. } = self;
        let field = 230.0;
        egui::Grid::new("db_general").num_columns(2).spacing(vec2(10.0, 9.0)).show(ui, |ui| {
            ui.label("Tipo de conexão:");
            ui.horizontal(|ui| { ui.label("padrão"); ui.add_space(28.0); ui.label(egui::RichText::new("Driver:").color(t::MUTED)); ui.label(c.motor.nome()); });
            ui.end_row();
            if c.motor == Motor::Sqlite {
                ui.label("Arquivo:");
                ui.horizontal(|ui| {
                    ui.add(TextEdit::singleline(&mut c.arquivo).desired_width(ui.available_width() - 40.0));
                    if ui.button("…").clicked() {
                        if let Some(path) = rfd::FileDialog::new().add_filter("SQLite", &["sqlite", "sqlite3", "db"]).pick_file() { c.arquivo = path.to_string_lossy().into_owned(); }
                    }
                });
                ui.end_row();
            } else {
                ui.label("Host:");
                ui.horizontal(|ui| {
                    ui.add(TextEdit::singleline(&mut c.host).desired_width(ui.available_width() - 120.0));
                    ui.label("Porta:");
                    ui.add(egui::DragValue::new(&mut c.porta).range(1..=65535));
                });
                ui.end_row();
                ui.label("Autenticação:");
                egui::ComboBox::from_id_salt("db_auth").width(field).selected_text("Usuário e senha").show_ui(ui, |ui| { let _ = ui.selectable_label(true, "Usuário e senha"); });
                ui.end_row();
                ui.label("Usuário:"); ui.add(TextEdit::singleline(&mut c.usuario).desired_width(field)); ui.end_row();
                ui.label("Senha:");
                ui.horizontal(|ui| {
                    let hidden = stored && password.is_empty() && c.retencao != Retencao::Nunca;
                    ui.add(TextEdit::singleline(password).password(true).hint_text(if hidden { "<oculta>" } else { "" }).desired_width(field));
                    ui.label("Salvar:");
                    egui::ComboBox::from_id_salt("retention").selected_text(retention(c.retencao).0).show_ui(ui, |ui| {
                        for r in [Retencao::Nunca, Retencao::AteReiniciar, Retencao::Sessao, Retencao::Sempre] {
                            ui.selectable_value(&mut c.retencao, r, retention(r).0).on_hover_text(retention(r).1);
                        }
                    });
                });
                ui.end_row();
                ui.label("Banco:"); ui.add(TextEdit::singleline(&mut c.banco).desired_width(field)); ui.end_row();
            }
            // the URL follows the fields; typing in it fills them back, the reference's "overrides"
            let id = ui.make_persistent_id("db_url");
            if !ui.memory(|m| m.has_focus(id)) { *url = jdbc(c); }
            ui.label("URL:");
            ui.vertical(|ui| {
                if ui.add(TextEdit::singleline(url).id(id).desired_width(ui.available_width())).changed() { parse_url(c, url); }
                ui.label(egui::RichText::new("Sobrepõe as opções acima").size(11.0).color(t::SECONDARY));
            });
            ui.end_row();
        });
    }
    fn test_row(&mut self, ui: &mut Ui) {
        ui.horizontal(|ui| {
            let link = egui::Button::new(egui::RichText::new("Testar conexão").color(t::LINK)).frame(false);
            if ui.add_enabled(valid(&self.connection) && self.testing.is_none(), link).clicked() {
                let session = Session::new(self.connection.clone(), self.password.clone());
                let rx = session.request(Op::Test, ui.ctx());
                self.testing = Some((session, rx)); self.message = None;
            }
            if self.testing.is_some() { ui.spinner(); ui.label(egui::RichText::new("Conectando…").color(t::MUTED)); }
            else if let Some((message, error)) = &self.message {
                let (mark, color) = if *error { ("✖", t::ERROR) } else { ("✓", look::RUN_GREEN) };
                ui.add(egui::Label::new(egui::RichText::new(format!("{mark} {message}")).color(color)).truncate());
            }
        });
    }
}
fn options(ui: &mut Ui, c: &mut Conexao) {
    ui.label(egui::RichText::new("Conexão").strong());
    ui.add_space(6.0);
    ui.checkbox(&mut c.somente_leitura, "Somente leitura").on_hover_text("Impede alterações de dados e de estrutura");
    ui.add_space(4.0);
    ui.horizontal(|ui| {
        ui.label("Controle de transação:");
        egui::ComboBox::from_id_salt("db_tx").selected_text(if c.auto_commit { "Auto" } else { "Manual" }).show_ui(ui, |ui| {
            ui.selectable_value(&mut c.auto_commit, true, "Auto").on_hover_text("As mudanças enviadas ao banco são confirmadas na hora");
            ui.selectable_value(&mut c.auto_commit, false, "Manual").on_hover_text("Confirme as mudanças enviadas ao banco com Commit");
        });
    });
}
fn ssh(ui: &mut Ui, c: &mut Conexao) {
    if c.motor == Motor::Sqlite { ui.label(egui::RichText::new("O SQLite é um arquivo local: não há túnel nem SSL.").color(t::MUTED)); return; }
    ui.add_enabled(false, egui::Checkbox::new(&mut false, "Usar túnel SSH")).on_disabled_hover_text("em breve");
    ui.add_space(10.0);
    ui.checkbox(&mut c.tls, "Usar SSL");
}
fn valid(c: &Conexao) -> bool {
    !c.nome.trim().is_empty() && if c.motor == Motor::Sqlite { !c.arquivo.trim().is_empty() } else { !c.host.trim().is_empty() && c.porta > 0 }
}
/// The "Salvar:" choice and the reference's hint for it.
fn retention(r: Retencao) -> (&'static str, &'static str) {
    match r {
        Retencao::Nunca => ("Nunca", "Pede a senha toda vez"),
        Retencao::AteReiniciar => ("Até reiniciar", "Pede a senha uma vez por abertura do app"),
        Retencao::Sessao => ("Sessão", "Esquece a senha ao desconectar"),
        Retencao::Sempre => ("Sempre", "Guarda a senha no Keychain"),
    }
}
fn scheme(motor: Motor) -> &'static str { if motor == Motor::Mariadb { "jdbc:mariadb://" } else { "jdbc:mysql://" } }
fn jdbc(c: &Conexao) -> String {
    if c.motor == Motor::Sqlite { return format!("jdbc:sqlite:{}", c.arquivo); }
    let host = if c.host.contains(':') && !c.host.starts_with('[') { format!("[{}]", c.host) } else { c.host.clone() };
    format!("{}{host}:{}/{}", scheme(c.motor), c.porta, c.banco)
}
/// Fills host, port and database (or the SQLite file) from a typed URL; false leaves `c` as it was.
fn parse_url(c: &mut Conexao, url: &str) -> bool {
    if c.motor == Motor::Sqlite {
        let Some(file) = url.strip_prefix("jdbc:sqlite:") else { return false };
        c.arquivo = file.into();
        return true;
    }
    let Some(rest) = url.strip_prefix(scheme(c.motor)) else { return false };
    let rest = rest.split(['?', ';']).next().unwrap_or("");
    let (address, banco) = rest.split_once('/').unwrap_or((rest, ""));
    let (host, porta) = match address.strip_prefix('[') {
        Some(v6) => { let Some((h, p)) = v6.split_once(']') else { return false }; (h, p.strip_prefix(':')) }
        None => match address.rsplit_once(':') { Some((h, p)) => (h, Some(p)), None => (address, None) },
    };
    let porta = match porta { Some(p) => match p.parse::<u16>() { Ok(p) if p > 0 => p, _ => return false }, None => c.motor.porta() };
    c.host = host.into(); c.porta = porta; c.banco = banco.into();
    true
}
pub(super) fn engines(ui: &mut Ui, connections: &[Conexao]) -> Option<Motor> {
    let mut pick = None;
    egui::ScrollArea::vertical().max_height(460.0).show(ui, |ui| {
        let mut recent: Vec<Motor> = vec![];
        for c in connections.iter().rev() { if ATIVOS.contains(&c.motor) && !recent.contains(&c.motor) { recent.push(c.motor); } }
        if !recent.is_empty() {
            ui.label(egui::RichText::new("Recentes").color(t::MUTED));
            for motor in recent { if ui.button(motor.nome()).clicked() { pick = Some(motor); ui.close(); } }
            ui.separator();
        }
        ui.label(egui::RichText::new("Suporte completo").color(t::MUTED));
        for motor in ATIVOS { if ui.button(motor.nome()).clicked() { pick = Some(motor); ui.close(); } }
        for name in ["Amazon Aurora MySQL", "Amazon DocumentDB", "Amazon DynamoDB", "Amazon Redshift", "Apache Cassandra", "Apache Derby", "Apache Hive", "Azure SQL Database", "Azure Synapse Analytics", "ClickHouse", "CockroachDB", "Couchbase Server", "Exasol", "Google BigQuery", "Greenplum", "H2", "HSQLDB", "IBM Db2", "MongoDB", "Oracle", "PostgreSQL", "Redis", "SQL Server", "Snowflake", "Sybase ASE", "Vertica"] {
            ui.add_enabled(false, egui::Button::new(format!("{name} — em breve")));
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_url_fills_the_fields_back() {
        let mut c = Conexao { motor: Motor::Mariadb, host: "localhost".into(), porta: 3306, ..Default::default() };
        assert_eq!(jdbc(&c), "jdbc:mariadb://localhost:3306/");
        assert!(parse_url(&mut c, "jdbc:mariadb://db.local:3308/loja?useSSL=false"));
        assert_eq!((c.host.as_str(), c.porta, c.banco.as_str()), ("db.local", 3308, "loja"));
        assert!(parse_url(&mut c, "jdbc:mariadb://[::1]/x"));
        assert_eq!((c.host.as_str(), c.porta, c.banco.as_str()), ("::1", 3306, "x"));
        assert_eq!(jdbc(&c), "jdbc:mariadb://[::1]:3306/x");
        // a half-typed port or another engine's URL leaves the fields alone
        assert!(!parse_url(&mut c, "jdbc:mariadb://h:"));
        assert!(!parse_url(&mut c, "jdbc:mysql://h:1/y"));
        assert_eq!(c.host, "::1");
        let mut f = Conexao::default();
        assert!(parse_url(&mut f, "jdbc:sqlite:/tmp/a.db"));
        assert_eq!(f.arquivo, "/tmp/a.db");
    }
}
