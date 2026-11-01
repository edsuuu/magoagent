//! The call's popup: a small window of its own above everything, from the moment the call starts.
//! Back to the chat (the pencil), the microphone, the agent's figure (Claude Code's mascot, else
//! the orb; hovered, the X that ends the call) and the speakers. Dragged by anywhere, it stays
//! where it is left, this call and the next.

use crate::app::voice_orb::{DOT, launch_size, paint_orb};
use crate::prelude::*;
use std::sync::atomic::Ordering;

pub(crate) const POPUP_SIZE: Vec2 = Vec2::new(236.0, 60.0);
const FILL: Color32 = Color32::from_rgb(0x26, 0x26, 0x28);
const EDGE: Color32 = Color32::from_rgb(0x3a, 0x3a, 0x3d);
const HOVER: Color32 = Color32::from_rgb(0x3a, 0x3a, 0x3d);
const INK: Color32 = Color32::from_rgb(0xd8, 0xd8, 0xd8);
/// The orb's diameter in the popup, once open.
const FIGURE: f32 = 40.0;

#[derive(Clone, Copy, Debug, Hash, PartialEq, Eq)]
pub(crate) enum PopupClick {
    Chat,
    Mute,
    Figure,
    Speakers,
}

/// The buttons in the popup `rect`: the pencil in its own disc, then the pill with the microphone,
/// the figure and the speakers.
pub(crate) fn popup_buttons(rect: Rect) -> [(PopupClick, Rect); 4] {
    let at = |x: f32, d: f32| Rect::from_center_size(Pos2::new(rect.left() + x, rect.center().y), Vec2::splat(d));
    [(PopupClick::Chat, at(30.0, 48.0)), (PopupClick::Mute, at(96.0, 36.0)), (PopupClick::Figure, at(149.0, 44.0)), (PopupClick::Speakers, at(202.0, 36.0))]
}

fn pill(rect: Rect) -> Rect {
    Rect::from_min_max(Pos2::new(rect.left() + 66.0, rect.center().y - 26.0), Pos2::new(rect.right() - 4.0, rect.center().y + 26.0))
}

/// A pencil on a page: write in the chat.
fn paint_pencil(p: &Painter, c: Pos2, ink: Color32) {
    let s = Stroke::new(1.6, ink);
    let page = vec![c + Vec2::new(1.0, -7.0), c + Vec2::new(-7.0, -7.0), c + Vec2::new(-7.0, 7.0), c + Vec2::new(7.0, 7.0), c + Vec2::new(7.0, -1.0)];
    p.add(Shape::line(page, s));
    p.line_segment([c + Vec2::new(-2.0, 2.0), c + Vec2::new(7.0, -7.0)], Stroke::new(2.4, ink));
}

impl DrawingApp {
    /// Each frame of a call: the popup's own window, where it was left (the first time, over the
    /// app's box).
    pub(crate) fn voice_popup(&mut self, ctx: &Context) {
        if !self.voice.on {
            return;
        }
        let at = *self.voice.popup_at.get_or_insert_with(|| match self.prefs.call_popup {
            Some([x, y]) => Pos2::new(x, y),
            None => {
                let outer = ctx.input(|i| i.viewport().outer_rect).unwrap_or(Rect::from_min_size(Pos2::ZERO, Vec2::new(1280.0, 800.0)));
                Pos2::new(outer.center().x - POPUP_SIZE.x / 2.0, outer.bottom() - POPUP_SIZE.y - 110.0)
            }
        });
        let builder = egui::ViewportBuilder::default()
            .with_title("Chat por voz")
            .with_transparent(true)
            .with_decorations(false)
            .with_always_on_top()
            .with_taskbar(false)
            .with_resizable(false)
            .with_active(false)
            .with_inner_size(POPUP_SIZE)
            .with_position(at);
        let muted = self.voice.shared.muted.load(Ordering::Relaxed);
        let speakers_off = self.voice.speakers_off;
        let mascot = self.voice_name() == "claude";
        let (state, now) = (self.avatar.state, self.now);
        let since = self.voice.connected_at.map(|at| (now - at) as f32);
        let speak = if self.voice.state == VoiceState::Speaking { 1.0 } else { 0.0 };
        let mic = f32::from_bits(self.voice.shared.level.load(Ordering::Relaxed)).min(1.0);
        let (click, moved) = ctx.show_viewport_immediate(egui::ViewportId::from_hash_of("voice_popup"), builder, |ui, _| {
            let rect = ui.max_rect();
            // dragged by anywhere: the system moves the window
            if ui.interact(rect, ui.scope_id().with("popup_drag"), Sense::drag()).drag_started() {
                ui.ctx().send_viewport_cmd(egui::ViewportCommand::StartDrag);
            }
            let buttons = popup_buttons(rect);
            let hovered: Vec<(PopupClick, bool, bool)> = buttons
                .iter()
                .map(|(click, r)| {
                    let tip = match click {
                        PopupClick::Chat => "Voltar ao chat",
                        PopupClick::Mute if muted => "Ativar microfone",
                        PopupClick::Mute => "Silenciar microfone",
                        PopupClick::Figure => "Encerrar chat por voz",
                        PopupClick::Speakers if speakers_off => "Ativar som dos alto-falantes",
                        PopupClick::Speakers => "Silenciar alto-falantes",
                    };
                    let r = ui.interact(*r, ui.scope_id().with(click), Sense::click()).on_hover_cursor(CursorIcon::PointingHand).on_hover_text(tip);
                    (*click, r.hovered(), r.clicked())
                })
                .collect();
            let p = ui.painter();
            let [chat, mute, figure, speakers] = buttons.map(|(_, r)| r.center());
            p.circle(chat, 24.0, FILL, Stroke::new(1.0, EDGE));
            p.rect(pill(rect), 26.0, FILL, Stroke::new(1.0, EDGE), StrokeKind::Inside);
            let over = |k: usize| hovered[k].1;
            if over(0) {
                p.circle_filled(chat, 23.0, HOVER);
            }
            paint_pencil(p, chat, INK);
            if muted || over(1) {
                p.circle_filled(mute, 16.0, if muted { crate::app::chat_list::RED.gamma_multiply(0.85) } else { HOVER });
            }
            crate::app::voice::paint_mic(p, mute, if muted { Color32::WHITE } else { INK }, muted);
            if speakers_off || over(3) {
                p.circle_filled(speakers, 16.0, HOVER);
            }
            crate::app::voice::paint_speaker(p, speakers, INK, speakers_off);
            // the figure: a dot breathing while it connects, then opening with the orb's spring
            match since {
                None => {
                    let breath = 1.0 + 0.25 * (0.5 - 0.5 * (now as f32 * std::f32::consts::TAU / 1.25).cos());
                    p.circle_filled(figure, DOT / 2.0 * breath, INK);
                }
                Some(a) => {
                    let d = launch_size(a, FIGURE);
                    if mascot {
                        crate::app::avatar::paint_mascot(p, figure, d * 1.3, state, now);
                    } else {
                        paint_orb(p, figure, d / 2.0, now, speak, mic, 1.0);
                    }
                }
            }
            if over(2) {
                p.circle_filled(figure, 18.0, Color32::from_black_alpha(110));
                for d in [Vec2::new(5.0, 5.0), Vec2::new(5.0, -5.0)] {
                    p.line_segment([figure - d, figure + d], Stroke::new(2.0, Color32::WHITE));
                }
            }
            let moved = ui.ctx().input(|i| i.viewport().outer_rect).map(|r| r.min);
            (hovered.iter().find(|h| h.2).map(|h| h.0), moved)
        });
        // where it was left, for the next call too
        if let Some(pos) = moved.filter(|p| self.prefs.call_popup != Some([p.x, p.y])) {
            self.prefs.call_popup = Some([pos.x, pos.y]);
        }
        ctx.request_repaint_after(std::time::Duration::from_millis(33));
        if let Some(click) = click {
            self.voice_popup_click(ctx, click);
        }
    }

    pub(crate) fn voice_popup_click(&mut self, ctx: &Context, click: PopupClick) {
        match click {
            PopupClick::Chat => {
                self.show_chat();
                ctx.send_viewport_cmd_to(egui::ViewportId::ROOT, egui::ViewportCommand::Minimized(false));
                ctx.send_viewport_cmd_to(egui::ViewportId::ROOT, egui::ViewportCommand::Focus);
            }
            PopupClick::Mute => self.voice_mute(),
            PopupClick::Figure => self.voice_toggle(),
            PopupClick::Speakers => self.voice_speakers(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_popup_has_its_four_buttons_apart_and_the_figure_ends_the_call() {
        let rect = Rect::from_min_size(Pos2::ZERO, POPUP_SIZE);
        let buttons = popup_buttons(rect);
        for (i, (_, a)) in buttons.iter().enumerate() {
            assert!(rect.contains_rect(*a), "{a:?}");
            assert!(buttons[i + 1..].iter().all(|(_, b)| !a.intersects(*b)), "{a:?}");
        }
        assert!(buttons[1..].iter().all(|(_, b)| pill(rect).contains_rect(*b)), "the three in the pill");
        let ctx = Context::default();
        let mut app = DrawingApp::default();
        app.voice.on = true;
        app.voice.state = VoiceState::Listening;
        app.voice_popup_click(&ctx, PopupClick::Mute);
        assert!(app.voice.shared.muted.load(Ordering::Relaxed));
        app.voice_popup_click(&ctx, PopupClick::Speakers);
        assert!(app.voice.speakers_off);
        app.chat_open = false;
        app.voice_popup_click(&ctx, PopupClick::Chat);
        assert!(app.chat_open);
        app.voice_popup_click(&ctx, PopupClick::Figure);
        assert!(!app.voice.on, "the X ends the call");
    }

    #[test]
    fn the_popup_shows_while_the_call_is_on_and_stays_where_it_was_left() {
        let ctx = Context::default();
        let mut app = DrawingApp::default();
        app.prefs.call_popup = Some([500.0, 300.0]);
        let frame = |app: &mut DrawingApp| ctx.run_ui(egui::RawInput::default(), |ui| app.voice_popup(ui.ctx())).textures_delta.clear();
        frame(&mut app);
        assert_eq!(app.voice.popup_at, None, "no call, no popup");
        app.voice.on = true;
        frame(&mut app);
        assert_eq!(app.voice.popup_at, Some(Pos2::new(500.0, 300.0)));
        app.voice.connected_at = Some(app.now - 5.0);
        frame(&mut app);
    }

    #[test]
    fn claude_codes_call_has_its_mascot_and_codexs_the_orb() {
        let ctx = Context::default();
        let mut app = DrawingApp::default();
        let mut e = app.new_element(ElementKind::Rectangle { x: 0.0, y: 0.0, width: 400.0, height: 300.0 });
        e.terminal = Some(TerminalSpec { name: "Revisor".into(), ..Default::default() });
        let id = e.id.clone();
        app.scene.elements.push(e);
        app.chat_agent = Some(id.clone());
        app.voice.on = true;
        app.voice.state = VoiceState::Listening;
        app.now = 10.0;
        app.voice.connected_at = Some(5.0);
        // the popup's figure: a picture (the mascot) or the orb's mesh of rings
        let figure = |app: &mut DrawingApp| {
            let mut output = ctx.run_ui(egui::RawInput::default(), |ui| app.voice_popup(ui.ctx()));
            output.textures_delta.clear();
            let meshes: Vec<_> = output.shapes.iter().filter_map(|s| if let Shape::Mesh(m) = &s.shape { Some(m.clone()) } else { None }).collect();
            (
                meshes.iter().any(|m| m.texture_id != egui::TextureId::default() && m.calc_bounds().width() >= 48.0),
                meshes.iter().any(|m| m.vertices.len() > 1000),
            )
        };
        figure(&mut app);
        assert_eq!(figure(&mut app), (true, false), "Claude Code: the mascot, no orb");
        app.scene.elements.iter_mut().find(|e| e.id == id).unwrap().terminal.as_mut().unwrap().command = "codex".into();
        assert_eq!(figure(&mut app), (false, true), "Codex: the orb");
    }
}
