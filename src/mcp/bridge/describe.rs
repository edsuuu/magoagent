use super::*;

pub(super) fn describe(nodes: &[Node], from: &str, shared: &Mutex<Shared>) -> String {
    // the app's reading (its Claude Code session's word over the screen's)
    let busy = |id: &str| {
        let shared = shared.lock().unwrap();
        match (shared.terminals.get(id), shared.states.get(id)) {
            (None, _) => "",
            (Some(t), _) if t.exited() => " (encerrado)",
            (_, Some(AgentState::Waiting)) => " (esperando resposta no terminal)",
            (_, Some(AgentState::Working(_) | AgentState::Active(_))) => " (trabalhando)",
            _ => " (parado)",
        }
    };
    let reachable = reachable(nodes, from);
    let outside = chat::outside(nodes, from);
    let mut out = String::new();
    if let Some(me) = nodes.iter().find(|n| n.id == from) {
        out += &format!("Você: \"{}\"\n", me.name);
    }
    for (kind, title, connected) in [
        (NodeKind::Terminal, "Agentes", " conectados"),
        (NodeKind::Note, "Notas", " conectadas"),
        (NodeKind::Portal, "Navegadores", " conectados"),
        (NodeKind::Tree, "Árvores de arquivos", " conectadas"),
    ] {
        let items: Vec<String> = match kind {
            // a chained note sits under the note it hangs from
            NodeKind::Note => note_tree(nodes, from).into_iter().map(|(depth, n)| format!("{}  - \"{}\"", "  ".repeat(depth), n.name)).collect(),
            _ => reachable
                .iter()
                .filter(|n| n.kind == kind || (kind == NodeKind::Terminal && n.kind == NodeKind::Chat))
                .map(|n| match n.kind {
                    NodeKind::Portal | NodeKind::Tree => format!("  - \"{}\" ({})", n.name, n.text),
                    NodeKind::Chat => format!("  - \"{}\" ({})", n.name, chat::state(shared, &n.id)),
                    _ => format!("  - \"{}\"{}", n.name, busy(&n.id)),
                })
                .collect(),
        };
        if !items.is_empty() {
            out += &format!("{title}{}:\n{}\n", if outside { "" } else { connected }, items.join("\n"));
        }
    }
    if reachable.is_empty() {
        // outside a terminal there is no "this terminal" to tie: the canvas itself has nothing
        out += if outside {
            "Nenhum agente, nota, navegador ou árvore no canvas deste espaço."
        } else {
            "Nada conectado. Ligue este terminal a outro (ou a uma nota) com uma seta, ou use o botão Conectar."
        };
    }
    out.trim_end().to_string()
}

/// Types `prompt` into the agent (`type_prompt`, or raw keys), says so (`delivered`) and waits for
/// its screen to settle. The answer is what it wrote to its `transcript` meanwhile (the screen
/// shows only its last lines), else the screen, and how it ended. A prompt it couldn't type
/// (`TermHandle::ready`) comes back as why.
pub(crate) fn ask(
    target: &TermHandle,
    prompt: &str,
    raw: bool,
    timeout: Duration,
    transcript: Option<PathBuf>,
    delivered: impl FnOnce(),
) -> Result<(Asked, String), String> {
    let mut said = transcript.map(Transcript::from_end);
    let mut reply = |screen: String| {
        said.as_mut()
            .and_then(|t| {
                t.poll();
                t.agent_words()
            })
            .unwrap_or(screen)
    };
    let before = target.screen_text();
    if raw {
        target.write(unescape(prompt));
    } else {
        target.type_prompt(prompt)?;
    }
    delivered();
    let start = Instant::now();
    let (mut last, mut changed_at) = (before, None::<Instant>);
    loop {
        std::thread::sleep(Duration::from_millis(250));
        let screen = target.screen_text();
        if screen != last {
            last = screen;
            changed_at = Some(Instant::now());
        }
        let quiet = changed_at.map_or(start.elapsed(), |t| t.elapsed());
        let settled = if changed_at.is_some() { quiet >= IDLE } else { quiet >= NO_REACTION };
        if target.exited() {
            return Ok((Asked::Exited, format!("{}\n\n(o terminal do agente encerrou)", reply(last))));
        }
        if settled {
            // stopped on a menu: the words alone would hide what it waits for
            if let Err(why) = target.ready() {
                let answer = reply(last.clone());
                let screen = if answer == last { String::new() } else { format!("\n\nA tela:\n{last}") };
                return Ok((Asked::Waiting, format!("{answer}\n\n(o agente {why}){screen}")));
            }
            return Ok((if changed_at.is_some() { Asked::Answered } else { Asked::Silent }, reply(last)));
        }
        if start.elapsed() >= timeout {
            return Ok((Asked::StillWorking, format!("{}\n\n(o agente ainda está trabalhando; leia de novo com check/read_terminal)", reply(last))));
        }
    }
}

/// `\n` Enter, `\t` Tab, `\e` Esc, `\xNN` a byte, `\\` a backslash. A real line break is Enter too:
/// it is what the MCP's `"\n"` arrives as, and a shell on Windows takes only the carriage return.
pub(crate) fn unescape(text: &str) -> Vec<u8> {
    let mut out = Vec::new();
    let mut chars = text.chars().peekable();
    while let Some(c) = chars.next() {
        match c {
            // CR LF is one Enter: the LF that follows is it
            '\r' if chars.peek() == Some(&'\n') => {}
            '\n' => out.push(b'\r'),
            '\\' => match chars.next() {
                Some('n') | Some('r') => out.push(b'\r'),
                Some('t') => out.push(b'\t'),
                Some('e') => out.push(0x1b),
                Some('x') => {
                    let hex: String = chars.by_ref().take(2).collect();
                    out.extend(u8::from_str_radix(&hex, 16).ok());
                }
                Some(other) => out.extend(other.to_string().into_bytes()),
                None => out.push(b'\\'),
            },
            _ => out.extend(c.to_string().into_bytes()),
        }
    }
    out
}

/// The page side of `portal click|fill`: the agent's cursor, labeled with its name, glides to
/// the element the way a person's would, then clicks it or types into it. No selector means the
/// field the page has focused (a vault login, after the agent clicked the password box).
pub(crate) fn cursor_script(agent: &str, selector: &str, fill: Option<&str>) -> String {
    let args = json!([agent, selector, fill]);
    format!(
        r##"(function([who, sel, text]) {{
  const el = sel ? document.querySelector(sel) : (document.activeElement !== document.body ? document.activeElement : null);
  if (!el) return sel ? 'nada encontrado com ' + sel : 'nenhum campo focado na página: clique no campo antes';
  let c = document.getElementById('__mago_cursor');
  if (!c) {{
    c = document.createElement('div');
    c.id = '__mago_cursor';
    c.style.cssText = 'position:fixed;left:0;top:0;z-index:2147483647;pointer-events:none;transition:transform .5s cubic-bezier(.25,.8,.25,1);font:600 12px -apple-system,system-ui,sans-serif';
    // built node by node, styled through the CSSOM: a page with Trusted Types refuses HTML strings,
    // one whose CSP has no 'unsafe-inline' refuses style attributes
    const svg = 'http://www.w3.org/2000/svg';
    const make = (ns, tag, attrs, css) => {{ const n = ns ? document.createElementNS(ns, tag) : document.createElement(tag); for (const k in attrs) n.setAttribute(k, attrs[k]); n.style.cssText = css; return n; }};
    const arrow = make(svg, 'svg', {{ width: '18', height: '22', viewBox: '0 0 18 22' }}, 'filter:drop-shadow(0 1px 2px rgba(0,0,0,.35))');
    arrow.appendChild(make(svg, 'path', {{ d: 'M1 1L1 17L5.5 13L8.5 20L11.5 18.6L8.6 12L15 12Z', fill: '#e8590c', stroke: '#fff', 'stroke-width': '1.5', 'stroke-linejoin': 'round' }}, ''));
    c.append(arrow, make(null, 'span', {{}}, 'position:absolute;left:15px;top:19px;padding:3px 8px;border-radius:999px;background:#e8590c;color:#fff;white-space:nowrap;box-shadow:0 1px 3px rgba(0,0,0,.3)'));
    c.style.transform = 'translate(' + innerWidth / 2 + 'px,' + innerHeight / 2 + 'px)';
    document.documentElement.appendChild(c);
  }}
  c.querySelector('span').textContent = who;
  el.scrollIntoView({{ block: 'center', inline: 'center' }});
  const r = el.getBoundingClientRect();
  requestAnimationFrame(() => {{ c.style.transform = 'translate(' + (r.left + r.width / 2) + 'px,' + (r.top + r.height / 2) + 'px)'; }});
  setTimeout(() => {{
    if (text === null) return el.click();
    el.focus();
    // the native setter, so pages built with React see the change too
    const value = Object.getOwnPropertyDescriptor(Object.getPrototypeOf(el), 'value');
    value && value.set ? value.set.call(el, text) : (el.value = text);
    el.dispatchEvent(new Event('input', {{ bubbles: true }}));
    el.dispatchEvent(new Event('change', {{ bubbles: true }}));
  }}, 550);
  return (text === null ? 'clicando em ' : 'preenchendo ') + sel;
}})({args})"##
    )
}
