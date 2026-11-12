use super::*;

#[test]
fn dirty_tab_never_overwrites_a_file_changed_on_disk() {
    let dir = std::env::temp_dir().join(format!("magoagent-ide-save-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let f = dir.join("a.txt");
    std::fs::write(&f, "um\n").unwrap();
    let mut ide = Ide::new(egui::Context::default(), "k", &dir.to_string_lossy());
    ide.open_file(f.clone(), None);
    ide.tabs[0].text = "meu\n".into();
    ide.tabs[0].dirty = true;
    // Claude Code (or git) rewrote it meanwhile
    std::fs::write(&f, "deles\n").unwrap();
    std::fs::File::options().write(true).open(&f).unwrap().set_modified(SystemTime::now() + Duration::from_secs(5)).unwrap();
    ide.save_all();
    ide.close_tab(0);
    assert_eq!((std::fs::read_to_string(&f).unwrap().as_str(), ide.tabs.len(), ide.tabs[0].dirty), ("deles\n", 1, true));
    assert!(ide.status.contains("mudou no disco"), "{}", ide.status);
    ide.run(Cmd::SaveAll); // the explicit save is the user's decision
    assert_eq!((std::fs::read_to_string(&f).unwrap().as_str(), ide.tabs[0].dirty), ("meu\n", false));
    let _ = std::fs::remove_dir_all(&dir);
}

/// `f` rewritten by someone else, with a later mtime than the tab saw.
fn rewrite(f: &Path, text: &str, later: u64) {
    std::fs::write(f, text).unwrap();
    std::fs::File::options().write(true).open(f).unwrap().set_modified(SystemTime::now() + Duration::from_secs(later)).unwrap();
}

#[test]
fn an_agent_edit_never_mixes_with_unsaved_text_and_fails_whole() {
    let dir = std::env::temp_dir().join(format!("magoagent-ide-agentedit-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let f = dir.join("a.txt");
    std::fs::write(&f, "um\ndois\n").unwrap();
    let mut ide = Ide::new(egui::Context::default(), "e", &dir.to_string_lossy());
    ide.open_file(f.clone(), None);
    // the person is typing: the agent is turned away and nothing moves
    (ide.tabs[0].text, ide.tabs[0].dirty) = ("meu\n".into(), true);
    let err = ide.agent_edit(&f, "um", "UM").unwrap_err();
    assert!(err.contains("não salvas"), "{err}");
    assert_eq!((ide.tabs[0].text.as_str(), std::fs::read_to_string(&f).unwrap().as_str()), ("meu\n", "um\ndois\n"));
    // discarded; the file changes on disk meanwhile: the edit goes on what is on disk
    ide.tabs[0].reload();
    rewrite(&f, "um\ntrês\n", 5);
    assert_eq!(ide.agent_edit(&f, "três", "TRÊS"), Ok((1, 1)));
    assert_eq!(std::fs::read_to_string(&f).unwrap(), "um\nTRÊS\n");
    // the write fails: the tab is as it was, clean
    let perms = std::fs::metadata(&f).unwrap().permissions();
    let mut read_only = perms.clone();
    read_only.set_readonly(true);
    std::fs::set_permissions(&f, read_only).unwrap();
    assert!(ide.agent_edit(&f, "um", "UM").is_err());
    assert_eq!((ide.tabs[0].text.as_str(), ide.tabs[0].dirty), ("um\nTRÊS\n", false));
    std::fs::set_permissions(&f, perms).unwrap();
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_conflicting_tab_goes_to_recovery_on_the_way_out_and_the_band_reloads_it() {
    let dir = std::env::temp_dir().join(format!("magoagent-ide-recover-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let f = dir.join("a.txt");
    std::fs::write(&f, "um\n").unwrap();
    let (ctx, mut ide) = shown(0);
    ide.recovery = Some(dir.join("recuperados"));
    ide.tabs.push(Tab::new(f.clone(), "um\n".into(), false));
    (ide.tabs[0].text, ide.tabs[0].dirty) = ("meu\n".into(), true);
    rewrite(&f, "deles\n", 5);
    assert!(ide.tabs[0].conflict());
    // closing: the disk keeps theirs, the person's text goes aside, and a second pass is the same file
    let message = ide.recover().unwrap();
    ide.recover();
    let kept: Vec<PathBuf> = std::fs::read_dir(dir.join("recuperados")).unwrap().map(|e| e.unwrap().path()).collect();
    assert_eq!(kept.len(), 1);
    assert!(message.contains(&kept[0].display().to_string()) && ide.status == message, "{message}");
    assert_eq!((std::fs::read_to_string(&kept[0]).unwrap().as_str(), std::fs::read_to_string(&f).unwrap().as_str()), ("meu\n", "deles\n"));
    // still open: the band over the editor offers both; "Recarregar do disco" takes theirs
    let out = frames(&ctx, &mut ide, vec![], 2);
    let at = out
        .shapes
        .iter()
        .find_map(|c| match &c.shape {
            Shape::Text(s) if s.galley.text() == "Recarregar do disco" => Some(s.pos + vec2(4.0, 6.0)),
            _ => None,
        })
        .expect("the band is shown");
    let button = |pressed| egui::Event::PointerButton { pos: at, button: egui::PointerButton::Primary, pressed, modifiers: Modifiers::NONE };
    frames(&ctx, &mut ide, vec![egui::Event::PointerMoved(at), button(true), button(false)], 2);
    assert_eq!((ide.tabs[0].text.as_str(), ide.tabs[0].dirty, ide.tabs[0].conflict()), ("deles\n", false, false));
    assert_eq!(ide.recover(), None, "nothing left to keep");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn conflict_resolution_reload_keep_and_reject_without_conflict() {
    let dir = std::env::temp_dir().join(format!("magoagent-ide-conflict-tool-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let f = dir.join("a.txt");
    std::fs::write(&f, "base\n").unwrap();
    let (_, mut ide) = shown(0);
    ide.tabs.push(Tab::new(f.clone(), "base\n".into(), false));
    (ide.tabs[0].text, ide.tabs[0].dirty) = ("meu\n".into(), true);
    rewrite(&f, "disco\n", 5);
    assert!(ide.resolve_conflict("", "reload").unwrap().contains("a.txt"));
    assert_eq!(ide.tabs[0].text, "disco\n");
    (ide.tabs[0].text, ide.tabs[0].dirty) = ("minha\n".into(), true);
    rewrite(&f, "outro\n", 5);
    assert!(ide.resolve_conflict("", "keep").unwrap().contains("a.txt"));
    assert_eq!(std::fs::read_to_string(&f).unwrap(), "minha\n");
    assert!(ide.resolve_conflict("", "reload").unwrap_err().contains("não está em conflito"));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn the_tree_picks_up_files_made_and_removed_outside() {
    let dir = std::env::temp_dir().join(format!("magoagent-ide-sync-{}", std::process::id()));
    std::fs::create_dir_all(dir.join("src")).unwrap();
    std::fs::write(dir.join("src/a.rs"), "").unwrap();
    std::fs::write(dir.join("z.txt"), "").unwrap();
    let mut ide = Ide::new(egui::Context::default(), "s", &dir.to_string_lossy());
    ide.card_click(0); // opens src: src, a.rs, z.txt
    ide.card_click(2);
    // a terminal adds and removes files meanwhile
    std::fs::write(dir.join("src/b.rs"), "").unwrap();
    std::fs::remove_file(dir.join("src/a.rs")).unwrap();
    std::fs::create_dir_all(dir.join("docs")).unwrap();
    ide.sync_tree();
    let rows = ide.card_rows();
    assert_eq!(rows.iter().map(|r| r.1.as_str()).collect::<Vec<_>>(), ["docs", "src", "b.rs", "z.txt"]);
    assert!(rows[1].3, "src stays open");
    assert_eq!(ide.card_sel, Some(3), "the selection stays on z.txt");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn the_cards_panel_fits_its_tabs_and_keeps_a_file_under_a_diff() {
    let (_, mut ide) = shown(12);
    let panel = Rect::from_min_size(egui::Pos2::ZERO, vec2(card::PANEL_W, 400.0));
    let tabs = ide.panel_tabs(panel);
    assert_eq!(tabs.len(), 12);
    assert!(tabs.iter().all(|(r, _)| r.right() <= panel.right() - 8.0 + 0.01), "{:?}", tabs.last());
    // a diff comes forward: the panel keeps the last file; closing that one shows another from its top
    let mut diff = Tab::new(ide.tabs[0].path.clone(), String::new(), false);
    diff.diff = Some(Box::new(git::Diff::new("um\n", "dois\n")));
    ide.tabs.push(diff);
    ide.active = 12;
    assert_eq!(ide.card_tab(), Some(11));
    ide.panel_scroll = 40.0;
    ide.close_tab(11);
    assert_eq!((ide.card_tab(), ide.panel_scroll), (Some(10), 0.0));
}

#[test]
fn a_tab_whose_file_goes_stays_marked_and_only_ctrl_s_writes_it_back() {
    let dir = std::env::temp_dir().join(format!("magoagent-ide-gone-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let f = dir.join("a.txt");
    std::fs::write(&f, "um").unwrap();
    let mut ide = Ide::new(egui::Context::default(), "g", &dir.to_string_lossy());
    ide.open_file(f.clone(), None);
    (ide.tabs[0].text, ide.tabs[0].dirty) = ("um dois".into(), true);
    std::fs::remove_file(&f).unwrap();
    ide.check_disk();
    assert!(ide.tabs[0].gone);
    // saving on the way out leaves it deleted; Ctrl+S writes it again
    ide.save_all();
    assert!(!f.exists());
    ide.save(0, true);
    ide.check_disk();
    assert!(!ide.tabs[0].gone);
    assert_eq!(std::fs::read_to_string(&f).unwrap(), "um dois");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn saving_keeps_the_line_being_typed_in_a_diff() {
    let dir = std::env::temp_dir().join(format!("magoagent-ide-diffsave-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let f = dir.join("a.txt");
    std::fs::write(&f, "um\ndois\n").unwrap();
    let mut ide = Ide::new(egui::Context::default(), "d", &dir.to_string_lossy());
    ide.open_diff(f.clone());
    // a double click on line 2 of the right side, "DOIS" typed, then Ctrl+S without Enter
    ide.tabs[ide.active].diff.as_mut().unwrap().editing = Some((1, "DOIS".into()));
    ide.run(Cmd::SaveAll);
    assert_eq!(std::fs::read_to_string(&f).unwrap(), "um\nDOIS\n");
    let _ = std::fs::remove_dir_all(&dir);
}

/// An IDE on a missing folder (no servers, no IDE integration) with `tabs` empty text tabs.
fn shown(tabs: usize) -> (egui::Context, Ide) {
    let ctx = egui::Context::default();
    let mut fonts = egui::FontDefinitions::default();
    for (name, family) in [("ide", egui::FontFamily::Proportional), ("ide-mono", egui::FontFamily::Monospace)] {
        fonts.families.insert(egui::FontFamily::Name(name.into()), fonts.families[&family].clone());
    }
    ctx.set_fonts(fonts);
    let root = std::env::temp_dir().join(format!("magoagent-ui-{}", uuid::Uuid::new_v4()));
    let mut ide = Ide::new(ctx.clone(), "ui", &root.to_string_lossy());
    ide.code_vision = false;
    for i in 0..tabs {
        ide.tabs.push(Tab::new(root.join(format!("tabela_{i}.txt")), String::new(), false));
    }
    (ctx, ide)
}
/// `n` frames, `events` in the first; the last one's output.
fn frames(ctx: &egui::Context, ide: &mut Ide, events: Vec<egui::Event>, n: usize) -> egui::FullOutput {
    let mut out = None;
    for events in std::iter::once(events).chain(std::iter::repeat_with(Vec::new)).take(n) {
        let screen_rect = Some(Rect::from_min_size(egui::Pos2::ZERO, vec2(1000.0, 600.0)));
        let mut o = ctx.run_ui(egui::RawInput { screen_rect, events, ..Default::default() }, |ui| ide.ui(ui, true));
        o.textures_delta.clear();
        out = Some(o);
    }
    out.unwrap()
}

#[test]
fn many_tabs_show_the_active_one_and_scroll_with_the_wheel() {
    let (ctx, mut ide) = shown(30);
    ide.active = 29;
    // the selected tab's box and the clip it is painted in
    let selected = |out: egui::FullOutput| {
        out.shapes
            .iter()
            .find_map(|c| match &c.shape {
                Shape::Rect(r) if r.fill == t::TAB_SELECTED => Some((c.clip_rect, r.rect)),
                _ => None,
            })
            .expect("the selected tab is painted")
    };
    let (clip, tab) = selected(frames(&ctx, &mut ide, vec![], 40));
    assert!(clip.contains_rect(tab), "the last tab opens in view: {tab:?} in {clip:?}");
    let wheel = egui::Event::MouseWheel {
        unit: egui::MouseWheelUnit::Point,
        delta: vec2(0.0, 300.0),
        phase: egui::TouchPhase::Move,
        source: egui::MouseWheelSource::Wheel,
        modifiers: Modifiers::NONE,
    };
    let (_, scrolled) = selected(frames(&ctx, &mut ide, vec![egui::Event::PointerMoved(tab.center()), wheel], 40));
    assert!(scrolled.left() > tab.left() + 100.0, "the wheel scrolls the tabs back: {scrolled:?}");
}

#[test]
fn the_esc_that_closes_a_menu_keeps_the_ide() {
    let (ctx, mut ide) = shown(1);
    let esc = || egui::Event::Key { key: Key::Escape, physical_key: None, pressed: true, repeat: false, modifiers: Modifiers::NONE };
    frames(&ctx, &mut ide, vec![], 2);
    egui::Popup::open_id(&ctx, egui::Id::unique("menu"));
    frames(&ctx, &mut ide, vec![esc()], 1);
    assert!(!ide.wants_canvas, "the Esc closed the menu");
    // a popup nobody shows lasts one more frame
    frames(&ctx, &mut ide, vec![], 1);
    frames(&ctx, &mut ide, vec![esc()], 1);
    assert!(ide.wants_canvas, "a second Esc goes back to the canvas");
}

#[test]
fn the_esc_that_closes_the_comment_window_keeps_the_ide() {
    let (ctx, mut ide) = shown(1);
    let esc = || egui::Event::Key { key: Key::Escape, physical_key: None, pressed: true, repeat: false, modifiers: Modifiers::NONE };
    frames(&ctx, &mut ide, vec![], 2);
    ide.code_comment = Some((ide.tabs[0].path.clone(), 1, 1, String::new()));
    frames(&ctx, &mut ide, vec![], 2);
    frames(&ctx, &mut ide, vec![esc()], 1);
    assert!(ide.code_comment.is_none() && !ide.wants_canvas, "the Esc closed the window, not the IDE");
}

#[test]
fn the_project_panel_selects_on_a_click_and_opens_on_the_second() {
    let (ctx, mut ide) = shown(0);
    let dir = std::env::temp_dir().join(format!("magoagent-project-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    for f in ["a.txt", "b.txt", "c.txt"] {
        std::fs::write(dir.join(f), "").unwrap();
    }
    let mut tree = tree::TreeNode::new(dir.clone(), true);
    tree.open = true;
    ide.tree = Some(tree);
    let out = frames(&ctx, &mut ide, vec![], 2);
    let at = out
        .shapes
        .iter()
        .find_map(|c| match &c.shape {
            Shape::Text(s) if s.galley.text() == "b.txt" => Some(s.pos + vec2(4.0, 6.0)),
            _ => None,
        })
        .expect("b.txt is listed");
    let button = |pressed| egui::Event::PointerButton { pos: at, button: egui::PointerButton::Primary, pressed, modifiers: Modifiers::NONE };
    let click = || vec![egui::Event::PointerMoved(at), button(true), button(false)];
    let key = |key| vec![egui::Event::Key { key, physical_key: None, pressed: true, repeat: false, modifiers: Modifiers::NONE }];
    frames(&ctx, &mut ide, click(), 1);
    assert_eq!((ide.card_sel, ide.tabs.len()), (Some(1), 0), "a click selects b.txt");
    frames(&ctx, &mut ide, key(Key::ArrowDown), 1);
    frames(&ctx, &mut ide, key(Key::Enter), 1);
    assert_eq!((ide.card_sel, ide.tabs.last().map(|t| file_name(&t.path))), (Some(2), Some("c.txt".into())), "the arrows move it, Enter opens");
    frames(&ctx, &mut ide, vec![], 20);
    frames(&ctx, &mut ide, click(), 1);
    frames(&ctx, &mut ide, click(), 1);
    assert_eq!(ide.tabs.last().map(|t| file_name(&t.path)).as_deref(), Some("b.txt"), "a second click opens");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_shift_chord_is_not_taken_by_its_plain_shortcut() {
    let (ctx, mut ide) = shown(1);
    let key = |key, modifiers| egui::Event::Key { key, physical_key: None, pressed: true, repeat: false, modifiers };
    // Ctrl+Shift+F (Cmd on the Mac) finds in the files, Ctrl+F in the file
    frames(&ctx, &mut ide, vec![key(Key::F, Modifiers::COMMAND | Modifiers::SHIFT)], 1);
    assert!(ide.find_files.is_some() && ide.find.is_none());
    ide.find_files = None;
    frames(&ctx, &mut ide, vec![key(Key::F, Modifiers::COMMAND)], 1);
    assert!(ide.find.is_some() && ide.find_files.is_none());
}

#[test]
fn claude_code_opens_with_ctrl_esc_and_ctrl_alt_esc() {
    // Windows keeps Ctrl+Esc for the Start menu: its labels say Ctrl+Alt+Esc, the same binding;
    // the Mac takes Cmd, like the rest of the IDE's keys
    let (ctx, mut ide) = shown(1);
    for modifiers in [Modifiers::COMMAND, Modifiers::COMMAND | Modifiers::ALT] {
        ide.wants_claude = false;
        frames(&ctx, &mut ide, vec![egui::Event::Key { key: Key::Escape, physical_key: None, pressed: true, repeat: false, modifiers }], 1);
        assert!(ide.wants_claude, "{modifiers:?}");
    }
}
