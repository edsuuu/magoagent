#[test]
fn local_loopback_base_only() {
    for good in ["http://127.0.0.1:3000", "http://localhost:8080", "http://127.0.0.1:47823"] {
        assert!(local_base(good).is_some(), "{good}");
        assert_eq!(local_base(good), Some(good.to_string()));
    }
    assert_eq!(local_base("http://127.0.0.1:3000/"), Some("http://127.0.0.1:3000".into()));
    for bad in [
        "https://127.0.0.1:3000",
        "http://192.168.1.10:3000",
        "http://example.com:3000",
        "http://127.0.0.1",
        "http://127.0.0.1:80",
        "http://127.0.0.1:abc",
        "127.0.0.1:3000",
        "",
    ] {
        assert!(local_base(bad).is_none(), "{bad}");
    }
}

#[test]
fn the_credential_is_saved_to_the_keychain_and_removed() {
    mock_keyring();
    let mut app = DrawingApp::default();
    assert_eq!(app.whatsapp_save_credential("user:pass"), Ok("Credencial guardada no cofre do sistema.".into()));
    app.integrations.whatsapp.load_credential();
    assert_eq!(app.integrations.whatsapp.credential.as_deref(), Some("user:pass"));
    assert!(app.whatsapp_save_credential("bad\nvalue").is_err());
    assert!(app.whatsapp_save_credential("nocolon").is_err());
    app.whatsapp_remove_credential().unwrap();
    let mut clean = DrawingApp::default();
    clean.integrations.whatsapp.load_credential();
    assert_eq!(clean.integrations.whatsapp.credential.as_deref(), Some(""));
}

#[test]
fn base64_encoding() {
    assert_eq!(base64(b"a"), "YQ==");
    assert_eq!(base64(b"ab"), "YWI=");
    assert_eq!(base64(b"abc"), "YWJj");
    assert_eq!(base64(b"user:pass"), "dXNlcjpwYXNz");
}

#[test]
fn gowa_devices_list_is_parsed() {
    let empty = r#"{"results":[]}"#;
    assert_eq!(gowa_status(empty), Link::NeedsPairing);
    let needs_qr = r#"{"results":[{"id":"coucou","state":"needs_qr"}]}"#;
    assert_eq!(gowa_status(needs_qr), Link::NeedsPairing);
    let logged_in = r#"{"results":[{"id":"coucou","state":"logged_in","jid":"5511999999999@s.whatsapp.net"}]}"#;
    assert_eq!(gowa_status(logged_in), Link::Connected);
    assert_eq!(gowa_status("garbage"), Link::Unavailable);
    assert_eq!(gowa_status(r#"{"error":"not found"}"#), Link::Unavailable);
}

#[test]
fn gateway_check_refuses_non_loopback() {
    let link = check_gateway("http://192.168.1.10:3000", "");
    assert_eq!(link, Link::Unavailable);
}

#[test]
fn whatsapp_test_does_nothing_in_tests() {
    let mut app = DrawingApp::default();
    app.prefs.integrations.whatsapp_base = "http://127.0.0.1:3000".into();
    assert!(!app.whatsapp_test());
    assert!(!app.integrations.whatsapp.checking);
}

#[test]
fn whatsapp_report_says_the_state() {
    let mut app = DrawingApp::default();
    assert_eq!(app.whatsapp_report(), "WhatsApp: desligado.");
    app.prefs.integrations.whatsapp = true;
    assert_eq!(app.whatsapp_report(), "WhatsApp: ainda não testado.");
    app.integrations.whatsapp.link = Link::Connected;
    assert_eq!(app.whatsapp_report(), "WhatsApp: gateway conectado.");
    app.integrations.whatsapp.link = Link::NeedsPairing;
    assert_eq!(app.whatsapp_report(), "WhatsApp: gateway disponível; WhatsApp precisa ser pareado nele.");
    app.integrations.whatsapp.link = Link::Unavailable;
    assert_eq!(app.whatsapp_report(), "WhatsApp: gateway indisponível ou resposta inválida.");
}
