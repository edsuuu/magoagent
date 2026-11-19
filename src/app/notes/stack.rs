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

/// Tabs never shrink below this (the reference app's `MIN_TAB_HEIGHT`); past what fits, the last one is "Mais N páginas".
const MIN_TAB_HEIGHT: f32 = 64.0;

/// The tab rail of a pile: a tab per visible page, and when the pages do not fit at `MIN_TAB_HEIGHT` a last one
/// standing for the `hidden` ones left out (the last pages).
pub(crate) struct Rail {
    pub(crate) tabs: Vec<Rect>,
    pub(crate) more: Option<(Rect, usize)>,
}

/// The rail of a pile of `page_count` pages whose front page sits in `frame` on screen: the tabs share the rail's height, 10 over
/// each other, at most 120 tall (the reference app's `railMembership` and `liveTabHeight`).
pub(crate) fn tab_rail(frame: Rect, page_count: usize, zoom: f32) -> Rail {
    let available = (frame.height() / zoom - 2.0 * RAIL_INSET).max(0.0);

    let (shown, hidden) = if page_count == 0 || available <= MIN_TAB_HEIGHT {
        (page_count.min(1), page_count.saturating_sub(1))
    } else {
        let capacity = (((available - TAB_OVERLAP) / (MIN_TAB_HEIGHT - TAB_OVERLAP)).floor() as usize).max(1);

        if page_count <= capacity {
            (page_count, 0)
        } else {
            let shown = (capacity - 1).max(1);

            (shown, page_count - shown)
        }
    };

    let slots = shown + usize::from(hidden > 0);
    let fit = (available + TAB_OVERLAP * (slots as f32 - 1.0)) / slots.max(1) as f32;
    let tab_height = fit.clamp(TAB_OVERLAP + 2.0, TAB_MAX_H);

    let rect = |index: usize, height: f32| {
        let top = frame.top() + (RAIL_INSET + index as f32 * (tab_height - TAB_OVERLAP)) * zoom;

        Rect::from_min_size(Pos2::new(frame.right() - TAB_TUCK * zoom, top), Vec2::new(TAB_W + TAB_TUCK, height) * zoom)
    };

    // the slots are laid out at `h`; the "more" tab is at most 64 tall
    Rail { tabs: (0..shown).map(|index| rect(index, tab_height)).collect(), more: (hidden > 0).then(|| (rect(shown, tab_height.min(MIN_TAB_HEIGHT)), hidden)) }
}

/// Where a pile of notes stands: from the first page's corner, its width (at least 270) and 1.4 times that with the rail.
pub(crate) fn page_rect(front: Rect) -> Rect {
    let width = front.width().max(PAGE_MIN_W);

    Rect::from_min_size(front.min, Vec2::new(width, front.height().max(((width + TAB_W) * PAGE_RATIO).round())))
}

/// A tab as the reference app's: from under the page it curves out to its full width, runs down and curves back.
/// The back ones are shaded from the page's side (0.22 of black) to 0.08 at 45% of their width.
fn tab_shape(rect: Rect, paper: Color32, front: bool) -> Shape {
    let (width, height) = (rect.width(), rect.height());
    // the curve's run at this zoom: the tab is (TAB_W + TAB_TUCK) wide on the page
    let run = (TAB_TRANSITION * width / (TAB_W + TAB_TUCK)).min(height / 3.0);

    let cubic = |control: [Pos2; 4], progress: f32| {
        let inverse = 1.0 - progress;

        (control[0].to_vec2() * inverse * inverse * inverse
            + control[1].to_vec2() * 3.0 * inverse * inverse * progress
            + control[2].to_vec2() * 3.0 * inverse * progress * progress
            + control[3].to_vec2() * progress * progress * progress)
            .to_pos2()
    };

    let top = [Pos2::new(0.0, 0.0), Pos2::new(0.0, run * 0.55), Pos2::new(width, run * 0.45), Pos2::new(width, run)];
    let bottom = [Pos2::new(width, height - run), Pos2::new(width, height - run * 0.45), Pos2::new(0.0, height - run * 0.55), Pos2::new(0.0, height)];
    let steps = 12;

    let edge: Vec<Pos2> =
        (0..=steps).map(|step| cubic(top, step as f32 / steps as f32)).chain((0..=steps).map(|step| cubic(bottom, step as f32 / steps as f32))).collect();

    let shade = |x: f32| {
        if front {
            return paper;
        }

        let fraction = (x / width.max(1.0) / 0.45).min(1.0);

        paper.lerp_to_gamma(Color32::BLACK, 0.22 + (0.08 - 0.22) * fraction)
    };

    let mut mesh = egui::Mesh::default();

    for edge_point in &edge {
        let at = rect.min + edge_point.to_vec2();

        mesh.colored_vertex(Pos2::new(rect.left(), at.y), shade(0.0));
        mesh.colored_vertex(at, shade(edge_point.x));
    }

    for step in 0..edge.len() as u32 - 1 {
        let (first, second, third, fourth) = (2 * step, 2 * step + 1, 2 * step + 2, 2 * step + 3);

        mesh.add_triangle(first, second, third);
        mesh.add_triangle(second, fourth, third);
    }

    Shape::mesh(mesh)
}

/// A page's name on its tab: the note's first line in lower case, words joined by "-" ("Nota um" is "nota-um").
pub(crate) fn tab_name(text: &str) -> String {
    let name = crate::mcp::bridge::note_name(text).to_lowercase();

    name.split_whitespace().collect::<Vec<_>>().join("-")
}

impl DrawingApp {
    /// The cards of the pile `index` belongs to, in scene order (just `index` when it is on no pile).
    pub(crate) fn stack_pages(&self, index: usize) -> Vec<usize> {
        let Some(element) = self.scene.elements.get(index) else { return Vec::new() };
        let group = element.group_id.as_ref().filter(|_| element.note.stacked);

        let pages: Vec<usize> = (0..self.scene.elements.len())
            .filter(|page_index| {
                let other = &self.scene.elements[*page_index];

                other.created.is_some() && other.note.stacked && group.is_some() && other.group_id.as_ref() == group
            })
            .collect();

        if pages.is_empty() { vec![index] } else { pages }
    }

    /// Moves a note card and its text so the card's corner is at `to`.
    fn tuck_note(&mut self, card: usize, to: Pos2) {
        let delta = to - element_bounds(&self.scene.elements[card]).min;

        for index in [Some(card), note_label(&self.scene.elements, card)].into_iter().flatten() {
            self.scene.elements[index].move_by(delta);
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
        for index in [Some(card), note_label(&self.scene.elements, card)].into_iter().flatten() {
            self.scene.elements[index].group_id = Some(group.to_string());
        }

        self.scene.elements[card].note.stacked = stacked;
        self.scene.elements[card].note.front = front;
    }

    /// The selected notes that are on no pile yet.
    pub(crate) fn loose_selected_notes(&self) -> Vec<usize> {
        let mut notes: Vec<usize> = self
            .selected
            .iter()
            .copied()
            .filter(|index| self.scene.elements.get(*index).is_some_and(|element| element.created.is_some() && !element.note.stacked))
            .collect();

        notes.sort_unstable();
        notes.dedup();
        notes
    }

    /// The front page of a pile in the selection, if any.
    fn selected_stack(&self) -> Option<usize> {
        let page = self.selected.iter().copied().find(|index| self.scene.elements.get(*index).is_some_and(|element| element.note.stacked))?;

        self.stack_pages(page).into_iter().find(|page_index| self.scene.elements[*page_index].note.front)
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

        notes.sort_by(|first, second| {
            let (first, second) = (element_bounds(&self.scene.elements[*first]).min, element_bounds(&self.scene.elements[*second]).min);

            if (first.y - second.y).abs() > 12.0 { first.y.total_cmp(&second.y) } else { first.x.total_cmp(&second.x) }
        });

        self.snapshot();
        let front = pile.unwrap_or(notes[0]);
        let page = page_rect(element_bounds(&self.scene.elements[front]));
        let group = pile.and_then(|pile_index| self.scene.elements[pile_index].group_id.clone()).unwrap_or_else(new_id);

        for card in notes.into_iter().chain([front]) {
            self.fit_page(card, page);
            self.regroup_note(card, &group, true, card == front);
        }

        self.editing = None;
        self.selected = vec![front];
        true
    }

    /// Remover do Fichário: page `page` leaves its pile and lands beside it; a pile left with one
    /// page is no pile any more.
    pub(crate) fn remove_from_stack(&mut self, page: usize) -> bool {
        let pages = self.stack_pages(page);

        if pages.len() < 2 || !self.scene.elements[page].note.stacked {
            return false;
        }

        self.snapshot();
        let frame = element_bounds(&self.scene.elements[page]);
        let rest: Vec<usize> = pages.into_iter().filter(|other| *other != page).collect();

        if self.scene.elements[page].note.front {
            self.scene.elements[rest[0]].note.front = true;
        }

        self.tuck_note(page, Pos2::new(frame.max.x + TAB_W + SPILL, frame.min.y));
        self.regroup_note(page, &new_id(), false, false);

        if let [last] = rest[..] {
            self.regroup_note(last, &new_id(), false, false);
        }

        self.editing = None;
        self.selected = vec![page];
        true
    }

    /// Nova Página: a blank note of the front page's size joins the pile in front, its text taking the keys.
    pub(crate) fn new_stack_page(&mut self, page: usize) -> bool {
        let Some(group) = self.scene.elements.get(page).filter(|element| element.note.stacked).and_then(|element| element.group_id.clone()) else {
            return false;
        };

        let pages = self.stack_pages(page);
        let [note, mut text] = self.new_note(element_bounds(&self.scene.elements[page]), String::new());

        text.style.font_size = 20.0;
        self.snapshot();

        for other in pages {
            self.scene.elements[other].note.front = false;
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
    pub(crate) fn unstack(&mut self, page: usize) -> bool {
        if !self.scene.elements.get(page).is_some_and(|element| element.note.stacked) {
            return false;
        }

        self.snapshot();
        let pages = self.stack_pages(page);
        let origin = element_bounds(&self.scene.elements[page]).min;

        for (position, card) in pages.iter().enumerate() {
            self.tuck_note(*card, origin + Vec2::splat(SPILL * position as f32));
            self.regroup_note(*card, &new_id(), false, false);
        }

        self.selected = pages;
        true
    }

    /// The pile's front page moves `delta` pages on (wrapping) and takes the selection.
    pub(crate) fn flip_stack(&mut self, page: usize, delta: i32) -> bool {
        let pages = self.stack_pages(page);
        let Some(at) = pages.iter().position(|other| self.scene.elements[*other].note.front) else { return false };

        pages.len() >= 2 && self.show_stack_page(pages[(at as i32 + delta).rem_euclid(pages.len() as i32) as usize])
    }

    /// Page `page` becomes its pile's front and takes the selection (a click on its tab).
    pub(crate) fn show_stack_page(&mut self, page: usize) -> bool {
        if !self.scene.elements.get(page).is_some_and(|element| element.note.stacked) {
            return false;
        }

        if !self.scene.elements[page].note.front {
            self.snapshot();

            for member in self.stack_pages(page) {
                self.scene.elements[member].note.front = member == page;
            }
        }

        self.editing = None;
        self.selected = vec![page];
        true
    }

    /// A lone note dropped with its center on a pile's front page joins it, behind that page.
    pub(crate) fn drop_into_stack(&mut self) {
        let [card] = self.selected[..] else { return };

        if !self.scene.elements.get(card).is_some_and(|element| element.created.is_some() && !element.note.stacked) {
            return;
        }

        let center = element_bounds(&self.scene.elements[card]).center();

        let target = (0..self.scene.elements.len()).rev().find(|index| {
            let other = &self.scene.elements[*index];

            *index != card && other.note.stacked && other.note.front && element_bounds(other).contains(center)
        });

        let Some(target) = target else { return };
        let group = self.scene.elements[target].group_id.clone().unwrap_or_else(new_id);

        self.fit_page(card, element_bounds(&self.scene.elements[target]));
        self.regroup_note(card, &group, true, false);
        self.selected = vec![target];
    }

    /// The pages of the pile of `index` left off its tab rail (they are in "Mais N páginas").
    pub(crate) fn hidden_stack_pages(&self, index: usize) -> Vec<usize> {
        let pages = self.stack_pages(index);
        let (frame, zoom) = self.terminal_frame(&self.scene.elements[index]);
        let shown = tab_rail(frame, pages.len(), zoom).tabs.len();

        pages.into_iter().skip(shown).collect()
    }

    /// The tab rail at the right of each pile's front page, one tab per page in its paper: a click
    /// shows that page, the hint is the page's first line.
    pub(crate) fn stack_tabs_ui(&mut self, ctx: &Context) {
        let (mut show, mut menu, mut more) = (None, None, None);

        for index in 0..self.scene.elements.len() {
            let element = &self.scene.elements[index];

            if !(element.created.is_some() && element.note.stacked && element.note.front) {
                continue;
            }

            let pages = self.stack_pages(index);
            let (frame, zoom) = self.terminal_frame(element);
            let Rail { tabs, more: overflow } = tab_rail(frame, pages.len(), zoom);

            if pages.len() < 2 || !tabs.iter().chain(overflow.as_ref().map(|more_tab| &more_tab.0)).any(|tab| self.canvas_rect.intersects(*tab)) {
                continue;
            }

            // the front's tab last, over the others
            let order: Vec<usize> = (0..tabs.len())
                .filter(|tab_index| pages[*tab_index] != index)
                .chain(pages.iter().position(|page_index| *page_index == index).filter(|tab_index| *tab_index < tabs.len()))
                .collect();

            let id = element.id.clone();

            egui::Area::new(egui::Id::unique(("stack_tabs", &id))).order(egui::Order::Foreground).fixed_pos(tabs[0].min).show(ctx, |ui| {
                for tab_index in order {
                    let page = &self.scene.elements[pages[tab_index]];
                    let (paper, ink) = note_colors(page, page.style.opacity);
                    let front = pages[tab_index] == index;
                    // a back tab starts at the page's edge, the front one under it, joined to it
                    let rect = if front { tabs[tab_index] } else { tabs[tab_index].with_min_x(tabs[tab_index].left() + TAB_TUCK * zoom) };
                    let response = ui.interact(rect, egui::Id::unique(("stack_tab", &id, tab_index)), Sense::click()).on_hover_cursor(CursorIcon::PointingHand);

                    ui.painter().add(tab_shape(rect, if response.hovered() && !front { paper.lerp_to_gamma(Color32::WHITE, 0.15) } else { paper }, front));
                    // the page's name down the tab, in the code face, the front one darker
                    let name = tab_name(&self.note_text(pages[tab_index]));
                    let room = rect.height() - TAB_TRANSITION * zoom - 10.0 * zoom;
                    let color = ink.gamma_multiply(if front { 0.95 } else { 0.6 });
                    let galley = ui.painter().layout(name, FontId::monospace(10.0 * zoom), color, f32::INFINITY);

                    let galley = if galley.size().x > room {
                        let mut job = egui::text::LayoutJob::single_section(
                            tab_name(&self.note_text(pages[tab_index])),
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
                        show = Some(pages[tab_index]);
                    }

                    if response.secondary_clicked() {
                        menu = Some((pages[tab_index], response.interact_pointer_pos().unwrap_or(rect.center())));
                    }
                }

                // "Mais N páginas": a back tab in the app's own paper with "+N" down it, the hidden pages in a menu
                if let Some((rect, hidden)) = overflow {
                    let rect = rect.with_min_x(rect.left() + TAB_TUCK * zoom);

                    let response = ui
                        .interact(rect, egui::Id::unique(("stack_more", &id)), Sense::click())
                        .on_hover_cursor(CursorIcon::PointingHand)
                        .tip(more_pages_label(hidden));

                    let paper = ui.visuals().window_fill;

                    ui.painter().add(tab_shape(rect, if response.hovered() { paper.lerp_to_gamma(Color32::WHITE, 0.15) } else { paper }, false));
                    let galley = ui.painter().layout_no_wrap(format!("+{hidden}"), FontId::monospace(10.0 * zoom), ui.visuals().weak_text_color());
                    let at = rect.center() + Vec2::new(galley.size().y / 2.0, -galley.size().x / 2.0);

                    ui.painter().add(egui::epaint::TextShape::new(at, galley, ui.visuals().weak_text_color()).with_angle(std::f32::consts::FRAC_PI_2));

                    if response.clicked() {
                        more = Some((index, rect.right_top() + Vec2::new(6.0, 0.0)));
                    }
                }
            });
        }

        // the pages past the rail, one row each; a click shows the page
        if let Some((front, at)) = more {
            let rows: Vec<Entry> = self
                .hidden_stack_pages(front)
                .into_iter()
                .map(|page_index| item(&format!("stack_show:{}", self.scene.elements[page_index].id), &tab_name(&self.note_text(page_index))))
                .collect();

            let target = crate::app::ui::context::Target::Element(self.scene.elements[front].id.clone());

            self.context = Some(crate::app::ui::context::ContextMenu::new(at, target, Some(rows)));
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
    pub(crate) fn uniform_stack_color(&mut self, index: usize) -> bool {
        let pages = self.stack_pages(index);

        if pages.len() < 2 {
            return false;
        }

        self.snapshot();
        let (fill, stroke) = (self.scene.elements[index].style.fill, self.scene.elements[index].style.stroke);

        for page in pages {
            self.scene.elements[page].style.fill = fill;

            if let Some(label) = note_label(&self.scene.elements, page) {
                self.scene.elements[label].style.stroke = stroke;
            }
        }

        true
    }

    /// Apagar Nota: page `page` leaves its pile and is deleted.
    pub(crate) fn delete_stack_page(&mut self, page: usize) {
        if self.remove_from_stack(page) {
            self.selected = vec![page];
            self.delete_selected();
        }
    }

    /// The rows a page adds to its note's menu (its tab's right click).
    pub(crate) fn stack_rows(&self, index: usize) -> Vec<Entry> {
        if !self.scene.elements.get(index).is_some_and(|element| element.note.stacked) {
            return Vec::new();
        }

        vec![item("stack_page", "Adicionar Nota"), item("stack_remove", "Remover do Fichário"), Entry::Sep, red("stack_delete", "Apagar Nota")]
    }

    /// The pile's own menu (a right click on its page), as the reference app's.
    pub(crate) fn fichario_rows(&self, index: usize) -> Vec<Entry> {
        let lock = if self.scene.elements[index].locked { "Desbloquear" } else { "Bloquear" };

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
            let index = self.node_named(name)?;

            if self.scene.elements[index].created.is_none() {
                return Err(format!("{name} não é uma nota"));
            }

            cards.push(index);
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
pub(crate) fn hidden_page(elements: &[Element], element: &Element) -> bool {
    let waits = |page: &Element| page.created.is_some() && page.note.stacked && !page.note.front;

    waits(element) || note_of_label(elements, element).is_some_and(|note_index| waits(&elements[note_index]))
}

/// "Mais N páginas" (one: "Mais 1 página").
pub(crate) fn more_pages_label(count: usize) -> String {
    if count == 1 { "Mais 1 página".into() } else { format!("Mais {count} páginas") }
}
