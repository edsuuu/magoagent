(() => {
  // the page side of Design: a layer to draw the stroke, the outline of the region it made, the
  // numbered annotations and the composer. The app answers through `window.__magoDesign`.
  if (window.__magoDesign) window.__magoDesign.stop();
  const cfg = __CONFIG__;
  const ACCENT = '#6965db';
  const MAC = navigator.platform.startsWith('Mac');
  const css = (el, s) => (el.style.cssText += s, el);
  const h = (tag, style, ...kids) => {
    const node = css(document.createElement(tag), style);
    for (const k of kids) if (k != null) node.append(k);
    return node;
  };
  // a page whose messages wry would drop (file:// on Windows) keeps them for the app to fetch
  const outbox = [];
  const drain = () => outbox.splice(0);
  const post = (op, data) => {
    const message = JSON.stringify({ nonce: cfg.nonce, op, ...data });
    if (cfg.queue) outbox.push(message);
    else window.ipc.postMessage(message);
  };
  const layer = css(document.createElement('canvas'), 'position:fixed;inset:0;width:100%;height:100%;z-index:2147483646;cursor:crosshair');
  const root = css(document.createElement('div'), 'position:fixed;inset:0;z-index:2147483647;pointer-events:none;font:12px -apple-system,system-ui,sans-serif;color:#e5e5ea');
  const pen = layer.getContext('2d');
  // the stroke's glow so far, stamped once per bit of the stroke and laid under its core each move
  const body = document.createElement('canvas');
  const glowPen = body.getContext('2d');
  const size = () => {
    for (const [c, g] of [[layer, pen], [body, glowPen]]) {
      c.width = innerWidth * devicePixelRatio;
      c.height = innerHeight * devicePixelRatio;
      g.setTransform(devicePixelRatio, 0, 0, devicePixelRatio, 0, 0);
    }
  };
  size();
  addEventListener('resize', size);
  // the outlines sit on the page (absolute, in its px), so they scroll with what they circle
  const marks = css(document.createElement('div'), 'position:absolute;left:0;top:0;width:0;height:0;overflow:visible;z-index:2147483647;pointer-events:none');
  document.documentElement.append(layer, marks, root);

  let annotations = cfg.annotations;
  let region = null;        // { rect: [x, y, w, h], preview: undefined (capturing) | null | dataURL }
  let popover = null;       // null | { mode: 'selection' | 'panel' }
  let error = null;
  let draft = '';
  let agent = cfg.agents.length ? cfg.agents[0].id : '';
  let stroke = null;        // the stroke being drawn, in the page's px
  let held = null;          // the last stroke, until its region comes back
  let light = null;         // how far the pen went, and the carry to the next stamp
  let morph = null;         // the stroke's light turning into the region's outline
  const nothing = 'Não há nada para selecionar aí. Desenhe ao redor da parte que você quer.';

  // the light, as the reference app's: blue, violet, pink and orange along 1600 px of stroke, a
  // gaussian glow stamped every 5 px under a white core, a bloom at the pen; on release it turns
  // into the region's rounded outline in 400 ms (ease out, the pen's end first) as the page dims
  const HUES = [[74, 153, 255], [168, 107, 255], [255, 107, 173], [255, 153, 66]];
  const STOPS = [0, 0.34, 0.67, 1];
  const SPAN = 1600, STEP = 5, SIGMA = 13, SETTLED = 6.5, CORE = 7, CORE_SETTLED = 5, MORPH = 400, RADIUS = 12;
  const WHITE = '255,255,255';
  const clamp = v => Math.min(1, Math.max(0, v));
  const smooth = (v, a, b) => { const x = clamp((v - a) / (b - a)); return x * x * (3 - 2 * x); };
  // the color at a share of the palette, in 64 steps, so the glows are a few cached pictures
  const tint = u => {
    u = Math.round(clamp(u) * 64) / 64;
    const i = Math.max(1, STOPS.findIndex(s => s >= u));
    const f = (u - STOPS[i - 1]) / (STOPS[i] - STOPS[i - 1]);
    return HUES[i - 1].map((c, k) => Math.round(c + (HUES[i][k] - c) * f)).join(',');
  };
  const sprites = new Map();
  const glow = (g, [x, y], rgb, sigma, alpha) => {
    if (!(alpha > 0.002)) return;
    let s = sprites.get(rgb);
    if (!s) {
      s = Object.assign(document.createElement('canvas'), { width: 64, height: 64 });
      const c = s.getContext('2d'), fill = c.createRadialGradient(32, 32, 0, 32, 32, 32);
      // a gaussian, out to 2.5 sigma
      for (const [at, a] of [[0, 1], [0.2, 0.88], [0.4, 0.61], [0.6, 0.32], [0.8, 0.14], [1, 0]]) fill.addColorStop(at, `rgba(${rgb},${a})`);
      c.fillStyle = fill;
      c.fillRect(0, 0, 64, 64);
      sprites.set(rgb, s);
    }
    const r = sigma * 2.5;
    g.globalAlpha = Math.min(1, alpha);
    g.drawImage(s, x - r, y - r, 2 * r, 2 * r);
    g.globalAlpha = 1;
  };
  // each stamp's alpha, so the overlapping ones cover 60%
  const stampAlpha = (sigma, spacing) => 1 - Math.pow(0.4, spacing / (sigma * 2.5066));
  const core = (g, path, width, alpha) => {
    g.lineCap = g.lineJoin = 'round';
    for (let d = 0; d <= 1; d += 0.25) {
      g.strokeStyle = `rgba(${WHITE},${(0.05 + 0.25 * d * d) * alpha})`;
      g.lineWidth = width * (1 - 0.78 * d);
      g.stroke(path);
    }
  };
  const bloom = (g, p, rgb, alpha) => { glow(g, p, rgb, 24, 0.3 * alpha); glow(g, p, WHITE, 11, 0.5 * alpha); };
  const poly = (pts, closed) => {
    const p = new Path2D();
    pts.forEach(([x, y], i) => (i ? p.lineTo(x, y) : p.moveTo(x, y)));
    if (closed) p.closePath();
    return p;
  };
  const span = (a, b) => Math.hypot(b[0] - a[0], b[1] - a[1]);
  const loopLength = pts => pts.reduce((sum, p, i) => sum + span(p, pts[(i + 1) % pts.length]), 0);
  // n points evenly along a closed line, and how far along each one is
  const resample = (loop, n) => {
    const total = loopLength(loop), pts = [], at = [];
    let i = 0, walked = 0;
    for (let k = 0; k < n; k++) {
      const want = k * total / n;
      while (i < loop.length - 1 && walked + span(loop[i], loop[i + 1]) < want) walked += span(loop[i], loop[++i]);
      const [a, b] = [loop[i], loop[(i + 1) % loop.length]], len = span(a, b), f = len ? (want - walked) / len : 0;
      pts.push([a[0] + (b[0] - a[0]) * f, a[1] + (b[1] - a[1]) * f]);
      at.push(want);
    }
    return { pts, at };
  };
  const area = pts => pts.reduce((sum, p, i) => sum + p[0] * pts[(i + 1) % pts.length][1] - pts[(i + 1) % pts.length][0] * p[1], 0);
  const view = ([x, y]) => [x - scrollX, y - scrollY];

  // the live stroke: the glow so far, its core and the bloom at the pen
  const paintStroke = () => {
    const pts = stroke.map(view);
    pen.clearRect(0, 0, innerWidth, innerHeight);
    pen.drawImage(body, 0, 0, innerWidth, innerHeight);
    const line = new Path2D();
    line.moveTo(...pts[0]);
    for (let i = 1; i < pts.length - 1; i++) line.quadraticCurveTo(...pts[i], (pts[i][0] + pts[i + 1][0]) / 2, (pts[i][1] + pts[i + 1][1]) / 2);
    line.lineTo(...pts[pts.length - 1]);
    core(pen, line, CORE, 1);
    bloom(pen, pts[pts.length - 1], tint(light.drawn / SPAN), 1);
  };

  // the stroke (closed back to its start) and the region's outline as n matching points, the
  // outline turned so each point has the least way to go
  const startMorph = ([x, y, w, ht]) => {
    const r = Math.min(RADIUS, w / 2, ht / 2), outline = [];
    for (const [cx, cy, a] of [[x + w - r, y + r, -0.5], [x + w - r, y + ht - r, 0], [x + r, y + ht - r, 0.5], [x + r, y + r, 1]]) {
      for (let s = 0; s <= 8; s++) outline.push([cx + r * Math.cos((a + s / 16) * Math.PI), cy + r * Math.sin((a + s / 16) * Math.PI)]);
    }
    const pts = held && held.length > 1 ? held.map(view) : outline;
    const drawn = loopLength(pts) - span(pts[pts.length - 1], pts[0]);
    const n = Math.min(400, Math.max(48, Math.round(Math.max(loopLength(pts), loopLength(outline)) / STEP)));
    const from = resample(pts, n);
    const to = resample(outline, n).pts;
    if (area(from.pts) < 0 !== area(to) < 0) to.reverse();
    let best = 0, low = Infinity;
    for (let k = 0; k < n; k++) {
      let cost = 0;
      for (let i = 0; i < n && cost < low; i++) cost += span(to[(i + k) % n], from.pts[i]) ** 2;
      if (cost < low) [low, best] = [cost, k];
    }
    const lit = from.at.map(d => d <= drawn + 0.001);
    const head = Math.max(0, lit.lastIndexOf(true));
    const m = morph = {
      from: from.pts, to: to.map((_, i) => to[(i + best) % n]), lit, head,
      hue: from.at.map(d => tint(d / SPAN)),
      phase: from.pts.map((_, i) => Math.min(1, Math.min((head - i + n) % n, (i - head + n) % n) / (n / 2))),
      start: performance.now(), scroll: [scrollX, scrollY],
    };
    held = null;
    const step = now => {
      if (morph !== m) return;
      const t = clamp((now - m.start) / MORPH);
      pen.clearRect(0, 0, innerWidth, innerHeight);
      paintMorph(pen, t, true);
      if (t < 1) return requestAnimationFrame(step);
      // landed: the outline moves into the page, where it scrolls with what it circles
      if (region) region.ring = settled();
      pen.clearRect(0, 0, innerWidth, innerHeight);
      morph = null;
      render();
    };
    requestAnimationFrame(step);
  };
  const paintMorph = (g, t, veil) => {
    const m = morph, n = m.from.length;
    const pts = m.from.map((p, i) => {
      const e = 1 - (1 - clamp((t - 0.18 * m.phase[i]) / 0.82)) ** 4;
      return [p[0] + (m.to[i][0] - p[0]) * e, p[1] + (m.to[i][1] - p[1]) * e];
    });
    const settle = smooth(t, 0.1, 0.85), closing = smooth(t, 0, 0.45);
    if (veil) {
      const dim = new Path2D();
      dim.rect(0, 0, innerWidth, innerHeight);
      dim.addPath(poly(pts, true));
      g.fillStyle = `rgba(0,0,0,${0.5 * smooth(t, 0.25, 0.9)})`;
      g.fill(dim, 'evenodd');
    }
    const sigma = SIGMA + (SETTLED - SIGMA) * settle;
    const alpha = stampAlpha(sigma, loopLength(pts) / n);
    pts.forEach((p, i) => glow(g, p, m.hue[i], sigma, alpha * (m.lit[i] ? 1 : closing)));
    const width = CORE + (CORE_SETTLED - CORE) * settle;
    core(g, poly(pts.slice(0, m.head + 1)), width, 1);
    core(g, poly([...pts.slice(m.head), pts[0]]), width, closing);
    bloom(g, pts[m.head], m.hue[m.head], 1 - smooth(t, 0, 0.35));
  };
  // the landed outline, drawn once into a canvas on the page
  const settled = () => {
    const [x, y, w, ht] = region.rect, pad = 20, r = devicePixelRatio;
    const c = Object.assign(document.createElement('canvas'), { width: Math.ceil((w + 2 * pad) * r), height: Math.ceil((ht + 2 * pad) * r) });
    css(c, `position:absolute;left:${x - pad}px;top:${y - pad}px;width:${c.width / r}px;height:${c.height / r}px`);
    const g = c.getContext('2d');
    g.setTransform(r, 0, 0, r, (pad - x + morph.scroll[0]) * r, (pad - y + morph.scroll[1]) * r);
    paintMorph(g, 1, false);
    return c;
  };
  const stopLight = () => {
    morph = null;
    held = null;
    pen.clearRect(0, 0, innerWidth, innerHeight);
  };

  const badge = (n, size = 22) => h('span', `position:absolute;left:${-size / 2 + 1}px;top:${-size / 2 + 1}px;width:${size}px;height:${size}px;border-radius:50%;background:${ACCENT};color:#fff;font:700 ${size * (n > 9 ? 0.4 : 0.5)}px -apple-system,system-ui,sans-serif;display:flex;align-items:center;justify-content:center;box-shadow:inset 0 0 0 2px #fff,0 1px 2px rgba(0,0,0,.3)`, String(n));
  // a kept annotation: its region ringed in the light's colors (there and back around), its number
  const RING = `conic-gradient(${[0, 1, 2, 3, 2, 1, 0].map((k, i) => `rgb(${HUES[k]}) ${(i * 100 / 6).toFixed(1)}%`).join()})`;
  const outline = ([x, y, w, ht], n) => h('div', `position:absolute;left:${x}px;top:${y}px;width:${w}px;height:${ht}px`,
    h('div', `position:absolute;inset:0;border-radius:${RADIUS}px;padding:2px;background:${RING};-webkit-mask:linear-gradient(#000 0 0) content-box,linear-gradient(#000 0 0);-webkit-mask-composite:xor;mask-composite:exclude`),
    css(badge(n, 24), 'left:-12px;top:-12px'));
  // the region drawn: the page dimmed around it and its landed outline
  const dimmed = ([x, y, w, ht]) => h('div', `position:absolute;left:${x}px;top:${y}px;width:${w}px;height:${ht}px;border-radius:${Math.min(RADIUS, w / 2, ht / 2)}px;box-shadow:0 0 0 200vmax rgba(0,0,0,.5)`);
  const button = (label, onclick, primary, disabled) => Object.assign(css(h('button', '', label), `padding:6px 12px;border:0;border-radius:8px;font:600 12px -apple-system,system-ui,sans-serif;cursor:${disabled ? 'default' : 'pointer'};color:#fff;background:${primary ? ACCENT : 'rgba(255,255,255,.1)'};opacity:${disabled ? 0.45 : 1}`), { onclick: disabled ? null : onclick });

  const strip = () => h('ul', 'list-style:none;margin:0;padding:6px 6px 0 6px;display:flex;gap:10px;overflow-x:auto;scrollbar-width:none',
    ...annotations.map((a, i) => h('li', 'flex:0 0 112px;display:flex;flex-direction:column;gap:6px',
      h('div', 'position:relative;height:80px;width:112px',
        a.thumb ? css(Object.assign(document.createElement('img'), { src: a.thumb, alt: '' }), 'width:112px;height:80px;object-fit:contain;border-radius:6px;background:rgba(255,255,255,.04)') : h('div', 'height:80px;border-radius:6px;background:rgba(255,255,255,.04)'),
        css(badge(i + 1, 22), 'left:-5px;top:-5px'),
        css(Object.assign(h('button', '', '×'), { title: `Remover anotação ${i + 1}`, onclick: () => post('drop', { index: i }) }), 'position:absolute;right:-5px;top:-5px;width:16px;height:16px;border:0;border-radius:50%;background:rgba(0,0,0,.55);color:#fff;font:700 10px sans-serif;cursor:pointer;padding:0')),
      h('p', `margin:0;min-height:28px;font-size:11px;line-height:14px;overflow:hidden;display:-webkit-box;-webkit-line-clamp:2;-webkit-box-orient:vertical;${a.note ? 'font-weight:500;color:#fff' : 'color:#8a8a93'}`, a.note || 'Sem nota'))));

  const render = () => {
    root.replaceChildren();
    marks.replaceChildren();
    // the region first: the kept ones stay bright over its dimming
    if (region && region.ring) marks.append(dimmed(region.rect), region.ring);
    annotations.forEach((a, i) => marks.append(outline(a.rect, i + 1)));
    if (error) root.append(h('div', 'position:fixed;left:50%;top:12px;transform:translateX(-50%);padding:6px 12px;border-radius:999px;background:#232329;color:#fff;white-space:nowrap', error));
    if (popover) {
      const box = renderPopover();
      root.append(box);
      if (popover.mode === 'selection') placeBelow(box);
    }
  };

  // under the region when the composer fits there, above it when it doesn't, and inside the page otherwise
  const placeBelow = box => {
    const [px, py, , ht] = region.rect;
    const [x, y] = [px - scrollX, py - scrollY];
    const hgt = box.offsetHeight;
    let top = y + ht + 10;
    if (top + hgt > innerHeight - 8) top = y - hgt - 10;
    if (top < 8) top = Math.max(8, innerHeight - hgt - 8);
    box.style.top = `${top}px`;
    box.style.left = `${Math.max(8, Math.min(x, innerWidth - 352))}px`;
  };

  const renderPopover = () => {
    const panel = popover.mode === 'panel';
    const place = panel ? 'top:12px;right:12px' : 'top:8px;left:8px';
    const box = css(h('div', '', h('div', 'font-weight:600;font-size:13px;color:#fff', panel ? 'Perguntar sobre estas anotações' : 'Perguntar sobre esta seleção')), `position:fixed;${place};width:340px;max-height:calc(100vh - 24px);overflow:auto;box-sizing:border-box;padding:12px;border-radius:12px;background:#232329;box-shadow:0 6px 20px rgba(0,0,0,.4);pointer-events:auto;display:flex;flex-direction:column;gap:8px`);
    if (!panel && region) {
      if (region.preview === undefined) box.append(h('div', 'color:#8a8a93', 'Capturando a região'));
      else if (region.preview) box.append(css(Object.assign(document.createElement('img'), { src: region.preview, alt: 'Prévia da região capturada' }), 'max-height:160px;width:100%;object-fit:contain;border-radius:6px;background:rgba(255,255,255,.04)'));
    }
    if (annotations.length) {
      box.append(h('div', 'display:flex;align-items:center;justify-content:space-between', h('span', 'color:#8a8a93;font-size:11px', 'Anotações'), css(Object.assign(h('button', '', 'Remover tudo'), { onclick: () => post('clear') }), 'border:0;background:none;color:#8a8a93;font:500 11px -apple-system,system-ui,sans-serif;cursor:pointer;padding:0')));
      box.append(strip());
    }
    const text = css(document.createElement('textarea'), 'width:100%;height:64px;box-sizing:border-box;border:0;border-radius:8px;padding:8px;background:#31303c;color:#fff;resize:none;font:inherit');
    text.placeholder = 'Adicionar um comentário…';
    text.maxLength = 4000; // PAGE_TEXT in design.rs
    text.value = draft;
    text.oninput = () => (draft = text.value);
    // over a region it keeps the annotation, as the hint says; in the panel it sends
    text.onkeydown = e => { if (e.key === 'Enter' && (e.metaKey || e.ctrlKey)) { e.preventDefault(); if (!panel && region) keep(); else send(); } };
    box.append(text);
    if (cfg.agents.length) {
      const pick = css(document.createElement('select'), 'width:100%;box-sizing:border-box;padding:6px;border:0;border-radius:8px;background:#31303c;color:#fff;font:inherit');
      pick.setAttribute('aria-label', 'Selecionar agente');
      for (const a of cfg.agents) pick.append(Object.assign(document.createElement('option'), { value: a.id, textContent: a.name, selected: a.id === agent }));
      pick.onchange = () => (agent = pick.value);
      box.append(pick);
    } else {
      box.append(h('div', 'color:#8a8a93', 'Nenhum agente conectado.'));
    }
    if (!panel && annotations.length) box.append(h('div', 'color:#8a8a93;font-size:11px', 'O envio inclui todas as anotações guardadas.'));
    const row = h('div', 'display:flex;gap:8px;justify-content:flex-end;align-items:center');
    if (!panel && region) row.append(Object.assign(button('Adicionar às anotações', keep, false, capturing()), { title: `Mantenha esta anotação e desenhe outra; depois envie todas juntas (${MAC ? '⌘Return' : 'Ctrl+Enter'})` }));
    row.append(button('Enviar', send, true, !cfg.agents.length || capturing()));
    box.append(row);
    return box;
  };

  // while the region's picture is on its way nothing leaves (the shortcut either): it would go without it
  const capturing = () => region != null && region.preview === undefined;
  const send = () => {
    if (!cfg.agents.length || capturing()) return;
    post('send', { comment: draft.trim(), agent });
    exit(false);
  };
  const keep = () => {
    if (!region || capturing()) return;
    post('keep', { comment: draft.trim() });
    stopLight();
    region = null; popover = null; draft = '';
    render();
  };
  const closePopover = () => {
    if (region) post('cancel');
    stopLight();
    region = null; popover = null; draft = '';
    render();
  };

  const key = e => {
    if (e.key !== 'Escape') return;
    e.preventDefault();
    if (popover) closePopover();
    else exit(true);
  };
  const exit = report => {
    morph = null;
    layer.remove(); marks.remove(); root.remove();
    removeEventListener('keydown', key, true);
    removeEventListener('resize', size);
    // a fetched page keeps its outbox reachable, so the app still gets the exit
    window.__magoDesign = cfg.queue ? { stop() {}, drain } : null;
    if (report) post('exit');
  };
  addEventListener('keydown', key, true);

  // the stroke in the page's px (the scroll added), as the region and its outline
  const at = e => [e.clientX + scrollX, e.clientY + scrollY];
  layer.onpointerdown = e => {
    if (popover) return closePopover();
    stopLight();
    stroke = [at(e)];
    light = { drawn: 0, carry: 0 };
    glowPen.clearRect(0, 0, innerWidth, innerHeight);
    try { layer.setPointerCapture(e.pointerId); } catch (_) { /* a synthetic pointer has no capture */ }
  };
  layer.onpointermove = e => {
    if (!stroke || stroke.length >= 2000) return;
    const [a, b] = [view(stroke[stroke.length - 1]), [e.clientX, e.clientY]];
    stroke.push(at(e));
    // a stamp every STEP px of the way, in the color of how far the pen went
    const len = span(a, b);
    for (; light.carry <= len; light.carry += STEP) {
      const f = len ? light.carry / len : 0;
      glow(glowPen, [a[0] + (b[0] - a[0]) * f, a[1] + (b[1] - a[1]) * f], tint((light.drawn + light.carry) / SPAN), SIGMA, stampAlpha(SIGMA, STEP));
    }
    light.carry -= len;
    light.drawn += len;
    paintStroke();
  };
  layer.onpointerup = () => {
    if (!stroke) return;
    const points = stroke;
    // the light stays until the region comes back and it turns into its outline
    held = stroke;
    stroke = null;
    post('region', { points, view: [innerWidth, innerHeight], scroll: [scrollX, scrollY] });
  };

  window.__magoDesign = {
    stop: () => exit(false),
    drain,
    region: rect => {
      region = { rect, preview: undefined };
      popover = { mode: 'selection' };
      error = null;
      startMorph(view(rect).concat(rect.slice(2)));
      render();
    },
    preview: src => { if (region) { region.preview = src; render(); } },
    error: () => {
      stopLight();
      error = nothing;
      render();
      setTimeout(() => { error = null; render(); }, 3500);
    },
    sync: list => {
      annotations = list;
      if (!list.length && popover && popover.mode === 'panel') popover = null;
      render();
    },
    panel: () => {
      if (popover && popover.mode === 'panel') popover = null;
      else { stopLight(); region = null; popover = { mode: 'panel' }; post('cancel'); }
      render();
    },
  };
  if (cfg.panel) popover = { mode: 'panel' };
  render();
  return 'design ligado';
})()
