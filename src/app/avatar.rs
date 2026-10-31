//! The avatar: MagoAgent's little wizard, in a window of its own above everything on the screen.
//! It slides to where the action is and shows in a balloon what the Maestro is saying.

use crate::prelude::*;
use crate::transcript::ChatItem;

/// The avatar window, in points: the balloon on the left, the wizard on the right.
pub(crate) const AVATAR_SIZE: Vec2 = Vec2::new(160.0, 120.0);
/// A balloon stays this long (seconds) unless another speech replaces it.
const SAYS_FOR: f64 = 6.0;
/// The glide to a new target takes this long (seconds).
const GLIDE: f32 = 0.35;
/// What the balloon shows of a speech: the first sentence, cut at this many characters.
const SAYS_CHARS: usize = 60;

/// Listening and Muted are the voice's (the Maestro's microphone), not set by the chat yet.
#[cfg_attr(not(test), allow(dead_code))]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) enum AvatarState {
    #[default]
    Idle,
    Listening,
    Thinking,
    Speaking,
    Muted,
}

#[derive(Default)]
pub(crate) struct Avatar {
    /// The window is on the screen (preference on and already placed).
    pub(crate) shown: bool,
    pub(crate) state: AvatarState,
    /// What the balloon says and since when (`now`).
    pub(crate) says: Option<(String, f64)>,
    /// Where the window is, where it left from and where it is going (screen coordinates, top-left).
    pos: Pos2,
    from: Pos2,
    target: Pos2,
    /// Progress of the glide, 0 to 1.
    glide: f32,
    /// The chat it follows and how many of the agent's answers it already said.
    heard: Option<(String, usize)>,
}

impl Avatar {
    /// Moves toward the target, easing out over `GLIDE` seconds; true while still moving.
    pub(crate) fn tick(&mut self, dt: f32) -> bool {
        if self.glide >= 1.0 {
            self.pos = self.target;
            return false;
        }
        self.glide = (self.glide + dt / GLIDE).min(1.0);
        let eased = 1.0 - (1.0 - self.glide).powi(3);
        self.pos = self.from.lerp(self.target, eased);
        self.glide < 1.0
    }

    #[cfg(test)]
    pub(crate) fn pos(&self) -> Pos2 {
        self.pos
    }
}

/// The first sentence of a speech, cut at `SAYS_CHARS`.
pub(crate) fn first_sentence(text: &str) -> String {
    let line = text.lines().map(str::trim).find(|l| !l.is_empty()).unwrap_or_default();
    let end = line.char_indices().find(|(_, c)| matches!(c, '.' | '!' | '?' | ':')).map_or(line.len(), |(i, c)| i + c.len_utf8());
    let sentence = line[..end].trim();
    match sentence.char_indices().nth(SAYS_CHARS) {
        Some((cut, _)) => format!("{}…", sentence[..cut].trim_end()),
        None => sentence.to_string(),
    }
}

impl DrawingApp {
    /// Shows `text` in the balloon for `SAYS_FOR` seconds, or until the next speech.
    pub(crate) fn avatar_say(&mut self, text: impl Into<String>) {
        self.avatar.says = Some((text.into(), self.now));
    }

    /// Where the window goes next (screen coordinates of its top-left corner).
    pub(crate) fn avatar_go_to(&mut self, screen: Pos2) {
        if screen == self.avatar.target {
            return;
        }
        self.avatar.from = self.avatar.pos;
        self.avatar.target = screen;
        self.avatar.glide = 0.0;
    }

    pub(crate) fn avatar_set(&mut self, state: AvatarState) {
        self.avatar.state = state;
    }

    /// From now on the avatar says what the agent of chat `id` answers.
    pub(crate) fn avatar_follow(&mut self, id: &str) {
        self.avatar.heard = Some((id.to_string(), self.agent_answers(id)));
    }

    /// Says the agent's newest answer in the followed chat, when one came since the last look.
    pub(crate) fn avatar_listen(&mut self) {
        let Some((id, heard)) = self.avatar.heard.clone() else { return };
        let answers = self.agent_answers(&id);
        if answers <= heard {
            return;
        }
        let newest = self.transcripts[&id].items.iter().rev().find_map(|i| match i {
            ChatItem::Agent(text) => Some(first_sentence(text)),
            _ => None,
        });
        self.avatar.heard = Some((id, answers));
        if let Some(text) = newest.filter(|t| !t.is_empty()) {
            self.avatar_say(text);
            self.avatar_set(AvatarState::Speaking);
        }
    }

    fn agent_answers(&self, id: &str) -> usize {
        self.transcripts.get(id).map_or(0, |t| t.items.iter().filter(|i| matches!(i, ChatItem::Agent(_))).count())
    }

    /// Where the avatar belongs: by the top-right corner of the window being worked in, else the
    /// bottom-right corner of the screen.
    fn avatar_target(&self, ctx: &Context) -> Pos2 {
        let (outer, monitor) = ctx.input(|i| (i.viewport().outer_rect, i.viewport().monitor_size));
        let origin = outer.map_or(Pos2::ZERO, |r| r.min);
        let focused = self.focused_window().and_then(|id| self.scene.elements.iter().find(|e| &e.id == id));
        if let Some(element) = focused {
            let (rect, _) = self.terminal_frame(element);
            // the wizard, at the window's right, straddles the corner
            return origin + rect.right_top().to_vec2() - Vec2::new(AVATAR_SIZE.x - 36.0, AVATAR_SIZE.y - 40.0);
        }
        let screen = match monitor {
            Some(size) => Rect::from_min_size(Pos2::ZERO, size),
            None => outer.unwrap_or(Rect::from_min_size(Pos2::ZERO, Vec2::new(1280.0, 800.0))),
        };
        screen.max - AVATAR_SIZE - Vec2::new(16.0, 48.0)
    }

    /// Each frame: the avatar's own window, above everything, following the action.
    pub(crate) fn avatar_ui(&mut self, ctx: &Context) {
        if !self.prefs.avatar {
            self.avatar.shown = false;
            return;
        }
        self.avatar_listen();
        if self.avatar.says.as_ref().is_some_and(|(_, since)| self.now - since > SAYS_FOR) {
            self.avatar.says = None;
            if self.avatar.state == AvatarState::Speaking {
                self.avatar.state = AvatarState::Idle;
            }
        }
        let target = self.avatar_target(ctx);
        if self.avatar.shown {
            self.avatar_go_to(target);
        } else {
            // appearing: right where it belongs, no slide from wherever it was
            self.avatar.pos = target;
            self.avatar.target = target;
            self.avatar.glide = 1.0;
            self.avatar.shown = true;
        }
        let moving = self.avatar.tick(ctx.input(|i| i.stable_dt).min(0.05));
        if moving || self.avatar.state != AvatarState::Idle || self.avatar.says.is_some() {
            ctx.request_repaint_after(std::time::Duration::from_millis(40));
        }
        // ponytail: on Linux without a compositor the window comes out opaque; a shaped window
        // (X11 shape extension) would need its own code
        let builder = egui::ViewportBuilder::default()
            .with_title("Avatar")
            .with_transparent(true)
            .with_decorations(false)
            .with_always_on_top()
            .with_mouse_passthrough(true)
            .with_taskbar(false)
            .with_resizable(false)
            .with_active(false)
            .with_inner_size(AVATAR_SIZE)
            .with_position(self.avatar.pos);
        let (state, says, now) = (self.avatar.state, self.avatar.says.clone(), self.now);
        ctx.show_viewport_immediate(egui::ViewportId::from_hash_of("avatar"), builder, |ui, _| {
            paint_avatar(ui.painter(), ui.max_rect(), state, says.as_ref().map(|(t, _)| t.as_str()), now);
        });
    }
}

const ROBE: Color32 = Color32::from_rgb(0x6c, 0x5c, 0xe7);
const ROBE_DARK: Color32 = Color32::from_rgb(0x4b, 0x3f, 0xb3);
const SKIN: Color32 = Color32::from_rgb(0xf5, 0xd0, 0xb0);
const INK: Color32 = Color32::from_rgb(0x2a, 0x24, 0x40);
const STAFF: Color32 = Color32::from_rgb(0x8d, 0x5a, 0x2b);
const ORB: Color32 = Color32::from_rgb(0x7f, 0xdb, 0xff);
const LISTENING: Color32 = Color32::from_rgb(0x3d, 0xd6, 0x8c);
const MUTED: Color32 = Color32::from_rgb(0xe5, 0x48, 0x48);
const BALLOON: Color32 = Color32::from_rgba_premultiplied(0x2a, 0x2a, 0x32, 0xeb);

/// The wizard at the window's right, the balloon at its left, on a see-through background.
pub(crate) fn paint_avatar(painter: &Painter, rect: Rect, state: AvatarState, says: Option<&str>, now: f64) {
    let cx = rect.right() - 36.0;
    let base = rect.bottom() - 6.0;
    let head = Pos2::new(cx, base - 48.0);
    if state == AvatarState::Listening {
        let pulse = 0.5 + 0.5 * (now * 4.0).sin() as f32;
        painter.circle_stroke(Pos2::new(cx, base - 40.0), 34.0, Stroke::new(2.0, LISTENING.gamma_multiply(0.5 + 0.5 * pulse)));
    }
    // staff behind the robe, its orb above the shoulder
    painter.line_segment([Pos2::new(cx + 18.0, base), Pos2::new(cx + 26.0, base - 66.0)], Stroke::new(3.0, STAFF));
    let orb = Pos2::new(cx + 26.0, base - 68.0);
    painter.circle_filled(orb, 5.0, ORB);
    painter.circle_stroke(orb, 5.0, Stroke::new(1.0, ROBE_DARK));
    // robe
    let robe = vec![Pos2::new(cx - 22.0, base), Pos2::new(cx + 22.0, base), Pos2::new(cx + 10.0, base - 44.0), Pos2::new(cx - 10.0, base - 44.0)];
    painter.add(Shape::convex_polygon(robe, ROBE, Stroke::new(1.0, ROBE_DARK)));
    // head, eyes and mouth
    painter.circle_filled(head, 11.0, SKIN);
    let blink = state == AvatarState::Listening && (now * 1.3).fract() < 0.12;
    for dx in [-4.5, 4.5] {
        let eye = Pos2::new(head.x + dx, head.y - 1.0);
        if blink {
            painter.line_segment([eye - Vec2::new(2.0, 0.0), eye + Vec2::new(2.0, 0.0)], Stroke::new(1.5, INK));
        } else {
            painter.circle_filled(eye, 1.8, INK);
        }
    }
    let mouth = Pos2::new(head.x, head.y + 5.5);
    match state {
        AvatarState::Speaking => {
            let open = 1.5 + 2.5 * (now * 14.0).sin().abs() as f32;
            painter.add(egui::epaint::EllipseShape::filled(mouth, Vec2::new(3.0, open), INK));
        }
        _ => {
            painter.line_segment([mouth - Vec2::new(3.0, 0.0), mouth + Vec2::new(3.0, 0.0)], Stroke::new(1.5, INK));
        }
    }
    // hat: brim over the forehead, the cone leaning to the right
    painter.rect_filled(Rect::from_center_size(Pos2::new(cx, head.y - 8.0), Vec2::new(40.0, 6.0)), 3.0, ROBE_DARK);
    let hat = vec![Pos2::new(cx - 15.0, head.y - 10.0), Pos2::new(cx + 15.0, head.y - 10.0), Pos2::new(cx + 6.0, head.y - 42.0)];
    painter.add(Shape::convex_polygon(hat, ROBE, Stroke::new(1.0, ROBE_DARK)));
    painter.circle_filled(Pos2::new(cx + 2.0, head.y - 22.0), 2.2, Color32::from_rgb(0xff, 0xd7, 0x5e));
    match state {
        AvatarState::Thinking => {
            for i in 0..3 {
                let phase = ((now * 1.5 - i as f64 * 0.25).fract() as f32 * 2.0 - 1.0).abs();
                let dot = Pos2::new(cx - 34.0 + i as f32 * 9.0, head.y - 30.0 - 3.0 * (1.0 - phase));
                painter.circle_filled(dot, 2.5, INK.gamma_multiply(0.4 + 0.6 * (1.0 - phase)));
            }
        }
        AvatarState::Muted => {
            let badge = Pos2::new(cx + 26.0, head.y - 18.0);
            painter.circle_filled(badge, 8.0, MUTED);
            painter.rect_filled(Rect::from_center_size(badge - Vec2::new(2.0, 0.0), Vec2::new(4.0, 5.0)), 0.5, Color32::WHITE);
            painter.line_segment([badge + Vec2::new(-5.0, 5.0), badge + Vec2::new(5.0, -5.0)], Stroke::new(1.5, Color32::WHITE));
        }
        _ => {}
    }
    if let Some(text) = says {
        let galley = painter.layout(text.to_string(), FontId::proportional(11.5), Color32::WHITE, rect.width() - 96.0);
        let size = galley.size() + Vec2::new(16.0, 12.0);
        let balloon = Rect::from_min_size(rect.min + Vec2::splat(4.0), size);
        painter.rect_filled(balloon, 8.0, BALLOON);
        // the tail points at the wizard's head
        let tail = Pos2::new(balloon.right(), (head.y - 4.0).clamp(balloon.top() + 10.0, balloon.bottom() - 4.0));
        painter.add(Shape::convex_polygon(vec![tail - Vec2::new(1.0, 7.0), tail + Vec2::new(10.0, 0.0), tail - Vec2::new(1.0, -3.0)], BALLOON, Stroke::NONE));
        painter.galley(balloon.min + Vec2::new(8.0, 6.0), galley, Color32::WHITE);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_avatar_glides_to_its_target_and_stops() {
        let mut app = DrawingApp::default();
        app.avatar_go_to(Pos2::new(300.0, 200.0));
        assert!(app.avatar.tick(0.1));
        let halfway = app.avatar.pos();
        assert!(halfway.x > 0.0 && halfway.x < 300.0, "{halfway:?}");
        assert!(app.avatar.tick(0.1));
        assert!(app.avatar.pos().x > halfway.x);
        assert!(!app.avatar.tick(0.2), "arrives within 0.35 s");
        assert_eq!(app.avatar.pos(), Pos2::new(300.0, 200.0));
        assert!(!app.avatar.tick(0.1), "stays put");
        // the same target again doesn't restart the glide
        app.avatar_go_to(Pos2::new(300.0, 200.0));
        assert!(!app.avatar.tick(0.1));
    }

    #[test]
    fn a_speech_shows_for_six_seconds_or_until_the_next() {
        let mut app = DrawingApp::default();
        app.prefs.avatar = true;
        app.now = 10.0;
        app.avatar_say("Olá");
        assert_eq!(app.avatar.says.as_ref().map(|(t, _)| t.as_str()), Some("Olá"));
        app.now = 12.0;
        app.avatar_say("Pronto");
        assert_eq!(app.avatar.says.as_ref().map(|(t, _)| t.as_str()), Some("Pronto"));
        let ctx = Context::default();
        let frame = |app: &mut DrawingApp, time: f64| {
            app.now = time;
            ctx.run_ui(egui::RawInput { time: Some(time), ..Default::default() }, |ui| app.avatar_ui(ui.ctx())).textures_delta.clear();
        };
        frame(&mut app, 17.0);
        assert!(app.avatar.says.is_some(), "still showing at 5 s");
        assert!(app.avatar.shown);
        frame(&mut app, 18.5);
        assert_eq!(app.avatar.says, None, "gone after 6 s");
        app.prefs.avatar = false;
        frame(&mut app, 19.0);
        assert!(!app.avatar.shown);
    }

    #[test]
    fn the_chat_hooks_move_the_state_along() {
        let mut app = DrawingApp::default();
        app.now = 1.0;
        let transcript = app.transcripts.entry("c1".into()).or_insert_with(|| crate::transcript::Transcript::new(PathBuf::new()));
        transcript.items.push(ChatItem::Agent("Resposta antiga.".into()));
        app.avatar_follow("c1");
        app.avatar_set(AvatarState::Thinking);
        app.avatar_listen();
        assert_eq!(app.avatar.state, AvatarState::Thinking, "nothing new yet");
        assert_eq!(app.avatar.says, None);
        app.transcripts.get_mut("c1").unwrap().items.push(ChatItem::Agent("Criei o arquivo. Agora vou testar.".into()));
        app.avatar_listen();
        assert_eq!(app.avatar.state, AvatarState::Speaking);
        assert_eq!(app.avatar.says, Some(("Criei o arquivo.".to_string(), 1.0)));
        app.avatar_listen();
        assert_eq!(app.avatar.says, Some(("Criei o arquivo.".to_string(), 1.0)), "said once");
        app.avatar_set(AvatarState::Idle);
        assert_eq!(app.avatar.state, AvatarState::Idle);
    }

    #[test]
    fn the_balloon_shows_the_first_sentence_cut_short() {
        assert_eq!(first_sentence("\n  Pronto! Fiz tudo."), "Pronto!");
        assert_eq!(first_sentence("Sem ponto final"), "Sem ponto final");
        let long = "a".repeat(80);
        assert_eq!(first_sentence(&long), format!("{}…", "a".repeat(60)));
        assert_eq!(first_sentence(""), "");
    }

    #[test]
    fn the_wizard_paints_in_every_state() {
        let ctx = Context::default();
        let mut out = ctx.run_ui(egui::RawInput::default(), |ui| {
            let painter = Painter::new(ui.ctx().clone(), egui::LayerId::background(), Rect::from_min_size(Pos2::ZERO, AVATAR_SIZE));
            for state in [AvatarState::Idle, AvatarState::Listening, AvatarState::Thinking, AvatarState::Speaking, AvatarState::Muted] {
                paint_avatar(&painter, Rect::from_min_size(Pos2::ZERO, AVATAR_SIZE), state, Some("Olá, tudo bem?"), 0.3);
            }
        });
        out.textures_delta.clear();
    }
}
