//! A conversation kept after /clear: what was said so far becomes a chat of its own in the list, by its title, with the
//! agent's session to go on from and no card on the canvas; the chat that was cleared keeps its id (its card, its ropes,
//! its team) and starts a new session.

use super::*;

/// The name of a non-Maestro chat's earlier conversation: its own name stays with it (the team calls it by that name).
fn earlier_name(name: &str) -> String {
    format!("{name} (antes do /clear)")
}

/// What heads a conversation that /clear started, a line across it with the time: "Nova sessão · 14:32".
const NEW_SESSION: &str = "Nova sessão";

/// The line that heads a new session, at `ms` since 1970.
pub(crate) fn new_session_mark(ms: i64) -> String {
    crate::app::chat::viewer::local_clock(ms).map_or_else(|| NEW_SESSION.to_string(), |clock| format!("{NEW_SESSION} · {clock}"))
}

/// Whether a note of the conversation is that line.
pub(crate) fn is_new_session_mark(text: &str) -> bool {
    text == NEW_SESSION || text.strip_prefix(NEW_SESSION).and_then(|rest| rest.strip_prefix(" · ")).is_some_and(|clock| !clock.contains('\n'))
}

impl DrawingApp {
    /// Chat `id`'s conversation so far as a chat of its own, right after it in the list: its title, its session, what
    /// the app keeps of it (a Codex chat's file), no card. The chat itself then has no session, and the next message
    /// names it again (a recruit keeps its name). The title it went by; None when nothing was said yet.
    pub(crate) fn archive_conversation(&mut self, id: &str) -> Option<String> {
        let index = self.chats.iter().position(|chat| chat.id == id && chat.session.is_some())?;
        let chat = self.chats[index].clone();
        // as the list shows it: the name given (or taken from the first message), else the conversation's own title
        let own = self.transcripts.get(id).and_then(|transcript| transcript.title.clone()).filter(|_| !chat.named);
        let title = own.unwrap_or_else(|| chat.name.clone());
        let title = if chat.recruit { earlier_name(&title) } else { title };
        let earlier = Chat { id: uuid::Uuid::new_v4().simple().to_string(), name: title.clone(), named: true, draft: false, ..chat.clone() };
        // what the app keeps of a Codex conversation goes with it (Claude Code's is the session file itself)
        let items = |chat: &str| self.autosave.as_ref().map(|autosave| autosave.with_file_name("chats").join(format!("{chat}.json")));

        if let (Some(from), Some(to)) = (items(id), items(&earlier.id))
            && from.is_file()
        {
            let _ = fs::rename(from, to);
        }

        if let Some(transcript) = self.transcripts.remove(id) {
            self.transcripts.insert(earlier.id.clone(), transcript);
        }

        let earlier_id = earlier.id.clone();

        self.chats.insert(index + 1, earlier);
        self.save_chat_items(&earlier_id);
        let cleared = &mut self.chats[index];

        cleared.session = None;

        if !cleared.recruit {
            // named again by its next first message, as a new chat is
            let program = cleared.agent.clone();
            let preset = self.prefs.presets.iter().find(|preset| preset.command.split_whitespace().next() == Some(program.as_str()));
            let agent = preset.map_or_else(|| if program == "codex" { "Codex".to_string() } else { "Claude Code".to_string() }, |preset| preset.name.clone());

            (cleared.name, cleared.named) = (chat_name(&agent, &cleared.cwd), false);
        }

        self.save_chats();
        Some(title)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A chat of `agent` with its card on the canvas, in a folder of its own beside a drawing of its own; (app, chat, folder).
    fn talked(agent: (&str, &str), name: &str) -> (DrawingApp, String, PathBuf) {
        let dir = std::env::temp_dir().join(format!("mago-clear-{}", uuid::Uuid::new_v4().simple()));

        fs::create_dir_all(&dir).unwrap();
        let mut app = DrawingApp { autosave: Some(dir.join("autosave.mago")), ..Default::default() };

        app.create_chat(&NewChat { agent: 0, cwd: "~/projeto".into(), mode: 0 }, &[Preset::new(agent.0, agent.1)]);
        let id = app.chat_agent.clone().unwrap();

        (app.chats[0].session, app.chats[0].name, app.chats[0].named) = (Some("t1".into()), name.into(), true);
        app.chat_transcript(&id).items = vec![ChatItem::User("arrume o login".into()), ChatItem::Agent("pronto, arrumei".into())];
        app.save_chat_items(&id);
        app.save_chats();
        (app, id, dir)
    }

    #[test]
    fn clear_keeps_the_conversation_in_the_list_as_a_chat_of_its_own_and_the_card_starts_over() {
        let (mut app, id, dir) = talked(("Codex", "codex"), "Arrumar o login");
        let ctx = Context::default();

        assert!(app.chat_submit(&ctx, &id, "/clear", "/clear"));
        assert_eq!(app.chats.len(), 2);
        // the chat cleared: the same id and card, no session, named again by its next first message
        let (cleared, earlier) = (&app.chats[0], &app.chats[1]);

        assert_eq!((cleared.id.as_str(), cleared.session.as_deref(), cleared.name.as_str(), cleared.named), (id.as_str(), None, "Codex · projeto", false));
        assert_eq!(app.scene.elements.iter().filter(|element| element.chat.is_some()).count(), 1, "no card vanished nor came");
        assert!(app.chat_node(&id).is_some() && app.chat_node(&earlier.id).is_none());

        // the conversation so far: its title, its session to resume, what was said
        assert_eq!(
            (earlier.name.as_str(), earlier.session.as_deref(), earlier.agent.as_str(), earlier.recruit),
            ("Arrumar o login", Some("t1"), "codex", false)
        );

        let earlier = earlier.id.clone();

        assert!(app.transcripts[&earlier].items.contains(&ChatItem::Agent("pronto, arrumei".into())));
        // a new session, said at its top with the time, and where the one before went
        let items = &app.transcripts[&id].items;

        assert!(
            matches!(&items[..], [ChatItem::System(mark), ChatItem::System(note)] if is_new_session_mark(mark) && note.contains("Conversas") && note.contains("Arrumar o login")),
            "{items:?}"
        );

        // a restart finds both, and the Codex conversation in the earlier one's file
        let (saved, _) = crate::file::read_chats(&dir.join("chats.json"));

        assert_eq!(
            saved.iter().map(|chat| (chat.id.clone(), chat.session.clone())).collect::<Vec<_>>(),
            [(id.clone(), None), (earlier.clone(), Some("t1".into()))]
        );

        let items = |chat: &str| fs::read_to_string(dir.join("chats").join(format!("{chat}.json"))).unwrap_or_default();

        assert!(items(&earlier).contains("pronto, arrumei") && !items(&id).contains("pronto, arrumei"));
        // and it goes on where it was: its next message resumes the session
        assert!(chat_args("codex", 0, "t1", app.chats[1].session.is_some()).ends_with(&["resume".into(), "t1".into(), "-".into()]));
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_new_session_is_headed_by_its_line_and_its_time() {
        let mark = new_session_mark(now_ms());

        assert!(mark.starts_with("Nova sessão") && is_new_session_mark(&mark), "{mark}");
        assert!(is_new_session_mark("Nova sessão · 14:32") && is_new_session_mark("Nova sessão"));

        for other in ["Nova sessão começou", "Nova sessão · 14:32\nmais", "Conversa anterior", "A conversa anterior ficou na lista"] {
            assert!(!is_new_session_mark(other), "{other}");
        }
    }

    #[test]
    fn a_recruit_keeps_its_name_and_a_chat_never_talked_to_leaves_nothing_behind() {
        let (mut app, id, dir) = talked(("Claude Code", "claude"), "Revisor");

        app.chats[0].recruit = true;
        app.chat_clear(&id);
        assert_eq!((app.chats[0].name.as_str(), app.chats[0].session.as_deref()), ("Revisor", None), "the team calls it by its name");
        assert_eq!((app.chats[1].name.as_str(), app.chats[1].recruit), ("Revisor (antes do /clear)", true));
        // cleared again with nothing said since: no second entry
        app.chat_clear(&id);
        assert_eq!(app.chats.len(), 2);
        let _ = fs::remove_dir_all(&dir);
    }
}
