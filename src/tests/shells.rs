use super::*;

#[test]
fn terminal_keys_and_paste() {
    let none = Modifiers::NONE;
    assert_eq!(key_bytes(Key::Enter, none, false), Some(b"\r".to_vec()));
    assert_eq!(key_bytes(Key::C, Modifiers::CTRL, false), Some(vec![3]));
    assert_eq!(key_bytes(Key::ArrowUp, none, true), Some(b"\x1bOA".to_vec()));
    assert_eq!(key_bytes(Key::ArrowUp, Modifiers::SHIFT, false), Some(b"\x1b[1;2A".to_vec()));
    assert_eq!(key_bytes(Key::Tab, Modifiers::SHIFT, false), Some(b"\x1b[Z".to_vec()));
    assert_eq!(key_bytes(Key::A, none, false), None, "printable keys come as Event::Text");
    assert_eq!(paste_bytes("a\nb", false), b"a\rb".to_vec());
    assert_eq!(paste_bytes("a\x1b[201~b", true), b"\x1b[200~a[201~b\x1b[201~".to_vec());
    // the grid's cell is the monospace font's real advance
    let ctx = Context::default();
    ctx.run_ui(Default::default(), |_| {}).textures_delta.clear();
    let job = egui::text::LayoutJob::simple_singleline("M".repeat(100), FontId::monospace(TERMINAL_FONT), Color32::WHITE);
    let advance = ctx.fonts_mut(|f| f.layout_job(job)).size().x / 100.0;
    assert!((advance - CELL.x).abs() < 0.01, "{advance}");
    assert_eq!(grid_size(terminal_size(80, 24)), (80, 24));
    // block elements are drawn as exact cell parts: full, lower half, left 3/8, a quadrant mix
    assert_eq!(block_parts('█'), Some((vec![[0.0, 0.0, 1.0, 1.0]], 1.0)));
    assert_eq!(block_parts('▄'), Some((vec![[0.0, 0.5, 1.0, 1.0]], 1.0)));
    assert_eq!(block_parts('▍'), Some((vec![[0.0, 0.0, 0.375, 1.0]], 1.0)));
    assert_eq!(block_parts('▛').map(|(p, _)| p.len()), Some(3));
    assert_eq!(block_parts('a'), None);
}

#[cfg(unix)]
#[test]
fn terminal_runs_a_program_and_shows_its_output() {
    let program = ("/bin/sh", &["-c", "printf 'ol\\303\\241 \\033[31mmundo\\033[0m'"][..]);
    let session = TerminalSession::spawn("", "", (40, 5), None, vec![], Some(program), 10_000).unwrap();
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
    while !session.exited() && std::time::Instant::now() < deadline {
        std::thread::sleep(std::time::Duration::from_millis(20));
    }
    assert!(session.exited());
    assert_eq!(session.handle.screen_text(), "olá mundo");
}

#[cfg(windows)]
#[test]
fn clearing_a_windows_terminal_has_the_shell_clear_too() {
    // ConPTY would paint the old screen back on resize: Limpar buffer also sends ^L to the program
    let program = ("powershell", &["-NoProfile", "-Command", "Write-Host ('tecla ' + [int][Console]::ReadKey($true).KeyChar)"][..]);
    let session = TerminalSession::spawn("", "", (40, 5), None, vec![], Some(program), 10_000).unwrap();
    session.handle.clear();
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(20);
    while !session.exited() && std::time::Instant::now() < deadline {
        std::thread::sleep(std::time::Duration::from_millis(20));
    }
    assert!(session.exited());
    assert_eq!(session.handle.screen_text(), "tecla 12");
}

#[cfg(windows)]
#[test]
fn a_windows_terminal_opens_powershell_7_when_it_is_installed() {
    // Windows PowerShell's default policy stops npm's claude.ps1; PowerShell 7's lets it run
    let path = std::env::var_os("PATH").unwrap_or_default();
    let major = if std::env::split_paths(&path).any(|dir| dir.join("pwsh.exe").is_file()) { 7 } else { 5 };
    let command = "Write-Host \"versao $($PSVersionTable.PSVersion.Major)\"; exit";
    let session = TerminalSession::spawn("", command, (60, 20), None, vec![], None, 10_000).unwrap();
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(30);
    while !session.exited() && std::time::Instant::now() < deadline {
        std::thread::sleep(std::time::Duration::from_millis(50));
    }
    assert!(session.exited());
    let screen = session.handle.screen_text();
    assert!(screen.contains(&format!("versao {major}")), "{screen}");
}
