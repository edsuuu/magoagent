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
fn a_text_is_measured_once_and_again_when_it_changes() {
    // a 200-line note's text: measured glyph by glyph the first time, then known by its fingerprint
    let text: String = (1..=200).map(|line| format!("Linha {line}: um texto longo para medir uma vez só.\n")).collect();
    let first = text_size(&text, 20.0);
    assert_eq!(text_size(&text, 20.0), first);
    assert_eq!(first.y, 201.0 * 20.0 * LINE_HEIGHT);
    // the same length with other letters, and another size: measured again
    let narrow = text.replacen("Linha 1:", "iiiii 1:", 1).replace('m', "i");
    assert_eq!(narrow.len(), text.len());
    assert!(text_size(&narrow, 20.0).x < first.x);
    assert_eq!(text_size(&text, 10.0).y, first.y / 2.0);
    assert_ne!(fingerprint(b"abcdefgh1"), fingerprint(b"abcdefgh2"));
    assert_ne!(fingerprint(b""), fingerprint(b"\0"), "the length counts");
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
    let plain = crate::editor::theme::without_ligatures(font.clone());
    assert!(!tags(&plain).iter().any(|t| ["calt", "liga", "clig", "dlig"].contains(&t.as_str())), "{:?}", tags(&plain));
    assert_eq!(plain.len(), font.len(), "only tags change");
    assert_eq!(crate::editor::theme::without_ligatures(b"nada".to_vec()), b"nada", "not a font: as it came");
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

#[test]
fn the_build_of_the_repo_says_dev_in_its_title_and_icon() {
    // run from `target/` the tests are the app in development (a CARGO_TARGET_DIR elsewhere is not)
    let development = crate::file::development();
    assert_eq!(crate::window_names().0 == format!("{APP_TITLE} DEV"), development);
    let dev_icon: &[u8] = include_bytes!("../../assets/icon-dev.png");
    assert_eq!(crate::app_icon() == dev_icon, development);
    let dev = image::load_from_memory(dev_icon).expect("the DEV icon is a png");
    let installed = image::load_from_memory(include_bytes!("../../assets/icon.png")).expect("the icon is a png");
    assert_eq!((dev.width(), dev.height()), (installed.width(), installed.height()), "same size as the app's icon");
    assert_ne!(dev.to_rgba8().into_raw(), installed.to_rgba8().into_raw(), "with its badge");
}
