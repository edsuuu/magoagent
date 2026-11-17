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
    // the wheel: the history in a shell, the program's own when it reports the mouse (Claude Code's fullscreen view), arrows in less
    use alacritty_terminal::term::TermMode;
    let shell = TermMode::default();
    assert_eq!(wheel_bytes(3, shell, (0, 0), false), None, "a shell scrolls its history");
    let tui = shell | TermMode::ALT_SCREEN | TermMode::MOUSE_REPORT_CLICK | TermMode::MOUSE_DRAG | TermMode::SGR_MOUSE;
    assert_eq!(wheel_bytes(2, tui, (4, 9), false), Some(b"\x1b[<64;5;10M\x1b[<64;5;10M".to_vec()));
    assert_eq!(wheel_bytes(-1, tui, (0, 0), false), Some(b"\x1b[<65;1;1M".to_vec()));
    assert_eq!(wheel_bytes(1, tui, (0, 0), true), None, "Shift forces the history");
    assert_eq!(wheel_bytes(1, tui - TermMode::SGR_MOUSE, (0, 0), false), Some(vec![0x1b, b'[', b'M', 96, 33, 33]));
    assert_eq!(wheel_bytes(-2, shell | TermMode::ALT_SCREEN, (0, 0), false), Some(b"\x1bOB\x1bOB".to_vec()), "less gets arrows");
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
fn a_program_that_reports_the_mouse_gets_the_wheel() {
    // Claude Code's fullscreen view: raw keys with VT input (so ConPTY passes the mouse modes on), the alternate
    // screen and SGR mouse reports; it shows what the wheel sent
    let script = std::env::temp_dir().join(format!("mago-roda-{}.ps1", std::process::id()));
    std::fs::write(
        &script,
        r#"Add-Type -Name C -Namespace W -MemberDefinition '[DllImport("kernel32.dll")] public static extern System.IntPtr GetStdHandle(int h); [DllImport("kernel32.dll")] public static extern bool GetConsoleMode(System.IntPtr h, out int m); [DllImport("kernel32.dll")] public static extern bool SetConsoleMode(System.IntPtr h, int m);'
$h = [W.C]::GetStdHandle(-10); $m = 0; $null = [W.C]::GetConsoleMode($h, [ref]$m)
$null = [W.C]::SetConsoleMode($h, ($m -band (-bnot 0x7)) -bor 0x200)
$e = [char]27; Write-Host -NoNewline ($e + '[?1049h' + $e + '[?1000h' + $e + '[?1006h')
$b = New-Object byte[] 64; $n = [Console]::OpenStandardInput().Read($b, 0, 64)
Write-Host -NoNewline ($e + '[?1049l'); Write-Host ('lido ' + ($b[0..($n - 1)] -join ','))"#,
    )
    .unwrap();
    let path = script.display().to_string();
    let program = ("powershell", &["-NoProfile", "-ExecutionPolicy", "Bypass", "-File", &path][..]);
    let session = TerminalSession::spawn("", "", (40, 5), None, vec![], Some(program), 10_000).unwrap();
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(20);
    while !session.handle.mode().contains(alacritty_terminal::term::TermMode::SGR_MOUSE) && std::time::Instant::now() < deadline {
        std::thread::sleep(std::time::Duration::from_millis(20));
    }
    let wheel = wheel_bytes(1, session.handle.mode(), (2, 3), false);
    assert_eq!(wheel, Some(b"[<64;3;4M".to_vec()), "the mouse modes came through ConPTY");
    session.handle.write(wheel.unwrap_or_default());
    while !session.exited() && std::time::Instant::now() < deadline {
        std::thread::sleep(std::time::Duration::from_millis(20));
    }
    let _ = std::fs::remove_file(&script);
    assert_eq!(session.handle.screen_text(), "lido 27,91,60,54,52,59,51,59,52,77");
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
