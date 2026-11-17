//! Home: the agent in focus in a card at the left (the mascot big) and the grid of
//! agents at the right. What the card says follows what the agent does (`stage.rs`): the tool it is
//! on with a glimpse of the file or the command's output, an edit's change in a card of its own, how a turn ended, and
//! the wizard's dizziness when it is poked too much.

use super::marquee::marquee;
use super::paint::{AMBER, CARD, CARD_FLAT, DIM, DIM_3, INK, Out, PILL_H, RED, View, aim, font, mago, pill, semi, wash, wash_with};
use super::stage::{Activity, Phase};
use super::{Line, Tab, entry, forms, main_state};
use crate::app::chat::list::clipped;
use crate::app::mascot::mago::MagoState;
use crate::prelude::*;

const GREEN: Color32 = Color32::from_rgb(0x34, 0xd3, 0x99);
const RED_TEXT: Color32 = Color32::from_rgb(0xff, 0x8d, 0x97);
/// The page's height when an edit takes the card over (the change needs room).
pub(super) const EDIT_H: f32 = 290.0;
/// How many lines of the change show per second as it appears.
const LINES_PER_SECOND: f32 = 9.0;
const ROW: f32 = 18.0;

fn hairline() -> Stroke {
    Stroke::new(1.0, Color32::from_white_alpha(9))
}

/// What an empty page says: its title and what to do, by the tab; Claude Code's knows whether its hooks are in
/// (asked: "se os hooks estão instalados aqui precisa saber").
pub(crate) fn empty_words(tab: Tab, hooks: bool) -> (&'static str, &'static str) {
    match tab {
        Tab::Claude if hooks => ("Nenhuma sessão do Claude Code", "Os hooks estão instalados: as sessões aparecem aqui quando começarem"),
        Tab::Claude => ("Nenhuma sessão do Claude Code", "Instale os hooks em Ajustes › Integrações"),
        Tab::GitHub => ("O GitHub está desligado", "Ligue em Ajustes › Integrações"),
        Tab::Music => ("Nada tocando agora", "Ligue o Spotify em Ajustes › Integrações"),
        _ => ("Nenhum agente ainda", "Crie um chat em  +"),
    }
}

/// Whether Claude Code's settings have our hooks (any version), read again every 3 s while asked: it is a file.
fn hooks_installed(ui: &egui::Ui, time: f64) -> bool {
    let id = egui::Id::unique("island_claude_hooks_installed");
    if let Some((at, installed)) = ui.ctx().data(|data| data.get_temp::<(f64, bool)>(id))
        && (0.0..3.0).contains(&(time - at))
    {
        return installed;
    }
    let installed = crate::app::maestro::claude_hooks::settings_path().is_some_and(|path| crate::app::maestro::claude_hooks::installed_at(&path));
    ui.ctx().data_mut(|data| data.insert_temp(id, (time, installed)));
    installed
}

/// The two cards the page is made of, by what the agent does.
fn halves(page: Rect, editing: bool) -> (Rect, Rect) {
    let width = if editing { 150.0 } else { 322.0 };
    let left = Rect::from_min_size(page.min, Vec2::new(width, page.height()));
    (left, Rect::from_min_max(Pos2::new(left.right() + 10.0, page.top()), page.max))
}

impl DrawingApp {
    /// The Home page inside `page` of the island `rect`: the entrance first, then the cards.
    pub(super) fn island_home(&self, ui: &mut egui::Ui, rect: Rect, page: Rect, view: &View, time: f64, out: &mut Out) {
        let editing = view.activity.as_ref().is_some_and(|activity| activity.phase == Phase::Editing);
        let (left, right) = halves(page, editing);
        let focus = view.focus.as_ref().and_then(|id| view.lines.iter().find(|line| &line.id == id));
        let state = if view.dizzy { MagoState::Idle } else { focus.map_or(main_state(view.mood, view.celebrating), |line| line.mago) };
        let home = if editing { Pos2::new(left.left() + 36.0, left.top() + 40.0) } else { Pos2::new(left.left() + 58.0, left.center().y) };
        let size = if editing { 46.0 } else { 62.0 };
        // the entrance: the wizard drops into the middle of one card; it ends folding to compact, where the island's
        // wizard springs from that spot (`aim` at size 0)
        if let Some(t) = view.entrance {
            let pose = entry::pose(t);
            ui.painter().rect(page, 20, CARD.gamma_multiply(pose.card), hairline(), StrokeKind::Inside);
            if pose.card > 0.0 {
                entry::paint_particles(ui.painter(), rect.min, page, t);
            }
            let at = rect.min + pose.at.to_vec2();
            let waving = if pose.waving { MagoState::Waving } else { state };
            crate::app::mascot::mago::paint_scaled(ui.painter(), at, pose.scale, |painter| {
                mago(painter, view.pointer, "island-main", at, pose.size, waving, time);
            });
            aim(view, rect.min + entry::CENTER + Vec2::new(0.0, entry::SHIFT), 0.0, state);
            return;
        }
        if view.dizzy {
            return self.home_dizzy(ui, page, view);
        }
        self.home_cards(ui, (left, right), view, focus, time, out);
        aim(view, home, size, state);
    }

    /// The focus card (and what is under it: its wash) and the grid of agents.
    fn home_cards(&self, ui: &mut egui::Ui, (left, right): (Rect, Rect), view: &View, focus: Option<&Line>, time: f64, out: &mut Out) {
        // the grid: two columns of pills, a mini wizard and the name, the page's own (`Tab::shows`)
        let grid = view.tab.grid();
        let lines: Vec<&Line> = view.lines.iter().filter(|line| grid.shows(line)).collect();
        // nothing to show: one card across, not an empty one beside it
        let alone = focus.is_none() && lines.is_empty();
        let left = if alone { left.union(right) } else { left };
        for card in if alone { vec![left] } else { vec![left, right] } {
            ui.painter().rect(card, 20, CARD, hairline(), StrokeKind::Inside);
        }
        let activity = view.activity.as_ref();
        let state = focus.map_or(main_state(view.mood, view.celebrating), |line| line.mago);
        wash(&ui.painter().with_clip_rect(left), left, state);
        match (focus, activity) {
            (Some(line), Some(activity)) if activity.phase == Phase::Editing => {
                self.edit_side(ui, left, line, activity, time);
                self.edit_card(ui, right, view, activity);
                return;
            }
            (Some(line), Some(activity)) => self.activity_card(ui, left, line, activity, view),
            (Some(line), None) if self.integration_card(ui, left, line) => {}
            (Some(line), None) => self.idle_card(ui, left, line, out),
            (None, _) => {
                let text_x = left.left() + 106.0;
                let hooks = view.tab == Tab::Claude && hooks_installed(ui, time);
                let (title, hint) = empty_words(view.tab, hooks);
                // on the wizard's middle line, as the wizard
                ui.painter().text(Pos2::new(text_x, left.center().y - 10.0), egui::Align2::LEFT_CENTER, title, font(13.0), INK);
                ui.painter().text(Pos2::new(text_x, left.center().y + 10.0), egui::Align2::LEFT_CENTER, hint, font(11.5), DIM);
            }
        }
        if alone {
            return;
        }
        let inner = right.shrink2(Vec2::new(8.0, 0.0));
        ui.scope_builder(egui::UiBuilder::new().max_rect(inner), |ui| {
            ui.spacing_mut().item_spacing = Vec2::splat(4.0);
            ui.spacing_mut().scroll = crate::app::ui::scroll_style();
            let rows = lines.len().div_ceil(2) as f32;
            let content = rows * PILL_H + (rows - 1.0).max(0.0) * 4.0;
            ui.add_space(((inner.height() - content) / 2.0).max(0.0));
            egui::ScrollArea::vertical().id_salt("island_grid").auto_shrink([false, false]).show(ui, |ui| {
                let width = (inner.width() - 4.0) / 2.0;
                for pair in lines.chunks(2) {
                    ui.horizontal(|ui| {
                        for line in pair {
                            pill(ui, line, width, self.opens_on_click(line, view.tab), time, out);
                        }
                    });
                }
            });
        });
    }

    /// Whether a click on `line`'s pill opens it, as its Abrir: when the person put it in the card already, and on Claude
    /// Code's page for the app's own agents (a session of the canvas) at the first click. Not a tool's: its card has its
    /// own buttons, and its Abrir leaves the app (GitHub's page).
    fn opens_on_click(&self, line: &Line, tab: Tab) -> bool {
        let tool = line.id.starts_with(crate::app::integrations::ITEM_PREFIX);
        let canvas = tab == Tab::Claude && !line.id.starts_with(crate::app::maestro::outside::PILL_PREFIX);
        (canvas || self.island.focus.as_deref() == Some(line.id.as_str())) && !tool
    }

    /// An agent with nothing to say of its turn: its name, its state and the buttons.
    fn idle_card(&self, ui: &mut egui::Ui, left: Rect, line: &Line, out: &mut Out) {
        let text_x = left.left() + 106.0;
        let width = left.right() - text_x - 14.0;
        self.name_row(ui, left, line, None);
        let color = if line.state == crate::app::ui::agents::AgentState::Waiting { AMBER } else { DIM };
        let detail = clipped(ui.painter(), line.detail.clone(), font(11.5), color, width);
        ui.painter().galley(Pos2::new(text_x, left.top() + 36.0), detail, color);
        let (mut x, y) = (text_x, left.bottom() - 38.0);
        let open = forms::chip(ui, Pos2::new(x, y), "Abrir", true);
        if open.clicked() {
            out.open_agent = Some(line.id.clone());
        }
        x = open.rect.right() + 8.0;
        // a chat's Abrir is its conversation here, the same as Conversar: on the canvas the second goes there instead
        if line.chat && line.canvas {
            if forms::chip(ui, Pos2::new(x, y), "No canvas", false).on_hover_text("Abrir o cartão no canvas do app").clicked() {
                out.canvas = Some(line.id.clone());
            }
        } else if line.chat && forms::chip(ui, Pos2::new(x, y), "Conversar", false).clicked() {
            out.talk = Some(line.id.clone());
        }
    }

    /// "● name  Claude Code" and, at the right, the step's place in the turn.
    fn name_row(&self, ui: &mut egui::Ui, card: Rect, line: &Line, place: Option<(usize, usize)>) {
        let text_x = card.left() + 106.0;
        let width = card.right() - text_x - 14.0;
        let y = card.top() + 19.0;
        ui.painter().circle_filled(Pos2::new(text_x + 4.0, y), 4.0, line.color);
        // `.who`: the name 600 12px, the tool 11px
        // cut, the name scrolls under the pointer
        let room = (width - 14.0) * 0.6;
        let hovered = ui.rect_contains_pointer(Rect::from_min_size(Pos2::new(text_x + 14.0, y - 9.0), Vec2::new(room, 18.0)));
        let name_width = marquee(ui, egui::Id::unique(("island_card_name", &line.id)), &line.name, semi(12.0), INK, (text_x + 14.0, y), room, hovered);
        let kind = clipped(ui.painter(), line.kind.clone(), font(11.0), DIM, (width - 14.0 - name_width - 28.0).max(0.0));
        ui.painter().galley(Pos2::new(text_x + 14.0 + name_width + 6.0, y - kind.size().y / 2.0 + 0.5), kind, DIM);
        if let Some((step, total)) = place {
            ui.painter().text(Pos2::new(card.right() - 14.0, y), egui::Align2::RIGHT_CENTER, format!("{step}/{total}"), font(11.0), DIM_3);
        }
    }

    /// The tool in progress, or how the turn ended, with a glimpse under it (the file read, what the command printed).
    fn activity_card(&self, ui: &mut egui::Ui, left: Rect, line: &Line, activity: &Activity, view: &View) {
        self.name_row(ui, left, line, Some(activity.place).filter(|(_, total)| *total > 0));
        let text_x = left.left() + 106.0;
        let width = left.right() - text_x - 14.0;
        let y = left.top() + 40.0;
        let painter = ui.painter();
        let (mark, verb, verb_color) = match activity.phase {
            Phase::Done => (Some((GREEN, true)), "Concluído".to_string(), GREEN),
            Phase::Failed => (Some((RED, false)), "Falhou".to_string(), RED_TEXT),
            _ => (None, activity.verb.clone(), INK),
        };
        let mut x = text_x;
        if let Some((color, ok)) = mark {
            round_mark(painter, Pos2::new(x + 7.0, y), color, ok);
            x += 19.0;
        } else {
            file_mark(painter, Pos2::new(x + 6.0, y));
            x += 19.0;
        }
        let verb = clipped(painter, verb, font(13.0), verb_color, width - 19.0);
        painter.galley(Pos2::new(x, y - verb.size().y / 2.0), verb.clone(), verb_color);
        x += verb.size().x + 8.0;
        let (words, font, color) = match activity.phase {
            Phase::Working => (activity.file.clone().unwrap_or_else(|| activity.target.clone()), FontId::monospace(10.0), DIM),
            _ => (
                activity.headline.clone().or_else(|| activity.diff.map(|(added, removed)| format!("+{added} −{removed}"))).unwrap_or_default(),
                font(11.5),
                DIM,
            ),
        };
        let words = clipped(painter, words, font, color, (left.right() - 14.0 - x).max(0.0));
        painter.galley(Pos2::new(x, y - words.size().y / 2.0), words, color);
        // the glimpse: lines in a box
        let area = Rect::from_min_max(Pos2::new(text_x - 4.0, left.top() + 54.0), Pos2::new(left.right() - 14.0, left.bottom() - 8.0));
        let mut rows: Vec<(Option<usize>, String, Color32)> = Vec::new();
        match activity.phase {
            Phase::Working if activity.verb != "Bash" => rows.extend(view.glimpse.iter().map(|(number, text)| (Some(*number), text.clone(), DIM))),
            _ => {
                if activity.verb == "Bash" && !activity.target.is_empty() {
                    rows.push((None, format!("$ {}", activity.target), INK));
                }
                for text in &activity.output {
                    let lower = text.to_lowercase();
                    let color = if activity.phase == Phase::Failed {
                        RED_TEXT
                    } else if lower.contains("pass") || lower.contains("ok") && lower.contains("test") {
                        GREEN
                    } else {
                        DIM
                    };
                    rows.push((None, text.clone(), color));
                }
            }
        }
        if rows.is_empty() {
            return;
        }
        painter.rect_filled(area, 8, CARD_FLAT);
        let capacity = ((area.height() - 6.0) / 13.0).floor().max(1.0) as usize;
        for (index, (number, text, color)) in rows.into_iter().take(capacity).enumerate() {
            let row_y = area.top() + 8.0 + index as f32 * 13.0;
            let mut at = area.left() + 8.0;
            if let Some(number) = number {
                painter.text(Pos2::new(at, row_y), egui::Align2::LEFT_CENTER, number.to_string(), FontId::monospace(10.0), DIM_3);
                at += 20.0;
            }
            let galley = clipped(painter, text, FontId::monospace(10.5), color, area.right() - 8.0 - at);
            painter.galley(Pos2::new(at, row_y - galley.size().y / 2.0), galley, color);
        }
    }

    /// The editing page's left card: the wizard, the agent and the tools of the turn (done, running, and "Concluído" waiting).
    fn edit_side(&self, ui: &mut egui::Ui, card: Rect, line: &Line, activity: &Activity, time: f64) {
        let painter = ui.painter();
        let x = card.left() + 16.0;
        let name = clipped(painter, line.name.clone(), semi(12.0), INK, card.width() - 32.0);
        let top = card.top() + 82.0;
        painter.galley(Pos2::new(x, top), name, INK);
        let kind = clipped(painter, line.kind.clone(), font(11.0), DIM, card.width() - 32.0);
        painter.galley(Pos2::new(x, top + 18.0), kind, DIM);
        let mut y = top + 50.0;
        for step in activity.moves.iter().map(|step| (step.verb.clone(), step.running, step.failed)).chain([("Concluído".to_string(), false, false)]) {
            let (verb, running, failed) = step;
            let waiting = verb == "Concluído";
            let center = Pos2::new(x + 7.0, y + 7.0);
            if running {
                let turn = time as f32 * 6.0;
                let arc: Vec<Pos2> = (0..14).map(|index| center + Vec2::angled(turn + index as f32 * 0.3) * 5.5).collect();
                painter.add(Shape::line(arc, Stroke::new(1.6, INK)));
            } else if failed {
                round_mark(painter, center, RED, false);
            } else {
                round_mark(painter, center, if waiting { Color32::from_rgb(0x3a, 0x3d, 0x44) } else { Color32::from_rgb(0x45, 0x48, 0x50) }, true);
            }
            let color = if running {
                INK
            } else if failed {
                RED_TEXT
            } else if waiting {
                Color32::from_rgb(0x5f, 0x64, 0x6d)
            } else {
                DIM_3
            };
            let galley = clipped(painter, verb, font(12.5), color, card.width() - 48.0);
            for dx in if running { vec![0.0, 0.5] } else { vec![0.0] } {
                painter.galley(Pos2::new(x + 20.0 + dx, center.y - galley.size().y / 2.0), galley.clone(), color);
            }
            y += 24.0;
        }
    }

    /// The change the agent is making: its file as a tab (a dot while it changes), the path, and the lines of the diff
    /// as they appear.
    fn edit_card(&self, ui: &mut egui::Ui, card: Rect, view: &View, activity: &Activity) {
        let painter = ui.painter();
        let file = activity.file.clone().unwrap_or_else(|| activity.target.clone());
        let name = file.rsplit(['/', '\\']).next().unwrap_or(&file).to_string();
        let tab_label = clipped(painter, name.clone(), font(12.0), INK, 160.0);
        let tab = Rect::from_min_size(Pos2::new(card.left() + 12.0, card.top() + 8.0), Vec2::new(tab_label.size().x + 56.0, 24.0));
        painter.rect_filled(tab, 7, Color32::from_rgb(0x1d, 0x1f, 0x23));
        language_chip(painter, Pos2::new(tab.left() + 14.0, tab.center().y), &name);
        painter.galley(Pos2::new(tab.left() + 26.0, tab.center().y - tab_label.size().y / 2.0), tab_label.clone(), INK);
        painter.circle_filled(Pos2::new(tab.right() - 12.0, tab.center().y), 3.0, AMBER);
        let path = clipped(painter, file.clone(), FontId::monospace(10.5), DIM_3, (card.width() - tab.width() - 44.0).max(40.0));
        painter.galley(Pos2::new(card.right() - 16.0 - path.size().x, card.top() + 20.0 - path.size().y / 2.0), path, DIM_3);
        let area = Rect::from_min_max(card.min + Vec2::new(8.0, 40.0), card.max - Vec2::new(8.0, 8.0));
        painter.rect_filled(area, 12, CARD_FLAT);
        if activity.patch.is_empty() {
            let words = match activity.diff {
                Some((added, removed)) => format!("+{added} −{removed}"),
                None => "Aplicando a mudança…".to_string(),
            };
            painter.text(area.center(), egui::Align2::CENTER_CENTER, words, font(12.0), DIM);
            return;
        }
        let shown = (view.since * LINES_PER_SECOND).ceil() as usize;
        let capacity = ((area.height() - 12.0) / ROW).floor().max(1.0) as usize;
        let mut number = activity.patch_line;
        let clip = painter.with_clip_rect(area);
        for (index, line) in activity.patch.iter().take(shown.min(capacity)).enumerate() {
            let (mark, text) = line.split_at(line.chars().next().map_or(0, char::len_utf8));
            let row = Rect::from_min_size(Pos2::new(area.left(), area.top() + 6.0 + index as f32 * ROW), Vec2::new(area.width(), ROW));
            let (fill, ink) = match mark {
                "+" => (Some(Color32::from_rgba_unmultiplied(0x22, 0xc5, 0x5e, 40)), Color32::from_rgb(0xc4, 0xf1, 0xd6)),
                "-" => (Some(Color32::from_rgba_unmultiplied(0xf4, 0x50, 0x5e, 44)), Color32::from_rgb(0xf0, 0xa6, 0xad)),
                _ => (None, Color32::from_rgb(0xc9, 0xcd, 0xd4)),
            };
            if let Some(fill) = fill {
                clip.rect_filled(row, 0, fill);
            }
            clip.text(Pos2::new(area.left() + 34.0, row.center().y), egui::Align2::RIGHT_CENTER, number.to_string(), FontId::monospace(10.5), DIM_3);
            clip.text(Pos2::new(area.left() + 44.0, row.center().y), egui::Align2::LEFT_CENTER, mark.trim(), FontId::monospace(11.0), ink);
            clip.text(Pos2::new(area.left() + 58.0, row.center().y), egui::Align2::LEFT_CENTER, text, FontId::monospace(11.0), ink);
            if mark != "-" {
                number += 1;
            }
        }
    }

    /// Poked too much: the wizard dizzy at the left and a card washed in red and purple.
    fn home_dizzy(&self, ui: &mut egui::Ui, page: Rect, view: &View) {
        ui.painter().rect(page, 20, CARD, hairline(), StrokeKind::Inside);
        let clip = ui.painter().with_clip_rect(page);
        wash_with(&clip, page, RED, 0.55, 0.0);
        wash_with(&clip, page, Color32::from_rgb(0x63, 0x66, 0xf1), 0.5, -page.width() * 0.22);
        let at = Pos2::new(page.left() + 58.0, page.center().y);
        aim(view, at, 62.0, MagoState::Idle);
        let x = page.left() + 106.0;
        ui.painter().text(Pos2::new(x, page.center().y - 11.0), egui::Align2::LEFT_CENTER, "Muitos toques de uma vez.", font(15.0), INK);
        ui.painter().text(Pos2::new(x, page.center().y + 11.0), egui::Align2::LEFT_CENTER, "Me dá um segundo…", font(12.5), DIM);
    }
}

/// A small disc with a check (done) or a cross (failed).
fn round_mark(painter: &Painter, center: Pos2, color: Color32, ok: bool) {
    painter.circle_filled(center, 6.5, color);
    let stroke = Stroke::new(1.6, if color == RED || color == GREEN { Color32::BLACK } else { CARD });
    if ok {
        painter.add(Shape::line(vec![center + Vec2::new(-3.0, 0.2), center + Vec2::new(-0.8, 2.4), center + Vec2::new(3.2, -2.4)], stroke));
    } else {
        painter.line_segment([center + Vec2::new(-2.5, -2.5), center + Vec2::new(2.5, 2.5)], stroke);
        painter.line_segment([center + Vec2::new(2.5, -2.5), center + Vec2::new(-2.5, 2.5)], stroke);
    }
}

/// A page with a folded corner: what a tool that reads a file shows.
fn file_mark(painter: &Painter, center: Pos2) {
    let rect = Rect::from_center_size(center, Vec2::new(10.0, 13.0));
    painter.rect_stroke(rect, 2, Stroke::new(1.3, INK), StrokeKind::Inside);
    for offset in [-2.0, 1.0] {
        painter.line_segment([center + Vec2::new(-2.5, offset), center + Vec2::new(2.5, offset)], Stroke::new(1.0, INK));
    }
}

/// The little square before a file's name: its language's colour and letters.
fn language_chip(painter: &Painter, center: Pos2, name: &str) {
    let extension = name.rsplit_once('.').map_or("", |(_, extension)| extension).to_lowercase();
    let (color, letters) = match extension.as_str() {
        "ts" | "tsx" => (Color32::from_rgb(0x31, 0x78, 0xc6), "TS"),
        "js" | "jsx" | "mjs" => (Color32::from_rgb(0xc9, 0xa7, 0x1d), "JS"),
        "rs" => (Color32::from_rgb(0xc2, 0x6a, 0x3c), "RS"),
        "py" => (Color32::from_rgb(0x35, 0x72, 0xa5), "PY"),
        "md" => (DIM_3, "MD"),
        "json" => (Color32::from_rgb(0x8a, 0x6d, 0xc9), "{}"),
        _ => (DIM_3, "··"),
    };
    painter.rect_filled(Rect::from_center_size(center, Vec2::splat(12.0)), 3, color);
    painter.text(center, egui::Align2::CENTER_CENTER, letters, font(6.5), Color32::WHITE);
}

/// Whether Home must be tall: an edit is on and its change needs room.
pub(super) fn tall(activity: Option<&Activity>) -> bool {
    activity.is_some_and(|activity| activity.phase == Phase::Editing)
}
