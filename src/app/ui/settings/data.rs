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

/// Ajustes › Dados: where everything is kept, what leaves this computer and what the Maestro remembers.
pub(super) fn settings_data(ui: &mut egui::Ui, k: &K) {
    let open = |path: &std::path::Path| {
        let url = format!("file:///{}", path.to_string_lossy().replace('\\', "/").trim_start_matches('/'));
        crate::app::browser::open_external(&url);
    };
    title(ui, k, "LOCAL DE ARMAZENAMENTO");
    note(
        ui,
        k,
        "Tudo fica numa pasta só deste computador: os desenhos e os espaços de trabalho, os chats, as tarefas, o fichário, as skills, os eventos dos agentes, as preferências e o que foi recuperado de um fechamento.",
    );
    if let Some(folder) = autosave_path().and_then(|p| p.parent().map(|d| d.to_path_buf())) {
        row(ui, k, "Local", "", |ui| {
            if button(ui, k, "Mostrar na Pasta").clicked() {
                open(&folder);
            }
            ui.label(egui::RichText::new(folder.display().to_string()).size(SMALL).color(k.muted_fg));
        });
    }
    divider(ui, k);
    title(ui, k, "O QUE SAI DESTE COMPUTADOR");
    note(
        ui,
        k,
        "O app não envia nada por conta própria. Os agentes (Claude Code, Codex) falam com os serviços deles, com o login deles. A voz do Codex e a voz pela API da OpenAI mandam o áudio para esses serviços; a voz local fica aqui. As páginas dos navegadores são sites comuns, cada navegador com o seu perfil.",
    );
    divider(ui, k);
    title(ui, k, "MEMÓRIA DO MAESTRO");
    note(ui, k, "O que ele guardou sobre como você trabalha, para as próximas conversas.");
    ui.add_space(3.25);
    // never the real file in a test
    let memory = if cfg!(test) { String::new() } else { crate::app::chat::tools::memory_text() };
    if memory.trim().is_empty() {
        note(ui, k, "Nada guardado ainda.");
    } else {
        // ponytail: the file is read each frame while this tab shows, and only its start is shown; the file itself has it all
        let shown: String = memory.chars().take(1500).collect();
        let g = galley(ui, &shown, BODY, BODY_LINE, k.fg, ui.available_width());
        let (rect, _) = ui.allocate_exact_size(g.size().max(Vec2::new(ui.available_width(), 0.0)), Sense::hover());
        ui.painter().galley(rect.min, g, k.fg);
    }
    if let Some(path) = crate::app::chat::tools::memory_path().filter(|p| p.is_file()) {
        ui.add_space(3.25);
        if button(ui, k, "Abrir o arquivo da memória").clicked() {
            open(&path);
        }
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
