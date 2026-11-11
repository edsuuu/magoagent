//! Notes from MCP calls: create one, change its text, color or title, and set its action buttons.

use super::*;
use crate::app::notes::rename_note_text;

/// A paper by name (Portuguese or the palette's English one) or by hex, as the nearest of the ten.
fn paper_named(color: &str) -> Result<[u8; 4], String> {
    const ENGLISH: [&str; 10] = ["yellow", "pink", "blue", "green", "orange", "purple", "white", "charcoal", "slate", "midnight"];
    let lower = color.trim().to_lowercase();
    if let Some(k) = NOTE_PAPERS.iter().position(|(_, name)| name.to_lowercase() == lower).or_else(|| ENGLISH.iter().position(|n| *n == lower)) {
        return Ok(NOTE_PAPERS[k].0);
    }
    parse_hex(&json!(color)).map(nearest_paper).ok_or_else(|| {
        let names: Vec<&str> = NOTE_PAPERS.iter().map(|(_, n)| *n).collect();
        format!("cor `{color}` desconhecida: use um nome ({}) ou um hex", names.join(", "))
    })
}

/// `[{label, action, args?}]` as buttons; each action has to be one of the app's.
fn buttons_from(list: &Value) -> Result<Vec<NoteButton>, String> {
    list.as_array()
        .ok_or("`buttons` precisa ser uma lista de {label, action, args}")?
        .iter()
        .map(|item| {
            let (label, action) = (item["label"].as_str().unwrap_or_default().trim(), item["action"].as_str().unwrap_or_default().trim());
            if label.is_empty() {
                return Err("todo botão precisa de `label`".to_string());
            }
            if !app_tools::is_action(action) {
                return Err(format!("`{action}` não é uma ação do app (veja as ferramentas do app, como chat_new, open_path, ide_folder)"));
            }
            Ok(NoteButton { label: label.into(), action: action.into(), args: item["args"].clone() })
        })
        .collect()
}

/// Creates a note (no `id`) or changes one: `text`, `title` (its first line), `color`, `x`, `y`,
/// `width`, `height` and `buttons` (which replace the ones it has).
pub(crate) fn note_call(scene: &mut Scene, args: &Value) -> Result<Value, String> {
    let paper = args["color"].as_str().map(paper_named).transpose()?;
    let buttons = args.get("buttons").filter(|b| !b.is_null()).map(buttons_from).transpose()?;
    let id = match args["id"].as_str() {
        Some(id) => id.to_string(),
        None => {
            // beside what is drawn when no place is given
            let right = scene.elements.iter().map(|e| element_bounds(e).max.x).fold(f32::MIN, f32::max);
            let x = args["x"].as_f64().unwrap_or(if scene.elements.is_empty() { 0.0 } else { right as f64 + 40.0 });
            let id = new_id();
            let item = json!({ "type": "stickynote", "id": id, "x": x, "y": args["y"].as_f64().unwrap_or(0.0), "backgroundColor": "#fff9c4" });
            add_elements(scene, &[item])?;
            id
        }
    };
    let index = scene.elements.iter().position(|e| e.id == id && e.created.is_some()).ok_or_else(|| format!("`{id}` não é uma nota"))?;
    if let ElementKind::Rectangle { x, y, width, height } = &mut scene.elements[index].kind {
        let num = |key: &str, now: f32| args[key].as_f64().map_or(now, |v| v as f32);
        (*x, *y) = (num("x", *x), num("y", *y));
        (*width, *height) = (num("width", *width).max(NOTE_MIN.x), num("height", *height).max(NOTE_MIN.y));
    }
    if let Some(paper) = paper {
        scene.elements[index].style.fill = Some(paper);
    }
    if let Some(buttons) = buttons {
        scene.elements[index].note.buttons = buttons;
    }
    // the text is the note's label element
    let label = note_label(&scene.elements, index);
    if args["text"].is_string() || args["title"].is_string() || label.is_none() {
        let current = label.and_then(|l| match &scene.elements[l].kind {
            ElementKind::Text { value, .. } => Some(value.clone()),
            _ => None,
        });
        // a title with a new text heads it; a title alone renames the first line the note has
        let text = match (args["text"].as_str(), args["title"].as_str()) {
            (Some(text), Some(title)) => format!(
                "# {}
{text}",
                title.trim()
            ),
            (Some(text), None) => text.to_string(),
            (None, Some(title)) => rename_note_text(&current.unwrap_or_default(), title),
            (None, None) => current.unwrap_or_default(),
        };
        match label {
            Some(l) => {
                if let ElementKind::Text { value, .. } = &mut scene.elements[l].kind {
                    *value = text;
                }
            }
            None => {
                add_label(scene, index, &json!({ "text": text, "fontSize": 20 }))?;
            }
        }
    }
    let note = &scene.elements[index];
    let color = NOTE_PAPERS.iter().find(|(p, _)| Some(*p) == note.style.fill).map(|(_, n)| *n);
    Ok(json!({ "id": id, "color": color, "buttons": note.note.buttons.len() }))
}

fn note_buttons_schema() -> Value {
    json!({ "type": "array", "items": { "type": "object", "properties": {
        "label": { "type": "string" }, "action": { "type": "string" }, "args": { "type": "object" },
    }, "required": ["label", "action"] } })
}

/// The note tools' definitions.
pub(crate) fn note_tools() -> Vec<Value> {
    json!([
        {
            "name": "note",
            "description": "Cria uma nota no canvas (sem `id`) ou muda uma (com `id`): `text` (markdown), `title` (a primeira linha), `color` \
                (Amarelo, Rosa, Azul, Verde, Laranja, Roxo, Branco, Carvão, Ardósia, Meia-noite; o texto acompanha), x, y, width, height \
                e `buttons`: [{label, action, args}], botões no pé da nota que a pessoa clica para rodar uma ação do app (substituem os que ela tem).",
            "inputSchema": { "type": "object", "properties": {
                "id": { "type": "string" }, "text": { "type": "string" }, "title": { "type": "string" }, "color": { "type": "string" },
                "x": { "type": "number" }, "y": { "type": "number" }, "width": { "type": "number" }, "height": { "type": "number" },
                "buttons": note_buttons_schema(),
            } },
        },
        {
            "name": "note_buttons",
            "description": "Define os botões de ação de uma nota (substituem os que ela tem; lista vazia tira todos). Cada botão tem `label`, `action` \
                (o nome de uma ferramenta do app, como chat_new, open_path, ide_folder, chat_send) e `args` (os argumentos dela). \
                Rodam só quando a pessoa clica; ação inválida mostra o erro num aviso.",
            "inputSchema": { "type": "object", "properties": { "id": { "type": "string", "description": "o id da nota" }, "buttons": note_buttons_schema() }, "required": ["id", "buttons"] },
        },
    ])
    .as_array()
    .cloned()
    .unwrap_or_default()
}
