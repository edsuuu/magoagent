use crate::prelude::*;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Action {
    Collapse,
    SelectPill(usize),
    Open,
    Search,
    Settings,
    ChooseOption(usize),
}

fn pressed(input: &egui::InputState, key: Key, modifiers: impl Fn(Modifiers) -> bool) -> bool {
    input.events.iter().any(|event| {
        matches!(event, egui::Event::Key { key: pressed_key, pressed: true, repeat: false, modifiers: held, .. } if *pressed_key == key && modifiers(*held))
    })
}

pub(super) fn actions(input: &egui::InputState, open: bool, option_count: usize) -> Vec<Action> {
    if !open {
        return Vec::new();
    }

    // off the Mac the Ctrl key sets `command` too: only ⌘ is left out
    let control = |modifiers: Modifiers| modifiers.ctrl && !modifiers.alt && !modifiers.shift && !modifiers.mac_cmd;
    let bare = |modifiers: Modifiers| !modifiers.ctrl && !modifiers.alt && !modifiers.shift && !modifiers.command;
    let mut actions = Vec::new();

    if pressed(input, Key::Escape, bare) {
        actions.push(Action::Collapse);
    }

    if input.events.iter().any(|event| matches!(event, egui::Event::Key { modifiers, .. } if control(*modifiers))) {
        for (key, index) in
            [(Key::Num1, 0), (Key::Num2, 1), (Key::Num3, 2), (Key::Num4, 3), (Key::Num5, 4), (Key::Num6, 5), (Key::Num7, 6), (Key::Num8, 7), (Key::Num9, 8)]
        {
            if pressed(input, key, control) {
                actions.push(Action::SelectPill(index));
            }
        }

        for (key, action) in [(Key::O, Action::Open), (Key::K, Action::Search), (Key::Comma, Action::Settings)] {
            if pressed(input, key, control) {
                actions.push(action);
            }
        }
    }

    if option_count > 0 {
        for (key, index) in
            [(Key::Num1, 0), (Key::Num2, 1), (Key::Num3, 2), (Key::Num4, 3), (Key::Num5, 4), (Key::Num6, 5), (Key::Num7, 6), (Key::Num8, 7), (Key::Num9, 8)]
        {
            if pressed(input, key, bare) && index < option_count {
                actions.push(Action::ChooseOption(index));
            }
        }
    }

    actions
}

#[cfg(test)]
mod tests {
    use super::*;

    fn input(events: Vec<egui::Event>) -> egui::InputState {
        let context = Context::default();
        let mut state = None;

        let mut output = context.run_ui(egui::RawInput { events, ..Default::default() }, |ui| {
            state = Some(ui.input(|input| input.clone()));
        });

        output.textures_delta.clear();
        state.unwrap()
    }

    fn key(key: Key, modifiers: Modifiers) -> egui::Event {
        egui::Event::Key { key, physical_key: None, pressed: true, repeat: false, modifiers }
    }

    #[test]
    fn maps_island_shortcuts_only_while_open() {
        let control = Modifiers { ctrl: true, ..Modifiers::NONE };

        assert_eq!(actions(&input(vec![key(Key::Num3, control)]), true, 0), [Action::SelectPill(2)]);
        assert_eq!(actions(&input(vec![key(Key::O, control)]), true, 0), [Action::Open]);
        assert_eq!(actions(&input(vec![key(Key::K, control)]), true, 0), [Action::Search]);
        assert_eq!(actions(&input(vec![key(Key::Comma, control)]), true, 0), [Action::Settings]);
        assert_eq!(actions(&input(vec![key(Key::Escape, Modifiers::NONE)]), true, 0), [Action::Collapse]);
        // Ctrl as Windows and Linux hand it over: `command` held with it
        let real = Modifiers { ctrl: true, command: true, ..Modifiers::NONE };

        assert_eq!(actions(&input(vec![key(Key::K, real)]), true, 0), [Action::Search]);
        assert_eq!(actions(&input(vec![key(Key::Num1, real)]), true, 0), [Action::SelectPill(0)]);
        assert!(actions(&input(vec![key(Key::K, Modifiers { mac_cmd: true, ..real })]), true, 0).is_empty(), "⌘ is not Ctrl");
        assert!(actions(&input(vec![key(Key::Escape, Modifiers::NONE)]), false, 0).is_empty());
    }

    #[test]
    fn chooses_question_options_by_number_without_stealing_modified_keys() {
        assert_eq!(actions(&input(vec![key(Key::Num2, Modifiers::NONE)]), true, 3), [Action::ChooseOption(1)]);
        assert!(actions(&input(vec![key(Key::Num2, Modifiers { ctrl: true, ..Modifiers::NONE })]), true, 3).contains(&Action::SelectPill(1)));
        assert!(actions(&input(vec![key(Key::Num9, Modifiers::NONE)]), true, 2).is_empty());
    }
}
