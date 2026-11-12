use super::*;

/// Bytes a key sends; printable text arrives on its own as `Event::Text`.
pub(crate) fn key_bytes(key: Key, m: Modifiers, app_cursor: bool) -> Option<Vec<u8>> {
    let code = 1 + m.shift as u8 + 2 * m.alt as u8 + 4 * m.ctrl as u8;
    let cursor = |c: char| match (code, app_cursor) {
        (1, true) => format!("\x1bO{c}"),
        (1, false) => format!("\x1b[{c}"),
        _ => format!("\x1b[1;{code}{c}"),
    };
    let tilde = |n: u8| if code > 1 { format!("\x1b[{n};{code}~") } else { format!("\x1b[{n}~") };
    let word = m.alt && !m.ctrl && !m.shift;
    let text = match key {
        // macOS line editing: Cmd+←/→ to the line's start/end, Cmd+⌫ clears it, Option+←/→ by word
        Key::ArrowLeft if m.mac_cmd => "\x01".into(),
        Key::ArrowRight if m.mac_cmd => "\x05".into(),
        Key::Backspace if m.mac_cmd => "\x15".into(),
        Key::ArrowLeft if word => "\x1bb".into(),
        Key::ArrowRight if word => "\x1bf".into(),
        Key::ArrowUp => cursor('A'),
        Key::ArrowDown => cursor('B'),
        Key::ArrowRight => cursor('C'),
        Key::ArrowLeft => cursor('D'),
        Key::Home => cursor('H'),
        Key::End => cursor('F'),
        // Shift/Option+Enter: a new line in agent prompts instead of sending
        Key::Enter if m.shift || m.alt => "\x1b\r".into(),
        Key::Enter => "\r".into(),
        Key::Backspace if m.alt => "\x1b\x7f".into(),
        Key::Backspace if m.ctrl => "\x08".into(),
        Key::Backspace => "\x7f".into(),
        Key::Tab if m.shift => "\x1b[Z".into(),
        Key::Tab => "\t".into(),
        Key::Escape => "\x1b".into(),
        Key::Insert => tilde(2),
        Key::Delete => tilde(3),
        Key::PageUp => tilde(5),
        Key::PageDown => tilde(6),
        Key::F1 => "\x1bOP".into(),
        Key::F2 => "\x1bOQ".into(),
        Key::F3 => "\x1bOR".into(),
        Key::F4 => "\x1bOS".into(),
        Key::F5 => tilde(15),
        Key::F6 => tilde(17),
        Key::F7 => tilde(18),
        Key::F8 => tilde(19),
        Key::F9 => tilde(20),
        Key::F10 => tilde(21),
        Key::F11 => tilde(23),
        Key::F12 => tilde(24),
        _ if m.ctrl && !m.alt => {
            let byte = match key {
                Key::Space | Key::Num2 => 0,
                Key::OpenBracket => 0x1b,
                Key::Backslash => 0x1c,
                Key::CloseBracket => 0x1d,
                Key::Num6 => 0x1e,
                Key::Minus => 0x1f,
                _ => match key.name().as_bytes() {
                    [c @ b'A'..=b'Z'] => c - b'A' + 1,
                    _ => return None,
                },
            };
            return Some(vec![byte]);
        }
        _ => return None,
    };
    Some(text.into_bytes())
}

/// Pasted text: line breaks as Enter, inside bracketed-paste markers when the program asked
/// for them (shells then insert it instead of running each line).
pub(crate) fn paste_bytes(text: &str, bracketed: bool) -> Vec<u8> {
    let text = text.replace("\r\n", "\r").replace('\n', "\r");
    if bracketed {
        // an ESC inside could end the paste early and run the rest as typed keys
        format!("\x1b[200~{}\x1b[201~", text.replace('\x1b', "")).into_bytes()
    } else {
        text.into_bytes()
    }
}
