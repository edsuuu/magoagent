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

/// What the agent reads: where, what is under the stroke, then the comment.
#[cfg_attr(target_os = "linux", allow(dead_code))] // no pages on Linux
pub(crate) fn design_prompt(portal: &str, note: &Value) -> Option<String> {
    let comment = note["comment"].as_str()?.trim();
    let r = &note["rect"];
    let n = |v: &Value| v.as_f64().unwrap_or(0.0).round();
    let mut text = format!("Anotação no portal \"{portal}\" ({})", note["url"].as_str().unwrap_or_default());
    text += &format!("\nRegião circulada: x {} y {}, {}×{} px na página.", n(&r["x"]), n(&r["y"]), n(&r["w"]), n(&r["h"]));
    let elements = note["elements"].as_array().map(Vec::as_slice).unwrap_or_default();
    if !elements.is_empty() {
        text += "\nElementos sob o traço (do menor ao maior):";
        for e in elements.iter().take(8) {
            text += &format!("\n- `{}` \"{}\"", e["id"].as_str().unwrap_or("?"), e["text"].as_str().unwrap_or_default());
        }
    }
    Some(format!("{text}\n---\n{comment}"))
}

impl DrawingApp {
    /// The terminals roped to an element.
    fn roped_agents(&self, id: &str) -> Vec<String> {
        self.links().get(id).into_iter().flatten().filter(|o| self.scene.elements.iter().any(|e| &e.id == *o && e.terminal.is_some())).cloned().collect()
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
    pub(crate) fn pencil_note(&mut self, portal: &str, tab: u64, message: &str) {
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
        let agents = self.roped_agents(portal);
        if agents.is_empty() {
            self.toast("Nenhum agente ligado ao portal: a anotação não foi enviada");
            return;
        }
        for agent in &agents {
            self.send_prompt(agent, &prompt);
        }
        self.toast(format!("Anotação enviada a {}", if agents.len() == 1 { "1 agente".to_string() } else { format!("{} agentes", agents.len()) }));
    }
}
