//! The call's voice on OpenAI's API: what is said is heard by `gpt-transcribe` and the answer is
//! said by `gpt-4o-mini-tts` (24 kHz PCM, played as it comes). Nothing to load: the call starts at
//! once. The key (Ajustes › API) goes to curl on its standard input, never on the command line.
// ponytail: /v1/audio/speech shuts down on 2027-01-06; move the speaking to the Realtime API
// (gpt-realtime-2.1-mini) before then

use crate::prelude::*;
use crate::voice::Player;
use std::io::Write;
use std::process::Stdio;

/// The voices offered in Ajustes › Voz, the ones OpenAI recommends first.
pub(crate) const OPENAI_VOICES: [(&str, &str); 9] = [
    ("marin", "Marin (feminina)"),
    ("coral", "Coral (feminina)"),
    ("nova", "Nova (feminina)"),
    ("shimmer", "Shimmer (feminina)"),
    ("sage", "Sage (feminina)"),
    ("cedar", "Cedar (masculina)"),
    ("ash", "Ash (masculina)"),
    ("ballad", "Ballad (masculina)"),
    ("onyx", "Onyx (masculina)"),
];

const API: &str = "https://api.openai.com/v1/audio";
const SPEAKING: &str = "Fale em português do Brasil, com naturalidade, calor e ritmo de conversa, como quem explica algo a um colega.";

/// The key from Ajustes › API, else the environment's.
pub(crate) fn key(prefs: &Prefs) -> Option<String> {
    let key = prefs.openai_key.trim();
    let key = if key.is_empty() { std::env::var("OPENAI_API_KEY").unwrap_or_default() } else { key.to_string() };
    (!key.trim().is_empty()).then(|| key.trim().to_string())
}

/// A curl config (`curl -K -`): the key's header and each option, quoted as curl reads them.
fn config(key: &str, options: &[(&str, &str)]) -> String {
    let quote = |s: &str| s.replace('\\', "\\\\").replace('"', "\\\"");
    let mut text = format!("header = \"Authorization: Bearer {}\"\n", quote(key));
    for (name, value) in options {
        text += &format!("{name} = \"{}\"\n", quote(value));
    }
    text
}

/// curl with `config` on its standard input.
fn curl(args: &[&str], config: String) -> std::io::Result<std::process::Child> {
    let mut child = crate::ide::command("curl").args(args).args(["-K", "-"]).stdin(Stdio::piped()).stdout(Stdio::piped()).stderr(Stdio::piped()).spawn()?;
    if let Some(mut stdin) = child.stdin.take() {
        stdin.write_all(config.as_bytes())?;
    }
    Ok(child)
}

/// The words in `samples` (16 kHz mono); `language` is "pt", "en" or "auto".
pub(crate) fn transcribe(key: &str, language: &str, samples: &[f32]) -> Result<String, String> {
    let path = std::env::temp_dir().join(format!("magoagent-voz-{}.wav", uuid::Uuid::new_v4().simple()));
    fs::write(&path, crate::voice::wav(samples, crate::voice::SAMPLE_RATE)).map_err(|e| e.to_string())?;
    let file = format!("file=@{}", path.display());
    let url = format!("{API}/transcriptions");
    let ask = |language: Option<&str>| {
        let lang = language.map(|l| format!("languages[]={l}"));
        let mut options = vec![("url", url.as_str()), ("form", "model=gpt-transcribe"), ("form", "response_format=json"), ("form", file.as_str())];
        options.extend(lang.as_deref().map(|l| ("form", l)));
        let output = curl(&["-sS", "--fail-with-body"], config(key, &options)).and_then(|c| c.wait_with_output()).map_err(|e| format!("curl: {e}"))?;
        heard(&output.stdout, output.status.success(), &String::from_utf8_lossy(&output.stderr))
    };
    let lang = (language != "auto").then_some(language);
    // the language is a hint: without it, if the API doesn't take it
    let result = ask(lang).or_else(|e| if lang.is_some() { ask(None) } else { Err(e) });
    let _ = fs::remove_file(&path);
    result
}

/// The text of a transcription's answer, or its error's message.
fn heard(body: &[u8], ok: bool, stderr: &str) -> Result<String, String> {
    let json: Value = serde_json::from_slice(body).unwrap_or_default();
    if !ok {
        return Err(json["error"]["message"].as_str().map_or_else(|| stderr.trim().to_string(), str::to_string));
    }
    Ok(json["text"].as_str().unwrap_or_default().trim().to_string())
}

/// `text` said by the OpenAI `voice`, played as it arrives.
pub(crate) fn say(key: &str, voice: &str, text: &str) -> Option<Player> {
    let body = json!({ "model": "gpt-4o-mini-tts", "voice": voice, "input": text, "response_format": "pcm", "instructions": SPEAKING }).to_string();
    let url = format!("{API}/speech");
    let options = [("url", url.as_str()), ("header", "Content-Type: application/json"), ("data-binary", body.as_str())];
    let child = curl(&["-sS", "-N", "--fail"], config(key, &options)).ok()?;
    Some(Player::from_pcm(child, 24_000))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_key_goes_in_curls_config_quoted() {
        let text = config("sk-1", &[("url", "https://x"), ("data-binary", r#"{"input":"a \"b\"\nc\\d"}"#)]);
        assert_eq!(text.lines().next(), Some(r#"header = "Authorization: Bearer sk-1""#));
        assert!(text.contains(r#"data-binary = "{\"input\":\"a \\\"b\\\"\\nc\\\\d\"}""#), "{text}");
    }

    #[test]
    fn a_transcription_gives_its_text_or_its_error() {
        assert_eq!(heard(br#"{"text":" Oi, tudo bem? "}"#, true, ""), Ok("Oi, tudo bem?".into()));
        assert_eq!(heard(br#"{"error":{"message":"Incorrect API key"}}"#, false, "curl: (22)"), Err("Incorrect API key".into()));
        assert_eq!(heard(b"", false, " curl: (6) no host "), Err("curl: (6) no host".into()));
    }

    #[test]
    fn the_key_comes_from_ajustes() {
        let prefs = Prefs { openai_key: " sk-9 ".into(), ..Default::default() };
        assert_eq!(key(&prefs).as_deref(), Some("sk-9"));
    }
}
