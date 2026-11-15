//! The island's "+" page, as the reference's launch card: the wizard at the left over an indigo wash, the project and
//! what the agent should do; sent, a new chat starts in a workspace of its own (optionally in a git worktree) with those
//! words. The project drops a list of the recent ones down from its button, over the text box. Also the island's small
//! round buttons.

use super::NewForm;
use super::paint::{DIM, DIM_3, INK, Out, View, aim, font, wash_with};
use crate::app::chat::direct::folder_label;
use crate::app::chat::list::clipped;
use crate::prelude::*;

const CARD: Color32 = Color32::from_rgb(0x14, 0x15, 0x18);
const INDIGO: Color32 = Color32::from_rgb(0x63, 0x66, 0xf1);
const PICKED: Color32 = Color32::from_rgb(0x81, 0x8c, 0xf8);
const FAILED: Color32 = Color32::from_rgb(0xf4, 0x50, 0x5e);
/// The projects kept, most recent first (the reference's `MAX_PROJECTS`).
pub(crate) const MOST_PROJECTS: usize = 10;

/// A root ("C:\", "/") or a home folder: never a project.
fn root_or_home(path: &str) -> bool {
    let path = path.trim().trim_end_matches(['\\', '/']);
    let parts: Vec<&str> = path.split(['\\', '/']).filter(|part| !part.is_empty()).collect();
    match parts.as_slice() {
        [] => true,
        [drive] => drive.ends_with(':') || *drive == "root",
        [first, _] => first.eq_ignore_ascii_case("home") || first.eq_ignore_ascii_case("Users"),
        [drive, users, _] => drive.ends_with(':') && users.eq_ignore_ascii_case("Users"),
        _ => false,
    }
}

/// `folder` at the front of `list`, once (letter case ignored), ten at most; a root or a home folder leaves it as it is.
pub(crate) fn remember_project(folder: &str, list: &[String]) -> Vec<String> {
    let folder = folder.trim().trim_end_matches(['\\', '/']).to_string();
    if folder.is_empty() || root_or_home(&folder) {
        return list.to_vec();
    }
    let others = list.iter().filter(|other| !other.eq_ignore_ascii_case(&folder)).cloned();
    std::iter::once(folder.clone()).chain(others).take(MOST_PROJECTS).collect()
}

/// The page: the card, the wizard, the title and the project, then the text box (the projects drop over it). Enter or the
/// round button sends it (`out.create`); "Escolher uma pasta…" asks the system (`out.pick_folder`).
pub(super) fn new_chat_page(ui: &mut egui::Ui, page: Rect, form: &mut Option<NewForm>, view: &View, out: &mut Out) {
    let Some(form) = form.as_mut() else { return };
    ui.painter().rect(page, 20, CARD, Stroke::new(1.0, Color32::from_white_alpha(9)), StrokeKind::Inside);
    wash_with(&ui.painter().with_clip_rect(page), page, INDIGO, 0.5, 0.0);
    aim(view, Pos2::new(page.left() + 48.0, page.center().y), 52.0, MagoState::Idle, 0.0);
    // the reference's .launch-body: 98 from the card's left, 16 from its right, its rows 8 apart and centred
    let body = Rect::from_min_max(Pos2::new(page.left() + 98.0, page.top() + 4.0), Pos2::new(page.right() - 16.0, page.bottom() - 4.0));
    // the same height with the projects open: they drop over the text box, nothing above them moves
    let height = 22.0 + 8.0 + 40.0 + 8.0 + 14.0;
    let top = body.center().y - height / 2.0;
    let head = Rect::from_min_size(Pos2::new(body.left(), top), Vec2::new(body.width(), 22.0));
    // the project at the right of the title, the worktree before it
    let label = if form.folder.is_empty() { "Escolha um projeto".to_string() } else { folder_label(&form.folder) };
    let name = clipped(ui.painter(), label, font(11.0), INK, 160.0);
    let project = Rect::from_min_size(Pos2::new(head.right() - name.size().x - 34.0, head.top()), Vec2::new(name.size().x + 34.0, 22.0));
    let response = ui.interact(project, ui.scope_id().with("launch_project"), Sense::click()).on_hover_cursor(CursorIcon::PointingHand);
    let response = if form.folder.is_empty() { response } else { response.on_hover_text(&form.folder) };
    if response.hovered() || form.choosing {
        ui.painter().rect_filled(project, 6, Color32::from_white_alpha(26));
    }
    folder_icon(ui.painter(), Pos2::new(project.left() + 11.0, project.center().y), INK);
    ui.painter().galley(Pos2::new(project.left() + 19.0, project.center().y - name.size().y / 2.0), name, INK);
    chevron(ui.painter(), Pos2::new(project.right() - 9.0, project.center().y), INK);
    if response.clicked() {
        form.choosing = !form.choosing;
    }
    let worktree = Rect::from_min_size(Pos2::new(project.left() - 82.0, head.top()), Vec2::new(76.0, 22.0));
    let response = ui.interact(worktree, ui.scope_id().with("launch_worktree"), Sense::click()).on_hover_cursor(CursorIcon::PointingHand);
    let response = response.on_hover_text("Num worktree próprio (git): uma cópia do repositório, sem mexer nas alterações atuais.");
    if response.clicked() {
        form.worktree = !form.worktree;
    }
    let (ground, ink) = if form.worktree {
        (PICKED.gamma_multiply(0.22), PICKED)
    } else if response.hovered() {
        (Color32::from_white_alpha(26), INK)
    } else {
        (Color32::TRANSPARENT, DIM)
    };
    ui.painter().rect_filled(worktree, 6, ground);
    ui.painter().text(worktree.center(), egui::Align2::CENTER_CENTER, "worktree", font(11.0), ink);
    let title = clipped(ui.painter(), "Nova sessão de agente".to_string(), font(14.0), INK, worktree.left() - head.left() - 8.0);
    ui.painter().galley(Pos2::new(head.left(), head.center().y - title.size().y / 2.0), title, INK);
    let below = head.bottom() + 8.0;
    // the reference's .chat-bar: the words and the round button that sends them
    let bar = Rect::from_min_size(Pos2::new(body.left(), below), Vec2::new(body.width(), 40.0));
    ui.painter().rect_filled(bar, 12, Color32::from_white_alpha(18));
    let send = Rect::from_center_size(Pos2::new(bar.right() - 10.0 - 14.0, bar.center().y), Vec2::splat(28.0));
    let field = Rect::from_min_max(Pos2::new(bar.left() + 10.0, bar.top() + 6.0), Pos2::new(send.left() - 8.0, bar.bottom() - 6.0));
    let edit = ui.put(
        field,
        egui::TextEdit::singleline(&mut form.prompt)
            .id(ui.scope_id().with("launch_prompt"))
            .hint_text(egui::RichText::new("O que o agente deve fazer?").color(DIM_3))
            .frame(egui::Frame::NONE)
            .font(font(13.0))
            .text_color(INK)
            .align(egui::Align2::LEFT_CENTER),
    );
    let entered = edit.lost_focus() && ui.input(|input| input.key_pressed(egui::Key::Enter));
    let ready = !form.folder.is_empty();
    let response = ui.interact(send, ui.scope_id().with("launch_send"), Sense::click()).on_hover_cursor(CursorIcon::PointingHand);
    let scale = if response.is_pointer_button_down_on() { 0.92 } else { 1.0 };
    ui.painter().circle_filled(send.center(), 14.0 * scale, INK.gamma_multiply(if ready { 1.0 } else { 0.4 }));
    arrow_up(ui.painter(), send.center(), Color32::from_rgb(0x0b, 0x0c, 0x0e));
    if response.clicked() || entered {
        if ready {
            out.create = true;
        } else {
            out.pick_folder = true;
        }
    }
    let (hint, color) = match &form.failed {
        Some(why) => (why.clone(), FAILED),
        None => ("Abre um chat novo no projeto, num espaço próprio, com as suas palavras.".to_string(), DIM),
    };
    let hint = clipped(ui.painter(), hint, font(11.0), color, body.width());
    ui.painter().galley(Pos2::new(body.left(), bar.bottom() + 8.0), hint, color);
    // the projects drop down from their button, over the text box, in 0.16 s; a click outside folds them
    let shown = ui.ctx().animate_bool_with_time(ui.scope_id().with("launch_projects_drop"), form.choosing, 0.16);
    if shown > 0.0 {
        let rows = form.projects.len() as f32;
        let full = 12.0 + rows * 26.0 + if rows > 0.0 { 5.0 } else { 0.0 } + 24.0;
        let width = 220.0_f32.min(body.width());
        let full = Rect::from_min_size(
            Pos2::new(project.right() - width, project.bottom() + 4.0),
            Vec2::new(width, full.min(page.bottom() - 6.0 - project.bottom() - 4.0)),
        );
        let eased = egui::emath::easing::cubic_out(shown);
        let drop = Rect::from_min_size(full.min, Vec2::new(full.width(), full.height() * eased));
        projects_menu(ui, full, drop, form, out);
        let outside = ui.input(|input| input.pointer.any_click() && input.pointer.interact_pos().is_some_and(|at| !full.contains(at) && !project.contains(at)));
        if outside {
            form.choosing = false;
        }
    }
}

/// The recent projects in a panel at `full` (seen as far as `drop`, while it drops), the one chosen ticked, and
/// "Escolher uma pasta…". The panel takes the clicks over it, so none reaches the text box under it.
fn projects_menu(ui: &mut egui::Ui, full: Rect, drop: Rect, form: &mut NewForm, out: &mut Out) {
    ui.interact(drop, ui.scope_id().with("launch_projects_panel"), Sense::click());
    ui.painter().rect(drop, 12, Color32::from_rgb(0x16, 0x17, 0x1b), Stroke::new(1.0, Color32::from_white_alpha(15)), StrokeKind::Inside);
    let area = full.shrink(6.0);
    ui.scope_builder(egui::UiBuilder::new().max_rect(area), |ui| {
        ui.set_clip_rect(drop.shrink(1.0).intersect(ui.clip_rect()));
        ui.spacing_mut().scroll = egui::style::ScrollStyle::floating();
        egui::ScrollArea::vertical().id_salt("launch_projects").max_height(area.height()).show(ui, |ui| {
            ui.spacing_mut().item_spacing.y = 2.0;
            let row = |ui: &mut egui::Ui, text: String, on: bool| {
                let (rect, response) = ui.allocate_exact_size(Vec2::new(area.width() - 6.0, 24.0), Sense::click());
                let response = response.on_hover_cursor(CursorIcon::PointingHand);
                if on {
                    ui.painter().rect_filled(rect, 6, PICKED.gamma_multiply(0.1));
                } else if response.hovered() {
                    ui.painter().rect_filled(rect, 6, Color32::from_white_alpha(14));
                }
                let ink = if on { PICKED } else { Color32::from_rgb(0xc8, 0xcd, 0xd4) };
                let name = clipped(ui.painter(), text, font(12.0), ink, rect.width() - 36.0);
                ui.painter().galley(Pos2::new(rect.left() + 8.0, rect.center().y - name.size().y / 2.0), name, ink);
                if on {
                    check(ui.painter(), Pos2::new(rect.right() - 14.0, rect.center().y), ink);
                }
                response
            };
            for path in form.projects.clone() {
                let on = path.eq_ignore_ascii_case(&form.folder);
                if row(ui, folder_label(&path), on).on_hover_text(&path).clicked() {
                    (form.folder, form.choosing) = (path, false);
                }
            }
            if !form.projects.is_empty() {
                let (rule, _) = ui.allocate_exact_size(Vec2::new(area.width() - 6.0, 1.0), Sense::hover());
                ui.painter().rect_filled(rule, 0, Color32::from_white_alpha(20));
            }
            if row(ui, "Escolher uma pasta…".to_string(), false).clicked() {
                (out.pick_folder, form.choosing) = (true, false);
            }
        });
    });
}

fn folder_icon(painter: &Painter, center: Pos2, ink: Color32) {
    let stroke = Stroke::new(1.2, ink);
    let body = Rect::from_center_size(center + Vec2::new(0.0, 0.5), Vec2::new(10.0, 7.0));
    painter.rect_stroke(body, 1.5, stroke, StrokeKind::Middle);
    painter.line_segment([body.left_top() + Vec2::new(0.5, 0.0), body.left_top() + Vec2::new(0.5, -1.8)], stroke);
    painter.line_segment([body.left_top() + Vec2::new(0.5, -1.8), body.left_top() + Vec2::new(4.0, -1.8)], stroke);
}

fn chevron(painter: &Painter, center: Pos2, ink: Color32) {
    let stroke = Stroke::new(1.6, ink);
    painter.line_segment([center + Vec2::new(-3.0, -1.5), center + Vec2::new(0.0, 1.5)], stroke);
    painter.line_segment([center + Vec2::new(0.0, 1.5), center + Vec2::new(3.0, -1.5)], stroke);
}

fn arrow_up(painter: &Painter, center: Pos2, ink: Color32) {
    let stroke = Stroke::new(1.8, ink);
    painter.line_segment([center + Vec2::new(0.0, 5.0), center + Vec2::new(0.0, -5.0)], stroke);
    painter.line_segment([center + Vec2::new(-4.0, -1.0), center + Vec2::new(0.0, -5.0)], stroke);
    painter.line_segment([center + Vec2::new(4.0, -1.0), center + Vec2::new(0.0, -5.0)], stroke);
}

fn check(painter: &Painter, center: Pos2, ink: Color32) {
    let stroke = Stroke::new(1.6, ink);
    painter.line_segment([center + Vec2::new(-4.0, 0.0), center + Vec2::new(-1.0, 3.0)], stroke);
    painter.line_segment([center + Vec2::new(-1.0, 3.0), center + Vec2::new(4.5, -3.5)], stroke);
}

/// A small round button at `at` (its top left); `strong` is the one that does it. Allocates its room.
pub(super) fn chip(ui: &mut egui::Ui, at: Pos2, text: &str, strong: bool) -> egui::Response {
    let ink = if strong { Color32::BLACK } else { INK };
    let galley = ui.painter().layout_no_wrap(text.to_string(), font(11.5), ink);
    let rect = Rect::from_min_size(at, Vec2::new(galley.size().x + 22.0, 26.0));
    let response = ui.interact(rect, ui.scope_id().with(("island_chip", text)), Sense::click()).on_hover_cursor(CursorIcon::PointingHand);
    let ground = match (strong, response.hovered()) {
        (true, hovered) => Color32::from_rgb(0xf5, 0xf6, 0xf8).gamma_multiply(if hovered { 1.0 } else { 0.92 }),
        (false, true) => Color32::from_white_alpha(34),
        (false, false) => Color32::from_white_alpha(18),
    };
    ui.painter().rect_filled(rect, 13, ground);
    ui.painter().galley(Pos2::new(rect.center().x - galley.size().x / 2.0, rect.center().y - galley.size().y / 2.0), galley, ink);
    ui.advance_cursor_after_rect(rect);
    response
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn projects_are_the_recent_folders_never_a_root_nor_a_home_folder() {
        let list = remember_project(r"C:\code\loja", &[]);
        let list = remember_project(r"C:\code\site\", &list);
        assert_eq!(list, [r"C:\code\site", r"C:\code\loja"], "the latest in front, without the trailing separator");
        assert_eq!(remember_project(r"c:\CODE\loja", &list), [r"c:\CODE\loja", r"C:\code\site"], "once, whatever the case");
        for not_a_project in [r"C:\", "C:", "/", r"C:\Users\edsu", "/home/edsu", "/Users/edsu", "/root", ""] {
            assert_eq!(remember_project(not_a_project, &list), list, "{not_a_project}");
        }
        let many = (0..15).fold(Vec::new(), |list, index| remember_project(&format!("/code/{index}"), &list));
        assert_eq!(many.len(), MOST_PROJECTS);
        assert_eq!(many[0], "/code/14");
    }
}
