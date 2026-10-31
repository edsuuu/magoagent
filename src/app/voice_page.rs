//! The sound of a call on Codex's voice (voice_codex.rs): a page out of sight holds the microphone
//! and the speaker in a WebRTC call, as Codex's own app does; its offer and the answer go through
//! `codex app-server`, and it tells the app the levels for the button and the avatar.
// ponytail: on macOS a page from a custom scheme may not count as a secure context for the
// microphone; if getUserMedia is refused there, serve it from 127.0.0.1 instead

use crate::prelude::*;
use std::sync::atomic::Ordering;
use std::sync::mpsc::Receiver;

const PAGE: &str = r#"<!doctype html><meta charset="utf-8"><script>
const post = m => window.ipc.postMessage(JSON.stringify(m));
let pc, mic, audio, speaker, deaf = false;
function level(stream) {
  const an = audio.createAnalyser();
  audio.createMediaStreamSource(stream).connect(an);
  const buf = new Float32Array(1024);
  return () => { an.getFloatTimeDomainData(buf); let m = 0; for (const v of buf) m = Math.max(m, Math.abs(v)); return m; };
}
(async () => {
  try {
    audio = new AudioContext();
    audio.resume();
    pc = new RTCPeerConnection();
    mic = await navigator.mediaDevices.getUserMedia({ audio: { echoCancellation: true, noiseSuppression: true, autoGainControl: true } });
    mic.getTracks().forEach(t => pc.addTrack(t, mic));
    const heard = level(mic);
    let said = () => 0;
    pc.ontrack = e => {
      const out = speaker = new Audio(); out.autoplay = true; out.muted = deaf; out.srcObject = e.streams[0]; out.play().catch(() => {});
      said = level(e.streams[0]);
    };
    pc.onconnectionstatechange = () => { if (pc.connectionState === 'failed') post({ error: 'A conexão de áudio caiu.' }); };
    // what you say is written down in Portuguese: left to guess, short phrases came out in Arabic
    const events = pc.createDataChannel('oai-events');
    events.onopen = () => events.send(JSON.stringify({ type: 'session.update', session: { type: 'realtime',
      audio: { input: { transcription: { model: 'gpt-4o-mini-transcribe', language: 'pt' } } } } }));
    events.onmessage = e => { try { const m = JSON.parse(e.data); if (m.type === 'error') post({ note: m.error && m.error.message || 'erro' }); } catch (_) {} };
    await pc.setLocalDescription(await pc.createOffer());
    post({ offer: pc.localDescription.sdp });
    setInterval(() => post({ mic: heard(), out: said() }), 100);
  } catch (e) {
    post({ error: String(e) });
  }
})();
function answer(sdp) { pc.setRemoteDescription({ type: 'answer', sdp }).catch(e => post({ error: String(e) })); }
function mute(on) { mic && mic.getAudioTracks().forEach(t => t.enabled = !on); }
function speakers(off) { deaf = off; if (speaker) speaker.muted = off; }
</script>"#;

/// What the page says.
#[derive(Debug, PartialEq)]
pub(crate) enum PageNote {
    Offer(String),
    Levels {
        mic: f32,
        out: f32,
    },
    Failed(String),
    /// The voice's server refused something on the way (the call goes on).
    Note(String),
}

pub(crate) fn page_note(message: &str) -> Option<PageNote> {
    let v: Value = serde_json::from_str(message).ok()?;
    if let Some(sdp) = v["offer"].as_str() {
        return Some(PageNote::Offer(sdp.into()));
    }
    if let Some(note) = v["note"].as_str() {
        return Some(PageNote::Note(note.into()));
    }
    if let Some(error) = v["error"].as_str() {
        let why = if error.contains("NotAllowed") || error.contains("Permission") { crate::voice::MIC_DENIED } else { error };
        return Some(PageNote::Failed(why.into()));
    }
    Some(PageNote::Levels { mic: v["mic"].as_f64()? as f32, out: v["out"].as_f64()? as f32 })
}

/// The page, while a call on Codex's voice is on.
pub(crate) struct VoicePage {
    webview: wry::WebView,
    notes: Receiver<String>,
    muted: bool,
    speakers_off: bool,
}

fn open_page(window: &impl wry::raw_window_handle::HasWindowHandle, ctx: &Context) -> wry::Result<VoicePage> {
    let (tx, notes) = std::sync::mpsc::channel();
    let ctx = ctx.clone();
    let url = if cfg!(windows) { "http://magovoz.localhost/" } else { "magovoz://localhost/" };
    let webview = wry::WebViewBuilder::new()
        .with_custom_protocol("magovoz".into(), |_, _| {
            wry::http::Response::builder().header("Content-Type", "text/html; charset=utf-8").body(std::borrow::Cow::Borrowed(PAGE.as_bytes())).unwrap()
        })
        .with_url(url)
        .with_autoplay(true)
        .with_permission_handler(|kind| match kind {
            wry::PermissionKind::Microphone => wry::PermissionResponse::Allow,
            _ => wry::PermissionResponse::Deny,
        })
        // shown (a hidden page may be put to sleep) but a pixel outside the window
        .with_bounds(wry::Rect { position: wry::dpi::LogicalPosition::new(-10.0, -10.0).into(), size: wry::dpi::LogicalSize::new(1.0, 1.0).into() })
        .with_ipc_handler(move |request| {
            let _ = tx.send(request.into_body());
            ctx.request_repaint();
        })
        .build_as_child(window)?;
    Ok(VoicePage { webview, notes, muted: false, speakers_off: false })
}

impl DrawingApp {
    /// Every frame: the page while the call is on Codex's voice, its notes to the call, the
    /// answer and Mudo to it.
    pub(crate) fn sync_voice_page(&mut self, window: &impl wry::raw_window_handle::HasWindowHandle, ctx: &Context) {
        if !(self.voice.on && self.voice.codex.is_some()) {
            self.voice_page = None;
            return;
        }
        if self.voice_page.is_none() {
            match open_page(window, ctx) {
                Ok(page) => self.voice_page = Some(page),
                Err(e) => return self.voice_fail(format!("Não foi possível iniciar o chat por voz: {e}")),
            }
        }
        let notes: Vec<PageNote> = self.voice_page.as_ref().map(|p| p.notes.try_iter().filter_map(|m| page_note(&m)).collect()).unwrap_or_default();
        for note in notes {
            match note {
                PageNote::Offer(sdp) => self.voice.offer = Some(sdp),
                PageNote::Levels { mic, out } => {
                    self.voice.shared.level.store(mic.to_bits(), Ordering::Relaxed);
                    self.voice.talking = out > 0.02;
                }
                PageNote::Note(note) => self.voice.status = format!("Aviso da voz: {note}"),
                PageNote::Failed(why) => {
                    self.voice_fail(why);
                    self.voice_page = None;
                    return;
                }
            }
        }
        let Some(page) = self.voice_page.as_mut() else { return };
        if let Some(sdp) = self.voice.answer.take() {
            let _ = page.webview.evaluate_script(&format!("answer({})", json!(sdp)));
        }
        let muted = self.voice.shared.muted.load(Ordering::Relaxed);
        if muted != page.muted {
            page.muted = muted;
            let _ = page.webview.evaluate_script(&format!("mute({muted})"));
        }
        if self.voice.speakers_off != page.speakers_off {
            page.speakers_off = self.voice.speakers_off;
            let _ = page.webview.evaluate_script(&format!("speakers({})", page.speakers_off));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_pages_notes() {
        assert_eq!(page_note(r#"{"offer":"v=0"}"#), Some(PageNote::Offer("v=0".into())));
        assert_eq!(page_note(r#"{"mic":0.5,"out":0}"#), Some(PageNote::Levels { mic: 0.5, out: 0.0 }));
        assert_eq!(
            page_note(r#"{"error":"NotAllowedError: Permission denied"}"#),
            Some(PageNote::Failed("O acesso ao microfone é necessário para iniciar o chat por voz".into()))
        );
        assert_eq!(page_note(r#"{"note":"Unknown parameter"}"#), Some(PageNote::Note("Unknown parameter".into())));
        assert_eq!(page_note("lixo"), None);
    }
}
