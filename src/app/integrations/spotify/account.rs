//! The Spotify account, for what only the Web API knows (the app's queue, Liked Songs, the playlists, a search, the
//! heart): Authorization Code with PKCE, so no client secret. The person makes an app on Spotify's dashboard, pastes its
//! Client ID in Ajustes and registers `REDIRECT_URI` there; Conectar opens the browser on Spotify's page, a one-shot
//! listener on 127.0.0.1 takes the code back, and the app trades it for the tokens. The refresh token lives in the
//! system's keychain; the access token (an hour) only in memory, renewed a minute before it ends (`bearer`).

use super::web::{self, Failure, Profile};
use crate::app::integrations::http::{self, Request};
use crate::prelude::*;
use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{Receiver, channel};
use std::sync::{Arc, Mutex};

/// The refresh token's item in the keychain.
pub(crate) const TOKEN_ID: &str = "integration-spotify-refresh-token";
/// Where Spotify sends the browser back: a loopback IP literal (Spotify refuses `localhost`), plain http allowed there.
pub(crate) const REDIRECT_PORT: u16 = 8990;
pub(crate) const REDIRECT_URI: &str = "http://127.0.0.1:8990/callback";
/// Where the person makes the app and finds its Client ID.
pub(crate) const DASHBOARD: &str = "https://developer.spotify.com/dashboard";
pub(crate) const SCOPES: [&str; 7] = [
    "user-read-playback-state",
    "user-modify-playback-state",
    "user-read-currently-playing",
    "user-library-read",
    "user-library-modify",
    "playlist-read-private",
    "playlist-read-collaborative",
];
const AUTHORIZE: &str = "https://accounts.spotify.com/authorize";
const TOKEN_URL: &str = "https://accounts.spotify.com/api/token";
/// How long the browser has to come back, and how long before its end a token is renewed.
const LOGIN_FOR: std::time::Duration = std::time::Duration::from_secs(300);
const RENEW_BEFORE_MS: i64 = 60_000;
pub(crate) const RELINK: &str = "A conta do Spotify precisa ser conectada de novo em Ajustes › Integrações.";

/// SHA-256 (FIPS 180-4), for PKCE's challenge.
pub(crate) fn sha256(data: &[u8]) -> [u8; 32] {
    const ROUND: [u32; 64] = [
        0x428a2f98, 0x71374491, 0xb5c0fbcf, 0xe9b5dba5, 0x3956c25b, 0x59f111f1, 0x923f82a4, 0xab1c5ed5, 0xd807aa98, 0x12835b01, 0x243185be, 0x550c7dc3,
        0x72be5d74, 0x80deb1fe, 0x9bdc06a7, 0xc19bf174, 0xe49b69c1, 0xefbe4786, 0x0fc19dc6, 0x240ca1cc, 0x2de92c6f, 0x4a7484aa, 0x5cb0a9dc, 0x76f988da,
        0x983e5152, 0xa831c66d, 0xb00327c8, 0xbf597fc7, 0xc6e00bf3, 0xd5a79147, 0x06ca6351, 0x14292967, 0x27b70a85, 0x2e1b2138, 0x4d2c6dfc, 0x53380d13,
        0x650a7354, 0x766a0abb, 0x81c2c92e, 0x92722c85, 0xa2bfe8a1, 0xa81a664b, 0xc24b8b70, 0xc76c51a3, 0xd192e819, 0xd6990624, 0xf40e3585, 0x106aa070,
        0x19a4c116, 0x1e376c08, 0x2748774c, 0x34b0bcb5, 0x391c0cb3, 0x4ed8aa4a, 0x5b9cca4f, 0x682e6ff3, 0x748f82ee, 0x78a5636f, 0x84c87814, 0x8cc70208,
        0x90befffa, 0xa4506ceb, 0xbef9a3f7, 0xc67178f2,
    ];
    let mut hash: [u32; 8] = [0x6a09e667, 0xbb67ae85, 0x3c6ef372, 0xa54ff53a, 0x510e527f, 0x9b05688c, 0x1f83d9ab, 0x5be0cd19];
    let mut message = data.to_vec();
    message.push(0x80);
    while message.len() % 64 != 56 {
        message.push(0);
    }
    message.extend_from_slice(&(data.len() as u64).wrapping_mul(8).to_be_bytes());
    for block in message.chunks(64) {
        let mut words = [0u32; 64];
        for (index, word) in block.chunks(4).enumerate() {
            words[index] = u32::from_be_bytes([word[0], word[1], word[2], word[3]]);
        }
        for index in 16..64 {
            let (back15, back2) = (words[index - 15], words[index - 2]);
            let sigma0 = back15.rotate_right(7) ^ back15.rotate_right(18) ^ (back15 >> 3);
            let sigma1 = back2.rotate_right(17) ^ back2.rotate_right(19) ^ (back2 >> 10);
            words[index] = words[index - 16].wrapping_add(sigma0).wrapping_add(words[index - 7]).wrapping_add(sigma1);
        }
        let mut work = hash;
        for index in 0..64 {
            let [first, second, third, fourth, fifth, sixth, seventh, eighth] = work;
            let sum1 = fifth.rotate_right(6) ^ fifth.rotate_right(11) ^ fifth.rotate_right(25);
            let choice = (fifth & sixth) ^ (!fifth & seventh);
            let mixed = eighth.wrapping_add(sum1).wrapping_add(choice).wrapping_add(ROUND[index]).wrapping_add(words[index]);
            let sum0 = first.rotate_right(2) ^ first.rotate_right(13) ^ first.rotate_right(22);
            let majority = (first & second) ^ (first & third) ^ (second & third);
            work = [mixed.wrapping_add(sum0.wrapping_add(majority)), first, second, third, fourth.wrapping_add(mixed), fifth, sixth, seventh];
        }
        for (slot, value) in hash.iter_mut().zip(work) {
            *slot = slot.wrapping_add(value);
        }
    }
    let mut digest = [0u8; 32];
    for (index, word) in hash.iter().enumerate() {
        digest[index * 4..index * 4 + 4].copy_from_slice(&word.to_be_bytes());
    }
    digest
}

/// Base64 for addresses (RFC 4648 §5), without the padding, as PKCE wants it.
pub(crate) fn base64url(bytes: &[u8]) -> String {
    crate::render::base64(bytes).replace('+', "-").replace('/', "_").trim_end_matches('=').to_string()
}

/// A fresh code verifier: 64 random bytes (four v4 uuids, from the system's generator) in base64url, 86 characters.
pub(crate) fn verifier() -> String {
    let bytes: Vec<u8> = (0..4).flat_map(|_| *uuid::Uuid::new_v4().as_bytes()).collect();
    base64url(&bytes)
}

/// PKCE's S256 challenge of `verifier`.
pub(crate) fn challenge(verifier: &str) -> String {
    base64url(&sha256(verifier.as_bytes()))
}

/// `text` for an address's query: every byte but RFC 3986's unreserved ones as %XX.
pub(crate) fn encode(text: &str) -> String {
    text.bytes().map(|byte| if byte.is_ascii_alphanumeric() || b"-._~".contains(&byte) { (byte as char).to_string() } else { format!("%{byte:02X}") }).collect()
}

/// The %XX of a query read back (and + as a space).
pub(crate) fn decode(text: &str) -> String {
    let bytes = text.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut index = 0;
    while index < bytes.len() {
        let hex = bytes.get(index + 1..index + 3).filter(|hex| hex.iter().all(u8::is_ascii_hexdigit));
        let escaped = hex.and_then(|hex| u8::from_str_radix(std::str::from_utf8(hex).ok()?, 16).ok());
        match (bytes[index], escaped) {
            (b'%', Some(byte)) => {
                out.push(byte);
                index += 3;
                continue;
            }
            (b'+', _) => out.push(b' '),
            (byte, _) => out.push(byte),
        }
        index += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

/// A Client ID as Spotify's dashboard shows it: 32 hex digits.
pub(crate) fn client_id_looks_right(id: &str) -> bool {
    id.len() == 32 && id.chars().all(|letter| letter.is_ascii_hexdigit())
}

/// Spotify's authorize page for this app: the code comes back to `REDIRECT_URI` with `state`, made with `challenge`.
pub(crate) fn authorize_url(client_id: &str, challenge: &str, state: &str) -> String {
    let pairs = [
        ("response_type", "code"),
        ("client_id", client_id),
        ("scope", &SCOPES.join(" ")),
        ("redirect_uri", REDIRECT_URI),
        ("state", state),
        ("code_challenge_method", "S256"),
        ("code_challenge", challenge),
    ];
    let query: Vec<String> = pairs.iter().map(|(key, value)| format!("{key}={}", encode(value))).collect();
    format!("{AUTHORIZE}?{}", query.join("&"))
}

/// What the browser brought back to the listener.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum Callback {
    Code(String),
    /// Spotify said no (the person refused, or the app is not set up right), in words.
    Refused(String),
    /// Not our callback (the browser's favicon, a stray request, a state that is not ours): answered and ignored.
    Elsewhere,
}

/// The request line `GET /callback?code=…&state=… HTTP/1.1` read against the `state` this login sent.
pub(crate) fn callback(request_line: &str, state: &str) -> Callback {
    let mut words = request_line.split_whitespace();
    let (Some("GET"), Some(target)) = (words.next(), words.next()) else { return Callback::Elsewhere };
    let (path, query) = target.split_once('?').unwrap_or((target, ""));
    if path != "/callback" {
        return Callback::Elsewhere;
    }
    let value = |key: &str| query.split('&').filter_map(|pair| pair.split_once('=')).find(|(name, _)| *name == key).map(|(_, value)| decode(value));
    if value("state").as_deref() != Some(state) {
        return Callback::Elsewhere;
    }
    match (value("code"), value("error")) {
        (Some(code), None) if !code.is_empty() && code.len() < 2048 && code.chars().all(|letter| letter.is_ascii_graphic()) => Callback::Code(code),
        (_, Some(error)) if error == "access_denied" => Callback::Refused("Você não autorizou o acesso à conta do Spotify.".into()),
        (_, Some(error)) => {
            Callback::Refused(format!("O Spotify recusou o login: {}", error.chars().filter(|letter| letter.is_ascii_graphic()).take(80).collect::<String>()))
        }
        _ => Callback::Refused("O Spotify voltou sem o código do login.".into()),
    }
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct Tokens {
    pub(crate) access: String,
    /// When `access` ends, ms since the epoch.
    pub(crate) expires_at: i64,
    /// A new refresh token, when Spotify sent one (the first time always; on a renewal only sometimes).
    pub(crate) refresh: Option<String>,
}

/// The token endpoint's answer, at `now` (ms).
pub(crate) fn parse_tokens(root: &Value, now: i64) -> Option<Tokens> {
    let token = |key: &str| {
        root.get(key)
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|token| !token.is_empty() && token.len() <= 4096 && token.chars().all(|letter| letter.is_ascii_graphic()))
            .map(str::to_string)
    };
    let seconds = root.get("expires_in").and_then(Value::as_i64).unwrap_or(3600).clamp(60, 86_400);
    Some(Tokens { access: token("access_token")?, expires_at: now + seconds * 1000, refresh: token("refresh_token") })
}

/// Whether the access token must be renewed before a call: none yet, or ending within a minute.
pub(crate) fn needs_refresh(access: &str, expires_at: i64, now: i64) -> bool {
    access.is_empty() || now >= expires_at - RENEW_BEFORE_MS
}

/// The token endpoint's refusal in words; `renewal`: it was a refresh (a refused one means linking again).
pub(crate) fn token_failure(status: u16, body: &str, renewal: bool) -> Failure {
    let error = serde_json::from_str::<Value>(body).ok().and_then(|root| root.get("error").and_then(Value::as_str).map(str::to_string)).unwrap_or_default();
    let (message, unauthorized) = match error.as_str() {
        "invalid_grant" if renewal => (RELINK.to_string(), true),
        "invalid_grant" => ("O código do login expirou, ou o Redirect URI do app não é o de Ajustes: tente Conectar de novo.".into(), false),
        "invalid_client" => ("O Spotify não reconheceu o Client ID: confira o que foi colado em Ajustes.".into(), renewal),
        _ if status == 429 => ("Pedidos demais ao Spotify agora: tente daqui a pouco.".into(), false),
        _ => (format!("O Spotify recusou o login ({status})."), renewal && status == 400),
    };
    Failure { message, unauthorized, status }
}

/// The token endpoint with `form` (a form, never on the command line: curl reads it from its standard input).
fn token_request(form: &[(&str, &str)], renewal: bool) -> Result<Tokens, Failure> {
    let body = form.iter().map(|(key, value)| format!("{key}={}", encode(value))).collect::<Vec<_>>().join("&");
    let headers = vec!["Content-Type: application/x-www-form-urlencoded".into(), "User-Agent: MagoAgent".into()];
    let request = Request { method: "POST", url: TOKEN_URL, headers, body: Some(body), timeout_seconds: 8 };
    let answer = http::send(&request).map_err(|_| Failure { message: "Sem conexão com o Spotify.".into(), unauthorized: false, status: 0 })?;
    if answer.status != 200 {
        return Err(token_failure(answer.status, &answer.body, renewal));
    }
    let root: Value = serde_json::from_str(&answer.body).unwrap_or_default();
    parse_tokens(&root, now_ms()).ok_or(Failure { message: "A resposta do Spotify veio sem o token.".into(), unauthorized: false, status: answer.status })
}

/// What the threads share: the account's client id and refresh token, and the access token with its end.
pub(crate) struct Session {
    pub(crate) client_id: String,
    pub(crate) refresh: String,
    pub(crate) access: String,
    pub(crate) expires_at: i64,
}

pub(crate) type Shared = Arc<Mutex<Option<Session>>>;

/// A good access token: the one held, else a new one (a new refresh token Spotify hands goes to the keychain). One
/// renewal at a time: the others wait on the lock and take its token.
pub(crate) fn bearer(shared: &Shared) -> Result<String, Failure> {
    let mut held = shared.lock().map_err(|_| Failure { message: RELINK.into(), unauthorized: true, status: 0 })?;
    let session = held.as_mut().ok_or(Failure { message: super::playback::NEEDS_ACCOUNT.into(), unauthorized: false, status: 0 })?;
    if !needs_refresh(&session.access, session.expires_at, now_ms()) {
        return Ok(session.access.clone());
    }
    let tokens = token_request(&[("grant_type", "refresh_token"), ("refresh_token", &session.refresh), ("client_id", &session.client_id)], true)?;
    if let Some(refresh) = tokens.refresh.filter(|refresh| *refresh != session.refresh) {
        // the old one may stop working: the new one is kept even when the keychain refuses (until the app closes)
        let _ = set_secret(TOKEN_ID, &refresh);
        session.refresh = refresh;
    }
    (session.access, session.expires_at) = (tokens.access, tokens.expires_at);
    Ok(session.access.clone())
}

/// The access token was refused (401): the next call renews it.
pub(crate) fn expire(shared: &Shared) {
    if let Ok(mut held) = shared.lock()
        && let Some(session) = held.as_mut()
    {
        session.access.clear();
    }
}

#[derive(Clone, Debug, Default, PartialEq)]
pub(crate) enum Link {
    /// No account (or not looked for yet).
    #[default]
    Off,
    /// The browser is on Spotify's page; the listener waits for it.
    Waiting,
    Linked,
    Failed(String),
}

/// What a login brings back: the tokens and who.
pub(crate) struct Linked {
    pub(crate) client_id: String,
    pub(crate) tokens: Tokens,
    pub(crate) profile: Option<Profile>,
}

#[derive(Default)]
pub(crate) struct Account {
    pub(crate) link: Link,
    /// Who is linked, as Spotify names them, and their user id (Liked Songs' context is `spotify:user:<id>:collection`).
    pub(crate) name: String,
    pub(crate) user_id: String,
    pub(crate) shared: Shared,
    /// The keychain was looked at.
    loaded: bool,
    /// The login's thread answers here; the flag stops it (Cancelar, Desconectar).
    login: Option<(Receiver<Result<Linked, String>>, Arc<AtomicBool>)>,
}

impl Account {
    /// An account linked without the keychain or the network, for the tests.
    #[cfg(test)]
    pub(crate) fn with_session(user_id: &str) -> Account {
        let session = Session { client_id: "0".repeat(32), refresh: "refresh".into(), access: "access".into(), expires_at: i64::MAX };
        Account { link: Link::Linked, name: "Ana".into(), user_id: user_id.into(), shared: Arc::new(Mutex::new(Some(session))), loaded: true, login: None }
    }

    /// A refresh token is held: the Web API can be asked.
    pub(crate) fn linked(&self) -> bool {
        self.shared.lock().is_ok_and(|held| held.is_some())
    }

    fn forget(&mut self) {
        if let Some((_, stop)) = self.login.take() {
            stop.store(true, Ordering::Relaxed);
        }
        if let Ok(mut held) = self.shared.lock() {
            *held = None;
        }
        (self.name, self.user_id, self.link) = (String::new(), String::new(), Link::Off);
    }
}

/// One request on the listener: its first line, answered with a page that says what happened.
fn serve(mut stream: TcpStream, state: &str) -> Callback {
    let _ = stream.set_nonblocking(false);
    let _ = stream.set_read_timeout(Some(std::time::Duration::from_secs(3)));
    let mut request = Vec::new();
    let mut chunk = [0u8; 2048];
    while request.len() < 16 * 1024 && !request.windows(4).any(|window| window == b"\r\n\r\n") {
        match stream.read(&mut chunk) {
            Ok(0) | Err(_) => break,
            Ok(read) => request.extend_from_slice(&chunk[..read]),
        }
    }
    let text = String::from_utf8_lossy(&request);
    let found = callback(text.lines().next().unwrap_or_default(), state);
    let (status, words) = match &found {
        Callback::Code(_) => ("200 OK", "Conta do Spotify recebida. Pode fechar esta aba e voltar ao MagoAgent."),
        Callback::Refused(_) => ("200 OK", "O Spotify não autorizou. Volte ao MagoAgent e tente de novo."),
        Callback::Elsewhere => ("404 Not Found", ""),
    };
    let page = format!("<!doctype html><meta charset=\"utf-8\"><title>MagoAgent</title><p style=\"font:16px sans-serif;margin:3em\">{words}</p>");
    let _ = write!(stream, "HTTP/1.1 {status}\r\nContent-Type: text/html; charset=utf-8\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{page}", page.len());
    found
}

/// The login's thread: waits for the browser on `listener`, trades the code for the tokens and asks who it is.
fn login(listener: TcpListener, client_id: String, verifier: String, state: String, stop: Arc<AtomicBool>) -> Result<Linked, String> {
    listener.set_nonblocking(true).map_err(|error| error.to_string())?;
    let deadline = std::time::Instant::now() + LOGIN_FOR;
    let code = loop {
        if stop.load(Ordering::Relaxed) {
            return Err("Login do Spotify cancelado.".into());
        }
        if std::time::Instant::now() > deadline {
            return Err("O login do Spotify não voltou em 5 minutos: tente Conectar de novo.".into());
        }
        match listener.accept() {
            Ok((stream, _)) => match serve(stream, &state) {
                Callback::Code(code) => break code,
                Callback::Refused(why) => return Err(why),
                Callback::Elsewhere => {}
            },
            Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => std::thread::sleep(std::time::Duration::from_millis(100)),
            Err(_) => std::thread::sleep(std::time::Duration::from_millis(100)),
        }
    };
    drop(listener);
    let form = [("grant_type", "authorization_code"), ("code", &code), ("redirect_uri", REDIRECT_URI), ("client_id", &client_id), ("code_verifier", &verifier)];
    let tokens = token_request(&form, false).map_err(|failure| failure.message)?;
    if tokens.refresh.is_none() {
        return Err("O Spotify não mandou o token de renovação.".into());
    }
    let profile = web::profile_with(&tokens.access).ok();
    Ok(Linked { client_id, tokens, profile })
}

impl DrawingApp {
    /// Each frame the integration is on: the refresh token looked for once, and what a login brought back.
    pub(crate) fn spotify_account_tick(&mut self) {
        #[cfg(test)]
        mock_keyring();
        let client_id = self.prefs.integrations.spotify_client_id.trim().to_string();
        let account = &mut self.integrations.spotify.account;
        if !account.loaded {
            account.loaded = true;
            if client_id_looks_right(&client_id)
                && let Some(refresh) = take_secret(TOKEN_ID).filter(|refresh| !refresh.is_empty())
            {
                if let Ok(mut held) = account.shared.lock() {
                    *held = Some(Session { client_id, refresh, access: String::new(), expires_at: 0 });
                }
                account.link = Link::Linked;
            }
        }
        let Some(result) = account.login.as_ref().and_then(|(inbox, _)| inbox.try_recv().ok()) else { return };
        account.login = None;
        let said = match result {
            Ok(linked) => {
                let refresh = linked.tokens.refresh.clone().unwrap_or_default();
                let kept = set_secret(TOKEN_ID, &refresh);
                if let Ok(mut held) = account.shared.lock() {
                    *held = Some(Session { client_id: linked.client_id, refresh, access: linked.tokens.access, expires_at: linked.tokens.expires_at });
                }
                if let Some(profile) = linked.profile {
                    (account.name, account.user_id) = (profile.name, profile.id);
                }
                account.link = Link::Linked;
                let who = if account.name.is_empty() { String::new() } else { format!(" como {}", account.name) };
                match kept {
                    Ok(()) => format!("Conta do Spotify conectada{who}."),
                    Err(why) => format!("Conta do Spotify conectada{who}, mas o cofre do sistema recusou o token ({why}): vale só até fechar o app."),
                }
            }
            Err(why) => {
                account.link = Link::Failed(why.clone());
                why
            }
        };
        self.integrations.spotify.library.reset_lists();
        self.toast(said);
    }

    /// Conectar: the listener on `REDIRECT_PORT`, the browser on Spotify's page, and a thread that waits for it.
    pub(crate) fn spotify_connect(&mut self) -> Result<String, String> {
        let client_id = self.prefs.integrations.spotify_client_id.trim().to_string();
        if !client_id_looks_right(&client_id) {
            return Err("Cole o Client ID do seu app do Spotify (32 letras e números) antes de conectar.".into());
        }
        let account = &mut self.integrations.spotify.account;
        if account.login.is_some() {
            return Err("O navegador já está no login do Spotify: termine lá, ou cancele.".into());
        }
        let (verifier, state) = (verifier(), uuid::Uuid::new_v4().simple().to_string());
        let url = authorize_url(&client_id, &challenge(&verifier), &state);
        let (outbox, inbox) = channel();
        let stop = Arc::new(AtomicBool::new(false));
        // never a port, a browser or the network in a test
        if !cfg!(test) {
            let listener = TcpListener::bind(("127.0.0.1", REDIRECT_PORT))
                .map_err(|_| format!("A porta {REDIRECT_PORT} deste computador já está em uso: feche o que a usa e tente de novo."))?;
            let stopped = stop.clone();
            std::thread::spawn(move || drop(outbox.send(login(listener, client_id, verifier, state, stopped))));
            open_external(&url);
        } else {
            drop(outbox);
        }
        account.login = Some((inbox, stop));
        account.link = Link::Waiting;
        Ok("O navegador abriu no login do Spotify: autorize lá e volte.".into())
    }

    /// Cancelar the login in course.
    pub(crate) fn spotify_cancel_login(&mut self) {
        let account = &mut self.integrations.spotify.account;
        if let Some((_, stop)) = account.login.take() {
            stop.store(true, Ordering::Relaxed);
            account.link = Link::Off;
        }
    }

    /// Desconectar: the refresh token out of the keychain, and what the account brought forgotten.
    pub(crate) fn spotify_disconnect(&mut self) -> Result<String, String> {
        #[cfg(test)]
        mock_keyring();
        remove_secret(TOKEN_ID)?;
        let spotify = &mut self.integrations.spotify;
        spotify.account.forget();
        spotify.library.reset_lists();
        spotify.enrich();
        Ok("Conta do Spotify desconectada.".into())
    }
}
