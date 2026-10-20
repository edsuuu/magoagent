//! MCP server (`magoagent mcp`): lets any AI client draw on the MagoAgent canvas,
//! read it back and look at a rendered PNG. JSON-RPC over stdio, one message per line.
//! The shared autosave file is the source of truth, so it works with the app open or closed;
//! the open app merges these edits live and Cmd+Z undoes them.

use std::io::{BufRead, Write};

use serde_json::{Map, Value, json};

use super::*;

const PROTOCOL_VERSION: &str = "2025-06-18";

const INSTRUCTIONS: &str = "MagoAgent é um quadro branco com traço feito à mão. Coordenadas em pixels, x para a direita e \
y para baixo; o canvas é infinito. Elementos são objetos JSON: type (rectangle, ellipse, diamond, text, \
arrow, line, freedraw), x, y, width, height, strokeColor e backgroundColor (hex ou \"transparent\"), fillStyle \
(solid, hachure, cross-hatch), strokeWidth (1, 2, 4), strokeStyle (solid, dashed, dotted), roughness (0 arquiteto, \
1 artista, 2 cartunista), opacity (0-100), angle (radianos), fontSize, text e points (relativos a x,y). Atalhos: \
`label: {text}` coloca um texto centralizado dentro da forma ou no meio da seta (id \"<id>-label\", \
acompanha o dono e interrompe a linha da seta atrás dele, então use label em setas de \"sim\"/\"não\"); \
textos que cruzam linhas ganham contorno na cor do fundo automaticamente; \
para contêineres/cards use type \"frame\" (name opcional, vazio esconde o título): mover o quadro \
leva tudo o que está dentro dele; setas com `start: {id}` e \
`end: {id}` se prendem às formas e as acompanham quando elas se movem. Dê `id` aos elementos para referenciá-los. \
Fluxo sugerido: planeje o layout (ex.: caixas de 180x80 com 80px de espaço), crie tudo com add_elements, chame \
view para ver a imagem e corrija com update_elements.";

pub fn run() {
    let stdin = std::io::stdin();
    let mut out = std::io::stdout();
    for line in stdin.lock().lines() {
        let Ok(line) = line else {
            break;
        };
        if line.trim().is_empty() {
            continue;
        }
        let reply = match serde_json::from_str::<Value>(&line) {
            Ok(message) => handle(&message),
            Err(e) => Some(json!({ "jsonrpc": "2.0", "id": null, "error": { "code": -32700, "message": e.to_string() } })),
        };
        if let Some(reply) = reply {
            let _ = writeln!(out, "{reply}");
            let _ = out.flush();
        }
    }
}

static AGENT: std::sync::OnceLock<String> = std::sync::OnceLock::new();

/// A readable name for the MCP client: "claude-code" → "Claude Code".
fn agent_label(client: &str) -> String {
    match client {
        "claude-ai" => "Claude".into(),
        "codex-mcp-client" => "Codex".into(),
        "cursor-vscode" => "Cursor".into(),
        "" => "IA".into(),
        other => other
            .split(['-', '_', ' '])
            .filter(|w| !w.is_empty() && *w != "mcp" && *w != "client")
            .map(|w| {
                let mut chars = w.chars();
                chars.next().map_or(String::new(), |first| first.to_uppercase().chain(chars).collect())
            })
            .collect::<Vec<_>>()
            .join(" "),
    }
}

/// One JSON-RPC message in, at most one response out (notifications get none).
fn handle(message: &Value) -> Option<Value> {
    let id = message.get("id")?.clone();
    let params = &message["params"];
    let result = match message["method"].as_str().unwrap_or_default() {
        "initialize" => {
            // the app shows this name on the cursor while it replays what the AI drew
            let _ = AGENT.set(agent_label(params["clientInfo"]["name"].as_str().unwrap_or_default()));
            Ok(json!({
                "protocolVersion": params["protocolVersion"].as_str().unwrap_or(PROTOCOL_VERSION),
                "capabilities": { "tools": {} },
                "serverInfo": { "name": "magoagent", "version": env!("CARGO_PKG_VERSION") },
                "instructions": INSTRUCTIONS,
            }))
        }
        "ping" => Ok(json!({})),
        "tools/list" => Ok(json!({ "tools": tools() })),
        "tools/call" => Ok(call(params["name"].as_str().unwrap_or_default(), &params["arguments"])),
        other => Err(json!({ "code": -32601, "message": format!("método desconhecido: {other}") })),
    };
    Some(match result {
        Ok(result) => json!({ "jsonrpc": "2.0", "id": id, "result": result }),
        Err(error) => json!({ "jsonrpc": "2.0", "id": id, "error": error }),
    })
}

fn tools() -> Value {
    let elements = json!({ "type": "array", "items": { "type": "object" } });
    let ids = json!({ "type": "array", "items": { "type": "string" } });
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
            "description": "Salva o desenho em um arquivo: .rea (formato do MagoAgent), .png ou .svg.",
            "inputSchema": { "type": "object", "properties": { "path": { "type": "string" } }, "required": ["path"] },
        },
        {
            "name": "open_file",
            "description": "Substitui o desenho atual pelo conteúdo de um arquivo .excalidraw ou .rea.",
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
    ])
}

fn call(name: &str, args: &Value) -> Value {
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
        other => Err(format!("ferramenta desconhecida: {other}")),
    };
    match outcome {
        Ok(content) => json!({ "content": content }),
        Err(message) => json!({ "content": text(message), "isError": true }),
    }
}

fn text(message: String) -> Value {
    json!([{ "type": "text", "text": message }])
}

fn list<'a>(args: &'a Value, key: &str) -> Result<&'a Vec<Value>, String> {
    args[key].as_array().ok_or_else(|| format!("`{key}` precisa ser uma lista"))
}

/// Loads the shared drawing, runs `f`, and saves it back when `write` is set.
fn with_scene(write: bool, f: impl FnOnce(&mut Scene) -> Result<Value, String>) -> Result<Value, String> {
    let path = autosave_path().ok_or("HOME não definido")?;
    let mut scene = read_scene_file(&path).map(|(scene, _)| scene).unwrap_or_default();
    let result = f(&mut scene)?;
    if write {
        scene.agent = Some(AGENT.get().cloned().unwrap_or_else(|| "IA".into()));
        resolve_bindings(&mut scene.elements);
        recenter_labels(&mut scene.elements);
        write_scene_file(&path, &scene).map_err(|e| format!("não foi possível salvar: {e}"))?;
    }
    Ok(result)
}

/// Compact element JSON: only the fields an AI needs to reason about the drawing.
fn describe(scene: &Scene) -> String {
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

fn new_id() -> String {
    uuid::Uuid::new_v4().to_string()
}

/// Turns element JSON (plus the label/start/end shortcuts) into elements.
fn add_elements(scene: &mut Scene, items: &[Value]) -> Result<Vec<Value>, String> {
    let mut created = Vec::new();
    for item in items {
        let mut v = item.as_object().cloned().ok_or("cada elemento precisa ser um objeto")?;
        let kind = v.get("type").and_then(Value::as_str).ok_or("elemento sem `type`")?.to_string();
        let id = v.get("id").and_then(Value::as_str).map(String::from).unwrap_or_else(new_id);
        if scene.elements.iter().any(|e| e.id == id) {
            return Err(format!("já existe um elemento com id `{id}`"));
        }
        v.insert("id".into(), id.clone().into());
        let label = v.remove("label");
        match kind.as_str() {
            "rectangle" | "diamond" | "ellipse" => {
                v.entry("width").or_insert(json!(160.0));
                v.entry("height").or_insert(json!(80.0));
                if kind != "ellipse" {
                    // the app draws rounded corners by default
                    v.entry("roundness").or_insert(json!({ "type": 3 }));
                }
            }
            "arrow" | "line" => prepare_line(scene, &mut v)?,
            _ => {}
        }
        let element = from_interop_elements(&json!([Value::Object(v)]))
            .pop()
            .ok_or_else(|| format!("tipo não suportado: {kind}"))?;
        // a frame drawn after its contents still goes behind them, or its fill would hide them
        let at = if element.frame.is_some() {
            let area = element_bounds(&element);
            scene.elements.iter().position(|e| area.contains_rect(element_bounds(e))).unwrap_or(scene.elements.len())
        } else {
            scene.elements.len()
        };
        scene.elements.insert(at, element);
        created.push(json!({ "id": id, "type": kind }));
        if let Some(label) = label.filter(|l| !l.is_null()) {
            created.push(add_label(scene, at, &label)?);
        }
    }
    resolve_bindings(&mut scene.elements);
    Ok(created)
}

fn center_of(scene: &Scene, id: &str) -> Result<Pos2, String> {
    scene
        .elements
        .iter()
        .find(|e| e.id == id)
        .map(|e| element_bounds(e).center())
        .ok_or_else(|| format!("elemento `{id}` não existe"))
}

/// `start: {id}` / `end: {id}` become orbit bindings; missing points are aimed center to center.
fn prepare_line(scene: &Scene, v: &mut Map<String, Value>) -> Result<(), String> {
    let mut ends = [None, None];
    for (k, key) in ["start", "end"].into_iter().enumerate() {
        if let Some(target) = v.remove(key).and_then(|t| t["id"].as_str().map(String::from)) {
            let center = center_of(scene, &target)?;
            v.insert(
                format!("{key}Binding"),
                json!({ "elementId": target, "fixedPoint": [0.5, 0.5], "mode": "orbit" }),
            );
            ends[k] = Some(center);
        }
    }
    if v.get("points").is_none() {
        let x = v.get("x").and_then(Value::as_f64).map(|x| x as f32);
        let y = v.get("y").and_then(Value::as_f64).map(|y| y as f32);
        let start = ends[0].or(x.zip(y).map(|(x, y)| Pos2::new(x, y))).unwrap_or(Pos2::ZERO);
        let kept_end = v.get("_end").and_then(Value::as_array).map(|p| {
            Pos2::new(p[0].as_f64().unwrap_or(0.0) as f32, p[1].as_f64().unwrap_or(0.0) as f32)
        });
        let end = ends[1].or(kept_end).unwrap_or_else(|| {
            let w = v.get("width").and_then(Value::as_f64).unwrap_or(100.0) as f32;
            let h = v.get("height").and_then(Value::as_f64).unwrap_or(0.0) as f32;
            start + Vec2::new(w, h)
        });
        v.insert("x".into(), start.x.into());
        v.insert("y".into(), start.y.into());
        v.insert("points".into(), json!([[0.0, 0.0], [end.x - start.x, end.y - start.y]]));
    }
    v.remove("_end");
    Ok(())
}

/// Centered text grouped with its shape (or placed at an arrow's middle).
fn add_label(scene: &mut Scene, owner: usize, label: &Value) -> Result<Value, String> {
    let content = label["text"].as_str().or(label.as_str()).ok_or("`label` precisa de `text`")?;
    let size = label["fontSize"].as_f64().unwrap_or(20.0) as f32;
    let center = element_bounds(&scene.elements[owner]).center();
    let lines = text_lines(content);
    let longest = lines.iter().map(|l| l.chars().count()).max().unwrap_or(0) as f32;
    let width = (longest * size * CHAR_WIDTH).max(size * 0.5);
    let height = lines.len() as f32 * size * LINE_HEIGHT;
    let group = scene.elements[owner].group_id.clone().unwrap_or_else(new_id);
    scene.elements[owner].group_id = Some(group.clone());
    // the id ties the label to its owner, so it stays centered when the owner moves
    let owner_id = scene.elements[owner].id.clone();
    let id = format!("{owner_id}-label");
    scene.elements.retain(|e| e.id != id);
    let owner = scene.elements.iter().position(|e| e.id == owner_id).ok_or("dono do rótulo sumiu")?;
    let mut text = from_interop_elements(&json!([{
        "id": id,
        "type": "text",
        "x": center.x - width / 2.0,
        "y": center.y - height / 2.0,
        "text": content,
        "fontSize": size,
        "strokeColor": label["strokeColor"].as_str().unwrap_or("#1e1e1e"),
    }]))
    .pop()
    .ok_or("não foi possível criar o rótulo")?;
    text.group_id = Some(group);
    scene.elements.push(text);
    Ok(json!({ "id": id, "type": "text", "labelOf": scene.elements[owner].id }))
}

/// Patches go through the element JSON round trip, so every field the format knows can change.
fn update_elements(scene: &mut Scene, patches: &[Value]) -> Result<Vec<String>, String> {
    let mut updated = Vec::new();
    for patch in patches {
        let id = patch["id"].as_str().ok_or("cada alteração precisa de `id`")?;
        let index = scene.elements.iter().position(|e| e.id == id).ok_or_else(|| format!("elemento `{id}` não existe"))?;
        let mut v = interop_elements(&scene.elements[index..=index]).remove(0);
        let object = v.as_object_mut().ok_or("formato interno inválido")?;
        let patch = patch.as_object().cloned().unwrap_or_default();
        let mut combined = merge(object.clone(), &patch);
        if let ElementKind::Line { points, .. } = &scene.elements[index].kind {
            let rebinding = patch.contains_key("start") || patch.contains_key("end");
            if rebinding && !patch.contains_key("points") {
                // re-aim only the ends being re-bound; the other end stays where it was
                let (first, last) = (points[0], points[points.len() - 1]);
                combined.remove("points");
                if !patch.contains_key("x") && !patch.contains_key("y") {
                    combined.insert("x".into(), json!(first[0]));
                    combined.insert("y".into(), json!(first[1]));
                }
                combined.insert("_end".into(), json!(last));
            }
            for key in ["start", "end"] {
                if patch.get(key).is_some_and(Value::is_null) {
                    combined.remove(key);
                    combined.insert(format!("{key}Binding"), Value::Null);
                }
            }
            prepare_line(scene, &mut combined)?;
        }
        *object = combined;
        let mut element = from_interop_elements(&json!([v])).pop().ok_or("alteração inválida")?;
        element.group_id = scene.elements[index].group_id.clone();
        scene.elements[index] = element;
        updated.push(id.to_string());
    }
    resolve_bindings(&mut scene.elements);
    Ok(updated)
}

fn merge(mut base: Map<String, Value>, patch: &Map<String, Value>) -> Map<String, Value> {
    for (k, v) in patch {
        base.insert(k.clone(), v.clone());
    }
    base
}

fn view(scene: &Scene, args: &Value) -> Result<Value, String> {
    let ids: Vec<&str> = args["ids"].as_array().into_iter().flatten().filter_map(Value::as_str).collect();
    let elements: Vec<Element> =
        scene.elements.iter().filter(|e| ids.is_empty() || ids.contains(&e.id.as_str())).cloned().collect();
    if elements.is_empty() {
        return Ok(text("O desenho está vazio.".into()));
    }
    let subset = Scene { elements, ..scene.clone() };
    let bounds = subset.elements.iter().map(element_bounds).reduce(|a, b| a.union(b)).unwrap_or(Rect::ZERO);
    // keep the image under ~2000px on its longest side
    let scale = args["scale"].as_f64().unwrap_or(1.0).clamp(0.1, 3.0) as f32;
    let scale = scale.min(2000.0 / (bounds.width().max(bounds.height()) + 20.0));
    let png = render_svg_png(&scene_svg(&subset, Some(scene.background)), scale)?;
    Ok(json!([
        { "type": "image", "data": base64(&png), "mimeType": "image/png" },
        { "type": "text", "text": format!(
            "{} elemento(s); área x={:.0} y={:.0} largura={:.0} altura={:.0} (imagem com 10px de margem, escala {:.2})",
            subset.elements.len(), bounds.min.x, bounds.min.y, bounds.width(), bounds.height(), scale
        ) },
    ]))
}

fn save_file(scene: &Scene, path: &str) -> Result<Value, String> {
    let lower = path.to_lowercase();
    let bytes = if lower.ends_with(".png") {
        render_svg_png(&scene_svg(scene, Some(scene.background)), 2.0)?
    } else if lower.ends_with(".svg") {
        scene_svg(scene, Some(scene.background)).into_bytes()
    } else if lower.ends_with(".rea") {
        serde_json::to_vec_pretty(scene).map_err(|e| e.to_string())?
    } else if lower.ends_with(".excalidraw") || lower.ends_with(".json") {
        serde_json::to_vec_pretty(&to_interop(scene)).map_err(|e| e.to_string())?
    } else {
        return Err("use uma extensão .excalidraw, .png, .svg ou .rea".into());
    };
    fs::write(path, bytes).map_err(|e| e.to_string())?;
    Ok(text(format!("Salvo em {path}")))
}

/// Opens the .app bundle this binary lives in (…/MagoAgent.app/Contents/MacOS/magoagent).
fn show_app() -> Result<Value, String> {
    let exe = std::env::current_exe().map_err(|e| e.to_string())?;
    let bundle = exe.ancestors().find(|p| p.extension().is_some_and(|e| e == "app"));
    let mut command = std::process::Command::new("open");
    match bundle {
        Some(bundle) => command.arg(bundle),
        None => command.args(["-a", "MagoAgent"]),
    };
    command.status().map_err(|e| e.to_string())?;
    Ok(text("MagoAgent aberto".into()))
}

pub(crate) fn base64(bytes: &[u8]) -> String {
    const ALPHABET: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::with_capacity(bytes.len().div_ceil(3) * 4);
    for chunk in bytes.chunks(3) {
        let n = chunk.iter().enumerate().fold(0u32, |n, (i, b)| n | (*b as u32) << (16 - 8 * i));
        for i in 0..4 {
            out.push(if i <= chunk.len() { ALPHABET[(n >> (18 - 6 * i) & 63) as usize] as char } else { '=' });
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn by_id<'a>(scene: &'a Scene, id: &str) -> &'a Element {
        scene.elements.iter().find(|e| e.id == id).unwrap()
    }

    #[test]
    fn late_frame_goes_behind_its_contents() {
        let mut scene = Scene::default();
        let box_ = json!({ "type": "rectangle", "id": "b", "x": 20, "y": 20, "width": 50, "height": 30 });
        let frame = json!({ "type": "frame", "id": "f", "x": 0, "y": 0, "width": 200, "height": 100, "backgroundColor": "#000000" });
        add_elements(&mut scene, &[box_, frame]).unwrap();
        assert_eq!(scene.elements.iter().map(|e| e.id.as_str()).collect::<Vec<_>>(), ["f", "b"]);
    }

    #[test]
    fn agent_names_are_readable() {
        assert_eq!(agent_label("claude-code"), "Claude Code");
        assert_eq!(agent_label("codex-mcp-client"), "Codex");
        assert_eq!(agent_label("gemini_cli"), "Gemini Cli");
        assert_eq!(agent_label("ágil"), "Ágil");
        assert_eq!(agent_label(""), "IA");
    }

    #[test]
    fn base64_matches_rfc4648() {
        assert_eq!(base64(b""), "");
        assert_eq!(base64(b"f"), "Zg==");
        assert_eq!(base64(b"fo"), "Zm8=");
        assert_eq!(base64(b"foo"), "Zm9v");
        assert_eq!(base64(b"foobar"), "Zm9vYmFy");
    }

    #[test]
    fn diagram_with_labels_and_bound_arrow() {
        let mut scene = Scene::default();
        let created = add_elements(
            &mut scene,
            json!([
                { "id": "a", "type": "rectangle", "x": 0, "y": 0, "width": 200, "height": 100, "label": { "text": "Início" } },
                { "id": "b", "type": "ellipse", "x": 400, "y": 0, "width": 200, "height": 100, "backgroundColor": "#a5d8ff" },
                { "id": "seta", "type": "arrow", "start": { "id": "a" }, "end": { "id": "b" } },
            ])
            .as_array()
            .unwrap(),
        )
        .unwrap();
        assert_eq!(created.len(), 4, "label adds a text element");
        let label = scene.elements.iter().find(|e| matches!(e.kind, ElementKind::Text { .. })).unwrap();
        assert_eq!(label.id, "a-label");
        assert_eq!(label.group_id, by_id(&scene, "a").group_id);
        assert!((element_bounds(label).center() - Pos2::new(100.0, 50.0)).length() < 1.0);
        assert!(by_id(&scene, "a").style.round, "rectangles default to rounded corners");
        let ElementKind::Line { points, .. } = &by_id(&scene, "seta").kind else { panic!() };
        // orbit endpoints: just outside each outline, on the center line
        assert!((points[0][0] - 206.0).abs() < 0.5 && (points[1][0] - 394.0).abs() < 0.5, "{points:?}");
        // moving the ellipse drags the arrow along
        update_elements(&mut scene, json!([{ "id": "b", "y": 300 }]).as_array().unwrap()).unwrap();
        let ElementKind::Line { points, .. } = &by_id(&scene, "seta").kind else { panic!() };
        assert!(points[1][1] > 250.0, "{points:?}");
        // a moved shape takes its label along
        update_elements(&mut scene, json!([{ "id": "a", "x": 50 }]).as_array().unwrap()).unwrap();
        recenter_labels(&mut scene.elements);
        let label = by_id(&scene, "a-label");
        assert!((element_bounds(label).center().x - 150.0).abs() < 1.0);
        assert!(add_elements(&mut scene, json!([{ "id": "a", "type": "rectangle" }]).as_array().unwrap()).is_err());
    }

    #[test]
    fn update_changes_text_and_style_and_rebinds() {
        let mut scene = Scene::default();
        add_elements(
            &mut scene,
            json!([
                { "id": "t", "type": "text", "x": 10, "y": 10, "text": "olá" },
                { "id": "c", "type": "rectangle", "x": 300, "y": 0 },
                { "id": "s", "type": "arrow", "x": 0, "y": 200, "width": 100, "height": 0 },
            ])
            .as_array()
            .unwrap(),
        )
        .unwrap();
        update_elements(
            &mut scene,
            json!([
                { "id": "t", "text": "tchau", "strokeColor": "#e03131" },
                { "id": "s", "end": { "id": "c" } },
            ])
            .as_array()
            .unwrap(),
        )
        .unwrap();
        let t = by_id(&scene, "t");
        assert!(matches!(&t.kind, ElementKind::Text { value, .. } if value == "tchau"));
        assert_eq!(t.style.stroke, [0xe0, 0x31, 0x31, 255]);
        let s = by_id(&scene, "s");
        assert_eq!(s.ends.end.as_ref().map(|b| b.element_id.as_str()), Some("c"));
        let ElementKind::Line { points, .. } = &s.kind else { panic!() };
        assert_eq!(points[0], [0.0, 200.0], "the free end stays put");
    }

    #[test]
    fn view_returns_a_png() {
        let mut scene = Scene::default();
        add_elements(&mut scene, json!([{ "type": "diamond", "x": 0, "y": 0 }]).as_array().unwrap()).unwrap();
        let content = view(&scene, &json!({})).unwrap();
        assert_eq!(content[0]["mimeType"], "image/png");
        assert!(content[0]["data"].as_str().unwrap().starts_with("iVBORw0KGgo"), "PNG signature");
    }

    #[test]
    fn json_rpc_round_trip() {
        let init = handle(&json!({ "jsonrpc": "2.0", "id": 1, "method": "initialize", "params": { "protocolVersion": "2025-03-26" } })).unwrap();
        assert_eq!(init["result"]["protocolVersion"], "2025-03-26");
        assert!(handle(&json!({ "jsonrpc": "2.0", "method": "notifications/initialized" })).is_none());
        let list = handle(&json!({ "jsonrpc": "2.0", "id": 2, "method": "tools/list" })).unwrap();
        assert!(list["result"]["tools"].as_array().unwrap().iter().any(|t| t["name"] == "view"));
        let unknown = handle(&json!({ "jsonrpc": "2.0", "id": 3, "method": "nope" })).unwrap();
        assert_eq!(unknown["error"]["code"], -32601);
    }
}
