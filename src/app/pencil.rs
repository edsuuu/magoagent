//! The portal pencil: draw over a page around what should change and write what you want; the
//! agents roped to the portal get the request with the address and the elements under the
//! stroke. The drawing layer lives in the page (the webview sits above everything the app draws).

use crate::prelude::*;

/// The page side: a transparent canvas over the page to draw on; when a stroke ends, the
/// elements under it (data-testid or id, and their text) and a box for the comment. Sending posts
/// it to the app with the secret the app armed it with (`__NONCE__`); Esc leaves; turning it on
/// again restarts it.
const PENCIL_SCRIPT: &str = r##"(() => {
  if (typeof window.__magoPencil === 'function') window.__magoPencil();
  // at the top of the closure, never inside a handler: the page can read layer.onpointerup.toString()
  const nonce = '__NONCE__';
  const css = (el, s) => (el.style.cssText = s, el);
  // a canvas is a replaced element: inset alone doesn't stretch it, so the bitmap would show at its own size
  const layer = css(document.createElement('canvas'), 'position:fixed;inset:0;width:100%;height:100%;z-index:2147483646;cursor:crosshair');
  const tip = css(document.createElement('div'), 'position:fixed;left:50%;top:12px;transform:translateX(-50%);z-index:2147483647;padding:6px 12px;border-radius:999px;background:#6965db;color:#fff;font:600 12px -apple-system,system-ui,sans-serif;pointer-events:none');
  tip.textContent = 'Lápis: circule o que quer mudar · Esc sai';
  const pen = layer.getContext('2d');
  const size = () => { layer.width = innerWidth * devicePixelRatio; layer.height = innerHeight * devicePixelRatio; pen.scale(devicePixelRatio, devicePixelRatio); };
  size(); addEventListener('resize', size);
  document.documentElement.append(layer, tip);
  let points = [], drawing = false, box = null;
  const quit = () => { layer.remove(); tip.remove(); box && box.remove(); removeEventListener('keydown', key, true); removeEventListener('resize', size); window.__magoPencil = null; };
  window.__magoPencil = quit;
  const key = e => { if (e.key === 'Escape') { e.preventDefault(); box ? (box.remove(), box = null, pen.clearRect(0, 0, innerWidth, innerHeight)) : quit(); } };
  addEventListener('keydown', key, true);
  layer.onpointerdown = e => { if (box) return; drawing = true; points = [[e.clientX, e.clientY]]; pen.clearRect(0, 0, innerWidth, innerHeight); };
  layer.onpointermove = e => {
    if (!drawing) return;
    const [x, y] = points[points.length - 1];
    points.push([e.clientX, e.clientY]);
    pen.strokeStyle = '#6965db'; pen.lineWidth = 3; pen.lineCap = 'round';
    pen.beginPath(); pen.moveTo(x, y); pen.lineTo(e.clientX, e.clientY); pen.stroke();
  };
  layer.onpointerup = () => {
    if (!drawing) return;
    drawing = false;
    if (points.length < 2) return;
    const xs = points.map(p => p[0]), ys = points.map(p => p[1]);
    const r = { x: Math.min(...xs), y: Math.min(...ys), w: Math.max(...xs) - Math.min(...xs), h: Math.max(...ys) - Math.min(...ys) };
    // what is under the stroke: a 5×5 grid over its box, the 8 smallest elements
    layer.style.pointerEvents = 'none';
    const seen = new Set();
    for (let i = 0; i <= 4; i++) for (let j = 0; j <= 4; j++)
      for (const el of document.elementsFromPoint(r.x + r.w * i / 4, r.y + r.h * j / 4))
        if (el !== document.documentElement && el !== document.body) seen.add(el);
    layer.style.pointerEvents = '';
    const area = el => { const b = el.getBoundingClientRect(); return b.width * b.height; };
    const elements = [...seen].sort((a, b) => area(a) - area(b)).slice(0, 8).map(el => ({
      id: el.getAttribute('data-testid') || el.id || el.tagName.toLowerCase(),
      text: (el.innerText || el.getAttribute('aria-label') || el.getAttribute('alt') || '').replace(/\s+/g, ' ').trim().slice(0, 120),
    }));
    box = css(document.createElement('div'), `position:fixed;left:${Math.max(0, Math.min(r.x, innerWidth - 340))}px;top:${Math.max(0, Math.min(r.y + r.h + 8, innerHeight - 150))}px;z-index:2147483647;width:320px;padding:10px;border-radius:12px;background:#232329;box-shadow:0 6px 20px rgba(0,0,0,.4);font:13px -apple-system,system-ui,sans-serif`);
    const text = css(document.createElement('textarea'), 'width:100%;height:70px;box-sizing:border-box;border:0;border-radius:8px;padding:8px;background:#31303c;color:#fff;resize:none;font:inherit');
    text.placeholder = 'O que mudar aqui? (' + (navigator.platform.startsWith('Mac') ? '⌘' : 'Ctrl+') + 'Enter envia)';
    const send = css(document.createElement('button'), 'margin-top:8px;padding:6px 14px;border:0;border-radius:8px;background:#6965db;color:#fff;font:600 12px -apple-system,system-ui,sans-serif;cursor:pointer');
    send.textContent = 'Enviar ao agente';
    box.append(text, send);
    document.documentElement.append(box);
    text.focus();
    const go = () => {
      if (!text.value.trim()) return;
      window.ipc.postMessage(JSON.stringify({ nonce, comment: text.value.trim(), url: location.href, rect: r, elements }));
      box.remove(); box = null; pen.clearRect(0, 0, innerWidth, innerHeight);
    };
    send.onclick = go;
    text.onkeydown = e => { if (e.key === 'Enter' && (e.metaKey || e.ctrlKey)) { e.preventDefault(); go(); } };
  };
  return 'lápis ligado';
})()"##;

/// What the agent reads: where, what is under the stroke, then the comment. The page wrote the
/// note, so each of its fields is cut to one line of bounded length: it can't fake a line of
/// the prompt.
#[cfg_attr(target_os = "linux", allow(dead_code))] // no pages on Linux
pub(crate) fn design_prompt(portal: &str, note: &Value) -> Option<String> {
    let comment = note["comment"].as_str()?.trim();
    let line = |v: &Value, n: usize| -> String { v.as_str().unwrap_or_default().split_whitespace().collect::<Vec<_>>().join(" ").chars().take(n).collect() };
    let r = &note["rect"];
    let n = |v: &Value| v.as_f64().unwrap_or(0.0).round();
    let mut text = format!("Anotação no portal \"{portal}\" ({})", line(&note["url"], 500));
    text += &format!("\nRegião circulada: x {} y {}, {}×{} px na página.", n(&r["x"]), n(&r["y"]), n(&r["w"]), n(&r["h"]));
    let elements = note["elements"].as_array().map(Vec::as_slice).unwrap_or_default();
    if !elements.is_empty() {
        text += "\nElementos sob o traço (do menor ao maior):";
        for e in elements.iter().take(8) {
            text += &format!("\n- `{}` \"{}\"", line(&e["id"], 60), line(&e["text"], 120));
        }
    }
    Some(format!("{text}\n---\n{comment}"))
}

impl DrawingApp {
    /// The agent terminals roped to an element: the ones running a command. A plain shell would
    /// run each pasted line as a command.
    pub(crate) fn roped_agents(&self, id: &str) -> Vec<String> {
        self.links().get(id).into_iter().flatten().filter(|o| self.scene.elements.iter().any(|e| &e.id == *o && e.terminal.as_ref().is_some_and(|t| !t.command.trim().is_empty()))).cloned().collect()
    }

    /// The context bar's Lápis: the drawing layer goes into the portal's page, armed with a
    /// fresh secret. With nobody to send to, nothing to draw (the note would be lost).
    pub(crate) fn start_pencil(&mut self, id: &str) {
        if self.roped_agents(id).is_empty() {
            self.toast("Ligue um agente a este portal para usar o lápis");
            return;
        }
        self.arm_pencil(id, PENCIL_SCRIPT);
    }

    /// A drawn and written note arrived from a tab's page: it goes to the agents roped to the
    /// portal. Any page can post to the app, so only a note the size of a note, from the tab the
    /// pencil was armed in and carrying its secret, gets through; the rest is dropped quietly.
    #[cfg(not(target_os = "linux"))]
    pub(crate) fn pencil_note(&mut self, portal: &str, tab: u64, message: &str, ctx: &Context) {
        if message.len() > 16 * 1024 {
            return;
        }
        let Ok(note) = serde_json::from_str::<Value>(message) else { return };
        if !self.pencil_armed(portal, tab, note["nonce"].as_str()) {
            return;
        }
        let Some(element) = self.scene.elements.iter().find(|e| e.id == portal) else { return };
        let name = element.portal.as_ref().map_or("portal".into(), |p| p.name.clone());
        let Some(prompt) = design_prompt(&name, &note) else { return };
        // on the Mac the agent also gets a picture of the circled region: the note waits for it
        #[cfg(target_os = "macos")]
        if let Some(view) = self.portals.get(portal).and_then(|v| v.tabs.iter().find(|t| t.uid == tab)) {
            let r = |k: &str| note["rect"][k].as_f64().unwrap_or(0.0);
            let (events, portal, ctx) = (self.portal_events.0.clone(), portal.to_string(), ctx.clone());
            snapshot(&view.webview, [r("x") - 16.0, r("y") - 16.0, r("w") + 32.0, r("h") + 32.0], move |png| {
                let _ = events.send(PortalEvent::Shot { portal, prompt, png });
                ctx.request_repaint();
            });
            return;
        }
        // ponytail: no picture on Windows yet; WebView2's CapturePreview would give the page's
        let _ = (tab, ctx);
        self.deliver_note(portal, &prompt, None);
    }

    /// The note goes to the agents roped to the portal; with a picture, its path goes along (the
    /// agent opens it), saved in the temp folder.
    #[cfg(not(target_os = "linux"))]
    pub(crate) fn deliver_note(&mut self, portal: &str, prompt: &str, png: Option<Vec<u8>>) {
        let agents = self.roped_agents(portal);
        if agents.is_empty() {
            self.toast("Nenhum agente ligado ao portal: a anotação não foi enviada");
            return;
        }
        let dir = std::env::temp_dir().join("magoagent-lapis");
        let path = dir.join(format!("{}.png", new_id()));
        let saved = png.is_some_and(|png| fs::create_dir_all(&dir).and_then(|_| fs::write(&path, png)).is_ok());
        let prompt = if saved { prompt.replacen("\n---\n", &format!("\nRecorte da região (imagem): {}\n---\n", path.display()), 1) } else { prompt.to_string() };
        for agent in &agents {
            self.send_prompt(agent, &prompt);
        }
        self.toast(format!("Anotação enviada a {}", if agents.len() == 1 { "1 agente".to_string() } else { format!("{} agentes", agents.len()) }));
    }
}

/// A PNG of `rect` (x, y, width, height in the page's CSS pixels) of what the page shows now;
/// `done` runs on the main thread when WebKit has it (None if it failed).
#[cfg(target_os = "macos")]
pub(crate) fn snapshot(webview: &wry::WebView, [x, y, w, h]: [f64; 4], done: impl FnOnce(Option<Vec<u8>>) + 'static) {
    use objc2_app_kit::{NSBitmapImageFileType, NSBitmapImageRep, NSImage};
    use objc2_foundation::{NSDictionary, NSError, NSPoint, NSRect, NSSize};
    use wry::WebViewExtMacOS;
    let Some(mtm) = objc2::MainThreadMarker::new() else { return done(None) };
    let view = webview.webview();
    // the rect is in the view's points: the page's CSS px times its zoom (the canvas zoom, set in sync_portals)
    let zoom = unsafe { view.pageZoom() };
    let [x, y, w, h] = [x, y, w, h].map(|v| v * zoom);
    let config = unsafe { objc2_web_kit::WKSnapshotConfiguration::new(mtm) };
    unsafe { config.setRect(NSRect::new(NSPoint::new(x.max(0.0), y.max(0.0)), NSSize::new(w.max(1.0), h.max(1.0)))) };
    let done = std::cell::Cell::new(Some(done));
    let block = block2::RcBlock::new(move |image: *mut NSImage, _: *mut NSError| {
        let png = unsafe { image.as_ref() }.and_then(|image| {
            let tiff = image.TIFFRepresentation()?;
            let rep = NSBitmapImageRep::imageRepWithData(&tiff)?;
            unsafe { rep.representationUsingType_properties(NSBitmapImageFileType::PNG, &NSDictionary::new()) }.map(|d| d.to_vec())
        });
        if let Some(done) = done.take() {
            done(png);
        }
    });
    unsafe { view.takeSnapshotWithConfiguration_completionHandler(Some(&config), &block) };
}
