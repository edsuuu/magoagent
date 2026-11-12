use super::*;

impl DrawingApp {
    /// The round buttons at the box's end, before the send button: the call (bars when off; on,
    /// an X to end it, white while it connects and blue once on, as on Codex's call), and while it
    /// is on the microphone and the speakers. Gives (ligar, mudo, alto-falantes) clicked.
    pub(crate) fn voice_buttons(&self, ui: &mut egui::Ui, right: Pos2, look: &Look, t: f64) -> (bool, bool, bool) {
        let voice = &self.voice;
        let call = Rect::from_center_size(right, Vec2::splat(32.0));
        let call_r = ui.interact(call, ui.scope_id().with("chat_voice"), Sense::click()).on_hover_cursor(CursorIcon::PointingHand);
        let tip = match voice.state {
            VoiceState::Off => "Iniciar chat por voz",
            VoiceState::Preparing => "Cancelar chat por voz",
            _ => "Encerrar chat por voz",
        };
        let call_r = call_r.tip(tip);
        let p = ui.painter();
        let c = call.center();
        let mic = f32::from_bits(voice.shared.level.load(Ordering::Relaxed)).min(1.0);
        if voice.on {
            let preparing = voice.state == VoiceState::Preparing;
            let fill = if preparing { look.raised } else { ORB_BLUE.gamma_multiply(if call_r.hovered() { 0.9 } else { 1.0 }) };
            let ink = if preparing { look.text } else { Color32::WHITE };
            p.circle_filled(c, 16.0, fill);
            if call_r.hovered() && !preparing {
                // under the pointer: the X that ends it
                for d in [Vec2::new(4.5, 4.5), Vec2::new(4.5, -4.5)] {
                    p.line_segment([c - d, c + d], Stroke::new(1.8, ink));
                }
            } else {
                paint_bars(p, c, call_bars(voice.state, mic, t), ink);
            }
            ui.ctx().request_repaint_after(std::time::Duration::from_millis(33));
        } else {
            let hovered = call_r.hovered();
            let (fill, ink) = if hovered { (look.raised, look.text) } else { (Color32::TRANSPARENT, look.soft) };
            p.circle_filled(c, 16.0, fill);
            // five bars: the call; they wave under the pointer
            paint_bars(p, c, if hovered { call_bars(VoiceState::Speaking, 0.0, t) } else { BARS }, ink);
            if hovered {
                ui.ctx().request_repaint_after(std::time::Duration::from_millis(33));
            }
            return (call_r.clicked(), false, false);
        }
        // the speakers: a speaker with its waves, crossed out when silenced
        let speakers = Rect::from_center_size(right - Vec2::new(72.0, 0.0), Vec2::splat(32.0));
        let off = voice.speakers_off;
        let speakers_r = ui.interact(speakers, ui.scope_id().with("chat_speakers"), Sense::click()).on_hover_cursor(CursorIcon::PointingHand).tip(if off {
            "Ativar som dos alto-falantes"
        } else {
            "Silenciar alto-falantes"
        });
        let p = ui.painter();
        if off || speakers_r.hovered() {
            p.circle_filled(speakers.center(), 16.0, look.raised);
        }
        paint_speaker(p, speakers.center(), if off { look.text } else { look.soft }, off);
        let mute = Rect::from_center_size(right - Vec2::new(36.0, 0.0), Vec2::splat(32.0));
        let muted = voice.shared.muted.load(Ordering::Relaxed);
        let mute_r = ui.interact(mute, ui.scope_id().with("chat_mute"), Sense::click()).on_hover_cursor(CursorIcon::PointingHand).tip(if muted {
            "Ativar microfone"
        } else {
            "Silenciar microfone"
        });
        let p = ui.painter();
        if muted || mute_r.hovered() {
            p.circle_filled(mute.center(), 16.0, if muted { crate::app::chat::list::RED.gamma_multiply(0.85) } else { look.raised });
        }
        paint_mic(p, mute.center(), if muted { Color32::WHITE } else { look.soft }, muted);
        (call_r.clicked(), mute_r.clicked(), speakers_r.clicked())
    }
}
