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
    egui::ScrollArea::vertical().auto_shrink(false).show(ui, |ui| {
        ui.spacing_mut().item_spacing.y = 0.0;
        for c in &db.conexoes {
            let expanded = db.open.contains(&c.id);
            let name = if c.motor == Motor::Mysql { format!("{}@{}", c.nome, c.host) } else { c.nome.clone() };
            let catalog = db.catalogs.get(&c.id);
            if look::tree_row(ui, 0, Some(expanded), look::Mark::Source, &name, c.motor.nome(), catalog.map(|c| c.schemas.len())).clicked() {
                action.pick = Some(c.id.clone()); action.toggle = Some(c.id.clone());
            }
            if !expanded { continue; }
            let Some(catalog) = catalog else { continue };
            for schema in &catalog.schemas {
                let sk = schema_key(&c.id, &schema.name);
                if look::tree_row(ui, 1, Some(db.open.contains(&sk)), look::Mark::Schema, &schema.name, "", None).clicked() { action.toggle = Some(sk.clone()); }
                if !db.open.contains(&sk) { continue; }
                let folders: &[&str] = if c.motor == Motor::Mysql { &["tabelas", "views", "rotinas", "triggers", "eventos"] } else { &["tabelas"] };
                for &folder in folders {
                    let tables: Vec<_> = schema.tables.iter().filter(|t| t.view == (folder == "views") && matches!(folder, "tabelas" | "views")).collect();
                    let objects: Vec<_> = schema.objects.iter().filter(|o| match folder {
                        "rotinas" => matches!(o.kind, ObjectKind::Procedure | ObjectKind::Function), "triggers" => o.kind == ObjectKind::Trigger, "eventos" => o.kind == ObjectKind::Event, _ => false,
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
                            if ui.button("Mostrar DDL").clicked() {
                                action.ddl = Some((c.id.clone(), Object { schema: schema.name.clone(), name: table.name.clone(), kind: if table.view { ObjectKind::View } else { ObjectKind::Table } })); ui.close();
                            }
                        });
                        if db.open.contains(&tk) { table_details(ui, table); }
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
fn table_details(ui: &mut Ui, table: &Table) {
    for c in &table.columns {
        let mut note = c.kind.to_lowercase();
        let pk_count = table.columns.iter().filter(|c| c.pk > 0).count();
        if c.auto_increment || (!table.mysql && c.pk > 0 && pk_count == 1 && c.kind.eq_ignore_ascii_case("integer") && !table.without_rowid) { note += " (auto_increment)"; }
        if c.notnull { note += " not null"; } else { note += " null"; }
        if let Some(d) = &c.default { note += &format!(" = {d}"); }
        look::tree_row(ui, 4, None, look::Mark::Column { pk: c.pk > 0 }, &c.name, &note, None);
    }
    let mut pk: Vec<_> = table.columns.iter().filter(|c| c.pk > 0).collect(); pk.sort_by_key(|c| c.pk);
    if !pk.is_empty() { look::tree_row(ui, 4, None, look::Mark::Key(look::PRIMARY_KEY), "PRIMARY KEY", &pk.iter().map(|c| c.name.as_str()).collect::<Vec<_>>().join(", "), None); }
    for fk in &table.foreign { look::tree_row(ui, 4, None, look::Mark::Key(look::FOREIGN_KEY), fk, "", None); }
    for (name, cols) in &table.indexes { look::tree_row(ui, 4, None, look::Mark::Index, name, &format!("({cols})"), None); }
}
