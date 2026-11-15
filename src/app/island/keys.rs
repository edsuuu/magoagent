//! The keyboard on the island: what each global shortcut does here (`app/hotkeys/`, pressed with any program in front).
//! The island only calls `island_keys` as it draws.

use super::keymap::{Action as KeyAction, actions};
use super::paint::{Out, View};
use super::{Tab, alphas, sounds};
use crate::app::hotkeys::GlobalAction;
use crate::app::ui::agents::AgentState;
use crate::prelude::*;

/// The pill `step` pills after `current` in `ids`, going around; from the first with none in focus.
pub(crate) fn cycle_pill(ids: &[String], current: Option<&str>, step: isize) -> Option<String> {
    let at = current.and_then(|id| ids.iter().position(|other| other == id)).unwrap_or(0);
    ids.get((at as isize + step).rem_euclid(ids.len().max(1) as isize) as usize).cloned()
}

impl DrawingApp {
    /// A global shortcut was pressed: what it does on the island.
    pub(crate) fn run_global(&mut self, ctx: &Context, action: GlobalAction) {
        let time = ctx.input(|input| input.time);
        let in_focus = self.hotkeys.pill.clone().or_else(|| self.hotkeys.shown.clone()).or_else(|| self.island.focus.clone());
        match action {
            GlobalAction::OpenChat => {
                self.island_open_on(Tab::Chat, time);
                if self.chat_agent.is_none() {
                    self.chat_agent = self.chats.first().map(|chat| chat.id.clone());
                }
                self.hotkeys.take_keyboard = true;
            }
            // a question or a permission keeps the island open on its card: the keyboard goes to it, so a number answers
            GlobalAction::GoToAlert if !self.person_asks.borrow().is_empty() => self.hotkeys.take_keyboard = true,
            GlobalAction::GoToAlert => {
                let outside = self.outside_pills().into_iter().map(|pill| (pill.id, pill.state));
                match self.agent_rows().into_iter().map(|row| (row.id, row.state)).chain(outside).find(|(_, state)| *state == AgentState::Waiting) {
                    Some((id, _)) => self.island_show(&id, time),
                    // nothing waits: the island says so
                    None => self.island_chime(sounds::ERROR, time),
                }
            }
            GlobalAction::JumpToTerminal => {
                if let Some(id) = in_focus {
                    self.hotkeys.terminal_jump = Some(id.clone());
                    if !id.starts_with(crate::app::maestro::outside::PILL_PREFIX) {
                        self.open_agent(&id);
                        self.raise_window = true;
                    }
                    self.island.machine.force_petit(time);
                }
            }
            GlobalAction::NextPill | GlobalAction::PreviousPill => {
                let step = if action == GlobalAction::NextPill { 1 } else { -1 };
                if let Some(id) = cycle_pill(&self.hotkeys.pills, in_focus.as_deref(), step) {
                    self.hotkeys.pill = Some(id);
                    self.island_open_on(Tab::Home, time);
                    self.island_chime(sounds::BLIP, time);
                }
            }
            GlobalAction::MuteToggle => {
                self.prefs.island_sound = !self.prefs.island_sound;
                // heard when the sounds come back
                self.island_chime(sounds::BLIP, time);
            }
        }
        ctx.request_repaint();
    }

    /// The island open on `tab`, as a click on that tab leaves it.
    fn island_open_on(&mut self, tab: Tab, time: f64) {
        (self.island.tab, self.island.form) = (tab, None);
        self.island.machine.force_home(time);
    }

    /// As the island is drawn, in its own window: what the global shortcuts walk is kept (the pills as shown and the
    /// one in focus), the pill the keyboard chose stays in focus, and the window takes the keyboard when asked to.
    pub(super) fn island_keys(&mut self, ui: &egui::Ui, view: &View, out: &mut Out) {
        if self.hotkeys.pills.iter().ne(view.lines.iter().map(|line| &line.id)) {
            self.hotkeys.pills = view.lines.iter().map(|line| line.id.clone()).collect();
        }
        if self.hotkeys.shown != view.focus {
            self.hotkeys.shown = view.focus.clone();
        }
        // the pill the keyboard chose: until the pointer picks another, the island folds or the pill goes
        if out.focus.is_some() || !self.island.open || !self.hotkeys.pill.as_ref().is_some_and(|pill| self.hotkeys.pills.contains(pill)) {
            self.hotkeys.pill = None;
        } else {
            out.focus = self.hotkeys.pill.clone();
        }
        // once the panel shows (its box is there to take the typing)
        if self.hotkeys.take_keyboard && alphas(self.island.anim.x).2 > 0.0 {
            self.hotkeys.take_keyboard = false;
            ui.ctx().send_viewport_cmd(egui::ViewportCommand::Focus);
            if view.asks.is_empty() && view.tab == Tab::Chat {
                ui.memory_mut(|memory| memory.request_focus(egui::Id::unique("chat_draft")));
            }
        }
        let option_count = self.person_asks.borrow().first().and_then(|ask| ask.questions.get(ask.step)).map_or(0, |question| question.options.len());
        let key_actions = actions(&ui.input(|input| input.clone()), self.island.open, option_count);
        let has_choose = key_actions.iter().any(|action| matches!(action, KeyAction::ChooseOption(_)));
        if has_choose && let Some(ask) = self.person_asks.borrow_mut().first_mut() {
            ask.shown = Some(f64::NEG_INFINITY);
        }
        for action in key_actions {
            match action {
                KeyAction::Collapse => {
                    if self.island.settings {
                        self.island.settings = false;
                        self.island.machine.interacted(ui.input(|input| input.time));
                    } else {
                        self.island.machine.force_petit(ui.input(|input| input.time));
                        self.island.tab = Tab::Home;
                    }
                }
                KeyAction::SelectPill(index) => {
                    if let Some(id) = self.hotkeys.pills.get(index).cloned() {
                        self.hotkeys.pill = Some(id);
                        self.island_open_on(Tab::Home, ui.input(|input| input.time));
                    }
                }
                KeyAction::Open => {
                    if view.tab == Tab::Chat {
                        self.chat_open = true;
                    }
                }
                KeyAction::Search => {
                    self.chat_search = Some(String::new());
                    ui.memory_mut(|memory| memory.request_focus(egui::Id::unique("chat_search")));
                }
                KeyAction::Settings => {
                    self.island.settings = !self.island.settings;
                    self.island.machine.interacted(ui.input(|input| input.time));
                }
                KeyAction::ChooseOption(index) => {
                    let mut asks = self.person_asks.borrow_mut();
                    if let Some(ask) = asks.first_mut() {
                        if ask.questions.is_empty() {
                            drop(asks);
                            let mut asks = self.person_asks.borrow_mut();
                            if !asks.is_empty() {
                                asks.remove(0).send(index == 0);
                            }
                        } else if let Some(picked) = ask.picked.get_mut(ask.step)
                            && let Some(question) = ask.questions.get(ask.step)
                            && index < question.options.len()
                        {
                            if question.multiple {
                                if picked.contains(&index) {
                                    picked.retain(|chosen| *chosen != index);
                                } else {
                                    picked.push(index);
                                }
                            } else {
                                *picked = vec![index];
                            }
                        }
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::chat::decide::PersonAsk;
    use crate::app::island::EXPANDED_W;

    const SIZE: Vec2 = Vec2::new(1200.0, 800.0);

    fn chat(id: &str, name: &str) -> crate::app::chat::direct::Chat {
        crate::app::chat::direct::Chat {
            id: id.into(),
            agent: "claude".into(),
            name: name.into(),
            cwd: "C:/p".into(),
            session: None,
            mode: 0,
            model: String::new(),
            effort: String::new(),
            recruit: false,
            named: false,
        }
    }

    /// The app with the island on, closed, and two chats; the time it is at.
    fn island() -> (DrawingApp, Context, f64) {
        let mut app = DrawingApp::default();
        app.prefs.island = true;
        app.chats = vec![chat("a", "Claude Code · loja"), chat("b", "Codex · site")];
        let ctx = Context::default();
        step(&mut app, &ctx, 1.0, vec![]);
        (app, ctx, 1.0)
    }

    fn step(app: &mut DrawingApp, ctx: &Context, time: f64, events: Vec<egui::Event>) {
        let raw = egui::RawInput { screen_rect: Some(Rect::from_min_size(Pos2::ZERO, SIZE)), time: Some(time), events, ..Default::default() };
        let mut output = ctx.run_ui(raw, |_| app.island_ui(ctx));
        output.textures_delta.clear();
    }

    /// Frames until `until`, with nothing pressed.
    fn wait(app: &mut DrawingApp, ctx: &Context, time: &mut f64, until: f64) {
        while *time < until {
            *time += 0.05;
            step(app, ctx, *time, vec![]);
        }
    }

    /// A question from a session outside the app, waiting on the island.
    fn ask(app: &mut DrawingApp, request: Value) -> std::sync::mpsc::Receiver<Result<String, String>> {
        let (reply, answers) = std::sync::mpsc::channel();
        app.person_asks.get_mut().push(PersonAsk::new(format!("{}1", crate::app::maestro::outside::ASK_PREFIX), &request, reply));
        answers
    }

    #[test]
    fn the_pills_are_walked_around_both_ways() {
        let ids = ["a".to_string(), "b".to_string(), "c".to_string()];
        assert_eq!(cycle_pill(&ids, Some("a"), 1).as_deref(), Some("b"));
        assert_eq!(cycle_pill(&ids, Some("c"), 1).as_deref(), Some("a"), "past the last: the first");
        assert_eq!(cycle_pill(&ids, Some("a"), -1).as_deref(), Some("c"), "before the first: the last");
        assert_eq!(cycle_pill(&ids, None, 1).as_deref(), Some("b"), "none in focus: from the first");
        assert_eq!(cycle_pill(&ids, Some("gone"), -1).as_deref(), Some("c"));
        assert_eq!(cycle_pill(&[], Some("a"), 1), None);
    }

    #[test]
    fn the_next_and_previous_pill_shortcuts_open_the_island_on_that_pill() {
        let (mut app, ctx, mut time) = island();
        assert_eq!(app.hotkeys.pills, ["a", "b"]);
        assert!(app.island.size().x < EXPANDED_W, "closed");
        app.run_global(&ctx, GlobalAction::NextPill);
        wait(&mut app, &ctx, &mut time, 2.5);
        assert_eq!(app.island.size().x, EXPANDED_W, "the island opened");
        assert_eq!((app.island.tab(), app.island.focus()), (Tab::Home, Some("b")), "on the second pill, which stays in focus");
        app.run_global(&ctx, GlobalAction::NextPill);
        step(&mut app, &ctx, time, vec![]);
        assert_eq!(app.island.focus(), Some("a"), "around to the first");
        app.run_global(&ctx, GlobalAction::PreviousPill);
        step(&mut app, &ctx, time, vec![]);
        assert_eq!(app.island.focus(), Some("b"));
        app.run_global(&ctx, GlobalAction::JumpToTerminal);
        assert_eq!(app.hotkeys.terminal_jump.as_deref(), Some("b"));
        // left alone the island folds, and the keyboard's pill goes with it
        wait(&mut app, &ctx, &mut time, 40.0);
        assert!(app.island.size().x < EXPANDED_W);
        assert_eq!(app.hotkeys.pill, None);
    }

    #[test]
    fn the_chat_shortcut_opens_the_islands_conversation_and_takes_the_keyboard() {
        let (mut app, ctx, mut time) = island();
        app.chat_agent = None;
        app.run_global(&ctx, GlobalAction::OpenChat);
        assert!(app.hotkeys.take_keyboard);
        wait(&mut app, &ctx, &mut time, 2.5);
        assert_eq!((app.island.tab(), app.chat_agent.as_deref()), (Tab::Chat, Some("a")), "the conversation, on the first chat");
        assert!(!app.hotkeys.take_keyboard, "taken once the panel showed");
        assert!(ctx.memory(|memory| memory.has_focus(egui::Id::unique("chat_draft"))), "the typing goes to its box");
    }

    #[test]
    fn the_alert_shortcut_goes_to_what_waits_and_says_when_nothing_does() {
        let (mut app, ctx, _) = island();
        let chimes = app.island.chimes();
        app.run_global(&ctx, GlobalAction::GoToAlert);
        assert_eq!(app.island.chimes(), chimes + 1, "nothing waits: the error tone");
        assert!(!app.hotkeys.take_keyboard);
        let _answers = ask(&mut app, json!({ "tool_name": "Bash", "input": { "command": "ls" } }));
        app.run_global(&ctx, GlobalAction::GoToAlert);
        assert!(app.hotkeys.take_keyboard, "a permission waits: the island takes the keyboard for it");
        assert_eq!(app.island.chimes(), chimes + 1);
    }

    #[test]
    fn the_mute_shortcut_turns_the_sounds_off_and_on() {
        let (mut app, ctx, _) = island();
        assert!(app.prefs.island_sound);
        app.run_global(&ctx, GlobalAction::MuteToggle);
        assert!(!app.prefs.island_sound);
        app.run_global(&ctx, GlobalAction::MuteToggle);
        assert!(app.prefs.island_sound);
    }
}
