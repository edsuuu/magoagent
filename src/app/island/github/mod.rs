//! The GitHub tab (asked: "do github, pode criar uma tab ... para ele mostrar as pr ativas, fechadas, q travou ci ou cd,
//! com review, mergeada"; "faltou o botao para abrir no navegador, preview para editar o comentario do pr, se deu
//! conflito"). At the left the person's pull requests under four filters, each with its state's icon, its CI and what
//! asks for them; at the right the one picked: what holds it back, its description written or previewed (or a comment),
//! and the buttons. Under the list, each repository's main branch CI.

use super::forms;
use super::icons;
use super::paint::{CARD, CARD_FLAT, DIM, DIM_3, HOVER, INK, font, semi};
use crate::app::chat::list::clipped;
use crate::app::integrations::github::{CiState, PrFilter, PrState, PullRequest, Pulse};
use crate::prelude::*;

mod activity;

/// The island's height on this tab: the list and the description need the room.
pub(super) const HEIGHT: f32 = 404.0;
const ROW_H: f32 = 38.0;
const GREEN: Color32 = Color32::from_rgb(0x3f, 0xb9, 0x50);
const RED: Color32 = Color32::from_rgb(0xf8, 0x51, 0x49);
const AMBER: Color32 = Color32::from_rgb(0xd2, 0x99, 0x22);
const ORANGE: Color32 = Color32::from_rgb(0xf0, 0x88, 0x3e);
const YELLOW: Color32 = Color32::from_rgb(0xf5, 0xa5, 0x24);
const PURPLE: Color32 = Color32::from_rgb(0xa3, 0x71, 0xf7);
const GRAY: Color32 = Color32::from_rgb(0x8b, 0x94, 0x9e);
const SEG_ON: Color32 = Color32::from_rgb(0x2b, 0x2e, 0x33);

/// What the tab keeps between frames.
#[derive(Default)]
pub(crate) struct Page {
    filter: PrFilter,
    picked: Option<String>,
    /// Comentar, instead of the description.
    comment: bool,
    /// Escrever, instead of Prévia (how it opens: as GitHub shows it).
    writing: bool,
    /// The description as it is written here, and the pull request it is of.
    draft: String,
    draft_of: String,
    /// The comment being written.
    note: String,
    /// Why the last press did nothing (a long description is not saved).
    refused: String,
    /// Atividade shows at the right instead of the pull request.
    activity: bool,
}

/// The icon of a pull request's state, in its colour: open green, a draft grey, merged purple, closed red.
fn state_icon(request: &PullRequest) -> (crate::editor::icons::Svg, Color32) {
    match (request.state, request.draft) {
        (PrState::Merged, _) => (icons::PULL_MERGED, PURPLE),
        (PrState::Closed, _) => (icons::PULL_CLOSED, RED),
        (PrState::Open, true) => (icons::PULL_DRAFT, GRAY),
        (PrState::Open, false) => (icons::PULL, GREEN),
    }
}

/// The CI's mark: passed, failed, running; none when there are no checks.
fn ci_mark(ci: CiState) -> Option<(&'static str, Color32)> {
    match ci {
        CiState::Success => Some(("✓", GREEN)),
        CiState::Failure => Some(("✕", RED)),
        CiState::Pending => Some(("●", AMBER)),
        CiState::Unknown => None,
    }
}

/// The words of a branch's CI in the footer.
fn ci_words(ci: CiState) -> &'static str {
    match ci {
        CiState::Success => "passou",
        CiState::Failure => "falhou",
        CiState::Pending => "rodando",
        CiState::Unknown => "sem checks",
    }
}

/// The pull request shown at the right: the one picked while the filter keeps it, else the first of the list.
fn shown<'a>(list: &[&'a PullRequest], picked: Option<&str>) -> Option<&'a PullRequest> {
    picked.and_then(|id| list.iter().find(|request| request.id == id).copied()).or_else(|| list.first().copied())
}

fn tinted(color: Color32, alpha: f32) -> Color32 {
    Color32::from_rgba_unmultiplied(color.r(), color.g(), color.b(), (alpha * 255.0) as u8)
}

/// A small tag at the end of a row ("conflito", "revisão"), ending at `right`; its left.
fn tag(painter: &Painter, right: f32, center_y: f32, text: &str, color: Color32) -> f32 {
    let galley = painter.layout_no_wrap(text.to_string(), font(11.0), color);
    let rect = Rect::from_min_max(Pos2::new(right - galley.size().x - 16.0, center_y - 10.0), Pos2::new(right, center_y + 10.0));
    painter.rect_filled(rect, 10, tinted(color, 0.13));
    painter.galley(Pos2::new(rect.left() + 8.0, center_y - galley.size().y / 2.0), galley, color);
    rect.left()
}

/// Two words side by side, one lit (`.seg`): which was clicked, if any.
fn segment(ui: &mut egui::Ui, at: Pos2, words: [&str; 2], on: usize, id: &str) -> (Option<usize>, f32) {
    let galleys: Vec<_> = words.iter().map(|word| ui.painter().layout_no_wrap(word.to_string(), font(11.0), DIM)).collect();
    let width = galleys.iter().map(|galley| galley.size().x + 20.0).sum::<f32>() + 4.0;
    let ground = Rect::from_min_size(at, Vec2::new(width, 24.0));
    ui.painter().rect_filled(ground, 12, HOVER);
    let mut x = ground.left() + 2.0;
    let mut clicked = None;
    for (index, galley) in galleys.into_iter().enumerate() {
        let item = Rect::from_min_size(Pos2::new(x, ground.top() + 2.0), Vec2::new(galley.size().x + 20.0, 20.0));
        let response = ui.interact(item, ui.scope_id().with((id, index)), Sense::click()).on_hover_cursor(CursorIcon::PointingHand);
        if index == on {
            ui.painter().rect_filled(item, 10, SEG_ON);
        }
        let ink = if index == on || response.hovered() { INK } else { DIM };
        ui.painter().text(item.center(), egui::Align2::CENTER_CENTER, words[index], font(11.0), ink);
        if response.clicked() {
            clicked = Some(index);
        }
        x = item.right();
    }
    (clicked, width)
}

impl DrawingApp {
    /// The tab inside `page`.
    pub(super) fn island_github(&mut self, ui: &mut egui::Ui, page: Rect, colors: &UiColors) {
        let gap = 10.0;
        let left = Rect::from_min_size(page.min, Vec2::new(((page.width() - gap) * 1.22 / 2.22).round(), page.height()));
        let right = Rect::from_min_max(Pos2::new(left.right() + gap, page.top()), page.max);
        for card in [left, right] {
            ui.painter().rect(card, 20, CARD, Stroke::new(1.0, Color32::from_white_alpha(9)), StrokeKind::Inside);
        }
        let Some(pulse) = self.integrations.github.pulse.take() else {
            let words = self.integrations.github.error.clone().unwrap_or_else(|| "Lendo os pull requests no GitHub…".into());
            let galley = clipped(ui.painter(), words, font(12.0), DIM_3, left.width() - 28.0);
            ui.painter().galley(left.center() - galley.size() / 2.0, galley, DIM_3);
            return;
        };
        let mut state = std::mem::take(&mut self.island.github);
        let list = pulse.listed(state.filter);
        self.github_list(ui, left, &pulse, &list, &mut state);
        let picked = shown(&list, state.picked.as_deref()).cloned();
        if state.activity {
            self.github_activity(ui, right, &pulse.login, &mut state);
        } else if let Some(request) = picked {
            self.github_side(ui, right, &request, colors, &mut state);
        } else {
            let words = if state.filter == PrFilter::NeedsYou { "Nada precisa de você agora" } else { "Nenhum pull request aqui" };
            ui.painter().text(right.center(), egui::Align2::CENTER_CENTER, words, font(12.0), DIM_3);
        }
        self.integrations.github.pulse = Some(pulse);
        self.island.github = state;
    }

    /// Ctrl+O on the tab: the pull request shown opens in the browser, as Abrir no navegador.
    pub(super) fn github_open_shown(&self) {
        let (Some(pulse), state) = (&self.integrations.github.pulse, &self.island.github) else { return };
        if let Some(request) = shown(&pulse.listed(state.filter), state.picked.as_deref()).filter(|_| !state.activity) {
            self.integrations.pressed.borrow_mut().push(format!("open {}", request.page()));
        }
    }

    /// The filters, the rows and the footer.
    fn github_list(&self, ui: &mut egui::Ui, card: Rect, pulse: &Pulse, list: &[&PullRequest], state: &mut Page) {
        let inner = card.shrink2(Vec2::new(12.0, 10.0));
        let mut x = inner.left();
        for filter in PrFilter::EVERY {
            let count = pulse.listed(filter).len();
            let on = state.filter == filter;
            let label = ui.painter().layout_no_wrap(filter.label().to_string(), font(11.5), DIM);
            let number = ui.painter().layout_no_wrap(count.to_string(), semi(11.5), INK);
            let rect = Rect::from_min_size(Pos2::new(x, inner.top()), Vec2::new(label.size().x + 5.0 + number.size().x + 18.0, 24.0));
            let response = ui.interact(rect, ui.scope_id().with(("github_filter", filter.key())), Sense::click()).on_hover_cursor(CursorIcon::PointingHand);
            let (ground, ink, count_ink) = match (on, filter == PrFilter::NeedsYou && count > 0) {
                (true, _) => (Color32::from_rgb(0xe8, 0xe9, 0xec), Color32::from_rgb(0x11, 0x11, 0x11), Color32::from_rgb(0x11, 0x11, 0x11)),
                (false, alert) => (if response.hovered() { SEG_ON } else { HOVER }, DIM, if alert { RED } else { INK }),
            };
            ui.painter().rect_filled(rect, 12, ground);
            let words_at = Pos2::new(rect.left() + 9.0, rect.center().y);
            let words = ui.painter().text(words_at, egui::Align2::LEFT_CENTER, filter.label(), font(11.5), ink);
            ui.painter().text(Pos2::new(words.right() + 5.0, rect.center().y), egui::Align2::LEFT_CENTER, count.to_string(), semi(11.5), count_ink);
            if response.clicked() {
                state.filter = filter;
            }
            x = rect.right() + 6.0;
        }
        let foot_y = inner.bottom() - 8.0;
        let rows = Rect::from_min_max(Pos2::new(card.left() + 6.0, inner.top() + 30.0), Pos2::new(card.right() - 6.0, foot_y - 14.0));
        if list.is_empty() {
            ui.painter().text(rows.center(), egui::Align2::CENTER_CENTER, "Nenhum pull request neste filtro", font(12.0), DIM_3);
        }
        // Ctrl+↓ and Ctrl+↑ pick the next and the one before, its row scrolled into sight
        let step = ui.input_mut(|input| match (input.consume_key(Modifiers::CTRL, Key::ArrowDown), input.consume_key(Modifiers::CTRL, Key::ArrowUp)) {
            (true, _) => 1,
            (_, true) => -1,
            _ => 0,
        });
        let mut reveal = None;
        if step != 0 && !list.is_empty() {
            let at = shown(list, state.picked.as_deref()).and_then(|request| list.iter().position(|other| other.id == request.id)).unwrap_or(0);
            let next = (at as isize + step).clamp(0, list.len() as isize - 1) as usize;
            (state.picked, state.activity) = (Some(list[next].id.clone()), false);
            reveal = state.picked.clone();
        }
        let shown_id = shown(list, state.picked.as_deref()).map(|request| request.id.clone()).filter(|_| !state.activity);
        let now = now_ms();
        ui.scope_builder(egui::UiBuilder::new().max_rect(rows), |ui| {
            ui.spacing_mut().item_spacing = Vec2::new(0.0, 2.0);
            ui.spacing_mut().scroll = crate::app::ui::scroll_style();
            egui::ScrollArea::vertical().id_salt("island_github_list").auto_shrink([false, false]).show(ui, |ui| {
                for request in list {
                    let (rect, response) = ui.allocate_exact_size(Vec2::new(ui.available_width(), ROW_H), Sense::click());
                    let response = response.on_hover_cursor(CursorIcon::PointingHand);
                    if reveal.as_deref() == Some(request.id.as_str()) {
                        response.scroll_to_me(None);
                    }
                    if response.hovered() || shown_id.as_deref() == Some(request.id.as_str()) {
                        ui.painter().rect_filled(rect, 10, HOVER);
                    }
                    let (icon, color) = state_icon(request);
                    icons::paint(ui.painter(), icon, Pos2::new(rect.left() + 8.0 + 10.0, rect.center().y), 18.0, color);
                    // at the end: the CI, then what asks for the person
                    let mut marks_left = rect.right() - 8.0;
                    if let Some((mark, color)) = ci_mark(request.ci) {
                        let at = Pos2::new(marks_left - 9.0, rect.center().y);
                        ui.painter().circle_filled(at, 9.0, tinted(color, 0.12));
                        ui.painter().text(at, egui::Align2::CENTER_CENTER, mark, semi(10.0), color);
                        marks_left -= 22.0;
                    }
                    if request.review_requested_of_me && request.is_open() {
                        marks_left = tag(ui.painter(), marks_left, rect.center().y, "revisão", YELLOW) - 4.0;
                    }
                    if request.conflicts() && request.is_open() {
                        marks_left = tag(ui.painter(), marks_left, rect.center().y, "conflito", ORANGE) - 4.0;
                    }
                    let text_x = rect.left() + 8.0 + 20.0 + 9.0;
                    let width = (marks_left - 5.0 - text_x).max(20.0);
                    let title = clipped(ui.painter(), request.title.clone(), semi(12.5), INK, width);
                    ui.painter().galley(Pos2::new(text_x, rect.top() + 4.0), title, INK);
                    let meta = clipped(ui.painter(), request.subtitle(now), font(11.0), DIM_3, width);
                    ui.painter().galley(Pos2::new(text_x, rect.top() + 21.0), meta, DIM_3);
                    if response.clicked() {
                        (state.picked, state.activity) = (Some(request.id.clone()), false);
                    }
                }
            });
        });
        // the last week at the end, and before it the main branch of each repository
        self.github_spark(ui, inner.right() - 8.0, foot_y, state);
        if pulse.main.is_empty() {
            return;
        }
        let mut x = inner.left() + 8.0;
        let said = ui.painter().text(Pos2::new(x, foot_y), egui::Align2::LEFT_CENTER, "Branch principal:", font(11.0), DIM_3);
        x = said.right() + 14.0;
        for branch in &pulse.main {
            if x > inner.right() - 60.0 - activity::SPARK_W {
                break;
            }
            let color = ci_mark(branch.ci).map_or(DIM_3, |(_, color)| color);
            let name = branch.repo.rsplit('/').next().unwrap_or(&branch.repo);
            let room = inner.right() - x - 12.0 - activity::SPARK_W - 16.0;
            let words = clipped(ui.painter(), format!("{name} {}", ci_words(branch.ci)), font(11.0), DIM, room);
            let width = words.size().x;
            // a click opens its Actions, where a red branch is explained
            let rect = Rect::from_min_size(Pos2::new(x - 3.0, foot_y - 9.0), Vec2::new(12.0 + width + 6.0, 18.0));
            let response = ui
                .interact(rect, ui.scope_id().with(("github_main", &branch.repo)), Sense::click())
                .on_hover_cursor(CursorIcon::PointingHand)
                .on_hover_text(format!("Abrir as Actions de {} ({})", branch.repo, branch.branch));
            if response.clicked() {
                self.integrations.pressed.borrow_mut().push(format!("open {}", branch.actions_page()));
            }
            let ink = if response.hovered() { INK } else { DIM };
            ui.painter().circle_filled(Pos2::new(x + 3.5, foot_y), 3.5, color);
            ui.painter().galley_with_override_text_color(Pos2::new(x + 12.0, foot_y - words.size().y / 2.0), words, ink);
            x += 12.0 + width + 14.0;
        }
    }

    /// The pull request picked: its title, what holds it back, the description or a comment, the buttons.
    fn github_side(&mut self, ui: &mut egui::Ui, card: Rect, request: &PullRequest, colors: &UiColors, state: &mut Page) {
        let inner = card.shrink2(Vec2::new(14.0, 12.0));
        let id = request.id.clone();
        // the detail is read once the pull request shows
        if self.github_detail(&id).is_none() && !self.github_detail_loading(&id) && self.github_detail_error(&id).is_none() {
            let _ = self.github_load_detail(&id);
        }
        let detail = self.github_detail(&id).cloned();
        if let Some(detail) = detail.as_ref().filter(|_| state.draft_of != id) {
            (state.draft, state.draft_of, state.refused) = (detail.body.clone(), id.clone(), String::new());
        }
        let (icon, color) = state_icon(request);
        icons::paint(ui.painter(), icon, Pos2::new(inner.left() + 8.0, inner.top() + 9.0), 16.0, color);
        let text_x = inner.left() + 24.0;
        let title = clipped(ui.painter(), request.title.clone(), semi(13.0), INK, inner.right() - text_x);
        ui.painter().galley(Pos2::new(text_x, inner.top()), title, INK);
        let mut meta = format!("{} #{}", request.repo(), request.number());
        if !request.head_ref.is_empty() && !request.base_ref.is_empty() {
            meta += &format!(" · {} → {}", request.head_ref, request.base_ref);
        }
        let meta = clipped(ui.painter(), meta, font(11.0), DIM_3, inner.right() - text_x);
        ui.painter().galley(Pos2::new(text_x, inner.top() + 18.0), meta, DIM_3);
        // what holds it back, each line to its page
        let mut y = inner.top() + 40.0;
        let blockers: Vec<(String, Option<String>)> = match &detail {
            Some(detail) => detail.blockers().into_iter().map(|blocker| (blocker.text, blocker.url)).collect(),
            None if request.conflicts() => vec![(format!("Conflito com a {}", if request.base_ref.is_empty() { "base" } else { &request.base_ref }), None)],
            None => Vec::new(),
        };
        let lines: Vec<(String, Option<String>, Color32, &str)> = if blockers.is_empty() {
            let (words, color, mark) = match request.state {
                PrState::Merged => (request.state_words(), PURPLE, "✓"),
                PrState::Closed => (request.state_words(), RED, "✕"),
                PrState::Open if request.ci == CiState::Pending => ("Checks rodando".to_string(), AMBER, "●"),
                PrState::Open => ("Nada travando".to_string(), GREEN, "✓"),
            };
            vec![(words, None, color, mark)]
        } else {
            blockers
                .into_iter()
                .map(|(text, url)| {
                    let (color, mark) = if text.starts_with("Conflito") {
                        (ORANGE, "!")
                    } else if text.starts_with("Mudanças") {
                        (YELLOW, "±")
                    } else {
                        (RED, "✕")
                    };
                    (text, url, color, mark)
                })
                .collect()
        };
        for (text, url, color, mark) in lines.into_iter().take(3) {
            let row = Rect::from_min_size(Pos2::new(inner.left(), y), Vec2::new(inner.width(), 18.0));
            let hovered = url.as_ref().is_some_and(|url| {
                let response = ui.interact(row, ui.scope_id().with(("github_blocker", &text)), Sense::click()).on_hover_cursor(CursorIcon::PointingHand);
                if response.clicked() {
                    self.integrations.pressed.borrow_mut().push(format!("open {url}"));
                }
                response.on_hover_text("Abrir no navegador").hovered()
            });
            let at = Pos2::new(row.left() + 8.0, row.center().y);
            ui.painter().circle_filled(at, 7.0, tinted(color, 0.16));
            ui.painter().text(at, egui::Align2::CENTER_CENTER, mark, semi(9.0), color);
            let words = clipped(ui.painter(), text, font(11.5), if hovered { INK } else { DIM }, row.width() - 24.0);
            ui.painter().galley(Pos2::new(row.left() + 24.0, row.center().y - words.size().y / 2.0), words, if hovered { INK } else { DIM });
            y += 22.0;
        }
        // Descrição or Comentar at the left, Escrever or Prévia at the right
        let seg_y = y + 4.0;
        if let (Some(index), _) = segment(ui, Pos2::new(inner.left(), seg_y), ["Descrição", "Comentar"], usize::from(state.comment), "github_mode") {
            state.comment = index == 1;
        }
        let modes = ["Escrever", "Prévia"];
        let width = modes.iter().map(|word| ui.painter().layout_no_wrap(word.to_string(), font(11.0), DIM).size().x + 20.0).sum::<f32>() + 4.0;
        if let (Some(index), _) = segment(ui, Pos2::new(inner.right() - width, seg_y), modes, usize::from(!state.writing), "github_view") {
            state.writing = index == 0;
        }
        // the buttons, and over them what the last press said
        let buttons_y = inner.bottom() - 26.0;
        let said = match (self.github_writing(), self.github_last_write(), state.refused.is_empty()) {
            (_, _, false) => Some((state.refused.clone(), RED)),
            (true, ..) => Some(("Enviando ao GitHub…".to_string(), DIM)),
            (false, Some(Ok(done)), _) => Some((done.clone(), GREEN)),
            (false, Some(Err(error)), _) => Some((error.clone(), RED)),
            _ => None,
        };
        let box_bottom = buttons_y - if said.is_some() { 26.0 } else { 8.0 };
        if let Some((words, color)) = said {
            let galley = clipped(ui.painter(), words, font(11.0), color, inner.width());
            ui.painter().galley(Pos2::new(inner.left(), buttons_y - 19.0), galley, color);
        }
        let area = Rect::from_min_max(Pos2::new(inner.left(), seg_y + 32.0), Pos2::new(inner.right(), box_bottom));
        ui.painter().rect(area, 12, CARD_FLAT, Stroke::new(1.0, Color32::from_white_alpha(12)), StrokeKind::Inside);
        let text_area = area.shrink2(Vec2::new(11.0, 8.0));
        let loading = detail.is_none();
        ui.scope_builder(egui::UiBuilder::new().max_rect(text_area), |ui| {
            ui.spacing_mut().scroll = crate::app::ui::scroll_style();
            egui::ScrollArea::vertical().id_salt(("island_github_text", &id, state.comment, state.writing)).auto_shrink([false, false]).show(ui, |ui| {
                let width = text_area.width() - 8.0;
                let text = if state.comment { &mut state.note } else { &mut state.draft };
                match (!state.writing, loading && !state.comment) {
                    (_, true) => {
                        let words = self.github_detail_error(&id).map_or("Lendo a descrição…".to_string(), str::to_string);
                        ui.label(egui::RichText::new(words).font(font(11.5)).color(DIM_3));
                    }
                    (true, _) if text.trim().is_empty() => {
                        let words = if state.comment { "Nada escrito ainda." } else { "Sem descrição." };
                        ui.label(egui::RichText::new(words).font(font(11.5)).color(DIM_3));
                    }
                    (true, _) => crate::app::chat::markdown::prose_with(ui, text, colors, true, width, false, &mut |_, _| {}),
                    (false, _) => {
                        let hint = if state.comment { "Escreva um comentário…" } else { "Escreva a descrição em Markdown…" };
                        ui.add(
                            egui::TextEdit::multiline(text)
                                .id(egui::Id::unique(("island_github_edit", &id, state.comment)))
                                .frame(egui::Frame::NONE)
                                .font(font(11.5))
                                .text_color(INK)
                                .desired_width(width)
                                .desired_rows(4)
                                .hint_text(egui::RichText::new(hint).font(font(11.5)).color(DIM_3)),
                        );
                    }
                }
            });
        });
        let mut x = inner.left();
        let open =
            forms::chip(ui, Pos2::new(x, buttons_y), "Abrir no navegador", true).on_hover_text("Ctrl+O · Ctrl+↓ e Ctrl+↑ passam de um pull request a outro");
        if open.clicked() {
            self.integrations.pressed.borrow_mut().push(format!("open {}", request.page()));
        }
        x = open.rect.right() + 6.0;
        let checks = forms::chip(ui, Pos2::new(x, buttons_y), "Ver checks", false);
        if checks.clicked() {
            self.integrations.pressed.borrow_mut().push(format!("open {}", request.checks_page()));
        }
        x = checks.rect.right() + 6.0;
        let changed = detail.as_ref().is_some_and(|detail| detail.body != state.draft);
        if !state.comment && changed && forms::chip(ui, Pos2::new(x, buttons_y), "Salvar", false).clicked() {
            state.refused = self.github_save_body(&id, &state.draft).err().unwrap_or_default();
        }
        if state.comment && !state.note.trim().is_empty() && forms::chip(ui, Pos2::new(x, buttons_y), "Comentar", false).clicked() {
            match self.github_comment(&id, state.note.trim()) {
                Ok(()) => state.note.clear(),
                Err(error) => state.refused = error,
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn request(id: &str, state: PrState, draft: bool) -> PullRequest {
        PullRequest { id: id.into(), title: id.into(), state, draft, ..Default::default() }
    }

    #[test]
    fn each_state_has_its_icon_and_colour() {
        assert_eq!(state_icon(&request("a#1", PrState::Open, false)).1, GREEN);
        assert_eq!(state_icon(&request("a#2", PrState::Open, true)).1, GRAY, "a draft");
        assert_eq!(state_icon(&request("a#3", PrState::Merged, false)).1, PURPLE);
        assert_eq!(state_icon(&request("a#4", PrState::Closed, false)).1, RED);
        assert_eq!(ci_mark(CiState::Failure), Some(("✕", RED)));
        assert_eq!(ci_mark(CiState::Unknown), None, "no checks: no mark");
    }

    #[test]
    fn the_one_picked_shows_while_the_filter_keeps_it_else_the_first() {
        let (first, second) = (request("a#1", PrState::Open, false), request("a#2", PrState::Open, false));
        let list = vec![&first, &second];
        assert_eq!(shown(&list, Some("a#2")).map(|request| request.id.as_str()), Some("a#2"));
        assert_eq!(shown(&list, Some("a#9")).map(|request| request.id.as_str()), Some("a#1"), "filtered away: the first");
        assert_eq!(shown(&list, None).map(|request| request.id.as_str()), Some("a#1"));
        assert!(shown(&[], Some("a#1")).is_none());
    }
}
