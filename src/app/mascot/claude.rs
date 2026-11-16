//! O Claude Code na ilha: o Clawd, o mascote dele como o terminal do Claude Code o desenha (`▐▛███▜▌`, `▝▜█████▛▘`,
//! `▘▘ ▝▝`), em pixels quadrados (16 × 10), no lugar do mago quando o agente (ou quem pergunta) é o Claude Code. Ele se
//! move como o mago, pelo mesmo motor (`mago::with_motion`): respira, pula, pisca, olha em volta ou para o cursor, e o
//! estado do mago vira a cara dele. Os pixels caem inteiros na tela (nítidos mesmo nos minis de 11 px da ilha), e o
//! desenho de 30 colunas do avatar (`mascot/mod.rs`), que nesse tamanho virava um risco, fica para os tamanhos grandes.

use crate::app::mascot::mago::{EyeKind, MagoMotion, MagoState, gaze_toward, with_motion};
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

/// O Clawd em faixas de 2 pixels de altura (os quartos de bloco do terminal são duas vezes mais altos que largos):
/// a cabeça, os olhos (os dois buracos), os braços, a barriga e as quatro pernas.
const SPRITE: [&str; 5] = ["..############..", "..##.######.##..", "################", "..############..", "...#.#....#.#..."];
/// Largura e altura em pixels do desenho.
const COLUMNS: f32 = 16.0;
const ROWS: f32 = 10.0;
/// As colunas dos olhos e a faixa deles (de cima, em pixels).
const EYES: [f32; 2] = [4.0, 11.0];
const EYE_TOP: f32 = 2.0;
const BODY: Color32 = Color32::from_rgb(0xd9, 0x77, 0x57);
const EYE: Color32 = Color32::from_rgb(0x1f, 0x1e, 0x1d);

/// O mascote em `center`, `size` de largura, olhando em volta sozinho (os minis); `id` dá a cada um o seu ritmo.
pub(crate) fn paint_claude(painter: &Painter, id: &str, center: Pos2, size: f32, state: MagoState, time: f64) {
    paint_claude_toward(painter, id, center, size, state, time, None);
}

/// O mesmo, com os olhos em `pointer` (nas coordenadas do pintor; nenhum: olha em volta sozinho), como o mago da ilha.
pub(crate) fn paint_claude_toward(painter: &Painter, id: &str, center: Pos2, size: f32, state: MagoState, time: f64, pointer: Option<Pos2>) {
    if size < 1.0 || !center.x.is_finite() || !center.y.is_finite() {
        return;
    }
    let gaze = pointer.map(|pointer| gaze_toward(center, pointer));
    with_motion(id, state, time, gaze, |motion| paint_clawd(painter, center, size, motion, time));
    let frame = if matches!(state, MagoState::Idle | MagoState::Sleeping) { 66 } else { 33 };
    painter.ctx().request_repaint_after(std::time::Duration::from_millis(frame));
}

/// O tamanho de um pixel do desenho para `size` de largura: inteiro na tela quando dá (um pouco menor ou maior), para os
/// quadrados não borrarem nem deixarem frestas entre si.
pub(crate) fn pixel_size(size: f32, pixels_per_point: f32) -> f32 {
    let wanted = size / COLUMNS * pixels_per_point;
    let whole = if wanted >= 0.75 { wanted.round().max(1.0) } else { wanted };
    whole / pixels_per_point
}

/// Os retângulos do corpo (sem os olhos), em pixels do desenho: cada sequência de uma faixa num só, sem frestas.
/// `legs` sobe cada par de pernas (0 a 1 pixel) e `arms` os braços (0 a 2), para andar e acenar.
pub(crate) fn body_runs(legs: [f32; 2], arms: f32) -> Vec<Rect> {
    let mut runs = Vec::new();
    for (row, line) in SPRITE.iter().enumerate() {
        let top = row as f32 * 2.0;
        let mut start = None;
        for (column, cell) in line.chars().chain(std::iter::once('.')).enumerate() {
            // os olhos são pintados à parte, por cima do corpo inteiro
            let filled = cell == '#' || (row == 1 && EYES.contains(&(column as f32)));
            match (filled, start) {
                (true, None) => start = Some(column),
                (false, Some(first)) => {
                    let (left, right) = (first as f32, column as f32);
                    if row == 2 {
                        // os braços (as duas colunas de cada ponta) sobem; a barriga fica
                        let lift = arms.clamp(0.0, 2.0);
                        runs.push(Rect::from_min_max(Pos2::new(left.max(2.0), top), Pos2::new(right.min(14.0), top + 2.0)));
                        runs.push(Rect::from_min_max(Pos2::new(0.0, top - lift), Pos2::new(2.0, top + 2.0 - lift)));
                        runs.push(Rect::from_min_max(Pos2::new(14.0, top - lift), Pos2::new(16.0, top + 2.0 - lift)));
                    } else if row == 4 {
                        // cada perna: o par 0 (a 1ª e a 3ª) e o par 1 (a 2ª e a 4ª) se revezam
                        let pair = [3.0, 10.0].contains(&left) as usize ^ 1;
                        let lift = legs[pair].clamp(0.0, 1.0);
                        runs.push(Rect::from_min_max(Pos2::new(left, top), Pos2::new(right, top + 2.0 - lift)));
                    } else {
                        runs.push(Rect::from_min_max(Pos2::new(left, top), Pos2::new(right, top + 2.0)));
                    }
                    start = None;
                }
                _ => {}
            }
        }
    }
    runs
}

/// Os dois olhos em pixels do desenho para o olhar (`look`, -1 a 1 de cada lado), a abertura (`open`, o piscar) e o tipo.
pub(crate) fn eye_rects(look: Vec2, open: f32, kind: EyeKind) -> [Rect; 2] {
    let (shift_x, shift_y) = (look.x.clamp(-1.0, 1.0) * 0.6, look.y.clamp(-1.0, 1.0) * 0.5);
    let (wide, tall, drop) = match kind {
        EyeKind::Wide => (1.2, 2.2, -0.1),
        EyeKind::Focus => (1.0, 1.0, 0.8),
        EyeKind::Happy => (1.0, 0.8, 0.0),
        EyeKind::Sleepy => (1.0, 0.5, 1.2),
        EyeKind::Round | EyeKind::Cross | EyeKind::Spiral => (1.0, 2.0, 0.0),
    };
    // piscando, a pálpebra desce até uma linha
    let tall = (tall * open.clamp(0.0, 1.0)).max(0.4);
    EYES.map(|column| {
        let center = Pos2::new(column + 0.5 + shift_x, EYE_TOP + 1.0 + drop + shift_y);
        Rect::from_center_size(center, Vec2::new(wide, tall))
    })
}

/// O Clawd `size` de largura em `center`, movido por `motion`.
fn paint_clawd(painter: &Painter, center: Pos2, size: f32, motion: &MagoMotion, time: f64) {
    let ppp = painter.ctx().pixels_per_point();
    let unit = pixel_size(size, ppp);
    let snap = |value: f32| if unit * ppp >= 1.0 { (value * ppp).round() / ppp } else { value };
    // a respiração e o pouso amassam (em altura), o pulo levanta: em pixels inteiros, como um desenho de pixel-art anda
    let squash = (motion.squash_y.value * (1.0 + motion.breath * 0.5)).clamp(0.8, 1.2);
    let lift = snap(motion.hop.max(0.0) * size * 0.9);
    let feet = Pos2::new(snap(center.x - COLUMNS * unit / 2.0), snap(center.y + ROWS * unit / 2.0) - lift);
    let place = |rect: Rect| {
        let top = feet.y - (ROWS - rect.top()) * unit * squash;
        let bottom = feet.y - (ROWS - rect.bottom()) * unit * squash;
        Rect::from_min_max(Pos2::new(feet.x + rect.left() * unit, snap(top)), Pos2::new(feet.x + rect.right() * unit, snap(bottom)))
    };
    // a sombra no chão, menor no pulo
    if size >= 26.0 {
        let shade = (1.0 - motion.hop * 3.0).clamp(0.4, 1.0);
        let ground = Pos2::new(center.x, center.y + ROWS * unit / 2.0 + unit * 0.6);
        painter.add(Shape::ellipse_filled(ground, Vec2::new(COLUMNS * unit * 0.4 * shade, unit * 0.7 * shade), Color32::from_black_alpha(36)));
    }
    let state = motion.look;
    let body = BODY.lerp_to_gamma(motion.tint_color, motion.tint_amount.value);
    // trabalhando, as pernas andam; acenando ou no fim de uma tarefa, os braços sobem
    let walking = state.eye == EyeKind::Focus;
    let step = |phase: f64| if walking { ((time * 7.0 + phase).sin().max(0.0)) as f32 } else { 0.0 };
    let arms = motion.hands.value * 2.0;
    for run in body_runs([step(0.0), step(std::f64::consts::PI)], arms) {
        painter.rect_filled(place(run), 0.0, body);
    }
    let look = Vec2::new(motion.look_x.value, motion.look_y.value);
    for eye in eye_rects(look, motion.open.value, state.eye) {
        painter.rect_filled(place(eye), 0.0, EYE);
    }
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
    fn the_sprite_is_clawd_sixteen_by_ten_with_two_eyes_and_four_legs() {
        assert!(SPRITE.iter().all(|row| row.chars().count() == COLUMNS as usize));
        assert_eq!(SPRITE.len() as f32 * 2.0, ROWS);
        let runs = body_runs([0.0, 0.0], 0.0);
        let union = runs.iter().fold(runs[0], |all, run| all.union(*run));
        assert_eq!(union, Rect::from_min_max(Pos2::ZERO, Pos2::new(COLUMNS, ROWS)), "the whole figure, arms and legs");
        let legs = runs.iter().filter(|run| run.top() == 8.0).count();
        assert_eq!(legs, 4);
        // the eyes are holes in the head's second band
        assert!(EYES.iter().all(|&column| SPRITE[1].chars().nth(column as usize) == Some('.')));
        // a pair of legs lifts, the arms go up
        let walking = body_runs([1.0, 0.0], 2.0);
        assert_eq!(walking.iter().filter(|run| run.top() == 8.0 && run.height() == 1.0).count(), 2);
        assert!(walking.iter().any(|run| run.left() == 0.0 && run.top() == 2.0), "an arm raised");
    }

    #[test]
    fn its_pixels_land_whole_on_the_screen_and_the_eyes_look_and_blink() {
        assert_eq!(pixel_size(12.65, 1.0), 1.0, "a mini: one screen pixel each, not 0.8");
        assert_eq!(pixel_size(57.0, 1.0), 4.0);
        assert_eq!(pixel_size(57.0, 2.0), 3.5, "half points on a 2x screen");
        assert!(pixel_size(6.0, 1.0) < 0.75, "tiny: as it is");
        let [left, right] = eye_rects(Vec2::ZERO, 1.0, EyeKind::Round);
        assert_eq!((left.width(), left.height()), (1.0, 2.0));
        assert_eq!(right.center().x - left.center().x, 7.0);
        let looking = eye_rects(Vec2::new(1.0, 0.0), 1.0, EyeKind::Round);
        assert!(looking[0].center().x > left.center().x, "to the right");
        let blinking = eye_rects(Vec2::ZERO, 0.0, EyeKind::Round);
        assert!(blinking[0].height() < 0.5, "shut: a line");
        assert!(eye_rects(Vec2::ZERO, 1.0, EyeKind::Sleepy)[0].height() < 1.0);
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
            for size in [12.65, 30.0, 57.0] {
                let output =
                    Context::default().run_ui(egui::RawInput::default(), |ui| paint_claude(ui.painter(), "teste", Pos2::new(50.0, 50.0), size, state, 3.3));
                let rects = output.shapes.iter().filter(|shape| matches!(shape.shape, Shape::Rect(_))).count();
                assert!(rects >= 10, "{state:?} {size}: the body's runs and the two eyes ({rects})");
            }
        }
        let output =
            Context::default().run_ui(egui::RawInput::default(), |ui| paint_claude(ui.painter(), "sem-tamanho", Pos2::ZERO, 0.0, MagoState::Idle, 1.0));
        assert!(output.shapes.is_empty());
    }
}
