use super::*;

#[test]
fn the_chat_reads_claude_code_files() {
    use crate::transcript::{ChatItem, Session, Transcript, assign, chat_items, project_folder};
    assert_eq!(project_folder("/Users/eu/.claude/my_proj"), "-Users-eu--claude-my-proj");
    let line = |v: Value| chat_items(&v);
    assert_eq!(line(json!({"type": "user", "message": {"content": "oi"}})), vec![ChatItem::User("oi".into())]);
    let reply = json!({"type": "assistant", "message": {"content": [
        {"type": "thinking", "thinking": "x"},
        {"type": "text", "text": "Feito."},
        {"type": "tool_use", "name": "Bash", "input": {"command": "ls -la\npwd", "description": "lista"}},
    ]}});
    let bash = crate::transcript::Step { name: "Bash".into(), what: "ls -la".into(), input: "ls -la\npwd".into(), ..Default::default() };
    assert_eq!(line(reply), vec![ChatItem::Agent("Feito.".into()), ChatItem::Tool(bash)]);
    // tool results, side conversations and bookkeeping lines stay out
    assert!(line(json!({"type": "user", "message": {"content": [{"type": "tool_result", "content": "ok"}]}})).is_empty());
    // but an image a tool gave back (a PNG read, a page's print) is the agent showing it
    let png = json!({"type": "image", "source": {"type": "base64", "media_type": "image/png", "data": "iVBO"}});
    let read = json!({"type": "user", "message": {"content": [{"type": "tool_result", "content": [png.clone(), {"type": "text", "text": "x"}]}]}});
    assert_eq!(line(read), vec![ChatItem::Image { data: "iVBO".into(), media: "image/png".into(), user: false, at: 0 }]);
    assert!(
        line(json!({"type": "user", "message": {"content": [{"type": "tool_result", "is_error": true, "content": [png.clone()]}]}}))
            .iter()
            .all(|i| !matches!(i, ChatItem::Image { .. }))
    );
    // in the order of their blocks
    let both = json!({"type": "user", "message": {"content": [{"type": "tool_result", "content": [png.clone()]}, {"type": "tool_result", "is_error": true, "content": "falhou"}]}});
    assert!(matches!(line(both).as_slice(), [ChatItem::Image { .. }, ChatItem::Error(_)]), "the image first, as it came");
    // MagoAgent's own print of the canvas stays out: the canvas is behind the chat
    let note = format!("3 elemento(s); área x=0 y=0 largura=10 altura=10 ({}, escala 1.00)", crate::transcript::VIEW_MARK);
    assert!(
        line(json!({"type": "user", "message": {"content": [{"type": "tool_result", "content": [png.clone(), {"type": "text", "text": note}]}]}})).is_empty()
    );
    assert!(line(json!({"type": "assistant", "isSidechain": true, "message": {"content": "x"}})).is_empty());
    assert!(line(json!({"type": "api-request", "message": {"content": "x"}})).is_empty());
    // a slash command shows as Claude Code's note, its other notes not at all; a "<" of the person's stays
    let command = "<command-message>model</command-message>\n<command-name>/model</command-name>\n<command-args>opus</command-args>";
    assert_eq!(line(json!({"type": "user", "message": {"content": command}})), vec![ChatItem::System("/model opus".into())]);
    assert!(line(json!({"type": "user", "message": {"content": "<local-command-stdout>ok</local-command-stdout>"}})).is_empty());
    assert_eq!(line(json!({"type": "user", "message": {"content": "<div> some"}})), vec![ChatItem::User("<div> some".into())]);
    let summary = json!({"type": "user", "isCompactSummary": true, "message": {"content": "This session is being continued..."}});
    assert_eq!(line(summary), vec![ChatItem::System("A conversa foi resumida para caber no contexto.".into())]);
    // what failed: the API, a tool
    let api = |n: u64| json!({"type": "system", "subtype": "api_error", "error": {"formatted": "529 Overloaded\nmais"}, "retryAttempt": n, "maxRetries": 10});
    assert_eq!(line(api(2)), vec![ChatItem::Error("Erro da API (tentativa 2 de 10): 529 Overloaded".into())]);
    let failed = json!({"type": "user", "message": {"content": [{"type": "tool_result", "is_error": true, "content": "<tool_use_error>File has not been read yet.</tool_use_error>"}]}});
    assert_eq!(line(failed), vec![ChatItem::Error("Ferramenta falhou: File has not been read yet.".into())]);
    let last = json!({"type": "assistant", "isApiErrorMessage": true, "message": {"model": "<synthetic>", "content": [{"type": "text", "text": "API Error: 529 Overloaded"}]}});
    assert_eq!(line(last.clone()), vec![ChatItem::Error("Erro da API: 529 Overloaded".into())]);
    // the person's refusal and interruption are no failure
    let refused = json!({"type": "user", "message": {"content": [{"type": "tool_result", "is_error": true, "content": "The user doesn't want to proceed with this tool use. The tool use was rejected."}]}});
    assert_eq!(line(refused), vec![ChatItem::System("Você recusou o uso da ferramenta.".into())]);
    let stopped = json!({"type": "user", "message": {"content": [{"type": "text", "text": "[Request interrupted by user for tool use]"}]}});
    assert_eq!(line(stopped), vec![ChatItem::System("Interrompido pela pessoa.".into())]);
    // what was pasted, without its marks; what was typed while it worked; tags with words after them
    let pasted = json!({"type": "user", "message": {"content": "veja <pasted_content id=\"1\">o log\n</pasted_content id=\"1\"> isso"}});
    assert_eq!(line(pasted), vec![ChatItem::User("veja o log\n isso".into())]);
    let queued = json!({"type": "attachment", "attachment": {"type": "queued_command", "commandMode": "prompt", "prompt": "e os testes?"}});
    assert_eq!(line(queued), vec![ChatItem::User("e os testes?".into())]);
    assert_eq!(line(json!({"type": "user", "message": {"content": "<b>oi</b> e mais"}})), vec![ChatItem::User("<b>oi</b> e mais".into())]);

    let dir = std::env::temp_dir().join(format!("mago-chat-{}", std::process::id()));
    let sessions = dir.join("sessions");
    std::fs::create_dir_all(&sessions).unwrap();
    let write = |name: &str, v: Value| std::fs::write(sessions.join(name), v.to_string()).unwrap();
    write("1.json", json!({"sessionId": "velha", "cwd": "/p", "startedAt": 1000, "status": "idle"}));
    let pid = std::process::id();
    write("2.json", json!({"sessionId": "nova", "cwd": "/p", "startedAt": 9000, "status": "busy", "entrypoint": "cli", "pid": pid, "statusUpdatedAt": 9100}));
    // a claude that crashed left its file saying busy: its process is gone
    let mut gone = if cfg!(windows) { crate::ide::command("cmd").args(["/c", "exit"]).spawn() } else { std::process::Command::new("true").spawn() }.unwrap();
    gone.wait().unwrap();
    write("6.json", json!({"sessionId": "caiu", "cwd": "/p", "startedAt": 9900, "status": "busy", "pid": gone.id()}));
    write("5.json", json!({"sessionId": "espera", "cwd": "/r", "startedAt": 9000, "status": "waiting", "waitingFor": "input needed"}));
    write("3.json", json!({"sessionId": "outra", "cwd": "/q", "startedAt": 9500}));
    // the desktop app's session in the same folder is not in any terminal
    write("4.json", json!({"sessionId": "mesa", "cwd": "/p", "startedAt": 9999, "entrypoint": "claude-desktop"}));
    let found = crate::transcript::sessions(&sessions);
    assert_eq!(found.len(), 4);
    let session = |id: &str, cwd: &str, started| Session { id: id.into(), cwd: cwd.into(), started, ..Default::default() };
    // what it says it is doing, and since when
    let nova = found.iter().find(|s| s.id == "nova").unwrap();
    assert_eq!((nova.pid, nova.status.as_str(), nova.status_at, nova.waiting_for.as_deref()), (pid, "busy", 9100, None));
    assert_eq!(found.iter().find(|s| s.id == "espera").unwrap().waiting_for.as_deref(), Some("input needed"));
    // two Claude terminals in one folder: the newer takes the newer session, the older the other
    let terminals = [("velho".to_string(), "/p".to_string(), 500, 0), ("novo".to_string(), "/p".to_string(), 8000, 0)];
    let mut pairs = assign(&terminals, &found, std::collections::HashMap::new);
    pairs.sort_by(|a, b| a.0.cmp(&b.0));
    let ids: Vec<(&str, &str)> = pairs.iter().map(|(t, s)| (t.as_str(), s.id.as_str())).collect();
    assert_eq!(ids, [("novo", "nova"), ("velho", "velha")]);
    // nothing started in that folder since the terminal opened: no conversation
    assert!(assign(&[("t".to_string(), "/p".to_string(), 20000, 0)], &found, std::collections::HashMap::new).is_empty());
    // the terminal's folder as typed, the session's as Claude Code resolves it (/tmp → /private/tmp)
    let typed = std::env::temp_dir().to_str().unwrap().to_string();
    let resolved = std::env::temp_dir().canonicalize().unwrap().to_str().unwrap().to_string();
    let real = assign(&[("t".to_string(), typed, 0, 0)], &[session("real", &resolved, 1)], std::collections::HashMap::new);
    assert_eq!(real.len(), 1);
    // resuming a conversation took Claude Code to its folder: the session whose process descends
    // from the terminal's shell (claude 30 ← shell 20 ← the app 10) is the terminal's
    let moved = Session { pid: 30, ..session("retomada", "/outra", 5) };
    let other = Session { pid: 31, ..session("alheia", "/outra", 6) };
    let tree = || std::collections::HashMap::from([(30, 20), (20, 10), (31, 99)]);
    let pairs = assign(&[("t".to_string(), "/macropad".to_string(), 0, 20)], &[moved, other], tree);
    assert_eq!(pairs.iter().map(|(t, s)| (t.as_str(), s.id.as_str())).collect::<Vec<_>>(), [("t", "retomada")]);
    assert!(crate::transcript::parents().contains_key(&std::process::id()), "this test's own process is listed");

    // read from where it stopped; an unfinished line waits for the rest, even cut inside a character
    let file = dir.join("t.jsonl");
    let head =
        b"{\"type\":\"ai-title\",\"aiTitle\":\"Plano\"}\n{\"type\":\"user\",\"message\":{\"content\":\"um\"}}\n{\"type\":\"user\",\"message\":{\"content\":\"a";
    std::fs::write(&file, [head.as_slice(), &"ç".as_bytes()[..1]].concat()).unwrap();
    let mut transcript = Transcript::new(file.clone());
    transcript.poll();
    assert_eq!((transcript.title.as_deref(), transcript.items.len()), (Some("Plano"), 1));
    let mut more = std::fs::OpenOptions::new().append(true).open(&file).unwrap();
    std::io::Write::write_all(&mut more, &[&"ç".as_bytes()[1..], "ão\"}}\n".as_bytes()].concat()).unwrap();
    transcript.poll();
    assert_eq!(transcript.items.last(), Some(&ChatItem::User("ação".into())));
    // the person's title stays over the summaries'; a retry's error replaces the one before
    let lines: String = [json!({"type": "custom-title", "customTitle": "Meu"}), json!({"type": "ai-title", "aiTitle": "Outro"}), api(1), api(2)]
        .iter()
        .map(|v| format!("{v}\n"))
        .collect();
    std::io::Write::write_all(&mut more, lines.as_bytes()).unwrap();
    transcript.poll();
    assert_eq!(transcript.title.as_deref(), Some("Meu"));
    assert_eq!(
        transcript.items[transcript.items.len() - 2..],
        [ChatItem::User("ação".into()), ChatItem::Error("Erro da API (tentativa 2 de 10): 529 Overloaded".into())]
    );
    // and the last word after the retries replaces them too, as the answer an ask reads
    std::io::Write::write_all(&mut more, format!("{last}\n").as_bytes()).unwrap();
    transcript.poll();
    assert_eq!(transcript.items.last(), Some(&ChatItem::Error("Erro da API: 529 Overloaded".into())));
    assert_eq!(transcript.items.len(), 3);
    assert_eq!(transcript.agent_words().as_deref(), Some("Erro da API: 529 Overloaded"));
    // the tools called stay running until their answers come
    let call = |id: &str, file: &str| json!({"type": "assistant", "message": {"content": [{"type": "tool_use", "id": id, "name": "Read", "input": {"file_path": file}}]}});
    let answer = |id: &str| json!({"type": "user", "message": {"content": [{"type": "tool_result", "tool_use_id": id, "content": "ok"}]}});
    std::io::Write::write_all(
        &mut more,
        format!(
            "{}
{}
",
            call("a", "a.rs"),
            call("b", "b.rs")
        )
        .as_bytes(),
    )
    .unwrap();
    transcript.poll();
    assert_eq!(transcript.running, [("a".to_string(), "Read a.rs".to_string()), ("b".into(), "Read b.rs".into())]);
    std::io::Write::write_all(
        &mut more,
        format!(
            "{}
",
            answer("b")
        )
        .as_bytes(),
    )
    .unwrap();
    transcript.poll();
    assert_eq!(transcript.running, [("a".to_string(), "Read a.rs".to_string())]);
    // a step takes what its answer says: lines read, lines changed, a new file; a command's output so far
    let at = |s: u32| format!("2026-10-28T20:05:{s:02}.500Z");
    let call_at = |id: &str, name: &str, s| json!({"type": "assistant", "timestamp": at(s), "message": {"content": [{"type": "tool_use", "id": id, "name": name, "input": {"file_path": "x"}}]}});
    let answer_at = |id: &str, s, result: Value| json!({"type": "user", "timestamp": at(s), "toolUseResult": result, "message": {"content": [{"type": "tool_result", "tool_use_id": id, "content": "ok"}]}});
    let lines = [
        call_at("r", "Read", 1),
        answer_at("r", 2, json!({"type": "text", "file": {"numLines": 212}})),
        call_at("e", "Edit", 3),
        answer_at("e", 4, json!({"structuredPatch": [{"lines": [" a", "+b", "+c", "-d"]}, {"lines": ["+e"]}]})),
        call_at("w", "Write", 5),
        answer_at("w", 9, json!({"type": "create", "structuredPatch": []})),
        call_at("x", "Bash", 10),
        json!({"type": "progress", "parentToolUseID": "x", "data": {"type": "bash_progress", "output": "PHPUnit\n....   14 / 23 ( 60%)\n\n"}}),
    ];
    std::io::Write::write_all(&mut more, lines.iter().map(|v| format!("{v}\n")).collect::<String>().as_bytes()).unwrap();
    transcript.poll();
    let steps: Vec<&crate::transcript::Step> = transcript.items.iter().filter_map(|i| if let ChatItem::Tool(s) = i { Some(s) } else { None }).collect();
    let [.., read, edit, write, bash] = steps.as_slice() else { panic!("{steps:?}") };
    assert_eq!((read.note.as_deref(), edit.diff, write.note.as_deref()), (Some("212 linhas"), Some((3, 1)), Some("novo")));
    assert_eq!((write.started, write.ended), (crate::transcript::unix_seconds(&at(5)), crate::transcript::unix_seconds(&at(9))));
    assert_eq!((bash.output.as_deref(), bash.ended), (Some("....   14 / 23 ( 60%)"), None));
    assert_eq!(crate::transcript::unix_seconds("1970-01-02T00:00:01.5Z"), Some(86_401.5));
    assert_eq!(crate::transcript::unix_seconds("2026-10-28T20:05:00Z"), Some(1_793_217_900.0));
    transcript.running.retain(|(id, _)| id == "a");
    // a side conversation's calls aren't this agent's; the person's next message ends what was left
    let side = json!({"type": "assistant", "isSidechain": true, "message": {"content": [{"type": "tool_use", "id": "s", "name": "Bash", "input": {"command": "ls"}}]}});
    std::io::Write::write_all(
        &mut more,
        format!(
            "{side}
"
        )
        .as_bytes(),
    )
    .unwrap();
    transcript.poll();
    assert_eq!(transcript.running.len(), 1);
    std::io::Write::write_all(
        &mut more,
        format!(
            "{}
",
            json!({"type": "user", "message": {"content": "de novo"}})
        )
        .as_bytes(),
    )
    .unwrap();
    transcript.poll();
    assert!(transcript.running.is_empty());
    // the newest 20 images keep their pixels, the ones before a note
    let print = json!({"type": "user", "message": {"content": [{"type": "image", "source": {"media_type": "image/png", "data": "iVBO"}}]}});
    std::io::Write::write_all(
        &mut more,
        format!(
            "{print}
"
        )
        .repeat(21)
        .as_bytes(),
    )
    .unwrap();
    transcript.poll();
    let images = transcript.items.iter().filter(|i| matches!(i, ChatItem::Image { .. })).count();
    assert_eq!(images, 20);
    assert!(matches!(&transcript.items[transcript.items.len() - 21], ChatItem::System(_)));
    // a 3 MB print at the end hides the messages before it from the first 2 MB read: it reads further back
    let big = dir.join("big.jsonl");
    let said: String = (0..60).map(|n| format!("{}\n", json!({"type": "user", "message": {"content": format!("m{n}")}}))).collect();
    let huge = json!({"type": "user", "message": {"content": [{"type": "image", "source": {"media_type": "image/png", "data": "A".repeat(3 << 20)}}]}});
    std::fs::write(&big, format!("{said}{huge}\n")).unwrap();
    let mut opened = Transcript::new(big);
    opened.poll();
    assert_eq!(opened.items.len(), 61, "the 60 messages and the print");
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn plain_claude_resumes_the_conversation_its_terminal_remembers_while_the_file_is_there() {
    let dir = std::env::temp_dir().join(format!("mago-retomar-{}", uuid::Uuid::new_v4().simple()));
    let spec = TerminalSpec { command: "claude".into(), claude_session: "0b6c-41".into(), claude_folder: "-p".into(), ..Default::default() };
    assert_eq!(spec.launch_command(Some(&dir)), "claude", "its file is gone: a new conversation");
    let file = crate::transcript::transcript_path(&dir, "-p", "0b6c-41");
    std::fs::create_dir_all(file.parent().unwrap()).unwrap();
    std::fs::write(&file, "{}\n").unwrap();
    assert_eq!(spec.launch_command(Some(&dir)), "claude --resume 0b6c-41");
    assert_eq!(TerminalSpec { resume: false, ..spec.clone() }.launch_command(Some(&dir)), "claude", "Retomar off");
    assert_eq!(TerminalSpec { command: "claude --model opus".into(), ..spec.clone() }.launch_command(Some(&dir)), "claude --model opus", "only plain claude");
    assert_eq!(spec.launch_command(None), "claude");
    // typed into a shell: an id that isn't one stays out
    let odd = "x;rm";
    std::fs::write(crate::transcript::transcript_path(&dir, "-p", odd), "{}\n").unwrap();
    assert_eq!(TerminalSpec { claude_session: odd.into(), ..spec }.launch_command(Some(&dir)), "claude");
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn a_bash_step_keeps_its_whole_command_its_folder_and_the_end_of_its_output() {
    use crate::transcript::{ChatItem, LOG_MAX, Transcript};
    let dir = std::env::temp_dir().join(format!("mago-card-{}", uuid::Uuid::new_v4().simple()));
    std::fs::create_dir_all(&dir).unwrap();
    let file = dir.join("s.jsonl");
    let call = |id: &str, command: &str| {
        json!({"type": "assistant", "cwd": "/p/web", "timestamp": "2026-10-28T20:05:01.000Z", "message": {"content": [
            {"type": "tool_use", "id": id, "name": "Bash", "input": {"command": command, "description": "testes"}}]}})
    };
    let answer = |id: &str, result: Value, error: bool, said: &str| {
        json!({"type": "user", "timestamp": "2026-10-28T20:05:04.000Z", "toolUseResult": result, "message": {"content": [
            {"type": "tool_result", "tool_use_id": id, "is_error": error, "content": said}]}})
    };
    let huge = format!("primeira\n{}\núltima", "y".repeat(3 * LOG_MAX));
    let read = json!({"type": "assistant", "message": {"content": [{"type": "tool_use", "id": "r", "name": "Read", "input": {"file_path": "/p/a.rs"}}]}});
    let lines = [
        call("a", "cd web\nnpm test"),
        answer("a", json!({"stdout": "ok 3 testes\n", "stderr": "aviso: lento", "interrupted": false}), false, "ok"),
        call("b", "cargo build"),
        answer("b", json!({"stdout": huge, "stderr": ""}), false, "ok"),
        call("c", "false"),
        answer("c", json!("Error: Exit code 1"), true, "Exit code 1\nfalhou feio"),
        read,
    ];
    std::fs::write(&file, lines.iter().map(|v| format!("{v}\n")).collect::<String>()).unwrap();
    let mut t = Transcript::new(file);
    t.poll();
    let steps: Vec<_> = t.items.iter().filter_map(|i| if let ChatItem::Tool(s) = i { Some(s) } else { None }).collect();
    let [a, b, c, r] = steps.as_slice() else { panic!("{:?}", t.items) };
    assert_eq!((a.what.as_str(), a.input.as_str(), a.cwd.as_deref()), ("cd web", "cd web\nnpm test", Some("/p/web")));
    assert_eq!((a.log.as_str(), a.error.as_deref(), a.ended.map(|e| e - a.started.unwrap())), ("ok 3 testes\naviso: lento", None, Some(3.0)));
    assert!(b.log.starts_with("… (cortado") && b.log.ends_with("última") && !b.log.contains("primeira"), "only its end");
    assert!(b.log.len() < LOG_MAX + 64, "{}", b.log.len());
    assert_eq!((c.failed, c.error.as_deref()), (true, Some("Exit code 1\nfalhou feio")));
    assert_eq!(r.files, ["/p/a.rs"]);
    let _ = std::fs::remove_dir_all(dir);
}
