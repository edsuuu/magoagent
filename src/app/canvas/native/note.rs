//! A sticky note for SwiftUI: its card as `paint_note` draws it (ops), its text as egui's note editor lays it out (runs
//! and marks, `markdown::live_runs`: at rest no caret, so every mark hides as it rests), the buttons at its foot and, on
//! a pile's front page, the tab rail.

use super::*;
use crate::app::notes::stack::{Rail, TAB_TRANSITION, TAB_TUCK, more_pages_label, tab_edge, tab_name, tab_rail, tab_shade};
use crate::render::markdown::{LiveRun, Ornament};

/// What goes over a hidden mark (`markdown::Ornament`): "bullet", "task", "quote" or "rule", at character `at`.
#[derive(Serialize, Clone, Debug, PartialEq)]
pub(crate) struct Mark {
    kind: &'static str,
    at: usize,
    done: bool,
}

pub(crate) fn marks(ornaments: &[Ornament]) -> Vec<Mark> {
    ornaments
        .iter()
        .map(|ornament| match *ornament {
            Ornament::Bullet(at) => Mark { kind: "bullet", at, done: false },
            Ornament::Task(at, done) => Mark { kind: "task", at, done },
            Ornament::Quote(at) => Mark { kind: "quote", at, done: false },
            Ornament::Rule(at) => Mark { kind: "rule", at, done: false },
        })
        .collect()
}

/// A button at the note's foot (notes/buttons.rs), its box in the world.
#[derive(Serialize, Clone, Debug, PartialEq)]
pub(crate) struct Button {
    label: String,
    tip: String,
    pub(super) rect: [f32; 4],
}

/// A page's tab on the rail of a pile (notes/stack.rs): its outline in the world, filled with `paper` (a front tab) or
/// shaded from `shade[0]` at the page's side to `shade[1]` at 45% of its width (a back one).
#[derive(Serialize, Clone, Debug, PartialEq)]
pub(crate) struct Tab {
    pub(super) id: String,
    pub(super) rect: [f32; 4],
    outline: Vec<f32>,
    paper: [u8; 4],
    #[serde(skip_serializing_if = "Option::is_none")]
    shade: Option<[[u8; 4]; 2]>,
    ink: [u8; 4],
    name: String,
    /// How long the name may run down the tab before it is cut.
    room: f32,
    pub(super) front: bool,
}

/// "Mais N páginas": the last tab, the pages past the rail in its menu.
#[derive(Serialize, Clone, Debug, PartialEq)]
pub(crate) struct More {
    rect: [f32; 4],
    outline: Vec<f32>,
    count: usize,
    label: String,
    pages: Vec<Page>,
}

#[derive(Serialize, Clone, Debug, PartialEq)]
pub(crate) struct Page {
    id: String,
    name: String,
}

/// A sticky note's body: its markdown in `area`, under the paper and header in `ops`. No `text` while it is typed.
#[derive(Serialize, Clone, Debug, PartialEq)]
pub(crate) struct Note {
    text: Option<String>,
    ink: [u8; 4],
    pub(super) size: f32,
    area: [f32; 4],
    raw: bool,
    blurred: bool,
    pub(super) runs: Vec<LiveRun>,
    pub(super) marks: Vec<Mark>,
    pub(super) buttons: Vec<Button>,
    pub(super) tabs: Vec<Tab>,
    #[serde(skip_serializing_if = "Option::is_none")]
    more: Option<More>,
}

/// A pile's page as its tab shows it: id, paper, ink and name.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct PageLook {
    pub(crate) id: String,
    pub(crate) paper: Color32,
    pub(crate) ink: Color32,
    pub(crate) name: String,
}

impl DrawingApp {
    /// The pages of the pile whose front page is `index`, for its tabs; none for any other note.
    pub(crate) fn native_pages(&self, index: usize) -> Vec<PageLook> {
        let element = &self.scene.elements[index];

        if !(element.created.is_some() && element.note.stacked && element.note.front) {
            return Vec::new();
        }

        let pages = self.stack_pages(index);

        if pages.len() < 2 {
            return Vec::new();
        }

        pages
            .into_iter()
            .map(|page| {
                let page_element = &self.scene.elements[page];
                let (paper, ink) = note_colors(page_element, page_element.style.opacity);

                PageLook { id: page_element.id.clone(), paper, ink, name: tab_name(&self.note_text(page)) }
            })
            .collect()
    }
}

fn outline(rect: Rect) -> Vec<f32> {
    tab_edge(rect).iter().flat_map(|point| [rect.min.x + point.x, rect.min.y + point.y]).collect()
}

/// The tab rail of a pile's front page `id` at `bounds`, in the world (notes/stack.rs `stack_tabs_ui` at 100%).
fn tabs(id: &str, bounds: Rect, pages: &[PageLook]) -> (Vec<Tab>, Option<More>) {
    let Rail { tabs, more } = tab_rail(bounds, pages.len(), 1.0);

    let mut out: Vec<Tab> = tabs
        .iter()
        .zip(pages)
        .map(|(rect, page)| {
            let front = page.id == id;
            // a back tab starts at the page's edge, the front one under it, joined to it
            let rect = if front { *rect } else { rect.with_min_x(rect.left() + TAB_TUCK) };

            Tab {
                id: page.id.clone(),
                rect: rect_of(rect),
                outline: outline(rect),
                paper: color(page.paper),
                shade: (!front).then(|| [color(tab_shade(page.paper, 0.0, rect.width())), color(tab_shade(page.paper, rect.width(), rect.width()))]),
                ink: color(page.ink.gamma_multiply(if front { 0.95 } else { 0.6 })),
                name: page.name.clone(),
                room: rect.height() - TAB_TRANSITION - 10.0,
                front,
            }
        })
        .collect();

    // the front's tab last, over the others
    out.sort_by_key(|tab| tab.front);

    let more = more.map(|(rect, count)| {
        let rect = rect.with_min_x(rect.left() + TAB_TUCK);
        let shown = tabs.len();
        let pages = pages[shown..].iter().map(|page| Page { id: page.id.clone(), name: page.name.clone() }).collect();

        More { rect: rect_of(rect), outline: outline(rect), count, label: more_pages_label(count), pages }
    });

    (out, more)
}

/// The card of a sticky note as `paint_note` draws it: shadowless paper with a hairline, the header strip, the tape and
/// the name; its text, buttons and tabs go in `note`.
pub(crate) fn note_drawing(note: &Element, look: &Look, out: &mut Drawing) {
    let bounds = local_bounds(note);
    let alpha = note.style.opacity as f32 / 100.0;
    let radius = if note.style.round { 8.0_f32.min(bounds.width() / 2.0).min(bounds.height() / 2.0).min(NOTE_HEADER) } else { 0.0 };
    let (paper, ink) = note_colors(note, note.style.opacity);
    let header = Rect::from_min_size(bounds.min, Vec2::new(bounds.width(), NOTE_HEADER.min(bounds.height())));
    let strip = if ink == Color32::WHITE { Color32::from_white_alpha(26) } else { Color32::from_black_alpha(15) };
    let hairline = color(Color32::from_black_alpha(20).gamma_multiply(alpha));
    let card = corners(bounds, radius);

    out.ops.push(Op::Fill { points: card.clone(), color: color(paper) });
    out.ops.push(Op::Stroke { points: card, color: hairline, width: 0.5, dash: None, closed: true });
    let header_outline = corner_outline(header, [radius, radius, 0.0, 0.0]).iter().flat_map(|point| [point.x, point.y]).collect();

    out.ops.push(Op::Fill { points: header_outline, color: color(strip.gamma_multiply(alpha)) });
    let tape: Vec<f32> = tape(bounds).iter().flat_map(|point| [point.x, point.y]).collect();

    out.ops.push(Op::Fill { points: tape.clone(), color: color(Color32::from_rgba_unmultiplied(0xe9, 0xe4, 0xd4, 205).gamma_multiply(alpha)) });
    out.ops.push(Op::Stroke { points: tape, color: color(Color32::from_black_alpha(34).gamma_multiply(alpha)), width: 0.8, dash: None, closed: true });
    let ink = ink.gamma_multiply(alpha);
    let text = look.note_text.clone().unwrap_or_default();

    if !note.note.blurred {
        let muted = ink.gamma_multiply(0.6);
        let (name, name_color) = if text.trim().is_empty() { ("Sem título".to_string(), muted.gamma_multiply(0.7)) } else { (note_name(&text), muted) };
        let size = 10.0;
        let y = bounds.min.y + (NOTE_HEADER - size * LINE_HEIGHT) / 2.0;

        out.ops.push(Op::Text { value: name, x: bounds.min.x + NOTE_PAD, y, size, color: color(name_color), hand: false, center: 0.0, halo: None });
    }

    let (runs, ornaments) =
        if note.note.raw || note.note.blurred || look.hide_text { Default::default() } else { markdown::live_runs(&text, note.note.font_size, None) };

    let (rects, height) = crate::app::notes::buttons::layout(&note.note.buttons, bounds.width() - 2.0 * NOTE_PAD);
    let foot = bounds.min + Vec2::new(NOTE_PAD, bounds.height() - NOTE_PAD - height);

    let buttons = note
        .note
        .buttons
        .iter()
        .zip(rects)
        .map(|(button, rect)| Button {
            label: button.label.clone(),
            tip: crate::app::notes::buttons::describe(button),
            rect: rect_of(rect.translate(foot.to_vec2())),
        })
        .collect();

    let (tabs, more) = tabs(&note.id, bounds, &look.pages);

    out.note = Some(Note {
        text: (!look.hide_text).then_some(text),
        ink: color(ink),
        size: note.note.font_size,
        area: rect_of(note_text_box(note)),
        raw: note.note.raw,
        blurred: note.note.blurred,
        runs,
        marks: marks(&ornaments),
        buttons,
        tabs,
        more,
    });

    // the tabs stand out of the page's right edge
    if let Some(rail) = out.note.as_ref().and_then(|note| note.tabs.iter().map(|tab| tab.rect).chain(note.more.as_ref().map(|more| more.rect)).last()) {
        let reach = Rect::from_min_size(Pos2::new(out.reach[0], out.reach[1]), Vec2::new(out.reach[2], out.reach[3]));

        out.reach = rect_of(reach.union(Rect::from_min_size(Pos2::new(rail[0], rail[1]), Vec2::new(rail[2], rail[3]))));
    }
}
