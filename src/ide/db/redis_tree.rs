//! Key prefixes are folders, keeping a full key distinct from a same-named prefix.
use super::*;
use std::collections::BTreeMap;

#[derive(Default)]
pub(super) struct Folder {
    pub folders: BTreeMap<String, Folder>,
    pub keys: Vec<usize>,
}
pub(super) fn group(tables: &[Table]) -> Folder {
    let mut root = Folder::default();
    for (i, table) in tables.iter().enumerate() {
        let mut node = &mut root;
        let parts: Vec<_> = table.name.split(':').collect();
        for part in &parts[..parts.len() - 1] {
            node = node.folders.entry((*part).into()).or_default();
        }
        node.keys.push(i);
    }
    root
}
pub(super) fn show(ui: &mut Ui, db: &Db, id: &str, schema: &session::Schema, action: &mut tree::Action) {
    if let Some(warning) = &schema.warning {
        ui.colored_label(t::MUTED, warning);
    }
    branch(ui, db, id, schema, &group(&schema.tables), &mut vec![], 2, action);
}
#[allow(clippy::too_many_arguments)]
fn branch(ui: &mut Ui, db: &Db, id: &str, schema: &session::Schema, folder: &Folder, path: &mut Vec<String>, depth: usize, action: &mut tree::Action) {
    for (name, folder) in &folder.folders {
        path.push(name.clone());
        let key = tree::folder_key(id, &schema.name, &format!("redis:{}", path.join(":")));
        if look::tree_row(ui, depth, Some(db.open.contains(&key)), look::Mark::Folder, name, "", None).clicked() {
            action.toggle = Some(key.clone());
        }
        if db.open.contains(&key) {
            branch(ui, db, id, schema, folder, path, depth + 1, action);
        }
        path.pop();
    }
    for &i in &folder.keys {
        let table = &schema.tables[i];
        let name = table.name.rsplit(':').next().unwrap_or(&table.name);
        let r = look::tree_row(ui, depth, None, look::Mark::Table, name, &table.kind, None).on_hover_text(&table.name);
        if r.double_clicked() {
            action.table = Some((id.into(), table.clone()));
        }
        r.context_menu(|ui| {
            if ui.button("Abrir chave").clicked() {
                action.table = Some((id.into(), table.clone()));
                ui.close();
            }
        });
    }
}
