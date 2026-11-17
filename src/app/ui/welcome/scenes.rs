//! Luna's scenes on the welcome's slides: each a small loop in the app's look (the canvas filling with cards, notes
//! sticking and a rope to a terminal, the island opening over a screen, agents passing messages on ropes) with Luna,
//! the app's assistant (the mascot's wizard), presenting it from a speech bubble that types itself as the slide opens.

use super::parts::{self, Status};
use crate::app::mascot::mago::{paint_mago_toward, paint_mini};
use crate::app::ui::settings::kit::K;
use crate::prelude::*;
use std::f32::consts::TAU;

/// The box the scenes are drawn in (points at a scale of 1); the welcome scales it to the window.
pub(super) const DESIGN: Vec2 = Vec2::new(720.0, 300.0);
/// How fast the bubble types, characters a second.
const TYPE_RATE: f64 = 34.0;
/// How long a thing takes to come in or go out of a loop, as a part of it.
const FADE: f32 = 0.05;
/// Where the mini canvas (and the island's screen) sits beside Luna.
const BOARD: [f32; 4] = [230.0, 8.0, 490.0, 284.0];

/// What Luna shows on a slide.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Story {
    /// She says hello.
    Hello,
    /// The canvas fills with terminals, a chat and a file tree.
    Canvas,
    /// Notes drop in and stick, she writes one, a rope ties one to a terminal.
    Notes,
    /// The island opens over a screen with the agents and what they need.
    Island,
    /// Agents on ropes pass messages.
    Agents,
    /// She waits for the first space's name.
    Create,
}

impl Story {
    /// Its height in the design box: the space's step leaves room for the form under it.
    pub(super) fn height(self) -> f32 {
        if self == Story::Create { 170.0 } else { DESIGN.y }
    }

    /// How long its loop takes, in seconds.
    fn cycle(self) -> f64 {
        match self {
            Story::Hello | Story::Agents | Story::Create => 6.0,
            Story::Notes => 7.0,
            Story::Canvas | Story::Island => 8.0,
        }
    }

    /// How long after the slide opens the bubble starts typing: the hello waits for her wave.
    fn delay(self) -> f64 {
        if self == Story::Hello { 1.2 } else { 0.35 }
    }
}

/// The part of `text` typed `seconds` after the bubble started, in whole characters.
pub(super) fn typed(text: &str, seconds: f64) -> &str {
    let count = (seconds.max(0.0) * TYPE_RATE) as usize;
    text.char_indices().nth(count).map_or(text, |(at, _)| &text[..at])
}

/// Where a loop of `cycle` seconds is `elapsed` seconds in: from 0 up to (not including) 1.
pub(super) fn phase(elapsed: f64, cycle: f64) -> f32 {
    ((elapsed.rem_euclid(cycle) / cycle) as f32).min(1.0 - f32::EPSILON)
}

/// How far `at` is from `start` to `end`, 0 to 1.
pub(super) fn progress(at: f32, start: f32, end: f32) -> f32 {
    ((at - start) / (end - start)).clamp(0.0, 1.0)
}

/// How much shows at `at` of a thing that comes in at `enter` and goes out at `leave` (phases of a loop).
pub(super) fn presence(at: f32, enter: f32, leave: f32) -> f32 {
    progress(at, enter, enter + FADE).min(1.0 - progress(at, leave, leave + FADE))
}

fn smooth(t: f32) -> f32 {
    t * t * (3.0 - 2.0 * t)
}

/// An ease out that overshoots a little and settles: things popping in.
fn pop(t: f32) -> f32 {
    let (back, rest) = (1.7, t - 1.0);
    1.0 + (back + 1.0) * rest.powi(3) + back * rest.powi(2)
}

/// Paints `story` in `stage` (the design box at `scale`), its loop `elapsed` seconds in, Luna saying `line`.
#[allow(clippy::too_many_arguments)]
pub(super) fn paint_story(painter: &Painter, story: Story, stage: Rect, scale: f32, k: &K, line: &str, time: f64, elapsed: f64) {
    let easel = Easel { painter, origin: stage.min, scale, k, time, elapsed, phase: phase(elapsed, story.cycle()), story, line };
    match story {
        Story::Hello => easel.hello(),
        Story::Canvas => easel.canvas(),
        Story::Notes => easel.notes(),
        Story::Island => easel.island(),
        Story::Agents => easel.agents(),
        Story::Create => easel.create(),
    }
}

/// What a scene is drawn with: the design box on screen, the theme and where its loop is.
struct Easel<'a> {
    painter: &'a Painter,
    origin: Pos2,
    scale: f32,
    k: &'a K,
    time: f64,
    elapsed: f64,
    phase: f32,
    story: Story,
    line: &'a str,
}

impl Easel<'_> {
    fn at(&self, x: f32, y: f32) -> Pos2 {
        self.origin + Vec2::new(x, y) * self.scale
    }

    /// A rect of the design box, `x` and `y` from `within`'s top left (none: the box's).
    fn place(&self, within: Option<Rect>, [x, y, width, height]: [f32; 4]) -> Rect {
        let origin = within.map_or(self.origin, |rect| rect.min);
        Rect::from_min_size(origin + Vec2::new(x, y) * self.scale, Vec2::new(width, height) * self.scale)
    }

    /// Whether the bubble is still typing: she talks meanwhile.
    fn typing(&self) -> bool {
        let seconds = self.elapsed - self.story.delay();
        seconds >= 0.0 && typed(self.line, seconds).len() < self.line.len()
    }

    /// Her state: talking while the bubble types, else `otherwise`.
    fn state(&self, otherwise: MagoState) -> MagoState {
        if self.typing() { MagoState::Speaking } else { otherwise }
    }

    /// Luna `size` tall at `center`, looking at `look` (none: around by herself); the pointer, when it is on her, can
    /// poke her.
    /// Each scene's Luna is her own (`id` and the story): in a change of slide the leaving one and the coming one
    /// show together.
    fn luna(&self, id: &str, center: Pos2, size: f32, state: MagoState, look: Option<Pos2>) {
        let size = size * self.scale;
        let pointer = self.painter.ctx().pointer_latest_pos().filter(|pointer| pointer.distance(center) < size * 0.45);
        let id = format!("{id}-{:?}", self.story);
        paint_mago_toward(self.painter, &id, center, size, state, self.time, pointer.or(look));
    }

    /// Her line in the bubble, `width` wide with its `align` point at `at`, the tail toward `toward`; it fades in, then
    /// types itself.
    fn bubble(&self, at: Pos2, align: egui::Align2, width: f32, toward: Pos2) {
        let seconds = self.elapsed - self.story.delay();
        let alpha = smooth(((seconds + 0.25) / 0.25).clamp(0.0, 1.0) as f32);
        if alpha <= 0.0 {
            return;
        }
        let pad = Vec2::new(14.0, 10.0) * self.scale;
        // the whole line is laid out (the rest unseen), so the words never jump to another line while it types
        let shown = typed(self.line, seconds).len();
        let font = FontId::proportional(14.0 * self.scale);
        let mut job = egui::text::LayoutJob::default();
        job.wrap.max_width = width * self.scale - 2.0 * pad.x;
        for (text, color) in [(&self.line[..shown], self.k.fg.gamma_multiply(alpha)), (&self.line[shown..], Color32::TRANSPARENT)] {
            if !text.is_empty() {
                job.append(text, 0.0, egui::TextFormat { font_id: font.clone(), color, line_height: Some(19.0 * self.scale), ..Default::default() });
            }
        }
        let galley = self.painter.layout_job(job);
        let rect = align.anchor_size(at, galley.size() + 2.0 * pad).translate(Vec2::new(0.0, (1.0 - alpha) * 6.0 * self.scale));
        parts::bubble(self.painter, rect, toward, self.scale, alpha, self.k);
        self.painter.galley(rect.min + pad, galley, self.k.fg);
    }

    /// Luna at the left of a scene, floating, looking at `look`, with her bubble above her.
    fn presenter(&self, state: MagoState, look: Option<Pos2>) {
        let bob = (self.elapsed * 1.7).sin() as f32 * 5.0;
        let center = self.at(104.0, 214.0 + bob);
        self.luna("welcome-luna", center, 100.0, self.state(state), look);
        self.bubble(self.at(6.0, 14.0), egui::Align2::LEFT_TOP, 206.0, center - Vec2::new(0.0, 40.0 * self.scale));
    }

    /// Gold sparkles twinkling at `stars` (x, y, size).
    fn sparkles(&self, stars: &[[f32; 3]]) {
        for (index, [x, y, size]) in stars.iter().enumerate() {
            let twinkle = 0.5 + 0.5 * (self.elapsed * 2.2 + index as f64 * 1.9).sin() as f32;
            let color = parts::gold(self.k).gamma_multiply(0.25 + 0.6 * twinkle);
            parts::star(self.painter, self.at(*x, *y), size * self.scale * (0.6 + 0.4 * twinkle), self.elapsed as f32 * 0.3 + index as f32, color);
        }
    }

    /// "Oi! Eu sou a Luna": she waves, then says hello, with sparkles around her.
    fn hello(&self) {
        self.sparkles(&[[150.0, 70.0, 9.0], [318.0, 52.0, 6.0], [128.0, 214.0, 6.0], [330.0, 236.0, 8.0], [612.0, 196.0, 6.0]]);
        let bob = (self.elapsed * 1.5).sin() as f32 * 4.0;
        let center = self.at(232.0, 150.0 + bob);
        let state = if self.typing() {
            MagoState::Speaking
        } else if self.phase < 0.25 {
            MagoState::Waving
        } else {
            MagoState::Idle
        };
        self.luna("welcome-luna", center, 150.0, state, None);
        self.bubble(self.at(318.0, 146.0), egui::Align2::LEFT_CENTER, 270.0, center + Vec2::new(50.0, -16.0) * self.scale);
    }

    /// The canvas: its grid panning slowly, two terminals, a chat and a file tree popping in one by one while her eyes
    /// follow each.
    fn canvas(&self) {
        const LEAVE: f32 = 0.9;
        let (phase, zoom) = (self.phase, self.scale);
        let turn = phase * TAU;
        let pan = Vec2::new(turn.sin() * 10.0, (turn.cos() - 1.0) * 5.0);
        let area = self.place(None, BOARD);
        let board = parts::board(self.painter, area, zoom, pan * zoom, self.k);
        let cards: [(f32, [f32; 4]); 4] =
            [(0.04, [20.0, 24.0, 168.0, 112.0]), (0.16, [20.0, 150.0, 168.0, 112.0]), (0.28, [204.0, 24.0, 132.0, 238.0]), (0.4, [352.0, 24.0, 118.0, 238.0])];
        let mut newest = None;
        for (index, (enter, at)) in cards.into_iter().enumerate() {
            let alpha = presence(phase, enter, LEAVE);
            if alpha <= 0.0 {
                continue;
            }
            let grow = 0.85 + 0.15 * pop(progress(phase, enter, enter + 0.07));
            let home = self.place(Some(area.translate(pan * zoom)), at);
            let rect = Rect::from_center_size(home.center(), home.size() * grow);
            let inside = progress(phase, enter + 0.03, enter + 0.2);
            match index {
                0 => {
                    let lines = ["$ claude", "Olá! O que vamos criar?", "> Faça a tela de login", "Lendo src/login.rs…"];
                    parts::terminal(&board, rect, zoom * grow, alpha, "Luna", &lines, inside * lines.len() as f32, PRIMARY, self.k.dark);
                }
                1 => {
                    let lines = ["$ codex", "Rodando os testes…", "12 passaram"];
                    parts::terminal(&board, rect, zoom * grow, alpha, "Front-end", &lines, inside * lines.len() as f32, parts::GREEN, self.k.dark);
                }
                2 => parts::chat(&board, rect, zoom * grow, alpha, inside, self.k),
                _ => parts::tree(&board, rect, zoom * grow, alpha, inside, self.k),
            }
            if phase >= enter && phase < LEAVE {
                newest = Some(rect.center());
            }
        }
        let state = if (0.52..0.6).contains(&phase) { MagoState::Done } else { MagoState::Idle };
        self.presenter(state, newest);
    }

    /// Notes: two drop in and stick, she writes a third (her pen line drawing in), a rope ties the first to a terminal
    /// and the note goes along it.
    fn notes(&self) {
        const LEAVE: f32 = 0.9;
        let (phase, zoom) = (self.phase, self.scale);
        let area = self.place(None, BOARD);
        let board = parts::board(self.painter, area, zoom, Vec2::ZERO, self.k);
        let shell = self.place(Some(area), [296.0, 150.0, 174.0, 112.0]);
        let read = (0.8..0.95).contains(&phase);
        let lines = ["$ claude", "Pronto para ajudar.", "$ magoagent note read", "> Plano da sprint", "  [x] API de login", "  [ ] Tela de cadastro"];
        let shown = if read { 2.0 + progress(phase, 0.8, 0.86) * 4.0 } else { 2.0 };
        parts::terminal(&board, shell, zoom, 1.0, "Back-end", &lines, shown, parts::GREEN, self.k.dark);
        // the notes drop from above and stick
        let sprint: [parts::NoteLine; 3] = [("API de login", Some(true)), ("Tela de cadastro", Some(false)), ("Testes", Some(false))];
        let ideas: [parts::NoteLine; 2] = [("Modo escuro", None), ("Atalhos", None)];
        let dropped: [(f32, usize, [f32; 4], &str, &[parts::NoteLine]); 3] = [
            (0.04, 0, [22.0, 22.0, 134.0, 104.0], "Plano da sprint", &sprint),
            (0.14, 1, [176.0, 30.0, 104.0, 82.0], "Ideias", &ideas),
            (0.24, 2, [22.0, 150.0, 150.0, 112.0], "Lembrete", &[]),
        ];
        let mut rects = Vec::new();
        for (enter, paper, at, title, lines) in dropped {
            let alpha = presence(phase, enter, LEAVE);
            let fall = progress(phase, enter, enter + 0.07);
            let rect = self.place(Some(area), at).translate(Vec2::new(0.0, -(1.0 - pop(fall)) * 36.0 * zoom));
            rects.push(rect);
            if alpha > 0.0 {
                let tape = progress(phase, enter + 0.06, enter + 0.09);
                parts::note(&board, rect, zoom, alpha, paper, title, lines, tape, self.k.dark);
            }
        }
        // her note: a pen line writes itself, a gold star at its tip
        let pen = progress(phase, 0.32, 0.56);
        let mut tip = None;
        if presence(phase, 0.24, LEAVE) > 0.0 && pen > 0.0 {
            let [ink_r, ink_g, ink_b] = NOTE_INK[2];
            let ink = Color32::from_rgb(ink_r, ink_g, ink_b).gamma_multiply(presence(phase, 0.24, LEAVE));
            let start = rects[2].left_top() + Vec2::new(10.0, 44.0) * zoom;
            for word in parts::handwriting(start, zoom, pen) {
                tip = word.last().copied().or(tip);
                board.add(Shape::line(word, Stroke::new((1.2 * zoom).max(1.0), ink)));
            }
            if pen < 1.0 {
                if let Some(tip) = tip {
                    parts::star(&board, tip, 5.0 * zoom, self.elapsed as f32 * 4.0, parts::gold(self.k));
                }
            } else {
                tip = None;
            }
        }
        // the rope: from the plan to the terminal, then the note goes along it
        let rope = parts::rope_path(rects[0].right_center() + Vec2::new(0.0, 18.0 * zoom), shell.left_center(), 22.0 * zoom);
        let reach = progress(phase, 0.6, 0.7);
        let rope_alpha = 1.0 - progress(phase, LEAVE, LEAVE + FADE);
        let carrying = progress(phase, 0.72, 0.8);
        parts::rope(&board, &rope, reach, zoom, rope_alpha, carrying > 0.0 && carrying < 1.0, self.k);
        let mut dot = None;
        if carrying > 0.0 && carrying < 1.0 {
            let at = parts::along(&rope, carrying);
            parts::pulse(&board, at, zoom, 1.0);
            dot = Some(at);
        }
        let state = if pen > 0.0 && pen < 1.0 {
            MagoState::Working
        } else if (0.8..0.88).contains(&phase) {
            MagoState::Done
        } else {
            MagoState::Idle
        };
        let look = tip.or(dot).or_else(|| (reach > 0.0 && reach < 1.0).then(|| parts::along(&rope, reach)));
        self.presenter(state, look);
    }

    /// The island: a notch at the top of a little screen with Luna in it; it opens over the windows with the agents'
    /// pills (one working, one waiting on you, one done), then closes.
    fn island(&self) {
        let (phase, zoom) = (self.phase, self.scale);
        let screen = self.place(None, [16.0, 8.0, 480.0, 284.0]);
        let inside = parts::screen(self.painter, screen, zoom, self.k);
        let open = smooth(progress(phase, 0.14, 0.24)) * (1.0 - smooth(progress(phase, 0.78, 0.88)));
        let size = Vec2::new(120.0 + 214.0 * open, 30.0 + 160.0 * open) * zoom;
        let notch = Rect::from_min_size(Pos2::new(screen.center().x - size.x / 2.0, screen.top()), size);
        parts::notch(&inside, notch, (15.0 + 7.0 * open) * zoom);
        let finished = phase >= 0.56;
        let agents = [
            ("Front-end", "Escrevendo os testes", Color32::from_rgb(0xd9, 0x77, 0x57)),
            ("Back-end", "Pede para rodar o deploy", Color32::from_rgb(0x2d, 0xd4, 0xbf)),
            ("Revisor", if finished { "Revisão pronta" } else { "Revisando o login" }, Color32::from_rgb(0x60, 0xa5, 0xfa)),
        ];
        let statuses = [Status::Working, Status::NeedsYou, if finished { Status::Done } else { Status::Working }];
        // closed: one mini wizard per agent at the right
        if open < 0.3 {
            for (index, (_, _, color)) in agents.iter().enumerate() {
                let at = Pos2::new(notch.right() - (16.0 + index as f32 * 14.0) * zoom, notch.center().y + 1.0 * zoom);
                paint_mini(&inside, &format!("welcome-mini-{index}"), at, 10.0 * zoom, *color, MagoState::Idle, self.time);
            }
        }
        // open: the header and a pill per agent
        let shows = smooth(progress(open, 0.6, 1.0));
        if shows > 0.0 {
            let header = notch.top() + 25.0 * zoom;
            let waiting = phase < 0.78;
            parts::island_header(&inside, Pos2::new(notch.left() + 50.0 * zoom, header), Pos2::new(notch.right() - 16.0 * zoom, header), zoom, shows, waiting);
            for (index, (agent, status)) in agents.into_iter().zip(statuses).enumerate() {
                let enter = 0.26 + index as f32 * 0.06;
                let grow = pop(progress(phase, enter, enter + 0.06)) * shows;
                let row = Rect::from_min_size(
                    Pos2::new(notch.left() + 10.0 * zoom, notch.top() + (46.0 + index as f32 * 46.0) * zoom),
                    Vec2::new(size.x - 20.0 * zoom, 38.0 * zoom),
                );
                parts::island_row(&inside, row, zoom, grow, &format!("welcome-agent-{index}"), agent, status, self.time);
            }
        }
        // Luna: in the closed notch at its left, in the open one's header
        let ease = smooth(open);
        let center = Pos2::new(notch.left() + (24.0 + 5.0 * ease) * zoom, notch.top() + (15.0 + 10.0 * ease) * zoom);
        let state = if (0.56..0.66).contains(&phase) {
            MagoState::Done
        } else if (0.4..0.54).contains(&phase) {
            MagoState::NeedsYou
        } else {
            MagoState::Idle
        };
        self.luna("welcome-luna-island", center, 22.0 + 8.0 * ease, self.state(state), None);
        self.bubble(self.at(512.0, 24.0), egui::Align2::LEFT_TOP, 206.0, Pos2::new(notch.right(), center.y));
    }

    /// Agents on ropes: the Maestro asks the two others, each answers when done, a blue dot running along the rope with
    /// every message; her eyes follow the dots.
    fn agents(&self) {
        const CLEAR: f32 = 0.94;
        let (phase, zoom) = (self.phase, self.scale);
        let area = self.place(None, BOARD);
        let board = parts::board(self.painter, area, zoom, Vec2::ZERO, self.k);
        let maestro = self.place(Some(area), [160.0, 18.0, 170.0, 104.0]);
        let front = self.place(Some(area), [20.0, 168.0, 170.0, 98.0]);
        let back = self.place(Some(area), [300.0, 168.0, 170.0, 98.0]);
        let ropes = [
            parts::rope_path(maestro.left_bottom() + Vec2::new(28.0 * zoom, 0.0), Pos2::new(front.center().x, front.top()), 14.0 * zoom),
            parts::rope_path(maestro.right_bottom() - Vec2::new(28.0 * zoom, 0.0), Pos2::new(back.center().x, back.top()), 14.0 * zoom),
        ];
        // (rope, from the Maestro's end, leaves, arrives, who gets it (0 Maestro, 1 Front-end, 2 Back-end), what it says)
        let messages = [
            (0, true, 0.06, 0.22, 1, "> Luna: crie a tela"),
            (1, true, 0.16, 0.32, 2, "> Luna: faça a API"),
            (1, false, 0.5, 0.66, 0, "> Back-end: API pronta"),
            (0, false, 0.64, 0.8, 0, "> Front-end: tela pronta"),
        ];
        let mut dot = None;
        for (index, path) in ropes.iter().enumerate() {
            let lit = messages.iter().any(|(rope, _, leaves, arrives, _, _)| *rope == index && (*leaves..*arrives).contains(&phase));
            parts::rope(&board, path, 1.0, zoom, 1.0, lit, self.k);
        }
        for (rope, outward, leaves, arrives, _, _) in messages {
            let run = progress(phase, leaves, arrives);
            if run > 0.0 && run < 1.0 {
                let at = parts::along(&ropes[rope], if outward { run } else { 1.0 - run });
                parts::pulse(&board, at, zoom, 1.0);
                dot = Some(at);
            }
        }
        // each card: its first lines, while it works (blue dot) and what it writes then
        let cards: [(Rect, &str, Vec<&str>, (f32, f32), Option<(f32, &str)>); 3] = [
            (maestro, "Luna", vec!["$ magoagent list", "Agentes conectados:", "  - Front-end", "  - Back-end"], (0.06, 0.8), None),
            (front, "Front-end", vec!["$ claude"], (0.22, 0.64), Some((0.3, "Escrevendo login.tsx…"))),
            (back, "Back-end", vec!["$ codex"], (0.32, 0.5), Some((0.38, "Criando /api/login…"))),
        ];
        for (index, (rect, name, mut lines, (busy_from, busy_until), work)) in cards.into_iter().enumerate() {
            // what came to it along the ropes and what it did, until the loop starts over
            if phase < CLEAR {
                for (_, _, _, arrives, to, text) in messages {
                    if to == index && phase >= arrives {
                        lines.push(text);
                    }
                }
                if let Some((_, text)) = work.filter(|(from, _)| phase >= *from) {
                    lines.push(text);
                }
            }
            let dot = if (busy_from..busy_until).contains(&phase) { PRIMARY } else { parts::GREEN };
            let count = lines.len() as f32;
            parts::terminal(&board, rect, zoom, 1.0, name, &lines, count, dot, self.k.dark);
        }
        let state = if (0.8..0.9).contains(&phase) { MagoState::Done } else { MagoState::Idle };
        self.presenter(state, dot);
    }

    /// The space's step: she waits, looking down at the form.
    fn create(&self) {
        self.sparkles(&[[180.0, 34.0, 7.0], [330.0, 150.0, 6.0], [560.0, 26.0, 6.0]]);
        let bob = (self.elapsed * 1.5).sin() as f32 * 4.0;
        let center = self.at(248.0, 90.0 + bob);
        let state = self.state(MagoState::Listening);
        let look = (!self.typing()).then(|| center + Vec2::new(0.0, 320.0 * self.scale));
        self.luna("welcome-luna", center, 112.0, state, look);
        self.bubble(self.at(322.0, 84.0), egui::Align2::LEFT_CENTER, 250.0, center + Vec2::new(40.0, -14.0) * self.scale);
    }
}
