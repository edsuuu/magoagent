//! A pile of sticky notes (a card index): the notes sit on one another, the front page shows and
//! the others wait behind it. They share the group of the pile, so the pile moves, copies and
//! deletes as one; the hidden pages are "card text" of the front one (`is_card_text`).

use crate::app::ui::context::{Entry, item};
use crate::prelude::*;

/// Where the pages of an unstacked pile spill, one step each (24 px, the reference app's).
const SPILL: f32 = 24.0;
/// The tab rail: tabs this wide out of the page's right edge, tucked 2 px under it, 64 to 120 tall
/// and 10 over each other, from 10 below the top (the reference app's).
const TAB_W: f32 = 23.0;
const TAB_TUCK: f32 = 2.0;
const TAB_MIN_H: f32 = 64.0;
const TAB_MAX_H: f32 = 120.0;
const TAB_OVERLAP: f32 = 10.0;
const RAIL_INSET: f32 = 10.0;

/// The tabs of a pile of `n` pages whose front page sits in `frame` on screen.
pub(crate) fn tab_rects(frame: Rect, n: usize, zoom: f32) -> Vec<Rect> {
    let avail = frame.height() / zoom - 2.0 * RAIL_INSET;
    // ponytail: past what fits, tabs shrink under 64 instead of scrolling like the reference's rail
    let fit = (avail + TAB_OVERLAP * (n as f32 - 1.0)) / n.max(1) as f32;
    let h = fit.min(TAB_MAX_H).min(avail.max(TAB_MIN_H));
    (0..n)
        .map(|k| {
            let top = frame.top() + (RAIL_INSET + k as f32 * (h - TAB_OVERLAP)) * zoom;
            Rect::from_min_size(Pos2::new(frame.right() - TAB_TUCK * zoom, top), Vec2::new(TAB_W + TAB_TUCK, h) * zoom)
        })
        .collect()
}

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

    /// The front page of a pile in the selection, if any.
    fn selected_stack(&self) -> Option<usize> {
        let page = self.selected.iter().copied().find(|i| self.scene.elements.get(*i).is_some_and(|e| e.note.stacked))?;
        self.stack_pages(page).into_iter().find(|p| self.scene.elements[*p].note.front)
    }

    /// Colocar no Fichário: the selected loose notes become one pile at the first one's place
    /// (reading order: rows 12 px apart count as one row). Adicionar ao Fichário: with a pile in the
    /// selection they join it behind its front page.
    pub(crate) fn stack_notes(&mut self) -> bool {
        let mut notes = self.loose_selected_notes();
        let pile = self.selected_stack();
        if notes.len() < if pile.is_some() { 1 } else { 2 } {
            return false;
        }
        notes.sort_by(|a, b| {
            let (a, b) = (element_bounds(&self.scene.elements[*a]).min, element_bounds(&self.scene.elements[*b]).min);
            if (a.y - b.y).abs() > 12.0 { a.y.total_cmp(&b.y) } else { a.x.total_cmp(&b.x) }
        });
        self.snapshot();
        let front = pile.unwrap_or(notes[0]);
        let origin = element_bounds(&self.scene.elements[front]).min;
        let group = pile.and_then(|p| self.scene.elements[p].group_id.clone()).unwrap_or_else(new_id);
        for card in notes {
            self.tuck_note(card, origin);
            self.regroup_note(card, &group, true, card == front);
        }
        self.editing = None;
        self.selected = vec![front];
        true
    }

    /// Remover do Fichário: page `i` leaves its pile and lands beside it; a pile left with one
    /// page is no pile any more.
    pub(crate) fn remove_from_stack(&mut self, i: usize) -> bool {
        let pages = self.stack_pages(i);
        if pages.len() < 2 || !self.scene.elements[i].note.stacked {
            return false;
        }
        self.snapshot();
        let frame = element_bounds(&self.scene.elements[i]);
        let rest: Vec<usize> = pages.into_iter().filter(|p| *p != i).collect();
        if self.scene.elements[i].note.front {
            self.scene.elements[rest[0]].note.front = true;
        }
        self.tuck_note(i, Pos2::new(frame.max.x + TAB_W + SPILL, frame.min.y));
        self.regroup_note(i, &new_id(), false, false);
        if let [last] = rest[..] {
            self.regroup_note(last, &new_id(), false, false);
        }
        self.editing = None;
        self.selected = vec![i];
        true
    }

    /// Nova Página: a blank note of the front page's size joins the pile in front, its text taking the keys.
    pub(crate) fn new_stack_page(&mut self, i: usize) -> bool {
        let Some(group) = self.scene.elements.get(i).filter(|e| e.note.stacked).and_then(|e| e.group_id.clone()) else { return false };
        let pages = self.stack_pages(i);
        let [note, mut text] = self.new_note(element_bounds(&self.scene.elements[i]), String::new());
        text.style.font_size = 20.0;
        self.snapshot();
        for p in pages {
            self.scene.elements[p].note.front = false;
        }
        self.scene.elements.extend([note, text]);
        let card = self.scene.elements.len() - 2;
        self.regroup_note(card, &group, true, true);
        self.selected = vec![card];
        self.editing = Some(card + 1);
        self.edit_focused = false;
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
        pages.len() >= 2 && self.show_stack_page(pages[(at as i32 + delta).rem_euclid(pages.len() as i32) as usize])
    }

    /// Page `page` becomes its pile's front and takes the selection (a click on its tab).
    pub(crate) fn show_stack_page(&mut self, page: usize) -> bool {
        if !self.scene.elements.get(page).is_some_and(|e| e.note.stacked) {
            return false;
        }
        if !self.scene.elements[page].note.front {
            self.snapshot();
            for p in self.stack_pages(page) {
                self.scene.elements[p].note.front = p == page;
            }
        }
        self.editing = None;
        self.selected = vec![page];
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

    /// The tab rail at the right of each pile's front page, one tab per page in its paper: a click
    /// shows that page, the hint is the page's first line.
    pub(crate) fn stack_tabs_ui(&mut self, ctx: &Context) {
        let mut show = None;
        for i in 0..self.scene.elements.len() {
            let e = &self.scene.elements[i];
            if !(e.created.is_some() && e.note.stacked && e.note.front) {
                continue;
            }
            let pages = self.stack_pages(i);
            let (frame, zoom) = self.terminal_frame(e);
            let tabs = tab_rects(frame, pages.len(), zoom);
            if pages.len() < 2 || !tabs.iter().any(|t| self.canvas_rect.intersects(*t)) {
                continue;
            }
            // the front's tab last, over the others
            let order: Vec<usize> = (0..pages.len()).filter(|k| pages[*k] != i).chain(pages.iter().position(|p| *p == i)).collect();
            let id = e.id.clone();
            egui::Area::new(egui::Id::unique(("stack_tabs", &id))).order(egui::Order::Foreground).fixed_pos(tabs[0].min).show(ctx, |ui| {
                for k in order {
                    let page = &self.scene.elements[pages[k]];
                    let (paper, ink) = note_colors(page, page.style.opacity);
                    let front = pages[k] == i;
                    let text = self.note_text(pages[k]);
                    let first = text.lines().map(str::trim).find(|l| !l.is_empty()).unwrap_or("");
                    let tip = if first.is_empty() { format!("Página {}", k + 1) } else { format!("Página {}: {first}", k + 1) };
                    let response =
                        ui.interact(tabs[k], egui::Id::unique(("stack_tab", &id, k)), Sense::click()).on_hover_cursor(CursorIcon::PointingHand).tip(tip);
                    let fill = if front { paper } else { paper.gamma_multiply(if response.hovered() { 0.85 } else { 0.6 }) };
                    let round = (8.0 * zoom).min(255.0) as u8;
                    let corner = egui::CornerRadius { nw: 0, sw: 0, ne: round, se: round };
                    ui.painter().rect(tabs[k], corner, fill, Stroke::new(zoom, ink.gamma_multiply(0.18)), StrokeKind::Inside);
                    let at = tabs[k].center() + Vec2::new(TAB_TUCK * zoom / 2.0, 0.0);
                    ui.painter().text(at, egui::Align2::CENTER_CENTER, (k + 1).to_string(), FontId::proportional(10.0 * zoom), ink.gamma_multiply(0.7));
                    if response.clicked() {
                        show = Some(pages[k]);
                    }
                }
            });
        }
        if let Some(page) = show {
            self.show_stack_page(page);
        }
    }

    /// The menu rows of a pile's front page.
    pub(crate) fn stack_rows(&self, i: usize) -> Vec<Entry> {
        if !self.scene.elements.get(i).is_some_and(|e| e.note.stacked) {
            return Vec::new();
        }
        vec![
            item("stack_page", "Nova Página"),
            item("stack_next", "Próxima página"),
            item("stack_prev", "Página anterior"),
            item("stack_remove", "Remover do Fichário"),
            item("unstack", "Desempilhar Todas as Páginas"),
        ]
    }

    /// The MCP's `note_stack`: stack the named notes (into the pile of one of them, if any), or act
    /// on the pile of the first.
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
            "show" => self.show_stack_page(first),
            "new_page" => self.new_stack_page(first),
            "remove" => self.remove_from_stack(first),
            other => return Err(format!("ação desconhecida: {other}")),
        };
        if done {
            Ok("Feito.".into())
        } else {
            Err("nada a fazer: stack pede 2 ou mais notas soltas (ou uma pilha e uma nota solta); as outras ações, uma nota de uma pilha".into())
        }
    }
}

/// A page waiting behind its pile's front: neither it nor its text is painted or hit.
pub(crate) fn hidden_page(elements: &[Element], e: &Element) -> bool {
    let waits = |c: &Element| c.created.is_some() && c.note.stacked && !c.note.front;
    waits(e) || note_of_label(elements, e).is_some_and(|n| waits(&elements[n]))
}
