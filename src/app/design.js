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
  const size = () => {
    layer.width = innerWidth * devicePixelRatio;
    layer.height = innerHeight * devicePixelRatio;
    pen.setTransform(devicePixelRatio, 0, 0, devicePixelRatio, 0, 0);
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
  let stroke = null;
  const nothing = 'Não há nada para selecionar aí. Desenhe ao redor da parte que você quer.';

  const badge = (n, size = 22) => h('span', `position:absolute;left:${-size / 2 + 1}px;top:${-size / 2 + 1}px;width:${size}px;height:${size}px;border-radius:50%;background:${ACCENT};color:#fff;font:700 ${n > 9 ? 9 : 11}px -apple-system,system-ui,sans-serif;display:flex;align-items:center;justify-content:center;box-shadow:inset 0 0 0 2px #fff,0 1px 2px rgba(0,0,0,.3)`, String(n));
  const outline = ([x, y, w, h2], n, active) => {
    const box = css(document.createElement('div'), `position:absolute;left:${x}px;top:${y}px;width:${w}px;height:${h2}px;box-sizing:border-box;border:2px solid ${ACCENT};border-radius:4px;background:${active ? 'rgba(105,101,219,.08)' : 'transparent'}`);
    if (n) box.append(css(badge(n, 24), 'position:absolute;left:-12px;top:-12px'));
    return box;
  };
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
    annotations.forEach((a, i) => marks.append(outline(a.rect, i + 1, false)));
    if (region) marks.append(outline(region.rect, null, true));
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
    region = null; popover = null; draft = '';
    render();
  };
  const closePopover = () => {
    if (region) post('cancel');
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
    stroke = [at(e)];
    pen.clearRect(0, 0, innerWidth, innerHeight);
    try { layer.setPointerCapture(e.pointerId); } catch (_) { /* a synthetic pointer has no capture */ }
  };
  layer.onpointermove = e => {
    if (!stroke) return;
    const [x, y] = stroke[stroke.length - 1];
    if (stroke.length < 2000) stroke.push(at(e));
    pen.strokeStyle = ACCENT; pen.lineWidth = 3; pen.lineCap = 'round'; pen.lineJoin = 'round';
    pen.beginPath(); pen.moveTo(x - scrollX, y - scrollY); pen.lineTo(e.clientX, e.clientY); pen.stroke();
  };
  layer.onpointerup = () => {
    if (!stroke) return;
    const points = stroke;
    stroke = null;
    pen.clearRect(0, 0, innerWidth, innerHeight);
    post('region', { points, view: [innerWidth, innerHeight], scroll: [scrollX, scrollY] });
  };

  window.__magoDesign = {
    stop: () => exit(false),
    drain,
    region: rect => {
      region = { rect, preview: undefined };
      popover = { mode: 'selection' };
      error = null;
      render();
    },
    preview: src => { if (region) { region.preview = src; render(); } },
    error: () => {
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
      else { region = null; popover = { mode: 'panel' }; post('cancel'); }
      render();
    },
  };
  if (cfg.panel) popover = { mode: 'panel' };
  render();
  return 'design ligado';
})()
