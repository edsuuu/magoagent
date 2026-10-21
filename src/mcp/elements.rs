//! Adding and updating elements from MCP calls.

use serde_json::Map;

use super::*;

pub(crate) fn new_id() -> String {
    uuid::Uuid::new_v4().to_string()
}

/// Turns element JSON (plus the label/start/end shortcuts) into elements.
pub(crate) fn add_elements(scene: &mut Scene, items: &[Value]) -> Result<Vec<Value>, String> {
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
            "terminal" | "portal" | "ide" => {
                let size = match kind.as_str() {
                    "terminal" => terminal_size(80, 24),
                    "ide" => IDE_SIZE,
                    _ => Vec2::new(960.0, 640.0),
                };
                v.entry("width").or_insert(json!(size.x));
                v.entry("height").or_insert(json!(size.y));
                // dark box for exports and view; the app paints the live terminal, page or IDE
                v.entry("backgroundColor").or_insert(json!("#181818"));
                v.entry("strokeColor").or_insert(json!("#3a3a3a"));
                v.entry("roughness").or_insert(json!(0));
                v.entry("roundness").or_insert(json!({ "type": 3 }));
            }
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

pub(crate) fn center_of(scene: &Scene, id: &str) -> Result<Pos2, String> {
    scene
        .elements
        .iter()
        .find(|e| e.id == id)
        .map(|e| element_bounds(e).center())
        .ok_or_else(|| format!("elemento `{id}` não existe"))
}

/// `start: {id}` / `end: {id}` become orbit bindings; missing points are aimed center to center.
pub(crate) fn prepare_line(scene: &Scene, v: &mut Map<String, Value>) -> Result<(), String> {
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
pub(crate) fn add_label(scene: &mut Scene, owner: usize, label: &Value) -> Result<Value, String> {
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
pub(crate) fn update_elements(scene: &mut Scene, patches: &[Value]) -> Result<Vec<String>, String> {
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

pub(crate) fn merge(mut base: Map<String, Value>, patch: &Map<String, Value>) -> Map<String, Value> {
    for (k, v) in patch {
        base.insert(k.clone(), v.clone());
    }
    base
}
