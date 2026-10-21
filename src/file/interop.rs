//! The element JSON shared by the clipboard and the MCP server.

use crate::prelude::*;

pub(crate) fn parse_hex(value: &Value) -> Option<[u8; 4]> {
    let s = value.as_str()?.trim_start_matches('#');
    let s: String = if s.len() == 3 { s.chars().flat_map(|c| [c, c]).collect() } else { s.into() };
    if s.len() < 6 {
        return None; // "transparent" and friends
    }
    let byte = |i: usize| u8::from_str_radix(s.get(i..i + 2)?, 16).ok();
    Some([byte(0)?, byte(2)?, byte(4)?, if s.len() >= 8 { byte(6)? } else { 255 }])
}

pub(crate) fn interop_elements(elements: &[Element]) -> Vec<Value> {
    // targets list the arrows bound to them, so other whiteboards move those arrows too
    let mut bound: std::collections::HashMap<&str, Vec<Value>> = std::collections::HashMap::new();
    for e in elements {
        for b in [&e.ends.start, &e.ends.end].into_iter().flatten() {
            bound.entry(b.element_id.as_str()).or_default().push(json!({ "id": e.id, "type": "arrow" }));
        }
    }
    let binding = |b: &Option<Binding>| match b {
        Some(b) => json!({ "elementId": b.element_id, "fixedPoint": b.fixed, "mode": if b.inside { "inside" } else { "orbit" } }),
        None => Value::Null,
    };
    let head = |h: Option<Head>| serde_json::to_value(h).unwrap_or(Value::Null);
    elements
        .iter()
        .map(|e| {
            let b = local_bounds(e);
            let st = &e.style;
            let mut v = json!({
                "id": e.id,
                "x": b.min.x, "y": b.min.y, "width": b.width(), "height": b.height(),
                "angle": e.angle,
                "strokeColor": hex(st.stroke),
                "backgroundColor": st.fill.map(hex).unwrap_or_else(|| "transparent".into()),
                "fillStyle": match st.fill_style { FillStyle::Hachure => "hachure", FillStyle::CrossHatch => "cross-hatch", FillStyle::Solid => "solid" },
                "strokeWidth": st.stroke_width,
                "strokeStyle": match st.stroke_style { StrokeStyle::Solid => "solid", StrokeStyle::Dashed => "dashed", StrokeStyle::Dotted => "dotted" },
                "roughness": st.roughness,
                "opacity": st.opacity,
                "groupIds": e.group_id.iter().collect::<Vec<_>>(),
                "frameId": null,
                "roundness": null,
                "seed": Rng::new(&e.id, 0).0 as u32 >> 1,
                "version": 1,
                "versionNonce": 0,
                "isDeleted": false,
                "boundElements": bound.get(e.id.as_str()).cloned().map(Value::Array).unwrap_or(Value::Null),
                "updated": 0,
                "link": null,
                "locked": e.locked,
            });
            let relative = |points: &[[f32; 2]]| -> Value {
                let first = points.first().copied().unwrap_or([0.0, 0.0]);
                points.iter().map(|p| json!([p[0] - first[0], p[1] - first[1]])).collect()
            };
            if let Some(name) = &e.frame {
                v["name"] = name.clone().into();
            }
            match &e.kind {
                ElementKind::Rectangle { .. } | ElementKind::Diamond { .. } | ElementKind::Ellipse { .. } => {
                    let kind = match e.kind {
                        ElementKind::Rectangle { .. } if e.terminal.is_some() => "terminal",
                        ElementKind::Rectangle { .. } if e.portal.is_some() => "portal",
                        ElementKind::Rectangle { .. } if e.ide.is_some() => "ide",
                        ElementKind::Rectangle { .. } if e.image.is_some() => "image",
                        ElementKind::Rectangle { .. } if e.created.is_some() => "stickynote",
                        ElementKind::Rectangle { .. } if e.frame.is_some() => "frame",
                        ElementKind::Rectangle { .. } => "rectangle",
                        ElementKind::Diamond { .. } => "diamond",
                        _ => "ellipse",
                    };
                    v["type"] = kind.into();
                    if let Some(url) = &e.image {
                        v["dataURL"] = url.clone().into();
                    }
                    if let Some(created) = e.created {
                        v["created"] = created.into();
                    }
                    if let Some(t) = &e.terminal {
                        (v["name"], v["cwd"], v["command"]) = (t.name.clone().into(), t.cwd.clone().into(), t.command.clone().into());
                    }
                    if let Some(p) = &e.portal {
                        (v["name"], v["url"]) = (p.name.clone().into(), p.url.clone().into());
                        if p.tabs.len() > 1 {
                            (v["tabs"], v["active"]) = (json!(p.tabs), p.active.into());
                        }
                    }
                    if let Some(ide) = &e.ide {
                        (v["name"], v["root"]) = (ide.name.clone().into(), ide.root.clone().into());
                    }
                    if st.round && kind != "ellipse" && kind != "image" {
                        v["roundness"] = json!({ "type": 3 });
                    }
                }
                ElementKind::Line { points, arrow } => {
                    v["type"] = if *arrow { "arrow" } else { "line" }.into();
                    v["x"] = points.first().map_or(0.0, |p| p[0]).into();
                    v["y"] = points.first().map_or(0.0, |p| p[1]).into();
                    v["points"] = relative(points);
                    v["lastCommittedPoint"] = Value::Null;
                    if e.rope {
                        v["rope"] = true.into();
                    }
                    v["startBinding"] = binding(&e.ends.start);
                    v["endBinding"] = binding(&e.ends.end);
                    v["startArrowhead"] = if *arrow { head(e.ends.start_head) } else { Value::Null };
                    v["endArrowhead"] = if *arrow { head(e.ends.end_head) } else { Value::Null };
                    if st.round {
                        v["roundness"] = json!({ "type": 2 });
                    }
                }
                ElementKind::FreeDraw { points } => {
                    v["type"] = "freedraw".into();
                    v["x"] = points.first().map_or(0.0, |p| p[0]).into();
                    v["y"] = points.first().map_or(0.0, |p| p[1]).into();
                    v["points"] = relative(points);
                    v["pressures"] = json!([]);
                    v["simulatePressure"] = true.into();
                    v["lastCommittedPoint"] = Value::Null;
                }
                ElementKind::Text { value, .. } => {
                    v["type"] = "text".into();
                    v["text"] = value.clone().into();
                    v["originalText"] = value.clone().into();
                    v["fontSize"] = st.font_size.into();
                    v["fontFamily"] = 5.into();
                    v["textAlign"] = "left".into();
                    v["verticalAlign"] = "top".into();
                    v["containerId"] = Value::Null;
                    v["lineHeight"] = LINE_HEIGHT.into();
                    v["autoResize"] = true.into();
                }
            }
            v
        })
        .collect()
}

fn str_field(v: &Value, key: &str) -> String {
    v[key].as_str().unwrap_or_default().to_string()
}

pub(crate) fn from_interop_elements(elements: &Value) -> Vec<Element> {
    let f = |v: &Value, k: &str| v[k].as_f64().unwrap_or(0.0) as f32;
    elements
        .as_array()
        .into_iter()
        .flatten()
        .filter(|v| v["isDeleted"] != true)
        .filter_map(|v| {
            let (x, y, width, height) = (f(v, "x"), f(v, "y"), f(v, "width"), f(v, "height"));
            let points = || -> Vec<[f32; 2]> {
                v["points"]
                    .as_array()
                    .into_iter()
                    .flatten()
                    .map(|p| [x + p[0].as_f64().unwrap_or(0.0) as f32, y + p[1].as_f64().unwrap_or(0.0) as f32])
                    .collect()
            };
            let kind = match v["type"].as_str()? {
                "rectangle" | "frame" | "image" | "stickynote" | "terminal" | "portal" | "ide" => ElementKind::Rectangle { x, y, width, height },
                "diamond" => ElementKind::Diamond { x, y, width, height },
                "ellipse" => ElementKind::Ellipse { x, y, width, height },
                "line" => ElementKind::Line { points: points(), arrow: false },
                "arrow" => ElementKind::Line { points: points(), arrow: true },
                "freedraw" => ElementKind::FreeDraw { points: points() },
                "text" => ElementKind::Text { x, y, value: v["text"].as_str().unwrap_or_default().into() },
                _ => return None,
            };
            let style = Style {
                stroke: parse_hex(&v["strokeColor"]).unwrap_or(BLACK),
                fill: parse_hex(&v["backgroundColor"]),
                stroke_width: v["strokeWidth"].as_f64().unwrap_or(2.0) as f32,
                stroke_style: match v["strokeStyle"].as_str() {
                    Some("dashed") => StrokeStyle::Dashed,
                    Some("dotted") => StrokeStyle::Dotted,
                    _ => StrokeStyle::Solid,
                },
                fill_style: match v["fillStyle"].as_str() {
                    Some("hachure") => FillStyle::Hachure,
                    Some("cross-hatch") => FillStyle::CrossHatch,
                    _ => FillStyle::Solid,
                },
                roughness: v["roughness"].as_u64().unwrap_or(1).min(2) as u8,
                round: !v["roundness"].is_null(),
                opacity: v["opacity"].as_u64().unwrap_or(100).min(100) as u8,
                font_size: v["fontSize"].as_f64().unwrap_or(20.0) as f32,
            };
            let mut element = Element {
                id: v["id"].as_str().map(String::from).unwrap_or_else(|| uuid::Uuid::new_v4().to_string()),
                // outermost group, so nested groups still move together
                group_id: v["groupIds"].as_array().and_then(|g| g.last()).and_then(|g| g.as_str()).map(String::from),
                kind,
                angle: 0.0,
                locked: v["locked"].as_bool().unwrap_or(false),
                frame: (v["type"] == "frame").then(|| v["name"].as_str().unwrap_or_default().to_string()),
                image: (v["type"] == "image").then(|| v["dataURL"].as_str().unwrap_or_default().to_string()),
                created: (v["type"] == "stickynote").then(|| v["created"].as_i64().unwrap_or(0)),
                terminal: (v["type"] == "terminal").then(|| TerminalSpec {
                    name: str_field(v, "name"),
                    cwd: str_field(v, "cwd"),
                    command: str_field(v, "command"),
                }),
                portal: (v["type"] == "portal").then(|| PortalSpec {
                    name: str_field(v, "name"),
                    url: str_field(v, "url"),
                    tabs: v["tabs"].as_array().into_iter().flatten().filter_map(|t| t.as_str().map(String::from)).collect(),
                    active: v["active"].as_u64().unwrap_or(0) as usize,
                }),
                ide: (v["type"] == "ide").then(|| IdeSpec { name: str_field(v, "name"), root: str_field(v, "root") }),
                rope: v["rope"] == true,
                ends: ArrowEnds {
                    start_head: serde_json::from_value(v["startArrowhead"].clone()).ok().flatten(),
                    end_head: match v.get("endArrowhead") {
                        Some(h) => serde_json::from_value(h.clone()).ok().flatten(),
                        None => Some(Head::Arrow),
                    },
                    start: parse_binding(&v["startBinding"]),
                    end: parse_binding(&v["endBinding"]),
                },
                style,
            };
            let angle = v["angle"].as_f64().unwrap_or(0.0) as f32;
            if angle != 0.0 {
                // Rotation is around the element center; lines get the rotation baked in
                let center = element.center();
                element.rotate_around(center, angle);
            }
            Some(element)
        })
        .collect()
}

pub(crate) fn parse_binding(v: &Value) -> Option<Binding> {
    let fixed = v["fixedPoint"].as_array().map_or([0.5, 0.5], |p| {
        [p.first().and_then(Value::as_f64).unwrap_or(0.5) as f32, p.get(1).and_then(Value::as_f64).unwrap_or(0.5) as f32]
    });
    Some(Binding { element_id: v["elementId"].as_str()?.into(), fixed, inside: v["mode"] == "inside" })
}

/// This app's own `.mago` format (`.rea` before).
pub(crate) fn parse_scene(data: &str) -> Result<Scene, String> {
    serde_json::from_str(data).map_err(|e| e.to_string())
}
