//! A pile of sticky notes (a card index): the notes sit on one another, the front page shows and
//! the others wait behind it. They share the group of the pile, so the pile moves, copies and
//! deletes as one; the hidden pages are "card text" of the front one (`is_card_text`).

use crate::app::ui::context::{Entry, item};
use crate::prelude::*;

/// Where the pages of an unstacked pile spill, one step each (24 px, the reference app's).
const SPILL: f32 = 24.0;

impl DrawingApp {
    /// The cards of the pile `i` belongs to, in scene order (just `i` when it is on no pile).
    pub(crate) fn stack_pages(&self, i: usize) -> Vec<usize> {
        let Some(e) = self.scene.elements.get(i) else { return Vec::new() };
        let group = e.group_id.as_ref().filter(|_| e.note.stacked);
        let pages: Vec<usize> = (0..self.scene.elements.len())
            .filter(|j| {
                let o = &self.scene.elements[*j];
                o.created.is_some() && o.note.stacked && group.is_some() && o.group_id.as_ref() == group
            })
            .collect();
        if pages.is_empty() { vec![i] } else { pages }
    }

    /// Moves a note card and its text so the card's corner is at `to`.
    fn tuck_note(&mut self, card: usize, to: Pos2) {
        let delta = to - element_bounds(&self.scene.elements[card]).min;
        for j in [Some(card), note_label(&self.scene.elements, card)].into_iter().flatten() {
            self.scene.elements[j].move_by(delta);
        }
    }

    /// Sets the group of a note card and of its text.
    fn regroup_note(&mut self, card: usize, group: &str, stacked: bool, front: bool) {
        for j in [Some(card), note_label(&self.scene.elements, card)].into_iter().flatten() {
            self.scene.elements[j].group_id = Some(group.to_string());
        }
        self.scene.elements[card].note.stacked = stacked;
        self.scene.elements[card].note.front = front;
    }

    /// The selected notes that are on no pile yet.
    pub(crate) fn loose_selected_notes(&self) -> Vec<usize> {
        let mut notes: Vec<usize> =
            self.selected.iter().copied().filter(|i| self.scene.elements.get(*i).is_some_and(|e| e.created.is_some() && !e.note.stacked)).collect();
        notes.sort_unstable();
        notes.dedup();
        notes
    }

    /// Arquivar notas: the selected notes become one pile at the first one's place (reading order:
    /// rows 12 px apart count as one row).
    pub(crate) fn stack_notes(&mut self) -> bool {
        let mut notes = self.loose_selected_notes();
        if notes.len() < 2 {
            return false;
        }
        notes.sort_by(|a, b| {
            let (a, b) = (element_bounds(&self.scene.elements[*a]).min, element_bounds(&self.scene.elements[*b]).min);
            if (a.y - b.y).abs() > 12.0 { a.y.total_cmp(&b.y) } else { a.x.total_cmp(&b.x) }
        });
        self.snapshot();
        let origin = element_bounds(&self.scene.elements[notes[0]]).min;
        let group = new_id();
        for (k, card) in notes.iter().enumerate() {
            self.tuck_note(*card, origin);
            self.regroup_note(*card, &group, true, k == 0);
        }
        self.selected = vec![notes[0]];
        true
    }

    /// Desempilhar: the pages spill out one step apart and are loose notes again.
    pub(crate) fn unstack(&mut self, i: usize) -> bool {
        if !self.scene.elements.get(i).is_some_and(|e| e.note.stacked) {
            return false;
        }
        self.snapshot();
        let pages = self.stack_pages(i);
        let origin = element_bounds(&self.scene.elements[i]).min;
        for (k, card) in pages.iter().enumerate() {
            self.tuck_note(*card, origin + Vec2::splat(SPILL * k as f32));
            self.regroup_note(*card, &new_id(), false, false);
        }
        self.selected = pages;
        true
    }

    /// The pile's front page moves `delta` pages on (wrapping) and takes the selection.
    pub(crate) fn flip_stack(&mut self, i: usize, delta: i32) -> bool {
        let pages = self.stack_pages(i);
        let Some(at) = pages.iter().position(|p| self.scene.elements[*p].note.front) else { return false };
        if pages.len() < 2 {
            return false;
        }
        let next = pages[(at as i32 + delta).rem_euclid(pages.len() as i32) as usize];
        self.snapshot();
        self.scene.elements[pages[at]].note.front = false;
        self.scene.elements[next].note.front = true;
        self.editing = None;
        self.selected = vec![next];
        true
    }

    /// A lone note dropped with its center on a pile's front page joins it, behind that page.
    pub(crate) fn drop_into_stack(&mut self) {
        let [card] = self.selected[..] else { return };
        if !self.scene.elements.get(card).is_some_and(|e| e.created.is_some() && !e.note.stacked) {
            return;
        }
        let center = element_bounds(&self.scene.elements[card]).center();
        let target = (0..self.scene.elements.len()).rev().find(|j| {
            let o = &self.scene.elements[*j];
            *j != card && o.note.stacked && o.note.front && element_bounds(o).contains(center)
        });
        let Some(target) = target else { return };
        let group = self.scene.elements[target].group_id.clone().unwrap_or_else(new_id);
        self.tuck_note(card, element_bounds(&self.scene.elements[target]).min);
        self.regroup_note(card, &group, true, false);
        self.selected = vec![target];
    }

    /// "2/3" over a pile's front page.
    pub(crate) fn paint_stack_badge(&self, painter: &Painter, frame: Rect, i: usize) {
        let pages = self.stack_pages(i);
        let Some(at) = pages.iter().position(|p| *p == i) else { return };
        if pages.len() < 2 {
            return;
        }
        let rect = Rect::from_min_size(frame.right_top() + Vec2::new(-46.0, 6.0), Vec2::new(40.0, 18.0));
        painter.rect_filled(rect, 9, Color32::from_black_alpha(110));
        painter.text(rect.center(), egui::Align2::CENTER_CENTER, format!("{}/{}", at + 1, pages.len()), FontId::proportional(11.0), Color32::WHITE);
    }

    /// The menu rows of a pile's front page.
    pub(crate) fn stack_rows(&self, i: usize) -> Vec<Entry> {
        if !self.scene.elements.get(i).is_some_and(|e| e.note.stacked) {
            return Vec::new();
        }
        vec![item("stack_next", "Próxima página"), item("stack_prev", "Página anterior"), item("unstack", "Desempilhar páginas")]
    }

    /// The MCP's `note_stack`: stack the named notes, or flip / spill the pile of the first.
    pub(crate) fn act_note_stack(&mut self, action: &str, names: &[String]) -> Result<String, String> {
        let mut cards = Vec::new();
        for name in names {
            let i = self.node_named(name)?;
            if self.scene.elements[i].created.is_none() {
                return Err(format!("{name} não é uma nota"));
            }
            cards.push(i);
        }
        let first = *cards.first().ok_or("diga a nota")?;
        let done = match action {
            "stack" => {
                self.selected = cards;
                self.stack_notes()
            }
            "unstack" => self.unstack(first),
            "next" => self.flip_stack(first, 1),
            "prev" => self.flip_stack(first, -1),
            other => return Err(format!("ação desconhecida: {other}")),
        };
        if done { Ok("Feito.".into()) } else { Err("nada a fazer: empilhar pede 2 ou mais notas soltas; as outras ações, uma nota de uma pilha".into()) }
    }
}

/// A page waiting behind its pile's front: neither it nor its text is painted or hit.
pub(crate) fn hidden_page(elements: &[Element], e: &Element) -> bool {
    let waits = |c: &Element| c.created.is_some() && c.note.stacked && !c.note.front;
    waits(e) || note_of_label(elements, e).is_some_and(|n| waits(&elements[n]))
}
