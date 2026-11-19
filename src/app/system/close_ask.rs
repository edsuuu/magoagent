//! Closing the window with agents at work: with "Fechar mantém em segundo plano" the app went on running hidden and
//! the agents kept working without the person knowing. The close now asks: go on in the background, stop everything
//! (chat/stop.rs) and close, or stay.

use crate::prelude::*;

/// What the question was answered with.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) enum CloseAnswer {
    Background,
    StopAndClose,
    Stay,
}

impl DrawingApp {
    /// The close asked while agents work: the question, until it is answered; what it was answered with.
    pub(crate) fn close_ask_ui(&mut self, ctx: &Context) -> Option<CloseAnswer> {
        let working = self.agents_at_work();
        let colors = self.colors();
        let mut background = false;

        let (stop, stay) = dialog_ui(ctx, &colors, "Agentes trabalhando", "Parar tudo e fechar", |ui| {
            let names = if working.is_empty() { "Há mensagens na fila para os agentes".to_string() } else { working.join(", ") };

            dialog_note(ui, &format!("{names}. Eles só continuam com a sua permissão: deixe-os seguir em segundo plano ou pare tudo."), &colors);
            ui.add_space(8.0);
            background = ui.button("Continuar em segundo plano").clicked();
        });

        match (stop, background, stay) {
            (true, ..) => Some(CloseAnswer::StopAndClose),
            (_, true, _) => Some(CloseAnswer::Background),
            (.., true) => Some(CloseAnswer::Stay),
            _ => None,
        }
    }
}
