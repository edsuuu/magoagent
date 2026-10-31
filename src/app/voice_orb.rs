//! The call's orb, as on Codex's call: while it connects the conversation gives way to a pulsing
//! dot that opens into the orb with a spring; the cover stays the whole call, the orb in its middle
//! with its tide moving with the voice. A call that can't start says why, with Tentar novamente.

use crate::app::chat_list::Look;
use crate::prelude::*;
use std::sync::atomic::Ordering;

/// The orb's colours, from Codex's call: deep blue on top, lighter toward a white tide below.
const ORB_DEEP: Color32 = Color32::from_rgb(0x27, 0x55, 0xdc);
pub(crate) const ORB_BLUE: Color32 = Color32::from_rgb(0x3a, 0x6e, 0xec);
const ORB_FOAM: Color32 = Color32::from_rgb(0xe9, 0xee, 0xff);
/// The orb (112 across); the connecting dot.
const LAUNCH_RADIUS: f32 = 56.0;
const DOT: f32 = 12.0;
/// The launch, in seconds after the call connects: the dot holds, the orb grows out of it and
/// springs to rest.
const SETTLE: f32 = 0.325;
const GROW: f32 = 0.52;
const REST: f32 = 1.125;

/// A button of the card shown when the call couldn't start.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) enum LaunchClick {
    Settings,
    Retry,
    Back,
}

fn smooth(k: f32) -> f32 {
    let k = k.clamp(0.0, 1.0);
    k * k * (3.0 - 2.0 * k)
}

/// The orb's diameter `a` s after the call connected: the dot's until it grows, then out to `full`
/// with a spring that overshoots about 6% and rests by `REST`.
pub(crate) fn launch_size(a: f32, full: f32) -> f32 {
    if a <= SETTLE {
        return DOT;
    }
    if a <= GROW {
        let k = (a - SETTLE) / (GROW - SETTLE);
        return DOT + (full - DOT) * k * k * (2.0 - k);
    }
    let b = a - GROW;
    full * (1.0 + 0.175 * (-8.94 * b).exp() * (9.64 * b).sin())
}

fn mix(a: Color32, b: Color32, k: f32) -> Color32 {
    let m = |a: u8, b: u8| (a as f32 + (b as f32 - a as f32) * k).round() as u8;
    Color32::from_rgb(m(a.r(), b.r()), m(a.g(), b.g()), m(a.b(), b.b()))
}

/// The orb's colour at (x, y), both -1..1 from its center (y down): blue down to a tide whose
/// waves move with `t`, faster and taller with `speak` (0..1), rippled by the microphone's `mic`.
pub(crate) fn orb_color(x: f32, y: f32, t: f64, speak: f32, mic: f32) -> Color32 {
    let t = (t % 3600.0) as f32;
    let speed = 1.0 + 1.6 * speak;
    let height = 1.0 + 1.5 * speak + 2.0 * mic;
    let tide = 0.42 + height * (0.06 * (x * 2.3 + t * 1.1 * speed).sin() + 0.03 * (x * 5.3 - t * 1.9 * speed).sin());
    mix(mix(ORB_DEEP, ORB_BLUE, smooth((y + 1.0) / (tide + 1.0))), ORB_FOAM, smooth((y - tide + 0.16) / 0.3))
}

/// The orb as a disc of rings, each vertex coloured by `orb_color`, with a feathered rim; `alpha`
/// fades it in.
fn paint_orb(p: &Painter, center: Pos2, radius: f32, t: f64, speak: f32, mic: f32, alpha: f32) {
    const RINGS: u32 = 18;
    const SIDES: u32 = 72;
    let at = |r: f32, k: u32| Vec2::angled(k as f32 / SIDES as f32 * std::f32::consts::TAU) * r;
    let color = |x: f32, y: f32| orb_color(x, y, t, speak, mic).gamma_multiply(alpha);
    let mut mesh = egui::Mesh::default();
    mesh.colored_vertex(center, color(0.0, 0.0));
    for ring in 1..=RINGS {
        for k in 0..SIDES {
            let d = at(ring as f32 / RINGS as f32, k);
            mesh.colored_vertex(center + d * radius, color(d.x, d.y));
        }
    }
    for k in 0..SIDES {
        mesh.colored_vertex(center + at(radius + 1.0, k), Color32::TRANSPARENT);
    }
    let ring = |i: u32, k: u32| 1 + (i - 1) * SIDES + k % SIDES;
    for k in 0..SIDES {
        mesh.add_triangle(0, ring(1, k), ring(1, k + 1));
    }
    for i in 1..=RINGS {
        for k in 0..SIDES {
            mesh.add_triangle(ring(i, k), ring(i, k + 1), ring(i + 1, k));
            mesh.add_triangle(ring(i, k + 1), ring(i + 1, k + 1), ring(i + 1, k));
        }
    }
    p.add(Shape::mesh(mesh));
}

/// Where the system lets the app have the microphone.
fn mic_settings() -> Option<&'static str> {
    if cfg!(windows) {
        Some("ms-settings:privacy-microphone")
    } else if cfg!(target_os = "macos") {
        Some("x-apple.systempreferences:com.apple.preference.security?Privacy_Microphone")
    } else {
        None
    }
}

impl DrawingApp {
    /// The call over the conversation (`cover`), the whole call long: the launch, then the orb at
    /// rest in its middle; or the card of a call that couldn't start.
    pub(crate) fn voice_stage(&self, ui: &egui::Ui, cover: Rect, look: &Look, t: f64) -> Option<LaunchClick> {
        let voice = &self.voice;
        if !voice.on {
            return voice.launch_failed.as_ref().and_then(|(title, text, mic)| launch_card(ui, cover, look, title, text, *mic));
        }
        let p = ui.painter();
        let since = voice.connected_at.map(|at| (self.now - at) as f32);
        ui.interact(cover, ui.scope_id().with("voice_cover"), Sense::click());
        p.rect_filled(cover, 0, look.bg);
        let center = cover.center();
        match since {
            // connecting: a dot breathing to 1.25 and back every 1.25 s
            None => {
                let breath = 1.0 + 0.25 * (0.5 - 0.5 * (t as f32 * std::f32::consts::TAU / 1.25).cos());
                p.circle_filled(center, DOT / 2.0 * breath, look.text);
            }
            Some(a) => {
                let radius = launch_size(a, LAUNCH_RADIUS * 2.0) / 2.0;
                let shown = smooth((a - SETTLE) / (GROW - SETTLE));
                if shown < 1.0 {
                    p.circle_filled(center, radius, look.text.gamma_multiply(1.0 - shown));
                }
                let speak = ui.ctx().animate_bool_with_time(ui.scope_id().with("orb_speak"), voice.state == VoiceState::Speaking, 0.2);
                let mic = f32::from_bits(voice.shared.level.load(Ordering::Relaxed)).min(1.0);
                paint_orb(p, center, radius, t, speak, mic, shown);
            }
        }
        let launching = since.is_none_or(|a| a < REST);
        ui.ctx().request_repaint_after(std::time::Duration::from_millis(if launching { 16 } else { 33 }));
        None
    }

    /// A button of the card of a call that couldn't start.
    pub(crate) fn voice_launch_click(&mut self, click: LaunchClick) {
        if click == LaunchClick::Settings {
            if let Some(url) = mic_settings() {
                crate::app::portal::open_external(url);
            }
            return;
        }
        self.voice.launch_failed = None;
        self.voice.status.clear();
        if click == LaunchClick::Retry {
            self.voice_toggle();
        }
    }
}

/// Why the call couldn't start, over the conversation, with what to do: the system's microphone
/// settings (when it was refused), Tentar novamente, Voltar.
fn launch_card(ui: &egui::Ui, cover: Rect, look: &Look, title: &str, text: &str, mic: bool) -> Option<LaunchClick> {
    ui.interact(cover, ui.scope_id().with("voice_cover"), Sense::click());
    let p = ui.painter();
    p.rect_filled(cover, 0, look.bg);
    let c = cover.center();
    p.text(c - Vec2::new(0.0, 30.0), egui::Align2::CENTER_BOTTOM, title, FontId::proportional(16.0), look.text);
    let galley = p.layout(text.to_string(), FontId::proportional(14.0), look.soft, (cover.width() - 32.0).min(420.0));
    let top = c.y - 24.0;
    p.galley(Pos2::new(c.x - galley.size().x / 2.0, top), galley.clone(), look.soft);
    let mut buttons = vec![];
    if mic && mic_settings().is_some() {
        buttons.push(("Abrir configurações do sistema", LaunchClick::Settings));
    }
    buttons.extend([("Tentar novamente", LaunchClick::Retry), ("Voltar", LaunchClick::Back)]);
    let font = FontId::proportional(14.0);
    let widths: Vec<f32> = buttons.iter().map(|(label, _)| p.layout_no_wrap(label.to_string(), font.clone(), look.text).size().x + 32.0).collect();
    let mut x = c.x - (widths.iter().sum::<f32>() + 8.0 * (widths.len() - 1) as f32) / 2.0;
    let y = top + galley.size().y + 16.0;
    let mut clicked = None;
    for (k, ((label, click), w)) in buttons.into_iter().zip(widths).enumerate() {
        let rect = Rect::from_min_size(Pos2::new(x, y), Vec2::new(w, 36.0));
        let r = ui.interact(rect, ui.scope_id().with(("voice_launch", k)), Sense::click()).on_hover_cursor(CursorIcon::PointingHand);
        // the first is the way out the card suggests
        let (fill, ink) = if k == 0 { (look.text, look.bg) } else { (look.raised, look.text) };
        p.rect_filled(rect, 18, if r.hovered() { fill.gamma_multiply(0.85) } else { fill });
        p.text(rect.center(), egui::Align2::CENTER_CENTER, label, font.clone(), ink);
        if r.clicked() {
            clicked = Some(click);
        }
        x += w + 8.0;
    }
    clicked
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_orb_is_blue_above_and_white_below_and_its_tide_moves_with_the_voice() {
        let (top, bottom) = (orb_color(0.0, -0.9, 0.0, 0.0, 0.0), orb_color(0.0, 0.9, 0.0, 0.0, 0.0));
        assert!(top.b() as u32 > top.r() as u32 * 3 && bottom.r() > 200 && bottom.g() > 200);
        assert_ne!(orb_color(0.3, 0.42, 1.0, 1.0, 0.0), orb_color(0.3, 0.42, 1.0, 0.0, 0.0));
    }

    #[test]
    fn the_dot_opens_into_the_orb_with_a_spring_and_a_failed_start_says_why() {
        assert_eq!(launch_size(0.1, 112.0), DOT);
        assert!((launch_size(GROW, 112.0) - 112.0).abs() < 0.01);
        let peak = (0..60).map(|k| launch_size(GROW + k as f32 * 0.01, 112.0)).fold(0.0, f32::max);
        assert!(peak > 114.0 && peak < 120.0, "{peak}");
        assert!((launch_size(REST, 112.0) - 112.0).abs() < 1.0);
        // the microphone refused while it connects: the card, then Voltar
        let mut app = DrawingApp::default();
        app.voice.on = true;
        app.voice.state = VoiceState::Preparing;
        app.voice_fail(crate::voice::MIC_DENIED.into());
        assert_eq!(app.voice.launch_failed.as_ref().map(|f| (f.0.as_str(), f.2)), Some(("Permissão do microfone negada", true)));
        app.voice_launch_click(LaunchClick::Back);
        assert!(app.voice.launch_failed.is_none() && app.voice.status.is_empty());
        // once on, an error ends the call without the card
        app.voice.on = true;
        app.voice.state = VoiceState::Listening;
        app.voice_fail("Erro no chat por voz: caiu".into());
        assert!(app.voice.launch_failed.is_none() && !app.voice.on);
    }
}
