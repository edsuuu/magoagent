//! Ajustes › Dados: where the canvas, the app's data and the chats are, and Limpar todos os dados.

use super::kit::*;
use crate::prelude::*;

/// Opens a folder (or a file) in the system's own program.
pub(super) fn open_path(path: &std::path::Path) {
    let url = format!("file:///{}", path.to_string_lossy().replace('\\', "/").trim_start_matches('/'));

    crate::app::browser::open_external(&url);
}

impl DrawingApp {
    /// Ajustes › Dados, in three groups: where the canvas's drawings are saved, the app's data, and where the chats are.
    /// Each place with its path and a button that opens it.
    pub(super) fn settings_data(&mut self, ui: &mut egui::Ui, k: &K) {
        let drawing = self.autosave.clone().or_else(autosave_path);
        let folder = drawing.as_ref().and_then(|path| path.parent().map(|parent| parent.to_path_buf()));

        title(ui, k, "DESENHOS DO CANVAS");

        note(
            ui,
            k,
            "O canvas se salva sozinho a cada mudança. Cada espaço de trabalho tem o seu desenho, um arquivo .mago; a lista dos espaços fica no espacos.json, na mesma pasta.",
        );

        if let (Some(drawing), Some(folder)) = (&drawing, &folder) {
            let space = self.spaces.list.iter().find(|space| space.id == self.spaces.current).map(|space| space.name.clone());

            let hint = match space {
                Some(name) => format!("{} (espaço {name})", drawing.display()),
                None => drawing.display().to_string(),
            };

            if row(ui, k, "Desenho em uso", &hint, |ui| button(ui, k, "Abrir pasta").clicked()) {
                open_path(folder);
            }
        }

        divider(ui, k);
        title(ui, k, "DADOS DO APP");

        if let Some(folder) = &folder {
            if row(ui, k, "Pasta de dados", &folder.display().to_string(), |ui| button(ui, k, "Abrir").clicked()) {
                open_path(folder);
            }

            let hint =
                "Os desenhos e os espaços de trabalho, as tarefas, o fichário, as preferências e o que foi recuperado de um fechamento. Tudo neste computador.";

            row(ui, k, "O que fica nela", hint, |_| ());
        }

        // never the real file in a test
        let memory = if cfg!(test) { String::new() } else { crate::app::chat::tools::memory_text() };
        let notes = memory.lines().filter(|line| !line.trim().is_empty()).count();

        let hint = match notes {
            0 => "Nada guardado ainda.".to_string(),
            1 => "1 anotação sobre como você trabalha, para as próximas conversas.".to_string(),
            count => format!("{count} anotações sobre como você trabalha, para as próximas conversas."),
        };

        let file = crate::app::chat::tools::memory_path().filter(|path| path.is_file());

        if row(ui, k, "Memória da Luna", &hint, |ui| file.is_some() && button(ui, k, "Abrir").clicked())
            && let Some(path) = &file
        {
            open_path(path);
        }

        divider(ui, k);
        title(ui, k, "CHATS");

        if let Some(folder) = &folder {
            let chats = folder.join("chats");
            let hint = format!("{}: a lista dos chats (chats.json) e o que foi dito nos do Codex (a pasta chats).", folder.display());

            if row(ui, k, "Chats do app", &hint, |ui| button(ui, k, "Abrir").clicked()) {
                open_path(if chats.is_dir() { &chats } else { folder });
            }
        }

        if let Some(projects) = crate::transcript::claude_dir().map(|dir| dir.join("projects")) {
            let hint = format!("{}: o Claude Code guarda as conversas dele, e o app as lê de lá.", projects.display());

            if row(ui, k, "Conversas do Claude Code", &hint, |ui| projects.is_dir() && button(ui, k, "Abrir").clicked()) {
                open_path(&projects);
            }
        }

        if let Some(projects) = projects_dir() {
            let hint = format!("{}: onde trabalha um chat sem pasta escolhida.", projects.display());

            if row(ui, k, "Chats sem pasta", &hint, |ui| projects.is_dir() && button(ui, k, "Abrir").clicked()) {
                open_path(&projects);
            }
        }

        divider(ui, k);
        title(ui, k, "LIMPAR");
        let hint = "Apaga os desenhos, os chats, os ajustes e as senhas guardadas e abre o MagoAgent do zero. Pergunta antes.";

        if row(ui, k, "Limpar todos os dados", hint, |ui| button(ui, k, "Limpar…")).clicked() {
            self.wipe_confirm = true;
        }
    }
}

impl DrawingApp {
    /// Limpar todos os dados asks first: what goes, where, and that it can't be undone (`system/wipe.rs` does it).
    pub(crate) fn wipe_confirm_ui(&mut self, ctx: &Context) {
        if super::native_on() {
            return;
        }

        use crate::app::system::wipe::plan;
        let colors = self.colors();
        let plan = plan();

        let (confirm, cancel) = crate::app::ui::dialog_over(ctx, &colors, "Apagar todos os dados do MagoAgent?", "Apagar tudo", |ui| {
            let k = K::new(self.dark());

            let mut lines = vec![
                "O MagoAgent fecha os agentes e os terminais, apaga e abre de novo como na primeira vez. Vão embora:".to_string(),
                "• os desenhos, os espaços, as notas e as tarefas".into(),
                "• os chats e as conversas guardadas".into(),
                "• os ajustes e a memória da Luna".into(),
            ];

            if plan.keychain {
                lines.push("• o cofre de senhas do navegador e os tokens do GitHub, do Spotify e do WhatsApp".into());
                lines.push("• a pasta ~/.magoagent, onde trabalham os chats sem pasta escolhida".into());
            } else {
                lines.push("Esta é a versão de desenvolvimento: só a pasta dela é apagada (o cofre e o ~/.magoagent são do app instalado).".into());
            }

            lines.push("Os hooks instalados no Claude Code e no Codex ficam como estão. Não dá para desfazer.".into());

            for line in lines {
                let text = galley(ui, &line, BODY, BODY_LINE, k.fg, ui.available_width());
                let (rect, _) = ui.allocate_exact_size(Vec2::new(ui.available_width(), text.size().y), Sense::hover());

                ui.painter().galley(rect.min, text, k.fg);
                ui.add_space(3.0);
            }

            ui.add_space(5.0);
            let folders = plan.folders.iter().map(|folder| folder.display().to_string()).collect::<Vec<_>>().join("\n");
            let text = galley(ui, &folders, SMALL, SMALL_LINE, k.muted_fg, ui.available_width());
            let (rect, _) = ui.allocate_exact_size(Vec2::new(ui.available_width(), text.size().y), Sense::hover());

            ui.painter().galley(rect.min, text, k.muted_fg);
        });

        if cancel {
            self.wipe_confirm = false;
        } else if confirm {
            self.wipe_confirm = false;

            if let Err(why) = self.wipe_all_data() {
                self.toast(format!("Nada foi apagado: {why}."));
            }
        }
    }
}
