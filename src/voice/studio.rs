//! The call's voice on a VoiceStudio server running on this computer (or in the network). VoiceStudio
//! (AGPL-3.0) is a separate program: nothing of it is in MagoAgent, the call only talks to its
//! OpenAI-compatible HTTP API (`/v1/audio/speech`, `/v1/audio/transcriptions`, `/v1/audio/voices`),
//! through curl as the OpenAI engine does. What is said is heard by its speech-to-text and the answer
//! is said by the voice chosen in Ajustes › Voz (24 kHz PCM, played as it comes).
// ponytail: one request per answer, no streaming of the text; the server's own queue (one GPU job at
// a time) orders the sentences

use super::openai::{config, curl, heard};
use crate::prelude::*;
use crate::voice::Player;
use std::sync::{Arc, Mutex};

/// The address the VoiceStudio server listens on by default.
pub(crate) const STUDIO_URL: &str = "http://127.0.0.1:3900";
/// A voice every server has: it maps to the engine's default voice.
pub(crate) const STUDIO_VOICE: &str = "alloy";

/// Where the server is and what to ask of it.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct StudioServer {
    /// Without the trailing slash, with the scheme (`http://127.0.0.1:3900`).
    pub(crate) url: String,
    /// The API key, empty when the server asks for none.
    pub(crate) key: String,
    pub(crate) voice: String,
}

impl StudioServer {
    pub(crate) fn from_prefs(prefs: &Prefs) -> StudioServer {
        let url = prefs.voice_studio_url.trim().trim_end_matches('/');
        let url = match url {
            "" => STUDIO_URL.to_string(),
            url if url.contains("://") => url.to_string(),
            url => format!("http://{url}"),
        };
        let voice = prefs.voice_studio_voice.trim();
        StudioServer { url, key: prefs.voice_studio_key.trim().to_string(), voice: if voice.is_empty() { STUDIO_VOICE.into() } else { voice.into() } }
    }

    /// How to bring the server up, for when it isn't there.
    fn missing(&self) -> String {
        format!(
            "Não achei o VoiceStudio em {}. Abra o app dele (github.com/debpalash/VoiceStudio/releases) ou suba o servidor com: docker run -d -p 127.0.0.1:3900:3900 ghcr.io/debpalash/voicestudio:stable. O endereço fica em Ajustes › Voz.",
            self.url
        )
    }

    /// The voices the server offers, as (id, name): its default aliases and the profiles made in it.
    /// Also the check that the server is up and takes the key.
    pub(crate) fn voices(&self) -> Result<Vec<(String, String)>, String> {
        let url = format!("{}/v1/audio/voices", self.url);
        let child = curl(&["-sS", "--fail-with-body", "--max-time", "6"], config(&self.key, &[("url", url.as_str())])).map_err(|e| format!("curl: {e}"))?;
        let output = child.wait_with_output().map_err(|e| format!("curl: {e}"))?;
        if !output.status.success() {
            // 22: the server answered with an error; anything else, it was not reached
            return Err(match output.status.code() {
                Some(22) => match heard_error(&output.stdout) {
                    Some(why) if why.to_lowercase().contains("auth") => "O VoiceStudio pede a chave de API: ponha em Ajustes › Voz.".to_string(),
                    Some(why) => format!("O VoiceStudio respondeu: {why}"),
                    None => format!("O VoiceStudio em {} recusou o pedido.", self.url),
                },
                _ => self.missing(),
            });
        }
        let not_it = || format!("{} não parece um servidor VoiceStudio.", self.url);
        let json: Value = serde_json::from_slice(&output.stdout).map_err(|_| not_it())?;
        let text = |voice: &Value, field: &str| voice[field].as_str().map(str::to_string);
        let voices = json["voices"].as_array().ok_or_else(not_it)?;
        Ok(voices.iter().filter_map(|v| Some((text(v, "voice_id").or_else(|| text(v, "id"))?, text(v, "name")?))).collect())
    }

    /// The curl that asks for `text` said, its standard output the raw PCM.
    fn speech(&self, text: &str) -> std::io::Result<std::process::Child> {
        let body = json!({ "model": "tts-1", "voice": self.voice, "input": text, "response_format": "pcm" }).to_string();
        let url = format!("{}/v1/audio/speech", self.url);
        let options = [("url", url.as_str()), ("header", "Content-Type: application/json"), ("data-binary", body.as_str())];
        curl(&["-sS", "-N", "--fail"], config(&self.key, &options))
    }

    /// `text` said by the chosen voice, played as it arrives.
    pub(crate) fn say(&self, text: &str) -> Option<Player> {
        Some(Player::from_pcm(self.speech(text).ok()?, 24_000))
    }

    /// The words in `samples` (16 kHz mono); `language` is "pt", "en" or "auto".
    pub(crate) fn transcribe(&self, language: &str, vocabulary: &str, samples: &[f32]) -> Result<String, String> {
        let path = std::env::temp_dir().join(format!("magoagent-voz-{}.wav", uuid::Uuid::new_v4().simple()));
        fs::write(&path, crate::voice::wav(samples, crate::voice::SAMPLE_RATE)).map_err(|e| e.to_string())?;
        let (url, file) = (format!("{}/v1/audio/transcriptions", self.url), format!("file=@{}", path.display()));
        let language = format!("language={language}");
        let mut options = vec![("url", url.as_str()), ("form", "model=whisper-1"), ("form", "response_format=json"), ("form", file.as_str())];
        let prompt = format!("prompt={vocabulary}");
        if !vocabulary.is_empty() {
            options.push(("form", prompt.as_str()));
        }
        if language != "language=auto" {
            options.push(("form", language.as_str()));
        }
        let result = curl(&["-sS", "--fail-with-body"], config(&self.key, &options)).and_then(|c| c.wait_with_output());
        let _ = fs::remove_file(&path);
        let output = result.map_err(|e| format!("curl: {e}"))?;
        if matches!(output.status.code(), Some(code) if code != 22 && code != 0) {
            return Err(self.missing());
        }
        heard(&output.stdout, output.status.success(), &String::from_utf8_lossy(&output.stderr))
    }
}

/// The message of an OpenAI-shaped error body.
fn heard_error(body: &[u8]) -> Option<String> {
    let json: Value = serde_json::from_slice(body).ok()?;
    json["error"]["message"].as_str().or_else(|| json["detail"].as_str()).map(str::to_string)
}

/// What Ajustes › Voz shows of the server's voices: not asked yet, being asked, or the answer.
#[derive(Clone, Debug, Default)]
pub(crate) enum StudioVoices {
    #[default]
    Idle,
    Loading,
    Done(Vec<(String, String)>),
    Failed(String),
}

/// Asks `server` for its voices on a thread; `list` goes from Loading to the answer.
pub(crate) fn fetch_voices(server: StudioServer, list: Arc<Mutex<StudioVoices>>, ctx: Context) {
    *list.lock().unwrap() = StudioVoices::Loading;
    std::thread::spawn(move || {
        *list.lock().unwrap() = server.voices().map_or_else(StudioVoices::Failed, StudioVoices::Done);
        ctx.request_repaint();
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::{Read, Write};
    use std::net::TcpListener;
    use std::sync::mpsc::{Receiver, channel};

    /// A server on a free port that answers like VoiceStudio's API; every request it gets (head and
    /// body) comes out of the receiver. `pcm` is what its speech answers.
    fn fake_server(pcm: Vec<u8>) -> (String, Receiver<String>) {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let url = format!("http://{}", listener.local_addr().unwrap());
        let (tx, rx) = channel();
        std::thread::spawn(move || {
            for stream in listener.incoming() {
                let Ok(mut stream) = stream else { return };
                let mut request = Vec::new();
                let mut buf = [0u8; 4096];
                let (mut head_end, mut length) = (None, 0);
                loop {
                    let n = stream.read(&mut buf).unwrap_or(0);
                    request.extend_from_slice(&buf[..n]);
                    if head_end.is_none() {
                        head_end = request.windows(4).position(|w| w == b"\r\n\r\n").map(|i| i + 4);
                        let head = String::from_utf8_lossy(&request[..head_end.unwrap_or(0)]).to_lowercase();
                        length = head.split("content-length:").nth(1).and_then(|l| l.lines().next()).and_then(|l| l.trim().parse().ok()).unwrap_or(0);
                    }
                    if n == 0 || head_end.is_some_and(|h| request.len() >= h + length) {
                        break;
                    }
                }
                let text = String::from_utf8_lossy(&request).to_string();
                let line = text.lines().next().unwrap_or_default().to_string();
                let (kind, body): (&str, Vec<u8>) = if line.contains("/v1/audio/voices") {
                    (
                        "application/json",
                        br#"{"voices":[{"voice_id":"alloy","id":"alloy","name":"Alloy"},{"voice_id":"p1","id":"p1","name":"Locutora"}],"engines":[]}"#.to_vec(),
                    )
                } else if line.contains("/v1/audio/speech") {
                    ("audio/pcm", pcm.clone())
                } else if line.contains("/v1/audio/transcriptions") {
                    ("application/json", br#"{"text":" Oi, tudo bem? "}"#.to_vec())
                } else {
                    ("application/json", br#"{"error":{"message":"not found"}}"#.to_vec())
                };
                let head = format!("HTTP/1.1 200 OK\r\nContent-Type: {kind}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n", body.len());
                let _ = stream.write_all(head.as_bytes());
                let _ = stream.write_all(&body);
                let _ = tx.send(text);
            }
        });
        (url, rx)
    }

    fn server(url: &str) -> StudioServer {
        StudioServer { url: url.into(), key: "chave-1".into(), voice: "p1".into() }
    }

    #[test]
    fn the_server_lists_its_voices_and_is_asked_with_the_key() {
        let (url, requests) = fake_server(Vec::new());
        let voices = server(&url).voices().unwrap();
        assert_eq!(voices, vec![("alloy".to_string(), "Alloy".to_string()), ("p1".to_string(), "Locutora".to_string())]);
        let request = requests.recv().unwrap();
        assert!(request.starts_with("GET /v1/audio/voices "), "{request}");
        assert!(request.to_lowercase().contains("authorization: bearer chave-1"), "{request}");
    }

    #[test]
    fn the_answer_is_asked_in_the_chosen_voice_and_its_audio_comes_back() {
        // four samples: 0, 1, -1, 32767
        let pcm = vec![0, 0, 1, 0, 255, 255, 255, 127];
        let (url, requests) = fake_server(pcm.clone());
        let mut child = server(&url).speech("Terminei os testes.").unwrap();
        let mut audio = Vec::new();
        child.stdout.take().unwrap().read_to_end(&mut audio).unwrap();
        assert!(child.wait().unwrap().success());
        assert_eq!(audio, pcm);
        let request = requests.recv().unwrap();
        assert!(request.starts_with("POST /v1/audio/speech "), "{request}");
        let body: Value = serde_json::from_str(request.split("\r\n\r\n").nth(1).unwrap()).unwrap();
        assert_eq!((body["voice"].as_str(), body["input"].as_str(), body["response_format"].as_str()), (Some("p1"), Some("Terminei os testes."), Some("pcm")));
    }

    #[test]
    fn what_is_heard_goes_as_a_wav_and_its_words_come_back() {
        let (url, requests) = fake_server(Vec::new());
        let words = StudioServer { key: String::new(), ..server(&url) }.transcribe("pt", "", &[0.0; 1600]).unwrap();
        assert_eq!(words, "Oi, tudo bem?");
        let request = requests.recv().unwrap();
        assert!(request.starts_with("POST /v1/audio/transcriptions "), "{request}");
        assert!(request.contains("name=\"language\"") && request.contains("RIFF"), "{request}");
        assert!(!request.to_lowercase().contains("authorization"), "no key, no header");
    }

    #[test]
    fn without_the_server_the_message_says_how_to_bring_it_up() {
        // a port nobody listens on
        let port = TcpListener::bind("127.0.0.1:0").unwrap().local_addr().unwrap().port();
        let why = server(&format!("http://127.0.0.1:{port}")).voices().unwrap_err();
        assert!(why.contains("Não achei o VoiceStudio") && why.contains("docker run") && why.contains(&port.to_string()), "{why}");
    }

    #[test]
    fn the_address_is_cleaned_and_defaults_to_voicestudios() {
        let at =
            |url: &str, voice: &str| StudioServer::from_prefs(&Prefs { voice_studio_url: url.into(), voice_studio_voice: voice.into(), ..Default::default() });
        assert_eq!((at("", "").url.as_str(), at("", "").voice.as_str()), (STUDIO_URL, STUDIO_VOICE));
        assert_eq!(at(" localhost:3900/ ", "p1").url, "http://localhost:3900");
        assert_eq!(at("https://voz.casa/", "p1").url, "https://voz.casa");
    }
}
