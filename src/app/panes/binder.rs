//! Fichário: the binder's notes (maestro/binder.rs, `fichario/` beside the drawing) as a pile, the
//! top one shown with arrows to the others, "2 de 5"; "Nova nota" writes one, "Pôr no canvas" makes
//! a sticky note of the one shown.

use super::live;
use crate::app::maestro::binder;
use crate::prelude::*;

/// How often the folder is read again while the panel is open, in seconds (the agents write in it).
const REREAD: f64 = 3.0;
const SHEET_HEIGHT: f32 = 230.0;

#[derive(Default)]
pub(crate) struct BinderPane {
    /// The notes as last read: name and text.
    pub(crate) notes: Vec<(String, String)>,
    pub(crate) read_at: Option<f64>,
    /// The note on top.
    pub(crate) page: usize,
    pub(crate) draft: String,
}

/// The note's name in a draft: its first line with words, without Markdown's marks.
pub(crate) fn draft_name(text: &str) -> String {
    let line = text.lines().map(|l| l.trim().trim_start_matches(['#', '-', '>', '*', ' '])).find(|l| !l.is_empty()).unwrap_or_default();
    line.chars().take(60).collect()
}

impl DrawingApp {
    fn binder_notes_dir(&self) -> Option<PathBuf> {
        self.autosave.as_ref().map(|p| p.with_file_name("fichario"))
    }

    /// The notes, read again when `now` is `REREAD` past the last reading (or never read).
    pub(crate) fn read_binder(&mut self, now: f64) {
        if self.panes.binder.read_at.is_some_and(|at| now - at < REREAD) {
            return;
        }
        let Some(dir) = self.binder_notes_dir() else { return };
        let notes = binder::list(&dir).into_iter().map(|(name, _, _)| {
            let text = binder::read(&dir, &name).unwrap_or_default();
            (name, text)
        });
        let pane = &mut self.panes.binder;
        pane.notes = notes.collect();
        pane.read_at = Some(now);
        pane.page = pane.page.min(pane.notes.len().saturating_sub(1));
    }

    pub(crate) fn binder_pane(&mut self, ui: &mut egui::Ui) {
        let colors = self.colors();
        self.read_binder(ui.input(|i| i.time));
        let count = self.panes.binder.notes.len();
        if count == 0 {
            ui.label(egui::RichText::new("O fichário está vazio.").color(colors.muted));
        } else {
            let page = self.panes.binder.page.min(count - 1);
            let (name, text) = self.panes.binder.notes[page].clone();
            let (sheet, _) = ui.allocate_exact_size(Vec2::new(ui.available_width(), SHEET_HEIGHT), Sense::hover());
            let sheet = sheet.shrink2(Vec2::new(8.0, 4.0));
            let [r, g, b, _] = STICKY_PAPER;
            let paper = Color32::from_rgb(r, g, b);
            let p = ui.painter();
            // the pages behind it peek out, darker
            for (behind, offset) in [(2, Vec2::new(-5.0, 6.0)), (1, Vec2::new(6.0, 4.0))] {
                if count > behind {
                    p.rect_filled(sheet.translate(offset), 4, paper.lerp_to_gamma(Color32::BLACK, 0.08 * behind as f32));
                }
            }
            p.rect_filled(sheet, 4, paper);
            let ink = Color32::from_rgb(0x1b, 0x1b, 0x1f);
            let inner = sheet.shrink(16.0);
            let body = text.lines().skip_while(|l| l.trim().is_empty()).skip(1).collect::<Vec<_>>().join("\n");
            let title = p.layout(draft_name(&text).chars().take(80).collect::<String>(), FontId::proportional(14.0), ink, inner.width());
            let title_height = title.size().y;
            let clipped = p.with_clip_rect(inner);
            clipped.galley(inner.min, title, ink);
            let body = p.layout(body.trim().to_string(), FontId::proportional(14.0), ink, inner.width());
            clipped.galley(inner.min + Vec2::new(0.0, title_height + 6.0), body, ink);
            ui.horizontal(|ui| {
                let previous = ui.add_enabled(count > 1, egui::Button::new("‹").min_size(Vec2::new(28.0, 24.0))).on_hover_text("Nota anterior").clicked();
                let label = format!("{} de {count}", page + 1);
                let width = ui.available_width() - 28.0 - ui.spacing().item_spacing.x;
                ui.add_sized(Vec2::new(width, 24.0), egui::Label::new(egui::RichText::new(label).color(colors.muted)));
                let next = ui.add_enabled(count > 1, egui::Button::new("›").min_size(Vec2::new(28.0, 24.0))).on_hover_text("Próxima nota").clicked();
                if previous && live(ui) {
                    self.binder_turn(-1);
                }
                if next && live(ui) {
                    self.binder_turn(1);
                }
            });
            if ui.button("Pôr no canvas").on_hover_text(format!("Uma nota adesiva com {name}")).clicked() && live(ui) {
                self.binder_to_canvas(&text);
            }
        }
        let draft = egui::TextEdit::multiline(&mut self.panes.binder.draft)
            .hint_text("Escreva uma nota; a primeira linha é o nome")
            .background_color(colors.surface_low);
        ui.add_sized(Vec2::new(ui.available_width(), 64.0), draft);
        if ui.add_enabled(!self.panes.binder.draft.trim().is_empty(), egui::Button::new("Nova nota")).clicked() && live(ui) {
            self.binder_new_note();
        }
    }

    /// The arrows: the next (`1`) or the previous (`-1`) note on top, round the pile.
    pub(crate) fn binder_turn(&mut self, step: isize) {
        let pane = &mut self.panes.binder;
        let count = pane.notes.len() as isize;
        if count > 0 {
            pane.page = (pane.page as isize + step).rem_euclid(count) as usize;
        }
    }

    /// "Nova nota": the draft into the binder under its first line's name (a name taken gets a
    /// number), and on top.
    pub(crate) fn binder_new_note(&mut self) {
        let text = self.panes.binder.draft.trim().to_string();
        let Some(dir) = self.binder_notes_dir() else {
            self.toast("Sem pasta de dados: o fichário não tem onde ficar.");
            return;
        };
        let base = draft_name(&text);
        let taken = |name: &str| binder::note_file(name).is_some_and(|file| dir.join(file).exists());
        let name = (1..).map(|n| if n == 1 { base.clone() } else { format!("{base} {n}") }).find(|name| !taken(name)).unwrap_or(base);
        match binder::write(&dir, &name, &text, false) {
            Ok(_) => {
                self.panes.binder.draft.clear();
                self.panes.binder.read_at = None;
                self.read_binder(0.0);
                let stem = binder::note_file(&name).unwrap_or_default().trim_end_matches(".md").to_string();
                self.panes.binder.page = self.panes.binder.notes.iter().position(|(n, _)| *n == stem).unwrap_or(0);
                self.panes.binder.read_at = None;
            }
            Err(why) => self.toast(why),
        }
    }

    /// "Pôr no canvas": a sticky note with the note's text, in the middle of what the canvas shows.
    pub(crate) fn binder_to_canvas(&mut self, text: &str) {
        let middle = if self.canvas_rect.is_positive() { self.to_world(self.canvas_rect.center()) } else { Pos2::ZERO };
        let [note, mut label] = self.new_note(Rect::from_center_size(middle, NOTE_SIZE), text.trim_end().to_string());
        label.style.font_size = 20.0;
        self.snapshot();
        self.scene.elements.extend([note, label]);
        self.selected = vec![self.scene.elements.len() - 2];
        self.toast("Nota posta no canvas.");
    }
}
