//! Ajustes › Email: the bridge to a mail MCP server (Gmail or another) the person sets up (`chat/email.rs`). The agents
//! of the chats get it next to the app's own MCP; the app starts nothing and downloads nothing.

use super::kit::*;
use crate::app::chat::email::{GMAIL_EXAMPLE, probe, split_args};
use crate::prelude::*;
use std::time::Duration;

/// Where the last "Testar" left its answer, kept for the frame that draws it.
fn test_id() -> egui::Id {
    egui::Id::unique("settings_email_test")
}

impl DrawingApp {
    pub(super) fn settings_email(&mut self, ui: &mut egui::Ui, k: &K) {
        title(ui, k, "SERVIDOR DE EMAIL");
        note(
            ui,
            k,
            "Aponte um servidor MCP de email (o do Gmail, por exemplo) e os agentes dos chats ganham as ferramentas dele: ler, buscar, rascunhar e responder. O app não instala nem baixa nada: o programa precisa estar no seu computador, e o servidor cuida do próprio login com o Google. Se o seu Claude Code já tem o conector do Gmail (claude.ai), os chats dele já o usam e você não precisa disto.",
        );
        ui.add_space(3.25);
        toggle(ui, k, "Dar o servidor aos agentes", "Vale para os chats novos e para as mensagens seguintes de cada chat.", &mut self.prefs.email.on);
        let email = &mut self.prefs.email;
        row(ui, k, "Programa", "O comando que inicia o servidor, como npx.", |ui| {
            input(ui, k, egui::Id::unique("settings_email_command"), "Programa", &mut email.command, "npx", 230.0, false)
        });
        row(ui, k, "Argumentos", "Numa linha só; aspas juntam uma palavra com espaço.", |ui| {
            input(ui, k, egui::Id::unique("settings_email_args"), "Argumentos", &mut email.args, "-y pacote-do-servidor", 230.0, false)
        });
        let hint = "Preenche o servidor de Gmail mais usado (@gongrzhe/server-gmail-autoauth-mcp). O npx baixa o pacote na primeira vez que o servidor roda; o login do Google é feito por ele, seguindo as instruções do pacote.";
        if row(ui, k, "Exemplo", hint, |ui| button(ui, k, "Usar o do Gmail").clicked()) {
            (email.command, email.args) = (GMAIL_EXAMPLE.0.into(), GMAIL_EXAMPLE.1.into());
        }
        let status = email.status();
        let (command, args) = (email.command.trim().to_string(), split_args(&email.args));
        let found = email.found();
        let answer = ui.data(|d| d.get_temp::<Result<String, String>>(test_id()));
        let testing = ui.data(|d| d.get_temp::<bool>(test_id().with("running"))).unwrap_or(false);
        let label = if testing { "Testando…" } else { "Testar" };
        if row(ui, k, "Conexão", &status, |ui| button(ui, k, label).clicked()) && found && !testing {
            let ctx = ui.ctx().clone();
            ui.data_mut(|d| {
                d.insert_temp(test_id().with("running"), true);
                d.remove::<Result<String, String>>(test_id());
            });
            std::thread::spawn(move || {
                let result = probe(&command, &args, Duration::from_secs(20));
                ctx.data_mut(|d| {
                    d.insert_temp(test_id(), result);
                    d.insert_temp(test_id().with("running"), false);
                });
                ctx.request_repaint();
            });
        }
        match answer {
            Some(Ok(name)) => note(ui, k, &format!("O servidor respondeu: {name}.")),
            Some(Err(why)) => note(ui, k, &format!("Não deu certo: {why}.")),
            None if testing => ui.ctx().request_repaint_after(Duration::from_millis(200)),
            None => {}
        }
        divider(ui, k);
        title(ui, k, "PRIVACIDADE");
        note(
            ui,
            k,
            "Os agentes só leem ou mandam email pelas ferramentas do servidor que você pôs aqui, e o modo do chat (Ajustes › Chat) decide se o agente pede a sua permissão para usá-las. O MagoAgent não guarda o seu email nem a senha: o login fica com o servidor.",
        );
    }
}
