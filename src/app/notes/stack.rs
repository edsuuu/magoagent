//! A pile of sticky notes (a card index): the notes sit on one another, the front page shows and
//! the others wait behind it. They share the group of the pile, so the pile moves, copies and
//! deletes as one; the hidden pages are "card text" of the front one (`is_card_text`).

use crate::app::ui::context::{Entry, item, red};
use crate::prelude::*;

/// Where the pages of an unstacked pile spill, one step each (24 px, the reference app's).
const SPILL: f32 = 24.0;
/// The tab rail: tabs this wide out of the page's right edge, the front one tucked 2 px under it, at most 120 tall
/// and 10 over each other, from 10 below the top (the reference app's).
const TAB_W: f32 = 23.0;
const TAB_TUCK: f32 = 2.0;
const TAB_MAX_H: f32 = 120.0;
const TAB_OVERLAP: f32 = 10.0;
const RAIL_INSET: f32 = 10.0;
/// How far down a tab's curve runs from the page to its full width.
const TAB_TRANSITION: f32 = 26.0;
/// A pile is a page standing up: at least this wide, and 1.4 times as tall as it is wide with the rail.
const PAGE_MIN_W: f32 = 270.0;
const PAGE_RATIO: f32 = 1.4;

/// The tabs of a pile of `n` pages whose front page sits in `frame` on screen: they share the rail's height, 10 over
/// each other, at most 120 tall.
// ponytail: past what fits, tabs shrink instead of the reference's "Mais N páginas" tab
pub(crate) fn tab_rects(frame: Rect, n: usize, zoom: f32) -> Vec<Rect> {
    let avail = frame.height() / zoom - 2.0 * RAIL_INSET;
    let fit = (avail + TAB_OVERLAP * (n as f32 - 1.0)) / n.max(1) as f32;
    let h = fit.clamp(TAB_OVERLAP + 2.0, TAB_MAX_H);
    (0..n)
        .map(|k| {
            let top = frame.top() + (RAIL_INSET + k as f32 * (h - TAB_OVERLAP)) * zoom;
            Rect::from_min_size(Pos2::new(frame.right() - TAB_TUCK * zoom, top), Vec2::new(TAB_W + TAB_TUCK, h) * zoom)
        })
        .collect()
}

/// Where a pile of notes stands: from the first page's corner, its width (at least 270) and 1.4 times that with the rail.
pub(crate) fn page_rect(front: Rect) -> Rect {
    let w = front.width().max(PAGE_MIN_W);
    Rect::from_min_size(front.min, Vec2::new(w, front.height().max(((w + TAB_W) * PAGE_RATIO).round())))
}

/// A tab as the reference app's: from under the page it curves out to its full width, runs down and curves back.
/// The back ones are shaded from the page's side (0.22 of black) to 0.08 at 45% of their width.
fn tab_shape(rect: Rect, paper: Color32, front: bool) -> Shape {
    let (w, h) = (rect.width(), rect.height());
    // the curve's run at this zoom: the tab is (TAB_W + TAB_TUCK) wide on the page
    let t = (TAB_TRANSITION * w / (TAB_W + TAB_TUCK)).min(h / 3.0);
    let cubic = |p: [Pos2; 4], s: f32| {
        let u = 1.0 - s;
        (p[0].to_vec2() * u * u * u + p[1].to_vec2() * 3.0 * u * u * s + p[2].to_vec2() * 3.0 * u * s * s + p[3].to_vec2() * s * s * s).to_pos2()
    };
    let top = [Pos2::new(0.0, 0.0), Pos2::new(0.0, t * 0.55), Pos2::new(w, t * 0.45), Pos2::new(w, t)];
    let bottom = [Pos2::new(w, h - t), Pos2::new(w, h - t * 0.45), Pos2::new(0.0, h - t * 0.55), Pos2::new(0.0, h)];
    let steps = 12;
    let edge: Vec<Pos2> = (0..=steps).map(|k| cubic(top, k as f32 / steps as f32)).chain((0..=steps).map(|k| cubic(bottom, k as f32 / steps as f32))).collect();
    let shade = |x: f32| {
        if front {
            return paper;
        }
        let f = (x / w.max(1.0) / 0.45).min(1.0);
        paper.lerp_to_gamma(Color32::BLACK, 0.22 + (0.08 - 0.22) * f)
    };
    let mut mesh = egui::Mesh::default();
    for p in &edge {
        let at = rect.min + p.to_vec2();
        mesh.colored_vertex(Pos2::new(rect.left(), at.y), shade(0.0));
        mesh.colored_vertex(at, shade(p.x));
    }
    for k in 0..edge.len() as u32 - 1 {
        let (a, b, c, d) = (2 * k, 2 * k + 1, 2 * k + 2, 2 * k + 3);
        mesh.add_triangle(a, b, c);
        mesh.add_triangle(b, d, c);
    }
    Shape::mesh(mesh)
}

/// A page's name on its tab: the note's first line in lower case, words joined by "-" ("Nota um" is "nota-um").
pub(crate) fn tab_name(text: &str) -> String {
    let name = crate::bridge::note_name(text).to_lowercase();
    name.split_whitespace().collect::<Vec<_>>().join("-")
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

    /// Puts a note card (and its text, at the card's center) over `rect`: a page of a pile takes the pile's size.
    // ponytail: a page spilled out of the pile keeps the pile's size, the reference's keeps its own
    fn fit_page(&mut self, card: usize, rect: Rect) {
        if let ElementKind::Rectangle { x, y, width, height } = &mut self.scene.elements[card].kind {
            (*x, *y, *width, *height) = (rect.min.x, rect.min.y, rect.width(), rect.height());
        }
        if let Some(label) = note_label(&self.scene.elements, card)
            && let ElementKind::Text { x, y, .. } = &mut self.scene.elements[label].kind
        {
            (*x, *y) = (rect.center().x, rect.center().y);
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
        let page = page_rect(element_bounds(&self.scene.elements[front]));
        let group = pile.and_then(|p| self.scene.elements[p].group_id.clone()).unwrap_or_else(new_id);
        for card in notes.into_iter().chain([front]) {
            self.fit_page(card, page);
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
        self.fit_page(card, element_bounds(&self.scene.elements[target]));
        self.regroup_note(card, &group, true, false);
        self.selected = vec![target];
    }

    /// The tab rail at the right of each pile's front page, one tab per page in its paper: a click
    /// shows that page, the hint is the page's first line.
    pub(crate) fn stack_tabs_ui(&mut self, ctx: &Context) {
        let (mut show, mut menu) = (None, None);
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
                    // a back tab starts at the page's edge, the front one under it, joined to it
                    let rect = if front { tabs[k] } else { tabs[k].with_min_x(tabs[k].left() + TAB_TUCK * zoom) };
                    let response = ui.interact(rect, egui::Id::unique(("stack_tab", &id, k)), Sense::click()).on_hover_cursor(CursorIcon::PointingHand);
                    ui.painter().add(tab_shape(rect, if response.hovered() && !front { paper.lerp_to_gamma(Color32::WHITE, 0.15) } else { paper }, front));
                    // the page's name down the tab, in the code face, the front one darker
                    let name = tab_name(&self.note_text(pages[k]));
                    let room = rect.height() - TAB_TRANSITION * zoom - 10.0 * zoom;
                    let color = ink.gamma_multiply(if front { 0.95 } else { 0.6 });
                    let galley = ui.painter().layout(name, FontId::monospace(10.0 * zoom), color, f32::INFINITY);
                    let galley = if galley.size().x > room {
                        let mut job = egui::text::LayoutJob::single_section(
                            tab_name(&self.note_text(pages[k])),
                            egui::TextFormat::simple(FontId::monospace(10.0 * zoom), color),
                        );
                        job.wrap = egui::text::TextWrapping::truncate_at_width(room.max(1.0));
                        ui.painter().layout_job(job)
                    } else {
                        galley
                    };
                    let size = galley.size();
                    // turned a quarter clockwise: it reads from the top down, centered on the tab
                    let at = rect.center() + Vec2::new(size.y / 2.0, -size.x / 2.0);
                    ui.painter().add(egui::epaint::TextShape::new(at, galley, color).with_angle(std::f32::consts::FRAC_PI_2));
                    if response.clicked() {
                        show = Some(pages[k]);
                    }
                    if response.secondary_clicked() {
                        menu = Some((pages[k], response.interact_pointer_pos().unwrap_or(rect.center())));
                    }
                }
            });
        }
        if let Some(page) = show {
            self.show_stack_page(page);
        }
        // a tab's right click is its page's menu: the note's own rows and the pile's
        if let Some((page, at)) = menu {
            self.show_stack_page(page);
            let target = crate::app::ui::context::Target::Element(self.scene.elements[page].id.clone());
            let rows = self.note_rows(page, &self.scene.elements[page].id.clone());
            self.context = Some(crate::app::ui::context::ContextMenu::new(at, target, Some(rows)));
        }
    }

    /// Cor Uniforme: every page of the pile takes the front page's paper.
    pub(crate) fn uniform_stack_color(&mut self, i: usize) -> bool {
        let pages = self.stack_pages(i);
        if pages.len() < 2 {
            return false;
        }
        self.snapshot();
        let (fill, stroke) = (self.scene.elements[i].style.fill, self.scene.elements[i].style.stroke);
        for p in pages {
            self.scene.elements[p].style.fill = fill;
            if let Some(label) = note_label(&self.scene.elements, p) {
                self.scene.elements[label].style.stroke = stroke;
            }
        }
        true
    }

    /// Apagar Nota: page `i` leaves its pile and is deleted.
    pub(crate) fn delete_stack_page(&mut self, i: usize) {
        if self.remove_from_stack(i) {
            self.selected = vec![i];
            self.delete_selected();
        }
    }

    /// The rows a page adds to its note's menu (its tab's right click).
    pub(crate) fn stack_rows(&self, i: usize) -> Vec<Entry> {
        if !self.scene.elements.get(i).is_some_and(|e| e.note.stacked) {
            return Vec::new();
        }
        vec![item("stack_page", "Adicionar Nota"), item("stack_remove", "Remover do Fichário"), Entry::Sep, red("stack_delete", "Apagar Nota")]
    }

    /// The pile's own menu (a right click on its page), as the reference app's.
    pub(crate) fn fichario_rows(&self, i: usize) -> Vec<Entry> {
        let lock = if self.scene.elements[i].locked { "Desbloquear" } else { "Bloquear" };
        vec![
            item("stack_uniform", "Cor Uniforme"),
            Entry::Sep,
            item("unstack", "Desempilhar Todas as Páginas"),
            Entry::Sep,
            item("copy_el", "Copiar Elemento"),
            item("dup", "Duplicar"),
            item("lock", lock),
            Entry::Sep,
            red("delete", "Apagar"),
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
            "uniform" => self.uniform_stack_color(first),
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
