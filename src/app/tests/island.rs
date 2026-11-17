//! The island: the pill with the summary, opening with the pointer or a question, the new chat in a workspace of its own.

use super::*;
use crate::app::chat::decide::PersonAsk;
use crate::app::island::{CLOSED_H, CLOSED_W, COMPACT_H, COMPACT_W, EXPANDED_W, Tab};

const SIZE: Vec2 = Vec2::new(1200.0, 800.0);

fn chat(id: &str, agent: &str, name: &str) -> crate::app::chat::direct::Chat {
    crate::app::chat::direct::Chat {
        id: id.into(),
        agent: agent.into(),
        name: name.into(),
        cwd: "C:/p".into(),
        session: None,
        mode: 0,
        model: String::new(),
        effort: String::new(),
        recruit: false,
        named: false,
        draft: false,
    }
}

fn app_with_island() -> DrawingApp {
    let mut app = DrawingApp::default();
    app.prefs.island = true;
    app.chats = vec![chat("a", "claude", "Claude Code · loja"), chat("b", "codex", "Codex · site")];
    app
}

fn frame_at(app: &mut DrawingApp, ctx: &Context, time: f64, events: Vec<egui::Event>) {
    let raw = egui::RawInput { screen_rect: Some(Rect::from_min_size(Pos2::ZERO, SIZE)), time: Some(time), events, ..Default::default() };
    let mut output = ctx.run_ui(raw, |_| app.island_ui(ctx));
    output.textures_delta.clear();
}

/// Frames from `from` to `to` seconds, 50 ms apart.
fn run(app: &mut DrawingApp, ctx: &Context, from: f64, to: f64, events: Vec<egui::Event>) {
    let mut time = from;
    while time < to {
        frame_at(app, ctx, time, events.clone());
        time += 0.05;
    }
}

/// One frame of the island; the texts it painted.
fn painted(app: &mut DrawingApp, ctx: &Context, time: f64) -> Vec<String> {
    let raw = egui::RawInput { screen_rect: Some(Rect::from_min_size(Pos2::ZERO, SIZE)), time: Some(time), events: vec![], ..Default::default() };
    let mut output = ctx.run_ui(raw, |_| app.island_ui(ctx));
    output.textures_delta.clear();
    output.shapes.iter().filter_map(|shape| if let egui::Shape::Text(text) = &shape.shape { Some(text.galley.text().to_string()) } else { None }).collect()
}

/// One frame of the island, and the time it asked before the next one.
fn frame_delay(app: &mut DrawingApp, ctx: &Context, time: f64) -> std::time::Duration {
    let raw = egui::RawInput { screen_rect: Some(Rect::from_min_size(Pos2::ZERO, SIZE)), time: Some(time), events: vec![], ..Default::default() };
    let mut output = ctx.run_ui(raw, |_| app.island_ui(ctx));
    output.textures_delta.clear();
    output.viewport_output.get(&egui::ViewportId::ROOT).map_or(std::time::Duration::MAX, |viewport| viewport.repaint_delay)
}

/// Asked ("como a Dynamic Island do iPhone quando toca música"): the pill's bars rise with the song, each with its own
/// swing; without the groove (the dance off, or macOS), the clock's smooth sway.
#[test]
fn the_pills_bars_rise_with_the_song_each_with_its_own_swing() {
    use crate::app::integrations::spotify::groove::{Dance, Groove};
    use crate::app::island::pill::bars;
    let groove = |energy, bass| Groove { bpm: 120.0, phase: 0.25, energy, bass, bands: None, dance: Dance::Sway, beat: 0.5, since: 0.0 };
    let loud = bars(Some(&groove(0.9, 0.7)), 1.0);
    let quiet = bars(Some(&groove(0.1, 0.0)), 1.0);
    let average = |heights: &[f32]| heights.iter().sum::<f32>() / heights.len() as f32;
    assert!(loud.iter().all(|height| (0.0..=1.0).contains(height)));
    assert!(average(&loud) > average(&quiet), "the louder the song, the higher the bars: {loud:?} against {quiet:?}");
    assert!(loud.windows(2).any(|pair| (pair[0] - pair[1]).abs() > 0.02), "each bar with its own swing: {loud:?}");
    // no groove: the clock moves them, smoothly and on its own
    let first = bars(None, 2.0);
    assert!(first.iter().all(|height| (0.15..0.95).contains(height)), "{first:?}");
    assert_ne!(first, bars(None, 2.4), "the sway goes on");
    assert!(first.windows(2).any(|pair| (pair[0] - pair[1]).abs() > 0.02), "each bar with its own swing: {first:?}");
}

/// Asked ("Luna à esquerda, capa e frequência no meio, os mini agentes à direita; sem mini agentes, à direita"): the
/// cover and the bars go together, in the middle between the wizard and the minis, or at the right without minis.
#[test]
fn the_cover_and_the_bars_sit_in_the_middle_or_at_the_right() {
    use crate::app::island::minis::mini_cells;
    use crate::app::island::pill::spread;
    let pill = Rect::from_min_size(Pos2::ZERO, Vec2::new(288.0, 40.0));
    // the bars take 52 (ten of 2.5 with 3 between); no agents: the pair ends as far from the right as the wizard starts from the left (25)
    let (cover, bars) = spread(pill, None);
    assert!(bars > cover + 22.0, "the bars after the cover");
    assert!((bars + 52.0 - (288.0 - 25.0)).abs() < 1.0, "at the right: the bars end at {}", bars + 52.0);
    // with agents: the pair's middle halfway between the wizard (55) and the minis' grid
    let grid = mini_cells(pill, 2).into_iter().reduce(|grid, cell| grid.union(cell)).unwrap();
    let (cover, bars) = spread(pill, Some(grid));
    let middle = (cover + bars + 52.0) / 2.0;
    assert!((middle - (55.0 + grid.left()) / 2.0).abs() < 1.0, "in the middle: {middle} between 55 and {}", grid.left());
    assert!(bars + 52.0 < grid.left(), "clear of the minis");
}

/// Asked ("ali a Luna não reage à música"): open on Início, while the song plays she dances too, and her dance asks for
/// the listening; paused, nothing asks for it.
#[test]
fn open_on_home_luna_dances_to_the_song() {
    use crate::app::integrations::spotify::{Now, Track};
    let ctx = Context::default();
    let mut app = app_with_island();
    app.prefs.integrations.spotify = true;
    app.integrations.spotify.now = Some(Now { track: Track::new("505", "Arctic Monkeys", "", "").unwrap(), playing: true, ..Default::default() });
    app.island_show_page(Tab::Home);
    run(&mut app, &ctx, 1.0, 1.4, vec![]);
    assert_eq!(app.island.size().x, EXPANDED_W, "open");
    app.integrations_tick(&ctx);
    assert_eq!(app.integrations.spotify.dancer.starts, u32::from(!cfg!(target_os = "macos")), "her dance asks to hear the song");
}

/// While the song plays the pill is alive and keeps asking for frames; paused, it asks for none.
#[test]
fn the_pill_asks_for_frames_while_the_song_plays_and_not_paused() {
    use crate::app::integrations::spotify::{Now, Track};
    let ctx = Context::default();
    let mut app = app_with_island();
    // with no dance, the frames while it plays are the bars'
    app.prefs.integrations.luna_dances = false;
    run(&mut app, &ctx, 1.0, 1.3, vec![]);
    assert_eq!(app.island.size(), Vec2::new(CLOSED_W, CLOSED_H), "the notch, with nothing waiting");
    let mut now = Now { track: Track::new("Canção", "Banda", "", "").unwrap(), playing: true, ..Default::default() };
    app.integrations.spotify.now = Some(now.clone());
    let playing = frame_delay(&mut app, &ctx, 2.0);
    assert!(playing <= std::time::Duration::from_millis(33), "the bars are moving: a frame at the island's rate ({playing:?})");
    now.playing = false;
    app.integrations.spotify.now = Some(now);
    let paused = frame_delay(&mut app, &ctx, 2.2);
    assert!(paused >= std::time::Duration::from_millis(60), "paused: only the wizard's slow blink ({paused:?})");
    app.integrations.spotify.now = None;
    assert_eq!(paused, frame_delay(&mut app, &ctx, 2.4), "paused, the pill asks for no frame, as with no song");
}

/// Asked ("se eu clicar na música ele me leva para a tab da música"): a click on the cover in the notch opens the
/// island on Música.
#[test]
fn a_click_on_the_songs_cover_opens_the_island_on_the_music() {
    use crate::app::integrations::spotify::{Now, Track};
    let ctx = Context::default();
    let mut app = app_with_island();
    app.prefs.integrations.spotify = true;
    app.integrations.spotify.now = Some(Now { track: Track::new("505", "Arctic Monkeys", "", "").unwrap(), playing: false, ..Default::default() });
    run(&mut app, &ctx, 1.0, 1.3, vec![]);
    assert_eq!(app.island.size(), Vec2::new(CLOSED_W, CLOSED_H), "the notch");
    // the cover sits 4 from the notch's left, 16 wide
    let cover = Pos2::new(SIZE.x / 2.0 - CLOSED_W / 2.0 + 12.0, CLOSED_H / 2.0);
    let press = |pressed| egui::Event::PointerButton { pos: cover, button: egui::PointerButton::Primary, pressed, modifiers: Default::default() };
    frame_at(&mut app, &ctx, 1.4, vec![egui::Event::PointerMoved(cover), press(true)]);
    frame_at(&mut app, &ctx, 1.45, vec![press(false)]);
    run(&mut app, &ctx, 1.5, 3.5, vec![egui::Event::PointerMoved(cover)]);
    assert_eq!((app.island.size().x, app.island.tab()), (EXPANDED_W, Tab::Music), "open on Música");
}

/// Asked ("tem momento que a Luna fica parada como se nada estivesse tocando"): an agent at work or done does not stop
/// her dance; what needs the person does.
#[test]
fn luna_dances_through_the_agents_work_and_stops_for_the_person() {
    use crate::app::island::paint::dances_through;
    use crate::app::mascot::mago::MagoState;
    for state in [MagoState::Idle, MagoState::Sleeping, MagoState::Working, MagoState::Done] {
        assert!(dances_through(state), "{state:?}");
    }
    for state in [MagoState::NeedsYou, MagoState::Asking, MagoState::Error] {
        assert!(!dances_through(state), "{state:?}");
    }
}

/// Asked ("o dono quer as duas: as do canvas (terminais do app rodando Claude Code e chats do app com agente claude) e
/// as de fora"): Claude Code's page shows the app's Claude agents, each saying it is on the canvas, and not the others.
#[test]
fn the_claude_page_has_the_apps_agents_on_the_canvas() {
    let ctx = Context::default();
    let mut app = app_with_island();
    app.island_show_page(Tab::Claude);
    run(&mut app, &ctx, 1.0, 1.4, vec![]);
    assert_eq!(app.island.size().x, EXPANDED_W, "open on Claude Code");
    let texts = painted(&mut app, &ctx, 1.5);
    assert!(texts.iter().any(|text| text == "Claude Code · no canvas"), "{texts:?}");
    assert!(texts.iter().any(|text| text == "loja"), "the chat's name in the grid: {texts:?}");
    assert!(!texts.iter().any(|text| text.contains("site")), "the app's Codex is no Claude Code session ({:?}): {texts:?}", app.island.tab());
}

/// Asked ("se os hooks estão instalados aqui precisa saber"): the empty Claude Code page says so when they are in.
#[test]
fn the_empty_claude_page_knows_whether_the_hooks_are_in() {
    use crate::app::island::home::empty_words;
    assert!(empty_words(Tab::Claude, false).1.starts_with("Instale os hooks"));
    assert!(empty_words(Tab::Claude, true).1.starts_with("Os hooks estão instalados"));
}

#[test]
fn the_island_is_a_notch_that_opens_with_the_pointer_and_folds_when_it_leaves() {
    let ctx = Context::default();
    let mut app = app_with_island();
    run(&mut app, &ctx, 1.0, 1.3, vec![]);
    assert_eq!(app.island.size(), Vec2::new(CLOSED_W, CLOSED_H), "closed: the small notch");
    // it hangs from the top centre: the pointer on it shows compact, a click opens the panel
    let on_it = Pos2::new(SIZE.x / 2.0, CLOSED_H / 2.0);
    run(&mut app, &ctx, 2.0, 2.5, vec![egui::Event::PointerMoved(on_it)]);
    assert!((app.island.size().x - COMPACT_W).abs() < 4.0, "compact on hover: {}", app.island.size().x);
    let press = |pressed| egui::Event::PointerButton { pos: on_it, button: egui::PointerButton::Primary, pressed, modifiers: Default::default() };
    frame_at(&mut app, &ctx, 2.6, vec![press(true)]);
    frame_at(&mut app, &ctx, 2.65, vec![press(false)]);
    run(&mut app, &ctx, 2.7, 4.0, vec![egui::Event::PointerMoved(on_it)]);
    assert_eq!(app.island.size().x, EXPANDED_W, "open");
    assert_eq!(app.island.size().y, 270.0, "Início: the summary and the list");
    // the pointer left: it stays 15 s, then folds to compact
    frame_at(&mut app, &ctx, 4.05, vec![egui::Event::PointerGone]);
    run(&mut app, &ctx, 4.1, 18.0, vec![]);
    assert!(app.island.size().x > COMPACT_W + 50.0, "still open under 15 s");
    run(&mut app, &ctx, 20.0, 22.0, vec![]);
    assert!((app.island.size().x - COMPACT_W).abs() < 4.0, "folded to compact after 15 s: {}", app.island.size().x);
    run(&mut app, &ctx, 80.0, 82.0, vec![]);
    assert_eq!(app.island.size(), Vec2::new(CLOSED_W, CLOSED_H), "60 s after folding to compact, it returns to the notch");
    // off: nothing is kept
    app.prefs.island = false;
    frame_at(&mut app, &ctx, 7.0, vec![]);
    assert_eq!(app.island.size(), Vec2::ZERO);
}

#[test]
fn opened_it_folds_after_the_cursor_left_even_though_egui_never_heard_it_leave() {
    // Windows: outside the island the window lets the cursor through, so egui keeps the last pointer on it
    let ctx = Context::default();
    let mut app = app_with_island();
    let on_it = Pos2::new(SIZE.x / 2.0, CLOSED_H / 2.0);
    app.island.over = Some(true);
    run(&mut app, &ctx, 1.0, 1.5, vec![egui::Event::PointerMoved(on_it)]);
    let press = |pressed| egui::Event::PointerButton { pos: on_it, button: egui::PointerButton::Primary, pressed, modifiers: Default::default() };
    frame_at(&mut app, &ctx, 1.6, vec![press(true)]);
    frame_at(&mut app, &ctx, 1.65, vec![press(false)]);
    run(&mut app, &ctx, 1.7, 3.0, vec![]);
    assert_eq!(app.island.size().x, EXPANDED_W, "open");
    // the system's cursor went elsewhere, with no PointerGone: 15 s later it folds
    app.island.over = Some(false);
    run(&mut app, &ctx, 3.0, 17.0, vec![]);
    assert_eq!(app.island.size().x, EXPANDED_W, "still open under 15 s");
    run(&mut app, &ctx, 17.0, 19.5, vec![]);
    assert!((app.island.size().x - COMPACT_W).abs() < 4.0, "folded to compact: {}", app.island.size().x);
}

#[test]
fn closed_the_wizard_looks_around_by_itself_and_awake_its_eyes_are_on_the_pointer() {
    use crate::app::mascot::mago::mago_gaze;
    let ctx = Context::default();
    let mut app = app_with_island();
    // closed, the pointer far to the right of it: not followed
    let far = Pos2::new(SIZE.x - 10.0, 400.0);
    run(&mut app, &ctx, 1.0, 2.0, vec![egui::Event::PointerMoved(far)]);
    assert_eq!(app.island.size(), Vec2::new(CLOSED_W, CLOSED_H));
    let alone = mago_gaze("island-main").unwrap();
    assert!(alone.x.abs() <= 0.5 && alone.y.abs() <= 0.2, "its own slow wander: {alone:?}");
    // the pointer comes to it and goes away again: awake (compact), the eyes go after the pointer
    let on_it = Pos2::new(SIZE.x / 2.0, CLOSED_H / 2.0);
    run(&mut app, &ctx, 2.0, 2.2, vec![egui::Event::PointerMoved(on_it)]);
    run(&mut app, &ctx, 2.2, 4.0, vec![egui::Event::PointerMoved(far)]);
    assert_eq!(app.island.size(), Vec2::new(COMPACT_W, COMPACT_H));
    let following = mago_gaze("island-main").unwrap();
    assert!(following.x > 0.9, "on the pointer, far to the right: {following:?}");
}

#[test]
fn a_question_takes_the_island_over() {
    let ctx = Context::default();
    let mut app = app_with_island();
    run(&mut app, &ctx, 1.0, 1.3, vec![]);
    assert_eq!(app.island.size().x, CLOSED_W);
    let (reply, _answers) = std::sync::mpsc::channel();
    app.person_asks.get_mut().push(PersonAsk::new("a".into(), &json!({ "tool_name": "Bash", "input": { "command": "rm -rf x" } }), reply));
    frame_at(&mut app, &ctx, 2.0, vec![]);
    assert!(app.island.size().x > CLOSED_W, "it grows with no pointer on it");
}

/// An app on a drawing of its own in a fresh folder, with an agent installed.
fn app_in(name: &str) -> (DrawingApp, PathBuf, Vec<Preset>) {
    let dir = std::env::temp_dir().join(format!("mago-ilha-{name}-{}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    // today's drawing already there: with none, it is a first run and there is no space yet
    fs::write(dir.join("autosave.mago"), r#"{"version":1,"elements":[]}"#).unwrap();
    let mut app = DrawingApp { autosave: Some(dir.join("autosave.mago")), ..Default::default() };
    app.load_spaces();
    app.load_autosave();
    (app, dir, vec![Preset::new("Claude Code", "claude")])
}

#[test]
fn a_new_chat_gets_a_workspace_of_its_own_and_the_one_in_progress_stays() {
    use crate::app::island::NewForm;
    let (mut app, dir, installed) = app_in("novo");
    // the work in progress in the main space
    let element = app.new_element(ElementKind::Rectangle { x: 10.0, y: 10.0, width: 50.0, height: 50.0 });
    app.snapshot();
    app.scene.elements.push(element);
    let project = dir.join("loja");
    fs::create_dir_all(&project).unwrap();
    let form = NewForm { folder: project.to_string_lossy().into_owned(), ..Default::default() };
    let message = app.island_new_chat(&form, &installed).unwrap();
    assert!(message.contains("loja"), "{message}");
    assert_eq!(app.current_space().map(|space| space.name.as_str()), Some("loja"), "the new space is the one shown");
    assert_eq!(app.chats.len(), 1);
    assert_eq!(app.chats[0].cwd, project.to_string_lossy());
    assert_eq!(app.chat_agent.as_deref(), Some(app.chats[0].id.as_str()));
    assert!(!app.scene.elements.is_empty() && app.scene.elements.iter().all(|e| e.chat.is_some()), "only its card is on the new drawing");
    // the other space keeps its drawing
    app.switch_space("principal").unwrap();
    assert!(app.scene.elements.iter().any(|e| e.chat.is_none()), "the work in progress is there");
    // the same folder again: another space, not an error
    app.island_new_chat(&form, &installed).unwrap();
    assert_eq!(app.current_space().map(|space| space.name.as_str()), Some("loja 2"));
    // a folder that is not there, and no agent
    let gone = NewForm { folder: dir.join("nada").to_string_lossy().into_owned(), ..form.clone() };
    assert!(app.island_new_chat(&gone, &installed).unwrap_err().contains("não existe"));
    assert!(app.island_new_chat(&form, &[]).unwrap_err().contains("Nenhum agente"));
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn a_new_chat_can_run_in_a_worktree_of_its_own() {
    use crate::app::island::NewForm;
    let (mut app, dir, installed) = app_in("worktree");
    let repo = dir.join("repo");
    fs::create_dir_all(&repo).unwrap();
    let git = |args: &[&str]| crate::editor::run_git(&repo, args);
    // no git on this machine: nothing to check here
    if git(&["init"]).is_err() {
        return;
    }
    fs::write(repo.join("a.txt"), "a").unwrap();
    git(&["add", "-A"]).unwrap();
    git(&["-c", "user.name=t", "-c", "user.email=t@t", "commit", "-m", "base"]).unwrap();
    // changes in progress in the repository
    fs::write(repo.join("a.txt"), "mudou").unwrap();
    let form = NewForm { name: "Revisão".into(), folder: repo.to_string_lossy().into_owned(), worktree: true, ..Default::default() };
    let message = app.island_new_chat(&form, &installed).unwrap();
    assert!(message.contains("worktree"), "{message}");
    let cwd = PathBuf::from(&app.chats[0].cwd);
    // the Windows runner gives TEMP as a short 8.3 path and git answers with the long one: compare the end
    assert!(cwd.parent().is_some_and(|parent| parent.ends_with(PathBuf::from(".magoagent").join("worktrees"))), "{}", cwd.display());
    assert_eq!(fs::read_to_string(cwd.join("a.txt")).unwrap(), "a", "the copy is of the commit");
    assert_eq!(fs::read_to_string(repo.join("a.txt")).unwrap(), "mudou", "the repository's changes stay");
    assert_eq!(app.current_space().map(|space| space.name.as_str()), Some("Revisão"));
    // not a repository: refused, and no space made
    let plain = dir.join("plain");
    fs::create_dir_all(&plain).unwrap();
    let spaces = app.spaces.list.len();
    let refused = NewForm { folder: plain.to_string_lossy().into_owned(), ..form };
    assert!(app.island_new_chat(&refused, &installed).unwrap_err().contains("repositório"));
    assert_eq!(app.spaces.list.len(), spaces);
    let _ = git(&["worktree", "remove", "--force", &cwd.to_string_lossy()]);
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn the_island_is_on_by_default_and_its_place_is_kept_in_the_prefs() {
    let prefs = Prefs::default();
    assert!(prefs.island && prefs.island_at.is_none());
    let mut saved = prefs.clone();
    saved.island = false;
    let back: Prefs = serde_json::from_str(&serde_json::to_string(&saved).unwrap()).unwrap();
    assert!(!back.island, "turned off, it stays off");
    saved.island = true;
    saved.island_at = Some([2880.0, 0.0]);
    let back: Prefs = serde_json::from_str(&serde_json::to_string(&saved).unwrap()).unwrap();
    assert!(back.island && back.island_at == Some([2880.0, 0.0]));
    // prefs from before the island still load, with it on
    let old: Prefs = serde_json::from_str("{}").unwrap();
    assert!(old.island);
}

#[test]
fn the_island_waits_for_the_welcome_to_be_done() {
    let ctx = Context::default();
    let mut app = app_with_island();
    app.welcome = Some(Default::default());
    run(&mut app, &ctx, 1.0, 2.0, vec![]);
    assert_eq!(app.island.size(), Vec2::ZERO, "no island over the welcome");
    // a session outside asking meanwhile: back to its terminal, no card waiting behind the welcome
    let (reply, answer) = std::sync::mpsc::channel();
    let payload =
        json!({ "hook_event_name": "PermissionRequest", "session_id": "s1", "cwd": "/tmp/loja", "tool_name": "Bash", "tool_input": { "command": "ls" } });
    app.take_outside_hook(&payload, Some(reply));
    assert!(app.person_asks.borrow().is_empty());
    assert!(answer.try_recv().is_ok(), "the hook was answered at once: the terminal asks");
    // through the welcome, the island comes (compact: the session outside is at work)
    app.welcome = None;
    run(&mut app, &ctx, 2.0, 3.0, vec![]);
    assert!((app.island.size().x - COMPACT_W).abs() < 4.0, "{:?}", app.island.size());
}

#[test]
fn an_agent_opened_from_the_island_shows_in_its_conversation_or_in_the_big_chat() {
    let mut app = app_with_island();
    app.island_show("b", 5.0);
    assert!(app.island.tab == crate::app::island::Tab::Chat && app.chat_agent.as_deref() == Some("b"));
    assert!(!app.raise_window, "the island has it: the app stays where it is");
    // with the chat screen open the agent opens there and the app's window comes up
    let mut app = app_with_island();
    app.chat_open = true;
    app.island_show("a", 5.0);
    assert!(app.raise_window && app.chat_agent.as_deref() == Some("a"));
}

#[test]
fn no_canvas_on_the_card_brings_the_app_up_on_the_agents_card() {
    let mut app = app_with_island();
    let mut card = app.new_element(ElementKind::Rectangle { x: 600.0, y: 0.0, width: 200.0, height: 120.0 });
    card.id = "c".into();
    app.scene.elements.push(card);
    app.chat_open = true;
    app.island_on_canvas("c");
    assert!(!app.chat_open && app.raise_window, "the canvas, with the app in front");
    assert_eq!((app.selected.clone(), app.terminal_focus.as_deref()), (vec![0], Some("c")));
}

/// Asked ("tem vezes q vai tudo para baixo, e a ilha some, precisa ter um botao nas configurações para voltar a ilha"):
/// whatever was saved, it goes back to the top of the screen, through Ajustes and through the MCP.
#[test]
fn the_island_lost_off_the_screen_comes_back_to_where_it_starts() {
    let ctx = Context::default();
    let mut app = app_with_island();
    app.prefs.island_at = Some([9000.0, 4000.0]);
    run(&mut app, &ctx, 0.0, 0.2, vec![]);
    app.reset_island();
    assert!(app.prefs.island_at.is_none(), "the saved place is forgotten");
    run(&mut app, &ctx, 0.2, 0.4, vec![]);
    assert_eq!(app.island_place(&ctx), None, "nothing saved yet: no place chosen in Ajustes");
    app.prefs.island_at = Some([9000.0, 4000.0]);
    let answer = app.act("set_setting", &serde_json::json!({ "name": "island_place", "value": "reset" }));
    assert!(answer.is_ok(), "{answer:?}");
    assert!(app.prefs.island_at.is_none());
}

/// Every page of the island and Início's "Ocultar tudo" / "Mostrar ocultos" through the MCP, as by the hand.
#[test]
fn the_mcp_opens_each_page_and_hides_the_summary() {
    let ctx = Context::default();
    let mut app = app_with_island();
    run(&mut app, &ctx, 0.0, 0.3, vec![]);
    for (name, tab) in [("claude", Tab::Claude), ("github", Tab::GitHub), ("spotify", Tab::Music), ("chats", Tab::Chats), ("home", Tab::Home)] {
        let answer = app.act("set_setting", &serde_json::json!({ "name": "island_page", "value": name }));
        assert!(answer.is_ok(), "{name}: {answer:?}");
        assert_eq!(app.island.tab(), tab, "{name}");
    }
    assert!(app.act("set_setting", &serde_json::json!({ "name": "island_page", "value": "nenhuma" })).is_err());
    app.act("set_setting", &serde_json::json!({ "name": "island_feed", "value": "hide_all" })).unwrap();
    app.act("set_setting", &serde_json::json!({ "name": "island_feed", "value": "show_hidden" })).unwrap();
}

/// The island as drawn: hung from the top centre.
fn island_rect(app: &DrawingApp) -> Rect {
    let size = app.island.size();
    Rect::from_min_size(Pos2::new((SIZE.x / 2.0 - size.x / 2.0).round(), 0.0), size)
}

/// A header button, counted from the right: 0 the close, 1 the sound, 2 the settings, 3 the voice, 4 the size.
fn right_button(app: &DrawingApp, index: usize) -> Pos2 {
    let rect = island_rect(app);
    Pos2::new(rect.right() - 25.0 - index as f32 * 32.0, rect.top() + 25.0)
}

/// A click as the hand makes it: the pointer comes, rests, presses, lets go; then a few frames for it to settle.
fn click(app: &mut DrawingApp, ctx: &Context, time: &mut f64, at: Pos2) {
    let press = |pressed| egui::Event::PointerButton { pos: at, button: egui::PointerButton::Primary, pressed, modifiers: Default::default() };
    for events in [vec![egui::Event::PointerMoved(at)], vec![egui::Event::PointerMoved(at)], vec![press(true)], vec![press(false)]] {
        frame_at(app, ctx, *time, events);
        *time += 0.05;
    }
    rest(app, ctx, time, at, 0.5);
}

/// The pointer still at `at` for `seconds`.
fn rest(app: &mut DrawingApp, ctx: &Context, time: &mut f64, at: Pos2, seconds: f64) {
    let end = *time + seconds;
    while *time < end {
        frame_at(app, ctx, *time, vec![egui::Event::PointerMoved(at)]);
        *time += 0.05;
    }
}

/// The app, its island opened by clicking on the compact pill, and the time it is at.
fn opened() -> (DrawingApp, Context, f64) {
    let ctx = Context::default();
    let mut app = DrawingApp::default();
    app.prefs.island = true;
    app.chats = vec![chat("a", "claude", "Claude Code · loja"), chat("b", "codex", "Codex · site")];
    let mut time = 1.0;
    let notch = Pos2::new(SIZE.x / 2.0, 10.0);
    while time < 2.0 {
        frame_at(&mut app, &ctx, time, vec![egui::Event::PointerMoved(notch)]);
        time += 0.05;
    }
    assert!((app.island.size().x - COMPACT_W).abs() < 4.0, "compact on hover: {}", app.island.size().x);
    let press = |pressed| egui::Event::PointerButton { pos: notch, button: egui::PointerButton::Primary, pressed, modifiers: Default::default() };
    frame_at(&mut app, &ctx, time, vec![press(true)]);
    time += 0.05;
    frame_at(&mut app, &ctx, time, vec![press(false)]);
    time += 0.05;
    while time < 3.0 {
        frame_at(&mut app, &ctx, time, vec![egui::Event::PointerMoved(notch)]);
        time += 0.05;
    }
    assert!((app.island.size().x - EXPANDED_W).abs() < 1.0, "clicked to open: {}", app.island.size().x);
    (app, ctx, time)
}

/// Opening the app's Ajustes from the island's settings page sets app.settings_open true, closes the island's
/// settings page, and folds the island back to compact.
#[test]
fn the_islands_settings_link_opens_app_settings_closes_island_settings_and_folds_to_compact() {
    let (mut app, ctx, mut time) = opened();
    // open the island's settings page via the gear button
    let gear = right_button(&app, 2);
    click(&mut app, &ctx, &mut time, gear);
    assert!(app.island.settings_open(), "the gear opens the island's settings page");
    assert_eq!(app.island.size().x, EXPANDED_W, "the island is expanded");
    // find and click the "Ajustes…" link in the island's settings page
    // the settings page is drawn inside the page area (PAGE_TOP = 42 from top, 10 from bottom)
    // the link is on the third row (centers[2]) at the right side of the page
    let rect = island_rect(&app);
    let page_top = rect.top() + 42.0; // PAGE_TOP
    let page_bottom = rect.bottom() - 10.0;
    let page_center_y = (page_top + page_bottom) / 2.0;
    // the three rows: 20, 24, 18 with 10 gap = 82 total, centered on page_center_y
    // centers[2] = page_center_y - 41 + 20 + 10 + 24 + 10 + 9 = page_center_y + 32
    let centers_2 = page_center_y + 32.0;
    let link_at = Pos2::new(rect.right() - 26.0 - 60.0, centers_2 - 8.0); // right - 16 - text_width
    click(&mut app, &ctx, &mut time, link_at);
    // the app's settings window is now open
    assert!(app.settings_open, "app.settings_open is true");
    // the island's settings page is closed
    assert!(!app.island.settings_open(), "the island's settings page is closed");
    // the island has folded back to compact
    rest(&mut app, &ctx, &mut time, link_at, 0.5);
    assert!((app.island.size().x - COMPACT_W).abs() < 4.0, "the island folded to compact: {:?}", app.island.size());
}
