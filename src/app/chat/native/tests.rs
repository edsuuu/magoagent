use super::*;
use crate::app::chat::direct::{CHAT_MODES, Chat};
use crate::transcript::{Step, Transcript};

fn app_with_chat() -> DrawingApp {
    let mut app = DrawingApp::default();

    app.chats.push(Chat {
        id: "c1".into(),
        agent: "claude".into(),
        name: "Claude Code · loja".into(),
        cwd: "/tmp/loja".into(),
        session: None,
        mode: 0,
        model: String::new(),
        effort: String::new(),
        recruit: false,
        named: false,
        draft: false,
    });

    let mut transcript = Transcript::new("/nada.jsonl".into());
    let step = |id: &str, name: &str| ChatItem::Tool(Step { id: id.into(), name: name.into(), what: "x".into(), ..Default::default() });

    transcript.items =
        vec![ChatItem::User("oi".into()), step("a", "Read"), step("b", "Edit"), ChatItem::Agent("Feito:\n\n```rust\nfn a() {}\n```".into()), step("c", "Bash")];

    transcript.running = vec![("c".into(), "Bash x".into())];
    app.transcripts.insert("c1".into(), transcript);
    app.chat_agent = Some("c1".into());
    app
}

#[test]
fn the_snapshot_has_the_messages_the_tool_runs_folded_and_the_chips() {
    let mut app = app_with_chat();
    let snapshot = app.native_chat_snapshot("c1").expect("the chat");
    let kinds: Vec<&str> = snapshot.items.iter().map(|item| item.kind).collect();

    assert_eq!(kinds, ["user", "tools", "agent", "tools"]);
    assert_eq!(snapshot.items.iter().map(|item| item.id.as_str()).collect::<Vec<_>>(), ["0", "1", "3", "4"]);
    // two tools with words after them fold; the last one runs open
    assert!(snapshot.items[1].folded && snapshot.items[1].summary.starts_with("2 ferramentas"));
    assert!(!snapshot.items[3].folded && snapshot.items[3].steps[0].live == "running");
    let parts: Vec<(&str, &str)> = snapshot.items[2].parts.iter().map(|part| (part.kind, part.text.as_str())).collect();

    assert_eq!(parts, [("text", ""), ("code", "fn a() {}")]);
    assert_eq!(snapshot.items[2].parts[0].paras[0].lines[0].runs[0].text, "Feito:");
    assert_eq!(snapshot.items[2].parts[1].info, "rust");
    assert!(serde_json::to_value(&snapshot).is_ok_and(|json| json["items"][0]["text"] == "oi"));
    let chips = snapshot.chips.expect("a chat has chips");

    assert_eq!((chips.folder.as_str(), chips.folder_locked, chips.modes.len()), ("loja", false, CHAT_MODES.len()));
    assert!(snapshot.fresh && snapshot.is_chat && snapshot.state == "ready");
}

#[test]
fn the_verbs_change_the_draft_queue_and_chips_as_the_egui_chat_does() {
    let mut app = app_with_chat();

    app.native_chat_act("draft", &json!({ "text": "/st" })).expect("draft");
    assert_eq!(app.chat_draft, "/st");
    let snapshot = app.native_chat_snapshot("c1").expect("the chat");

    assert!(snapshot.slash.iter().any(|entry| entry.name == "status"));
    // Editar: the queue goes back to the box, and SwiftUI sees a new draft
    app.queue_message("c1", "depois".into());
    app.native_chat_act("queue", &json!({ "chat": "c1", "action": "edit" })).expect("edit");
    let seq = NATIVE.with_borrow(|native| native.draft_seq);

    app.native_chat();
    assert!(NATIVE.with_borrow(|native| native.draft_seq) > seq && app.chat_draft.starts_with("depois"));
    app.native_chat_act("footer", &json!({ "chat": "c1", "mode": 1 })).expect("mode");
    app.native_chat_act("footer", &json!({ "chat": "c1", "effort": "medium" })).expect("effort");
    // stored "" when it is the agent's own default (this machine's settings): the chip says what runs
    let chips = app.native_chat_snapshot("c1").and_then(|snapshot| snapshot.chips).expect("chips");

    assert_eq!((app.chats[0].mode, chips.effort.as_str()), (1, "medium"));
    app.native_chat_act("send", &json!({ "chat": "c1", "text": "vai\n" })).expect("send");
    assert!(app.chat_draft.is_empty() && NATIVE.with_borrow(|native| native.sends == [("c1".to_string(), "vai".to_string())]));
    assert!(app.native_chat_act("nada", &json!({})).is_err());
    let list = app.native_chats_snapshot();

    assert_eq!(list.current.as_deref(), Some("c1"));
    assert!(list.groups.iter().flat_map(|group| &group.entries).any(|entry| entry.id == "c1" && entry.chat));
}

#[test]
fn the_words_go_as_markdown_lines_and_runs_with_the_files_they_name() {
    let base = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));

    let text =
        "## Resumo\n\nVeja o **Cargo.toml** e `src/main.rs`, *já*.\n- um\n  - dentro\n2. dois\n> citação\n\n| Arquivo | Linhas |\n|---|---:|\n| `x` | 3 |";

    let parts = super::words::parts(text, Some(base), 0.0);
    let paras = &parts[0].paras;
    let kinds: Vec<(&str, usize, &str)> = paras[1].lines.iter().map(|line| (line.kind, line.level, line.mark.as_str())).collect();

    assert_eq!((paras[0].lines[0].kind, paras[0].lines[0].level), ("heading", 2));
    assert_eq!(kinds, [("text", 0, ""), ("item", 0, "•"), ("item", 2, "•"), ("item", 0, "2."), ("quote", 0, "")]);
    let runs = &paras[1].lines[0].runs;
    let cargo = runs.iter().find(|run| run.text == "Cargo.toml").expect("the file named");

    assert!(cargo.bold && cargo.path.as_deref().is_some_and(|path| path.ends_with("Cargo.toml")));
    let main = runs.iter().find(|run| run.code).expect("the code");

    assert!(main.text == "src/main.rs" && main.path.is_some());
    assert!(runs.iter().any(|run| run.italic && run.text == "já"));
    // a table: its rows of cells, the header in bold
    let rows = &paras[2].rows;

    assert_eq!(rows.len(), 2);
    assert!(rows[0][0][0].bold && rows[1][0][0].code && rows[1][1][0].text == "3");
}

#[test]
fn the_amber_card_and_the_search_answer_as_the_egui_chat() {
    let mut app = app_with_chat();
    let (reply, answer) = std::sync::mpsc::channel();

    let request = json!({ "tool_name": "AskUserQuestion", "input": { "questions": [
        { "question": "Qual cor?", "header": "Cor", "multiSelect": false, "options": [{ "label": "Azul" }, { "label": "Verde" }] },
        { "question": "Quais?", "header": "Frutas", "multiSelect": true, "options": [{ "label": "Maçã" }, { "label": "Uva" }] },
    ] } });

    let mut ask = crate::app::chat::decide::PersonAsk::new("c1".into(), &request, reply);

    ask.shown = Some(-100.0);
    app.person_asks.get_mut().push(ask);
    app.native_chat_act("ask", &json!({ "chat": "c1", "action": "pick", "index": 1 })).expect("pick");
    let ask = app.native_chat_snapshot("c1").and_then(|snapshot| snapshot.ask).expect("the card");

    // one answer: on to the next question
    assert_eq!((ask.step, ask.picked[0].as_slice(), ask.complete), (1, &[1][..], false));
    assert!(app.native_chat_act("ask", &json!({ "chat": "c1", "action": "allow" })).is_err());
    app.native_chat_act("ask", &json!({ "chat": "c1", "action": "other", "text": "Pera" })).expect("other");
    app.native_chat_act("ask", &json!({ "chat": "c1", "action": "allow" })).expect("send");
    assert!(app.person_asks.get_mut().is_empty() && answer.try_recv().is_ok());
    // Buscar na conversa: only what says it
    app.native_chat_act("search", &json!({ "chat": "c1", "query": "OI" })).expect("search");
    let snapshot = app.native_chat_snapshot("c1").expect("the chat");

    assert_eq!((snapshot.items.len(), snapshot.items[0].text.as_str(), snapshot.query.as_str()), (1, "oi", "OI"));
    app.native_chat_act("search", &json!({ "chat": "c1", "query": "" })).expect("search");
    assert_eq!(app.native_chat_snapshot("c1").map(|snapshot| snapshot.items.len()), Some(4));
}

#[test]
fn the_list_follows_exibicao_and_the_header_names_the_agent() {
    let mut app = app_with_chat();
    let snapshot = app.native_chat_snapshot("c1").expect("the chat");

    // the header's name is the chat's before " · folder"; Claude Code's chat has the context ring
    assert_eq!(snapshot.title, "Claude Code");
    assert!(snapshot.ring);
    let list = app.native_chats_snapshot();

    assert!(!list.compact && list.by_space && list.order == "created" && list.agent_filter == "all");
    assert!(list.groups.iter().all(|group| !group.name.is_empty()));
    assert!(list.groups.iter().flat_map(|group| &group.entries).all(|entry| !entry.mago.is_empty()));
    app.prefs.chat_list.set("chat_list_density", "compact").unwrap();
    app.prefs.chat_list.set("chat_list_group", "off").unwrap();
    let list = app.native_chats_snapshot();

    // ungrouped: one list with no space's name over it
    assert!(list.compact && !list.by_space);
    assert!(list.groups.len() == 1 && list.groups[0].name.is_empty());
    app.prefs.chat_list.set("chat_list_agent", "codex").unwrap();
    let list = app.native_chats_snapshot();

    // only Codex's chats: the Claude Code one leaves the list and the counts
    assert_eq!((list.agent_filter, list.counts[0]), ("codex", 0));
    assert!(list.groups.iter().all(|group| group.entries.is_empty()));
}

#[test]
fn the_contexto_panel_goes_out_while_open_and_its_buttons_act_as_egui() {
    use crate::app::chat::context::{Server, Status, parse_context};

    let mut app = app_with_chat();

    // closed: null; the ring's open (the MCP's chat_context) brings it, still unread
    assert!(app.context_snapshot().is_none());
    app.act_chat_context("c1", "open").expect("open");
    let panel = app.context_snapshot().expect("open");

    assert_eq!((panel.chat.as_str(), panel.note.as_str()), ("c1", "Mande uma mensagem para ver o contexto."));
    assert!(panel.report.is_none() && panel.connectors.is_empty());
    // a /context read and seven connectors: the numbers, the one that failed first, past 5 folded
    let text = "**Tokens:** 301.8k / 400k (75%)\n\n### Estimated usage by category\n\n| Category | Tokens | Percentage |\n|---|---|---|\n| Messages | 221k | 55.2% |\n| Free space | 65.2k | 16.3% |\n| Autocompact buffer | 33k | 8.3% |\n\n### Memory Files\n\n| Type | Path | Tokens |\n|---|---|---|\n| Project | /p/CLAUDE.md | 21 |\n";
    let info = app.chat_ctx.info.entry("c1".into()).or_default();

    info.report = Some(parse_context(text).expect("the report"));
    info.listed = (0..7).map(|index| Server { name: format!("s{index}"), status: if index == 6 { Status::Failed } else { Status::Connected } }).collect();
    let panel = app.context_snapshot().expect("open");
    let report = panel.report.expect("read");

    assert_eq!((report.percent.as_str(), report.used.as_str(), report.until.as_str()), ("75%", "301,8k de 400k tokens", "65,2k"));
    assert_eq!(report.legend.iter().map(|row| row.name).collect::<Vec<_>>(), ["Mensagens", "Reserva da auto-compactação", "Livre"]);
    assert!(!report.legend[2].counts && report.bar.len() == 2 && report.memory == "Memória: CLAUDE.md");
    assert_eq!((panel.connectors[0].status, panel.connected.as_str(), panel.more.as_str()), ("failed", "6 de 7 conectados", "+ 2 conectados"));
    // "Compactar automaticamente em" reads again; Compactar and Tentar wait for the frame; × closes
    app.native_chat_act("context", &json!({ "chat": "c1", "action": "autocompact", "value": "200k" })).expect("pick");
    assert!(app.prefs.chat_autocompact == "200k" && app.chat_ctx.info["c1"].dirty);
    assert!(app.native_chat_act("context", &json!({ "chat": "c1", "action": "autocompact", "value": "x; rm" })).is_err());
    app.native_chat_act("context", &json!({ "chat": "c1", "action": "retry" })).expect("retry");
    app.native_chat_act("context", &json!({ "chat": "c1", "action": "compact" })).expect("compact");
    assert!(app.chat_ctx.info["c1"].retried && NATIVE.with_borrow(|native| native.context == [("c1".to_string(), "retry"), ("c1".to_string(), "compact")]));
    app.act_chat_context("c1", "close").expect("close");
    assert!(app.context_snapshot().is_none());
}
