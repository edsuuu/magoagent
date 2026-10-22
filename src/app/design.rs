//! Portal Design, as the reference app's: arm it on a portal, draw around what should change, and
//! the circled region goes to the agent picked in the composer, with a comment. Kept annotations
//! wait for the next send. The page is a native view above everything the app draws, so the
//! overlay (design.js) lives in the page; the app keeps each region's picture (macOS: the page's
//! snapshot) and the annotations of every portal.

use crate::prelude::*;

/// The overlay's script; `__CONFIG__` becomes the nonce, the agents, the annotations and whether
/// the annotations' composer opens at once.
const DESIGN_SCRIPT: &str = include_str!("design.js");

/// A stroke this small is a tap: nothing to select.
const TAP: f64 = 5.0;

/// The box of a stroke in the page's CSS px; None when the stroke is only a tap.
pub(crate) fn design_region(points: &[[f64; 2]]) -> Option<[f64; 4]> {
    let mut min = [f64::MAX; 2];
    let mut max = [f64::MIN; 2];
    for p in points {
        for k in 0..2 {
            min[k] = min[k].min(p[k]);
            max[k] = max[k].max(p[k]);
        }
    }
    let (w, h) = (max[0] - min[0], max[1] - min[1]);
    (w >= TAP || h >= TAP).then_some([min[0], min[1], w, h])
}

/// A kept annotation: a region of the page, the note written over it, and its picture.
#[derive(Clone, Debug, Default, PartialEq)]
pub(crate) struct Annotation {
    pub(crate) rect: [f64; 4],
    pub(crate) note: String,
    pub(crate) png: Option<Vec<u8>>,
}

/// The region the stroke made, and its picture once the page has taken it.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct Region {
    pub(crate) rect: [f64; 4],
    pub(crate) png: Option<Vec<u8>>,
}

/// Design armed on a portal's open tab.
#[derive(Clone, Debug)]
pub(crate) struct DesignSession {
    pub(crate) portal: String,
    pub(crate) tab: u64,
    /// Counts the strokes, so a picture that comes back late goes only to its own region.
    pub(crate) seq: u64,
    pub(crate) region: Option<Region>,
}

/// A region as the prompt names it: where it is, its picture (a file the agent opens) and its note.
pub(crate) struct Mark {
    pub(crate) rect: [f64; 4],
    pub(crate) picture: Option<String>,
    pub(crate) note: String,
}

impl Mark {
    fn line(&self) -> String {
        let [x, y, w, h] = self.rect;
        let mut line = format!("x {} y {}, {}×{} px na página.", x.round(), y.round(), w.round(), h.round());
        if let Some(picture) = &self.picture {
            line += &format!(" Recorte (imagem): {picture}.");
        }
        if !self.note.is_empty() {
            line += &format!(" Nota: {}", self.note);
        }
        line
    }
}

/// What the agent reads: the kept annotations numbered, then the circled region, then the comment.
pub(crate) fn design_prompt(portal: &str, url: &str, kept: &[Mark], region: Option<&Mark>, comment: &str) -> String {
    let mut text = format!("Anotação no portal \"{portal}\" ({url})");
    for (i, mark) in kept.iter().enumerate() {
        text += &format!("\nAnotação {}: {}", i + 1, mark.line());
    }
    if let Some(mark) = region {
        text += &format!("\nRegião circulada: {}", mark.line());
    }
    format!("{text}\n---\n{comment}")
}

/// Standard base64, for a picture the page shows.
pub(crate) fn base64(bytes: &[u8]) -> String {
    const ALPHABET: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    bytes
        .chunks(3)
        .flat_map(|c| {
            let n = c.iter().enumerate().fold(0u32, |n, (i, &b)| n | (b as u32) << (16 - 8 * i));
            (0..4).map(move |i| if i <= c.len() { ALPHABET[(n >> (18 - 6 * i) & 63) as usize] as char } else { '=' })
        })
        .collect()
}

/// The picture of a region, as a data URL the page can show.
fn data_url(png: &Option<Vec<u8>>) -> Option<String> {
    png.as_ref().map(|png| format!("data:image/png;base64,{}", base64(png)))
}

/// Text the page supplied, as one short line of the prompt.
fn one_line(text: &str) -> String {
    text.split_whitespace().collect::<Vec<_>>().join(" ").chars().take(500).collect()
}

/// Saves a picture for the agent to open (in the temp folder): its path, if it was saved.
fn save_picture(png: &Option<Vec<u8>>) -> Option<String> {
    let dir = std::env::temp_dir().join("magoagent-design");
    let png = png.as_ref()?;
    fs::create_dir_all(&dir).ok()?;
    let path = dir.join(format!("{}.png", new_id()));
    fs::write(&path, png).ok()?;
    Some(path.display().to_string())
}

/// A PNG of `rect` (x, y, width, height in the page's CSS pixels) of what the page shows now;
/// `done` runs on the main thread when WebKit has it (None if it failed).
#[cfg(target_os = "macos")]
pub(crate) fn snapshot(webview: &wry::WebView, [x, y, w, h]: [f64; 4], done: impl FnOnce(Option<Vec<u8>>) + 'static) {
    use objc2_app_kit::{NSBitmapImageFileType, NSBitmapImageRep, NSImage};
    use objc2_foundation::{NSDictionary, NSError, NSPoint, NSRect, NSSize};
    use wry::WebViewExtMacOS;
    let Some(mtm) = objc2::MainThreadMarker::new() else { return done(None) };
    let view = webview.webview();
    // the rect is in the view's points: the page's CSS px times its zoom (the canvas zoom, set in sync_portals)
    let zoom = unsafe { view.pageZoom() };
    let [x, y, w, h] = [x, y, w, h].map(|v| v * zoom);
    let config = unsafe { objc2_web_kit::WKSnapshotConfiguration::new(mtm) };
    unsafe { config.setRect(NSRect::new(NSPoint::new(x.max(0.0), y.max(0.0)), NSSize::new(w.max(1.0), h.max(1.0)))) };
    let done = std::cell::Cell::new(Some(done));
    let block = block2::RcBlock::new(move |image: *mut NSImage, _: *mut NSError| {
        let png = unsafe { image.as_ref() }.and_then(|image| {
            let tiff = image.TIFFRepresentation()?;
            let rep = NSBitmapImageRep::imageRepWithData(&tiff)?;
            unsafe { rep.representationUsingType_properties(NSBitmapImageFileType::PNG, &NSDictionary::new()) }.map(|d| d.to_vec())
        });
        if let Some(done) = done.take() {
            done(png);
        }
    });
    unsafe { view.takeSnapshotWithConfiguration_completionHandler(Some(&config), &block) };
}

impl DrawingApp {
    /// The agent terminals roped to an element: the ones running a command. A plain shell would
    /// run each pasted line as a command.
    pub(crate) fn roped_agents(&self, id: &str) -> Vec<String> {
        self.links().get(id).into_iter().flatten().filter(|o| self.scene.elements.iter().any(|e| &e.id == *o && e.terminal.as_ref().is_some_and(|t| !t.command.trim().is_empty()))).cloned().collect()
    }

    /// The agents the composer offers for a portal: (terminal id, name).
    pub(crate) fn design_agents(&self, portal: &str) -> Vec<(String, String)> {
        self.roped_agents(portal)
            .into_iter()
            .map(|id| {
                let name = self.scene.elements.iter().find(|e| e.id == id).and_then(|e| e.terminal.as_ref()).map_or_else(|| "Agente".into(), |t| t.name.clone());
                (id, name)
            })
            .collect()
    }

    /// The Design button: arms it on the portal, or takes it off where it is armed.
    pub(crate) fn toggle_design(&mut self, id: &str) {
        if self.design.as_ref().is_some_and(|d| d.portal == id) {
            self.design_exit();
        } else {
            self.arm_design(id, false);
        }
    }

    /// Arms Design on a portal's open page: the overlay goes into the page with the agents and the
    /// kept annotations. `panel` opens the annotations' composer at once.
    pub(crate) fn arm_design(&mut self, id: &str, panel: bool) {
        self.design_exit();
        #[cfg(not(target_os = "linux"))]
        self.arm_tab(id, panel);
        #[cfg(target_os = "linux")]
        let _ = (id, panel);
    }

    #[cfg(not(target_os = "linux"))]
    fn arm_tab(&mut self, id: &str, panel: bool) {
        let agents: Vec<Value> = self.design_agents(id).into_iter().map(|(id, name)| json!({ "id": id, "name": name })).collect();
        let nonce = uuid::Uuid::new_v4().simple().to_string();
        let config = json!({ "nonce": nonce, "agents": agents, "annotations": self.annotation_list(id), "panel": panel });
        let script = DESIGN_SCRIPT.replace("__CONFIG__", &config.to_string());
        let uid = match self.open_tab_view_mut(id) {
            Some(tab) => {
                // ponytail: the secret lives in the page's world, so a page that hooked
                // JSON.stringify before the user sent a note there can forge notes until it
                // navigates; closing that needs a pristine post captured by an initialization
                // script at document start (per platform), the way wry installs window.ipc
                tab.secret = Some(nonce);
                let _ = tab.webview.evaluate_script(&script);
                tab.uid
            }
            None => {
                self.toast("Nenhum quadro do portal está disponível. Saia do Design e tente novamente quando o portal estiver visível.");
                return;
            }
        };
        self.design = Some(DesignSession { portal: id.into(), tab: uid, seq: 0, region: None });
    }

    /// Takes Design off the page; the kept annotations stay.
    pub(crate) fn design_exit(&mut self) {
        if let Some(design) = self.design.take() {
            self.portal_script(&design.portal, "window.__magoDesign&&window.__magoDesign.stop()");
        }
    }

    /// The kept annotations as the page shows them: region, note and a thumbnail.
    fn annotation_list(&self, portal: &str) -> Vec<Value> {
        self.annotations.get(portal).into_iter().flatten().map(|a| json!({ "rect": a.rect, "note": a.note, "thumb": data_url(&a.png) })).collect()
    }

    /// Pushes the kept annotations to the page's overlay, if it is armed on the portal.
    fn design_sync(&self, portal: &str) {
        let list = Value::Array(self.annotation_list(portal));
        self.portal_script(portal, &format!("window.__magoDesign&&window.__magoDesign.sync({list})"));
    }

    pub(crate) fn keep_annotation(&mut self, portal: &str, rect: [f64; 4], note: String, png: Option<Vec<u8>>) {
        self.annotations.entry(portal.into()).or_default().push(Annotation { rect, note, png });
    }

    /// Drops one kept annotation; the rest renumber.
    pub(crate) fn drop_annotation(&mut self, portal: &str, index: usize) {
        if let Some(kept) = self.annotations.get_mut(portal) {
            if index < kept.len() {
                kept.remove(index);
            }
            if kept.is_empty() {
                self.annotations.remove(portal);
            }
        }
    }

    pub(crate) fn clear_annotations(&mut self, portal: &str) {
        self.annotations.remove(portal);
    }

    /// A message from the overlay in a tab's page: it must come from the armed tab and carry its
    /// secret, so no other page can drive Design.
    #[cfg(not(target_os = "linux"))]
    pub(crate) fn design_message(&mut self, portal: &str, tab: u64, message: &str, ctx: &Context) {
        if message.len() > 256 * 1024 || !self.design.as_ref().is_some_and(|d| d.portal == portal && d.tab == tab) {
            return;
        }
        let Ok(msg) = serde_json::from_str::<Value>(message) else { return };
        if !self.secret_armed(portal, tab, msg["nonce"].as_str()) {
            return;
        }
        match msg["op"].as_str().unwrap_or_default() {
            "region" => {
                let points: Vec<[f64; 2]> = msg["points"].as_array().into_iter().flatten().filter_map(|p| Some([p.get(0)?.as_f64()?, p.get(1)?.as_f64()?])).collect();
                let Some(rect) = design_region(&points) else {
                    self.portal_script(portal, "window.__magoDesign&&window.__magoDesign.error()");
                    return;
                };
                let Some(design) = self.design.as_mut() else { return };
                design.seq += 1;
                design.region = Some(Region { rect, png: None });
                let seq = design.seq;
                self.portal_script(portal, &format!("window.__magoDesign&&window.__magoDesign.region({})", json!(rect)));
                // the picture of the region comes back on its own (macOS); elsewhere there is none
                #[cfg(target_os = "macos")]
                if let Some(view) = self.open_tab_view(portal) {
                    let events = self.portal_events.0.clone();
                    let (portal, ctx) = (portal.to_string(), ctx.clone());
                    snapshot(&view.webview, rect, move |png| {
                        let _ = events.send(PortalEvent::Shot { portal, seq, png });
                        ctx.request_repaint();
                    });
                    return;
                }
                let _ = ctx;
                self.portal_script(portal, "window.__magoDesign&&window.__magoDesign.preview(null)");
            }
            "keep" => {
                let note = msg["comment"].as_str().unwrap_or_default().trim().to_string();
                let Some(region) = self.design.as_mut().and_then(|d| d.region.take()) else { return };
                self.keep_annotation(portal, region.rect, note, region.png);
                self.design_sync(portal);
            }
            "drop" => {
                if let Some(index) = msg["index"].as_u64() {
                    self.drop_annotation(portal, index as usize);
                    self.design_sync(portal);
                }
            }
            "clear" => {
                self.clear_annotations(portal);
                self.design_sync(portal);
            }
            "cancel" => {
                if let Some(design) = self.design.as_mut() {
                    design.region = None;
                }
            }
            "exit" => self.design = None,
            "send" => {
                let comment = msg["comment"].as_str().unwrap_or_default().trim().to_string();
                let agent = msg["agent"].as_str().unwrap_or_default().to_string();
                self.design_send(portal, &agent, &comment);
            }
            _ => {}
        }
    }

    /// The picture of a region came back: it goes with the region, if that is still the one drawn.
    #[cfg(not(target_os = "linux"))]
    pub(crate) fn design_shot(&mut self, portal: &str, seq: u64, png: Option<Vec<u8>>) {
        let Some(design) = self.design.as_mut().filter(|d| d.portal == portal && d.seq == seq) else { return };
        let Some(region) = design.region.as_mut() else { return };
        let preview = data_url(&png);
        region.png = png;
        self.portal_script(portal, &format!("window.__magoDesign&&window.__magoDesign.preview({})", json!(preview)));
    }

    /// Send: the kept annotations and the region go to the picked agent with the comment, in one
    /// prompt with each picture; Design ends and the kept annotations are gone.
    #[cfg(not(target_os = "linux"))]
    fn design_send(&mut self, portal: &str, agent: &str, comment: &str) {
        let region = self.design.take().and_then(|d| d.region);
        if !self.roped_agents(portal).iter().any(|a| a == agent) {
            self.toast("Nenhum agente ligado ao portal: a anotação não foi enviada");
            return;
        }
        let Some(element) = self.scene.elements.iter().find(|e| e.id == portal) else { return };
        let (name, url) = element.portal.as_ref().map_or(("Portal".to_string(), String::new()), |p| (one_line(&p.name), one_line(&p.url)));
        let kept: Vec<Mark> = self.annotations.get(portal).into_iter().flatten().map(|a| Mark { rect: a.rect, picture: save_picture(&a.png), note: a.note.clone() }).collect();
        let region = region.map(|r| Mark { rect: r.rect, picture: save_picture(&r.png), note: String::new() });
        let prompt = design_prompt(&name, &url, &kept, region.as_ref(), comment);
        self.send_prompt(agent, &prompt);
        self.clear_annotations(portal);
        self.toast("Anotação enviada a 1 agente");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_stroke_becomes_its_bounding_box_and_a_tap_selects_nothing() {
        assert_eq!(design_region(&[[10.0, 10.0], [50.0, 30.0], [30.0, 70.0]]), Some([10.0, 10.0, 40.0, 60.0]));
        assert_eq!(design_region(&[[10.0, 10.0], [12.0, 11.0]]), None);
        assert_eq!(design_region(&[]), None);
    }

    #[test]
    fn annotations_renumber_when_one_is_dropped() {
        let mut app = DrawingApp::default();
        app.keep_annotation("p", [0.0, 0.0, 10.0, 10.0], "um".into(), None);
        app.keep_annotation("p", [5.0, 5.0, 10.0, 10.0], "dois".into(), None);
        app.drop_annotation("p", 0);
        assert_eq!(app.annotations["p"].iter().map(|a| a.note.as_str()).collect::<Vec<_>>(), ["dois"]);
        app.drop_annotation("p", 0);
        assert!(!app.annotations.contains_key("p"));
    }

    #[test]
    fn prompt_lists_each_annotation_then_the_region_then_the_comment() {
        let kept = [Mark { rect: [1.0, 2.0, 3.0, 4.0], picture: Some("/tmp/a.png".into()), note: "troque a cor".into() }];
        let region = Mark { rect: [9.0, 8.0, 7.0, 6.0], picture: None, note: String::new() };
        let text = design_prompt("Loja", "https://exemplo.com/conta", &kept, Some(&region), "isso");
        assert_eq!(
            text,
            "Anotação no portal \"Loja\" (https://exemplo.com/conta)\nAnotação 1: x 1 y 2, 3×4 px na página. Recorte (imagem): /tmp/a.png. Nota: troque a cor\nRegião circulada: x 9 y 8, 7×6 px na página.\n---\nisso"
        );
    }

    #[test]
    fn base64_matches_the_standard() {
        assert_eq!((base64(b"Man"), base64(b"Ma"), base64(b"M")), ("TWFu".to_string(), "TWE=".to_string(), "TQ==".to_string()));
    }
}
