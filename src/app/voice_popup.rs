//! The call's popup: a small window of its own above everything, from the moment the call starts.
//! The chat in a popup of its own (the pencil), the microphone, the agent's figure (Claude Code's mascot, else
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
        let chat_popup = self.chat_popup;
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
                        PopupClick::Chat if chat_popup => "Fechar o chat",
                        PopupClick::Chat => "Abrir o chat num popup",
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
                        crate::app::mascot::paint_mascot(p, figure, d * 1.3, state, now);
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
        #[cfg(windows)]
        if !self.voice.popup_plain {
            self.voice.popup_plain = plain::strip("Chat por voz");
        }
        // where it was left, for the next call too
        if let Some(pos) = moved.filter(|p| self.prefs.call_popup != Some([p.x, p.y])) {
            self.prefs.call_popup = Some([pos.x, pos.y]);
        }
        ctx.request_repaint_after(std::time::Duration::from_millis(33));
        if let Some(click) = click {
            self.voice_popup_click(click);
        }
    }

    pub(crate) fn voice_popup_click(&mut self, click: PopupClick) {
        match click {
            PopupClick::Chat => self.chat_popup = !self.chat_popup,
            PopupClick::Mute => self.voice_mute(),
            PopupClick::Figure => self.voice_toggle(),
            PopupClick::Speakers => self.voice_speakers(),
        }
    }

    /// The call's chat in a window of its own, above whatever the person is in: what was said and
    /// understood, and the box to write in. Its own close button closes it.
    pub(crate) fn chat_popup_ui(&mut self, ctx: &Context) {
        if !self.chat_popup {
            return;
        }
        let builder =
            egui::ViewportBuilder::default().with_title("Chat").with_always_on_top().with_inner_size([440.0, 640.0]).with_min_inner_size([360.0, 420.0]);
        let close = ctx.show_viewport_immediate(egui::ViewportId::from_hash_of("chat_popup"), builder, |ui, _| {
            // every window clears see-through during a call (clear_color): this one's ground is solid
            ui.painter().rect_filled(ui.max_rect(), 0, ui.ctx().global_style().visuals.window_fill());
            // the chat screen draws only while the chat is open: open here, whatever the app shows
            let shown = std::mem::replace(&mut self.chat_open, true);
            self.chat_screen(ui.ctx());
            self.chat_open = shown;
            ui.ctx().input(|i| i.viewport().close_requested())
        });
        if close {
            self.chat_popup = false;
        }
    }
}

/// Windows 11 draws a border and rounds the corners of every window, the popup's too: a square
/// around its disc and pill. Its window goes without them.
#[cfg(windows)]
mod plain {
    #[link(name = "user32")]
    unsafe extern "system" {
        fn FindWindowW(class: *const u16, title: *const u16) -> isize;
        fn GetWindowThreadProcessId(hwnd: isize, pid: *mut u32) -> u32;
    }
    #[link(name = "dwmapi")]
    unsafe extern "system" {
        fn DwmSetWindowAttribute(hwnd: isize, attribute: u32, value: *const u32, size: u32) -> i32;
    }
    const DWMWA_WINDOW_CORNER_PREFERENCE: u32 = 33;
    const DWMWCP_DONOTROUND: u32 = 1;
    const DWMWA_BORDER_COLOR: u32 = 34;
    const DWMWA_COLOR_NONE: u32 = 0xFFFF_FFFE;

    /// True once this app's window called `title` is found and stripped.
    pub(super) fn strip(title: &str) -> bool {
        let wide: Vec<u16> = title.encode_utf16().chain([0]).collect();
        // SAFETY: a nul-terminated title; the window, if any, is checked to be this process's
        unsafe {
            let hwnd = FindWindowW(std::ptr::null(), wide.as_ptr());
            let mut pid = 0;
            if hwnd == 0 || GetWindowThreadProcessId(hwnd, &mut pid) == 0 || pid != std::process::id() {
                return false;
            }
            DwmSetWindowAttribute(hwnd, DWMWA_WINDOW_CORNER_PREFERENCE, &DWMWCP_DONOTROUND, 4);
            DwmSetWindowAttribute(hwnd, DWMWA_BORDER_COLOR, &DWMWA_COLOR_NONE, 4);
        }
        true
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
        app.voice_popup_click(PopupClick::Mute);
        assert!(app.voice.shared.muted.load(Ordering::Relaxed));
        app.voice_popup_click(PopupClick::Speakers);
        assert!(app.voice.speakers_off);
        app.voice_popup_click(PopupClick::Chat);
        assert!(app.chat_popup, "the pencil opens the chat in its popup");
        let mut out = ctx.run_ui(egui::RawInput::default(), |ui| app.chat_popup_ui(ui.ctx()));
        out.textures_delta.clear();
        assert!(!app.chat_open, "the app keeps showing what it showed");
        app.voice_popup_click(PopupClick::Chat);
        assert!(!app.chat_popup, "and closes it");
        app.voice_popup_click(PopupClick::Figure);
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
        // see-through around its rounded disc and pill
        assert_eq!(eframe::App::clear_color(&app, &egui::Visuals::dark()), [0.0; 4]);
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
        // the popup's figure: the mascot's squares or the orb's mesh of rings
        let figure = |app: &mut DrawingApp| {
            let mut output = ctx.run_ui(egui::RawInput::default(), |ui| app.voice_popup(ui.ctx()));
            output.textures_delta.clear();
            // the mascot is its drawing's squares, a couple of pixels each
            let squares = output.shapes.iter().filter(|s| matches!(&s.shape, Shape::Rect(r) if r.rect.width() < 4.0)).count();
            (squares > 100, output.shapes.iter().any(|s| matches!(&s.shape, Shape::Mesh(m) if m.vertices.len() > 1000)))
        };
        figure(&mut app);
        assert_eq!(figure(&mut app), (true, false), "Claude Code: the mascot, no orb");
        app.scene.elements.iter_mut().find(|e| e.id == id).unwrap().terminal.as_mut().unwrap().command = "codex".into();
        assert_eq!(figure(&mut app), (false, true), "Codex: the orb");
    }
}
