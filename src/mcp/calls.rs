//! MCP tool list and dispatch.


use super::*;

pub(crate) fn tools() -> Value {
    let elements = json!({ "type": "array", "items": { "type": "object" } });
    let ids = json!({ "type": "array", "items": { "type": "string" } });
    let agent_name = json!({ "type": "string", "description": "o nome do agente, como list_agents mostra" });
    json!([
        {
            "name": "get_scene",
            "description": "Lista os elementos do desenho atual (id, tipo, posição, tamanho, cores, texto, vínculos de setas) e a área ocupada.",
            "inputSchema": { "type": "object", "properties": {} },
        },
        {
            "name": "add_elements",
            "description": "Adiciona elementos (objetos JSON com type, x, y, width, height, cores, text, points). Extras: `label: {text, fontSize?}` em formas e setas; \
                `start: {id}` / `end: {id}` em setas para ligá-las a formas (os pontos são calculados sozinhos). \
                Devolve os ids criados.",
            "inputSchema": { "type": "object", "properties": { "elements": elements }, "required": ["elements"] },
        },
        {
            "name": "update_elements",
            "description": "Altera elementos existentes: cada item tem `id` mais os campos a trocar (x, y, width, height, text, \
                strokeColor, backgroundColor, angle, start/end...). Setas ligadas acompanham as formas movidas.",
            "inputSchema": { "type": "object", "properties": { "elements": elements }, "required": ["elements"] },
        },
        {
            "name": "delete_elements",
            "description": "Apaga elementos pelos ids.",
            "inputSchema": { "type": "object", "properties": { "ids": ids }, "required": ["ids"] },
        },
        {
            "name": "clear_canvas",
            "description": "Apaga todo o desenho.",
            "inputSchema": { "type": "object", "properties": {} },
        },
        {
            "name": "view",
            "description": "Renderiza o desenho (ou só os ids informados) como PNG para você ver como ficou.",
            "inputSchema": { "type": "object", "properties": {
                "ids": ids,
                "scale": { "type": "number", "description": "1 a 3 (padrão 1)" },
            } },
        },
        {
            "name": "save_file",
            "description": "Salva o desenho em um arquivo: .mago (formato do MagoAgent), .png ou .svg.",
            "inputSchema": { "type": "object", "properties": { "path": { "type": "string" } }, "required": ["path"] },
        },
        {
            "name": "open_file",
            "description": "Substitui o desenho atual pelo conteúdo de um arquivo .mago.",
            "inputSchema": { "type": "object", "properties": { "path": { "type": "string" } }, "required": ["path"] },
        },
        {
            "name": "set_background",
            "description": "Muda a cor de fundo da tela (hex, ex.: #ffffff).",
            "inputSchema": { "type": "object", "properties": { "color": { "type": "string" } }, "required": ["color"] },
        },
        {
            "name": "show_app",
            "description": "Abre o app MagoAgent para o usuário ver o desenho ao vivo.",
            "inputSchema": { "type": "object", "properties": {} },
        },
        {
            "name": "list_agents",
            "description": "Lista os agentes (terminais do canvas), notas e portais ligados a você por uma seta, com o estado \
                de cada agente (trabalhando, parado, encerrado). Fora de um terminal do MagoAgent, lista todos.",
            "inputSchema": { "type": "object", "properties": {} },
        },
        {
            "name": "ask_agent",
            "description": "Manda um prompt a um agente conectado (outro terminal do canvas), espera ele terminar e devolve a \
                tela dele. Se o tempo acabar antes, acompanhe com read_terminal; não reenvie o prompt. `raw: true` envia \
                teclas cruas (\\n Enter, \\t Tab, \\e Esc, \\x03 Ctrl+C) para menus e comandos de shell.",
            "inputSchema": { "type": "object", "properties": {
                "name": agent_name,
                "prompt": { "type": "string" },
                "raw": { "type": "boolean" },
                "timeout": { "type": "number", "description": "segundos de espera (padrão 50)" },
            }, "required": ["name", "prompt"] },
        },
        {
            "name": "read_terminal",
            "description": "Lê a tela atual de um agente conectado, sem mandar nada.",
            "inputSchema": { "type": "object", "properties": { "name": agent_name }, "required": ["name"] },
        },
        {
            "name": "read_note",
            "description": "Lê uma nota adesiva conectada (markdown). O nome de uma nota é a primeira linha dela.",
            "inputSchema": { "type": "object", "properties": { "name": { "type": "string" } }, "required": ["name"] },
        },
        {
            "name": "write_note",
            "description": "Troca o texto de uma nota conectada. Mudar a primeira linha muda o nome da nota.",
            "inputSchema": { "type": "object", "properties": {
                "name": { "type": "string" },
                "text": { "type": "string" },
            }, "required": ["name", "text"] },
        },
        {
            "name": "portal_navigate",
            "description": "Abre um endereço num portal (página web do canvas) conectado a você.",
            "inputSchema": { "type": "object", "properties": {
                "name": { "type": "string", "description": "o nome do portal, como list_agents mostra" },
                "url": { "type": "string" },
            }, "required": ["name", "url"] },
        },
        {
            "name": "portal_read",
            "description": "Lê o texto visível da página de um portal conectado.",
            "inputSchema": { "type": "object", "properties": { "name": { "type": "string" } }, "required": ["name"] },
        },
        {
            "name": "portal_click",
            "description": "Clica num elemento da página de um portal conectado. Seu cursor, com seu nome, aparece na \
                página e desliza até o elemento antes do clique, para quem estiver olhando acompanhar.",
            "inputSchema": { "type": "object", "properties": {
                "name": { "type": "string" },
                "selector": { "type": "string", "description": "seletor CSS do elemento, ex.: button[type=submit]" },
            }, "required": ["name", "selector"] },
        },
        {
            "name": "portal_fill",
            "description": "Digita um texto num campo da página de um portal conectado, com seu cursor indo até ele.",
            "inputSchema": { "type": "object", "properties": {
                "name": { "type": "string" },
                "selector": { "type": "string" },
                "text": { "type": "string" },
            }, "required": ["name", "selector", "text"] },
        },
        {
            "name": "portal_eval",
            "description": "Roda JavaScript na página de um portal conectado e devolve o resultado (JSON). Útil para \
                clicar (document.querySelector(...).click()), preencher campos ou ler partes da página.",
            "inputSchema": { "type": "object", "properties": {
                "name": { "type": "string" },
                "script": { "type": "string" },
            }, "required": ["name", "script"] },
        },
        {
            "name": "create_note",
            "description": "Cria uma nota adesiva ligada ao seu terminal (só de dentro de um terminal do MagoAgent).",
            "inputSchema": { "type": "object", "properties": { "text": { "type": "string" } }, "required": ["text"] },
        },
    ])
}

pub(crate) fn call(name: &str, args: &Value) -> Value {
    let outcome = match name {
        "get_scene" => with_scene(false, |scene| Ok(text(describe(scene)))),
        "add_elements" => with_scene(true, |scene| {
            let added = add_elements(scene, list(args, "elements")?)?;
            Ok(text(json!({ "created": added }).to_string()))
        }),
        "update_elements" => with_scene(true, |scene| {
            let updated = update_elements(scene, list(args, "elements")?)?;
            Ok(text(json!({ "updated": updated }).to_string()))
        }),
        "delete_elements" => with_scene(true, |scene| {
            let ids: Vec<&str> = list(args, "ids")?.iter().filter_map(Value::as_str).collect();
            let before = scene.elements.len();
            scene.elements.retain(|e| !ids.contains(&e.id.as_str()));
            Ok(text(format!("{} elemento(s) apagado(s)", before - scene.elements.len())))
        }),
        "clear_canvas" => with_scene(true, |scene| {
            scene.elements.clear();
            Ok(text("Tela limpa".into()))
        }),
        "view" => with_scene(false, |scene| view(scene, args)),
        "save_file" => with_scene(false, |scene| save_file(scene, args["path"].as_str().unwrap_or_default())),
        "open_file" => with_scene(true, |scene| {
            let path = args["path"].as_str().unwrap_or_default();
            *scene = fs::read_to_string(path).map_err(|e| e.to_string()).and_then(|d| parse_scene(&d))?;
            Ok(text(format!("{} elemento(s) carregado(s) de {path}", scene.elements.len())))
        }),
        "set_background" => with_scene(true, |scene| {
            scene.background = parse_hex(&args["color"]).ok_or("cor inválida, use hex como #f8f9fa")?;
            Ok(text("Fundo alterado".into()))
        }),
        "show_app" => show_app(),
        "list_agents" => agent(json!({ "cmd": "list" }), 15.0),
        "ask_agent" => {
            let wait = args["timeout"].as_f64().unwrap_or(50.0);
            let ask = json!({ "cmd": "ask", "name": args["name"], "text": args["prompt"], "raw": args["raw"], "timeout": wait });
            agent(ask, wait)
        }
        "read_terminal" => agent(json!({ "cmd": "check", "name": args["name"] }), 15.0),
        "read_note" => agent(json!({ "cmd": "note_read", "name": args["name"] }), 15.0),
        "write_note" => agent(json!({ "cmd": "note_write", "name": args["name"], "text": args["text"] }), 15.0),
        "create_note" => agent(json!({ "cmd": "note_create", "text": args["text"] }), 15.0),
        "portal_navigate" => agent(json!({ "cmd": "portal_navigate", "name": args["name"], "text": args["url"] }), 15.0),
        "portal_read" => agent(json!({ "cmd": "portal_text", "name": args["name"] }), 30.0),
        "portal_eval" => agent(json!({ "cmd": "portal_eval", "name": args["name"], "text": args["script"] }), 30.0),
        "portal_click" => agent(json!({ "cmd": "portal_click", "name": args["name"], "selector": args["selector"] }), 30.0),
        "portal_fill" => agent(json!({ "cmd": "portal_fill", "name": args["name"], "selector": args["selector"], "text": args["text"] }), 30.0),
        other => Err(format!("ferramenta desconhecida: {other}")),
    };
    match outcome {
        Ok(content) => json!({ "content": content }),
        Err(message) => json!({ "content": text(message), "isError": true }),
    }
}

/// Agent tools go to the running app through the bridge.
fn agent(command: Value, wait_secs: f64) -> Result<Value, String> {
    request(command, std::time::Duration::from_secs_f64(wait_secs.clamp(1.0, 3600.0))).map(text)
}

pub(crate) fn text(message: String) -> Value {
    json!([{ "type": "text", "text": message }])
}

pub(crate) fn list<'a>(args: &'a Value, key: &str) -> Result<&'a Vec<Value>, String> {
    args[key].as_array().ok_or_else(|| format!("`{key}` precisa ser uma lista"))
}

/// Loads the shared drawing, runs `f`, and saves it back when `write` is set.
pub(crate) fn with_scene(write: bool, f: impl FnOnce(&mut Scene) -> Result<Value, String>) -> Result<Value, String> {
    let path = autosave_path().ok_or("pasta de dados do usuário não encontrada")?;
    let mut scene = read_scene_file(&path).map(|(scene, _)| scene).unwrap_or_default();
    let result = f(&mut scene)?;
    if write {
        scene.agent = Some(AGENT.get().cloned().unwrap_or_else(|| "IA".into()));
        scene.agent_terminal = std::env::var("MAGOAGENT_TERMINAL_ID").ok().filter(|id| !id.is_empty());
        resolve_bindings(&mut scene.elements);
        recenter_labels(&mut scene.elements);
        write_scene_file(&path, &scene).map_err(|e| format!("não foi possível salvar: {e}"))?;
    }
    Ok(result)
}

/// Compact element JSON: only the fields an AI needs to reason about the drawing.
pub(crate) fn describe(scene: &Scene) -> String {
    const KEEP: [&str; 20] = [
        "id", "type", "x", "y", "width", "height", "angle", "strokeColor", "backgroundColor", "fillStyle",
        "strokeWidth", "strokeStyle", "text", "fontSize", "points", "startBinding", "endBinding", "startArrowhead",
        "endArrowhead", "groupIds",
    ];
    let elements: Vec<Value> = interop_elements(&scene.elements)
        .into_iter()
        .map(|mut v| {
            if let Some(object) = v.as_object_mut() {
                object.retain(|k, value| {
                    KEEP.contains(&k.as_str())
                        && !value.is_null()
                        && *value != json!([])
                        && !(k == "angle" && *value == json!(0.0))
                        && !(k == "backgroundColor" && *value == "transparent")
                });
            }
            v
        })
        .collect();
    let bounds = scene.elements.iter().map(element_bounds).reduce(|a, b| a.union(b));
    json!({
        "background": hex(scene.background),
        "bounds": bounds.map(|b| json!({ "x": b.min.x, "y": b.min.y, "width": b.width(), "height": b.height() })),
        "elements": elements,
    })
    .to_string()
}
