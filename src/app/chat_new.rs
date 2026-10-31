//! A new chat from the chat itself, with no canvas in sight: the agents installed here (the quick
//! starts whose program is on the PATH), each opening a terminal of its own in chat mode.

use crate::prelude::*;

impl DrawingApp {
    /// The quick starts that run an agent found on this machine (a shell isn't one), looked up
    /// again every 10 s: one installed meanwhile shows up.
    pub(crate) fn installed_agents(&mut self) -> Vec<Preset> {
        if self.chat_installed.as_ref().is_none_or(|(at, _)| self.now - at > 10.0) {
            let found = |p: &Preset| p.command.split_whitespace().next().is_some_and(|program| crate::ide::lsp::which(program).is_some());
            self.chat_installed = Some((self.now, self.prefs.presets.iter().filter(|p| found(p)).cloned().collect()));
        }
        self.chat_installed.as_ref().map(|(_, list)| list.clone()).unwrap_or_default()
    }

    /// A terminal running `preset` where the canvas has room, in chat mode, the chat on it.
    pub(crate) fn new_chat(&mut self, preset: &Preset) {
        let want = Rect::from_center_size(self.to_world(self.canvas_rect.center()), terminal_size(100, 30));
        let at = self.free_spot(want);
        let spec = TerminalSpec { name: preset.name.clone(), cwd: self.new_folder(), command: preset.command.clone(), chat: true, ..Default::default() };
        self.terminal_dialog = Some(TerminalDialog { target: None, at, spec, tab: 0 });
        self.confirm_terminal_dialog();
        // the chat has the keys, not the terminal behind it
        self.terminal_focus = None;
    }
}

#[cfg(test)]
mod tests {
    use crate::prelude::*;

    #[test]
    fn a_new_chat_opens_its_agent_in_a_terminal_of_its_own() {
        let mut app = DrawingApp::default();
        app.canvas_rect = Rect::from_min_size(Pos2::ZERO, Vec2::new(1200.0, 800.0));
        let codex = Preset { name: "Codex".into(), command: "codex".into(), on: true };
        app.new_chat(&codex);
        app.new_chat(&codex);
        let terminals: Vec<&Element> = app.scene.elements.iter().filter(|e| e.terminal.is_some()).collect();
        let names: Vec<&str> = terminals.iter().map(|e| e.terminal.as_ref().unwrap().name.as_str()).collect();
        assert_eq!(names, ["Codex", "Codex 2"]);
        assert!(terminals.iter().all(|e| e.terminal.as_ref().is_some_and(|t| t.chat && t.command == "codex")));
        // side by side, not on top of each other; the chat on the newest, with the keys
        assert!(!element_bounds(terminals[0]).intersects(element_bounds(terminals[1])));
        assert!(app.chat_open && app.chat_agent.as_ref() == Some(&terminals[1].id) && app.terminal_focus.is_none());
    }
}
