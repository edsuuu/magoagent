//! Ajustes › Dados and Sobre.

use super::kit::*;
use crate::prelude::*;

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
