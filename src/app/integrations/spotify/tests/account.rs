//! The account: PKCE, Spotify's authorize page, the callback, the tokens and when to renew them.

use crate::app::integrations::spotify::account::{
    Callback, REDIRECT_URI, SCOPES, authorize_url, base64url, callback, challenge, client_id_looks_right, decode, encode, needs_refresh, parse_tokens, sha256,
    token_failure, verifier,
};
use crate::prelude::*;

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}

#[test]
fn sha256_gives_the_standards_digests() {
    assert_eq!(hex(&sha256(b"")), "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855");
    assert_eq!(hex(&sha256(b"abc")), "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad");
    let two_blocks = b"abcdbcdecdefdefgefghfghighijhijkijkljklmklmnlmnomnopnopq";
    assert_eq!(hex(&sha256(two_blocks)), "248d6a61d20638b8e5c026930c3e6039a33ce45964ff2167f6ecedd419db06c1");
    // a message whose length lands where the padding needs a block of its own
    assert_eq!(hex(&sha256(&[b'a'; 64])), "ffe054fe7ae0cb6dc65c3af9b61d5209f439851db43d0ba5997337df154668eb");
}

#[test]
fn pkce_makes_the_challenge_of_rfc_7636() {
    assert_eq!(challenge("dBjftJeZ4CVP-mB92K27uhbUJU1p1r_wW1gFWFOEjXk"), "E9Melhoa2OwvFrEMTJguCHaoeK1t8URWbuGJSstw-cM", "RFC 7636, appendix B");
    assert_eq!(base64url(&[0xfb, 0xff, 0xfe]), "-__-", "no + / or =");
    let (first, second) = (verifier(), verifier());
    assert_eq!(first.len(), 86, "between 43 and 128 characters");
    assert!(first.chars().all(|letter| letter.is_ascii_alphanumeric() || letter == '-' || letter == '_'), "{first}");
    assert_ne!(first, second, "a new one each login");
}

#[test]
fn the_authorize_page_asks_for_the_scopes_with_the_challenge() {
    let url = authorize_url("0123456789abcdef0123456789abcdef", "E9Melhoa2OwvFrEMTJguCHaoeK1t8URWbuGJSstw-cM", "estado");
    let (page, query) = url.split_once('?').unwrap();
    assert_eq!(page, "https://accounts.spotify.com/authorize");
    let pairs: Vec<(String, String)> =
        query.split('&').map(|pair| pair.split_once('=').unwrap()).map(|(key, value)| (key.to_string(), decode(value))).collect();
    let value = |key: &str| pairs.iter().find(|(name, _)| name == key).map(|(_, value)| value.as_str()).unwrap_or_default();
    assert_eq!(value("response_type"), "code");
    assert_eq!(value("client_id"), "0123456789abcdef0123456789abcdef");
    assert_eq!(value("redirect_uri"), "http://127.0.0.1:8990/callback");
    assert_eq!((value("state"), value("code_challenge_method")), ("estado", "S256"));
    assert_eq!(value("code_challenge"), "E9Melhoa2OwvFrEMTJguCHaoeK1t8URWbuGJSstw-cM");
    assert_eq!(value("scope").split(' ').collect::<Vec<_>>(), SCOPES);
    for scope in ["user-read-playback-state", "user-modify-playback-state", "user-library-modify", "playlist-read-private"] {
        assert!(SCOPES.contains(&scope), "{scope}");
    }
    assert!(query.contains("redirect_uri=http%3A%2F%2F127.0.0.1%3A8990%2Fcallback"), "encoded");
    assert!(REDIRECT_URI.starts_with("http://127.0.0.1:"), "a loopback address: Spotify takes it without https");
}

#[test]
fn the_callback_brings_the_code_only_for_this_login() {
    let line = |target: &str| format!("GET {target} HTTP/1.1");
    assert_eq!(callback(&line("/callback?code=AQB-x_9&state=abc"), "abc"), Callback::Code("AQB-x_9".into()));
    assert_eq!(callback(&line("/callback?state=abc&code=AQB%2Dx"), "abc"), Callback::Code("AQB-x".into()), "any order, decoded");
    assert_eq!(callback(&line("/callback?code=AQB&state=outro"), "abc"), Callback::Elsewhere, "another login's state");
    assert_eq!(callback(&line("/callback?code=AQB"), "abc"), Callback::Elsewhere, "no state");
    assert_eq!(callback(&line("/favicon.ico"), "abc"), Callback::Elsewhere);
    assert_eq!(callback("POST /callback?code=AQB&state=abc HTTP/1.1", "abc"), Callback::Elsewhere);
    assert_eq!(callback("", "abc"), Callback::Elsewhere);
    assert_eq!(callback(&line("/callback?error=access_denied&state=abc"), "abc"), Callback::Refused("Você não autorizou o acesso à conta do Spotify.".into()));
    let Callback::Refused(why) = callback(&line("/callback?error=invalid_scope&state=abc"), "abc") else { panic!() };
    assert!(why.contains("invalid_scope"), "{why}");
    assert!(matches!(callback(&line("/callback?state=abc"), "abc"), Callback::Refused(_)), "no code");
}

#[test]
fn a_query_is_encoded_and_read_back() {
    assert_eq!(encode("a b/ç~"), "a%20b%2F%C3%A7~");
    assert_eq!(decode("a%20b%2F%C3%A7~+c"), "a b/ç~ c");
    assert_eq!(decode("100%"), "100%", "a lone % stays");
    assert_eq!(decode("%zz"), "%zz");
    assert!(client_id_looks_right("0123456789abcdef0123456789ABCDEF"));
    assert!(!client_id_looks_right("") && !client_id_looks_right("0123456789abcdef") && !client_id_looks_right(&"g".repeat(32)));
}

#[test]
fn the_tokens_are_read_and_renewed_a_minute_before_they_end() {
    let root = json!({ "access_token": "BQD", "token_type": "Bearer", "expires_in": 3600, "refresh_token": "AQC", "scope": "user-read-playback-state" });
    let tokens = parse_tokens(&root, 1_000).unwrap();
    assert_eq!((tokens.access.as_str(), tokens.expires_at, tokens.refresh.as_deref()), ("BQD", 3_601_000, Some("AQC")));
    let renewal = parse_tokens(&json!({ "access_token": "BQE", "expires_in": 3600 }), 0).unwrap();
    assert_eq!(renewal.refresh, None, "a renewal may keep the refresh token");
    assert_eq!(parse_tokens(&json!({ "error": "invalid_grant" }), 0), None);
    assert_eq!(parse_tokens(&json!({ "access_token": "com espaço" }), 0), None, "nothing odd goes into a header");
    assert!(needs_refresh("", i64::MAX, 0), "none yet");
    assert!(!needs_refresh("BQD", 3_601_000, 3_540_000));
    assert!(needs_refresh("BQD", 3_601_000, 3_541_000), "within a minute of the end");
    assert!(needs_refresh("BQD", 3_601_000, 4_000_000));
}

#[test]
fn a_refused_token_says_why_and_a_refused_renewal_unlinks() {
    let body = r#"{"error":"invalid_grant","error_description":"Refresh token revoked"}"#;
    let renewal = token_failure(400, body, true);
    assert!(renewal.unauthorized && renewal.message.contains("conectada de novo"), "{renewal:?}");
    let login = token_failure(400, body, false);
    assert!(!login.unauthorized && login.message.contains("Redirect URI"), "{login:?}");
    assert!(token_failure(400, r#"{"error":"invalid_client"}"#, false).message.contains("Client ID"));
    assert!(token_failure(429, "", false).message.contains("daqui a pouco"));
    assert!(token_failure(503, "", false).message.contains("503"));
}
