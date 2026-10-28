//! Data sources, schemas and object folders, using the existing tree marks.
use super::*;
use session::{Object, ObjectKind};

#[derive(Default)]
pub(super) struct Action {
    pub toggle: Option<String>, pub pick: Option<String>,
    pub table: Option<(String, Table)>, pub ddl: Option<(String, Object)>,
}
// Length-prefixed parts keep names containing separators distinct.
fn key(parts: &[&str]) -> String { parts.iter().map(|p| format!("{}:{p}", p.len())).collect() }
pub(super) fn schema_key(id: &str, schema: &str) -> String { key(&[id, schema]) }
pub(super) fn folder_key(id: &str, schema: &str, folder: &str) -> String { key(&[id, schema, folder]) }

pub(super) fn show(ui: &mut Ui, db: &Db) -> Action {
    let mut action = Action::default();
    egui::ScrollArea::both().auto_shrink(false).show(ui, |ui| {
        ui.spacing_mut().item_spacing.y = 0.0;
        for c in &db.conexoes {
            let expanded = db.open.contains(&c.id);
            let name = if c.motor != Motor::Sqlite { format!("{}@{}", c.nome, c.host) } else { c.nome.clone() };
            let dialect = c.motor.dialect();
            let catalog = db.catalogs.get(&c.id);
            // the active source (the one the toolbar acts on) is lit
            let (row, right) = look::list_row(ui, db.active.as_ref() == Some(&c.id), 0, Some(expanded), look::Mark::Source, &name, c.motor.nome(), catalog.map(|c| c.schemas.len()));
            ui.expand_to_include_x(right + 8.0);
            if row.clicked() {
                action.pick = Some(c.id.clone()); action.toggle = Some(c.id.clone());
            }
            if !expanded { continue; }
            let Some(catalog) = catalog else { continue };
            for schema in &catalog.schemas {
                let sk = schema_key(&c.id, &schema.name);
                if look::tree_row(ui, 1, Some(db.open.contains(&sk)), look::Mark::Schema, &schema.name, "", schema.count).clicked() { action.toggle = Some(sk.clone()); }
                if !db.open.contains(&sk) { continue; }
                if dialect == Dialect::Redis { redis_tree::show(ui, db, &c.id, schema, &mut action); continue; }
                let folders: &[&str] = match dialect { Dialect::Mongo => &["coleções"], Dialect::Mysql => &["tabelas", "views", "rotinas", "triggers", "eventos"], Dialect::Postgres => &["tabelas", "views", "views materializadas", "sequências", "rotinas"], _ => &["tabelas"] };
                for &folder in folders {
                    let tables: Vec<_> = schema.tables.iter().filter(|t| match folder { "tabelas" | "coleções" => !t.view, "views" => t.view && !t.materialized, "views materializadas" => t.materialized, _ => false }).collect();
                    let objects: Vec<_> = schema.objects.iter().filter(|o| match folder {
                        "sequências" => o.kind == ObjectKind::Sequence, "rotinas" => matches!(o.kind, ObjectKind::Procedure | ObjectKind::Function), "triggers" => o.kind == ObjectKind::Trigger, "eventos" => o.kind == ObjectKind::Event, _ => false,
                    }).collect();
                    let fk = folder_key(&c.id, &schema.name, folder);
                    let count = tables.len() + objects.len();
                    if look::tree_row(ui, 2, Some(db.open.contains(&fk)), look::Mark::Folder, folder, &count.to_string(), None).clicked() { action.toggle = Some(fk.clone()); }
                    if !db.open.contains(&fk) { continue; }
                    for table in tables {
                        let tk = key(&[&c.id, &schema.name, folder, &table.name]);
                        let response = look::tree_row(ui, 3, Some(db.open.contains(&tk)), look::Mark::Table, &table.name, "", None).on_hover_text("Clique duplo abre os dados; menu de contexto mostra o DDL");
                        if response.double_clicked() { action.table = Some((c.id.clone(), table.clone())); }
                        else if response.clicked() {
                            action.toggle = Some(tk.clone());
                        }
                        response.context_menu(|ui| {
                            if ui.button("Editar dados").clicked() { action.table = Some((c.id.clone(), table.clone())); ui.close(); }
                            if !matches!(dialect, Dialect::Mongo | Dialect::Redis) && ui.button("Mostrar DDL").clicked() {
                                action.ddl = Some((c.id.clone(), Object { oid: None, schema: schema.name.clone(), name: table.name.clone(), kind: if table.materialized { ObjectKind::MaterializedView } else if table.view { ObjectKind::View } else { ObjectKind::Table } })); ui.close();
                            }
                        });
                        if db.open.contains(&tk) { table_details(ui, table, &c.id, &mut action); }
                    }
                    for object in objects {
                        let response = look::tree_row(ui, 3, None, look::Mark::Index, &object.name, object.kind.sql(), None).on_hover_text("Clique duplo mostra o DDL");
                        if response.double_clicked() { action.ddl = Some((c.id.clone(), object.clone())); }
                        response.context_menu(|ui| {
                            if ui.button("Mostrar DDL").clicked() { action.ddl = Some((c.id.clone(), object.clone())); ui.close(); }
                        });
                    }
                }
            }
        }
    });
    action
}
fn table_details(ui: &mut Ui, table: &Table, id: &str, action: &mut Action) {
    if table.dialect == Dialect::Mongo {
        mongo_columns(ui, &table.columns, 4);
        for (name, cols) in &table.indexes { look::tree_row(ui, 4, None, look::Mark::Index, name, cols, None); }
        return;
    }
    for c in &table.columns {
        let mut note = c.kind.to_lowercase();
        let pk_count = table.columns.iter().filter(|c| c.pk > 0).count();
        if c.auto_increment || (table.dialect == Dialect::Sqlite && c.pk > 0 && pk_count == 1 && c.kind.eq_ignore_ascii_case("integer") && !table.without_rowid) { note += " (auto-incremento)"; }
        if c.generated { note += " (gerada)"; }
        if c.notnull { note += " not null"; } else { note += " null"; }
        if let Some(d) = &c.default { note += &format!(" = {d}"); }
        look::tree_row(ui, 4, None, look::Mark::Column { pk: c.pk > 0 }, &c.name, &note, None);
    }
    let mut pk: Vec<_> = table.columns.iter().filter(|c| c.pk > 0).collect(); pk.sort_by_key(|c| c.pk);
    if !pk.is_empty() { look::tree_row(ui, 4, None, look::Mark::Key(look::PRIMARY_KEY), "PRIMARY KEY", &pk.iter().map(|c| c.name.as_str()).collect::<Vec<_>>().join(", "), None); }
    for fk in &table.foreign { look::tree_row(ui, 4, None, look::Mark::Key(look::FOREIGN_KEY), fk, "", None); }
    for (name, cols) in &table.indexes {
        let response = look::tree_row(ui, 4, None, look::Mark::Index, name, &format!("({cols})"), None);
        if table.dialect == Dialect::Postgres {
            let object = Object { schema: table.schema.clone(), name: name.clone(), kind: ObjectKind::Index, oid: None };
            if response.double_clicked() { action.ddl = Some((id.into(), object.clone())); }
            response.context_menu(|ui| { if ui.button("Mostrar DDL").clicked() { action.ddl = Some((id.into(), object)); ui.close(); } });
        }
    }
    for trigger in &table.triggers {
        let response = look::tree_row(ui, 4, None, look::Mark::Index, &trigger.name, "trigger", None);
        if response.double_clicked() { action.ddl = Some((id.into(), trigger.clone())); }
        response.context_menu(|ui| { if ui.button("Mostrar DDL").clicked() { action.ddl = Some((id.into(), trigger.clone())); ui.close(); } });
    }
}

fn mongo_columns(ui: &mut Ui, columns: &[Column], depth: usize) {
    for c in columns {
        look::tree_row(ui, depth, None, look::Mark::Column { pk: c.pk > 0 }, &c.name, &c.kind, None);
        mongo_columns(ui, &c.children, depth + 1);
    }
}
