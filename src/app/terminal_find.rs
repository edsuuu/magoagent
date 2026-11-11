//! "Buscar no Terminal": the bar over a terminal's top-right corner (⌘F, Ctrl+Shift+F elsewhere)
//! that finds text in its screen and scrollback with the grid's own regex search, marks the
//! matches in the grid and steps through them.

use crate::app::ui::menu_look::{MenuIcon, paint_menu_icon_sized};
use crate::prelude::*;

/// The search of one terminal.
pub(crate) struct TermFind {
    pub(crate) id: String,
    pub(crate) query: String,
    /// The query is a regex, not the text it says (the MCP's; the bar searches text).
    pub(crate) regex: bool,
    pub(crate) found: Found,
    /// The match the bar is on.
    pub(crate) current: Option<usize>,
    /// What the last search ran on: the query, and the shell's output at the time.
    ran: (String, bool, u64),
    ran_at: f64,
    /// The field takes the keys on the next frame.
    focus: bool,
}

impl TermFind {
    /// "2 de 5", "Nenhum resultado", or nothing before there is a query.
    pub(crate) fn summary(&self) -> String {
        match (self.query.is_empty(), self.found.matches.len(), self.current) {
            (true, ..) => String::new(),
            (false, 0, _) => "Nenhum resultado".into(),
            (false, total, current) => format!("{} de {total}", current.map_or(0, |c| c + 1)),
        }
    }
}

/// The shortcut that opens it, as the bar's tip says it.
pub(crate) const FIND_KEYS: &str = if cfg!(target_os = "macos") { "⌘F" } else { "Ctrl+Shift+F" };

impl DrawingApp {
    /// Opens the bar on terminal `id`, with `seed` in it if given (what was selected, say).
    pub(crate) fn open_find(&mut self, id: &str, seed: Option<String>) {
        match &mut self.term_ui.find {
            Some(f) if f.id == id => f.focus = true,
            _ => {
                self.term_ui.find = Some(TermFind {
                    id: id.into(),
                    query: String::new(),
                    regex: false,
                    found: Found::default(),
                    current: None,
                    ran: Default::default(),
                    ran_at: 0.0,
                    focus: true,
                })
            }
        }
        if let (Some(f), Some(seed)) = (&mut self.term_ui.find, seed.filter(|s| !s.is_empty() && !s.contains('\n'))) {
            f.query = seed;
        }
        // the keys go to the bar's field while it is open
        self.terminal_focus = Some(id.into());
    }

    pub(crate) fn close_find(&mut self) {
        self.term_ui.find = None;
    }

    /// Runs the search again when the query, or what the shell printed, changed.
    pub(crate) fn refresh_find(&mut self) {
        let now = self.now;
        let Some(f) = &mut self.term_ui.find else { return };
        let Some(Ok(session)) = self.terminals.get(&f.id) else { return };
        let stamp = (f.query.clone(), f.regex, session.handle.output_stamp());
        // new output searches again at most five times a second: a scrollback is a lot to read
        if f.ran == stamp || (f.ran.0 == stamp.0 && f.ran.1 == stamp.1 && now - f.ran_at < 0.2) {
            return;
        }
        let (was, kept) = (f.current.and_then(|c| f.found.matches.get(c).cloned()), f.ran.0 == stamp.0 && f.ran.1 == stamp.1);
        f.found = session.handle.find_all(&f.query, f.regex);
        // the same match stays current as lines come in; a new query starts at the newest one
        f.current = match (kept.then_some(was).flatten(), f.found.matches.len()) {
            (_, 0) => None,
            (Some(old), _) => f.found.matches.iter().position(|m| *m == old).or(Some(f.found.matches.len() - 1)),
            (None, n) => Some(n - 1),
        };
        f.ran = stamp;
        f.ran_at = now;
        if !kept && let Some(m) = f.current.and_then(|c| f.found.matches.get(c)) {
            session.handle.reveal(m);
        }
    }

    /// Goes to the next (`forward`) or the previous match, round; the screen follows.
    pub(crate) fn find_step(&mut self, forward: bool) {
        self.refresh_find();
        let Some(f) = &mut self.term_ui.find else { return };
        let n = f.found.matches.len();
        if n == 0 {
            return;
        }
        let at = f.current.map_or(if forward { 0 } else { n - 1 }, |c| if forward { (c + 1) % n } else { (c + n - 1) % n });
        f.current = Some(at);
        if let Some(Ok(session)) = self.terminals.get(&f.id) {
            session.handle.reveal(&f.found.matches[at]);
        }
    }

    /// The visible matches of terminal `id` by screen row, for painting its grid.
    pub(crate) fn find_marks(&self, id: &str) -> Vec<Vec<(usize, usize, bool)>> {
        match (&self.term_ui.find, self.terminals.get(id)) {
            (Some(f), Some(Ok(session))) if f.id == id => session.handle.marks(&f.found.matches, f.current),
            _ => Vec::new(),
        }
    }

    /// The bar, 8 and 38 points in from the card's top-right corner.
    pub(crate) fn find_ui(&mut self, ctx: &Context, card: Rect, zoom: f32) {
        let Some(id) = self.term_ui.find.as_ref().map(|f| f.id.clone()) else { return };
        if self.terminals.get(&id).is_none_or(|s| s.is_err()) {
            return;
        }
        self.refresh_find();
        let dark = self.dark();
        let ink = glass_ink(dark);
        let (mut step, mut close) = (None, false);
        let Some(f) = self.term_ui.find.as_mut() else { return };
        let summary = f.summary();
        let any = !f.found.matches.is_empty();
        let focus = std::mem::take(&mut f.focus);
        let width = 10.0 * 2.0 + 11.0 + 6.0 + 150.0 + 6.0 + 76.0 + 6.0 + 1.0 + 3.0 * (18.0 + 6.0);
        let at = Pos2::new(card.right() - 8.125 * zoom - width, card.top() + 38.0 * zoom);
        egui::Area::new(egui::Id::unique("terminal_find")).fixed_pos(at).order(egui::Order::Foreground).show(ctx, |ui| {
            let (rect, _) = ui.allocate_exact_size(Vec2::new(width, 28.0), Sense::hover());
            paint_glass(ui.painter(), rect, 14.0, dark, false);
            ui.painter().rect_stroke(rect, 14.0, Stroke::new(0.5, ink.gamma_multiply(0.12)), StrokeKind::Inside);
            let inside = rect.shrink2(Vec2::new(10.0, 4.0));
            let row = egui::UiBuilder::new().max_rect(inside).layout(egui::Layout::left_to_right(egui::Align::Center));
            ui.scope_builder(row, |ui| {
                ui.spacing_mut().item_spacing.x = 6.0;
                let (icon, _) = ui.allocate_exact_size(Vec2::splat(11.0), Sense::hover());
                paint_menu_icon_sized(ui.painter(), MenuIcon::Search, icon.center(), ink.gamma_multiply(0.6), 11.0);
                let field = egui::TextEdit::singleline(&mut f.query)
                    .frame(egui::Frame::NONE)
                    .hint_text("Buscar")
                    .font(FontId::proportional(12.0))
                    .text_color(ink)
                    .desired_width(150.0)
                    .id(egui::Id::unique("terminal_find_field"));
                let response = ui.add(field);
                if focus {
                    response.request_focus();
                }
                // Enter is the next match, Shift+Enter the one before, Esc closes the bar
                if response.lost_focus() && ui.input(|i| i.key_pressed(Key::Enter)) {
                    step = Some(!ui.input(|i| i.modifiers.shift));
                    response.request_focus();
                }
                if response.has_focus() && ui.input(|i| i.key_pressed(Key::Escape)) {
                    close = true;
                }
                let (counter, _) = ui.allocate_exact_size(Vec2::new(76.0, 18.0), Sense::hover());
                ui.painter().text(counter.right_center(), egui::Align2::RIGHT_CENTER, &summary, FontId::proportional(11.0), ink.gamma_multiply(0.6));
                let (line, _) = ui.allocate_exact_size(Vec2::new(1.0, 14.0), Sense::hover());
                ui.painter().vline(line.center().x, line.y_range(), Stroke::new(1.0, ink.gamma_multiply(0.2)));
                for (arrow, tip, on) in [(false, "Ocorrência anterior", any), (true, "Próxima ocorrência", any)] {
                    let (b, r) = ui.allocate_exact_size(Vec2::splat(18.0), Sense::click());
                    let c = b.center();
                    let (a, z) = if arrow { (-2.0, 2.0) } else { (2.0, -2.0) };
                    let s = Stroke::new(1.5, ink.gamma_multiply(if on { 1.0 } else { 0.4 }));
                    ui.painter().add(Shape::line(vec![c + Vec2::new(-3.5, a), c + Vec2::new(0.0, z), c + Vec2::new(3.5, a)], s));
                    if r.on_hover_cursor(CursorIcon::PointingHand).tip(tip).clicked() && on {
                        step = Some(arrow);
                    }
                }
                let (b, r) = ui.allocate_exact_size(Vec2::splat(18.0), Sense::click());
                let s = Stroke::new(1.5, ink.gamma_multiply(0.6));
                let c = b.center();
                ui.painter().add(Shape::line(vec![c + Vec2::new(-3.0, -3.0), c + Vec2::new(3.0, 3.0)], s));
                ui.painter().add(Shape::line(vec![c + Vec2::new(-3.0, 3.0), c + Vec2::new(3.0, -3.0)], s));
                if r.on_hover_cursor(CursorIcon::PointingHand).tip("Fechar a barra de busca").clicked() {
                    close = true;
                }
            });
        });
        if close {
            self.close_find();
        } else if let Some(forward) = step {
            self.find_step(forward);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn app_with_output(text: &str) -> (DrawingApp, String) {
        let mut app = DrawingApp::default();
        let mut e = app.new_element(ElementKind::Rectangle { x: 0.0, y: 0.0, width: 640.0, height: 400.0 });
        e.terminal = Some(TerminalSpec::default());
        let id = e.id.clone();
        app.scene.elements.push(e);
        let line = format!("echo {text}");
        let (program, args): (&str, Vec<&str>) = if cfg!(windows) { ("cmd", vec!["/c", &line]) } else { ("sh", vec!["-c", &line]) };
        let session = TerminalSession::spawn("", "", (60, 6), None, vec![], Some((program, &args)), 100).unwrap();
        for _ in 0..100 {
            if session.handle.screen_text().contains(text) {
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(50));
        }
        app.terminals.insert(id.clone(), Ok(session));
        (app, id)
    }

    #[test]
    fn the_bar_finds_steps_round_and_closes() {
        let (mut app, id) = app_with_output("lamp post, lamp shade, LAMP");
        app.open_find(&id, None);
        assert_eq!(app.terminal_focus.as_deref(), Some(id.as_str()));
        app.term_ui.find.as_mut().unwrap().query = "lamp".into();
        app.refresh_find();
        let f = app.term_ui.find.as_ref().unwrap();
        assert_eq!(f.found.matches.len(), 3, "smart case: a lower case query ignores the case");
        assert_eq!(f.summary(), "3 de 3", "a new search is on the newest match");
        app.find_step(true);
        assert_eq!(app.term_ui.find.as_ref().unwrap().summary(), "1 de 3", "round");
        app.find_step(false);
        assert_eq!(app.term_ui.find.as_ref().unwrap().summary(), "3 de 3");
        let marks = app.find_marks(&id);
        assert_eq!(marks.iter().flatten().filter(|(.., current)| *current).count(), 1);
        assert_eq!(marks.iter().flatten().count(), 3);
        app.term_ui.find.as_mut().unwrap().query = "nothing like it".into();
        app.refresh_find();
        assert_eq!(app.term_ui.find.as_ref().unwrap().summary(), "Nenhum resultado");
        app.close_find();
        assert!(app.term_ui.find.is_none() && app.find_marks(&id).is_empty());
    }

    #[test]
    fn a_selected_word_seeds_the_bar_but_not_a_line_break() {
        let (mut app, id) = app_with_output("seed word");
        app.open_find(&id, Some("two\nlines".into()));
        assert_eq!(app.term_ui.find.as_ref().unwrap().query, "");
        app.open_find(&id, Some("seed".into()));
        assert_eq!(app.term_ui.find.as_ref().unwrap().query, "seed");
    }

    #[test]
    fn the_bar_draws_over_the_card() {
        let (mut app, id) = app_with_output("drawn");
        app.open_find(&id, Some("drawn".into()));
        let ctx = Context::default();
        ctx.run_ui(Default::default(), |_| app.find_ui(&ctx, Rect::from_min_size(Pos2::new(50.0, 50.0), Vec2::new(640.0, 400.0)), 1.0)).textures_delta.clear();
        assert!(app.term_ui.find.is_some());
    }
}
