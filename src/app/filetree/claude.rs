//! The tree's Claude Code button: a `claude` terminal roped to it, in its folder.

use crate::prelude::*;

impl DrawingApp {
    /// The `claude` terminal roped to the tree, made beside it in its folder the first time, back on the
    /// canvas and typing.
    pub(crate) fn claude_for_tree(&mut self, tree: &str) {
        let Some(element) = self.scene.elements.iter().find(|e| e.id == tree).cloned() else { return };
        let is_claude =
            |id: &String| self.scene.elements.iter().any(|e| &e.id == id && e.terminal.as_ref().is_some_and(|t| t.command.trim_start().starts_with("claude")));
        let roped = self.links().get(tree).and_then(|l| l.iter().find(|id| is_claude(id)).cloned());
        let terminal = match roped {
            Some(id) => id,
            None => {
                let b = local_bounds(&element);
                let at = Rect::from_min_size(Pos2::new(b.max.x + 60.0, b.min.y), terminal_size(100, 30));
                let cwd = element.filetree.map(|s| s.root).unwrap_or_default();
                let spec = TerminalSpec { name: String::new(), cwd, command: "claude".into(), ..Default::default() };
                self.terminal_dialog = Some(TerminalDialog { target: None, at, spec, tab: 0 });
                self.confirm_terminal_dialog();
                let Some(id) = self.terminal_focus.clone() else { return };
                // one Cmd+Z takes back the terminal and its rope
                self.rope_between(tree, &id, false);
                id
            }
        };
        self.set_elevated(None);
        self.selected = self.scene.elements.iter().position(|e| e.id == terminal).into_iter().collect();
        self.terminal_focus = Some(terminal);
    }
}
