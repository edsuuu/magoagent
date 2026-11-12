use super::*;

fn shown(connections: &[Conexao]) -> Vec<String> {
    let ctx = egui::Context::default();
    let mut picked = Some(Motor::Sqlite);
    let mut out = ctx.run_ui(Default::default(), |ui| picked = dialog::engines(ui, connections));
    out.textures_delta.clear();
    assert_eq!(picked, None, "nothing is picked without a press");
    out.shapes
        .iter()
        .filter_map(|clipped| match &clipped.shape {
            egui::epaint::Shape::Text(t) => Some(t.galley.text().to_string()),
            _ => None,
        })
        .collect()
}

#[test]
fn the_plus_menu_lists_only_the_engines_that_connect() {
    let words = shown(&[]);
    assert_eq!(words, ["Fonte de dados", "MariaDB", "MongoDB", "MySQL", "PostgreSQL", "Redis", "SQLite"], "{words:?}");
    assert!(words.iter().all(|w| !w.contains("breve")), "no 'em breve' rows");
    // the engines already used come first, under their own heading
    let words = shown(&[Conexao { motor: Motor::Redis, ..Default::default() }]);
    assert_eq!(&words[..3], ["Recentes", "Redis", "Fonte de dados"], "{words:?}");
}
