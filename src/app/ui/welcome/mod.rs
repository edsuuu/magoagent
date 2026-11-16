//! The welcome (Boas-vindas): slides over the whole window, once after an install or an update, presented by Luna, the
//! app's assistant: each slide one of her little scenes (scenes.rs, drawn with parts.rs) over a title and a line, the
//! dots under them. With no space yet (a first run) the last step asks for the first one: the app has nothing to open
//! before it.

mod parts;
mod scenes;

use super::settings::kit::{self, K};
use crate::prelude::*;
use scenes::{DESIGN, Story, paint_story};

/// A slide: Luna's scene and what she says in it, its title and its line ('\n' breaks it).
struct Slide {
    story: Story,
    luna: &'static str,
    title: &'static str,
    subtitle: &'static str,
    /// The tints of the gradient behind it.
    tints: [[u8; 3]; 3],
}

const SLIDES: [Slide; 6] = [
    Slide {
        story: Story::Hello,
        luna: "Oi! Eu sou a Luna, a assistente do MagoAgent.",
        title: "Boas-vindas ao MagoAgent",
        subtitle: "Um quadro branco onde agentes de código trabalham juntos.\nTerminais, chats, navegador e arquivos lado a lado, num canvas infinito.",
        tints: [[0x8b, 0x5c, 0xf6], [0x38, 0xbd, 0xf8], [0xf4, 0x72, 0xb6]],
    },
    Slide {
        story: Story::Canvas,
        luna: "Este é o canvas, um quadro sem fim. Terminais, chats e arquivos ficam lado a lado.",
        title: "Abra terminais. Faça anotações. Esboce ideias.",
        subtitle: "Cada agente, arquivo e pensamento tem o seu lugar no quadro.\nOrganize como preferir.",
        tints: [[0x22, 0xc5, 0x5e], [0x38, 0xbd, 0xf8], [0xfa, 0xcc, 0x15]],
    },
    Slide {
        story: Story::Notes,
        luna: "Eu anoto tudo! Ligue uma nota a um terminal com uma corda e o agente lê o que está nela.",
        title: "Notas que os agentes leem.",
        subtitle: "Notas adesivas guardam planos, listas e lembretes.\nLigue uma nota a um terminal com uma corda e o agente lê o que está nela.",
        tints: [[0xfa, 0xcc, 0x15], [0xf4, 0x72, 0xb6], [0x38, 0xbd, 0xf8]],
    },
    Slide {
        story: Story::Island,
        luna: "Eu moro na ilha, por cima de todas as janelas. Quando um agente precisa de você, eu aviso!",
        title: "A ilha.",
        subtitle: "Uma pílula no topo da tela, por cima de todas as janelas.\nMostra o que cada agente está fazendo e o que precisa de você.",
        tints: [[0x38, 0xbd, 0xf8], [0x8b, 0x5c, 0xf6], [0x14, 0xb8, 0xa6]],
    },
    Slide {
        story: Story::Agents,
        luna: "Pelas cordas, um agente pede e o outro responde. Eu fico de olho em todos.",
        title: "Agentes que conversam entre si.",
        subtitle: "Ligue terminais e notas com cordas: os agentes colaboram,\ntrocam dados e escrevem resultados, tudo em tempo real.",
        tints: [[0xf9, 0x73, 0x16], [0xf4, 0x72, 0xb6], [0x8b, 0x5c, 0xf6]],
    },
    Slide {
        story: Story::Create,
        luna: "Agora é com você: dê um nome ao espaço e escolha a pasta do projeto.",
        title: "Crie o seu primeiro espaço",
        subtitle: "Cada espaço é um projeto: um quadro e uma pasta raiz.\nTerminais e árvores novos dele começam nessa pasta.",
        tints: [[0x14, 0xb8, 0xa6], [0x22, 0xc5, 0x5e], [0x38, 0xbd, 0xf8]],
    },
];
/// The slide that asks for the first space: shown only while there is none.
const CREATE: usize = 5;
/// The slide with the note on the agents' CLIs under its line.
const AGENTS: usize = 4;
const DISCLAIMER: &str = "O MagoAgent usa os CLIs de agentes (Claude Code, Codex) já instalados e com assinatura ativa ou chave de API.";

#[derive(Clone, Debug, Default)]
pub(crate) struct Welcome {
    pub(crate) slide: usize,
    pub(crate) name: String,
    pub(crate) folder: String,
    /// Why the space was not created, under the button.
    pub(crate) error: String,
    /// The slide on view and when it opened: its scene and Luna's bubble start over from there.
    opened: Option<(usize, f64)>,
}

/// How many slides there are: the space's only while there is none.
fn count(no_space: bool) -> usize {
    CREATE + usize::from(no_space)
}

impl DrawingApp {
    /// Opens the welcome: from the start the first time, right on the space's step when only that is missing.
    pub(crate) fn start_welcome(&mut self) {
        let slide = if self.prefs.welcome_seen { CREATE } else { 0 };
        self.welcome = Some(Welcome { slide, ..Default::default() });
        self.chat_open = false;
    }

    /// The last step: the space asked for is created and opened, or the slides just close.
    fn finish_welcome(&mut self, mut welcome: Welcome) {
        if self.spaces.list.is_empty() {
            let created = self.create_space(&welcome.name, &welcome.folder).and_then(|id| self.switch_space(&id));
            if let Err(error) = created {
                welcome.error = error;
                self.welcome = Some(welcome);
                return;
            }
            self.prefs.folders.remember(&welcome.folder);
        }
        self.prefs.welcome_seen = true;
        self.chat_open = self.prefs.start_in_chat;
    }

    pub(crate) fn welcome_ui(&mut self, ctx: &Context) {
        let Some(mut welcome) = self.welcome.take() else { return };
        let no_space = self.spaces.list.is_empty();
        let slides = count(no_space);
        welcome.slide = welcome.slide.min(slides - 1);
        let k = K::new(self.dark());
        let c = self.colors();
        let time = ctx.input(|i| i.time);
        let screen = ctx.content_rect();
        let slide = &SLIDES[welcome.slide];
        let opened = match welcome.opened {
            Some((shown, at)) if shown == welcome.slide => at,
            _ => welcome.opened.insert((welcome.slide, time)).1,
        };
        let elapsed = (time - opened).max(0.0);
        let last = welcome.slide + 1 == slides;
        let (mut go, mut advance) = (None, false);
        let folders = &mut self.prefs.folders;
        let id = egui::Id::unique("welcome");
        egui::Area::new(id).order(egui::Order::Foreground).fixed_pos(screen.min).movable(false).show(ctx, |ui| {
            // it takes every click: nothing under it answers
            let (rect, _) = ui.allocate_exact_size(screen.size(), Sense::click_and_drag());
            let painter = ui.painter().clone();
            painter.rect_filled(rect, 0, k.bg);
            paint_gradient(&painter, rect, k.bg, &slide.tints, if k.dark { 0.22 } else { 0.3 }, time);
            // Luna's scene, then the words under it, the whole in the middle of the window
            let scale = (rect.height() * 0.4 / DESIGN.y).clamp(0.55, 1.3).min((rect.width() - 48.0) / DESIGN.x);
            let art = slide.story.height() * scale;
            let form = if welcome.slide == CREATE { 150.0 } else { 0.0 };
            let note = if welcome.slide == AGENTS { 60.0 } else { 0.0 };
            let block = art + 150.0 + form + note;
            let top = rect.center().y - block / 2.0 - 20.0;
            let stage = Rect::from_center_size(Pos2::new(rect.center().x, top + art / 2.0), Vec2::new(DESIGN.x, slide.story.height()) * scale);
            paint_story(&painter, slide.story, stage, scale, &k, slide.luna, time, elapsed);
            let mut y = top + art + 28.0;
            painter.text(Pos2::new(rect.center().x, y), egui::Align2::CENTER_TOP, slide.title, kit::semibold(26.0), k.fg);
            y += 44.0;
            for line in slide.subtitle.lines() {
                painter.text(Pos2::new(rect.center().x, y), egui::Align2::CENTER_TOP, line, FontId::proportional(15.0), k.muted_fg);
                y += 22.0;
            }
            if welcome.slide == AGENTS {
                y += 10.0;
                let note = kit::galley(ui, DISCLAIMER, 11.5, 16.0, k.muted_fg, 400.0);
                let card = Rect::from_center_size(Pos2::new(rect.center().x, y + note.size().y / 2.0 + 8.0), note.size() + Vec2::new(24.0, 16.0));
                painter.rect_stroke(card, 8, Stroke::new(1.0, k.line(1.0)), StrokeKind::Inside);
                painter.galley(card.min + Vec2::new(12.0, 8.0), note, k.muted_fg);
                y = card.bottom();
            }
            if welcome.slide == CREATE {
                y += 18.0;
                let fields = Rect::from_min_size(Pos2::new(rect.center().x - DIALOG_WIDTH / 2.0, y), Vec2::new(DIALOG_WIDTH, form));
                ui.scope_builder(egui::UiBuilder::new().max_rect(fields), |ui| {
                    ui.spacing_mut().item_spacing = Vec2::ZERO;
                    let name = dialog_field(ui, "Nome", &mut welcome.name, "Projeto", DIALOG_WIDTH, &c);
                    if welcome.name.is_empty() && !ui.memory(|m| m.focused().is_some()) {
                        name.request_focus();
                    }
                    ui.add_space(16.0);
                    dialog_folder(ui, "Pasta raiz", &mut welcome.folder, "~/projetos/loja", "Pasta raiz do espaço", &c, folders);
                });
                y += form - 20.0;
            }
            // the button: Continuar, Começar at the end, or the space's
            let label = match (last, welcome.slide == CREATE) {
                (_, true) => "Criar meu primeiro espaço",
                (true, false) => "Começar",
                _ => "Continuar",
            };
            let text = painter.layout_no_wrap(label.into(), kit::semibold(14.0), k.bg);
            let button = Rect::from_center_size(Pos2::new(rect.center().x, y + 36.0 + 20.0), Vec2::new(text.size().x + 64.0, 40.0));
            let response = ui.interact(button, id.with("next"), Sense::click()).on_hover_cursor(CursorIcon::PointingHand);
            response.widget_info(|| egui::WidgetInfo::labeled(egui::Role::Button, true, label));
            painter.rect_filled(button, 20, k.fg.gamma_multiply(if response.hovered() { 0.85 } else { 0.72 }));
            painter.galley(button.center() - text.size() / 2.0, text, k.bg);
            advance |= response.clicked();
            if !welcome.error.is_empty() {
                painter.text(
                    Pos2::new(rect.center().x, button.bottom() + 10.0),
                    egui::Align2::CENTER_TOP,
                    &welcome.error,
                    FontId::proportional(12.5),
                    c.danger,
                );
            }
            // the dots: one per slide, the one on view darker
            let dots = Pos2::new(rect.center().x - (slides as f32 - 1.0) * 14.0, rect.bottom() - 28.0);
            for index in 0..slides {
                let at = dots + Vec2::new(index as f32 * 28.0, 0.0);
                let dot = ui
                    .interact(Rect::from_center_size(at, Vec2::splat(28.0)), id.with(("dot", index)), Sense::click())
                    .on_hover_cursor(CursorIcon::PointingHand);
                let alpha = if index == welcome.slide {
                    0.8
                } else if dot.hovered() {
                    0.3
                } else {
                    0.15
                };
                painter.circle_filled(at, 3.5, k.fg.gamma_multiply(alpha));
                if dot.clicked() {
                    go = Some(index);
                }
            }
        });
        ctx.move_to_top(egui::LayerId::new(egui::Order::Foreground, id));
        ctx.request_repaint();
        // the arrows walk the slides (not inside a field), Enter goes on
        let typing = ctx.egui_wants_keyboard_input();
        let (left, right, enter) = ctx.input(|i| (i.key_pressed(Key::ArrowLeft), i.key_pressed(Key::ArrowRight), i.key_pressed(Key::Enter)));
        if !typing && left {
            go = welcome.slide.checked_sub(1);
        }
        if !typing && right {
            go = Some(welcome.slide + 1).filter(|next| *next < slides);
        }
        advance |= enter;
        if let Some(index) = go {
            (welcome.slide, welcome.error) = (index, String::new());
        } else if advance && last {
            return self.finish_welcome(welcome);
        } else if advance {
            welcome.slide += 1;
        }
        self.welcome = Some(welcome);
    }
}

/// A soft mesh gradient: a 3×3 grid of points over `rect`, the corners and the middle tinted by turns with
/// `tints` at `strength` over `ground`, drifting slowly with `time`.
fn paint_gradient(painter: &Painter, rect: Rect, ground: Color32, tints: &[[u8; 3]; 3], strength: f32, time: f64) {
    let mut mesh = egui::Mesh::default();
    for row in 0..3 {
        for column in 0..3 {
            let index = row * 3 + column;
            let phase = (time * 0.25 + index as f64 * 0.7).sin() as f32;
            let [r, g, b] = tints[index % 3];
            let weight = strength * (0.55 + 0.45 * phase) * if index == 4 { 0.5 } else { 1.0 };
            let color = ground.lerp_to_gamma(Color32::from_rgb(r, g, b), weight.max(0.0));
            let at = Pos2::new(rect.left() + rect.width() * column as f32 / 2.0, rect.top() + rect.height() * row as f32 / 2.0);
            mesh.colored_vertex(at, color);
        }
    }
    for row in 0..2u32 {
        for column in 0..2u32 {
            let corner = row * 3 + column;
            mesh.add_triangle(corner, corner + 1, corner + 4);
            mesh.add_triangle(corner, corner + 4, corner + 3);
        }
    }
    // the bottom fades back to the ground, under the button and the dots
    let fade = Rect::from_min_max(Pos2::new(rect.left(), rect.bottom() - rect.height() * 0.35), rect.max);
    painter.add(Shape::mesh(mesh));
    let mut veil = egui::Mesh::default();
    veil.colored_vertex(fade.left_top(), Color32::TRANSPARENT);
    veil.colored_vertex(fade.right_top(), Color32::TRANSPARENT);
    veil.colored_vertex(fade.right_bottom(), ground.gamma_multiply(0.9));
    veil.colored_vertex(fade.left_bottom(), ground.gamma_multiply(0.9));
    veil.add_triangle(0, 1, 2);
    veil.add_triangle(0, 2, 3);
    painter.add(Shape::mesh(veil));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_space_step_shows_only_without_a_space() {
        assert_eq!(count(false), 5);
        assert_eq!(count(true), 6);
        assert_eq!(SLIDES[CREATE].title, "Crie o seu primeiro espaço");
        assert_eq!(SLIDES[CREATE].story, Story::Create);
        assert_eq!(SLIDES[AGENTS].title, "Agentes que conversam entre si.");
    }

    #[test]
    fn luna_presents_every_slide() {
        // one scene each, she says hello first, and every line of hers is short enough for her bubble
        assert_eq!(SLIDES[0].story, Story::Hello);
        assert_eq!(SLIDES[0].luna, "Oi! Eu sou a Luna, a assistente do MagoAgent.");
        for (index, slide) in SLIDES.iter().enumerate() {
            assert!(SLIDES[..index].iter().all(|other| other.story != slide.story), "{:?} twice", slide.story);
            assert!((20..=100).contains(&slide.luna.chars().count()), "{}", slide.luna);
        }
    }

    #[test]
    fn the_bubble_types_whole_characters() {
        let line = "Oi! Eu sou a Luna, ação.";
        assert_eq!(scenes::typed(line, -1.0), "");
        assert_eq!(scenes::typed(line, 0.0), "");
        // 34 a second: 3 characters at 0.1 s
        assert_eq!(scenes::typed(line, 0.1), "Oi!");
        // never inside a character of more than one byte
        let through = scenes::typed(line, 21.5 / 34.0);
        assert_eq!(through, "Oi! Eu sou a Luna, aç");
        assert_eq!(scenes::typed(line, 60.0), line);
        assert_eq!(scenes::typed("", 1.0), "");
    }

    #[test]
    fn the_scenes_loop() {
        assert_eq!(scenes::phase(0.0, 8.0), 0.0);
        assert_eq!(scenes::phase(4.0, 8.0), 0.5);
        // a loop later it is back where it was, and it never reaches 1
        assert!((scenes::phase(13.0, 8.0) - scenes::phase(5.0, 8.0)).abs() < 1e-6);
        assert!(scenes::phase(8.0 - 1e-9, 8.0) < 1.0);
        assert!((0.0..1.0).contains(&scenes::phase(-1.0, 8.0)));
        // a thing in the loop: out before it comes in, whole between, out again after it leaves
        assert!((scenes::progress(0.5, 0.25, 0.75) - 0.5).abs() < 1e-6);
        assert_eq!(scenes::progress(0.1, 0.25, 0.75), 0.0);
        assert_eq!(scenes::progress(0.9, 0.25, 0.75), 1.0);
        assert_eq!(scenes::presence(0.0, 0.1, 0.9), 0.0);
        assert_eq!(scenes::presence(0.5, 0.1, 0.9), 1.0);
        assert!(scenes::presence(0.12, 0.1, 0.9) > 0.0 && scenes::presence(0.12, 0.1, 0.9) < 1.0);
        assert_eq!(scenes::presence(0.99, 0.1, 0.9), 0.0);
    }

    #[test]
    fn the_welcome_creates_the_first_space() {
        let folder = std::env::temp_dir().join(format!("magoagent-welcome-{}", std::process::id()));
        let _ = fs::remove_dir_all(&folder);
        fs::create_dir_all(&folder).unwrap();
        let scene = folder.join("autosave.mago");
        // a first run: no drawing, no spaces
        assert!(crate::app::workspaces::read_spaces(&scene).0.list.is_empty());
        let mut app = DrawingApp::default();
        app.autosave = Some(scene.clone());
        app.load_spaces();
        app.start_welcome();
        assert_eq!(app.welcome.as_ref().map(|w| w.slide), Some(0));
        // no name: it stays, saying why
        app.finish_welcome(app.welcome.clone().unwrap());
        assert!(app.welcome.as_ref().is_some_and(|w| !w.error.is_empty()));
        let welcome = Welcome { name: "Loja".into(), ..app.welcome.take().unwrap() };
        app.finish_welcome(welcome);
        assert!(app.welcome.is_none() && app.prefs.welcome_seen);
        assert_eq!(app.spaces.list.iter().map(|w| w.name.as_str()).collect::<Vec<_>>(), ["Loja"]);
        assert_eq!(app.spaces.current, app.spaces.list[0].id);
        // seen once: only the space's step comes back, if the spaces go
        app.start_welcome();
        assert_eq!(app.welcome.as_ref().map(|w| w.slide), Some(CREATE));
        let _ = fs::remove_dir_all(&folder);
    }
}
