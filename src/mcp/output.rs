//! Rendering, saving and showing the app for MCP clients.


use super::*;

pub(crate) fn view(scene: &Scene, args: &Value) -> Result<Value, String> {
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

pub(crate) fn save_file(scene: &Scene, path: &str) -> Result<Value, String> {
    let lower = path.to_lowercase();
    let bytes = if lower.ends_with(".png") {
        render_svg_png(&scene_svg(scene, Some(scene.background)), 2.0)?
    } else if lower.ends_with(".svg") {
        scene_svg(scene, Some(scene.background)).into_bytes()
    } else if lower.ends_with(".mago") || lower.ends_with(".rea") {
        serde_json::to_vec_pretty(scene).map_err(|e| e.to_string())?
    } else if lower.ends_with(".excalidraw") || lower.ends_with(".json") {
        serde_json::to_vec_pretty(&to_interop(scene)).map_err(|e| e.to_string())?
    } else {
        return Err("use uma extensão .mago, .excalidraw, .png ou .svg".into());
    };
    fs::write(path, bytes).map_err(|e| e.to_string())?;
    Ok(text(format!("Salvo em {path}")))
}

/// Opens the app: on macOS the .app bundle this binary lives in (`open` brings an already
/// open window forward); elsewhere this same binary without the `mcp` argument.
// ponytail: off macOS a second window opens if one is already running; both stay in sync
// through the shared file. Add a single-instance check if that bothers anyone.
pub(crate) fn show_app() -> Result<Value, String> {
    let exe = std::env::current_exe().map_err(|e| e.to_string())?;
    if cfg!(target_os = "macos") {
        let bundle = exe.ancestors().find(|p| p.extension().is_some_and(|e| e == "app"));
        let mut command = std::process::Command::new("open");
        match bundle {
            Some(bundle) => command.arg(bundle),
            None => command.args(["-a", "MagoAgent"]),
        };
        command.status().map_err(|e| e.to_string())?;
    } else {
        std::process::Command::new(exe).spawn().map_err(|e| e.to_string())?;
    }
    Ok(text("MagoAgent aberto".into()))
}
