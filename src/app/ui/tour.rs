//! A apresentação de primeiro uso: depois das boas-vindas (welcome/), uma vez, a tela escurece com um recorte em
//! volta de cada parte do app (a barra de ferramentas, o botão do chat, a lateral, os cantos do canvas, Ajustes) e um
//! balão diz o que ela faz, com os pontinhos do passo, Voltar, Próximo e Pular. Os alvos são os retângulos que o egui
//! registrou ao pintar cada widget neste quadro (ou no anterior), achados pelo id: um alvo fora da tela deixa o passo
//! no meio, sem recorte. Vista até o fim ou pulada, não volta sozinha (`Prefs.tour_seen`); Ajustes › Geral a abre de novo.

use super::settings::kit::{self, K};
use crate::prelude::*;

/// A part of the app a step points at.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) enum Spot {
    Toolbar,
    Chat,
    Sidebar,
    Panes,
    Corners,
    Settings,
}

pub(crate) struct Step {
    pub(crate) spot: Option<Spot>,
    pub(crate) title: &'static str,
    pub(crate) text: &'static str,
}

pub(crate) const STEPS: [Step; 7] = [
    Step {
        spot: None,
        title: "Boas-vindas ao MagoAgent",
        text: "Um quadro onde ficam os seus terminais, chats e agentes. Em poucos passos você vê onde fica cada coisa.",
    },
    Step {
        spot: Some(Spot::Toolbar),
        title: "Ferramentas",
        text: "Seleção (V), Mão (H) e, para criar: Terminal ou chat (C), Árvore de arquivos (G), Navegador (W) e Nota (N). O último botão abre as ferramentas de desenho.",
    },
    Step { spot: Some(Spot::Chat), title: "Chat", text: "Abre o chat em tela cheia, fora do canvas. Clique de novo para voltar ao quadro." },
    Step {
        spot: Some(Spot::Sidebar),
        title: "Espaços de trabalho",
        text: "Cada espaço é um projeto, com o seu quadro e a sua pasta. O botão ☰ dobra e abre esta lateral.",
    },
    Step { spot: Some(Spot::Panes), title: "Painéis", text: "Alterações abre ao lado do quadro. O número no botão diz quantos arquivos mudaram." },
    Step {
        spot: Some(Spot::Corners), title: "Navegar no canvas", text: "Andares, minimapa e zoom ficam neste canto. O uso dos planos fica no pé da lateral."
    },
    Step {
        spot: Some(Spot::Settings),
        title: "Ajustes",
        text: "Tema, atalhos, agentes, voz e integrações. Esta apresentação volta em Ajustes › Geral. Pronto, é só começar.",
    },
];

/// The balloon's width, the hole's margin around its target and how long the hole takes from one target to the next.
const TIP_WIDTH: f32 = 320.0;
const PAD: f32 = 6.0;
const MARGIN: f32 = 16.0;
const GAP: f32 = 18.0;
const SLIDE: f64 = 0.3;
const RADIUS: f32 = 12.0;

#[derive(Clone, Debug, Default)]
pub(crate) struct Tour {
    pub(crate) step: usize,
    /// The hole drawn when the step changed, and when: the next one slides from there.
    moved: Option<(Option<Rect>, f64)>,
    /// The hole of the last frame.
    pub(crate) hole: Option<Rect>,
    /// The chat was open when the tour began: it comes back at the end.
    back_to_chat: bool,
}

impl Tour {
    /// Goes to `step` (`time`: now), the hole sliding from where it was.
    pub(crate) fn go(&mut self, step: usize, time: f64) {
        if step != self.step && step < STEPS.len() {
            self.moved = Some((self.hole, time));
            self.step = step;
        }
    }

    /// The hole at `time` around `target` (already padded): sliding from the last step's for `SLIDE` seconds, then on it.
    pub(crate) fn hole_at(&self, target: Option<Rect>, time: f64) -> Option<Rect> {
        let target = target?;
        let Some((Some(from), at)) = self.moved else { return Some(target) };
        let linear = ((time - at) / SLIDE).clamp(0.0, 1.0) as f32;
        let eased = 1.0 - (1.0 - linear).powi(3);

        Some(Rect::from_min_max(from.min.lerp(target.min, eased), from.max.lerp(target.max, eased)))
    }
}

/// Where `spot` is on screen now, by the ids its widgets were painted with (toolbar.rs, titlebar.rs, sidebar.rs, panes/mod.rs); an
/// area's rect is that of its "move" widget. `None` when none of them was painted or it is off the screen.
pub(crate) fn spot_rect(ctx: &Context, spot: Spot) -> Option<Rect> {
    let widget = |name: &str| ctx.read_response(egui::Id::unique(name)).map(|response| response.rect);
    let area = |name: &str| ctx.read_response(egui::Id::unique(name).with("move")).map(|response| response.rect);

    let found = match spot {
        Spot::Toolbar => area("tools"),
        Spot::Chat => widget("top_chat"),
        Spot::Sidebar => area("sidebar"),
        Spot::Panes => (0..4)
            .filter_map(|index| ctx.read_response(egui::Id::unique(("pane_button", index))).map(|response| response.rect))
            .reduce(|one, other| one.union(other)),
        Spot::Corners => ["footer", "minimap", "floors_pill"].into_iter().filter_map(area).reduce(|one, other| one.union(other)),
        Spot::Settings => widget("sidebar_settings").or_else(|| widget("top_settings")),
    };

    let screen = ctx.content_rect();

    found.map(|rect| rect.intersect(screen)).filter(|rect| rect.is_positive() && rect.width() >= 4.0 && rect.height() >= 4.0)
}

/// Where the balloon of `size` goes: in the middle of the screen, only the hole moving from step to step; when the middle
/// would cover the hole, under it if it fits, else over it, else beside it.
pub(crate) fn tip_place(hole: Option<Rect>, size: Vec2, screen: Rect) -> Rect {
    let middle = Rect::from_center_size(screen.center(), size);
    let Some(hole) = hole.filter(|hole| hole.intersects(middle.expand(GAP))) else { return middle };
    let inside = screen.shrink(MARGIN);
    let x = (hole.center().x - size.x / 2.0).clamp(inside.left(), (inside.right() - size.x).max(inside.left()));
    let y = (hole.center().y - size.y / 2.0).clamp(inside.top(), (inside.bottom() - size.y).max(inside.top()));

    let at = if hole.bottom() + GAP + size.y <= inside.bottom() {
        Pos2::new(x, hole.bottom() + GAP)
    } else if hole.top() - GAP - size.y >= inside.top() {
        Pos2::new(x, hole.top() - GAP - size.y)
    } else if hole.right() + GAP + size.x <= inside.right() {
        Pos2::new(hole.right() + GAP, y)
    } else {
        Pos2::new((hole.left() - GAP - size.x).max(inside.left()), y)
    };

    Rect::from_min_size(at, size)
}

/// The veil over `screen` with a hole of rounded corners: four bands around the hole and, in each of its corners, the
/// part of the square outside the arc.
fn paint_veil(painter: &Painter, screen: Rect, hole: Option<Rect>, color: Color32) {
    let Some(hole) = hole else {
        painter.rect_filled(screen, 0, color);

        return;
    };

    let mut mesh = egui::Mesh::default();

    for band in [
        Rect::from_min_max(screen.min, Pos2::new(screen.right(), hole.top())),
        Rect::from_min_max(Pos2::new(screen.left(), hole.bottom()), screen.max),
        Rect::from_min_max(Pos2::new(screen.left(), hole.top()), Pos2::new(hole.left(), hole.bottom())),
        Rect::from_min_max(Pos2::new(hole.right(), hole.top()), Pos2::new(screen.right(), hole.bottom())),
    ] {
        if band.is_positive() {
            mesh.add_colored_rect(band, color);
        }
    }

    let radius = RADIUS.min(hole.width() / 2.0).min(hole.height() / 2.0);
    let corners = [(hole.left_top(), 1.0, 1.0), (hole.right_top(), -1.0, 1.0), (hole.right_bottom(), -1.0, -1.0), (hole.left_bottom(), 1.0, -1.0)];

    for (corner, sx, sy) in corners {
        let center = corner + Vec2::new(sx * radius, sy * radius);
        let first = mesh.vertices.len() as u32;

        mesh.colored_vertex(corner, color);

        for index in 0..=8 {
            let angle = index as f32 / 8.0 * std::f32::consts::FRAC_PI_2;

            mesh.colored_vertex(center - Vec2::new(sx * radius * angle.cos(), sy * radius * angle.sin()), color);

            if index > 0 {
                mesh.add_triangle(first, first + index, first + index + 1);
            }
        }
    }

    painter.add(Shape::mesh(mesh));
}

impl DrawingApp {
    /// Opens the tour at `step`, over the canvas (the chat and Ajustes close; the chat comes back at the end).
    pub(crate) fn start_tour_at(&mut self, step: usize) {
        let back_to_chat = self.tour.as_ref().map_or(self.chat_open, |tour| tour.back_to_chat);

        self.tour = Some(Tour { step: step.min(STEPS.len() - 1), back_to_chat, ..Default::default() });
        self.chat_open = false;
        self.settings_open = false;
    }

    pub(crate) fn start_tour(&mut self) {
        self.start_tour_at(0);
    }

    /// Concluir, Pular or Esc: seen, it does not come back by itself.
    pub(crate) fn end_tour(&mut self) {
        if let Some(tour) = self.tour.take() {
            self.prefs.tour_seen = true;
            self.chat_open = tour.back_to_chat;
        }
    }

    /// When the app opens: the tour asked by a test scene (`asked`, MAGOAGENT_TOUR), else once, after the welcome and
    /// never under a roteiro.
    pub(crate) fn tour_on_open(&mut self, asked: Option<usize>) {
        if let Some(step) = asked {
            self.start_tour_at(step);
        } else if self.script.is_none() && !self.prefs.tour_seen && self.welcome.is_none() {
            self.start_tour();
        }
    }

    pub(crate) fn tour_ui(&mut self, ctx: &Context) {
        if self.welcome.is_some() {
            return;
        }

        let Some(mut tour) = self.tour.take() else { return };
        let k = K::new(self.dark());
        let time = ctx.input(|i| i.time);
        let screen = ctx.content_rect();
        let last = STEPS.len() - 1;
        let step = &STEPS[tour.step];
        let target = step.spot.and_then(|spot| spot_rect(ctx, spot)).map(|rect| rect.expand(PAD).intersect(screen));
        let hole = tour.hole_at(target, time);

        tour.hole = hole;

        if tour.moved.is_some_and(|(_, at)| time - at < SLIDE) {
            ctx.request_repaint();
        }

        let (mut back, mut next, mut skip) = (false, false, false);
        let id = egui::Id::unique("tour");

        egui::Area::new(id).order(egui::Order::Foreground).fixed_pos(screen.min).movable(false).show(ctx, |ui| {
            // it takes every click: nothing under it answers
            let (rect, _) = ui.allocate_exact_size(screen.size(), Sense::click_and_drag());
            let painter = ui.painter().clone();

            paint_veil(&painter, rect, hole, Color32::from_black_alpha(153));

            if let Some(hole) = hole {
                painter.rect_stroke(hole, RADIUS, Stroke::new(2.0, k.primary), StrokeKind::Inside);
            }

            let inner = TIP_WIDTH - 32.0;
            let title = painter.layout(step.title.into(), kit::semibold(16.0), k.fg, inner);
            let text = kit::galley(ui, step.text, 14.0, 20.0, k.muted_fg, inner);
            let size = Vec2::new(TIP_WIDTH, 16.0 + title.size().y + 6.0 + text.size().y + 14.0 + 30.0 + 16.0);
            let tip = tip_place(hole, size, rect);
            let shadow = egui::epaint::Shadow { offset: [0, 20], blur: 50, spread: 0, color: Color32::from_black_alpha(100) };

            painter.add(shadow.as_shape(tip, 14));
            painter.rect_filled(tip, 14, k.bg);
            painter.rect_stroke(tip, 14, Stroke::new(1.0, k.border), StrokeKind::Inside);
            let mut y = tip.top() + 16.0;

            painter.galley(Pos2::new(tip.left() + 16.0, y), title.clone(), k.fg);
            y += title.size().y + 6.0;
            painter.galley(Pos2::new(tip.left() + 16.0, y), text.clone(), k.muted_fg);
            let row = tip.bottom() - 16.0 - 15.0;
            // the dots: the step on view wider, in the accent
            let mut x = tip.left() + 16.0;

            for index in 0..STEPS.len() {
                let width = if index == tour.step { 16.0 } else { 6.0 };
                let dot = Rect::from_min_size(Pos2::new(x, row - 3.0), Vec2::new(width, 6.0));

                painter.rect_filled(dot, 3, if index == tour.step { k.primary } else { k.soft_fg.gamma_multiply(0.45) });
                x += width + 4.0;
            }

            // right to left: Próximo (Começar, Concluir), Voltar, Pular
            let label = match tour.step {
                0 => "Começar",
                step if step == last => "Concluir",
                _ => "Próximo",
            };

            let mut right = tip.right() - 16.0;

            let mut button = |ui: &mut egui::Ui, text: &str, fill: Option<Color32>, ink: Color32| {
                let galley = painter.layout_no_wrap(text.into(), kit::semibold(12.5), ink);
                let width = galley.size().x + if fill.is_some() { 24.0 } else { 8.0 };
                let rect = Rect::from_min_max(Pos2::new(right - width, row - 15.0), Pos2::new(right, row + 15.0));

                right = rect.left() - 8.0;
                let response = ui.interact(rect, id.with(text), Sense::click()).on_hover_cursor(CursorIcon::PointingHand);

                response.widget_info(|| egui::WidgetInfo::labeled(egui::Role::Button, true, text));

                if let Some(fill) = fill {
                    painter.rect_filled(rect, 8, if response.hovered() { fill.gamma_multiply(0.85) } else { fill });
                }

                let ink = if fill.is_none() && response.hovered() { k.fg } else { ink };

                painter.galley(rect.center() - galley.size() / 2.0, galley, ink);
                response.clicked()
            };

            next = button(ui, label, Some(k.primary), Color32::WHITE);

            if tour.step > 0 {
                back = button(ui, "Voltar", Some(k.button), k.fg);
            }

            if tour.step < last {
                skip = button(ui, "Pular", None, k.muted_fg);
            }
        });

        ctx.move_to_top(egui::LayerId::new(egui::Order::Foreground, id));
        super::titlebar::raise_window_buttons(ctx, egui::LayerId::new(egui::Order::Foreground, id));

        // the keys: Esc skips, the arrows walk, Enter goes on
        let (escape, left, right, enter) = ctx.input_mut(|i| {
            let mut take = |key| i.consume_key(Modifiers::NONE, key);

            (take(Key::Escape), take(Key::ArrowLeft), take(Key::ArrowRight), take(Key::Enter))
        });

        next |= right || enter;
        back |= left;
        skip |= escape;
        self.tour = Some(tour);
        self.tour_input(back, next, skip, time);
    }

    /// What the balloon's buttons (or keys) asked, at `time`.
    pub(crate) fn tour_input(&mut self, back: bool, next: bool, skip: bool, time: f64) {
        let Some(tour) = self.tour.as_mut() else { return };

        if skip || (next && tour.step + 1 == STEPS.len()) {
            self.end_tour();
        } else if next {
            tour.go(tour.step + 1, time);
        } else if back && tour.step > 0 {
            tour.go(tour.step - 1, time);
        }
    }
}
