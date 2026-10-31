//! The avatar: the Maestro's bar, in a window of its own above everything on the screen. It
//! slides to where the action is and shows what the Maestro is saying, its orb, and the mute and
//! close buttons.

use crate::prelude::*;
use crate::transcript::ChatItem;

/// The avatar window, in points: the speech on top, the state, orb and buttons below.
pub(crate) const AVATAR_SIZE: Vec2 = Vec2::new(420.0, 96.0);
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
    /// The followed chat's tool: its program ("claude", "codex") and name ("Claude Code"); the
    /// figure in the bar is its mascot, the orb when there is none.
    pub(crate) who: (String, String),
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
        if let Some(chat) = self.chats.iter().find(|c| c.id == id) {
            let name = chat.name.split(" · ").next().unwrap_or(&chat.agent).to_string();
            self.avatar.who = (chat.agent.clone(), name);
        }
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

    /// Where the avatar belongs: at the bottom of the window being worked in, centered, else at
    /// the bottom of the screen, above the taskbar.
    fn avatar_target(&self, ctx: &Context) -> Pos2 {
        let (outer, monitor) = ctx.input(|i| (i.viewport().outer_rect, i.viewport().monitor_size));
        let origin = outer.map_or(Pos2::ZERO, |r| r.min);
        let focused = self.focused_window().and_then(|id| self.scene.elements.iter().find(|e| &e.id == id));
        if let Some(element) = focused {
            let (rect, _) = self.terminal_frame(element);
            return origin + Vec2::new(rect.center().x - AVATAR_SIZE.x / 2.0, rect.bottom() - AVATAR_SIZE.y - 12.0);
        }
        let screen = match monitor {
            Some(size) => Rect::from_min_size(Pos2::ZERO, size),
            None => outer.unwrap_or(Rect::from_min_size(Pos2::ZERO, Vec2::new(1280.0, 800.0))),
        };
        Pos2::new(screen.center().x - AVATAR_SIZE.x / 2.0, screen.bottom() - AVATAR_SIZE.y - 64.0)
    }

    /// A button of the bar: Mute toggles the microphone (Muted <-> Listening), Close hides the avatar.
    pub(crate) fn avatar_click(&mut self, click: AvatarClick) {
        match click {
            AvatarClick::Mute => self.avatar.state = if self.avatar.state == AvatarState::Muted { AvatarState::Listening } else { AvatarState::Muted },
            AvatarClick::Close => self.prefs.avatar = false,
        }
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
            .with_taskbar(false)
            .with_resizable(false)
            .with_active(false)
            .with_inner_size(AVATAR_SIZE)
            .with_position(self.avatar.pos);
        let (state, says, now, who) = (self.avatar.state, self.avatar.says.clone(), self.now, self.avatar.who.clone());
        let click = ctx.show_viewport_immediate(egui::ViewportId::from_hash_of("avatar"), builder, |ui, _| {
            let rect = ui.max_rect();
            paint_avatar(ui.painter(), rect, state, says.as_ref().map(|(t, _)| t.as_str()), now, &who);
            [(AvatarClick::Mute, mute_button(rect)), (AvatarClick::Close, close_button(rect))].into_iter().find_map(|(click, button)| {
                ui.interact(button, ui.scope_id().with(click), Sense::click()).on_hover_cursor(CursorIcon::PointingHand).clicked().then_some(click)
            })
        });
        if let Some(click) = click {
            self.avatar_click(click);
        }
    }
}

#[derive(Clone, Copy, Debug, Hash, PartialEq, Eq)]
pub(crate) enum AvatarClick {
    Mute,
    Close,
}

const BAR: Color32 = Color32::from_rgba_premultiplied(0x30, 0x30, 0x30, 0xf4);
const BAR_EDGE: Color32 = Color32::from_rgb(0x44, 0x44, 0x44);
const DIM: Color32 = Color32::from_rgb(0x8e, 0x8e, 0x8e);
const ORB: Color32 = Color32::from_rgb(0x3b, 0x6e, 0xf5);
const ORB_LIGHT: Color32 = Color32::from_rgb(0xa9, 0xc4, 0xff);
const LISTENING: Color32 = Color32::from_rgb(0x3d, 0xd6, 0x8c);
const MUTED: Color32 = Color32::from_rgb(0xe5, 0x48, 0x48);
const CLOSE: Color32 = Color32::from_rgb(0x2f, 0x6c, 0xe3);

/// The orb's center in the bar.
fn orb_center(rect: Rect) -> Pos2 {
    Pos2::new(rect.right() - 108.0, rect.bottom() - 30.0)
}

pub(crate) fn mute_button(rect: Rect) -> Rect {
    Rect::from_center_size(Pos2::new(rect.right() - 66.0, rect.bottom() - 30.0), Vec2::splat(28.0))
}

pub(crate) fn close_button(rect: Rect) -> Rect {
    Rect::from_center_size(Pos2::new(rect.right() - 28.0, rect.bottom() - 30.0), Vec2::splat(28.0))
}

/// The bar: what the Maestro says (or a hint), the state, the tool's mascot, mute and close.
pub(crate) fn paint_avatar(painter: &Painter, rect: Rect, state: AvatarState, says: Option<&str>, now: f64, who: &(String, String)) {
    let bar = rect.shrink(2.0);
    painter.rect(bar, 18.0, BAR, Stroke::new(1.0, BAR_EDGE), StrokeKind::Inside);
    // the speech, one line, or the hint
    let (text, color) = match says {
        Some(text) => (text.to_string(), Color32::WHITE),
        None if who.1.is_empty() => ("Trabalhe com o MagoAgent".to_string(), DIM),
        None => (format!("Trabalhe com o {}", who.1), DIM),
    };
    let galley = painter.layout(text, FontId::proportional(13.0), color, bar.width() - 32.0);
    painter.galley(bar.min + Vec2::new(16.0, 14.0), galley, color);
    let label = match state {
        AvatarState::Idle => "Pronto",
        AvatarState::Listening => "Ouvindo",
        AvatarState::Thinking => "Pensando…",
        AvatarState::Speaking => "Falando",
        AvatarState::Muted => "Microfone desligado",
    };
    let label_color = match state {
        AvatarState::Listening => LISTENING,
        AvatarState::Muted => MUTED,
        _ => DIM,
    };
    painter.text(Pos2::new(bar.left() + 16.0, bar.bottom() - 30.0), egui::Align2::LEFT_CENTER, label, FontId::proportional(12.0), label_color);
    match who.0.as_str() {
        "claude" => paint_clawd(painter, orb_center(rect), state, now),
        "codex" => paint_codex(painter, orb_center(rect), state),
        _ => paint_orb(painter, orb_center(rect), 13.0, state, now),
    }
    // mute: a red disc when the microphone is off
    let mute = mute_button(rect);
    let muted = state == AvatarState::Muted;
    if muted {
        painter.circle_filled(mute.center(), 14.0, MUTED);
    }
    let ink = if muted { Color32::WHITE } else { DIM };
    let c = mute.center();
    painter.rect_filled(Rect::from_center_size(c - Vec2::new(0.0, 2.5), Vec2::new(6.0, 11.0)), 3.0, ink);
    let cup = vec![c + Vec2::new(-5.0, 0.0), c + Vec2::new(-5.0, 3.0), c + Vec2::new(0.0, 7.0), c + Vec2::new(5.0, 3.0), c + Vec2::new(5.0, 0.0)];
    painter.add(Shape::line(cup, Stroke::new(1.5, ink)));
    painter.line_segment([c + Vec2::new(0.0, 7.0), c + Vec2::new(0.0, 10.0)], Stroke::new(1.5, ink));
    if muted {
        painter.line_segment([c + Vec2::new(-7.0, 8.0), c + Vec2::new(7.0, -8.0)], Stroke::new(1.8, Color32::WHITE));
    }
    // close
    let close = close_button(rect);
    painter.circle_filled(close.center(), 14.0, CLOSE);
    let c = close.center();
    painter.line_segment([c + Vec2::new(-4.5, -4.5), c + Vec2::new(4.5, 4.5)], Stroke::new(1.8, Color32::WHITE));
    painter.line_segment([c + Vec2::new(-4.5, 4.5), c + Vec2::new(4.5, -4.5)], Stroke::new(1.8, Color32::WHITE));
}

const CLAUDE_ORANGE: Color32 = Color32::from_rgb(0xd9, 0x77, 0x57);

/// Claude Code's little orange fellow: a rounded body with two eyes and short legs; it blinks
/// while listening, looks around while thinking, bobs while speaking and goes grey when muted.
pub(crate) fn paint_clawd(painter: &Painter, center: Pos2, state: AvatarState, now: f64) {
    let t = now as f32;
    let bob = if state == AvatarState::Speaking { 2.5 * (t * 11.0).sin().abs() } else { 0.0 };
    let body_color = if state == AvatarState::Muted { Color32::from_gray(0x80) } else { CLAUDE_ORANGE };
    let c = center - Vec2::new(0.0, bob);
    let body = Rect::from_center_size(c, Vec2::new(26.0, 20.0));
    // legs and arms under and beside the body
    for dx in [-7.0, 7.0] {
        painter.rect_filled(Rect::from_center_size(Pos2::new(c.x + dx, body.bottom() + 2.0), Vec2::new(5.0, 6.0)), 1.5, body_color);
    }
    for dx in [-15.0, 15.0] {
        painter.rect_filled(Rect::from_center_size(Pos2::new(c.x + dx, c.y + 2.0), Vec2::new(5.0, 4.0)), 1.5, body_color);
    }
    painter.rect_filled(body, 5.0, body_color);
    let blink = state == AvatarState::Listening && (now * 1.3).fract() < 0.12;
    let look = if state == AvatarState::Thinking { 2.0 * (t * 2.0).sin() } else { 0.0 };
    for dx in [-5.0, 5.0] {
        let eye = Pos2::new(c.x + dx + look, c.y - 1.0);
        let size = if blink { Vec2::new(4.0, 1.5) } else { Vec2::new(3.5, 7.0) };
        painter.rect_filled(Rect::from_center_size(eye, size), 1.0, Color32::from_rgb(0x1f, 0x1a, 0x17));
    }
    if state == AvatarState::Listening {
        painter.circle_stroke(center, 21.0, Stroke::new(1.5, LISTENING.gamma_multiply(0.6 + 0.4 * (t * 3.0).sin())));
    }
}

/// Codex: OpenAI's mark on a light disc (assets/ide-icons/NOTICE); grey when muted.
pub(crate) fn paint_codex(painter: &Painter, center: Pos2, state: AvatarState) {
    let disc = if state == AvatarState::Muted { Color32::from_gray(0x80) } else { Color32::from_gray(0xf2) };
    painter.circle_filled(center, 14.0, disc);
    crate::ide::icons::paint_tint(painter, crate::ide::icons::OPENAI, Rect::from_center_size(center, Vec2::splat(18.0)), Color32::BLACK);
}

/// The orb: a blue sphere with a light swirling inside; it breathes while listening, spins
/// while thinking, throbs while speaking and goes grey when muted.
pub(crate) fn paint_orb(painter: &Painter, center: Pos2, radius: f32, state: AvatarState, now: f64) {
    let t = now as f32;
    let (radius, spin, base, light) = match state {
        AvatarState::Listening => (radius * (1.0 + 0.08 * (t * 3.0).sin()), 1.2, ORB, ORB_LIGHT),
        AvatarState::Thinking => (radius, 4.0, ORB, ORB_LIGHT),
        AvatarState::Speaking => (radius * (1.0 + 0.18 * (t * 11.0).sin().abs()), 2.0, ORB, Color32::WHITE),
        AvatarState::Muted => (radius, 0.3, Color32::from_gray(0x70), Color32::from_gray(0xb0)),
        AvatarState::Idle => (radius, 0.6, ORB, ORB_LIGHT),
    };
    if state == AvatarState::Listening {
        painter.circle_stroke(center, radius + 5.0, Stroke::new(1.5, LISTENING.gamma_multiply(0.6 + 0.4 * (t * 3.0).sin())));
    }
    painter.circle_filled(center, radius, base);
    // the lights stay inside the sphere: offset + their radius <= the orb's
    let a = t * spin;
    let glow = center + Vec2::angled(a) * (radius * 0.35);
    painter.circle_filled(glow, radius * 0.62, light.gamma_multiply(0.55));
    let shine = center + Vec2::angled(a + 2.4) * (radius * 0.5);
    painter.circle_filled(shine, radius * 0.4, Color32::WHITE.gamma_multiply(0.7));
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
        app.chats.push(crate::app::chat_direct::Chat {
            id: "c1".into(),
            agent: "claude".into(),
            name: "Claude Code · loja".into(),
            cwd: "~/loja".into(),
            session: None,
            mode: 0,
            model: String::new(),
            effort: String::new(),
        });
        app.avatar_follow("c1");
        assert_eq!(app.avatar.who, ("claude".to_string(), "Claude Code".to_string()));
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
    fn mute_toggles_the_microphone_and_close_hides_the_avatar() {
        let mut app = DrawingApp::default();
        app.prefs.avatar = true;
        app.avatar_set(AvatarState::Listening);
        app.avatar_click(AvatarClick::Mute);
        assert_eq!(app.avatar.state, AvatarState::Muted);
        app.avatar_click(AvatarClick::Mute);
        assert_eq!(app.avatar.state, AvatarState::Listening);
        app.avatar_click(AvatarClick::Close);
        assert!(!app.prefs.avatar);
        let bar = Rect::from_min_size(Pos2::ZERO, AVATAR_SIZE);
        assert!(bar.contains_rect(mute_button(bar)) && bar.contains_rect(close_button(bar)));
        assert!(!mute_button(bar).intersects(close_button(bar)));
    }

    #[test]
    fn the_bar_paints_in_every_state() {
        let ctx = Context::default();
        let mut out = ctx.run_ui(egui::RawInput::default(), |ui| {
            let painter = Painter::new(ui.ctx().clone(), egui::LayerId::background(), Rect::from_min_size(Pos2::ZERO, AVATAR_SIZE));
            for state in [AvatarState::Idle, AvatarState::Listening, AvatarState::Thinking, AvatarState::Speaking, AvatarState::Muted] {
                for who in [(String::new(), String::new()), ("claude".into(), "Claude Code".into()), ("codex".into(), "Codex".into())] {
                    paint_avatar(&painter, Rect::from_min_size(Pos2::ZERO, AVATAR_SIZE), state, Some("Olá, tudo bem?"), 0.3, &who);
                    paint_avatar(&painter, Rect::from_min_size(Pos2::ZERO, AVATAR_SIZE), state, None, 0.3, &who);
                }
            }
        });
        out.textures_delta.clear();
    }
}
