use super::*;

#[test]
fn portal_tabs() {
    // one page shows at a time, but the data keeps every tab (and the bar's hidden state)
    let mut portal = PortalSpec {
        name: "Docs".into(),
        url: "https://a.dev".into(),
        tabs: vec!["https://a.dev".into(), "https://b.dev".into()],
        active: 1,
        chrome_hidden: true,
        unloaded: true,
        user_agent: "MeuBot/1.0".into(),
        storage: crate::app::browser::extras::StorageScope::Global,
        muted: true,
    };
    assert_eq!((portal.tab_urls(), portal.active_tab()), (vec!["https://a.dev".to_string(), "https://b.dev".into()], 1));
    portal.set_tab_url(1, "https://b.dev/x".into());
    assert_eq!((portal.active_tab(), portal.url.as_str()), (1, "https://b.dev/x"));
    let mut e = DrawingApp::default().new_element(ElementKind::Rectangle { x: 0.0, y: 0.0, width: 9.0, height: 9.0 });
    e.portal = Some(portal.clone());
    assert_eq!(round_trip(&[e])[0].portal.as_ref(), Some(&portal));
}

#[test]
fn text_box_follows_its_glyphs() {
    // narrow letters take less room than wide ones: the box is the painted width, not a char count
    let width = |value: &str| {
        let e = DrawingApp::default().new_element(ElementKind::Text { x: 0.0, y: 0.0, value: value.into() });
        local_bounds(&e).width()
    };
    assert!(width("iiiiiiiiii") < width("mmmmmmmmmm") * 0.6, "{} vs {}", width("iiiiiiiiii"), width("mmmmmmmmmm"));
    assert_eq!(text_advance("", 20.0), 0.0);
}

#[test]
fn the_minimap_starts_closed_and_stays_as_left() {
    assert!(!Prefs::default().minimap);
    // preferences saved before the field read closed
    let old: Prefs = serde_json::from_str(r#"{"grid": true}"#).unwrap();
    assert!(!old.minimap);
    let open = Prefs { minimap: true, ..Default::default() };
    assert!(serde_json::from_str::<Prefs>(&serde_json::to_string(&open).unwrap()).unwrap().minimap);
}

#[test]
fn the_code_font_can_lose_its_ligatures() {
    let tags = |font: &[u8]| -> Vec<String> {
        let face = ttf_parser::Face::parse(font, 0).unwrap();
        face.tables().gsub.map(|g| g.features.into_iter().map(|f| f.tag.to_string()).collect()).unwrap_or_default()
    };
    let font = include_bytes!("../../assets/Assistant.ttf").to_vec();
    assert!(tags(&font).iter().any(|t| t == "liga"), "{:?}", tags(&font));
    let plain = crate::ide::theme::without_ligatures(font.clone());
    assert!(!tags(&plain).iter().any(|t| ["calt", "liga", "clig", "dlig"].contains(&t.as_str())), "{:?}", tags(&plain));
    assert_eq!(plain.len(), font.len(), "only tags change");
    assert_eq!(crate::ide::theme::without_ligatures(b"nada".to_vec()), b"nada", "not a font: as it came");
}

#[test]
fn a_message_shown_when_sent_is_not_shown_again_when_the_agent_writes_it_down() {
    use crate::transcript::{ChatItem, Transcript};
    let dir = std::env::temp_dir().join(format!("magoagent-echo-{}", uuid::Uuid::new_v4()));
    std::fs::create_dir_all(&dir).unwrap();
    let file = dir.join("s.jsonl");
    let lines =
        [json!({"type": "user", "message": {"content": "oi"}}), json!({"type": "assistant", "message": {"content": [{"type": "text", "text": "olá"}]}})];
    std::fs::write(&file, lines.iter().map(|v| format!("{v}\n")).collect::<String>()).unwrap();
    let mut t = Transcript::new(file);
    t.items.push(ChatItem::User("oi".into()));
    t.echoes = 1;
    t.poll();
    assert_eq!(t.items, [ChatItem::User("oi".into()), ChatItem::Agent("olá".into())]);
    assert_eq!(t.echoes, 0);
    let _ = std::fs::remove_dir_all(dir);
}
