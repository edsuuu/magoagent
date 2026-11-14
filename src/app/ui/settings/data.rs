//! Ajustes › Atalhos, Dados and Sobre.

use super::kit::*;
use crate::prelude::*;

/// Ajustes › Atalhos: the shortcuts the app answers to, by subject, written the Mac way (`keys_label`
/// turns them into Ctrl and Alt elsewhere). Kept beside app/shortcuts.rs by hand.
const SHORTCUTS: [(&str, &[(&str, &str)]); 4] = [
    (
        "GERAL",
        &[
            ("Ajustes", "Cmd+,"),
            ("Paleta de comandos", "Cmd+/"),
            ("Painel de agentes", "Cmd+J"),
            ("Chat", "Cmd+1"),
            ("Mostrar ou esconder a grade", "Cmd+'"),
            ("Tema claro ou escuro", "Shift+Option+D"),
        ],
    ),
    ("ARQUIVO", &[("Salvar", "Cmd+S"), ("Salvar como", "Cmd+Shift+S"), ("Abrir", "Cmd+O"), ("Exportar imagem", "Cmd+Shift+E")]),
    (
        "EDIÇÃO",
        &[
            ("Desfazer", "Cmd+Z"),
            ("Refazer", "Cmd+Shift+Z ou Cmd+Y"),
            ("Selecionar tudo", "Cmd+A"),
            ("Duplicar", "Cmd+D"),
            ("Apagar a seleção", "Delete"),
            ("Limpar o canvas", "Cmd+Delete"),
            ("Agrupar", "Cmd+G"),
            ("Desagrupar", "Cmd+Shift+G"),
            ("Travar ou destravar", "Cmd+Shift+L"),
            ("Arrumar a seleção", "Cmd+Shift+T"),
            ("Mover a seleção (com Shift, 5 vezes mais)", "Setas"),
            ("Uma camada à frente ou atrás", "Cmd+] e Cmd+["),
            ("Trazer para a frente ou enviar para trás", "Cmd+Shift+] e Cmd+Shift+["),
        ],
    ),
    (
        "VISTA",
        &[
            ("Aproximar e afastar", "Cmd++ e Cmd+-"),
            ("Voltar a 100%", "Cmd+0"),
            ("Ver tudo", "Shift+1"),
            ("Ver a seleção", "Shift+2"),
            ("Mover a vista", "Espaço + arrastar"),
        ],
    ),
];

/// The pieces of a shortcut's text, each a key (`true`, drawn as a keycap) or a word between keys.
fn key_pieces(keys: &str) -> Vec<(String, bool)> {
    let mut pieces = vec![];
    for word in keys_label(keys).split(' ') {
        let plain = matches!(word, "ou" | "e" | "+") || word.chars().all(|c| c.is_lowercase());
        if plain {
            pieces.push((word.to_string(), false));
        } else {
            // "Ctrl+Shift+Z" is three caps; a lone "+" or "+" after a key ("Cmd++") is the plus key itself
            let mut keys: Vec<String> = word.split('+').map(String::from).collect();
            if keys.len() > 1 && keys.last().is_some_and(|l| l.is_empty()) {
                keys.pop();
                if let Some(last) = keys.last_mut() {
                    *last = "+".into();
                }
            }
            pieces.extend(keys.into_iter().filter(|key| !key.is_empty()).map(|key| (key, true)));
        }
    }
    pieces
}

/// Whether a shortcut answers to the search: its name or its keys (as written on this system), any case.
fn shortcut_matches(what: &str, keys: &str, query: &str) -> bool {
    let query = query.trim().to_lowercase();
    query.is_empty() || what.to_lowercase().contains(&query) || keys_label(keys).to_lowercase().contains(&query)
}

/// Ajustes › Atalhos: `SHORTCUTS`, what on the left and its keys, as keycaps, on the right.
pub(super) fn settings_shortcuts(ui: &mut egui::Ui, k: &K) {
    let id = egui::Id::unique("settings_shortcut_search");
    let mut query = ui.ctx().data(|d| d.get_temp::<String>(id)).unwrap_or_default();
    let width = ui.available_width();
    input(ui, k, id.with("field"), "Buscar atalho", &mut query, "Buscar pelo nome ou pela tecla", width, false);
    ui.ctx().data_mut(|d| d.insert_temp(id, query.clone()));
    ui.add_space(16.25);
    let mut shown = 0;
    for (subject, rows) in SHORTCUTS.iter() {
        let rows: Vec<_> = rows.iter().filter(|(what, keys)| shortcut_matches(what, keys, &query)).collect();
        if rows.is_empty() {
            continue;
        }
        if shown > 0 {
            divider(ui, k);
        }
        shown += 1;
        title(ui, k, subject);
        for (what, keys) in rows {
            let (rect, _) = ui.allocate_exact_size(Vec2::new(ui.available_width(), 38.0), Sense::hover());
            let text = galley(ui, what, BODY, BODY_LINE, k.fg, f32::INFINITY);
            ui.painter().galley(Pos2::new(rect.left(), rect.center().y - text.size().y / 2.0), text, k.fg);
            // from the right edge inwards
            let mut x = rect.right();
            for (piece, cap) in key_pieces(keys).into_iter().rev() {
                let g = ui.painter().layout_no_wrap(piece, FontId::proportional(SMALL), if cap { k.fg } else { k.muted_fg });
                let width = g.size().x + if cap { 12.0 } else { 0.0 };
                let spot = Rect::from_min_size(Pos2::new(x - width, rect.center().y - 11.0), Vec2::new(width, 22.0));
                if cap {
                    ui.painter().rect_filled(spot, 4.0, k.muted);
                    ui.painter().rect_stroke(spot, 4.0, Stroke::new(1.0, k.line(0.6)), StrokeKind::Inside);
                }
                let color = if cap { k.fg } else { k.muted_fg };
                ui.painter().galley(Pos2::new(spot.center().x - g.size().x / 2.0, spot.center().y - g.size().y / 2.0), g, color);
                x -= width + 4.0;
            }
        }
    }
    if shown == 0 {
        note(ui, k, "Nenhum atalho com esse nome ou essa tecla.");
    }
    ui.add_space(6.5);
    note(ui, k, "As letras das ferramentas de desenho só valem com a fileira de desenho aberta; dentro de um terminal, as teclas são dele.");
}

/// Ajustes › Dados, in rows as the other tabs: the data folder and the chats (each with Abrir), what the
/// Maestro remembers and what leaves this computer.
pub(super) fn settings_data(ui: &mut egui::Ui, k: &K) {
    let open = |path: &std::path::Path| {
        let url = format!("file:///{}", path.to_string_lossy().replace('\\', "/").trim_start_matches('/'));
        crate::app::browser::open_external(&url);
    };
    title(ui, k, "ARMAZENAMENTO");
    note(ui, k, "Tudo do app fica numa pasta só, neste computador.");
    ui.add_space(3.25);
    if let Some(folder) = autosave_path().and_then(|path| path.parent().map(|parent| parent.to_path_buf())) {
        if row(ui, k, "Pasta de dados", &folder.display().to_string(), |ui| button(ui, k, "Abrir").clicked()) {
            open(&folder);
        }
        let hint = "Os desenhos e os espaços de trabalho, as tarefas, o fichário, as preferências e o que foi recuperado de um fechamento.";
        row(ui, k, "O que fica nela", hint, |_| ());
        let chats = folder.join("chats");
        let hint = "As conversas do chat; as do Claude Code ficam também na pasta dele.";
        if row(ui, k, "Chats", hint, |ui| button(ui, k, "Abrir").clicked()) {
            open(if chats.is_dir() { &chats } else { &folder });
        }
    }
    divider(ui, k);
    title(ui, k, "MEMÓRIA DO MAESTRO");
    // never the real file in a test
    let memory = if cfg!(test) { String::new() } else { crate::app::chat::tools::memory_text() };
    let notes = memory.lines().filter(|line| !line.trim().is_empty()).count();
    let hint = match notes {
        0 => "Nada guardado ainda.".to_string(),
        1 => "1 anotação sobre como você trabalha, para as próximas conversas.".to_string(),
        count => format!("{count} anotações sobre como você trabalha, para as próximas conversas."),
    };
    let file = crate::app::chat::tools::memory_path().filter(|path| path.is_file());
    if row(ui, k, "O que ele guardou", &hint, |ui| file.is_some() && button(ui, k, "Abrir").clicked())
        && let Some(path) = &file
    {
        open(path);
    }
    divider(ui, k);
    title(ui, k, "PRIVACIDADE");
    let rows = [
        ("O app", "Não envia nada por conta própria."),
        ("Agentes", "O Claude Code e o Codex falam com os serviços deles, com o login de cada um."),
        ("Navegadores", "Sites comuns, cada navegador com o seu perfil."),
    ];
    for (label, hint) in rows {
        row(ui, k, label, hint, |_| ());
    }
}

#[test]
fn the_shortcut_search_matches_the_name_or_the_keys() {
    assert!(shortcut_matches("Salvar como", "Cmd+Shift+S", ""));
    assert!(shortcut_matches("Salvar como", "Cmd+Shift+S", " SALVAR "));
    assert!(shortcut_matches("Desfazer", "Cmd+Z", "z"));
    assert!(shortcut_matches("Ajustes", "Cmd+,", &keys_label("Cmd+,").to_lowercase()));
    assert!(!shortcut_matches("Desfazer", "Cmd+Z", "salvar"));
}
