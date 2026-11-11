//! A terminal's card on the canvas: the title strip (the mark, the name, its state), the grid in
//! the terminal's own theme and font size, Desfocar's veil and the folder strip with the processes
//! button; and the parts over it that take clicks (that button, the veil's, the search bar and the
//! processes popover).

use crate::app::terminal_procs::{paint_procs_button, procs_button};
use crate::app::ui::menu_look::{MenuIcon, paint_menu_icon_sized};
use crate::prelude::*;

/// The reference app's title strip: 8.125 in from the edge, 4.875 between its parts.
const HEAD_PAD: f32 = 8.125;
const HEAD_GAP: f32 = 4.875;
/// Its name is `text-xs` of a 13px rem.
const NAME_SIZE: f32 = 9.75;
/// The veil's eye button, and its eye.
const VEIL_BUTTON: f32 = 44.0;

impl DrawingApp {
    pub(crate) fn paint_terminal(&self, painter: &Painter, element: &Element, rect: Rect, zoom: f32) {
        let focused = self.terminal_focus.as_ref() == Some(&element.id);
        // being typed into shows as the still dashed outline around it (paint.rs), not here
        let radius = (8.0 * zoom).min(255.0) as u8;
        let spec = element.terminal.clone().unwrap_or_default();
        let (look, font) = self.terminal_look(&spec);
        let outline = if look == Look::default() { Color32::from_gray(0x3a) } else { look.fg.gamma_multiply(0.2) };
        painter.rect(rect, radius, look.bg, Stroke::new(1.0, outline), StrokeKind::Inside);
        let painter = painter.with_clip_rect(rect.shrink(1.0).intersect(painter.clip_rect()));
        let (session, error) = match self.terminals.get(&element.id) {
            Some(Ok(session)) => (Some(session), None),
            Some(Err(error)) => (None, Some(error.clone())),
            None => (None, None),
        };
        let (strip, muted, light) = (look.strip(), look.muted(), look.fg);
        // the title strip: the mark, the name; at the right what state it is in (and the way back when enlarged)
        let header = Rect::from_min_size(rect.min, Vec2::new(rect.width(), TERMINAL_HEADER * zoom));
        painter.rect_filled(header, egui::CornerRadius { nw: radius, ne: radius, sw: 0, se: 0 }, strip);
        painter.hline(header.x_range(), header.bottom() - 0.5, Stroke::new(1.0, look.rule()));
        use crate::app::ui::agents::AgentState;
        let enlarged = self.terminal_elevated.as_ref() == Some(&element.id);
        let state = session.map(|_| self.agent_state(&element.id));
        let attention = !focused && (matches!(state, Some(AgentState::Waiting)) || self.terminal_unread.contains(&element.id));
        let dot = match state {
            _ if attention => Some(Color32::from_rgb(0xff, 0x45, 0x3a)),
            Some(AgentState::Ended) => Some(Color32::from_rgb(0xe0, 0x31, 0x31)),
            Some(AgentState::Working(_) | AgentState::Active(_)) => Some(PRIMARY),
            Some(AgentState::Waiting) => Some(crate::app::ui::agents::AMBER),
            Some(AgentState::Idle(_)) => Some(Color32::from_rgb(0x2f, 0x9e, 0x44)),
            None => None,
        };
        let right = header.right() - if enlarged { 34.0 * zoom } else { 0.0 };
        if let Some(dot) = dot {
            painter.circle_filled(Pos2::new(right - (HEAD_PAD + 3.25) * zoom, header.center().y), 3.25 * zoom, dot);
        }
        let mut x = header.left() + HEAD_PAD * zoom;
        if zoom >= 0.6 {
            let tint = spec.color.map_or(light, |[r, g, b]| Color32::from_rgb(r, g, b));
            terminal_icon(&painter, &spec, Pos2::new(x + 5.5 * zoom, header.center().y), tint);
        }
        x += (11.0 + HEAD_GAP) * zoom;
        let name = painter.text(Pos2::new(x, header.center().y), egui::Align2::LEFT_CENTER, spec.label(), FontId::proportional(NAME_SIZE * zoom), light);
        if spec.maestro && zoom >= 0.6 {
            maestro_crown(&painter, name.right_center() + Vec2::new(HEAD_GAP + 5.0, 0.0) * zoom, zoom, crate::app::ui::agents::AMBER);
        }
        if enlarged {
            paint_icon(&painter, Icon::Expand, Self::restore_button(rect).center(), light);
        }
        if let Some(session) = session {
            let marks = self.find_marks(&element.id);
            let view = GridView { look: &look, font, marks: &marks };
            session.paint(&painter, rect.min + Vec2::new(TERMINAL_PAD, TERMINAL_HEADER + 4.0) * zoom, zoom, focused, &view);
        }
        let footer = Rect::from_min_max(Pos2::new(rect.left(), rect.bottom() - TERMINAL_FOOTER * zoom), rect.max);
        if spec.look.blurred {
            // Desfocar: the screen goes behind a veil of the terminal's color, with an eye to lift it
            let body = Rect::from_min_max(header.left_bottom(), footer.right_top());
            painter.rect_filled(body, 0.0, look.bg);
            paint_menu_icon_sized(&painter, MenuIcon::EyeOff, body.center(), look.fg.gamma_multiply(0.25), 22.0 * zoom);
        }
        // the folder strip: where it runs, and what to do when the shell is gone
        painter.rect_filled(footer, egui::CornerRadius { nw: 0, ne: 0, sw: radius, se: radius }, strip);
        painter.hline(footer.x_range(), footer.top() + 0.5, Stroke::new(1.0, look.rule()));
        let small = FontId::monospace(10.0 * zoom);
        painter.text(footer.left_center() + Vec2::new(HEAD_PAD * zoom, 0.0), egui::Align2::LEFT_CENTER, display_folder(&spec.cwd), small.clone(), muted);
        let note = match (session, error) {
            (_, Some(error)) => Some(format!("Não foi possível abrir o shell: {error}")),
            (Some(s), _) if s.exited() => Some("Processo encerrado — clique para reiniciar".into()),
            (None, None) if spec.unloaded => Some("Este terminal está descarregado.".into()),
            _ => None,
        };
        if let Some(note) = note {
            let button = procs_button(footer, zoom);
            painter.text(Pos2::new(button.left() - HEAD_GAP * zoom, footer.center().y), egui::Align2::RIGHT_CENTER, note, small, light);
        }
        if session.is_some() {
            paint_procs_button(&painter, footer, zoom, muted);
        }
    }

    /// What lies over the terminals and takes clicks: the search bar and the processes popover of
    /// the terminals that have them, and on the one the bar is about (focused, enlarged or
    /// selected) the processes button and the veil's eye. Also sends to the clipboard what an
    /// action queued.
    pub(crate) fn terminal_extras(&mut self, ctx: &Context) {
        self.flush_copy_out(ctx);
        let card = |app: &Self, id: &str| {
            let e = app.scene.elements.iter().find(|e| e.id == id && e.terminal.is_some())?;
            let (rect, zoom) = app.terminal_frame(e);
            Some((e.clone(), rect, zoom))
        };
        let footer_of = |rect: Rect, zoom: f32| Rect::from_min_max(Pos2::new(rect.left(), rect.bottom() - TERMINAL_FOOTER * zoom), rect.max);
        if let Some(id) = self.term_ui.find.as_ref().map(|f| f.id.clone()) {
            match card(self, &id) {
                // another window is enlarged over it: the bar waits under it, as a portal's does
                Some((e, ..)) if self.lifted_other(&e) => {}
                Some((_, rect, zoom)) => self.find_ui(ctx, rect, zoom),
                None => self.close_find(),
            }
        }
        if let Some(id) = self.term_ui.procs.as_ref().map(|p| p.id.clone()) {
            match card(self, &id) {
                Some((e, rect, zoom)) if !self.lifted_other(&e) => self.procs_ui(ctx, procs_button(footer_of(rect, zoom), zoom)),
                // its terminal is gone, or another window was enlarged over it (by an agent too, with no press to close it)
                _ => self.term_ui.procs = None,
            }
        }
        let Some((element, rect, zoom)) = self.active_node().map(|e| e.id.clone()).and_then(|id| card(self, &id)) else { return };
        if zoom < 0.5 || self.lifted_other(&element) {
            return;
        }
        let spec = element.terminal.clone().unwrap_or_default();
        let live = matches!(self.terminals.get(&element.id), Some(Ok(_)));
        let footer = footer_of(rect, zoom);
        let (mut procs, mut unblur) = (false, false);
        if live {
            let button = procs_button(footer, zoom);
            egui::Area::new(egui::Id::unique("terminal_procs_button")).fixed_pos(button.min).order(egui::Order::Middle).show(ctx, |ui| {
                let (_, response) = ui.allocate_exact_size(button.size(), Sense::click());
                procs = response.on_hover_cursor(CursorIcon::PointingHand).tip("Processos").clicked();
            });
        }
        if spec.look.blurred {
            let body = Rect::from_min_max(Pos2::new(rect.left(), rect.top() + TERMINAL_HEADER * zoom), footer.right_top());
            let button = Rect::from_center_size(body.center(), Vec2::splat(VEIL_BUTTON * zoom));
            egui::Area::new(egui::Id::unique("terminal_veil_button")).fixed_pos(button.min).order(egui::Order::Middle).show(ctx, |ui| {
                let (_, response) = ui.allocate_exact_size(button.size(), Sense::click());
                unblur = response.on_hover_cursor(CursorIcon::PointingHand).tip("Remover Desfoque").clicked();
            });
        }
        if procs {
            self.toggle_procs(&element.id);
        }
        if unblur && let Some(i) = self.terminal_index(&element.id) {
            self.set_blurred(i, false);
        }
    }
}
