//! Who is who among the agents (mascot/identity.rs): and, of the sessions outside the app, "fica sempre com o prefixo
//! claude code".

use super::*;
use crate::app::mascot::identity::own_color;

/// Two chats of one program ("Agente Telegram" and "Luna", both Codex's), their ids chosen so they want the same colour.
fn twins() -> (String, String) {
    let first = "c-0".to_string();
    let second = (1..500).map(|index| format!("c-{index}")).find(|id| own_color(id) == own_color(&first)).unwrap();

    (first, second)
}

fn session(app: &mut DrawingApp, id: &str, cwd: &str, origin: &str) {
    let mut payload = json!({ "hook_event_name": "UserPromptSubmit", "session_id": id, "cwd": cwd, "prompt": "oi" });

    payload["mago_origin"] = origin.into();
    app.take_hook(&format!("ext:{id}"), &payload, None);
}

#[test]
fn two_agents_of_one_program_get_two_colours_that_stay() {
    let (first, second) = twins();
    let mut app = DrawingApp::default();

    app.chats = vec![chat(&first, "codex", "Agente Telegram"), chat(&second, "codex", "Luna")];
    let colors = |app: &DrawingApp| app.running_entries().iter().map(|entry| (entry.id.clone(), entry.color)).collect::<Vec<_>>();
    let shown = colors(&app);

    assert_ne!(shown[0].1, shown[1].1, "the same program, two colours: {shown:?}");
    // the same in every place: the sidebar's, the conversation's and the card's ask `agent_color` too
    assert_eq!(app.agent_color(&first), shown[0].1);
    assert_eq!(app.agent_color(&second), shown[1].1);
    // from one run to the next and whatever the order of the list
    let mut again = DrawingApp::default();

    again.chats = vec![chat(&second, "codex", "Luna"), chat(&first, "codex", "Agente Telegram")];
    assert_eq!((again.agent_color(&first), again.agent_color(&second)), (shown[0].1, shown[1].1));
    // a third comes: the two keep theirs
    app.chats.push(chat("c-novo", "claude", "Faísca"));
    assert_eq!((app.agent_color(&first), app.agent_color(&second)), (shown[0].1, shown[1].1));
    let third = app.agent_color("c-novo");

    assert!(third != shown[0].1 && third != shown[1].1);
    // the voice's Luna keeps her blue: no agent has it
    assert!(app.running_entries().iter().all(|entry| entry.color != crate::app::mascot::mago::LUME));
}

#[test]
fn no_two_agents_in_the_list_show_the_same_name() {
    let mut app = DrawingApp::default();

    app.chats = vec![chat("a", "codex", "Luna"), chat("b", "codex", "Luna"), chat("c", "claude", "Luna 2")];
    let names: Vec<String> = app.running_entries().into_iter().map(|entry| entry.name).collect();

    assert_eq!(names, ["Luna", "Luna 3", "Luna 2"]);
    // their cards on the canvas say the same
    assert_eq!((app.chat_card_name("a"), app.chat_card_name("b"), app.chat_card_name("nada")), (Some("Luna".into()), Some("Luna 3".into()), None));
}

#[test]
fn a_session_outside_shows_its_folder_without_the_program_before_it() {
    let mut app = DrawingApp::default();

    session(&mut app, "s1", "/var/www/projects/magoagent/.claude/worktrees/agent-a47cc47699b17a639", "");
    session(&mut app, "s2", "/var/www/projects/magoagent/.claude/worktrees/agent-ac4efc7d75cc082a9", "");
    // the same folder in Claude Code and in Claude Desktop: two pills
    session(&mut app, "s3", "C:/Users/eu/loja", "");
    session(&mut app, "s4", "C:/Users/eu/loja", "claude-desktop");
    let entries = app.running_entries();
    let names: Vec<&str> = entries.iter().map(|entry| entry.name.as_str()).collect();

    assert_eq!(names, ["agent-a47cc47", "agent-ac4efc7", "loja", "loja 2"]);
    assert!(entries.iter().all(|entry| !entry.name.starts_with("Claude Code")), "{names:?}");
    // the program goes under the name when it is not Claude Code, the group's usual one
    assert_eq!(entries[3].last, "Claude Desktop · oi");
    assert_eq!(entries[2].last, "oi");
    // two Claude Code sessions side by side never in one colour; Claude Desktop's keeps its own
    assert_ne!(entries[0].color, entries[1].color);
    assert_eq!(entries[3].color, rgb_hex(0xd97757));
    // the Maestro still hears them whole (`demands`)
    assert!(app.outside_report().contains("- Claude Desktop · loja 2 — "), "{}", app.outside_report());
}

#[test]
fn a_session_in_claude_desktops_colour_moves_the_agent_that_had_one_like_it() {
    let mut app = DrawingApp::default();
    let orange = rgb_hex(0xf97316);
    let folder = (0..500).map(|index| format!("/w/p{index}")).find(|cwd| own_color(&format!("pill:code:{cwd}")) == orange).unwrap();

    session(&mut app, "s1", &folder, "");
    assert_eq!(app.running_entries()[0].color, orange);
    // Claude Desktop's orange comes, in the same folder: the Claude Code one no longer looks like it
    session(&mut app, "s2", &folder, "claude-desktop");
    let colors: Vec<Color32> = app.running_entries().iter().map(|entry| entry.color).collect();

    assert_eq!(colors[1], rgb_hex(0xd97757));
    assert!(colors[0] != orange && colors[0] != rgb_hex(0xef4444), "{colors:?}");
}

#[test]
fn a_recruit_takes_no_name_an_agent_off_view_has() {
    let mut app = DrawingApp::default();
    // every name but one taken, that one by a terminal in another workspace
    let last = AGENT_NAMES[AGENT_NAMES.len() - 1];

    for name in &AGENT_NAMES[..AGENT_NAMES.len() - 1] {
        app.chats.push(crate::app::chat::direct::Chat { recruit: true, ..chat(&name.to_lowercase(), "claude", name) });
    }

    app.away.insert("longe".into(), ("outro-espaço".into(), TerminalSpec { name: last.to_lowercase(), ..Default::default() }));
    let name = app.agent_name();

    assert!(name.ends_with(" 2"), "all taken, in any case: numbered ({name})");
    assert!(app.chats.iter().all(|chat| chat.name != name) && last.to_lowercase() != name.to_lowercase());
}
