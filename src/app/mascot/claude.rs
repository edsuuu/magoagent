//! O Claude Code na ilha: o mascote de pixel-art que o app já tem (`mascot/mod.rs`) no lugar do mago, quando o agente
//! (ou quem pergunta) é o Claude Code. O estado do mago vira a expressão dele.

use crate::app::maestro::avatar::AvatarState;
use crate::app::mascot::mago::MagoState;
use crate::app::mascot::paint_mascot_still;
use crate::prelude::*;

/// Qual personagem a ilha mostra para um agente.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) enum Mascot {
    #[default]
    Mago,
    Claude,
}

impl Mascot {
    /// O programa como o cartão o nomeia (`stage::program_name`): só o Claude Code ganha o mascote dele.
    pub(crate) fn of_program(program: &str) -> Self {
        if program == "Claude Code" { Self::Claude } else { Self::Mago }
    }
}

/// A expressão do mascote para o estado do mago: trabalhando pensa, esperando você fica atento, terminou pula.
fn avatar_state(state: MagoState) -> AvatarState {
    match state {
        MagoState::Working => AvatarState::Thinking,
        MagoState::NeedsYou | MagoState::Asking | MagoState::Listening => AvatarState::Listening,
        MagoState::Done | MagoState::Speaking | MagoState::Waving => AvatarState::Speaking,
        MagoState::Sleeping => AvatarState::Muted,
        MagoState::Idle | MagoState::Error => AvatarState::Idle,
    }
}

/// O mascote em `center`, `size` de largura; `id` dá a cada um o seu ritmo (piscam fora de compasso).
pub(crate) fn paint_claude(painter: &Painter, id: &str, center: Pos2, size: f32, state: MagoState, time: f64) {
    if size < 1.0 {
        return;
    }
    let phase = id.bytes().fold(0u32, |hash, byte| hash.wrapping_mul(31).wrapping_add(byte as u32)) as f64 % 7.0;
    paint_mascot_still(painter, center, size, avatar_state(state), time + phase);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_claude_code_gets_its_own_mascot() {
        assert_eq!(Mascot::of_program("Claude Code"), Mascot::Claude);
        assert_eq!(Mascot::of_program("Codex"), Mascot::Mago);
        assert_eq!(Mascot::of_program("Hermes Agent"), Mascot::Mago);
    }

    #[test]
    fn it_draws_in_every_state_and_nothing_without_size() {
        let states = [
            MagoState::Idle,
            MagoState::Working,
            MagoState::NeedsYou,
            MagoState::Asking,
            MagoState::Done,
            MagoState::Error,
            MagoState::Sleeping,
            MagoState::Waving,
        ];
        for state in states {
            let output =
                Context::default().run_ui(egui::RawInput::default(), |ui| paint_claude(ui.painter(), "teste", Pos2::new(50.0, 50.0), 60.0, state, 3.3));
            assert!(output.shapes.len() > 100, "{state:?} paints the squares");
        }
        let output =
            Context::default().run_ui(egui::RawInput::default(), |ui| paint_claude(ui.painter(), "sem-tamanho", Pos2::ZERO, 0.0, MagoState::Idle, 1.0));
        assert!(output.shapes.is_empty());
    }
}
